// The DoxIA login look, shared by the login screen (doxia-greeter, under
// greetd) and the lock screen (doxia-shell's lock/), so the two stay one
// screen: the animated landscape (landscape.rs), the brand and a large clock
// on the left, the machine's name at the bottom, and the sign-in floating in
// the lower third with no card: the avatar, the name and the password on a
// thin line. Its own palette (login.css), whatever the theme.

pub mod landscape;

use gtk::prelude::*;
use gtk::{gdk, glib};
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub const CSS: &str = include_str!("login.css");
pub const PLACEHOLDER: &str = "Senha";
pub const FAILED: &str = "Senha incorreta. Tente novamente.";
const G_FINGERPRINT: &str = "\u{f0237}";
// Spelled out ("sexta-feira"): GLib's %A says "sexta".
const WEEKDAYS: [&str; 7] = ["segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado", "domingo"];

// Above the theme's CSS (doxia-theme installs it at USER priority).
pub fn install_css() {
  let Some(display) = gdk::Display::default() else { return };
  let provider = gtk::CssProvider::new();
  provider.load_from_string(CSS);
  gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_USER + 1);
}

pub fn label(text: &str, classes: &[&str]) -> gtk::Label {
  let l = gtk::Label::new(Some(text));
  for c in classes {
    l.add_css_class(c);
  }
  l
}

// The avatar: the account's picture in a soft ring, or its initial in a dark
// disc when there is none.
pub fn avatar(initial: &str, face: Option<&Path>) -> gtk::DrawingArea {
  let area = gtk::DrawingArea::new();
  area.set_content_width(98);
  area.set_content_height(98);
  area.set_halign(gtk::Align::Center);
  set_avatar(&area, initial, face);
  area
}

pub fn set_avatar(area: &gtk::DrawingArea, initial: &str, face: Option<&Path>) {
  let initial = initial.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
  let face = face.and_then(|p| gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(p, 176, 176, false).ok());
  area.set_draw_func(move |_, cr, w, h| {
    let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0);
    cr.arc(cx, cy, 48.0, 0.0, 2.0 * std::f64::consts::PI);
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.5);
    cr.set_line_width(2.0);
    let _ = cr.stroke();
    cr.arc(cx, cy, 44.0, 0.0, 2.0 * std::f64::consts::PI);
    if let Some(pixbuf) = &face {
      let _ = cr.save();
      cr.clip();
      cr.translate(cx - 44.0, cy - 44.0);
      cr.scale(88.0 / pixbuf.width() as f64, 88.0 / pixbuf.height() as f64);
      cr.set_source_pixbuf(pixbuf, 0.0, 0.0);
      let _ = cr.paint();
      let _ = cr.restore();
      return;
    }
    cr.set_source_rgba(0.1, 0.11, 0.13, 0.7);
    let _ = cr.fill();
    let layout = pangocairo::functions::create_layout(cr);
    let mut font = gtk::pango::FontDescription::from_string("Red Hat Text Bold");
    font.set_absolute_size(88.0 * 0.42 * gtk::pango::SCALE as f64);
    layout.set_font_description(Some(&font));
    layout.set_text(&initial);
    let (tw, th) = layout.pixel_size();
    cr.move_to(cx - tw as f64 / 2.0, cy - th as f64 / 2.0);
    cr.set_source_rgb(0.95, 0.95, 0.95);
    pangocairo::functions::show_layout(cr, &layout);
  });
  area.queue_draw();
}

// The landscape, redrawn at up to 30 frames a second (none while paused);
// the weather is asked for once a second.
pub fn landscape_area(sky: impl Fn() -> Option<landscape::Sky> + 'static, paused: impl Fn() -> bool + 'static) -> gtk::DrawingArea {
  let area = gtk::DrawingArea::new();
  area.set_hexpand(true);
  area.set_vexpand(true);
  let started = Instant::now();
  let current = Rc::new(std::cell::Cell::new(landscape::Sky::default()));
  let c = current.clone();
  area.set_draw_func(move |_, cr, w, h| {
    let now = glib::DateTime::now_local().ok();
    let hour = now.map(|d| d.hour() as f64 + d.minute() as f64 / 60.0 + d.seconds() / 3600.0).unwrap_or(12.0);
    let scene = landscape::Scene::new(w as f64, h as f64, started.elapsed().as_secs_f64(), hour, c.get());
    landscape::draw(cr, &scene);
  });
  let last = std::cell::Cell::new(0i64);
  area.add_tick_callback(move |a, clock| {
    let now = clock.frame_time();
    if now - last.get() >= 33_000 && !paused() {
      last.set(now);
      a.queue_draw();
    }
    glib::ControlFlow::Continue
  });
  let weak = area.downgrade();
  let read = move || {
    if let Some(found) = sky() {
      current.set(found);
    }
  };
  read();
  glib::timeout_add_seconds_local(1, move || {
    if weak.upgrade().is_none() {
      return glib::ControlFlow::Break;
    }
    read();
    glib::ControlFlow::Continue
  });
  area
}

// One monitor's screen without the sign-in: the landscape, the brand, the
// clock and the date, the machine's name. The sign-in goes on `overlay`.
pub struct Backdrop {
  pub overlay: gtk::Overlay,
  // The left margin everything on the left lines up on, and the height.
  pub left: i32,
  pub height: f64,
}

pub fn backdrop(monitor: Option<&gdk::Monitor>, brand: &Path, area: gtk::DrawingArea) -> Backdrop {
  let geometry = monitor.map(|m| m.geometry());
  let (mw, mh) = geometry.map(|g| (g.width() as f64, g.height() as f64)).unwrap_or((1366.0, 768.0));
  let left = (mw * 0.07).round() as i32;

  let overlay = gtk::Overlay::new();
  overlay.set_child(Some(&area));

  // Top left: the brand, small.
  let scale = monitor.map(|m| m.scale_factor()).unwrap_or(1).max(1);
  let picture = gtk::Picture::new();
  if let Ok(pixbuf) = gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(brand, -1, 26 * scale, true) {
    picture.set_paintable(Some(&gdk::Texture::for_pixbuf(&pixbuf)));
    picture.set_size_request(pixbuf.width() / scale, 26);
  }
  picture.set_can_shrink(true);
  picture.set_content_fit(gtk::ContentFit::Contain);
  picture.set_opacity(0.85);
  picture.set_halign(gtk::Align::Start);
  picture.set_valign(gtk::Align::Start);
  picture.set_margin_start(left);
  picture.set_margin_top(48);
  picture.set_can_target(false);
  overlay.add_overlay(&picture);

  // Left: the clock and the date.
  let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
  column.set_halign(gtk::Align::Start);
  column.set_valign(gtk::Align::Start);
  column.set_margin_start(left);
  column.set_margin_top((mh * 0.3).round() as i32);
  column.set_can_target(false);
  let clock = label("", &["login-clock"]);
  clock.set_xalign(0.0);
  let attrs = gtk::pango::AttrList::new();
  attrs.insert(gtk::pango::AttrSize::new_size_absolute((mh * 0.15).round() as i32 * gtk::pango::SCALE));
  clock.set_attributes(Some(&attrs));
  column.append(&clock);
  let bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
  bar.add_css_class("login-accent");
  bar.set_size_request(56, 4);
  bar.set_halign(gtk::Align::Start);
  column.append(&bar);
  let date = label("", &["login-date"]);
  date.set_xalign(0.0);
  date.set_margin_top(6);
  column.append(&date);
  overlay.add_overlay(&column);
  let (c, d) = (clock.downgrade(), date.downgrade());
  let tick = move || {
    let (Some(c), Some(d)) = (c.upgrade(), d.upgrade()) else { return glib::ControlFlow::Break };
    if let Ok(now) = glib::DateTime::now_local() {
      c.set_text(&now.format("%H:%M").map(|s| s.to_string()).unwrap_or_default());
      let weekday = WEEKDAYS[(now.day_of_week() as usize).clamp(1, 7) - 1];
      d.set_text(&format!("{weekday}, {}", now.format("%-d de %B").map(|s| s.to_string()).unwrap_or_default()));
    }
    glib::ControlFlow::Continue
  };
  tick();
  glib::timeout_add_seconds_local(1, tick);

  // Bottom left: the machine's name.
  let host = label(&glib::host_name(), &["login-host"]);
  host.set_halign(gtk::Align::Start);
  host.set_valign(gtk::Align::End);
  host.set_margin_start(left);
  host.set_margin_bottom(30);
  host.set_can_target(false);
  overlay.add_overlay(&host);

  Backdrop { overlay, left, height: mh }
}

// The sign-in's column, floating in the lower third.
pub fn sign_in(height: f64) -> gtk::Box {
  let column = gtk::Box::new(gtk::Orientation::Vertical, 14);
  column.set_size_request(380, -1);
  column.set_halign(gtk::Align::Center);
  column.set_valign(gtk::Align::End);
  column.set_margin_bottom((height * 0.1).round() as i32);
  column
}

// A field drawn as a thin line, like a game menu's: the line widens and
// brightens while the field has the focus, and turns red on a failure.
pub fn line(entry: &impl IsA<gtk::Widget>) -> (gtk::Overlay, gtk::Box) {
  let overlay = gtk::Overlay::new();
  let field = gtk::Box::new(gtk::Orientation::Horizontal, 0);
  field.add_css_class("login-line");
  entry.set_hexpand(true);
  entry.set_margin_start(44);
  entry.set_margin_end(44);
  field.append(entry);
  overlay.set_child(Some(&field));
  (overlay, field)
}

// The password on its line: the fingerprint at its left end (hidden until a
// finger is enrolled) and the submit button at its right end.
#[derive(Clone)]
pub struct PasswordLine {
  pub line: gtk::Overlay,
  pub field: gtk::Box,
  pub entry: gtk::PasswordEntry,
  pub hint: gtk::Label,
  pub submit: gtk::Button,
}

pub fn password_line() -> PasswordLine {
  let entry = gtk::PasswordEntry::new();
  entry.add_css_class("login-entry");
  entry.set_alignment(0.5);
  entry.set_placeholder_text(Some(PLACEHOLDER));
  let (line, field) = line(&entry);
  let hint = label(G_FINGERPRINT, &["login-fingerprint"]);
  hint.set_halign(gtk::Align::Start);
  hint.set_valign(gtk::Align::Center);
  hint.set_margin_start(8);
  hint.set_visible(false);
  line.add_overlay(&hint);
  let submit = gtk::Button::with_label("→");
  submit.add_css_class("login-submit");
  submit.set_halign(gtk::Align::End);
  submit.set_valign(gtk::Align::Center);
  submit.set_can_focus(false);
  line.add_overlay(&submit);
  PasswordLine { line, field, entry, hint, submit }
}

pub fn set_failed(field: &gtk::Box, failed: bool) {
  if failed {
    field.add_css_class("error");
  } else {
    field.remove_css_class("error");
  }
}

// A wrong password shakes the sign-in.
pub fn shake(boxes: Vec<glib::WeakRef<gtk::Box>>) {
  let steps = [(-10, 50u64), (10, 70), (-6, 60), (0, 50)];
  let mut at = 0;
  for (dx, ms) in steps {
    at += ms;
    let boxes = boxes.clone();
    glib::timeout_add_local_once(Duration::from_millis(at), move || {
      for b in boxes.iter().filter_map(|b| b.upgrade()) {
        b.set_margin_start((2 * dx).max(0));
        b.set_margin_end((-2 * dx).max(0));
      }
    });
  }
}
