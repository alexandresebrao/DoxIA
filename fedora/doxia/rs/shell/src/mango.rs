// What the shell needs from mango, read off its IPC (mmsg), as
// shell/Commons/Compositor.qml does: the tags of each output, the output in
// focus, the keyboard layout and where the tiled windows sit (the background's
// parallax). Each `mmsg watch` prints the state, then a
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
  // mango's symbol for its layout: "S" is the scroller.
  pub layout: String,
}

// A window's place: what the background needs to follow a scroller strip.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Client {
  pub id: u64,
  pub monitor: String,
  pub tags: Vec<u32>,
  pub x: i64,
  pub width: i64,
  // Floating or minimized: not part of the strip.
  pub loose: bool,
}

pub fn parse_clients(json: &Value) -> Option<Vec<Client>> {
  let all = json.get("clients")?.as_array()?;
  let flag = |c: &Value, k: &str| c.get(k).and_then(Value::as_bool).unwrap_or(false);
  Some(
    all
      .iter()
      .map(|c| Client {
        id: c.get("id").and_then(Value::as_u64).unwrap_or(0),
        monitor: c.get("monitor").and_then(Value::as_str).unwrap_or("").to_string(),
        tags: c.get("tags").and_then(Value::as_array).map(|t| t.iter().filter_map(Value::as_u64).map(|t| t as u32).collect()).unwrap_or_default(),
        x: c.get("x").and_then(Value::as_i64).unwrap_or(0),
        width: c.get("width").and_then(Value::as_i64).unwrap_or(0),
        loose: flag(c, "is_floating") || flag(c, "is_minimized") || flag(c, "is_scratchpad") || flag(c, "is_namedscratchpad"),
      })
      .collect(),
  )
}

// How far the tiled windows of `tag` on `monitor` are scrolled past the
// screen's left edge `left`: 0 when the strip starts on screen (every layout
// but a scroller that overflows).
pub fn scrolled(clients: &[Client], monitor: &str, tag: u32, left: i64) -> i64 {
  clients
    .iter()
    .filter(|c| c.monitor == monitor && !c.loose && c.tags.contains(&tag))
    .map(|c| left - c.x)
    .max()
    .unwrap_or(0)
    .max(0)
}

#[derive(Default)]
struct State {
  clients: Vec<Client>,
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
            layout: t.get("layout").and_then(Value::as_str).unwrap_or("").to_string(),
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
    crate::watch::lines(&["mmsg", "watch", "all-clients"], move |line| {
      let (Some(me), Ok(json)) = (weak.upgrade(), serde_json::from_str::<Value>(line)) else { return };
      if let Some(clients) = parse_clients(&json).filter(|c| *c != me.st.borrow().clients) {
        me.st.borrow_mut().clients = clients;
        me.changed();
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

  // How far the active tag's strip on `monitor` is scrolled (see scrolled()).
  pub fn scrolled(&self, monitor: &str, left: i64) -> i64 {
    let st = self.st.borrow();
    let Some(tag) = st.tags.get(monitor).and_then(|t| t.iter().find(|t| t.active)).map(|t| t.index) else { return 0 };
    scrolled(&st.clients, monitor, tag, left)
  }

  // Whether the active tag on `monitor` lays its windows out as a scroller.
  pub fn scroller(&self, monitor: &str) -> bool {
    let st = self.st.borrow();
    st.tags.get(monitor).and_then(|t| t.iter().find(|t| t.active)).is_some_and(|t| t.layout == "S")
  }

  // Whether the active tag's tiled windows on `monitor` run past the screen
  // (`left`, `width` wide): a scroller with somewhere to go.
  pub fn overflows(&self, monitor: &str, left: i64, width: i64) -> bool {
    let st = self.st.borrow();
    let Some(tag) = st.tags.get(monitor).and_then(|t| t.iter().find(|t| t.active)).map(|t| t.index) else { return false };
    st.clients.iter().filter(|c| c.monitor == monitor && !c.loose && c.tags.contains(&tag)).any(|c| c.x < left || c.x + c.width > left + width)
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
      r#"{"all_tags":[{"monitor":"Virtual-1","tags":[{"index":1,"is_active":true,"is_urgent":false,"layout":"S","client_count":3},{"index":2,"is_active":false,"is_urgent":true,"client_count":0}]}]}"#,
    )
    .unwrap();
    let tags = parse_tags(&json).unwrap();
    assert_eq!(tags["Virtual-1"][0], Tag { index: 1, active: true, urgent: false, clients: 3, layout: "S".into() });
    assert!(tags["Virtual-1"][1].urgent);
    assert!(parse_tags(&serde_json::json!({"layout": "x"})).is_none());
  }

  #[test]
  fn measures_how_far_the_strip_scrolled() {
    let json: Value = serde_json::from_str(
      r#"{"clients":[{"id":1,"monitor":"eDP-1","tags":[1],"x":-253},{"id":2,"monitor":"eDP-1","tags":[1],"x":399},{"id":3,"monitor":"eDP-1","tags":[1],"x":-900,"is_floating":true},{"id":4,"monitor":"eDP-1","tags":[2],"x":-2000}]}"#,
    )
    .unwrap();
    let clients = parse_clients(&json).unwrap();
    assert_eq!(scrolled(&clients, "eDP-1", 1, 0), 253);
    assert_eq!(scrolled(&clients, "eDP-1", 1, -300), 0);
    assert_eq!(scrolled(&clients, "eDP-1", 3, 0), 0);
    assert_eq!(scrolled(&clients, "HDMI-A-1", 1, 0), 0);
  }
}
