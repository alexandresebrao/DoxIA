// System widgets of the bar: the keyboard layout and the update icon.

use super::super::panel::Panel;
use super::super::ui;
use super::super::widget::{Button, Kind, Slot};
use super::super::Ctx;
use crate::mango;
use gtk::glib;
use gtk::prelude::*;
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

#[allow(dead_code)]
#[path = "../../../../atualizar/src/model.rs"]
mod update_model;

// KeyboardLayoutModel.layoutBriefs: xkbcli's YAML pairs each layout's
// brief ("en") with its description ("English (US)").
pub fn layout_briefs(text: &str) -> HashMap<String, String> {
  let mut out = HashMap::new();
  let mut brief = String::new();
  for line in text.lines() {
    if line.trim_start().starts_with("- ") {
      brief.clear();
    }
    if let Some(v) = line.strip_prefix("  brief: ") {
      brief = v.trim_matches('\'').to_string();
    } else if let Some(v) = line.strip_prefix("  description: ") {
      if !brief.is_empty() {
        out.insert(v.to_string(), std::mem::take(&mut brief));
      }
    }
  }
  out
}

// The brief without its script, or the description's first word; three
// letters, upper case.
pub fn short_label(description: &str, briefs: &HashMap<String, String>) -> String {
  if description.is_empty() {
    return String::new();
  }
  let label = match briefs.get(description).filter(|b| !b.is_empty()) {
    Some(b) => b.split('-').next().unwrap_or("").to_string(),
    None => description.split_whitespace().next().unwrap_or("").to_string(),
  };
  label.chars().take(3).collect::<String>().to_uppercase()
}

// omarchy.keyboard-layout (KeyboardLayout.qml): the active layout's short
// code, only when there is more than one to switch between; a click
// switches.
pub fn keyboard_layout(ctx: &Rc<Ctx>) -> gtk::Widget {
  let button = Button::new(ctx, Kind::Text, "");
  button.widget.add_css_class("caption");
  let pad = (6.0 * ctx.style.spacing).round() as i32;
  button.label.set_margin_start(pad);
  button.label.set_margin_end(pad);
  button.widget.set_visible(false);
  button.on_press(|_| mango::dispatch(&["switch_keyboard_layout"]));
  let briefs: Rc<RefCell<HashMap<String, String>>> = Rc::default();
  let multiple = Rc::new(Cell::new(false));
  let last = Rc::new(RefCell::new(String::new()));
  let paint = {
    let (b, briefs, multiple) = (button.clone(), briefs.clone(), multiple.clone());
    move || {
      let full = mango::get().keyboard_layout();
      let text = short_label(&full, &briefs.borrow());
      b.set_text(if multiple.get() { &text } else { "" });
      b.set_tooltip(&full);
    }
  };
  let check_layouts = {
    let (multiple, paint) = (multiple.clone(), paint.clone());
    move || {
      let (multiple, paint) = (multiple.clone(), paint.clone());
      glib::spawn_future_local(async move {
        let out = crate::watch::output(&["omarchy-mango-option", "xkb_rules_layout"]).await.unwrap_or_default();
        multiple.set(out.contains(','));
        paint();
      });
    }
  };
  {
    let (briefs, paint) = (briefs.clone(), paint.clone());
    glib::spawn_future_local(async move {
      let out = crate::watch::output(&["xkbcli", "list", "--load-exotic"]).await.unwrap_or_default();
      *briefs.borrow_mut() = layout_briefs(&out);
      paint();
    });
  }
  check_layouts();
  let weak = button.widget.downgrade();
  mango::get().connect(move || {
    if weak.upgrade().is_none() {
      return;
    }
    // A reload that adds a layout raises no event of its own, but switching
    // to it does.
    let full = mango::get().keyboard_layout();
    if *last.borrow() != full {
      *last.borrow_mut() = full;
      check_layouts();
    }
    paint();
  });
  Slot::new(ctx, &button.widget).widget.clone().upcast()
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
  None,
  Available,
  Running,
  Password,
  Staged,
  Failed,
}

pub fn phase(status: Option<&Value>, run: Option<&Value>, last: Option<&Value>) -> Phase {
  use update_model as m;
  let running = run.is_some_and(|r| m::flag(r, "running"));
  let waiting = running && run.and_then(|r| r.get("askpass")).is_some_and(|a| m::flag(a, "waiting"));
  let failed = last.is_some_and(|l| l.get("seen") == Some(&Value::Bool(false)) && m::text(l, "result") == "failed");
  if running {
    if waiting { Phase::Password } else { Phase::Running }
  } else if m::staged_pending(status) {
    Phase::Staged
  } else if failed {
    Phase::Failed
  } else if m::anything(status) {
    Phase::Available
  } else {
    Phase::None
  }
}

struct Update {
  status: Option<Value>,
  run: Option<Value>,
  last: Option<Value>,
}

fn home() -> String {
  std::env::var("HOME").unwrap_or_default()
}

fn runtime() -> String {
  std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into())
}

// omarchy.system-update (shell/plugins/bar/widgets/SystemUpdate.qml): the
// update icon, while there is something to update, an update runs or waits
// for the reboot, or the last one failed; its panel sums that up and opens
// Atualizar DoxIA. It also checks on a timer (omarchy-update-check every
// "checkHours"), updates Flatpak apps in the background with "autoFlatpak",
// and says so when an update finishes.
pub fn system_update(ctx: &Rc<Ctx>) -> gtk::Widget {
  use update_model as m;
  let button = Button::new(ctx, Kind::Status, "\u{f021}");
  button.widget.set_visible(false);
  let slot = Slot::new(ctx, &button.widget);
  let panel = Panel::new(ctx, &button.widget, &slot, ctx.style.space(300.0));
  let space = { let s = ctx.style.clone(); move |px: f64| s.space(px) };
  let palette = doxia_theme::palette();

  let body = ui::vbox(space(12.0));
  let hero = ui::Hero::new(&space);
  body.append(&hero.widget);
  let rail = ui::Rail::new(space(5.0), palette.accent);
  body.append(&rail.widget);
  let detail = ui::text("");
  detail.add_css_class("dim");
  body.append(&detail);
  let buttons = ui::hbox(space(8.0));
  buttons.set_halign(gtk::Align::End);
  body.append(&buttons);
  panel.content.append(&body);

  let state = Rc::new(RefCell::new(Update { status: None, run: None, last: None }));
  let current = Rc::new(Cell::new(Phase::None));
  let pulse_started = Rc::new(Cell::new(None::<Instant>));

  let open_window = {
    let p = Rc::downgrade(&panel);
    move || {
      if let Some(p) = p.upgrade() {
        p.close();
      }
      crate::run::argv(&["omarchy-launch-update"]);
    }
  };

  let render = {
    let (b, state, current, panel_w) = (button.clone(), state.clone(), current.clone(), Rc::downgrade(&panel));
    let (hero, rail, detail, buttons) = (Rc::new(hero), Rc::new(rail), detail.clone(), buttons.clone());
    let pulse_started = pulse_started.clone();
    move || {
      let st = state.borrow();
      let (status, run, last) = (st.status.as_ref(), st.run.as_ref(), st.last.as_ref());
      let ph = phase(status, run, last);
      current.set(ph);
      b.widget.set_visible(ph != Phase::None);
      let glyph = if ph == Phase::Failed { "\u{f071}" } else { "\u{f021}" };
      b.set_text(glyph);
      for c in ["staged", "failed", "running"] {
        b.widget.remove_css_class(c);
      }
      match ph {
        Phase::Staged => b.widget.add_css_class("staged"),
        Phase::Failed => b.widget.add_css_class("failed"),
        Phase::Running | Phase::Password => b.widget.add_css_class("running"),
        _ => {}
      }
      b.set_tooltip(match ph {
        Phase::Running => "Atualizando o DoxIA…",
        Phase::Password => "A atualização espera sua senha",
        Phase::Staged => "Atualização pronta: instala no próximo reinício",
        Phase::Failed => "A última atualização não terminou",
        _ => "Atualizações do DoxIA disponíveis",
      });
      // A soft pulse while an update runs.
      if matches!(ph, Phase::Running | Phase::Password) {
        if pulse_started.get().is_none() {
          pulse_started.set(Some(Instant::now()));
          let (w, ps) = (b.widget.downgrade(), pulse_started.clone());
          glib::timeout_add_local(Duration::from_millis(33), move || {
            let (Some(w), Some(start)) = (w.upgrade(), ps.get()) else {
              return glib::ControlFlow::Break;
            };
            let phase = (start.elapsed().as_secs_f64() / 1.6).fract();
            w.set_opacity(0.35 + 0.65 * ((phase * std::f64::consts::TAU).cos() * 0.5 + 0.5));
            glib::ControlFlow::Continue
          });
        }
      } else if pulse_started.take().is_some() {
        b.widget.set_opacity(1.0);
      }

      // The panel.
      let step = m::current_step(run);
      let title = match ph {
        Phase::Running => step.as_ref().map(|s| m::text(s, "label").to_string()).unwrap_or_else(|| "Atualizando…".into()),
        Phase::Password => "Aguardando senha".into(),
        Phase::Staged => "Instala no reinício".into(),
        Phase::Failed => "A atualização não terminou".into(),
        _ => m::plural(m::updates_count(status), "atualização", "atualizações"),
      };
      let subtitle = match ph {
        Phase::Running => format!("{}%", run.map(|r| m::num(r, "progress")).unwrap_or(0.0).round()),
        Phase::Staged => m::plural(m::staged_packages(status).len(), "pacote pronto", "pacotes prontos"),
        Phase::Available => format!("VERIFICADO {}", m::ago(status.map(|s| m::num(s, "checkedAt")).unwrap_or(0.0)).to_uppercase()),
        _ => String::new(),
      };
      hero.set(glyph, &title, &subtitle);
      for c in ["staged", "failed", "running"] {
        hero.icon.remove_css_class(c);
      }
      for c in b.widget.css_classes() {
        if ["staged", "failed", "running"].contains(&c.as_str()) {
          hero.icon.add_css_class(&c);
        }
      }
      rail.widget.set_visible(ph == Phase::Running);
      rail.set(run.map(|r| m::num(r, "progress")).unwrap_or(0.0) / 100.0);
      let text = match ph {
        Phase::Available => {
          let mut lines = Vec::new();
          if m::system_count(status) > 0 {
            lines.push(format!("Sistema: {}", m::system_names(status, 3)));
          }
          if m::flatpak_count(status) > 0 {
            lines.push(format!("Flatpak: {}", m::flatpak_names(status, 2)));
          }
          if m::doxia_behind(status) > 0 {
            lines.push(format!("DoxIA: {}", m::plural(m::doxia_behind(status), "novidade", "novidades")));
          }
          lines.join("\n")
        }
        Phase::Staged => "A atualização já foi baixada e será instalada quando você reiniciar ou desligar.".into(),
        Phase::Password => "Abra a janela para confirmar a senha.".into(),
        Phase::Failed => m::failure_text(last),
        Phase::Running => step.as_ref().map(|s| m::text(s, "detail").to_string()).unwrap_or_default(),
        Phase::None => String::new(),
      };
      detail.set_text(&text);
      detail.set_visible(!text.is_empty());
      if let Some(p) = panel_w.upgrade() {
        p.relayout();
      }
      drop(st);
      crate::popup::clear(&buttons);
      if ph == Phase::Available {
        let p = panel_w.clone();
        buttons.append(&ui::button("Depois", false, move || {
          if let Some(p) = p.upgrade() {
            p.close();
          }
        }));
      }
      if ph == Phase::Staged {
        let ow = open_window.clone();
        buttons.append(&ui::button("Abrir", false, move || ow()));
      }
      let primary = match ph {
        Phase::Available => "Atualizar",
        Phase::Staged => "Reiniciar",
        _ => "Abrir",
      };
      let (ow, p) = (open_window.clone(), panel_w.clone());
      buttons.append(&ui::button(primary, true, move || {
        if ph == Phase::Staged {
          if let Some(p) = p.upgrade() {
            p.close();
          }
          crate::run::argv(&["omarchy-system-reboot"]);
        } else {
          ow();
        }
      }));
    }
  };

  let status_path = format!("{}/.cache/omarchy/update-status.json", home());
  let run_path = format!("{}/omarchy-update/state.json", runtime());
  let last_path = format!("{}/.local/state/omarchy/update-last.json", home());
  let reload = {
    let (state, render) = (state.clone(), render.clone());
    let (sp, rp, lp) = (status_path.clone(), run_path.clone(), last_path.clone());
    move || {
      {
        let mut st = state.borrow_mut();
        st.status = m::read(&sp);
        st.run = m::read(&rp);
        st.last = m::read(&lp);
      }
      notify_result(state.borrow().last.as_ref(), &lp);
      render();
    }
  };
  reload();
  // The files change under us: watch them (the run every second while one
  // is going, as its file is rewritten in place).
  let mut monitors = Vec::new();
  for path in [&status_path, &run_path, &last_path] {
    let file = gtk::gio::File::for_path(path);
    if let Ok(mon) = file.monitor_file(gtk::gio::FileMonitorFlags::NONE, gtk::gio::Cancellable::NONE) {
      let r = reload.clone();
      mon.connect_changed(move |_, _, _, _| r());
      monitors.push(mon);
    }
  }
  let w = button.widget.downgrade();
  let (r, cur) = (reload.clone(), current.clone());
  glib::timeout_add_seconds_local(1, move || {
    let Some(_) = w.upgrade() else { return glib::ControlFlow::Break };
    if matches!(cur.get(), Phase::Running | Phase::Password) {
      r();
    }
    glib::ControlFlow::Continue
  });
  // Kept with the widget.
  button.widget.connect_destroy(move |_| {
    let _ = &monitors;
  });

  // The periodic check, and the background Flatpak update.
  let check = {
    let (state, r) = (state.clone(), reload.clone());
    move || {
      let (state, r) = (state.clone(), r.clone());
      glib::spawn_future_local(async move {
        let prefs = crate::watch::output(&["omarchy-update-pref", "json"]).await.and_then(|t| m::parse(&t)).unwrap_or_default();
        let _ = crate::watch::output(&["omarchy-update-check"]).await;
        r();
        let st = state.borrow();
        let running = st.run.as_ref().is_some_and(|r| m::flag(r, "running"));
        if prefs.get("autoFlatpak") == Some(&Value::Bool(true)) && !running && m::flatpak_count(st.status.as_ref()) > 0 {
          crate::run::argv(&["omarchy-update-run", "start", "flatpak"]);
        }
      });
    }
  };
  // One checker for the whole shell, not one per screen.
  if ctx.first_screen {
    check();
    let c = check.clone();
    let w = button.widget.downgrade();
    glib::spawn_future_local(async move {
      loop {
        let prefs = crate::watch::output(&["omarchy-update-pref", "json"]).await.and_then(|t| m::parse(&t)).unwrap_or_default();
        let hours = prefs.get("checkHours").and_then(Value::as_f64).unwrap_or(6.0).max(1.0);
        glib::timeout_future(Duration::from_secs_f64(hours * 3600.0)).await;
        if w.upgrade().is_none() {
          break;
        }
        c();
      }
    });
  }
  let p = panel.clone();
  button.on_press(move |_| p.toggle());
  slot.widget.clone().upcast()
}

// One toast per finished run: `notified` marks it (the window keeps `seen`).
fn notify_result(last: Option<&Value>, path: &str) {
  use update_model as m;
  let Some(last) = last else { return };
  if last.get("seen") != Some(&Value::Bool(false)) || m::flag(last, "notified") || m::num(last, "finishedAt") <= 0.0 {
    return;
  }
  let result = m::text(last, "result");
  if result != "ok" && result != "failed" {
    return;
  }
  if m::now() - m::num(last, "finishedAt") > 86400.0 {
    return;
  }
  let sum = last.get("summary").cloned().unwrap_or_default();
  let n = |k: &str| sum.get(k).and_then(Value::as_u64).unwrap_or(0) as usize;
  let mut body = Vec::new();
  if n("packages") > 0 {
    body.push(m::plural(n("packages"), "pacote", "pacotes"));
  }
  if n("migrations") > 0 {
    body.push(m::plural(n("migrations"), "migração", "migrações"));
  }
  if n("orphans") > 0 {
    body.push(m::plural(n("orphans"), "órfão removido", "órfãos removidos"));
  }
  let reboot = m::text(last, "reboot");
  if !reboot.is_empty() {
    body.push(format!("{reboot}: reinicie"));
  }
  if result == "ok" {
    let title = if m::text(last, "mode") == "flatpak" { "Apps Flatpak atualizados" } else { "DoxIA atualizado" };
    let text = if body.is_empty() { "Sem pendências".to_string() } else { body.join(" · ") };
    crate::run::argv(&["omarchy-notification-send", "-g", "\u{f021}", title, &text]);
  } else {
    crate::run::argv(&[
      "omarchy-notification-send", "-u", "critical", "-g", "\u{f071}", "A atualização não terminou", &m::failure_text(Some(last)), "--exec",
      "omarchy-launch-update",
    ]);
  }
  crate::run::argv(&["sh", "-c", "f=\"$1\"; jq '.notified = true' \"$f\" > \"$f.tmp\" && mv \"$f.tmp\" \"$f\"", "sh", path]);
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn layout_codes() {
    let briefs = layout_briefs("- layout: 'us'\n  variant: ''\n  brief: 'en'\n  description: English (US)\n- layout: 'mm'\n  brief: 'my-zwg'\n  description: Burmese (Zawgyi)\n");
    assert_eq!(short_label("English (US)", &briefs), "EN");
    assert_eq!(short_label("Burmese (Zawgyi)", &briefs), "MY");
    assert_eq!(short_label("Portuguese (Brazil)", &briefs), "POR");
    assert_eq!(short_label("", &briefs), "");
  }

  #[test]
  fn update_phases() {
    assert_eq!(phase(None, None, None), Phase::None);
    assert_eq!(phase(None, Some(&json!({"running": true, "askpass": {"waiting": true}})), None), Phase::Password);
    assert_eq!(phase(None, None, Some(&json!({"seen": false, "result": "failed"}))), Phase::Failed);
  }
}
