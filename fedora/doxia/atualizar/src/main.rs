// Atualizar DoxIA: what an update would change, when each part installs, its
// progress and preferences. Opened by omarchy-launch-update (Menu > Atualizar >
// DoxIA and the bar icon). `--prefs` opens on the preferences. A second launch
// raises the window that is already open.

mod model;
mod theme;
mod window;

use gtk::prelude::*;
use gtk::{gio, glib};
use std::cell::RefCell;
use std::rc::Rc;

fn main() -> glib::ExitCode {
  let app = gtk::Application::builder()
    .application_id("org.doxia.Atualizar")
    .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
    .build();

  let current: Rc<RefCell<Option<Rc<window::Window>>>> = Rc::default();
  app.connect_command_line(move |app, cmd| {
    let prefs = cmd.arguments().iter().any(|a| a.to_str() == Some("--prefs"));
    let win = current.borrow_mut().get_or_insert_with(|| window::Window::new(app)).clone();
    win.present(prefs);
    glib::ExitCode::SUCCESS
  });
  app.run()
}
