// The clock (shell/plugins/panels/clock/BarWidget.qml): the date and time
// in the entry's Qt format ("format", or "verticalFormat" stacked on a side
// bar), redrawn on the minute. A right click walks the common formats and
// keeps the new one in shell.json; a middle click opens the timezone menu;
// a left click the calendar.

use super::super::config::{self, setting_str};
use super::super::widget::{Button, Kind, Slot};
use super::super::Ctx;
use gtk::glib;
use gtk::prelude::*;
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;

const FORMATS: [&str; 8] = [
  "dddd HH:mm",
  "dddd h:mm AP",
  "HH:mm",
  "h:mm AP",
  "ddd d MMM HH:mm",
  "ddd d MMM h:mm AP",
  "d MMMM 'W'ww yyyy",
  "yyyy-MM-dd HH:mm",
];
const VERTICAL_FORMATS: [&str; 4] = ["HH\n—\nmm", "h\n—\nmm\nAP", "dd\nMMM\n'W'ww\n''yy", "HH\nmm"];

// Model.clockFormatRing: the presets, then the configured alternate and
// current format when they are something else, in a fixed order.
pub fn ring(configured: &str, alt: &str, vertical: bool) -> Vec<String> {
  let presets: &[&str] = if vertical { &VERTICAL_FORMATS } else { &FORMATS };
  let mut out: Vec<String> = Vec::new();
  for f in presets.iter().copied().chain([alt, configured]) {
    if !f.is_empty() && !out.iter().any(|o| o == f) {
      out.push(f.into());
    }
  }
  out
}

pub fn next_format(ring: &[String], current: &str) -> String {
  match ring.iter().position(|f| f == current) {
    Some(i) => ring[(i + 1) % ring.len()].clone(),
    None => ring.first().cloned().unwrap_or_default(),
  }
}

// ISO 8601 week: the week owning the Thursday of the date's Monday week.
pub fn iso_week(dt: &glib::DateTime) -> i32 {
  dt.week_of_year()
}

// A Qt date format (QLocale::toString) for `dt`: d dd ddd dddd, M MM MMM
// MMMM, yy yyyy, h hh H HH, m mm, s ss, AP ap, 'quoted' text ('' a quote),
// and ww, the ISO week the clock substitutes before Qt sees it.
pub fn qt_format(dt: &glib::DateTime, format: &str) -> String {
  let chars: Vec<char> = format.chars().collect();
  let mut out = String::new();
  let mut i = 0;
  let f = |spec: &str| dt.format(spec).map(|s| s.to_string()).unwrap_or_default();
  let pm = dt.hour() >= 12;
  let ap = format.contains("AP") || format.contains("ap");
  while i < chars.len() {
    let c = chars[i];
    if c == '\'' {
      // '' is a quote; otherwise text up to the next quote.
      if chars.get(i + 1) == Some(&'\'') {
        out.push('\'');
        i += 2;
        continue;
      }
      i += 1;
      while i < chars.len() {
        if chars[i] == '\'' {
          if chars.get(i + 1) == Some(&'\'') {
            out.push('\'');
            i += 2;
            continue;
          }
          break;
        }
        out.push(chars[i]);
        i += 1;
      }
      i += 1;
      continue;
    }
    let mut n = 1;
    while chars.get(i + n) == Some(&c) {
      n += 1;
    }
    let hour12 = match dt.hour() % 12 { 0 => 12, h => h };
    let piece = match (c, n) {
      ('d', 1) => dt.day_of_month().to_string(),
      ('d', 2) => format!("{:02}", dt.day_of_month()),
      ('d', 3) => f("%a"),
      ('d', _) => f("%A"),
      ('M', 1) => dt.month().to_string(),
      ('M', 2) => format!("{:02}", dt.month()),
      ('M', 3) => f("%b"),
      ('M', _) => f("%B"),
      ('y', 2) => format!("{:02}", dt.year() % 100),
      ('y', 4) => dt.year().to_string(),
      ('H', 1) => dt.hour().to_string(),
      ('H', _) => format!("{:02}", dt.hour()),
      ('h', 1) => if ap { hour12.to_string() } else { dt.hour().to_string() },
      ('h', _) => if ap { format!("{hour12:02}") } else { format!("{:02}", dt.hour()) },
      ('m', 1) => dt.minute().to_string(),
      ('m', _) => format!("{:02}", dt.minute()),
      ('s', 1) => dt.second().to_string(),
      ('s', _) => format!("{:02}", dt.second()),
      ('w', 2) => format!("{:02}", iso_week(dt)),
      ('A', 1) if chars.get(i + 1) == Some(&'P') => {
        i += 2;
        out.push_str(if pm { "PM" } else { "AM" });
        continue;
      }
      ('a', 1) if chars.get(i + 1) == Some(&'p') => {
        i += 2;
        out.push_str(if pm { "pm" } else { "am" });
        continue;
      }
      _ => chars[i..i + n].iter().collect(),
    };
    out.push_str(&piece);
    i += n;
  }
  out
}

pub fn build(ctx: &Rc<Ctx>, entry: &Value) -> gtk::Widget {
  let vertical = ctx.vertical();
  let entry = Rc::new(RefCell::new(entry.clone()));
  let button = Button::new(ctx, Kind::Text, "");
  button.label.set_justify(gtk::Justification::Center);
  let slot = Slot::new(ctx, &button.widget);
  let format_key = if vertical { "verticalFormat" } else { "format" };
  let format = {
    let entry = entry.clone();
    move || {
      let e = entry.borrow();
      if vertical { setting_str(&e, "verticalFormat", "HH\n—\nmm") } else { setting_str(&e, "format", "dddd HH:mm") }
    }
  };
  let paint = {
    let (b, format) = (button.clone(), format.clone());
    move || {
      if let Ok(now) = glib::DateTime::now_local() {
        b.set_text(&qt_format(&now, &format()));
      }
    }
  };
  paint();
  // On the minute, then every minute.
  let p = paint.clone();
  let wait = glib::DateTime::now_local().map(|n| 60 - n.second() as u32).unwrap_or(60);
  let w = button.widget.downgrade();
  glib::timeout_add_seconds_local_once(wait, move || {
    p();
    glib::timeout_add_seconds_local(60, move || {
      if w.upgrade().is_none() {
        return glib::ControlFlow::Break;
      }
      p();
      glib::ControlFlow::Continue
    });
  });

  let calendar = super::calendar::Calendar::new(ctx, &button.widget, &slot, entry.clone());
  let p = paint.clone();
  button.on_press(move |b| match b {
    3 => {
      let mut e = entry.borrow_mut();
      let alt_key = if vertical { "verticalFormatAlt" } else { "formatAlt" };
      let alt_default = if vertical { "dd\nMMM\n'W'ww\n''yy" } else { "d MMMM 'W'ww yyyy" };
      let current = format();
      let next = next_format(&ring(&current, &setting_str(&e, alt_key, alt_default), vertical), &current);
      if next.is_empty() || next == current {
        return;
      }
      if !e.is_object() {
        *e = serde_json::json!({ "id": config::entry_id(&e) });
      }
      e[format_key] = Value::String(next);
      let written = e.clone();
      drop(e);
      p();
      config::update_entry(&written);
    }
    2 => crate::run::shell("omarchy-menu-timezone"),
    _ => calendar.toggle(),
  });
  slot.widget.clone().upcast()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn formats_like_qt() {
    let dt = glib::DateTime::from_local(2026, 1, 5, 9, 7, 3.0).unwrap();
    assert_eq!(qt_format(&dt, "dd/MM HH:mm"), "05/01 09:07");
    assert_eq!(qt_format(&dt, "d/M yy h:mm AP"), "5/1 26 9:07 AM");
    assert_eq!(qt_format(&dt, "'W'ww yyyy"), "W02 2026");
    assert_eq!(qt_format(&dt, "HH\n—\nmm"), "09\n—\n07");
    assert_eq!(qt_format(&dt, "dd\nMMM\n'W'ww\n''yy").lines().nth(3), Some("'26"));
    let r = ring("HH:mm", "d MMMM 'W'ww yyyy", false);
    assert_eq!(r.len(), 8);
    assert_eq!(next_format(&r, "HH:mm"), "h:mm AP");
    assert_eq!(next_format(&r, "yyyy-MM-dd HH:mm"), "dddd HH:mm");
    assert_eq!(next_format(&ring("dd/MM HH:mm", "x", false), "dd/MM HH:mm"), "dddd HH:mm");
  }
}
