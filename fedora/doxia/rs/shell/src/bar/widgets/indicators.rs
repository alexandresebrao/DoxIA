// omarchy.indicators (shell/plugins/bar/widgets/Indicators.qml and
// indicators/): small status glyphs. The active ones always show; the
// inactive ones show dimmed while the pointer is over the widget (or always
// with "alwaysShow"). "items" picks and orders them; by default: dictation,
// screen recording, reminders, night light, do-not-disturb, stay awake.
// IPC target "omarchy.indicators": refresh (screen recording, reminders).

use super::super::config::setting;
use super::super::widget::{Button, Kind, Slot};
use super::super::Ctx;
use gtk::prelude::*;
use gtk::{gio, glib};
use serde_json::Value;
use std::cell::Cell;
use std::rc::Rc;

thread_local! {
  // Every screen's refreshers, for the one IPC target.
  static REFRESHERS: std::cell::RefCell<Vec<std::rc::Weak<dyn Fn()>>> = const { std::cell::RefCell::new(Vec::new()) };
}

const DEFAULT: [&str; 6] = ["Dictation", "ScreenRecording", "Reminder", "NightLight", "Dnd", "StayAwake"];

struct Indicator {
  button: Rc<Button>,
  active: Cell<bool>,
}

fn home() -> String {
  std::env::var("HOME").unwrap_or_default()
}

pub fn build(ctx: &Rc<Ctx>, entry: &Value) -> gtk::Widget {
  let ids: Vec<String> = setting(entry, "items")
    .or_else(|| setting(entry, "indicators"))
    .and_then(Value::as_array)
    .filter(|a| !a.is_empty())
    .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from).or_else(|| v["id"].as_str().map(String::from))).collect())
    .unwrap_or_else(|| DEFAULT.iter().map(|s| s.to_string()).collect());
  let always = setting(entry, "alwaysShow").and_then(Value::as_bool) == Some(true);
  let row = gtk::Box::new(if ctx.vertical() { gtk::Orientation::Vertical } else { gtk::Orientation::Horizontal }, 0);
  row.add_css_class("bar-indicators");
  let hovered = Rc::new(Cell::new(false));
  let indicators: Rc<std::cell::RefCell<Vec<Rc<Indicator>>>> = Rc::default();
  let paint = {
    let (ind, hovered) = (indicators.clone(), hovered.clone());
    Rc::new(move || {
      for i in ind.borrow().iter() {
        let a = i.active.get();
        i.button.widget.set_visible(a || always || hovered.get());
        i.button.set_dimmed(!a);
      }
    })
  };
  let refreshers: Rc<std::cell::RefCell<Vec<Rc<dyn Fn()>>>> = Rc::default();
  for id in ids {
    let b = Button::new(ctx, Kind::Status, "");
    let ind = Rc::new(Indicator { button: b.clone(), active: Cell::new(false) });
    let set = {
      let (ind, paint) = (ind.clone(), paint.clone());
      move |active: bool, text: &str, tip: &str| {
        ind.active.set(active);
        ind.button.set_text(text);
        ind.button.set_tooltip(tip);
        paint();
      }
    };
    match id.as_str() {
      "Dictation" => {
        set(false, "\u{f036c}", "Ditar");
        let s = set.clone();
        let w = b.widget.downgrade();
        crate::watch::lines(&["bash", "-c", "omarchy-voxtype-status"], move |line| {
          if w.upgrade().is_none() {
            return;
          }
          let data: Value = serde_json::from_str(line.trim()).unwrap_or_default();
          let state = data["alt"].as_str().or_else(|| data["class"].as_str()).unwrap_or("idle").to_string();
          match state.as_str() {
            "recording" => s(true, "\u{f036c}", "Gravando ditado"),
            "transcribing" => s(false, "\u{f051f}", "Transcrevendo…"),
            _ => s(false, "\u{f036c}", "Ditar"),
          }
        });
        b.on_press(|_| crate::run::shell("omarchy-voxtype-config"));
      }
      "ScreenRecording" => {
        let s = set.clone();
        let refresh: Rc<dyn Fn()> = Rc::new(move || {
          let s = s.clone();
          glib::spawn_future_local(async move {
            let rec = crate::watch::output(&["pgrep", "--quiet", "-f", "^gpu-screen-recorder"]).await.is_some();
            s(rec, "\u{f0ec2}", if rec { "Parar gravação" } else { "Gravar tela" });
          });
        });
        refresh();
        refreshers.borrow_mut().push(refresh);
        let a = ind.clone();
        b.on_press(move |_| {
          crate::run::shell(if a.active.get() { "omarchy-capture-screenrecording --stop-recording" } else { "omarchy-menu toggle trigger.capture.screenrecord" });
        });
      }
      "Reminder" => {
        let s = set.clone();
        let refresh: Rc<dyn Fn()> = Rc::new(move || {
          let s = s.clone();
          glib::spawn_future_local(async move {
            let out = crate::watch::output(&["omarchy-reminder", "show", "--json"]).await.unwrap_or_default();
            let data: Value = serde_json::from_str(out.trim()).unwrap_or_default();
            let count = data["count"].as_u64().unwrap_or(0);
            s(count > 0, "\u{f088c}", data["tooltip"].as_str().unwrap_or(""));
          });
        });
        refresh();
        refreshers.borrow_mut().push(refresh);
        let a = ind.clone();
        b.on_press(move |_| {
          if a.active.get() {
            crate::run::argv(&["omarchy-reminder", "show"]);
          } else {
            crate::run::argv(&["omarchy-reminder", "-i"]);
          }
        });
      }
      "NightLight" => {
        let night = crate::nightlight::get();
        let s = set.clone();
        let n = night.clone();
        let show = move || {
          let on = n.enabled();
          s(on, "\u{f050e}", if on { "Desativar luz noturna" } else { "Luz noturna" });
        };
        show();
        let w = b.widget.downgrade();
        night.connect(move || {
          if w.upgrade().is_some() {
            show();
          }
        });
        b.on_press(move |_| {
          let on = !night.enabled();
          night.set(on);
        });
      }
      "Dnd" => {
        // Always shown; dimmer while notifications are silenced.
        let path = format!("{}/.local/state/omarchy/notifications.json", home());
        let s = set.clone();
        let b2 = b.clone();
        let read = Rc::new(move || {
          let text = std::fs::read_to_string(&path).unwrap_or_default();
          let dnd = serde_json::from_str::<Value>(&text).ok().and_then(|v| v["dnd"].as_bool()) == Some(true);
          s(true, if dnd { "\u{f009b}" } else { "\u{f009a}" }, if dnd { "Permitir notificações" } else { "Silenciar notificações" });
          if dnd { b2.widget.add_css_class("silenced") } else { b2.widget.remove_css_class("silenced") }
        });
        read();
        if let Ok(m) = gio::File::for_path(format!("{}/.local/state/omarchy/notifications.json", home())).monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
          let r = read.clone();
          m.connect_changed(move |_, _, _, _| r());
          b.widget.connect_destroy(move |_| {
            let _ = &m;
          });
        }
        b.on_press(|_| crate::run::argv(&["omarchy-toggle-notification-silencing"]));
      }
      "StayAwake" => {
        // The flag file both idle services keep; the toggle goes to
        // whichever runs.
        let flag = format!("{}/.local/state/omarchy/indicators/stay-awake", home());
        let s = set.clone();
        let read = Rc::new(move || {
          let on = std::path::Path::new(&flag).exists();
          s(on, "\u{f0176}", if on { "Permitir bloqueio e proteção de tela" } else { "Manter acordado" });
        });
        read();
        let dir = format!("{}/.local/state/omarchy/indicators", home());
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(m) = gio::File::for_path(&dir).monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
          let r = read.clone();
          m.connect_changed(move |_, _, _, _| r());
          b.widget.connect_destroy(move |_| {
            let _ = &m;
          });
        }
        b.on_press(|_| crate::run::argv(&["omarchy-shell", "idle", "toggle"]));
      }
      _ => continue,
    }
    row.append(&b.widget);
    indicators.borrow_mut().push(ind);
  }
  paint();
  let motion = gtk::EventControllerMotion::new();
  let (h1, p1, h2, p2) = (hovered.clone(), paint.clone(), hovered.clone(), paint.clone());
  motion.connect_enter(move |_, _, _| {
    h1.set(true);
    p1();
  });
  motion.connect_leave(move |_| {
    h2.set(false);
    p2();
  });
  row.add_controller(motion);
  // Room to hover even while nothing shows.
  row.set_size_request(ctx.style.status_slot / 2, -1);
  REFRESHERS.with(|r| {
    let mut r = r.borrow_mut();
    r.retain(|w| w.upgrade().is_some());
    r.extend(refreshers.borrow().iter().map(Rc::downgrade));
  });
  if ctx.first_screen {
    if let Some(bar) = ctx.bar.upgrade() {
      bar.add_ipc("omarchy.indicators", Box::new(move |method, _| {
        if method != "refresh" {
          return None;
        }
        let all: Vec<Rc<dyn Fn()>> = REFRESHERS.with(|r| r.borrow().iter().filter_map(|w| w.upgrade()).collect());
        for f in all {
          f();
        }
        Some("ok".into())
      }));
    }
  }
  let keep = (indicators.clone(), refreshers.clone());
  row.connect_destroy(move |_| {
    let _ = &keep;
  });
  Slot::new(ctx, &row).widget.clone().upcast()
}
