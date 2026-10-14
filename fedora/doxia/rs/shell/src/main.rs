// doxia-shell: the long-running half of the DoxIA shell, taking over from
// Quickshell one surface at a time: the OSD, the notifications, the polkit
// agent, the lock screen, the Omarchy menu and the Start menu so far. It
// owns org.doxia.Shell on the session bus and answers the same IPC calls
// Quickshell does for those targets (ipc.rs), which
// bin/omarchy-shell tries first:
//
//   busctl --user call org.doxia.Shell /org/doxia/Shell/Ipc org.doxia.Shell \
//     Call ssas osd show 1 '{"icon":"volume","value":"40"}'
//
// Running doxia-shell again while it runs does nothing.

mod apps;
mod card;
mod clipboard;
mod emojis;
mod images;
mod iniciar;
mod ipc;
mod lock;
mod menu;
mod notify;
mod osd;
mod popup;
mod polkit;
mod quickshell;
mod run;

use gtk::prelude::*;
use gtk::{gio, glib};
use std::rc::Rc;

fn main() -> glib::ExitCode {
  // The lock screen's PAM child (lock/pam.rs), before any GTK.
  let args: Vec<String> = std::env::args().collect();
  // What this build takes over from Quickshell, for fedora/doxia/apply-bar
  // (builds before 0.7.0 reject the option).
  if args.get(1).map(String::as_str) == Some("--features") {
    println!("notifications polkit lock menu iniciar clipboard emojis image-picker");
    return glib::ExitCode::SUCCESS;
  }
  if args.get(1).map(String::as_str) == Some("--pam") {
    let code = lock::pam::run(args.get(2).map(String::as_str).unwrap_or(""));
    return glib::ExitCode::from(code as u8);
  }

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

    let lock = lock::Lock::new(app);
    crate::quickshell::disable("omarchy.lock");
    let l = lock.clone();
    ipc.add("lock", Box::new(move |method, _| l.ipc(method)));

    // Quickshell's plugin surface for the plugins taken over (the menu,
    // the Start menu and the pickers so far); any other plugin, or a call
    // with none, stays Quickshell's.
    let omarchy_menu = menu::OmarchyMenu::new(app);
    let iniciar = iniciar::Iniciar::new(app);
    let clipboard = clipboard::Clipboard::new(app);
    let emojis = emojis::Emojis::new(app);
    let images = images::Images::new(app);
    let i = images.clone();
    ipc.add("image-selector", Box::new(move |method, args| i.selector_ipc(method, args)));
    ipc.add(
      "shell",
      Box::new(move |method, args| {
        let not_ours = || Some("Target not found.".to_string());
        match args.first().map(String::as_str) {
          Some("omarchy.menu") => omarchy_menu.ipc(method, &args[1..]),
          Some("doxia.iniciar") => iniciar.ipc(method, &args[1..]),
          Some("omarchy.clipboard") => clipboard.ipc(method, &args[1..]),
          Some("omarchy.emojis") => emojis.ipc(method, &args[1..]),
          Some("omarchy.image-picker") => images.plugin_ipc(method, &args[1..]),
          _ => not_ours(),
        }
      }),
    );

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
