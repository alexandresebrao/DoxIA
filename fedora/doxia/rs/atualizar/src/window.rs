// The window: same views, texts and actions as the shell plugin it replaces
// (shell/plugins/update/UpdateWindow.qml). The work is all in bin/: the window
// starts omarchy-update-run and follows the JSON files in model.rs. Being its
// own process, it stays open while the update restarts or reloads the shell.

use crate::model::{self, Value};
use doxia_theme::Styler;
use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use std::cell::RefCell;
use std::ffi::OsStr;
use std::rc::Rc;
use std::time::{Duration, Instant};

const G_COG: &str = "\u{f013}";
const G_CHECK: &str = "\u{f00c}";
const G_WARN: &str = "\u{f071}";
const G_REFRESH: &str = "\u{f021}";
const G_LOCK: &str = "\u{f023}";
const G_CUBE: &str = "\u{f1b2}";
const G_ARCHIVE: &str = "\u{f187}";
const G_BRANCH: &str = "\u{f126}";
const G_POWER: &str = "\u{f011}";
const G_TIMES: &str = "\u{f00d}";

#[derive(Clone, Copy, PartialEq, Debug)]
enum View {
  Idle,
  Password,
  Running,
  Staged,
  Result,
}

struct Paths {
  status: String,
  run: String,
  last: String,
  log: String,
}

struct State {
  status: Option<Value>,
  run: Option<Value>,
  last: Option<Value>,
  prefs: Value,
  checking: bool,
  prefs_tab: bool,
  log_open: bool,
  log_tail: Vec<String>,
  starting_until: Option<Instant>,
  rendered: String,
}

pub struct Window {
  window: gtk::ApplicationWindow,
  header_icon: gtk::Label,
  title: gtk::Label,
  subtitle: gtk::Label,
  tab_updates: gtk::Button,
  tab_prefs: gtk::Button,
  body: gtk::Box,
  footer: gtk::Box,
  footer_rule: gtk::Box,
  password: gtk::PasswordEntry,
  styler: Styler,
  paths: Paths,
  state: RefCell<State>,
}

fn label(text: &str, classes: &[&str]) -> gtk::Label {
  let l = gtk::Label::new(Some(text));
  l.set_xalign(0.0);
  for c in classes {
    l.add_css_class(c);
  }
  l
}

fn vbox(spacing: i32) -> gtk::Box {
  gtk::Box::new(gtk::Orientation::Vertical, spacing)
}

fn hbox(spacing: i32) -> gtk::Box {
  gtk::Box::new(gtk::Orientation::Horizontal, spacing)
}

fn clear(b: &gtk::Box) {
  while let Some(child) = b.first_child() {
    b.remove(&child);
  }
}

fn on_click(widget: &impl IsA<gtk::Widget>, f: impl Fn() + 'static) {
  let click = gtk::GestureClick::new();
  click.connect_released(move |g, _, _, _| {
    g.set_state(gtk::EventSequenceState::Claimed);
    f();
  });
  widget.add_controller(click);
  widget.set_cursor_from_name(Some("pointer"));
}

// Fire and forget, but reaped: argv straight to exec, no shell.
fn spawn(argv: &[&str]) {
  let args: Vec<&OsStr> = argv.iter().map(OsStr::new).collect();
  if let Ok(p) = gio::Subprocess::newv(&args, gio::SubprocessFlags::NONE) {
    p.wait_async(gio::Cancellable::NONE, |_| {});
  }
}

async fn output(argv: &[&str]) -> Option<String> {
  let args: Vec<&OsStr> = argv.iter().map(OsStr::new).collect();
  let p = gio::Subprocess::newv(&args, gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE).ok()?;
  let (out, _) = p.communicate_utf8_future(None).await.ok()?;
  out.map(|s| s.to_string())
}

fn tail(path: &str, bytes: u64) -> String {
  use std::io::{Read, Seek, SeekFrom};
  let Ok(mut f) = std::fs::File::open(path) else { return String::new() };
  let len = f.metadata().map(|m| m.len()).unwrap_or(0);
  let _ = f.seek(SeekFrom::Start(len.saturating_sub(bytes)));
  let mut buf = Vec::new();
  let _ = f.read_to_end(&mut buf);
  String::from_utf8_lossy(&buf).into_owned()
}

impl Window {
  pub fn new(app: &gtk::Application) -> Rc<Window> {
    let home = std::env::var("HOME").unwrap_or_default();
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let paths = Paths {
      status: format!("{home}/.cache/omarchy/update-status.json"),
      run: format!("{runtime}/omarchy-update/state.json"),
      last: format!("{home}/.local/state/omarchy/update-last.json"),
      log: "/tmp/omarchy-update.log".into(),
    };

    let window = gtk::ApplicationWindow::builder()
      .application(app)
      .title("Atualizar DoxIA")
      .default_width(560)
      .default_height(620)
      .build();
    window.set_size_request(480, 520);
    window.add_css_class("doxia");
    let styler = Styler::install(include_str!("style.css"));

    // Header: icon, title, tabs
    let header = hbox(12);
    header.add_css_class("header");
    let icon_box = gtk::CenterBox::new();
    icon_box.add_css_class("header-icon");
    icon_box.set_valign(gtk::Align::Center);
    let header_icon = label(G_REFRESH, &["glyph-heading"]);
    header_icon.set_xalign(0.5);
    icon_box.set_center_widget(Some(&header_icon));
    header.append(&icon_box);

    let titles = vbox(2);
    titles.set_hexpand(true);
    titles.set_valign(gtk::Align::Center);
    let title = label("", &["title"]);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let subtitle = label("", &["subtitle"]);
    subtitle.set_ellipsize(gtk::pango::EllipsizeMode::End);
    titles.append(&title);
    titles.append(&subtitle);
    header.append(&titles);

    let tabs = hbox(2);
    tabs.set_valign(gtk::Align::Center);
    let tab_updates = gtk::Button::with_label("Atualizações");
    let tab_prefs = gtk::Button::with_label("Preferências");
    for b in [&tab_updates, &tab_prefs] {
      b.add_css_class("ctl");
      b.add_css_class("small");
      b.set_focus_on_click(false);
      tabs.append(b);
    }
    header.append(&tabs);

    let header_rule = hbox(0);
    header_rule.add_css_class("rule");

    // Body
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

    let footer_rule = hbox(0);
    footer_rule.add_css_class("rule");
    let footer = hbox(8);
    footer.add_css_class("footer");

    let root = vbox(0);
    root.append(&header);
    root.append(&header_rule);
    root.append(&scroller);
    root.append(&footer_rule);
    root.append(&footer);
    window.set_child(Some(&root));

    let password = gtk::PasswordEntry::new();
    password.set_show_peek_icon(true);
    password.add_css_class("field");
    password.set_hexpand(true);

    let w = Rc::new(Window {
      window,
      header_icon,
      title,
      subtitle,
      tab_updates,
      tab_prefs,
      body,
      footer,
      footer_rule,
      password,
      styler,
      paths,
      state: RefCell::new(State {
        status: None,
        run: None,
        last: None,
        prefs: serde_json::json!({"mode": "boot", "autoremoveOrphans": true, "snapshot": true, "checkHours": 6, "autoFlatpak": false}),
        checking: false,
        prefs_tab: false,
        log_open: false,
        log_tail: Vec::new(),
        starting_until: None,
        rendered: String::new(),
      }),
    });

    let me = w.clone();
    w.tab_updates.connect_clicked(move |_| me.set_tab(false));
    let me = w.clone();
    w.tab_prefs.connect_clicked(move |_| {
      me.set_tab(true);
      me.load_prefs();
    });

    let me = w.clone();
    w.password.connect_activate(move |e| {
      let text = e.text().to_string();
      if !text.is_empty() {
        me.answer(text);
      }
    });

    let keys = gtk::EventControllerKey::new();
    let me = w.clone();
    keys.connect_key_pressed(move |_, key, _, _| {
      if key == gdk::Key::Escape {
        me.window.close();
        return glib::Propagation::Stop;
      }
      glib::Propagation::Proceed
    });
    w.window.add_controller(keys);

    let me = w.clone();
    w.window.connect_close_request(move |_| {
      if me.view() == View::Result {
        me.mark_seen();
      }
      glib::Propagation::Proceed
    });

    // The files are small: re-reading them is cheaper than trusting inotify
    // with the renames omarchy-update-state does.
    let me = w.clone();
    glib::timeout_add_local(Duration::from_millis(700), move || {
      me.tick();
      glib::ControlFlow::Continue
    });

    w
  }

  pub fn present(self: &Rc<Self>, prefs_tab: bool) {
    {
      let mut s = self.state.borrow_mut();
      s.prefs_tab = prefs_tab;
      s.rendered.clear();
    }
    self.reload_theme(true);
    self.reload_files();
    self.load_prefs();
    let stale = {
      let s = self.state.borrow();
      s.status.as_ref().map(|st| model::now() - model::num(st, "checkedAt") > 600.0).unwrap_or(true)
    };
    if !self.running() && stale {
      self.check(false);
    }
    self.render();
    self.window.present();
  }

  // ---- data ---------------------------------------------------------------

  fn reload_theme(&self, force: bool) {
    if self.styler.reload(force) {
      self.state.borrow_mut().rendered.clear();
    }
  }

  fn reload_files(&self) {
    let status = model::read(&self.paths.status);
    let run = model::read(&self.paths.run);
    let last = model::read(&self.paths.last);
    let mut s = self.state.borrow_mut();
    if run.as_ref().is_some_and(|r| model::flag(r, "running")) {
      s.starting_until = None;
    }
    s.status = status;
    s.run = run;
    s.last = last;
  }

  fn tick(self: &Rc<Self>) {
    if !self.window.is_visible() {
      return;
    }
    self.reload_theme(false);
    self.reload_files();
    // A run that never reached omarchy-update-state begin (lock held, say).
    let expired = self.state.borrow().starting_until.is_some_and(|t| Instant::now() > t);
    if expired {
      self.state.borrow_mut().starting_until = None;
      self.check(false);
    }
    let view = self.view();
    let log_open = self.state.borrow().log_open;
    if log_open && matches!(view, View::Running | View::Result) {
      let lines = model::log_lines(&tail(&self.paths.log, 6000), 9);
      self.state.borrow_mut().log_tail = lines;
    }
    self.render();
  }

  fn running(&self) -> bool {
    self.state.borrow().run.as_ref().is_some_and(|r| model::flag(r, "running"))
  }

  fn view(&self) -> View {
    let s = self.state.borrow();
    let run = s.run.as_ref().filter(|r| model::flag(r, "running"));
    if let Some(run) = run {
      let waiting = run.get("askpass").is_some_and(|a| model::flag(a, "waiting"));
      return if waiting { View::Password } else { View::Running };
    }
    if s.starting_until.is_some() {
      return View::Running;
    }
    if model::staged_pending(s.status.as_ref()) {
      return View::Staged;
    }
    let unseen = s.last.as_ref().is_some_and(|l| {
      let result = model::text(l, "result");
      l.get("seen").and_then(Value::as_bool) == Some(false)
        && !result.is_empty()
        && result != "canceled"
        && (result != "ok" || model::now() - model::num(l, "finishedAt") < 3600.0)
    });
    if unseen { View::Result } else { View::Idle }
  }

  // ---- actions ------------------------------------------------------------

  fn set_tab(self: &Rc<Self>, prefs: bool) {
    self.state.borrow_mut().prefs_tab = prefs;
    self.render();
  }

  fn check(self: &Rc<Self>, refresh: bool) {
    if self.state.borrow().checking {
      return;
    }
    self.state.borrow_mut().checking = true;
    self.render();
    let me = self.clone();
    glib::spawn_future_local(async move {
      let argv: &[&str] = if refresh { &["omarchy-update-check", "--refresh"] } else { &["omarchy-update-check"] };
      output(argv).await;
      me.state.borrow_mut().checking = false;
      me.reload_files();
      me.render();
    });
  }

  fn load_prefs(self: &Rc<Self>) {
    let me = self.clone();
    glib::spawn_future_local(async move {
      if let Some(p) = output(&["omarchy-update-pref", "json"]).await.and_then(|t| model::parse(&t)) {
        me.state.borrow_mut().prefs = p;
        me.render();
      }
    });
  }

  fn set_pref(self: &Rc<Self>, key: &str, value: Value) {
    let text = match &value {
      Value::String(s) => s.clone(),
      v => v.to_string(),
    };
    if let Some(obj) = self.state.borrow_mut().prefs.as_object_mut() {
      obj.insert(key.to_string(), value);
    }
    spawn(&["omarchy-update-pref", "set", key, &text]);
    self.render();
  }

  fn start(self: &Rc<Self>, mode: &str) {
    self.mark_seen();
    self.state.borrow_mut().starting_until = Some(Instant::now() + Duration::from_secs(8));
    spawn(&["omarchy-update-run", "start", mode]);
    self.render();
  }

  // The password goes through stdin, never argv (visible in ps).
  fn answer(self: &Rc<Self>, password: String) {
    self.password.set_text("");
    let me = self.clone();
    glib::spawn_future_local(async move {
      let args = [OsStr::new("omarchy-update-run"), OsStr::new("answer")];
      if let Ok(p) = gio::Subprocess::newv(&args, gio::SubprocessFlags::STDIN_PIPE) {
        let _ = p.communicate_utf8_future(Some(format!("{password}\n").into())).await;
      }
      me.reload_files();
      me.render();
    });
  }

  fn mark_seen(&self) {
    let path = &self.paths.last;
    let mut s = self.state.borrow_mut();
    let Some(last) = s.last.as_mut() else { return };
    if last.get("seen").and_then(Value::as_bool) != Some(false) {
      return;
    }
    last["seen"] = Value::Bool(true);
    // Only the flag changes on disk: whatever else is there stays as written.
    if let Some(mut disk) = model::read(path) {
      disk["seen"] = Value::Bool(true);
      let tmp = format!("{path}.tmp");
      if std::fs::write(&tmp, disk.to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, path);
      }
    }
  }

  fn close(&self) {
    self.window.close();
  }

  // ---- rendering ----------------------------------------------------------

  fn render(self: &Rc<Self>) {
    let view = self.view();
    self.render_header(view);

    // Rebuild the body and footer only when what they show changed, so hover
    // and focus survive the polling.
    let key = {
      let s = self.state.borrow();
      format!(
        "{view:?}|{}|{}|{}|{:?}|{:?}|{:?}|{}|{}|{:?}",
        s.prefs_tab, s.checking, s.log_open, s.status, s.run, s.last, s.prefs, s.rendered.is_empty(), s.log_tail
      )
    };
    if self.state.borrow().rendered == key {
      return;
    }
    let was_password = self.state.borrow().rendered.starts_with("Password|");
    self.state.borrow_mut().rendered = key;

    if let Some(parent) = self.password.parent().and_downcast::<gtk::Box>() {
      parent.remove(&self.password);
    }
    clear(&self.body);
    clear(&self.footer);

    let prefs_tab = self.state.borrow().prefs_tab;
    if prefs_tab {
      self.tab_prefs.add_css_class("selected");
      self.tab_updates.remove_css_class("selected");
      self.body_prefs();
    } else {
      self.tab_updates.add_css_class("selected");
      self.tab_prefs.remove_css_class("selected");
      match view {
        View::Idle => self.body_idle(),
        View::Password => self.body_password(),
        View::Running => self.body_running(),
        View::Staged => self.body_staged(),
        View::Result => self.body_result(),
      }
      self.render_footer(view);
    }
    self.footer.set_visible(!prefs_tab);
    self.footer_rule.set_visible(!prefs_tab);

    if view == View::Password && !prefs_tab && !was_password {
      self.password.set_text("");
      self.password.grab_focus();
    }
  }

  fn render_header(&self, view: View) {
    let s = self.state.borrow();
    let ok = s.last.as_ref().is_some_and(|l| model::text(l, "result") == "ok");
    let (glyph, color) = if s.prefs_tab {
      (G_COG, "c-accent")
    } else {
      match view {
        View::Result if ok => (G_CHECK, "c-ok"),
        View::Result => (G_WARN, "c-urgent"),
        View::Staged => (G_REFRESH, "c-warn"),
        View::Password => (G_LOCK, "c-accent"),
        _ => (G_REFRESH, "c-accent"),
      }
    };
    self.header_icon.set_text(glyph);
    self.header_icon.set_css_classes(&["glyph-heading", color]);

    if s.prefs_tab {
      self.title.set_text("Preferências");
      self.subtitle.set_text("~/.config/omarchy/update.json");
    } else {
      self.title.set_text(&header_title(&s, view));
      self.subtitle.set_text(&header_subtitle(&s, view));
    }
  }

  fn render_footer(self: &Rc<Self>, view: View) {
    let s = self.state.borrow();
    let status = s.status.as_ref();
    let last = s.last.as_ref();
    let boot = model::text(&s.prefs, "mode") == "boot";
    let left = hbox(8);
    left.set_hexpand(true);
    left.set_valign(gtk::Align::Center);
    let right = hbox(8);
    right.set_valign(gtk::Align::Center);

    let me = self.clone();
    let button = move |text: &str, primary: bool, f: Box<dyn Fn(&Rc<Window>)>| -> gtk::Button {
      let b = gtk::Button::with_label(text);
      b.add_css_class(if primary { "primary" } else { "ctl" });
      let me = me.clone();
      b.connect_clicked(move |_| f(&me));
      b
    };

    let last_ok = last.is_some_and(|l| model::text(l, "result") == "ok");
    let last_reboot = last.map(|l| model::text(l, "reboot").to_string()).unwrap_or_default();
    let last_boot = last.is_some_and(|l| model::text(l, "mode") == "boot");

    match view {
      View::Idle => {
        let refresh_text = format!("{G_REFRESH}  {}", if s.checking { "Verificando\u{2026}" } else { "Verificar" });
        left.append(&button(&refresh_text, false, Box::new(|w| w.check(true))));
        if !s.checking && model::system_count(status) > 0 {
          let other = if boot { "Atualizar agora mesmo" } else { "Instalar no reinício" };
          let mode = if boot { "now" } else { "boot" };
          right.append(&button(other, false, Box::new(move |w| w.start(mode))));
        }
        if !s.checking && model::anything(status) {
          let text = if model::system_count(status) > 0 && boot { "Preparar para o reinício" } else { "Atualizar agora" };
          right.append(&button(
            text,
            true,
            Box::new(|w| {
              let (sys, only_flatpak, mode) = {
                let s = w.state.borrow();
                (
                  model::system_count(s.status.as_ref()),
                  model::only_flatpak(s.status.as_ref()),
                  model::text(&s.prefs, "mode").to_string(),
                )
              };
              if sys > 0 {
                w.start(&mode)
              } else if only_flatpak {
                w.start("flatpak")
              } else {
                w.start("now")
              }
            }),
          ));
        }
      }
      View::Password => {
        right.append(&button("Cancelar", false, Box::new(|_| spawn(&["omarchy-update-run", "stop"]))));
        right.append(&button(
          "Continuar",
          true,
          Box::new(|w| {
            let text = w.password.text().to_string();
            if !text.is_empty() {
              w.answer(text);
            }
          }),
        ));
      }
      View::Running => {
        left.append(&label("Pode fechar: o ícone da barra mostra o progresso.", &["footer-note"]));
        right.append(&button("Ocultar", false, Box::new(|w| w.close())));
      }
      View::Staged => {
        left.append(&button(
          "Cancelar atualização",
          false,
          Box::new(|_| spawn(&["omarchy-update-run", "cancel-staged"])),
        ));
        right.append(&button("Fechar", false, Box::new(|w| w.close())));
        right.append(&button("Reiniciar agora", true, Box::new(|_| spawn(&["omarchy-system-reboot"]))));
      }
      View::Result => {
        if !last_ok && !last_boot {
          let log = self.paths.log.clone();
          left.append(&button(
            "Ver log",
            false,
            Box::new(move |_| {
              spawn(&["omarchy-launch-floating-terminal-with-presentation", "less", "-R", "+G", &log]);
            }),
          ));
        }
        if !last_reboot.is_empty() || !last_ok {
          right.append(&button(
            "Fechar",
            false,
            Box::new(|w| {
              w.mark_seen();
              w.close()
            }),
          ));
        }
        let text = if !last_reboot.is_empty() {
          "Reiniciar agora"
        } else if !last_ok {
          "Tentar de novo"
        } else {
          "Fechar"
        };
        right.append(&button(
          text,
          true,
          Box::new(move |w| {
            let (reboot, ok) = {
              let s = w.state.borrow();
              let l = s.last.as_ref();
              (
                l.is_some_and(|l| !model::text(l, "reboot").is_empty()),
                l.is_some_and(|l| model::text(l, "result") == "ok"),
              )
            };
            w.mark_seen();
            if reboot {
              spawn(&["omarchy-system-reboot"]);
            } else if !ok {
              w.check(true);
            } else {
              w.close();
            }
          }),
        ));
      }
    }
    self.footer.append(&left);
    self.footer.append(&right);
  }

  fn body_idle(self: &Rc<Self>) {
    let s = self.state.borrow();
    let st = s.status.as_ref();
    let boot = model::text(&s.prefs, "mode") == "boot";
    let col = vbox(10);

    let sys = model::system_count(st);
    col.append(&source_row(
      G_CUBE,
      &if sys > 0 { format!("Sistema · {}", model::plural(sys, "pacote", "pacotes")) } else { "Sistema".into() },
      &if sys > 0 { model::system_names(st, 4) } else { "em dia".into() },
      if sys == 0 { "EM DIA" } else if boot { "NO REINÍCIO" } else { "AGORA" },
      if sys == 0 { "c-dim" } else if boot { "c-warn" } else { "c-ok" },
      sys > 0,
    ));

    let fp = model::flatpak_count(st);
    col.append(&source_row(
      G_ARCHIVE,
      &if fp > 0 { format!("Flatpak · {}", model::plural(fp, "item", "itens")) } else { "Flatpak".into() },
      &if fp > 0 { model::flatpak_names(st, 3) } else { "em dia".into() },
      if fp > 0 { "NA HORA" } else { "EM DIA" },
      if fp > 0 { "c-ok" } else { "c-dim" },
      fp > 0,
    ));

    let dx = model::doxia_behind(st);
    let head = model::doxia_head(st);
    col.append(&source_row(
      G_BRANCH,
      &if dx > 0 { format!("DoxIA · {}", model::plural(dx, "novidade", "novidades")) } else { "DoxIA".into() },
      &if dx > 0 {
        model::doxia_first_commit(st)
      } else if head.is_empty() {
        "em dia".into()
      } else {
        format!("{head} · em dia")
      },
      if dx > 0 { "NA HORA" } else { "EM DIA" },
      if dx > 0 { "c-ok" } else { "c-dim" },
      dx > 0,
    ));

    if !s.checking && st.is_some() {
      let tone = if sys > 0 && boot {
        "warn"
      } else if model::anything(st) {
        "ok"
      } else {
        "plain"
      };
      let text = if sys > 0 {
        if boot {
          "Os pacotes do sistema serão baixados agora e instalados no próximo reinício. Você pode continuar usando o computador."
        } else {
          "Os pacotes do sistema serão instalados agora. Feche os apps que estiverem sendo atualizados."
        }
      } else if model::only_flatpak(st) {
        "Só apps Flatpak. Eles são atualizados agora, sem senha e sem reiniciar."
      } else if dx > 0 {
        "Só novidades do DoxIA. São aplicadas agora, sem reiniciar."
      } else {
        "Tudo em dia."
      };
      col.append(&banner(tone, if tone == "warn" { G_REFRESH } else { G_CHECK }, text, false));
    }

    let errors = model::errors(st);
    if !errors.is_empty() {
      col.append(&banner("error", G_WARN, &errors.join(" "), false));
    }

    if s.checking || st.is_none() {
      col.append(&banner("plain", G_REFRESH, "Procurando atualizações…", true));
    }
    self.body.append(&col);
  }

  fn body_password(self: &Rc<Self>) {
    let s = self.state.borrow();
    let run = s.run.as_ref();
    let col = vbox(10);
    col.append(&label("Será executado como root:", &["caption"]));
    let lines: Vec<String> = run
      .and_then(|r| r.get("rootCommands"))
      .and_then(Value::as_array)
      .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
      .unwrap_or_default();
    col.append(&command_box(&lines, true));
    let user = std::env::var("USER").unwrap_or_default();
    col.append(&label(&format!("Senha de {user}"), &["caption"]));
    col.append(&self.password);
    let attempt = run.and_then(|r| r.get("askpass")).map(|a| model::num(a, "attempt")).unwrap_or(0.0);
    if attempt > 1.0 {
      col.append(&label("Senha incorreta. Tente de novo.", &["caption", "c-urgent"]));
    }
    self.body.append(&col);
  }

  fn body_running(self: &Rc<Self>) {
    let s = self.state.borrow();
    let col = vbox(12);
    let bar = gtk::ProgressBar::new();
    bar.add_css_class("line");
    bar.set_fraction(s.run.as_ref().map(|r| model::num(r, "progress") / 100.0).unwrap_or(0.0).clamp(0.0, 1.0));
    col.append(&bar);
    col.append(&step_list(&model::steps(s.run.as_ref())));
    drop(s);
    col.append(&self.log_box());
    self.body.append(&col);
  }

  fn body_staged(self: &Rc<Self>) {
    let s = self.state.borrow();
    let packages = model::staged_packages(s.status.as_ref());
    let autoremove = model::flag(&s.prefs, "autoremoveOrphans");
    let col = vbox(12);
    let count = if packages.is_empty() { String::new() } else { packages.len().to_string() };
    let steps = serde_json::json!([
      {"id": "a", "label": "Pacotes baixados e testados", "state": "done", "detail": count},
      {"id": "b", "label": "Instalar durante o reinício", "state": "todo"},
      {"id": "c", "label": "Migrações e configuração do DoxIA", "state": "todo"},
      {"id": "d", "label": "Remover pacotes órfãos", "state": if autoremove { "todo" } else { "skip" },
       "detail": if autoremove { "auto" } else { "desligado" }}
    ]);
    col.append(&step_list(steps.as_array().unwrap()));
    if !packages.is_empty() {
      let mut lines: Vec<String> = packages.iter().take(6).cloned().collect();
      if packages.len() > 6 {
        lines.push(format!("+ {} outros", packages.len() - 6));
      }
      col.append(&command_box(&lines, false));
    }
    col.append(&banner(
      "warn",
      G_POWER,
      "A atualização é instalada na próxima vez que você reiniciar ou desligar, antes de voltar para o login. Até lá, continue usando normalmente.",
      false,
    ));
    self.body.append(&col);
  }

  fn body_result(self: &Rc<Self>) {
    let s = self.state.borrow();
    let last = s.last.as_ref();
    let col = vbox(12);
    let ok = last.is_some_and(|l| model::text(l, "result") == "ok");
    if last.is_some() && !ok {
      col.append(&banner("error", G_WARN, &model::failure_text(last), false));
    }
    let steps: Vec<Value> = model::steps(last).into_iter().filter(|st| model::text(st, "state") != "todo").collect();
    col.append(&step_list(&steps));
    let reboot = last.map(|l| model::text(l, "reboot").to_string()).unwrap_or_default();
    if !reboot.is_empty() {
      col.append(&banner("warn", G_REFRESH, &format!("{reboot}. Reinicie para usar a versão nova."), false));
    }
    let show_log = last.is_some_and(|l| !ok && model::text(l, "mode") != "boot");
    drop(s);
    if show_log {
      col.append(&self.log_box());
    }
    self.body.append(&col);
  }

  fn log_box(self: &Rc<Self>) -> gtk::Box {
    let s = self.state.borrow();
    let col = vbox(6);
    let toggle = gtk::Button::with_label(&format!("{} Saída", if s.log_open { "▾" } else { "▸" }));
    toggle.add_css_class("ctl");
    toggle.add_css_class("plain");
    toggle.add_css_class("small");
    toggle.set_halign(gtk::Align::Start);
    let me = self.clone();
    toggle.connect_clicked(move |_| {
      let open = !me.state.borrow().log_open;
      me.state.borrow_mut().log_open = open;
      if open {
        let lines = model::log_lines(&tail(&me.paths.log, 6000), 9);
        me.state.borrow_mut().log_tail = lines;
      }
      me.render();
    });
    col.append(&toggle);
    if s.log_open {
      let inset = vbox(0);
      inset.add_css_class("inset");
      let text = if s.log_tail.is_empty() { "…".to_string() } else { s.log_tail.join("\n") };
      let l = label(&text, &["log"]);
      l.set_ellipsize(gtk::pango::EllipsizeMode::End);
      l.set_lines(9);
      inset.append(&l);
      col.append(&inset);
    }
    col
  }

  fn body_prefs(self: &Rc<Self>) {
    let s = self.state.borrow();
    let prefs = s.prefs.clone();
    let snapper = s.status.as_ref().is_none_or(|st| st.get("snapper").and_then(Value::as_bool) != Some(false));
    drop(s);
    let col = vbox(10);
    let mode = model::text(&prefs, "mode").to_string();

    col.append(&label("QUANDO APLICAR OS PACOTES DO SISTEMA", &["caption", "section"]));
    col.append(&self.radio_card(
      mode == "boot",
      "No próximo reinício",
      "Baixa agora e instala com o sistema parado (dnf offline), mais seguro e sem apps abertos usando bibliotecas trocadas. Flatpak e DoxIA continuam sendo aplicados na hora.",
      "boot",
    ));
    col.append(&self.radio_card(
      mode == "now",
      "Agora, com o sistema rodando",
      "Instala na hora e só pede reinício se o kernel ou o mango mudarem.",
      "now",
    ));

    let section = |text: &str| {
      let l = label(text, &["caption", "section"]);
      l.set_margin_top(6);
      l
    };
    col.append(&section("LIMPEZA"));
    col.append(&self.toggle_row(
      "Remover pacotes órfãos automaticamente",
      "Roda dnf autoremove no fim da atualização, sem perguntar.",
      model::flag(&prefs, "autoremoveOrphans"),
      true,
      "autoremoveOrphans",
    ));
    col.append(&self.toggle_row(
      "Criar snapshot antes de atualizar",
      if snapper { "Snapper. Dá para voltar pelo menu de boot se algo quebrar." } else { "O Snapper não está instalado." },
      model::flag(&prefs, "snapshot") && snapper,
      snapper,
      "snapshot",
    ));

    col.append(&section("VERIFICAÇÃO"));
    col.append(&self.check_hours_row(model::num(&prefs, "checkHours") as i64));
    col.append(&self.toggle_row(
      "Atualizar Flatpaks sozinho",
      "Em segundo plano, sem abrir esta janela. Uma notificação avisa quando terminar.",
      model::flag(&prefs, "autoFlatpak"),
      true,
      "autoFlatpak",
    ));
    self.body.append(&col);
  }

  fn radio_card(self: &Rc<Self>, checked: bool, title: &str, description: &str, mode: &'static str) -> gtk::Box {
    let card = hbox(10);
    card.add_css_class("radio-card");
    if checked {
      card.add_css_class("checked");
    }
    let dot = gtk::CenterBox::new();
    dot.add_css_class("radio-dot");
    dot.set_valign(gtk::Align::Start);
    dot.set_halign(gtk::Align::Start);
    dot.set_margin_top(2);
    if checked {
      let inner = hbox(0);
      inner.add_css_class("radio-inner");
      inner.set_halign(gtk::Align::Center);
      inner.set_valign(gtk::Align::Center);
      dot.set_center_widget(Some(&inner));
    }
    card.append(&dot);
    let text = vbox(3);
    text.set_hexpand(true);
    text.append(&label(title, &["row-title"]));
    let d = label(description, &["desc"]);
    d.set_wrap(true);
    d.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    text.append(&d);
    card.append(&text);
    let me = self.clone();
    on_click(&card, move || me.set_pref("mode", Value::String(mode.into())));
    card
  }

  fn toggle_row(self: &Rc<Self>, title: &str, description: &str, checked: bool, enabled: bool, key: &'static str) -> gtk::Box {
    let row = hbox(12);
    row.add_css_class("toggle-row");
    if !enabled {
      row.add_css_class("disabled");
    }
    let text = vbox(3);
    text.set_hexpand(true);
    text.set_valign(gtk::Align::Center);
    let t = label(title, &["toggle-title"]);
    t.set_ellipsize(gtk::pango::EllipsizeMode::End);
    text.append(&t);
    let d = label(description, &["toggle-desc"]);
    d.set_wrap(true);
    text.append(&d);
    row.append(&text);
    let sw = gtk::Switch::new();
    sw.set_active(checked);
    sw.set_can_target(false);
    sw.set_can_focus(false);
    sw.set_valign(gtk::Align::Center);
    row.append(&sw);
    if enabled {
      let me = self.clone();
      on_click(&row, move || me.set_pref(key, Value::Bool(!checked)));
    }
    row
  }

  fn check_hours_row(self: &Rc<Self>, hours: i64) -> gtk::Box {
    const OPTIONS: [(i64, &str); 5] =
      [(1, "a cada hora"), (3, "a cada 3 horas"), (6, "a cada 6 horas"), (12, "a cada 12 horas"), (24, "uma vez por dia")];
    let row = hbox(12);
    row.add_css_class("toggle-row");
    let t = label("Procurar atualizações", &["toggle-title"]);
    t.set_hexpand(true);
    t.set_valign(gtk::Align::Center);
    row.append(&t);
    let labels: Vec<&str> = OPTIONS.iter().map(|(_, l)| *l).collect();
    let dd = gtk::DropDown::from_strings(&labels);
    dd.add_css_class("choice");
    dd.set_valign(gtk::Align::Center);
    dd.set_selected(OPTIONS.iter().position(|(h, _)| *h == hours).unwrap_or(2) as u32);
    let me = self.clone();
    dd.connect_selected_notify(move |d| {
      if let Some((h, _)) = OPTIONS.get(d.selected() as usize) {
        me.set_pref("checkHours", Value::from(*h));
      }
    });
    row.append(&dd);
    row
  }
}

fn header_title(s: &State, view: View) -> String {
  match view {
    View::Password => "Confirme sua senha".into(),
    View::Running => model::current_step(s.run.as_ref())
      .map(|st| model::text(&st, "label").to_string())
      .unwrap_or_else(|| "Preparando…".into()),
    View::Staged => "Pronto para instalar".into(),
    View::Result => {
      let Some(last) = s.last.as_ref() else { return String::new() };
      if model::text(last, "result") != "ok" {
        return "A atualização não terminou".into();
      }
      match model::text(last, "mode") {
        "boot" => "Sistema atualizado".into(),
        "flatpak" => "Apps atualizados".into(),
        _ => "Tudo atualizado".into(),
      }
    }
    View::Idle => {
      if s.checking && s.status.is_none() {
        return "Procurando atualizações".into();
      }
      let n = model::updates_count(s.status.as_ref());
      if n > 0 { model::plural(n, "atualização", "atualizações") } else { "Tudo em dia".into() }
    }
  }
}

fn header_subtitle(s: &State, view: View) -> String {
  match view {
    View::Password => {
      let boot = s.run.as_ref().is_some_and(|r| model::text(r, "mode") == "boot");
      if boot { "Para instalar no próximo reinício".into() } else { "Uma vez, para toda a atualização".into() }
    }
    View::Running => {
      let mut parts = Vec::new();
      if let Some(step) = model::current_step(s.run.as_ref()) {
        let detail = step.get("detail").map(|d| d.as_str().map(String::from).unwrap_or_else(|| d.to_string()));
        if let Some(d) = detail.filter(|d| !d.is_empty() && d != "null") {
          parts.push(d);
        }
      }
      if let Some(started) = s.run.as_ref().map(|r| model::num(r, "startedAt")).filter(|t| *t > 0.0) {
        parts.push(model::duration(model::now() - started));
      }
      parts.join(" · ")
    }
    View::Staged => {
      let n = model::staged_packages(s.status.as_ref()).len();
      let prefix = if n > 0 { format!("{} · ", model::plural(n, "pacote", "pacotes")) } else { String::new() };
      format!("{prefix}preparado às {}", model::clock(model::staged_at(s.status.as_ref())))
    }
    View::Result => {
      let Some(last) = s.last.as_ref() else { return String::new() };
      let mut bits = Vec::new();
      let packages = last.get("summary").map(|sum| model::num(sum, "packages") as usize).unwrap_or(0);
      if packages > 0 {
        bits.push(model::plural(packages, "pacote", "pacotes"));
      }
      let (start, end) = (model::num(last, "startedAt"), model::num(last, "finishedAt"));
      if start > 0.0 && end > 0.0 {
        bits.push(model::duration(end - start));
      }
      bits.push(format!("às {}", model::clock(end)));
      bits.join(" · ")
    }
    View::Idle => {
      if s.checking {
        "Verificando…".into()
      } else {
        format!("Verificado {}", model::ago(s.status.as_ref().map(|st| model::num(st, "checkedAt")).unwrap_or(0.0)))
      }
    }
  }
}

fn source_row(glyph: &str, title: &str, subtitle: &str, chip: &str, chip_color: &str, active: bool) -> gtk::Box {
  let row = hbox(12);
  row.add_css_class("source-row");
  if !active {
    row.add_css_class("inactive");
  }
  let icon = gtk::CenterBox::new();
  icon.add_css_class("source-icon");
  icon.set_valign(gtk::Align::Center);
  let g = label(glyph, &["glyph-title"]);
  g.set_xalign(0.5);
  icon.set_center_widget(Some(&g));
  row.append(&icon);
  let text = vbox(2);
  text.set_hexpand(true);
  text.set_valign(gtk::Align::Center);
  for (t, class) in [(title, "row-title"), (subtitle, "row-sub")] {
    let l = label(t, &[class]);
    l.set_ellipsize(gtk::pango::EllipsizeMode::End);
    text.append(&l);
  }
  row.append(&text);
  let c = label(chip, &["chip", chip_color]);
  c.set_valign(gtk::Align::Center);
  row.append(&c);
  row
}

fn banner(tone: &str, glyph: &str, text: &str, spinning: bool) -> gtk::Box {
  let b = hbox(10);
  b.add_css_class("banner");
  b.add_css_class(tone);
  let color = match tone {
    "ok" => "c-ok",
    "warn" => "c-warn",
    "error" => "c-urgent",
    _ => "glyph-body",
  };
  let g = label(glyph, &["glyph-body", color]);
  g.set_valign(gtk::Align::Start);
  if spinning {
    g.add_css_class("pulse");
  }
  b.append(&g);
  let t = label(text, &["banner-text"]);
  t.set_wrap(true);
  t.set_wrap_mode(gtk::pango::WrapMode::WordChar);
  t.set_hexpand(true);
  b.append(&t);
  b
}

fn command_box(lines: &[String], commands: bool) -> gtk::Box {
  let b = vbox(3);
  b.add_css_class("inset");
  for line in lines {
    let l = gtk::Label::new(None);
    l.set_xalign(0.0);
    l.add_css_class("cmd");
    l.set_wrap(true);
    l.set_wrap_mode(gtk::pango::WrapMode::Char);
    let escaped = glib::markup_escape_text(line);
    if commands {
      l.set_markup(&format!("<span foreground=\"{}\">#</span> {escaped}", doxia_theme::accent()));
    } else {
      l.set_markup(&escaped);
    }
    b.append(&l);
  }
  b
}

fn step_list(steps: &[Value]) -> gtk::Box {
  let col = vbox(2);
  for step in steps {
    let st = match model::text(step, "state") {
      "" => "todo",
      s => s,
    };
    let row = hbox(10);
    row.add_css_class("step");
    let dot = gtk::CenterBox::new();
    dot.add_css_class("dot");
    dot.add_css_class(st);
    dot.set_valign(gtk::Align::Center);
    if st == "done" || st == "fail" {
      let g = label(if st == "done" { G_CHECK } else { G_TIMES }, &["dot-glyph"]);
      g.set_xalign(0.5);
      dot.set_center_widget(Some(&g));
    }
    row.append(&dot);
    let l = label(model::text(step, "label"), &["step-label", st]);
    l.set_hexpand(true);
    l.set_ellipsize(gtk::pango::EllipsizeMode::End);
    row.append(&l);
    let detail = match step.get("detail") {
      Some(Value::String(s)) if !s.is_empty() => s.clone(),
      Some(Value::Number(n)) => n.to_string(),
      _ if st == "done" && model::num(step, "seconds") > 1.0 => model::duration(model::num(step, "seconds")),
      _ => String::new(),
    };
    row.append(&label(&detail, &["step-detail"]));
    col.append(&row);
  }
  col
}
