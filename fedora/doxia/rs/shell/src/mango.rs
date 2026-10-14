// What the shell needs from mango, read off its IPC (mmsg), as
// shell/Commons/Compositor.qml does: the tags of each output, the output in
// focus and the keyboard layout. Each `mmsg watch` prints the state, then a
// line per change. One instance per process (mango::get()).

use gtk::glib;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tag {
  pub index: u32,
  pub active: bool,
  pub urgent: bool,
  pub clients: u32,
}

#[derive(Default)]
struct State {
  tags: HashMap<String, Vec<Tag>>,
  focused_monitor: String,
  keyboard_layout: String,
  // The focused window: its id and title.
  focused_client: Option<(u64, String)>,
}

#[derive(Default)]
pub struct Mango {
  st: RefCell<State>,
  listeners: RefCell<Vec<Box<dyn Fn()>>>,
}

thread_local! {
  static MANGO: RefCell<Option<Rc<Mango>>> = const { RefCell::new(None) };
}

pub fn get() -> Rc<Mango> {
  MANGO.with(|m| {
    if let Some(m) = m.borrow().as_ref() {
      return m.clone();
    }
    let mango = Rc::new(Mango::default());
    *m.borrow_mut() = Some(mango.clone());
    mango.start();
    mango
  })
}

pub fn parse_tags(json: &Value) -> Option<HashMap<String, Vec<Tag>>> {
  let all = json.get("all_tags")?.as_array()?;
  let mut out = HashMap::new();
  for entry in all {
    let monitor = entry.get("monitor").and_then(Value::as_str).unwrap_or("").to_string();
    let tags = entry
      .get("tags")
      .and_then(Value::as_array)
      .map(|t| {
        t.iter()
          .map(|t| Tag {
            index: t.get("index").and_then(Value::as_u64).unwrap_or(0) as u32,
            active: t.get("is_active").and_then(Value::as_bool).unwrap_or(false),
            urgent: t.get("is_urgent").and_then(Value::as_bool).unwrap_or(false),
            clients: t.get("client_count").and_then(Value::as_u64).unwrap_or(0) as u32,
          })
          .collect()
      })
      .unwrap_or_default();
    out.insert(monitor, tags);
  }
  Some(out)
}

impl Mango {
  fn start(self: &Rc<Self>) {
    let weak = Rc::downgrade(self);
    crate::watch::lines(&["mmsg", "watch", "all-tags"], move |line| {
      let (Some(me), Ok(json)) = (weak.upgrade(), serde_json::from_str::<Value>(line)) else { return };
      if let Some(tags) = parse_tags(&json) {
        me.st.borrow_mut().tags = tags;
        me.changed();
      }
    });
    let weak = Rc::downgrade(self);
    crate::watch::lines(&["mmsg", "watch", "focusing-client"], move |line| {
      let (Some(me), Ok(json)) = (weak.upgrade(), serde_json::from_str::<Value>(line)) else { return };
      let client = json.get("id").and_then(Value::as_u64).map(|id| (id, json.get("title").and_then(Value::as_str).unwrap_or("").to_string()));
      if me.st.borrow().focused_client != client {
        me.st.borrow_mut().focused_client = client;
        me.changed();
      }
      match json.get("monitor").and_then(Value::as_str).filter(|m| !m.is_empty()) {
        Some(monitor) => me.set_focused(monitor),
        // No focused window: the output under the pointer.
        None => {
          let weak = Rc::downgrade(&me);
          glib::spawn_future_local(async move {
            let out = crate::watch::output(&["mmsg", "get", "cursorpos"]).await.unwrap_or_default();
            let json: Value = serde_json::from_str(&out).unwrap_or_default();
            if let (Some(me), Some(m)) = (weak.upgrade(), json.get("monitor").and_then(Value::as_str)) {
              me.set_focused(m);
            }
          });
        }
      }
    });
    let weak = Rc::downgrade(self);
    crate::watch::lines(&["mmsg", "watch", "keyboardlayout"], move |line| {
      let (Some(me), Ok(json)) = (weak.upgrade(), serde_json::from_str::<Value>(line)) else { return };
      if let Some(layout) = json.get("layout").and_then(Value::as_str) {
        me.st.borrow_mut().keyboard_layout = layout.into();
        me.changed();
      }
    });
  }

  fn set_focused(&self, monitor: &str) {
    if self.st.borrow().focused_monitor == monitor {
      return;
    }
    self.st.borrow_mut().focused_monitor = monitor.into();
    self.changed();
  }

  fn changed(&self) {
    for l in self.listeners.borrow().iter() {
      l();
    }
  }

  // Called after every change.
  pub fn connect(&self, f: impl Fn() + 'static) {
    self.listeners.borrow_mut().push(Box::new(f));
  }

  // A monitor's tags (the focused one's for an empty name).
  pub fn tags(&self, monitor: &str) -> Vec<Tag> {
    let st = self.st.borrow();
    let name = if monitor.is_empty() { st.focused_monitor.as_str() } else { monitor };
    st.tags.get(name).cloned().unwrap_or_default()
  }

  pub fn focused_client(&self) -> Option<(u64, String)> {
    self.st.borrow().focused_client.clone()
  }

  pub fn focused_monitor(&self) -> String {
    self.st.borrow().focused_monitor.clone()
  }

  pub fn keyboard_layout(&self) -> String {
    self.st.borrow().keyboard_layout.clone()
  }
}

// A mango function: dispatch(&["view", "3"]).
pub fn dispatch(call: &[&str]) {
  let _ = std::process::Command::new("mmsg")
    .args(["dispatch", &call.join(",")])
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .spawn()
    .map(|mut c| std::thread::spawn(move || c.wait()));
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn reads_tags_per_monitor() {
    let json: Value = serde_json::from_str(
      r#"{"all_tags":[{"monitor":"Virtual-1","tags":[{"index":1,"is_active":true,"is_urgent":false,"client_count":3},{"index":2,"is_active":false,"is_urgent":true,"client_count":0}]}]}"#,
    )
    .unwrap();
    let tags = parse_tags(&json).unwrap();
    assert_eq!(tags["Virtual-1"][0], Tag { index: 1, active: true, urgent: false, clients: 3 });
    assert!(tags["Virtual-1"][1].urgent);
    assert!(parse_tags(&serde_json::json!({"layout": "x"})).is_none());
  }
}
