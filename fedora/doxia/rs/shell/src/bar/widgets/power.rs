// omarchy.power (shell/plugins/panels/power): the battery's icon (with its
// percentage while "showPercentage", which a right click toggles), only
// with a battery. Its panel: the battery's state in rotating phrases, its
// charge as a bar, size, cycles, time and rate (omarchy-battery-status),
// and the power profiles (omarchy-powerprofiles-list), each a button that
// keeps it for battery or AC (omarchy-powerprofiles-set).

use super::super::config::{self, setting};
use super::super::member::Member;
use super::super::panel::Panel;
use super::super::ui::{self, CursorRow};
use super::super::widget::{Button, Kind, Slot};
use super::super::Ctx;
use crate::upower::{self, UPower};
use gtk::glib;
use gtk::prelude::*;
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

const CHARGING_ICONS: [&str; 10] =
  ["\u{f089c}", "\u{f0086}", "\u{f0087}", "\u{f0088}", "\u{f089d}", "\u{f0089}", "\u{f089e}", "\u{f008a}", "\u{f008b}", "\u{f0085}"];
const ICONS: [&str; 10] = ["\u{f007a}", "\u{f007b}", "\u{f007c}", "\u{f007d}", "\u{f007e}", "\u{f007f}", "\u{f0080}", "\u{f0081}", "\u{f0082}", "\u{f0079}"];
const CHARGING_PHRASES: [&str; 9] = [
  "Pumping power", "Injecting electrons", "Pouring juice", "Amassing watts", "Hoarding joules", "Sucking volts", "Topping reserves", "Soaking amps", "Inhaling kilowatts",
];
const BATTERY_PHRASES: [&str; 9] = [
  "Slurping power", "Spending joules", "Draining watts", "Burning electrons", "Sipping juice", "Spending coulombs", "Bleeding amps", "Guzzling volts", "Munching reserves",
];

pub fn parse_key_value(raw: &str) -> HashMap<String, String> {
  raw.lines().filter_map(|l| l.split_once('\t')).filter(|(k, _)| !k.is_empty()).map(|(k, v)| (k.to_string(), v.trim().to_string())).collect()
}

// (profiles, active)
pub fn parse_profiles(raw: &str) -> (Vec<String>, String) {
  let mut list = Vec::new();
  let mut active = String::new();
  for line in raw.lines().map(str::trim).filter(|l| !l.is_empty()) {
    let mut parts = line.split('\t');
    let name = parts.next().unwrap_or("").to_string();
    if parts.next() == Some("1") {
      active = name.clone();
    }
    list.push(name);
  }
  (list, active)
}

pub fn profile_icon(name: &str) -> &'static str {
  match name {
    "power-saver" => "\u{f032a}",
    "balanced" => "\u{f029a}",
    "performance" => "\u{f04c5}",
    _ => "\u{f0084}",
  }
}

// Held at a charge limit: plugged, not discharging, and pending, or "full"
// short of full, or charging with no current to speak of.
pub fn threshold_active(present: bool, on_battery: bool, state: u32, fraction: f64, rate: f64, time_to_full: i64) -> bool {
  if !present || on_battery || state == upower::DISCHARGING {
    return false;
  }
  if state == upower::PENDING_CHARGE {
    return true;
  }
  if state == upower::FULLY_CHARGED && fraction < 0.99 {
    return true;
  }
  if state != upower::CHARGING || fraction >= 0.99 {
    return false;
  }
  rate <= 0.2 || time_to_full >= 8 * 3600
}

fn threshold(up: &UPower) -> bool {
  threshold_active(up.present(), up.on_battery(), up.state(), up.fraction(), up.energy_rate(), up.time_to_full())
}

pub fn battery_icon(up: &UPower) -> &'static str {
  if !up.present() {
    return "";
  }
  let i = ((up.fraction() * 10.0).floor() as usize).min(9);
  if threshold(up) {
    ICONS[i]
  } else if up.state() == upower::FULLY_CHARGED {
    "\u{f0085}"
  } else if !up.on_battery() {
    CHARGING_ICONS[i]
  } else {
    ICONS[i]
  }
}

fn mode_label(up: &UPower) -> &'static str {
  if !up.present() {
    ""
  } else if threshold(up) {
    "Threshold"
  } else if up.on_battery() {
    "On battery"
  } else if up.fraction() >= 1.0 {
    "Fully charged"
  } else {
    "Charging"
  }
}

pub fn build(ctx: &Rc<Ctx>, entry: &Value) -> Member {
  let space = { let s = ctx.style.clone(); move |px: f64| s.space(px) };
  let entry = Rc::new(RefCell::new(entry.clone()));
  let up = upower::get();
  let button = Button::new(ctx, Kind::Icon, "");
  let slot = Slot::new(ctx, &button.widget);
  let panel = Panel::new(ctx, &button.widget, &slot, space(380.0));
  let palette = doxia_theme::palette();

  let body = ui::vbox(space(14.0));
  let hero = ui::hbox(space(14.0));
  let hero_icon = ui::label("", "panel-hero-icon");
  hero.append(&hero_icon);
  let labels = ui::vbox(space(2.0));
  labels.set_hexpand(true);
  labels.set_valign(gtk::Align::Center);
  let title = ui::label("Battery", "panel-hero-title");
  title.set_xalign(0.0);
  labels.append(&title);
  let status = ui::label("", "panel-hero-subtitle");
  status.set_xalign(0.0);
  labels.append(&status);
  hero.append(&labels);
  let percent = ui::label("—", "power-percent");
  hero.append(&percent);
  body.append(&hero);
  let rail = ui::Rail::new(space(8.0), palette.foreground);
  body.append(&rail.widget);
  let info = gtk::Grid::new();
  info.set_column_spacing(space(20.0) as u32);
  info.set_row_spacing(space(4.0) as u32);
  info.set_column_homogeneous(true);
  let pair = |col: i32, row: i32| {
    let b = ui::hbox(space(8.0));
    let l = ui::label("", "power-info-label");
    let v = ui::label("", "power-info-value");
    l.set_hexpand(true);
    l.set_xalign(0.0);
    b.append(&l);
    b.append(&v);
    info.attach(&b, col, row, 1, 1);
    (l, v)
  };
  let (size_l, size_v) = pair(0, 0);
  let (cycles_l, cycles_v) = pair(0, 1);
  let (time_l, time_v) = pair(1, 0);
  let (rate_l, rate_v) = pair(1, 1);
  size_l.set_text("Battery size");
  cycles_l.set_text("Charge cycles");
  body.append(&info);
  body.append(&ui::separator());
  let profiles_col = ui::vbox(space(10.0));
  profiles_col.append(&ui::section("Power profile"));
  let profile_row = ui::hbox(space(6.0));
  profile_row.set_homogeneous(true);
  profiles_col.append(&profile_row);
  body.append(&profiles_col);
  panel.content.append(&body);

  let battery: Rc<RefCell<HashMap<String, String>>> = Rc::default();
  let profiles: Rc<RefCell<(Vec<String>, String)>> = Rc::default();
  let phrase = Rc::new(Cell::new(0usize));
  let cursor = ui::Cursor::new();

  let paint: Rc<dyn Fn()> = {
    let (up, b, entry, battery, profiles, phrase) = (up.clone(), button.clone(), entry.clone(), battery.clone(), profiles.clone(), phrase.clone());
    let (hero_icon, status, percent, rail, info, time_l, time_v, rate_l, rate_v, size_v, cycles_v, profile_row, cursor) =
      (hero_icon.clone(), status.clone(), percent.clone(), rail.clone(), info.clone(), time_l.clone(), time_v.clone(), rate_l.clone(), rate_v.clone(), size_v.clone(), cycles_v.clone(), profile_row.clone(), cursor.clone());
    let vertical = ctx.vertical();
    let icon_slot = ctx.style.icon_slot;
    let panel_w = Rc::downgrade(&panel);
    Rc::new(move || {
      let present = up.present();
      b.widget.set_visible(present);
      if !present {
        if let Some(p) = panel_w.upgrade() {
          p.close();
        }
        return;
      }
      let show_pct = setting(&entry.borrow(), "showPercentage").and_then(Value::as_bool) == Some(true) && !vertical;
      let icon = battery_icon(&up);
      b.set_text(&if show_pct { format!("{}% {icon}", (up.fraction() * 100.0).round()) } else { icon.to_string() });
      b.widget.set_size_request(if show_pct { icon_slot * 2 } else { icon_slot }, -1);
      let th = threshold(&up);
      let full = up.state() == upower::FULLY_CHARGED && !th;
      let discharging = up.on_battery();
      let idle = full || (!discharging && up.fraction() >= 1.0) || th;
      let charging = !discharging && !idle;
      hero_icon.set_text(icon);
      let phrases: &[&str] = if full { &[] } else if charging { &CHARGING_PHRASES } else if discharging { &BATTERY_PHRASES } else { &[] };
      let text = if full { "Fully charged" } else if !phrases.is_empty() { phrases[phrase.get() % phrases.len()] } else { mode_label(&up) };
      status.set_text(&text.to_uppercase());
      rail.set(up.fraction());
      let bat = battery.borrow();
      percent.set_text(bat.get("percentage").map(String::as_str).unwrap_or("—"));
      info.set_visible(bat.contains_key("percentage"));
      size_v.set_text(bat.get("size").map(String::as_str).unwrap_or(""));
      cycles_v.set_text(bat.get("cycles").map(String::as_str).unwrap_or("—"));
      time_l.set_text(if th { "Charge limit" } else if discharging { "Time left" } else { "Time to full" });
      time_v.set_text(if th { bat.get("threshold").map(String::as_str).unwrap_or("-") } else if idle { "-" } else { bat.get("time").map(String::as_str).unwrap_or("—") });
      rate_l.set_text(if th { "Battery state" } else if discharging { "Discharging" } else { "Charging" });
      rate_v.set_text(if th { "Holding" } else if idle { "-" } else { bat.get("rate").map(String::as_str).unwrap_or("") });
      drop(bat);
      crate::popup::clear(&profile_row);
      cursor.clear();
      let (list, active) = profiles.borrow().clone();
      for name in list {
        let mut label = name.clone();
        if let Some(c) = label.get_mut(0..1) {
          c.make_ascii_uppercase();
        }
        let source = if discharging { "battery" } else { "ac" };
        let n = name.clone();
        let btn = ui::button(&format!("{}  {label}", profile_icon(&name)), false, move || {
          crate::run::argv(&["omarchy-powerprofiles-set", source, &n]);
        });
        btn.add_css_class("profile");
        if name == active {
          btn.add_css_class("selected");
        }
        profile_row.append(&btn);
        let b2 = btn.clone();
        cursor.add(CursorRow { widget: btn.upcast(), activate: Some(Rc::new(move || b2.emit_clicked())), adjust: None, mute: None });
      }
      if let Some(p) = panel_w.upgrade() {
        p.relayout();
      }
    })
  };
  let refresh: Rc<dyn Fn()> = {
    let (battery, profiles, paint) = (battery.clone(), profiles.clone(), paint.clone());
    Rc::new(move || {
      let (battery, profiles, paint) = (battery.clone(), profiles.clone(), paint.clone());
      glib::spawn_future_local(async move {
        let b = crate::watch::output(&["omarchy-battery-status", "--shell"]).await.unwrap_or_default();
        let kv = parse_key_value(&b);
        if !kv.is_empty() {
          *battery.borrow_mut() = kv;
        }
        let p = crate::watch::output(&["omarchy-powerprofiles-list", "--active-state"]).await.unwrap_or_default();
        let parsed = parse_profiles(&p);
        if !parsed.0.is_empty() {
          *profiles.borrow_mut() = parsed;
        }
        paint();
      });
    })
  };
  paint();
  let (p, w) = (paint.clone(), button.widget.downgrade());
  up.connect(move || {
    if w.upgrade().is_some() {
      p();
    }
  });
  {
    let (r, panel_w, c, phrase, p) = (refresh.clone(), Rc::downgrade(&panel), cursor.clone(), phrase.clone(), paint.clone());
    panel.connect_open(move || {
      c.reset();
      let (r, panel_w, phrase, p) = (r.clone(), panel_w.clone(), phrase.clone(), p.clone());
      glib::spawn_future_local(async move {
        let mut ticks = 0u32;
        while panel_w.upgrade().is_some_and(|p| p.is_open()) {
          if ticks % 5 == 0 {
            r();
          }
          glib::timeout_future(Duration::from_millis(1000)).await;
          ticks += 1;
          // The phrase turns every 2.8 s or so.
          if ticks % 3 == 0 {
            phrase.set(phrase.get() + 1);
            p();
          }
        }
      });
    });
    let c = cursor.clone();
    panel.connect_key(move |k, _| c.key(k));
  }
  let right: Rc<dyn Fn()> = {
    let (entry, p) = (entry.clone(), paint.clone());
    Rc::new(move || {
      let mut e = entry.borrow_mut();
      if !e.is_object() {
        *e = serde_json::json!({ "id": config::entry_id(&e) });
      }
      let on = setting(&e, "showPercentage").and_then(Value::as_bool) == Some(true);
      e["showPercentage"] = Value::Bool(!on);
      let written = e.clone();
      drop(e);
      config::update_entry(&written);
      p();
    })
  };
  if ctx.first_screen {
    if let Some(bar) = ctx.bar.upgrade() {
      let (p, r) = (Rc::downgrade(&panel), right.clone());
      bar.add_ipc("omarchy.power", Box::new(move |method, _| {
        let panel = p.upgrade()?;
        match method {
          "togglePercentage" => r(),
          "open" | "show" => panel.open(),
          "close" | "hide" => panel.close(),
          "toggle" => panel.toggle(),
          _ => return None,
        }
        Some("ok".into())
      }));
    }
  }
  Member { id: "omarchy.power".into(), title: "Bateria".into(), button, slot, panel, wheel: None, right: Some(right) }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn battery_parsing_and_threshold() {
    assert_eq!(parse_key_value("percentage\t80%\nbad\nsize\t50 Wh\n").get("size").map(String::as_str), Some("50 Wh"));
    assert_eq!(parse_profiles("power-saver\t0\nbalanced\t1\n"), (vec!["power-saver".into(), "balanced".into()], "balanced".into()));
    assert!(threshold_active(true, false, upower::PENDING_CHARGE, 0.8, 5.0, 100));
    assert!(threshold_active(true, false, upower::CHARGING, 0.8, 0.1, 100));
    assert!(!threshold_active(true, false, upower::CHARGING, 0.8, 10.0, 600));
    assert!(!threshold_active(true, true, upower::DISCHARGING, 0.5, 10.0, 0));
  }
}
