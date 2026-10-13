// Jira, GitLab and Claude. Every call blocks: the pages run them with
// gio::spawn_blocking and get the result back on the GTK thread.

use crate::config::{self, Credentials, str_of, text_of};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const CLAUDE_MODEL: &str = "sonnet";
const CLAUDE_TIMEOUT: Duration = Duration::from_secs(180);

pub type Result<T> = std::result::Result<T, String>;

fn agent() -> ureq::Agent {
  ureq::Agent::config_builder()
    .timeout_global(Some(Duration::from_secs(30)))
    .http_status_as_error(false)
    .build()
    .into()
}

fn base64(input: &[u8]) -> String {
  const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  let mut out = String::new();
  for chunk in input.chunks(3) {
    let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
    let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
    for i in 0..4 {
      if i <= chunk.len() {
        out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
      } else {
        out.push('=');
      }
    }
  }
  out
}

// Everything but unreserved characters, like Python's quote(safe="").
pub fn quote(s: &str) -> String {
  s.bytes()
    .map(|b| match b {
      b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
      _ => format!("%{b:02X}"),
    })
    .collect()
}

// (status, body) for any HTTP answer; Err only when there was none.
fn send(method: &str, url: &str, headers: &[(&str, String)], body: Option<&Value>) -> Result<(u16, String)> {
  let agent = agent();
  let result = match (method, body) {
    ("GET", _) => {
      let mut req = agent.get(url);
      for (k, v) in headers {
        req = req.header(*k, v);
      }
      req.call()
    }
    (_, body) => {
      let mut req = agent.post(url);
      for (k, v) in headers {
        req = req.header(*k, v);
      }
      req.send(body.map(Value::to_string).unwrap_or_default())
    }
  };
  let mut resp = result.map_err(|e| format!("Falha de conexão: {e}"))?;
  let status = resp.status().as_u16();
  let text = resp.body_mut().read_to_string().unwrap_or_default();
  Ok((status, text))
}

fn parse_ok(text: &str) -> Value {
  if text.trim().is_empty() { json!({}) } else { serde_json::from_str(text).unwrap_or_else(|_| json!({})) }
}

// ---- Jira ---------------------------------------------------------------------

pub fn jira(site: &str, creds: &Credentials, method: &str, path: &str, body: Option<Value>) -> Result<Value> {
  let auth = base64(format!("{}:{}", creds.email, creds.token).as_bytes());
  let headers = [
    ("Authorization", format!("Basic {auth}")),
    ("Accept", "application/json".into()),
    ("Content-Type", "application/json".into()),
  ];
  let (status, text) = send(method, &format!("{site}{path}"), &headers, body.as_ref())?;
  if (200..300).contains(&status) {
    return Ok(parse_ok(&text));
  }
  let msg = if status == 401 {
    "E-mail ou API token inválidos.".to_string()
  } else {
    match serde_json::from_str::<Value>(&text) {
      Ok(data) => {
        let mut msg = match str_of(&data, "errorMessage") {
          "" => data["errorMessages"]
            .as_array()
            .map(|m| m.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("; "))
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| text.clone()),
          m => m.to_string(),
        };
        if let Some(errors) = data["errors"].as_object().filter(|e| !e.is_empty()) {
          let parts: Vec<String> = errors.iter().map(|(k, v)| format!("{k}: {}", v.as_str().unwrap_or(&v.to_string()))).collect();
          msg += &format!(" {}", parts.join("; "));
        }
        msg
      }
      Err(_) => text.chars().take(300).collect(),
    }
  };
  Err(format!("HTTP {status}: {msg}"))
}

pub fn jira_create_request(site: &str, creds: &Credentials, action: &Value, values: Value) -> Result<Value> {
  jira(
    site,
    creds,
    "POST",
    "/rest/servicedeskapi/request",
    Some(json!({
      "serviceDeskId": text_of(action, "serviceDesk"),
      "requestTypeId": text_of(action, "requestType"),
      "requestFieldValues": values,
    })),
  )
}

pub fn jira_search(site: &str, creds: &Credentials, jql: &str) -> Result<Vec<Value>> {
  let mut issues = Vec::new();
  let mut token: Option<String> = None;
  loop {
    let mut body = json!({"jql": jql, "maxResults": 100,
      "fields": ["summary", "status", "priority", "project", "issuetype", "updated"]});
    if let Some(t) = &token {
      body["nextPageToken"] = json!(t);
    }
    let page = jira(site, creds, "POST", "/rest/api/3/search/jql", Some(body))?;
    issues.extend(page["issues"].as_array().cloned().unwrap_or_default());
    token = page["nextPageToken"].as_str().map(String::from);
    if page["isLast"].as_bool().unwrap_or(true) || token.is_none() {
      return Ok(issues);
    }
  }
}

// ---- GitLab -------------------------------------------------------------------

fn gitlab_token(host: &str) -> Result<String> {
  let out = Command::new("glab").args(["config", "get", "token", "--host", host]).stdin(Stdio::null()).output();
  let token = out.ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
  if token.is_empty() {
    return Err(format!("Sem token do GitLab para {host}. Rode `glab auth login` no terminal."));
  }
  Ok(token)
}

fn gitlab(host: &str, method: &str, path: &str, body: Option<Value>) -> Result<Value> {
  let headers = [("PRIVATE-TOKEN", gitlab_token(host)?), ("Content-Type", "application/json".to_string())];
  let (status, text) = send(method, &format!("https://{host}/api/v4{path}"), &headers, body.as_ref())?;
  if (200..300).contains(&status) {
    return Ok(serde_json::from_str(&text).unwrap_or_else(|_| parse_ok(&text)));
  }
  let msg = if status == 401 {
    "token do glab inválido ou expirado (rode `glab auth login`).".to_string()
  } else {
    match serde_json::from_str::<Value>(&text) {
      Ok(data) => {
        let m = data.get("message").or_else(|| data.get("error")).cloned().unwrap_or(Value::String(text.clone()));
        match m {
          Value::Object(map) => map
            .iter()
            .map(|(k, v)| match v {
              Value::Array(a) => format!("{k}: {}", a.iter().map(|x| x.as_str().map(String::from).unwrap_or(x.to_string())).collect::<Vec<_>>().join(", ")),
              v => format!("{k}: {}", v.as_str().map(String::from).unwrap_or(v.to_string())),
            })
            .collect::<Vec<_>>()
            .join("; "),
          Value::Array(a) => a.iter().map(|x| x.as_str().map(String::from).unwrap_or(x.to_string())).collect::<Vec<_>>().join("; "),
          Value::String(s) => s,
          other => other.to_string(),
        }
      }
      Err(_) => text.chars().take(300).collect(),
    }
  };
  Err(format!("HTTP {status}: {msg}"))
}

pub fn gitlab_branches(host: &str, project: &str) -> Result<Vec<String>> {
  let data = gitlab(host, "GET", &format!("/projects/{}/repository/branches?per_page=100", quote(project)), None)?;
  let mut names: Vec<String> =
    data.as_array().map(|a| a.iter().map(|b| str_of(b, "name").to_string()).collect()).unwrap_or_default();
  names.sort();
  Ok(names)
}

pub fn gitlab_pipeline(host: &str, project: &str, reference: &str, variables: &[(String, String)]) -> Result<Value> {
  let vars: Vec<Value> = variables.iter().map(|(k, v)| json!({"key": k, "value": v})).collect();
  gitlab(host, "POST", &format!("/projects/{}/pipeline", quote(project)), Some(json!({"ref": reference, "variables": vars})))
}

// ---- Claude -------------------------------------------------------------------

fn claude_prompt(spec: &Value) -> Result<String> {
  match str_of(spec, "promptFile") {
    "" => Ok(str_of(spec, "prompt").to_string()),
    file => std::fs::read_to_string(config::expand_path(file)).map_err(|e| format!("Prompt do Claude: {e}")),
  }
}

// A new version of the text from Claude Code (-p, no tools), with the other
// fields as context.
pub fn claude_rewrite(spec: &Value, context: &[(String, String)], text: &str) -> Result<String> {
  let mut prompt: String = context
    .iter()
    .map(|(label, value)| format!("{label}: {}\n", if value.is_empty() { "(não informado)" } else { value }))
    .collect();
  prompt += &format!("\nTexto escrito por quem está preenchendo:\n{text}");
  let system = claude_prompt(spec)?;
  let model = match str_of(spec, "model") {
    "" => CLAUDE_MODEL,
    m => m,
  };
  let mut child = Command::new("claude")
    .args(["-p", "--model", model, "--system-prompt", &system, "--tools", "", "--strict-mcp-config"])
    .args(["--setting-sources", "", "--no-session-persistence"])
    .current_dir(std::env::temp_dir())
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|e| {
      if e.kind() == std::io::ErrorKind::NotFound { "Comando claude não encontrado no PATH".to_string() } else { e.to_string() }
    })?;
  if let Some(mut stdin) = child.stdin.take() {
    let _ = stdin.write_all(prompt.as_bytes());
  }
  let read = |pipe: Option<Box<dyn Read + Send>>| {
    std::thread::spawn(move || {
      let mut s = String::new();
      if let Some(mut p) = pipe {
        let _ = p.read_to_string(&mut s);
      }
      s
    })
  };
  let out = read(child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
  let err = read(child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
  let deadline = Instant::now() + CLAUDE_TIMEOUT;
  let status = loop {
    match child.try_wait() {
      Ok(Some(status)) => break status,
      Ok(None) if Instant::now() > deadline => {
        let _ = child.kill();
        let _ = child.wait();
        return Err("O Claude demorou demais para responder".into());
      }
      Ok(None) => std::thread::sleep(Duration::from_millis(100)),
      Err(e) => return Err(e.to_string()),
    }
  };
  let stdout = out.join().unwrap_or_default().trim().to_string();
  let stderr = err.join().unwrap_or_default();
  if !status.success() || stdout.is_empty() {
    let source = if stderr.trim().is_empty() { stdout.clone() } else { stderr };
    let last = source.trim().lines().last().map(String::from);
    return Err(format!("Claude: {}", last.unwrap_or_else(|| format!("saiu com código {}", status.code().unwrap_or(-1)))));
  }
  Ok(stdout)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn base64_pads() {
    assert_eq!(base64(b"a:b"), "YTpi");
    assert_eq!(base64(b"ab"), "YWI=");
    assert_eq!(base64(b"a"), "YQ==");
  }

  #[test]
  fn quotes_project_paths() {
    assert_eq!(quote("grupo/projeto x"), "grupo%2Fprojeto%20x");
  }
}
