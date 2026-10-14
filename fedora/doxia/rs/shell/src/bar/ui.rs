// What the bar's panels are made of, after shell/Ui: the hero (a large
// glyph beside a title and a small-caps subtitle), section headers, a
// progress rail, wrapped text, buttons (bordered, primary, plain glyphs),
// toggles and rows that take clicks.

use super::widget::{self, rounded};
use gtk::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

pub fn label(text: &str, classes: &str) -> gtk::Label {
  let l = gtk::Label::new(Some(text));
  for c in classes.split_whitespace() {
    l.add_css_class(c);
  }
  l
}

pub fn vbox(spacing: i32) -> gtk::Box {
  gtk::Box::new(gtk::Orientation::Vertical, spacing)
}

pub fn hbox(spacing: i32) -> gtk::Box {
  gtk::Box::new(gtk::Orientation::Horizontal, spacing)
}

// PanelHero: a display-size glyph, then the title over the subtitle.
pub struct Hero {
  pub widget: gtk::Box,
  pub icon: gtk::Label,
  pub title: gtk::Label,
  pub subtitle: gtk::Label,
}

impl Hero {
  pub fn new(space: &impl Fn(f64) -> i32) -> Hero {
    let widget = hbox(space(14.0));
    widget.add_css_class("panel-hero");
    let icon = label("", "panel-hero-icon");
    let col = vbox(space(2.0));
    col.set_valign(gtk::Align::Center);
    col.set_hexpand(true);
    let title = label("", "panel-hero-title");
    title.set_xalign(0.0);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let subtitle = label("", "panel-hero-subtitle");
    subtitle.set_xalign(0.0);
    subtitle.set_ellipsize(gtk::pango::EllipsizeMode::End);
    col.append(&title);
    col.append(&subtitle);
    widget.append(&icon);
    widget.append(&col);
    Hero { widget, icon, title, subtitle }
  }

  pub fn set(&self, icon: &str, title: &str, subtitle: &str) {
    self.icon.set_text(icon);
    self.title.set_text(title);
    self.subtitle.set_text(subtitle);
    self.subtitle.set_visible(!subtitle.is_empty());
  }
}

// PanelSectionHeader: small caps, dimmed.
pub fn section(text: &str) -> gtk::Label {
  let l = label(&text.to_uppercase(), "panel-section");
  l.set_xalign(0.0);
  l
}

pub fn separator() -> gtk::Box {
  let b = hbox(0);
  b.add_css_class("panel-separator");
  b
}

// Wrapped body text.
pub fn text(text: &str) -> gtk::Label {
  let l = label(text, "panel-text");
  l.set_wrap(true);
  l.set_wrap_mode(gtk::pango::WrapMode::WordChar);
  l.set_xalign(0.0);
  l
}

// A rail filled to `fraction` in the accent (the update's progress, a
// battery's charge).
pub struct Rail {
  pub widget: gtk::DrawingArea,
  fraction: Rc<Cell<f64>>,
}

impl Rail {
  pub fn new(height: i32, accent: [f64; 4]) -> Rail {
    let widget = gtk::DrawingArea::new();
    widget.add_css_class("panel-rail");
    widget.set_content_height(height);
    widget.set_hexpand(true);
    let fraction = Rc::new(Cell::new(0.0f64));
    let f = fraction.clone();
    widget.set_draw_func(move |w, cr, width, height| {
      let fg = w.color();
      let (wd, h) = (width as f64, height as f64);
      rounded(cr, 0.0, 0.0, wd, h, h / 2.0);
      cr.set_source_rgba(fg.red() as f64, fg.green() as f64, fg.blue() as f64, 0.12);
      let _ = cr.fill();
      let done = (wd * f.get().clamp(0.0, 1.0)).round();
      if done > 0.0 {
        rounded(cr, 0.0, 0.0, done, h, h / 2.0);
        cr.set_source_rgba(accent[0], accent[1], accent[2], accent[3]);
        let _ = cr.fill();
      }
    });
    Rail { widget, fraction }
  }

  pub fn set(&self, fraction: f64) {
    self.fraction.set(fraction);
    self.widget.queue_draw();
  }
}

// A bordered button (shell/Ui/Button), or the primary one.
pub fn button(text: &str, primary: bool, on_click: impl Fn() + 'static) -> gtk::Button {
  let b = gtk::Button::with_label(text);
  b.add_css_class(if primary { "primary" } else { "ctl" });
  b.set_cursor_from_name(Some("pointer"));
  b.set_focus_on_click(false);
  b.connect_clicked(move |_| on_click());
  b
}

// A glyph that takes clicks (PanelActionButton).
pub fn action(glyph: &str, tooltip: &str, on_click: impl Fn() + 'static) -> gtk::Label {
  let l = label(glyph, "panel-action");
  l.set_cursor_from_name(Some("pointer"));
  if !tooltip.is_empty() {
    l.set_tooltip_text(Some(tooltip));
  }
  widget::on_press(&l, move |_| on_click());
  l
}

// A switch for a setting (Toggle / ToggleSwitch).
pub fn toggle(active: bool, on_change: impl Fn(bool) + 'static) -> gtk::Switch {
  let s = gtk::Switch::new();
  s.set_active(active);
  s.set_valign(gtk::Align::Center);
  s.set_focus_on_click(false);
  s.connect_state_set(move |_, on| {
    on_change(on);
    gtk::glib::Propagation::Proceed
  });
  s
}

// A row that highlights on hover and takes clicks.
pub fn row(child: &impl IsA<gtk::Widget>, on_click: impl Fn(u32) + 'static) -> gtk::Box {
  let b = hbox(0);
  b.add_css_class("panel-row");
  b.append(child);
  b.set_cursor_from_name(Some("pointer"));
  widget::on_press(&b, on_click);
  b
}

// PanelSlider: a thin track filled to the value, with a round knob that
// grows under the pointer. Dragging reports `moved` as it goes and
// `released` at the end; the wheel steps; a right click is a secondary
// action (mute, for the audio sliders).
pub struct Slider {
  pub widget: gtk::DrawingArea,
  value: Cell<f64>,
  live: Rc<Cell<f64>>,
  dragging: Rc<Cell<bool>>,
  min: f64,
  max: f64,
  on_moved: std::cell::RefCell<Option<Rc<dyn Fn(f64)>>>,
  on_released: std::cell::RefCell<Option<Rc<dyn Fn(f64)>>>,
  on_right: std::cell::RefCell<Option<Rc<dyn Fn()>>>,
}

impl Slider {
  pub fn new(min: f64, max: f64, step: f64, space: &impl Fn(f64) -> i32) -> Rc<Slider> {
    let widget = gtk::DrawingArea::new();
    widget.add_css_class("panel-slider");
    widget.set_hexpand(true);
    widget.set_content_width(space(200.0));
    let knob = 14.0f64.max((28.0 * 0.38f64).round());
    widget.set_content_height(space(22.0).max((knob + 6.0) as i32));
    widget.set_cursor_from_name(Some("pointer"));
    let me = Rc::new(Slider {
      widget,
      value: Cell::new(min),
      live: Rc::new(Cell::new(min)),
      dragging: Rc::new(Cell::new(false)),
      min,
      max,
      on_moved: Default::default(),
      on_released: Default::default(),
      on_right: Default::default(),
    });
    let hot = Rc::new(Cell::new(false));
    let (live, h, range) = (me.live.clone(), hot.clone(), (min, max));
    // Read once: a theme switch rebuilds the bar and its panels.
    let c = doxia_theme::controls();
    me.widget.set_draw_func(move |_, cr, w, height| {
      let (w, height) = (w as f64, height as f64);
      let track_h = 4.0f64.max((28.0 * 0.11f64).round());
      let ty = (height - track_h) / 2.0;
      rounded(cr, 0.0, ty, w, track_h, track_h / 2.0);
      cr.set_source_rgba(c.selected_fill[0], c.selected_fill[1], c.selected_fill[2], c.selected_fill[3]);
      let _ = cr.fill();
      let progress = ((live.get() - range.0) / (range.1 - range.0).max(0.0001)).clamp(0.0, 1.0);
      if progress > 0.0 {
        rounded(cr, 0.0, ty, w * progress, track_h, track_h / 2.0);
        cr.set_source_rgba(c.text[0], c.text[1], c.text[2], c.text[3]);
        let _ = cr.fill();
      }
      let size = if h.get() { knob * 1.15 } else { knob };
      let kx = (w * progress - knob / 2.0).clamp(0.0, w - knob) + knob / 2.0;
      cr.arc(kx, height / 2.0, size / 2.0, 0.0, std::f64::consts::TAU);
      cr.set_source_rgba(c.background[0], c.background[1], c.background[2], 1.0);
      let _ = cr.fill();
      cr.arc(kx, height / 2.0, size / 2.0 - 2.0, 0.0, std::f64::consts::TAU);
      cr.set_source_rgba(c.text[0], c.text[1], c.text[2], c.text[3]);
      let _ = cr.fill();
    });

    let motion = gtk::EventControllerMotion::new();
    let (h1, h2, w1, w2) = (hot.clone(), hot.clone(), me.widget.downgrade(), me.widget.downgrade());
    motion.connect_enter(move |_, _, _| {
      h1.set(true);
      if let Some(w) = w1.upgrade() {
        w.queue_draw();
      }
    });
    motion.connect_leave(move |_| {
      h2.set(false);
      if let Some(w) = w2.upgrade() {
        w.queue_draw();
      }
    });
    me.widget.add_controller(motion);

    let value_at = {
      let w = me.widget.downgrade();
      move |x: f64| {
        let width = w.upgrade().map(|w| w.width() as f64).unwrap_or(1.0).max(1.0);
        (min + (x / width).clamp(0.0, 1.0) * (max - min)).clamp(min, max)
      }
    };
    let drag = gtk::GestureDrag::new();
    drag.set_button(1);
    let (weak, va) = (Rc::downgrade(&me), value_at.clone());
    drag.connect_drag_begin(move |_, x, _| {
      let Some(me) = weak.upgrade() else { return };
      me.dragging.set(true);
      me.set_live(va(x));
    });
    let (weak, va) = (Rc::downgrade(&me), value_at.clone());
    drag.connect_drag_update(move |g, dx, _| {
      let Some(me) = weak.upgrade() else { return };
      if let Some((x, _)) = g.start_point() {
        me.set_live(va(x + dx));
      }
    });
    let weak = Rc::downgrade(&me);
    drag.connect_drag_end(move |_, _, _| {
      let Some(me) = weak.upgrade() else { return };
      me.dragging.set(false);
      let v = me.live.get();
      let f = me.on_released.borrow().clone();
      if let Some(f) = f {
        f(v);
      }
      me.live.set(me.value.get());
      me.widget.queue_draw();
    });
    me.widget.add_controller(drag);

    let right = gtk::GestureClick::new();
    right.set_button(3);
    let weak = Rc::downgrade(&me);
    right.connect_released(move |_, _, _, _| {
      let f = weak.upgrade().and_then(|me| me.on_right.borrow().clone());
      if let Some(f) = f {
        f();
      }
    });
    me.widget.add_controller(right);

    let weak = Rc::downgrade(&me);
    widget::on_wheel(&me.widget, move |_, dy| {
      let Some(me) = weak.upgrade() else { return };
      if dy == 0.0 {
        return;
      }
      let next = (me.live.get() + if dy < 0.0 { step } else { -step }).clamp(me.min, me.max);
      me.set_live(next);
      let f = me.on_released.borrow().clone();
      if let Some(f) = f {
        f(next);
      }
    });
    me
  }

  fn set_live(&self, v: f64) {
    self.live.set(v);
    self.widget.queue_draw();
    let f = self.on_moved.borrow().clone();
    if let Some(f) = f {
      f(v);
    }
  }

  // The value from outside (ignored while dragging).
  pub fn set(&self, v: f64) {
    self.value.set(v);
    if !self.dragging.get() {
      self.live.set(v);
      self.widget.queue_draw();
    }
  }

  pub fn dragging(&self) -> bool {
    self.dragging.get()
  }

  // What it shows: the dragged value while dragging.
  pub fn live(&self) -> f64 {
    self.live.get()
  }

  pub fn connect_moved(&self, f: impl Fn(f64) + 'static) {
    *self.on_moved.borrow_mut() = Some(Rc::new(f));
  }

  pub fn connect_released(&self, f: impl Fn(f64) + 'static) {
    *self.on_released.borrow_mut() = Some(Rc::new(f));
  }

  pub fn connect_right_click(&self, f: impl Fn() + 'static) {
    *self.on_right.borrow_mut() = Some(Rc::new(f));
  }
}

// The keyboard cursor of a panel (CursorSurface + PanelKeyCatcher): rows in
// order, each with what Enter does, what Left/Right do (a volume step) and
// what m does. Up/Down walk them; the pointer over a row puts the cursor
// there; the first key only shows it.
#[derive(Clone)]
pub struct CursorRow {
  pub widget: gtk::Widget,
  pub activate: Option<Rc<dyn Fn()>>,
  pub adjust: Option<Rc<dyn Fn(f64)>>,
  pub mute: Option<Rc<dyn Fn()>>,
}

#[derive(Default)]
pub struct Cursor {
  rows: std::cell::RefCell<Vec<CursorRow>>,
  index: Cell<Option<usize>>,
  shown: Cell<bool>,
}

impl Cursor {
  pub fn new() -> Rc<Cursor> {
    Rc::new(Cursor::default())
  }

  pub fn clear(&self) {
    self.rows.borrow_mut().clear();
  }

  pub fn reset(&self) {
    self.shown.set(false);
    self.index.set(None);
    self.paint();
  }

  pub fn add(self: &Rc<Self>, row: CursorRow) {
    let i = self.rows.borrow().len();
    let motion = gtk::EventControllerMotion::new();
    let weak = Rc::downgrade(self);
    motion.connect_enter(move |_, _, _| {
      if let Some(me) = weak.upgrade() {
        me.shown.set(true);
        me.index.set(Some(i));
        me.paint();
      }
    });
    row.widget.add_controller(motion);
    self.rows.borrow_mut().push(row);
    self.paint();
  }

  fn paint(&self) {
    for (i, r) in self.rows.borrow().iter().enumerate() {
      if self.shown.get() && self.index.get() == Some(i) { r.widget.add_css_class("cursor") } else { r.widget.remove_css_class("cursor") }
    }
  }

  fn current(&self) -> Option<CursorRow> {
    self.index.get().and_then(|i| self.rows.borrow().get(i).cloned())
  }

  // Keys: true when used.
  pub fn key(&self, key: gtk::gdk::Key) -> bool {
    use gtk::gdk::Key;
    let n = self.rows.borrow().len();
    if n == 0 {
      return false;
    }
    let moving = matches!(key, Key::Up | Key::Down | Key::k | Key::j | Key::Left | Key::Right | Key::h | Key::l);
    if moving && !self.shown.get() {
      self.shown.set(true);
      if self.index.get().is_none() {
        self.index.set(Some(0));
      }
      self.paint();
      return true;
    }
    match key {
      Key::Down | Key::j => self.index.set(Some(self.index.get().map(|i| (i + 1).min(n - 1)).unwrap_or(0))),
      Key::Up | Key::k => self.index.set(Some(self.index.get().map(|i| i.saturating_sub(1)).unwrap_or(0))),
      Key::Left | Key::h | Key::Right | Key::l => {
        let step = if matches!(key, Key::Left | Key::h) { -0.05 } else { 0.05 };
        if let Some(f) = self.current().and_then(|r| r.adjust) {
          f(step);
        }
        return true;
      }
      Key::Return | Key::KP_Enter | Key::space => {
        if !self.shown.get() {
          return false;
        }
        if let Some(f) = self.current().and_then(|r| r.activate) {
          f();
        }
        return true;
      }
      Key::m | Key::M => {
        if let Some(f) = self.current().and_then(|r| r.mute) {
          f();
        }
        return true;
      }
      _ => return false,
    }
    self.paint();
    if let Some(r) = self.current() {
      // Keep it in view inside a scrolled panel.
      r.widget.grab_focus();
    }
    true
  }
}
