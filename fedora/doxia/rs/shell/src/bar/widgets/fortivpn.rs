// doxia.fortivpn (fedora/doxia/plugins/doxia.fortivpn): openfortivpn
// profiles from the bar. The icon (a lock while connected, dim otherwise);
// a right click connects or disconnects the current profile. Its panel:
// the state with the switch, errors and an untrusted gateway certificate
// to trust, the tunnel's interface, address, peer and routes, and the
// profiles to connect; then the list to manage them and the form to edit
// one. Status from status.sh, connections through vpnctl.sh, profiles
// through the root helper (pkexec, allowed by setup-system.sh's rule); the
// password goes to it on stdin.

use super::super::config::setting_str;
use super::super::panel::Panel;
use super::super::ui;
use super::super::widget::{Button, Kind, Slot};
use super::super::Ctx;
use gtk::prelude::*;
use gtk::{gdk, gio, glib, pango};
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

const VPN: &str = "\u{F0582}";
const LOCK: &str = "\u{F099D}";
const COG: &str = "\u{F0493}";
const PLUS: &str = "\u{F0415}";
const PENCIL: &str = "\u{F03EB}";
const TRASH: &str = "\u{F01B4}";
const BACK: &str = "\u{F004D}";
const HELPER: &str = "/usr/local/libexec/omarchy-fortivpn-helper";

fn plugin_dir() -> String {
  format!("{}/fedora/doxia/plugins/doxia.fortivpn", std::env::var("OMARCHY_PATH").unwrap_or_default())
}

pub fn duration(seconds: f64) -> String {
  if seconds < 0.0 {
    return String::new();
  }
  let s = seconds.floor() as u64;
  let (h, m) = (s / 3600, (s % 3600) / 60);
  if h > 0 {
    format!("{h}h {m:02}m")
  } else if m > 0 {
    format!("{m}m {:02}s", s % 60)
  } else {
    format!("{}s", s % 60)
  }
}

pub fn valid_name(name: &str) -> bool {
  (1..=32).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

#[derive(Default, Clone)]
struct Status {
  helper: bool,
  active: String,
  active_state: String,
  focus: String,
  focus_state: String,
  external_pid: i64,
  since: f64,
  iface: String,
  ip: String,
  peer: String,
  routes: i64,
  error: String,
  cert: String,
}

#[derive(Default)]
struct State {
  st: Status,
  loaded: bool,
  profiles: Vec<Value>,
  profiles_loaded: bool,
  pending: String,
  pending_profile: String,
  action_error: String,
  view: &'static str,
  confirm_delete: String,
  form_error: String,
  editing: Option<Value>,
  busy: bool,
}

impl State {
  fn connected(&self) -> bool {
    !self.st.ip.is_empty()
  }
  fn external(&self) -> bool {
    self.st.external_pid > 0 && self.st.active.is_empty()
  }
  fn starting(&self) -> bool {
    self.st.active_state == "activating" || (self.st.active_state == "active" && !self.connected())
  }
  fn transitioning(&self) -> bool {
    !self.pending.is_empty() || self.starting() || self.st.active_state == "deactivating"
  }
  fn has(&self, name: &str) -> bool {
    self.profiles.iter().any(|p| p["name"] == name)
  }
  fn current(&self) -> String {
    if !self.st.active.is_empty() {
      self.st.active.clone()
    } else if !self.st.focus.is_empty() && self.has(&self.st.focus) {
      self.st.focus.clone()
    } else {
      self.profiles.first().and_then(|p| p["name"].as_str()).unwrap_or("").to_string()
    }
  }
  fn switch_on(&self) -> bool {
    self.pending == "up" || (self.pending.is_empty() && (self.connected() || self.starting()))
  }
  fn error_text(&self) -> String {
    if !self.action_error.is_empty() {
      self.action_error.clone()
    } else if self.st.focus_state == "failed" || self.starting() {
      self.st.error.clone()
    } else {
      String::new()
    }
  }
  fn profile_state(&self, name: &str) -> &'static str {
    if name != self.st.active {
      return if name == self.st.focus && self.st.focus_state == "failed" { "failed" } else { "" };
    }
    if self.st.active_state == "active" && self.connected() {
      "connected"
    } else if self.starting() {
      "activating"
    } else if self.st.active_state == "deactivating" {
      "deactivating"
    } else {
      ""
    }
  }
  fn status_text(&self, now: f64) -> String {
    if self.pending == "up" || self.starting() {
      format!("Conectando a {}…", self.current())
    } else if self.pending == "down" || self.st.active_state == "deactivating" {
      "Desconectando…".into()
    } else if self.connected() {
      let who = if self.external() { "Conectado (manual)".to_string() } else { format!("Conectado · {}", self.st.active) };
      let d = if self.st.since > 0.0 { duration(now - self.st.since) } else { String::new() };
      format!("{who} · {d}")
    } else if self.st.focus_state == "failed" {
      format!("Falha em {}", self.st.focus)
    } else {
      "Desconectado".into()
    }
  }
}

fn now() -> f64 {
  std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

async fn run(argv: &[String], input: Option<String>) -> (bool, String, String) {
  let args: Vec<&std::ffi::OsStr> = argv.iter().map(|s| s.as_ref()).collect();
  let mut flags = gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_PIPE;
  if input.is_some() {
    flags |= gio::SubprocessFlags::STDIN_PIPE;
  }
  let Ok(child) = gio::Subprocess::newv(&args, flags) else { return (false, String::new(), "não foi possível rodar".into()) };
  let res = child.communicate_utf8_future(input).await;
  let (out, err) = res.map(|(o, e)| (o.map(|s| s.to_string()).unwrap_or_default(), e.map(|s| s.to_string()).unwrap_or_default())).unwrap_or_default();
  (child.is_successful(), out, err)
}

type Weak = Rc<RefCell<std::rc::Weak<dyn Fn()>>>;

fn call(w: &Weak) {
  if let Some(f) = w.borrow().upgrade() {
    f();
  }
}

pub fn build(ctx: &Rc<Ctx>, entry: &Value) -> gtk::Widget {
  let space = { let s = ctx.style.clone(); move |px: f64| s.space(px) };
  let interval = setting_str(entry, "refreshIntervalSec", "10").parse::<u64>().unwrap_or(10).max(2);
  let button = Button::new(ctx, Kind::Icon, VPN);
  let slot = Slot::new(ctx, &button.widget);
  let panel = Panel::new(ctx, &button.widget, &slot, space(360.0));
  let state = Rc::new(RefCell::new(State { view: "main", st: Status { helper: true, ..Status::default() }, ..State::default() }));
  let body = ui::vbox(space(12.0));
  let scroll = gtk::ScrolledWindow::new();
  scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
  scroll.set_propagate_natural_height(true);
  scroll.set_max_content_height(space(620.0));
  scroll.set_child(Some(&body));
  panel.content.append(&scroll);
  let render_w: Weak = Rc::new(RefCell::new(std::rc::Weak::<Box<dyn Fn()>>::new() as std::rc::Weak<dyn Fn()>));
  let status_w: Weak = Rc::new(RefCell::new(std::rc::Weak::<Box<dyn Fn()>>::new() as std::rc::Weak<dyn Fn()>));
  let profiles_w: Weak = Rc::new(RefCell::new(std::rc::Weak::<Box<dyn Fn()>>::new() as std::rc::Weak<dyn Fn()>));

  let notify = |title: &str, body: &str| crate::run::argv(&["omarchy-notification-send", "-g", VPN, title, body]);

  let read_status: Rc<dyn Fn()> = {
    let (state, b, render_w, profiles_w) = (state.clone(), button.clone(), render_w.clone(), profiles_w.clone());
    Rc::new(move || {
      let (state, b, render_w, profiles_w) = (state.clone(), b.clone(), render_w.clone(), profiles_w.clone());
      glib::spawn_future_local(async move {
        let focus = state.borrow().pending_profile.clone();
        let out = crate::watch::output(&["bash", &format!("{}/status.sh", plugin_dir()), &focus]).await.unwrap_or_default();
        let Ok(s) = serde_json::from_str::<Value>(out.trim()) else { return };
        let str_of = |k: &str| s[k].as_str().unwrap_or("").to_string();
        let new = Status {
          helper: s["helperInstalled"].as_bool().unwrap_or(false),
          active: str_of("activeProfile"),
          active_state: str_of("activeState"),
          focus: str_of("focus"),
          focus_state: str_of("focusState"),
          external_pid: s["externalPid"].as_i64().unwrap_or(0),
          since: s["since"].as_f64().unwrap_or(0.0),
          iface: str_of("iface"),
          ip: str_of("ip"),
          peer: str_of("peer"),
          routes: s["routes"].as_i64().unwrap_or(0),
          error: str_of("error"),
          cert: str_of("cert"),
        };
        let (was_connected, was_helper, need_profiles) = {
          let st = state.borrow();
          (st.connected(), st.st.helper, !st.profiles_loaded)
        };
        {
          let mut st = state.borrow_mut();
          st.st = new;
          if st.pending == "up" && ((st.connected() && st.st.active_state == "active") || st.st.focus_state == "failed") {
            st.pending.clear();
          }
          if st.pending == "down" && !st.connected() && st.st.active.is_empty() && st.st.external_pid == 0 {
            st.pending.clear();
          }
          if st.pending.is_empty() {
            st.pending_profile.clear();
          }
          if st.loaded && was_connected != st.connected() {
            if st.connected() {
              let who = if st.st.active.is_empty() { "manual".to_string() } else { st.st.active.clone() };
              notify("VPN conectada", &format!("{who} · {}", st.st.ip));
            } else {
              notify("VPN desconectada", if st.st.focus_state == "failed" { &st.st.error } else { "" });
            }
          }
          st.loaded = true;
        }
        let st = state.borrow();
        b.set_text(if st.connected() { LOCK } else { VPN });
        b.set_dimmed(!st.connected());
        b.widget.set_opacity(if st.transitioning() { 0.6 } else { 1.0 });
        b.set_tooltip(&format!("FortiVPN: {}", st.status_text(now())));
        let helper = st.st.helper;
        drop(st);
        if helper && (!was_helper || need_profiles) {
          call(&profiles_w);
        }
        if state.borrow().view != "edit" {
          call(&render_w);
        }
      });
    })
  };
  *status_w.borrow_mut() = Rc::downgrade(&read_status);
  let read_profiles: Rc<dyn Fn()> = {
    let (state, render_w) = (state.clone(), render_w.clone());
    Rc::new(move || {
      if !state.borrow().st.helper {
        return;
      }
      let (state, render_w) = (state.clone(), render_w.clone());
      glib::spawn_future_local(async move {
        let (_, out, _) = run(&["pkexec".into(), HELPER.into(), "list".into()], None).await;
        if let Ok(Value::Array(mut list)) = serde_json::from_str::<Value>(out.trim()) {
          list.sort_by_key(|p| p["name"].as_str().unwrap_or("").to_string());
          let mut st = state.borrow_mut();
          st.profiles = list;
          st.profiles_loaded = true;
        }
        if state.borrow().view != "edit" {
          call(&render_w);
        }
      });
    })
  };
  *profiles_w.borrow_mut() = Rc::downgrade(&read_profiles);

  // A command; on failure, its error shows.
  let action: Rc<dyn Fn(Vec<String>, bool)> = {
    let (state, status_w, profiles_w, render_w) = (state.clone(), status_w.clone(), profiles_w.clone(), render_w.clone());
    Rc::new(move |argv: Vec<String>, reload_profiles: bool| {
      if state.borrow().busy {
        return;
      }
      state.borrow_mut().busy = true;
      let (state, status_w, profiles_w, render_w) = (state.clone(), status_w.clone(), profiles_w.clone(), render_w.clone());
      glib::spawn_future_local(async move {
        let (ok, _, err) = run(&argv, None).await;
        {
          let mut st = state.borrow_mut();
          st.busy = false;
          if !ok {
            st.pending.clear();
            st.action_error = if err.trim().is_empty() { "Comando falhou".into() } else { err.trim().to_string() };
          }
        }
        if reload_profiles {
          call(&profiles_w);
        }
        call(&status_w);
        call(&render_w);
      });
    })
  };
  let connect_profile: Rc<dyn Fn(String)> = {
    let (state, action) = (state.clone(), action.clone());
    Rc::new(move |name: String| {
      if name.is_empty() {
        return;
      }
      {
        let mut st = state.borrow_mut();
        st.action_error.clear();
        st.pending = "up".into();
        st.pending_profile = name.clone();
      }
      action(vec!["bash".into(), format!("{}/vpnctl.sh", plugin_dir()), "connect".into(), name], false);
    })
  };
  let disconnect: Rc<dyn Fn()> = {
    let (state, action) = (state.clone(), action.clone());
    Rc::new(move || {
      let (external, pid) = {
        let mut st = state.borrow_mut();
        st.action_error.clear();
        st.pending = "down".into();
        (st.external(), st.st.external_pid)
      };
      let script = format!("{}/vpnctl.sh", plugin_dir());
      if external {
        action(vec!["bash".into(), script, "kill".into(), pid.to_string()], false);
      } else {
        action(vec!["bash".into(), script, "disconnect".into()], false);
      }
    })
  };
  let toggle_vpn: Rc<dyn Fn()> = {
    let (state, cp, dc, p, render_w) = (state.clone(), connect_profile.clone(), disconnect.clone(), Rc::downgrade(&panel), render_w.clone());
    Rc::new(move || {
      let (busy, on, current) = {
        let st = state.borrow();
        (st.busy, st.switch_on(), st.current())
      };
      if busy {
        return;
      }
      if on {
        dc();
      } else if !current.is_empty() {
        cp(current);
      } else if let Some(p) = p.upgrade() {
        p.open();
        state.borrow_mut().view = "list";
        call(&render_w);
      }
    })
  };

  // ---- Views.
  let render: Rc<dyn Fn()> = {
    let (state, body, panel_w, render_w) = (state.clone(), body.clone(), Rc::downgrade(&panel), render_w.clone());
    let (toggle_vpn, connect_profile, disconnect, action, profiles_w, status_w) = (toggle_vpn.clone(), connect_profile.clone(), disconnect.clone(), action.clone(), profiles_w.clone(), status_w.clone());
    let space = space.clone();
    Rc::new(move || {
      let Some(panel) = panel_w.upgrade() else { return };
      crate::popup::clear(&body);
      let view = state.borrow().view;
      let back: Rc<dyn Fn()> = {
        let (state, render_w, p) = (state.clone(), render_w.clone(), panel_w.clone());
        Rc::new(move || {
          let v = state.borrow().view;
          state.borrow_mut().confirm_delete.clear();
          match v {
            "edit" => state.borrow_mut().view = "list",
            "list" => state.borrow_mut().view = "main",
            _ => {
              if let Some(p) = p.upgrade() {
                p.close();
              }
              return;
            }
          }
          call(&render_w);
        })
      };
      let header = |title: &str, act: Option<(&str, &str, Rc<dyn Fn()>)>| {
        let h = ui::hbox(space(8.0));
        let b2 = back.clone();
        h.append(&ui::action(BACK, "Voltar", move || b2()));
        let t = ui::label(title, "panel-hero-title");
        t.set_hexpand(true);
        t.set_xalign(0.0);
        t.set_ellipsize(pango::EllipsizeMode::End);
        h.append(&t);
        if let Some((g, tip, f)) = act {
          h.append(&ui::action(g, tip, move || f()));
        }
        h
      };
      let error_label = |text: &str| {
        let l = ui::text(text);
        l.add_css_class("failed");
        l
      };
      let setup_notice = || {
        let c = ui::vbox(space(4.0));
        c.append(&error_label("Falta a configuração de sistema. Rode uma vez no terminal:"));
        let cmd = ui::text(&format!("sudo {}/setup-system.sh", plugin_dir()));
        cmd.set_wrap_mode(pango::WrapMode::Char);
        c.append(&cmd);
        c
      };
      let start_edit: Rc<dyn Fn(Option<Value>)> = {
        let (state, render_w) = (state.clone(), render_w.clone());
        Rc::new(move |p: Option<Value>| {
          {
            let mut st = state.borrow_mut();
            st.editing = Some(p.unwrap_or(Value::Null));
            st.form_error.clear();
            st.view = "edit";
          }
          call(&render_w);
        })
      };
      match view {
        "main" => {
          let st = state.borrow();
          let hero = ui::hbox(space(8.0));
          let h = ui::Hero::new(&space);
          h.set(if st.connected() { LOCK } else { VPN }, "FortiVPN", &st.status_text(now()));
          h.icon.set_opacity(if st.connected() { 1.0 } else { 0.5 });
          h.subtitle.remove_css_class("panel-hero-subtitle");
          h.subtitle.add_css_class("fortivpn-status");
          h.widget.set_hexpand(true);
          hero.append(&h.widget);
          let (state2, render_w2) = (state.clone(), render_w.clone());
          hero.append(&ui::action(COG, "Configurar conexões", move || {
            state2.borrow_mut().confirm_delete.clear();
            state2.borrow_mut().view = "list";
            call(&render_w2);
          }));
          if !st.current().is_empty() || st.connected() {
            let tv = toggle_vpn.clone();
            let sw = ui::state_switch(st.switch_on(), move || tv());
            sw.set_sensitive(!st.busy);
            sw.set_tooltip_text(Some(&if st.switch_on() { "Desconectar".to_string() } else { format!("Conectar a {}", st.current()) }));
            hero.append(&sw);
          }
          body.append(&hero);
          if !st.st.helper {
            body.append(&setup_notice());
          }
          let err = st.error_text();
          if !err.is_empty() {
            body.append(&error_label(&err));
          }
          if !st.st.cert.is_empty() && st.st.focus_state != "active" {
            let c = ui::vbox(space(4.0));
            let t = ui::text(&format!("O certificado do gateway não é confiável:\n{}", st.st.cert));
            t.add_css_class("dim");
            t.set_wrap_mode(pango::WrapMode::Char);
            c.append(&t);
            let (focus, digest, cp, state2, profiles_w2) = (st.st.focus.clone(), st.st.cert.clone(), connect_profile.clone(), state.clone(), profiles_w.clone());
            c.append(&ui::button("Confiar neste certificado e conectar", false, move || {
              let (focus, digest, cp, state2, profiles_w2) = (focus.clone(), digest.clone(), cp.clone(), state2.clone(), profiles_w2.clone());
              state2.borrow_mut().action_error.clear();
              glib::spawn_future_local(async move {
                let (ok, _, err) = run(&["pkexec".into(), HELPER.into(), "trust".into(), focus.clone(), digest], None).await;
                if ok {
                  cp(focus);
                } else {
                  state2.borrow_mut().action_error = if err.trim().is_empty() { "Não foi possível salvar o certificado".into() } else { err.trim().into() };
                }
                call(&profiles_w2);
              });
            }));
            body.append(&c);
          }
          if st.external() {
            let t = ui::text(&format!("Sessão iniciada fora do painel (PID {}). Desconectar pedirá sua senha.", st.st.external_pid));
            t.add_css_class("dim");
            body.append(&t);
          }
          if st.connected() {
            body.append(&ui::separator());
            for (k, v) in [("Interface", st.st.iface.clone()), ("Endereço", st.st.ip.clone()), ("Peer", st.st.peer.clone()), ("Rotas", st.st.routes.to_string())] {
              let r = ui::hbox(space(8.0));
              let l = ui::label(k, "power-info-label");
              l.set_hexpand(true);
              l.set_xalign(0.0);
              r.append(&l);
              r.append(&ui::label(&v, "power-info-value"));
              body.append(&r);
            }
          }
          if st.st.helper {
            body.append(&ui::separator());
            let col = ui::vbox(space(6.0));
            col.append(&ui::section("Conexões"));
            if st.profiles_loaded && st.profiles.is_empty() {
              let t = ui::label("Nenhuma conexão cadastrada.", "dim");
              t.set_xalign(0.0);
              col.append(&t);
              let se = start_edit.clone();
              col.append(&ui::button(&format!("{PLUS}  Nova conexão"), false, move || se(None)));
            }
            for p in &st.profiles {
              let name = p["name"].as_str().unwrap_or("").to_string();
              let pstate = st.profile_state(&name);
              let r = ui::hbox(space(8.0));
              let g = ui::label(if pstate == "connected" { LOCK } else { VPN }, if pstate == "failed" { "fortivpn-glyph failed" } else { "fortivpn-glyph" });
              g.set_opacity(if pstate.is_empty() { 0.5 } else { 1.0 });
              r.append(&g);
              let names = ui::vbox(space(1.0));
              names.set_hexpand(true);
              let n = ui::label(&name, "network-ssid");
              n.set_xalign(0.0);
              names.append(&n);
              let port = p["port"].as_str().map(String::from).or_else(|| p["port"].as_u64().map(|n| n.to_string())).unwrap_or_else(|| "443".into());
              let detail = ui::label(&format!("{} @ {}:{port}", p["username"].as_str().unwrap_or(""), p["host"].as_str().unwrap_or("")), "network-status");
              detail.set_xalign(0.0);
              detail.set_ellipsize(pango::EllipsizeMode::End);
              names.append(&detail);
              r.append(&names);
              let label = match pstate {
                "connected" => "Conectado",
                "activating" => "Conectando…",
                "deactivating" => "Saindo…",
                "failed" => "Falhou",
                _ => "",
              };
              r.append(&ui::label(label, if pstate == "failed" { "network-status failed" } else { "network-status" }));
              let (cp, dc, name2, state2) = (connect_profile.clone(), disconnect.clone(), name.clone(), state.clone());
              let row = ui::row(&r, move |_| {
                if state2.borrow().busy {
                  return;
                }
                let s = state2.borrow().profile_state(&name2);
                if s == "connected" || s == "activating" {
                  dc();
                } else {
                  cp(name2.clone());
                }
              });
              if pstate == "connected" {
                row.add_css_class("selected");
              }
              row.set_tooltip_text(Some(if pstate.is_empty() { "Conectar" } else { "" }));
              col.append(&row);
            }
            body.append(&col);
          }
        }
        "list" => {
          let st = state.borrow();
          let se = start_edit.clone();
          body.append(&header("Conexões", st.st.helper.then(|| (PLUS, "Nova conexão", Rc::new(move || se(None)) as Rc<dyn Fn()>))));
          if !st.st.helper {
            body.append(&setup_notice());
          } else if st.profiles.is_empty() {
            let t = ui::text("Nenhuma conexão ainda. Use + para criar a primeira.");
            t.add_css_class("dim");
            body.append(&t);
          }
          if !st.action_error.is_empty() {
            body.append(&error_label(&st.action_error));
          }
          for p in &st.profiles {
            let name = p["name"].as_str().unwrap_or("").to_string();
            let confirming = st.confirm_delete == name;
            let r = ui::hbox(space(6.0));
            let names = ui::vbox(space(1.0));
            names.set_hexpand(true);
            let n = ui::label(&name, "network-ssid");
            n.set_xalign(0.0);
            names.append(&n);
            let port = p["port"].as_str().map(String::from).or_else(|| p["port"].as_u64().map(|n| n.to_string())).unwrap_or_else(|| "443".into());
            let d = ui::label(
              &if confirming { "Clique na lixeira de novo para apagar".to_string() } else { format!("{}:{port}", p["host"].as_str().unwrap_or("")) },
              if confirming { "network-status failed" } else { "network-status" },
            );
            d.set_xalign(0.0);
            names.append(&d);
            r.append(&names);
            let (se, p2) = (start_edit.clone(), p.clone());
            r.append(&ui::action(PENCIL, "Editar", move || se(Some(p2.clone()))));
            let (state2, action2, render_w2, name2) = (state.clone(), action.clone(), render_w.clone(), name.clone());
            let trash = ui::action(TRASH, if confirming { "Confirmar exclusão" } else { "Apagar" }, move || {
              let confirm = state2.borrow().confirm_delete == name2;
              if !confirm {
                state2.borrow_mut().confirm_delete = name2.clone();
                call(&render_w2);
                return;
              }
              state2.borrow_mut().confirm_delete.clear();
              action2(vec!["pkexec".into(), HELPER.into(), "delete".into(), name2.clone()], true);
            });
            if confirming {
              trash.add_css_class("failed");
            }
            r.append(&trash);
            let (se, p2) = (start_edit.clone(), p.clone());
            body.append(&ui::row(&r, move |_| se(Some(p2.clone()))));
          }
        }
        _ => {
          let st = state.borrow();
          let p = st.editing.clone().unwrap_or(Value::Null);
          let original = p["name"].as_str().unwrap_or("").to_string();
          let has_password = p["hasPassword"].as_bool() == Some(true);
          body.append(&header(&if original.is_empty() { "Nova conexão".to_string() } else { format!("Editar {original}") }, None));
          let grid = gtk::Grid::new();
          grid.set_column_spacing(space(10.0) as u32);
          grid.set_row_spacing(space(8.0) as u32);
          let field = |row: i32, label: &str, value: &str, placeholder: &str| {
            let l = ui::label(label, "power-info-label");
            l.set_xalign(0.0);
            grid.attach(&l, 0, row, 1, 1);
            let e = gtk::Entry::new();
            e.add_css_class("field");
            e.set_hexpand(true);
            e.set_text(value);
            e.set_placeholder_text(Some(placeholder));
            grid.attach(&e, 1, row, 1, 1);
            e
          };
          let port = p["port"].as_str().map(String::from).or_else(|| p["port"].as_u64().map(|n| n.to_string())).unwrap_or_else(|| "443".into());
          let name_e = field(0, "Nome", &original, "empresa");
          let host_e = field(1, "Gateway", p["host"].as_str().unwrap_or(""), "vpn.empresa.com.br");
          let port_e = field(2, "Porta", &port, "443");
          let user_e = field(3, "Usuário", p["username"].as_str().unwrap_or(""), "");
          let pass_l = ui::label("Senha", "power-info-label");
          pass_l.set_xalign(0.0);
          grid.attach(&pass_l, 0, 4, 1, 1);
          let pass_e = gtk::PasswordEntry::new();
          pass_e.add_css_class("field");
          pass_e.set_show_peek_icon(true);
          pass_e.set_property("placeholder-text", if has_password { "manter a atual" } else { "" });
          grid.attach(&pass_e, 1, 4, 1, 1);
          let realm_e = field(5, "Realm", p["realm"].as_str().unwrap_or(""), "opcional");
          let certs = p["trustedCerts"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).unwrap_or_default();
          let cert_e = field(6, "Certificado", &certs, "sha256 confiável (opcional)");
          body.append(&grid);
          let toggle_row = |title: &str, desc: &str, on: bool| {
            let r = ui::hbox(space(10.0));
            let c = ui::vbox(space(2.0));
            c.set_hexpand(true);
            let t = ui::label(title, "network-ssid");
            t.set_xalign(0.0);
            c.append(&t);
            let d = ui::text(desc);
            d.add_css_class("dim");
            c.append(&d);
            r.append(&c);
            let sw = gtk::Switch::new();
            sw.set_active(on);
            sw.set_valign(gtk::Align::Center);
            r.append(&sw);
            body.append(&r);
            sw
          };
          let dns = toggle_row("Usar DNS da VPN", "Aplica os servidores DNS enviados pelo gateway.", p["setDns"].as_bool() != Some(false));
          let routes = toggle_row("Configurar rotas", "Adiciona as rotas enviadas pelo gateway.", p["setRoutes"].as_bool() != Some(false));
          let accept = toggle_row(
            "Aceitar IP remoto (pppd ≥ 2.5)",
            "Equivale a --pppd-accept-remote; necessário no pppd 2.5 ou mais novo.",
            if p.is_null() { true } else { p["acceptRemote"].as_bool() == Some(true) },
          );
          let error = error_label(&st.form_error);
          error.set_visible(!st.form_error.is_empty());
          body.append(&error);
          let buttons = ui::hbox(space(8.0));
          buttons.set_halign(gtk::Align::End);
          let b2 = back.clone();
          buttons.append(&ui::button("Cancelar", false, move || b2()));
          let save: Rc<dyn Fn()> = {
            let (state, profiles_w, render_w, error) = (state.clone(), profiles_w.clone(), render_w.clone(), error.clone());
            let (name_e, host_e, port_e, user_e, pass_e, realm_e, cert_e) = (name_e.clone(), host_e.clone(), port_e.clone(), user_e.clone(), pass_e.clone(), realm_e.clone(), cert_e.clone());
            let (dns, routes, accept, original) = (dns.clone(), routes.clone(), accept.clone(), original.clone());
            Rc::new(move || {
              let name = name_e.text().trim().to_string();
              let fail = |msg: &str| {
                error.set_text(msg);
                error.set_visible(true);
              };
              if !valid_name(&name) {
                return fail("Nome: use letras, números, - ou _ (até 32).");
              }
              if host_e.text().trim().is_empty() {
                return fail("Informe o host do gateway.");
              }
              if user_e.text().trim().is_empty() {
                return fail("Informe o usuário.");
              }
              if !has_password && pass_e.text().is_empty() {
                return fail("Informe a senha.");
              }
              error.set_visible(false);
              let payload = serde_json::json!({
                "host": host_e.text().trim(), "port": port_e.text().trim(), "username": user_e.text().trim(),
                "password": pass_e.text().as_str(), "realm": realm_e.text().trim(), "trustedCerts": cert_e.text().trim(),
                "setDns": dns.is_active(), "setRoutes": routes.is_active(), "acceptRemote": accept.is_active(),
              });
              let mut argv = vec!["pkexec".to_string(), HELPER.into(), "save".into(), name.clone()];
              if !original.is_empty() && original != name {
                argv.push(original.clone());
              }
              let (state, profiles_w, render_w, error) = (state.clone(), profiles_w.clone(), render_w.clone(), error.clone());
              glib::spawn_future_local(async move {
                let (ok, _, err) = run(&argv, Some(format!("{payload}\n"))).await;
                if !ok {
                  error.set_text(if err.trim().is_empty() { "Não foi possível salvar" } else { err.trim() });
                  error.set_visible(true);
                  return;
                }
                state.borrow_mut().view = "list";
                call(&profiles_w);
                call(&render_w);
              });
            })
          };
          let s2 = save.clone();
          buttons.append(&ui::button("Salvar", false, move || s2()));
          body.append(&buttons);
          for e in [&name_e, &host_e, &port_e, &user_e, &realm_e, &cert_e] {
            let s2 = save.clone();
            e.connect_activate(move |_| s2());
          }
          let s2 = save.clone();
          pass_e.connect_activate(move |_| s2());
          let focus: gtk::Widget = if original.is_empty() { name_e.upcast() } else { host_e.upcast() };
          glib::idle_add_local_once(move || {
            focus.grab_focus();
          });
        }
      }
      let _ = &status_w;
      panel.relayout();
    })
  };
  *render_w.borrow_mut() = Rc::downgrade(&render);
  let keep = (render.clone(), read_status.clone(), read_profiles.clone());
  button.widget.connect_destroy(move |_| {
    let _ = &keep;
  });
  read_status();
  // Every refreshIntervalSec, every second while it changes state; the
  // connected time ticks while open.
  {
    let (rs, state2, w, render2, panel_w) = (read_status.clone(), state.clone(), button.widget.downgrade(), render.clone(), Rc::downgrade(&panel));
    glib::spawn_future_local(async move {
      let mut last = Instant::now();
      loop {
        glib::timeout_future(Duration::from_secs(1)).await;
        if w.upgrade().is_none() {
          break;
        }
        let fast = state2.borrow().transitioning();
        if fast || last.elapsed() >= Duration::from_secs(interval) {
          last = Instant::now();
          rs();
        } else if panel_w.upgrade().is_some_and(|p| p.is_open()) && state2.borrow().connected() && state2.borrow().view == "main" {
          render2();
        }
      }
    });
  }
  {
    let (state2, rs, rp, render2) = (state.clone(), read_status.clone(), read_profiles.clone(), render.clone());
    panel.connect_open(move || {
      {
        let mut st = state2.borrow_mut();
        st.view = "main";
        st.confirm_delete.clear();
      }
      render2();
      rs();
      rp();
    });
    let (state2, tv, rs, rp, render2, p) = (state.clone(), toggle_vpn.clone(), read_status.clone(), read_profiles.clone(), render.clone(), Rc::downgrade(&panel));
    panel.connect_key(move |k, _| {
      let view = state2.borrow().view;
      if k == gdk::Key::Escape {
        match view {
          "edit" => state2.borrow_mut().view = "list",
          "list" => state2.borrow_mut().view = "main",
          _ => {
            if let Some(p) = p.upgrade() {
              p.close();
            }
            return true;
          }
        }
        render2();
        return true;
      }
      if view != "main" {
        return false;
      }
      match k {
        gdk::Key::c | gdk::Key::C | gdk::Key::space => tv(),
        gdk::Key::r | gdk::Key::R => {
          rs();
          rp();
        }
        gdk::Key::s | gdk::Key::S => {
          state2.borrow_mut().view = "list";
          render2();
        }
        _ => return false,
      }
      true
    });
  }
  let (p, tv) = (panel.clone(), toggle_vpn.clone());
  button.on_press(move |b| if b == 3 { tv() } else { p.toggle() });
  if ctx.first_screen {
    if let Some(bar) = ctx.bar.upgrade() {
      let (p, state2, render2, cp, dc, tv) = (Rc::downgrade(&panel), state.clone(), render.clone(), connect_profile.clone(), disconnect.clone(), toggle_vpn.clone());
      bar.add_ipc("doxia.fortivpn", Box::new(move |method, args| {
        let panel = p.upgrade()?;
        Some(match method {
          "open" | "show" => {
            panel.open();
            "ok".into()
          }
          "close" | "hide" => {
            panel.close();
            "ok".into()
          }
          "toggle" => {
            panel.toggle();
            "ok".into()
          }
          "settings" => {
            panel.open();
            state2.borrow_mut().view = "list";
            render2();
            "ok".into()
          }
          "newProfile" => {
            panel.open();
            {
              let mut st = state2.borrow_mut();
              st.editing = Some(Value::Null);
              st.view = "edit";
            }
            render2();
            "ok".into()
          }
          "connect" => {
            let name = args.first().filter(|a| !a.is_empty()).cloned().unwrap_or_else(|| state2.borrow().current());
            cp(name);
            "ok".into()
          }
          "disconnect" => {
            dc();
            "ok".into()
          }
          "toggleVpn" => {
            tv();
            "ok".into()
          }
          "status" => state2.borrow().status_text(now()),
          _ => return None,
        })
      }));
    }
  }
  slot.widget.clone().upcast()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn durations_and_names() {
    assert_eq!(duration(5.0), "5s");
    assert_eq!(duration(125.0), "2m 05s");
    assert_eq!(duration(3700.0), "1h 01m");
    assert!(valid_name("empresa_01"));
    assert!(!valid_name("com espaço"));
    assert!(!valid_name(""));
  }
}
