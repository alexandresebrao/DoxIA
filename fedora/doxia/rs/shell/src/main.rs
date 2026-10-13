// doxia-shell: the long-running half of the DoxIA shell, taking over from
// Quickshell one surface at a time. It owns org.doxia.Shell on the session bus
// and each surface is a GApplication action, so scripts reach it with busctl
// (see bin/omarchy-osd):
//
//   busctl --user call org.doxia.Shell /org/doxia/Shell org.gtk.Actions \
//     Activate 'sava{sv}' osd 1 s '{"icon":"volume","value":"40"}' 0
//
// Running doxia-shell again while it runs does nothing.

mod osd;

use gtk::prelude::*;
use gtk::{gio, glib};
use std::rc::Rc;

fn main() -> glib::ExitCode {
  let app = gtk::Application::builder().application_id("org.doxia.Shell").build();

  app.connect_startup(|app| {
    let styler = Rc::new(doxia_theme::Styler::install(include_str!("style.css")));
    let osd = osd::Osd::new(app);

    let action = gio::SimpleAction::new("osd", Some(glib::VariantTy::STRING));
    let o = osd.clone();
    action.connect_activate(move |_, param| {
      if let Some(payload) = param.and_then(|p| p.str()) {
        o.show_json(payload);
      }
    });
    app.add_action(&action);

    let hide = gio::SimpleAction::new("osd-close", None);
    hide.connect_activate(move |_, _| osd.hide());
    app.add_action(&hide);

    // Theme switches.
    glib::timeout_add_seconds_local(2, move || {
      styler.reload(false);
      glib::ControlFlow::Continue
    });

    // A service: no window to keep it alive.
    std::mem::forget(app.hold());
  });
  app.connect_activate(|_| {});
  app.run()
}
