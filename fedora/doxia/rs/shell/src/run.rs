// Commands the menus start. Each runs in a scope of its own under
// app-graphical.slice (where uwsm puts apps): doxia-shell is a service, and a
// restart of it (the update does one) would otherwise take everything it
// started along, the update window included. Quickshell's ran in the session
// service, which lives as long as the session.

use std::process::{Command, Stdio};

fn spawn(cmd: &mut Command) {
  use std::os::unix::process::CommandExt;
  let _ = cmd
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .process_group(0)
    .spawn()
    .map(|mut c| std::thread::spawn(move || c.wait()));
}

fn scoped(argv: &[&str]) -> Command {
  let mut cmd = Command::new("systemd-run");
  cmd.args(["--user", "--scope", "--quiet", "--collect", "--slice=app-graphical.slice", "--"]).args(argv);
  cmd
}

// A shell command, through a login shell for the session's PATH.
pub fn shell(command: &str) {
  if !command.trim().is_empty() {
    spawn(&mut scoped(&["bash", "-lc", command]));
  }
}

// An argv, no shell in between.
pub fn argv(argv: &[&str]) {
  if !argv.is_empty() {
    spawn(&mut scoped(argv));
  }
}
