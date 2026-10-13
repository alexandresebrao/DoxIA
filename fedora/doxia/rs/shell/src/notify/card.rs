// One notification card, laid out like components/NotificationCard.qml: a
// 40px icon (the image, else the app icon, else the glyph), the summary (two
// lines at most) and the body (three), 380px wide. Left click acts, right
// click dismisses (wired by the caller).

use super::markup;
use super::model::{self, Entry, CRITICAL};
use gtk::prelude::*;

fn icon_widget(value: &str) -> Option<gtk::Image> {
  if value.is_empty() || value.starts_with("image://") {
    return None;
  }
  let path = model::local_image_file(value);
  if !path.is_empty() {
    return std::path::Path::new(&path).is_file().then(|| gtk::Image::from_file(&path));
  }
  let display = gtk::gdk::Display::default()?;
  gtk::IconTheme::for_display(&display).has_icon(value).then(|| gtk::Image::from_icon_name(value))
}

pub fn build(entry: &Entry) -> gtk::Box {
  let body = markup::body(&entry.body, &entry.app, &entry.app_icon);
  let single_line = body.empty;
  let source = if entry.image.is_empty() { entry.app_icon.as_str() } else { entry.image.as_str() };
  let image = icon_widget(source);
  let has_glyph = !entry.glyph.is_empty();
  // A glyph-only one-liner draws the glyph inline, small.
  let compact_glyph = has_glyph && image.is_none() && single_line;
  // "X  text" summaries already carry their icon.
  let redundant = single_line && !has_glyph && model::summary_starts_with_glyph(&entry.summary);

  let card = gtk::Box::new(gtk::Orientation::Horizontal, 0);
  card.add_css_class("notif-card");
  if entry.urgency == CRITICAL {
    card.add_css_class("critical");
  }
  card.set_size_request(380, -1);
  card.set_halign(gtk::Align::End);
  card.set_cursor_from_name(Some("pointer"));

  let row = gtk::Box::new(gtk::Orientation::Horizontal, if redundant { 0 } else if compact_glyph { 8 } else { 12 });
  row.set_margin_start(12);
  row.set_margin_end(12);
  let v = if single_line { 7 } else { 10 };
  row.set_margin_top(v);
  row.set_margin_bottom(v);
  row.set_hexpand(true);

  if !redundant && !compact_glyph && (image.is_some() || has_glyph) {
    let slot: gtk::Widget = match image {
      Some(img) => {
        img.set_pixel_size(40);
        img.upcast()
      }
      None => {
        let g = gtk::Label::new(Some(&entry.glyph));
        g.add_css_class("notif-glyph");
        g.upcast()
      }
    };
    slot.set_size_request(40, 40);
    slot.set_valign(gtk::Align::Center);
    row.append(&slot);
  }
  if compact_glyph {
    let g = gtk::Label::new(Some(&entry.glyph));
    g.add_css_class("notif-glyph-small");
    g.set_valign(gtk::Align::Center);
    row.append(&g);
  }

  let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
  text.set_hexpand(true);
  text.set_valign(gtk::Align::Center);
  if !entry.summary.is_empty() {
    let s = gtk::Label::new(Some(&entry.summary));
    s.add_css_class("notif-summary");
    s.set_xalign(0.0);
    s.set_wrap(true);
    s.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    s.set_lines(2);
    s.set_ellipsize(gtk::pango::EllipsizeMode::End);
    s.set_max_width_chars(1);
    s.set_hexpand(true);
    text.append(&s);
  }
  if !single_line {
    let b = gtk::Label::new(None);
    b.set_markup(&body.markup);
    b.add_css_class("notif-body");
    b.set_xalign(0.0);
    b.set_wrap(true);
    b.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    b.set_lines(3);
    b.set_ellipsize(gtk::pango::EllipsizeMode::End);
    b.set_max_width_chars(1);
    b.set_hexpand(true);
    b.set_margin_top(2);
    // Links open in the browser; the card's own click stays for the rest.
    text.append(&b);
  }
  row.append(&text);
  card.append(&row);
  card
}
