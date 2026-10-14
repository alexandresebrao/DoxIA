// omarchy.monitor (shell/plugins/panels/monitor): the display icon; the
// wheel steps the brightness by 5% with the OSD. Its panel (state from
// omarchy-monitor-state): the brightness slider and its name ("Coruja
// noturna" .. "Sol a pino"), the text size in stops
// (omarchy-display-text-size), the scale of the focused screen in clean
// steps (omarchy-mango-monitor-scaling) and, with several screens, each one
// to switch on or off. IPC target "omarchy.monitor": brightness <percent>,
// state, open, close, toggle, show, hide.

use super::super::member::Member;
use super::super::panel::Panel;
use super::super::ui::{self, CursorRow};
use super::super::widget::{Button, Kind, Slot};
use super::super::Ctx;
use gtk::glib;
use gtk::prelude::*;
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

const SCALES: [&str; 6] = ["1", "1.25", "1.6", "2", "3", "4"];
const TEXT_STOPS: [f64; 7] = [9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 20.0];

pub fn brightness_name(p: f64) -> &'static str {
  match p.round() as i64 {
    p if p >= 95 => "Sol a pino",
    p if p >= 80 => "Explosão solar",
    p if p >= 65 => "Hora dourada",
    p if p >= 45 => "Dia claro",
    p if p >= 30 => "Brilho suave",
    p if p >= 20 => "Luz de abajur",
    p if p >= 10 => "Luz de velas",
    _ => "Coruja noturna",
  }
}

pub fn normalize_scale(s: &str) -> String {
  match s.trim().parse::<f64>() {
    Ok(n) => {
      let r = (n * 100.0).round() / 100.0;
      if r.fract() == 0.0 { format!("{}", r as i64) } else { format!("{r}") }
    }
    Err(_) => String::new(),
  }
}

fn gcd(mut a: i64, mut b: i64) -> i64 {
  while b != 0 {
    (a, b) = (b, a % b);
  }
  a
}

// The nearest scale at or above `scale` that divides the mode evenly (in
// 1/120 steps, as mango and Wayland's fractional scale do).
pub fn clean_scale(scale: f64, w: f64, h: f64) -> String {
  if scale <= 0.0 || w <= 0.0 || h <= 0.0 {
    return String::new();
  }
  let divisor = gcd((w * 120.0).round() as i64, (h * 120.0).round() as i64);
  let mut units = ((scale * 120.0).round() as i64).min(divisor).max(1);
  while divisor % units != 0 {
    units += 1;
  }
  normalize_scale(&(units as f64 / 120.0).to_string())
}

// The presets, one per clean scale they land on.
pub fn available_scales(w: f64, h: f64) -> Vec<String> {
  if w <= 0.0 || h <= 0.0 {
    return SCALES.iter().map(|s| s.to_string()).collect();
  }
  let mut best: Vec<(String, String, f64)> = Vec::new();
  for s in SCALES {
    let requested: f64 = s.parse().unwrap();
    let eff = clean_scale(requested, w, h);
    let dist = (requested - eff.parse::<f64>().unwrap_or(0.0)).abs();
    match best.iter_mut().find(|(k, _, _)| *k == eff) {
      Some(e) if dist < e.2 => *e = (eff, s.to_string(), dist),
      Some(_) => {}
      None => best.push((eff, s.to_string(), dist)),
    }
  }
  best.into_iter().map(|(_, v, _)| v).collect()
}

#[derive(Clone, Debug, Default)]
struct State {
  brightness: Option<f64>,
  focused: String,
  scale: String,
  displays: Vec<Value>,
}

fn parse_state(raw: &str) -> State {
  let lines: Vec<&str> = raw.lines().collect();
  let line = |i: usize| lines.get(i).map(|l| l.trim()).unwrap_or("");
  let b = line(0);
  State {
    brightness: (b != "unavailable" && !b.is_empty()).then(|| b.parse::<f64>().unwrap_or(0.0).clamp(0.0, 100.0)),
    focused: line(5).to_string(),
    scale: normalize_scale(line(6)),
    displays: serde_json::from_str::<Value>(line(7)).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default(),
  }
}

pub fn build(ctx: &Rc<Ctx>) -> Member {
  let space = { let s = ctx.style.clone(); move |px: f64| s.space(px) };
  let screens = gtk::gdk::Display::default().map(|d| d.monitors().n_items()).unwrap_or(1);
  let glyph = if screens > 1 { "\u{f037a}" } else { "\u{f0379}" };
  let button = Button::new(ctx, Kind::Icon, glyph);
  let slot = Slot::new(ctx, &button.widget);
  let panel = Panel::new(ctx, &button.widget, &slot, space(380.0));
  let state = Rc::new(RefCell::new(State::default()));
  let pending = Rc::new(Cell::new(None::<f64>));
  let setting = Rc::new(Cell::new(false));
  let font_base = ctx.style.font_base;

  // Brightness: one omarchy-brightness-display at a time; the last value
  // asked for while one runs goes next.
  let set_brightness: Rc<dyn Fn(f64)> = {
    let (state, pending, setting) = (state.clone(), pending.clone(), setting.clone());
    Rc::new(move |v: f64| {
      let p = v.round().clamp(1.0, 100.0);
      state.borrow_mut().brightness = Some(p);
      pending.set(Some(p));
      if setting.replace(true) {
        return;
      }
      let (state, pending, setting) = (state.clone(), pending.clone(), setting.clone());
      glib::spawn_future_local(async move {
        while let Some(p) = pending.take() {
          let monitor = state.borrow().focused.clone();
          let _ = crate::watch::output(&["omarchy-brightness-display", "--no-osd", "--monitor", &monitor, &format!("{p}%")]).await;
        }
        setting.set(false);
      });
    })
  };

  let body = ui::vbox(space(14.0));
  let scroll = gtk::ScrolledWindow::new();
  scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
  scroll.set_propagate_natural_height(true);
  scroll.set_max_content_height(space(560.0));
  scroll.set_child(Some(&body));
  panel.content.append(&scroll);
  let cursor = ui::Cursor::new();

  // Reading the state again after a change, from inside the panel.
  type Again = Rc<RefCell<std::rc::Weak<dyn Fn()>>>;
  let again: Again = Rc::new(RefCell::new(std::rc::Weak::<Box<dyn Fn()>>::new() as std::rc::Weak<dyn Fn()>));
  let render: Rc<dyn Fn()> = {
    let (state, body, cursor, set_brightness, panel_w) = (state.clone(), body.clone(), cursor.clone(), set_brightness.clone(), Rc::downgrade(&panel));
    let space = space.clone();
    let refresh_w = again.clone();
    Rc::new(move || {
      let st = state.borrow().clone();
      crate::popup::clear(&body);
      cursor.clear();
      let hero = ui::hbox(space(14.0));
      hero.append(&ui::label(if st.displays.len() > 1 { "\u{f037a}" } else { "\u{f0379}" }, "panel-hero-icon"));
      let labels = ui::vbox(space(2.0));
      labels.set_valign(gtk::Align::Center);
      let t = ui::label("Tela", "panel-hero-title");
      t.set_xalign(0.0);
      labels.append(&t);
      let name = ui::label(&st.brightness.map(|b| brightness_name(b).to_uppercase()).unwrap_or_default(), "panel-hero-subtitle");
      name.set_xalign(0.0);
      labels.append(&name);
      hero.append(&labels);
      body.append(&hero);

      let head = |title: &str, right: &str| {
        let h = gtk::CenterBox::new();
        h.set_start_widget(Some(&ui::section(title)));
        let r = ui::label(right, "audio-percent");
        r.set_margin_end(space(6.0));
        h.set_end_widget(Some(&r));
        (h, r)
      };
      if let Some(b) = st.brightness {
        body.append(&ui::separator());
        let col = ui::vbox(space(6.0));
        let (h, pct) = head("Brilho", &format!("{}%", b.round()));
        col.append(&h);
        let slider = ui::Slider::new(1.0, 100.0, 1.0, &space);
        slider.set(b);
        slider.widget.set_margin_start(space(6.0));
        slider.widget.set_margin_end(space(6.0));
        let (p, n) = (pct.clone(), name.clone());
        // Live while dragging, applied 180 ms after it settles.
        let debounce = Rc::new(Cell::new(0u64));
        let sb = set_brightness.clone();
        slider.connect_moved(move |v| {
          p.set_text(&format!("{}%", v.round()));
          n.set_text(&brightness_name(v).to_uppercase());
          let g = debounce.get() + 1;
          debounce.set(g);
          let (d, sb) = (debounce.clone(), sb.clone());
          glib::timeout_add_local_once(Duration::from_millis(180), move || {
            if d.get() == g {
              sb(v);
            }
          });
        });
        let sb = set_brightness.clone();
        slider.connect_released(move |v| sb(v));
        col.append(&slider.widget);
        let (sb, st2) = (set_brightness.clone(), state.clone());
        cursor.add(CursorRow {
          widget: slider.widget.clone().upcast(),
          activate: None,
          adjust: Some(Rc::new(move |d| {
            let now = st2.borrow().brightness.unwrap_or(50.0);
            sb(now + d * 100.0);
          })),
          mute: None,
        });
        body.append(&col);
      }

      // Text size.
      body.append(&ui::separator());
      let col = ui::vbox(space(6.0));
      let (h, px) = head("Tamanho do texto", &format!("{}px", font_base.round()));
      col.append(&h);
      let nearest = TEXT_STOPS.iter().enumerate().min_by(|a, b| (a.1 - font_base).abs().total_cmp(&(b.1 - font_base).abs())).map(|(i, _)| i).unwrap_or(3);
      let slider = ui::Slider::new(0.0, (TEXT_STOPS.len() - 1) as f64, 1.0, &space);
      slider.set(nearest as f64);
      slider.widget.set_margin_start(space(6.0));
      slider.widget.set_margin_end(space(6.0));
      let p = px.clone();
      slider.connect_moved(move |v| p.set_text(&format!("{}px", TEXT_STOPS[v.round() as usize])));
      slider.connect_released(|v| crate::run::argv(&["omarchy-display-text-size", &TEXT_STOPS[v.round() as usize].to_string()]));
      col.append(&slider.widget);
      let current = Rc::new(Cell::new(nearest));
      cursor.add(CursorRow {
        widget: slider.widget.clone().upcast(),
        activate: None,
        adjust: Some(Rc::new(move |d| {
          let i = (current.get() as i64 + if d < 0.0 { -1 } else { 1 }).clamp(0, TEXT_STOPS.len() as i64 - 1) as usize;
          current.set(i);
          crate::run::argv(&["omarchy-display-text-size", &TEXT_STOPS[i].to_string()]);
        })),
        mute: None,
      });
      body.append(&col);

      // Scale of the focused screen.
      body.append(&ui::separator());
      let col = ui::vbox(space(10.0));
      let enabled = st.displays.iter().filter(|d| d["enabled"].as_bool() == Some(true)).count();
      let (h, _) = head("Escala", if enabled > 1 { &st.focused } else { "" });
      col.append(&h);
      let focused = st.displays.iter().find(|d| d["focused"].as_bool() == Some(true));
      let (w, hgt) = focused.map(|d| (d["width"].as_f64().unwrap_or(0.0), d["height"].as_f64().unwrap_or(0.0))).unwrap_or((0.0, 0.0));
      let pills = ui::hbox(space(6.0));
      pills.set_homogeneous(true);
      for s in available_scales(w, hgt) {
        let eff = if w > 0.0 { clean_scale(s.parse().unwrap_or(1.0), w, hgt) } else { normalize_scale(&s) };
        let active = eff == st.scale;
        let s2 = s.clone();
        let rw = refresh_w.clone();
        let b = ui::button(&format!("{eff}x"), false, move || {
          let rw = rw.clone();
          let s2 = s2.clone();
          glib::spawn_future_local(async move {
            let _ = crate::watch::output(&["bash", "-c", &format!("omarchy-mango-monitor-scaling {s2}")]).await;
            if let Some(f) = rw.borrow().upgrade() {
              f();
            }
          });
        });
        b.add_css_class("scale");
        if active {
          b.add_css_class("selected");
        }
        pills.append(&b);
        let b2 = b.clone();
        cursor.add(CursorRow { widget: b.upcast(), activate: Some(Rc::new(move || b2.emit_clicked())), adjust: None, mute: None });
      }
      col.append(&pills);
      body.append(&col);

      // Screens.
      if st.displays.len() > 1 {
        body.append(&ui::separator());
        let col = ui::vbox(space(6.0));
        col.append(&ui::section("Telas"));
        for d in &st.displays {
          let name = d["name"].as_str().unwrap_or("").to_string();
          let on = d["enabled"].as_bool() == Some(true);
          let r = ui::hbox(space(8.0));
          r.append(&ui::label("\u{f0379}", "audio-device-glyph"));
          let l = ui::label(&format!("{name}{}", if d["focused"].as_bool() == Some(true) { " · em foco" } else { "" }), "audio-device");
          l.set_hexpand(true);
          l.set_xalign(0.0);
          r.append(&l);
          r.append(&ui::label(if on { "\u{f012c}" } else { "" }, "audio-device-glyph"));
          let can_toggle = !(on && enabled <= 1);
          let rw = refresh_w.clone();
          let toggle: Rc<dyn Fn()> = Rc::new(move || {
            if !can_toggle {
              return;
            }
            let call = format!("{}{name}", if on { "disable_monitor," } else { "enable_monitor," });
            let rw = rw.clone();
            glib::spawn_future_local(async move {
              let _ = crate::watch::output(&["mmsg", "dispatch", &call]).await;
              if let Some(f) = rw.borrow().upgrade() {
                f();
              }
            });
          });
          let t = toggle.clone();
          let row = ui::row(&r, move |_| t());
          if !can_toggle {
            row.set_opacity(0.6);
          }
          col.append(&row);
          cursor.add(CursorRow { widget: row.upcast(), activate: Some(toggle), adjust: None, mute: None });
        }
        body.append(&col);
      }
      if let Some(p) = panel_w.upgrade() {
        p.relayout();
      }
    })
  };
  let refresh: Rc<dyn Fn()> = {
    let (state, render) = (state.clone(), render.clone());
    Rc::new(move || {
      let (state, render) = (state.clone(), render.clone());
      glib::spawn_future_local(async move {
        let raw = crate::watch::output(&["omarchy-monitor-state"]).await.unwrap_or_default();
        *state.borrow_mut() = parse_state(&raw);
        render();
      });
    })
  };
  *again.borrow_mut() = Rc::downgrade(&refresh);
  {
    let (r, c, panel_w) = (refresh.clone(), cursor.clone(), Rc::downgrade(&panel));
    panel.connect_open(move || {
      c.reset();
      let (r, panel_w) = (r.clone(), panel_w.clone());
      glib::spawn_future_local(async move {
        while panel_w.upgrade().is_some_and(|p| p.is_open()) {
          r();
          glib::timeout_future(Duration::from_secs(5)).await;
        }
      });
    });
    let c = cursor.clone();
    panel.connect_key(move |k, _| c.key(k));
  }
  // Read once at start, for the wheel.
  refresh();

  let wheel: Rc<dyn Fn(f64, f64)> = {
    let (state, sb, bar) = (state.clone(), set_brightness.clone(), ctx.bar.clone());
    Rc::new(move |_, dy| {
      let Some(b) = state.borrow().brightness else { return };
      if dy == 0.0 {
        return;
      }
      let next = (b + if dy < 0.0 { 5.0 } else { -5.0 }).clamp(1.0, 100.0);
      sb(next);
      if let Some(bar) = bar.upgrade() {
        bar.osd("brightness", next);
      }
    })
  };
  if ctx.first_screen {
    if let Some(bar) = ctx.bar.upgrade() {
      let (p, state, sb) = (Rc::downgrade(&panel), state.clone(), set_brightness.clone());
      bar.add_ipc("omarchy.monitor", Box::new(move |method, args| {
        let panel = p.upgrade()?;
        Some(match method {
          "brightness" => {
            let v = args.first().and_then(|a| a.trim_end_matches('%').parse::<f64>().ok()).unwrap_or(50.0);
            sb(v);
            format!("got {}", v.round().clamp(1.0, 100.0))
          }
          "state" => {
            let st = state.borrow();
            serde_json::json!({
              "brightness": st.brightness.unwrap_or(0.0),
              "brightnessAvailable": st.brightness.is_some(),
              "focusedMonitor": st.focused,
              "scale": st.scale,
              "displays": st.displays,
            })
            .to_string()
          }
          "open" | "show" => {
            panel.open();
            "ok".into()
          }
          "close" | "hide" => {
            panel.close();
            "ok".into()
          }
          "toggle" => {
            panel.toggle();
            "ok".into()
          }
          _ => return None,
        })
      }));
    }
  }
  Member { id: "omarchy.monitor".into(), title: "Tela".into(), button, slot, panel, wheel: Some(wheel), right: None }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn scales_and_names() {
    assert_eq!(normalize_scale("1.250000"), "1.25");
    assert_eq!(normalize_scale("2"), "2");
    assert_eq!(clean_scale(1.25, 2560.0, 1600.0), "1.25");
    assert_eq!(clean_scale(1.6, 1920.0, 1080.0), "1.6");
    assert_eq!(available_scales(0.0, 0.0).len(), 6);
    assert_eq!(brightness_name(50.0), "Dia claro");
    let s = parse_state("40\n\n\n\n\nVirtual-1\n1\n[{\"name\":\"Virtual-1\",\"enabled\":true,\"focused\":true,\"width\":1280,\"height\":800}]\n");
    assert_eq!((s.brightness, s.focused.as_str(), s.displays.len()), (Some(40.0), "Virtual-1", 1));
    assert_eq!(parse_state("unavailable\n").brightness, None);
  }
}
