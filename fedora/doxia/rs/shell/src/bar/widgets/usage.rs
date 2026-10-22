// doxia.usage (fedora/doxia/plugins/doxia.usage/Usage.qml): CPU and memory,
// each an icon with a small graph of the last minute beside it, and, while
// on battery or not full, the battery with a vertical bar filling with the
// percentage; in the accent (gold past 75%; the battery red under 20% and
// gold under 35%). The exact value is each meter's tooltip; a click opens
// btop. Horizontal bars only.

use super::super::widget::{self, Slot};
use super::super::Ctx;
use gtk::prelude::*;
use gtk::{glib, pango};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;

#[derive(Clone, Copy, Default)]
struct Sample {
  sum: u64,
  idle: u64,
}

// /proc/stat's first line: (total, idle + iowait).
pub fn cpu_sample(stat: &str) -> Option<(u64, u64)> {
  let line = stat.lines().find(|l| l.starts_with("cpu "))?;
  let n: Vec<u64> = line.split_whitespace().skip(1).filter_map(|v| v.parse().ok()).collect();
  if n.len() < 4 {
    return None;
  }
  Some((n.iter().sum(), n[3] + n.get(4).copied().unwrap_or(0)))
}

// MemTotal and MemAvailable, in kB.
pub fn memory(meminfo: &str) -> Option<(u64, u64)> {
  let field = |name: &str| {
    meminfo.lines().find(|l| l.starts_with(name)).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse::<u64>().ok())
  };
  Some((field("MemTotal:")?, field("MemAvailable:")?))
}

// Samples a graph keeps: a minute at the default 2 s interval.
const HISTORY: usize = 30;

struct Meter {
  icon: char,
  // A graph of the last samples instead of a bar.
  graph: bool,
  history: VecDeque<f64>,
  rotation: f64,
  value: f64,
  // Shown: what the bar draws now, easing towards `value`.
  shown: f64,
  fill: [f64; 4],
  icon_color: Option<[f64; 4]>,
  tip: String,
  visible: bool,
  // Where it was drawn, for the tooltip.
  x0: f64,
  x1: f64,
}

pub fn build(ctx: &Rc<Ctx>, entry: &serde_json::Value) -> Option<gtk::Widget> {
  if ctx.vertical() {
    return None;
  }
  let interval = super::super::config::setting(entry, "intervalSec").and_then(|v| v.as_u64().or_else(|| v.as_str()?.parse().ok())).unwrap_or(2).max(1);
  let palette = doxia_theme::palette();
  let (accent, yellow, red) = (palette.accent, palette.yellow, palette.red);
  let stroke = ctx.style.text;
  let unit = ctx.style.icon_canvas as f64;
  let icon_font = ctx.style.icon_font as f64;
  let space = |px: f64| ctx.style.space(px) as f64;
  let (gap_icon, gap_meter) = (space(3.0), space(6.0));

  let meters = Rc::new(RefCell::new(vec![
    Meter { icon: '\u{EC19}', graph: true, history: VecDeque::new(), rotation: 0.0, value: 0.0, shown: 0.0, fill: accent, icon_color: None, tip: String::new(), visible: true, x0: 0.0, x1: 0.0 },
    Meter { icon: '\u{EFC5}', graph: true, history: VecDeque::new(), rotation: -90.0, value: 0.0, shown: 0.0, fill: accent, icon_color: None, tip: String::new(), visible: true, x0: 0.0, x1: 0.0 },
    Meter { icon: '\u{F140B}', graph: false, history: VecDeque::new(), rotation: 0.0, value: 0.0, shown: 0.0, fill: accent, icon_color: None, tip: String::new(), visible: false, x0: 0.0, x1: 0.0 },
  ]));
  let area = gtk::DrawingArea::new();
  area.add_css_class("bar-usage");
  area.set_cursor_from_name(Some("pointer"));
  area.set_content_height(ctx.bar_size());
  let bar_w = (unit * 0.24).round().max(3.0);
  let bar_h = (unit * 0.95).round();
  let graph_w = (unit * 2.0).round();
  let m = meters.clone();
  let a = area.clone();
  area.set_draw_func(move |_, cr, _w, h| {
    let mut x = 6.0;
    let h = h as f64;
    for meter in m.borrow_mut().iter_mut() {
      if !meter.visible {
        continue;
      }
      meter.x0 = x;
      // The glyph's ink, measured and centered (turned on its side, width
      // and height swap).
      let layout = a.create_pango_layout(Some(&meter.icon.to_string()));
      let mut font = pango::FontDescription::from_string("DoxIA Shell");
      font.set_absolute_size(icon_font * pango::SCALE as f64);
      layout.set_font_description(Some(&font));
      let (ink, _) = layout.pixel_extents();
      let sideways = meter.rotation % 180.0 != 0.0;
      let (iw, ih) = if sideways { (ink.height() as f64, ink.width() as f64) } else { (ink.width() as f64, ink.height() as f64) };
      let cx = x + iw / 2.0;
      let cy = h / 2.0;
      cr.save().ok();
      cr.translate(cx, cy);
      cr.rotate(meter.rotation.to_radians());
      cr.translate(-(ink.x() as f64 + ink.width() as f64 / 2.0), -(ink.y() as f64 + ink.height() as f64 / 2.0));
      let c = meter.icon_color.unwrap_or(stroke);
      cr.set_source_rgba(c[0], c[1], c[2], c[3]);
      pangocairo::functions::show_layout(cr, &layout);
      cr.restore().ok();
      let _ = ih;
      x += iw.ceil() + gap_icon;
      let by = ((h - bar_h) / 2.0).round();
      if meter.graph {
        draw_graph(cr, &meter.history, x, by, graph_w, bar_h, stroke, meter.fill);
        x += graph_w;
        meter.x1 = x;
        x += gap_meter;
        continue;
      }
      // The bar: a faint track, filled from the bottom.
      widget::rounded(cr, x, by, bar_w, bar_h, bar_w / 2.0);
      cr.set_source_rgba(stroke[0], stroke[1], stroke[2], 0.16);
      let _ = cr.fill();
      let fh = (bar_h * meter.shown.clamp(0.0, 1.0)).max(bar_w);
      widget::rounded(cr, x, by + bar_h - fh, bar_w, fh, bar_w / 2.0);
      cr.set_source_rgba(meter.fill[0], meter.fill[1], meter.fill[2], meter.fill[3]);
      let _ = cr.fill();
      x += bar_w;
      meter.x1 = x;
      x += gap_meter;
    }
  });
  let width_of = {
    let m = meters.clone();
    let a = area.clone();
    move || {
      let mut w = 12.0;
      let mut n = 0;
      for meter in m.borrow().iter().filter(|m| m.visible) {
        let layout = a.create_pango_layout(Some(&meter.icon.to_string()));
        let mut font = pango::FontDescription::from_string("DoxIA Shell");
        font.set_absolute_size(icon_font * pango::SCALE as f64);
        layout.set_font_description(Some(&font));
        let (ink, _) = layout.pixel_extents();
        let iw = if meter.rotation % 180.0 != 0.0 { ink.height() } else { ink.width() } as f64;
        w += iw.ceil() + gap_icon + if meter.graph { graph_w } else { bar_w };
        n += 1;
      }
      w += gap_meter * (n.max(1) - 1) as f64;
      w.ceil() as i32
    }
  };
  area.set_content_width(width_of());

  // Tooltip per meter.
  area.set_has_tooltip(true);
  let m = meters.clone();
  area.connect_query_tooltip(move |_, x, _, _, tooltip| {
    let x = x as f64;
    match m.borrow().iter().find(|m| m.visible && x >= m.x0 - 2.0 && x <= m.x1 + 2.0) {
      Some(meter) => {
        tooltip.set_text(Some(&meter.tip));
        true
      }
      None => false,
    }
  });
  widget::on_press(&area, |_| crate::run::shell("omarchy-launch-or-focus-tui btop"));

  // The bars ease to their values (about 400 ms), only while they move.
  let animating = Rc::new(Cell::new(false));
  let animate = {
    let (m, a, animating) = (meters.clone(), area.downgrade(), animating.clone());
    move || {
      if animating.replace(true) {
        return;
      }
      let (m, a, animating) = (m.clone(), a.clone(), animating.clone());
      glib::timeout_add_local(Duration::from_millis(33), move || {
        let Some(a) = a.upgrade() else { return glib::ControlFlow::Break };
        let mut moved = false;
        for meter in m.borrow_mut().iter_mut() {
          let d = meter.value - meter.shown;
          if d.abs() > 0.002 {
            meter.shown += d * 0.25;
            moved = true;
          } else {
            meter.shown = meter.value;
          }
        }
        a.queue_draw();
        if moved {
          glib::ControlFlow::Continue
        } else {
          animating.set(false);
          glib::ControlFlow::Break
        }
      });
    }
  };

  // Values: /proc every interval, the battery as UPower says.
  let last = Rc::new(Cell::new(None::<Sample>));
  let refresh = {
    let (m, a) = (meters.clone(), area.downgrade());
    let width_of = width_of.clone();
    move || {
      let Some(a) = a.upgrade() else { return false };
      let stat = std::fs::read_to_string("/proc/stat").unwrap_or_default();
      let meminfo = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
      let mut ms = m.borrow_mut();
      // The first CPU value needs a second sample: no graph point yet.
      let first = last.get().is_none();
      if let Some((sum, idle)) = cpu_sample(&stat) {
        if let Some(prev) = last.get() {
          let (dt, di) = (sum.saturating_sub(prev.sum), idle.saturating_sub(prev.idle));
          if dt > 0 {
            ms[0].value = (1.0 - di as f64 / dt as f64).clamp(0.0, 1.0);
          }
        }
        last.set(Some(Sample { sum, idle }));
      }
      ms[0].fill = if ms[0].value > 0.75 { yellow } else { accent };
      ms[0].tip = format!("CPU: {}%", (ms[0].value * 100.0).round());
      if let Some((total, avail)) = memory(&meminfo).filter(|(t, _)| *t > 0) {
        ms[1].value = (total - avail.min(total)) as f64 / total as f64;
        ms[1].tip = format!(
          "Memória: {}% ({:.1} / {:.1} GiB)",
          (ms[1].value * 100.0).round(),
          (total - avail.min(total)) as f64 / 1048576.0,
          total as f64 / 1048576.0
        );
      }
      ms[1].fill = if ms[1].value > 0.75 { yellow } else { accent };
      for meter in ms.iter_mut().filter(|m| m.graph).skip(first as usize) {
        meter.history.push_back(meter.value);
        if meter.history.len() > HISTORY {
          meter.history.pop_front();
        }
      }
      let up = crate::upower::get();
      let present = up.present();
      let bat = up.fraction();
      let plugged = present && !up.on_battery();
      let was = ms[2].visible;
      ms[2].visible = present && (up.on_battery() || bat < 0.995);
      ms[2].value = bat;
      ms[2].icon = if plugged { '\u{F0241}' } else { '\u{F140B}' };
      ms[2].icon_color = plugged.then_some(accent);
      ms[2].fill = if bat < 0.2 { red } else if bat < 0.35 { yellow } else { accent };
      let charging = plugged && bat < 1.0;
      ms[2].tip = format!(
        "Bateria: {}%{}",
        (bat * 100.0).round(),
        if charging { " (carregando)" } else if up.on_battery() { "" } else { " (na tomada)" }
      );
      let resized = was != ms[2].visible;
      drop(ms);
      if resized {
        a.set_content_width(width_of());
      }
      animate();
      true
    }
  };
  refresh();
  let r = refresh.clone();
  glib::timeout_add_seconds_local(interval as u32, move || if r() { glib::ControlFlow::Continue } else { glib::ControlFlow::Break });
  let r = refresh.clone();
  crate::upower::get().connect(move || {
    r();
  });
  Some(Slot::new(ctx, &area).widget.clone().upcast())
}

// The samples as an area filled under a line, the newest at the right end,
// over a faint rounded track.
fn draw_graph(cr: &gtk::cairo::Context, history: &VecDeque<f64>, x: f64, y: f64, w: f64, h: f64, stroke: [f64; 4], fill: [f64; 4]) {
  let r = 3.0_f64.min(h / 2.0);
  widget::rounded(cr, x, y, w, h, r);
  cr.set_source_rgba(stroke[0], stroke[1], stroke[2], 0.10);
  let _ = cr.fill_preserve();
  cr.save().ok();
  cr.clip();
  let step = w / (HISTORY - 1) as f64;
  let n = history.len();
  let point = |i: usize, v: f64| (x + w - (n - 1 - i) as f64 * step, y + h - 1.0 - (h - 2.0) * v.clamp(0.0, 1.0));
  if n >= 2 {
    let (x0, _) = point(0, 0.0);
    cr.move_to(x0, y + h);
    for (i, v) in history.iter().enumerate() {
      let (px, py) = point(i, *v);
      cr.line_to(px, py);
    }
    cr.line_to(x + w, y + h);
    cr.close_path();
    cr.set_source_rgba(fill[0], fill[1], fill[2], 0.35);
    let _ = cr.fill();
    for (i, v) in history.iter().enumerate() {
      let (px, py) = point(i, *v);
      if i == 0 { cr.move_to(px, py) } else { cr.line_to(px, py) }
    }
    cr.set_source_rgba(fill[0], fill[1], fill[2], fill[3]);
    cr.set_line_width(1.5);
    cr.set_line_join(gtk::cairo::LineJoin::Round);
    let _ = cr.stroke();
  }
  cr.restore().ok();
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn reads_proc() {
    assert_eq!(cpu_sample("cpu  10 0 10 70 10 0 0 0 0 0\ncpu0 1 2 3 4"), Some((100, 80)));
    assert_eq!(memory("MemTotal:  1000 kB\nMemFree: 1 kB\nMemAvailable:  250 kB\n"), Some((1000, 250)));
  }
}
