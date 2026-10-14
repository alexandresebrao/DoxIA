// The reminder prompt, replacing shell/plugins/reminders (ReminderFlow.qml):
// a one-line card that asks for the minutes, then for the message, and hands
// both to omarchy-reminder. Opened with `omarchy-shell shell summon
// omarchy.reminders` (omarchy-reminder with no arguments).

use crate::popup::{self, label};
use gtk::gdk;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

const WIDTH: i32 = 300;
const MARGIN: i32 = 18 + 2;
const HEADER: i32 = 34;

// ReminderFlowModel.validMinutes: a positive whole number, as typed.
pub fn valid_minutes(value: &str) -> Option<String> {
  let minutes = value.trim();
  let digits = !minutes.is_empty() && minutes.bytes().all(|b| b.is_ascii_digit());
  (digits && minutes.bytes().any(|b| b != b'0')).then(|| minutes.to_string())
}

#[derive(Default, PartialEq)]
enum Step {
  #[default]
  Minutes,
  Message,
}

#[derive(Default)]
struct State {
  opened: bool,
  step: Step,
  minutes: String,
  filter: String,
}

pub struct Reminders {
  window: gtk::Window,
  root: gtk::Box,
  card: Rc<popup::Card>,
  text: gtk::Label,
  im: gtk::IMMulticontext,
  st: RefCell<State>,
}

impl Reminders {
  pub fn new(app: &gtk::Application) -> Rc<Reminders> {
    let window = popup::window(app, "doxia-reminders", "reminders-window");
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let text = label("", "menu-header");
    text.set_xalign(0.0);
    text.set_ellipsize(gtk::pango::EllipsizeMode::End);
    text.set_height_request(HEADER);
    let card = Rc::new(popup::Card::new(&text, MARGIN));
    card.overlay.set_halign(gtk::Align::Center);
    card.overlay.set_valign(gtk::Align::Center);
    card.overlay.set_vexpand(true);
    root.append(&card.overlay);
    window.set_child(Some(&root));

    let me = Rc::new(Reminders { window, root, card, text, im: gtk::IMMulticontext::new(), st: RefCell::default() });
    me.connect_input();
    me
  }

  pub fn ipc(self: &Rc<Self>, method: &str, _args: &[String]) -> Option<String> {
    match method {
      "summon" => self.open(),
      "toggle" => {
        if self.st.borrow().opened { self.close() } else { self.open() }
      }
      "hide" => self.close(),
      "call" => {}
      _ => return None,
    }
    Some("ok".into())
  }

  fn open(&self) {
    self.card.refresh();
    let (w, _) = crate::emojis::screen();
    // The card's size is set: the text over it adds nothing to it.
    self.card.overlay.set_size_request(WIDTH.min(w - 2 * crate::menu::gaps_out()), MARGIN * 2 + HEADER);
    *self.st.borrow_mut() = State { opened: true, ..State::default() };
    self.paint();
    self.window.set_visible(true);
    self.im.focus_in();
  }

  fn close(&self) {
    self.st.borrow_mut().opened = false;
    self.im.focus_out();
    self.window.set_visible(false);
  }

  fn set_filter(&self, filter: String) {
    self.st.borrow_mut().filter = filter;
    self.paint();
  }

  fn paint(&self) {
    let st = self.st.borrow();
    let prompt = if st.step == Step::Message { "Reminder message" } else { "Remind in minutes" };
    if st.filter.is_empty() {
      self.text.set_text(&format!("{prompt}..."));
      self.text.add_css_class("placeholder");
    } else {
      self.text.set_text(&st.filter);
      self.text.remove_css_class("placeholder");
    }
  }

  fn submit(&self) {
    let (asking_minutes, filter) = {
      let st = self.st.borrow();
      (st.step == Step::Minutes, st.filter.clone())
    };
    let bin = |name: &str| format!("{}/bin/{name}", std::env::var("OMARCHY_PATH").unwrap_or_default());
    if asking_minutes {
      if filter.trim().is_empty() {
        return self.close();
      }
      let Some(minutes) = valid_minutes(&filter) else {
        crate::run::argv(&[&bin("omarchy-notification-send"), "Invalid reminder", "Enter the number of minutes"]);
        return;
      };
      *self.st.borrow_mut() = State { opened: true, step: Step::Message, minutes, filter: String::new() };
      self.paint();
      return;
    }
    let minutes = self.st.borrow().minutes.clone();
    self.close();
    let reminder = bin("omarchy-reminder");
    let mut args = vec![reminder.as_str(), minutes.as_str()];
    if !filter.is_empty() {
      args.push(&filter);
    }
    crate::run::argv(&args);
  }

  fn connect_input(self: &Rc<Self>) {
    let me = Rc::downgrade(self);
    let typed = Rc::downgrade(self);
    popup::connect_keys(
      &self.window,
      &self.im,
      move |key, state| me.upgrade().is_some_and(|me| me.key(key, state)),
      move |text| {
        if let Some(me) = typed.upgrade() {
          let filter = me.st.borrow().filter.clone();
          me.set_filter(filter + text);
        }
      },
    );
    let me = Rc::downgrade(self);
    popup::close_on_outside_click(&self.root, self.card.clone(), move || {
      if let Some(me) = me.upgrade() {
        me.close();
      }
    });
  }

  fn key(&self, key: gdk::Key, state: gdk::ModifierType) -> bool {
    let filter = self.st.borrow().filter.clone();
    if key == gdk::Key::Escape {
      if filter.is_empty() { self.close() } else { self.set_filter(String::new()) }
      return true;
    }
    if let Some(next) = popup::edit_filter(&filter, key, state) {
      self.set_filter(next);
      return true;
    }
    if matches!(key, gdk::Key::Return | gdk::Key::KP_Enter) {
      self.submit();
      return true;
    }
    false
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn minutes_are_positive_whole_numbers() {
    assert_eq!(valid_minutes(" 15 ").as_deref(), Some("15"));
    assert_eq!(valid_minutes("007").as_deref(), Some("007"));
    assert_eq!(valid_minutes("0"), None);
    assert_eq!(valid_minutes("00"), None);
    assert_eq!(valid_minutes("1.5"), None);
    assert_eq!(valid_minutes("-3"), None);
    assert_eq!(valid_minutes(""), None);
  }
}
