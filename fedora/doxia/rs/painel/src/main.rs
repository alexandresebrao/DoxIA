// Painel Rápido do DoxIA: everyday shortcuts in a floating window. The actions
// come from ~/.config/omarchy/painel.json (the /painel Claude Code skill writes
// it): jira-request, jira-tasks, gitlab-pipeline, claude-text, link, command.
//
//   doxia-painel [<action id> | --favorito]
//
// A second launch reuses the open window. Jira credentials (e-mail + API token
// per site) live in ~/.config/omarchy/painel-credenciais.json (mode 600); GitLab
// uses glab's token; the pipelines' recent combinations go to
// ~/.local/state/doxia-painel/.

mod app;
mod config;
mod net;
mod pages;
mod ui;

use gtk::prelude::*;
use gtk::{gio, glib};
use std::cell::RefCell;
use std::rc::Rc;

fn main() -> glib::ExitCode {
  let application = gtk::Application::builder()
    .application_id("org.doxia.Painel")
    .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
    .build();

  let current: Rc<RefCell<Option<Rc<app::App>>>> = Rc::default();
  application.connect_command_line(move |application, cmd| {
    let args: Vec<String> = cmd.arguments().iter().skip(1).filter_map(|a| a.to_str().map(String::from)).collect();
    let existing = current.borrow().clone().filter(|w| w.window.is_visible());
    let fresh = existing.is_none();
    let win = existing.unwrap_or_else(|| {
      let w = app::App::new(application);
      *current.borrow_mut() = Some(w.clone());
      w
    });
    let mut target = args.first().cloned();
    if target.as_deref() == Some("--favorito") {
      target = win.cfg().get("favorite").and_then(|f| f.as_str()).map(String::from);
    }
    if let Some(t) = target.filter(|t| !t.is_empty()) {
      // A link or command from the command line needs no window.
      if win.open_action(&t, true) == Some("direct") && fresh {
        win.window.destroy();
        *current.borrow_mut() = None;
        return glib::ExitCode::SUCCESS;
      }
    }
    win.window.present();
    glib::ExitCode::SUCCESS
  });
  application.run()
}
