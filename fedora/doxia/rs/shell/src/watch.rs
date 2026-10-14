// Commands the shell reads from: a watch that prints a line per change (mmsg
// watch, pactl subscribe), started again when it ends, and one-shot runs
// whose output comes back on the main loop.

use gtk::prelude::*;
use gtk::{gio, glib};
use std::time::Duration;

// Runs `argv` and hands each line to `on_line`; when it ends (the compositor
// restarting, a socket not up yet) it starts again after two seconds.
pub fn lines(argv: &[&str], on_line: impl Fn(&str) + 'static) {
  let argv: Vec<String> = argv.iter().map(|s| s.to_string()).collect();
  let on_line = std::rc::Rc::new(on_line);
  start(argv, on_line);
}

fn start(argv: Vec<String>, on_line: std::rc::Rc<dyn Fn(&str)>) {
  glib::spawn_future_local(async move {
    let args: Vec<&std::ffi::OsStr> = argv.iter().map(|s| s.as_ref()).collect();
    if let Ok(child) = gio::Subprocess::newv(&args, gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE) {
      if let Some(out) = child.stdout_pipe() {
        let reader = gio::DataInputStream::new(&out);
        while let Ok(Some(line)) = reader.read_line_utf8_future(glib::Priority::DEFAULT).await {
          on_line(&line);
        }
      }
      let _ = child.wait_future().await;
    }
    glib::timeout_add_local_once(Duration::from_secs(2), move || start(argv, on_line));
  });
}

// Runs `argv` to the end and returns what it printed (None when it could
// not start or failed).
pub async fn output(argv: &[&str]) -> Option<String> {
  let args: Vec<&std::ffi::OsStr> = argv.iter().map(|s| s.as_ref()).collect();
  let child = gio::Subprocess::newv(&args, gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE).ok()?;
  let (out, _) = child.communicate_utf8_future(None).await.ok()?;
  child.is_successful().then(|| out.map(|s| s.to_string()).unwrap_or_default())
}

// The same through bash -lc, for the session's PATH and shell syntax.
pub async fn shell(command: &str) -> Option<String> {
  output(&["bash", "-lc", command]).await
}
