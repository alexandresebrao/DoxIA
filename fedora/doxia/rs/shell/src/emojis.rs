// The emoji picker, replacing shell/plugins/emojis (Emojis.qml): a grid of
// the emojis in shell/plugins/emojis/emojis.json, filtered by their keywords
// as you type, inserted with omarchy-menu-emoji-insert. Opened with
// `omarchy-shell shell toggle omarchy.emojis` (omarchy-menu-emoji).

use crate::popup::{self, label};
use gtk::prelude::*;
use gtk::gdk;
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;

const WIDTH: i32 = 400;
const HEIGHT: i32 = 500;
const MARGIN: i32 = 18 + 2;
const HEADER: i32 = 34;
const SPACING: i32 = 6;
const CELL: i32 = 44;
const LIMIT: usize = 1000;

#[derive(Clone)]
pub struct Emoji {
  pub glyph: String,
  pub keywords: String,
}

pub fn parse(raw: &str) -> Vec<Emoji> {
  let Ok(Value::Array(items)) = serde_json::from_str::<Value>(raw) else { return Vec::new() };
  items
    .iter()
    .filter_map(|i| {
      let glyph = i.get("e")?.as_str()?.to_string();
      (!glyph.is_empty()).then(|| Emoji { glyph, keywords: i.get("k").and_then(Value::as_str).unwrap_or("").to_lowercase() })
    })
    .collect()
}

// EmojiSearch.filterEmojis: the query anywhere in the keywords, in order.
pub fn filter(all: &[Emoji], query: &str, limit: usize) -> Vec<Emoji> {
  let needle = query.trim().to_lowercase();
  all.iter().filter(|e| needle.is_empty() || e.keywords.contains(&needle)).take(limit).cloned().collect()
}

#[derive(Default)]
struct State {
  opened: bool,
  all: Vec<Emoji>,
  shown: Vec<Emoji>,
  filter: String,
}

pub struct Emojis {
  window: gtk::Window,
  root: gtk::Box,
  card: Rc<popup::Card>,
  header: gtk::Label,
  model: gtk::StringList,
  selection: gtk::SingleSelection,
  grid: gtk::GridView,
  empty: gtk::Box,
  empty_label: gtk::Label,
  im: gtk::IMMulticontext,
  st: RefCell<State>,
}

impl Emojis {
  pub fn new(app: &gtk::Application) -> Rc<Emojis> {
    let window = popup::window(app, "omarchy-emojis", "emojis-window");
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let content = gtk::Box::new(gtk::Orientation::Vertical, SPACING);
    let header = label("", "menu-header");
    header.set_xalign(0.0);
    header.set_ellipsize(gtk::pango::EllipsizeMode::End);
    header.set_height_request(HEADER);
    content.append(&header);

    let model = gtk::StringList::new(&[]);
    let selection = gtk::SingleSelection::new(Some(model.clone()));
    selection.set_autoselect(false);
    selection.set_can_unselect(true);
    let factory = gtk::SignalListItemFactory::new();
    let grid = gtk::GridView::new(Some(selection.clone()), Some(factory.clone()));
    grid.add_css_class("emoji-grid");
    let columns = ((WIDTH - MARGIN * 2) / CELL).max(1) as u32;
    grid.set_min_columns(columns);
    grid.set_max_columns(columns);
    // The window takes every key; the grid only shows.
    grid.set_focusable(false);
    grid.set_can_focus(false);
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::External);
    scroller.set_vexpand(true);
    scroller.set_child(Some(&grid));
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
    stack.set_vexpand(true);
    content.append(&stack);

    let card = Rc::new(popup::Card::new(&content, MARGIN));
    card.overlay.set_halign(gtk::Align::Center);
    card.overlay.set_valign(gtk::Align::Center);
    card.overlay.set_vexpand(true);
    root.append(&card.overlay);
    window.set_child(Some(&root));

    let me = Rc::new(Emojis {
      window,
      root,
      card,
      header,
      model,
      selection,
      grid,
      empty,
      empty_label,
      im: gtk::IMMulticontext::new(),
      st: RefCell::default(),
    });

    let weak = Rc::downgrade(&me);
    factory.connect_setup(move |_, item| {
      let item = item.downcast_ref::<gtk::ListItem>().unwrap();
      let cell = label("", "emoji-cell");
      cell.set_size_request(CELL, CELL);
      // The pointer picks the cell it is over, as the QML's hover does.
      let motion = gtk::EventControllerMotion::new();
      let (w, i) = (weak.clone(), item.downgrade());
      motion.connect_enter(move |_, _, _| {
        if let (Some(me), Some(item)) = (w.upgrade(), i.upgrade()) {
          me.selection.set_selected(item.position());
        }
      });
      cell.add_controller(motion);
      let click = gtk::GestureClick::new();
      let (w, i) = (weak.clone(), item.downgrade());
      click.connect_released(move |_, _, _, _| {
        if let (Some(me), Some(item)) = (w.upgrade(), i.upgrade()) {
          me.activate(item.position());
        }
      });
      cell.add_controller(click);
      item.set_child(Some(&cell));
    });
    factory.connect_bind(|_, item| {
      let item = item.downcast_ref::<gtk::ListItem>().unwrap();
      let text = item.item().and_downcast::<gtk::StringObject>().map(|s| s.string().to_string()).unwrap_or_default();
      if let Some(cell) = item.child().and_downcast::<gtk::Label>() {
        cell.set_text(&text);
      }
    });

    let path = format!("{}/shell/plugins/emojis/emojis.json", std::env::var("OMARCHY_PATH").unwrap_or_default());
    me.st.borrow_mut().all = parse(&std::fs::read_to_string(path).unwrap_or_default());
    me.connect_input();
    me
  }

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
    let (w, h) = screen();
    self.card.overlay.set_size_request(WIDTH.min(w - 2 * crate::menu::gaps_out()), HEIGHT.min(h - 2 * crate::menu::gaps_out()));
    self.st.borrow_mut().opened = true;
    self.set_filter(String::new());
    self.window.set_visible(true);
    self.im.focus_in();
  }

  fn close(&self) {
    self.st.borrow_mut().opened = false;
    self.im.focus_out();
    self.window.set_visible(false);
  }

  fn set_filter(&self, filter: String) {
    let shown = {
      let mut st = self.st.borrow_mut();
      st.shown = self::filter(&st.all, &filter, LIMIT);
      st.filter = filter.clone();
      st.shown.clone()
    };
    let glyphs: Vec<&str> = shown.iter().map(|e| e.glyph.as_str()).collect();
    self.model.splice(0, self.model.n_items(), &glyphs);
    self.header.set_text(if filter.is_empty() { "Search emojis…" } else { &filter });
    if filter.is_empty() { self.header.add_css_class("placeholder") } else { self.header.remove_css_class("placeholder") }
    self.empty.set_visible(shown.is_empty());
    self.empty_label.set_text(&format!("No matches for “{filter}”"));
    if !shown.is_empty() {
      self.select(0);
    }
  }

  fn select(&self, index: u32) {
    let count = self.model.n_items();
    if count == 0 {
      return;
    }
    let index = index.min(count - 1);
    self.selection.set_selected(index);
    self.grid.scroll_to(index, gtk::ListScrollFlags::NONE, None);
  }

  fn activate(&self, index: u32) {
    let glyph = self.st.borrow().shown.get(index as usize).map(|e| e.glyph.clone());
    let Some(glyph) = glyph else { return };
    self.close();
    let insert = format!("{}/bin/omarchy-menu-emoji-insert", std::env::var("OMARCHY_PATH").unwrap_or_default());
    crate::run::argv(&[&insert, &glyph]);
  }

  fn connect_input(self: &Rc<Self>) {
    let me = Rc::downgrade(self);
    let typed = Rc::downgrade(self);
    popup::connect_keys(
      &self.window,
      &self.im,
      move |key, state| me.upgrade().is_some_and(|me| me.key(key, state)),
      move |text| {
        if let Some(me) = typed.upgrade() {
          let filter = me.st.borrow().filter.clone();
          me.set_filter(filter + text);
        }
      },
    );
    let me = Rc::downgrade(self);
    popup::close_on_outside_click(&self.root, self.card.clone(), move || {
      if let Some(me) = me.upgrade() {
        me.close();
      }
    });
  }

  fn key(&self, key: gdk::Key, state: gdk::ModifierType) -> bool {
    use gdk::Key;
    let filter = self.st.borrow().filter.clone();
    if let Some(next) = popup::edit_filter(&filter, key, state) {
      self.set_filter(next);
      return true;
    }
    let count = self.model.n_items() as i64;
    let columns = self.grid.max_columns() as i64;
    let current = match self.selection.selected() {
      gtk::INVALID_LIST_POSITION => None,
      n => Some(n as i64),
    };
    // Left/Right wrap around; Up/Down and the page keys stop at the ends.
    let rows_visible = ((self.grid.height() / CELL).max(1)) as i64;
    let next = |delta: i64, wrap: bool| -> u32 {
      match current {
        None => if delta < 0 { (count - 1) as u32 } else { 0 },
        Some(i) if wrap => (i + delta).rem_euclid(count) as u32,
        Some(i) => (i + delta).clamp(0, count - 1) as u32,
      }
    };
    match key {
      Key::Escape => {
        if filter.is_empty() { self.close() } else { self.set_filter(String::new()) }
      }
      _ if count == 0 => return matches!(key, Key::Left | Key::Right | Key::Up | Key::Down | Key::Return | Key::KP_Enter),
      Key::Left => self.select(next(-1, true)),
      Key::Right => self.select(next(1, true)),
      Key::Up => self.select(next(-columns, false)),
      Key::Down => self.select(next(columns, false)),
      Key::Page_Up => self.select(next(-columns * rows_visible, false)),
      Key::Page_Down => self.select(next(columns * rows_visible, false)),
      Key::Return | Key::KP_Enter => match current {
        Some(i) => self.activate(i as u32),
        None => self.select(0),
      },
      _ => return false,
    }
    true
  }
}

pub fn screen() -> (i32, i32) {
  gdk::Display::default()
    .and_then(|d| d.monitors().item(0))
    .and_downcast::<gdk::Monitor>()
    .map(|m| (m.geometry().width(), m.geometry().height()))
    .unwrap_or((1280, 800))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn filters_by_keywords_in_order() {
    let all = parse(r#"[{"e":"😀","k":"grinning face smile"},{"e":"😃","k":"Smiley Happy"},{"e":"","k":"x"}]"#);
    assert_eq!(all.len(), 2);
    let glyphs = |q: &str| filter(&all, q, 10).into_iter().map(|e| e.glyph).collect::<Vec<_>>();
    assert_eq!(glyphs("smil"), ["😀", "😃"]);
    assert_eq!(glyphs(" HAPPY "), ["😃"]);
    assert_eq!(filter(&all, "", 1).len(), 1);
  }
}
