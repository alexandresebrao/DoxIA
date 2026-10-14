// The system tray (StatusNotifierItem, Quickshell.Services.SystemTray).
//
// The watcher: doxia-shell queues for org.kde.StatusNotifierWatcher and
// serves it once it gets the name (Quickshell holds it while it runs; apps
// register again when the owner changes). Either way the items are read
// from whoever owns it, through its property and signals, so the host side
// does not care whose watcher it is.
//
// Items: their id, title, status, icon (a theme name, with the item's own
// theme path, or pixmaps), tooltip and menu (com.canonical.dbusmenu), read
// whole on every change signal. One instance per process (sni::get()).

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

const WATCHER: &str = "org.kde.StatusNotifierWatcher";
const WATCHER_PATH: &str = "/StatusNotifierWatcher";
const ITEM: &str = "org.kde.StatusNotifierItem";
const MENU: &str = "com.canonical.dbusmenu";

const WATCHER_XML: &str = r#"<node>
  <interface name="org.kde.StatusNotifierWatcher">
    <method name="RegisterStatusNotifierItem"><arg name="service" type="s" direction="in"/></method>
    <method name="RegisterStatusNotifierHost"><arg name="service" type="s" direction="in"/></method>
    <property name="RegisteredStatusNotifierItems" type="as" access="read"/>
    <property name="IsStatusNotifierHostRegistered" type="b" access="read"/>
    <property name="ProtocolVersion" type="i" access="read"/>
    <signal name="StatusNotifierItemRegistered"><arg type="s"/></signal>
    <signal name="StatusNotifierItemUnregistered"><arg type="s"/></signal>
    <signal name="StatusNotifierHostRegistered"/>
  </interface>
</node>"#;

#[derive(Clone, Debug, PartialEq)]
pub enum Icon {
  None,
  Name(String, String),
  Pixmap(i32, i32, Vec<u8>),
}

#[derive(Clone, Debug)]
pub struct Item {
  // "service/path", as the watcher lists it.
  pub key: String,
  pub service: String,
  pub path: String,
  pub id: String,
  pub title: String,
  pub status: String,
  pub icon: Icon,
  pub tooltip: String,
  pub menu: String,
  pub only_menu: bool,
}

impl Item {
  pub fn tooltip_text(&self) -> String {
    [&self.tooltip, &self.title, &self.id].into_iter().find(|s| !s.is_empty()).cloned().unwrap_or_default()
  }

  pub fn display_name(&self) -> String {
    if !self.title.trim().is_empty() {
      return self.title.trim().into();
    }
    if !self.tooltip.trim().is_empty() {
      return self.tooltip.trim().into();
    }
    self.id.rsplit('/').next().filter(|s| !s.is_empty()).unwrap_or("Unknown").to_string()
  }
}

// "service/path" or a bare service (path /StatusNotifierItem) or a bare
// path (the sender's).
pub fn split_key(key: &str, sender: &str) -> (String, String) {
  if key.starts_with('/') {
    return (sender.to_string(), key.to_string());
  }
  match key.find('/') {
    Some(i) => (key[..i].to_string(), key[i..].to_string()),
    None => (key.to_string(), "/StatusNotifierItem".to_string()),
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MenuEntry {
  pub id: i32,
  pub label: String,
  pub enabled: bool,
  pub separator: bool,
  pub toggle: bool,
  pub checked: bool,
  pub icon: String,
  pub children: Vec<MenuEntry>,
}

fn parse_menu(v: &glib::Variant) -> Option<MenuEntry> {
  // (ia{sv}av)
  let id = v.child_value(0).get::<i32>()?;
  let props = glib::VariantDict::new(Some(&v.child_value(1)));
  let s = |k: &str| props.lookup_value(k, None).and_then(|v| v.get::<String>());
  let visible = props.lookup_value("visible", None).and_then(|v| v.get::<bool>()).unwrap_or(true);
  if !visible {
    return None;
  }
  let children_v = v.child_value(2);
  let children = (0..children_v.n_children()).filter_map(|i| children_v.child_value(i).as_variant()).filter_map(|c| parse_menu(&c)).collect();
  Some(MenuEntry {
    id,
    // Mnemonics: "_File" is "File", "__" an underscore.
    label: s("label").unwrap_or_default().replace("__", "\u{0}").replace('_', "").replace('\u{0}', "_"),
    enabled: props.lookup_value("enabled", None).and_then(|v| v.get::<bool>()).unwrap_or(true),
    separator: s("type").as_deref() == Some("separator"),
    toggle: s("toggle-type").is_some_and(|t| !t.is_empty()),
    checked: props.lookup_value("toggle-state", None).and_then(|v| v.get::<i32>()) == Some(1),
    icon: s("icon-name").unwrap_or_default(),
    children,
  })
}

#[derive(Default)]
pub struct Tray {
  items: RefCell<Vec<Item>>,
  conn: RefCell<Option<gio::DBusConnection>>,
  // Our watcher, while we own the name.
  registered: RefCell<Vec<String>>,
  hosts: RefCell<HashSet<String>>,
  subscriptions: RefCell<Vec<gio::SignalSubscription>>,
  listeners: RefCell<Vec<Box<dyn Fn()>>>,
}

thread_local! {
  static TRAY: RefCell<Option<Rc<Tray>>> = const { RefCell::new(None) };
}

pub fn get() -> Rc<Tray> {
  TRAY.with(|t| {
    if let Some(t) = t.borrow().as_ref() {
      return t.clone();
    }
    let me = Rc::new(Tray::default());
    *t.borrow_mut() = Some(me.clone());
    me.start();
    me
  })
}

impl Tray {
  fn start(self: &Rc<Self>) {
    let weak = Rc::downgrade(self);
    glib::spawn_future_local(async move {
      let Ok(conn) = gio::bus_get_future(gio::BusType::Session).await else { return };
      let Some(me) = weak.upgrade() else { return };
      *me.conn.borrow_mut() = Some(conn.clone());
      me.serve_watcher(&conn);
      me.follow_watcher(&conn);
    });
  }

  fn changed(&self) {
    for l in self.listeners.borrow().iter() {
      l();
    }
  }

  pub fn connect(&self, f: impl Fn() + 'static) {
    self.listeners.borrow_mut().push(Box::new(f));
  }

  pub fn items(&self) -> Vec<Item> {
    self.items.borrow().clone()
  }

  // Our own watcher, served once the name is ours.
  fn serve_watcher(self: &Rc<Self>, conn: &gio::DBusConnection) {
    let info = gio::DBusNodeInfo::for_xml(WATCHER_XML).expect("watcher xml");
    let iface = info.lookup_interface(WATCHER).expect("watcher interface");
    let (w1, w2) = (Rc::downgrade(self), Rc::downgrade(self));
    let registered = conn
      .register_object(WATCHER_PATH, &iface)
      .method_call(move |conn, sender, _, _, method, params, invocation| {
        let Some(me) = w1.upgrade() else { return };
        let arg = params.child_value(0).get::<String>().unwrap_or_default();
        let sender = sender.unwrap_or("").to_string();
        match method {
          "RegisterStatusNotifierItem" => {
            let (service, path) = split_key(&arg, &sender);
            let key = format!("{service}{path}");
            if !me.registered.borrow().contains(&key) {
              me.registered.borrow_mut().push(key.clone());
              let _ = conn.emit_signal(None, WATCHER_PATH, WATCHER, "StatusNotifierItemRegistered", Some(&(key.clone(),).to_variant()));
              me.watch_owner(&conn, &service, key);
            }
            invocation.return_value(None);
          }
          "RegisterStatusNotifierHost" => {
            me.hosts.borrow_mut().insert(arg);
            let _ = conn.emit_signal(None, WATCHER_PATH, WATCHER, "StatusNotifierHostRegistered", None);
            invocation.return_value(None);
          }
          _ => invocation.return_dbus_error("org.freedesktop.DBus.Error.UnknownMethod", method),
        }
      })
      .property(move |_, _, _, _, name| {
        let items = w2.upgrade().map(|me| me.registered.borrow().clone()).unwrap_or_default();
        match name {
          "RegisteredStatusNotifierItems" => items.to_variant(),
          "IsStatusNotifierHostRegistered" => true.to_variant(),
          _ => 0i32.to_variant(),
        }
      })
      .build();
    if registered.is_err() {
      return;
    }
    // Queued: Quickshell keeps it while it runs.
    let id = gio::bus_own_name_on_connection(conn, WATCHER, gio::BusNameOwnerFlags::NONE, |_, _| {}, |_, _| {});
    std::mem::forget(id);
  }

  // An item whose app quits leaves our watcher's list.
  fn watch_owner(self: &Rc<Self>, conn: &gio::DBusConnection, service: &str, key: String) {
    let weak = Rc::downgrade(self);
    let c = conn.clone();
    let id = gio::bus_watch_name_on_connection(
      conn,
      service,
      gio::BusNameWatcherFlags::NONE,
      |_, _, _| {},
      move |_, _| {
        if let Some(me) = weak.upgrade() {
          me.registered.borrow_mut().retain(|k| *k != key);
          let _ = c.emit_signal(None, WATCHER_PATH, WATCHER, "StatusNotifierItemUnregistered", Some(&(key.clone(),).to_variant()));
        }
      },
    );
    std::mem::forget(id);
  }

  // The items of whoever owns the watcher name, kept up to date.
  fn follow_watcher(self: &Rc<Self>, conn: &gio::DBusConnection) {
    let mut subs = Vec::new();
    for signal in ["StatusNotifierItemRegistered", "StatusNotifierItemUnregistered"] {
      let weak = Rc::downgrade(self);
      let c = conn.clone();
      subs.push(conn.subscribe_to_signal(Some(WATCHER), Some(WATCHER), Some(signal), Some(WATCHER_PATH), None, gio::DBusSignalFlags::NONE, move |_| {
        if let Some(me) = weak.upgrade() {
          me.reload(&c);
        }
      }));
    }
    // A new owner (Quickshell gone, or us): read its list.
    let weak = Rc::downgrade(self);
    let c = conn.clone();
    subs.push(conn.subscribe_to_signal(
      Some("org.freedesktop.DBus"),
      Some("org.freedesktop.DBus"),
      Some("NameOwnerChanged"),
      Some("/org/freedesktop/DBus"),
      Some(WATCHER),
      gio::DBusSignalFlags::NONE,
      move |_| {
        if let Some(me) = weak.upgrade() {
          me.register_host(&c);
          me.reload(&c);
        }
      },
    ));
    // An item's own changes.
    for signal in ["NewIcon", "NewTitle", "NewStatus", "NewToolTip", "NewAttentionIcon", "NewMenu"] {
      let weak = Rc::downgrade(self);
      let c = conn.clone();
      subs.push(conn.subscribe_to_signal(None, Some(ITEM), Some(signal), None, None, gio::DBusSignalFlags::NONE, move |s| {
        let (sender, path) = (s.sender_name.to_string(), s.object_path.to_string());
        if let Some(me) = weak.upgrade() {
          me.refresh_item(&c, &sender, &path);
        }
      }));
    }
    *self.subscriptions.borrow_mut() = subs;
    self.register_host(conn);
    self.reload(conn);
  }

  fn register_host(&self, conn: &gio::DBusConnection) {
    conn.call(
      Some(WATCHER),
      WATCHER_PATH,
      WATCHER,
      "RegisterStatusNotifierHost",
      Some(&("org.doxia.Shell",).to_variant()),
      None,
      gio::DBusCallFlags::NONE,
      2000,
      gio::Cancellable::NONE,
      |_| {},
    );
  }

  fn reload(self: &Rc<Self>, conn: &gio::DBusConnection) {
    let (weak, conn) = (Rc::downgrade(self), conn.clone());
    glib::spawn_future_local(async move {
      let args = (WATCHER, "RegisteredStatusNotifierItems").to_variant();
      let keys = conn
        .call_future(Some(WATCHER), WATCHER_PATH, "org.freedesktop.DBus.Properties", "Get", Some(&args), None, gio::DBusCallFlags::NONE, 2000)
        .await
        .ok()
        .and_then(|r| r.child_value(0).as_variant())
        .and_then(|v| v.get::<Vec<String>>())
        .unwrap_or_default();
      let mut items = Vec::new();
      for key in keys {
        let (service, path) = split_key(&key, "");
        if let Some(item) = read_item(&conn, &key, &service, &path).await {
          items.push(item);
        }
      }
      let Some(me) = weak.upgrade() else { return };
      *me.items.borrow_mut() = items;
      me.changed();
    });
  }

  fn refresh_item(self: &Rc<Self>, conn: &gio::DBusConnection, sender: &str, path: &str) {
    let key = {
      let items = self.items.borrow();
      match items.iter().find(|i| i.path == path && (i.service == sender || unique_matches(conn, &i.service, sender))) {
        Some(i) => i.key.clone(),
        None => return,
      }
    };
    let (weak, conn) = (Rc::downgrade(self), conn.clone());
    glib::spawn_future_local(async move {
      let (service, path) = split_key(&key, "");
      let fresh = read_item(&conn, &key, &service, &path).await;
      let Some(me) = weak.upgrade() else { return };
      if let Some(fresh) = fresh {
        if let Some(slot) = me.items.borrow_mut().iter_mut().find(|i| i.key == key) {
          *slot = fresh;
        }
        me.changed();
      }
    });
  }

  fn call(&self, item: &Item, method: &str, args: Option<glib::Variant>) {
    if let Some(conn) = self.conn.borrow().as_ref() {
      conn.call(Some(&item.service), &item.path, ITEM, method, args.as_ref(), None, gio::DBusCallFlags::NONE, 2000, gio::Cancellable::NONE, |_| {});
    }
  }

  pub fn activate(&self, item: &Item) {
    self.call(item, "Activate", Some((0i32, 0i32).to_variant()));
  }

  pub fn secondary_activate(&self, item: &Item) {
    self.call(item, "SecondaryActivate", Some((0i32, 0i32).to_variant()));
  }

  pub fn context_menu(&self, item: &Item) {
    self.call(item, "ContextMenu", Some((0i32, 0i32).to_variant()));
  }

  pub fn scroll(&self, item: &Item, delta: i32) {
    self.call(item, "Scroll", Some((delta, "vertical").to_variant()));
  }

  pub async fn menu(&self, item: &Item) -> Option<MenuEntry> {
    let conn = self.conn.borrow().clone()?;
    if item.menu.is_empty() {
      return None;
    }
    let _ = conn
      .call_future(Some(&item.service), &item.menu, MENU, "AboutToShow", Some(&(0i32,).to_variant()), None, gio::DBusCallFlags::NONE, 1000)
      .await;
    let reply = conn
      .call_future(Some(&item.service), &item.menu, MENU, "GetLayout", Some(&(0i32, -1i32, Vec::<String>::new()).to_variant()), None, gio::DBusCallFlags::NONE, 2000)
      .await
      .ok()?;
    parse_menu(&reply.child_value(1))
  }

  pub async fn about_to_show(&self, item: &Item, id: i32) {
    let Some(conn) = self.conn.borrow().clone() else { return };
    let _ = conn
      .call_future(Some(&item.service), &item.menu, MENU, "AboutToShow", Some(&(id,).to_variant()), None, gio::DBusCallFlags::NONE, 1000)
      .await;
  }

  pub fn menu_click(&self, item: &Item, id: i32) {
    if let Some(conn) = self.conn.borrow().as_ref() {
      let now = (glib::monotonic_time() / 1000) as u32;
      let args = (id, "clicked", glib::Variant::from_variant(&0i32.to_variant()), now).to_variant();
      conn.call(Some(&item.service), &item.menu, MENU, "Event", Some(&args), None, gio::DBusCallFlags::NONE, 2000, gio::Cancellable::NONE, |_| {});
    }
  }
}

// A well-known service name owned by the unique name a signal came from.
fn unique_matches(_conn: &gio::DBusConnection, service: &str, sender: &str) -> bool {
  service.starts_with(':') && service == sender
}

async fn read_item(conn: &gio::DBusConnection, key: &str, service: &str, path: &str) -> Option<Item> {
  let reply = conn
    .call_future(Some(service), path, "org.freedesktop.DBus.Properties", "GetAll", Some(&(ITEM,).to_variant()), None, gio::DBusCallFlags::NONE, 2000)
    .await
    .ok()?;
  let props = glib::VariantDict::new(Some(&reply.child_value(0)));
  let s = |k: &str| props.lookup_value(k, None).and_then(|v| v.get::<String>()).unwrap_or_default();
  let theme_path = s("IconThemePath");
  let icon = match s("IconName") {
    name if !name.is_empty() => Icon::Name(name, theme_path),
    _ => props.lookup_value("IconPixmap", None).and_then(|v| best_pixmap(&v)).unwrap_or(Icon::None),
  };
  let tooltip = props
    .lookup_value("ToolTip", None)
    .filter(|v| v.n_children() >= 3)
    .and_then(|v| v.child_value(2).get::<String>())
    .unwrap_or_default();
  let menu = props.lookup_value("Menu", None).and_then(|v| v.str().map(String::from)).unwrap_or_default();
  Some(Item {
    key: key.to_string(),
    service: service.to_string(),
    path: path.to_string(),
    id: s("Id"),
    title: s("Title"),
    status: s("Status"),
    icon,
    tooltip,
    menu,
    only_menu: props.lookup_value("ItemIsMenu", None).and_then(|v| v.get::<bool>()).unwrap_or(false),
  })
}

// The largest of the a(iiay) pixmaps.
fn best_pixmap(v: &glib::Variant) -> Option<Icon> {
  (0..v.n_children())
    .map(|i| v.child_value(i))
    .filter_map(|p| {
      let w = p.child_value(0).get::<i32>()?;
      let h = p.child_value(1).get::<i32>()?;
      let data = p.child_value(2).fixed_array::<u8>().ok()?.to_vec();
      (w > 0 && h > 0 && data.len() == (w * h * 4) as usize).then_some((w, h, data))
    })
    .max_by_key(|(w, _, _)| *w)
    .map(|(w, h, d)| Icon::Pixmap(w, h, d))
}

// A paintable for an item's icon at `size` pixels.
pub fn icon_paintable(icon: &Icon, size: i32, scale: i32) -> Option<gdk::Paintable> {
  match icon {
    Icon::None => None,
    Icon::Name(name, theme_path) => {
      let display = gdk::Display::default()?;
      let theme = gtk::IconTheme::for_display(&display);
      if !theme_path.is_empty() && !theme.search_path().iter().any(|p| p.to_string_lossy() == *theme_path) {
        theme.add_search_path(theme_path);
      }
      let looked = theme.lookup_icon(name, &[], size, scale, gtk::TextDirection::None, gtk::IconLookupFlags::empty());
      // An icon by path, or one the theme lacks but its own dir has.
      if !theme.has_icon(name) {
        for ext in ["png", "svg"] {
          let file = std::path::Path::new(theme_path).join(format!("{name}.{ext}"));
          if file.exists() {
            return gdk::Texture::from_filename(&file).ok().map(|t| t.upcast());
          }
        }
      }
      Some(looked.upcast())
    }
    Icon::Pixmap(w, h, data) => {
      // ARGB32 in network byte order: to RGBA.
      let mut rgba = Vec::with_capacity(data.len());
      for px in data.chunks_exact(4) {
        rgba.extend_from_slice(&[px[1], px[2], px[3], px[0]]);
      }
      let bytes = glib::Bytes::from_owned(rgba);
      Some(gdk::MemoryTexture::new(*w, *h, gdk::MemoryFormat::R8g8b8a8, &bytes, (*w * 4) as usize).upcast())
    }
  }
}

pub fn symbolic(icon: &Icon) -> bool {
  matches!(icon, Icon::Name(n, _) if n.ends_with("-symbolic"))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn keys_split_into_service_and_path() {
    assert_eq!(split_key(":1.753/org/ayatana/NotificationItem/spotify", ""), (":1.753".into(), "/org/ayatana/NotificationItem/spotify".into()));
    assert_eq!(split_key("org.kde.foo", ""), ("org.kde.foo".into(), "/StatusNotifierItem".into()));
    assert_eq!(split_key("/StatusNotifierItem", ":1.9"), (":1.9".into(), "/StatusNotifierItem".into()));
  }
}
