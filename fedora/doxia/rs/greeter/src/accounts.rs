// What the login screen offers: the people's accounts (/etc/passwd, the uid
// range /etc/login.defs gives them), the Wayland sessions installed, and the
// last account and session used (kept in greetd's home, the greeter's user).

use std::path::{Path, PathBuf};

const STATE: &str = "/var/lib/greetd/doxia-greeter.state";
const SESSION_DIRS: [&str; 2] = ["/usr/local/share/wayland-sessions", "/usr/share/wayland-sessions"];
// The DoxIA session, for a first boot with nothing remembered yet.
pub const DEFAULT_SESSION: &str = "/usr/share/wayland-sessions/omarchy.desktop";

#[derive(Clone, Debug, PartialEq)]
pub struct User {
  pub name: String,
  pub real: String,
  pub shell: String,
  pub face: Option<PathBuf>,
}

impl User {
  // As the screen shows it: the full name when there is one.
  pub fn shown(&self) -> &str {
    if self.real.is_empty() { &self.name } else { &self.real }
  }
}

fn login_defs(text: &str, key: &str, fallback: u32) -> u32 {
  text
    .lines()
    .filter_map(|l| l.split_once(char::is_whitespace))
    .find(|(k, _)| *k == key)
    .and_then(|(_, v)| v.trim().parse().ok())
    .unwrap_or(fallback)
}

pub fn parse_users(passwd: &str, defs: &str) -> Vec<User> {
  let (min, max) = (login_defs(defs, "UID_MIN", 1000), login_defs(defs, "UID_MAX", 60000));
  let mut found: Vec<(u32, User)> = passwd
    .lines()
    .filter_map(|line| {
      let f: Vec<&str> = line.split(':').collect();
      if f.len() < 7 {
        return None;
      }
      let uid: u32 = f[2].parse().ok()?;
      let shell = f[6].trim();
      if uid < min || uid > max || shell.ends_with("nologin") || shell.ends_with("/false") || shell.is_empty() {
        return None;
      }
      let real = f[4].split(',').next().unwrap_or("").trim().to_string();
      let face = Path::new("/var/lib/AccountsService/icons").join(f[0]);
      Some((
        uid,
        User { name: f[0].to_string(), real, shell: shell.to_string(), face: face.is_file().then_some(face) },
      ))
    })
    .collect();
  found.sort_by_key(|(uid, _)| *uid);
  found.into_iter().map(|(_, u)| u).collect()
}

pub fn users() -> Vec<User> {
  parse_users(
    &std::fs::read_to_string("/etc/passwd").unwrap_or_default(),
    &std::fs::read_to_string("/etc/login.defs").unwrap_or_default(),
  )
}

// An account typed in by hand ("Outro usuário"): its shell, if it has one here.
pub fn shell_of(name: &str) -> String {
  std::fs::read_to_string("/etc/passwd")
    .unwrap_or_default()
    .lines()
    .map(|l| l.split(':').collect::<Vec<_>>())
    .find(|f| f.len() >= 7 && f[0] == name)
    .map(|f| f[6].trim().to_string())
    .filter(|s| !s.is_empty())
    .unwrap_or_else(|| "/bin/bash".into())
}

#[derive(Clone, Debug, PartialEq)]
pub struct Session {
  pub path: PathBuf,
  pub name: String,
  pub exec: String,
  pub desktops: Vec<String>,
}

fn in_path(program: &str) -> bool {
  if program.contains('/') {
    return Path::new(program).is_file();
  }
  std::env::var("PATH").unwrap_or_else(|_| "/usr/local/bin:/usr/bin:/bin".into()).split(':').any(|d| Path::new(d).join(program).is_file())
}

pub fn parse_session(path: &Path, text: &str) -> Option<Session> {
  let mut in_entry = false;
  let (mut name, mut localized, mut exec, mut try_exec, mut desktops, mut hidden) =
    (None, None, None, None, vec![], false);
  for line in text.lines().map(str::trim) {
    if line.starts_with('[') {
      in_entry = line == "[Desktop Entry]";
      continue;
    }
    let Some((key, value)) = line.split_once('=') else { continue };
    if !in_entry {
      continue;
    }
    let value = value.trim().to_string();
    match key.trim() {
      "Name" => name = Some(value),
      "Name[pt_BR]" | "Name[pt]" if localized.is_none() => localized = Some(value),
      "Exec" => exec = Some(value),
      "TryExec" => try_exec = Some(value),
      "DesktopNames" => desktops = value.split(';').filter(|s| !s.is_empty()).map(String::from).collect(),
      "Hidden" | "NoDisplay" if value == "true" => hidden = true,
      _ => {}
    }
  }
  let exec = exec?;
  if hidden || try_exec.is_some_and(|t| !in_path(&t)) {
    return None;
  }
  // Field codes (%f, %U...) mean nothing for a session.
  let exec = exec.split_whitespace().filter(|w| !(w.len() == 2 && w.starts_with('%'))).collect::<Vec<_>>().join(" ");
  let name = localized.or(name).unwrap_or_else(|| path.file_stem().unwrap_or_default().to_string_lossy().into_owned());
  Some(Session { path: path.to_path_buf(), name, exec, desktops })
}

// The sessions by name, /usr/local first when both have the same file.
pub fn sessions() -> Vec<Session> {
  let mut seen = std::collections::HashSet::new();
  let mut list = vec![];
  for dir in SESSION_DIRS {
    let Ok(entries) = std::fs::read_dir(dir) else { continue };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
      if path.extension().is_none_or(|e| e != "desktop") || !seen.insert(path.file_name().map(|n| n.to_owned())) {
        continue;
      }
      if let Some(s) = std::fs::read_to_string(&path).ok().and_then(|t| parse_session(&path, &t)) {
        list.push(s);
      }
    }
  }
  list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
  list
}

// For sh: greetd joins the command's words with spaces and runs them with
// `sh -c` (after sourcing /etc/profile and ~/.profile).
fn quote(word: &str) -> String {
  format!("'{}'", word.replace('\'', "'\\''"))
}

impl Session {
  // What greetd runs: the session through the account's login shell, so its
  // profile (~/.bash_profile, ~/.zprofile) sets the environment too, as a
  // desktop login always did.
  pub fn command(&self, shell: &str) -> Vec<String> {
    [shell, "-l", "-c", &format!("exec {}", self.exec)].iter().map(|w| quote(w)).collect()
  }

  pub fn env(&self) -> Vec<String> {
    let mut env = vec!["XDG_SESSION_TYPE=wayland".to_string()];
    if let Some(first) = self.desktops.first() {
      env.push(format!("XDG_SESSION_DESKTOP={first}"));
      env.push(format!("XDG_CURRENT_DESKTOP={}", self.desktops.join(":")));
    }
    env
  }
}

#[derive(Default, Debug, PartialEq)]
pub struct State {
  pub user: String,
  pub session: String,
}

pub fn parse_state(text: &str) -> State {
  let mut state = State::default();
  for line in text.lines() {
    match line.split_once('=') {
      Some(("user", v)) => state.user = v.trim().to_string(),
      Some(("session", v)) => state.session = v.trim().to_string(),
      _ => {}
    }
  }
  state
}

pub fn load_state() -> State {
  parse_state(&std::fs::read_to_string(STATE).unwrap_or_default())
}

pub fn save_state(user: &str, session: &Path) {
  let text = format!("user={user}\nsession={}\n", session.display());
  let tmp = format!("{STATE}.new");
  if std::fs::write(&tmp, text).is_ok() {
    let _ = std::fs::rename(&tmp, STATE);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn accounts_people_only() {
    let passwd = "root:x:0:0:root:/root:/bin/bash\n\
      greetd:x:973:973:greetd daemon:/var/lib/greetd:/sbin/nologin\n\
      zeca:x:1001:1001::/home/zeca:/bin/zsh\n\
      ana:x:1000:1000:Ana Souza,,,:/home/ana:/bin/bash\n\
      gone:x:1002:1002::/home/gone:/usr/sbin/nologin\n\
      nobody:x:65534:65534:Kernel Overflow User:/:/sbin/nologin\n";
    let users = parse_users(passwd, "UID_MIN 1000\nUID_MAX 60000\n");
    let names: Vec<_> = users.iter().map(|u| (u.name.as_str(), u.shown())).collect();
    assert_eq!(names, [("ana", "Ana Souza"), ("zeca", "zeca")]);
    assert_eq!(users[1].shell, "/bin/zsh");
  }

  #[test]
  fn sessions_from_desktop_files() {
    let path = Path::new("/usr/share/wayland-sessions/omarchy.desktop");
    let s = parse_session(
      path,
      "[Desktop Entry]\nName=DoxIA\nName[pt_BR]=DoxIA (pt)\nExec=uwsm start -- /usr/bin/x %U\nDesktopNames=mango;wlroots\nType=Application\n[Desktop Action x]\nName=Other\n",
    )
    .unwrap();
    assert_eq!(s.name, "DoxIA (pt)");
    assert_eq!(s.exec, "uwsm start -- /usr/bin/x");
    assert_eq!(s.command("/bin/zsh").join(" "), "'/bin/zsh' '-l' '-c' 'exec uwsm start -- /usr/bin/x'");
    let odd = Session { exec: "echo 'oi'".into(), ..s.clone() };
    let line = format!("exec {}", odd.command("/bin/sh").join(" "));
    let out = std::process::Command::new("/bin/sh").args(["-c", &line]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "oi\n");
    assert_eq!(s.env(), ["XDG_SESSION_TYPE=wayland", "XDG_SESSION_DESKTOP=mango", "XDG_CURRENT_DESKTOP=mango:wlroots"]);
    assert!(parse_session(path, "[Desktop Entry]\nName=X\nExec=x\nNoDisplay=true\n").is_none());
    assert!(parse_session(path, "[Desktop Entry]\nName=X\nExec=x\nTryExec=/nonexistent/x\n").is_none());
  }

  #[test]
  fn state_round_trip() {
    let s = parse_state("user=ana\nsession=/usr/share/wayland-sessions/omarchy.desktop\n");
    assert_eq!(s, State { user: "ana".into(), session: DEFAULT_SESSION.into() });
  }
}
