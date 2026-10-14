// Idle: the screensaver, then the lock, replacing shell/services/idle
// (Service.qml). shell.json's idle.screensaver and idle.lock say after how
// many idle seconds each comes (150 and 300). Idle for the earlier of the
// two starts a cycle: the screensaver (omarchy-launch-screensaver, unless
// the screen is locked) and the lock (omarchy-system-lock) each at their
// time. Activity ends the cycle (omarchy-system-wake) — but once the
// screensaver runs it keeps the cycle armed, since starting it reads as
// activity; its window closing before the lock is what ends it then. The
// stay-awake flag (~/.local/state/omarchy/indicators/stay-awake) turns it
// all off.
//
// IPC target "idle": status, debug, enable, disable, toggle.

use crate::idle_monitor::IdleMonitor;
use gtk::prelude::*;
use gtk::{gio, glib};
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

const SCREENSAVER_CLASS: &str = "org.omarchy.screensaver";

fn home() -> PathBuf {
  PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

fn stay_awake_path() -> PathBuf {
  home().join(".local/state/omarchy/indicators/stay-awake")
}

// IdleModel.secondsFromConfig.
pub fn seconds(value: &Value, fallback: u32) -> u32 {
  match value.as_f64() {
    Some(n) if n.is_finite() && n >= 0.0 => n.floor() as u32,
    _ => fallback,
  }
}

fn config() -> (u32, u32) {
  let text = std::fs::read_to_string(home().join(".config/omarchy/shell.json")).unwrap_or_default();
  let json: Value = serde_json::from_str(&text).unwrap_or_default();
  (seconds(&json["idle"]["screensaver"], 150), seconds(&json["idle"]["lock"], 300))
}

#[derive(Default)]
struct State {
  screensaver: u32,
  lock: u32,
  stay_awake: bool,
  idle: bool,
  in_cycle: bool,
  screensaver_started: bool,
  screensaver_windows: usize,
  grace: bool,
  last_event: String,
  last_event_at: String,
  timers: Vec<(String, glib::SourceId)>,
}

pub struct Idle {
  st: RefCell<State>,
  monitor: RefCell<Option<IdleMonitor>>,
  file_monitors: RefCell<Vec<gio::FileMonitor>>,
  window_poll: Cell<bool>,
}

fn now_iso() -> String {
  glib::DateTime::now_utc().ok().and_then(|d| d.format_iso8601().ok()).map(|s| s.to_string()).unwrap_or_default()
}

fn shell(command: &str) {
  crate::run::shell(command);
}

impl Idle {
  pub fn new() -> Rc<Idle> {
    let (screensaver, lock) = config();
    let me = Rc::new(Idle {
      st: RefCell::new(State { screensaver, lock, stay_awake: stay_awake_path().exists(), ..State::default() }),
      monitor: RefCell::default(),
      file_monitors: RefCell::default(),
      window_poll: Cell::new(false),
    });
    let weak = Rc::downgrade(&me);
    *me.monitor.borrow_mut() = IdleMonitor::new(move |idle| {
      if let Some(me) = weak.upgrade() {
        me.idle_changed(idle);
      }
    });
    me.apply_timeout();
    me.watch();
    me.log("service-ready", "");
    me
  }

  fn log(&self, event: &str, detail: &str) {
    let mut st = self.st.borrow_mut();
    st.last_event = if detail.is_empty() { event.to_string() } else { format!("{event}: {detail}") };
    st.last_event_at = now_iso();
  }

  fn enabled(&self) -> bool {
    !self.st.borrow().stay_awake
  }

  fn first_timeout(&self) -> u32 {
    let st = self.st.borrow();
    st.screensaver.min(st.lock)
  }

  fn apply_timeout(&self) {
    let t = self.enabled().then(|| self.first_timeout());
    if let Some(m) = self.monitor.borrow().as_ref() {
      m.set_timeout(t);
    }
  }

  // shell.json (the timeouts) and the stay-awake flag's directory.
  fn watch(self: &Rc<Self>) {
    let mut mons = Vec::new();
    if let Ok(m) = gio::File::for_path(home().join(".config/omarchy/shell.json")).monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
      let weak = Rc::downgrade(self);
      m.connect_changed(move |_, _, _, _| {
        let Some(me) = weak.upgrade() else { return };
        let (ss, lock) = config();
        let changed = {
          let st = me.st.borrow();
          (st.screensaver, st.lock) != (ss, lock)
        };
        if changed {
          {
            let mut st = me.st.borrow_mut();
            st.screensaver = ss;
            st.lock = lock;
          }
          me.apply_timeout();
        }
      });
      mons.push(m);
    }
    let dir = stay_awake_path().parent().map(PathBuf::from).unwrap_or_default();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(m) = gio::File::for_path(&dir).monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
      let weak = Rc::downgrade(self);
      m.connect_changed(move |_, _, _, _| {
        if let Some(me) = weak.upgrade() {
          me.apply_stay_awake(stay_awake_path().exists(), false, "state-file");
        }
      });
      mons.push(m);
    }
    *self.file_monitors.borrow_mut() = mons;
  }

  fn clear_timer(&self, name: &str) {
    let mut st = self.st.borrow_mut();
    if let Some(i) = st.timers.iter().position(|(n, _)| n == name) {
      let (_, id) = st.timers.remove(i);
      id.remove();
    }
  }

  fn timer_running(&self, name: &str) -> bool {
    self.st.borrow().timers.iter().any(|(n, _)| n == name)
  }

  fn start_timer(self: &Rc<Self>, name: &'static str, delay: Duration, f: impl Fn(&Rc<Idle>) + 'static) {
    self.clear_timer(name);
    let weak = Rc::downgrade(self);
    let id = glib::timeout_add_local_once(delay, move || {
      let Some(me) = weak.upgrade() else { return };
      me.st.borrow_mut().timers.retain(|(n, _)| n != name);
      f(&me);
    });
    self.st.borrow_mut().timers.push((name.to_string(), id));
  }

  fn launch_screensaver(self: &Rc<Self>) {
    self.st.borrow_mut().screensaver_started = true;
    self.start_timer("grace", Duration::from_secs(3), |me| me.grace_over());
    self.log("process-start", "screensaver");
    shell("[[ $(omarchy-shell lock isLocked 2>/dev/null) == \"true\" ]] || omarchy-launch-screensaver");
    self.poll_windows();
  }

  fn lock_system(&self, reason: &str) {
    self.log("lock-system", reason);
    for t in ["screensaver", "lock", "grace"] {
      self.clear_timer(t);
    }
    {
      let mut st = self.st.borrow_mut();
      st.in_cycle = false;
      st.screensaver_started = false;
      st.screensaver_windows = 0;
    }
    shell("omarchy-system-lock");
  }

  fn start_cycle(self: &Rc<Self>) {
    if self.st.borrow().in_cycle {
      self.log("idle-cycle-already-running", "");
      return;
    }
    let (ss, lock) = {
      let st = self.st.borrow();
      (st.screensaver, st.lock)
    };
    self.log("idle-cycle-start", &format!("screensaver={ss} lock={lock}"));
    {
      let mut st = self.st.borrow_mut();
      st.in_cycle = true;
      st.screensaver_started = false;
      st.screensaver_windows = 0;
    }
    let first = ss.min(lock);
    let (ss_delay, lock_delay) = (ss - first, lock - first);
    if ss_delay == 0 {
      self.launch_screensaver();
    } else {
      self.start_timer("screensaver", Duration::from_secs(ss_delay as u64), |me| me.launch_screensaver());
    }
    if lock_delay == 0 {
      self.lock_system("lock-timeout-immediate");
    } else {
      self.start_timer("lock", Duration::from_secs(lock_delay as u64), |me| {
        if me.enabled() && me.st.borrow().in_cycle {
          me.lock_system("lock-timeout");
        }
      });
    }
  }

  fn cancel_cycle(&self, reason: &str) {
    self.log("idle-cycle-cancel", reason);
    for t in ["screensaver", "lock", "grace"] {
      self.clear_timer(t);
    }
    let was = self.st.borrow().in_cycle;
    if was {
      shell("omarchy-system-wake");
    }
    let mut st = self.st.borrow_mut();
    st.in_cycle = false;
    st.screensaver_started = false;
    st.screensaver_windows = 0;
  }

  fn grace_over(&self) {
    let st = self.st.borrow();
    let cancel = self.enabled() && st.in_cycle && st.screensaver_started && st.screensaver_windows == 0 && !st.idle;
    drop(st);
    if cancel {
      self.cancel_cycle("screensaver-not-running");
    }
  }

  fn idle_changed(self: &Rc<Self>, idle: bool) {
    self.st.borrow_mut().idle = idle;
    self.log("idle-monitor", if idle { "idle" } else { "active" });
    if !self.enabled() {
      return;
    }
    if idle {
      self.start_cycle();
      return;
    }
    let (in_cycle, started, windows) = {
      let st = self.st.borrow();
      (st.in_cycle, st.screensaver_started, st.screensaver_windows)
    };
    if !in_cycle {
      return;
    }
    // Starting the screensaver can read as activity: once it is up (or
    // still in its grace), the cycle stays armed.
    if started && (windows > 0 || self.timer_running("grace")) {
      self.log("idle-monitor-active", "screensaver cycle remains armed");
      return;
    }
    self.cancel_cycle("activity");
  }

  // The screensaver's windows, while a cycle has started it: its window
  // closing before the lock ends the cycle.
  fn poll_windows(self: &Rc<Self>) {
    if self.window_poll.replace(true) {
      return;
    }
    let weak = Rc::downgrade(self);
    glib::spawn_future_local(async move {
      loop {
        glib::timeout_future(Duration::from_millis(700)).await;
        let Some(me) = weak.upgrade() else { return };
        let (in_cycle, started) = {
          let st = me.st.borrow();
          (st.in_cycle, st.screensaver_started)
        };
        if !in_cycle || !started {
          me.window_poll.set(false);
          return;
        }
        let out = crate::watch::output(&["mmsg", "get", "all-clients"]).await.unwrap_or_default();
        let json: Value = serde_json::from_str(&out).unwrap_or_default();
        let count = json["clients"].as_array().map(|c| c.iter().filter(|c| c["appid"].as_str() == Some(SCREENSAVER_CLASS)).count()).unwrap_or(0);
        let before = me.st.borrow().screensaver_windows;
        me.st.borrow_mut().screensaver_windows = count;
        if count > 0 {
          me.clear_timer("grace");
        }
        if before > 0 && count == 0 && me.enabled() {
          // Dismissed before the lock: that was activity.
          me.cancel_cycle("screensaver-dismissed");
          me.window_poll.set(false);
          return;
        }
      }
    });
  }

  fn apply_stay_awake(self: &Rc<Self>, on: bool, persist: bool, reason: &str) -> &'static str {
    if persist {
      let path = stay_awake_path();
      if on {
        let _ = std::fs::create_dir_all(path.parent().unwrap());
        let _ = std::fs::write(&path, "");
      } else {
        let _ = std::fs::remove_file(&path);
      }
    }
    let changed = self.st.borrow().stay_awake != on;
    self.st.borrow_mut().stay_awake = on;
    if changed {
      self.log("stay-awake", &format!("{} {reason}", if on { "enabled" } else { "disabled" }));
      if on {
        self.cancel_cycle("stay-awake");
      }
      self.apply_timeout();
    }
    if on { "disabled" } else { "enabled" }
  }

  pub fn stay_awake(&self) -> bool {
    self.st.borrow().stay_awake
  }

  fn status(&self) -> String {
    let st = self.st.borrow();
    let first = st.screensaver.min(st.lock);
    serde_json::json!({
      "enabled": !st.stay_awake,
      "stayAwake": st.stay_awake,
      "stayAwakeStateLoaded": true,
      "stayAwakeStatePath": stay_awake_path().to_string_lossy(),
      "idle": st.idle,
      "inIdleCycle": st.in_cycle,
      "screensaverStarted": st.screensaver_started,
      "screensaver": st.screensaver,
      "lock": st.lock,
      "screensaverDelay": st.screensaver - first,
      "lockDelay": st.lock - first,
      "screensaverWindows": st.screensaver_windows,
      "timers": {
        "screensaver": st.timers.iter().any(|(n, _)| n == "screensaver"),
        "lock": st.timers.iter().any(|(n, _)| n == "lock"),
        "screensaverLaunchGrace": st.timers.iter().any(|(n, _)| n == "grace"),
      },
      "monitor": self.monitor.borrow().is_some(),
      "lastEvent": st.last_event,
      "lastEventAt": st.last_event_at,
    })
    .to_string()
  }

  pub fn ipc(self: &Rc<Self>, method: &str) -> Option<String> {
    Some(match method {
      "status" | "debug" => self.status(),
      "enable" => self.apply_stay_awake(false, true, "ipc").into(),
      "disable" => self.apply_stay_awake(true, true, "ipc").into(),
      "toggle" => {
        let on = self.enabled();
        self.apply_stay_awake(on, true, "ipc").into()
      }
      _ => return None,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn timeouts_from_config() {
    assert_eq!(seconds(&json!(120), 150), 120);
    assert_eq!(seconds(&json!(-1), 150), 150);
    assert_eq!(seconds(&json!("x"), 300), 300);
    assert_eq!(seconds(&json!(90.7), 300), 90);
  }
}
