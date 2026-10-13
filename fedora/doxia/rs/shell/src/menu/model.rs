// The Omarchy menu's data: shell/plugins/menu/MenuModel.js in Rust. The
// JSONC files (default/omarchy/omarchy-menu.jsonc, then the user's
// ~/.config/omarchy/extensions/omarchy-menu.jsonc on top), routes, the
// `when:`/`checked:` guards batched into one bash script, search and the
// rows a menu shows.

use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
  Menu,
  Action,
  Link,
  App,
}

#[derive(Clone, Debug)]
pub struct Item {
  pub id: String,
  pub parent: String,
  pub kind: Kind,
  pub icon: String,
  pub icon_font: String,
  pub label: String,
  pub title: String,
  pub target: String,
  pub description: String,
  pub action: String,
  pub provider: String,
  pub aliases: Vec<String>,
  pub when: String,
  pub checked: String,
  pub order: usize,
  // The menu whose provider produced the row, which drops it on the next run.
  pub provider_menu: String,
  // App rows: the desktop id without ".desktop".
  pub app_id: String,
}

impl Item {
  pub fn new(id: &str, parent: &str, kind: Kind, label: &str) -> Item {
    Item {
      id: id.into(),
      parent: parent.into(),
      kind,
      icon: String::new(),
      icon_font: String::new(),
      label: label.into(),
      title: String::new(),
      target: String::new(),
      description: String::new(),
      action: String::new(),
      provider: String::new(),
      aliases: Vec::new(),
      when: String::new(),
      checked: String::new(),
      order: 0,
      provider_menu: String::new(),
      app_id: String::new(),
    }
  }

  fn opens(&self) -> bool {
    matches!(self.kind, Kind::Menu | Kind::Link)
  }
}

// Comment lines and trailing commas, as the JS does (comments only at the
// start of a line, so "https://..." inside a value survives).
pub fn strip_jsonc(raw: &str) -> String {
  let kept: Vec<&str> = raw.lines().filter(|l| !l.trim_start().starts_with("//")).collect();
  let text = kept.join("\n");
  let chars: Vec<char> = text.chars().collect();
  let mut out = String::with_capacity(text.len());
  for (i, &c) in chars.iter().enumerate() {
    if c == ',' {
      let next = chars[i + 1..].iter().find(|c| !c.is_whitespace());
      if matches!(next, Some('}') | Some(']')) {
        continue;
      }
    }
    out.push(c);
  }
  out
}

fn text(v: &Value, key: &str) -> String {
  match v.get(key) {
    Some(Value::String(s)) => s.clone(),
    _ => String::new(),
  }
}

// Fields a JSONC entry sets, for merging an override over a default.
#[derive(Clone, Debug, Default)]
pub struct Entry {
  pub id: String,
  fields: serde_json::Map<String, Value>,
}

pub fn parse(raw: &str) -> Vec<Entry> {
  let stripped = strip_jsonc(raw);
  let Ok(parsed) = serde_json::from_str::<Value>(&stripped) else { return Vec::new() };
  let source = match parsed.get("items") {
    Some(Value::Object(items)) => items.clone(),
    _ => match parsed {
      Value::Object(m) => m,
      _ => return Vec::new(),
    },
  };
  source
    .into_iter()
    .filter_map(|(id, v)| match v {
      Value::Object(fields) => Some(Entry { id, fields }),
      _ => None,
    })
    .collect()
}

fn normalize(id: &str, v: &Value) -> Item {
  let parent = match v.get("parent") {
    Some(Value::String(p)) => p.clone(),
    _ if id.contains('.') => id.rsplit_once('.').unwrap().0.to_string(),
    _ => "root".into(),
  };
  let action = text(v, "action");
  let target = text(v, "target");
  let kind = if !action.is_empty() {
    Kind::Action
  } else if !target.is_empty() {
    Kind::Link
  } else {
    Kind::Menu
  };
  let aliases = match v.get("aliases") {
    Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str()).filter(|s| !s.is_empty()).map(String::from).collect(),
    Some(Value::String(s)) if !s.is_empty() => vec![s.clone()],
    _ => Vec::new(),
  };
  let label = text(v, "label");
  Item {
    parent: if id == "root" { String::new() } else { parent },
    kind,
    icon: text(v, "icon"),
    icon_font: text(v, "iconFont"),
    label: if label.is_empty() { id.into() } else { label },
    title: text(v, "title"),
    target,
    description: text(v, "description"),
    action,
    provider: text(v, "provider"),
    aliases,
    when: text(v, "when"),
    checked: text(v, "checked"),
    ..Item::new(id, "", kind, "")
  }
}

#[derive(Clone, Debug, Default)]
pub struct Menu {
  pub items: HashMap<String, Item>,
  pub order: Vec<String>,
}

// What the guards answered: `when:` false hides, `checked:` true marks.
#[derive(Clone, Debug, Default)]
pub struct Guards {
  pub when: HashMap<String, bool>,
  pub checked: HashMap<String, bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
  pub id: String,
  pub kind: Kind,
  pub icon: String,
  pub icon_font: String,
  pub label: String,
  pub target: String,
  pub detail: String,
  pub path: String,
  pub action: String,
  pub app_id: String,
  pub score: i64,
  // The first row of the matches below the current menu, under a divider.
  pub divider: bool,
}

// The user file's keys go over the default's: a partial entry changes only
// what it names, and the row keeps its place.
pub fn merge(defaults: &[Entry], user: &[Entry]) -> Menu {
  let mut fields: HashMap<String, serde_json::Map<String, Value>> = HashMap::new();
  let mut order = Vec::new();
  for e in defaults.iter().chain(user) {
    let slot = fields.entry(e.id.clone()).or_insert_with(|| {
      order.push(e.id.clone());
      Default::default()
    });
    for (k, v) in &e.fields {
      slot.insert(k.clone(), v.clone());
    }
  }
  let mut items: HashMap<String, Item> =
    order.iter().map(|id| (id.clone(), normalize(id, &Value::Object(fields[id].clone())))).collect();
  if !items.contains_key("root") {
    let mut root = Item::new("root", "", Kind::Menu, "Ir");
    root.parent = String::new();
    items.insert("root".into(), root);
    order.insert(0, "root".into());
  }
  for (i, id) in order.iter().enumerate() {
    items.get_mut(id).unwrap().order = i;
  }
  Menu { items, order }
}

fn searchable_token(v: &str) -> String {
  v.replace(['.', '_', '-'], " ")
}

fn leaf(id: &str) -> &str {
  id.rsplit('.').next().unwrap_or(id)
}

fn name_search_text(e: &Item) -> String {
  let aliases: Vec<String> = e.aliases.iter().map(|a| searchable_token(a)).collect();
  [e.label.clone(), searchable_token(leaf(&e.id)), aliases.join(" ")].join(" ").to_lowercase()
}

fn term_in_words(term: &str, text: &str) -> bool {
  text.to_lowercase().split_whitespace().any(|w| w == term)
}

fn description_matches(query: &str, text: &str) -> bool {
  query.to_lowercase().split_whitespace().all(|t| term_in_words(t, text))
}

pub fn slugify(v: &str) -> String {
  let mut out = String::new();
  for c in v.to_lowercase().chars() {
    if c.is_ascii_lowercase() || c.is_ascii_digit() {
      out.push(c);
    } else if !out.ends_with('-') {
      out.push('-');
    }
  }
  let s = out.trim_matches('-');
  if s.is_empty() { "item".into() } else { s.into() }
}

impl Menu {
  pub fn item(&self, id: &str) -> Option<&Item> {
    self.items.get(id)
  }

  // An exact id, else a declared alias (never an app row's: those carry
  // .desktop keywords), else the input itself.
  pub fn resolve_route(&self, input: &str) -> String {
    let raw = input.to_lowercase().replace('_', "-");
    if raw.is_empty() || raw == "go" || raw == "menu" {
      return "root".into();
    }
    if self.items.contains_key(&raw) {
      return raw;
    }
    for id in &self.order {
      let Some(e) = self.item(id) else { continue };
      if e.kind == Kind::App {
        continue;
      }
      if e.aliases.iter().any(|a| a.to_lowercase().replace('_', "-") == raw) {
        return e.id.clone();
      }
    }
    raw
  }

  pub fn depth(&self, id: &str) -> usize {
    let mut depth = 0;
    let mut cur = self.item(id);
    while let Some(e) = cur {
      if e.parent.is_empty() || e.parent == "root" || depth >= 32 {
        break;
      }
      depth += 1;
      cur = self.item(&e.parent);
    }
    depth
  }

  pub fn path(&self, id: &str) -> String {
    let mut labels = Vec::new();
    let mut cur = self.item(id);
    while let Some(e) = cur {
      if e.id == "root" || labels.len() >= 32 {
        break;
      }
      labels.insert(0, e.label.clone());
      cur = self.item(&e.parent);
    }
    labels.join(" › ")
  }

  pub fn parent_path(&self, id: &str) -> String {
    match self.item(id) {
      Some(e) if !e.parent.is_empty() && e.parent != "root" => self.path(&e.parent),
      _ => String::new(),
    }
  }

  pub fn is_descendant(&self, id: &str, ancestor: &str) -> bool {
    if ancestor == "root" {
      return id != "root";
    }
    let mut cur = self.item(id);
    let mut guard = 0;
    while let Some(e) = cur {
      if e.parent.is_empty() || guard >= 32 {
        break;
      }
      if e.parent == ancestor {
        return true;
      }
      cur = self.item(&e.parent);
      guard += 1;
    }
    false
  }

  // Guarded rows hide on an explicit false; a static submenu hides when none
  // of its rows show; provider menus always show (their rows load later).
  pub fn is_visible(&self, guards: &Guards, e: &Item, depth: usize) -> bool {
    if !e.when.is_empty() && guards.when.get(&e.id) == Some(&false) {
      return false;
    }
    if !e.opens() || !e.provider.is_empty() {
      return true;
    }
    if depth >= 32 {
      return false;
    }
    let target = if e.kind == Kind::Link { &e.target } else { &e.id };
    self.order.iter().filter_map(|id| self.item(id)).any(|c| &c.parent == target && self.is_visible(guards, c, depth + 1))
  }

  fn label_for(&self, guards: &Guards, e: &Item) -> String {
    if !e.checked.is_empty() && guards.checked.get(&e.id) == Some(&true) {
      format!("{} ✓", e.label)
    } else {
      e.label.clone()
    }
  }

  fn matches_query(&self, e: &Item, query: &str, visible: bool) -> bool {
    if e.id == "root" || !visible {
      return false;
    }
    let name = name_search_text(e);
    let desc = e.description.to_lowercase();
    query.to_lowercase().split_whitespace().all(|t| name.contains(t) || term_in_words(t, &desc))
  }

  pub fn search_score(&self, e: &Item, query: &str) -> i64 {
    let needle = query.trim().to_lowercase();
    let label = e.label.to_lowercase();
    let name = name_search_text(e);
    let desc = e.description.to_lowercase();
    let mut score: i64 = if label == needle {
      if e.parent == "root" { 2 } else { 0 }
    } else if e.kind == Kind::App && label.split_whitespace().any(|w| w == needle) {
      // An installed app named by the query ("zen") beats Install > Zen.
      0
    } else if label.starts_with(&needle) {
      10
    } else if label.contains(&needle) {
      30
    } else if name.contains(&needle) {
      40
    } else if description_matches(&needle, &desc) {
      60
    } else {
      80
    };
    if e.opens() {
      score -= 2;
    }
    if e.kind == Kind::App {
      score -= 5;
    }
    score * 1000 + self.depth(&e.id) as i64 * 25 + e.order as i64
  }

  fn row(&self, guards: &Guards, e: &Item, detail: String, score: i64) -> Row {
    Row {
      id: e.id.clone(),
      kind: e.kind,
      icon: e.icon.clone(),
      icon_font: e.icon_font.clone(),
      label: self.label_for(guards, e),
      target: if e.kind == Kind::Link { e.target.clone() } else { e.id.clone() },
      detail,
      path: self.path(&e.id),
      action: e.action.clone(),
      app_id: e.app_id.clone(),
      score,
      divider: false,
    }
  }

  // What `active` shows: its visible rows, or with a query the matches below
  // it, its own rows first and the deeper ones after a divider.
  pub fn rows(&self, guards: &Guards, active: &str, query: &str) -> Vec<Row> {
    let query = query.trim();
    if query.is_empty() {
      let mut rows: Vec<Row> = self
        .order
        .iter()
        .filter_map(|id| self.item(id))
        .filter(|c| c.parent == active && self.is_visible(guards, c, 0))
        .map(|c| self.row(guards, c, c.description.clone(), c.order as i64))
        .collect();
      // Alphabetical whatever order the desktop entries came in.
      if active == "apps" {
        rows.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()).then_with(|| a.id.cmp(&b.id)));
      }
      return rows;
    }
    let mut current = Vec::new();
    let mut deeper = Vec::new();
    for id in &self.order {
      let Some(e) = self.item(id) else { continue };
      if e.id == "root" || !self.is_descendant(&e.id, active) {
        continue;
      }
      if !self.matches_query(e, query, self.is_visible(guards, e, 0)) {
        continue;
      }
      let row = self.row(guards, e, self.parent_path(&e.id), self.search_score(e, query));
      if e.parent == active { current.push(row) } else { deeper.push(row) }
    }
    let sort = |a: &Row, b: &Row| a.score.cmp(&b.score).then_with(|| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
    current.sort_by(sort);
    deeper.sort_by(sort);
    if !current.is_empty() {
      if let Some(first) = deeper.first_mut() {
        first.divider = true;
      }
    }
    current.extend(deeper);
    current
  }

  // Swaps the rows a provider produced for `menu_id` (the apps included) for
  // a new batch, leaving every other item where it was.
  pub fn swap_rows(&mut self, menu_id: &str, rows: Vec<Item>) {
    self.order.retain(|id| self.items.get(id).is_some_and(|e| e.provider_menu != menu_id));
    self.items.retain(|_, e| e.provider_menu != menu_id);
    for mut row in rows {
      if row.id.is_empty() || self.items.contains_key(&row.id) {
        continue;
      }
      row.provider_menu = menu_id.into();
      row.order = self.order.len();
      self.order.push(row.id.clone());
      self.items.insert(row.id.clone(), row);
    }
  }
}

// Rows from a bash provider: `label<TAB>value<TAB>current` per line.
pub fn provider_rows(output: &str, menu_id: &str, icon: &str, action: impl Fn(&str) -> String) -> Vec<Item> {
  let mut taken = std::collections::HashSet::new();
  let mut rows = Vec::new();
  for line in output.lines() {
    let line = line.trim();
    if line.is_empty() {
      continue;
    }
    let parts: Vec<&str> = line.split('\t').collect();
    let label = parts[0];
    if label.is_empty() {
      continue;
    }
    let value = parts.get(1).filter(|v| !v.is_empty()).copied().unwrap_or(label);
    let current = parts.get(2).copied().unwrap_or("");
    // Fira Code and Fira-Code slugify alike; a repeated id would be dropped.
    let mut id = format!("{menu_id}.{}", slugify(value));
    while !taken.insert(id.clone()) {
      id.push('-');
    }
    let mut row = Item::new(&id, menu_id, Kind::Action, label);
    row.icon = if value == current { "✓".into() } else { icon.into() };
    row.action = action(value);
    rows.push(row);
  }
  rows
}

// --- Guards ------------------------------------------------------------------

// Readers a `checked:` asks again and again (Defaults > Browser has seven
// rows on omarchy-default-browser): captured once, up front, since a value
// cached inside one $(...) never reaches the next.
const GUARD_READERS: [&str; 6] = [
  "omarchy-channel-current",
  "omarchy-default-agent",
  "omarchy-default-browser",
  "omarchy-default-editor",
  "omarchy-default-terminal",
  "omarchy-dns",
];

fn reader_slot(i: usize) -> String {
  format!("${{__omarchy_read_{i}}}")
}

fn substitute_readers(expression: &str) -> String {
  let mut out = expression.to_string();
  for (i, r) in GUARD_READERS.iter().enumerate() {
    out = out.replace(&format!("$({r})"), &reader_slot(i));
  }
  out
}

fn is_package_name(w: &str) -> bool {
  let mut chars = w.chars();
  chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
    && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'))
}

// Every literal name after omarchy-pkg-present/-missing, in order, for one
// rpm query.
fn package_names(guards: &str) -> Vec<String> {
  let stop = |c: char| c.is_whitespace() || matches!(c, ';' | '|' | '&' | ')' | '}');
  let mut names: Vec<String> = Vec::new();
  let mut rest = guards;
  while let Some(at) = rest.find("omarchy-pkg-") {
    rest = &rest[at + "omarchy-pkg-".len()..];
    let Some(mut tail) = rest.strip_prefix("present").or_else(|| rest.strip_prefix("missing")) else { continue };
    loop {
      let spaced = tail.trim_start_matches([' ', '\t']);
      let end = spaced.find(stop).unwrap_or(spaced.len());
      if spaced.len() == tail.len() || end == 0 {
        break;
      }
      let word = &spaced[..end];
      if is_package_name(word) && !names.iter().any(|n| n == word) {
        names.push(word.to_string());
      }
      tail = &spaced[end..];
    }
  }
  names
}

fn guard_helpers(guards: &str) -> String {
  let names = package_names(guards);
  let mut helpers = String::from("declare -A __omarchy_pkgs=()\n");
  if !names.is_empty() {
    helpers += &format!("__omarchy_pkg_names=({})\n", names.join(" "));
    helpers += concat!(
      "__omarchy_pkg_out=$(LC_ALL=C rpm -q --whatprovides --qf '%{NAME}\\n' \"${__omarchy_pkg_names[@]}\" 2>&1)\n",
      "__omarchy_pkg_ok=0; [[ -n $__omarchy_pkg_out ]] && __omarchy_pkg_ok=1\n",
      "while IFS= read -r __omarchy_pkg_line; do case $__omarchy_pkg_line in\n",
      "  \"no package provides \"*) __omarchy_pkgs[${__omarchy_pkg_line#no package provides }]=0 ;;\n",
      "  *[[:space:]]*) __omarchy_pkg_ok=0 ;;\n",
      "esac; done <<<\"$__omarchy_pkg_out\"\n",
      "if (( __omarchy_pkg_ok )); then\n",
      "  for __omarchy_pkg in \"${__omarchy_pkg_names[@]}\"; do : \"${__omarchy_pkgs[$__omarchy_pkg]:=1}\"; done\n",
      "else\n",
      "  __omarchy_pkgs=()\n",
      "fi\n",
    );
  }
  helpers
    + concat!(
      "__omarchy_pkg_has() { [[ -n $1 ]] && case ${__omarchy_pkgs[$1]-} in 1) return 0 ;; 0) return 1 ;; esac; ",
      "rpm -q --whatprovides \"$1\" &>/dev/null; }\n",
      "omarchy-pkg-present() { local p; for p in \"$@\"; do __omarchy_pkg_has \"$p\" || return 1; done; return 0; }\n",
      "omarchy-pkg-missing() { local p; for p in \"$@\"; do __omarchy_pkg_has \"$p\" || return 0; done; return 1; }\n",
      "omarchy-cmd-present() { local c; for c in \"$@\"; do command -v \"$c\" &>/dev/null || return 1; done; return 0; }\n",
      "omarchy-cmd-missing() { local c; for c in \"$@\"; do command -v \"$c\" &>/dev/null || return 0; done; return 1; }\n",
    )
}

fn guard_line(id: &str, tag: char, expression: &str) -> String {
  format!(
    "if {{ {}; }} >/dev/null 2>&1; then echo {id}:{tag}:1; else echo {id}:{tag}:0; fi\n",
    substitute_readers(expression)
  )
}

// One bash script for every `when:` and `checked:`, answering
// `<id>:<w|c>:<0|1>` per line; empty when nothing is guarded.
pub fn guard_script(menu: &Menu) -> String {
  let mut guards = String::new();
  for id in &menu.order {
    let e = &menu.items[id];
    if !e.when.is_empty() {
      guards += &guard_line(id, 'w', &e.when);
    }
    if !e.checked.is_empty() {
      guards += &guard_line(id, 'c', &e.checked);
    }
  }
  if guards.is_empty() {
    return guards;
  }
  let mut prelude = guard_helpers(&guards);
  for (i, r) in GUARD_READERS.iter().enumerate() {
    if guards.contains(&reader_slot(i)) {
      // `|| :` so a failing reader can't stop an errexit login shell.
      prelude += &format!("__omarchy_read_{i}=$({r} 2>/dev/null) || :\n");
    }
  }
  prelude + &guards
}

pub fn parse_guards(output: &str) -> Guards {
  let mut g = Guards::default();
  for line in output.lines() {
    let Some((rest, value)) = line.trim().rsplit_once(':') else { continue };
    let Some((id, tag)) = rest.rsplit_once(':') else { continue };
    let map = match tag {
      "w" => &mut g.when,
      "c" => &mut g.checked,
      _ => continue,
    };
    map.insert(id.to_string(), value == "1");
  }
  g
}

#[cfg(test)]
mod tests {
  use super::*;

  fn menu(raw: &str) -> Menu {
    merge(&parse(raw), &[])
  }

  #[test]
  fn parses_jsonc_and_merges_overrides_in_place() {
    let defaults = parse(
      r#"{
        // comment
        "style": {"icon":"x","label":"Style"},
        "style.theme": {"label":"Theme","action":"omarchy-theme-switcher","url":"https://a//b"},
        "about": {"label":"About","action":"a"},
      }"#,
    );
    assert_eq!(defaults.len(), 3);
    let user = parse(r#"{"style.theme": {"label":"Theme picker"}, "extra": {"label":"Extra"}}"#);
    let m = merge(&defaults, &user);
    assert_eq!(m.order, ["root", "style", "style.theme", "about", "extra"]);
    let theme = m.item("style.theme").unwrap();
    assert_eq!((theme.label.as_str(), theme.action.as_str(), theme.order), ("Theme picker", "omarchy-theme-switcher", 2));
    assert_eq!(theme.parent, "style");
    assert_eq!(m.item("root").unwrap().label, "Ir");
    assert_eq!(m.path("style.theme"), "Style › Theme picker");
    assert_eq!(m.parent_path("style.theme"), "Style");
    assert!(m.is_descendant("style.theme", "style"));
    assert_eq!(slugify("Power Saver!"), "power-saver");
  }

  #[test]
  fn visibility_follows_guards_and_children() {
    let m = menu(
      r#"{"hardware": {"label":"H"}, "hardware.laptop": {"label":"L","when":"x","action":"a"},
          "nested": {"label":"N"}, "nested.branch": {"label":"B"}, "nested.branch.leaf": {"label":"F","when":"y","action":"a"},
          "dynamic": {"label":"D","provider":"fonts"}}"#,
    );
    let off = Guards { when: [("hardware.laptop".into(), false), ("nested.branch.leaf".into(), false)].into(), ..Default::default() };
    assert!(!m.is_visible(&off, m.item("hardware").unwrap(), 0));
    assert!(!m.is_visible(&off, m.item("nested").unwrap(), 0));
    assert!(m.is_visible(&Guards::default(), m.item("hardware").unwrap(), 0));
    assert!(m.is_visible(&off, m.item("dynamic").unwrap(), 0));
  }

  #[test]
  fn routes_ids_then_aliases_never_apps() {
    let mut m = menu(r#"{"system": {"label":"Sistema","aliases":["power-menu"]}, "system.lock": {"label":"L","action":"a"}}"#);
    let mut app = Item::new("apps.htop", "apps", Kind::App, "htop");
    app.aliases = vec!["system".into(), "process".into()];
    m.swap_rows("apps", vec![app]);
    assert_eq!(m.resolve_route("system"), "system");
    assert_eq!(m.resolve_route("process"), "process");
    assert_eq!(m.resolve_route("power_menu"), "system");
    assert_eq!(m.resolve_route(""), "root");
    assert_eq!(m.resolve_route("no-such-route"), "no-such-route");
  }

  #[test]
  fn search_ranks_and_splits_current_from_deeper() {
    let m = menu(
      r#"{"style": {"label":"Style"}, "style.theme": {"label":"Theme","aliases":["colors"],"description":"Pick appearance","action":"a"},
          "theme": {"label":"Theme menu","action":"b"}}"#,
    );
    let g = Guards::default();
    let theme = m.item("style.theme").unwrap();
    assert!(m.search_score(theme, "theme") < m.search_score(theme, "appearance"));
    let rows = m.rows(&g, "root", "theme");
    assert_eq!(rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["theme", "style.theme"]);
    assert!(rows[1].divider && rows[1].detail == "Style");
    assert_eq!(m.rows(&g, "root", "colors")[0].id, "style.theme");
    assert!(m.rows(&g, "root", "missing").is_empty());
  }

  #[test]
  fn providers_swap_their_own_rows() {
    let mut m = menu(r#"{"style.font": {"label":"Fonte","provider":"fonts"}, "style.font.keep": {"label":"K","action":"a"}}"#);
    let rows = provider_rows("Fira Code\tFira Code\tFira Code\nFira-Code\tFira-Code\tx\n", "style.font", "", |v| format!("set {v}"));
    assert_eq!(rows[0].icon, "✓");
    assert_eq!((rows[0].id.as_str(), rows[1].id.as_str()), ("style.font.fira-code", "style.font.fira-code-"));
    m.swap_rows("style.font", rows);
    m.swap_rows("style.font", provider_rows("Mono\n", "style.font", "", |v| v.into()));
    let ids: Vec<String> = m.rows(&Guards::default(), "style.font", "").into_iter().map(|r| r.id).collect();
    assert_eq!(ids, ["style.font.keep", "style.font.mono"]);
  }

  #[test]
  fn guard_script_batches_and_parses() {
    let m = menu(
      r#"{"a": {"label":"A","when":"omarchy-pkg-present vim tmux; true","action":"x"},
          "b": {"label":"B","checked":"[[ $(omarchy-default-browser) == x ]]","action":"y"}}"#,
    );
    let s = guard_script(&m);
    assert!(s.contains("__omarchy_pkg_names=(vim tmux)"));
    assert!(s.contains("__omarchy_read_2=$(omarchy-default-browser 2>/dev/null) || :"));
    assert!(s.contains("if { [[ ${__omarchy_read_2} == x ]]; } >/dev/null 2>&1; then echo b:c:1; else echo b:c:0; fi"));
    let out = std::process::Command::new("bash").args(["-c", &s]).output().unwrap();
    let g = parse_guards(&String::from_utf8_lossy(&out.stdout));
    assert!(g.when.contains_key("a") && g.checked.contains_key("b"));
    let g = parse_guards("x.y:w:0\nz:c:1\n");
    assert_eq!((g.when["x.y"], g.checked["z"]), (false, true));
  }
}
