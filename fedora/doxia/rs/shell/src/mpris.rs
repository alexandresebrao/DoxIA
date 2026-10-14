// MPRIS media players on the session bus (Quickshell.Services.Mpris): every
// org.mpris.MediaPlayer2.* name but playerctld (which only proxies the
// others), with what they play and what they can do. One instance per
// process (mpris::get()); listeners hear of every change.

use gtk::prelude::*;
use gtk::{gio, glib};
use std::cell::RefCell;
use std::rc::Rc;

const PREFIX: &str = "org.mpris.MediaPlayer2.";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";

pub struct Player {
  pub bus_name: String,
  root: gio::DBusProxy,
  player: gio::DBusProxy,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Loop {
  None,
  Track,
  Playlist,
}

impl Player {
  fn prop<T: glib::variant::FromVariant>(&self, name: &str) -> Option<T> {
    self.player.cached_property(name)?.get::<T>()
  }

  fn meta(&self) -> Option<glib::VariantDict> {
    Some(glib::VariantDict::new(Some(&self.player.cached_property("Metadata")?)))
  }

  fn meta_str(&self, key: &str) -> String {
    self.meta().and_then(|m| m.lookup_value(key, None)).and_then(|v| v.get::<String>()).unwrap_or_default()
  }

  pub fn title(&self) -> String {
    self.meta_str("xesam:title")
  }

  pub fn artist(&self) -> String {
    let Some(v) = self.meta().and_then(|m| m.lookup_value("xesam:artist", None)) else { return String::new() };
    v.get::<Vec<String>>().map(|a| a.join(", ")).or_else(|| v.get::<String>()).unwrap_or_default()
  }

  pub fn album(&self) -> String {
    self.meta_str("xesam:album")
  }

  pub fn art_url(&self) -> String {
    self.meta_str("mpris:artUrl")
  }

  fn track_id(&self) -> Option<glib::Variant> {
    self.meta().and_then(|m| m.lookup_value("mpris:trackid", None))
  }

  // Seconds; 0 when unknown.
  pub fn length(&self) -> f64 {
    let Some(v) = self.meta().and_then(|m| m.lookup_value("mpris:length", None)) else { return 0.0 };
    v.get::<i64>().map(|n| n as f64).or_else(|| v.get::<u64>().map(|n| n as f64)).unwrap_or(0.0) / 1e6
  }

  pub fn playing(&self) -> bool {
    self.prop::<String>("PlaybackStatus").as_deref() == Some("Playing")
  }

  pub fn identity(&self) -> String {
    let name = self
      .root
      .cached_property("Identity")
      .and_then(|v| v.get::<String>())
      .filter(|s| !s.is_empty())
      .or_else(|| self.root.cached_property("DesktopEntry").and_then(|v| v.get::<String>()))
      .unwrap_or_default();
    let mut c = name.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
  }

  pub fn desktop_entry(&self) -> String {
    self.root.cached_property("DesktopEntry").and_then(|v| v.get::<String>()).unwrap_or_default()
  }

  pub fn raw_identity(&self) -> String {
    self.root.cached_property("Identity").and_then(|v| v.get::<String>()).unwrap_or_default()
  }

  pub fn can(&self, what: &str) -> bool {
    self.prop::<bool>(what).unwrap_or(false)
  }

  pub fn can_raise(&self) -> bool {
    self.root.cached_property("CanRaise").and_then(|v| v.get::<bool>()).unwrap_or(false)
  }

  pub fn shuffle(&self) -> Option<bool> {
    self.prop::<bool>("Shuffle")
  }

  pub fn loop_state(&self) -> Option<Loop> {
    match self.prop::<String>("LoopStatus")?.as_str() {
      "Track" => Some(Loop::Track),
      "Playlist" => Some(Loop::Playlist),
      _ => Some(Loop::None),
    }
  }

  pub fn volume(&self) -> Option<f64> {
    self.prop::<f64>("Volume")
  }

  fn call(&self, method: &str, args: Option<&glib::Variant>) {
    self.player.call(method, args, gio::DBusCallFlags::NONE, 2000, gio::Cancellable::NONE, |_| {});
  }

  fn set(&self, property: &str, value: glib::Variant) {
    let args = (PLAYER, property, value).to_variant();
    self.player.connection().call(
      Some(&self.bus_name),
      "/org/mpris/MediaPlayer2",
      "org.freedesktop.DBus.Properties",
      "Set",
      Some(&args),
      None,
      gio::DBusCallFlags::NONE,
      2000,
      gio::Cancellable::NONE,
      |_| {},
    );
  }

  pub fn play_pause(&self) {
    self.call("PlayPause", None);
  }

  pub fn next(&self) {
    self.call("Next", None);
  }

  pub fn previous(&self) {
    self.call("Previous", None);
  }

  pub fn raise(&self) {
    self.root.call("Raise", None, gio::DBusCallFlags::NONE, 2000, gio::Cancellable::NONE, |_| {});
  }

  pub fn set_shuffle(&self, on: bool) {
    self.set("Shuffle", on.to_variant());
  }

  pub fn set_loop(&self, state: Loop) {
    let name = match state {
      Loop::None => "None",
      Loop::Track => "Track",
      Loop::Playlist => "Playlist",
    };
    self.set("LoopStatus", name.to_variant());
  }

  pub fn set_volume(&self, v: f64) {
    self.set("Volume", v.clamp(0.0, 1.0).to_variant());
  }

  // Seconds. MPRIS reports the position only when asked.
  pub async fn position(&self) -> f64 {
    let args = (PLAYER, "Position").to_variant();
    let reply = self
      .player
      .connection()
      .call_future(Some(&self.bus_name), "/org/mpris/MediaPlayer2", "org.freedesktop.DBus.Properties", "Get", Some(&args), None, gio::DBusCallFlags::NONE, 2000)
      .await;
    reply
      .ok()
      .and_then(|r| r.child_value(0).as_variant())
      .and_then(|v| v.get::<i64>())
      .map(|us| us as f64 / 1e6)
      .unwrap_or(0.0)
  }

  pub fn set_position(&self, seconds: f64) {
    let Some(track) = self.track_id().and_then(|t| t.str().map(String::from)) else { return };
    let Ok(path) = glib::variant::ObjectPath::try_from(track) else { return };
    let args = (path, (seconds.max(0.0) * 1e6) as i64).to_variant();
    self.call("SetPosition", Some(&args));
  }
}

#[derive(Default)]
pub struct Mpris {
  players: RefCell<Vec<Rc<Player>>>,
  subscription: RefCell<Option<gio::SignalSubscription>>,
  listeners: RefCell<Vec<Box<dyn Fn()>>>,
}

thread_local! {
  static MPRIS: RefCell<Option<Rc<Mpris>>> = const { RefCell::new(None) };
}

pub fn get() -> Rc<Mpris> {
  MPRIS.with(|m| {
    if let Some(m) = m.borrow().as_ref() {
      return m.clone();
    }
    let me = Rc::new(Mpris::default());
    *m.borrow_mut() = Some(me.clone());
    me.start();
    me
  })
}

impl Mpris {
  fn start(self: &Rc<Self>) {
    let weak = Rc::downgrade(self);
    glib::spawn_future_local(async move {
      let Ok(conn) = gio::bus_get_future(gio::BusType::Session).await else { return };
      let Some(me) = weak.upgrade() else { return };
      // Players coming and going.
      let w = Rc::downgrade(&me);
      let c = conn.clone();
      let sub = conn.subscribe_to_signal(
        Some("org.freedesktop.DBus"),
        Some("org.freedesktop.DBus"),
        Some("NameOwnerChanged"),
        Some("/org/freedesktop/DBus"),
        Some(PREFIX.trim_end_matches('.')),
        gio::DBusSignalFlags::MATCH_ARG0_NAMESPACE,
        move |signal| {
          let Some((name, _, new_owner)) = signal.parameters.get::<(String, String, String)>() else { return };
          let Some(me) = w.upgrade() else { return };
          if new_owner.is_empty() {
            me.players.borrow_mut().retain(|p| p.bus_name != name);
            me.changed();
          } else {
            me.add(&c, &name);
          }
        },
      );
      *me.subscription.borrow_mut() = Some(sub);
      let names = conn
        .call_future(Some("org.freedesktop.DBus"), "/org/freedesktop/DBus", "org.freedesktop.DBus", "ListNames", None, None, gio::DBusCallFlags::NONE, 2000)
        .await
        .ok()
        .and_then(|r| r.child_value(0).get::<Vec<String>>())
        .unwrap_or_default();
      for name in names {
        me.add(&conn, &name);
      }
    });
  }

  fn add(self: &Rc<Self>, conn: &gio::DBusConnection, name: &str) {
    if !name.starts_with(PREFIX) || name.contains("playerctld") || self.players.borrow().iter().any(|p| p.bus_name == name) {
      return;
    }
    let (weak, conn, name) = (Rc::downgrade(self), conn.clone(), name.to_string());
    glib::spawn_future_local(async move {
      let proxy = |iface: &'static str| {
        gio::DBusProxy::new_future(&conn, gio::DBusProxyFlags::NONE, None, Some(&name), "/org/mpris/MediaPlayer2", iface)
      };
      let (Ok(root), Ok(player)) = (proxy("org.mpris.MediaPlayer2").await, proxy(PLAYER).await) else { return };
      let Some(me) = weak.upgrade() else { return };
      if me.players.borrow().iter().any(|p| p.bus_name == name) {
        return;
      }
      for p in [&root, &player] {
        let w = Rc::downgrade(&me);
        p.connect_local("g-properties-changed", false, move |_| {
          if let Some(me) = w.upgrade() {
            me.changed();
          }
          None
        });
      }
      me.players.borrow_mut().push(Rc::new(Player { bus_name: name, root, player }));
      me.changed();
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

  pub fn players(&self) -> Vec<Rc<Player>> {
    self.players.borrow().clone()
  }

  // The one to show: the chosen one, else one playing, else one with a
  // track, else any.
  pub fn pick(&self, selected: &str) -> Option<Rc<Player>> {
    let players = self.players.borrow();
    players
      .iter()
      .find(|p| p.bus_name == selected)
      .or_else(|| players.iter().find(|p| p.playing()))
      .or_else(|| players.iter().find(|p| !p.title().is_empty()))
      .or_else(|| players.first())
      .cloned()
  }
}
