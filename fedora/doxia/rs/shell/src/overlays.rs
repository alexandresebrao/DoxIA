// Full-screen overlays the network panel and the menu summon, replacing
// shell/plugins/panels/{wifiqr,speedtest,disk-speedtest} and
// shell/Ui/SpeedTestOverlay.qml: a dark scrim with the content centered;
// a click outside or Escape closes.
//
// - omarchy.wifiqr: the Wi-Fi's QR code (omarchy-network-qr --meta) and,
//   on a click, its password (omarchy-network-password).
// - omarchy.speedtest: download then upload, 5 s each, through
//   omarchy-network-speedtest, on two dials.
// - omarchy.disk-speedtest: omarchy-disk-speedtest's read and write.

use gtk::prelude::*;
use gtk::{gdk, gio, glib, pango};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

fn window(app: &gtk::Application, namespace: &str) -> (gtk::Window, gtk::Box) {
  let window = gtk::Window::builder().application(app).decorated(false).build();
  window.add_css_class("doxia");
  window.add_css_class("overlay-window");
  window.init_layer_shell();
  window.set_namespace(Some(namespace));
  window.set_layer(Layer::Overlay);
  window.set_keyboard_mode(KeyboardMode::Exclusive);
  for e in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
    window.set_anchor(e, true);
  }
  window.set_exclusive_zone(-1);
  let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
  content.set_halign(gtk::Align::Center);
  content.set_valign(gtk::Align::Center);
  window.set_child(Some(&content));
  (window, content)
}

// Closing on Escape and on a click off the content.
fn closing(window: &gtk::Window, content: &gtk::Box, close: Rc<dyn Fn()>) {
  let keys = gtk::EventControllerKey::new();
  let c = close.clone();
  keys.connect_key_pressed(move |_, k, _, _| {
    if k == gdk::Key::Escape {
      c();
      return glib::Propagation::Stop;
    }
    glib::Propagation::Proceed
  });
  window.add_controller(keys);
  let click = gtk::GestureClick::new();
  let (w, ct) = (window.downgrade(), content.downgrade());
  click.connect_released(move |_, _, x, y| {
    let (Some(w), Some(ct)) = (w.upgrade(), ct.upgrade()) else { return };
    let inside = ct.compute_bounds(&w).is_some_and(|b| b.contains_point(&gtk::graphene::Point::new(x as f32, y as f32)));
    if !inside {
      close();
    }
  });
  window.add_controller(click);
}

fn label(text: &str, class: &str) -> gtk::Label {
  let l = gtk::Label::new(Some(text));
  for c in class.split_whitespace() {
    l.add_css_class(c);
  }
  l
}

// omarchy-network-qr --meta: "meta\t<iface>\t<security>\t<ssid>", then a
// square 0/1 matrix (empty when it is not one).
pub fn parse_qr(raw: &str) -> ((String, String, String), Vec<String>) {
  let mut lines: Vec<&str> = raw.trim().lines().filter(|l| !l.is_empty()).collect();
  let mut meta = (String::new(), String::new(), String::new());
  if lines.first().is_some_and(|l| l.starts_with("meta\t")) {
    let f: Vec<&str> = lines.remove(0).split('\t').collect();
    meta = (f.get(1).unwrap_or(&"").to_string(), f.get(2).unwrap_or(&"").to_string(), f.get(3..).map(|s| s.join("\t")).unwrap_or_default());
  }
  let size = lines.first().map(|l| l.len()).unwrap_or(0);
  let ok = size > 0 && lines.len() == size && lines.iter().all(|l| l.len() == size && l.bytes().all(|b| b == b'0' || b == b'1'));
  (meta, if ok { lines.into_iter().map(String::from).collect() } else { Vec::new() })
}

pub struct WifiQr {
  window: gtk::Window,
  content: gtk::Box,
  generation: Cell<u64>,
}

impl WifiQr {
  pub fn new(app: &gtk::Application) -> Rc<WifiQr> {
    let (window, content) = window(app, "doxia-network-qr");
    let me = Rc::new(WifiQr { window, content, generation: Cell::new(0) });
    let weak = Rc::downgrade(&me);
    closing(&me.window, &me.content, Rc::new(move || {
      if let Some(me) = weak.upgrade() {
        me.close();
      }
    }));
    me
  }

  pub fn ipc(self: &Rc<Self>, method: &str, args: &[String]) -> Option<String> {
    match method {
      "summon" | "toggle" if !(method == "toggle" && self.window.is_visible()) => {
        let payload: serde_json::Value = serde_json::from_str(args.first().map(String::as_str).unwrap_or("{}")).unwrap_or_default();
        self.open(payload["iface"].as_str().unwrap_or(""), payload["ssid"].as_str().unwrap_or(""));
      }
      "toggle" | "hide" => self.close(),
      "call" => {}
      _ => return None,
    }
    Some("ok".into())
  }

  fn close(&self) {
    self.generation.set(self.generation.get() + 1);
    self.window.set_visible(false);
  }

  fn open(self: &Rc<Self>, iface: &str, ssid: &str) {
    let g = self.generation.get() + 1;
    self.generation.set(g);
    crate::popup::clear(&self.content);
    let title = label(&if ssid.is_empty() { "WI-FI".into() } else { ssid.to_uppercase() }, "overlay-title");
    title.set_ellipsize(pango::EllipsizeMode::End);
    title.set_max_width_chars(32);
    self.content.append(&title);
    let status = label("Generating QR code…", "overlay-dim");
    self.content.append(&status);
    self.window.set_visible(true);
    let me = self.clone();
    let iface = iface.to_string();
    glib::spawn_future_local(async move {
      let mut argv = vec!["omarchy-network-qr".to_string(), "--meta".into()];
      if !iface.is_empty() {
        argv.push(iface.clone());
      }
      let args: Vec<&std::ffi::OsStr> = argv.iter().map(|s| s.as_ref()).collect();
      let res = match gio::Subprocess::newv(&args, gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_PIPE) {
        Ok(child) => {
          let out = child.communicate_utf8_future(None).await.ok();
          let (o, e) = out.map(|(o, e)| (o.map(|s| s.to_string()).unwrap_or_default(), e.map(|s| s.to_string()).unwrap_or_default())).unwrap_or_default();
          (child.is_successful(), o, e)
        }
        Err(_) => (false, String::new(), String::new()),
      };
      if me.generation.get() != g {
        return;
      }
      let (ok, out, err) = res;
      let ((qr_iface, security, qr_ssid), rows) = parse_qr(&out);
      if !qr_ssid.is_empty() {
        title.set_text(&qr_ssid.to_uppercase());
      }
      if !ok || rows.is_empty() {
        status.set_text(if err.trim().is_empty() { "Could not generate the Wi-Fi QR code" } else { err.trim() });
        status.add_css_class("overlay-error");
        return;
      }
      status.set_visible(false);
      let size = rows.len() as i32;
      let module = (240 / size).max(4);
      let area = gtk::DrawingArea::new();
      area.set_content_width(size * module);
      area.set_content_height(size * module);
      area.set_halign(gtk::Align::Center);
      area.set_draw_func(move |_, cr, w, h| {
        crate::card::rounded(cr, 0.0, 0.0, w as f64, h as f64, 6.0);
        cr.set_source_rgb(1.0, 1.0, 1.0);
        let _ = cr.fill();
        cr.set_source_rgb(0x11 as f64 / 255.0, 0x11 as f64 / 255.0, 0x11 as f64 / 255.0);
        for (y, row) in rows.iter().enumerate() {
          for (x, b) in row.bytes().enumerate() {
            if b == b'1' {
              cr.rectangle((x as i32 * module) as f64, (y as i32 * module) as f64, module as f64, module as f64);
            }
          }
        }
        let _ = cr.fill();
      });
      me.content.append(&area);
      me.content.append(&label("Scan to join this network", "overlay-dim"));
      if !security.is_empty() && security != "nopass" {
        let pw = label("Show password", "overlay-password");
        pw.set_wrap(true);
        pw.set_wrap_mode(pango::WrapMode::Char);
        pw.set_max_width_chars(32);
        pw.set_cursor_from_name(Some("pointer"));
        let shown = Rc::new(RefCell::new(None::<String>));
        let click = gtk::GestureClick::new();
        let (p, sh, qi) = (pw.clone(), shown.clone(), qr_iface.clone());
        click.connect_released(move |g, _, _, _| {
          g.set_state(gtk::EventSequenceState::Claimed);
          if p.has_css_class("visible") {
            p.remove_css_class("visible");
            p.set_text("Show password");
            return;
          }
          if let Some(s) = sh.borrow().clone() {
            p.add_css_class("visible");
            p.set_text(&s);
            return;
          }
          let (p, sh, qi) = (p.clone(), sh.clone(), qi.clone());
          glib::spawn_future_local(async move {
            match crate::watch::output(&["omarchy-network-password", &qi]).await.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
              Some(s) => {
                *sh.borrow_mut() = Some(s.clone());
                p.add_css_class("visible");
                p.set_text(&s);
              }
              None => {
                p.add_css_class("overlay-error");
                p.set_text("Could not read the Wi-Fi password");
              }
            }
          });
        });
        pw.add_controller(click);
        me.content.append(&pw);
      }
    });
  }
}

// A speed dial: a 270° arc filled to value/scale, ticks, a needle and the
// reading.
struct Dial {
  area: gtk::DrawingArea,
  value: Rc<Cell<f64>>,
  shown: Rc<Cell<f64>>,
  live: Rc<Cell<bool>>,
}

impl Dial {
  fn new(caption: &str, unit: &str, scale: Rc<Cell<f64>>, accent: [f64; 4]) -> Dial {
    let area = gtk::DrawingArea::new();
    area.set_content_width(210);
    area.set_content_height(210);
    let (value, shown, live) = (Rc::new(Cell::new(0.0)), Rc::new(Cell::new(0.0)), Rc::new(Cell::new(false)));
    let (sh, lv, va) = (shown.clone(), live.clone(), value.clone());
    let (caption, unit) = (caption.to_string(), unit.to_string());
    area.set_draw_func(move |a, cr, w, h| {
      let (w, h) = (w as f64, h as f64);
      let (cx, cy) = (w / 2.0, h / 2.0);
      let arc_w = 4.0;
      let r = w.min(h) / 2.0 - arc_w;
      let engaged = lv.get() || va.get() > 0.0;
      cr.push_group();
      let start = 135f64.to_radians();
      let sweep = 270f64.to_radians();
      let frac = (sh.get() / scale.get().max(1.0)).clamp(0.0, 1.0);
      cr.set_line_cap(gtk::cairo::LineCap::Round);
      cr.set_line_width(arc_w);
      cr.set_source_rgba(1.0, 1.0, 1.0, 0.14);
      cr.arc(cx, cy, r, start, start + sweep);
      let _ = cr.stroke();
      if frac > 0.004 {
        cr.set_line_width(arc_w * 3.0);
        cr.set_source_rgba(accent[0], accent[1], accent[2], 0.18);
        cr.arc(cx, cy, r, start, start + sweep * frac);
        let _ = cr.stroke();
        cr.set_line_width(arc_w);
        cr.set_source_rgba(accent[0], accent[1], accent[2], 1.0);
        cr.arc(cx, cy, r, start, start + sweep * frac);
        let _ = cr.stroke();
      }
      for i in 0..46 {
        let major = i % 5 == 0;
        let ang = start + sweep * i as f64 / 45.0;
        let outer = r - arc_w * 2.0 + arc_w - if major { 0.0 } else { 2.0 };
        let len = if major { 10.0 } else { 6.0 };
        cr.set_line_width(if major { 2.0 } else { 1.0 });
        cr.set_source_rgba(1.0, 1.0, 1.0, if major { 0.3 } else { 0.12 });
        cr.move_to(cx + ang.cos() * outer, cy + ang.sin() * outer);
        cr.line_to(cx + ang.cos() * (outer - len), cy + ang.sin() * (outer - len));
        let _ = cr.stroke();
      }
      // The needle, fading towards the center.
      let ang = start + sweep * frac;
      let (tip, tail) = (r - arc_w * 2.0 - 10.0, r - arc_w * 2.0 - 10.0 - w * 0.32);
      let g = gtk::cairo::LinearGradient::new(cx + ang.cos() * tip, cy + ang.sin() * tip, cx + ang.cos() * tail, cy + ang.sin() * tail);
      g.add_color_stop_rgba(0.0, accent[0], accent[1], accent[2], 1.0);
      g.add_color_stop_rgba(0.55, accent[0], accent[1], accent[2], 1.0);
      g.add_color_stop_rgba(1.0, accent[0], accent[1], accent[2], 0.0);
      let _ = cr.set_source(&g);
      cr.set_line_width(3.0);
      cr.move_to(cx + ang.cos() * tip, cy + ang.sin() * tip);
      cr.line_to(cx + ang.cos() * tail, cy + ang.sin() * tail);
      let _ = cr.stroke();
      // The reading, the unit, the caption.
      let text = |t: &str, size: f64, bold: bool, y: f64, alpha: f64| {
        let layout = a.create_pango_layout(Some(t));
        let mut f = pango::FontDescription::from_string("DoxIA Shell");
        f.set_absolute_size(size * pango::SCALE as f64);
        if bold {
          f.set_weight(pango::Weight::Bold);
        }
        layout.set_font_description(Some(&f));
        let (tw, _) = layout.pixel_size();
        cr.set_source_rgba(1.0, 1.0, 1.0, alpha);
        cr.move_to(cx - tw as f64 / 2.0, y);
        pangocairo::functions::show_layout(cr, &layout);
      };
      let reading = sh.get();
      let rt = if reading < 10.0 { format!("{reading:.1}").replace('.', ",") } else { format!("{}", reading.round() as i64) };
      text(&rt, 24.0, true, cy + 14.0, 1.0);
      text(&unit, 10.0, false, cy + 46.0, 0.55);
      text(&caption, 10.0, true, h - 16.0, 0.55);
      let _ = cr.pop_group_to_source();
      let _ = cr.paint_with_alpha(if engaged { 1.0 } else { 0.5 });
    });
    Dial { area, value, shown, live }
  }

  fn set(&self, v: f64) {
    self.value.set(v);
  }
}

pub struct SpeedTest {
  window: gtk::Window,
  content: gtk::Box,
  title: gtk::Label,
  error: gtk::Label,
  again: gtk::Button,
  dials: [Dial; 2],
  scale: Rc<Cell<f64>>,
  stops: Vec<f64>,
  running: Cell<bool>,
  generation: Cell<u64>,
  disk: bool,
}

impl SpeedTest {
  // `disk`: omarchy-disk-speedtest's read/write, else the network's
  // download/upload.
  pub fn new(app: &gtk::Application, disk: bool) -> Rc<SpeedTest> {
    let (window, content) = window(app, if disk { "doxia-disk-speedtest" } else { "doxia-network-speedtest" });
    let accent = doxia_theme::palette().accent;
    let stops: Vec<f64> = if disk { vec![500.0, 1000.0, 2500.0, 5000.0, 10000.0, 15000.0] } else { vec![100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0] };
    let scale = Rc::new(Cell::new(stops[0]));
    let unit = if disk { "MB/s" } else { "Mbps" };
    let (l, r) = if disk { ("READ", "WRITE") } else { ("DOWNLOAD", "UPLOAD") };
    let title = label("", "overlay-title");
    content.append(&title);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 48);
    row.set_halign(gtk::Align::Center);
    let dials = [Dial::new(l, unit, scale.clone(), accent), Dial::new(r, unit, scale.clone(), accent)];
    row.append(&dials[0].area);
    row.append(&dials[1].area);
    content.append(&row);
    let again = gtk::Button::with_label("Run Again");
    again.add_css_class("ctl");
    again.add_css_class("overlay-again");
    again.set_halign(gtk::Align::Center);
    again.set_tooltip_text(Some(if disk { "Measure again" } else { "Measure again via fast.com" }));
    content.append(&again);
    let error = label("", "overlay-error");
    error.set_wrap(true);
    error.set_max_width_chars(48);
    error.set_justify(gtk::Justification::Center);
    content.append(&error);
    let me = Rc::new(SpeedTest {
      window,
      content,
      title,
      error,
      again,
      dials,
      scale,
      stops,
      running: Cell::new(false),
      generation: Cell::new(0),
      disk,
    });
    let weak = Rc::downgrade(&me);
    closing(&me.window, &me.content, Rc::new(move || {
      if let Some(me) = weak.upgrade() {
        me.close();
      }
    }));
    let weak = Rc::downgrade(&me);
    me.again.connect_clicked(move |_| {
      if let Some(me) = weak.upgrade() {
        me.run();
      }
    });
    let keys = gtk::EventControllerKey::new();
    let weak = Rc::downgrade(&me);
    keys.connect_key_pressed(move |_, k, _, _| {
      if matches!(k, gdk::Key::Return | gdk::Key::KP_Enter) {
        if let Some(me) = weak.upgrade().filter(|m| !m.running.get()) {
          me.run();
        }
        return glib::Propagation::Stop;
      }
      glib::Propagation::Proceed
    });
    me.window.add_controller(keys);
    me
  }

  pub fn ipc(self: &Rc<Self>, method: &str, args: &[String]) -> Option<String> {
    match method {
      "summon" | "toggle" if !(method == "toggle" && self.window.is_visible()) => {
        let payload: serde_json::Value = serde_json::from_str(args.first().map(String::as_str).unwrap_or("{}")).unwrap_or_default();
        self.open(payload["connection"].as_str().map(String::from));
      }
      "toggle" | "hide" => self.close(),
      "call" => {}
      _ => return None,
    }
    Some("ok".into())
  }

  fn close(&self) {
    self.generation.set(self.generation.get() + 1);
    self.running.set(false);
    self.window.set_visible(false);
  }

  fn open(self: &Rc<Self>, connection: Option<String>) {
    self.title.set_text("");
    match connection {
      Some(c) => self.title.set_text(&c.to_uppercase()),
      None if !self.disk => {
        let t = self.title.clone();
        glib::spawn_future_local(async move {
          let out = crate::watch::output(&["omarchy-network-status"]).await.unwrap_or_default();
          let f: Vec<&str> = out.trim().split('\t').collect();
          t.set_text(&match f.first() {
            Some(&"wifi") => f.get(1).filter(|s| !s.is_empty()).unwrap_or(&"Wi-Fi").to_uppercase(),
            Some(&"ethernet") => "ETHERNET".into(),
            _ => String::new(),
          });
        });
      }
      None => {}
    }
    self.window.set_visible(true);
    self.run();
  }

  fn grow_scale(&self, v: f64) {
    let next = self.stops.iter().copied().find(|s| v <= s * 0.92).unwrap_or(*self.stops.last().unwrap());
    if next > self.scale.get() {
      self.scale.set(next);
    }
  }

  fn run(self: &Rc<Self>) {
    let g = self.generation.get() + 1;
    self.generation.set(g);
    self.running.set(true);
    self.again.set_opacity(0.0);
    self.again.set_sensitive(false);
    self.error.set_visible(false);
    self.scale.set(self.stops[0]);
    for d in &self.dials {
      d.set(0.0);
      d.live.set(false);
    }
    self.animate(g);
    let me = self.clone();
    glib::spawn_future_local(async move {
      let phases: Vec<(usize, Vec<&str>)> = if me.disk {
        vec![(0, vec!["omarchy-disk-speedtest"])]
      } else {
        vec![(0, vec!["omarchy-network-speedtest", "down"]), (1, vec!["omarchy-network-speedtest", "up"])]
      };
      for (dial, argv) in phases {
        if me.generation.get() != g {
          return;
        }
        me.dials[dial].live.set(true);
        let args: Vec<&std::ffi::OsStr> = argv.iter().map(|s| s.as_ref()).collect();
        let Ok(child) = gio::Subprocess::newv(&args, gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_PIPE) else { break };
        // The network phases stop after 5 s.
        if !me.disk {
          let (c, gen2, me2) = (child.clone(), g, me.clone());
          glib::timeout_add_local_once(Duration::from_secs(5), move || {
            if me2.generation.get() == gen2 {
              c.force_exit();
            }
          });
        }
        let reader = gio::DataInputStream::new(&child.stdout_pipe().unwrap());
        while let Ok(Some(line)) = reader.read_line_utf8_future(glib::Priority::DEFAULT).await {
          if me.generation.get() != g {
            child.force_exit();
            return;
          }
          let parts: Vec<&str> = line.split_whitespace().collect();
          if me.disk {
            match parts.as_slice() {
              ["disk", rest @ ..] => me.title.set_text(&rest.join(" ").to_uppercase()),
              [kind @ ("read" | "write"), v, ..] => {
                if let Ok(v) = v.parse::<f64>() {
                  let i = if *kind == "read" { 0 } else { 1 };
                  me.dials[0].live.set(i == 0);
                  me.dials[1].live.set(i == 1);
                  me.grow_scale(v);
                  me.dials[i].set(v);
                }
              }
              _ => {}
            }
          } else if let Some(v) = parts.first().and_then(|v| v.parse::<f64>().ok()).filter(|v| *v >= 0.0) {
            me.grow_scale(v);
            me.dials[dial].set(v);
          }
        }
        let _ = child.wait_future().await;
        let forced = !me.disk && child.has_signaled();
        if !child.is_successful() && !forced && me.generation.get() == g {
          let err = match child.stderr_pipe() {
            Some(e) => {
              let r = gio::DataInputStream::new(&e);
              r.read_upto_future(b"\0", glib::Priority::DEFAULT).await.ok().map(|s| String::from_utf8_lossy(&s).into_owned()).unwrap_or_default()
            }
            None => String::new(),
          };
          me.error.set_text(if err.trim().is_empty() { if me.disk { "Disk speed test failed" } else { "Speed test failed" } } else { err.trim() });
          me.error.set_visible(true);
          break;
        }
        me.dials[dial].live.set(false);
      }
      if me.generation.get() == g {
        me.running.set(false);
        for d in &me.dials {
          d.live.set(false);
        }
        me.again.set_opacity(1.0);
        me.again.set_sensitive(true);
      }
    });
  }

  // The dials sweep up and back when a run starts, then follow their
  // values (600 ms ease).
  fn animate(self: &Rc<Self>, g: u64) {
    let start = Instant::now();
    let weak = Rc::downgrade(self);
    glib::timeout_add_local(Duration::from_millis(16), move || {
      let Some(me) = weak.upgrade() else { return glib::ControlFlow::Break };
      if me.generation.get() != g || !me.window.is_visible() {
        return glib::ControlFlow::Break;
      }
      let t = start.elapsed().as_secs_f64();
      let full = me.scale.get();
      for d in &me.dials {
        let shown = if t < 0.55 {
          let x = t / 0.55;
          full * if x < 0.5 { 4.0 * x * x * x } else { 1.0 - (-2.0 * x + 2.0f64).powi(3) / 2.0 }
        } else if t < 1.2 {
          let x = (t - 0.55) / 0.65;
          full * (1.0 - (1.0 - (1.0 - x).powi(3)))
        } else {
          let cur = d.shown.get();
          cur + (d.value.get() - cur) * 0.12
        };
        d.shown.set(shown);
        d.area.queue_draw();
      }
      if !me.running.get() && t > 1.2 && me.dials.iter().all(|d| (d.shown.get() - d.value.get()).abs() < 0.01) {
        return glib::ControlFlow::Break;
      }
      glib::ControlFlow::Continue
    });
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn qr_output() {
    let (meta, rows) = parse_qr("meta\twlan0\tWPA\tCasa\tdo\tZé\n101\n010\n101\n");
    assert_eq!(meta, ("wlan0".into(), "WPA".into(), "Casa\tdo\tZé".into()));
    assert_eq!(rows.len(), 3);
    assert!(parse_qr("meta\twlan0\tWPA\tx\n10\n010\n").1.is_empty());
  }
}
