// The Start menu's data, from fedora/doxia/plugins/doxia.iniciar/IniciarModel.js:
// the Programs folders by XDG category and the recent documents GTK and
// Nautilus keep in ~/.local/share/recently-used.xbel.

use crate::apps::App;
use gtk::glib;

// The first category that matches, in this order (Settings before System,
// which often come together; Internet before Escritório, so mail clients that
// also say Office, like Evolution, sit with Thunderbird); the rest go to Outros.
const CATEGORIES: [(&[&str], &str, &str); 11] = [
  (&["Game"], "Jogos", "applications-games"),
  (&["Development"], "Desenvolvimento", "applications-development"),
  (&["Graphics"], "Gráficos", "applications-graphics"),
  (&["AudioVideo", "Audio", "Video"], "Multimídia", "applications-multimedia"),
  (&["Network", "Email"], "Internet", "applications-internet"),
  (&["Office"], "Escritório", "applications-office"),
  (&["Education"], "Educação", "applications-education"),
  (&["Science"], "Ciência", "applications-science"),
  (&["Settings"], "Configurações", "preferences-system"),
  (&["System"], "Sistema", "applications-system"),
  (&["Utility"], "Acessórios", "applications-utilities"),
];
const OTHER: (&str, &str) = ("Outros", "applications-other");

pub struct Folder {
  pub label: &'static str,
  pub icon: &'static str,
  pub apps: Vec<App>,
}

fn category_of(app: &App) -> (&'static str, &'static str) {
  CATEGORIES
    .iter()
    .find(|(keys, _, _)| keys.iter().any(|k| app.categories.iter().any(|c| c == k)))
    .map(|(_, label, icon)| (*label, *icon))
    .unwrap_or(OTHER)
}

// Folders in alphabetical order (Outros last), none empty; the apps keep the
// order they came in.
pub fn folders(apps: &[App]) -> Vec<Folder> {
  let mut out: Vec<Folder> = Vec::new();
  for app in apps {
    let (label, icon) = category_of(app);
    match out.iter_mut().find(|f| f.label == label) {
      Some(f) => f.apps.push(app.clone()),
      None => out.push(Folder { label, icon, apps: vec![app.clone()] }),
    }
  }
  let key = |f: &Folder| (f.label == OTHER.0, collate(f.label));
  out.sort_by(|a, b| key(a).cmp(&key(b)));
  out
}

// Accents sort with their letter (Ciência before Configurações, Educação
// before Escritório), as pt-BR collation does.
fn collate(s: &str) -> String {
  s.to_lowercase()
    .chars()
    .map(|c| match c {
      'á' | 'à' | 'â' | 'ã' => 'a',
      'é' | 'ê' => 'e',
      'í' => 'i',
      'ó' | 'ô' | 'õ' => 'o',
      'ú' => 'u',
      'ç' => 'c',
      c => c,
    })
    .collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Document {
  pub href: String,
  pub name: String,
  pub icon: String,
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
  let key = format!("{name}=\"");
  let start = tag.find(&format!(" {key}")).map(|i| i + 1).or_else(|| tag.starts_with(&key).then_some(0))?;
  let rest = &tag[start + key.len()..];
  rest.find('"').map(|end| &rest[..end])
}

fn unescape(v: &str) -> String {
  v.replace("&quot;", "\"").replace("&apos;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

// Local files, newest first.
pub fn recent_documents(xml: &str, limit: usize) -> Vec<Document> {
  let mut out: Vec<(String, Document)> = Vec::new();
  let mut rest = xml;
  while let Some(at) = rest.find("<bookmark") {
    rest = &rest[at + "<bookmark".len()..];
    if !rest.starts_with(|c: char| c.is_whitespace() || c == '>') {
      continue;
    }
    let Some(tag_end) = rest.find('>') else { break };
    let attrs = &rest[..tag_end];
    let body_end = rest.find("</bookmark>").unwrap_or(rest.len());
    let body = &rest[tag_end.min(body_end)..body_end];
    let Some(href) = attr(attrs, "href").map(unescape).filter(|h| h.starts_with("file://")) else { continue };
    let stamp = attr(attrs, "modified").or_else(|| attr(attrs, "visited")).unwrap_or("").to_string();
    let mime = body
      .find("mime-type")
      .and_then(|i| attr(&body[i + "mime-type".len()..], "type"))
      .unwrap_or("");
    let raw = &href[href.rfind('/').map(|i| i + 1).unwrap_or(0)..];
    let name = glib::uri_unescape_string(raw, None::<&str>).map(|s| s.to_string()).unwrap_or_else(|| raw.to_string());
    let icon = if mime.is_empty() { "text-x-generic".into() } else { mime.replacen('/', "-", 1) };
    out.push((stamp, Document { href, name, icon }));
  }
  out.sort_by(|a, b| b.0.cmp(&a.0));
  out.into_iter().take(limit).map(|(_, d)| d).collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn app(name: &str, categories: &[&str]) -> App {
    App {
      id: name.to_lowercase(),
      name: name.into(),
      generic: String::new(),
      comment: String::new(),
      keywords: Vec::new(),
      categories: categories.iter().map(|c| c.to_string()).collect(),
      icon: None,
    }
  }

  #[test]
  fn groups_by_first_matching_category() {
    let apps = [
      app("Steam", &["Game"]),
      app("Settings", &["Settings", "System"]),
      app("Misc", &[]),
      app("Code", &["Development", "Utility"]),
      app("Mpv", &["AudioVideo"]),
      app("Evolution", &["GNOME", "GTK", "Office", "Email", "Calendar"]),
    ];
    let f = folders(&apps);
    let labels: Vec<&str> = f.iter().map(|f| f.label).collect();
    assert_eq!(labels, ["Configurações", "Desenvolvimento", "Internet", "Jogos", "Multimídia", "Outros"]);
    assert_eq!(f[0].apps[0].name, "Settings");
  }

  #[test]
  fn reads_recent_files_newest_first() {
    let xml = r#"<xbel>
      <bookmark href="file:///home/a/Relat%C3%B3rio.pdf" added="x" modified="2026-10-01T10:00:00Z">
        <info><metadata><mime:mime-type type="application/pdf"/></metadata></info></bookmark>
      <bookmark href="https://example.com" modified="2026-10-09T10:00:00Z"></bookmark>
      <bookmark href="file:///home/a/a&amp;b.txt" visited="2026-10-05T10:00:00Z"><mime:mime-type type="text/plain"/></bookmark>
    </xbel>"#;
    let docs = recent_documents(xml, 15);
    assert_eq!(docs.len(), 2);
    assert_eq!((docs[0].name.as_str(), docs[0].icon.as_str()), ("a&b.txt", "text-plain"));
    assert_eq!((docs[1].name.as_str(), docs[1].icon.as_str()), ("Relatório.pdf", "application-pdf"));
  }
}
