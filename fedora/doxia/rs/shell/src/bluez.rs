// BlueZ on the system bus (Quickshell.Bluetooth): the first adapter (on,
// discovering) and the devices it knows, read whole with GetManagedObjects
// and again (coalesced) on every signal BlueZ sends. One instance per
// process (bluez::get()).

use gtk::prelude::*;
use gtk::{gio, glib};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Adapter {
  pub path: String,
  pub powered: bool,
  pub discovering: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Device {
  pub path: String,
  pub address: String,
  pub name: String,
  pub alias: String,
  pub icon: String,
  pub connected: bool,
  pub paired: bool,
  pub bonded: bool,
  pub trusted: bool,
  pub battery: Option<u8>,
}

impl Device {
  pub fn label(&self) -> String {
    let a = self.alias.trim();
    if !a.is_empty() { a.to_string() } else { self.name.trim().to_string() }
  }

  pub fn known(&self) -> bool {
    self.paired || self.bonded || self.trusted
  }
}

fn is_uuid_like(t: &str) -> bool {
  let t = t.trim();
  let hex = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_hexdigit());
  let parts: Vec<&str> = t.split('-').collect();
  (parts.len() == 5 && [8, 4, 4, 4, 12].iter().zip(&parts).all(|(n, p)| p.len() == *n && hex(p)))
    || (t.len() == 32 && hex(t))
    || (t.starts_with("0x") && (6..=34).contains(&t.len()) && hex(&t[2..]))
}

fn is_address_like(t: &str) -> bool {
  let t = t.trim();
  let parts: Vec<&str> = t.split([':', '-']).collect();
  parts.len() == 6 && parts.iter().all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_hexdigit()))
}

// A device worth listing has a name a person gave it.
pub fn has_human_name(d: &Device) -> bool {
  let l = d.label();
  !l.is_empty() && !is_uuid_like(&l) && !is_address_like(&l)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
  pub adapter: Option<Adapter>,
  pub devices: Vec<Device>,
}

fn parse(objects: &glib::Variant) -> Snapshot {
  let mut snap = Snapshot::default();
  // a{oa{sa{sv}}}
  for i in 0..objects.n_children() {
    let entry = objects.child_value(i);
    let path = entry.child_value(0).str().unwrap_or("").to_string();
    let ifaces = entry.child_value(1);
    let iface = |name: &str| -> Option<glib::VariantDict> {
      (0..ifaces.n_children()).map(|j| ifaces.child_value(j)).find(|e| e.child_value(0).str() == Some(name)).map(|e| glib::VariantDict::new(Some(&e.child_value(1))))
    };
    let b = |d: &glib::VariantDict, k: &str| d.lookup_value(k, None).and_then(|v| v.get::<bool>()).unwrap_or(false);
    let s = |d: &glib::VariantDict, k: &str| d.lookup_value(k, None).and_then(|v| v.get::<String>()).unwrap_or_default();
    if let Some(a) = iface("org.bluez.Adapter1") {
      if snap.adapter.is_none() {
        snap.adapter = Some(Adapter { path: path.clone(), powered: b(&a, "Powered"), discovering: b(&a, "Discovering") });
      }
    }
    if let Some(d) = iface("org.bluez.Device1") {
      let battery = iface("org.bluez.Battery1").and_then(|bt| bt.lookup_value("Percentage", None)).and_then(|v| v.get::<u8>());
      snap.devices.push(Device {
        path,
        address: s(&d, "Address"),
        name: s(&d, "Name"),
        alias: s(&d, "Alias"),
        icon: s(&d, "Icon"),
        connected: b(&d, "Connected"),
        paired: b(&d, "Paired"),
        bonded: b(&d, "Bonded"),
        trusted: b(&d, "Trusted"),
        battery,
      });
    }
  }
  snap
}

#[derive(Default)]
pub struct Bluez {
  snap: RefCell<Rc<Snapshot>>,
  conn: RefCell<Option<gio::DBusConnection>>,
  pending: Cell<bool>,
  subscriptions: RefCell<Vec<gio::SignalSubscription>>,
  listeners: RefCell<Vec<Box<dyn Fn()>>>,
}

thread_local! {
  static BLUEZ: RefCell<Option<Rc<Bluez>>> = const { RefCell::new(None) };
}

pub fn get() -> Rc<Bluez> {
  BLUEZ.with(|b| {
    if let Some(b) = b.borrow().as_ref() {
      return b.clone();
    }
    let me = Rc::new(Bluez::default());
    *b.borrow_mut() = Some(me.clone());
    me.start();
    me
  })
}

impl Bluez {
  fn start(self: &Rc<Self>) {
    let weak = Rc::downgrade(self);
    glib::spawn_future_local(async move {
      let Ok(conn) = gio::bus_get_future(gio::BusType::System).await else { return };
      let Some(me) = weak.upgrade() else { return };
      *me.conn.borrow_mut() = Some(conn.clone());
      let mut subs = Vec::new();
      for (iface, member) in [
        ("org.freedesktop.DBus.ObjectManager", None),
        ("org.freedesktop.DBus.Properties", Some("PropertiesChanged")),
      ] {
        let w = Rc::downgrade(&me);
        subs.push(conn.subscribe_to_signal(Some("org.bluez"), Some(iface), member, None, None, gio::DBusSignalFlags::NONE, move |_| {
          if let Some(me) = w.upgrade() {
            me.refresh();
          }
        }));
      }
      // BlueZ starting or stopping.
      let w = Rc::downgrade(&me);
      subs.push(conn.subscribe_to_signal(
        Some("org.freedesktop.DBus"),
        Some("org.freedesktop.DBus"),
        Some("NameOwnerChanged"),
        Some("/org/freedesktop/DBus"),
        Some("org.bluez"),
        gio::DBusSignalFlags::NONE,
        move |_| {
          if let Some(me) = w.upgrade() {
            me.refresh();
          }
        },
      ));
      *me.subscriptions.borrow_mut() = subs;
      me.refresh();
    });
  }

  pub fn refresh(self: &Rc<Self>) {
    if self.pending.replace(true) {
      return;
    }
    let weak = Rc::downgrade(self);
    glib::spawn_future_local(async move {
      glib::timeout_future(Duration::from_millis(80)).await;
      let Some(me) = weak.upgrade() else { return };
      me.pending.set(false);
      let Some(conn) = me.conn.borrow().clone() else { return };
      let snap = conn
        .call_future(Some("org.bluez"), "/", "org.freedesktop.DBus.ObjectManager", "GetManagedObjects", None, None, gio::DBusCallFlags::NO_AUTO_START, 2000)
        .await
        .map(|r| parse(&r.child_value(0)))
        .unwrap_or_default();
      if *me.snap.borrow().as_ref() != snap {
        *me.snap.borrow_mut() = Rc::new(snap);
        for l in me.listeners.borrow().iter() {
          l();
        }
      }
    });
  }

  pub fn snapshot(&self) -> Rc<Snapshot> {
    self.snap.borrow().clone()
  }

  pub fn connect(&self, f: impl Fn() + 'static) {
    self.listeners.borrow_mut().push(Box::new(f));
  }

  pub fn set_discovering(&self, on: bool) {
    let (Some(conn), Some(adapter)) = (self.conn.borrow().clone(), self.snap.borrow().adapter.clone()) else { return };
    let method = if on { "StartDiscovery" } else { "StopDiscovery" };
    conn.call(Some("org.bluez"), &adapter.path, "org.bluez.Adapter1", method, None, None, gio::DBusCallFlags::NONE, 2000, gio::Cancellable::NONE, |_| {});
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn names_people_gave() {
    let d = |alias: &str| Device { alias: alias.into(), ..Default::default() };
    assert!(has_human_name(&d("WH-1000XM4")));
    assert!(!has_human_name(&d("AA:BB:CC:DD:EE:FF")));
    assert!(!has_human_name(&d("0000110b-0000-1000-8000-00805f9b34fb")));
    assert!(!has_human_name(&d("")));
  }
}
