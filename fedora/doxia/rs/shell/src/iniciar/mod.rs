// The Start menu, replacing fedora/doxia/plugins/doxia.iniciar (Iniciar.qml):
// Windows 98 style in the theme's menu colors, a vertical "DoxIA" banner,
// relief edges and cascading submenus that open on hover. Typing anywhere
// goes to Localizar; Executar... runs a command. Opened with
// `omarchy-shell shell toggle doxia.iniciar` (the bar emblem and
// SUPER+ALT+SPACE); `{"path": [2, 0]}` opens the cascade along those rows.

pub mod model;

use crate::apps::{self, App};
use gtk::prelude::*;
use gtk::{gdk, gio, glib, graphene, pango};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

const MAX_LEVELS: usize = 5;
// Iniciar.qml's sizes.
const BEVEL: i32 = 3;
const PAD: i32 = 3;
const BANNER: i32 = 30;
const ROOT_ROW: i32 = 38;
const ROW: i32 = 26;
const SEPARATOR: i32 = 9;
const ROOT_WIDTH: i32 = 250;
const SUB_WIDTH: i32 = 260;
const HOVER_MS: u64 = 280;

#[derive(Clone)]
enum Icon {
  None,
  // A theme icon, drawn in its symbolic form (tinted like the text) when
  // there is one, else in gray.
  Named(String, &'static str),
  App(Option<gio::Icon>),
  // The theme's emblem (the DoxIA ∞), tinted like the text.
  Emblem,
}

#[derive(Clone)]
enum Sub {
  Programs,
  Folder(usize),
  Documents,
  Settings,
  Power,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
  Menu,
  Search,
  Run,
}

#[derive(Clone)]
enum Act {
  Shell(String),
  App(String),
  Open(String),
}

#[derive(Clone)]
enum Entry {
  Sep,
  Item { label: String, icon: Icon, sub: Option<Sub>, mode: Option<Mode>, act: Option<Act>, disabled: bool },
}

impl Entry {
  fn command(label: &str, icon: &str, command: &str) -> Entry {
    Entry::Item {
      label: label.into(),
      icon: Icon::Named(icon.into(), ""),
      sub: None,
      mode: None,
      act: Some(Act::Shell(command.into())),
      disabled: false,
    }
  }
  fn folder(label: &str, icon: &str, sub: Sub) -> Entry {
    Entry::Item { label: label.into(), icon: Icon::Named(icon.into(), ""), sub: Some(sub), mode: None, act: None, disabled: false }
  }
  fn disabled(label: &str) -> Entry {
    Entry::Item { label: label.into(), icon: Icon::None, sub: None, mode: None, act: None, disabled: true }
  }
  fn app(a: &App) -> Entry {
    Entry::Item {
      label: a.name.clone(),
      icon: Icon::App(a.icon.clone()),
      sub: None,
      mode: None,
      act: Some(Act::App(a.id.clone())),
      disabled: false,
    }
  }
  fn selectable(&self) -> bool {
    matches!(self, Entry::Item { disabled: false, .. })
  }
  fn opens(&self) -> bool {
    matches!(self, Entry::Item { sub: Some(_), .. } | Entry::Item { mode: Some(_), .. })
  }
}

struct Level {
  items: Vec<Entry>,
  parent: Option<usize>,
  mode: Mode,
  selected: Option<usize>,
  // Built for this level: the column and its rows.
  column: gtk::Box,
  rows: Vec<gtk::Box>,
  scroller: gtk::ScrolledWindow,
  field: Option<gtk::Label>,
  x: i32,
  y: i32,
}

#[derive(Default)]
struct State {
  opened: bool,
  levels: Vec<Level>,
  focus: usize,
  typed: String,
  apps: Vec<App>,
  folders: Vec<model::Folder>,
  hover: Option<glib::SourceId>,
}

pub struct Iniciar {
  window: gtk::Window,
  fixed: gtk::Fixed,
  im: gtk::IMMulticontext,
  gap: i32,
  st: RefCell<State>,
}

fn label(text: &str, class: &str) -> gtk::Label {
  let l = gtk::Label::new(Some(text));
  l.add_css_class(class);
  l
}

fn row_height(level: usize, e: &Entry) -> i32 {
  match e {
    Entry::Sep => SEPARATOR,
    _ if level == 0 => ROOT_ROW,
    _ => ROW,
  }
}

fn header_height(mode: Mode) -> i32 {
  if mode == Mode::Menu { 0 } else { ROW + 8 }
}

fn row_y(level: usize, items: &[Entry], index: usize) -> i32 {
  items.iter().take(index).map(|e| row_height(level, e)).sum()
}

fn content_height(level: usize, items: &[Entry], mode: Mode) -> i32 {
  row_y(level, items, items.len()) + header_height(mode) + if mode == Mode::Run { ROW } else { 0 }
}

fn column_width(level: usize) -> i32 {
  (if level == 0 { ROOT_WIDTH + BANNER } else { SUB_WIDTH }) + (BEVEL + PAD) * 2
}

impl Iniciar {
  pub fn new(app: &gtk::Application) -> Rc<Iniciar> {
    let window = gtk::Window::builder().application(app).decorated(false).build();
    window.add_css_class("doxia");
    window.add_css_class("iniciar-window");
    window.init_layer_shell();
    window.set_namespace(Some("doxia-iniciar"));
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::Exclusive);
    for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
      window.set_anchor(edge, true);
    }
    // Keeps clear of the bar: the menu starts right under the emblem.
    window.set_exclusive_zone(0);
    let fixed = gtk::Fixed::new();
    fixed.set_hexpand(true);
    fixed.set_vexpand(true);
    window.set_child(Some(&fixed));

    let gap = crate::menu::gaps_out() + 2;
    let me = Rc::new(Iniciar { window, fixed, im: gtk::IMMulticontext::new(), gap, st: RefCell::new(State::default()) });
    me.connect_input();
    me
  }

  // `shell summon|toggle|hide doxia.iniciar [payload]`.
  pub fn ipc(self: &Rc<Self>, method: &str, args: &[String]) -> Option<String> {
    match method {
      "summon" => self.open(args.first().map(String::as_str).unwrap_or("{}")),
      "toggle" => {
        if self.st.borrow().opened {
          self.dismiss();
        } else {
          self.open(args.first().map(String::as_str).unwrap_or("{}"));
        }
      }
      "hide" => self.dismiss(),
      "call" => {}
      _ => return None,
    }
    Some("ok".into())
  }

  fn open(self: &Rc<Self>, payload: &str) {
    self.truncate(0);
    {
      let mut st = self.st.borrow_mut();
      st.apps = apps::search(&apps::all(), "");
      st.folders = model::folders(&st.apps);
      st.typed.clear();
      st.focus = 0;
      st.opened = true;
    }
    self.clear_levels();
    let root = self.root_items();
    self.push_level(None, root, Mode::Menu);
    self.window.set_visible(true);
    self.im.focus_in();
    let path: Vec<usize> = serde_json::from_str::<serde_json::Value>(payload)
      .ok()
      .and_then(|p| p.get("path").and_then(|v| v.as_array()).cloned())
      .map(|a| a.iter().filter_map(|v| v.as_u64()).map(|v| v as usize).collect())
      .unwrap_or_default();
    for (level, index) in path.into_iter().enumerate().take(MAX_LEVELS - 1) {
      self.set_selection(level, Some(index));
      if !self.expand(level, index, true) {
        break;
      }
    }
  }

  fn dismiss(&self) {
    let mut st = self.st.borrow_mut();
    st.opened = false;
    if let Some(id) = st.hover.take() {
      id.remove();
    }
    drop(st);
    self.im.focus_out();
    self.window.set_visible(false);
  }

  // --- Contents --------------------------------------------------------------

  fn emblem_item(&self) -> Entry {
    Entry::Item {
      label: "DoxIA Express".into(),
      icon: Icon::Emblem,
      sub: None,
      mode: None,
      act: Some(Act::Shell("omarchy-shell shell summon doxia.menu '{\"menu\":\"root\"}'".into())),
      disabled: false,
    }
  }

  fn root_items(&self) -> Vec<Entry> {
    vec![
      self.emblem_item(),
      Entry::Sep,
      Entry::folder("Programas", "applications-other", Sub::Programs),
      Entry::folder("Documentos", "folder-documents", Sub::Documents),
      Entry::folder("Configurações", "preferences-system", Sub::Settings),
      Entry::Item {
        label: "Localizar".into(),
        icon: Icon::Named("system-search".into(), ""),
        sub: None,
        mode: Some(Mode::Search),
        act: None,
        disabled: false,
      },
      Entry::command("Ajuda", "help-browser", "omarchy-menu-keybindings"),
      Entry::Item {
        label: "Executar...".into(),
        icon: Icon::Named("system-run".into(), ""),
        sub: None,
        mode: Some(Mode::Run),
        act: None,
        disabled: false,
      },
      Entry::Sep,
      Entry::folder("Desligar...", "system-shutdown", Sub::Power),
    ]
  }

  fn sub_items(&self, sub: &Sub) -> Vec<Entry> {
    let st = self.st.borrow();
    match sub {
      Sub::Programs => st.folders.iter().enumerate().map(|(i, f)| Entry::folder(f.label, f.icon, Sub::Folder(i))).collect(),
      Sub::Folder(i) => st.folders.get(*i).map(|f| f.apps.iter().map(Entry::app).collect()).unwrap_or_default(),
      Sub::Documents => {
        let mut items = vec![Entry::command("Meus documentos", "folder-documents", "xdg-open \"$(xdg-user-dir DOCUMENTS)\""), Entry::Sep];
        let xbel = format!("{}/.local/share/recently-used.xbel", std::env::var("HOME").unwrap_or_default());
        let recent = model::recent_documents(&std::fs::read_to_string(xbel).unwrap_or_default(), 15);
        if recent.is_empty() {
          items.push(Entry::disabled("(Vazio)"));
        }
        for d in recent {
          items.push(Entry::Item {
            label: d.name,
            icon: Icon::Named(d.icon, "text-x-generic"),
            sub: None,
            mode: None,
            act: Some(Act::Open(d.href)),
            disabled: false,
          });
        }
        items
      }
      Sub::Settings => {
        let mut items = vec![
          self.emblem_item(),
          Entry::command("Tema", "preferences-desktop-appearance", "omarchy-menu toggle theme"),
          Entry::command("Papel de parede", "preferences-desktop-wallpaper", "omarchy-menu toggle background"),
          Entry::command(
            "Atalhos de teclado",
            "preferences-desktop-keyboard-shortcuts",
            "omarchy-launch-config-editor \"$HOME/.config/mango/bindings.conf\"",
          ),
        ];
        let settings: Vec<&App> = st.apps.iter().filter(|a| a.categories.iter().any(|c| c == "Settings")).collect();
        if !settings.is_empty() {
          items.push(Entry::Sep);
        }
        items.extend(settings.into_iter().map(Entry::app));
        items
      }
      Sub::Power => vec![
        Entry::command("Bloquear", "system-lock-screen", "omarchy-system-lock"),
        Entry::command("Suspender", "system-suspend", "systemctl suspend"),
        Entry::command("Sair da sessão", "system-log-out", "omarchy-system-logout"),
        Entry::Sep,
        Entry::command("Reiniciar", "system-reboot", "omarchy-system-reboot"),
        Entry::command("Desligar", "system-shutdown", "omarchy-system-shutdown"),
      ],
    }
  }

  fn search_items(&self, query: &str) -> Vec<Entry> {
    let st = self.st.borrow();
    let mut items: Vec<Entry> = apps::search(&st.apps, query).iter().take(14).map(Entry::app).collect();
    if items.is_empty() {
      items.push(Entry::disabled("Nada encontrado"));
    }
    items
  }

  // --- Columns ---------------------------------------------------------------

  fn screen(&self) -> (i32, i32) {
    let (w, h) = (self.fixed.width(), self.fixed.height());
    if w > 0 && h > 0 {
      return (w, h);
    }
    gdk::Display::default()
      .and_then(|d| d.monitors().item(0))
      .and_downcast::<gdk::Monitor>()
      .map(|m| (m.geometry().width(), m.geometry().height()))
      .unwrap_or((1280, 800))
  }

  fn icon_widget(&self, icon: &Icon, size: i32) -> Option<gtk::Widget> {
    let theme = gtk::IconTheme::for_display(&gdk::Display::default()?);
    let image = match icon {
      Icon::None => return None,
      Icon::App(Some(g)) => gtk::Image::from_gicon(g),
      Icon::App(None) => gtk::Image::from_icon_name("application-x-executable"),
      Icon::Named(name, fallback) => {
        let symbolic = [name.as_str(), fallback]
          .iter()
          .filter(|n| !n.is_empty() && !n.contains('/'))
          .map(|n| format!("{}-symbolic", n.trim_end_matches("-symbolic")))
          .find(|n| theme.has_icon(n));
        match symbolic {
          Some(s) => gtk::Image::from_icon_name(&s),
          None => {
            let image = gtk::Image::from_icon_name(name);
            image.add_css_class("gray");
            image
          }
        }
      }
      Icon::Emblem => {
        let path = format!("{}/emblem.png", doxia_theme::theme_dir().display());
        let area = gtk::DrawingArea::new();
        area.set_size_request(size, size);
        area.add_css_class("iniciar-emblem");
        let pixbuf = gtk::gdk_pixbuf::Pixbuf::from_file_at_size(&path, size * 2, size * 2).ok();
        area.set_draw_func(move |area, cr, w, h| {
          let Some(pixbuf) = &pixbuf else { return };
          let color = area.color();
          let scale = (w as f64 / pixbuf.width() as f64).min(h as f64 / pixbuf.height() as f64);
          cr.scale(scale, scale);
          cr.push_group();
          cr.set_source_pixbuf(pixbuf, 0.0, 0.0);
          let _ = cr.paint();
          let Ok(mask) = cr.pop_group() else { return };
          cr.set_source_rgba(color.red() as f64, color.green() as f64, color.blue() as f64, color.alpha() as f64);
          let _ = cr.mask(&mask);
        });
        return Some(area.upcast());
      }
    };
    image.set_pixel_size(size);
    Some(image.upcast())
  }

  fn build_row(self: &Rc<Self>, level: usize, index: usize, e: &Entry) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.set_height_request(row_height(level, e));
    let Entry::Item { label: text, icon, disabled, .. } = e else {
      row.add_css_class("iniciar-sep");
      let line = gtk::Box::new(gtk::Orientation::Vertical, 0);
      line.add_css_class("iniciar-sep-line");
      line.set_hexpand(true);
      line.set_valign(gtk::Align::Center);
      line.set_margin_start(4);
      line.set_margin_end(4);
      row.append(&line);
      return row;
    };
    row.add_css_class("iniciar-row");
    if level == 0 {
      row.add_css_class("root");
    }
    if *disabled {
      row.add_css_class("disabled");
    }
    let size = if level == 0 { 26 } else { 18 };
    let slot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    slot.set_size_request(size, size);
    slot.set_valign(gtk::Align::Center);
    slot.set_margin_start(if level == 0 { 8 } else { 5 });
    if let Some(w) = self.icon_widget(icon, size) {
      slot.append(&w);
    }
    row.append(&slot);
    let name = label(text, "iniciar-label");
    name.set_xalign(0.0);
    name.set_hexpand(true);
    name.set_margin_start(9);
    name.set_ellipsize(pango::EllipsizeMode::End);
    row.append(&name);
    let arrow = label(if matches!(e, Entry::Item { sub: Some(_), .. } | Entry::Item { mode: Some(Mode::Search), .. }) { "▶" } else { "" }, "iniciar-arrow");
    arrow.set_margin_end(6);
    arrow.set_margin_start(4);
    row.append(&arrow);

    if e.selectable() {
      let motion = gtk::EventControllerMotion::new();
      let me = Rc::downgrade(self);
      motion.connect_enter(move |_, _, _| {
        if let Some(me) = me.upgrade() {
          me.hovered(level, index);
        }
      });
      row.add_controller(motion);
      let click = gtk::GestureClick::new();
      let me = Rc::downgrade(self);
      click.connect_released(move |g, _, _, _| {
        g.set_state(gtk::EventSequenceState::Claimed);
        let Some(me) = me.upgrade() else { return };
        me.stop_hover();
        me.st.borrow_mut().focus = level;
        me.set_selection(level, Some(index));
        me.activate(level, index);
      });
      row.add_controller(click);
    }
    row
  }

  // The banner: the name lying along the root column, read bottom to top.
  fn banner() -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.add_css_class("iniciar-banner");
    area.set_size_request(BANNER, -1);
    area.set_vexpand(true);
    area.set_draw_func(|area, cr, w, h| {
      let layout = area.create_pango_layout(Some("DoxIA"));
      let mut font = pango::FontDescription::from_string("DoxIA Shell Bold");
      font.set_absolute_size((BANNER as f64 * 0.68).round() * pango::SCALE as f64);
      layout.set_font_description(Some(&font));
      let (_, th) = layout.pixel_size();
      cr.translate(((w - th) / 2) as f64, (h - 10) as f64);
      cr.rotate(-std::f64::consts::FRAC_PI_2);
      cr.set_source_rgb(1.0, 1.0, 1.0);
      pangocairo::functions::show_layout(cr, &layout);
    });
    area
  }

  fn push_level(self: &Rc<Self>, parent: Option<usize>, items: Vec<Entry>, mode: Mode) {
    let level = self.st.borrow().levels.len();
    if level >= MAX_LEVELS {
      return;
    }
    let (sw, sh) = self.screen();
    let width = column_width(level);
    let full = (content_height(level, &items, mode) + (BEVEL + PAD) * 2).min(sh - self.gap * 2);

    // Theme border, then the 2px relief: light top/left, dark bottom/right.
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.add_css_class("iniciar-column");
    column.set_size_request(width, full);
    let relief = gtk::Box::new(gtk::Orientation::Vertical, 0);
    relief.add_css_class("iniciar-relief");
    relief.set_vexpand(true);
    let inner = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    inner.add_css_class("iniciar-inner");
    inner.set_vexpand(true);
    relief.append(&inner);
    column.append(&relief);
    if level == 0 {
      inner.append(&Self::banner());
    }
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.set_hexpand(true);
    inner.append(&body);

    let mut field = None;
    if mode != Mode::Menu {
      let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
      header.set_height_request(header_height(mode));
      header.set_margin_start(6);
      header.set_margin_end(4);
      header.append(&label(if mode == Mode::Run { "Abrir:" } else { "Nome:" }, "iniciar-field-label"));
      let sunken = gtk::Box::new(gtk::Orientation::Horizontal, 0);
      sunken.add_css_class("iniciar-field");
      sunken.set_hexpand(true);
      sunken.set_valign(gtk::Align::Start);
      sunken.set_margin_top(3);
      sunken.set_height_request(ROW);
      let text = label("", "iniciar-field-text");
      text.set_xalign(0.0);
      text.set_hexpand(true);
      text.set_margin_start(6);
      text.set_ellipsize(pango::EllipsizeMode::Start);
      sunken.append(&text);
      header.append(&sunken);
      body.append(&header);
      field = Some(text);
      if mode == Mode::Run {
        let hint = label("Enter executa o comando", "iniciar-hint");
        hint.set_xalign(0.0);
        hint.set_height_request(ROW);
        hint.set_margin_start(8);
        body.append(&hint);
      }
    }

    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let rows: Vec<gtk::Box> = items.iter().enumerate().map(|(i, e)| self.build_row(level, i, e)).collect();
    for r in &rows {
      list.append(r);
    }
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::External);
    scroller.set_vexpand(true);
    scroller.set_child(Some(&list));
    body.append(&scroller);

    // Next to the column it came from, its first row level with the row
    // that opened it, kept on screen.
    let (x, y) = {
      let st = self.st.borrow();
      match (st.levels.last(), parent) {
        (Some(prev), Some(p)) => {
          let x = (prev.x + column_width(level - 1) - BEVEL * 2).min(sw - width);
          let scrolled = prev.scroller.vadjustment().value() as i32;
          let wanted = prev.y + header_height(prev.mode) - scrolled + row_y(level - 1, &prev.items, p);
          (x, wanted.clamp(self.gap, (sh - self.gap - full).max(self.gap)))
        }
        _ => (self.gap, self.gap),
      }
    };
    // Swallows clicks on the column's own edges and gaps.
    column.add_controller(gtk::GestureClick::new());
    self.fixed.put(&column, x as f64, y as f64);
    self.st.borrow_mut().levels.push(Level { items, parent, mode, selected: None, column, rows, scroller, field, x, y });
    self.paint_field();
  }

  fn clear_levels(&self) {
    let mut st = self.st.borrow_mut();
    for l in st.levels.drain(..) {
      self.fixed.remove(&l.column);
    }
  }

  // Drops the columns past `level`.
  fn truncate(&self, level: usize) {
    let mut st = self.st.borrow_mut();
    while st.levels.len() > level + 1 {
      let l = st.levels.pop().unwrap();
      self.fixed.remove(&l.column);
    }
    if st.focus > level {
      st.focus = level;
    }
    if level == 0 {
      st.typed.clear();
    }
  }

  fn paint(&self) {
    let st = self.st.borrow();
    for l in &st.levels {
      for (i, r) in l.rows.iter().enumerate() {
        if l.selected == Some(i) && l.items[i].selectable() {
          r.add_css_class("selected");
        } else {
          r.remove_css_class("selected");
        }
      }
      if let Some(i) = l.selected {
        if let (Some(r), Some(list)) = (l.rows.get(i), l.rows.first().and_then(|r| r.parent())) {
          if let Some(b) = r.compute_bounds(&list) {
            let adj = l.scroller.vadjustment();
            let (top, bottom) = (b.y() as f64, (b.y() + b.height()) as f64);
            if top < adj.value() {
              adj.set_value(top);
            } else if bottom > adj.value() + adj.page_size() {
              adj.set_value(bottom - adj.page_size());
            }
          }
        }
      }
    }
  }

  fn paint_field(&self) {
    let st = self.st.borrow();
    for (i, l) in st.levels.iter().enumerate() {
      if let Some(f) = &l.field {
        f.set_text(&format!("{}{}", st.typed, if st.focus == i { "▏" } else { "" }));
      }
    }
  }

  // Highlights `index` in `level`; the rows that led there stay lit.
  fn set_selection(&self, level: usize, index: Option<usize>) {
    {
      let mut st = self.st.borrow_mut();
      if let Some(l) = st.levels.get_mut(level) {
        l.selected = index;
      }
      for l in (1..=level.min(st.levels.len().saturating_sub(1))).rev() {
        let parent = st.levels[l].parent;
        st.levels[l - 1].selected = parent;
      }
    }
    self.paint();
  }

  // Opens the row's submenu (or Localizar/Executar); from the keyboard the
  // first row of it is highlighted too.
  fn expand(self: &Rc<Self>, level: usize, index: usize, keyboard: bool) -> bool {
    let entry = self.st.borrow().levels.get(level).and_then(|l| l.items.get(index).cloned());
    let Some(Entry::Item { sub, mode, disabled: false, .. }) = entry else { return false };
    let (items, mode) = match (sub, mode) {
      (Some(sub), _) => {
        let mut items = self.sub_items(&sub);
        if items.is_empty() {
          items.push(Entry::disabled("(Vazio)"));
        }
        (items, Mode::Menu)
      }
      (None, Some(Mode::Search)) => (self.search_items(""), Mode::Search),
      (None, Some(Mode::Run)) => (Vec::new(), Mode::Run),
      _ => {
        self.truncate(level);
        return false;
      }
    };
    self.truncate(level);
    if mode != Mode::Menu {
      self.st.borrow_mut().typed.clear();
    }
    self.push_level(Some(index), items, mode);
    if keyboard {
      self.st.borrow_mut().focus = level + 1;
      self.select_first(level + 1);
    }
    self.paint();
    self.paint_field();
    true
  }

  fn select_first(&self, level: usize) {
    let first = self.st.borrow().levels.get(level).and_then(|l| l.items.iter().position(Entry::selectable));
    self.set_selection(level, first);
  }

  fn activate(self: &Rc<Self>, level: usize, index: usize) {
    let entry = self.st.borrow().levels.get(level).and_then(|l| l.items.get(index).cloned());
    let Some(e) = entry.filter(Entry::selectable) else { return };
    if e.opens() {
      self.expand(level, index, true);
      return;
    }
    let Entry::Item { act: Some(act), .. } = e else { return };
    self.dismiss();
    match act {
      Act::Shell(c) => crate::run::shell(&c),
      Act::App(id) => apps::launch(&id),
      Act::Open(href) => crate::run::argv(&["xdg-open", &href]),
    }
  }

  fn hovered(self: &Rc<Self>, level: usize, index: usize) {
    self.st.borrow_mut().focus = level;
    self.set_selection(level, Some(index));
    self.stop_hover();
    let me = Rc::downgrade(self);
    let id = glib::timeout_add_local_once(Duration::from_millis(HOVER_MS), move || {
      if let Some(me) = me.upgrade() {
        me.st.borrow_mut().hover = None;
        me.expand(level, index, false);
      }
    });
    self.st.borrow_mut().hover = Some(id);
  }

  fn stop_hover(&self) {
    if let Some(id) = self.st.borrow_mut().hover.take() {
      id.remove();
    }
  }

  fn move_selection(&self, delta: i64) {
    let (level, next) = {
      let st = self.st.borrow();
      let level = st.focus;
      let Some(l) = st.levels.get(level) else { return };
      let n = l.items.len() as i64;
      if n == 0 {
        return;
      }
      let mut index = l.selected.map(|i| i as i64);
      for _ in 0..n {
        let i = match index {
          None => if delta > 0 { 0 } else { n - 1 },
          Some(i) => (i + delta).rem_euclid(n),
        };
        index = Some(i);
        if l.items[i as usize].selectable() {
          break;
        }
      }
      (level, index.map(|i| i as usize))
    };
    self.set_selection(level, next);
    self.truncate(level);
    self.paint_field();
  }

  fn set_typed(self: &Rc<Self>, value: String) {
    let deepest = self.st.borrow().levels.len() - 1;
    if self.st.borrow().levels[deepest].mode == Mode::Search {
      // Rebuilt in place: same parent, new rows (truncating to the root
      // clears the text, so it goes in after).
      let parent = self.st.borrow().levels[deepest].parent;
      self.truncate(deepest - 1);
      let items = self.search_items(&value);
      self.push_level(parent, items, Mode::Search);
      self.select_first(deepest);
    }
    {
      let mut st = self.st.borrow_mut();
      st.typed = value;
      st.focus = deepest;
    }
    self.paint_field();
  }

  // Typing outside Executar opens Localizar with the text.
  fn start_search(self: &Rc<Self>, text: &str) {
    let index = self.st.borrow().levels[0].items.iter().position(|e| matches!(e, Entry::Item { mode: Some(Mode::Search), .. }));
    let Some(index) = index else { return };
    self.set_selection(0, Some(index));
    self.expand(0, index, true);
    self.set_typed(text.to_string());
  }

  fn deepest_mode(&self) -> Mode {
    self.st.borrow().levels.last().map(|l| l.mode).unwrap_or(Mode::Menu)
  }

  // --- Input -----------------------------------------------------------------

  fn connect_input(self: &Rc<Self>) {
    let keys = gtk::EventControllerKey::new();
    self.im.set_client_widget(Some(&self.window));
    keys.set_im_context(Some(&self.im));
    let me = Rc::downgrade(self);
    self.im.connect_commit(move |_, text| {
      let Some(me) = me.upgrade() else { return };
      if !me.st.borrow().opened {
        return;
      }
      let text: String = text.chars().filter(|c| !c.is_control()).collect();
      if text.is_empty() {
        return;
      }
      if me.deepest_mode() == Mode::Menu {
        me.start_search(&text);
      } else {
        let typed = me.st.borrow().typed.clone();
        me.set_typed(typed + &text);
      }
    });
    let me = Rc::downgrade(self);
    keys.connect_key_pressed(move |_, key, _, state| {
      let Some(me) = me.upgrade() else { return glib::Propagation::Proceed };
      if me.key(key, state) { glib::Propagation::Stop } else { glib::Propagation::Proceed }
    });
    self.window.add_controller(keys);

    // A click outside every column closes the menu.
    let click = gtk::GestureClick::new();
    let me = Rc::downgrade(self);
    click.connect_released(move |_, _, x, y| {
      let Some(me) = me.upgrade() else { return };
      let point = graphene::Point::new(x as f32, y as f32);
      let inside = me.st.borrow().levels.iter().any(|l| l.column.compute_bounds(&me.fixed).is_some_and(|b| b.contains_point(&point)));
      if !inside {
        me.dismiss();
      }
    });
    self.fixed.add_controller(click);
  }

  fn key(self: &Rc<Self>, key: gdk::Key, state: gdk::ModifierType) -> bool {
    use gdk::Key;
    let mode = self.deepest_mode();
    let typing = mode != Mode::Menu;
    let typed = self.st.borrow().typed.clone();
    let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
    let other = state.intersects(gdk::ModifierType::ALT_MASK | gdk::ModifierType::SUPER_MASK | gdk::ModifierType::META_MASK);
    match key {
      Key::Escape => {
        let depth = self.st.borrow().levels.len();
        if typing && !typed.is_empty() {
          self.set_typed(String::new());
        } else if depth > 1 {
          self.truncate(depth - 2);
          self.st.borrow_mut().focus = depth - 2;
          self.paint();
          self.paint_field();
        } else {
          self.dismiss();
        }
      }
      Key::BackSpace if typing && !typed.is_empty() && !other => {
        let next = if ctrl {
          typed.trim_end().trim_end_matches(|c: char| !c.is_whitespace()).to_string()
        } else {
          let mut t = typed;
          t.pop();
          t
        };
        self.set_typed(next);
      }
      Key::u | Key::U if typing && ctrl && !other && !typed.is_empty() => self.set_typed(String::new()),
      Key::Up => self.move_selection(-1),
      Key::Down => self.move_selection(1),
      Key::Right => {
        let (level, sel, opens) = {
          let st = self.st.borrow();
          let l = &st.levels[st.focus];
          (st.focus, l.selected, l.selected.and_then(|i| l.items.get(i)).is_some_and(Entry::opens))
        };
        if let (true, Some(i)) = (opens, sel) {
          self.expand(level, i, true);
        }
      }
      Key::Left => {
        let focus = self.st.borrow().focus;
        if focus > 0 {
          self.truncate(focus - 1);
          self.st.borrow_mut().focus = focus - 1;
          self.paint();
          self.paint_field();
        }
      }
      Key::Return | Key::KP_Enter => {
        if mode == Mode::Run {
          let command = typed.trim().to_string();
          if !command.is_empty() {
            self.dismiss();
            crate::run::argv(&["uwsm-app", "--", "bash", "-lc", &command]);
          }
        } else {
          let (level, sel) = {
            let st = self.st.borrow();
            (st.focus, st.levels.get(st.focus).and_then(|l| l.selected))
          };
          if let Some(i) = sel {
            self.activate(level, i);
          }
        }
      }
      _ => return false,
    }
    true
  }
}
