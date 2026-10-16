// The desktop background, replacing shell/plugins/background
// (Background.qml): the image ~/.local/state/omarchy/current/background
// points to, cropped to fill each screen on the background layer. A new one
// comes in through a slanted band that widens from the middle; a theme
// switch hands over its colors as the band starts, so the bar and the
// popups change with the picture. Double-click picks a background, a
// right double-click a theme.
//
// Parallax, on a tag laid out as mango's scroller: the picture zooms in
// ZOOM times and slides PARALLAX of the way the tiled windows scroll, easing
// over PAN like mango's own move animation, and stops at its edge, so a long
// strip leaves it there. Any other layout eases it back to the plain picture. While three fingers swipe on the
// touchpad (touchpad.rs) it follows them, LIVE of the screen's width per
// width of pad, until mango moves the windows.
//
// IPC target "background": refresh, set <path>, setInstant <path>,
// transition <from> <path>, themeTransition <from> <path> <final> <colors>
// <shell> (bin/omarchy-theme-set, bin/omarchy-theme-bg-set).

use gtk::gdk_pixbuf::Pixbuf;
use gtk::prelude::*;
use gtk::{cairo, gdk, gio, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

pub const REVEAL: Duration = Duration::from_millis(420);
const SLANT: f64 = -0.18;
const THEME_FALLBACK: Duration = Duration::from_millis(300);
const PARALLAX: f64 = 0.15;
const ZOOM: f64 = 1.25;
const LIVE: f64 = 0.3;
// mango's animation_duration_move.
const PAN: Duration = Duration::from_millis(500);

// Easing.InOutCubic.
pub fn ease(t: f64) -> f64 {
  let t = t.clamp(0.0, 1.0);
  if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}

// Close to mango's animation_curve_move (0.46,1.0,0.29,1): fast, then settles.
fn ease_out(t: f64) -> f64 {
  1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(4)
}

// Where the picture goes on a screen `w` wide for a strip scrolled
// `scrolled` pixels: [how far it slides, its zoom]. PARALLAX of the strip,
// up to the picture's extra width, on a scroller; plain otherwise.
pub fn pan_target(scrolled: i64, w: f64, scroller: bool) -> [f64; 2] {
  if !scroller {
    return [0.0, 1.0];
  }
  [(PARALLAX * scrolled as f64).clamp(0.0, w * (ZOOM - 1.0)), ZOOM]
}

// Redraws `s` while its picture slides. A timer, like the band's: the
// background layer may get no frame callbacks, and a tick callback would
// then stall with the slide.
fn slide(s: &Rc<Screen>) {
  if s.ticking.replace(true) {
    return;
  }
  let weak = Rc::downgrade(s);
  glib::timeout_add_local(Duration::from_millis(16), move || {
    let Some(s) = weak.upgrade() else { return glib::ControlFlow::Break };
    s.area.queue_draw();
    let mut pan = s.pan.borrow_mut();
    if pan.started.is_some_and(|t| t.elapsed() >= PAN) {
      pan.started = None;
    }
    let going = pan.started.is_some();
    s.ticking.set(going);
    if going { glib::ControlFlow::Continue } else { glib::ControlFlow::Break }
  });
}

// The band's corners at `progress`: top left, top right, bottom right,
// bottom left. At 1 it covers the whole screen.
pub fn band(w: f64, h: f64, progress: f64) -> [(f64, f64); 4] {
  let top = w / 2.0 - SLANT * h / 2.0;
  let bottom = w / 2.0 + SLANT * h / 2.0;
  let spread = (w / 2.0 + SLANT.abs() * h / 2.0 + 4.0) * progress;
  [(top - spread, 0.0), (top + spread, 0.0), (bottom + spread, h), (bottom - spread, h)]
}

#[derive(Clone)]
struct Image {
  path: String,
  pixbuf: Option<Pixbuf>,
}

impl Image {
  fn new(path: &str) -> Image {
    Image { path: path.into(), pixbuf: None }
  }
}

#[derive(Default)]
struct State {
  current: String,
  displayed: Option<Image>,
  old: Option<Image>,
  incoming: Option<Image>,
  // Some while the band moves.
  started: Option<Instant>,
  progress: f64,
  version: u64,
  // A theme switch's colors.toml and shell.toml, base64, until applied.
  theme: Option<(String, String)>,
}

// The picture's [slide, zoom] on its way from `from` to `to`.
struct Pan {
  from: [f64; 2],
  to: [f64; 2],
  // Some while it slides.
  started: Option<Instant>,
}

impl Default for Pan {
  fn default() -> Pan {
    Pan { from: [0.0, 1.0], to: [0.0, 1.0], started: None }
  }
}

impl Pan {
  fn now(&self) -> [f64; 2] {
    match self.started {
      Some(t) => {
        let p = ease_out(t.elapsed().as_secs_f64() / PAN.as_secs_f64());
        [0, 1].map(|i| self.from[i] + (self.to[i] - self.from[i]) * p)
      }
      None => self.to,
    }
  }

  // From where it shows (plus the fingers' `live` slide) to `to`.
  fn go(&mut self, live: f64, to: [f64; 2]) {
    let [x, z] = self.now();
    *self = Pan { from: [x + live, z], to, started: Some(Instant::now()) };
  }
}

struct Screen {
  window: gtk::Window,
  area: gtk::DrawingArea,
  // mango's name for the output and its left edge, to follow its strip.
  name: String,
  left: i64,
  pan: RefCell<Pan>,
  // How far the fingers have taken the picture past `pan`, and whether the
  // timer that redraws a slide runs.
  live: Cell<f64>,
  ticking: Cell<bool>,
  // The images cropped to this screen, newest first: (path, w, h, surface).
  cache: RefCell<Vec<(String, i32, i32, cairo::ImageSurface)>>,
}

pub struct Background {
  app: gtk::Application,
  screens: RefCell<Vec<Rc<Screen>>>,
  st: RefCell<State>,
  themed: Box<dyn Fn()>,
  // A three-finger swipe under way: where its travel stood when it started
  // (or when mango last moved the windows) and where it stands.
  touch: Cell<Option<(f64, f64)>>,
}

fn cover(pixbuf: &Pixbuf, w: i32, h: i32) -> Option<cairo::ImageSurface> {
  let surface = cairo::ImageSurface::create(cairo::Format::Rgb24, w, h).ok()?;
  let cr = cairo::Context::new(&surface).ok()?;
  let (pw, ph) = (pixbuf.width() as f64, pixbuf.height() as f64);
  let scale = (w as f64 / pw).max(h as f64 / ph);
  cr.translate((w as f64 - pw * scale) / 2.0, (h as f64 - ph * scale) / 2.0);
  cr.scale(scale, scale);
  cr.set_source_pixbuf(pixbuf, 0.0, 0.0);
  cr.source().set_filter(cairo::Filter::Good);
  cr.paint().ok()?;
  drop(cr);
  Some(surface)
}

impl Screen {
  fn surface(&self, image: &Image, w: i32, h: i32) -> Option<cairo::ImageSurface> {
    let pixbuf = image.pixbuf.as_ref()?;
    let mut cache = self.cache.borrow_mut();
    if let Some(i) = cache.iter().position(|(p, cw, ch, _)| *p == image.path && *cw == w && *ch == h) {
      let hit = cache.remove(i);
      let surface = hit.3.clone();
      cache.insert(0, hit);
      return Some(surface);
    }
    let surface = cover(pixbuf, w, h)?;
    cache.insert(0, (image.path.clone(), w, h, surface.clone()));
    cache.truncate(4);
    Some(surface)
  }
}

impl Background {
  pub fn new(app: &gtk::Application, themed: impl Fn() + 'static) -> Rc<Background> {
    let me = Rc::new(Background { app: app.clone(), screens: RefCell::default(), st: RefCell::default(), themed: Box::new(themed), touch: Cell::default() });
    if let Some(display) = gdk::Display::default() {
      let weak = Rc::downgrade(&me);
      display.monitors().connect_items_changed(move |_, _, _, _| {
        if let Some(me) = weak.upgrade() {
          me.place();
        }
      });
    }
    me.place();
    let weak = Rc::downgrade(&me);
    crate::mango::get().connect(move || {
      if let Some(me) = weak.upgrade() {
        me.follow();
      }
    });
    let weak = Rc::downgrade(&me);
    crate::touchpad::watch(move |travel| {
      if let Some(me) = weak.upgrade() {
        me.swipe(travel);
      }
    });
    crate::quickshell::disable("omarchy.background");
    me.refresh();
    me
  }

  pub fn ipc(self: &Rc<Self>, method: &str, args: &[String]) -> Option<String> {
    let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
    match method {
      "refresh" => self.refresh(),
      "set" => self.transition("", arg(0), arg(0), false, false),
      "setInstant" => self.transition("", arg(0), arg(0), true, false),
      "transition" => self.transition(arg(0), arg(1), arg(1), false, false),
      "themeTransition" => {
        self.transition(arg(0), arg(1), arg(2), false, true);
        self.st.borrow_mut().theme = Some((arg(3).to_string(), arg(4).to_string()));
        let animating = { let st = self.st.borrow(); st.incoming.is_some() && st.progress < 1.0 };
        if animating {
          let weak = Rc::downgrade(self);
          glib::timeout_add_local_once(THEME_FALLBACK, move || {
            if let Some(me) = weak.upgrade() {
              me.apply_theme();
            }
          });
        } else {
          self.apply_theme();
        }
      }
      _ => return None,
    }
    Some("ok".into())
  }

  // One window per screen, made again when screens come and go.
  fn place(self: &Rc<Self>) {
    for s in self.screens.borrow_mut().drain(..) {
      s.window.destroy();
    }
    let Some(display) = gdk::Display::default() else { return };
    let monitors = display.monitors();
    for i in 0..monitors.n_items() {
      let Some(monitor) = monitors.item(i).and_downcast::<gdk::Monitor>() else { continue };
      let screen = self.screen(&monitor);
      screen.window.set_visible(true);
      self.screens.borrow_mut().push(screen);
    }
  }

  fn screen(self: &Rc<Self>, monitor: &gdk::Monitor) -> Rc<Screen> {
    let window = gtk::Window::builder().application(&self.app).decorated(false).build();
    window.add_css_class("doxia");
    window.add_css_class("background-window");
    window.init_layer_shell();
    window.set_namespace(Some("doxia-background"));
    window.set_layer(Layer::Background);
    window.set_keyboard_mode(KeyboardMode::None);
    window.set_monitor(Some(monitor));
    for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
      window.set_anchor(edge, true);
    }
    window.set_exclusive_zone(-1);
    let area = gtk::DrawingArea::new();
    window.set_child(Some(&area));
    let name = monitor.connector().map(|c| c.to_string()).unwrap_or_default();
    let left = monitor.geometry().x() as i64;
    let screen = Rc::new(Screen { window, area, name, left, pan: RefCell::default(), live: Cell::default(), ticking: Cell::default(), cache: RefCell::default() });
    let mango = crate::mango::get();
    screen.pan.borrow_mut().to = pan_target(mango.scrolled(&screen.name, screen.left), monitor.geometry().width() as f64, mango.scroller(&screen.name));

    let (weak, s) = (Rc::downgrade(self), Rc::downgrade(&screen));
    screen.area.set_draw_func(move |_, cr, w, h| {
      if let (Some(me), Some(s)) = (weak.upgrade(), s.upgrade()) {
        me.draw(&s, cr, w, h);
      }
    });

    let click = gtk::GestureClick::new();
    click.set_button(0);
    click.connect_pressed(|g, n, _, _| {
      if n != 2 {
        return;
      }
      if g.current_button() == gdk::BUTTON_SECONDARY {
        crate::run::shell("theme=$(omarchy-theme-switcher); [[ -n $theme ]] && omarchy-theme-set \"$theme\" >/dev/null 2>&1");
      } else {
        crate::run::shell("background=$(omarchy-theme-bg-switcher); [[ -n $background ]] && omarchy-theme-bg-set \"$background\"");
      }
    });
    screen.area.add_controller(click);
    screen
  }

  fn draw(&self, screen: &Screen, cr: &cairo::Context, w: i32, h: i32) {
    let st = self.st.borrow();
    cr.set_source_rgb(0.0, 0.0, 0.0);
    let _ = cr.paint();
    let [slide, zoom] = screen.pan.borrow().now();
    let (fw, fh) = (w as f64, h as f64);
    let offset = (slide + screen.live.get()).clamp(0.0, fw * (zoom - 1.0)).round();
    // Plain and fully zoomed are drawn as cropped; in between, the zoomed
    // crop scaled down to `zoom`, centered.
    let (wide, tall) = ((fw * ZOOM).ceil() as i32, (fh * ZOOM).ceil() as i32);
    let paint = |image: &Option<Image>| {
      let Some(image) = image.as_ref() else { return false };
      if (zoom - 1.0).abs() < 1e-3 {
        let Some(surface) = screen.surface(image, w, h) else { return false };
        return cr.set_source_surface(&surface, 0.0, 0.0).is_ok() && cr.paint().is_ok();
      }
      let Some(surface) = screen.surface(image, wide, tall) else { return false };
      let k = zoom / ZOOM;
      cr.save().ok();
      cr.translate(-offset - (wide as f64 * k - fw * zoom) / 2.0, (fh - tall as f64 * k) / 2.0);
      cr.scale(k, k);
      let ok = cr.set_source_surface(&surface, 0.0, 0.0).is_ok() && {
        cr.source().set_filter(cairo::Filter::Good);
        cr.paint().is_ok()
      };
      cr.restore().ok();
      ok
    };
    let Some(incoming) = st.incoming.as_ref().filter(|_| st.started.is_some()) else {
      paint(&st.displayed);
      return;
    };
    if !paint(&st.old) {
      paint(&st.displayed);
    }
    let corners = band(w as f64, h as f64, st.progress);
    cr.move_to(corners[0].0, corners[0].1);
    for (x, y) in &corners[1..] {
      cr.line_to(*x, *y);
    }
    cr.close_path();
    cr.clip();
    paint(&Some(incoming.clone()));
    cr.reset_clip();
  }

  // Slides each screen's picture after its strip when the windows move.
  // A swipe that moved the picture hands it over from where it shows.
  fn follow(&self) {
    let mango = crate::mango::get();
    for s in self.screens.borrow().iter() {
      let target = pan_target(mango.scrolled(&s.name, s.left), s.area.width() as f64, mango.scroller(&s.name));
      let mut pan = s.pan.borrow_mut();
      if (target[0] - pan.to[0]).abs() < 0.5 && target[1] == pan.to[1] {
        continue;
      }
      pan.go(s.live.replace(0.0), target);
      drop(pan);
      if let Some((_, at)) = self.touch.get() {
        self.touch.set(Some((at, at)));
      }
      slide(s);
    }
  }

  // Three fingers on the touchpad: the picture of the focused screen goes
  // with them while its strip overflows; on lifting it eases back to where
  // mango says, which a moment later is where the windows went.
  fn swipe(&self, travel: Option<f64>) {
    let Some(at) = travel else {
      self.touch.set(None);
      for s in self.screens.borrow().iter().filter(|s| s.live.get() != 0.0) {
        let mut pan = s.pan.borrow_mut();
        let to = pan.to;
        pan.go(s.live.replace(0.0), to);
        drop(pan);
        slide(s);
      }
      return;
    };
    let (start, _) = self.touch.get().unwrap_or((at, at));
    self.touch.set(Some((start, at)));
    let mango = crate::mango::get();
    let focused = mango.focused_monitor();
    for s in self.screens.borrow().iter().filter(|s| s.name == focused) {
      let w = s.area.width() as f64;
      if !mango.scroller(&s.name) || !mango.overflows(&s.name, s.left, w as i64) {
        continue;
      }
      // Fingers to the left bring the windows on the right in: the picture
      // moves left with them.
      let [base, _] = s.pan.borrow().now();
      let live = (base - (at - start) * w * LIVE).clamp(0.0, w * (ZOOM - 1.0)) - base;
      s.live.set(live);
      s.area.queue_draw();
    }
  }

  fn redraw(&self) {
    for s in self.screens.borrow().iter() {
      s.area.queue_draw();
    }
  }

  fn refresh(self: &Rc<Self>) {
    let home = std::env::var("HOME").unwrap_or_default();
    let link = format!("{home}/.local/state/omarchy/current/background");
    if let Ok(path) = std::fs::canonicalize(link) {
      let path = path.to_string_lossy().into_owned();
      self.transition("", &path, &path, false, false);
    }
  }

  // Background.transitionBackground: `path` comes in (over `from`, or what
  // shows), and `final` is what it stands for once in: a theme switch shows
  // a snapshot it deletes afterwards.
  fn transition(self: &Rc<Self>, from: &str, path: &str, final_path: &str, instant: bool, force: bool) {
    let (path, final_path, from) = (path.trim(), final_path.trim(), from.trim());
    let final_path = if final_path.is_empty() { path } else { final_path };
    let version = {
      let mut st = self.st.borrow_mut();
      if path.is_empty() || (!force && final_path == st.current) {
        return;
      }
      st.current = final_path.into();
      st.version += 1;
      st.started = None;
      if instant || st.displayed.is_none() {
        st.old = None;
        st.incoming = None;
        st.progress = 1.0;
        st.version
      } else {
        st.old = (!from.is_empty()).then(|| Image::new(from)).or_else(|| st.displayed.clone());
        st.incoming = Some(Image::new(path));
        st.progress = 0.0;
        st.version
      }
    };
    let direct = self.st.borrow().incoming.is_none();
    if direct {
      // Swapped in once decoded; the old picture stays up until then.
      self.load(path, version, |st, pixbuf| {
        if pixbuf.is_some() {
          st.displayed = Some(Image { path: st.current.clone(), pixbuf });
        }
      });
      return;
    }
    if let Some(old) = self.st.borrow().old.as_ref().filter(|o| o.pixbuf.is_none()).map(|o| o.path.clone()) {
      // Gone (a snapshot already deleted): the band runs over what shows.
      self.load(&old, version, |st, pixbuf| match pixbuf {
        Some(p) => st.old.iter_mut().for_each(|o| o.pixbuf = Some(p.clone())),
        None => st.old = None,
      });
    }
    self.load(path, version, |st, pixbuf| match pixbuf {
      Some(p) => st.incoming.iter_mut().for_each(|i| i.pixbuf = Some(p.clone())),
      None => st.incoming = None,
    });
  }

  fn load(self: &Rc<Self>, path: &str, version: u64, store: impl FnOnce(&mut State, Option<Pixbuf>) + 'static) {
    let weak = Rc::downgrade(self);
    let file = gio::File::for_path(path);
    glib::spawn_future_local(async move {
      let pixbuf = match file.read_future(glib::Priority::DEFAULT).await {
        Ok(stream) => Pixbuf::from_stream_future(&stream).await.ok(),
        Err(_) => None,
      };
      let Some(me) = weak.upgrade() else { return };
      {
        let mut st = me.st.borrow_mut();
        if st.version != version {
          return;
        }
        store(&mut st, pixbuf);
      }
      me.maybe_reveal();
      me.redraw();
    });
  }

  // The band starts once the incoming picture (and the one it covers) is in.
  fn maybe_reveal(self: &Rc<Self>) {
    {
      let mut st = self.st.borrow_mut();
      let ready = st.incoming.as_ref().is_some_and(|i| i.pixbuf.is_some())
        && st.old.as_ref().is_none_or(|o| o.pixbuf.is_some());
      if !ready || st.started.is_some() {
        return;
      }
      st.started = Some(Instant::now());
    }
    self.apply_theme();
    let weak = Rc::downgrade(self);
    glib::timeout_add_local(Duration::from_millis(16), move || {
      let Some(me) = weak.upgrade() else { return glib::ControlFlow::Break };
      let mut st = me.st.borrow_mut();
      let Some(started) = st.started else { return glib::ControlFlow::Break };
      let t = started.elapsed().as_secs_f64() / REVEAL.as_secs_f64();
      st.progress = ease(t);
      if t >= 1.0 {
        let current = st.current.clone();
        st.displayed = st.incoming.take().map(|i| Image { path: current, pixbuf: i.pixbuf });
        st.old = None;
        st.started = None;
        st.progress = 1.0;
      }
      let done = st.started.is_none();
      drop(st);
      me.redraw();
      if done { glib::ControlFlow::Break } else { glib::ControlFlow::Continue }
    });
  }

  // The theme's colors reach doxia-shell's CSS as the reveal starts.
  fn apply_theme(&self) {
    if self.st.borrow_mut().theme.take().is_none() {
      return;
    }
    (self.themed)();
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn band_widens_from_the_middle_to_cover() {
    let [tl, tr, br, bl] = band(1000.0, 500.0, 0.0);
    assert_eq!((tl.0, tr.0), (545.0, 545.0));
    assert_eq!((bl.0, br.0), (455.0, 455.0));
    let [tl, tr, br, bl] = band(1000.0, 500.0, 1.0);
    assert!(tl.0 < 0.0 && bl.0 < 0.0 && tr.0 > 1000.0 && br.0 > 1000.0);
    assert_eq!(ease(0.5), 0.5);
    assert_eq!((ease(0.0), ease(1.0), ease(2.0)), (0.0, 1.0, 1.0));
  }

  #[test]
  fn the_picture_slides_with_the_strip_up_to_its_extra_width() {
    assert_eq!(pan_target(0, 1000.0, true), [0.0, ZOOM]);
    assert_eq!(pan_target(-50, 1000.0, true), [0.0, ZOOM]);
    assert!((pan_target(1000, 1000.0, true)[0] - 150.0).abs() < 1e-9);
    assert!((pan_target(10_000, 1000.0, true)[0] - 250.0).abs() < 1e-9);
    assert_eq!(pan_target(1000, 1000.0, false), [0.0, 1.0]);
    assert_eq!((ease_out(0.0), ease_out(1.0)), (0.0, 1.0));
  }
}
