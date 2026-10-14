// A card drawn like the shell's BorderSurface: a rounded, translucent fill
// with the border inside its edge, flat or a gradient. GTK's border-image
// ignores border-radius, and a gradient background behind a translucent fill
// shows through it, so cairo draws both.

use doxia_theme::Card;
use gtk::cairo;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub fn rounded(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
  let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
  let pi = std::f64::consts::PI;
  cr.new_sub_path();
  cr.arc(x + w - r, y + r, r, -pi / 2.0, 0.0);
  cr.arc(x + w - r, y + h - r, r, 0.0, pi / 2.0);
  cr.arc(x + r, y + h - r, r, pi / 2.0, pi);
  cr.arc(x + r, y + r, r, pi, 1.5 * pi);
  cr.close_path();
}

// BorderGeometry.gradientEndpoints: the angle runs from the x axis with y
// pointing down, through the center, long enough to reach the corners.
fn gradient(w: f64, h: f64, card: &Card) -> cairo::LinearGradient {
  let rad = card.border.angle.to_radians();
  let (dx, dy) = (rad.cos(), rad.sin());
  let len = ((w * dx).abs() + (h * dy).abs()) / 2.0;
  let (cx, cy) = (w / 2.0, h / 2.0);
  let g = cairo::LinearGradient::new(cx - dx * len, cy - dy * len, cx + dx * len, cy + dy * len);
  let n = card.border.colors.len();
  for (i, c) in card.border.colors.iter().enumerate() {
    let at = if n <= 1 { i as f64 } else { i as f64 / (n - 1) as f64 };
    g.add_color_stop_rgba(at, c[0], c[1], c[2], c[3]);
  }
  if n == 1 {
    let c = card.border.colors[0];
    g.add_color_stop_rgba(1.0, c[0], c[1], c[2], c[3]);
  }
  g
}

pub fn draw(cr: &cairo::Context, w: f64, h: f64, card: &Card) {
  let [r, g, b, a] = card.fill;
  rounded(cr, 0.0, 0.0, w, h, card.radius);
  cr.set_source_rgba(r, g, b, a);
  let _ = cr.fill();
  let bw = card.border_width;
  if bw <= 0.0 || card.border.colors.is_empty() {
    return;
  }
  // The ring between the outer edge and the edge inset by the border width.
  cr.set_fill_rule(cairo::FillRule::EvenOdd);
  rounded(cr, 0.0, 0.0, w, h, card.radius);
  rounded(cr, bw, bw, w - 2.0 * bw, h - 2.0 * bw, (card.radius - bw).max(0.0));
  if cr.set_source(&gradient(w, h, card)).is_ok() {
    let _ = cr.fill();
  }
}

// The drawing under a card's content; `set` swaps the look (a theme switch).
pub struct Surface {
  pub area: gtk::DrawingArea,
  card: Rc<RefCell<Card>>,
}

impl Surface {
  pub fn new(card: Card) -> Surface {
    let area = gtk::DrawingArea::new();
    let card = Rc::new(RefCell::new(card));
    let c = card.clone();
    area.set_draw_func(move |_, cr, w, h| draw(cr, w as f64, h as f64, &c.borrow()));
    Surface { area, card }
  }

  pub fn set(&self, card: Card) {
    *self.card.borrow_mut() = card;
    self.area.queue_draw();
  }
}
