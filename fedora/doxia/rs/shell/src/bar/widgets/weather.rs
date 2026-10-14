// omarchy.weather (shell/plugins/panels/weather): the current conditions'
// glyph in the bar; a click opens the forecast, centered on the bar: the
// temperature large with the feel, wind and humidity, the next 24 hours as
// a temperature line over rain bars (or a picked day's hours), and the next
// three days. Clicking the location edits it, with Open-Meteo's geocoding
// as you type. A right click sends the conditions as a notification, a
// middle click refreshes.
//
// The data is fetched once for the shell (wttr.in for the place and its
// conditions, Open-Meteo for the forecast, faster and with day and night),
// every "refreshMinutes" (15).

use super::super::config::setting_str;
use super::super::panel::Panel;
use super::super::ui;
use super::super::widget::{self, Button, Kind, Slot};
use super::super::Ctx;
use super::weather_model as m;
use gtk::prelude::*;
use gtk::{gdk, gio, glib, pango};
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

#[derive(Default)]
struct State {
  location: m::Location,
  wttr: Option<Value>,
  meteo: Option<Value>,
  wttr_location: String,
  icon: String,
}

#[derive(Default)]
pub struct Weather {
  st: RefCell<State>,
  listeners: RefCell<Vec<Box<dyn Fn()>>>,
  fetching: Cell<bool>,
  started: Cell<bool>,
  monitor: RefCell<Option<gio::FileMonitor>>,
}

thread_local! {
  static WEATHER: Rc<Weather> = Rc::new(Weather::default());
}

fn location_path() -> String {
  format!("{}/.local/state/omarchy/settings/weather.json", std::env::var("HOME").unwrap_or_default())
}

fn today() -> String {
  glib::DateTime::now_local().ok().and_then(|d| d.format("%Y-%m-%d").ok()).map(|s| s.to_string()).unwrap_or_default()
}

async fn curl(timeout: u32, url: &str) -> Option<Value> {
  let out = crate::watch::output(&["curl", "-fsS", "--max-time", &timeout.to_string(), url]).await?;
  serde_json::from_str(out.trim()).ok()
}

impl Weather {
  fn get(refresh_minutes: u64) -> Rc<Weather> {
    let me = WEATHER.with(|w| w.clone());
    if !me.started.replace(true) {
      me.read_location();
      let file = gio::File::for_path(location_path());
      if let Ok(mon) = file.monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
        let w = Rc::downgrade(&me);
        mon.connect_changed(move |_, _, _, _| {
          if let Some(me) = w.upgrade() {
            if me.read_location() {
              me.refresh();
            }
          }
        });
        *me.monitor.borrow_mut() = Some(mon);
      }
      me.refresh();
      let w = Rc::downgrade(&me);
      glib::timeout_add_seconds_local((refresh_minutes.max(1) * 60) as u32, move || {
        let Some(me) = w.upgrade() else { return glib::ControlFlow::Break };
        me.refresh();
        glib::ControlFlow::Continue
      });
    }
    me
  }

  // True when the location changed.
  fn read_location(&self) -> bool {
    let loc = m::parse_location_file(&std::fs::read_to_string(location_path()).unwrap_or_default());
    let changed = self.st.borrow().location != loc;
    self.st.borrow_mut().location = loc;
    changed
  }

  fn changed(&self) {
    for l in self.listeners.borrow().iter() {
      l();
    }
  }

  fn connect(&self, f: impl Fn() + 'static) {
    self.listeners.borrow_mut().push(Box::new(f));
  }

  fn has_coordinates(&self) -> bool {
    let st = self.st.borrow();
    st.location.latitude.is_some() && st.location.longitude.is_some()
  }

  pub fn refresh(self: &Rc<Self>) {
    if self.fetching.replace(true) {
      return;
    }
    let me = self.clone();
    glib::spawn_future_local(async move {
      // Stored coordinates go straight to Open-Meteo; otherwise wttr.in
      // finds the place first. Each tries three times, 2.5 s apart.
      if me.has_coordinates() {
        me.fetch_meteo(None).await;
      }
      let query = m::wttr_query(&me.st.borrow().location);
      if query.is_empty() {
        let raw = crate::watch::output(&["curl", "-fsS", "--max-time", "4", "https://wttr.in/?format=%l"]).await.unwrap_or_default();
        if let Some(place) = raw.trim().split(',').next().filter(|s| !s.is_empty()) {
          me.st.borrow_mut().wttr_location = place.to_string();
        }
      }
      for attempt in 0..4 {
        if let Some(report) = curl(10, &format!("https://wttr.in/{query}?format=j1")).await {
          if !me.has_coordinates() && me.st.borrow().icon.is_empty() {
            let icon = m::wttr_current(&report).map(|c| c.icon).unwrap_or_default();
            me.st.borrow_mut().icon = icon;
          }
          me.st.borrow_mut().wttr = Some(report.clone());
          me.changed();
          if !me.has_coordinates() {
            me.fetch_meteo(Some(&report)).await;
          }
          break;
        }
        if attempt < 3 {
          glib::timeout_future(Duration::from_millis(2500)).await;
        }
      }
      me.fetching.set(false);
    });
  }

  async fn fetch_meteo(&self, wttr: Option<&Value>) {
    let (lat, lon) = {
      let st = self.st.borrow();
      match (st.location.latitude, st.location.longitude) {
        (Some(a), Some(b)) => (a, b),
        _ => {
          let area = wttr.map(|w| &w["nearest_area"][0]);
          let n = |k: &str| area.and_then(|a| a[k].as_str()).and_then(|s| s.parse::<f64>().ok());
          match (n("latitude"), n("longitude")) {
            (Some(a), Some(b)) => (a, b),
            _ => return,
          }
        }
      }
    };
    let url = format!(
      "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}\
       &daily=weather_code,temperature_2m_max,temperature_2m_min\
       &hourly=temperature_2m,precipitation,precipitation_probability\
       &current=temperature_2m,apparent_temperature,relative_humidity_2m,wind_speed_10m,weather_code,is_day\
       &forecast_days=4&timezone=auto"
    );
    for attempt in 0..4 {
      if let Some(report) = curl(5, &url).await {
        if let Some(cur) = m::open_meteo_current(&report) {
          if !cur.icon.is_empty() {
            self.st.borrow_mut().icon = cur.icon;
          }
        }
        self.st.borrow_mut().meteo = Some(report);
        self.changed();
        return;
      }
      if attempt < 3 {
        glib::timeout_future(Duration::from_millis(2500)).await;
      }
    }
  }

  fn current(&self) -> Option<m::Current> {
    let st = self.st.borrow();
    let meteo = st.meteo.as_ref().and_then(m::open_meteo_current);
    let has = st.location.latitude.is_some();
    if has && meteo.is_some() {
      return meteo;
    }
    st.wttr.as_ref().and_then(m::wttr_current).or(meteo)
  }

  fn place(&self) -> String {
    let st = self.st.borrow();
    if !st.location.name.is_empty() {
      return st.location.name.clone();
    }
    if !st.wttr_location.is_empty() {
      return st.wttr_location.clone();
    }
    st.wttr.as_ref().and_then(|w| w["nearest_area"][0]["areaName"][0]["value"].as_str()).unwrap_or("").to_string()
  }

  fn country(&self) -> String {
    let st = self.st.borrow();
    st.wttr.as_ref().and_then(|w| w["nearest_area"][0]["country"][0]["value"].as_str()).unwrap_or("").to_string()
  }
}

fn day_name(date: &str) -> String {
  let parts: Vec<i32> = date.get(..10).unwrap_or("").split('-').filter_map(|p| p.parse().ok()).collect();
  let [y, mo, d] = parts[..] else { return String::new() };
  let Ok(dt) = glib::DateTime::from_local(y, mo, d, 12, 0, 0.0) else { return String::new() };
  // Qt's pt_BR "dddd": the full name ("terça-feira"), capitalized.
  const NAMES: [&str; 7] = ["Domingo", "Segunda-feira", "Terça-feira", "Quarta-feira", "Quinta-feira", "Sexta-feira", "Sábado"];
  NAMES[(dt.day_of_week() % 7) as usize].to_string()
}

pub fn build(ctx: &Rc<Ctx>, entry: &Value) -> gtk::Widget {
  let refresh_minutes = setting_str(entry, "refreshMinutes", "15").parse::<u64>().unwrap_or(15);
  let unit = setting_str(entry, "unit", "");
  let weather = Weather::get(refresh_minutes);
  let space = { let s = ctx.style.clone(); move |px: f64| s.space(px) };
  let palette = doxia_theme::palette();

  let button = Button::new(ctx, Kind::Status, "");
  button.widget.set_visible(false);
  let slot = Slot::new(ctx, &button.widget);
  let panel = Panel::centered(ctx, &button.widget, &slot, space(480.0));
  panel.content.add_css_class("weather");

  // ---- Hero: glyph and temperature; location and stats on the right.
  let body = ui::vbox(space(14.0));
  let hero = ui::hbox(space(20.0));
  let left = ui::hbox(space(16.0));
  left.set_margin_start(space(16.0));
  left.set_hexpand(true);
  let hero_icon = ui::label("—", "weather-hero-icon");
  hero_icon.set_margin_top(10);
  left.append(&hero_icon);
  let temp_row = ui::hbox(space(2.0));
  temp_row.set_valign(gtk::Align::Center);
  let temp_big = ui::label("—", "weather-temp");
  let temp_unit = ui::label("", "weather-unit");
  temp_unit.set_valign(gtk::Align::Start);
  temp_unit.set_margin_top(space(10.0));
  temp_row.append(&temp_big);
  temp_row.append(&temp_unit);
  left.append(&temp_row);
  hero.append(&left);
  let right = ui::vbox(space(12.0));
  right.set_margin_end(space(20.0));
  right.set_valign(gtk::Align::Center);
  let place_row = ui::hbox(space(6.0));
  place_row.add_css_class("weather-place");
  place_row.set_cursor_from_name(Some("pointer"));
  place_row.append(&ui::label("\u{f041}", "weather-place-glyph"));
  let place = ui::label("", "weather-place-name");
  place_row.append(&place);
  right.append(&place_row);
  let edit_row = ui::hbox(space(6.0));
  let field = gtk::Entry::new();
  field.add_css_class("field");
  field.set_placeholder_text(Some("Buscar cidade"));
  field.set_width_chars(18);
  edit_row.append(&field);
  let clear = ui::label("✕", "weather-clear");
  clear.set_cursor_from_name(Some("pointer"));
  edit_row.append(&clear);
  edit_row.set_visible(false);
  right.append(&edit_row);
  let stats = ui::hbox(space(36.0));
  let stat = |name: &str| {
    let col = ui::vbox(space(5.0));
    col.append(&ui::label(name, "weather-stat-name"));
    let v = ui::label("", "weather-stat");
    v.set_xalign(0.0);
    col.append(&v);
    stats.append(&col);
    v
  };
  let feels = stat("SENSAÇÃO");
  let wind = stat("VENTO");
  let humidity = stat("UMIDADE");
  right.append(&stats);
  hero.append(&right);
  body.append(&hero);
  let suggestions = ui::vbox(0);
  body.append(&suggestions);
  let loading = ui::label("Buscando previsão…", "weather-loading");
  loading.set_xalign(0.0);
  body.append(&loading);
  let divider = ui::separator();
  body.append(&divider);

  // ---- Hourly chart.
  let chart_box = ui::vbox(space(10.0));
  let chart_head = gtk::CenterBox::new();
  chart_head.set_margin_start(space(16.0));
  chart_head.set_margin_end(space(16.0));
  let chart_title = ui::label("PRÓXIMAS 24 HORAS", "weather-caption");
  let back = ui::label("VOLTAR PARA HOJE", "weather-caption link");
  back.set_cursor_from_name(Some("pointer"));
  chart_head.set_start_widget(Some(&chart_title));
  chart_head.set_end_widget(Some(&back));
  chart_box.append(&chart_head);
  let chart = Rc::new(Chart::new(&space, palette.accent, palette.blue));
  chart.widget.set_margin_start(space(16.0));
  chart.widget.set_margin_end(space(16.0));
  chart_box.append(&chart.widget);
  body.append(&chart_box);

  // ---- The next three days.
  let days = ui::hbox(space(20.0));
  days.set_halign(gtk::Align::Center);
  body.append(&days);
  panel.content.append(&body);

  let selected = Rc::new(RefCell::new(String::new()));
  let editing = Rc::new(Cell::new(false));
  let suggestion_list: Rc<RefCell<Vec<m::Suggestion>>> = Rc::default();
  let suggestion_index = Rc::new(Cell::new(0usize));

  let render: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::default();
  let paint: Rc<dyn Fn()> = {
    let (w, b, render) = (weather.clone(), button.clone(), Rc::downgrade(&render));
    let (hero_icon, temp_big, temp_unit, place, place_row, edit_row, stats) =
      (hero_icon.clone(), temp_big.clone(), temp_unit.clone(), place.clone(), place_row.clone(), edit_row.clone(), stats.clone());
    let (feels, wind, humidity, loading, divider) = (feels.clone(), wind.clone(), humidity.clone(), loading.clone(), divider.clone());
    let (chart_box, chart_title, back, chart, days, selected, editing) =
      (chart_box.clone(), chart_title.clone(), back.clone(), chart.clone(), days.clone(), selected.clone(), editing.clone());
    let panel_w = Rc::downgrade(&panel);
    let space = space.clone();
    Rc::new(move || {
      let icon = w.st.borrow().icon.clone();
      b.set_text(&icon);
      let imperial = m::use_imperial(&unit, &std::env::var("LANG").unwrap_or_default(), &w.country());
      let cur = w.current();
      hero_icon.set_text(if icon.is_empty() { "—" } else { &icon });
      temp_big.set_text(&cur.as_ref().map(|c| if imperial { c.temp_f.clone() } else { c.temp_c.clone() }).unwrap_or_else(|| "—".into()));
      temp_unit.set_text(if cur.is_some() { if imperial { "°F" } else { "°C" } } else { "" });
      let name = w.place();
      place.set_text(&name.to_uppercase());
      place_row.set_visible(!editing.get() && !name.is_empty());
      edit_row.set_visible(editing.get());
      stats.set_visible(cur.is_some());
      loading.set_visible(cur.is_none());
      if let Some(c) = &cur {
        let unit_letter = if imperial { "°F" } else { "°C" };
        let f = if imperial { &c.feels_f } else { &c.feels_c };
        feels.set_text(&if f.is_empty() { String::new() } else { format!("{f}{unit_letter}") });
        wind.set_text(&if imperial { format!("{} mph", c.wind_mph) } else { format!("{} km/h", c.wind_kmh) });
        humidity.set_text(&format!("{}%", c.humidity));
      }
      let (wttr, meteo) = { let st = w.st.borrow(); (st.wttr.clone(), st.meteo.clone()) };
      let forecast = m::forecast_days(wttr.as_ref(), meteo.as_ref(), &today());
      divider.set_visible(!forecast.is_empty());
      let sel = selected.borrow().clone();
      let picked = if sel.is_empty() { Vec::new() } else { m::hourly(meteo.as_ref(), &sel) };
      let showing_picked = !picked.is_empty();
      let series = if showing_picked { picked } else { m::hourly(meteo.as_ref(), "") };
      chart_box.set_visible(!series.is_empty());
      chart_title.set_text(&if showing_picked { day_name(&sel).to_uppercase() } else { "PRÓXIMAS 24 HORAS".into() });
      back.set_visible(showing_picked);
      chart.set(series, !showing_picked, imperial);
      crate::popup::clear(&days);
      for day in forecast {
        let cell = ui::hbox(space(10.0));
        cell.add_css_class("weather-day");
        if showing_picked && sel == day.date {
          cell.add_css_class("picked");
        }
        cell.append(&ui::label(&day.icon, "weather-day-icon"));
        let col = ui::vbox(space(2.0));
        col.set_valign(gtk::Align::Center);
        col.append(&ui::label(&day_name(&day.date).to_uppercase(), "weather-caption"));
        let temps = ui::hbox(space(6.0));
        let (hi, lo) = if imperial { (&day.max_f, &day.min_f) } else { (&day.max_c, &day.min_c) };
        temps.append(&ui::label(&format!("{hi}°"), "weather-day-temp"));
        temps.append(&ui::label(&format!("{lo}°"), "weather-day-temp low"));
        temps.set_halign(gtk::Align::Start);
        col.append(&temps);
        cell.append(&col);
        cell.set_cursor_from_name(Some("pointer"));
        let (sel_cell, date, r) = (selected.clone(), day.date.clone(), render.clone());
        widget::on_press(&cell, move |_| {
          let now = sel_cell.borrow().clone();
          *sel_cell.borrow_mut() = if now == date { String::new() } else { date.clone() };
          if let Some(f) = r.upgrade().and_then(|r| r.borrow().clone()) {
            f();
          }
        });
        days.append(&cell);
      }
      if let Some(p) = panel_w.upgrade() {
        p.relayout();
      }
    })
  };
  *render.borrow_mut() = Some(paint.clone());
  let keep = render.clone();
  button.widget.connect_destroy(move |_| {
    let _ = &keep;
  });
  paint();
  let (p, wk) = (paint.clone(), button.widget.downgrade());
  weather.connect(move || {
    if wk.upgrade().is_some() {
      p();
    }
  });

  // ---- Location editing.
  let render_suggestions: Rc<dyn Fn()> = {
    let (sugg, list, idx) = (suggestions.clone(), suggestion_list.clone(), suggestion_index.clone());
    let space = space.clone();
    let pick: Rc<RefCell<Option<Rc<dyn Fn(m::Suggestion)>>>> = Rc::default();
    let pick_out = pick.clone();
    let f: Rc<dyn Fn()> = Rc::new(move || {
      crate::popup::clear(&sugg);
      for (i, s) in list.borrow().iter().enumerate() {
        let r = ui::hbox(space(8.0));
        r.set_margin_start(space(16.0));
        r.append(&ui::label(&s.name, "weather-suggestion"));
        if !s.description.is_empty() {
          r.append(&ui::label(&s.description, "weather-suggestion-region"));
        }
        let (p, s2) = (pick_out.clone(), s.clone());
        let row = ui::row(&r, move |_| {
          if let Some(f) = p.borrow().clone() {
            f(s2.clone());
          }
        });
        if i == idx.get() {
          row.add_css_class("selected");
        }
        sugg.append(&row);
      }
    });
    // Choosing saves name and coordinates (omarchy-weather-location).
    let (ed, p) = (editing.clone(), paint.clone());
    *pick.borrow_mut() = Some(Rc::new(move |s: m::Suggestion| {
      crate::run::argv(&["omarchy-weather-location", "--set", &s.name, &format!("{},{}", s.latitude, s.longitude)]);
      ed.set(false);
      p();
    }));
    let keep = pick.clone();
    suggestions.connect_destroy(move |_| {
      let _ = &keep;
    });
    f
  };
  {
    let (ed, p, field, list, sugg) = (editing.clone(), paint.clone(), field.clone(), suggestion_list.clone(), suggestions.clone());
    let w = weather.clone();
    widget::on_press(&place_row, move |_| {
      ed.set(true);
      list.borrow_mut().clear();
      crate::popup::clear(&sugg);
      field.set_text(&w.st.borrow().location.name);
      p();
      field.grab_focus();
    });
  }
  {
    // Geocoding, 300 ms after the last keystroke; only the latest answer counts.
    let generation = Rc::new(Cell::new(0u64));
    let (list, idx, rs, ed) = (suggestion_list.clone(), suggestion_index.clone(), render_suggestions.clone(), editing.clone());
    field.connect_changed(move |f| {
      if !ed.get() {
        return;
      }
      let query = f.text().trim().to_string();
      let g = generation.get() + 1;
      generation.set(g);
      let (latest, list, idx, rs) = (generation.clone(), list.clone(), idx.clone(), rs.clone());
      glib::spawn_future_local(async move {
        glib::timeout_future(Duration::from_millis(300)).await;
        if latest.get() != g {
          return;
        }
        let found = if query.chars().count() < 2 {
          Vec::new()
        } else {
          let url = format!("https://geocoding-api.open-meteo.com/v1/search?name={}&count=5&language=en&format=json", glib::Uri::escape_string(&query, None, false));
          m::parse_geocoding(&crate::watch::output(&["curl", "-fsS", "--max-time", "5", &url]).await.unwrap_or_default())
        };
        if latest.get() != g {
          return;
        }
        *list.borrow_mut() = found;
        idx.set(0);
        rs();
      });
    });
    let keys = gtk::EventControllerKey::new();
    let (list, idx, rs, ed, p, sugg) = (suggestion_list.clone(), suggestion_index.clone(), render_suggestions.clone(), editing.clone(), paint.clone(), suggestions.clone());
    let f2 = field.clone();
    keys.connect_key_pressed(move |_, key, _, _| {
      match key {
        gdk::Key::Escape => {
          ed.set(false);
          crate::popup::clear(&sugg);
          p();
        }
        gdk::Key::Down => {
          if idx.get() + 1 < list.borrow().len() {
            idx.set(idx.get() + 1);
            rs();
          }
        }
        gdk::Key::Up => {
          if idx.get() > 0 {
            idx.set(idx.get() - 1);
            rs();
          }
        }
        gdk::Key::Return | gdk::Key::KP_Enter => {
          let name = f2.text().trim().to_string();
          let chosen = list.borrow().get(idx.get()).cloned();
          match (name.is_empty(), chosen) {
            (true, _) => crate::run::argv(&["omarchy-weather-location", "--clear"]),
            (false, Some(s)) => crate::run::argv(&["omarchy-weather-location", "--set", &s.name, &format!("{},{}", s.latitude, s.longitude)]),
            (false, None) => crate::run::argv(&["omarchy-weather-location", "--set", &name]),
          }
          ed.set(false);
          crate::popup::clear(&sugg);
          p();
        }
        _ => return glib::Propagation::Proceed,
      }
      glib::Propagation::Stop
    });
    field.add_controller(keys);
    let (ed, p, sugg) = (editing.clone(), paint.clone(), suggestions.clone());
    widget::on_press(&clear, move |_| {
      crate::run::argv(&["omarchy-weather-location", "--clear"]);
      ed.set(false);
      crate::popup::clear(&sugg);
      p();
    });
  }
  {
    let (sel, p) = (selected.clone(), paint.clone());
    widget::on_press(&back, move |_| {
      sel.borrow_mut().clear();
      p();
    });
  }
  {
    let (sel, ed, sugg) = (selected.clone(), editing.clone(), suggestions.clone());
    panel.connect_close(move || {
      sel.borrow_mut().clear();
      ed.set(false);
      crate::popup::clear(&sugg);
    });
    let (w, p) = (weather.clone(), paint.clone());
    panel.connect_open(move || {
      w.read_location();
      w.refresh();
      p();
    });
    let (ed, p, field) = (editing.clone(), paint.clone(), field.clone());
    let w = weather.clone();
    panel.connect_key(move |key, _| {
      if ed.get() {
        return false;
      }
      if matches!(key, gdk::Key::Return | gdk::Key::KP_Enter) {
        ed.set(true);
        field.set_text(&w.st.borrow().location.name);
        p();
        field.grab_focus();
        return true;
      }
      false
    });
  }
  let (p2, w) = (panel.clone(), weather.clone());
  button.on_press(move |b| match b {
    3 => crate::run::shell("omarchy-notification-send \"$(omarchy-weather-status)\""),
    2 => w.refresh(),
    _ => p2.toggle(),
  });
  if ctx.first_screen {
    if let Some(bar) = ctx.bar.upgrade() {
      let (p, ed, pa, field, w) = (Rc::downgrade(&panel), editing.clone(), paint.clone(), field.clone(), weather.clone());
      bar.add_ipc("omarchy.weather", Box::new(move |method, _| {
        let panel = p.upgrade()?;
        match method {
          "open" | "show" => panel.open(),
          "close" | "hide" => panel.close(),
          "toggle" => panel.toggle(),
          "edit" => {
            panel.open();
            ed.set(true);
            field.set_text(&w.st.borrow().location.name);
            pa();
            field.grab_focus();
          }
          _ => return None,
        }
        Some("ok".into())
      }));
    }
  }
  slot.widget.clone().upcast()
}

// HourlyChart.qml: the temperature line over the rain bars on one hour
// axis, the extremes labeled, a readout row over it (the hovered hour, or
// the range and the rain in total).
struct Chart {
  widget: gtk::Box,
  readout_hour: gtk::Label,
  readout_temp: gtk::Label,
  readout_rain: gtk::Label,
  area: gtk::DrawingArea,
  data: Rc<RefCell<(Vec<m::Hour>, bool, bool)>>,
  hover: Rc<Cell<Option<usize>>>,
}

fn temps(series: &[m::Hour], imperial: bool) -> Vec<Option<f64>> {
  series.iter().map(|h| h.temp_c.map(|t| if imperial { m::c_to_f(t) } else { t })).collect()
}

fn extremes(t: &[Option<f64>]) -> (Option<usize>, Option<usize>) {
  let (mut lo, mut hi) = (None::<usize>, None::<usize>);
  for (i, v) in t.iter().enumerate() {
    let Some(v) = v else { continue };
    if lo.is_none_or(|l| *v < t[l].unwrap()) {
      lo = Some(i);
    }
    if hi.is_none_or(|h| *v > t[h].unwrap()) {
      hi = Some(i);
    }
  }
  (lo, hi)
}

fn temp_text(v: Option<f64>) -> String {
  v.map(|v| format!("{}°", v.round() as i64)).unwrap_or_else(|| "—".into())
}

impl Chart {
  fn new(space: &impl Fn(f64) -> i32, temp_color: [f64; 4], rain_color: [f64; 4]) -> Chart {
    let widget = ui::vbox(space(8.0));
    let readout = ui::hbox(space(14.0));
    let readout_hour = ui::label("", "weather-readout dim");
    readout.append(&readout_hour);
    let t = ui::hbox(space(6.0));
    let swatch = gtk::DrawingArea::new();
    swatch.set_content_width(space(10.0));
    swatch.set_content_height(2);
    swatch.set_valign(gtk::Align::Center);
    swatch.set_draw_func(move |_, cr, w, h| {
      cr.set_source_rgba(temp_color[0], temp_color[1], temp_color[2], 1.0);
      widget::rounded(cr, 0.0, 0.0, w as f64, h as f64, 1.0);
      let _ = cr.fill();
    });
    t.append(&swatch);
    let readout_temp = ui::label("", "weather-readout");
    t.append(&readout_temp);
    readout.append(&t);
    let r = ui::hbox(space(6.0));
    let rain_swatch = gtk::DrawingArea::new();
    rain_swatch.set_content_width(space(8.0));
    rain_swatch.set_content_height(space(8.0));
    rain_swatch.set_valign(gtk::Align::Center);
    rain_swatch.set_draw_func(move |_, cr, w, h| {
      cr.set_source_rgba(rain_color[0], rain_color[1], rain_color[2], 1.0);
      widget::rounded(cr, 0.0, 0.0, w as f64, h as f64, 2.0);
      let _ = cr.fill();
    });
    r.append(&rain_swatch);
    let readout_rain = ui::label("", "weather-readout");
    r.append(&readout_rain);
    readout.append(&r);
    widget.append(&readout);

    let (temp_h, rain_h, label_room, axis_room) = (space(56.0) as f64, space(34.0) as f64, space(18.0) as f64, space(18.0) as f64);
    let area = gtk::DrawingArea::new();
    area.set_content_height((label_room * 3.0 + temp_h + rain_h + axis_room) as i32);
    area.set_hexpand(true);
    widget.append(&area);
    let data: Rc<RefCell<(Vec<m::Hour>, bool, bool)>> = Rc::default();
    let hover = Rc::new(Cell::new(None::<usize>));
    let surface = doxia_theme::surface_card("popups").fill;
    let (d, hv) = (data.clone(), hover.clone());
    area.set_draw_func(move |a, cr, width, _| {
      let (series, starts_now, imperial) = &*d.borrow();
      let n = series.len();
      if n == 0 {
        return;
      }
      let fg = a.color();
      let fg = [fg.red() as f64, fg.green() as f64, fg.blue() as f64];
      let width = width as f64;
      let slot = width / n as f64;
      let x_at = |i: usize| slot * (i as f64 + 0.5);
      let t = temps(series, *imperial);
      let (lo_i, hi_i) = extremes(&t);
      let (mut lo, mut hi) = (lo_i.and_then(|i| t[i]).unwrap_or(0.0), hi_i.and_then(|i| t[i]).unwrap_or(1.0));
      if hi - lo < 4.0 {
        let mid = (hi + lo) / 2.0;
        lo = mid - 2.0;
        hi = mid + 2.0;
      }
      let t_top = label_room;
      let t_bottom = t_top + temp_h;
      let y_at = |v: f64| t_bottom - (v - lo) / (hi - lo) * temp_h;
      let rain_top = t_bottom + label_room * 2.0;
      let rain_bottom = rain_top + rain_h;
      let (mut max_mm, mut max_i) = (0.0, None);
      for (i, h) in series.iter().enumerate() {
        if h.precip_mm > max_mm {
          max_mm = h.precip_mm;
          max_i = Some(i);
        }
      }
      let rain_scale = max_mm.max(2.0);
      let hover = hv.get();
      if let Some(h) = hover {
        cr.set_source_rgba(fg[0], fg[1], fg[2], 0.25);
        cr.set_line_width(1.0);
        cr.move_to(x_at(h).round() + 0.5, t_top - 4.0);
        cr.line_to(x_at(h).round() + 0.5, rain_bottom);
        let _ = cr.stroke();
      }
      let bar_w = (slot - 2.0).min(slot * 0.6).max(3.0);
      for (i, h) in series.iter().enumerate() {
        if h.precip_mm <= 0.0 {
          continue;
        }
        let bh = (h.precip_mm / rain_scale * rain_h).max(2.0);
        let x0 = x_at(i) - bar_w / 2.0;
        let r = 4.0f64.min(bar_w / 2.0).min(bh);
        cr.new_sub_path();
        cr.move_to(x0, rain_bottom);
        cr.line_to(x0, rain_bottom - bh + r);
        cr.arc(x0 + r, rain_bottom - bh + r, r, std::f64::consts::PI, 1.5 * std::f64::consts::PI);
        cr.line_to(x0 + bar_w - r, rain_bottom - bh);
        cr.arc(x0 + bar_w - r, rain_bottom - bh + r, r, 1.5 * std::f64::consts::PI, 0.0);
        cr.line_to(x0 + bar_w, rain_bottom);
        cr.close_path();
        let alpha = if hover.is_some_and(|hh| hh != i) { 0.55 } else { 1.0 };
        cr.set_source_rgba(rain_color[0], rain_color[1], rain_color[2], alpha);
        let _ = cr.fill();
      }
      cr.set_source_rgba(fg[0], fg[1], fg[2], 0.15);
      cr.set_line_width(1.0);
      cr.move_to(0.0, rain_bottom.round() + 0.5);
      cr.line_to(width, rain_bottom.round() + 0.5);
      let _ = cr.stroke();
      let label = |text: &str, x: f64, y: f64, c: [f64; 4]| {
        let layout = a.create_pango_layout(Some(text));
        let mut font = pango::FontDescription::from_string("DoxIA Shell");
        font.set_absolute_size(10.0 * pango::SCALE as f64);
        layout.set_font_description(Some(&font));
        let (lw, _) = layout.pixel_size();
        let left = (x - lw as f64 / 2.0).clamp(0.0, (width - lw as f64).max(0.0));
        let baseline = layout.baseline() as f64 / pango::SCALE as f64;
        cr.set_source_rgba(c[0], c[1], c[2], c[3]);
        cr.move_to(left, y - baseline);
        pangocairo::functions::show_layout(cr, &layout);
      };
      if let Some(i) = max_i {
        let top = rain_bottom - (max_mm / rain_scale * rain_h).max(2.0);
        label(&m::format_precip(max_mm, *imperial), x_at(i), top - 5.0, [fg[0], fg[1], fg[2], 0.83]);
      }
      cr.set_source_rgba(temp_color[0], temp_color[1], temp_color[2], 1.0);
      cr.set_line_width(2.0);
      cr.set_line_join(gtk::cairo::LineJoin::Round);
      cr.set_line_cap(gtk::cairo::LineCap::Round);
      let mut drawing = false;
      for (i, v) in t.iter().enumerate() {
        match v {
          Some(v) if drawing => cr.line_to(x_at(i), y_at(*v)),
          Some(v) => {
            cr.move_to(x_at(i), y_at(*v));
            drawing = true;
          }
          None => drawing = false,
        }
      }
      let _ = cr.stroke();
      let marker = |i: usize| {
        let Some(v) = t[i] else { return };
        cr.arc(x_at(i), y_at(v), 4.0, 0.0, std::f64::consts::TAU);
        cr.set_source_rgba(temp_color[0], temp_color[1], temp_color[2], 1.0);
        let _ = cr.fill_preserve();
        cr.set_line_width(2.0);
        cr.set_source_rgba(surface[0], surface[1], surface[2], 1.0);
        let _ = cr.stroke();
      };
      let fgc = [fg[0], fg[1], fg[2], 1.0];
      if let Some(h) = hi_i {
        marker(h);
        label(&temp_text(t[h]), x_at(h), y_at(t[h].unwrap()) - 9.0, fgc);
      }
      if let Some(l) = lo_i.filter(|l| Some(*l) != hi_i) {
        marker(l);
        label(&temp_text(t[l]), x_at(l), y_at(t[l].unwrap()) + 9.0 + 10.0, fgc);
      }
      if let Some(h) = hover {
        marker(h);
      }
      let axis_y = rain_bottom + axis_room - 4.0;
      let muted = [fg[0], fg[1], fg[2], 0.66];
      if *starts_now {
        label("Agora", x_at(0), axis_y, muted);
      }
      for (i, h) in series.iter().enumerate() {
        if (*starts_now && i < 2) || h.hour % 3 != 0 {
          continue;
        }
        label(&format!("{}h", h.hour), x_at(i), axis_y, muted);
      }
    });
    let chart = Chart { widget, readout_hour, readout_temp, readout_rain, area, data, hover };
    let motion = gtk::EventControllerMotion::new();
    let (d, hv, a) = (chart.data.clone(), chart.hover.clone(), chart.area.downgrade());
    let (rh, rt, rr) = (chart.readout_hour.clone(), chart.readout_temp.clone(), chart.readout_rain.clone());
    let update = Rc::new(move |x: Option<f64>| {
      let Some(area) = a.upgrade() else { return };
      let n = d.borrow().0.len();
      let idx = x.filter(|_| n > 0).map(|x| ((x / (area.width() as f64 / n as f64)).floor() as usize).min(n - 1));
      hv.set(idx);
      area.queue_draw();
      Chart::readout(&d.borrow(), idx, &rh, &rt, &rr);
    });
    let u = update.clone();
    motion.connect_motion(move |_, x, _| u(Some(x)));
    let u = update.clone();
    motion.connect_leave(move |_| u(None));
    chart.area.add_controller(motion);
    chart
  }

  fn readout(data: &(Vec<m::Hour>, bool, bool), idx: Option<usize>, hour: &gtk::Label, temp: &gtk::Label, rain: &gtk::Label) {
    let (series, _, imperial) = data;
    let t = temps(series, *imperial);
    match idx.and_then(|i| series.get(i).map(|p| (i, p))) {
      Some((i, p)) => {
        hour.set_visible(true);
        hour.set_text(&format!("{}h", p.hour));
        temp.set_text(&temp_text(t[i]));
        let mut text = format!("chuva {}", m::format_precip(p.precip_mm, *imperial));
        if let Some(pr) = p.probability {
          text.push_str(&format!(" ({}%)", pr.round()));
        }
        rain.set_text(&text);
      }
      None => {
        hour.set_visible(false);
        let (lo, hi) = extremes(&t);
        temp.set_text(&match (lo, hi) {
          (Some(l), Some(h)) => format!("{} a {}", temp_text(t[l]), temp_text(t[h])),
          _ => "—".into(),
        });
        let total: f64 = series.iter().map(|h| h.precip_mm).sum();
        rain.set_text(&if total > 0.0 { format!("chuva {} no total", m::format_precip(total, *imperial)) } else { "sem chuva prevista".into() });
      }
    }
  }

  fn set(&self, series: Vec<m::Hour>, starts_now: bool, imperial: bool) {
    *self.data.borrow_mut() = (series, starts_now, imperial);
    self.hover.set(None);
    Chart::readout(&self.data.borrow(), None, &self.readout_hour, &self.readout_temp, &self.readout_rain);
    self.area.queue_draw();
  }
}
