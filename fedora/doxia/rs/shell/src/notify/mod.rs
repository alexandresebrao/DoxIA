// The notification server (org.freedesktop.Notifications) and its popups,
// replacing shell/plugins/notifications. Lifetimes by urgency (critical ones
// too), hover pauses the countdown, do-not-disturb with its bypasses,
// replaces_id updates in place, a repeat replaces its older copy, at most
// three popups at once, popups survive restarts. Whatever leaves the screen
// goes to the history (the last fifty), which the bar's bell lists, like
// KDE's. Same IPC (`omarchy-shell notifications <method>`).
//
// The bus name has one owner. doxia-shell queues for it and turns Quickshell's
// service off in shell.json (crate::quickshell::disable); the name
// comes here once Quickshell restarts without it. Until then the IPC target
// answers "Target not found." and omarchy-shell keeps talking to Quickshell.

mod card;
pub mod markup;
pub mod model;
mod store;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use model::{CRITICAL, Entry, LOW, NORMAL};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;
use store::Store;

const PATH: &str = "/org/freedesktop/Notifications";
const IFACE: &str = "org.freedesktop.Notifications";
const TICK_MS: i64 = 100;

const XML: &str = r#"<node>
  <interface name="org.freedesktop.Notifications">
    <method name="GetCapabilities"><arg type="as" direction="out"/></method>
    <method name="Notify">
      <arg name="app_name" type="s" direction="in"/>
      <arg name="replaces_id" type="u" direction="in"/>
      <arg name="app_icon" type="s" direction="in"/>
      <arg name="summary" type="s" direction="in"/>
      <arg name="body" type="s" direction="in"/>
      <arg name="actions" type="as" direction="in"/>
      <arg name="hints" type="a{sv}" direction="in"/>
      <arg name="expire_timeout" type="i" direction="in"/>
      <arg name="id" type="u" direction="out"/>
    </method>
    <method name="CloseNotification"><arg name="id" type="u" direction="in"/></method>
    <method name="GetServerInformation">
      <arg name="name" type="s" direction="out"/>
      <arg name="vendor" type="s" direction="out"/>
      <arg name="version" type="s" direction="out"/>
      <arg name="spec_version" type="s" direction="out"/>
    </method>
    <signal name="NotificationClosed"><arg name="id" type="u"/><arg name="reason" type="u"/></signal>
    <signal name="ActionInvoked"><arg name="id" type="u"/><arg name="action_key" type="s"/></signal>
    <signal name="ActivationToken"><arg name="id" type="u"/><arg name="activation_token" type="s"/></signal>
  </interface>
</node>"#;

// NotificationClosed reasons.
const EXPIRED: u32 = 1;
const DISMISSED: u32 = 2;
const CLOSED: u32 = 3;

struct Popup {
  entry: Entry,
  // From a previous server generation (a restart) or a history replay: its
  // id may now belong to a fresh notification, so it never matches one.
  restored: bool,
  // Whether a file under notifications/ mirrors it (replays have none).
  has_file: bool,
  has_default_action: bool,
  lifetime: u64,
  remaining: i64,
  hovered: bool,
  card: gtk::Box,
}

thread_local! {
  static INSTANCE: RefCell<std::rc::Weak<Notifications>> = const { RefCell::new(std::rc::Weak::new()) };
}

// The server, for the bar's history panel.
pub fn get() -> Option<Rc<Notifications>> {
  INSTANCE.with(|i| i.borrow().upgrade())
}

// A history entry as a card, for the history panel.
pub fn history_card(entry: &Entry) -> gtk::Box {
  card::build(entry)
}

pub struct Notifications {
  store: Store,
  // History entries newer than this have not been seen in the panel.
  seen: Cell<i64>,
  listeners: RefCell<Vec<Box<dyn Fn()>>>,
  // The bar's history panel, for showHistory.
  history_opener: RefCell<Option<Box<dyn Fn()>>>,
  dnd: Cell<bool>,
  owns_name: Cell<bool>,
  next_id: Cell<u32>,
  popups: RefCell<Vec<Popup>>,
  conn: RefCell<Option<gio::DBusConnection>>,
  window: gtk::Window,
  column: gtk::Box,
}

fn now_ms() -> i64 {
  glib::real_time() / 1000
}

impl Notifications {
  pub fn new(app: &gtk::Application) -> Rc<Notifications> {
    let window = gtk::Window::builder().application(app).decorated(false).resizable(false).build();
    window.add_css_class("doxia");
    window.add_css_class("notif-window");
    window.init_layer_shell();
    window.set_namespace(Some("doxia-notifications"));
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::None);
    window.set_exclusive_zone(-1);
    window.set_anchor(Edge::Top, true);
    window.set_anchor(Edge::Right, true);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    column.set_halign(gtk::Align::End);
    window.set_child(Some(&column));

    let store = Store::new();
    let me = Rc::new(Notifications {
      dnd: Cell::new(store.load_dnd()),
      store,
      seen: Cell::new(now_ms()),
      listeners: RefCell::default(),
      history_opener: RefCell::default(),
      owns_name: Cell::new(false),
      next_id: Cell::new(1),
      popups: RefCell::default(),
      conn: RefCell::default(),
      window,
      column,
    });
    me.place();

    let weak = Rc::downgrade(&me);
    glib::timeout_add_local(Duration::from_millis(TICK_MS as u64), move || {
      let Some(me) = weak.upgrade() else { return glib::ControlFlow::Break };
      me.tick();
      glib::ControlFlow::Continue
    });

    me.restore();
    me.store.sweep_images();
    INSTANCE.with(|i| *i.borrow_mut() = Rc::downgrade(&me));
    me
  }

  // ---- the history, for the bar ----------------------------------------------

  // Called whenever the history or do-not-disturb changes.
  pub fn connect_changed(&self, f: impl Fn() + 'static) {
    self.listeners.borrow_mut().push(Box::new(f));
  }

  fn changed(&self) {
    for f in self.listeners.borrow().iter() {
      f();
    }
  }

  pub fn set_history_opener(&self, f: impl Fn() + 'static) {
    *self.history_opener.borrow_mut() = Some(Box::new(f));
  }

  // Newest first.
  pub fn history(&self) -> Vec<Entry> {
    self.store.history()
  }

  pub fn unread(&self) -> usize {
    let seen = self.seen.get();
    self.store.history().iter().filter(|e| e.timestamp > seen).count()
  }

  pub fn mark_seen(&self) {
    self.seen.set(now_ms());
    self.changed();
  }

  pub fn forget(&self, entry: &Entry) {
    self.store.forget(entry);
    self.changed();
  }

  pub fn clear_history(&self) {
    self.store.clear_history();
    self.changed();
  }

  // A click on a history entry: what a click on its popup would have done,
  // short of the sender's action (that popup is gone); then it leaves.
  pub fn activate(&self, entry: &Entry) {
    if let Some(argv) = model::parse_exec_argv(&entry.exec_argv) {
      spawn(&argv);
    } else if !entry.app.is_empty() {
      let focus = format!("{}/bin/omarchy-mango-focus-app", std::env::var("OMARCHY_PATH").unwrap_or_default());
      spawn(&[focus, entry.app.clone()]);
    }
    self.forget(entry);
  }

  pub fn dnd(&self) -> bool {
    self.dnd.get()
  }

  // Top-right, clear of the bar when it sits on the top or right edge.
  pub fn place(&self) {
    let p = doxia_theme::placement();
    self.window.set_margin(Edge::Top, if p.bar_edge == "top" { p.clearance } else { p.gap });
    self.window.set_margin(Edge::Right, if p.bar_edge == "right" { p.clearance } else { p.gap });
  }

  // ---- D-Bus ----------------------------------------------------------------

  pub fn register(self: &Rc<Self>, conn: &gio::DBusConnection) {
    *self.conn.borrow_mut() = Some(conn.clone());
    let info = gio::DBusNodeInfo::for_xml(XML).expect("notifications xml");
    let iface = info.lookup_interface(IFACE).expect("notifications interface");
    let me = self.clone();
    let registered = conn
      .register_object(PATH, &iface)
      .method_call(move |_conn, _sender, _path, _iface, method, params, invocation| {
        let reply: Option<glib::Variant> = match method {
          "GetCapabilities" => Some(
            (vec!["actions", "body", "body-markup", "body-hyperlinks", "icon-static", "persistence"],).to_variant(),
          ),
          "GetServerInformation" => Some(("doxia-shell", "DoxIA", env!("CARGO_PKG_VERSION"), "1.2").to_variant()),
          "Notify" => me.on_notify(&params).map(|id| (id,).to_variant()),
          "CloseNotification" => {
            if let Some((id,)) = params.get::<(u32,)>() {
              me.close_by_id(id);
            }
            Some(().to_variant())
          }
          _ => None,
        };
        match reply {
          Some(v) => invocation.return_value(Some(&v)),
          None => invocation.return_dbus_error("org.freedesktop.DBus.Error.InvalidArgs", method),
        }
      })
      .build();
    if let Err(e) = registered {
      glib::g_warning!("doxia-shell", "notifications object not registered: {e}");
      return;
    }

    // Queued (no DO_NOT_QUEUE): while Quickshell holds the name we wait in
    // line, and get it the moment its service is turned off.
    let acquired = self.clone();
    let lost = self.clone();
    let id = gio::bus_own_name_on_connection(
      conn,
      IFACE,
      gio::BusNameOwnerFlags::NONE,
      move |_, _| acquired.owns_name.set(true),
      move |_, _| lost.owns_name.set(false),
    );
    std::mem::forget(id);
    crate::quickshell::disable("omarchy.notifications");

    // A Quickshell that ever ran its notification service keeps the name for
    // as long as it runs, plugin unloaded or not, and drops what it receives:
    // notifications would vanish until the next login. apply-bar keeps the
    // service off so this is rare; when it happens anyway, restart Quickshell
    // once, and only when the session is certainly unlocked
    // (omarchy-restart-shell re-locks a session it thinks is locked).
    let me = self.clone();
    let conn = conn.clone();
    glib::timeout_add_seconds_local_once(10, move || {
      if me.owns_name.get() {
        return;
      }
      glib::spawn_future_local(async move {
        if name_owner_comm(&conn, IFACE).await.as_deref() == Some("quickshell") && session_unlocked() {
          glib::g_message!("doxia-shell", "notifications: Quickshell holds the bus name; restarting it");
          spawn(&["omarchy-restart-shell".to_string()]);
        }
      });
    });
  }

  fn emit(&self, signal: &str, args: glib::Variant) {
    if let Some(conn) = self.conn.borrow().as_ref() {
      let _ = conn.emit_signal(None, PATH, IFACE, signal, Some(&args));
    }
  }

  fn on_notify(self: &Rc<Self>, params: &glib::Variant) -> Option<u32> {
    let (app, replaces, app_icon, summary, body, actions, hints, timeout) =
      params.get::<(String, u32, String, String, String, Vec<String>, HashMap<String, glib::Variant>, i32)>()?;
    let hint_str = |k: &str| hints.get(k).and_then(|v| v.str().map(String::from)).unwrap_or_default();
    let urgency = hints.get("urgency").and_then(|v| v.get::<u8>()).map(|u| u.min(CRITICAL)).unwrap_or(NORMAL);
    let transient = hints.get("transient").and_then(|v| v.get::<bool>()).unwrap_or(false);
    let has_default_action = actions.chunks(2).any(|pair| pair[0] == "default");

    // A replaces_id that names a live (not restored) popup keeps its identity.
    let replacing = replaces > 0
      && self.popups.borrow().iter().any(|p| p.entry.original_id == replaces && !p.restored);
    let id = if replacing {
      replaces
    } else {
      let id = self.next_id.get();
      self.next_id.set(id.wrapping_add(1).max(1));
      id
    };
    let timestamp = if replacing {
      self.popups.borrow().iter().find(|p| p.entry.original_id == id && !p.restored).map(|p| p.entry.timestamp).unwrap()
    } else {
      now_ms()
    };

    let mut entry = Entry {
      id,
      original_id: id,
      app,
      app_icon,
      summary,
      body,
      image: String::new(),
      glyph: hint_str("omarchy-glyph"),
      exec_argv: hint_str("omarchy-exec-argv"),
      urgency,
      expire_timeout: timeout.max(0) as f64,
      timestamp,
      deadline: None,
    };
    entry.image = self.image_from_hints(&hints, &entry);
    // No icon of its own: the one of the app that sent it.
    if entry.image.is_empty() && card::icon_widget(&entry.app_icon).is_none() {
      if let Some(icon) = desktop_icon(&hint_str("desktop-entry"), &entry.app) {
        entry.app_icon = icon;
      }
    }

    if self.dnd.get() && !model::bypasses_dnd(&entry.app, urgency) {
      // Never on screen, so history is the only trace it can leave.
      if !transient && !model::is_ephemeral_app(&entry.app) {
        self.store.write_history(&entry);
        self.changed();
      }
      self.emit("NotificationClosed", (id, DISMISSED).to_variant());
      return Some(id);
    }

    let lifetime = model::duration_for(urgency, entry.expire_timeout);
    let persisted = self.store.write_popup(&entry);
    if replacing {
      let mut popups = self.popups.borrow_mut();
      if let Some(p) = popups.iter_mut().find(|p| p.entry.original_id == id && !p.restored) {
        if !p.entry.same_content(&persisted) {
          // New text deserves a full look.
          p.remaining = lifetime as i64;
        }
        p.lifetime = lifetime;
        p.entry = persisted;
        p.has_default_action = has_default_action;
        let new_card = self.card_for(&p.entry);
        self.column.insert_child_after(&new_card, Some(&p.card));
        self.column.remove(&p.card);
        p.card = new_card;
      }
    } else {
      // A repeat (same sender, same text) takes the older copy's place.
      let repeats: Vec<usize> = self
        .popups
        .borrow()
        .iter()
        .enumerate()
        .filter(|(_, p)| p.entry.app == persisted.app && p.entry.summary == persisted.summary && p.entry.body == persisted.body)
        .map(|(i, _)| i)
        .collect();
      for i in repeats.into_iter().rev() {
        self.drop_repeat(i);
      }
      self.insert(0, persisted, false, true, has_default_action);
    }
    Some(id)
  }

  // image-data / image-path / icon_data hints: in-memory pixels are saved as
  // a PNG under the entry's image name, which is also the persisted copy.
  fn image_from_hints(&self, hints: &HashMap<String, glib::Variant>, entry: &Entry) -> String {
    for key in ["image-path", "image_path"] {
      if let Some(path) = hints.get(key).and_then(|v| v.str().map(String::from)).filter(|p| !p.is_empty()) {
        return path;
      }
    }
    for key in ["image-data", "image_data", "icon_data"] {
      let Some(v) = hints.get(key) else { continue };
      let Some((w, h, stride, alpha, _bits, channels, data)) = v.get::<(i32, i32, i32, bool, i32, i32, Vec<u8>)>() else {
        continue;
      };
      if w <= 0 || h <= 0 || stride <= 0 || data.len() < (stride * (h - 1) + w * channels) as usize {
        continue;
      }
      let format = if alpha { gdk::MemoryFormat::R8g8b8a8 } else { gdk::MemoryFormat::R8g8b8 };
      let texture = gdk::MemoryTexture::new(w, h, format, &glib::Bytes::from_owned(data), stride as usize);
      let path = self.store.images_dir().join(format!("{}-image", entry.stem()));
      if texture.save_to_png(&path).is_ok() {
        return format!("file://{}", path.display());
      }
    }
    String::new()
  }

  // ---- popups ---------------------------------------------------------------

  fn insert(self: &Rc<Self>, at: usize, entry: Entry, restored: bool, has_file: bool, has_default_action: bool) {
    let lifetime = model::duration_for(entry.urgency, entry.expire_timeout);
    let card = self.card_for(&entry);
    {
      let popups = self.popups.borrow();
      match at.checked_sub(1).and_then(|i| popups.get(i)) {
        Some(before) => self.column.insert_child_after(&card, Some(&before.card)),
        None => self.column.prepend(&card),
      }
    }
    let popup = Popup { entry, restored, has_file, has_default_action, lifetime, remaining: lifetime as i64, hovered: false, card };
    let mut popups = self.popups.borrow_mut();
    let at = at.min(popups.len());
    popups.insert(at, popup);
    let excess = popups.len().saturating_sub(model::MAX_VISIBLE);
    drop(popups);
    self.window.set_visible(true);
    // The oldest go to history, as if they had expired.
    for _ in 0..excess {
      let last = self.popups.borrow().len() - 1;
      self.remove(last, EXPIRED);
    }
  }

  fn drop_repeat(&self, i: usize) {
    let popup = self.popups.borrow_mut().remove(i);
    self.column.remove(&popup.card);
    if popup.has_file {
      self.store.discard(&popup.entry);
    }
    if !popup.restored {
      self.emit("NotificationClosed", (popup.entry.original_id, CLOSED).to_variant());
    }
  }

  fn card_for(self: &Rc<Self>, entry: &Entry) -> gtk::Box {
    let card = card::build(entry);
    let stamp = (entry.timestamp, entry.original_id);
    let weak = Rc::downgrade(self);
    let click = gtk::GestureClick::new();
    click.set_button(0);
    click.connect_released(move |g, _, _, _| {
      let Some(me) = weak.upgrade() else { return };
      let Some(i) = me.index_of(stamp) else { return };
      if g.current_button() == gdk::BUTTON_SECONDARY {
        me.remove(i, DISMISSED);
      } else {
        me.invoke(i);
      }
    });
    card.add_controller(click);
    let motion = gtk::EventControllerMotion::new();
    for entering in [true, false] {
      let weak = Rc::downgrade(self);
      let f = move || {
        if let Some(me) = weak.upgrade() {
          if let Some(i) = me.index_of(stamp) {
            me.popups.borrow_mut()[i].hovered = entering;
          }
        }
      };
      if entering {
        motion.connect_enter(move |_, _, _| f());
      } else {
        motion.connect_leave(move |_| f());
      }
    }
    card.add_controller(motion);
    card
  }

  fn index_of(&self, stamp: (i64, u32)) -> Option<usize> {
    self.popups.borrow().iter().position(|p| (p.entry.timestamp, p.entry.original_id) == stamp)
  }

  fn tick(self: &Rc<Self>) {
    let expired: Vec<usize> = {
      let mut popups = self.popups.borrow_mut();
      popups
        .iter_mut()
        .enumerate()
        .filter(|(_, p)| p.lifetime > 0 && !p.hovered)
        .filter_map(|(i, p)| {
          p.remaining -= TICK_MS;
          (p.remaining <= 0).then_some(i)
        })
        .collect()
    };
    for i in expired.into_iter().rev() {
      self.remove(i, EXPIRED);
    }
  }

  // Off the screen for any reason: the file becomes history, the sender hears
  // why (unless the popup belongs to an earlier server).
  fn remove(&self, i: usize, reason: u32) {
    let popup = {
      let mut popups = self.popups.borrow_mut();
      if i >= popups.len() {
        return;
      }
      popups.remove(i)
    };
    self.column.remove(&popup.card);
    if popup.has_file {
      self.store.archive(&popup.entry);
      self.changed();
    }
    if !popup.restored {
      self.emit("NotificationClosed", (popup.entry.original_id, reason).to_variant());
    }
    if self.popups.borrow().is_empty() {
      self.window.set_visible(false);
    }
  }

  fn close_by_id(&self, id: u32) {
    let i = self.popups.borrow().iter().position(|p| p.entry.original_id == id && !p.restored);
    if let Some(i) = i {
      self.remove(i, CLOSED);
    }
  }

  // Omarchy's own toasts carry their click as an argv; other senders get
  // their "default" action, or their window focused.
  fn invoke(&self, i: usize) {
    let (entry, restored, has_default) = {
      let popups = self.popups.borrow();
      let Some(p) = popups.get(i) else { return };
      (p.entry.clone(), p.restored, p.has_default_action)
    };
    if let Some(argv) = model::parse_exec_argv(&entry.exec_argv) {
      spawn(&argv);
    } else if !restored && has_default {
      // Without an xdg-activation token the sender may not raise its window
      // (Chrome, kitty): mango keeps the focus where it is, on another tag.
      // The spec has the token go first, then the action.
      if let Some(token) = activation_token() {
        self.emit("ActivationToken", (entry.original_id, token).to_variant());
      }
      self.emit("ActionInvoked", (entry.original_id, "default").to_variant());
    } else if !entry.app.is_empty() {
      let focus = format!("{}/bin/omarchy-mango-focus-app", std::env::var("OMARCHY_PATH").unwrap_or_default());
      spawn(&[focus, entry.app.clone()]);
    }
    self.remove(i, DISMISSED);
  }

  // Popups that were on screen when the previous shell stopped; the expired
  // ones go to history as if they had expired on screen. Survivors start a
  // full lifetime again, recorded as a deadline in case of another restart.
  fn restore(self: &Rc<Self>) {
    let now = now_ms();
    for mut entry in self.store.popups() {
      let duration = model::duration_for(entry.urgency, entry.expire_timeout);
      if model::popup_expired(&entry, duration, now) {
        self.store.archive(&entry);
        continue;
      }
      if duration > 0 {
        entry.deadline = Some(now + duration as i64);
        self.store.write_popup(&entry);
      }
      let at = self.popups.borrow().len();
      self.insert(at, entry, true, true, false);
    }
  }

  // ---- IPC (omarchy-shell notifications <method>) ---------------------------

  pub fn set_dnd(&self, on: bool) -> String {
    self.dnd.set(on);
    self.store.save_dnd(on);
    self.changed();
    self.dnd_state()
  }

  fn dnd_state(&self) -> String {
    if self.dnd.get() { "on".into() } else { "off".into() }
  }

  fn dismiss_all(&self) {
    while !self.popups.borrow().is_empty() {
      self.remove(0, DISMISSED);
    }
  }

  // The archived files, plus whatever is on screen now, as toasts again.
  fn show_history(self: &Rc<Self>) {
    let live: Vec<Entry> =
      self.popups.borrow().iter().filter(|p| p.entry.original_id > 0).map(|p| self.store.persistable(&p.entry)).collect();
    self.dismiss_all();
    let rows = model::history_rows(&self.store.history(), &live, model::HISTORY_LIMIT);
    if rows.is_empty() {
      let placeholder = Entry {
        app: "omarchy-action".into(),
        summary: "No recent notifications".into(),
        glyph: "\u{f009a}".into(),
        urgency: LOW,
        timestamp: now_ms(),
        ..Default::default()
      };
      self.insert(0, placeholder, true, false, false);
      return;
    }
    for row in rows {
      let at = self.popups.borrow().len();
      self.insert(at, row, true, false, false);
    }
  }

  pub fn ipc(self: &Rc<Self>, method: &str, args: &[String]) -> Option<String> {
    // Until the bus name is ours, Quickshell is the server: let it answer.
    if !self.owns_name.get() {
      return Some("Target not found.".into());
    }
    let arg = args.first().map(String::as_str).unwrap_or("");
    Some(match method {
      "ping" => "ok".into(),
      "dndState" | "isDnd" => self.dnd_state(),
      "toggleDnd" => self.set_dnd(!self.dnd.get()),
      "setDnd" => self.set_dnd(matches!(arg.to_lowercase().as_str(), "true" | "1" | "on" | "yes")),
      "showHistory" => {
        match self.history_opener.borrow().as_ref() {
          Some(open) => open(),
          None => self.show_history(),
        }
        "ok".into()
      }
      "clear" => {
        self.clear_history();
        "ok".into()
      }
      "dismissAll" => {
        self.dismiss_all();
        "ok".into()
      }
      "dismissOne" => {
        if self.popups.borrow().is_empty() {
          return Some("none".into());
        }
        self.remove(0, DISMISSED);
        "ok".into()
      }
      "invokeLast" => {
        if self.popups.borrow().is_empty() {
          return Some("none".into());
        }
        self.invoke(0);
        "ok".into()
      }
      "dismiss" => {
        if arg.is_empty() {
          return Some("none".into());
        }
        let hits: Vec<usize> = self
          .popups
          .borrow()
          .iter()
          .enumerate()
          .filter(|(_, p)| p.entry.summary.contains(arg))
          .map(|(i, _)| i)
          .collect();
        for &i in hits.iter().rev() {
          self.remove(i, DISMISSED);
        }
        if hits.is_empty() { "none".into() } else { "ok".into() }
      }
      _ => return None,
    })
  }
}

// An xdg-activation token from the click just made on the card (GDK asks
// the compositor with that input's serial).
fn activation_token() -> Option<String> {
  let ctx = gdk::Display::default()?.app_launch_context();
  ctx.startup_notify_id(None::<&gio::AppInfo>, &[]).map(|t| t.to_string())
}

// Detached, argv as given (no shell).
fn spawn(argv: &[String]) {
  use std::os::unix::process::CommandExt;
  let Some((program, args)) = argv.split_first() else { return };
  let _ = std::process::Command::new(program)
    .args(args)
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .process_group(0)
    .spawn()
    .map(|mut c| std::thread::spawn(move || c.wait()));
}

// The process name of a bus name's owner.
async fn name_owner_comm(conn: &gio::DBusConnection, name: &str) -> Option<String> {
  let reply = conn
    .call_future(
      Some("org.freedesktop.DBus"),
      "/org/freedesktop/DBus",
      "org.freedesktop.DBus",
      "GetConnectionUnixProcessID",
      Some(&(name,).to_variant()),
      None,
      gio::DBusCallFlags::NONE,
      2000,
    )
    .await
    .ok()?;
  let (pid,) = reply.get::<(u32,)>()?;
  std::fs::read_to_string(format!("/proc/{pid}/comm")).ok().map(|c| c.trim().to_string())
}

// omarchy-mango-session-locked exits 1 for unlocked, 0 locked, 2 unknown.
fn session_unlocked() -> bool {
  std::process::Command::new("omarchy-mango-session-locked")
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .status()
    .is_ok_and(|s| s.code() == Some(1))
}

// The sender's icon from its .desktop file: the desktop-entry hint (what
// Flatpak apps send through the portal, with no app name), else the app name
// as a desktop id or as the app's display name. A path or a themed name.
fn desktop_icon(desktop_entry: &str, app: &str) -> Option<String> {
  let strip = |id: &str| id.trim_end_matches(".desktop").to_lowercase();
  let (entry, app) = (strip(desktop_entry), app.to_lowercase());
  let all = gio::AppInfo::all();
  let id_is = |want: &str| all.iter().find(|a| !want.is_empty() && a.id().is_some_and(|id| strip(&id) == want));
  let info = id_is(&entry).or_else(|| id_is(&app)).or_else(|| all.iter().find(|a| !app.is_empty() && a.name().to_lowercase() == app))?;
  let icon = info.icon()?;
  if let Some(file) = icon.downcast_ref::<gio::FileIcon>() {
    return file.file().path().map(|p| p.to_string_lossy().into_owned());
  }
  let theme = gtk::IconTheme::for_display(&gdk::Display::default()?);
  icon.downcast_ref::<gio::ThemedIcon>()?.names().into_iter().find(|n| theme.has_icon(n)).map(String::from)
}
