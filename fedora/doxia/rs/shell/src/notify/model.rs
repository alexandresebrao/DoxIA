// What a notification is once received, and the rules around it: the port of
// shell/plugins/notifications/NotificationLogic.js. The files it writes are the
// ones the Quickshell service writes (one compact JSON line per popup, named
// <timestamp>-<originalId>.json), so either side can restore the other's
// popups and history.

use serde_json::{Map, Value, json};

pub const LOW: u8 = 0;
pub const NORMAL: u8 = 1;
pub const CRITICAL: u8 = 2;

pub const HISTORY_LIMIT: usize = 50;
// Popups on screen at once; an older one beyond them goes to history.
pub const MAX_VISIBLE: usize = 3;
const LOW_MS: u64 = 5000;
const NORMAL_MS: u64 = 8000;
const CRITICAL_MS: u64 = 20000;
const MAX_MS: u64 = 30000;

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
  pub id: u32,
  pub original_id: u32,
  pub app: String,
  pub app_icon: String,
  pub summary: String,
  pub body: String,
  pub image: String,
  pub glyph: String,
  pub exec_argv: String,
  pub urgency: u8,
  pub expire_timeout: f64,
  pub timestamp: i64,
  // Absolute expiry set when a restore restarts a surviving popup's lifetime.
  pub deadline: Option<i64>,
}

impl Default for Entry {
  fn default() -> Entry {
    Entry {
      id: 0,
      original_id: 0,
      app: String::new(),
      app_icon: String::new(),
      summary: String::new(),
      body: String::new(),
      image: String::new(),
      glyph: String::new(),
      exec_argv: String::new(),
      urgency: NORMAL,
      expire_timeout: 0.0,
      timestamp: 0,
      deadline: None,
    }
  }
}

fn s(v: &Value, key: &str) -> String {
  v.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

fn n(v: &Value, key: &str) -> f64 {
  v.get(key).and_then(Value::as_f64).filter(|x| x.is_finite()).unwrap_or(0.0)
}

impl Entry {
  // popupEntry: a parsed file line.
  pub fn from_json(v: &Value) -> Entry {
    let id = n(v, "id") as u32;
    let original = match n(v, "originalId") as u32 {
      0 => id,
      o => o,
    };
    Entry {
      id,
      original_id: original,
      app: s(v, "app"),
      app_icon: s(v, "appIcon"),
      summary: s(v, "summary"),
      body: s(v, "body"),
      image: s(v, "image"),
      glyph: s(v, "glyph"),
      exec_argv: s(v, "execArgv"),
      urgency: v.get("urgency").and_then(Value::as_u64).map(|u| u.min(2) as u8).unwrap_or(NORMAL),
      expire_timeout: n(v, "expireTimeout").max(0.0),
      timestamp: n(v, "timestamp") as i64,
      deadline: Some(n(v, "deadline") as i64).filter(|d| *d > 0),
    }
  }

  // serializePopup: one line, the keys the QML service writes.
  pub fn to_json(&self) -> String {
    let mut m = Map::new();
    m.insert("id".into(), json!(self.id));
    m.insert("originalId".into(), json!(self.original_id));
    m.insert("app".into(), json!(self.app));
    m.insert("appIcon".into(), json!(self.app_icon));
    m.insert("summary".into(), json!(self.summary));
    m.insert("body".into(), json!(self.body));
    m.insert("image".into(), json!(self.image));
    m.insert("glyph".into(), json!(self.glyph));
    m.insert("execArgv".into(), json!(self.exec_argv));
    m.insert("urgency".into(), json!(self.urgency));
    m.insert("expireTimeout".into(), json!(self.expire_timeout));
    m.insert("timestamp".into(), json!(self.timestamp));
    if let Some(d) = self.deadline {
      m.insert("deadline".into(), json!(d));
    }
    Value::Object(m).to_string()
  }

  // historyEntry: what a replay shows, with the standard lifetime.
  pub fn for_history(&self) -> Entry {
    Entry { expire_timeout: 0.0, deadline: None, ..self.clone() }
  }

  pub fn stem(&self) -> String {
    format!("{}-{}", self.timestamp, self.original_id)
  }

  pub fn file_name(&self) -> String {
    format!("{}.json", self.stem())
  }

  // What the card draws: a change in any of these is an in-place update.
  pub fn same_content(&self, other: &Entry) -> bool {
    self.app == other.app
      && self.app_icon == other.app_icon
      && self.summary == other.summary
      && self.body == other.body
      && self.image == other.image
      && self.glyph == other.glyph
      && self.exec_argv == other.exec_argv
      && self.urgency == other.urgency
      && self.expire_timeout == other.expire_timeout
  }
}

// How long a popup stays: the sender's timeout, honoured within [5 s, 8 s or
// 20 s (critical), 30 s]. Nothing stays for good: the history keeps it.
pub fn duration_for(urgency: u8, expire_timeout: f64) -> u64 {
  let requested = if expire_timeout.is_finite() && expire_timeout > 0.0 { expire_timeout.round() as u64 } else { 0 };
  match urgency {
    CRITICAL => requested.max(CRITICAL_MS).min(MAX_MS),
    LOW => requested.max(LOW_MS).min(MAX_MS),
    _ => requested.max(NORMAL_MS).min(MAX_MS),
  }
}

// Through do-not-disturb: Omarchy's own action toasts, and critical alerts
// from the bare CLI (chat apps set their own app name).
pub fn bypasses_dnd(app: &str, urgency: u8) -> bool {
  app == "omarchy-action" || (app == "notify-send" && urgency == CRITICAL)
}

pub fn is_ephemeral_app(app: &str) -> bool {
  app == "notify-send" || app == "omarchy-action"
}

// A JSON argv from omarchy-notification-send --exec, checked for shape only.
pub fn parse_exec_argv(value: &str) -> Option<Vec<String>> {
  let parsed: Value = serde_json::from_str(value).ok()?;
  let list = parsed.as_array()?;
  let argv: Vec<String> = list.iter().map(|x| x.as_str().map(String::from)).collect::<Option<_>>()?;
  match argv.first() {
    Some(p) if !p.is_empty() && !p.starts_with('-') => Some(argv),
    _ => None,
  }
}

pub fn popup_expired(entry: &Entry, duration: u64, now: i64) -> bool {
  if let Some(d) = entry.deadline {
    return now >= d;
  }
  duration > 0 && now - entry.timestamp >= duration as i64
}

// One entry per line, newest first; torn lines are skipped.
pub fn parse_files(raw: &str) -> Vec<Entry> {
  let mut out: Vec<Entry> = raw
    .lines()
    .filter_map(|l| serde_json::from_str::<Value>(l.trim()).ok())
    .filter(Value::is_object)
    .map(|v| Entry::from_json(&v))
    .collect();
  out.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
  out
}

// The replay: the toasts on screen plus the archived ones, newest first,
// one per file name.
pub fn history_rows(archived: &[Entry], live: &[Entry], limit: usize) -> Vec<Entry> {
  let mut seen = std::collections::HashSet::new();
  let mut out: Vec<Entry> = live
    .iter()
    .chain(archived.iter())
    .filter(|e| seen.insert(e.file_name()))
    .map(Entry::for_history)
    .collect();
  out.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
  out.truncate(limit);
  out
}

pub fn is_chromium(app: &str, app_icon: &str) -> bool {
  let source = format!("{app}\n{app_icon}").to_lowercase();
  ["chrom", "brave", "vivaldi", "microsoft-edge", "opera"].iter().any(|b| source.contains(b))
}

// "X  text": a summary that already leads with a glyph and two spaces.
pub fn summary_starts_with_glyph(summary: &str) -> bool {
  let mut chars = summary.trim_start().chars();
  if chars.next().is_none() {
    return false;
  }
  chars.take_while(|c| *c == ' ').count() >= 2
}

// The filesystem path behind a file-backed image, or "" for themed icon names.
pub fn local_image_file(value: &str) -> String {
  let path = match value.strip_prefix("file://") {
    Some(p) => gtk::glib::uri_unescape_string(p, None::<&str>).map(|s| s.to_string()).unwrap_or_else(|| p.to_string()),
    None => value.to_string(),
  };
  if path.starts_with('/') { path } else { String::new() }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn durations() {
    assert_eq!(duration_for(CRITICAL, 5000.0), 20000);
    assert_eq!(duration_for(LOW, 0.0), 5000);
    assert_eq!(duration_for(NORMAL, 3000.0), 8000);
    assert_eq!(duration_for(NORMAL, 12000.0), 12000);
    assert_eq!(duration_for(NORMAL, 99000.0), 30000);
  }

  #[test]
  fn json_round_trip_matches_the_qml_format() {
    let line = r#"{"id":3,"originalId":3,"app":"Slack","appIcon":"","summary":"Oi","body":"b","image":"","glyph":"","execArgv":"","urgency":1,"expireTimeout":0,"timestamp":1791900000000}"#;
    let e = Entry::from_json(&serde_json::from_str(line).unwrap());
    assert_eq!(e.file_name(), "1791900000000-3.json");
    assert_eq!(Entry::from_json(&serde_json::from_str(&e.to_json()).unwrap()), e);
  }

  #[test]
  fn dnd_and_exec_rules() {
    assert!(bypasses_dnd("omarchy-action", LOW));
    assert!(bypasses_dnd("notify-send", CRITICAL));
    assert!(!bypasses_dnd("Slack", CRITICAL));
    assert_eq!(parse_exec_argv(r#"["xdg-open","/tmp/a b"]"#), Some(vec!["xdg-open".into(), "/tmp/a b".into()]));
    assert_eq!(parse_exec_argv(r#"["-rf"]"#), None);
    assert_eq!(parse_exec_argv(r#"[1]"#), None);
    assert_eq!(parse_exec_argv("nope"), None);
  }

  #[test]
  fn expiry_and_history() {
    let e = Entry { timestamp: 1000, original_id: 1, ..Default::default() };
    assert!(popup_expired(&e, 8000, 9000));
    assert!(!popup_expired(&e, 8000, 8999));
    assert!(!popup_expired(&e, 0, 1_000_000));
    assert!(!popup_expired(&Entry { deadline: Some(20000), ..e.clone() }, 8000, 15000));
    let a = Entry { timestamp: 5, original_id: 2, expire_timeout: 9.0, ..Default::default() };
    let rows = history_rows(&[e.clone(), a.clone()], &[a.clone()], 10);
    assert_eq!(rows.iter().map(|r| r.timestamp).collect::<Vec<_>>(), vec![1000, 5]);
    assert_eq!(rows[1].expire_timeout, 0.0);
  }

  #[test]
  fn glyph_summaries() {
    assert!(summary_starts_with_glyph("\u{f0e7}  Pronto"));
    assert!(!summary_starts_with_glyph("Pronto agora"));
  }
}
