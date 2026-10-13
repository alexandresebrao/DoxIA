// painel.json, the Jira credentials and the pipelines' recent combinations:
// the same files and formats as the Python app this replaces.

use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;

pub const MAX_RECENT: usize = 6;

fn home() -> PathBuf {
  PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

fn xdg(var: &str, fallback: &str) -> PathBuf {
  std::env::var(var).ok().filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| home().join(fallback))
}

pub fn config_file() -> PathBuf {
  xdg("XDG_CONFIG_HOME", ".config").join("omarchy/painel.json")
}

pub fn credentials_file() -> PathBuf {
  xdg("XDG_CONFIG_HOME", ".config").join("omarchy/painel-credenciais.json")
}

fn state_dir() -> PathBuf {
  xdg("XDG_STATE_HOME", ".local/state").join("doxia-painel")
}

// "~/x" and "$HOME/x" style paths from painel.json.
pub fn expand_path(path: &str) -> PathBuf {
  let mut out = path.to_string();
  if out == "~" || out.starts_with("~/") {
    out = format!("{}{}", home().display(), &out[1..]);
  }
  while let Some(start) = out.find('$') {
    let rest = &out[start + 1..];
    let (name, len) = if let Some(inner) = rest.strip_prefix('{') {
      match inner.find('}') {
        Some(end) => (&inner[..end], end + 2),
        None => break,
      }
    } else {
      let end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(rest.len());
      (&rest[..end], end)
    };
    if name.is_empty() {
      break;
    }
    let value = std::env::var(name).unwrap_or_default();
    out = format!("{}{}{}", &out[..start], value, &out[start + 1 + len..]);
  }
  PathBuf::from(out)
}

pub fn tilde(path: &PathBuf) -> String {
  let h = home().display().to_string();
  path.display().to_string().replacen(&h, "~", 1)
}

// (config, error): no file is an empty config without an error.
pub fn load() -> (Value, Option<String>) {
  let path = config_file();
  let empty = json!({"actions": []});
  let text = match fs::read_to_string(&path) {
    Ok(t) => t,
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (empty, None),
    Err(e) => return (empty, Some(format!("{}: {e}", path.display()))),
  };
  match serde_json::from_str::<Value>(&text) {
    Ok(mut cfg) if cfg.is_object() && cfg.get("actions").is_none_or(Value::is_array) => {
      if cfg.get("actions").is_none() {
        cfg["actions"] = json!([]);
      }
      (cfg, None)
    }
    Ok(_) => (empty, Some(format!("{}: esperado um objeto com a lista \"actions\"", path.display()))),
    Err(e) => (empty, Some(format!("{}: {e}", path.display()))),
  }
}

pub fn has_page(kind: &str) -> bool {
  matches!(kind, "jira-request" | "jira-tasks" | "gitlab-pipeline" | "claude-text")
}

// Drops malformed actions: (valid, warnings).
pub fn validate(cfg: &Value) -> (Vec<Value>, Vec<String>) {
  let required = |kind: &str| -> Option<&'static [&'static str]> {
    Some(match kind {
      "jira-request" => &["serviceDesk", "requestType", "fields"],
      "jira-tasks" | "claude-text" => &[],
      "gitlab-pipeline" => &["project"],
      "link" => &["url"],
      "command" => &["run"],
      _ => return None,
    })
  };
  let present = |v: Option<&Value>| match v {
    None | Some(Value::Null) => false,
    Some(Value::String(s)) => !s.is_empty(),
    Some(Value::Array(a)) => !a.is_empty(),
    Some(Value::Bool(b)) => *b,
    Some(_) => true,
  };
  let mut valid = Vec::new();
  let mut warnings = Vec::new();
  let mut seen = HashSet::new();
  for a in cfg["actions"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
    if !a.is_object() {
      warnings.push("ação que não é um objeto".into());
      continue;
    }
    let id = str_of(a, "id");
    let kind = str_of(a, "type");
    let Some(needs) = required(kind).filter(|_| !id.is_empty() && present(a.get("title"))) else {
      warnings.push(format!("ação {}: precisa de id, title e um type conhecido", if id.is_empty() { "?" } else { id }));
      continue;
    };
    if seen.contains(id) || id == "home" || id == "credenciais" {
      warnings.push(format!("ação {id}: id repetido ou reservado"));
      continue;
    }
    let lacking: Vec<&str> = needs.iter().copied().filter(|k| !present(a.get(*k))).collect();
    if !lacking.is_empty() {
      warnings.push(format!("ação {id}: faltando {}", lacking.join(", ")));
      continue;
    }
    seen.insert(id.to_string());
    valid.push(a.clone());
  }
  (valid, warnings)
}

pub fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
  v.get(key).and_then(Value::as_str).unwrap_or("")
}

// Numbers and strings alike ("10" or 10 in painel.json).
pub fn text_of(v: &Value, key: &str) -> String {
  match v.get(key) {
    Some(Value::String(s)) => s.clone(),
    Some(Value::Null) | None => String::new(),
    Some(other) => other.to_string(),
  }
}

pub fn jira_site(cfg: &Value, action: &Value) -> String {
  let site = match str_of(action, "site") {
    "" => cfg.get("jira").map(|j| str_of(j, "site")).unwrap_or(""),
    s => s,
  };
  site.trim_end_matches('/').to_string()
}

#[derive(Clone, Default)]
pub struct Credentials {
  pub email: String,
  pub token: String,
}

pub fn load_credentials(site: &str) -> Credentials {
  let data: Value = fs::read_to_string(credentials_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
  let entry = &data[site];
  Credentials { email: str_of(entry, "email").to_string(), token: str_of(entry, "token").to_string() }
}

// Mode 600 from the first byte: the file holds API tokens.
pub fn save_credentials(site: &str, creds: &Credentials) -> std::io::Result<()> {
  let path = credentials_file();
  let mut data: Value = fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| json!({}));
  if !data.is_object() {
    data = json!({});
  }
  data[site] = json!({"email": creds.email, "token": creds.token});
  if let Some(dir) = path.parent() {
    fs::create_dir_all(dir)?;
  }
  let mut f = fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&path)?;
  f.write_all(serde_json::to_string_pretty(&data).unwrap_or_default().as_bytes())?;
  fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
}

pub type Combo = Map<String, Value>;

fn recent_file(action_id: &str) -> PathBuf {
  state_dir().join(format!("{action_id}.json"))
}

// The last combinations that have every required key.
pub fn load_recent(action_id: &str, keys: &[String]) -> Vec<Combo> {
  let data: Value = fs::read_to_string(recent_file(action_id)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
  data["recent"]
    .as_array()
    .map(|list| {
      list
        .iter()
        .filter_map(|r| r.as_object().cloned())
        .filter(|r| keys.iter().all(|k| r.get(k).and_then(Value::as_str).is_some_and(|s| !s.is_empty())))
        .collect()
    })
    .unwrap_or_default()
}

pub fn save_recent(action_id: &str, keys: &[String], combo: &Combo) {
  let mut recent = vec![combo.clone()];
  recent.extend(load_recent(action_id, keys).into_iter().filter(|r| r != combo));
  recent.truncate(MAX_RECENT);
  let _ = fs::create_dir_all(state_dir());
  let _ = fs::write(recent_file(action_id), serde_json::to_string_pretty(&json!({"recent": recent})).unwrap_or_default());
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn validate_reports_like_the_python_app() {
    let cfg = json!({"actions": [
      {"id": "a", "type": "link", "title": "A", "url": "https://x"},
      {"id": "a", "type": "link", "title": "dup", "url": "https://x"},
      {"id": "home", "type": "link", "title": "H", "url": "https://x"},
      {"id": "b", "type": "jira-request", "title": "B", "serviceDesk": 217},
      {"id": "c", "type": "nope", "title": "C"},
      3
    ]});
    let (valid, warnings) = validate(&cfg);
    assert_eq!(valid.len(), 1);
    assert_eq!(warnings, vec![
      "ação a: id repetido ou reservado",
      "ação home: id repetido ou reservado",
      "ação b: faltando requestType, fields",
      "ação c: precisa de id, title e um type conhecido",
      "ação que não é um objeto",
    ]);
  }

  #[test]
  fn expands_home_and_vars() {
    let h = home().display().to_string();
    assert_eq!(expand_path("~/x").display().to_string(), format!("{h}/x"));
    assert_eq!(expand_path("$HOME/y").display().to_string(), format!("{h}/y"));
    assert_eq!(expand_path("${HOME}/z").display().to_string(), format!("{h}/z"));
  }

  #[test]
  fn site_falls_back_to_the_default() {
    let cfg = json!({"jira": {"site": "https://a.atlassian.net/"}});
    assert_eq!(jira_site(&cfg, &json!({})), "https://a.atlassian.net");
    assert_eq!(jira_site(&cfg, &json!({"site": "https://b.net"})), "https://b.net");
  }
}
