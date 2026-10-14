// The clipboard history, replacing shell/plugins/clipboard (Clipboard.qml):
// the watchers that record what is copied, and the picker that pastes from
// it. Opened with `omarchy-shell shell toggle omarchy.clipboard`
// (omarchy-menu-clipboard).
//
// The watchers are wl-paste --watch runs of the plugin's capture.sh, one for
// text and one for images, which print an entry per copy; doxia-shell keeps
// the history file (newest first, 300 entries). Quickshell's plugin ran the
// same watchers, so doxia-shell turns it off in shell.json and ends the ones
// it left before starting its own: two sets would record every copy twice.

pub mod model;

use crate::popup::{self, label};
use gtk::prelude::*;
use gtk::{gdk, gio, glib, pango};
use model::{Entry, Kind, Row};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

const WIDTH: i32 = 875;
const HEIGHT: i32 = 600;
const PADDING: i32 = 18;
const MARGIN: i32 = PADDING + 2;
const HEADER: i32 = 34;
const SPACING: i32 = 6;
const ROW: i32 = 50;
const ROW_SPACING: i32 = 4;

fn omarchy_path() -> String {
  std::env::var("OMARCHY_PATH").unwrap_or_default()
}

fn history_path() -> PathBuf {
  PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".local/state/omarchy/clipboard-history.json")
}

fn capture_script() -> String {
  format!("{}/shell/plugins/clipboard/capture.sh", omarchy_path())
}

#[derive(Default)]
struct State {
  opened: bool,
  history: Vec<Entry>,
  rows: Vec<Row>,
  filter: String,
  selected: usize,
  cursor: bool,
  pointer_origin: Option<(f64, f64)>,
  pointer_live: bool,
  // What the history file held when doxia-shell last wrote or read it.
  written: String,
}

pub struct Clipboard {
  window: gtk::Window,
  root: gtk::Box,
  card: Rc<popup::Card>,
  header: gtk::Label,
  list: gtk::Box,
  scroller: gtk::ScrolledWindow,
  preview_text: gtk::Label,
  preview_image: gtk::Picture,
  empty: gtk::Box,
  empty_label: gtk::Label,
  confirm: Rc<popup::Confirm>,
  im: gtk::IMMulticontext,
  st: RefCell<State>,
}

impl Clipboard {
  pub fn new(app: &gtk::Application) -> Rc<Clipboard> {
    let window = popup::window(app, "omarchy-clipboard", "clipboard-window");
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let content = gtk::Box::new(gtk::Orientation::Vertical, SPACING);
    let header = label("", "menu-header");
    header.set_xalign(0.0);
    header.set_ellipsize(pango::EllipsizeMode::End);
    header.set_height_request(HEADER);
    content.append(&header);

    // The list on the left half, the selected entry in full on the right.
    let halves = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    halves.set_homogeneous(true);
    halves.set_vexpand(true);
    let list = gtk::Box::new(gtk::Orientation::Vertical, ROW_SPACING);
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::External);
    scroller.set_child(Some(&list));
    scroller.set_margin_end(PADDING);
    halves.append(&scroller);
    let preview = gtk::Box::new(gtk::Orientation::Vertical, 0);
    preview.add_css_class("clipboard-preview");
    let preview_text = label("", "clipboard-preview-text");
    preview_text.set_wrap(true);
    preview_text.set_wrap_mode(pango::WrapMode::Char);
    preview_text.set_xalign(0.0);
    preview_text.set_yalign(0.0);
    preview_text.set_valign(gtk::Align::Start);
    preview_text.set_selectable(false);
    let preview_scroll = gtk::ScrolledWindow::new();
    preview_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::External);
    preview_scroll.set_child(Some(&preview_text));
    preview_scroll.set_vexpand(true);
    preview_scroll.set_margin_start(PADDING);
    let preview_image = gtk::Picture::new();
    preview_image.set_content_fit(gtk::ContentFit::Contain);
    preview_image.set_valign(gtk::Align::Start);
    preview_image.set_vexpand(true);
    preview_image.set_margin_start(PADDING);
    preview.append(&preview_scroll);
    preview.append(&preview_image);
    halves.append(&preview);
    let empty = gtk::Box::new(gtk::Orientation::Vertical, 8);
    empty.set_valign(gtk::Align::Center);
    empty.append(&label("\u{f014c}", "menu-empty-icon"));
    let empty_label = label("", "menu-empty");
    empty.append(&empty_label);
    let stack = gtk::Overlay::new();
    stack.set_child(Some(&halves));
    stack.add_overlay(&empty);
    stack.set_vexpand(true);
    content.append(&stack);

    let card = Rc::new(popup::Card::new(&content, MARGIN));
    card.overlay.set_halign(gtk::Align::Center);
    card.overlay.set_valign(gtk::Align::Center);
    card.overlay.set_vexpand(true);
    root.append(&card.overlay);
    window.set_child(Some(&root));

    let me = Rc::new_cyclic(|weak: &std::rc::Weak<Clipboard>| {
      let weak = weak.clone();
      let confirm = popup::Confirm::new("Delete", MARGIN, move |yes| {
        if let Some(me) = weak.upgrade() {
          me.answer_clear(yes);
        }
      });
      card.overlay.add_overlay(&confirm.widget);
      // The preview's text and image take turns.
      preview_scroll.set_visible(true);
      preview_image.set_visible(false);
      Clipboard {
        window,
        root,
        card,
        header,
        list,
        scroller,
        preview_text,
        preview_image,
        empty,
        empty_label,
        confirm,
        im: gtk::IMMulticontext::new(),
        st: RefCell::default(),
      }
    });
    me.load();
    me.watch_file();
    me.connect_input();
    me.start_watchers();
    me
  }

  // --- History ---------------------------------------------------------------

  fn load(self: &Rc<Self>) {
    let raw = std::fs::read_to_string(history_path()).unwrap_or_default();
    {
      let mut st = self.st.borrow_mut();
      if raw == st.written && !raw.is_empty() {
        return;
      }
      st.history = model::parse_history(if raw.is_empty() { "[]" } else { &raw });
      st.written = raw;
    }
    if self.st.borrow().opened {
      self.rebuild();
    }
  }

  // Others may edit the file too; doxia-shell's own writes are skipped.
  fn watch_file(self: &Rc<Self>) {
    let Ok(monitor) = gio::File::for_path(history_path()).monitor_file(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
    else {
      return;
    };
    let me = Rc::downgrade(self);
    monitor.connect_changed(move |_, _, _, event| {
      if matches!(
        event,
        gio::FileMonitorEvent::ChangesDoneHint | gio::FileMonitorEvent::Created | gio::FileMonitorEvent::Renamed | gio::FileMonitorEvent::MovedIn
      ) {
        if let Some(me) = me.upgrade() {
          me.load();
        }
      }
    });
    std::mem::forget(monitor);
  }

  fn save(&self) {
    let text = model::to_json(&self.st.borrow().history);
    let path = history_path();
    if let Some(dir) = path.parent() {
      let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, &text).is_ok() && std::fs::rename(&tmp, &path).is_ok() {
      self.st.borrow_mut().written = text;
    }
  }

  fn add(self: &Rc<Self>, entry: Entry) {
    {
      let mut st = self.st.borrow_mut();
      st.history = model::add(&st.history, entry, model::LIMIT);
    }
    self.save();
    if self.st.borrow().opened {
      self.rebuild();
    }
  }

  // --- Watchers --------------------------------------------------------------

  fn start_watchers(self: &Rc<Self>) {
    // Quickshell's plugin goes off first, then whatever watchers it (or a
    // doxia-shell before this one) left are ended, then ours start.
    crate::quickshell::disable("omarchy.clipboard");
    let me = self.clone();
    glib::timeout_add_local_once(Duration::from_secs(2), move || {
      let _ = std::process::Command::new("pkill")
        .args(["-f", "wl-paste .*--watch .*/shell/plugins/clipboard/capture\\.sh"])
        .status();
      me.capture_current();
      me.watch("text");
      me.watch("image/png");
    });
  }

  // What is on the clipboard right now.
  fn capture_current(self: &Rc<Self>) {
    let me = self.clone();
    glib::spawn_future_local(async move {
      let Ok(p) = gio::Subprocess::newv(&[std::ffi::OsStr::new(&capture_script())], gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE)
      else {
        return;
      };
      if let Ok((Some(out), _)) = p.communicate_utf8_future(None).await {
        if let Some(entry) = model::parse_line(&out) {
          me.add(entry);
        }
      }
    });
  }

  // One wl-paste --watch: an entry per line; it comes back if it dies, or
  // the history would silently stop. pdeathsig takes it down with us.
  fn watch(self: &Rc<Self>, kind: &'static str) {
    let script = capture_script();
    let argv = ["setpriv", "--pdeathsig", "TERM", "wl-paste", "--type", kind, "--watch", &script, kind];
    let argv: Vec<&std::ffi::OsStr> = argv.iter().map(std::ffi::OsStr::new).collect();
    let Ok(process) = gio::Subprocess::newv(&argv, gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE) else {
      return;
    };
    let me = self.clone();
    glib::spawn_future_local(async move {
      if let Some(out) = process.stdout_pipe() {
        let lines = gio::DataInputStream::new(&out);
        while let Ok(Some(line)) = lines.read_line_utf8_future(glib::Priority::DEFAULT).await {
          if let Some(entry) = model::parse_line(&line) {
            me.add(entry);
          }
        }
      }
      let _ = process.wait_future().await;
      glib::timeout_add_local_once(Duration::from_secs(1), move || me.watch(kind));
    });
  }

  // --- IPC and display -------------------------------------------------------

  pub fn ipc(self: &Rc<Self>, method: &str, _args: &[String]) -> Option<String> {
    match method {
      "summon" => self.open(),
      "toggle" => {
        if self.st.borrow().opened { self.close() } else { self.open() }
      }
      "hide" => self.close(),
      "call" => {}
      _ => return None,
    }
    Some("ok".into())
  }

  fn open(self: &Rc<Self>) {
    self.card.refresh();
    let gaps = crate::menu::gaps_out();
    let (w, h) = crate::emojis::screen();
    self.card.overlay.set_size_request(WIDTH.min(w - 2 * gaps), HEIGHT.min(h - 2 * gaps));
    {
      let mut st = self.st.borrow_mut();
      st.opened = true;
      st.filter.clear();
      st.selected = 0;
      st.cursor = true;
      st.pointer_origin = None;
      st.pointer_live = false;
    }
    self.confirm.hide();
    self.rebuild();
    self.window.set_visible(true);
    self.im.focus_in();
  }

  fn close(&self) {
    self.st.borrow_mut().opened = false;
    self.confirm.hide();
    self.im.focus_out();
    self.window.set_visible(false);
  }

  fn rebuild(self: &Rc<Self>) {
    let mut st = self.st.borrow_mut();
    st.rows = model::rows(&st.history, &st.filter, model::ROWS);
    if st.rows.is_empty() {
      st.selected = 0;
    } else if st.selected >= st.rows.len() {
      st.selected = st.rows.len() - 1;
    }
    popup::clear(&self.list);
    for (i, row) in st.rows.iter().enumerate() {
      self.list.append(&self.row_widget(row, i));
    }
    self.header.set_text(if st.filter.is_empty() { "Search clipboard…" } else { &st.filter });
    if st.filter.is_empty() { self.header.add_css_class("placeholder") } else { self.header.remove_css_class("placeholder") }
    self.empty.set_visible(st.rows.is_empty());
    self.empty_label.set_text(&if st.history.is_empty() {
      "Clipboard is empty".to_string()
    } else {
      format!("No matches for “{}”", st.filter)
    });
    drop(st);
    self.paint();
  }

  fn row_widget(self: &Rc<Self>, row: &Row, index: usize) -> gtk::Box {
    let widget = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    widget.add_css_class("clipboard-row");
    widget.set_height_request(ROW);
    if !row.preview_image.is_empty() {
      let thumb = gtk::Picture::for_filename(&row.preview_image);
      thumb.set_content_fit(gtk::ContentFit::Contain);
      thumb.set_size_request(ROW - 16, ROW - 16);
      thumb.set_can_shrink(true);
      widget.append(&thumb);
    }
    let text = label(&row.preview_text, "clipboard-row-text");
    text.set_xalign(0.0);
    text.set_hexpand(true);
    text.set_ellipsize(pango::EllipsizeMode::End);
    text.set_single_line_mode(true);
    if matches!(row.kind, Kind::Image | Kind::File) {
      text.add_css_class("dim");
    }
    widget.append(&text);

    let motion = gtk::EventControllerMotion::new();
    let me = Rc::downgrade(self);
    motion.connect_motion(move |_, _, _| {
      let Some(me) = me.upgrade() else { return };
      let changed = {
        let mut st = me.st.borrow_mut();
        if !st.pointer_live || me.confirm.open() || (st.cursor && st.selected == index) {
          false
        } else {
          st.cursor = true;
          st.selected = index;
          true
        }
      };
      if changed {
        me.paint();
      }
    });
    widget.add_controller(motion);
    let click = gtk::GestureClick::new();
    let me = Rc::downgrade(self);
    click.connect_released(move |g, _, _, _| {
      g.set_state(gtk::EventSequenceState::Claimed);
      let Some(me) = me.upgrade() else { return };
      if me.confirm.open() {
        return;
      }
      me.paste(index, Action::Paste);
    });
    widget.add_controller(click);
    widget
  }

  // The cursor row and the preview of it.
  fn paint(&self) {
    let st = self.st.borrow();
    let mut child = self.list.first_child();
    let mut i = 0;
    while let Some(w) = child {
      if st.cursor && i == st.selected { w.add_css_class("selected") } else { w.remove_css_class("selected") }
      i += 1;
      child = w.next_sibling();
    }
    let row = st.rows.get(st.selected);
    let image = row.map(|r| r.preview_image.clone()).unwrap_or_default();
    if let Some(scroll) = self.preview_text.parent().and_then(|p| p.parent()) {
      scroll.set_visible(image.is_empty());
    }
    self.preview_image.set_visible(!image.is_empty());
    if image.is_empty() {
      self.preview_text.set_text(row.map(|r| r.full_text.as_str()).unwrap_or(""));
    } else {
      self.preview_image.set_filename(Some(&image));
    }
    // Keep the cursor row in view.
    if st.cursor && !st.rows.is_empty() {
      let adj = self.scroller.vadjustment();
      let top = (st.selected as i32 * (ROW + ROW_SPACING)) as f64;
      let bottom = top + ROW as f64;
      if top < adj.value() {
        adj.set_value(top);
      } else if bottom > adj.value() + adj.page_size() {
        adj.set_value(bottom - adj.page_size());
      }
    }
  }

  fn select(&self, index: Option<usize>, delta: i64) {
    {
      let mut st = self.st.borrow_mut();
      let count = st.rows.len() as i64;
      if count == 0 {
        return;
      }
      st.pointer_origin = None;
      st.pointer_live = false;
      st.selected = match index {
        Some(i) => i.min(count as usize - 1),
        None if !st.cursor => if delta < 0 { (count - 1) as usize } else { 0 },
        None => (st.selected as i64 + delta).rem_euclid(count) as usize,
      };
      st.cursor = true;
    }
    self.paint();
  }

  fn set_filter(self: &Rc<Self>, filter: String) {
    {
      let mut st = self.st.borrow_mut();
      st.filter = filter;
      st.selected = 0;
      st.cursor = true;
      st.pointer_origin = None;
      st.pointer_live = false;
    }
    self.rebuild();
  }

  fn remove_selected(self: &Rc<Self>) {
    {
      let mut st = self.st.borrow_mut();
      let Some(index) = st.rows.get(st.selected).map(|r| r.index) else { return };
      st.history.remove(index);
      if st.rows.len() <= 1 {
        st.selected = 0;
        st.cursor = false;
      } else if st.selected >= st.rows.len() - 1 {
        st.selected = st.rows.len() - 2;
      }
      st.pointer_origin = None;
      st.pointer_live = false;
    }
    self.save();
    self.rebuild();
  }

  fn answer_clear(self: &Rc<Self>, yes: bool) {
    self.confirm.hide();
    if !yes {
      return;
    }
    {
      let mut st = self.st.borrow_mut();
      st.history.clear();
      st.selected = 0;
      st.cursor = false;
    }
    self.save();
    self.rebuild();
  }

  fn paste(&self, index: usize, action: Action) {
    let row = self.st.borrow().rows.get(index).cloned();
    let Some(row) = row else { return };
    self.close();
    let bin = |name: &str| format!("{}/bin/{name}", omarchy_path());
    let at = row.index.to_string();
    match (action, row.kind) {
      (Action::Open, _) => crate::run::argv(&[&bin("omarchy-clipboard-open"), "--history-index", &at]),
      (Action::Paste, Kind::Image) => crate::run::argv(&[&bin("omarchy-clipboard-paste-file"), &row.mime, &row.path]),
      (Action::Copy, Kind::Image) => crate::run::argv(&[&bin("omarchy-clipboard-paste-file"), "--copy-only", &row.mime, &row.path]),
      (_, _) if row.full_text.is_empty() => {}
      (Action::Paste, _) => crate::run::argv(&[&bin("omarchy-clipboard-paste-text"), "--shift-insert", "--history-index", &at]),
      (Action::Copy, _) => crate::run::argv(&[&bin("omarchy-clipboard-paste-text"), "--copy-only", "--history-index", &at]),
    }
  }

  // --- Input -----------------------------------------------------------------

  fn connect_input(self: &Rc<Self>) {
    let me = Rc::downgrade(self);
    let typed = Rc::downgrade(self);
    popup::connect_keys(
      &self.window,
      &self.im,
      move |key, state| me.upgrade().is_some_and(|me| me.key(key, state)),
      move |text| {
        let Some(me) = typed.upgrade() else { return };
        if me.confirm.open() {
          return;
        }
        let filter = me.st.borrow().filter.clone();
        me.set_filter(filter + text);
      },
    );
    let motion = gtk::EventControllerMotion::new();
    let me = Rc::downgrade(self);
    motion.connect_motion(move |_, x, y| {
      let Some(me) = me.upgrade() else { return };
      let mut st = me.st.borrow_mut();
      match st.pointer_origin {
        _ if st.pointer_live => {}
        None => st.pointer_origin = Some((x, y)),
        Some((ox, oy)) if (ox - x).abs() > 1.0 || (oy - y).abs() > 1.0 => st.pointer_live = true,
        _ => {}
      }
    });
    self.window.add_controller(motion);
    let me = Rc::downgrade(self);
    popup::close_on_outside_click(&self.root, self.card.clone(), move || {
      if let Some(me) = me.upgrade() {
        me.close();
      }
    });
  }

  fn key(self: &Rc<Self>, key: gdk::Key, state: gdk::ModifierType) -> bool {
    use gdk::Key;
    if self.confirm.open() {
      if let Some(yes) = self.confirm.key(key) {
        self.answer_clear(yes);
      }
      return true;
    }
    let (filter, cursor, count, selected, empty) = {
      let st = self.st.borrow();
      (st.filter.clone(), st.cursor, st.rows.len(), st.selected, st.history.is_empty())
    };
    if let Some(next) = popup::edit_filter(&filter, key, state) {
      self.set_filter(next);
      return true;
    }
    let shift = state.contains(gdk::ModifierType::SHIFT_MASK);
    let alt = state.contains(gdk::ModifierType::ALT_MASK);
    match key {
      Key::Escape => {
        if filter.is_empty() { self.close() } else { self.set_filter(String::new()) }
      }
      Key::Delete if shift => {
        if !empty {
          self.confirm.ask("Delete entire clipboard history?");
        }
      }
      Key::Delete => self.remove_selected(),
      Key::Up => self.select(None, -1),
      Key::Down => self.select(None, 1),
      Key::Page_Up => self.select(None, -6),
      Key::Page_Down => self.select(None, 6),
      Key::Home => self.select(Some(0), 0),
      Key::End => self.select(Some(count.saturating_sub(1)), 0),
      Key::Return | Key::KP_Enter => {
        if cursor {
          self.paste(selected, if alt { Action::Open } else if shift { Action::Copy } else { Action::Paste });
        } else if count > 0 {
          self.st.borrow_mut().cursor = true;
          self.paint();
        }
      }
      _ => return false,
    }
    true
  }
}

#[derive(Clone, Copy)]
enum Action {
  Paste,
  Copy,
  Open,
}
