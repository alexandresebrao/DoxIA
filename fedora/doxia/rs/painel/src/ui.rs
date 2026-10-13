// Building blocks for the pages, in the shell's look (doxia-theme's base.css
// plus style.css).

use gtk::prelude::*;
use gtk::{gio, glib};
use std::cell::RefCell;
use std::rc::Rc;

pub const G_BACK: &str = "\u{f053}";
pub const G_NEXT: &str = "\u{f054}";
pub const G_EXTERNAL: &str = "\u{f08e}";
pub const G_GLOBE: &str = "\u{f0ac}";
pub const G_REFRESH: &str = "\u{f021}";
pub const G_COPY: &str = "\u{f0c5}";
pub const G_PASTE: &str = "\u{f0ea}";
pub const G_MAGIC: &str = "\u{f0d0}";
pub const G_KEY: &str = "\u{f084}";
pub const G_WARN: &str = "\u{f071}";
pub const G_GRID: &str = "\u{f00a}";
pub const G_LIST: &str = "\u{f03a}";
pub const G_BOLT: &str = "\u{f0e7}";

pub fn label(text: &str, classes: &[&str]) -> gtk::Label {
  let l = gtk::Label::new(Some(text));
  l.set_xalign(0.0);
  for c in classes {
    l.add_css_class(c);
  }
  l
}

pub fn wrapped(text: &str, classes: &[&str]) -> gtk::Label {
  let l = label(text, classes);
  l.set_wrap(true);
  l.set_wrap_mode(gtk::pango::WrapMode::WordChar);
  l
}

pub fn vbox(spacing: i32) -> gtk::Box {
  gtk::Box::new(gtk::Orientation::Vertical, spacing)
}

pub fn hbox(spacing: i32) -> gtk::Box {
  gtk::Box::new(gtk::Orientation::Horizontal, spacing)
}

pub fn clear(b: &gtk::Box) {
  while let Some(child) = b.first_child() {
    b.remove(&child);
  }
}

pub fn on_click(widget: &impl IsA<gtk::Widget>, f: impl Fn() + 'static) {
  let click = gtk::GestureClick::new();
  click.connect_released(move |g, _, _, _| {
    g.set_state(gtk::EventSequenceState::Claimed);
    f();
  });
  widget.add_controller(click);
  widget.set_cursor_from_name(Some("pointer"));
}

pub fn button(text: &str, primary: bool) -> gtk::Button {
  let b = gtk::Button::with_label(text);
  b.add_css_class(if primary { "primary" } else { "ctl" });
  b
}

// A bordered button with a glyph before the text.
pub fn glyph_button(glyph: &str, text: &str) -> gtk::Button {
  let b = gtk::Button::new();
  b.add_css_class("ctl");
  let row = hbox(6);
  row.append(&label(glyph, &["glyph-body"]));
  row.append(&label(text, &[]));
  b.set_child(Some(&row));
  b
}

// Just a glyph, for the header.
pub fn icon_button(glyph: &str, tooltip: &str) -> gtk::Button {
  let b = gtk::Button::with_label(glyph);
  b.add_css_class("ctl");
  b.add_css_class("icon");
  b.set_tooltip_text(Some(tooltip));
  b.set_valign(gtk::Align::Center);
  b
}

pub fn open_url(url: &str) {
  let _ = gio::AppInfo::launch_default_for_uri(url, None::<&gio::AppLaunchContext>);
}

pub fn copy(text: &str) {
  if let Some(display) = gtk::gdk::Display::default() {
    display.clipboard().set_text(text);
  }
}

// Runs f on a worker thread and hands its result to done on the GTK thread.
pub fn background<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static, done: impl FnOnce(T) + 'static) {
  glib::spawn_future_local(async move {
    if let Ok(value) = gio::spawn_blocking(f).await {
      done(value);
    }
  });
}

// Header (back, title, subtitle, actions), a scrolled body and a footer.
pub struct Frame {
  pub root: gtk::Box,
  pub body: gtk::Box,
  pub footer: gtk::Box,
  pub status: gtk::Label,
  pub actions: gtk::Box,
  pub back: gtk::Button,
}

impl Frame {
  pub fn new(title: &str, subtitle: &str, glyph: &str) -> Frame {
    let root = vbox(0);
    let header = hbox(12);
    header.add_css_class("header");

    let back = icon_button(G_BACK, "Voltar (Esc)");
    header.append(&back);

    let icon = gtk::CenterBox::new();
    icon.add_css_class("header-icon");
    icon.set_valign(gtk::Align::Center);
    let g = label(glyph, &["glyph-heading", "c-accent"]);
    g.set_xalign(0.5);
    icon.set_center_widget(Some(&g));
    header.append(&icon);

    let titles = vbox(2);
    titles.set_hexpand(true);
    titles.set_valign(gtk::Align::Center);
    let t = label(title, &["title"]);
    t.set_ellipsize(gtk::pango::EllipsizeMode::End);
    titles.append(&t);
    if !subtitle.is_empty() {
      let s = label(subtitle, &["subtitle"]);
      s.set_ellipsize(gtk::pango::EllipsizeMode::End);
      titles.append(&s);
    }
    header.append(&titles);

    let actions = hbox(4);
    actions.set_valign(gtk::Align::Center);
    header.append(&actions);
    root.append(&header);

    let rule = hbox(0);
    rule.add_css_class("rule");
    root.append(&rule);

    let body = vbox(12);
    body.set_margin_start(18);
    body.set_margin_end(18);
    body.set_margin_top(16);
    body.set_margin_bottom(16);
    let scroller = gtk::ScrolledWindow::builder()
      .hscrollbar_policy(gtk::PolicyType::Never)
      .vexpand(true)
      .child(&body)
      .build();
    root.append(&scroller);

    let footer_rule = hbox(0);
    footer_rule.add_css_class("rule");
    root.append(&footer_rule);
    let footer = hbox(8);
    footer.add_css_class("footer");
    let status = wrapped("", &["footer-note"]);
    status.set_hexpand(true);
    status.set_valign(gtk::Align::Center);
    footer.append(&status);
    root.append(&footer);
    // Pages without footer buttons hide it.
    let fr = footer_rule.clone();
    footer.connect_visible_notify(move |f| fr.set_visible(f.is_visible()));

    Frame { root, body, footer, status, actions, back }
  }

  // Buttons and spinners on the right of the footer.
  pub fn add(&self, widget: &impl IsA<gtk::Widget>) {
    widget.set_valign(gtk::Align::Center);
    self.footer.append(widget);
  }

  pub fn set_status(&self, text: &str) {
    self.status.set_text(text);
  }
}

// One form field: a single line entry or a multi-line text box.
#[derive(Clone)]
pub enum Field {
  Line(gtk::Entry),
  Text(gtk::TextView),
}

impl Field {
  pub fn value(&self) -> String {
    match self {
      Field::Line(e) => e.text().trim().to_string(),
      Field::Text(v) => {
        let b = v.buffer();
        b.text(&b.start_iter(), &b.end_iter(), false).trim().to_string()
      }
    }
  }

  pub fn set_value(&self, text: &str) {
    match self {
      Field::Line(e) => e.set_text(text),
      Field::Text(v) => {
        let b = v.buffer();
        b.set_text(text);
        b.place_cursor(&b.start_iter());
      }
    }
  }

  pub fn grab_focus(&self) {
    match self {
      Field::Line(e) => e.grab_focus(),
      Field::Text(v) => v.grab_focus(),
    };
  }

  pub fn set_editable(&self, editable: bool) {
    match self {
      Field::Line(e) => e.set_editable(editable),
      Field::Text(v) => v.set_editable(editable),
    }
  }
}

// Caption above, the input below; the hint is the placeholder.
pub fn field(title: &str, hint: &str, multiline: bool, min_height: i32) -> (gtk::Box, Field) {
  let col = vbox(6);
  col.append(&label(title, &["caption"]));
  if multiline {
    let view = gtk::TextView::new();
    view.set_wrap_mode(gtk::WrapMode::WordChar);
    view.set_accepts_tab(false);
    view.set_top_margin(8);
    view.set_bottom_margin(8);
    view.set_left_margin(10);
    view.set_right_margin(10);
    view.add_css_class("field-text");
    let scroller = gtk::ScrolledWindow::builder()
      .hscrollbar_policy(gtk::PolicyType::Never)
      .min_content_height(min_height)
      .child(&view)
      .build();
    scroller.add_css_class("text-box");
    col.append(&scroller);
    if !hint.is_empty() {
      col.append(&wrapped(hint, &["hint"]));
    }
    (col, Field::Text(view))
  } else {
    let entry = gtk::Entry::new();
    entry.add_css_class("field");
    if !hint.is_empty() {
      entry.set_placeholder_text(Some(hint));
      entry.set_tooltip_text(Some(hint));
    }
    col.append(&entry);
    (col, Field::Line(entry))
  }
}

// Big glyph, title and description in the middle of a page.
pub fn status_page(glyph: &str, title: &str, description: &str) -> (gtk::Box, gtk::Label, gtk::Label, gtk::Label) {
  let col = vbox(8);
  col.add_css_class("status-page");
  col.set_valign(gtk::Align::Center);
  col.set_vexpand(true);
  let g = label(glyph, &["status-glyph", "c-dim"]);
  g.set_xalign(0.5);
  let t = label(title, &["status-title"]);
  t.set_xalign(0.5);
  t.set_wrap(true);
  t.set_justify(gtk::Justification::Center);
  let d = label(description, &["desc"]);
  d.set_xalign(0.5);
  d.set_wrap(true);
  d.set_justify(gtk::Justification::Center);
  d.set_use_markup(true);
  col.append(&g);
  col.append(&t);
  col.append(&d);
  (col, g, t, d)
}

// Short messages at the bottom of the window, with an optional action.
pub struct Toasts {
  revealer: gtk::Revealer,
  text: gtk::Label,
  action: gtk::Button,
  on_action: Rc<RefCell<Option<Box<dyn Fn()>>>>,
  serial: Rc<RefCell<u64>>,
}

impl Toasts {
  pub fn new(overlay: &gtk::Overlay) -> Toasts {
    let revealer = gtk::Revealer::new();
    revealer.set_transition_type(gtk::RevealerTransitionType::None);
    revealer.set_valign(gtk::Align::End);
    revealer.set_halign(gtk::Align::Center);
    revealer.set_margin_bottom(68);
    revealer.set_margin_start(18);
    revealer.set_margin_end(18);
    let row = hbox(12);
    row.add_css_class("toast");
    let text = wrapped("", &["toast-text"]);
    text.set_valign(gtk::Align::Center);
    row.append(&text);
    let action = button("", false);
    action.add_css_class("small");
    action.set_valign(gtk::Align::Center);
    row.append(&action);
    revealer.set_child(Some(&row));
    overlay.add_overlay(&revealer);

    let on_action: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::default();
    let cb = on_action.clone();
    let r = revealer.clone();
    action.connect_clicked(move |_| {
      if let Some(f) = cb.borrow().as_ref() {
        f();
      }
      r.set_reveal_child(false);
    });
    Toasts { revealer, text, action, on_action, serial: Rc::default() }
  }

  pub fn show(&self, text: &str, seconds: u32) {
    self.show_with(text, seconds, None, None::<fn()>);
  }

  pub fn show_with(&self, text: &str, seconds: u32, action: Option<&str>, f: Option<impl Fn() + 'static>) {
    self.text.set_text(text);
    self.action.set_visible(action.is_some());
    if let Some(a) = action {
      self.action.set_label(a);
    }
    *self.on_action.borrow_mut() = f.map(|f| Box::new(f) as Box<dyn Fn()>);
    self.revealer.set_reveal_child(true);
    let serial = {
      let mut s = self.serial.borrow_mut();
      *s += 1;
      *s
    };
    let current = self.serial.clone();
    let r = self.revealer.clone();
    glib::timeout_add_seconds_local_once(seconds, move || {
      if *current.borrow() == serial {
        r.set_reveal_child(false);
      }
    });
  }
}
