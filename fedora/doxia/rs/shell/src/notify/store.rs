// The notification files under ~/.local/state/omarchy, shared with the
// Quickshell service: notifications.json (the do-not-disturb preference),
// notifications/<stem>.json (one per popup on screen, so popups survive a
// restart), notifications/history/ (the last ten that left the screen) and
// notifications/images/ (copies of the images those entries show, which their
// senders delete on close). Small files, written synchronously.

use super::model::{self, Entry, HISTORY_LIMIT};
use std::fs;
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

const MAX_IMAGE: u64 = 5 * 1024 * 1024;

pub struct Store {
  state: PathBuf,
  popups: PathBuf,
  history: PathBuf,
  images: PathBuf,
}

fn write_atomic(path: &Path, text: &str) {
  let tmp = path.with_extension("tmp");
  if fs::write(&tmp, text).is_ok() {
    let _ = fs::rename(&tmp, path);
  }
}

impl Store {
  pub fn new() -> Store {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let state = home.join(".local/state/omarchy");
    let popups = state.join("notifications");
    let store = Store { history: popups.join("history"), images: popups.join("images"), popups, state };
    for dir in [&store.state, &store.popups, &store.history, &store.images] {
      let _ = fs::create_dir_all(dir);
    }
    store
  }

  pub fn images_dir(&self) -> &Path {
    &self.images
  }

  // ---- do-not-disturb -------------------------------------------------------

  pub fn load_dnd(&self) -> bool {
    fs::read_to_string(self.state.join("notifications.json"))
      .ok()
      .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
      .and_then(|v| v.get("dnd").and_then(serde_json::Value::as_bool))
      .unwrap_or(false)
  }

  pub fn save_dnd(&self, dnd: bool) {
    let text = serde_json::to_string_pretty(&serde_json::json!({"version": 3, "dnd": dnd})).unwrap_or_default();
    write_atomic(&self.state.join("notifications.json"), &(text + "\n"));
  }

  // ---- images ---------------------------------------------------------------

  // A bounded copy of a sender's file: regular files only (opened without
  // blocking, so a FIFO can't hang the shell), up to 5 MB.
  fn copy_image(from: &Path, to: &Path) -> bool {
    let Ok(mut f) = fs::OpenOptions::new().read(true).custom_flags(libc_o_nonblock()).open(from) else { return false };
    if !f.metadata().is_ok_and(|m| m.is_file() && m.len() <= MAX_IMAGE) {
      return false;
    }
    let mut buf = Vec::new();
    if f.by_ref().take(MAX_IMAGE + 1).read_to_end(&mut buf).is_err() || buf.len() as u64 > MAX_IMAGE {
      return false;
    }
    let tmp = to.with_extension("tmp");
    fs::write(&tmp, &buf).is_ok() && fs::rename(&tmp, to).is_ok()
  }

  // persistablePopup: file-backed images point at our own copies, named by
  // the entry's stem; in-memory ones that could not be saved are dropped.
  pub fn persistable(&self, entry: &Entry) -> Entry {
    let mut out = entry.clone();
    for (role, value) in [("appIcon", &mut out.app_icon), ("image", &mut out.image)] {
      if value.is_empty() {
        continue;
      }
      let source = model::local_image_file(value);
      if source.is_empty() {
        if value.starts_with("image://") {
          value.clear();
        }
        continue;
      }
      let copy = self.images.join(format!("{}-{role}", entry.stem()));
      if Path::new(&source) != copy && !Self::copy_image(Path::new(&source), &copy) {
        // Gone already: keep the original reference, which simply won't load.
        continue;
      }
      *value = format!("file://{}", copy.display());
    }
    out
  }

  fn remove_images(&self, stem: &str) {
    if let Ok(dir) = fs::read_dir(&self.images) {
      for f in dir.flatten() {
        let name = f.file_name().to_string_lossy().to_string();
        if name.starts_with(&format!("{stem}-")) {
          let _ = fs::remove_file(f.path());
        }
      }
    }
  }

  // ---- popups and history ---------------------------------------------------

  pub fn write_popup(&self, entry: &Entry) -> Entry {
    let persisted = self.persistable(entry);
    write_atomic(&self.popups.join(entry.file_name()), &(persisted.to_json() + "\n"));
    persisted
  }

  // The popup left the screen: its file becomes the newest history entry.
  pub fn archive(&self, entry: &Entry) {
    let name = entry.file_name();
    if fs::rename(self.popups.join(&name), self.history.join(&name)).is_ok() {
      self.trim_history();
    }
  }

  // A notification do-not-disturb kept off the screen goes straight to history.
  pub fn write_history(&self, entry: &Entry) {
    let persisted = self.persistable(entry);
    write_atomic(&self.history.join(entry.file_name()), &(persisted.to_json() + "\n"));
    self.trim_history();
  }

  fn trim_history(&self) {
    let mut names: Vec<(i64, String)> = fs::read_dir(&self.history)
      .map(|d| {
        d.flatten()
          .map(|f| f.file_name().to_string_lossy().to_string())
          .filter(|n| n.ends_with(".json"))
          .map(|n| (n.split('-').next().and_then(|t| t.parse().ok()).unwrap_or(0), n))
          .collect()
      })
      .unwrap_or_default();
    names.sort();
    let stale = names.len().saturating_sub(HISTORY_LIMIT);
    for (_, name) in names.into_iter().take(stale) {
      let _ = fs::remove_file(self.history.join(&name));
      self.remove_images(name.trim_end_matches(".json"));
    }
  }

  pub fn clear_history(&self) {
    for entry in self.read_dir(&self.history) {
      let _ = fs::remove_file(self.history.join(entry.file_name()));
      self.remove_images(&entry.stem());
    }
  }

  fn read_dir(&self, dir: &Path) -> Vec<Entry> {
    let mut raw = String::new();
    if let Ok(d) = fs::read_dir(dir) {
      for f in d.flatten() {
        if f.path().extension().is_some_and(|e| e == "json") {
          if let Ok(text) = fs::read_to_string(f.path()) {
            raw += text.trim();
            raw.push('\n');
          }
        }
      }
    }
    model::parse_files(&raw)
  }

  pub fn history(&self) -> Vec<Entry> {
    self.read_dir(&self.history)
  }

  pub fn popups(&self) -> Vec<Entry> {
    self.read_dir(&self.popups)
  }

  // Copies no JSON names any more (a restart between copy and write).
  pub fn sweep_images(&self) {
    let Ok(dir) = fs::read_dir(&self.images) else { return };
    for f in dir.flatten() {
      let name = f.file_name().to_string_lossy().to_string();
      if name.ends_with(".tmp") {
        let _ = fs::remove_file(f.path());
        continue;
      }
      let stem = name.rsplit_once('-').map(|(s, _)| s).unwrap_or(&name);
      let json = format!("{stem}.json");
      if !self.popups.join(&json).exists() && !self.history.join(&json).exists() {
        let _ = fs::remove_file(f.path());
      }
    }
  }
}

fn libc_o_nonblock() -> i32 {
  // O_NONBLOCK on Linux.
  0o4000
}
