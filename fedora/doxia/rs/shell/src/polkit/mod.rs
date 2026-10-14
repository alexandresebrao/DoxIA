// The session's polkit authentication agent, replacing shell/plugins/polkit:
// the same password card over a scrim, the same "Autorizar a execução de"
// label and fingerprint mode. polkitd calls BeginAuthentication here; the
// password goes to polkit's setuid helper (polkit-agent-helper-1, the one
// libpolkit-agent falls back to when its socket is off, as on Fedora) over
// its line protocol: the cookie on stdin, then PAM_PROMPT_ECHO_OFF/ON,
// PAM_ERROR_MSG and PAM_TEXT_INFO lines answered one per line, then SUCCESS
// or FAILURE. A failed password starts a new helper, so the card stays up
// until it succeeds or is cancelled (Escape, or polkitd's
// CancelAuthentication).
//
// One agent per session: doxia-shell turns Quickshell's off in shell.json and
// retries the registration until polkitd takes it.

mod model;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::time::Duration;

const PATH: &str = "/org/doxia/PolkitAgent";
const IFACE: &str = "org.freedesktop.PolicyKit1.AuthenticationAgent";
const HELPER: &str = "/usr/lib/polkit-1/polkit-agent-helper-1";
const G_LOCK: &str = "\u{f023}";
const G_FINGERPRINT: &str = "\u{f0237}";

const XML: &str = r#"<node>
  <interface name="org.freedesktop.PolicyKit1.AuthenticationAgent">
    <method name="BeginAuthentication">
      <arg name="action_id" type="s" direction="in"/>
      <arg name="message" type="s" direction="in"/>
      <arg name="icon_name" type="s" direction="in"/>
      <arg name="details" type="a{ss}" direction="in"/>
      <arg name="cookie" type="s" direction="in"/>
      <arg name="identities" type="a(sa{sv})" direction="in"/>
    </method>
    <method name="CancelAuthentication">
      <arg name="cookie" type="s" direction="in"/>
    </method>
  </interface>
</node>"#;

struct Request {
  cookie: String,
  message: String,
  user: String,
  invocation: gio::DBusMethodInvocation,
}

struct Helper {
  process: gio::Subprocess,
  stdin: gio::OutputStream,
}

pub struct Polkit {
  window: gtk::Window,
  label: gtk::Label,
  card: gtk::Box,
  glyph: gtk::Label,
  entry: gtk::PasswordEntry,
  current: RefCell<Option<Request>>,
  queue: RefCell<VecDeque<Request>>,
  helper: RefCell<Option<Helper>>,
  // Bumped per helper, so a dead helper's late lines are ignored.
  generation: Cell<u64>,
  response_required: Cell<bool>,
  submitted: Cell<bool>,
  fingerprint: Cell<bool>,
  registered: Cell<bool>,
}

fn user_name(uid: u32) -> Option<String> {
  std::fs::read_to_string("/etc/passwd").ok()?.lines().find_map(|line| {
    let f: Vec<&str> = line.split(':').collect();
    (f.len() > 2 && f[2].parse::<u32>().ok() == Some(uid)).then(|| f[0].to_string())
  })
}

fn own_uid() -> u32 {
  use std::os::unix::fs::MetadataExt;
  std::fs::metadata("/proc/self").map(|m| m.uid()).unwrap_or(0)
}

impl Polkit {
  pub fn new(app: &gtk::Application) -> Rc<Polkit> {
    let window = gtk::Window::builder().application(app).decorated(false).build();
    window.add_css_class("doxia");
    window.add_css_class("polkit-window");
    window.init_layer_shell();
    window.set_namespace(Some("doxia-polkit"));
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::Exclusive);
    window.set_exclusive_zone(-1);
    for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
      window.set_anchor(edge, true);
    }

    let column = gtk::Box::new(gtk::Orientation::Vertical, 10);
    column.set_halign(gtk::Align::Center);
    column.set_valign(gtk::Align::Center);
    let label = gtk::Label::new(None);
    label.add_css_class("polkit-label");
    label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    label.set_max_width_chars(48);
    label.set_halign(gtk::Align::Center);
    column.append(&label);

    let card = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    card.add_css_class("polkit-card");
    card.set_halign(gtk::Align::Center);
    let glyph = gtk::Label::new(Some(G_LOCK));
    glyph.add_css_class("polkit-glyph");
    glyph.set_width_request(26);
    card.append(&glyph);
    let entry = gtk::PasswordEntry::new();
    entry.add_css_class("polkit-entry");
    entry.set_hexpand(true);
    entry.set_width_request(240);
    card.append(&entry);
    column.append(&card);
    window.set_child(Some(&column));

    let me = Rc::new(Polkit {
      window,
      label,
      card,
      glyph,
      entry,
      current: RefCell::default(),
      queue: RefCell::default(),
      helper: RefCell::default(),
      generation: Cell::new(0),
      response_required: Cell::new(false),
      submitted: Cell::new(false),
      fingerprint: Cell::new(false),
      registered: Cell::new(false),
    });

    let weak = Rc::downgrade(&me);
    me.entry.connect_activate(move |_| {
      if let Some(me) = weak.upgrade() {
        me.submit();
      }
    });
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(&me);
    keys.connect_key_pressed(move |_, key, _, _| {
      let Some(me) = weak.upgrade() else { return glib::Propagation::Proceed };
      if key == gdk::Key::Escape {
        me.cancel_current("org.freedesktop.PolicyKit1.Error.Cancelled", "Cancelado");
        return glib::Propagation::Stop;
      }
      glib::Propagation::Proceed
    });
    me.window.add_controller(keys);
    // A click on the scrim gives the field its focus back.
    let click = gtk::GestureClick::new();
    let weak = Rc::downgrade(&me);
    click.connect_released(move |_, _, _, _| {
      if let Some(me) = weak.upgrade() {
        me.entry.grab_focus();
      }
    });
    me.window.add_controller(click);
    me
  }

  // ---- registration ---------------------------------------------------------

  pub fn start(self: &Rc<Self>) {
    let me = self.clone();
    glib::spawn_future_local(async move {
      let conn = match gio::bus_get_future(gio::BusType::System).await {
        Ok(c) => c,
        Err(e) => {
          glib::g_warning!("doxia-shell", "polkit: no system bus: {e}");
          return;
        }
      };
      if !me.export(&conn) {
        return;
      }
      crate::quickshell::disable("omarchy.polkit");
      // polkitd forgets its agents when it restarts (it is bus-activated and
      // can crash): register again every time it appears.
      let appeared = me.clone();
      let vanished = me.clone();
      let watch = gio::bus_watch_name_on_connection(
        &conn,
        "org.freedesktop.PolicyKit1",
        gio::BusNameWatcherFlags::AUTO_START,
        move |conn, _, _| {
          let me = appeared.clone();
          glib::spawn_future_local(async move { me.register_until_accepted(&conn).await });
        },
        move |_, _| vanished.registered.set(false),
      );
      std::mem::forget(watch);
    });
  }

  // Quickshell's agent may still hold the session: keep trying.
  async fn register_until_accepted(&self, conn: &gio::DBusConnection) {
    if self.registered.get() {
      return;
    }
    while !self.register(conn).await {
      glib::timeout_future(Duration::from_secs(3)).await;
    }
    self.registered.set(true);
  }

  fn export(self: &Rc<Self>, conn: &gio::DBusConnection) -> bool {
    let info = gio::DBusNodeInfo::for_xml(XML).expect("polkit xml");
    let iface = info.lookup_interface(IFACE).expect("polkit interface");
    let me = self.clone();
    conn
      .register_object(PATH, &iface)
      .method_call(move |_conn, _sender, _path, _iface, method, params, invocation| match method {
        "BeginAuthentication" => me.begin(&params, invocation),
        "CancelAuthentication" => {
          if let Some((cookie,)) = params.get::<(String,)>() {
            me.cancel_cookie(&cookie);
          }
          invocation.return_value(None);
        }
        _ => invocation.return_dbus_error("org.freedesktop.DBus.Error.UnknownMethod", method),
      })
      .build()
      .map_err(|e| glib::g_warning!("doxia-shell", "polkit: agent not exported: {e}"))
      .is_ok()
  }

  async fn session_id(conn: &gio::DBusConnection) -> Option<String> {
    if let Some(id) = std::env::var("XDG_SESSION_ID").ok().filter(|s| !s.is_empty()) {
      return Some(id);
    }
    // A user unit runs outside the session: ask logind for the user's
    // graphical (display) session.
    let user = conn
      .call_future(
        Some("org.freedesktop.login1"),
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
        "GetUser",
        Some(&(own_uid(),).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        5000,
      )
      .await
      .ok()?;
    let (path,) = user.get::<(glib::variant::ObjectPath,)>()?;
    let display = conn
      .call_future(
        Some("org.freedesktop.login1"),
        path.as_str(),
        "org.freedesktop.DBus.Properties",
        "Get",
        Some(&("org.freedesktop.login1.User", "Display").to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        5000,
      )
      .await
      .ok()?;
    let (value,) = display.get::<(glib::Variant,)>()?;
    let (id, _) = value.as_variant()?.get::<(String, glib::variant::ObjectPath)>()?;
    (!id.is_empty()).then_some(id)
  }

  async fn register(&self, conn: &gio::DBusConnection) -> bool {
    let Some(session) = Self::session_id(conn).await else {
      glib::g_warning!("doxia-shell", "polkit: no login session found");
      return false;
    };
    let mut details = HashMap::new();
    details.insert("session-id".to_string(), session.to_variant());
    let subject = ("unix-session".to_string(), details);
    let locale = std::env::var("LANG").unwrap_or_else(|_| "C".into());
    let result = conn
      .call_future(
        Some("org.freedesktop.PolicyKit1"),
        "/org/freedesktop/PolicyKit1/Authority",
        "org.freedesktop.PolicyKit1.Authority",
        "RegisterAuthenticationAgent",
        Some(&(subject, locale, PATH).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        5000,
      )
      .await;
    match result {
      Ok(_) => {
        glib::g_message!("doxia-shell", "polkit: agent registered for session {session}");
        true
      }
      Err(e) => {
        glib::g_message!("doxia-shell", "polkit: registration refused, retrying: {e}");
        false
      }
    }
  }

  // ---- requests -------------------------------------------------------------

  fn begin(self: &Rc<Self>, params: &glib::Variant, invocation: gio::DBusMethodInvocation) {
    type Identity = (String, HashMap<String, glib::Variant>);
    let Some((_action, message, _icon, _details, cookie, identities)) =
      params.get::<(String, String, String, HashMap<String, String>, String, Vec<Identity>)>()
    else {
      invocation.return_dbus_error("org.freedesktop.DBus.Error.InvalidArgs", "BeginAuthentication");
      return;
    };
    // Ourselves when allowed (the usual wheel case), else the first user.
    let uids: Vec<u32> = identities
      .iter()
      .filter(|(kind, _)| kind == "unix-user")
      .filter_map(|(_, d)| d.get("uid").and_then(|v| v.get::<u32>()))
      .collect();
    let me_uid = own_uid();
    let uid = uids.iter().copied().find(|u| *u == me_uid).or_else(|| uids.first().copied());
    let Some(user) = uid.and_then(user_name) else {
      invocation.return_dbus_error("org.freedesktop.PolicyKit1.Error.Failed", "No user to authenticate as");
      return;
    };
    self.queue.borrow_mut().push_back(Request { cookie, message, user, invocation });
    if self.current.borrow().is_none() {
      self.next();
    }
  }

  fn next(self: &Rc<Self>) {
    let Some(request) = self.queue.borrow_mut().pop_front() else {
      self.window.set_visible(false);
      return;
    };
    self.label.set_text(&model::authorization_label(&request.message));
    *self.current.borrow_mut() = Some(request);
    self.fingerprint.set(
      model::fingerprint_configured(&std::fs::read_to_string("/etc/pam.d/polkit-1").unwrap_or_default())
        && !lid_closed(),
    );
    self.start_helper();
    self.window.set_visible(true);
    self.entry.grab_focus();
  }

  fn start_helper(self: &Rc<Self>) {
    self.stop_helper();
    let (user, cookie) = match self.current.borrow().as_ref() {
      Some(r) => (r.user.clone(), r.cookie.clone()),
      None => return,
    };
    self.response_required.set(false);
    self.submitted.set(false);
    self.render(None);
    let argv = [std::ffi::OsStr::new(HELPER), std::ffi::OsStr::new(&user)];
    let process = match gio::Subprocess::newv(&argv, gio::SubprocessFlags::STDIN_PIPE | gio::SubprocessFlags::STDOUT_PIPE) {
      Ok(p) => p,
      Err(e) => {
        self.finish(Err(format!("{HELPER}: {e}")));
        return;
      }
    };
    let (Some(stdin), Some(stdout)) = (process.stdin_pipe(), process.stdout_pipe()) else { return };
    let generation = self.generation.get() + 1;
    self.generation.set(generation);
    *self.helper.borrow_mut() = Some(Helper { process, stdin: stdin.clone() });

    let me = self.clone();
    glib::spawn_future_local(async move {
      let _ = stdin.write_all_future(format!("{cookie}\n").into_bytes(), glib::Priority::DEFAULT).await;
      let lines = gio::DataInputStream::new(&stdout);
      loop {
        let line = lines.read_line_utf8_future(glib::Priority::DEFAULT).await;
        if me.generation.get() != generation {
          return;
        }
        match line {
          Ok(Some(line)) => {
            if me.on_helper_line(line.trim_end()) {
              return;
            }
          }
          // The helper ended without a verdict.
          _ => {
            me.failed();
            return;
          }
        }
      }
    });
  }

  fn stop_helper(&self) {
    self.generation.set(self.generation.get() + 1);
    if let Some(h) = self.helper.borrow_mut().take() {
      h.process.force_exit();
    }
  }

  // True when this line ends the helper's conversation.
  fn on_helper_line(self: &Rc<Self>, line: &str) -> bool {
    let (kind, text) = line.split_once(' ').unwrap_or((line, ""));
    match kind {
      "PAM_PROMPT_ECHO_OFF" | "PAM_PROMPT_ECHO_ON" => {
        self.response_required.set(true);
        self.submitted.set(false);
        self.entry.set_show_peek_icon(kind == "PAM_PROMPT_ECHO_ON");
        // A password prompt means PAM gave up on the reader (or skipped it).
        if !model::looks_fingerprint(text) {
          self.fingerprint.set(false);
        }
        self.render(None);
        self.entry.grab_focus();
        false
      }
      "PAM_ERROR_MSG" | "PAM_TEXT_INFO" => {
        if !text.is_empty() && !model::looks_fingerprint(text) {
          self.entry.set_placeholder_text(Some(text));
        }
        false
      }
      "SUCCESS" => {
        self.finish(Ok(()));
        true
      }
      "FAILURE" => {
        self.failed();
        true
      }
      _ => false,
    }
  }

  fn submit(self: &Rc<Self>) {
    if !self.response_required.get() || self.submitted.get() {
      return;
    }
    let password = self.entry.text().to_string();
    self.entry.set_text("");
    self.submitted.set(true);
    self.response_required.set(false);
    self.render(None);
    let Some(stdin) = self.helper.borrow().as_ref().map(|h| h.stdin.clone()) else { return };
    glib::spawn_future_local(async move {
      let _ = stdin.write_all_future(format!("{password}\n").into_bytes(), glib::Priority::DEFAULT).await;
    });
  }

  // Wrong password: say so for a moment, then a fresh helper asks again.
  fn failed(self: &Rc<Self>) {
    self.stop_helper();
    if self.current.borrow().is_none() {
      return;
    }
    self.render(Some("Senha incorreta"));
    let me = self.clone();
    glib::timeout_add_local_once(Duration::from_millis(1200), move || {
      if me.current.borrow().is_some() {
        me.start_helper();
      }
    });
  }

  fn finish(self: &Rc<Self>, result: Result<(), String>) {
    self.stop_helper();
    if let Some(request) = self.current.borrow_mut().take() {
      match result {
        Ok(()) => request.invocation.return_value(None),
        Err(e) => request.invocation.return_dbus_error("org.freedesktop.PolicyKit1.Error.Failed", &e),
      }
    }
    self.entry.set_text("");
    self.next();
  }

  fn cancel_current(self: &Rc<Self>, error: &str, text: &str) {
    self.stop_helper();
    if let Some(request) = self.current.borrow_mut().take() {
      request.invocation.return_dbus_error(error, text);
    }
    self.entry.set_text("");
    self.next();
  }

  fn cancel_cookie(self: &Rc<Self>, cookie: &str) {
    let is_current = self.current.borrow().as_ref().is_some_and(|r| r.cookie == cookie);
    if is_current {
      self.cancel_current("org.freedesktop.PolicyKit1.Error.Cancelled", "Cancelado pelo polkit");
      return;
    }
    let mut queue = self.queue.borrow_mut();
    if let Some(i) = queue.iter().position(|r| r.cookie == cookie) {
      if let Some(r) = queue.remove(i) {
        r.invocation.return_dbus_error("org.freedesktop.PolicyKit1.Error.Cancelled", "Cancelado pelo polkit");
      }
    }
  }

  // The card: the fingerprint glyph alone while PAM waits on the reader,
  // else the lock and the password field; red while an error shows.
  fn render(&self, error: Option<&str>) {
    let fingerprint = self.fingerprint.get() && !self.response_required.get() && !self.submitted.get() && error.is_none();
    self.glyph.set_text(if fingerprint { G_FINGERPRINT } else { G_LOCK });
    self.entry.set_visible(!fingerprint);
    if fingerprint {
      self.card.add_css_class("fingerprint");
    } else {
      self.card.remove_css_class("fingerprint");
    }
    for w in [self.card.upcast_ref::<gtk::Widget>(), self.glyph.upcast_ref(), self.entry.upcast_ref()] {
      if error.is_some() {
        w.add_css_class("error");
      } else {
        w.remove_css_class("error");
      }
    }
    let placeholder = match error {
      Some(e) => e,
      None if self.submitted.get() => "Verificando...",
      None if self.response_required.get() => "Digite a senha",
      None => "Aguarde...",
    };
    self.entry.set_placeholder_text(Some(placeholder));
    self.entry.set_editable(self.response_required.get() && error.is_none());
  }
}

fn lid_closed() -> bool {
  std::process::Command::new("omarchy-hw-laptop-closed")
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .status()
    .is_ok_and(|s| s.success())
}
