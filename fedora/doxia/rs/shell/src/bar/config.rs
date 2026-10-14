// The `bar` part of ~/.config/omarchy/shell.json (BarModel.js): the edge,
// fill, floating and locked flags, the center anchor and the widgets of each
// section, entries being {"id": ..., settings...} or a bare id.

use gtk4_layer_shell::Edge;
use serde_json::Value;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Position {
  #[default]
  Top,
  Bottom,
  Left,
  Right,
}

impl Position {
  pub fn parse(value: &str) -> Position {
    match value.trim() {
      "bottom" => Position::Bottom,
      "left" => Position::Left,
      "right" => Position::Right,
      _ => Position::Top,
    }
  }

  pub fn vertical(self) -> bool {
    matches!(self, Position::Left | Position::Right)
  }

  pub fn edge(self) -> Edge {
    match self {
      Position::Top => Edge::Top,
      Position::Bottom => Edge::Bottom,
      Position::Left => Edge::Left,
      Position::Right => Edge::Right,
    }
  }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
  pub left: Vec<Value>,
  pub center: Vec<Value>,
  pub right: Vec<Value>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BarConfig {
  pub position: Position,
  pub transparent: bool,
  pub floating: bool,
  pub locked: bool,
  pub center_anchor: String,
  pub layout: Layout,
}

pub fn entry_id(entry: &Value) -> String {
  match entry {
    Value::String(s) => s.clone(),
    Value::Object(o) => o.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
    _ => String::new(),
  }
}

// A setting of an entry, or None.
pub fn setting<'a>(entry: &'a Value, key: &str) -> Option<&'a Value> {
  entry.as_object().and_then(|o| o.get(key)).filter(|v| !v.is_null())
}

pub fn setting_str(entry: &Value, key: &str, fallback: &str) -> String {
  match setting(entry, key) {
    Some(Value::String(s)) => s.clone(),
    Some(v) => v.to_string(),
    None => fallback.into(),
  }
}

// The tray's drawer reveals inward, so the tray sits on its section's inner
// edge: first on the right, last elsewhere.
pub fn pin_tray_to_inner(entries: &[Value], right: bool) -> Vec<Value> {
  let mut tray = None;
  let mut out: Vec<Value> = Vec::new();
  for e in entries {
    if entry_id(e) == "omarchy.tray" { tray = Some(e.clone()) } else { out.push(e.clone()) }
  }
  if let Some(t) = tray {
    if right { out.insert(0, t) } else { out.push(t) }
  }
  out
}

pub fn parse(json: &Value) -> BarConfig {
  let bar = json.get("bar").cloned().unwrap_or_default();
  let section = |name: &str| bar["layout"][name].as_array().cloned().unwrap_or_default();
  BarConfig {
    position: Position::parse(bar["position"].as_str().unwrap_or("top")),
    transparent: bar["transparent"].as_bool() == Some(true),
    floating: bar["floating"].as_bool() == Some(true),
    locked: bar["locked"].as_bool() == Some(true),
    center_anchor: bar["centerAnchor"].as_str().unwrap_or("").to_string(),
    layout: Layout {
      left: pin_tray_to_inner(&section("left"), false),
      center: pin_tray_to_inner(&section("center"), false),
      right: pin_tray_to_inner(&section("right"), true),
    },
  }
}

pub fn load() -> BarConfig {
  let home = std::env::var("HOME").unwrap_or_default();
  let text = std::fs::read_to_string(format!("{home}/.config/omarchy/shell.json")).unwrap_or_default();
  parse(&serde_json::from_str(&text).unwrap_or_default())
}

// Writes `entry` over the first layout entry with its id (shell.json's
// updateEntryInline): a widget keeping a setting it changed, such as the
// clock's format.
pub fn update_entry(entry: &Value) -> bool {
  let id = entry_id(entry);
  let home = std::env::var("HOME").unwrap_or_default();
  let path = std::path::PathBuf::from(format!("{home}/.config/omarchy/shell.json"));
  let Ok(text) = std::fs::read_to_string(&path) else { return false };
  let Ok(mut json) = serde_json::from_str::<Value>(&text) else { return false };
  let mut done = false;
  for region in ["left", "center", "right"] {
    if let Some(list) = json["bar"]["layout"][region].as_array_mut() {
      if let Some(slot) = list.iter_mut().find(|e| entry_id(e) == id) {
        *slot = entry.clone();
        done = true;
        break;
      }
    }
  }
  if !done {
    return false;
  }
  let Ok(out) = serde_json::to_string_pretty(&json) else { return false };
  let tmp = path.with_extension("json.tmp");
  std::fs::write(&tmp, out + "\n").is_ok() && std::fs::rename(&tmp, &path).is_ok()
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn reads_the_bar_and_pins_the_tray_inward() {
    let cfg = parse(&json!({"bar": {
      "position": "left", "floating": true, "centerAnchor": "doxia.media",
      "layout": {"left": ["omarchy.tray", {"id": "a"}], "center": [], "right": [{"id": "b"}, {"id": "omarchy.tray"}]}
    }}));
    assert_eq!(cfg.position, Position::Left);
    assert!(cfg.floating && !cfg.transparent);
    assert_eq!(cfg.layout.left.iter().map(entry_id).collect::<Vec<_>>(), ["a", "omarchy.tray"]);
    assert_eq!(cfg.layout.right.iter().map(entry_id).collect::<Vec<_>>(), ["omarchy.tray", "b"]);
    assert_eq!(parse(&json!({})).position, Position::Top);
    assert_eq!(setting_str(&json!({"id": "c", "format": "HH:mm"}), "format", "x"), "HH:mm");
  }
}
