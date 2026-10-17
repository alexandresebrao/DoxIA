// Suspend, reboot and power off through logind, which lets the active seat's
// session (the greeter's) do them without a password.

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

#[derive(Clone, Copy, PartialEq)]
pub enum Action {
  Suspend,
  Reboot,
  PowerOff,
}

impl Action {
  fn method(self) -> &'static str {
    match self {
      Action::Suspend => "Suspend",
      Action::Reboot => "Reboot",
      Action::PowerOff => "PowerOff",
    }
  }
}

fn manager() -> Option<gio::DBusProxy> {
  gio::DBusProxy::for_bus_sync(
    gio::BusType::System,
    gio::DBusProxyFlags::DO_NOT_LOAD_PROPERTIES | gio::DBusProxyFlags::DO_NOT_CONNECT_SIGNALS,
    None,
    "org.freedesktop.login1",
    "/org/freedesktop/login1",
    "org.freedesktop.login1.Manager",
    gio::Cancellable::NONE,
  )
  .ok()
}

// The actions logind would carry out ("yes", or "challenge": polkit decides).
pub fn available() -> Vec<Action> {
  let Some(m) = manager() else { return vec![] };
  [Action::Suspend, Action::Reboot, Action::PowerOff]
    .into_iter()
    .filter(|a| {
      m.call_sync(&format!("Can{}", a.method()), None, gio::DBusCallFlags::NONE, 2000, gio::Cancellable::NONE)
        .ok()
        .and_then(|v| v.child_value(0).str().map(String::from))
        .is_some_and(|s| s == "yes" || s == "challenge")
    })
    .collect()
}

pub fn run(action: Action) {
  let Some(m) = manager() else { return };
  // false: not interactive, there is no one to ask.
  m.call(action.method(), Some(&(false,).to_variant()), gio::DBusCallFlags::NONE, -1, gio::Cancellable::NONE, |r| {
    if let Err(e) = r {
      glib::g_warning!("doxia-greeter", "power: {e}");
    }
  });
}
