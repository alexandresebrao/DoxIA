// The Omarchy menu, replacing shell/plugins/menu (Menu.qml): a card over a
// dim scrim, the JSONC tree with routes, search across it, providers (apps,
// fonts, power profiles), the `when:`/`checked:` guards, and the select and
// input prompts of omarchy-menu-select/-input. Same sizes, keys and files as
// the QML; the IPC is Quickshell's plugin surface
// (`omarchy-shell shell summon|toggle|hide|call omarchy.menu ...`).
//
// Not carried over: the "Launching…" OSD the shell showed when an app took
// more than two seconds to open a window.

pub mod model;

use crate::apps::{self, App};
use crate::card;
use gtk::prelude::*;
use gtk::{gdk, gio, glib, graphene, pango};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use model::{Guards, Item, Kind, Menu as Tree, Row};
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

// Menu.qml's sizes at the default scale.
const CARD_WIDTH: i32 = 300;
const WIDE_CARD_WIDTH: i32 = 520;
// The padding inside the card's 2px border.
const MARGIN: i32 = 18 + 2;
const HEADER: i32 = 34;
const SPACING: i32 = 6;
const ROW: i32 = 50;
const DETAIL_ROW: i32 = 58;
const ROW_SPACING: i32 = 3;
const DIVIDER: i32 = 17;
// What shows of the first row past the fold, so it reads as a cut row.
const PEEK: i32 = 28;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
  Menu,
  Select,
  Input,
}

struct Provider {
  script: &'static str,
  icon: &'static str,
  // Re-run every time its submenu opens (a font installed since).
  volatile: bool,
  action: fn(&str) -> String,
}

fn provider(name: &str) -> Option<Provider> {
  match name {
    "fonts" => Some(Provider {
      script: "current=$(omarchy-font-current 2>/dev/null); omarchy-font-list 2>/dev/null | while read -r f; do [[ -z $f ]] && continue; printf '%s\\t%s\\t%s\\n' \"$f\" \"$f\" \"$current\"; done",
      icon: "",
      volatile: true,
      action: |v| format!("omarchy-font-set {}", quote(v)),
    }),
    "power-profiles" => Some(Provider {
      script: "current=$(powerprofilesctl get 2>/dev/null); omarchy-powerprofiles-list 2>/dev/null | while read -r p; do [[ -z $p ]] && continue; printf '%s\\t%s\\t%s\\n' \"$p\" \"$p\" \"$current\"; done",
      icon: "\u{f040b}",
      volatile: false,
      action: |v| format!("omarchy-powerprofiles-set autodetect {}", quote(v)),
    }),
    _ => None,
  }
}

fn quote(v: &str) -> String {
  format!("'{}'", v.replace('\'', "'\\''"))
}

#[derive(Default)]
struct State {
  defaults: Vec<model::Entry>,
  user: Vec<model::Entry>,
  tree: Tree,
  guards: Guards,
  guards_running: bool,
  guards_pending: bool,
  revision: u64,
  loaded: HashSet<String>,
  apps: Vec<App>,

  opened: bool,
  mode: Option<Mode>,
  prompt: String,
  options: Vec<String>,
  selection_file: String,
  done_file: String,
  dmenu_width: i32,
  dmenu_max_height: i32,

  active: String,
  nav: Vec<String>,
  filter: String,
  rows: Vec<Row>,
  // Each row's top inside the list, for scrolling it into view.
  tops: Vec<i32>,
  selected: usize,
  cursor: bool,

  // The card stays centered until the first search or move, then keeps its
  // top line (and the starting menu's rows height as its ceiling).
  card_top: Option<i32>,
  max_rows: Option<i32>,

  // The pointer selects only once it moves after an open or a key press.
  pointer_origin: Option<(f64, f64)>,
  pointer_live: bool,

  confirm: Option<(String, String)>,
  confirm_index: usize,
}

impl State {
  fn mode(&self) -> Mode {
    self.mode.unwrap_or(Mode::Menu)
  }

  fn dmenu(&self) -> bool {
    self.mode() != Mode::Menu
  }

  fn row_height(&self, row: &Row) -> i32 {
    if (!self.filter.is_empty() || self.dmenu()) && !row.detail.is_empty() { DETAIL_ROW } else { ROW }
  }
}

pub struct OmarchyMenu {
  window: gtk::Window,
  root: gtk::Box,
  card: gtk::Overlay,
  surface: card::Surface,
  header: gtk::Label,
  scroller: gtk::ScrolledWindow,
  list: gtk::Box,
  empty: gtk::Box,
  empty_label: gtk::Label,
  confirm: gtk::Box,
  confirm_label: gtk::Label,
  confirm_buttons: [gtk::Label; 2],
  im: gtk::IMMulticontext,
  gaps: i32,
  st: RefCell<State>,
}

fn label(text: &str, class: &str) -> gtk::Label {
  let l = gtk::Label::new(Some(text));
  l.add_css_class(class);
  l
}

fn omarchy_path() -> String {
  std::env::var("OMARCHY_PATH").unwrap_or_default()
}

pub fn gaps_out() -> i32 {
  std::process::Command::new("omarchy-mango-option")
    .arg("gap_outer_horizontal")
    .output()
    .ok()
    .and_then(|o| String::from_utf8(o.stdout).ok())
    .and_then(|s| s.trim().parse::<f64>().ok())
    .map(|n| (n / 2.0).round().max(0.0) as i32)
    .unwrap_or(5)
}

// When every row fits, the list gets its full height; when not, the card
// ends mid-row, never on a row boundary.
fn folded(totals: &[i32], available: i32) -> i32 {
  let count = totals.len();
  if count == 0 {
    return ROW;
  }
  if totals[count - 1] <= available {
    return totals[count - 1];
  }
  let mut full = 0;
  while full < count && totals[full] <= available {
    full += 1;
  }
  while full > 1 && totals[full - 1] + ROW_SPACING + PEEK > available {
    full -= 1;
  }
  if full < 1 {
    return available.max(ROW);
  }
  totals[full - 1] + ROW_SPACING + PEEK
}

impl OmarchyMenu {
  pub fn new(app: &gtk::Application) -> Rc<OmarchyMenu> {
    let window = gtk::Window::builder().application(app).decorated(false).build();
    window.add_css_class("doxia");
    window.add_css_class("menu-window");
    window.init_layer_shell();
    window.set_namespace(Some("omarchy-menu"));
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::Exclusive);
    for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
      window.set_anchor(edge, true);
    }
    window.set_exclusive_zone(-1);

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.set_hexpand(true);
    root.set_vexpand(true);

    let content = gtk::Box::new(gtk::Orientation::Vertical, SPACING);
    content.set_margin_top(MARGIN);
    content.set_margin_bottom(MARGIN);
    content.set_margin_start(MARGIN);
    content.set_margin_end(MARGIN);

    let header = label("", "menu-header");
    header.set_xalign(0.0);
    header.set_ellipsize(pango::EllipsizeMode::End);
    header.set_height_request(HEADER);
    content.append(&header);

    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::External);
    scroller.set_child(Some(&list));
    let empty = gtk::Box::new(gtk::Orientation::Vertical, 8);
    empty.set_valign(gtk::Align::Center);
    empty.append(&label("\u{f0209}", "menu-empty-icon"));
    let empty_label = label("", "menu-empty");
    empty_label.set_wrap(true);
    empty_label.set_justify(gtk::Justification::Center);
    empty.append(&empty_label);
    let stack = gtk::Overlay::new();
    stack.set_child(Some(&scroller));
    stack.add_overlay(&empty);
    content.append(&stack);

    let surface = card::Surface::new(doxia_theme::menu_card());
    let card = gtk::Overlay::new();
    card.add_css_class("menu-card");
    card.set_child(Some(&surface.area));
    card.add_overlay(&content);
    card.set_halign(gtk::Align::Center);
    card.set_valign(gtk::Align::Center);
    card.set_vexpand(true);

    // Uninstall confirmation over the card, like shell/Ui/ConfirmDialog.qml.
    let confirm = gtk::Box::new(gtk::Orientation::Vertical, 14);
    confirm.add_css_class("menu-confirm");
    confirm.set_valign(gtk::Align::Fill);
    confirm.set_halign(gtk::Align::Fill);
    let inner = gtk::Box::new(gtk::Orientation::Vertical, 14);
    inner.set_valign(gtk::Align::Center);
    inner.set_vexpand(true);
    inner.set_margin_start(MARGIN);
    inner.set_margin_end(MARGIN);
    let confirm_label = label("", "menu-confirm-text");
    confirm_label.set_wrap(true);
    confirm_label.set_justify(gtk::Justification::Center);
    inner.append(&confirm_label);
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::Center);
    let confirm_buttons = [label("Cancelar", "menu-confirm-button"), label("Desinstalar", "menu-confirm-button")];
    for b in &confirm_buttons {
      b.set_width_request(88);
      buttons.append(b);
    }
    confirm_buttons[1].add_css_class("destructive");
    inner.append(&buttons);
    confirm.append(&inner);
    confirm.set_visible(false);
    card.add_overlay(&confirm);

    root.append(&card);
    window.set_child(Some(&root));

    let im = gtk::IMMulticontext::new();
    let menu = Rc::new(OmarchyMenu {
      window,
      root,
      card,
      surface,
      header,
      scroller,
      list,
      empty,
      empty_label,
      confirm,
      confirm_label,
      confirm_buttons,
      im,
      gaps: gaps_out(),
      st: RefCell::new(State { active: "root".into(), dmenu_width: 300, ..Default::default() }),
    });
    menu.connect_input();
    menu.watch_sources();
    menu.load_sources();
    menu
  }

  // --- Sources -------------------------------------------------------------

  fn default_path() -> String {
    format!("{}/default/omarchy/omarchy-menu.jsonc", omarchy_path())
  }

  fn user_path() -> String {
    format!("{}/.config/omarchy/extensions/omarchy-menu.jsonc", std::env::var("HOME").unwrap_or_default())
  }

  fn load_sources(self: &Rc<Self>) {
    let read = |p: String| model::parse(&std::fs::read_to_string(p).unwrap_or_default());
    {
      let mut st = self.st.borrow_mut();
      st.defaults = read(Self::default_path());
      st.user = read(Self::user_path());
      st.tree = model::merge(&st.defaults, &st.user);
      st.revision += 1;
      st.loaded.clear();
    }
    self.evaluate_guards();
    if self.st.borrow().opened {
      self.rebuild();
      self.load_providers();
    }
  }

  // Edits to either file apply live.
  fn watch_sources(self: &Rc<Self>) {
    for path in [Self::default_path(), Self::user_path()] {
      let Ok(monitor) = gio::File::for_path(path).monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) else { continue };
      let me = Rc::downgrade(self);
      monitor.connect_changed(move |_, _, _, event| {
        if matches!(event, gio::FileMonitorEvent::ChangesDoneHint | gio::FileMonitorEvent::Created | gio::FileMonitorEvent::Deleted) {
          if let Some(me) = me.upgrade() {
            me.load_sources();
          }
        }
      });
      // Kept for the life of the process.
      std::mem::forget(monitor);
    }
    let me = Rc::downgrade(self);
    gio::AppInfoMonitor::get().connect_changed(move |_| {
      let Some(me) = me.upgrade() else { return };
      let loaded = me.st.borrow().loaded.contains("apps");
      if loaded {
        me.load_apps();
      }
    });
  }

  // One bash run answers every `when:` and `checked:`; the menu opens on the
  // last answers and updates when the new ones land.
  fn evaluate_guards(self: &Rc<Self>) {
    let script = {
      let mut st = self.st.borrow_mut();
      if st.guards_running {
        st.guards_pending = true;
        return;
      }
      st.guards_pending = false;
      let script = model::guard_script(&st.tree);
      if script.is_empty() {
        st.guards = Guards::default();
        return;
      }
      st.guards_running = true;
      script
    };
    let me = self.clone();
    glib::spawn_future_local(async move {
      let answer = run_capture(&["bash", "-lc", &script]).await;
      let pending = {
        let mut st = me.st.borrow_mut();
        st.guards_running = false;
        // A killed or failed run told only part of it: keep the last set.
        if let Some(out) = answer {
          st.guards = model::parse_guards(&out);
        }
        st.guards_pending
      };
      if me.st.borrow().opened {
        me.rebuild();
      }
      if pending {
        me.evaluate_guards();
      }
    });
  }

  fn load_apps(self: &Rc<Self>) {
    let all = apps::all();
    let rows: Vec<Item> = all
      .iter()
      .map(|a| {
        let mut row = Item::new(&format!("apps.{}", a.id), "apps", Kind::App, &a.name);
        row.description = a.generic.clone();
        row.aliases = std::iter::once(a.generic.clone()).filter(|g| !g.is_empty()).chain(a.keywords.iter().cloned()).collect();
        row.app_id = a.id.clone();
        row
      })
      .collect();
    {
      let mut st = self.st.borrow_mut();
      st.apps = all;
      st.loaded.insert("apps".into());
      st.tree.swap_rows("apps", rows);
    }
    if self.st.borrow().opened {
      self.rebuild();
    }
  }

  fn load_provider(self: &Rc<Self>, id: &str) {
    let (name, revision) = {
      let st = self.st.borrow();
      let Some(e) = st.tree.item(id) else { return };
      if e.provider.is_empty() || st.loaded.contains(id) {
        return;
      }
      (e.provider.clone(), st.revision)
    };
    if name == "apps" {
      self.load_apps();
      return;
    }
    let Some(spec) = provider(&name) else { return };
    self.st.borrow_mut().loaded.insert(id.into());
    let me = self.clone();
    let id = id.to_string();
    glib::spawn_future_local(async move {
      let Some(out) = run_capture(&["bash", "-lc", spec.script]).await else { return };
      if me.st.borrow().revision != revision {
        return;
      }
      let rows = model::provider_rows(&out, &id, spec.icon, spec.action);
      me.st.borrow_mut().tree.swap_rows(&id, rows);
      if me.st.borrow().opened {
        me.rebuild();
      }
    });
  }

  // The active menu's provider, or with a search every provider below it.
  fn load_providers(self: &Rc<Self>) {
    let ids: Vec<String> = {
      let st = self.st.borrow();
      if st.dmenu() {
        return;
      }
      if st.filter.trim().is_empty() {
        vec![st.active.clone()]
      } else {
        st.tree
          .order
          .iter()
          .filter(|id| {
            st.tree.item(id).is_some_and(|e| !e.provider.is_empty())
              && (st.active == "root" || **id == st.active || st.tree.is_descendant(id, &st.active))
          })
          .cloned()
          .collect()
      }
    };
    for id in ids {
      self.load_provider(&id);
    }
  }

  // --- IPC ------------------------------------------------------------------

  // `shell summon|toggle|hide|call omarchy.menu [payload]`.
  pub fn ipc(self: &Rc<Self>, method: &str, args: &[String]) -> Option<String> {
    let payload = args.first().map(String::as_str).unwrap_or("{}");
    match method {
      "summon" => self.open(payload),
      "toggle" => {
        if self.st.borrow().opened {
          self.cancel();
        } else {
          self.open(payload);
        }
      }
      "hide" => self.cancel(),
      // `call omarchy.menu <method> <arg>`.
      "call" => match args.first().map(String::as_str) {
        Some("refresh") => self.load_sources(),
        Some("ping") => {}
        _ => return Some("unknown".into()),
      },
      _ => return None,
    }
    Some("ok".into())
  }

  pub fn open(self: &Rc<Self>, payload: &str) {
    let p: Value = serde_json::from_str(payload).unwrap_or_default();
    let mode = p.get("mode").and_then(Value::as_str).unwrap_or("");
    if mode == "select" || mode == "input" {
      self.open_dmenu(&p, mode);
    } else {
      let route = p.get("initialMenu").or_else(|| p.get("menu")).and_then(Value::as_str).unwrap_or("root").to_string();
      self.open_route(&route);
    }
  }

  fn open_route(self: &Rc<Self>, route: &str) {
    let (id, action) = {
      let st = self.st.borrow();
      let id = st.tree.resolve_route(route);
      match st.tree.item(&id) {
        // An alias for a leaf runs it instead of opening it.
        Some(e) if e.kind == Kind::Action && !e.action.is_empty() => (id, Some(e.action.clone())),
        Some(e) if e.kind == Kind::Link && !e.target.is_empty() => (e.target.clone(), None),
        _ => (id, None),
      }
    };
    if let Some(action) = action {
      self.cancel();
      crate::run::shell(&action);
      return;
    }
    self.finish_request(None);
    {
      let mut st = self.st.borrow_mut();
      st.mode = Some(Mode::Menu);
      st.active = if st.tree.item(&id).is_some() { id.clone() } else { "root".into() };
      st.nav.clear();
      st.filter.clear();
      st.selected = 0;
      st.cursor = true;
      let invalidate = st.tree.item(&st.active).and_then(|e| provider(&e.provider)).is_some_and(|p| p.volatile);
      if invalidate {
        let active = st.active.clone();
        st.loaded.remove(&active);
      }
    }
    self.evaluate_guards();
    self.show();
    self.load_providers();
  }

  fn open_dmenu(self: &Rc<Self>, p: &Value, mode: &str) {
    // A prompt still waiting gets its "nothing" before this one takes over.
    self.finish_request(None);
    {
      let mut st = self.st.borrow_mut();
      st.mode = Some(if mode == "input" { Mode::Input } else { Mode::Select });
      let prompt = p.get("prompt").and_then(Value::as_str).unwrap_or("");
      st.prompt = if prompt.is_empty() { (if mode == "input" { "Digite" } else { "Selecione" }).into() } else { prompt.into() };
      st.options = p
        .get("options")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(|o| o.as_str().map(String::from).unwrap_or_else(|| o.to_string())).collect())
        .unwrap_or_default();
      st.selection_file = p.get("selectionFile").and_then(Value::as_str).unwrap_or("").into();
      st.done_file = p.get("doneFile").and_then(Value::as_str).unwrap_or("").into();
      st.dmenu_width = p.get("width").and_then(Value::as_i64).unwrap_or(300).max(1) as i32;
      st.dmenu_max_height = p.get("maxHeight").and_then(Value::as_i64).unwrap_or(0).max(0) as i32;
      st.active = "root".into();
      st.nav.clear();
      st.filter.clear();
      st.selected = 0;
      st.cursor = mode != "input";
    }
    self.show();
  }

  fn show(self: &Rc<Self>) {
    self.surface.set(doxia_theme::menu_card());
    {
      let mut st = self.st.borrow_mut();
      st.opened = true;
      st.card_top = None;
      st.max_rows = None;
      st.confirm = None;
      st.pointer_origin = None;
      st.pointer_live = false;
    }
    self.confirm.set_visible(false);
    self.card.set_valign(gtk::Align::Center);
    self.card.set_margin_top(0);
    self.rebuild();
    self.window.set_visible(true);
    self.im.focus_in();
  }

  fn close(&self) {
    let mut st = self.st.borrow_mut();
    st.opened = false;
    st.filter.clear();
    st.confirm = None;
    drop(st);
    self.im.focus_out();
    self.window.set_visible(false);
  }

  // Writes what a select/input prompt answers (None: nothing) and lets
  // omarchy-menu-select/-input go on.
  fn finish_request(&self, selection: Option<&str>) {
    let (selection_file, done_file) = {
      let mut st = self.st.borrow_mut();
      if st.done_file.is_empty() {
        return;
      }
      (std::mem::take(&mut st.selection_file), std::mem::take(&mut st.done_file))
    };
    if let Some(s) = selection {
      let _ = std::fs::write(&selection_file, format!("{s}\n"));
    }
    let _ = std::fs::write(&done_file, "");
  }

  fn cancel(&self) {
    self.finish_request(None);
    self.close();
  }

  // --- Display ---------------------------------------------------------------

  fn dmenu_rows(st: &State) -> Vec<Row> {
    if st.mode() == Mode::Input {
      return Vec::new();
    }
    let query = st.filter.trim().to_lowercase();
    let mut rows = Vec::new();
    for (i, option) in st.options.iter().enumerate() {
      // "<label>", "<glyph>\t<label>" or "<glyph>\t<label>\t<subtext>".
      let mut parts: Vec<&str> = option.split('\t').collect();
      let icon = if parts.len() > 1 { parts.remove(0) } else { "" };
      let label = if parts.is_empty() { "" } else { parts.remove(0) };
      let detail = parts.join("\t");
      if !query.is_empty() && !label.to_lowercase().contains(&query) && !detail.to_lowercase().contains(&query) {
        continue;
      }
      rows.push(Row {
        id: format!("dmenu.{i}"),
        kind: Kind::Action,
        icon: icon.into(),
        icon_font: String::new(),
        label: label.into(),
        target: String::new(),
        detail,
        path: String::new(),
        action: String::new(),
        app_id: String::new(),
        score: i as i64,
        divider: false,
      });
    }
    rows
  }

  fn screen_height(&self) -> i32 {
    let h = self.root.height();
    if h > 0 {
      return h;
    }
    gdk::Display::default()
      .and_then(|d| d.monitors().item(0))
      .and_downcast::<gdk::Monitor>()
      .map(|m| m.geometry().height())
      .unwrap_or(800)
  }

  fn screen_width(&self) -> i32 {
    let w = self.root.width();
    if w > 0 {
      return w;
    }
    gdk::Display::default()
      .and_then(|d| d.monitors().item(0))
      .and_downcast::<gdk::Monitor>()
      .map(|m| m.geometry().width())
      .unwrap_or(1280)
  }

  // Height the rows may take before the card leaves the screen (or passes
  // its frozen top), within 70% of it: more reads as a page.
  fn available_rows(&self, st: &State) -> i32 {
    let h = self.screen_height();
    let top = st.card_top.unwrap_or(self.gaps);
    let mut available = h - top - self.gaps - MARGIN * 2 - HEADER - SPACING;
    if let Some(max) = st.max_rows {
      available = available.min(max);
    }
    if st.dmenu() && st.dmenu_max_height > 0 {
      available = available.min(st.dmenu_max_height);
    }
    available.min((h as f64 * 0.7).round() as i32)
  }

  fn rebuild(self: &Rc<Self>) {
    let mut st = self.st.borrow_mut();
    if st.dmenu() {
      st.rows = Self::dmenu_rows(&st);
    } else {
      if st.tree.item(&st.active).is_none() {
        st.active = "root".into();
      }
      st.rows = st.tree.rows(&st.guards, &st.active, &st.filter);
    }
    if st.rows.is_empty() {
      st.selected = 0;
    } else if st.selected >= st.rows.len() {
      st.selected = st.rows.len() - 1;
    }

    while let Some(child) = self.list.first_child() {
      self.list.remove(&child);
    }
    let mut tops = Vec::new();
    let mut totals = Vec::new();
    let mut y = 0;
    for (i, row) in st.rows.iter().enumerate() {
      if i > 0 {
        y += ROW_SPACING;
      }
      let mut top_gap = if i > 0 { ROW_SPACING } else { 0 };
      if row.divider {
        let line = gtk::Box::new(gtk::Orientation::Vertical, 0);
        line.add_css_class("menu-divider");
        line.set_valign(gtk::Align::Center);
        line.set_margin_start(4);
        line.set_margin_end(4);
        let slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
        slot.set_height_request(DIVIDER);
        slot.set_margin_top(top_gap);
        slot.append(&line);
        line.set_vexpand(true);
        self.list.append(&slot);
        y += DIVIDER;
        top_gap = 0;
      }
      let height = st.row_height(row);
      let widget = self.row_widget(&st, row, i, height);
      widget.set_margin_top(top_gap);
      self.list.append(&widget);
      tops.push(y);
      y += height;
      totals.push(y);
    }
    st.tops = tops;

    let input = st.mode() == Mode::Input;
    let rows_height = if input { 0 } else { folded(&totals, self.available_rows(&st)) };
    self.scroller.set_visible(!input);
    self.scroller.set_min_content_height(rows_height);
    self.scroller.set_max_content_height(rows_height);
    self.empty.set_visible(st.rows.is_empty() && !input);
    self.empty_label.set_text(&if st.filter.is_empty() {
      "Nada aqui ainda".to_string()
    } else {
      format!("Nada encontrado para “{}”", st.filter)
    });

    let title = if !st.filter.is_empty() {
      st.filter.clone()
    } else if st.dmenu() {
      format!("{}…", st.prompt)
    } else {
      let e = st.tree.item(&st.active);
      let name = e.map(|e| if e.title.is_empty() { e.label.clone() } else { e.title.clone() }).unwrap_or_else(|| "Ir".into());
      format!("{name}…")
    };
    self.header.set_text(&title);
    if st.filter.is_empty() {
      self.header.add_css_class("placeholder");
    } else {
      self.header.remove_css_class("placeholder");
    }

    let width = if st.dmenu() {
      st.dmenu_width
    } else if st.active == "trigger.capture.screenrecord" || st.active == "style.font" {
      WIDE_CARD_WIDTH
    } else {
      CARD_WIDTH
    };
    let width = width.min(self.screen_width() - self.gaps * 2);
    let card_height = MARGIN * 2 + HEADER + if input { 0 } else { SPACING + rows_height };
    self.card.set_size_request(width, card_height);
    if let Some(top) = st.card_top {
      self.card.set_valign(gtk::Align::Start);
      self.card.set_margin_top(top);
    }
    drop(st);
    self.paint_selection();
    let me = self.clone();
    glib::idle_add_local_once(move || me.reveal());
  }

  fn row_widget(self: &Rc<Self>, st: &State, row: &Row, index: usize, height: i32) -> gtk::Box {
    let widget = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    widget.add_css_class("menu-row");
    widget.set_height_request(height);

    let app = row.kind == Kind::App;
    let has_icon = !row.icon.is_empty() || app;
    if app {
      let icon = st.apps.iter().find(|a| a.id == row.app_id).and_then(|a| a.icon.clone());
      let image = match icon {
        Some(icon) => gtk::Image::from_gicon(&icon),
        None => gtk::Image::from_icon_name("application-x-executable"),
      };
      image.set_pixel_size(18);
      image.set_size_request(36, -1);
      image.set_margin_start(8);
      widget.append(&image);
    } else if has_icon {
      let glyph = label(&row.icon, "menu-icon");
      glyph.set_width_request(36);
      glyph.set_margin_start(8);
      if !row.icon_font.is_empty() {
        let attrs = pango::AttrList::new();
        attrs.insert(pango::AttrFontDesc::new(&pango::FontDescription::from_string(&row.icon_font)));
        glyph.set_attributes(Some(&attrs));
      }
      widget.append(&glyph);
    }

    let text = gtk::Box::new(gtk::Orientation::Vertical, 3);
    text.set_valign(gtk::Align::Center);
    text.set_hexpand(true);
    text.set_margin_start(if has_icon { 6 } else { 18 });
    text.set_margin_end(6);
    let name = label(&row.label, "menu-label");
    name.set_xalign(0.0);
    name.set_ellipsize(pango::EllipsizeMode::End);
    text.append(&name);
    if (!st.filter.is_empty() || st.dmenu()) && !row.detail.is_empty() {
      let detail = label(&row.detail, "menu-detail");
      detail.set_xalign(0.0);
      detail.set_ellipsize(pango::EllipsizeMode::End);
      text.append(&detail);
    }
    widget.append(&text);

    let chevron = label(if matches!(row.kind, Kind::Menu | Kind::Link) { "›" } else { "" }, "menu-chevron");
    chevron.set_width_request(14);
    chevron.set_margin_end(8);
    widget.append(&chevron);

    let motion = gtk::EventControllerMotion::new();
    let me = Rc::downgrade(self);
    let hover = move || {
      let Some(me) = me.upgrade() else { return };
      let changed = {
        let mut st = me.st.borrow_mut();
        if !st.pointer_live || st.confirm.is_some() || (st.cursor && st.selected == index) {
          false
        } else {
          st.cursor = true;
          st.selected = index;
          true
        }
      };
      if changed {
        me.paint_selection();
      }
    };
    let h = hover.clone();
    motion.connect_enter(move |_, _, _| h());
    motion.connect_motion(move |_, _, _| hover());
    widget.add_controller(motion);

    let click = gtk::GestureClick::new();
    let me = Rc::downgrade(self);
    click.connect_released(move |gesture, _, _, _| {
      gesture.set_state(gtk::EventSequenceState::Claimed);
      let Some(me) = me.upgrade() else { return };
      if me.st.borrow().confirm.is_some() {
        return;
      }
      {
        let mut st = me.st.borrow_mut();
        st.cursor = true;
        st.selected = index;
      }
      me.activate(index, true);
    });
    widget.add_controller(click);
    widget
  }

  fn paint_selection(&self) {
    let st = self.st.borrow();
    let mut child = self.list.first_child();
    let mut index = 0;
    while let Some(w) = child {
      if w.has_css_class("menu-row") {
        if st.cursor && index == st.selected {
          w.add_css_class("selected");
        } else {
          w.remove_css_class("selected");
        }
        index += 1;
      }
      child = w.next_sibling();
    }
  }

  // Scrolls the cursor row into view with the next row peeking past it in
  // the direction of travel.
  fn reveal(&self) {
    let st = self.st.borrow();
    if st.rows.is_empty() || st.selected >= st.tops.len() {
      return;
    }
    let adj = self.scroller.vadjustment();
    let page = adj.page_size();
    let top = st.tops[st.selected] as f64;
    let bottom = top + st.row_height(&st.rows[st.selected]) as f64;
    let reach = (PEEK + ROW_SPACING) as f64;
    let mut value = adj.value();
    if st.selected + 1 < st.rows.len() && bottom + reach > value + page {
      value = bottom + reach - page;
    }
    if st.selected > 0 && top - reach < value {
      value = top - reach;
    }
    if st.selected == 0 {
      value = 0.0;
    }
    adj.set_value(value.clamp(0.0, (adj.upper() - page).max(0.0)));
  }

  // The card's current top, kept from the first search keystroke or move on.
  fn freeze_top(&self) {
    let mut st = self.st.borrow_mut();
    if st.card_top.is_some() || !self.window.is_visible() {
      return;
    }
    if let Some(bounds) = self.card.compute_bounds(&self.root) {
      st.card_top = Some(bounds.y().round() as i32);
      st.max_rows = Some(self.scroller.max_content_height());
    }
  }

  // --- Actions ---------------------------------------------------------------

  fn select(self: &Rc<Self>, delta: i64) {
    {
      let mut st = self.st.borrow_mut();
      let count = st.rows.len() as i64;
      if count == 0 {
        return;
      }
      st.pointer_origin = None;
      st.pointer_live = false;
      if !st.cursor {
        st.cursor = true;
        st.selected = if delta < 0 { (count - 1) as usize } else { 0 };
      } else {
        st.selected = ((st.selected as i64 + delta).rem_euclid(count)) as usize;
      }
    }
    self.paint_selection();
    self.reveal();
  }

  fn set_filter(self: &Rc<Self>, filter: String) {
    self.freeze_top();
    {
      let mut st = self.st.borrow_mut();
      st.filter = filter;
      st.selected = 0;
      st.cursor = st.mode() != Mode::Input;
      st.pointer_origin = None;
      st.pointer_live = false;
    }
    self.rebuild();
    if !self.st.borrow().filter.trim().is_empty() {
      self.load_providers();
    }
  }

  fn set_active(self: &Rc<Self>, id: &str, push: bool) {
    self.freeze_top();
    {
      let mut st = self.st.borrow_mut();
      let id = if st.tree.item(id).is_some() { id.to_string() } else { "root".into() };
      if push && id != st.active {
        let active = st.active.clone();
        st.nav.push(active);
      }
      st.active = id.clone();
      st.filter.clear();
      st.selected = 0;
      st.cursor = true;
      let volatile = st.tree.item(&id).and_then(|e| provider(&e.provider)).is_some_and(|p| p.volatile);
      if volatile {
        st.loaded.remove(&id);
      }
    }
    self.rebuild();
    self.load_providers();
  }

  fn go_back(self: &Rc<Self>) {
    let previous = {
      let mut st = self.st.borrow_mut();
      if st.active == "root" {
        return;
      }
      match st.nav.pop() {
        Some(p) => p,
        None => st.tree.item(&st.active).map(|e| e.parent.clone()).filter(|p| !p.is_empty()).unwrap_or_else(|| "root".into()),
      }
    };
    self.set_active(&previous, false);
  }

  fn activate(self: &Rc<Self>, index: usize, _pointer: bool) {
    let (mode, row, filter) = {
      let st = self.st.borrow();
      (st.mode(), st.rows.get(index).cloned(), st.filter.clone())
    };
    match mode {
      Mode::Input => {
        self.finish_request(Some(&filter));
        self.close();
      }
      Mode::Select => {
        let Some(row) = row else { return };
        let answer = if row.detail.is_empty() { row.label } else { format!("{}\t{}", row.label, row.detail) };
        self.finish_request(Some(&answer));
        self.close();
      }
      Mode::Menu => {
        let Some(row) = row else { return };
        match row.kind {
          Kind::Menu | Kind::Link => self.set_active(&row.target, true),
          Kind::App => {
            self.close();
            apps::launch(&row.app_id);
          }
          Kind::Action => {
            self.close();
            crate::run::shell(&row.action);
          }
        }
      }
    }
  }

  fn ask_uninstall(&self) {
    let mut st = self.st.borrow_mut();
    if !st.cursor {
      return;
    }
    let Some(row) = st.rows.get(st.selected).filter(|r| r.kind == Kind::App).cloned() else { return };
    self.confirm_label.set_text(&format!("Deseja desinstalar {}?", row.label));
    st.confirm = Some((row.app_id, row.label));
    st.confirm_index = 1;
    drop(st);
    self.paint_confirm();
    self.confirm.set_visible(true);
  }

  fn paint_confirm(&self) {
    let index = self.st.borrow().confirm_index;
    for (i, b) in self.confirm_buttons.iter().enumerate() {
      if i == index { b.add_css_class("selected") } else { b.remove_css_class("selected") }
    }
  }

  fn answer_confirm(self: &Rc<Self>, uninstall: bool) {
    let target = self.st.borrow_mut().confirm.take();
    self.confirm.set_visible(false);
    let Some((id, name)) = target.filter(|_| uninstall) else { return };
    self.cancel();
    let remover = format!("{}/bin/omarchy-remove-launcher-entry", omarchy_path());
    crate::run::argv(&[&remover, &id, &name]);
  }

  // --- Input -----------------------------------------------------------------

  fn connect_input(self: &Rc<Self>) {
    let keys = gtk::EventControllerKey::new();
    self.im.set_client_widget(Some(&self.window));
    keys.set_im_context(Some(&self.im));
    let me = Rc::downgrade(self);
    self.im.connect_commit(move |_, text| {
      let Some(me) = me.upgrade() else { return };
      let (typing, filter) = {
        let st = me.st.borrow();
        (st.opened && st.confirm.is_none(), st.filter.clone())
      };
      let printable: String = text.chars().filter(|c| !c.is_control()).collect();
      if typing && !printable.is_empty() {
        me.set_filter(filter + &printable);
      }
    });
    let me = Rc::downgrade(self);
    keys.connect_key_pressed(move |_, key, _, state| {
      let Some(me) = me.upgrade() else { return glib::Propagation::Proceed };
      if me.key(key, state) { glib::Propagation::Stop } else { glib::Propagation::Proceed }
    });
    self.window.add_controller(keys);

    // The pointer counts once it moves after the card appeared under it.
    let motion = gtk::EventControllerMotion::new();
    let me = Rc::downgrade(self);
    motion.connect_motion(move |_, x, y| {
      let Some(me) = me.upgrade() else { return };
      let mut st = me.st.borrow_mut();
      if st.pointer_live {
        return;
      }
      match st.pointer_origin {
        None => st.pointer_origin = Some((x, y)),
        Some((ox, oy)) if (ox - x).abs() > 1.0 || (oy - y).abs() > 1.0 => st.pointer_live = true,
        _ => {}
      }
    });
    self.window.add_controller(motion);

    // A click outside the card closes it.
    let click = gtk::GestureClick::new();
    let me = Rc::downgrade(self);
    click.connect_released(move |_, _, x, y| {
      let Some(me) = me.upgrade() else { return };
      let inside = me
        .card
        .compute_bounds(&me.root)
        .is_some_and(|b| b.contains_point(&graphene::Point::new(x as f32, y as f32)));
      if !inside {
        me.cancel();
      }
    });
    self.root.add_controller(click);

    for (i, button) in self.confirm_buttons.iter().enumerate() {
      let click = gtk::GestureClick::new();
      let me = Rc::downgrade(self);
      click.connect_released(move |_, _, _, _| {
        if let Some(me) = me.upgrade() {
          me.answer_confirm(i == 1);
        }
      });
      button.add_controller(click);
    }
  }

  fn key(self: &Rc<Self>, key: gdk::Key, state: gdk::ModifierType) -> bool {
    use gdk::Key;
    let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
    let other = state.intersects(gdk::ModifierType::ALT_MASK | gdk::ModifierType::SUPER_MASK | gdk::ModifierType::META_MASK);

    if self.st.borrow().confirm.is_some() {
      match key {
        Key::Escape => self.answer_confirm(false),
        Key::Left | Key::Right | Key::Tab | Key::ISO_Left_Tab => {
          let mut st = self.st.borrow_mut();
          st.confirm_index = 1 - st.confirm_index;
          drop(st);
          self.paint_confirm();
        }
        Key::Return | Key::KP_Enter => {
          let yes = self.st.borrow().confirm_index == 1;
          self.answer_confirm(yes);
        }
        _ => return false,
      }
      return true;
    }

    let (filter, dmenu, mode, cursor, count, selected) = {
      let st = self.st.borrow();
      (st.filter.clone(), st.dmenu(), st.mode(), st.cursor, st.rows.len(), st.selected)
    };
    match key {
      Key::Delete => self.ask_uninstall(),
      Key::Escape => {
        if filter.is_empty() { self.cancel() } else { self.set_filter(String::new()) }
      }
      // Util.editsFilter: Backspace (Ctrl: a word), Ctrl+U clears.
      Key::BackSpace if !filter.is_empty() && !other => {
        let next = if ctrl {
          let trimmed = filter.trim_end();
          trimmed.trim_end_matches(|c: char| !c.is_whitespace()).to_string()
        } else {
          let mut f = filter;
          f.pop();
          f
        };
        self.set_filter(next);
      }
      Key::u | Key::U if ctrl && !other && !filter.is_empty() && !state.contains(gdk::ModifierType::SHIFT_MASK) => {
        self.set_filter(String::new())
      }
      Key::BackSpace | Key::Left if filter.is_empty() => self.go_back(),
      Key::Up => self.select(-1),
      Key::Down => self.select(1),
      Key::Page_Up => self.select(-6),
      Key::Page_Down => self.select(6),
      Key::Return | Key::KP_Enter | Key::Right => {
        if dmenu {
          if mode == Mode::Input {
            self.activate(0, false);
          } else if count > 0 {
            self.activate(if cursor { selected } else { 0 }, false);
          }
        } else if cursor {
          self.activate(selected, false);
        } else if count > 0 {
          self.st.borrow_mut().cursor = true;
          self.paint_selection();
        }
      }
      _ => return false,
    }
    true
  }
}

// stdout of a command that exited 0; None otherwise.
async fn run_capture(argv: &[&str]) -> Option<String> {
  let argv: Vec<&std::ffi::OsStr> = argv.iter().map(std::ffi::OsStr::new).collect();
  let process = gio::Subprocess::newv(&argv, gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE).ok()?;
  let (out, _) = process.communicate_utf8_future(None).await.ok()?;
  (process.is_successful()).then(|| out.map(|s| s.to_string()).unwrap_or_default())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn folds_mid_row() {
    let totals: Vec<i32> = (1..=20).map(|n| n * ROW + (n - 1) * ROW_SPACING).collect();
    assert_eq!(folded(&totals, 10_000), *totals.last().unwrap());
    let h = folded(&totals, 300);
    assert!(h <= 300 && !totals.contains(&h));
    assert_eq!(folded(&[], 300), ROW);
    assert_eq!(quote("it's"), "'it'\\''s'");
  }
}
