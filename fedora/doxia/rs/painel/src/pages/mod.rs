// One page per action type with a page; link and command run directly.

pub mod claude_text;
pub mod credentials;
pub mod gitlab;
pub mod home;
pub mod jira_request;
pub mod jira_tasks;

use crate::app::AppRef;
use crate::config::{self, str_of};
use gtk::gdk;
use serde_json::Value;
use std::rc::Rc;

pub trait Page {
  fn widget(&self) -> gtk::Widget;
  // Each time the page comes into view.
  fn shown(self: Rc<Self>) {}
  // Shortcuts for this page; true when taken.
  fn key(&self, _key: gdk::Key, _mods: gdk::ModifierType) -> bool {
    false
  }
}

pub fn build(kind: &str, app: AppRef, cfg: &Value, action: &Value) -> Option<Rc<dyn Page>> {
  Some(match kind {
    "jira-request" => jira_request::JiraRequest::new(app, cfg, action),
    "jira-tasks" => jira_tasks::JiraTasks::new(app, cfg, action),
    "gitlab-pipeline" => gitlab::Pipeline::new(app, action),
    "claude-text" => claude_text::ClaudeText::new(app, action),
    _ => return None,
  })
}

pub fn ctrl(mods: gdk::ModifierType) -> bool {
  mods.contains(gdk::ModifierType::CONTROL_MASK)
}

pub fn is_enter(key: gdk::Key) -> bool {
  key == gdk::Key::Return || key == gdk::Key::KP_Enter
}

pub fn is_key(key: gdk::Key, letter: char) -> bool {
  key.to_lower().to_unicode() == Some(letter)
}

// link opens the URL; command runs `run` (a bash string or an argv list) in
// `dir`, detached from the panel.
pub fn run_direct(action: &Value) {
  if str_of(action, "type") == "link" {
    crate::ui::open_url(str_of(action, "url"));
    return;
  }
  let argv: Vec<String> = match &action["run"] {
    Value::String(cmd) => vec!["bash".into(), "-c".into(), cmd.clone()],
    Value::Array(list) => {
      list.iter().map(|a| config::expand_path(a.as_str().unwrap_or("")).display().to_string()).collect()
    }
    _ => return,
  };
  let Some((program, args)) = argv.split_first() else { return };
  let dir = match str_of(action, "dir") {
    "" => "~",
    d => d,
  };
  use std::os::unix::process::CommandExt;
  let _ = std::process::Command::new(program)
    .args(args)
    .current_dir(config::expand_path(dir))
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .process_group(0)
    .spawn()
    .map(|mut c| std::thread::spawn(move || c.wait()));
}

// The header's back button goes one page back.
pub fn wire_back(back: &gtk::Button, app: &AppRef) {
  use gtk::prelude::*;
  let app = app.clone();
  back.connect_clicked(move |_| {
    if let Some(app) = app.upgrade() {
      app.pop();
    }
  });
}
