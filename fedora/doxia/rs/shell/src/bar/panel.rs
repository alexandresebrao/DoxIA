// A bar widget's panel, like shell/Ui/KeyboardPanel.qml: a card in the
// [popups] colors on a full-screen overlay of the bar's screen, off the bar
// by the outer gap and under its widget (or centered on the bar), held
// inside the screen. A click anywhere else on that screen closes it; the bar
// strip is left out of the overlay's input, so a click on another widget
// reaches the bar and opens that one instead. Keys go to the panel: the
// overlay takes the keyboard for a moment when it opens (so a panel summoned
// by a key binding gets it), then only on demand.
//
// Opening one closes any other, and the widget's slot shows the open mark
// while it is up. A group of panels (omarchy.group) shares one card, with a
// tab strip over the content.

use super::config::Position;
use super::widget::Slot;
use super::Ctx;
use gtk::prelude::*;
use gtk::{cairo, gdk, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

thread_local! {
  static OPEN: RefCell<Weak<Panel>> = const { RefCell::new(Weak::new()) };
}

pub struct Panel {
  window: gtk::Window,
  fixed: gtk::Fixed,
  frame: gtk::Overlay,
  card: crate::card::Surface,
  // Where the panel's own widgets go, inside the card's padding.
  pub content: gtk::Box,
  // A group's tab strip, over the content.
  pub tabs: gtk::Box,
  anchor: glib::WeakRef<gtk::Widget>,
  bar_window: glib::WeakRef<gtk::Window>,
  slot: RefCell<Weak<Slot>>,
  position: Position,
  center_on_bar: bool,
  gap: i32,
  width: Cell<i32>,
  max_height: Cell<i32>,
  opened: Cell<bool>,
  on_open: RefCell<Vec<Box<dyn Fn()>>>,
  on_close: RefCell<Vec<Box<dyn Fn()>>>,
  on_key: RefCell<Option<Box<dyn Fn(gdk::Key, gdk::ModifierType) -> bool>>>,
  on_tab: RefCell<Option<Box<dyn Fn(bool)>>>,
  bar: RefCell<Weak<super::Bar>>,
}

impl Panel {
  // `anchor` is the widget it opens under; `width` its content width.
  pub fn new(ctx: &Ctx, anchor: &impl IsA<gtk::Widget>, slot: &Rc<Slot>, width: i32) -> Rc<Panel> {
    Panel::build(ctx, anchor, slot, width, false)
  }

  // A panel centered on the bar rather than under its widget (the calendar).
  pub fn centered(ctx: &Ctx, anchor: &impl IsA<gtk::Widget>, slot: &Rc<Slot>, width: i32) -> Rc<Panel> {
    Panel::build(ctx, anchor, slot, width, true)
  }

  fn build(ctx: &Ctx, anchor: &impl IsA<gtk::Widget>, slot: &Rc<Slot>, width: i32, center_on_bar: bool) -> Rc<Panel> {
    let app = ctx.window.application();
    let window = gtk::Window::builder().decorated(false).build();
    if let Some(app) = app {
      window.set_application(Some(&app));
    }
    window.add_css_class("doxia");
    window.add_css_class("panel-window");
    window.init_layer_shell();
    window.set_namespace(Some("doxia-panel"));
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::None);
    if let Some(monitor) = ctx.window.surface().and_then(|s| gdk::Display::default()?.monitor_at_surface(&s)) {
      window.set_monitor(Some(&monitor));
    }
    for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
      window.set_anchor(edge, true);
    }
    window.set_exclusive_zone(-1);

    let fixed = gtk::Fixed::new();
    let card_style = doxia_theme::surface_card("popups");
    let pad = (14.0 * ctx.style.spacing).round() as i32 + card_style.border_width.ceil() as i32;
    let card = crate::card::Surface::new(card_style);
    let frame = gtk::Overlay::new();
    frame.add_css_class("panel-card");
    frame.set_child(Some(&card.area));
    let inner = gtk::Box::new(gtk::Orientation::Vertical, 0);
    inner.set_margin_top(pad);
    inner.set_margin_bottom(pad);
    inner.set_margin_start(pad);
    inner.set_margin_end(pad);
    let tabs = gtk::Box::new(gtk::Orientation::Horizontal, ctx.style.space(4.0));
    tabs.add_css_class("panel-tabs");
    tabs.set_homogeneous(true);
    tabs.set_margin_bottom(ctx.style.space(12.0));
    tabs.set_visible(false);
    inner.append(&tabs);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.add_css_class("panel-content");
    inner.append(&content);
    frame.add_overlay(&inner);
    frame.set_measure_overlay(&inner, true);
    fixed.put(&frame, 0.0, 0.0);
    window.set_child(Some(&fixed));

    let me = Rc::new(Panel {
      window,
      fixed,
      frame,
      card,
      content,
      tabs,
      anchor: anchor.as_ref().downgrade(),
      bar_window: ctx.window.downgrade(),
      slot: RefCell::new(Rc::downgrade(slot)),
      position: ctx.position,
      center_on_bar,
      gap: ctx.style.gaps_out,
      width: Cell::new(width),
      max_height: Cell::new(0),
      opened: Cell::new(false),
      on_open: RefCell::default(),
      on_close: RefCell::default(),
      on_key: RefCell::default(),
      on_tab: RefCell::default(),
      bar: RefCell::new(Weak::new()),
    });
    me.connect_input();
    // Listed for the keys and IPC that open panels.
    if let (Some((id, region, order)), Some(bar)) = (ctx.current.borrow().clone(), ctx.bar.upgrade()) {
      let anchor_w = anchor.as_ref().downgrade();
      bar.register(super::Handle {
        id,
        region,
        order,
        monitor: ctx.monitor.clone(),
        panel: Rc::downgrade(&me),
        visible: Rc::new(move || anchor_w.upgrade().is_some_and(|a| a.is_visible())),
      });
    }
    *me.bar.borrow_mut() = ctx.bar.clone();
    // Gone with the bar it belongs to (rebuilt on a new layout or theme).
    let w = me.window.downgrade();
    ctx.window.connect_destroy(move |_| {
      if let Some(w) = w.upgrade() {
        w.destroy();
      }
    });
    me
  }

  fn connect_input(self: &Rc<Self>) {
    // A click off the card closes.
    let click = gtk::GestureClick::new();
    click.set_button(0);
    let weak = Rc::downgrade(self);
    click.connect_pressed(move |g, _, x, y| {
      let Some(me) = weak.upgrade() else { return };
      let inside = me.frame.compute_bounds(&me.fixed).is_some_and(|b| b.contains_point(&gtk::graphene::Point::new(x as f32, y as f32)));
      if !inside {
        g.set_state(gtk::EventSequenceState::Claimed);
        me.close();
      }
    });
    self.fixed.add_controller(click);

    let keys = gtk::EventControllerKey::new();
    let weak = Rc::downgrade(self);
    keys.connect_key_pressed(move |_, key, _, state| {
      let Some(me) = weak.upgrade() else { return glib::Propagation::Proceed };
      if matches!(key, gdk::Key::Tab | gdk::Key::ISO_Left_Tab) {
        let back = key == gdk::Key::ISO_Left_Tab || state.contains(gdk::ModifierType::SHIFT_MASK);
        if let Some(f) = me.on_tab.borrow().as_ref() {
          f(back);
          return glib::Propagation::Stop;
        }
        // Alone: the next panel on the bar.
        if let Some(bar) = me.bar.borrow().upgrade() {
          if bar.switch_from(&me, back) {
            return glib::Propagation::Stop;
          }
        }
      }
      if let Some(f) = me.on_key.borrow().as_ref() {
        if f(key, state) {
          return glib::Propagation::Stop;
        }
      }
      if key == gdk::Key::Escape {
        me.close();
        return glib::Propagation::Stop;
      }
      glib::Propagation::Proceed
    });
    self.window.add_controller(keys);

    // Placed once its size is known, and again when it changes.
    let weak = Rc::downgrade(self);
    self.window.connect_map(move |_| {
      if let Some(me) = weak.upgrade() {
        me.limit_input();
        me.place();
      }
    });
  }

  // The panel's keys first (true: used); Escape closes otherwise.
  pub fn connect_key(&self, f: impl Fn(gdk::Key, gdk::ModifierType) -> bool + 'static) {
    *self.on_key.borrow_mut() = Some(Box::new(f));
  }

  // Tab and Shift+Tab (true): a group's tabs.
  pub fn connect_tab(&self, f: impl Fn(bool) + 'static) {
    *self.on_tab.borrow_mut() = Some(Box::new(f));
  }

  // Before every opening (to refresh what it shows).
  pub fn connect_open(&self, f: impl Fn() + 'static) {
    self.on_open.borrow_mut().push(Box::new(f));
  }

  pub fn connect_close(&self, f: impl Fn() + 'static) {
    self.on_close.borrow_mut().push(Box::new(f));
  }

  // A group's tabs move the mark to the group's slot.
  pub fn set_slot(&self, slot: &Rc<Slot>) {
    *self.slot.borrow_mut() = Rc::downgrade(slot);
  }

  // A group's panels open under the group's icon.
  pub fn set_anchor(&self, anchor: &impl IsA<gtk::Widget>) {
    self.anchor.set(Some(anchor.as_ref()));
  }

  pub fn set_width(&self, width: i32) {
    self.width.set(width);
  }

  pub fn is_open(&self) -> bool {
    self.opened.get()
  }

  pub fn open(self: &Rc<Self>) {
    if self.opened.get() {
      return;
    }
    if let Some(other) = OPEN.with(|o| o.borrow().upgrade()) {
      if !Rc::ptr_eq(&other, self) {
        other.close();
      }
    }
    OPEN.with(|o| *o.borrow_mut() = Rc::downgrade(self));
    self.opened.set(true);
    self.card.set(doxia_theme::surface_card("popups"));
    for f in self.on_open.borrow().iter() {
      f();
    }
    if let Some(slot) = self.slot.borrow().upgrade() {
      slot.set_open(true);
    }
    // The keyboard for a moment, then on demand (KeyboardPanel's prime).
    self.window.set_keyboard_mode(KeyboardMode::Exclusive);
    self.window.set_visible(true);
    self.place();
    let weak = Rc::downgrade(self);
    glib::timeout_add_local_once(Duration::from_millis(75), move || {
      if let Some(me) = weak.upgrade().filter(|m| m.opened.get()) {
        me.window.set_keyboard_mode(KeyboardMode::OnDemand);
      }
    });
  }

  pub fn close(&self) {
    if !self.opened.get() {
      return;
    }
    self.opened.set(false);
    self.window.set_keyboard_mode(KeyboardMode::None);
    self.window.set_visible(false);
    if let Some(slot) = self.slot.borrow().upgrade() {
      slot.set_open(false);
    }
    for f in self.on_close.borrow().iter() {
      f();
    }
  }

  pub fn toggle(self: &Rc<Self>) {
    if self.opened.get() { self.close() } else { self.open() }
  }

  // The card's size changed (content shown or hidden): place it again.
  pub fn relayout(self: &Rc<Self>) {
    if self.opened.get() {
      let weak = Rc::downgrade(self);
      glib::idle_add_local_once(move || {
        if let Some(me) = weak.upgrade() {
          me.place();
        }
      });
    }
  }

  fn screen_size(&self) -> (i32, i32) {
    let monitor = self.bar_window.upgrade().and_then(|w| w.surface()).and_then(|s| gdk::Display::default()?.monitor_at_surface(&s));
    monitor.map(|m| (m.geometry().width(), m.geometry().height())).unwrap_or((1280, 800))
  }

  fn bar_extent(&self) -> (i32, i32) {
    self.bar_window.upgrade().map(|w| (w.width(), w.height())).unwrap_or((0, 0))
  }

  // How tall the card may be: the screen less the bar, the gap and a margin.
  pub fn available_height(&self) -> i32 {
    let (sw, sh) = self.screen_size();
    let _ = sw;
    let (_, bh) = self.bar_extent();
    let margin = self.gap;
    let used = if self.position.vertical() { margin * 2 } else { bh + self.gap + margin };
    (sh - used).max(120)
  }

  fn place(&self) {
    let (sw, sh) = self.screen_size();
    let (bw, bh) = self.bar_extent();
    // At least the panel's width, more when its content asks for it.
    self.frame.set_size_request(self.width.get().min(sw - 2 * self.gap).max(1), -1);
    let (_, natural_w, _, _) = self.frame.measure(gtk::Orientation::Horizontal, -1);
    let width = natural_w.min(sw - 2 * self.gap).max(1);
    let (_, natural, _, _) = self.frame.measure(gtk::Orientation::Vertical, width);
    let height = natural.min(self.available_height());
    self.max_height.set(height);
    // Fixed gives its children their minimum size, which for a scrolled
    // body is next to nothing: ask for the measured one.
    self.frame.set_size_request(width, height);
    let (ax, ay, aw, ah) = self
      .anchor
      .upgrade()
      .zip(self.bar_window.upgrade())
      .and_then(|(a, w)| a.compute_bounds(&w))
      .map(|b| (b.x() as i32, b.y() as i32, b.width() as i32, b.height() as i32))
      .unwrap_or((0, 0, 0, 0));
    let gap = self.gap;
    let (mut x, mut y) = match (self.position, self.center_on_bar) {
      (Position::Top, true) => (sw / 2 - width / 2, bh + gap),
      (Position::Bottom, true) => (sw / 2 - width / 2, sh - bh - height - gap),
      (Position::Left, true) => (bw + gap, sh / 2 - height / 2),
      (Position::Right, true) => (sw - bw - width - gap, sh / 2 - height / 2),
      (Position::Top, false) => (ax + aw / 2 - width / 2, bh + gap),
      (Position::Bottom, false) => (ax + aw / 2 - width / 2, sh - bh - height - gap),
      (Position::Left, false) => (bw + gap, ay + ah / 2 - height / 2),
      (Position::Right, false) => (sw - bw - width - gap, ay + ah / 2 - height / 2),
    };
    x = x.clamp(gap, (sw - width - gap).max(gap));
    y = y.clamp(gap, (sh - height - gap).max(gap));
    self.fixed.move_(&self.frame, x as f64, y as f64);
  }

  // The bar strip takes its own clicks.
  fn limit_input(&self) {
    let Some(surface) = self.window.surface() else { return };
    let (sw, sh) = self.screen_size();
    let (bw, bh) = self.bar_extent();
    let rect = match self.position {
      Position::Top => cairo::RectangleInt::new(0, bh, sw, sh - bh),
      Position::Bottom => cairo::RectangleInt::new(0, 0, sw, sh - bh),
      Position::Left => cairo::RectangleInt::new(bw, 0, sw - bw, sh),
      Position::Right => cairo::RectangleInt::new(0, 0, sw - bw, sh),
    };
    surface.set_input_region(Some(&cairo::Region::create_rectangle(&rect)));
  }
}
