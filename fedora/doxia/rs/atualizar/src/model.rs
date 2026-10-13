// The data behind the window, as in shell/plugins/update/UpdateModel.js (the
// bar icon still uses that one). Three JSON files:
//   status  ~/.cache/omarchy/update-status.json         omarchy-update-check
//   run     $XDG_RUNTIME_DIR/omarchy-update/state.json  omarchy-update-state (run in progress)
//   last    ~/.local/state/omarchy/update-last.json     last finished run (or boot finish)

pub use serde_json::Value;
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> f64 {
  SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

pub fn parse(text: &str) -> Option<Value> {
  serde_json::from_str::<Value>(text).ok().filter(Value::is_object)
}

pub fn read(path: &str) -> Option<Value> {
  std::fs::read_to_string(path).ok().and_then(|t| parse(&t))
}

pub fn num(v: &Value, key: &str) -> f64 {
  v.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

pub fn text<'a>(v: &'a Value, key: &str) -> &'a str {
  v.get(key).and_then(Value::as_str).unwrap_or("")
}

pub fn flag(v: &Value, key: &str) -> bool {
  v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn list<'a>(v: Option<&'a Value>, path: &[&str]) -> &'a [Value] {
  let mut cur = v;
  for key in path {
    cur = cur.and_then(|c| c.get(*key));
  }
  cur.and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

pub fn system_count(status: Option<&Value>) -> usize {
  list(status, &["system"]).len()
}

fn flatpak_runtimes(status: Option<&Value>) -> usize {
  status.and_then(|s| s.get("flatpak")).map(|f| num(f, "runtimes") as usize).unwrap_or(0)
}

pub fn flatpak_count(status: Option<&Value>) -> usize {
  list(status, &["flatpak", "apps"]).len() + flatpak_runtimes(status)
}

pub fn doxia_behind(status: Option<&Value>) -> usize {
  status.and_then(|s| s.get("doxia")).map(|d| num(d, "behind") as usize).unwrap_or(0)
}

pub fn doxia_first_commit(status: Option<&Value>) -> String {
  list(status, &["doxia", "commits"]).first().and_then(Value::as_str).unwrap_or("").to_string()
}

pub fn doxia_head(status: Option<&Value>) -> String {
  status.and_then(|s| s.get("doxia")).map(|d| text(d, "head").to_string()).unwrap_or_default()
}

pub fn staged_pending(status: Option<&Value>) -> bool {
  status.and_then(|s| s.get("staged")).map(|s| flag(s, "pending")).unwrap_or(false)
}

pub fn staged_packages(status: Option<&Value>) -> Vec<String> {
  list(status, &["staged", "packages"]).iter().filter_map(|p| p.as_str().map(String::from)).collect()
}

pub fn staged_at(status: Option<&Value>) -> f64 {
  status.and_then(|s| s.get("staged")).map(|s| num(s, "stagedAt")).unwrap_or(0.0)
}

pub fn errors(status: Option<&Value>) -> Vec<String> {
  list(status, &["errors"]).iter().filter_map(|e| e.as_str().map(String::from)).collect()
}

pub fn anything(status: Option<&Value>) -> bool {
  system_count(status) > 0 || flatpak_count(status) > 0 || doxia_behind(status) > 0 || staged_pending(status)
}

pub fn only_flatpak(status: Option<&Value>) -> bool {
  flatpak_count(status) > 0 && system_count(status) == 0 && doxia_behind(status) == 0
}

pub fn plural(n: usize, one: &str, many: &str) -> String {
  format!("{n} {}", if n == 1 { one } else { many })
}

const SUBPACKAGE_SUFFIXES: [&str; 10] =
  ["libs", "selinux", "session-helper", "extensions", "common", "devel", "data", "core", "modules", "tools"];

// "flatpak, nautilus, rsync +6": subpackages of the same name collapse.
pub fn system_names(status: Option<&Value>, max: usize) -> String {
  let mut seen = HashSet::new();
  let mut names = Vec::new();
  for pkg in list(status, &["system"]) {
    let name = text(pkg, "name");
    let base = SUBPACKAGE_SUFFIXES
      .iter()
      .find_map(|s| name.strip_suffix(&format!("-{s}")))
      .unwrap_or(name);
    if seen.insert(base.to_string()) {
      names.push(base.to_string());
    }
  }
  let shown = names.iter().take(max).cloned().collect::<Vec<_>>().join(", ");
  if names.len() > max { format!("{shown} +{}", names.len() - max) } else { shown }
}

pub fn flatpak_names(status: Option<&Value>, max: usize) -> String {
  let apps: Vec<&str> = list(status, &["flatpak", "apps"]).iter().map(|a| text(a, "name")).collect();
  let mut out = apps.iter().take(max).copied().collect::<Vec<_>>().join(", ");
  if apps.len() > max {
    out += &format!(" +{}", apps.len() - max);
  }
  let runtimes = flatpak_runtimes(status);
  if runtimes > 0 {
    if !out.is_empty() {
      out += " · ";
    }
    out += &plural(runtimes, "runtime", "runtimes");
  }
  out
}

pub fn updates_count(status: Option<&Value>) -> usize {
  system_count(status) + flatpak_count(status) + usize::from(doxia_behind(status) > 0)
}

// Local time as HH:MM.
pub fn clock(epoch: f64) -> String {
  if epoch <= 0.0 {
    return String::new();
  }
  let dt = gtk::glib::DateTime::from_unix_local(epoch as i64);
  dt.and_then(|d| d.format("%H:%M")).map(|s| s.to_string()).unwrap_or_default()
}

pub fn ago(epoch: f64) -> String {
  if epoch <= 0.0 {
    return "nunca".into();
  }
  let s = (now() - epoch).max(0.0) as u64;
  match s {
    0..60 => "agora".into(),
    60..3600 => format!("há {} min", s / 60),
    3600..86400 => format!("há {} h", s / 3600),
    _ => format!("há {} d", s / 86400),
  }
}

pub fn duration(seconds: f64) -> String {
  let seconds = seconds.max(0.0).round() as u64;
  if seconds < 60 {
    return format!("{seconds} s");
  }
  let rest = seconds % 60;
  if rest > 0 { format!("{} min {rest} s", seconds / 60) } else { format!("{} min", seconds / 60) }
}

pub fn steps(run: Option<&Value>) -> Vec<Value> {
  list(run, &["steps"]).to_vec()
}

pub fn current_step(run: Option<&Value>) -> Option<Value> {
  let current = run.map(|r| text(r, "current")).unwrap_or("");
  list(run, &["steps"]).iter().find(|s| text(s, "id") == current).cloned()
}

fn step_label(run: &Value, id: &str) -> String {
  list(Some(run), &["steps"])
    .iter()
    .find(|s| text(s, "id") == id)
    .map(|s| text(s, "label").to_string())
    .unwrap_or_else(|| id.to_string())
}

// A finished run's failure message: omarchy-update records the id of the step
// that failed; the boot finish writes a sentence.
pub fn failure_text(last: Option<&Value>) -> String {
  let Some(last) = last else { return String::new() };
  let msg = text(last, "message");
  if !msg.is_empty() && !msg.contains(' ') {
    return format!("A atualização parou no passo “{}”.", step_label(last, msg));
  }
  if msg.is_empty() { "A atualização parou antes de terminar.".into() } else { msg.to_string() }
}

// Drops ANSI escapes: CSI sequences, charset selection and keypad modes.
fn strip_ansi(input: &str) -> String {
  let mut out = String::with_capacity(input.len());
  let mut chars = input.chars().peekable();
  while let Some(c) = chars.next() {
    if c != '\x1b' {
      out.push(c);
      continue;
    }
    match chars.next() {
      Some('[') => {
        // Parameters and intermediates, then one final byte in @..~.
        for c in chars.by_ref() {
          if ('@'..='~').contains(&c) {
            break;
          }
        }
      }
      Some(']') => {
        // OSC (sudo's 3008 session marks): up to BEL or ESC \.
        while let Some(c) = chars.next() {
          if c == '\x07' {
            break;
          }
          if c == '\x1b' {
            chars.next();
            break;
          }
        }
      }
      Some('(' | ')') => {
        chars.next();
      }
      _ => {}
    }
  }
  out
}

// Last lines of the update transcript (script(1) output): no ANSI codes, and
// only what each carriage return left on screen.
pub fn log_lines(raw: &str, count: usize) -> Vec<String> {
  let clean = strip_ansi(raw);
  let lines: Vec<String> = clean
    .split('\n')
    .map(|l| l.split('\r').filter(|p| !p.is_empty()).last().unwrap_or("").replace('\0', ""))
    .filter(|l| !l.trim().is_empty() && !l.starts_with("Script started"))
    .collect();
  lines[lines.len().saturating_sub(count)..].to_vec()
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn names_collapse_subpackages() {
    let s = json!({"system": [{"name": "flatpak"}, {"name": "flatpak-libs"}, {"name": "rsync"}, {"name": "a"}, {"name": "b"}, {"name": "c"}]});
    assert_eq!(system_names(Some(&s), 4), "flatpak, rsync, a, b +1");
  }

  #[test]
  fn flatpak_names_with_runtimes() {
    let s = json!({"flatpak": {"apps": [{"name": "A"}, {"name": "B"}], "runtimes": 1}});
    assert_eq!(flatpak_names(Some(&s), 3), "A, B · 1 runtime");
    assert_eq!(flatpak_count(Some(&s)), 3);
  }

  #[test]
  fn failure_names_the_step() {
    let l = json!({"message": "hooks", "steps": [{"id": "hooks", "label": "Configuração do DoxIA"}]});
    assert_eq!(failure_text(Some(&l)), "A atualização parou no passo “Configuração do DoxIA”.");
    assert_eq!(failure_text(Some(&json!({"message": "Senha não confirmada"}))), "Senha não confirmada");
  }

  #[test]
  fn log_keeps_last_carriage_return() {
    let raw = "Script started on x\n\x1b[32mUpdate\x1b[0m\nprogress 10%\rprogress 90%\r\n\x1b]3008;start=1\x1b\\done\n";
    assert_eq!(log_lines(raw, 9), vec!["Update", "progress 90%", "done"]);
  }

  #[test]
  fn durations() {
    assert_eq!(duration(59.4), "59 s");
    assert_eq!(duration(120.0), "2 min");
    assert_eq!(duration(125.0), "2 min 5 s");
  }
}
