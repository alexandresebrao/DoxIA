// The desktop applications, as shell/services/AppLibrary.qml serves them to
// the menus: what GIO lists and would show here (NoDisplay, Hidden,
// OnlyShowIn and NotShowIn against XDG_CURRENT_DESKTOP), less the ids in
// default/omarchy/launcher.hides, searched like AppSearch.js. Launches go
// through gtk-launch in a uwsm scope, as the shell's do.

use gtk::gio;
use gtk::prelude::*;
use std::collections::HashSet;

#[derive(Clone)]
pub struct App {
  // The desktop id without ".desktop".
  pub id: String,
  pub name: String,
  pub generic: String,
  pub comment: String,
  pub keywords: Vec<String>,
  pub categories: Vec<String>,
  pub icon: Option<gio::Icon>,
}

fn strip_desktop(id: &str) -> &str {
  id.strip_suffix(".desktop").unwrap_or(id)
}

pub fn hidden_ids() -> HashSet<String> {
  let path = format!("{}/default/omarchy/launcher.hides", std::env::var("OMARCHY_PATH").unwrap_or_default());
  std::fs::read_to_string(path)
    .unwrap_or_default()
    .lines()
    .map(|l| strip_desktop(l.trim()).to_string())
    .filter(|l| !l.is_empty())
    .collect()
}

pub fn all() -> Vec<App> {
  let hidden = hidden_ids();
  let mut seen = HashSet::new();
  let mut apps = Vec::new();
  for info in gio::AppInfo::all() {
    if !info.should_show() {
      continue;
    }
    let Some(desktop) = info.downcast_ref::<gio_unix::DesktopAppInfo>() else { continue };
    let Some(id) = info.id() else { continue };
    let id = strip_desktop(&id).to_string();
    if hidden.contains(&id) || !seen.insert(id.clone()) {
      continue;
    }
    let name = info.name().to_string();
    if name.is_empty() {
      continue;
    }
    apps.push(App {
      id,
      name,
      generic: desktop.generic_name().map(|g| g.to_string()).unwrap_or_default(),
      comment: info.description().map(|d| d.to_string()).unwrap_or_default(),
      keywords: desktop.keywords().iter().map(|k| k.to_string()).collect(),
      categories: desktop.categories().map(|c| c.split(';').filter(|c| !c.is_empty()).map(String::from).collect()).unwrap_or_default(),
      icon: info.icon(),
    });
  }
  apps
}

fn search_text(a: &App) -> String {
  [a.name.as_str(), &a.generic, &a.comment, &a.keywords.join(" "), &a.id].join(" ").to_lowercase()
}

// camelCase and separators split into words, for the acronym.
fn words(v: &str) -> Vec<String> {
  let mut spaced = String::new();
  let mut prev: Option<char> = None;
  for c in v.chars() {
    if c.is_ascii_uppercase() && prev.is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit()) {
      spaced.push(' ');
    }
    spaced.push(if "._:/\\-".contains(c) { ' ' } else { c });
    prev = Some(c);
  }
  spaced.to_lowercase().split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).map(String::from).collect()
}

fn acronym(a: &App) -> String {
  words(&[a.name.as_str(), &a.generic, &a.keywords.join(" "), &a.id].join(" ")).iter().filter_map(|w| w.chars().next()).collect()
}

fn term_matches(a: &App, term: &str) -> bool {
  term.is_empty()
    || a.name.to_lowercase().contains(term)
    || a.id.to_lowercase().contains(term)
    || search_text(a).contains(term)
    || (term.chars().count() <= 5 && acronym(a).contains(term))
}

// AppSearch.fuzzyScore: -1 for no match, higher is better.
pub fn score(a: &App, query: &str) -> i64 {
  let q = query.trim().to_lowercase();
  if q.is_empty() {
    return 0;
  }
  if !q.split_whitespace().all(|t| term_matches(a, t)) {
    return -1;
  }
  let len = |s: &str| s.chars().count() as i64;
  let at = |hay: &str| hay.find(&q).map(|b| hay[..b].chars().count() as i64);
  let name = a.name.to_lowercase();
  let id = a.id.to_lowercase();
  match (at(&name), at(&id)) {
    (Some(0), _) => return 10000 - len(&name),
    (_, Some(0)) => return 9500 - len(&id),
    (Some(n), _) => return 8000 - n * 10 - len(&name),
    (_, Some(i)) => return 7600 - i * 10 - len(&id),
    _ => {}
  }
  if let Some(h) = at(&search_text(a)) {
    return 6000 - h;
  }
  let acr = acronym(a);
  match at(&acr) {
    Some(0) => 5000 - len(&acr),
    Some(i) => 4600 - i * 10 - len(&acr),
    None => 4000 - len(&name),
  }
}

// Matches best first (all of them by name without a query).
pub fn search(apps: &[App], query: &str) -> Vec<App> {
  let q = query.trim();
  let mut rows: Vec<(i64, String, &App)> =
    apps.iter().map(|a| (score(a, q), a.name.to_lowercase(), a)).filter(|(s, _, _)| *s >= 0).collect();
  rows.sort_by(|a, b| if q.is_empty() { a.1.cmp(&b.1) } else { b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)) });
  rows.into_iter().map(|(_, _, a)| a.clone()).collect()
}

pub fn launch(id: &str) {
  crate::run::argv(&["uwsm-app", "--", "gtk-launch", &format!("{id}.desktop")]);
}

#[cfg(test)]
mod tests {
  use super::*;

  fn app(id: &str, name: &str, generic: &str, keywords: &[&str]) -> App {
    App {
      id: id.into(),
      name: name.into(),
      generic: generic.into(),
      comment: String::new(),
      keywords: keywords.iter().map(|k| k.to_string()).collect(),
      categories: Vec::new(),
      icon: None,
    }
  }

  #[test]
  fn scores_like_app_search() {
    let zen = app("app.zen_browser.zen", "Zen Browser", "Web Browser", &[]);
    let code = app("code", "Visual Studio Code", "Text Editor", &["vscode"]);
    assert_eq!(score(&zen, "zen"), 10000 - 11);
    assert!(score(&code, "vsc") > 0);
    assert_eq!(score(&code, "zzz"), -1);
    let names: Vec<String> = search(&[code.clone(), zen.clone()], "").into_iter().map(|a| a.name).collect();
    assert_eq!(names, ["Visual Studio Code", "Zen Browser"]);
    assert_eq!(search(&[code, zen], "browser")[0].name, "Zen Browser");
    assert_eq!(words("org.gnome.TextEditor"), ["org", "gnome", "text", "editor"]);
  }
}
