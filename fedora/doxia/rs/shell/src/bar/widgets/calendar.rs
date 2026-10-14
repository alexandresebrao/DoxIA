// The clock's calendar (shell/plugins/panels/clock/Panel.qml): today large
// over the year's progress (and, once a birth year is set by double-clicking
// that bar, a life one), then a six-row month grid with ISO week numbers
// and the month stepper. Chevrons, the wheel, the arrows (and h/j/k/l, [ ],
// { }) step months and years; t or Enter goes back to today; "W", or the
// grid's W heading, flips the week between Monday and Sunday starts.

use super::super::config::{self, setting};
use super::super::panel::Panel;
use super::super::widget::{self, Slot};
use super::super::Ctx;
use gtk::glib;
use gtk::prelude::*;
use gtk::gdk;
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub struct Day {
  pub year: i32,
  pub month: i32,
  pub day: i32,
  pub in_month: bool,
  pub weekend: bool,
  pub today: bool,
}

fn date(y: i32, m: i32, d: i32) -> glib::DateTime {
  glib::DateTime::from_local(y, m, d, 12, 0, 0.0).unwrap()
}

// Model.weekdayOrder etc. use JS weekdays: 0 Sunday .. 6 Saturday.
fn js_weekday(dt: &glib::DateTime) -> i32 {
  dt.day_of_week() % 7
}

pub fn iso_week(y: i32, m: i32, d: i32) -> i32 {
  date(y, m, d).week_of_year()
}

// Model.monthGrid: six rows of seven days from the week holding the 1st,
// each numbered by the ISO week owning its Thursday.
pub fn month_grid(year: i32, month: i32, week_start: i32, today: (i32, i32, i32)) -> Vec<(i32, Vec<Day>)> {
  let first = date(year, month, 1);
  let leading = (js_weekday(&first) - week_start).rem_euclid(7);
  let mut cursor = first.add_days(-leading).unwrap();
  let mut weeks = Vec::new();
  for _ in 0..6 {
    let mut days = Vec::new();
    let mut thursday = None;
    for _ in 0..7 {
      let (y, m, d) = (cursor.year(), cursor.month(), cursor.day_of_month());
      let wd = js_weekday(&cursor);
      if wd == 4 {
        thursday = Some((y, m, d));
      }
      days.push(Day { year: y, month: m, day: d, in_month: m == month && y == year, weekend: wd == 0 || wd == 6, today: (y, m, d) == today });
      cursor = cursor.add_days(1).unwrap();
    }
    let (ty, tm, td) = thursday.unwrap_or((days[0].year, days[0].month, days[0].day));
    weeks.push((iso_week(ty, tm, td), days));
  }
  weeks
}

pub fn step_month(year: i32, month: i32, delta: i32) -> (i32, i32) {
  let total = year * 12 + (month - 1) + delta;
  (total.div_euclid(12), total.rem_euclid(12) + 1)
}

// Share of the year behind you: whole days done over days in the year.
pub fn year_progress(y: i32, m: i32, d: i32) -> f64 {
  let doy = date(y, m, d).day_of_year() as f64;
  let total = date(y, 12, 31).day_of_year() as f64;
  ((doy - 1.0) / total).clamp(0.0, 1.0)
}

pub fn parse_birth_year(value: &Value, now: i32) -> i32 {
  let text = match value {
    Value::Number(n) => n.to_string(),
    Value::String(s) => s.trim().to_string(),
    _ => return 0,
  };
  if text.len() != 4 || !text.bytes().all(|b| b.is_ascii_digit()) {
    return 0;
  }
  let year: i32 = text.parse().unwrap_or(0);
  if year > now || year < now - 120 { 0 } else { year }
}

pub fn parse_life_expectancy(value: &Value) -> i32 {
  let text = match value {
    Value::Number(n) => n.to_string(),
    Value::String(s) => s.trim().to_string(),
    _ => return 90,
  };
  match text.parse::<i32>() {
    Ok(n) if (1..=150).contains(&n) && text.bytes().all(|b| b.is_ascii_digit()) => n,
    _ => 90,
  }
}

// Model.normalizedWeekStart: a weekday name, its first three letters or a
// number; anything else falls back (the locale's first day, Monday here).
pub fn week_start(value: Option<&Value>, fallback: i32) -> i32 {
  const NAMES: [&str; 7] = ["sunday", "monday", "tuesday", "wednesday", "thursday", "friday", "saturday"];
  match value {
    Some(Value::Number(n)) => n.as_f64().map(|f| (f.round() as i32).rem_euclid(7)).unwrap_or(fallback),
    Some(Value::String(s)) => {
      let t = s.trim().to_lowercase();
      NAMES.iter().position(|n| *n == t || (t.len() == 3 && n.starts_with(&t))).map(|i| i as i32).or_else(|| t.parse::<i32>().ok().map(|n| n.rem_euclid(7))).unwrap_or(fallback)
    }
    _ => fallback,
  }
}

pub struct Calendar {
  panel: Rc<Panel>,
  // The clock's entry, shared with it: either may write it back.
  entry: Rc<RefCell<Value>>,
  view: Cell<(i32, i32)>,
  editing: Cell<bool>,
  body: gtk::Box,
  ctx_style: doxia_theme::BarStyle,
}

fn today() -> (i32, i32, i32) {
  let now = glib::DateTime::now_local().unwrap();
  (now.year(), now.month(), now.day_of_month())
}

fn lbl(text: &str, class: &str) -> gtk::Label {
  let l = gtk::Label::new(Some(text));
  for c in class.split_whitespace() {
    l.add_css_class(c);
  }
  l
}

impl Calendar {
  pub fn new(ctx: &Rc<Ctx>, anchor: &impl IsA<gtk::Widget>, slot: &Rc<Slot>, entry: Rc<RefCell<Value>>) -> Rc<Calendar> {
    let panel = Panel::centered(ctx, anchor, slot, ctx.style.space(560.0));
    panel.content.add_css_class("calendar");
    let body = gtk::Box::new(gtk::Orientation::Vertical, ctx.style.space(8.0));
    panel.content.append(&body);
    let me = Rc::new(Calendar {
      panel,
      entry,
      view: Cell::new((today().0, today().1)),
      editing: Cell::new(false),
      body,
      ctx_style: ctx.style.clone(),
    });
    let weak = Rc::downgrade(&me);
    me.panel.connect_open(move || {
      if let Some(me) = weak.upgrade() {
        me.editing.set(false);
        let t = today();
        me.view.set((t.0, t.1));
        me.render();
      }
    });
    let weak = Rc::downgrade(&me);
    me.panel.connect_key(move |key, _| weak.upgrade().is_some_and(|me| me.key(key)));
    me
  }

  pub fn toggle(&self) {
    self.panel.toggle();
  }

  fn setting(&self, key: &str) -> Option<Value> {
    setting(&self.entry.borrow(), key).cloned()
  }

  fn week_start(&self) -> i32 {
    week_start(self.setting("weekStartDay").as_ref(), 1)
  }

  fn persist(&self, values: &[(&str, Value)]) {
    let mut e = self.entry.borrow_mut();
    if !e.is_object() {
      *e = serde_json::json!({ "id": config::entry_id(&e) });
    }
    for (k, v) in values {
      e[*k] = v.clone();
    }
    let written = e.clone();
    drop(e);
    config::update_entry(&written);
  }

  fn move_month(self: &Rc<Self>, delta: i32) {
    let (y, m) = self.view.get();
    self.view.set(step_month(y, m, delta));
    self.render();
  }

  fn go_today(self: &Rc<Self>) {
    let t = today();
    self.view.set((t.0, t.1));
    self.render();
  }

  fn toggle_week_start(self: &Rc<Self>) {
    const NAMES: [&str; 7] = ["sunday", "monday", "tuesday", "wednesday", "thursday", "friday", "saturday"];
    let next = if self.week_start() == 1 { 0 } else { 1 };
    self.persist(&[("weekStartDay", Value::String(NAMES[next as usize].into()))]);
    self.render();
  }

  fn key(self: &Rc<Self>, key: gdk::Key) -> bool {
    if self.editing.get() {
      return false;
    }
    use gdk::Key;
    match key {
      Key::Left | Key::h => self.move_month(-1),
      Key::Right | Key::l => self.move_month(1),
      Key::Up | Key::k => self.move_month(-12),
      Key::Down | Key::j => self.move_month(12),
      Key::bracketleft => self.move_month(-1),
      Key::bracketright => self.move_month(1),
      Key::braceleft => self.move_month(-12),
      Key::braceright => self.move_month(12),
      Key::Return | Key::KP_Enter | Key::space | Key::t | Key::T => self.go_today(),
      Key::w | Key::W => self.toggle_week_start(),
      _ => return false,
    }
    true
  }

  fn render(self: &Rc<Self>) {
    crate::popup::clear(&self.body);
    let s = &self.ctx_style;
    let t = today();
    let (vy, vm) = self.view.get();
    let viewing_today = (vy, vm) == (t.0, t.1);

    // Hero: today; once the view moved away, the way back.
    let hero = gtk::Box::new(gtk::Orientation::Horizontal, s.space(22.0));
    hero.set_halign(gtk::Align::Center);
    hero.add_css_class("calendar-hero");
    let icon = lbl("󰃭", "calendar-hero-icon");
    icon.set_valign(gtk::Align::Baseline);
    let now = glib::DateTime::now_local().unwrap();
    let date_text = super::clock::qt_format(&now, "d 'de' MMMM");
    let big = lbl(&date_text, "calendar-hero-date");
    big.set_valign(gtk::Align::Baseline);
    hero.append(&icon);
    hero.append(&big);
    if !viewing_today {
      hero.add_css_class("link");
      hero.set_cursor_from_name(Some("pointer"));
      hero.set_tooltip_text(Some("Back to today"));
      let weak = Rc::downgrade(self);
      widget::on_press(&hero, move |_| {
        if let Some(me) = weak.upgrade() {
          me.go_today();
        }
      });
    }
    self.body.append(&hero);

    let grid_width = s.space(32.0) + s.space(14.0) + 7 * s.space(52.0) + 8 * s.space(2.0);
    let rail = |name: &str, done: f64| {
      let row = gtk::Box::new(gtk::Orientation::Horizontal, s.space(12.0));
      row.set_halign(gtk::Align::Center);
      row.set_size_request(grid_width, -1);
      row.append(&lbl(name, "calendar-caption"));
      let track = gtk::DrawingArea::new();
      track.set_hexpand(true);
      track.set_content_height(s.space(6.0));
      track.set_valign(gtk::Align::Center);
      let round = s.radius > 0.0;
      track.set_draw_func(move |w, cr, width, height| {
        let fg = w.color();
        let (wd, h) = (width as f64, height as f64);
        let r = if round { h / 2.0 } else { 0.0 };
        widget::rounded(cr, 0.0, 0.0, wd, h, r);
        cr.set_source_rgba(fg.red() as f64, fg.green() as f64, fg.blue() as f64, 0.12);
        let _ = cr.fill();
        if done > 0.0 {
          widget::rounded(cr, 0.0, 0.0, (wd * done).round(), h, r);
          cr.set_source_rgba(fg.red() as f64, fg.green() as f64, fg.blue() as f64, 0.85);
          let _ = cr.fill();
        }
      });
      track.add_css_class("calendar-track");
      row.append(&track);
      row.append(&lbl(&format!("{}%", (done * 100.0).round()), "calendar-small"));
      row
    };

    // Year progress, or the life inputs while editing.
    if self.editing.get() {
      let row = gtk::Box::new(gtk::Orientation::Horizontal, s.space(10.0));
      row.set_halign(gtk::Align::Center);
      row.set_margin_top(s.space(6.0));
      row.append(&lbl("BORN", "calendar-caption"));
      let born = gtk::Entry::new();
      born.set_placeholder_text(Some("year"));
      born.set_width_chars(5);
      let birth = parse_birth_year(&self.setting("birthYear").unwrap_or_default(), t.0);
      if birth > 0 {
        born.set_text(&birth.to_string());
      }
      row.append(&born);
      row.append(&lbl("LIVE TO", "calendar-caption"));
      let span = gtk::Entry::new();
      span.set_placeholder_text(Some("90"));
      span.set_width_chars(4);
      span.set_text(&parse_life_expectancy(&self.setting("lifeExpectancy").unwrap_or_default()).to_string());
      row.append(&span);
      let commit = {
        let (weak, born, span) = (Rc::downgrade(self), born.clone(), span.clone());
        move || {
          let Some(me) = weak.upgrade() else { return };
          let year = parse_birth_year(&Value::String(born.text().to_string()), today().0);
          let life = parse_life_expectancy(&Value::String(span.text().to_string()));
          me.persist(&[("birthYear", Value::from(year)), ("lifeExpectancy", Value::from(life))]);
          me.editing.set(false);
          me.render();
        }
      };
      for entry in [&born, &span] {
        let c = commit.clone();
        entry.connect_activate(move |_| c());
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, k, _, _| {
          if k == gdk::Key::Escape {
            if let Some(me) = weak.upgrade() {
              me.editing.set(false);
              me.render();
            }
            return glib::Propagation::Stop;
          }
          glib::Propagation::Proceed
        });
        entry.add_controller(keys);
      }
      self.body.append(&row);
      born.grab_focus();
    } else {
      let row = rail(&t.0.to_string(), year_progress(t.0, t.1, t.2));
      row.set_margin_top(s.space(6.0));
      let tap = gtk::GestureClick::new();
      let weak = Rc::downgrade(self);
      tap.connect_pressed(move |_, n, _, _| {
        if n == 2 {
          if let Some(me) = weak.upgrade() {
            me.editing.set(true);
            me.render();
          }
        }
      });
      row.add_controller(tap);
      self.body.append(&row);
    }

    // Memento mori.
    let birth = parse_birth_year(&self.setting("birthYear").unwrap_or_default(), t.0);
    if birth > 0 {
      let age = t.0 - birth;
      let span = parse_life_expectancy(&self.setting("lifeExpectancy").unwrap_or_default());
      let row = rail("LIFE", (age as f64 / span as f64).clamp(0.0, 1.0));
      row.set_tooltip_text(Some("Memento Mori"));
      let tap = gtk::GestureClick::new();
      let weak = Rc::downgrade(self);
      tap.connect_pressed(move |_, n, _, _| {
        if n == 2 {
          if let Some(me) = weak.upgrade() {
            me.persist(&[("birthYear", Value::from(0))]);
            me.render();
          }
        }
      });
      row.add_controller(tap);
      self.body.append(&row);
    }

    // The month grid.
    let grid = gtk::Grid::new();
    grid.set_halign(gtk::Align::Center);
    grid.set_margin_top(s.space(18.0));
    grid.set_column_spacing(s.space(2.0) as u32);
    grid.set_row_spacing(s.space(3.0) as u32);
    grid.add_css_class("calendar-grid");
    let ws = self.week_start();
    let w_head = lbl("W", "calendar-week-head");
    w_head.set_size_request(s.space(32.0), s.space(16.0));
    w_head.set_cursor_from_name(Some("pointer"));
    let next_start = if ws == 1 { "Sunday" } else { "Monday" };
    w_head.set_tooltip_text(Some(&format!("Start weeks on {next_start}")));
    let weak = Rc::downgrade(self);
    widget::on_press(&w_head, move |_| {
      if let Some(me) = weak.upgrade() {
        me.toggle_week_start();
      }
    });
    grid.attach(&w_head, 0, 0, 1, 1);
    let gutter = gtk::Box::new(gtk::Orientation::Vertical, 0);
    gutter.set_size_request(s.space(14.0), -1);
    grid.attach(&gutter, 1, 0, 1, 7);
    let rule = gtk::Box::new(gtk::Orientation::Vertical, 0);
    rule.add_css_class("calendar-rule");
    rule.set_halign(gtk::Align::Center);
    rule.set_size_request(1, -1);
    gutter.append(&rule);
    rule.set_vexpand(true);
    rule.set_margin_top(s.space(16.0) + s.space(3.0));
    for i in 0..7 {
      let wd = (ws + i) % 7;
      // 2023-01-01 was a Sunday.
      let name = date(2023, 1, 1 + wd).format("%a").map(|s| s.to_string()).unwrap_or_default().to_uppercase();
      let l = lbl(&name.trim_end_matches('.').to_string(), "calendar-weekday");
      l.set_size_request(s.space(52.0), s.space(16.0));
      grid.attach(&l, 2 + i, 0, 1, 1);
    }
    for (row, (week, days)) in month_grid(vy, vm, ws, t).into_iter().enumerate() {
      let w = lbl(&week.to_string(), "calendar-week");
      w.set_size_request(s.space(32.0), s.space(34.0));
      grid.attach(&w, 0, row as i32 + 1, 1, 1);
      for (col, day) in days.iter().enumerate() {
        let mut class = String::from("calendar-day");
        if !day.in_month {
          class.push_str(" outside");
        } else if day.weekend {
          class.push_str(" weekend");
        }
        if day.today {
          class.push_str(" today");
        }
        let l = lbl(&day.day.to_string(), &class);
        l.set_size_request(s.space(52.0), s.space(34.0));
        grid.attach(&l, 2 + col as i32, row as i32 + 1, 1, 1);
      }
    }
    let weak = Rc::downgrade(self);
    widget::on_wheel(&grid, move |_, dy| {
      if dy == 0.0 {
        return;
      }
      if let Some(me) = weak.upgrade() {
        me.move_month(if dy > 0.0 { 1 } else { -1 });
      }
    });
    self.body.append(&grid);

    // The month stepper, spanning the grid.
    let nav = gtk::CenterBox::new();
    nav.set_halign(gtk::Align::Center);
    nav.set_size_request(grid_width + 2 * s.space(8.0), -1);
    let chevron = |glyph: &str, tip: &str, delta: i32| {
      let b = lbl(glyph, "panel-action");
      b.set_tooltip_text(Some(tip));
      b.set_cursor_from_name(Some("pointer"));
      let weak = Rc::downgrade(self);
      widget::on_press(&b, move |_| {
        if let Some(me) = weak.upgrade() {
          me.move_month(delta);
        }
      });
      b
    };
    nav.set_start_widget(Some(&chevron("󰅁", "Previous month", -1)));
    let month = super::clock::qt_format(&date(vy, vm, 1), "MMMM yyyy").to_uppercase();
    let ml = lbl(&month, "calendar-month");
    ml.set_size_request(s.space(130.0), -1);
    nav.set_center_widget(Some(&ml));
    nav.set_end_widget(Some(&chevron("󰅂", "Next month", 1)));
    self.body.append(&nav);
    self.panel.relayout();
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn grid_has_six_weeks_from_the_week_start() {
    // October 2026 starts on a Thursday.
    let g = month_grid(2026, 10, 1, (2026, 10, 13));
    assert_eq!(g.len(), 6);
    assert_eq!((g[0].1[0].month, g[0].1[0].day), (9, 28));
    assert_eq!(g[0].0, 40);
    assert!(g[2].1[1].today);
    let sunday = month_grid(2026, 10, 0, (0, 0, 0));
    assert_eq!((sunday[0].1[0].month, sunday[0].1[0].day), (9, 27));
    assert_eq!(step_month(2026, 1, -1), (2025, 12));
    assert_eq!(step_month(2026, 12, 1), (2027, 1));
    assert_eq!(week_start(Some(&Value::String("sun".into())), 1), 0);
    assert_eq!(week_start(Some(&Value::String("Monday".into())), 0), 1);
    assert_eq!(week_start(None, 1), 1);
    assert_eq!(parse_birth_year(&Value::String("1985".into()), 2026), 1985);
    assert_eq!(parse_birth_year(&Value::from(3000), 2026), 0);
    assert_eq!(parse_life_expectancy(&Value::String("abc".into())), 90);
    assert!((year_progress(2026, 1, 1)).abs() < 1e-9);
  }
}
