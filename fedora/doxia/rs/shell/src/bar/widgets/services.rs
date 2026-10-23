// doxia.services (fedora/doxia/plugins/doxia.services): the development
// services of ~/.config/omarchy/services.json, run by the plugin's `svc`
// script as transient user units. The icon is dim while none runs, pulses
// while one is busy, and carries a red dot when one fell or its build
// failed. Its panel: each service with its state and uptime, start or
// stop, restart, build and log, its git branch (searchable) and its node
// or java version.

use super::super::panel::Panel;
use super::super::ui;
use super::super::widget::{Button, Kind, Slot};
use super::super::Ctx;
use gtk::prelude::*;
use gtk::{glib, pango};
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

fn svc() -> String {
  format!("{}/fedora/doxia/plugins/doxia.services/svc", std::env::var("OMARCHY_PATH").unwrap_or_default())
}

pub fn uptime(seconds: f64) -> String {
  let s = seconds.max(0.0).floor() as u64;
  if s < 60 {
    format!("{s}s")
  } else if s < 3600 {
    format!("{}min", s / 60)
  } else if s < 86400 {
    let m = (s / 60) % 60;
    if m > 0 { format!("{}h{m:02}", s / 3600) } else { format!("{}h", s / 3600) }
  } else {
    format!("{}d", s / 86400)
  }
}

pub fn state_text(s: &Value, now: f64) -> String {
  let state = s["state"].as_str().unwrap_or("");
  let mut t = match state {
    "busy" => format!("{}…", s["task"].as_str().unwrap_or("")),
    "running" => {
      let since = s["since"].as_f64().unwrap_or(0.0);
      let up = if since > 0.0 { uptime(now - since) } else { String::new() };
      let port = s["port"].as_u64().map(|p| format!(" · :{p}")).or_else(|| s["port"].as_str().filter(|p| !p.is_empty()).map(|p| format!(" · :{p}"))).unwrap_or_default();
      format!("rodando {up}{port}")
    }
    "stopping" => "parando…".into(),
    "failed" => "caiu — veja o log".into(),
    _ if s["taskFailed"].as_bool() == Some(true) => "build falhou — veja o log".into(),
    _ => "parado".into(),
  };
  if s["dirty"].as_bool() == Some(true) {
    t.push_str(" · alterações locais");
  }
  t
}

#[derive(Default)]
struct State {
  config: bool,
  services: Vec<Value>,
  branches: Value,
  runtimes: Value,
  read_at: f64,
}

fn now() -> f64 {
  std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

pub fn build(ctx: &Rc<Ctx>) -> gtk::Widget {
  let space = { let s = ctx.style.clone(); move |px: f64| s.space(px) };
  let button = Button::new(ctx, Kind::Icon, "\u{f048b}");
  // The fail dot over the glyph's corner.
  let overlay = gtk::Overlay::new();
  overlay.set_child(Some(&button.widget));
  let dot = gtk::DrawingArea::new();
  dot.set_can_target(false);
  let bg = ctx.style.background;
  dot.set_draw_func(move |_, cr, w, h| {
    let r = 4.5;
    let (x, y) = (w as f64 / 2.0 + 5.0, h as f64 / 2.0 - 6.0);
    cr.arc(x, y, r + 1.5, 0.0, std::f64::consts::TAU);
    cr.set_source_rgba(bg[0], bg[1], bg[2], 1.0);
    let _ = cr.fill();
    cr.arc(x, y, r, 0.0, std::f64::consts::TAU);
    cr.set_source_rgb(0xe5 as f64 / 255.0, 0x25 as f64 / 255.0, 0x2a as f64 / 255.0);
    let _ = cr.fill();
  });
  dot.set_visible(false);
  overlay.add_overlay(&dot);
  let slot = Slot::new(ctx, &overlay);
  let panel = Panel::new(ctx, &overlay, &slot, space(470.0));
  let st = Rc::new(RefCell::new(State { config: true, ..State::default() }));
  let body = ui::vbox(space(12.0));
  panel.content.append(&body);
  // A dropdown open keeps the list from being rebuilt under it.
  let menus_open = Rc::new(Cell::new(0));
  // Nor is it rebuilt while a press is on it (a touch tap lasts long enough
  // for a 1.5 s refresh to replace the icon under the finger and lose the
  // tap), nor when what it shows has not changed.
  let pressed_at = Rc::new(Cell::new(None::<Instant>));
  {
    let press = gtk::GestureClick::new();
    press.set_button(0);
    press.set_propagation_phase(gtk::PropagationPhase::Capture);
    let p = pressed_at.clone();
    press.connect_pressed(move |_, _, _, _| p.set(Some(Instant::now())));
    body.add_controller(press);
  }
  let shown = Rc::new(RefCell::new(String::new()));

  let again: Rc<RefCell<std::rc::Weak<dyn Fn()>>> = Rc::new(RefCell::new(std::rc::Weak::<Box<dyn Fn()>>::new() as std::rc::Weak<dyn Fn()>));
  let refresh: Rc<dyn Fn()> = {
    let (st, again) = (st.clone(), again.clone());
    Rc::new(move || {
      let (st, again) = (st.clone(), again.clone());
      glib::spawn_future_local(async move {
        let out = crate::watch::output(&[&svc(), "status"]).await.unwrap_or_default();
        if let Ok(data) = serde_json::from_str::<Value>(&out) {
          let mut s = st.borrow_mut();
          s.config = data["config"].as_bool().unwrap_or(false);
          s.services = data["services"].as_array().cloned().unwrap_or_default();
          s.read_at = now();
        }
        if let Some(f) = again.borrow().upgrade() {
          f();
        }
      });
    })
  };
  let run: Rc<dyn Fn(Vec<String>)> = {
    let r = refresh.clone();
    Rc::new(move |args: Vec<String>| {
      let mut argv = vec![svc()];
      argv.extend(args);
      let a: Vec<&str> = argv.iter().map(String::as_str).collect();
      crate::run::argv(&a);
      let r = r.clone();
      glib::timeout_add_local_once(Duration::from_millis(400), move || r());
    })
  };
  let read_lists: Rc<dyn Fn()> = {
    let (st, again) = (st.clone(), again.clone());
    Rc::new(move || {
      let (st, again) = (st.clone(), again.clone());
      glib::spawn_future_local(async move {
        let b = crate::watch::output(&[&svc(), "branches-all"]).await.unwrap_or_default();
        let r = crate::watch::output(&[&svc(), "runtimes"]).await.unwrap_or_default();
        {
          let mut s = st.borrow_mut();
          if let Ok(v) = serde_json::from_str(&b) {
            s.branches = v;
          }
          if let Ok(v) = serde_json::from_str(&r) {
            s.runtimes = v;
          }
        }
        if let Some(f) = again.borrow().upgrade() {
          f();
        }
      });
    })
  };

  let pulse_since = Rc::new(Cell::new(None::<Instant>));
  let render: Rc<dyn Fn()> = {
    let (st, b, dot, body, panel_w, run, menus_open, pulse_since) = (st.clone(), button.clone(), dot.clone(), body.clone(), Rc::downgrade(&panel), run.clone(), menus_open.clone(), pulse_since.clone());
    let (pressed_at, shown, again) = (pressed_at.clone(), shown.clone(), again.clone());
    let (refresh, read_lists) = (refresh.clone(), read_lists.clone());
    let space = space.clone();
    Rc::new(move || {
      let s = st.borrow();
      let running = s.services.iter().filter(|x| x["state"] == "running").count();
      let busy = s.services.iter().any(|x| x["state"] == "busy");
      let failed = s.services.iter().any(|x| x["state"] == "failed" || x["taskFailed"].as_bool() == Some(true));
      b.set_dimmed(running == 0 && !busy);
      dot.set_visible(failed);
      b.set_tooltip(&if !s.config {
        "Serviços: sem config".to_string()
      } else {
        format!("{}{running} de {} serviços rodando", if failed { "Algum serviço caiu ou o build falhou — " } else { "" }, s.services.len())
      });
      if busy && pulse_since.get().is_none() {
        pulse_since.set(Some(Instant::now()));
        let (w, ps) = (b.widget.downgrade(), pulse_since.clone());
        glib::timeout_add_local(Duration::from_millis(33), move || {
          let (Some(w), Some(t)) = (w.upgrade(), ps.get()) else {
            if let Some(w) = w.upgrade() {
              w.set_opacity(1.0);
            }
            return glib::ControlFlow::Break;
          };
          let phase = (t.elapsed().as_secs_f64() / 1.4).fract();
          w.set_opacity(0.4 + 0.6 * ((phase * std::f64::consts::TAU).cos() * 0.5 + 0.5));
          glib::ControlFlow::Continue
        });
      } else if !busy {
        pulse_since.set(None);
      }
      let Some(panel) = panel_w.upgrade() else { return };
      if !panel.is_open() || menus_open.get() > 0 {
        return;
      }
      const HOLD: Duration = Duration::from_millis(800);
      if let Some(t) = pressed_at.get().filter(|t| t.elapsed() < HOLD) {
        let again = again.clone();
        glib::timeout_add_local_once(HOLD - t.elapsed(), move || {
          if let Some(f) = again.borrow().upgrade() {
            f();
          }
        });
        return;
      }
      let key = format!("{}|{}|{}", s.services.iter().map(|x| format!("{x}{}", state_text(x, s.read_at))).collect::<String>(), s.branches, s.runtimes);
      if *shown.borrow() == key {
        return;
      }
      *shown.borrow_mut() = key;
      crate::popup::clear(&body);
      let hero = ui::hbox(space(14.0));
      hero.append(&ui::label("\u{f048b}", "panel-hero-icon"));
      let labels = ui::vbox(space(2.0));
      labels.set_hexpand(true);
      labels.set_valign(gtk::Align::Center);
      let t = ui::label("Serviços", "panel-hero-title");
      t.set_xalign(0.0);
      labels.append(&t);
      let sub = if s.services.is_empty() { "NENHUM CONFIGURADO".to_string() } else { format!("{running} DE {} RODANDO", s.services.len()) };
      let sl = ui::label(&sub, "panel-hero-subtitle");
      sl.set_xalign(0.0);
      labels.append(&sl);
      hero.append(&labels);
      let p2 = panel_w.clone();
      hero.append(&ui::action("\u{f03eb}", "Editar services.json", move || {
        let home = std::env::var("HOME").unwrap_or_default();
        crate::run::argv(&["omarchy-launch-editor", &format!("{home}/.config/omarchy/services.json")]);
        if let Some(p) = p2.upgrade() {
          p.close();
        }
      }));
      let (r1, r2) = (refresh.clone(), read_lists.clone());
      hero.append(&ui::action("\u{f0450}", "Atualizar branches e versões", move || {
        r1();
        r2();
      }));
      body.append(&hero);
      body.append(&ui::separator());
      if s.services.is_empty() {
        let l = ui::text("Nenhum serviço ainda. No Claude Code, use /servicos para adicionar um projeto node ou java.");
        l.add_css_class("dim");
        body.append(&l);
      }
      let now = s.read_at;
      for svc_v in &s.services {
        let id = svc_v["id"].as_str().unwrap_or("").to_string();
        let state = svc_v["state"].as_str().unwrap_or("");
        let is_running = state == "running";
        let is_busy = state == "busy" || state == "stopping";
        let row = ui::vbox(space(6.0));
        row.set_margin_bottom(space(6.0));
        let top = ui::hbox(space(10.0));
        let dot_l = ui::label("\u{25cf}", match state {
          "running" => "service-dot running",
          "failed" => "service-dot failed",
          _ if is_busy => "service-dot busy",
          _ => "service-dot",
        });
        top.append(&dot_l);
        let names = ui::vbox(space(1.0));
        names.set_hexpand(true);
        let name_row = ui::hbox(space(8.0));
        name_row.append(&ui::label(svc_v["name"].as_str().unwrap_or(&id), "service-name"));
        if let Some(ty) = svc_v["type"].as_str().filter(|t| !t.is_empty()) {
          name_row.append(&ui::label(ty, "service-type"));
        }
        names.append(&name_row);
        let bad = state == "failed" || (svc_v["taskFailed"].as_bool() == Some(true) && !is_running);
        let stl = ui::label(&state_text(svc_v, now), if bad { "service-state failed" } else { "service-state" });
        stl.set_xalign(0.0);
        stl.set_ellipsize(pango::EllipsizeMode::End);
        names.append(&stl);
        top.append(&names);
        let actions = ui::hbox(space(6.0));
        let (r, i) = (run.clone(), id.clone());
        let start = ui::action(if is_running { "\u{f04db}" } else { "\u{f040a}" }, if is_running { "Parar" } else { "Iniciar" }, move || {
          if !is_busy {
            r(vec![if is_running { "stop".into() } else { "start".into() }, i.clone()]);
          }
        });
        start.add_css_class(if is_running { "stop" } else { "start" });
        start.set_opacity(if is_busy { 0.35 } else { 1.0 });
        actions.append(&start);
        if is_running {
          let (r, i) = (run.clone(), id.clone());
          actions.append(&ui::action("\u{f0709}", "Reiniciar", move || r(vec!["restart".into(), i.clone()])));
        }
        if svc_v["hasBuild"].as_bool() == Some(true) {
          let (r, i) = (run.clone(), id.clone());
          let bl = ui::action("\u{f1322}", "Rodar os passos de build (sem trocar de branch)", move || {
            if !is_busy {
              r(vec!["build".into(), i.clone()]);
            }
          });
          bl.set_opacity(if is_busy { 0.35 } else { 1.0 });
          actions.append(&bl);
        }
        let (r, i, p2) = (run.clone(), id.clone(), panel_w.clone());
        let log = ui::action("\u{f018d}", "Log", move || {
          r(vec!["log".into(), i.clone()]);
          if let Some(p) = p2.upgrade() {
            p.close();
          }
        });
        if svc_v["taskFailed"].as_bool() == Some(true) {
          log.add_css_class("failed");
        }
        actions.append(&log);
        top.append(&actions);
        row.append(&top);

        // Branch and runtime.
        let line = ui::hbox(space(6.0));
        line.set_margin_start(space(19.0));
        let make_dropdown = |options: Vec<String>, labels: Vec<String>, current: &str, search: bool| {
          let model = gtk::StringList::new(&labels.iter().map(String::as_str).collect::<Vec<_>>());
          let dd = gtk::DropDown::new(Some(model), None::<gtk::Expression>);
          dd.add_css_class("choice");
          dd.set_enable_search(search);
          if search {
            dd.set_expression(Some(gtk::PropertyExpression::new(gtk::StringObject::static_type(), None::<gtk::Expression>, "string")));
          }
          if let Some(i) = options.iter().position(|o| o == current) {
            dd.set_selected(i as u32);
          }
          dd.set_sensitive(!is_busy);
          (dd, options)
        };
        let branch = svc_v["branch"].as_str().unwrap_or("").to_string();
        if svc_v["git"].as_bool() == Some(true) {
          line.append(&ui::label("\u{f062c}", "service-line-icon"));
          let mut list: Vec<String> = s.branches[&id].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
          if !branch.is_empty() && !list.contains(&branch) {
            list.insert(0, branch.clone());
          }
          let (dd, options) = make_dropdown(list.clone(), list, &branch, true);
          dd.set_hexpand(true);
          let (r, i, cur) = (run.clone(), id.clone(), branch.clone());
          dd.connect_selected_notify(move |d| {
            if let Some(v) = options.get(d.selected() as usize) {
              if *v != cur {
                r(vec!["checkout".into(), i.clone(), v.clone()]);
              }
            }
          });
          track_popup(&dd, &menus_open);
          line.append(&dd);
        } else {
          let spacer = ui::hbox(0);
          spacer.set_hexpand(true);
          line.append(&spacer);
        }
        let ty = svc_v["type"].as_str().unwrap_or("");
        line.append(&ui::label(if ty == "java" { "\u{f0b37}" } else { "\u{f0399}" }, "service-line-icon"));
        let runtime = svc_v["runtime"].as_str().unwrap_or("").to_string();
        let hint = svc_v["runtimeHint"].as_str().unwrap_or("");
        let mut values: Vec<String> = s.runtimes[ty].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
        if !runtime.is_empty() && !values.contains(&runtime) {
          values.insert(0, runtime.clone());
        }
        let mut options = vec![String::new()];
        options.extend(values.iter().cloned());
        let mut labels = vec![if hint.is_empty() { "auto".to_string() } else { format!("auto · {hint}") }];
        labels.extend(values);
        let (dd, options) = make_dropdown(options, labels, &runtime, false);
        dd.set_size_request(space(150.0), -1);
        let (r, i, cur) = (run.clone(), id.clone(), runtime.clone());
        dd.connect_selected_notify(move |d| {
          if let Some(v) = options.get(d.selected() as usize) {
            if *v != cur {
              let mut a = vec!["runtime".to_string(), i.clone()];
              if !v.is_empty() {
                a.push(v.clone());
              }
              r(a);
            }
          }
        });
        track_popup(&dd, &menus_open);
        line.append(&dd);
        row.append(&line);
        body.append(&row);
      }
      drop(s);
      panel.relayout();
    })
  };
  *again.borrow_mut() = Rc::downgrade(&render);
  let keep = render.clone();
  button.widget.connect_destroy(move |_| {
    let _ = &keep;
  });
  refresh();
  // Every 6 s, 1.5 s while open or busy.
  {
    let (r, st2, panel_w, w) = (refresh.clone(), st.clone(), Rc::downgrade(&panel), button.widget.downgrade());
    glib::spawn_future_local(async move {
      loop {
        let fast = panel_w.upgrade().is_some_and(|p| p.is_open()) || st2.borrow().services.iter().any(|x| x["state"] == "busy");
        glib::timeout_future(Duration::from_millis(if fast { 1500 } else { 6000 })).await;
        if w.upgrade().is_none() {
          break;
        }
        r();
      }
    });
  }
  {
    let (r, l, shown) = (refresh.clone(), read_lists.clone(), shown.clone());
    panel.connect_open(move || {
      shown.borrow_mut().clear();
      r();
      l();
    });
  }
  if ctx.first_screen {
    if let Some(bar) = ctx.bar.upgrade() {
      let p = Rc::downgrade(&panel);
      bar.add_ipc("doxia.services", Box::new(move |method, _| {
        let panel = p.upgrade()?;
        match method {
          "open" | "show" => panel.open(),
          "close" | "hide" => panel.close(),
          "toggle" => panel.toggle(),
          _ => return None,
        }
        Some("ok".into())
      }));
    }
  }
  let p = panel.clone();
  button.on_press(move |_| p.toggle());
  slot.widget.clone().upcast()
}

// The dropdown's list open, counted, so the panel waits to redraw.
fn track_popup(dd: &gtk::DropDown, open: &Rc<Cell<i32>>) {
  if let Some(pop) = dd.last_child().and_downcast::<gtk::Popover>() {
    let o = open.clone();
    pop.connect_show(move |_| o.set(o.get() + 1));
    let o = open.clone();
    pop.connect_hide(move |_| o.set((o.get() - 1).max(0)));
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn states() {
    assert_eq!(uptime(42.0), "42s");
    assert_eq!(uptime(3900.0), "1h05");
    assert_eq!(uptime(7200.0), "2h");
    assert_eq!(state_text(&json!({"state": "running", "since": 100.0, "port": 3000}), 160.0), "rodando 1min · :3000");
    assert_eq!(state_text(&json!({"state": "stopped", "taskFailed": true, "dirty": true}), 0.0), "build falhou — veja o log · alterações locais");
  }
}
