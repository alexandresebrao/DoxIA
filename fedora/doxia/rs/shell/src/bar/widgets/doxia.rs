// DoxIA's own bar widgets (fedora/doxia/plugins): the emblem that opens the
// Start menu, the workspaces and the screen-sharing dot.

use super::super::widget::{self, Button, Kind, Slot};
use super::super::Ctx;
use crate::mango;
use gtk::gdk_pixbuf::Pixbuf;
use gtk::prelude::*;
use gtk::glib;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

fn theme_file(name: &str) -> std::path::PathBuf {
  doxia_theme::theme_dir().join(name)
}

// doxia.emblem (Emblem.qml): the theme's emblem.png (the DoxIA ∞), or the
// Omarchy glyph without one. A click opens the Start menu, a right click a
// terminal.
pub fn emblem(ctx: &Rc<Ctx>) -> gtk::Widget {
  let size = ctx.bar_size();
  let unit = (size - 8).max(14) as f64;
  let height = (unit * 0.78).round() as i32;
  let holder = gtk::Box::new(gtk::Orientation::Horizontal, 0);
  holder.add_css_class("bar-emblem");
  holder.set_cursor_from_name(Some("pointer"));
  holder.set_tooltip_text(Some("Iniciar"));
  match Pixbuf::from_file_at_scale(theme_file("emblem.png"), -1, height * 2, true) {
    Ok(pixbuf) => {
      // Drawn from a pixbuf twice the size, so it stays sharp on scaled
      // screens; a Picture would ask for the texture's own (double) size.
      let width = (pixbuf.width() as f64 * height as f64 / pixbuf.height().max(1) as f64).round() as i32;
      let area = gtk::DrawingArea::new();
      area.set_content_width(width);
      area.set_content_height(height);
      area.set_halign(gtk::Align::Center);
      area.set_valign(gtk::Align::Center);
      area.set_hexpand(true);
      area.set_draw_func(move |_, cr, w, h| {
        cr.scale(w as f64 / pixbuf.width() as f64, h as f64 / pixbuf.height() as f64);
        cr.set_source_pixbuf(&pixbuf, 0.0, 0.0);
        let _ = cr.paint();
      });
      if ctx.vertical() { holder.set_size_request(size, height + 14) } else { holder.set_size_request(width + 14, size) }
      holder.append(&area);
    }
    Err(_) => {
      let glyph = gtk::Label::new(Some("\u{e900}"));
      glyph.add_css_class("bar-emblem-glyph");
      glyph.set_hexpand(true);
      holder.append(&glyph);
      if ctx.vertical() { holder.set_size_request(size, -1) } else { holder.set_size_request(-1, size) }
    }
  }
  widget::on_press(&holder, |b| {
    if b == 3 {
      crate::run::shell("xdg-terminal-exec");
    } else {
      crate::run::argv(&["omarchy-shell", "shell", "toggle", "doxia.iniciar"]);
    }
  });
  Slot::new(ctx, &holder).widget.clone().upcast()
}

const ICONS: [(u32, &str); 6] = [(1, "\u{f120}"), (2, "\u{f0ac}"), (3, "\u{f086}"), (4, "\u{f121}"), (5, "\u{f0e0}"), (6, "\u{f1bc}")];

fn icon(id: u32) -> Option<&'static str> {
  ICONS.iter().find(|(i, _)| *i == id).map(|(_, g)| *g)
}

// A workspace's icon or number, centered on its ink rather than its advance:
// these icons sit a couple of pixels right of their advance's middle, which
// a label would center (like OpticalGlyph in the Quickshell bar). Font and
// color from the CSS (.bar-workspace).
fn plate(text: String) -> gtk::DrawingArea {
  let area = gtk::DrawingArea::new();
  area.set_draw_func(move |a, cr, w, h| {
    let layout = a.create_pango_layout(Some(&text));
    let (ink, _) = layout.pixel_extents();
    let x = (w - ink.width()) as f64 / 2.0 - ink.x() as f64;
    let y = (h - ink.height()) as f64 / 2.0 - ink.y() as f64;
    let c = a.color();
    cr.set_source_rgba(c.red() as f64, c.green() as f64, c.blue() as f64, c.alpha() as f64);
    cr.move_to(x.round(), y.round());
    pangocairo::functions::show_layout(cr, &layout);
  });
  area
}

// 1..6 always, and any tag up to 10 with windows or on screen.
pub fn workspace_ids(tags: &[mango::Tag]) -> Vec<u32> {
  let mut ids: Vec<u32> = (1..=6).collect();
  for t in tags {
    if t.index > 0 && t.index <= 10 && (t.clients > 0 || t.active) && !ids.contains(&t.index) {
      ids.push(t.index);
    }
  }
  ids.sort();
  ids
}

fn out_cubic(t: f64) -> f64 {
  1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

// doxia.workspaces (Workspaces.qml): the tags of this bar's screen, the fixed
// ones as icons, with a glass pill in the theme's accent sliding to the
// active one. A click views a tag; the wheel steps through the busy ones.
pub fn workspaces(ctx: &Rc<Ctx>) -> gtk::Widget {
  let size = ctx.bar_size();
  let vertical = ctx.vertical();
  let plate_h = (size as f64 * 0.82).round() as i32;
  let plate_w = (plate_h as f64 * 1.05).round() as i32;
  let accent = ctx.style.accent;
  let monitor = ctx.monitor.clone();

  let overlay = gtk::Overlay::new();
  overlay.add_css_class("bar-workspaces");
  let pill = gtk::DrawingArea::new();
  overlay.set_child(Some(&pill));
  let row = gtk::Box::new(if vertical { gtk::Orientation::Vertical } else { gtk::Orientation::Horizontal }, 4);
  row.set_halign(gtk::Align::Center);
  row.set_valign(gtk::Align::Center);
  if vertical { row.set_margin_top(4); row.set_margin_bottom(4) } else { row.set_margin_start(4); row.set_margin_end(4) }
  overlay.add_overlay(&row);
  overlay.set_measure_overlay(&row, true);

  // The pill's place along the row, as a plate index: where it is and
  // where it goes. Drawn centered on the plates as laid out.
  let from = Rc::new(Cell::new(-1.0f64));
  let to = Rc::new(Cell::new(-1.0f64));
  let started = Rc::new(Cell::new(None::<Instant>));
  let plates: Rc<RefCell<Vec<(u32, gtk::DrawingArea)>>> = Rc::default();
  let (f, t, st, pl) = (from.clone(), to.clone(), started.clone(), plates.clone());
  pill.set_draw_func(move |area, cr, w, h| {
    if t.get() < 0.0 {
      return;
    }
    let (pw, ph) = if vertical { (plate_h as f64, plate_w as f64) } else { (plate_w as f64, plate_h as f64) };
    // A plate's pill start: its center less half the pill; between two
    // plates (an index caught mid-slide), in proportion.
    let plate_start = |i: usize| -> Option<f64> {
      let plates = pl.borrow();
      let (_, label) = plates.get(i)?;
      let b = label.compute_bounds(area)?;
      Some(if vertical { b.y() as f64 + (b.height() as f64 - ph) / 2.0 } else { b.x() as f64 + (b.width() as f64 - pw) / 2.0 })
    };
    let start_of = |index: f64| -> Option<f64> {
      let (lo, hi) = (index.floor() as usize, index.ceil() as usize);
      let a = plate_start(lo)?;
      Some(a + (plate_start(hi).unwrap_or(a) - a) * index.fract())
    };
    let Some(end) = start_of(t.get()) else { return };
    let progress = st.get().map(|s| out_cubic(s.elapsed().as_secs_f64() / 0.22)).unwrap_or(1.0);
    let at = match start_of(f.get()).filter(|_| f.get() >= 0.0) {
      Some(begin) => begin + (end - begin) * progress,
      None => end,
    };
    let (x, y) = if vertical { ((w as f64 - pw) / 2.0, at) } else { (at, (h as f64 - ph) / 2.0) };
    let r = pw.min(ph) / 2.0;
    widget::rounded(cr, x + 0.5, y + 0.5, pw - 1.0, ph - 1.0, r);
    cr.set_source_rgba(accent[0], accent[1], accent[2], 0.18);
    let _ = cr.fill_preserve();
    cr.set_source_rgba(accent[0], accent[1], accent[2], 0.3);
    cr.set_line_width(1.0);
    let _ = cr.stroke();
  });

  let mango = mango::get();
  let render = {
    let (row, plates, pill, mango, monitor) = (row.clone(), plates.clone(), pill.clone(), mango.clone(), monitor.clone());
    let (from, to, started) = (from.clone(), to.clone(), started.clone());
    move || {
      let tags = mango.tags(&monitor);
      let ids = workspace_ids(&tags);
      let current: Vec<u32> = plates.borrow().iter().map(|(i, _)| *i).collect();
      if current != ids {
        crate::popup::clear(&row);
        let mut list = Vec::new();
        for id in &ids {
          let text = icon(*id).map(String::from).unwrap_or_else(|| if *id == 10 { "0".into() } else { id.to_string() });
          let l = plate(text);
          l.add_css_class("bar-workspace");
          if icon(*id).is_none() {
            l.add_css_class("number");
          }
          if vertical { l.set_size_request(size, plate_w) } else { l.set_size_request(plate_w, size) }
          l.set_cursor_from_name(Some("pointer"));
          let (m, id) = (monitor.clone(), *id);
          widget::on_press(&l, move |_| {
            if !m.is_empty() {
              mango::dispatch(&["focusmon", &m]);
            }
            mango::dispatch(&["view", &id.to_string()]);
          });
          row.append(&l);
          list.push((id, l));
        }
        *plates.borrow_mut() = list;
      }
      let mut active_at = None;
      for (i, (id, l)) in plates.borrow().iter().enumerate() {
        let tag = tags.iter().find(|t| t.index == *id);
        let focused = tag.is_some_and(|t| t.active);
        let occupied = tag.is_some_and(|t| t.clients > 0);
        for (class, on) in [("focused", focused), ("occupied", occupied && !focused)] {
          if on { l.add_css_class(class) } else { l.remove_css_class(class) }
          l.queue_draw();
        }
        if focused {
          active_at = Some(i);
        }
      }
      // Slide the pill to the active plate.
      let target = active_at.map(|i| i as f64).unwrap_or(-1.0);
      if target != to.get() {
        let progress = started.get().map(|s| out_cubic(s.elapsed().as_secs_f64() / 0.22)).unwrap_or(1.0);
        let now_at = if from.get() < 0.0 || to.get() < 0.0 { to.get() } else { from.get() + (to.get() - from.get()) * progress };
        from.set(now_at);
        to.set(target);
        started.set(Some(Instant::now()));
        let (p, s) = (pill.clone(), started.clone());
        glib::timeout_add_local(Duration::from_millis(16), move || {
          p.queue_draw();
          if s.get().is_some_and(|t| t.elapsed() < Duration::from_millis(240)) { glib::ControlFlow::Continue } else { glib::ControlFlow::Break }
        });
      }
      pill.queue_draw();
    }
  };
  render();
  let weak = overlay.downgrade();
  let r = render.clone();
  mango.connect(move || {
    if weak.upgrade().is_some() {
      r();
    }
  });
  let m = monitor.clone();
  widget::on_wheel(&overlay, move |_, dy| {
    if dy == 0.0 {
      return;
    }
    if !m.is_empty() {
      mango::dispatch(&["focusmon", &m]);
    }
    mango::dispatch(&[if dy < 0.0 { "viewtoleft_have_client" } else { "viewtoright_have_client" }]);
  });
  Slot::new(ctx, &overlay).widget.clone().upcast()
}

// doxia.screenshare (Panel.qml): a red dot, pulsing, while the wlr portal
// streams the screen (its PipeWire node). A click stops the sharing
// (stop.sh). Chrome's "is sharing" bar, which mango's rule hides when it
// opens, sometimes gets its title later: it is tucked away again for a few
// seconds after sharing starts.
pub fn screenshare(ctx: &Rc<Ctx>) -> gtk::Widget {
  let button = Button::new(ctx, Kind::Icon, "\u{F044A}");
  button.widget.add_css_class("bar-screenshare");
  button.set_tooltip("Compartilhando a tela — clique para parar");
  button.widget.set_visible(false);
  let dir = format!("{}/fedora/doxia/plugins/doxia.screenshare", std::env::var("OMARCHY_PATH").unwrap_or_default());
  let stop = format!("{dir}/stop.sh");
  button.on_press(move |_| crate::run::argv(&["sh", &stop]));
  let sharing = Rc::new(Cell::new(false));
  let w = button.widget.downgrade();
  let check = move || {
    let (w, sharing) = (w.clone(), sharing.clone());
    glib::spawn_future_local(async move {
      let out = crate::watch::shell("pw-cli ls Node 2>/dev/null | grep -c 'node.name = \"xdg-desktop-portal-wlr\"'").await.unwrap_or_default();
      let now = out.trim().parse::<u32>().unwrap_or(0) > 0;
      let Some(w) = w.upgrade() else { return };
      if now == sharing.get() {
        return;
      }
      sharing.set(now);
      w.set_visible(now);
      if now {
        pulse(&w);
        hide_chrome_bar();
      }
    });
  };
  check();
  let weak = button.widget.downgrade();
  glib::timeout_add_seconds_local(2, move || {
    if weak.upgrade().is_none() {
      return glib::ControlFlow::Break;
    }
    check();
    glib::ControlFlow::Continue
  });
  Slot::new(ctx, &button.widget).widget.clone().upcast()
}

// 1 → 0.35 → 1 over 1.8 s, while visible.
fn pulse(w: &gtk::Box) {
  let start = Instant::now();
  let weak = w.downgrade();
  glib::timeout_add_local(Duration::from_millis(33), move || {
    let Some(w) = weak.upgrade() else { return glib::ControlFlow::Break };
    if !w.is_visible() {
      w.set_opacity(1.0);
      return glib::ControlFlow::Break;
    }
    let phase = (start.elapsed().as_secs_f64() / 1.8).fract();
    let s = (phase * std::f64::consts::TAU).cos() * 0.5 + 0.5;
    w.set_opacity(0.35 + 0.65 * s);
    glib::ControlFlow::Continue
  });
}

fn hide_chrome_bar() {
  let left = Rc::new(Cell::new(6));
  glib::timeout_add_local(Duration::from_millis(700), move || {
    crate::run::shell(
      "mmsg get all-clients | jq -r '.clients[] | select(.title | test(\"(está compartilhando|is sharing) \")) | select(.is_visible) | .id' | while read id; do mmsg dispatch tag_special_silent \"client,$id\"; done",
    );
    left.set(left.get() - 1);
    if left.get() > 0 { glib::ControlFlow::Continue } else { glib::ControlFlow::Break }
  });
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn shows_the_fixed_tags_and_busy_ones() {
    let tag = |index, active, clients| mango::Tag { index, active, urgent: false, clients, layout: String::new() };
    assert_eq!(workspace_ids(&[]), [1, 2, 3, 4, 5, 6]);
    assert_eq!(workspace_ids(&[tag(8, false, 2), tag(9, true, 0), tag(7, false, 0)]), [1, 2, 3, 4, 5, 6, 8, 9]);
  }
}
