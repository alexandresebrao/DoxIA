// doxia-greeter: the DoxIA login screen, a greetd greeter drawn with the lock
// screen's pieces (the doxia-login crate), so signing in and unlocking look
// the same. greetd runs it as its own user inside a bare mango
// (default/greetd/doxia-greeter-session); signing in tells greetd which
// session to start, then the greeter exits and mango with it.
//
// Like a game's title screen: the animated landscape, the brand and a large
// clock on the left, and the sign-in in the lower third with no card: the
// account (arrows switch between accounts), the password on a thin line, then
// "Outro usuário" and the session; the power actions in the bottom right
// corner. The other monitors show the landscape and the clock.
//
//   doxia-greeter              under greetd ($GREETD_SOCK)
//   doxia-greeter --windowed   in a plain window, to look at it in a session
//   doxia-greeter [--windowed] --snapshot <png> [keys]
//                              saves the first monitor's screen to a PNG once
//                              it settled (after `keys`, comma-separated: text,
//                              Tab, Escape, Left, Right, Up, Down, Other,
//                              Enter), then exits

mod accounts;
mod greetd;
mod power;

use doxia_login::landscape;
use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

const MENU_WIDTH: i32 = 260;

// The greeter's pictures (the brand, the power icon).
fn data_dir() -> PathBuf {
  std::env::var_os("DOXIA_GREETER_DATA").map(PathBuf::from).unwrap_or_else(|| "/usr/share/doxia/greeter".into())
}

fn link(text: &str) -> gtk::Button {
  let b = gtk::Button::with_label(text);
  b.add_css_class("login-link");
  b.set_can_focus(false);
  b.set_cursor_from_name(Some("pointer"));
  b
}

// The sign-in, on the first monitor only.
#[derive(Clone)]
struct SignIn {
  column: gtk::Box,
  avatar: gtk::DrawingArea,
  account: gtk::Box,
  arrows: [gtk::Button; 2],
  name: gtk::Label,
  manual_line: gtk::Overlay,
  manual: gtk::Entry,
  password: doxia_login::PasswordLine,
  status: gtk::Label,
  other: gtk::Button,
  session: gtk::Button,
  scrim: gtk::Box,
  menu: gtk::Box,
}

struct Greeter {
  main_loop: glib::MainLoop,
  windowed: bool,
  users: Vec<accounts::User>,
  sessions: Vec<accounts::Session>,
  power: Vec<power::Action>,
  user: Cell<usize>,
  session: Cell<usize>,
  manual: Cell<bool>,
  failed: Cell<bool>,
  busy: Cell<bool>,
  menu_open: Cell<bool>,
  error: RefCell<String>,
  sky: Rc<Cell<Option<landscape::Sky>>>,
  windows: RefCell<Vec<gtk::Window>>,
  sign_in: RefCell<Option<SignIn>>,
}

impl Greeter {
  fn new(main_loop: glib::MainLoop, windowed: bool) -> Rc<Greeter> {
    let users = accounts::users();
    let sessions = accounts::sessions();
    let state = accounts::load_state();
    let user = users.iter().position(|u| u.name == state.user).unwrap_or(0);
    let wanted = if state.session.is_empty() { accounts::DEFAULT_SESSION } else { &state.session };
    let session = sessions
      .iter()
      .position(|s| s.path.as_os_str() == wanted)
      .or_else(|| sessions.iter().position(|s| s.path.as_os_str() == accounts::DEFAULT_SESSION))
      .unwrap_or(0);
    Rc::new(Greeter {
      main_loop,
      windowed,
      users,
      sessions,
      power: power::available(),
      user: Cell::new(user),
      session: Cell::new(session),
      manual: Cell::new(false),
      failed: Cell::new(false),
      busy: Cell::new(false),
      menu_open: Cell::new(false),
      error: RefCell::default(),
      sky: Rc::default(),
      windows: RefCell::default(),
      sign_in: RefCell::default(),
    })
  }

  fn selected_user(&self) -> Option<&accounts::User> {
    self.users.get(self.user.get())
  }

  // ---- windows -----------------------------------------------------------------

  // One window per monitor, made again whenever the monitors change.
  fn build(self: &Rc<Self>) {
    for w in self.windows.borrow_mut().drain(..) {
      w.destroy();
    }
    self.sign_in.borrow_mut().take();
    let Some(display) = gdk::Display::default() else { return };
    let monitors: Vec<gdk::Monitor> =
      display.monitors().iter::<glib::Object>().filter_map(|m| m.ok()?.downcast::<gdk::Monitor>().ok()).collect();
    if self.windowed || monitors.is_empty() {
      // Laid out for the first monitor, at its size.
      let first = monitors.first();
      let window = self.window(first, true);
      if let (Some(m), Some(child)) = (first, window.child()) {
        child.set_size_request(m.geometry().width(), m.geometry().height());
      }
      window.present();
      self.windows.borrow_mut().push(window);
    } else {
      for (i, monitor) in monitors.iter().enumerate() {
        let window = self.window(Some(monitor), i == 0);
        window.init_layer_shell();
        window.set_namespace(Some("doxia-greeter"));
        window.set_layer(Layer::Top);
        window.set_monitor(Some(monitor));
        window.set_exclusive_zone(-1);
        window.set_keyboard_mode(if i == 0 { KeyboardMode::Exclusive } else { KeyboardMode::None });
        for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
          window.set_anchor(edge, true);
        }
        window.present();
        self.windows.borrow_mut().push(window);
      }
    }
    self.render();
    self.focus();
  }

  fn window(self: &Rc<Self>, monitor: Option<&gdk::Monitor>, with_sign_in: bool) -> gtk::Window {
    let window = gtk::Window::builder().decorated(false).title("DoxIA").build();
    window.add_css_class("login-window");
    let sky = self.sky.clone();
    let area = doxia_login::landscape_area(move || sky.get(), || false);
    let backdrop = doxia_login::backdrop(monitor, &data_dir().join("brand.png"), area);
    let overlay = backdrop.overlay;
    window.set_child(Some(&overlay));
    if !with_sign_in {
      return window;
    }
    let width = monitor.map(|m| m.geometry().width() as f64).unwrap_or(1366.0);
    if !self.power.is_empty() {
      overlay.add_overlay(&self.power_row(width));
    }
    let sign_in = self.sign_in_widgets(backdrop.height);
    overlay.add_overlay(&sign_in.column);
    overlay.add_overlay(&sign_in.scrim);
    overlay.add_overlay(&sign_in.menu);
    *self.sign_in.borrow_mut() = Some(sign_in);
    window
  }

  fn power_row(self: &Rc<Self>, width: f64) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    row.set_halign(gtk::Align::End);
    row.set_valign(gtk::Align::End);
    row.set_margin_end((width * 0.04).round() as i32);
    row.set_margin_bottom(24);
    for action in &self.power {
      let (text, icon) = match action {
        power::Action::Suspend => ("Suspender", false),
        power::Action::Reboot => ("Reiniciar", false),
        power::Action::PowerOff => ("Desligar", true),
      };
      let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
      if icon {
        let image = gtk::Image::from_file(data_dir().join("power.png"));
        image.set_pixel_size(16);
        content.append(&image);
      }
      content.append(&gtk::Label::new(Some(text)));
      let button = gtk::Button::new();
      button.set_child(Some(&content));
      button.add_css_class("login-power");
      button.set_can_focus(false);
      button.set_cursor_from_name(Some("pointer"));
      let action = *action;
      button.connect_clicked(move |_| power::run(action));
      row.append(&button);
    }
    row
  }

  fn sign_in_widgets(self: &Rc<Self>, height: f64) -> SignIn {
    let column = doxia_login::sign_in(height);

    // The account, with arrows when there is more than one.
    let account = gtk::Box::new(gtk::Orientation::Horizontal, 28);
    account.set_halign(gtk::Align::Center);
    let avatar = doxia_login::avatar("", None);
    let arrows = ["‹", "›"].map(|text| {
      let b = gtk::Button::with_label(text);
      b.add_css_class("login-arrow");
      b.set_valign(gtk::Align::Center);
      b.set_can_focus(false);
      b.set_cursor_from_name(Some("pointer"));
      b
    });
    account.append(&arrows[0]);
    account.append(&avatar);
    account.append(&arrows[1]);
    for (button, step) in arrows.iter().zip([-1, 1]) {
      let weak = Rc::downgrade(self);
      button.connect_clicked(move |_| {
        if let Some(me) = weak.upgrade() {
          me.switch_user(step);
        }
      });
    }
    column.append(&account);
    let name = doxia_login::label("", &["login-name"]);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    column.append(&name);

    // Someone not in the list: the name on its own line.
    let manual = gtk::Entry::new();
    manual.add_css_class("login-entry");
    manual.add_css_class("name");
    gtk::prelude::EditableExt::set_alignment(&manual, 0.5);
    manual.set_placeholder_text(Some("Nome de usuário"));
    let (manual_line, _) = doxia_login::line(&manual);
    column.append(&manual_line);

    let password = doxia_login::password_line();
    column.append(&password.line);
    let status = doxia_login::label("", &["login-status"]);
    status.set_size_request(-1, 16);
    status.set_wrap(true);
    status.set_justify(gtk::Justification::Center);
    column.append(&status);

    // "Outro usuário" · the session.
    let links = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    links.set_halign(gtk::Align::Center);
    let other = link("Outro usuário");
    let session = link("");
    links.append(&other);
    links.append(&doxia_login::label("·", &["login-dot"]));
    links.append(&session);
    column.append(&links);

    // The session menu, over the sign-in; a click anywhere else closes it.
    let scrim = gtk::Box::new(gtk::Orientation::Vertical, 0);
    scrim.set_hexpand(true);
    scrim.set_vexpand(true);
    scrim.set_visible(false);
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
    menu.add_css_class("login-menu");
    menu.set_size_request(MENU_WIDTH, -1);
    menu.set_halign(gtk::Align::Center);
    menu.set_valign(gtk::Align::End);
    menu.set_margin_bottom(column.margin_bottom() + 40);
    menu.set_visible(false);
    for (i, s) in self.sessions.iter().enumerate() {
      // The check mark in a column of its own, so the names line up.
      let content = gtk::Box::new(gtk::Orientation::Horizontal, 0);
      let check = gtk::Label::new(Some("✓"));
      check.set_size_request(22, -1);
      check.set_xalign(0.0);
      content.append(&check);
      content.append(&gtk::Label::new(Some(&s.name)));
      let row = gtk::Button::new();
      row.set_child(Some(&content));
      row.add_css_class("login-menu-row");
      row.set_can_focus(false);
      row.set_cursor_from_name(Some("pointer"));
      let weak = Rc::downgrade(self);
      row.connect_clicked(move |_| {
        if let Some(me) = weak.upgrade() {
          me.session.set(i);
          me.menu_open.set(false);
          me.render();
          me.focus();
        }
      });
      menu.append(&row);
    }

    let s = SignIn { column, avatar, account, arrows, name, manual_line, manual, password, status, other, session, scrim, menu };
    self.connect(&s);
    s
  }

  fn connect(self: &Rc<Self>, s: &SignIn) {
    let weak = Rc::downgrade(self);
    let on = move |f: fn(&Rc<Greeter>)| {
      let weak = weak.clone();
      move || {
        if let Some(me) = weak.upgrade() {
          f(&me);
        }
      }
    };
    let login = on(|me| me.login());
    s.password.entry.connect_activate(move |_| login());
    let login = on(|me| me.login());
    s.password.submit.connect_clicked(move |_| login());
    let weak = Rc::downgrade(self);
    s.password.entry.connect_changed(move |e| {
      let Some(me) = weak.upgrade() else { return };
      if !e.text().is_empty() && (me.failed.get() || !me.error.borrow().is_empty()) {
        me.failed.set(false);
        me.error.borrow_mut().clear();
        me.render();
      }
    });
    let toggle = on(|me| me.toggle_manual());
    s.other.connect_clicked(move |_| toggle());
    let menu = on(|me| {
      me.menu_open.set(!me.menu_open.get());
      me.render();
      me.focus();
    });
    s.session.connect_clicked(move |_| menu());
    let close = on(|me| {
      me.menu_open.set(false);
      me.render();
      me.focus();
    });
    let click = gtk::GestureClick::new();
    click.connect_released(move |_, _, _, _| close());
    s.scrim.add_controller(click);

    // The name typed in: Enter goes on to the password, Escape back to the list.
    let password = s.password.entry.clone();
    s.manual.connect_activate(move |_| {
      password.grab_focus();
    });
    let keys = gtk::EventControllerKey::new();
    let back = on(|me| me.back_to_list());
    keys.connect_key_pressed(move |_, key, _, _| {
      if key == gdk::Key::Escape {
        back();
        return glib::Propagation::Stop;
      }
      glib::Propagation::Proceed
    });
    s.manual.add_controller(keys);

    // In the password: Escape clears (or closes the menu, or goes back to the
    // list), Tab opens the session menu and the arrows move in it, and with
    // the field empty Left/Right switch accounts.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(self);
    keys.connect_key_pressed(move |_, key, _, _| match weak.upgrade() {
      Some(me) => me.key(key),
      None => glib::Propagation::Proceed,
    });
    s.password.entry.add_controller(keys);
  }

  fn key(self: &Rc<Self>, key: gdk::Key) -> glib::Propagation {
    let Some(entry) = self.sign_in.borrow().as_ref().map(|s| s.password.entry.clone()) else {
      return glib::Propagation::Proceed;
    };
    let empty = entry.text().is_empty();
    let open = self.menu_open.get();
    let last = self.sessions.len().saturating_sub(1);
    match key {
      gdk::Key::Escape if open => self.menu_open.set(false),
      gdk::Key::Escape if self.manual.get() => self.back_to_list(),
      gdk::Key::Escape => entry.set_text(""),
      gdk::Key::Tab | gdk::Key::ISO_Left_Tab => self.menu_open.set(!open),
      gdk::Key::Up if open => self.session.set(self.session.get().saturating_sub(1)),
      gdk::Key::Down if open => self.session.set((self.session.get() + 1).min(last)),
      gdk::Key::Left if !self.manual.get() && empty => self.switch_user(-1),
      gdk::Key::Right if !self.manual.get() && empty => self.switch_user(1),
      gdk::Key::Return | gdk::Key::KP_Enter => {
        self.menu_open.set(false);
        self.render();
        return glib::Propagation::Proceed;
      }
      _ => return glib::Propagation::Proceed,
    }
    self.render();
    glib::Propagation::Stop
  }

  // ---- state -------------------------------------------------------------------

  fn switch_user(&self, step: i32) {
    let n = self.users.len() as i32;
    if n < 2 {
      return;
    }
    self.user.set((self.user.get() as i32 + step).rem_euclid(n) as usize);
    self.manual.set(false);
    self.failed.set(false);
    self.error.borrow_mut().clear();
    if let Some(s) = self.sign_in.borrow().as_ref() {
      s.password.entry.set_text("");
    }
    self.render();
    self.focus();
  }

  fn toggle_manual(&self) {
    if self.manual.get() {
      self.back_to_list();
      return;
    }
    self.manual.set(true);
    self.failed.set(false);
    self.error.borrow_mut().clear();
    if let Some(s) = self.sign_in.borrow().as_ref() {
      s.password.entry.set_text("");
      s.manual.set_text("");
    }
    self.render();
    self.focus();
  }

  fn back_to_list(&self) {
    self.manual.set(false);
    self.menu_open.set(false);
    self.failed.set(false);
    self.error.borrow_mut().clear();
    if let Some(s) = self.sign_in.borrow().as_ref() {
      s.password.entry.set_text("");
    }
    self.render();
    self.focus();
  }

  fn focus(&self) {
    if let Some(s) = self.sign_in.borrow().as_ref() {
      if self.manual.get() && s.manual.text().is_empty() {
        s.manual.grab_focus();
      } else {
        s.password.entry.grab_focus();
      }
    }
  }

  fn render(&self) {
    let guard = self.sign_in.borrow();
    let Some(s) = guard.as_ref() else { return };
    let manual = self.manual.get();
    let user = self.selected_user();
    let shown = user.map(|u| u.shown().to_string()).unwrap_or_default();
    doxia_login::set_avatar(&s.avatar, &shown, user.and_then(|u| u.face.as_deref()));
    s.account.set_visible(!manual && user.is_some());
    s.name.set_text(&shown);
    s.name.set_visible(!manual && user.is_some());
    for a in &s.arrows {
      a.set_opacity(if self.users.len() > 1 { 1.0 } else { 0.0 });
      a.set_sensitive(self.users.len() > 1);
    }
    s.manual_line.set_visible(manual || user.is_none());
    let busy = self.busy.get();
    s.password.entry.set_placeholder_text(Some(if busy { "Verificando…" } else { doxia_login::PLACEHOLDER }));
    s.password.entry.set_editable(!busy);
    s.manual.set_editable(!busy);
    let error = self.error.borrow();
    let status = if self.failed.get() {
      doxia_login::FAILED.to_string()
    } else if !error.is_empty() {
      format!("Não foi possível entrar: {error}")
    } else {
      String::new()
    };
    s.status.set_text(&status);
    doxia_login::set_failed(&s.password.field, self.failed.get());
    s.other.set_label(if manual { "Voltar às contas" } else { "Outro usuário" });
    let session = self.sessions.get(self.session.get()).map(|s| s.name.as_str()).unwrap_or("nenhuma");
    s.session.set_label(&format!("Sessão: {session}  ▾"));
    let open = self.menu_open.get();
    if open {
      s.session.add_css_class("active");
    } else {
      s.session.remove_css_class("active");
    }
    s.scrim.set_visible(open);
    s.menu.set_visible(open);
    let mut row = s.menu.first_child();
    let mut i = 0;
    while let Some(r) = row {
      if let Some(check) = r.first_child().and_then(|content| content.first_child()) {
        check.set_opacity(if i == self.session.get() { 1.0 } else { 0.0 });
      }
      row = r.next_sibling();
      i += 1;
    }
  }

  // ---- signing in ----------------------------------------------------------------

  fn login(self: &Rc<Self>) {
    if self.busy.get() {
      return;
    }
    let Some((password, typed)) = self.sign_in.borrow().as_ref().map(|s| (s.password.entry.text().to_string(), s.manual.text().trim().to_string()))
    else {
      return;
    };
    let manual = self.manual.get() || self.users.is_empty();
    let (name, shell) = match (manual, self.selected_user()) {
      (false, Some(u)) => (u.name.clone(), u.shell.clone()),
      _ => (typed.clone(), accounts::shell_of(&typed)),
    };
    if name.is_empty() {
      return;
    }
    let Some(session) = self.sessions.get(self.session.get()).cloned() else {
      *self.error.borrow_mut() = "nenhuma sessão instalada".into();
      self.render();
      return;
    };
    self.busy.set(true);
    self.failed.set(false);
    self.error.borrow_mut().clear();
    self.menu_open.set(false);
    self.render();
    let me = self.clone();
    glib::spawn_future_local(async move {
      let (user, path) = (name.clone(), session.path.clone());
      let (cmd, env) = (session.command(&shell), session.env());
      let outcome = gio::spawn_blocking(move || greetd::login(&name, &password, &cmd, &env))
        .await
        .unwrap_or_else(|_| greetd::Outcome::Failed("o greeter falhou".into()));
      me.busy.set(false);
      match outcome {
        greetd::Outcome::Started => {
          accounts::save_state(&user, &path);
          me.main_loop.quit();
          return;
        }
        greetd::Outcome::Refused => {
          me.failed.set(true);
          if let Some(s) = me.sign_in.borrow().as_ref() {
            doxia_login::shake(vec![s.column.downgrade()]);
          }
        }
        greetd::Outcome::Failed(e) => {
          glib::g_warning!("doxia-greeter", "login: {e}");
          *me.error.borrow_mut() = e;
        }
      }
      if let Some(s) = me.sign_in.borrow().as_ref() {
        s.password.entry.set_text("");
      }
      me.render();
      me.focus();
    });
  }
}

// wttr.in's current conditions (located by IP), every 20 minutes; every
// minute until the network is up. Without them the sky stays clear.
fn fetch_weather(sky: Rc<Cell<Option<landscape::Sky>>>) {
  glib::spawn_future_local(async move {
    loop {
      let found = match gio::Subprocess::newv(
        &["curl", "-sf", "--max-time", "15", "https://wttr.in/?format=j1&lang=pt"].map(std::ffi::OsStr::new),
        gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE,
      ) {
        Ok(p) => p
          .communicate_utf8_future(None)
          .await
          .ok()
          .and_then(|(out, _)| serde_json::from_str::<serde_json::Value>(out?.as_str()).ok())
          .and_then(|v| landscape::sky_of(&v["current_condition"][0])),
        Err(_) => None,
      };
      let wait = if found.is_some() { 20 * 60 } else { 60 };
      if found.is_some() {
        sky.set(found);
      }
      glib::timeout_future(Duration::from_secs(wait)).await;
    }
  });
}

// The window as drawn, for --snapshot (screenshots don't work everywhere).
fn snapshot(window: &gtk::Window, path: &str) {
  let Some(child) = window.child() else { return };
  // A layer surface's window reports no size of its own.
  let (w, h) = (child.width(), child.height());
  // Lay it out now: a window off screen (a locked session) gets no frames.
  child.size_allocate(&gtk::Allocation::new(0, 0, w, h), -1);
  let snap = gtk::Snapshot::new();
  window.snapshot_child(&child, &snap);
  let Some(node) = snap.to_node() else {
    eprintln!("doxia-greeter: nothing to snapshot ({w}x{h})");
    return;
  };
  // Its own renderer: the window's goes away whenever it is not on screen.
  let renderer = gtk::gsk::CairoRenderer::new();
  if let Err(e) = renderer.realize(None) {
    eprintln!("doxia-greeter: {e}");
    return;
  }
  let texture = renderer.render_texture(node, Some(&gtk::graphene::Rect::new(0.0, 0.0, w as f32, h as f32)));
  renderer.unrealize();
  if let Err(e) = texture.save_to_png(path) {
    eprintln!("doxia-greeter: {e}");
  }
}

fn main() -> glib::ExitCode {
  let args: Vec<String> = std::env::args().collect();
  let snapshot_to = args.iter().position(|a| a == "--snapshot").and_then(|i| args.get(i + 1).cloned());
  let typed = snapshot_to.as_ref().and_then(|_| args.iter().position(|a| a == "--snapshot")).and_then(|i| args.get(i + 2).cloned());
  let windowed = args.iter().any(|a| a == "--windowed");
  if let Err(e) = gtk::init() {
    eprintln!("doxia-greeter: {e}");
    return glib::ExitCode::FAILURE;
  }
  doxia_login::install_css();
  let main_loop = glib::MainLoop::new(None, false);
  let greeter = Greeter::new(main_loop.clone(), windowed);
  fetch_weather(greeter.sky.clone());
  greeter.build();
  if let Some(display) = gdk::Display::default() {
    let weak = Rc::downgrade(&greeter);
    display.monitors().connect_items_changed(move |_, _, _, _| {
      let weak = weak.clone();
      // Once the change settles (a monitor comes with its geometry later).
      glib::timeout_add_local_once(Duration::from_millis(300), move || {
        if let Some(me) = weak.upgrade() {
          me.build();
        }
      });
    });
  }
  if windowed {
    if let Some(w) = greeter.windows.borrow().first() {
      let l = main_loop.clone();
      w.connect_close_request(move |_| {
        l.quit();
        glib::Propagation::Proceed
      });
    }
  }
  if let Some(path) = snapshot_to {
    let g = greeter.clone();
    let l = main_loop.clone();
    glib::timeout_add_local_once(Duration::from_millis(1500), move || {
      if let Some(keys) = typed {
        for key in keys.split(',') {
          match key {
            "Tab" | "Escape" | "Left" | "Right" | "Down" | "Up" => {
              g.key(gdk::Key::from_name(key).unwrap_or(gdk::Key::VoidSymbol));
            }
            "Other" => g.toggle_manual(),
            "Enter" => g.login(),
            text => {
              if let Some(s) = g.sign_in.borrow().as_ref() {
                if g.manual.get() && s.manual.text().is_empty() {
                  s.manual.set_text(text);
                } else {
                  s.password.entry.set_text(text);
                }
              }
            }
          }
        }
      }
      let (g, l) = (g.clone(), l.clone());
      glib::timeout_add_local_once(Duration::from_millis(1200), move || {
        if let Some(w) = g.windows.borrow().first() {
          snapshot(w, &path);
        }
        l.quit();
      });
    });
  }
  main_loop.run();
  glib::ExitCode::SUCCESS
}
