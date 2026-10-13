// The on-screen display: volume, brightness and short status messages in a
// card at the bottom of the screen. Same payload, icons and behavior as the
// Quickshell OSD it replaces (shell/plugins/osd, OsdModel.js). No animations:
// on vmwgfx an animated overlay stalled mango's frames.

use gtk::prelude::*;
use gtk::glib;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

#[derive(Debug, PartialEq)]
pub struct State {
  pub icon_key: String,
  pub has_progress: bool,
  pub value: i64,
  pub max: i64,
  pub message: String,
  pub icon: String,
  pub duration: u64,
}

pub fn icon_for(name: &str, percent: i64) -> String {
  let n = name.to_lowercase();
  let glyph = match n.as_str() {
    "volume-muted" | "volume-mute" | "muted" | "mute" => "\u{eee8}",
    "volume-low" => "\u{f026}",
    "volume-medium" => "\u{f027}",
    "volume-high" | "volume" => "\u{f028}",
    "microphone-muted" | "microphone-off" | "mic-muted" | "mic-off" => "\u{f036d}",
    "microphone" | "mic" => "\u{f036c}",
    "keyboard" => "\u{f030c}",
    "brightness" | "display" => "\u{f0379}",
    "touchpad" => "\u{f07f8}",
    "touch" | "touchscreen" => "\u{f0741}",
    "reboot" | "restart" => "\u{f0709}",
    "shutdown" | "power" | "poweroff" => "\u{f0425}",
    "logout" | "sign-out" | "leave" => "\u{f0343}",
    "media" | "player" | "media-source" | "player-source" => "\u{f075a}",
    "media-play" | "player-play" => "\u{f040a}",
    "media-pause" | "player-pause" => "\u{f03e4}",
    "media-next" | "player-next" => "\u{f04ad}",
    "media-previous" | "player-previous" => "\u{f04ae}",
    // Any other name is the glyph itself.
    _ if !n.is_empty() => return name.to_string(),
    _ if percent <= 0 => "\u{eee8}",
    _ if percent <= 33 => "\u{f026}",
    _ if percent <= 66 => "\u{f027}",
    _ => "\u{f028}",
  };
  glyph.to_string()
}

fn field(p: &Value, key: &str) -> String {
  match p.get(key) {
    Some(Value::String(s)) => s.clone(),
    Some(Value::Null) | None => String::new(),
    Some(v) => v.to_string(),
  }
}

// OsdModel.stateForShow over the omarchy-osd payload.
pub fn state_for(payload: &Value) -> State {
  let raw_value = field(payload, "value");
  let raw_message = field(payload, "message");
  let max = field(payload, "max").parse::<i64>().unwrap_or(100).max(1);
  let parsed = raw_value.trim().parse::<i64>().ok();
  let has_progress = !raw_value.is_empty() && parsed.is_some() && raw_message.is_empty();
  let value = if has_progress { parsed.unwrap().clamp(0, max) } else { 0 };
  let percent = if has_progress { (value as f64 * 100.0 / max as f64).round() as i64 } else { -1 };
  let progress_text = field(payload, "progressText");
  let message = if !raw_message.is_empty() {
    raw_message
  } else if has_progress {
    if progress_text.is_empty() { format!("{percent}%") } else { progress_text }
  } else {
    String::new()
  };
  let duration = match field(payload, "duration") {
    d if d.is_empty() => 1200,
    d => d.parse::<i64>().map(|n| n.max(0) as u64).unwrap_or(1200),
  };
  let icon_key = field(payload, "icon");
  State {
    icon: icon_for(&icon_key, percent),
    icon_key: icon_key.to_lowercase(),
    has_progress,
    value,
    max,
    message,
    duration,
  }
}

pub struct Osd {
  window: gtk::Window,
  icon: gtk::Label,
  bar: gtk::ProgressBar,
  text: gtk::Label,
  serial: RefCell<u64>,
}

impl Osd {
  pub fn new(app: &gtk::Application) -> Rc<Osd> {
    let window = gtk::Window::builder().application(app).decorated(false).resizable(false).build();
    window.add_css_class("doxia");
    window.add_css_class("osd-window");
    window.init_layer_shell();
    // omarchy-* so mango's layer rules for the shell apply.
    window.set_namespace(Some("omarchy-osd"));
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::None);
    window.set_anchor(Edge::Bottom, true);
    window.set_margin(Edge::Bottom, 67);
    window.set_exclusive_zone(-1);

    let card = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    card.add_css_class("osd-card");
    let icon = gtk::Label::new(None);
    icon.add_css_class("osd-icon");
    let bar = gtk::ProgressBar::new();
    bar.add_css_class("osd-bar");
    bar.set_valign(gtk::Align::Center);
    let text = gtk::Label::new(None);
    text.add_css_class("osd-text");
    text.set_ellipsize(gtk::pango::EllipsizeMode::End);
    text.set_single_line_mode(true);
    card.append(&icon);
    card.append(&bar);
    card.append(&text);
    window.set_child(Some(&card));

    // Visual only: an empty input region lets clicks through to what is below.
    window.connect_realize(|w| {
      if let Some(surface) = w.surface() {
        surface.set_input_region(Some(&gtk::cairo::Region::create()));
      }
    });

    Rc::new(Osd { window, icon, bar, text, serial: RefCell::new(0) })
  }

  pub fn show_json(self: &Rc<Self>, payload: &str) {
    let payload: Value = serde_json::from_str(payload).unwrap_or_default();
    self.show(&state_for(&payload));
  }

  pub fn show(self: &Rc<Self>, s: &State) {
    let media = s.icon_key.starts_with("media") || s.icon_key.starts_with("player");
    self.icon.set_text(&s.icon);
    // Progress pins the icon column and the readout, so neither jitters as
    // the value changes; messages take the width they need, up to a limit.
    self.icon.set_width_request(if s.has_progress { 28 } else { -1 });
    self.bar.set_visible(s.has_progress);
    self.bar.set_fraction(if s.has_progress { s.value as f64 / s.max as f64 } else { 0.0 });
    self.text.set_visible(!s.message.is_empty());
    self.text.set_text(&s.message);
    if s.has_progress {
      self.text.set_width_chars(4);
      self.text.set_max_width_chars(4);
      self.text.set_xalign(1.0);
    } else {
      self.text.set_width_chars(-1);
      self.text.set_max_width_chars(if media { 38 } else { 22 });
      self.text.set_xalign(0.0);
    }
    if let Some(card) = self.window.child().and_downcast::<gtk::Box>() {
      card.set_spacing(if s.has_progress { 16 } else { 11 });
    }
    self.window.set_visible(true);

    let serial = {
      let mut n = self.serial.borrow_mut();
      *n += 1;
      *n
    };
    if s.duration > 0 {
      let me = self.clone();
      glib::timeout_add_local_once(Duration::from_millis(s.duration), move || {
        if *me.serial.borrow() == serial {
          me.window.set_visible(false);
        }
      });
    }
  }

  pub fn hide(&self) {
    *self.serial.borrow_mut() += 1;
    self.window.set_visible(false);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn progress_payloads() {
    let s = state_for(&json!({"icon": "", "message": "", "value": "40", "progressText": "40%", "max": "100", "duration": ""}));
    assert!(s.has_progress);
    assert_eq!((s.value, s.message.as_str(), s.icon.as_str(), s.duration), (40, "40%", "\u{f027}", 1200));
    let s = state_for(&json!({"value": "150", "max": "100"}));
    assert_eq!((s.value, s.message.as_str()), (100, "100%"));
  }

  #[test]
  fn messages_win_over_values() {
    let s = state_for(&json!({"icon": "media-play", "message": "Spotify", "value": "10"}));
    assert!(!s.has_progress);
    assert_eq!((s.message.as_str(), s.icon.as_str()), ("Spotify", "\u{f040a}"));
  }

  #[test]
  fn durations_and_unknown_icons() {
    assert_eq!(state_for(&json!({"duration": "0"})).duration, 0);
    assert_eq!(state_for(&json!({"duration": "x"})).duration, 1200);
    assert_eq!(state_for(&json!({"icon": "\u{f0e7}", "message": "a"})).icon, "\u{f0e7}");
    assert_eq!(icon_for("", 0), "\u{eee8}");
  }
}
