// What the shell's summoned popups share (the Omarchy menu, the clipboard and
// emoji pickers): a full-screen overlay layer that takes the keyboard, a card
// drawn like BorderSurface on it, typed text through an input method (so
// dead keys and compose give "ç" and "ã"), the filter edits of Util.qml and
// a click outside the card to close.

use crate::card;
use gtk::prelude::*;
use gtk::{gdk, graphene};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::rc::Rc;

// A full-screen overlay with the keyboard; `namespace` keeps mango's layer
// rules for the shell's surfaces (omarchy-*).
pub fn window(app: &gtk::Application, namespace: &str, class: &str) -> gtk::Window {
  let window = gtk::Window::builder().application(app).decorated(false).build();
  window.add_css_class("doxia");
  window.add_css_class(class);
  window.init_layer_shell();
  window.set_namespace(Some(namespace));
  window.set_layer(Layer::Overlay);
  window.set_keyboard_mode(KeyboardMode::Exclusive);
  for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
    window.set_anchor(edge, true);
  }
  window.set_exclusive_zone(-1);
  window
}

// The [menu] card: the drawing under `content`, which sits `margin` inside.
pub struct Card {
  pub overlay: gtk::Overlay,
  pub surface: card::Surface,
}

impl Card {
  pub fn new(content: &impl IsA<gtk::Widget>, margin: i32) -> Card {
    let surface = card::Surface::new(doxia_theme::menu_card());
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&surface.area));
    content.set_margin_top(margin);
    content.set_margin_bottom(margin);
    content.set_margin_start(margin);
    content.set_margin_end(margin);
    overlay.add_overlay(content);
    Card { overlay, surface }
  }

  // The theme may have changed since the last time.
  pub fn refresh(&self) {
    self.surface.set(doxia_theme::menu_card());
  }

  pub fn contains(&self, root: &impl IsA<gtk::Widget>, x: f64, y: f64) -> bool {
    self.overlay.compute_bounds(root).is_some_and(|b| b.contains_point(&graphene::Point::new(x as f32, y as f32)))
  }
}

// The filter after an editing key, as Util.editsFilter/editedFilter: with
// text, Backspace drops a character (with Ctrl, a word) and Ctrl+U clears
// it. None: the key edits nothing.
pub fn edit_filter(filter: &str, key: gdk::Key, state: gdk::ModifierType) -> Option<String> {
  if filter.is_empty()
    || state.intersects(gdk::ModifierType::ALT_MASK | gdk::ModifierType::SUPER_MASK | gdk::ModifierType::META_MASK)
  {
    return None;
  }
  let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
  match key {
    gdk::Key::BackSpace if ctrl => Some(filter.trim_end().trim_end_matches(|c: char| !c.is_whitespace()).to_string()),
    gdk::Key::BackSpace => {
      let mut f = filter.to_string();
      f.pop();
      Some(f)
    }
    // Ctrl+Shift+U is Unicode input, not a clear.
    gdk::Key::u | gdk::Key::U if ctrl && !state.contains(gdk::ModifierType::SHIFT_MASK) => Some(String::new()),
    _ => None,
  }
}

// Keys and typed text for a popup window. `key` returns whether it used the
// key; `text` gets what the input method committed, control characters out.
pub fn connect_keys(
  window: &gtk::Window,
  im: &gtk::IMMulticontext,
  key: impl Fn(gdk::Key, gdk::ModifierType) -> bool + 'static,
  text: impl Fn(&str) + 'static,
) {
  let keys = gtk::EventControllerKey::new();
  im.set_client_widget(Some(window));
  keys.set_im_context(Some(im));
  im.connect_commit(move |_, committed| {
    let printable: String = committed.chars().filter(|c| !c.is_control()).collect();
    if !printable.is_empty() {
      text(&printable);
    }
  });
  keys.connect_key_pressed(move |_, k, _, state| if key(k, state) { gtk::glib::Propagation::Stop } else { gtk::glib::Propagation::Proceed });
  window.add_controller(keys);
}

// Calls `outside` for a click that lands off the card.
pub fn close_on_outside_click(root: &impl IsA<gtk::Widget>, card: Rc<Card>, outside: impl Fn() + 'static) {
  let click = gtk::GestureClick::new();
  let r = root.clone().upcast::<gtk::Widget>();
  click.connect_released(move |_, _, x, y| {
    if !card.contains(&r, x, y) {
      outside();
    }
  });
  root.add_controller(click);
}

// A yes/no question over the card, like shell/Ui/ConfirmDialog.qml: the
// confirming button is the one selected when it opens.
pub struct Confirm {
  pub widget: gtk::Box,
  text: gtk::Label,
  buttons: [gtk::Label; 2],
  index: std::cell::Cell<usize>,
}

impl Confirm {
  pub fn new(confirm: &str, margin: i32, answer: impl Fn(bool) + 'static) -> Rc<Confirm> {
    let widget = gtk::Box::new(gtk::Orientation::Vertical, 14);
    widget.add_css_class("popup-confirm");
    let inner = gtk::Box::new(gtk::Orientation::Vertical, 14);
    inner.set_valign(gtk::Align::Center);
    inner.set_vexpand(true);
    inner.set_margin_start(margin);
    inner.set_margin_end(margin);
    let text = label("", "popup-confirm-text");
    text.set_wrap(true);
    text.set_justify(gtk::Justification::Center);
    inner.append(&text);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.set_halign(gtk::Align::Center);
    let buttons = [label("Cancelar", "popup-confirm-button"), label(confirm, "popup-confirm-button")];
    buttons[1].add_css_class("destructive");
    let answer = Rc::new(answer);
    for (i, b) in buttons.iter().enumerate() {
      b.set_width_request(88);
      row.append(b);
      let click = gtk::GestureClick::new();
      let a = answer.clone();
      click.connect_released(move |g, _, _, _| {
        g.set_state(gtk::EventSequenceState::Claimed);
        a(i == 1);
      });
      b.add_controller(click);
    }
    inner.append(&row);
    widget.append(&inner);
    widget.set_visible(false);
    Rc::new(Confirm { widget, text, buttons, index: std::cell::Cell::new(1) })
  }

  pub fn ask(&self, message: &str) {
    self.text.set_text(message);
    self.index.set(1);
    self.paint();
    self.widget.set_visible(true);
  }

  pub fn hide(&self) {
    self.widget.set_visible(false);
  }

  pub fn open(&self) -> bool {
    self.widget.is_visible()
  }

  fn paint(&self) {
    for (i, b) in self.buttons.iter().enumerate() {
      if i == self.index.get() { b.add_css_class("selected") } else { b.remove_css_class("selected") }
    }
  }

  // While open it takes every key: Some(answer) when one was given.
  pub fn key(&self, key: gdk::Key) -> Option<bool> {
    match key {
      gdk::Key::Escape => Some(false),
      gdk::Key::Return | gdk::Key::KP_Enter => Some(self.index.get() == 1),
      gdk::Key::Left | gdk::Key::Right | gdk::Key::Tab | gdk::Key::ISO_Left_Tab => {
        self.index.set(1 - self.index.get());
        self.paint();
        None
      }
      _ => None,
    }
  }
}

// Clears a box (or any container) of its children.
pub fn clear(container: &impl IsA<gtk::Widget>) {
  while let Some(child) = container.first_child() {
    child.unparent();
  }
}

pub fn label(text: &str, class: &str) -> gtk::Label {
  let l = gtk::Label::new(Some(text));
  l.add_css_class(class);
  l
}

#[cfg(test)]
mod tests {
  use super::*;
  use gdk::{Key, ModifierType as M};

  #[test]
  fn filter_edits_follow_util_qml() {
    assert_eq!(edit_filter("foo bar", Key::BackSpace, M::empty()).as_deref(), Some("foo ba"));
    assert_eq!(edit_filter("foo bar", Key::BackSpace, M::CONTROL_MASK).as_deref(), Some("foo "));
    assert_eq!(edit_filter("foo", Key::u, M::CONTROL_MASK).as_deref(), Some(""));
    assert_eq!(edit_filter("foo", Key::U, M::CONTROL_MASK | M::SHIFT_MASK), None);
    assert_eq!(edit_filter("", Key::BackSpace, M::empty()), None);
    assert_eq!(edit_filter("foo", Key::BackSpace, M::ALT_MASK), None);
  }
}
