// A bar widget with a panel (audio, network, bluetooth, power, monitor):
// its icon button, its panel, and what the wheel and a right click on the
// icon do, so a group (omarchy.group) can stand in for its icon and show
// its panel as a tab.

use super::panel::Panel;
use super::widget::{Button, Slot};
use std::rc::Rc;

pub struct Member {
  pub id: String,
  pub title: String,
  pub button: Rc<Button>,
  pub slot: Rc<Slot>,
  pub panel: Rc<Panel>,
  // (dx, dy) from the wheel, dy > 0 down.
  pub wheel: Option<Rc<dyn Fn(f64, f64)>>,
  pub right: Option<Rc<dyn Fn()>>,
}

impl Member {
  // On the bar alone: the icon toggles the panel, the right click and the
  // wheel do what the member says.
  pub fn standalone(self) -> gtk::Widget {
    let (p, right) = (self.panel.clone(), self.right.clone());
    self.button.on_press(move |b| match b {
      3 => {
        if let Some(f) = &right {
          f();
        }
      }
      _ => p.toggle(),
    });
    if let Some(w) = self.wheel.clone() {
      self.button.on_wheel(move |dx, dy| w(dx, dy));
    }
    use gtk::prelude::*;
    self.slot.widget.clone().upcast()
  }
}
