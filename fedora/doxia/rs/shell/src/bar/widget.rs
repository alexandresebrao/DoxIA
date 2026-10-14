// What the bar's widgets are made of, after shell/Ui's WidgetButton,
// BarIconButton and BarIndicator: a label that takes clicks and the wheel,
// in the bar's text color (its "active" color when it calls for attention),
// dimmed or hidden, with a tooltip; and the slot every widget sits in, which
// marks the widget whose panel is open with a short accent line on the edge
// facing the desktop.

use super::Ctx;
use gtk::prelude::*;
use gtk::{cairo, gdk, glib};
use std::cell::Cell;
use std::rc::Rc;

pub use crate::card::rounded;

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
  // A glyph in a square slot (BarIconButton).
  Icon,
  // A status glyph in a narrower slot, caption sized (BarIndicator).
  Status,
  // Text with margins (WidgetButton).
  Text,
}

pub struct Button {
  pub widget: gtk::Box,
  pub label: gtk::Label,
}

impl Button {
  pub fn new(ctx: &Ctx, kind: Kind, text: &str) -> Rc<Button> {
    let orientation = if ctx.vertical() { gtk::Orientation::Vertical } else { gtk::Orientation::Horizontal };
    let widget = gtk::Box::new(orientation, 0);
    widget.add_css_class("bar-button");
    let label = gtk::Label::new(Some(text));
    label.set_hexpand(true);
    label.set_vexpand(true);
    label.set_halign(gtk::Align::Center);
    label.set_valign(gtk::Align::Center);
    widget.append(&label);
    let size = ctx.bar_size();
    let s = &ctx.style;
    match kind {
      Kind::Icon => {
        widget.add_css_class("bar-icon");
        if ctx.vertical() { widget.set_size_request(size, s.icon_slot) } else { widget.set_size_request(s.icon_slot, size) }
      }
      Kind::Status => {
        widget.add_css_class("bar-status");
        if ctx.vertical() { widget.set_size_request(size, s.status_slot) } else { widget.set_size_request(s.status_slot, size) }
      }
      Kind::Text => {
        widget.add_css_class("bar-text");
        if ctx.vertical() {
          widget.set_size_request(size, -1);
          let pad = (6.0 * s.spacing).round() as i32;
          label.set_margin_top(pad);
          label.set_margin_bottom(pad);
        } else {
          widget.set_size_request(-1, size);
          let pad = (8.5 * s.spacing).round() as i32;
          label.set_margin_start(pad);
          label.set_margin_end(pad);
        }
      }
    }
    widget.set_cursor_from_name(Some("pointer"));
    Rc::new(Button { widget, label })
  }

  pub fn set_text(&self, text: &str) {
    if self.label.text() != text {
      self.label.set_text(text);
    }
    self.widget.set_visible(!text.is_empty());
  }

  pub fn set_tooltip(&self, text: &str) {
    self.widget.set_tooltip_text((!text.is_empty()).then_some(text));
  }

  pub fn set_active(&self, active: bool) {
    if active { self.widget.add_css_class("active") } else { self.widget.remove_css_class("active") }
  }

  pub fn set_dimmed(&self, dimmed: bool) {
    if dimmed { self.widget.add_css_class("dimmed") } else { self.widget.remove_css_class("dimmed") }
  }

  // Clicks, with the button (1 left, 2 middle, 3 right).
  pub fn on_press(&self, f: impl Fn(u32) + 'static) {
    on_press(&self.widget, f);
  }

  // The wheel: dy > 0 is down (towards the user).
  pub fn on_wheel(&self, f: impl Fn(f64, f64) + 'static) {
    on_wheel(&self.widget, f);
  }
}

pub fn on_press(widget: &impl IsA<gtk::Widget>, f: impl Fn(u32) + 'static) {
  let click = gtk::GestureClick::new();
  click.set_button(0);
  click.connect_released(move |g, _, x, y| {
    // Only a release over the widget, as a click.
    if let Some(w) = g.widget() {
      if x < 0.0 || y < 0.0 || x > w.width() as f64 || y > w.height() as f64 {
        return;
      }
    }
    f(g.current_button());
  });
  widget.add_controller(click);
}

pub fn on_wheel(widget: &impl IsA<gtk::Widget>, f: impl Fn(f64, f64) + 'static) {
  let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES | gtk::EventControllerScrollFlags::DISCRETE);
  scroll.connect_scroll(move |_, dx, dy| {
    f(dx, dy);
    glib::Propagation::Stop
  });
  widget.add_controller(scroll);
}

// The slot around a widget: it carries the open-panel mark.
pub struct Slot {
  pub widget: gtk::Overlay,
  open: Rc<Cell<bool>>,
  mark: gtk::DrawingArea,
}

impl Slot {
  pub fn new(ctx: &Ctx, child: &impl IsA<gtk::Widget>) -> Rc<Slot> {
    let widget = gtk::Overlay::new();
    widget.set_child(Some(child));
    let mark = gtk::DrawingArea::new();
    mark.set_can_target(false);
    let open = Rc::new(Cell::new(false));
    let (o, position, accent, thick, inset, min) =
      (open.clone(), ctx.position, ctx.style.accent, ctx.style.space(2.0) as f64, ctx.style.space(2.0) as f64, ctx.style.space(10.0) as f64);
    mark.set_draw_func(move |_, cr, w, h| {
      if !o.get() {
        return;
      }
      let (w, h) = (w as f64, h as f64);
      let along = (if position.vertical() { h } else { w } * 0.55).round().max(min);
      let (x, y, mw, mh) = match position {
        super::config::Position::Top => ((w - along) / 2.0, h - thick - inset, along, thick),
        super::config::Position::Bottom => ((w - along) / 2.0, inset, along, thick),
        super::config::Position::Left => (w - thick - inset, (h - along) / 2.0, thick, along),
        super::config::Position::Right => (inset, (h - along) / 2.0, thick, along),
      };
      draw_pill(cr, x.round(), y.round(), mw, mh, [accent[0], accent[1], accent[2], 0.9]);
    });
    widget.add_overlay(&mark);
    // The mark follows the child: hidden with it.
    let m = mark.clone();
    child.as_ref().connect_visible_notify(move |c| m.set_visible(c.is_visible()));
    Rc::new(Slot { widget, open, mark })
  }

  pub fn set_open(&self, open: bool) {
    if self.open.get() != open {
      self.open.set(open);
      self.mark.queue_draw();
    }
  }
}

pub fn draw_pill(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, c: [f64; 4]) {
  rounded(cr, x, y, w, h, w.min(h) / 2.0);
  cr.set_source_rgba(c[0], c[1], c[2], c[3]);
  let _ = cr.fill();
}

pub fn rgba(c: [f64; 4]) -> gdk::RGBA {
  gdk::RGBA::new(c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32)
}
