// Three-finger swipes on the touchpad, read straight off its evdev node, for
// the background to follow the fingers while mango's gesture is under way
// (mango tells where the windows went only once it is over). The node is
// readable through a udev rule that hands touchpads, and only them, to the
// seat's user (TAG+="uaccess"); without it this stays quiet and tries again
// now and then. It only reads: mango still gets every event.

use gtk::glib;
use std::fs::File;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::rc::Rc;
use std::time::Duration;

const EV_SYN: u16 = 0;
const EV_KEY: u16 = 1;
const EV_ABS: u16 = 3;
const SYN_REPORT: u16 = 0;
const ABS_X: u16 = 0;
const BTN_TOOL_TRIPLETAP: u16 = 0x14e;
// struct input_event on 64 bits: timeval, type, code, value.
const EVENT_SIZE: usize = 24;
const RETRY: Duration = Duration::from_secs(10);

// The first touchpad udev knows of: /dev/input/eventN.
fn find() -> Option<String> {
  let mut nodes: Vec<_> = std::fs::read_dir("/sys/class/input").ok()?.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.starts_with("event")).collect();
  nodes.sort();
  nodes.into_iter().find_map(|n| {
    let dev = std::fs::read_to_string(format!("/sys/class/input/{n}/dev")).ok()?;
    let data = std::fs::read_to_string(format!("/run/udev/data/c{}", dev.trim())).ok()?;
    data.lines().any(|l| l == "E:ID_INPUT_TOUCHPAD=1").then(|| format!("/dev/input/{n}"))
  })
}

// ABS_X's range (EVIOCGABS), to turn the fingers' travel into a share of
// the pad's width.
fn width(file: &File) -> Option<f64> {
  let mut info = [0i32; 6];
  const EVIOCGABS_X: libc::c_ulong = 0x8018_4540;
  let r = unsafe { libc::ioctl(file.as_raw_fd(), EVIOCGABS_X as _, info.as_mut_ptr()) };
  (r == 0 && info[2] > info[1]).then(|| (info[2] - info[1]) as f64)
}

#[derive(Default)]
pub struct Swipe {
  three: bool,
  start: Option<i32>,
  x: Option<i32>,
}

impl Swipe {
  // One event; Some(Some(travel)) while three fingers move (a share of the
  // pad's width, negative to the left), Some(None) when they lift.
  pub fn feed(&mut self, kind: u16, code: u16, value: i32, width: f64) -> Option<Option<f64>> {
    match (kind, code) {
      (EV_KEY, BTN_TOOL_TRIPLETAP) => {
        let was = self.three;
        self.three = value != 0;
        self.start = None;
        self.x = None;
        (was && !self.three).then_some(None)
      }
      (EV_ABS, ABS_X) if self.three => {
        self.start.get_or_insert(value);
        self.x = Some(value);
        None
      }
      (EV_SYN, SYN_REPORT) if self.three => Some(Some((self.x? - self.start?) as f64 / width)),
      _ => None,
    }
  }
}

pub fn watch(on_swipe: impl Fn(Option<f64>) + 'static) {
  open(Rc::new(on_swipe));
}

fn open(on_swipe: Rc<dyn Fn(Option<f64>)>) {
  let file = find().and_then(|path| std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(path).ok());
  let Some(mut file) = file else {
    glib::timeout_add_local_once(RETRY, move || open(on_swipe));
    return;
  };
  let pad = width(&file).unwrap_or(1000.0);
  let fd = file.as_raw_fd();
  let mut swipe = Swipe::default();
  glib_unix::unix_fd_add_local(fd, glib::IOCondition::IN | glib::IOCondition::ERR | glib::IOCondition::HUP, move |_, _| {
    let mut buf = [0u8; EVENT_SIZE * 64];
    loop {
      match file.read(&mut buf) {
        Ok(n) if n >= EVENT_SIZE => {
          for ev in buf[..n].chunks_exact(EVENT_SIZE) {
            let kind = u16::from_ne_bytes([ev[16], ev[17]]);
            let code = u16::from_ne_bytes([ev[18], ev[19]]);
            let value = i32::from_ne_bytes([ev[20], ev[21], ev[22], ev[23]]);
            if let Some(travel) = swipe.feed(kind, code, value, pad) {
              on_swipe(travel);
            }
          }
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return glib::ControlFlow::Continue,
        // Gone (unplugged, a resume): look for it again.
        _ => {
          let on_swipe = on_swipe.clone();
          glib::timeout_add_local_once(RETRY, move || open(on_swipe));
          return glib::ControlFlow::Break;
        }
      }
    }
  });
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn follows_three_fingers_until_they_lift() {
    let mut s = Swipe::default();
    assert_eq!(s.feed(EV_ABS, ABS_X, 500, 1000.0), None);
    assert_eq!(s.feed(EV_SYN, SYN_REPORT, 0, 1000.0), None);
    assert_eq!(s.feed(EV_KEY, BTN_TOOL_TRIPLETAP, 1, 1000.0), None);
    assert_eq!(s.feed(EV_ABS, ABS_X, 600, 1000.0), None);
    assert_eq!(s.feed(EV_SYN, SYN_REPORT, 0, 1000.0), Some(Some(0.0)));
    s.feed(EV_ABS, ABS_X, 350, 1000.0);
    assert_eq!(s.feed(EV_SYN, SYN_REPORT, 0, 1000.0), Some(Some(-0.25)));
    assert_eq!(s.feed(EV_KEY, BTN_TOOL_TRIPLETAP, 0, 1000.0), Some(None));
    assert_eq!(s.feed(EV_SYN, SYN_REPORT, 0, 1000.0), None);
  }
}
