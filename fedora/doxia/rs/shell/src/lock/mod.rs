// The lock screen, replacing shell/plugins/lock: ext-session-lock through
// gtk4-layer-shell's session lock API, one view per monitor laid out like the
// login screen (default/sddm/omarchy/Main.qml) over its animated landscape
// (landscape.rs): the brand, the clock and the date on the left, the avatar,
// the name and the password on a thin line in the lower third. PAM in a child process (pam.rs) for the password
// (omarchy-lock-password) and, when a finger is enrolled, the reader
// (omarchy-lock-fingerprint) in parallel. Same IPC (`omarchy-shell lock
// lock|isLocked|status|preview|hidePreview`), same marker
// ($XDG_RUNTIME_DIR/omarchy-session-locked) for omarchy-mango-session-locked,
// same 5 s blank and wake on input.
//
// The compositor keeps the session locked if doxia-shell dies while locked;
// systemd restarts it, and the stranded-lock check at startup locks again so
// there is a password field to get back in.

pub mod landscape;
pub mod pam;

use gtk::glib::translate::{FromGlibPtrFull, ToGlibPtr};
use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::{Cell, RefCell};
use std::ffi::OsStr;
use std::rc::Rc;
use std::time::{Duration, Instant};

const PASSWORD_SERVICE: &str = "omarchy-lock-password";
const FINGERPRINT_SERVICE: &str = "omarchy-lock-fingerprint";
const G_FINGERPRINT: &str = "\u{f0237}";
const PLACEHOLDER: &str = "Senha";
// As the login screen writes them (GLib says "sexta").
const WEEKDAYS: [&str; 7] = ["segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado", "domingo"];
const FAILED: &str = "Senha incorreta. Tente novamente.";

#[link(name = "gtk4-layer-shell")]
unsafe extern "C" {
  fn gtk_session_lock_is_supported() -> glib::ffi::gboolean;
  fn gtk_session_lock_instance_new() -> *mut glib::gobject_ffi::GObject;
  fn gtk_session_lock_instance_lock(instance: *mut glib::gobject_ffi::GObject) -> glib::ffi::gboolean;
  fn gtk_session_lock_instance_unlock(instance: *mut glib::gobject_ffi::GObject);
  fn gtk_session_lock_instance_assign_window_to_monitor(
    instance: *mut glib::gobject_ffi::GObject,
    window: *mut gtk::ffi::GtkWindow,
    monitor: *mut gdk::ffi::GdkMonitor,
  );
}

fn marker() -> std::path::PathBuf {
  std::path::PathBuf::from(std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into())).join("omarchy-session-locked")
}

fn sh(command: &str) {
  let _ = std::process::Command::new("bash")
    .args(["-c", command])
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .spawn()
    .map(|mut c| std::thread::spawn(move || c.wait()));
}

fn brand_path() -> String {
  format!("{}/default/sddm/omarchy/brand.png", std::env::var("OMARCHY_PATH").unwrap_or_default())
}

// The account's name, as the login screen shows it.
fn shown_name() -> String {
  let real = glib::real_name().to_string_lossy().trim().to_string();
  if real.is_empty() || real == "Unknown" { glib::user_name().to_string_lossy().to_string() } else { real }
}

fn label(text: &str, classes: &[&str]) -> gtk::Label {
  let l = gtk::Label::new(Some(text));
  for c in classes {
    l.add_css_class(c);
  }
  l
}

// The avatar: the initial in a dark disc with a soft ring, as on the login
// screen (no face image there either: SDDM's stock one is a silhouette).
fn avatar(initial: &str) -> gtk::DrawingArea {
  let area = gtk::DrawingArea::new();
  area.set_content_width(98);
  area.set_content_height(98);
  area.set_halign(gtk::Align::Center);
  let initial = initial.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
  area.set_draw_func(move |_, cr, w, h| {
    let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0);
    cr.arc(cx, cy, 48.0, 0.0, 2.0 * std::f64::consts::PI);
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.5);
    cr.set_line_width(2.0);
    let _ = cr.stroke();
    cr.arc(cx, cy, 44.0, 0.0, 2.0 * std::f64::consts::PI);
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
  area
}

thread_local! {
  // While the displays are blanked, the landscape stops drawing.
  static BLANKED: Cell<bool> = const { Cell::new(false) };
}

// The landscape, redrawn at up to 30 frames a second (none while blanked);
// the weather is read once a second from the bar's.
fn landscape_area() -> gtk::DrawingArea {
  let area = gtk::DrawingArea::new();
  area.set_hexpand(true);
  area.set_vexpand(true);
  let started = Instant::now();
  let sky = Rc::new(Cell::new(landscape::Sky::default()));
  let s = sky.clone();
  area.set_draw_func(move |_, cr, w, h| {
    let now = glib::DateTime::now_local().ok();
    let hour = now.map(|d| d.hour() as f64 + d.minute() as f64 / 60.0 + d.seconds() / 3600.0).unwrap_or(12.0);
    let scene = landscape::Scene::new(w as f64, h as f64, started.elapsed().as_secs_f64(), hour, s.get());
    landscape::draw(cr, &scene);
  });
  let last = Cell::new(0i64);
  area.add_tick_callback(move |a, clock| {
    let now = clock.frame_time();
    if now - last.get() >= 33_000 && !BLANKED.with(Cell::get) {
      last.set(now);
      a.queue_draw();
    }
    glib::ControlFlow::Continue
  });
  let weak = area.downgrade();
  let read = move || {
    if let Some(found) = crate::bar::weather_now().as_ref().and_then(landscape::sky_of) {
      sky.set(found);
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

// One monitor's lock view: the landscape, the clock, the sign-in.
struct View {
  window: gtk::Window,
  entry: gtk::PasswordEntry,
  field: gtk::Box,
  hint: gtk::Label,
  status: gtk::Label,
  sign_in: gtk::Box,
}

pub struct Lock {
  app: gtk::Application,
  instance: RefCell<Option<glib::Object>>,
  views: RefCell<Vec<View>>,
  preview: RefCell<Option<View>>,
  requested: Cell<bool>,
  secure: Cell<bool>,
  authenticating: Cell<bool>,
  failure: RefCell<String>,
  attempts: Cell<u32>,
  fingerprint: Cell<bool>,
  password_child: RefCell<Option<gio::Subprocess>>,
  fingerprint_child: RefCell<Option<gio::Subprocess>>,
  // Bumps on every lock/unlock, so callbacks from an older lock do nothing.
  generation: Cell<u64>,
  blank_at: Cell<Option<Instant>>,
  last_wake: Cell<Option<Instant>>,
  last_event: RefCell<(String, String)>,
}

impl Lock {
  pub fn new(app: &gtk::Application) -> Rc<Lock> {
    let me = Rc::new(Lock {
      app: app.clone(),
      instance: RefCell::default(),
      views: RefCell::default(),
      preview: RefCell::default(),
      requested: Cell::new(false),
      secure: Cell::new(false),
      authenticating: Cell::new(false),
      failure: RefCell::default(),
      attempts: Cell::new(0),
      fingerprint: Cell::new(false),
      password_child: RefCell::default(),
      fingerprint_child: RefCell::default(),
      generation: Cell::new(0),
      blank_at: Cell::new(None),
      last_wake: Cell::new(None),
      last_event: RefCell::new(("init".into(), String::new())),
    });
    // The blank countdown (5 s after locking or the last input).
    let weak = Rc::downgrade(&me);
    glib::timeout_add_local(Duration::from_millis(500), move || {
      let Some(me) = weak.upgrade() else { return glib::ControlFlow::Break };
      if let Some(at) = me.blank_at.get() {
        if Instant::now() >= at && me.requested.get() && !me.authenticating.get() {
          me.blank_at.set(None);
          BLANKED.with(|b| b.set(true));
          sh("omarchy-brightness-keyboard off; omarchy-brightness-display off");
        }
      }
      glib::ControlFlow::Continue
    });
    me.check_stranded();
    me
  }

  fn log(&self, event: &str) {
    let now = glib::DateTime::now_utc().ok().and_then(|d| d.format_iso8601().ok()).map(|s| s.to_string()).unwrap_or_default();
    glib::g_message!("doxia-shell", "lock: {event}");
    *self.last_event.borrow_mut() = (event.to_string(), now);
  }

  fn pam_configured() -> bool {
    std::path::Path::new("/etc/pam.d").join(PASSWORD_SERVICE).is_file()
  }

  pub fn locked(&self) -> bool {
    self.requested.get() || self.secure.get()
  }

  // ---- locking ----------------------------------------------------------------

  pub fn lock(self: &Rc<Self>) -> &'static str {
    if !Self::pam_configured() {
      self.log("lock-denied: missing-pam");
      return "missing-pam";
    }
    if self.locked() {
      return "ok";
    }
    if unsafe { gtk_session_lock_is_supported() } == 0 {
      self.log("lock-denied: no ext-session-lock");
      return "failed";
    }
    let generation = self.generation.get() + 1;
    self.generation.set(generation);
    self.reset_auth();
    self.requested.set(true);
    let _ = std::fs::write(marker(), "");
    self.log("lock-requested");
    self.hide_preview();

    let instance = unsafe { glib::Object::from_glib_full(gtk_session_lock_instance_new()) };
    let weak = Rc::downgrade(self);
    instance.connect_local("monitor", false, move |args| {
      let me = weak.upgrade()?;
      let monitor = args.get(1)?.get::<gdk::Monitor>().ok()?;
      me.add_monitor(&monitor);
      None
    });
    let weak = Rc::downgrade(self);
    instance.connect_local("locked", false, move |_| {
      let me = weak.upgrade()?;
      me.secure.set(true);
      me.log("secure=true");
      me.arm_blank();
      me.refresh_fingerprint();
      None
    });
    let weak = Rc::downgrade(self);
    instance.connect_local("failed", false, move |_| {
      let me = weak.upgrade()?;
      me.log("lock-failed");
      // Another client holds the lock: nothing of ours to show.
      me.requested.set(false);
      me.secure.set(false);
      let _ = std::fs::remove_file(marker());
      None
    });
    let weak = Rc::downgrade(self);
    instance.connect_local("unlocked", false, move |_| {
      let me = weak.upgrade()?;
      me.cleanup("unlocked");
      None
    });
    *self.instance.borrow_mut() = Some(instance.clone());
    let started = unsafe { gtk_session_lock_instance_lock(instance.to_glib_none().0) } != 0;
    if !started {
      self.cleanup("lock-failed: immediate");
      return "failed";
    }
    let _ = generation;
    "ok"
  }

  fn add_monitor(self: &Rc<Self>, monitor: &gdk::Monitor) {
    let Some(instance) = self.instance.borrow().clone() else { return };
    let view = self.build_view(true, Some(monitor));
    unsafe {
      gtk_session_lock_instance_assign_window_to_monitor(
        instance.to_glib_none().0,
        view.window.to_glib_none().0,
        monitor.to_glib_none().0,
      );
    }
    view.window.present();
    view.entry.grab_focus();
    self.views.borrow_mut().push(view);
    self.render();
  }

  // The unlock itself: the instance lets go, ::unlocked cleans up.
  fn unlock(self: &Rc<Self>) {
    let instance = self.instance.borrow().clone();
    match instance {
      Some(i) => unsafe { gtk_session_lock_instance_unlock(i.to_glib_none().0) },
      None => self.cleanup("unlocked"),
    }
    // The marker goes with the lock even if ::unlocked never comes.
    let _ = std::fs::remove_file(marker());
  }

  fn cleanup(self: &Rc<Self>, event: &str) {
    self.generation.set(self.generation.get() + 1);
    self.requested.set(false);
    self.secure.set(false);
    self.reset_auth();
    self.blank_at.set(None);
    let _ = std::fs::remove_file(marker());
    for view in self.views.borrow_mut().drain(..) {
      view.window.destroy();
    }
    self.instance.borrow_mut().take();
    self.log(event);
    self.wake(true);
  }

  // A lock the compositor still holds with no client behind it (doxia-shell
  // or Quickshell died locked): lock again so there is a field to unlock with.
  fn check_stranded(self: &Rc<Self>) {
    let me = self.clone();
    glib::spawn_future_local(async move {
      for _ in 0..20 {
        glib::timeout_future(Duration::from_millis(500)).await;
        if me.locked() {
          return;
        }
        let Ok(p) = gio::Subprocess::newv(&[OsStr::new("omarchy-mango-session-locked")], gio::SubprocessFlags::STDOUT_SILENCE)
        else {
          return;
        };
        if p.wait_future().await.is_err() {
          return;
        }
        match p.exit_status() {
          // Undetermined (outputs still coming up): ask again.
          2 => continue,
          0 if !me.locked() && Self::pam_configured() => {
            me.log("lock-stranded: recovering");
            me.lock();
            return;
          }
          _ => return,
        }
      }
    });
  }

  // ---- authentication --------------------------------------------------------

  fn reset_auth(&self) {
    self.authenticating.set(false);
    self.failure.borrow_mut().clear();
    self.attempts.set(0);
    for child in [&self.password_child, &self.fingerprint_child] {
      if let Some(p) = child.borrow_mut().take() {
        p.force_exit();
      }
    }
  }

  // The child runs this very binary: /proc/self/exe still works after an
  // update replaced the file on disk.
  fn pam_child(service: &str, with_stdin: bool) -> Option<gio::Subprocess> {
    let flags = if with_stdin { gio::SubprocessFlags::STDIN_PIPE } else { gio::SubprocessFlags::NONE }
      | gio::SubprocessFlags::STDOUT_SILENCE
      | gio::SubprocessFlags::STDERR_SILENCE;
    gio::Subprocess::newv(&[OsStr::new("/proc/self/exe"), OsStr::new("--pam"), OsStr::new(service)], flags).ok()
  }

  fn submit(self: &Rc<Self>, password: String) {
    if !self.requested.get() || self.authenticating.get() || password.is_empty() {
      return;
    }
    self.wake(true);
    self.failure.borrow_mut().clear();
    self.authenticating.set(true);
    self.render();
    let Some(child) = Self::pam_child(PASSWORD_SERVICE, true) else {
      self.password_failed();
      return;
    };
    *self.password_child.borrow_mut() = Some(child.clone());
    let generation = self.generation.get();
    let me = self.clone();
    glib::spawn_future_local(async move {
      if let Some(stdin) = child.stdin_pipe() {
        let _ = stdin.write_all_future(format!("{password}\n").into_bytes(), glib::Priority::DEFAULT).await;
        let _ = stdin.close_future(glib::Priority::DEFAULT).await;
      }
      let ok = child.wait_future().await.is_ok() && child.has_exited() && child.exit_status() == 0;
      if me.generation.get() != generation {
        return;
      }
      me.password_child.borrow_mut().take();
      me.authenticating.set(false);
      if ok {
        me.unlock();
      } else {
        me.password_failed();
      }
    });
  }

  fn password_failed(self: &Rc<Self>) {
    self.authenticating.set(false);
    self.attempts.set(self.attempts.get() + 1);
    *self.failure.borrow_mut() = format!("Falha na autenticação ({})", self.attempts.get());
    for view in self.views.borrow().iter() {
      view.entry.set_text("");
    }
    self.render();
    self.shake();
    self.wake(true);
  }

  // An enrolled finger unlocks too, while the field stays usable.
  fn refresh_fingerprint(self: &Rc<Self>) {
    let me = self.clone();
    glib::spawn_future_local(async move {
      let check = "[[ -f /etc/pam.d/omarchy-lock-fingerprint ]] && command -v fprintd-list >/dev/null && fprintd-list \"$USER\" 2>/dev/null | grep -qi finger";
      let ok = match gio::Subprocess::newv(
        &[OsStr::new("bash"), OsStr::new("-c"), OsStr::new(check)],
        gio::SubprocessFlags::STDOUT_SILENCE | gio::SubprocessFlags::STDERR_SILENCE,
      ) {
        Ok(p) => p.wait_future().await.is_ok() && p.exit_status() == 0,
        Err(_) => false,
      };
      me.fingerprint.set(ok);
      me.render();
      if ok {
        me.start_fingerprint();
      }
    });
  }

  fn start_fingerprint(self: &Rc<Self>) {
    if !self.secure.get() || !self.fingerprint.get() || self.fingerprint_child.borrow().is_some() {
      return;
    }
    let Some(child) = Self::pam_child(FINGERPRINT_SERVICE, false) else { return };
    *self.fingerprint_child.borrow_mut() = Some(child.clone());
    let generation = self.generation.get();
    let started = Instant::now();
    let me = self.clone();
    glib::spawn_future_local(async move {
      let ok = child.wait_future().await.is_ok() && child.has_exited() && child.exit_status() == 0;
      if me.generation.get() != generation {
        return;
      }
      me.fingerprint_child.borrow_mut().take();
      if ok {
        me.unlock();
        return;
      }
      // Try again; slowly when PAM fails right away, so a broken reader can't spin.
      let delay = if started.elapsed() < Duration::from_secs(1) { 2000 } else { 250 };
      glib::timeout_future(Duration::from_millis(delay)).await;
      if me.generation.get() == generation {
        me.start_fingerprint();
      }
    });
  }

  // ---- blank and wake ----------------------------------------------------------

  fn arm_blank(&self) {
    self.blank_at.set(Some(Instant::now() + Duration::from_secs(5)));
  }

  // Input wakes the displays; at most once a second unless forced.
  fn wake(&self, force: bool) {
    let recent = self.last_wake.get().is_some_and(|t| t.elapsed() < Duration::from_secs(1));
    BLANKED.with(|b| b.set(false));
    if force || !recent {
      self.last_wake.set(Some(Instant::now()));
      sh("omarchy-system-wake");
    }
    if self.requested.get() {
      self.arm_blank();
    }
  }

  // ---- views -------------------------------------------------------------------

  fn build_view(self: &Rc<Self>, input: bool, monitor: Option<&gdk::Monitor>) -> View {
    let window = gtk::Window::builder().application(&self.app).decorated(false).build();
    window.add_css_class("doxia");
    window.add_css_class("lock-window");
    let geometry = monitor.map(|m| m.geometry());
    let (mw, mh) = geometry.map(|g| (g.width() as f64, g.height() as f64)).unwrap_or((1366.0, 768.0));
    let left = (mw * 0.07).round() as i32;

    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&landscape_area()));

    // Top left: the brand, small.
    let scale = monitor.map(|m| m.scale_factor()).unwrap_or(1).max(1);
    let brand = gtk::Picture::new();
    if let Ok(pixbuf) = gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(brand_path(), -1, 26 * scale, true) {
      brand.set_paintable(Some(&gdk::Texture::for_pixbuf(&pixbuf)));
      brand.set_size_request(pixbuf.width() / scale, 26);
    }
    brand.set_can_shrink(true);
    brand.set_content_fit(gtk::ContentFit::Contain);
    brand.set_opacity(0.85);
    brand.set_halign(gtk::Align::Start);
    brand.set_valign(gtk::Align::Start);
    brand.set_margin_start(left);
    brand.set_margin_top(48);
    overlay.add_overlay(&brand);

    // Left: the clock and the date.
    let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
    column.set_halign(gtk::Align::Start);
    column.set_valign(gtk::Align::Start);
    column.set_margin_start(left);
    column.set_margin_top((mh * 0.3).round() as i32);
    let clock = label("", &["lock-clock"]);
    clock.set_xalign(0.0);
    let attrs = gtk::pango::AttrList::new();
    attrs.insert(gtk::pango::AttrSize::new_size_absolute((mh * 0.15).round() as i32 * gtk::pango::SCALE));
    clock.set_attributes(Some(&attrs));
    column.append(&clock);
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    bar.add_css_class("lock-accent");
    bar.set_size_request(56, 4);
    bar.set_halign(gtk::Align::Start);
    column.append(&bar);
    let date = label("", &["lock-date"]);
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

    // The sign-in, floating in the lower third.
    let sign_in = gtk::Box::new(gtk::Orientation::Vertical, 14);
    sign_in.set_size_request(380, -1);
    sign_in.set_halign(gtk::Align::Center);
    sign_in.set_valign(gtk::Align::End);
    sign_in.set_margin_bottom((mh * 0.1).round() as i32);
    let name = shown_name();
    sign_in.append(&avatar(&name));
    sign_in.append(&label(&name, &["lock-name"]));

    // The password on a thin line, the fingerprint at its left end and the
    // submit button at its right end.
    let line = gtk::Overlay::new();
    let field = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    field.add_css_class("lock-line");
    let entry = gtk::PasswordEntry::new();
    entry.add_css_class("lock-entry");
    entry.set_hexpand(true);
    entry.set_alignment(0.5);
    entry.set_placeholder_text(Some(PLACEHOLDER));
    entry.set_margin_start(44);
    entry.set_margin_end(44);
    // The preview shows the field but takes no typing.
    entry.set_editable(input);
    entry.set_can_focus(input);
    field.append(&entry);
    line.set_child(Some(&field));
    let hint = label(G_FINGERPRINT, &["lock-fingerprint"]);
    hint.set_halign(gtk::Align::Start);
    hint.set_valign(gtk::Align::Center);
    hint.set_margin_start(8);
    hint.set_visible(false);
    line.add_overlay(&hint);
    let submit = gtk::Button::with_label("→");
    submit.add_css_class("lock-submit");
    submit.set_halign(gtk::Align::End);
    submit.set_valign(gtk::Align::Center);
    submit.set_can_focus(false);
    line.add_overlay(&submit);
    sign_in.append(&line);
    let status = label("", &["lock-status"]);
    status.set_size_request(-1, 16);
    sign_in.append(&status);
    overlay.add_overlay(&sign_in);

    // Bottom left: the machine's name.
    let host = label(&glib::host_name(), &["lock-host"]);
    host.set_halign(gtk::Align::Start);
    host.set_valign(gtk::Align::End);
    host.set_margin_start(left);
    host.set_margin_bottom(30);
    overlay.add_overlay(&host);
    window.set_child(Some(&overlay));

    if input {
      let weak = Rc::downgrade(self);
      let e = entry.clone();
      submit.connect_clicked(move |_| {
        let Some(me) = weak.upgrade() else { return };
        let text = e.text().to_string();
        e.set_text("");
        me.submit(text);
      });
      let weak = Rc::downgrade(self);
      entry.connect_activate(move |e| {
        let Some(me) = weak.upgrade() else { return };
        let text = e.text().to_string();
        e.set_text("");
        me.submit(text);
      });
      let weak = Rc::downgrade(self);
      entry.connect_changed(move |e| {
        let Some(me) = weak.upgrade() else { return };
        if !e.text().is_empty() {
          me.wake(false);
          if !me.failure.borrow().is_empty() {
            me.failure.borrow_mut().clear();
            me.render();
          }
        }
      });
      // Escape and Ctrl+U clear the field; any key wakes the displays.
      let keys = gtk::EventControllerKey::new();
      keys.set_propagation_phase(gtk::PropagationPhase::Capture);
      let weak = Rc::downgrade(self);
      let e = entry.clone();
      keys.connect_key_pressed(move |_, key, _, mods| {
        if let Some(me) = weak.upgrade() {
          me.wake(false);
        }
        let ctrl_u = mods.contains(gdk::ModifierType::CONTROL_MASK) && key.to_lower() == gdk::Key::u;
        if key == gdk::Key::Escape || ctrl_u {
          e.set_text("");
          return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
      });
      window.add_controller(keys);
      let motion = gtk::EventControllerMotion::new();
      let weak = Rc::downgrade(self);
      motion.connect_motion(move |_, _, _| {
        if let Some(me) = weak.upgrade() {
          me.wake(false);
        }
      });
      window.add_controller(motion);
      let click = gtk::GestureClick::new();
      let e = entry.clone();
      click.connect_released(move |_, _, _, _| {
        e.grab_focus();
      });
      window.add_controller(click);
    }
    View { window, entry, field, hint, status, sign_in }
  }

  // A wrong password shakes the sign-in, as on the login screen.
  fn shake(&self) {
    let boxes: Vec<_> = self.views.borrow().iter().map(|v| v.sign_in.downgrade()).collect();
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

  fn render(&self) {
    let failed = !self.failure.borrow().is_empty();
    let placeholder = if self.authenticating.get() { "Verificando…" } else { PLACEHOLDER };
    for view in self.views.borrow().iter() {
      view.entry.set_placeholder_text(Some(placeholder));
      view.entry.set_editable(!self.authenticating.get());
      view.hint.set_visible(self.fingerprint.get());
      view.status.set_text(if failed { FAILED } else { "" });
      if failed {
        view.field.add_css_class("error");
      } else {
        view.field.remove_css_class("error");
      }
    }
  }

  // The lock screen without locking (theme previews): any click or key ends it.
  pub fn show_preview(self: &Rc<Self>) {
    if self.preview.borrow().is_some() || self.locked() {
      return;
    }
    let first = gdk::Display::default().and_then(|d| d.monitors().item(0)).and_then(|m| m.downcast::<gdk::Monitor>().ok());
    let view = self.build_view(false, first.as_ref());
    view.window.init_layer_shell();
    view.window.set_namespace(Some("doxia-lock-preview"));
    view.window.set_layer(Layer::Overlay);
    view.window.set_keyboard_mode(KeyboardMode::Exclusive);
    view.window.set_exclusive_zone(-1);
    for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
      view.window.set_anchor(edge, true);
    }
    view.hint.set_visible(self.fingerprint.get());
    let weak = Rc::downgrade(self);
    let click = gtk::GestureClick::new();
    click.set_button(0);
    click.connect_released(move |_, _, _, _| {
      if let Some(me) = weak.upgrade() {
        me.hide_preview();
      }
    });
    view.window.add_controller(click);
    let weak = Rc::downgrade(self);
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(move |_, _, _, _| {
      if let Some(me) = weak.upgrade() {
        me.hide_preview();
      }
      glib::Propagation::Stop
    });
    view.window.add_controller(keys);
    view.window.present();
    *self.preview.borrow_mut() = Some(view);
  }

  pub fn hide_preview(&self) {
    if let Some(view) = self.preview.borrow_mut().take() {
      view.window.destroy();
    }
  }

  // ---- IPC (omarchy-shell lock <method>) ------------------------------------------

  pub fn ipc(self: &Rc<Self>, method: &str) -> Option<String> {
    Some(match method {
      "ping" => "ok".into(),
      "lock" => self.lock().into(),
      "isLocked" => self.locked().to_string(),
      "status" => {
        let (event, at) = self.last_event.borrow().clone();
        serde_json::json!({
          "locked": self.locked(),
          "requested": self.requested.get(),
          "pending": self.requested.get() && !self.secure.get(),
          "sessionLocked": self.secure.get(),
          "secure": self.secure.get(),
          "realScreens": gdk::Display::default().map(|d| d.monitors().n_items()).unwrap_or(0),
          "passwordPam": Self::pam_configured(),
          "fingerprint": self.fingerprint.get(),
          "authenticating": self.authenticating.get() || self.fingerprint_child.borrow().is_some(),
          "lastEvent": event,
          "lastEventAt": at,
        })
        .to_string()
      }
      "preview" => {
        self.show_preview();
        "ok".into()
      }
      "hidePreview" => {
        self.hide_preview();
        "ok".into()
      }
      _ => return None,
    })
  }
}
