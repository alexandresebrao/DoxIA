// UPower on the system bus: whether the machine runs on battery and its
// display device (the combined battery), as Quickshell.Services.UPower
// shows them. One instance per process (upower::get()); listeners hear of
// every change.

use gtk::prelude::*;
use gtk::{gio, glib};
use std::cell::RefCell;
use std::rc::Rc;

pub const CHARGING: u32 = 1;
pub const DISCHARGING: u32 = 2;
pub const FULLY_CHARGED: u32 = 4;
pub const PENDING_CHARGE: u32 = 5;

#[derive(Default)]
pub struct UPower {
  upower: RefCell<Option<gio::DBusProxy>>,
  device: RefCell<Option<gio::DBusProxy>>,
  listeners: RefCell<Vec<Box<dyn Fn()>>>,
}

thread_local! {
  static UPOWER: RefCell<Option<Rc<UPower>>> = const { RefCell::new(None) };
}

pub fn get() -> Rc<UPower> {
  UPOWER.with(|u| {
    if let Some(u) = u.borrow().as_ref() {
      return u.clone();
    }
    let me = Rc::new(UPower::default());
    *u.borrow_mut() = Some(me.clone());
    me.start();
    me
  })
}

impl UPower {
  fn start(self: &Rc<Self>) {
    let weak = Rc::downgrade(self);
    glib::spawn_future_local(async move {
      let proxy = |path: &'static str, iface: &'static str| {
        gio::DBusProxy::for_bus_future(gio::BusType::System, gio::DBusProxyFlags::NONE, None, "org.freedesktop.UPower", path, iface)
      };
      let (Ok(upower), Ok(device)) = (
        proxy("/org/freedesktop/UPower", "org.freedesktop.UPower").await,
        proxy("/org/freedesktop/UPower/devices/DisplayDevice", "org.freedesktop.UPower.Device").await,
      ) else {
        return;
      };
      let Some(me) = weak.upgrade() else { return };
      for p in [&upower, &device] {
        let weak = Rc::downgrade(&me);
        // connect_local: the handler holds the (main thread only) Rc.
        p.connect_local("g-properties-changed", false, move |_| {
          if let Some(me) = weak.upgrade() {
            me.changed();
          }
          None
        });
      }
      *me.upower.borrow_mut() = Some(upower);
      *me.device.borrow_mut() = Some(device);
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

  fn device<T: for<'a> glib::variant::FromVariant>(&self, name: &str) -> Option<T> {
    self.device.borrow().as_ref()?.cached_property(name)?.get::<T>()
  }

  pub fn on_battery(&self) -> bool {
    self.upower.borrow().as_ref().and_then(|p| p.cached_property("OnBattery")).and_then(|v| v.get::<bool>()).unwrap_or(false)
  }

  pub fn present(&self) -> bool {
    self.device::<bool>("IsPresent").unwrap_or(false)
  }

  // 0..1.
  pub fn fraction(&self) -> f64 {
    (self.device::<f64>("Percentage").unwrap_or(0.0) / 100.0).clamp(0.0, 1.0)
  }

  pub fn state(&self) -> u32 {
    self.device::<u32>("State").unwrap_or(0)
  }

  // Seconds, 0 when unknown.
  pub fn time_to_empty(&self) -> i64 {
    self.device::<i64>("TimeToEmpty").unwrap_or(0)
  }

  pub fn time_to_full(&self) -> i64 {
    self.device::<i64>("TimeToFull").unwrap_or(0)
  }

  pub fn energy_rate(&self) -> f64 {
    self.device::<f64>("EnergyRate").unwrap_or(0.0)
  }
}
