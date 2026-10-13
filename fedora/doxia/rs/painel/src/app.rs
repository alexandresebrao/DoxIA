// The window: a stack of pages (home, then the open action, then maybe the
// Jira credentials), the toasts over them and the keyboard shortcuts.

use crate::config::{self, str_of};
use crate::pages::{self, Page};
use crate::ui::Toasts;
use doxia_theme::Styler;
use gtk::prelude::*;
use gtk::{gdk, glib};
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

pub type AppRef = Weak<App>;

pub struct App {
  pub window: gtk::ApplicationWindow,
  stack: gtk::Stack,
  pub toasts: Toasts,
  _styler: Styler,
  state: RefCell<State>,
}

#[derive(Default)]
struct State {
  cfg: Value,
  actions: Vec<Value>,
  pages: HashMap<String, Rc<dyn Page>>,
  // Names in the stack, home first.
  nav: Vec<String>,
  after_credentials: Option<Box<dyn Fn()>>,
}

impl App {
  pub fn new(app: &gtk::Application) -> Rc<App> {
    let window = gtk::ApplicationWindow::builder()
      .application(app)
      .title("Painel Rápido")
      .default_width(560)
      .default_height(640)
      .build();
    window.add_css_class("doxia");
    let styler = Styler::install(include_str!("style.css"));

    let overlay = gtk::Overlay::new();
    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::SlideLeftRight);
    stack.set_transition_duration(160);
    overlay.set_child(Some(&stack));
    let toasts = Toasts::new(&overlay);
    window.set_child(Some(&overlay));

    let me = Rc::new(App { window, stack, toasts, _styler: styler, state: RefCell::default() });

    // Capture phase: Ctrl+Enter and friends reach the page before a text view
    // takes them as typing.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(&me);
    keys.connect_key_pressed(move |_, key, _, mods| {
      let Some(me) = weak.upgrade() else { return glib::Propagation::Proceed };
      if me.on_key(key, mods) { glib::Propagation::Stop } else { glib::Propagation::Proceed }
    });
    me.window.add_controller(keys);

    // Theme switches while the window is open.
    let weak = Rc::downgrade(&me);
    glib::timeout_add_seconds_local(2, move || {
      let Some(me) = weak.upgrade() else { return glib::ControlFlow::Break };
      me._styler.reload(false);
      glib::ControlFlow::Continue
    });

    me.load();
    me
  }

  pub fn cfg(&self) -> Value {
    self.state.borrow().cfg.clone()
  }

  // Reads painel.json and builds every page, so a form keeps what was typed
  // while going back and forth.
  pub fn load(self: &Rc<Self>) {
    let (cfg, mut error) = config::load();
    let (actions, warnings) = config::validate(&cfg);
    let mut toast = None;
    if !warnings.is_empty() && error.is_none() {
      if actions.is_empty() {
        error = Some(warnings.join("; "));
      } else {
        toast = Some(format!("Ignorado: {}", warnings.join("; ")));
      }
    }

    while let Some(child) = self.stack.first_child() {
      self.stack.remove(&child);
    }
    {
      let mut s = self.state.borrow_mut();
      s.cfg = cfg.clone();
      s.actions = actions.clone();
      s.pages.clear();
      s.nav = vec!["home".into()];
    }
    let weak = Rc::downgrade(self);
    let home: Rc<dyn Page> = pages::home::Home::new(weak.clone(), &actions, error);
    self.add_page("home", home);
    for action in &actions {
      let kind = str_of(action, "type");
      if let Some(page) = pages::build(kind, weak.clone(), &cfg, action) {
        self.add_page(str_of(action, "id"), page);
      }
    }
    self.stack.set_visible_child_name("home");
    if let Some(t) = toast {
      self.toasts.show(&t, 8);
    }
  }

  fn add_page(&self, name: &str, page: Rc<dyn Page>) {
    self.stack.add_named(&page.widget(), Some(name));
    self.state.borrow_mut().pages.insert(name.to_string(), page);
  }

  fn current(&self) -> Option<Rc<dyn Page>> {
    let s = self.state.borrow();
    s.nav.last().and_then(|n| s.pages.get(n)).cloned()
  }

  fn show(&self, name: &str) {
    self.stack.set_visible_child_name(name);
    if let Some(page) = self.current() {
      page.shown();
    }
  }

  pub fn reload(self: &Rc<Self>) {
    self.load();
    self.toasts.show("Configuração recarregada", 2);
  }

  // "page", "direct", or None for an unknown id.
  pub fn open_action(self: &Rc<Self>, id: &str, from_cli: bool) -> Option<&'static str> {
    let action = self.state.borrow().actions.iter().find(|a| str_of(a, "id") == id).cloned();
    let Some(action) = action else {
      self.toasts.show(&format!("Ação “{id}” não existe em {}", config::config_file().file_name().unwrap().to_string_lossy()), 5);
      return None;
    };
    if config::has_page(str_of(&action, "type")) {
      self.state.borrow_mut().nav = vec!["home".into(), id.to_string()];
      self.show(id);
      return Some("page");
    }
    pages::run_direct(&action);
    if !from_cli {
      self.window.close();
    }
    Some("direct")
  }

  pub fn pop(self: &Rc<Self>) {
    let name = {
      let mut s = self.state.borrow_mut();
      if s.nav.len() <= 1 {
        return;
      }
      let gone = s.nav.pop().unwrap();
      if gone == "credenciais" {
        s.pages.remove("credenciais");
      }
      s.nav.last().cloned().unwrap()
    };
    self.show(&name);
    if let Some(old) = self.stack.child_by_name("credenciais").filter(|_| !self.state.borrow().pages.contains_key("credenciais")) {
      // After the slide, so the page doesn't vanish mid-transition.
      let stack = self.stack.clone();
      glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || stack.remove(&old));
    }
  }

  pub fn push_credentials(self: &Rc<Self>, site: &str, then: Option<Box<dyn Fn()>>) {
    if site.is_empty() {
      self.toasts.show("Falta o site do Jira em painel.json", 5);
      return;
    }
    if let Some(old) = self.stack.child_by_name("credenciais") {
      self.stack.remove(&old);
    }
    let page: Rc<dyn Page> = pages::credentials::Credentials::new(Rc::downgrade(self), site);
    self.add_page("credenciais", page);
    {
      let mut s = self.state.borrow_mut();
      s.after_credentials = then;
      if s.nav.last().map(String::as_str) != Some("credenciais") {
        s.nav.push("credenciais".into());
      }
    }
    self.show("credenciais");
  }

  pub fn credentials_saved(self: &Rc<Self>) {
    self.pop();
    let then = self.state.borrow_mut().after_credentials.take();
    if let Some(f) = then {
      glib::idle_add_local_once(f);
    }
  }

  // The link goes to the clipboard, a toast and a desktop notification.
  pub fn notify_link(&self, title: &str, url: &str) {
    crate::ui::copy(url);
    let u = url.to_string();
    self.toasts.show_with(&format!("{title} · link copiado"), 8, Some("Abrir"), Some(move || crate::ui::open_url(&u)));
    let _ = std::process::Command::new("omarchy-notification-send")
      .args(["--app-name", "Painel Rápido", title, url])
      .stdin(std::process::Stdio::null())
      .spawn()
      .map(|mut c| std::thread::spawn(move || c.wait()));
  }

  fn on_key(self: &Rc<Self>, key: gdk::Key, mods: gdk::ModifierType) -> bool {
    if key == gdk::Key::Escape && mods.is_empty() {
      if self.state.borrow().nav.len() > 1 {
        self.pop();
      } else {
        self.window.close();
      }
      return true;
    }
    self.current().is_some_and(|p| p.key(key, mods))
  }
}
