// doxia-shell: the long-running half of the DoxIA shell, taking over from
// Quickshell one surface at a time: the OSD, the notifications, the polkit
// agent, the lock screen, the menus, the pickers, the reminder prompt, the
// battery warning and the desktop background so far. It
// owns org.doxia.Shell on the session bus and answers the same IPC calls
// Quickshell does for those targets (ipc.rs), which
// bin/omarchy-shell tries first:
//
//   busctl --user call org.doxia.Shell /org/doxia/Shell/Ipc org.doxia.Shell \
//     Call ssas osd show 1 '{"icon":"volume","value":"40"}'
//
// Running doxia-shell again while it runs does nothing.

mod apps;
mod background;
mod bar;
mod battery;
mod bluez;
mod card;
mod clipboard;
mod emojis;
mod images;
mod idle;
mod idle_monitor;
mod iniciar;
mod ipc;
mod lock;
mod mango;
mod menu;
mod mpris;
mod nightlight;
mod notify;
mod osd;
mod overlays;
mod pipewire;
mod popup;
mod polkit;
mod quickshell;
mod reminders;
mod run;
mod sni;
mod upower;
mod watch;

use gtk::prelude::*;
use gtk::{gio, glib};
use std::rc::Rc;

fn main() -> glib::ExitCode {
  // The lock screen's PAM child (lock/pam.rs), before any GTK.
  let args: Vec<String> = std::env::args().collect();
  // What this build takes over from Quickshell, for fedora/doxia/apply-bar
  // (builds before 0.7.0 reject the option).
  if args.get(1).map(String::as_str) == Some("--features") {
    println!("notifications polkit lock menu iniciar clipboard emojis image-picker reminders battery background bar idle");
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

    // Quickshell's plugin surface for the plugins taken over (the menus,
    // the pickers and the reminder prompt so far); any other plugin, or a
    // call with none, stays Quickshell's.
    let menu = menu::Menu::new(app);
    let iniciar = iniciar::Iniciar::new(app);
    let clipboard = clipboard::Clipboard::new(app);
    let emojis = emojis::Emojis::new(app);
    let images = images::Images::new(app);
    let reminders = reminders::Reminders::new(app);
    let i = images.clone();
    ipc.add("image-selector", Box::new(move |method, args| i.selector_ipc(method, args)));
    // The bar, unless DOXIA_BAR=0 leaves it to Quickshell; with it, the
    // network panel's QR code and speed tests.
    let with_bar = std::env::var("DOXIA_BAR").as_deref() != Ok("0");
    let wifiqr = with_bar.then(|| overlays::WifiQr::new(app));
    let speedtest = with_bar.then(|| overlays::SpeedTest::new(app, false));
    let disk_speedtest = with_bar.then(|| overlays::SpeedTest::new(app, true));

    // The bar, made further down: its panels answer for their widgets' ids.
    let bar_slot: Rc<std::cell::RefCell<Option<Rc<bar::Bar>>>> = Rc::default();
    let bs = bar_slot.clone();
    ipc.add(
      "shell",
      Box::new(move |method, args| {
        let not_ours = || Some("Target not found.".to_string());
        let bar = bs.borrow().clone();
        if method == "togglePanelAt" {
          let bar = bar?;
          let index = args.get(1).and_then(|i| i.trim().parse::<usize>().ok()).unwrap_or(0);
          return Some(bar.toggle_panel_at(args.first().map(String::as_str).unwrap_or(""), index));
        }
        match args.first().map(String::as_str) {
          // The omarchy.* names are Quickshell's, still sent by older
          // callers; bin/omarchy-shell hands doxia.* to Quickshell as those.
          Some("doxia.menu" | "omarchy.menu") => menu.ipc(method, &args[1..]),
          Some("doxia.iniciar") => iniciar.ipc(method, &args[1..]),
          Some("doxia.clipboard" | "omarchy.clipboard") => clipboard.ipc(method, &args[1..]),
          Some("doxia.emojis" | "omarchy.emojis") => emojis.ipc(method, &args[1..]),
          Some("doxia.image-picker" | "omarchy.image-picker") => images.plugin_ipc(method, &args[1..]),
          Some("doxia.reminders" | "omarchy.reminders") => reminders.ipc(method, &args[1..]),
          Some("omarchy.wifiqr" | "doxia.wifiqr") if wifiqr.is_some() => wifiqr.as_ref()?.ipc(method, &args[1..]),
          Some("omarchy.speedtest" | "doxia.speedtest") if speedtest.is_some() => speedtest.as_ref()?.ipc(method, &args[1..]),
          Some("omarchy.disk-speedtest" | "doxia.disk-speedtest") if disk_speedtest.is_some() => disk_speedtest.as_ref()?.ipc(method, &args[1..]),
          Some(id) => bar.and_then(|b| b.shell_call(method, id)).or_else(not_ours),
          _ => not_ours(),
        }
      }),
    );

    let (s, n) = (styler.clone(), notifications.clone());
    let background = background::Background::new(app, move || {
      s.reload(true);
      n.place();
    });
    ipc.add("background", Box::new(move |method, args| background.ipc(method, args)));

    crate::quickshell::disable("omarchy.battery");
    battery::Battery::start();

    let polkit = polkit::Polkit::new(app);
    polkit.start();

    if let Some(conn) = app.dbus_connection() {
      ipc.register(&conn);
      notifications.register(&conn);
    }

    // Idle (screensaver, then lock), taken from Quickshell unless
    // DOXIA_IDLE=0 leaves it there.
    if std::env::var("DOXIA_IDLE").as_deref() != Ok("0") {
      crate::quickshell::disable("omarchy.idle");
      let idle = idle::Idle::new();
      ipc.add("idle", Box::new(move |method, _| idle.ipc(method)));
    }

    // The bar, in place of Quickshell's: shell.json's bar.id picks the empty
    // doxia.bar plugin there (fedora/doxia/plugins/doxia.bar).
    let bar = with_bar.then(|| bar::Bar::new(app, &ipc));
    if bar.is_some() {
      crate::quickshell::take_bar();
    }
    *bar_slot.borrow_mut() = bar.clone();
    if bar.is_some() {
      let media = mpris::get();
      ipc.add("media", Box::new(move |method, _| media.ipc(method)));
      let night = nightlight::get();
      ipc.add("nightlight", Box::new(move |method, _| night.ipc(method)));
    }
    if let Some(b) = bar.clone() {
      ipc.add("doxia.bar", Box::new(move |method, _| b.ipc(method)));
    }
    if let Some(b) = bar.clone() {
      // Quickshell's name for it, which the scripts use.
      ipc.add("omarchy.bar", Box::new(move |method, _| b.ipc(method)));
    }

    // Theme switches: colors through the CSS, the bar's size and edge
    // through the popups' margins.
    glib::timeout_add_seconds_local(2, move || {
      if styler.reload(false) {
        notifications.place();
        if let Some(b) = bar.as_ref() {
          b.restyle();
        }
      }
      glib::ControlFlow::Continue
    });

    // A service: no window to keep it alive.
    std::mem::forget(app.hold());
  });
  app.connect_activate(|_| {});
  app.run()
}
