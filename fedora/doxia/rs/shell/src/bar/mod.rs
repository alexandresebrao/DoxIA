// The bar, replacing shell/plugins/bar (Bar.qml): one layer surface per
// screen on the edge shell.json's bar.position names, with the widgets of
// bar.layout in three sections. The left and right ones hug their ends; the
// center one is centered, or centered on bar.centerAnchor with the widgets
// before and after it hanging off its sides. bar.floating draws it as a
// rounded plate held off the edges; bar.transparent drops its fill.
//
// The `bar-off` toggle (~/.local/state/omarchy/toggles, omarchy-toggle-bar)
// parks it past its edge without an exclusive zone. IPC target
// "omarchy.bar": syncHidden, reservedTop.

pub mod config;
pub mod panel;
pub mod ui;
pub mod widget;
mod widgets;

use config::{BarConfig, Position};
use doxia_theme::BarStyle;
use gtk::prelude::*;
use gtk::{cairo, gdk, gio};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};

// What a widget is built with: its bar, the screen it is on and the look.
pub struct Ctx {
  pub bar: Weak<Bar>,
  pub monitor: String,
  pub position: Position,
  pub style: BarStyle,
  pub window: gtk::Window,
  // The first screen's bar runs what the whole shell needs once (the
  // update check).
  pub first_screen: bool,
}

impl Ctx {
  pub fn vertical(&self) -> bool {
    self.position.vertical()
  }

  pub fn bar_size(&self) -> i32 {
    if self.vertical() { self.style.size_vertical } else { self.style.size_horizontal }
  }
}

struct Screen {
  window: gtk::Window,
}

pub struct Bar {
  app: gtk::Application,
  ipc: Rc<crate::ipc::Ipc>,
  config: RefCell<BarConfig>,
  style: RefCell<BarStyle>,
  screens: RefCell<Vec<Screen>>,
  hidden: Cell<bool>,
  monitors: RefCell<Vec<gio::FileMonitor>>,
}

fn home() -> PathBuf {
  PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

fn off_flag() -> PathBuf {
  home().join(".local/state/omarchy/toggles/bar-off")
}

impl Bar {
  pub fn new(app: &gtk::Application, ipc: &Rc<crate::ipc::Ipc>) -> Rc<Bar> {
    let me = Rc::new(Bar {
      app: app.clone(),
      ipc: ipc.clone(),
      config: RefCell::new(config::load()),
      style: RefCell::new(doxia_theme::bar_style()),
      screens: RefCell::default(),
      hidden: Cell::new(off_flag().exists()),
      monitors: RefCell::default(),
    });
    if let Some(display) = gdk::Display::default() {
      let weak = Rc::downgrade(&me);
      display.monitors().connect_items_changed(move |_, _, _, _| {
        if let Some(me) = weak.upgrade() {
          me.build();
        }
      });
    }
    me.watch();
    me.build();
    me
  }

  pub fn ipc(self: &Rc<Self>, method: &str) -> Option<String> {
    match method {
      "syncHidden" => {
        self.sync_hidden();
        Some("ok".into())
      }
      "reservedTop" => {
        let cfg = self.config.borrow();
        let top = if self.hidden.get() || cfg.position != Position::Top { 0 } else { self.window_size() };
        Some(top.to_string())
      }
      _ => None,
    }
  }

  // An IPC target of a widget (its first screen's copy answers).
  pub fn add_ipc(&self, target: &str, handler: crate::ipc::Handler) {
    self.ipc.add(target, handler);
  }

  // The theme changed: sizes and colors.
  pub fn restyle(self: &Rc<Self>) {
    let style = doxia_theme::bar_style();
    if *self.style.borrow() != style {
      *self.style.borrow_mut() = style;
      self.build();
    } else {
      self.redraw();
    }
  }

  fn watch(self: &Rc<Self>) {
    let shell_json = gio::File::for_path(home().join(".config/omarchy/shell.json"));
    if let Ok(m) = shell_json.monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
      let weak = Rc::downgrade(self);
      m.connect_changed(move |_, _, _, event| {
        if !matches!(event, gio::FileMonitorEvent::ChangesDoneHint | gio::FileMonitorEvent::Created | gio::FileMonitorEvent::Renamed | gio::FileMonitorEvent::MovedIn) {
          return;
        }
        let Some(me) = weak.upgrade() else { return };
        let next = config::load();
        if *me.config.borrow() != next {
          *me.config.borrow_mut() = next;
          me.build();
        }
      });
      self.monitors.borrow_mut().push(m);
    }
    let toggles = gio::File::for_path(home().join(".local/state/omarchy/toggles"));
    if let Ok(m) = toggles.monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
      let weak = Rc::downgrade(self);
      m.connect_changed(move |_, _, _, _| {
        if let Some(me) = weak.upgrade() {
          me.sync_hidden();
        }
      });
      self.monitors.borrow_mut().push(m);
    }
  }

  fn sync_hidden(&self) {
    let hidden = off_flag().exists();
    if hidden != self.hidden.get() {
      self.hidden.set(hidden);
      for s in self.screens.borrow().iter() {
        self.place(&s.window);
      }
    }
  }

  fn floating_gap(&self) -> i32 {
    if self.config.borrow().floating { self.style.borrow().space(4.0) } else { 0 }
  }

  fn bar_size(&self) -> i32 {
    let style = self.style.borrow();
    if self.config.borrow().position.vertical() { style.size_vertical } else { style.size_horizontal }
  }

  // The layer surface: the bar plus, when floating, the gap to the edge.
  fn window_size(&self) -> i32 {
    self.bar_size() + self.floating_gap()
  }

  fn redraw(&self) {
    for s in self.screens.borrow().iter() {
      s.window.queue_draw();
      if let Some(child) = s.window.child() {
        child.queue_draw();
      }
    }
  }

  // All the bars again: on start, a screen change, a new layout or look.
  fn build(self: &Rc<Self>) {
    for s in self.screens.borrow_mut().drain(..) {
      s.window.destroy();
    }
    let Some(display) = gdk::Display::default() else { return };
    let monitors = display.monitors();
    for i in 0..monitors.n_items() {
      let Some(monitor) = monitors.item(i).and_downcast::<gdk::Monitor>() else { continue };
      let window = self.window(&monitor, i == 0);
      window.set_visible(true);
      self.screens.borrow_mut().push(Screen { window });
    }
  }

  fn place(&self, window: &gtk::Window) {
    let position = self.config.borrow().position;
    let size = self.window_size();
    let edge = position.edge();
    for e in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
      let along = if position.vertical() { matches!(e, Edge::Top | Edge::Bottom) } else { matches!(e, Edge::Left | Edge::Right) };
      window.set_anchor(e, e == edge || along);
      window.set_margin(e, 0);
    }
    if position.vertical() {
      window.set_default_size(size, -1);
      window.set_size_request(size, -1);
    } else {
      window.set_default_size(-1, size);
      window.set_size_request(-1, size);
    }
    if self.hidden.get() {
      // Parked just past the edge: showing again is only a margin change.
      window.set_margin(edge, -size);
      window.set_exclusive_zone(0);
    } else {
      window.set_exclusive_zone(size);
    }
  }

  fn window(self: &Rc<Self>, monitor: &gdk::Monitor, first_screen: bool) -> gtk::Window {
    let window = gtk::Window::builder().application(&self.app).decorated(false).build();
    window.add_css_class("doxia");
    window.add_css_class("bar-window");
    window.init_layer_shell();
    window.set_namespace(Some("doxia-bar"));
    window.set_layer(Layer::Top);
    window.set_keyboard_mode(KeyboardMode::None);
    window.set_monitor(Some(monitor));
    self.place(&window);

    let cfg = self.config.borrow().clone();
    let style = self.style.borrow().clone();
    let ctx = Rc::new(Ctx {
      bar: Rc::downgrade(self),
      monitor: monitor.connector().map(|c| c.to_string()).unwrap_or_default(),
      position: cfg.position,
      style: style.clone(),
      window: window.clone(),
      first_screen,
    });

    let overlay = gtk::Overlay::new();
    let plate = gtk::DrawingArea::new();
    let gap = self.floating_gap() as f64;
    let (floating, transparent, position) = (cfg.floating, cfg.transparent, cfg.position);
    let st = style.clone();
    plate.set_draw_func(move |_, cr, w, h| draw_plate(cr, w as f64, h as f64, &st, position, floating, transparent, gap));
    overlay.set_child(Some(&plate));

    let content = self.sections(&ctx, &cfg);
    let g = self.floating_gap();
    let inset = if floating { style.space(4.0) } else { style.space(8.0) };
    // The plate's inset from the screen edge and the ends, and the sections'
    // from the plate's ends.
    let (edge_margin, end_margin) = (g, g);
    match position {
      Position::Top => content.set_margin_top(edge_margin),
      Position::Bottom => content.set_margin_bottom(edge_margin),
      Position::Left => content.set_margin_start(edge_margin),
      Position::Right => content.set_margin_end(edge_margin),
    }
    if position.vertical() {
      content.set_margin_top(end_margin + inset);
      content.set_margin_bottom(end_margin + inset);
    } else {
      content.set_margin_start(end_margin + inset);
      content.set_margin_end(end_margin + inset);
    }
    overlay.add_overlay(&content);
    window.set_child(Some(&overlay));
    window
  }

  // Left and right hug the ends; the center sits on the middle (on its
  // anchor, when one is set), under them if they grow that far.
  fn sections(self: &Rc<Self>, ctx: &Rc<Ctx>, cfg: &BarConfig) -> gtk::Widget {
    let orientation = if ctx.vertical() { gtk::Orientation::Vertical } else { gtk::Orientation::Horizontal };
    let list = |entries: &[Value], align: gtk::Align| {
      let b = gtk::Box::new(orientation, 0);
      for entry in entries {
        if let Some(w) = widgets::build(ctx, entry) {
          b.append(&w);
        }
      }
      if ctx.vertical() { b.set_valign(align) } else { b.set_halign(align) }
      b
    };
    let overlay = gtk::Overlay::new();
    let center = gtk::CenterBox::new();
    center.set_orientation(orientation);
    let anchor = cfg.layout.center.iter().position(|e| config::entry_id(e) == cfg.center_anchor);
    match anchor {
      Some(i) => {
        center.set_start_widget(Some(&list(&cfg.layout.center[..i], gtk::Align::End)));
        if let Some(w) = widgets::build(ctx, &cfg.layout.center[i]) {
          center.set_center_widget(Some(&w));
        }
        center.set_end_widget(Some(&list(&cfg.layout.center[i + 1..], gtk::Align::Start)));
      }
      None => center.set_center_widget(Some(&list(&cfg.layout.center, gtk::Align::Center))),
    }
    overlay.set_child(Some(&center));
    let left = list(&cfg.layout.left, gtk::Align::Start);
    let right = list(&cfg.layout.right, gtk::Align::End);
    overlay.add_overlay(&left);
    overlay.add_overlay(&right);
    overlay.upcast()
  }
}

// The bar's fill: the whole surface, or the floating plate off the screen
// edge and both ends by the gap, rounded like the windows, with a faint
// edge. Transparent: nothing.
#[allow(clippy::too_many_arguments)]
fn draw_plate(cr: &cairo::Context, w: f64, h: f64, style: &BarStyle, position: Position, floating: bool, transparent: bool, gap: f64) {
  if transparent {
    return;
  }
  let [r, g, b, a] = style.background;
  if !floating {
    cr.set_source_rgba(r, g, b, a);
    let _ = cr.paint();
    return;
  }
  let (x, y, pw, ph) = match position {
    Position::Top => (gap, gap, w - 2.0 * gap, h - gap),
    Position::Bottom => (gap, 0.0, w - 2.0 * gap, h - gap),
    Position::Left => (gap, gap, w - gap, h - 2.0 * gap),
    Position::Right => (0.0, gap, w - gap, h - 2.0 * gap),
  };
  let radius = style.radius.min(pw.min(ph) / 2.0);
  widget::rounded(cr, x, y, pw, ph, radius);
  cr.set_source_rgba(r, g, b, a);
  let _ = cr.fill_preserve();
  let [tr, tg, tb, _] = style.text;
  cr.set_source_rgba(tr, tg, tb, 0.1);
  cr.set_line_width(1.0);
  let _ = cr.stroke();
}
