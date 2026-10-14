// The clipboard history, as shell/plugins/clipboard/ClipboardHistory.js keeps
// it in ~/.local/state/omarchy/clipboard-history.json: newest first, one
// entry per text or image, and the rows the picker shows for a search.

use serde_json::{Value, json};

pub const LIMIT: usize = 300;
pub const ROWS: usize = 50;
// The picker searches and shows a prefix of a huge paste only; pasting reads
// the whole entry back from the file by index.
const DISPLAY_TEXT_LIMIT: usize = 8192;

#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
  Text(String),
  Image { path: String, mime: String, captured_at: Option<String> },
}

impl Entry {
  pub fn from_json(v: &Value) -> Option<Entry> {
    if let Value::String(s) = v {
      return (!s.trim().is_empty()).then(|| Entry::Text(s.clone()));
    }
    let kind = v.get("type").or_else(|| v.get("kind")).and_then(Value::as_str).unwrap_or("");
    match kind {
      "text" => {
        let text = v.get("text").and_then(Value::as_str).unwrap_or("");
        (!text.trim().is_empty()).then(|| Entry::Text(text.into()))
      }
      "image" => {
        let path = v.get("path").and_then(Value::as_str).unwrap_or("");
        (!path.is_empty()).then(|| Entry::Image {
          path: path.into(),
          mime: v.get("mime").and_then(Value::as_str).filter(|m| !m.is_empty()).unwrap_or("image/png").into(),
          captured_at: v.get("capturedAt").filter(|c| !c.is_null()).map(|c| c.as_str().map(String::from).unwrap_or_else(|| c.to_string())),
        })
      }
      _ => None,
    }
  }

  pub fn to_json(&self) -> Value {
    match self {
      Entry::Text(t) => json!({"type": "text", "text": t}),
      Entry::Image { path, mime, captured_at } => {
        let mut v = json!({"type": "image", "path": path, "mime": mime});
        if let Some(c) = captured_at {
          v["capturedAt"] = json!(c);
        }
        v
      }
    }
  }

  fn key(&self) -> String {
    match self {
      Entry::Text(t) => format!("text:{t}"),
      Entry::Image { path, .. } => format!("image:{path}"),
    }
  }
}

pub fn parse_history(raw: &str) -> Vec<Entry> {
  match serde_json::from_str::<Value>(raw) {
    Ok(Value::Array(items)) => items.iter().filter_map(Entry::from_json).collect(),
    _ => Vec::new(),
  }
}

pub fn to_json(history: &[Entry]) -> String {
  let items: Vec<Value> = history.iter().take(LIMIT).map(Entry::to_json).collect();
  serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".into()) + "\n"
}

// A line capture.sh printed.
pub fn parse_line(line: &str) -> Option<Entry> {
  serde_json::from_str::<Value>(line.trim()).ok().as_ref().and_then(Entry::from_json)
}

// The entry goes first; an equal one further down goes away.
pub fn add(history: &[Entry], entry: Entry, limit: usize) -> Vec<Entry> {
  if limit == 0 {
    return Vec::new();
  }
  let key = entry.key();
  std::iter::once(entry).chain(history.iter().filter(|e| e.key() != key).cloned()).take(limit).collect()
}

fn decode_file_uri(uri: &str) -> Option<String> {
  let rest = uri.trim().strip_prefix("file://")?;
  let path = rest.strip_prefix("localhost").unwrap_or(rest);
  if !path.starts_with('/') {
    return None;
  }
  Some(gtk::glib::uri_unescape_string(path, None::<&str>).map(|s| s.to_string()).unwrap_or_else(|| path.to_string()))
}

// file:// lines in a text entry (what a file manager copies).
fn file_paths(text: &str) -> Vec<String> {
  text.lines().filter_map(decode_file_uri).collect()
}

fn file_name(path: &str) -> &str {
  path.rsplit('/').next().unwrap_or(path)
}

fn is_image_path(path: &str) -> bool {
  let lower = path.to_lowercase();
  [".png", ".jpg", ".jpeg", ".webp", ".gif", ".bmp", ".tif", ".tiff"].iter().any(|e| lower.ends_with(e))
}

fn file_text(paths: &[String]) -> String {
  match paths.len() {
    0 => String::new(),
    1 => file_name(&paths[0]).into(),
    n => format!("{n} files"),
  }
}

fn capped(text: &str) -> &str {
  if text.len() <= DISPLAY_TEXT_LIMIT {
    return text;
  }
  let mut end = DISPLAY_TEXT_LIMIT;
  while !text.is_char_boundary(end) {
    end -= 1;
  }
  // On a line break, so a file:// URI never ends up cut into a bogus path.
  match text[..end].rfind('\n') {
    Some(cut) if cut > 0 => &text[..cut],
    _ => &text[..end],
  }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
  Text,
  File,
  Image,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
  pub kind: Kind,
  pub full_text: String,
  pub preview_text: String,
  pub preview_image: String,
  pub path: String,
  pub mime: String,
  // Where the entry sits in the history file.
  pub index: usize,
}

pub fn rows(history: &[Entry], query: &str, limit: usize) -> Vec<Row> {
  let needle = query.trim().to_lowercase();
  let mut out = Vec::new();
  for (index, entry) in history.iter().enumerate() {
    if out.len() >= limit {
      break;
    }
    let row = match entry {
      Entry::Image { path, mime, captured_at } => {
        let search = format!("image screenshot {mime} {}", captured_at.as_deref().unwrap_or(""));
        if !needle.is_empty() && !search.to_lowercase().contains(&needle) {
          continue;
        }
        let preview_text = match captured_at {
          Some(at) if !at.is_empty() => format!("{} from {at}", if mime == "image/png" { "Screenshot" } else { "Image" }),
          _ => "Image".into(),
        };
        Row {
          kind: Kind::Image,
          full_text: String::new(),
          preview_text,
          preview_image: path.clone(),
          path: path.clone(),
          mime: mime.clone(),
          index,
        }
      }
      Entry::Text(text) => {
        let text = capped(text);
        let paths = file_paths(text);
        let files = file_text(&paths);
        if !needle.is_empty() && !format!("{text} {files}").to_lowercase().contains(&needle) {
          continue;
        }
        let single = (paths.len() == 1).then(|| paths[0].clone()).unwrap_or_default();
        Row {
          kind: if paths.is_empty() { Kind::Text } else { Kind::File },
          full_text: if paths.is_empty() { text.to_string() } else { paths.join("\n") },
          preview_text: if files.is_empty() { text.split_whitespace().collect::<Vec<_>>().join(" ") } else { files },
          preview_image: if is_image_path(&single) { single.clone() } else { String::new() },
          path: single,
          mime: "text/plain".into(),
          index,
        }
      }
    };
    out.push(row);
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn adds_newest_first_without_duplicates() {
    let h = vec![Entry::Text("a".into()), Entry::Text("b".into())];
    let h = add(&h, Entry::Text("b".into()), 300);
    assert_eq!(h, vec![Entry::Text("b".into()), Entry::Text("a".into())]);
    assert_eq!(add(&h, Entry::Text("c".into()), 2).len(), 2);
    assert!(parse_line("{\"type\":\"text\",\"text\":\"   \"}").is_none());
    let img = parse_line(r#"{"type":"image","mime":"image/png","path":"/tmp/x.png","capturedAt":"Tuesday 18:00"}"#).unwrap();
    assert_eq!(parse_history(&to_json(&[img.clone()])), vec![img]);
  }

  #[test]
  fn rows_describe_text_files_and_images() {
    let h = vec![
      Entry::Text("hello\n  world".into()),
      Entry::Text("file:///home/a/Foto%20boa.png".into()),
      Entry::Text("file:///a/x\nfile:///a/y".into()),
      Entry::Image { path: "/s/1.png".into(), mime: "image/png".into(), captured_at: Some("Monday 10:00".into()) },
    ];
    let r = rows(&h, "", 50);
    assert_eq!(r[0].preview_text, "hello world");
    assert_eq!((r[1].kind.clone(), r[1].preview_text.as_str(), r[1].preview_image.as_str()), (Kind::File, "Foto boa.png", "/home/a/Foto boa.png"));
    assert_eq!(r[2].preview_text, "2 files");
    assert_eq!(r[3].preview_text, "Screenshot from Monday 10:00");
    assert_eq!(rows(&h, "screenshot", 50)[0].index, 3);
    assert_eq!(rows(&h, "boa", 50)[0].index, 1);
  }
}
