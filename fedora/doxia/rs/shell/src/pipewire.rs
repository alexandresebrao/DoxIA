// PipeWire's audio nodes (Quickshell.Services.Pipewire), read with pw-dump
// whenever `pactl subscribe` reports a change, and changed with wpctl. The
// volume is the cubic one wpctl, pactl and Quickshell show (40% is a
// channel volume of 0.064). pactl's own JSON mangles non-ASCII names, which
// PipeWire hands out translated, so it only serves as the change signal.
// One instance per process (pipewire::get()).

use gtk::glib;
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Node {
  pub id: u32,
  pub name: String,
  pub description: String,
  pub nick: String,
  pub media_class: String,
  pub props: HashMap<String, String>,
  // Cubic, 0..1.5.
  pub volume: f64,
  pub muted: bool,
  pub has_volume: bool,
}

impl Node {
  pub fn prop(&self, key: &str) -> &str {
    self.props.get(key).map(String::as_str).unwrap_or("")
  }

  pub fn is_sink(&self) -> bool {
    self.media_class == "Audio/Sink"
  }

  pub fn is_source(&self) -> bool {
    self.media_class == "Audio/Source" || self.media_class == "Audio/Source/Virtual"
  }

  // An application's playback stream.
  pub fn is_playback(&self) -> bool {
    self.media_class == "Stream/Output/Audio"
  }

  pub fn is_capture(&self) -> bool {
    self.media_class == "Stream/Input/Audio"
  }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
  pub nodes: Vec<Node>,
  pub default_sink: String,
  pub default_source: String,
}

impl Snapshot {
  pub fn sink(&self) -> Option<&Node> {
    self.nodes.iter().find(|n| n.is_sink() && n.name == self.default_sink)
  }

  pub fn source(&self) -> Option<&Node> {
    self.nodes.iter().find(|n| n.is_source() && n.name == self.default_source)
  }

  pub fn sinks(&self) -> Vec<&Node> {
    self.nodes.iter().filter(|n| n.is_sink()).collect()
  }

  pub fn sources(&self) -> Vec<&Node> {
    self.nodes.iter().filter(|n| n.is_source()).collect()
  }

  pub fn playback(&self) -> Vec<&Node> {
    self.nodes.iter().filter(|n| n.is_playback()).collect()
  }
}

pub fn parse(dump: &str) -> Snapshot {
  let Ok(Value::Array(objects)) = serde_json::from_str::<Value>(dump) else { return Snapshot::default() };
  let mut snap = Snapshot::default();
  for o in &objects {
    match o["type"].as_str() {
      Some("PipeWire:Interface:Node") => {
        let props = &o["info"]["props"];
        let Some(class) = props["media.class"].as_str() else { continue };
        if !class.contains("Audio") {
          continue;
        }
        let s = |k: &str| props[k].as_str().unwrap_or("").to_string();
        let mut map = HashMap::new();
        if let Some(obj) = props.as_object() {
          for (k, v) in obj {
            if let Some(v) = v.as_str() {
              map.insert(k.clone(), v.to_string());
            }
          }
        }
        let p = &o["info"]["params"]["Props"][0];
        let volumes: Vec<f64> = p["channelVolumes"].as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
        let has_volume = !volumes.is_empty();
        let linear = if has_volume { volumes.iter().sum::<f64>() / volumes.len() as f64 } else { 0.0 };
        snap.nodes.push(Node {
          id: o["id"].as_u64().unwrap_or(0) as u32,
          name: s("node.name"),
          description: s("node.description"),
          nick: s("node.nick"),
          media_class: class.into(),
          props: map,
          volume: linear.max(0.0).cbrt(),
          muted: p["mute"].as_bool().unwrap_or(false),
          has_volume,
        });
      }
      Some("PipeWire:Interface:Metadata") if o["props"]["metadata.name"].as_str() == Some("default") => {
        for m in o["metadata"].as_array().into_iter().flatten() {
          let name = m["value"]["name"].as_str().unwrap_or("").to_string();
          match m["key"].as_str() {
            Some("default.audio.sink") => snap.default_sink = name,
            Some("default.audio.source") => snap.default_source = name,
            _ => {}
          }
        }
      }
      _ => {}
    }
  }
  snap
}

#[derive(Default)]
pub struct Pipewire {
  snap: RefCell<Rc<Snapshot>>,
  pending: Cell<bool>,
  listeners: RefCell<Vec<Box<dyn Fn()>>>,
}

thread_local! {
  static PIPEWIRE: RefCell<Option<Rc<Pipewire>>> = const { RefCell::new(None) };
}

pub fn get() -> Rc<Pipewire> {
  PIPEWIRE.with(|p| {
    if let Some(p) = p.borrow().as_ref() {
      return p.clone();
    }
    let me = Rc::new(Pipewire::default());
    *p.borrow_mut() = Some(me.clone());
    me.start();
    me
  })
}

impl Pipewire {
  fn start(self: &Rc<Self>) {
    let weak = Rc::downgrade(self);
    crate::watch::lines(&["pactl", "subscribe"], move |line| {
      if !line.contains("server") && !line.contains("sink") && !line.contains("source") {
        return;
      }
      if let Some(me) = weak.upgrade() {
        me.refresh();
      }
    });
    self.refresh();
  }

  // Coalesced: a burst of events reads once.
  pub fn refresh(self: &Rc<Self>) {
    if self.pending.replace(true) {
      return;
    }
    let weak = Rc::downgrade(self);
    glib::spawn_future_local(async move {
      glib::timeout_future(Duration::from_millis(60)).await;
      let dump = crate::watch::output(&["pw-dump"]).await.unwrap_or_default();
      let Some(me) = weak.upgrade() else { return };
      me.pending.set(false);
      let snap = parse(&dump);
      if *me.snap.borrow().as_ref() != snap {
        *me.snap.borrow_mut() = Rc::new(snap);
        for l in me.listeners.borrow().iter() {
          l();
        }
      }
    });
  }

  pub fn snapshot(&self) -> Rc<Snapshot> {
    self.snap.borrow().clone()
  }

  pub fn connect(&self, f: impl Fn() + 'static) {
    self.listeners.borrow_mut().push(Box::new(f));
  }
}

fn wpctl(args: &[&str]) {
  let _ = std::process::Command::new("wpctl")
    .args(args)
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .spawn()
    .map(|mut c| std::thread::spawn(move || c.wait()));
}

// Cubic, as shown.
pub fn set_volume(id: u32, volume: f64) {
  wpctl(&["set-volume", &id.to_string(), &format!("{:.3}", volume.clamp(0.0, 1.5))]);
}

pub fn set_mute(id: u32, muted: bool) {
  wpctl(&["set-mute", &id.to_string(), if muted { "1" } else { "0" }]);
}

pub fn set_default(id: u32) {
  wpctl(&["set-default", &id.to_string()]);
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn reads_nodes_and_defaults() {
    let dump = r#"[
      {"id": 53, "type": "PipeWire:Interface:Node", "info": {"props": {"media.class": "Audio/Sink", "node.name": "out", "node.description": "Saída analógica", "node.nick": "DAC"},
        "params": {"Props": [{"channelVolumes": [0.064, 0.064], "mute": false}]}}},
      {"id": 70, "type": "PipeWire:Interface:Node", "info": {"props": {"media.class": "Stream/Output/Audio", "application.name": "Spotify"}, "params": {"Props": [{"channelVolumes": [1.0], "mute": true}]}}},
      {"id": 30, "type": "PipeWire:Interface:Node", "info": {"props": {"node.name": "Dummy-Driver"}}},
      {"id": 0, "type": "PipeWire:Interface:Metadata", "props": {"metadata.name": "default"},
        "metadata": [{"key": "default.audio.sink", "value": {"name": "out"}}]}
    ]"#;
    let s = parse(dump);
    assert_eq!(s.nodes.len(), 2);
    let sink = s.sink().unwrap();
    assert!((sink.volume - 0.4).abs() < 0.001);
    assert_eq!(sink.description, "Saída analógica");
    assert_eq!(s.playback()[0].prop("application.name"), "Spotify");
    assert!(s.playback()[0].muted);
  }
}
