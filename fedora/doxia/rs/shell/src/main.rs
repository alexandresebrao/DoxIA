// doxia-shell: the long-running half of the DoxIA shell, taking over from
// Quickshell one surface at a time: the OSD, the notifications and the polkit
// agent so far. It owns org.doxia.Shell on the session bus and answers the
// same IPC calls Quickshell does for those targets (ipc.rs), which
// bin/omarchy-shell tries first:
//
//   busctl --user call org.doxia.Shell /org/doxia/Shell/Ipc org.doxia.Shell \
//     Call ssas osd show 1 '{"icon":"volume","value":"40"}'
//
// Running doxia-shell again while it runs does nothing.

mod ipc;
mod notify;
mod osd;
mod polkit;
mod quickshell;

use gtk::prelude::*;
use gtk::{gio, glib};
use std::rc::Rc;

fn main() -> glib::ExitCode {
  let app = gtk::Application::builder().application_id("org.doxia.Shell").build();

  app.connect_startup(|app| {
    let styler = Rc::new(doxia_theme::Styler::install(include_str!("style.css")));
    let ipc = Rc::new(ipc::Ipc::default());

    let osd = osd::Osd::new(app);
    // Kept for callers of the first doxia-shell release.
    let action = gio::SimpleAction::new("osd", Some(glib::VariantTy::STRING));
    let o = osd.clone();
    action.connect_activate(move |_, param| {
      if let Some(payload) = param.and_then(|p| p.str()) {
        o.show_json(payload);
      }
    });
    app.add_action(&action);
    let o = osd.clone();
    ipc.add(
      "osd",
      Box::new(move |method, args| {
        Some(match method {
          "show" => {
            o.show_json(args.first().map(String::as_str).unwrap_or("{}"));
            "ok".into()
          }
          "close" => {
            o.hide();
            "ok".into()
          }
          "state" => o.state().into(),
          "ping" => "ok".into(),
          _ => return None,
        })
      }),
    );

    let notifications = notify::Notifications::new(app);
    let n = notifications.clone();
    ipc.add("notifications", Box::new(move |method, args| n.ipc(method, args)));

    let polkit = polkit::Polkit::new(app);
    polkit.start();

    if let Some(conn) = app.dbus_connection() {
      ipc.register(&conn);
      notifications.register(&conn);
    }

    // Theme switches: colors through the CSS, the bar's size and edge
    // through the popups' margins.
    glib::timeout_add_seconds_local(2, move || {
      if styler.reload(false) {
        notifications.place();
      }
      glib::ControlFlow::Continue
    });

    // A service: no window to keep it alive.
    std::mem::forget(app.hold());
  });
  app.connect_activate(|_| {});
  app.run()
}
