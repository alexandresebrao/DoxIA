// The night light's state for the bar and the CLI, replacing
// shell/services/nightlight: omarchy-nightlight-temperature reads and sets
// the screen's temperature; under 6000 K counts as on (4000 K on, 6500 K
// off, as bin/omarchy-toggle-nightlight). IPC target "nightlight": status,
// refresh, enable, disable, toggle.

use gtk::glib;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const NIGHT: u32 = 4000;
const DAY: u32 = 6500;
const IDENTITY: u32 = 6000;

pub fn temperature(output: &str) -> Option<u32> {
  let digits: String = output.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
  digits.parse().ok()
}

#[derive(Default)]
pub struct Nightlight {
  temperature: Cell<Option<u32>>,
  applying: Cell<bool>,
  pending: Cell<Option<u32>>,
  listeners: RefCell<Vec<Box<dyn Fn()>>>,
}

impl Nightlight {
  pub fn new() -> Rc<Nightlight> {
    let me = Rc::new(Nightlight::default());
    me.refresh();
    me
  }

  pub fn enabled(&self) -> bool {
    self.temperature.get().is_some_and(|t| t < IDENTITY)
  }

  pub fn connect(&self, f: impl Fn() + 'static) {
    self.listeners.borrow_mut().push(Box::new(f));
  }

  fn changed(&self) {
    for l in self.listeners.borrow().iter() {
      l();
    }
  }

  pub fn refresh(self: &Rc<Self>) {
    let me = self.clone();
    glib::spawn_future_local(async move {
      let out = crate::watch::output(&["omarchy-nightlight-temperature"]).await;
      me.temperature.set(out.as_deref().and_then(temperature));
      me.changed();
    });
  }

  // One apply at a time; the last one asked for while it runs goes next.
  fn apply(self: &Rc<Self>, t: u32) {
    self.temperature.set(Some(t));
    self.changed();
    self.pending.set(Some(t));
    if self.applying.replace(true) {
      return;
    }
    let me = self.clone();
    glib::spawn_future_local(async move {
      while let Some(t) = me.pending.take() {
        let _ = crate::watch::output(&["omarchy-nightlight-temperature", &t.to_string()]).await;
      }
      me.applying.set(false);
      me.refresh();
    });
  }

  pub fn set(self: &Rc<Self>, on: bool) {
    self.apply(if on { NIGHT } else { DAY });
  }

  pub fn ipc(self: &Rc<Self>, method: &str) -> Option<String> {
    Some(match method {
      "status" => serde_json::json!({ "enabled": self.enabled(), "temperature": self.temperature.get() }).to_string(),
      "refresh" => {
        self.refresh();
        "ok".into()
      }
      "enable" => {
        self.set(true);
        "enabled".into()
      }
      "disable" => {
        self.set(false);
        "disabled".into()
      }
      "toggle" => {
        let on = !self.enabled();
        self.set(on);
        if on { "enabled".into() } else { "disabled".into() }
      }
      _ => return None,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn reads_the_temperature() {
    assert_eq!(temperature("4000\n"), Some(4000));
    assert_eq!(temperature("temperature: 6500K"), Some(6500));
    assert_eq!(temperature("off"), None);
  }
}
