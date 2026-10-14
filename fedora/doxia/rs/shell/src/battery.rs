// The low battery warning and the power profile switch, replacing
// shell/services/battery (Service.qml): every 30 seconds, and whenever the
// machine goes on or off battery, UPower's display device is checked, and
// omarchy-battery-low warns once per discharge at 10% or less. Going on or
// off battery also applies the profile saved for it (omarchy-powerprofiles-set).

use gtk::prelude::*;
use gtk::{gio, glib};
use std::cell::Cell;
use std::rc::Rc;

const THRESHOLD: f64 = 10.0;
const DISCHARGING: u32 = 2;

// BatteryModel.shouldWarnLowBattery, from UPower's Percentage (0-100):
// (warn now, is low). Low stays true while it lasts, so the warning goes out
// once per discharge.
pub fn should_warn(present: bool, percentage: f64, on_battery: bool, state: u32, notified: bool) -> (bool, bool) {
  if !present {
    return (false, false);
  }
  let low = on_battery && state == DISCHARGING && percentage.round() <= THRESHOLD;
  (low && !notified, low)
}

pub struct Battery {
  upower: gio::DBusProxy,
  device: gio::DBusProxy,
  // Kept across restarts of the shell, as Quickshell's PersistentProperties
  // kept it across reloads.
  notified: Cell<bool>,
}

fn flag_path() -> std::path::PathBuf {
  std::path::PathBuf::from(std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into())).join("doxia-battery-low-notified")
}

fn bin(name: &str) -> String {
  format!("{}/bin/{name}", std::env::var("OMARCHY_PATH").unwrap_or_default())
}

impl Battery {
  pub fn start() {
    glib::spawn_future_local(async {
      let proxy = |path: &'static str, iface: &'static str| {
        gio::DBusProxy::for_bus_future(gio::BusType::System, gio::DBusProxyFlags::NONE, None, "org.freedesktop.UPower", path, iface)
      };
      let (Ok(upower), Ok(device)) = (
        proxy("/org/freedesktop/UPower", "org.freedesktop.UPower").await,
        proxy("/org/freedesktop/UPower/devices/DisplayDevice", "org.freedesktop.UPower.Device").await,
      ) else {
        return;
      };
      let me = Rc::new(Battery { upower, device, notified: Cell::new(flag_path().exists()) });
      let m = me.clone();
      // connect_local: the handler holds the (main thread only) Rc.
      me.upower.connect_local("g-properties-changed", false, move |values| {
        let changed = values.get(1).and_then(|v| v.get::<glib::Variant>().ok());
        if changed.is_some_and(|c| glib::VariantDict::new(Some(&c)).contains("OnBattery")) {
          m.check();
          let source = if m.on_battery() { "battery" } else { "ac" };
          crate::run::argv(&[&bin("omarchy-powerprofiles-set"), source]);
        }
        None
      });
      let m = me.clone();
      glib::timeout_add_seconds_local(30, move || {
        m.check();
        glib::ControlFlow::Continue
      });
      me.check();
    });
  }

  fn on_battery(&self) -> bool {
    self.upower.cached_property("OnBattery").and_then(|v| v.get::<bool>()).unwrap_or(false)
  }

  fn check(&self) {
    let prop = |name: &str| self.device.cached_property(name);
    let present = prop("IsPresent").and_then(|v| v.get::<bool>()).unwrap_or(false);
    let percentage = prop("Percentage").and_then(|v| v.get::<f64>()).unwrap_or(0.0);
    let state = prop("State").and_then(|v| v.get::<u32>()).unwrap_or(0);
    let (warn, low) = should_warn(present, percentage, self.on_battery(), state, self.notified.get());
    if low != self.notified.get() {
      self.notified.set(low);
      if low { let _ = std::fs::write(flag_path(), ""); } else { let _ = std::fs::remove_file(flag_path()); }
    }
    if warn {
      crate::run::argv(&[&bin("omarchy-battery-low"), &(percentage.round() as i64).to_string()]);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn warns_once_when_discharging_low() {
    assert_eq!(should_warn(true, 10.6, true, DISCHARGING, false), (false, false));
    assert_eq!(should_warn(true, 10.0, true, DISCHARGING, false), (true, true));
    assert_eq!(should_warn(true, 8.0, true, DISCHARGING, true), (false, true));
    assert_eq!(should_warn(true, 8.0, false, 1, true), (false, false));
    assert_eq!(should_warn(false, 0.0, true, DISCHARGING, false), (false, false));
  }
}
