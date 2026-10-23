// doxia-freeze: freezes the screen for the capture scripts, in place of
// hyprpicker -r -z. Each monitor gets a grim screenshot of itself on an
// overlay layer surface, so slurp (an overlay surface mapped after it) picks
// over still content, and grim captures that same content once the pick is
// made.
//
//   doxia-freeze               freezes until killed
//   doxia-freeze --background  returns once the freeze is on screen and prints
//                              its PID, or prints nothing and exits 1 when the
//                              screen could not be frozen; the caller kills it

use gtk::prelude::*;
use gtk::{gdk, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::Cell;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::Duration;

// A freeze that never reaches the screen gives up instead of leaving the
// caller waiting.
const READY_TIMEOUT: Duration = Duration::from_secs(3);

// Forks before GTK starts any thread. The parent waits for the child to write
// one byte on the pipe (the freeze is painted) and prints the child's PID; an
// empty read means the child exited without freezing. Returns the write end in
// the child, or the exit code in the parent.
fn background() -> Result<libc::c_int, glib::ExitCode> {
  let mut fds = [0; 2];
  if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
    return Err(glib::ExitCode::FAILURE);
  }
  match unsafe { libc::fork() } {
    -1 => Err(glib::ExitCode::FAILURE),
    0 => unsafe {
      libc::close(fds[0]);
      // The caller reads the PID through $(...): the child must not hold its
      // stdout open.
      let null = libc::open(c"/dev/null".as_ptr(), libc::O_WRONLY);
      if null >= 0 {
        libc::dup2(null, libc::STDOUT_FILENO);
        libc::close(null);
      }
      Ok(fds[1])
    },
    pid => {
      unsafe { libc::close(fds[1]) };
      let mut byte = 0u8;
      let n = unsafe { libc::read(fds[0], (&raw mut byte).cast(), 1) };
      if n == 1 {
        println!("{pid}");
        Err(glib::ExitCode::SUCCESS)
      } else {
        Err(glib::ExitCode::FAILURE)
      }
    }
  }
}

// Every monitor's screenshot, taken together before any freeze is shown. PNG
// without compression: the fastest grim writes, and the freeze shows up that
// much sooner after the key press.
fn capture(monitors: &[gdk::Monitor]) -> Option<Vec<gdk::Texture>> {
  let children: Vec<_> = monitors
    .iter()
    .map(|m| {
      let name = m.connector().map(|c| c.to_string()).unwrap_or_default();
      Command::new("grim").args(["-l", "0", "-o", &name, "-"]).stdout(Stdio::piped()).stderr(Stdio::inherit()).spawn()
    })
    .collect::<Result<_, _>>()
    .map_err(|e| eprintln!("doxia-freeze: grim: {e}"))
    .ok()?;
  let mut textures = Vec::new();
  for child in children {
    let out = child.wait_with_output().ok().filter(|o| o.status.success())?;
    match gdk::Texture::from_bytes(&glib::Bytes::from_owned(out.stdout)) {
      Ok(t) => textures.push(t),
      Err(e) => {
        eprintln!("doxia-freeze: {e}");
        return None;
      }
    }
  }
  Some(textures)
}

fn freeze(monitor: &gdk::Monitor, texture: &gdk::Texture) -> gtk::Window {
  let window = gtk::Window::builder().decorated(false).build();
  window.init_layer_shell();
  // mango's layer rule for doxia-freeze turns off animation, blur and shadow:
  // the freeze has to look exactly like the screen it replaces.
  window.set_namespace(Some("doxia-freeze"));
  window.set_layer(Layer::Overlay);
  window.set_monitor(Some(monitor));
  window.set_keyboard_mode(KeyboardMode::None);
  for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
    window.set_anchor(edge, true);
  }
  window.set_exclusive_zone(-1);
  let picture = gtk::Picture::for_paintable(texture);
  picture.set_content_fit(gtk::ContentFit::Fill);
  picture.set_can_shrink(true);
  window.set_child(Some(&picture));
  window
}

fn main() -> glib::ExitCode {
  let ready_fd = if std::env::args().any(|a| a == "--background") {
    match background() {
      Ok(fd) => Some(fd),
      Err(code) => return code,
    }
  } else {
    None
  };

  if let Err(e) = gtk::init() {
    eprintln!("doxia-freeze: {e}");
    return glib::ExitCode::FAILURE;
  }
  let Some(display) = gdk::Display::default() else {
    return glib::ExitCode::FAILURE;
  };
  let monitors: Vec<gdk::Monitor> = display.monitors().iter::<gdk::Monitor>().filter_map(Result::ok).collect();
  let Some(textures) = capture(&monitors).filter(|t| !t.is_empty()) else {
    return glib::ExitCode::FAILURE;
  };

  let main_loop = glib::MainLoop::new(None, false);
  let unpainted = Rc::new(Cell::new(monitors.len()));
  for (monitor, texture) in monitors.iter().zip(&textures) {
    let window = freeze(monitor, texture);
    window.present();
    let Some(clock) = window.frame_clock() else {
      continue;
    };
    let unpainted = unpainted.clone();
    let painted = Cell::new(false);
    clock.connect_after_paint(move |_| {
      if painted.replace(true) {
        return;
      }
      unpainted.set(unpainted.get() - 1);
      if unpainted.get() == 0 {
        if let Some(fd) = ready_fd {
          unsafe {
            libc::write(fd, [1u8].as_ptr().cast(), 1);
            libc::close(fd);
          }
        }
      }
    });
  }
  {
    let main_loop = main_loop.clone();
    let unpainted = unpainted.clone();
    glib::timeout_add_local_once(READY_TIMEOUT, move || {
      if unpainted.get() > 0 {
        eprintln!("doxia-freeze: the freeze did not reach the screen");
        main_loop.quit();
      }
    });
  }
  main_loop.run();
  glib::ExitCode::FAILURE
}
