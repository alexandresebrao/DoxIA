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
pub mod member;
pub mod panel;
pub mod ui;
pub mod widget;
mod widgets;

pub use widgets::weather_now;

use config::{BarConfig, Position};
use doxia_theme::BarStyle;
use gtk::prelude::*;
use gtk::{cairo, gdk, gio, glib};
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
  // The layout entry being built: (id, region, index). A panel made while
  // it is set is listed for the keys and IPC that open panels.
  pub current: RefCell<Option<(String, String, usize)>>,
}

// A panel on the bar, as the keys and IPC that open panels find it.
pub struct Handle {
  pub id: String,
  pub region: String,
  pub order: usize,
  pub monitor: String,
  pub panel: Weak<panel::Panel>,
  pub visible: Rc<dyn Fn() -> bool>,
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
  handles: RefCell<Vec<Handle>>,
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
      handles: RefCell::default(),
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

  pub fn register(&self, handle: Handle) {
    self.handles.borrow_mut().push(handle);
  }

  // The panels a key or IPC call can reach: alive and on screen, the copy
  // on the focused screen first.
  fn live(&self) -> Vec<(String, String, usize, String, Rc<panel::Panel>)> {
    let focused = crate::mango::get().focused_monitor();
    let mut out: Vec<_> = self
      .handles
      .borrow()
      .iter()
      .filter(|h| (h.visible)())
      .filter_map(|h| h.panel.upgrade().map(|p| (h.id.clone(), h.region.clone(), h.order, h.monitor.clone(), p)))
      .collect();
    out.sort_by_key(|h| (h.3 != focused, h.3.clone(), h.2));
    out
  }

  // shell summon/toggle/hide <id> for a bar widget's panel.
  pub fn shell_call(&self, method: &str, id: &str) -> Option<String> {
    let (_, _, _, _, panel) = self.live().into_iter().find(|h| h.0 == id)?;
    match method {
      "summon" => panel.open(),
      "toggle" => panel.toggle(),
      "hide" => panel.close(),
      "call" => {}
      _ => return None,
    }
    Some("ok".into())
  }

  // The Nth panel (1-based) in a region of the focused screen's bar, a
  // group counting once: what the SUPER+CTRL+number keys open.
  pub fn toggle_panel_at(&self, region: &str, index: usize) -> String {
    let live = self.live();
    let Some(monitor) = live.first().map(|h| h.3.clone()) else { return "unknown".into() };
    let mut seen = Vec::new();
    let mut slots = Vec::new();
    for h in live.into_iter().filter(|h| h.1 == region && h.3 == monitor) {
      if !seen.contains(&h.2) {
        seen.push(h.2);
        slots.push(h);
      }
    }
    slots.sort_by_key(|h| h.2);
    match index.checked_sub(1).and_then(|i| slots.get(i)) {
      Some((id, _, _, _, panel)) => {
        panel.toggle();
        id.clone()
      }
      None => "unknown".into(),
    }
  }

  // Tab from an open panel: the next one in its region, same screen.
  pub fn switch_from(&self, from: &Rc<panel::Panel>, back: bool) -> bool {
    let live = self.live();
    let Some(me) = live.iter().find(|h| Rc::ptr_eq(&h.4, from)) else { return false };
    let (region, monitor, order) = (me.1.clone(), me.3.clone(), me.2);
    let mut seen = Vec::new();
    let mut slots = Vec::new();
    for h in live.iter().filter(|h| h.1 == region && h.3 == monitor) {
      if !seen.contains(&h.2) {
        seen.push(h.2);
        slots.push(h);
      }
    }
    slots.sort_by_key(|h| h.2);
    if slots.len() < 2 {
      return false;
    }
    let i = slots.iter().position(|h| h.2 == order).unwrap_or(0);
    let n = slots.len();
    let next = if back { (i + n - 1) % n } else { (i + 1) % n };
    slots[next].4.open();
    true
  }

  // An IPC target of a widget (its first screen's copy answers).
  pub fn add_ipc(&self, target: &str, handler: crate::ipc::Handler) {
    self.ipc.add(target, handler);
  }

  // The OSD, as Quickshell's summon("omarchy.osd") does for the wheel on the
  // volume and brightness icons.
  pub fn osd(&self, icon: &str, value: f64) {
    let payload = serde_json::json!({ "icon": icon, "value": value.round() as i64 }).to_string();
    self.ipc.invoke("osd", "show", &[payload]);
  }

  // The theme changed: sizes and colors.
  pub fn restyle(self: &Rc<Self>, css_changed: bool) {
    let style = doxia_theme::bar_style();
    if *self.style.borrow() != style {
      *self.style.borrow_mut() = style;
      self.crossfade();
    } else if css_changed {
      self.redraw();
    }
  }

  // A new look without the bar leaving the screen: the new bars come in
  // over the old ones as those fade out, in step with the background's band
  // (its length and easing), and only then take the exclusive zone.
  fn crossfade(self: &Rc<Self>) {
    if self.hidden.get() {
      self.build();
      return;
    }
    let old: Vec<gtk::Window> = self.screens.borrow_mut().drain(..).map(|s| s.window).collect();
    self.build();
    // The opacity goes on the contents: GTK leaves a toplevel's own alone.
    let fade = |w: &gtk::Window, o: f64| {
      if let Some(c) = w.child() {
        c.set_opacity(o);
      }
    };
    for s in self.screens.borrow().iter() {
      s.window.set_exclusive_zone(0);
      fade(&s.window, 0.0);
    }
    let fresh: Vec<gtk::Window> = self.screens.borrow().iter().map(|s| s.window.clone()).collect();
    // Building the widgets holds the main loop for a while: the clock starts
    // once the new bars are on screen, or the fade would be over before its
    // first step.
    let started: Cell<Option<std::time::Instant>> = Cell::new(None);
    let weak = Rc::downgrade(self);
    glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
      if started.get().is_none() {
        if !fresh.iter().all(|w| w.is_mapped()) {
          return glib::ControlFlow::Continue;
        }
        started.set(Some(std::time::Instant::now()));
      }
      let t = started.get().map_or(0.0, |s| s.elapsed().as_secs_f64()) / crate::background::REVEAL.as_secs_f64();
      let e = crate::background::ease(t);
      for w in &fresh {
        fade(w, e);
      }
      for w in &old {
        fade(w, 1.0 - e);
      }
      if t < 1.0 {
        return glib::ControlFlow::Continue;
      }
      for w in &old {
        w.destroy();
      }
      if let Some(me) = weak.upgrade() {
        for w in &fresh {
          me.place(w);
        }
      }
      glib::ControlFlow::Break
    });
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
        // Parked past the edge, the surface gets no frame callbacks, so the
        // margin back to 0 is never committed. Remapping commits it.
        if !hidden {
          s.window.set_visible(false);
        }
        self.place(&s.window);
        if !hidden {
          s.window.set_visible(true);
        }
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
    self.handles.borrow_mut().clear();
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
      current: RefCell::default(),
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
    let build_at = |entry: &Value, region: &str, index: usize| {
      *ctx.current.borrow_mut() = Some((config::entry_id(entry), region.to_string(), index));
      let w = widgets::build(ctx, entry);
      *ctx.current.borrow_mut() = None;
      w
    };
    let list = |entries: &[Value], align: gtk::Align, region: &str, offset: usize| {
      let b = gtk::Box::new(orientation, 0);
      for (i, entry) in entries.iter().enumerate() {
        if let Some(w) = build_at(entry, region, offset + i) {
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
        center.set_start_widget(Some(&list(&cfg.layout.center[..i], gtk::Align::End, "center", 0)));
        if let Some(w) = build_at(&cfg.layout.center[i], "center", i) {
          center.set_center_widget(Some(&w));
        }
        center.set_end_widget(Some(&list(&cfg.layout.center[i + 1..], gtk::Align::Start, "center", i + 1)));
      }
      None => center.set_center_widget(Some(&list(&cfg.layout.center, gtk::Align::Center, "center", 0))),
    }
    overlay.set_child(Some(&center));
    let left = list(&cfg.layout.left, gtk::Align::Start, "left", 0);
    let right = list(&cfg.layout.right, gtk::Align::End, "right", 0);
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
