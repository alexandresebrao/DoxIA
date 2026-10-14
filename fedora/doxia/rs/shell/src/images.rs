// The image picker, replacing shell/plugins/image-picker (ImagePicker.qml):
// the carousel omarchy-menu-images opens for the theme, background and boot
// screen switchers. Skewed slices either side of the selected image, which
// opens wide, its name under it, a filter for the long lists. All drawn with
// cairo; thumbnails decode off the main thread and stay cached.
//
// IPC as the shell's `image-selector` target: `open <dirs> <rows-base64>
// <selected> <selection-file> <done-file> <labels> <filterable>`, `preload
// <rows-base64> <selected> <labels> <filterable>` and `cancel <done-file>`;
// and the plugin surface (`shell summon|hide omarchy.image-picker <json>`).

use crate::popup;
use gtk::prelude::*;
use gtk::{cairo, gdk, gdk_pixbuf::Pixbuf, gio, glib, pango};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// ImagePicker.qml's geometry.
const EXPANDED_W: f64 = 768.0;
const EXPANDED_H: f64 = 475.0;
const SLICE_W: f64 = 108.0;
const SLICE_H: f64 = 432.0;
const SLICE_SPACING: f64 = -30.0;
const SKEW: f64 = 28.0;
const TOP: f64 = 30.0;
const NEARBY: i64 = 16;
// Decoded this wide: enough for the open slice.
const DECODE_W: i32 = 800;

#[derive(Clone, Debug, PartialEq)]
pub struct Image {
  pub path: String,
  pub thumbnail: String,
}

// "<path>\t<thumbnail>" rows, one image per file name.
pub fn load_rows(rows: &str) -> Vec<Image> {
  let mut seen = std::collections::HashSet::new();
  rows
    .split('\n')
    .filter_map(|row| {
      let mut cols = row.split('\t');
      let path = cols.next().filter(|p| !p.is_empty())?;
      let name = path.rsplit('/').next().unwrap_or(path).to_string();
      seen.insert(name).then(|| Image {
        path: path.into(),
        thumbnail: cols.next().filter(|t| !t.is_empty()).unwrap_or(path).into(),
      })
    })
    .collect()
}

pub fn name_for(path: &str) -> String {
  let file = path.rsplit('/').next().unwrap_or(path);
  match file.rfind('.') {
    Some(dot) if dot > 0 => file[..dot].into(),
    _ => file.into(),
  }
}

// "tokyo-night_dark" -> "Tokyo Night Dark".
pub fn label_for(path: &str) -> String {
  let spaced: String = name_for(path).chars().map(|c| if c == '-' || c == '_' { ' ' } else { c }).collect();
  let mut out = String::new();
  let mut start = true;
  for c in spaced.split_whitespace().collect::<Vec<_>>().join(" ").chars() {
    if start && c.is_alphanumeric() {
      out.extend(c.to_uppercase());
    } else {
      out.push(c);
    }
    start = !c.is_alphanumeric() && c != '_';
  }
  out
}

fn matches(img: &Image, filter: &str) -> bool {
  let needle = filter.to_lowercase();
  needle.is_empty() || name_for(&img.path).to_lowercase().contains(&needle) || label_for(&img.path).to_lowercase().contains(&needle)
}

#[derive(Default)]
struct State {
  opened: bool,
  images: Vec<Image>,
  loaded_rows: String,
  selected: usize,
  selected_image: String,
  selection_file: String,
  done_file: String,
  labels: bool,
  filterable: bool,
  filter: String,
  serial: u64,
  // Decoded thumbnails; None while one decodes.
  cache: HashMap<String, Option<Pixbuf>>,
}

impl State {
  fn matched(&self, i: usize) -> bool {
    self.images.get(i).is_some_and(|img| matches(img, &self.filter))
  }

  // Where `i` falls among the images the filter keeps.
  fn position(&self, i: usize) -> i64 {
    (0..i).filter(|&j| self.matched(j)).count() as i64
  }

  fn bottom_chrome(&self) -> f64 {
    match (self.labels, self.filterable) {
      (true, true) => 104.0,
      (true, false) => 74.0,
      (false, true) => 60.0,
      (false, false) => 30.0,
    }
  }
}

pub struct Images {
  window: gtk::Window,
  area: gtk::DrawingArea,
  im: gtk::IMMulticontext,
  st: RefCell<State>,
}

fn path_slice(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64) {
  cr.move_to(x + SKEW, y);
  cr.line_to(x + w, y);
  cr.line_to(x + w - SKEW, y + h);
  cr.line_to(x, y + h);
  cr.close_path();
}

impl Images {
  pub fn new(app: &gtk::Application) -> Rc<Images> {
    let window = popup::window(app, "doxia-image-selector", "images-window");
    let area = gtk::DrawingArea::new();
    area.set_hexpand(true);
    area.set_vexpand(true);
    window.set_child(Some(&area));
    let me = Rc::new(Images { window, area, im: gtk::IMMulticontext::new(), st: RefCell::default() });
    let weak = Rc::downgrade(&me);
    me.area.set_draw_func(move |area, cr, w, h| {
      if let Some(me) = weak.upgrade() {
        me.draw(area, cr, w as f64, h as f64);
      }
    });
    me.connect_input();
    me
  }

  // --- IPC -------------------------------------------------------------------

  // The `image-selector` target.
  pub fn selector_ipc(self: &Rc<Self>, method: &str, args: &[String]) -> Option<String> {
    let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
    let decode = |s: &str| String::from_utf8_lossy(&glib::base64_decode(s)).into_owned();
    match method {
      "open" => self.open(&arg(0), &decode(&arg(1)), &arg(2), &arg(3), &arg(4), arg(5) == "true", arg(6) == "true"),
      "preload" => self.preload(&decode(&arg(0)), &arg(1), arg(2) == "true", arg(3) == "true"),
      "cancel" => self.close_selector(&arg(0)),
      _ => return None,
    }
    Some("ok".into())
  }

  // `shell summon|toggle|hide omarchy.image-picker [json]`.
  pub fn plugin_ipc(self: &Rc<Self>, method: &str, args: &[String]) -> Option<String> {
    let p: serde_json::Value = serde_json::from_str(args.first().map(String::as_str).unwrap_or("{}")).unwrap_or_default();
    let s = |k: &str| match p.get(k) {
      Some(serde_json::Value::String(v)) => v.clone(),
      Some(v) if !v.is_null() => v.to_string(),
      _ => String::new(),
    };
    let summon = || self.open(&s("imageDirs"), &s("imageRows"), &s("selectedImage"), &s("selectionFile"), &s("doneFile"), s("showLabels") == "true", s("filterable") == "true");
    match method {
      "summon" => summon(),
      "toggle" if self.st.borrow().opened => self.cancel(),
      "toggle" => summon(),
      "hide" => self.cancel(),
      "call" => {}
      _ => return None,
    }
    Some("ok".into())
  }

  fn touch(path: &str) {
    if !path.is_empty() {
      let _ = std::fs::write(path, "");
    }
  }

  #[allow(clippy::too_many_arguments)]
  fn open(self: &Rc<Self>, dirs: &str, rows: &str, selected: &str, selection_file: &str, done_file: &str, labels: bool, filterable: bool) {
    let serial = {
      let mut st = self.st.borrow_mut();
      // A request still waiting is let go first.
      if !st.done_file.is_empty() && st.done_file != done_file {
        Self::touch(&st.done_file);
      }
      st.serial += 1;
      st.selected_image = selected.into();
      st.selection_file = selection_file.into();
      st.done_file = done_file.into();
      st.labels = labels;
      st.filterable = filterable;
      st.filter.clear();
      st.serial
    };
    if !rows.is_empty() {
      self.load(rows);
      self.show();
      return;
    }
    // Only directories: list them (with cached thumbnails) first.
    let script = format!("{}/shell/plugins/image-picker/list.sh", std::env::var("OMARCHY_PATH").unwrap_or_default());
    let dirs = dirs.to_string();
    let me = self.clone();
    glib::spawn_future_local(async move {
      let Ok(p) = gio::Subprocess::newv(&[std::ffi::OsStr::new(&script), std::ffi::OsStr::new(&dirs)], gio::SubprocessFlags::STDOUT_PIPE)
      else {
        return;
      };
      let out = p.communicate_utf8_future(None).await.ok().and_then(|(o, _)| o).map(|o| o.to_string()).unwrap_or_default();
      if me.st.borrow().serial == serial {
        me.load(&out);
        me.show();
      }
    });
  }

  // Theme and background hooks warm the next open; never while one shows.
  fn preload(self: &Rc<Self>, rows: &str, selected: &str, labels: bool, filterable: bool) {
    {
      let mut st = self.st.borrow_mut();
      if st.opened || !st.done_file.is_empty() {
        return;
      }
      st.selected_image = selected.into();
      st.labels = labels;
      st.filterable = filterable;
    }
    self.load(rows);
    self.decode_nearby();
  }

  fn load(&self, rows: &str) {
    let mut st = self.st.borrow_mut();
    if rows != st.loaded_rows || st.images.is_empty() {
      st.images = load_rows(rows);
      st.loaded_rows = rows.into();
    }
    st.selected = st.images.iter().position(|i| i.path == st.selected_image).unwrap_or(0);
  }

  fn show(self: &Rc<Self>) {
    if self.st.borrow().images.is_empty() {
      self.cancel();
      return;
    }
    self.st.borrow_mut().opened = true;
    self.decode_nearby();
    self.area.queue_draw();
    self.window.set_visible(true);
    self.im.focus_in();
  }

  fn hide(&self) {
    let mut st = self.st.borrow_mut();
    st.opened = false;
    st.filter.clear();
    drop(st);
    self.im.focus_out();
    self.window.set_visible(false);
  }

  fn cancel(&self) {
    let done = {
      let mut st = self.st.borrow_mut();
      st.selection_file.clear();
      std::mem::take(&mut st.done_file)
    };
    Self::touch(&done);
    self.hide();
  }

  fn close_selector(&self, done_file: &str) {
    self.st.borrow_mut().serial += 1;
    let current = self.st.borrow().done_file.clone();
    self.cancel();
    if !done_file.is_empty() && done_file != current {
      Self::touch(done_file);
    }
  }

  fn apply(&self) {
    let (path, selection_file, done_file) = {
      let mut st = self.st.borrow_mut();
      let path = if st.matched(st.selected) { st.images[st.selected].path.clone() } else { String::new() };
      (path, std::mem::take(&mut st.selection_file), std::mem::take(&mut st.done_file))
    };
    if path.is_empty() || selection_file.is_empty() {
      Self::touch(&done_file);
      self.hide();
      return;
    }
    let _ = std::fs::write(&selection_file, format!("{path}\n"));
    Self::touch(&done_file);
    self.hide();
  }

  // --- Selection -------------------------------------------------------------

  fn select(self: &Rc<Self>, index: usize) {
    {
      let mut st = self.st.borrow_mut();
      if !st.matched(index) {
        return;
      }
      st.selected = index;
    }
    self.decode_nearby();
    self.area.queue_draw();
  }

  fn step(self: &Rc<Self>, direction: i64) {
    let next = {
      let st = self.st.borrow();
      let n = st.images.len() as i64;
      (1..=n).map(|k| (st.selected as i64 + direction * k).rem_euclid(n) as usize).find(|&i| st.matched(i))
    };
    if let Some(i) = next {
      self.select(i);
    }
  }

  fn set_filter(self: &Rc<Self>, filter: String) {
    {
      let mut st = self.st.borrow_mut();
      st.filter = filter;
      if !st.matched(st.selected) {
        if let Some(first) = (0..st.images.len()).find(|&i| st.matched(i)) {
          st.selected = first;
        }
      }
    }
    self.decode_nearby();
    self.area.queue_draw();
  }

  // Thumbnails within reach of the selection decode in the background.
  fn decode_nearby(self: &Rc<Self>) {
    let wanted: Vec<String> = {
      let st = self.st.borrow();
      let sel = st.position(st.selected);
      (0..st.images.len())
        .filter(|&i| st.matched(i) && (st.position(i) - sel).abs() <= NEARBY)
        .map(|i| st.images[i].thumbnail.clone())
        .filter(|t| !st.cache.contains_key(t))
        .collect()
    };
    for path in wanted {
      self.st.borrow_mut().cache.insert(path.clone(), None);
      let me = Rc::downgrade(self);
      glib::spawn_future_local(async move {
        // gdk-pixbuf's async loader decodes on a worker thread.
        let decoded = match gio::File::for_path(&path).read_future(glib::Priority::DEFAULT).await {
          Ok(stream) => Pixbuf::from_stream_at_scale_future(&stream, DECODE_W, -1, true).await.ok(),
          Err(_) => None,
        };
        let Some(me) = me.upgrade() else { return };
        match decoded {
          Some(pixbuf) => {
            me.st.borrow_mut().cache.insert(path, Some(pixbuf));
            me.area.queue_draw();
          }
          // A failed decode tries again next time it comes near.
          None => {
            me.st.borrow_mut().cache.remove(&path);
          }
        }
      });
    }
  }

  // --- Drawing ---------------------------------------------------------------

  fn card_rect(&self, w: f64, h: f64) -> (f64, f64, f64, f64) {
    let st = self.st.borrow();
    let cw = (w - 80.0).min(EXPANDED_W + 13.0 * (SLICE_W + SLICE_SPACING) + 40.0);
    let ch = EXPANDED_H + TOP + st.bottom_chrome();
    ((w - cw) / 2.0, (h - ch) / 2.0, cw, ch)
  }

  // Each visible slice: (index, x, y, width, height, selected), far ones
  // first so the nearer ones (and the open one last) land on top.
  fn slices(&self, w: f64, h: f64) -> Vec<(usize, f64, f64, f64, f64, bool)> {
    let (_, top, _, _) = self.card_rect(w, h);
    let st = self.st.borrow();
    let step = SLICE_W + SLICE_SPACING;
    let carousel_w = EXPANDED_W + 13.0 * step;
    let preview_x = (w - carousel_w) / 2.0 + (carousel_w - EXPANDED_W) / 2.0;
    let sel = if st.matched(st.selected) { st.position(st.selected) } else { 0 };
    let mut out: Vec<(i64, (usize, f64, f64, f64, f64, bool))> = Vec::new();
    for i in 0..st.images.len() {
      if !st.matched(i) {
        continue;
      }
      let rel = st.position(i) - sel;
      if rel.abs() > NEARBY {
        continue;
      }
      let selected = i == st.selected;
      let x = if selected {
        preview_x
      } else if rel < 0 {
        preview_x + rel as f64 * step
      } else {
        preview_x + EXPANDED_W + SLICE_SPACING + (rel - 1) as f64 * step
      };
      let y = top + TOP + if selected { 0.0 } else { (EXPANDED_H - SLICE_H) / 2.0 };
      let (sw, sh) = if selected { (EXPANDED_W, EXPANDED_H) } else { (SLICE_W, SLICE_H) };
      out.push((if selected { 1000 } else { -rel.abs() }, (i, x, y, sw, sh, selected)));
    }
    out.sort_by_key(|(z, _)| *z);
    out.into_iter().map(|(_, s)| s).collect()
  }

  fn draw(&self, area: &gtk::DrawingArea, cr: &cairo::Context, w: f64, h: f64) {
    let colors = doxia_theme::image_picker();
    let rgba = |cr: &cairo::Context, c: [f64; 4], a: f64| cr.set_source_rgba(c[0], c[1], c[2], c[3] * a);
    rgba(cr, colors.scrim, 1.0);
    let _ = cr.paint();
    let slices = self.slices(w, h);
    let st = self.st.borrow();
    for &(i, x, y, sw, sh, selected) in &slices {
      let img = &st.images[i];
      cr.save().ok();
      path_slice(cr, x, y, sw, sh);
      cr.clip();
      match st.cache.get(&img.thumbnail) {
        Some(Some(pixbuf)) => {
          // Cover the slice, centered, like PreserveAspectCrop.
          let (pw, ph) = (pixbuf.width() as f64, pixbuf.height() as f64);
          let scale = (sw / pw).max(sh / ph);
          cr.translate(x + (sw - pw * scale) / 2.0, y + (sh - ph * scale) / 2.0);
          cr.scale(scale, scale);
          cr.set_source_pixbuf(pixbuf, 0.0, 0.0);
          let _ = cr.paint();
        }
        _ => {
          rgba(cr, colors.background, 1.0);
          let _ = cr.paint();
        }
      }
      cr.restore().ok();
      if !selected {
        path_slice(cr, x, y, sw, sh);
        rgba(cr, colors.background, 0.42);
        let _ = cr.fill();
      }
      path_slice(cr, x, y, sw, sh);
      if selected {
        rgba(cr, colors.selected_border, 1.0);
        cr.set_line_width(3.0);
      } else {
        rgba(cr, colors.unselected_border, 1.0);
        cr.set_line_width(1.0);
      }
      let _ = cr.stroke();
    }

    // The name under the carousel, and what was typed under that.
    let (_, top, _, _) = self.card_rect(w, h);
    let mut y = top + TOP + EXPANDED_H + 16.0;
    if st.labels {
      let text = if st.matched(st.selected) {
        label_for(&st.images[st.selected].path)
      } else if !st.filter.is_empty() {
        "No matches".into()
      } else {
        String::new()
      };
      y += self.outlined(area, cr, &text, 24.0, true, w / 2.0, y, &colors, 1.0);
      y += 8.0;
    }
    if st.filterable && !st.filter.is_empty() {
      self.outlined(area, cr, &st.filter, 14.0, false, w / 2.0, y, &colors, 0.85);
    }
  }

  // Centered text in the picker's color with Text.Outline in the background
  // color; returns its height.
  #[allow(clippy::too_many_arguments)]
  fn outlined(
    &self,
    area: &gtk::DrawingArea,
    cr: &cairo::Context,
    text: &str,
    size: f64,
    bold: bool,
    cx: f64,
    y: f64,
    colors: &doxia_theme::ImagePicker,
    alpha: f64,
  ) -> f64 {
    let layout = area.create_pango_layout(Some(text));
    // The QML sets no family: the default sans, not the shell's monospace.
    let mut font = pango::FontDescription::from_string("sans-serif");
    font.set_absolute_size(size * pango::SCALE as f64);
    if bold {
      font.set_weight(pango::Weight::Semibold);
    }
    layout.set_font_description(Some(&font));
    layout.set_width((EXPANDED_W * pango::SCALE as f64) as i32);
    layout.set_ellipsize(pango::EllipsizeMode::End);
    layout.set_alignment(pango::Alignment::Center);
    let (_, th) = layout.pixel_size();
    cr.move_to(cx - EXPANDED_W / 2.0, y);
    pangocairo::functions::layout_path(cr, &layout);
    let b = colors.background;
    cr.set_source_rgba(b[0], b[1], b[2], 0.7 * alpha);
    cr.set_line_width(2.0);
    let _ = cr.stroke();
    cr.move_to(cx - EXPANDED_W / 2.0, y);
    let t = colors.text;
    cr.set_source_rgba(t[0], t[1], t[2], t[3] * alpha);
    pangocairo::functions::show_layout(cr, &layout);
    th as f64
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
        let (filterable, filter) = {
          let st = me.st.borrow();
          (st.filterable, st.filter.clone())
        };
        if filterable {
          me.set_filter(filter + text);
        }
      },
    );
    // A click on the open slice picks it, on another selects it, off the
    // card cancels.
    let click = gtk::GestureClick::new();
    let me = Rc::downgrade(self);
    click.connect_released(move |_, _, x, y| {
      let Some(me) = me.upgrade() else { return };
      let (w, h) = (me.area.width() as f64, me.area.height() as f64);
      let hit = me.slices(w, h).into_iter().rev().find(|&(_, sx, sy, sw, sh, _)| x >= sx && x <= sx + sw && y >= sy && y <= sy + sh);
      match hit {
        Some((_, _, _, _, _, true)) => me.apply(),
        Some((i, ..)) => me.select(i),
        None => {
          let (cx, cy, cw, ch) = me.card_rect(w, h);
          if !(x >= cx && x <= cx + cw && y >= cy && y <= cy + ch) {
            me.cancel();
          }
        }
      }
    });
    self.area.add_controller(click);
  }

  fn key(self: &Rc<Self>, key: gdk::Key, state: gdk::ModifierType) -> bool {
    use gdk::Key;
    let (filter, filterable) = {
      let st = self.st.borrow();
      (st.filter.clone(), st.filterable)
    };
    if filterable {
      if let Some(next) = popup::edit_filter(&filter, key, state) {
        self.set_filter(next);
        return true;
      }
    }
    match key {
      Key::Escape => {
        if filter.is_empty() { self.cancel() } else { self.set_filter(String::new()) }
      }
      Key::Return | Key::KP_Enter => self.apply(),
      Key::Left | Key::ISO_Left_Tab => self.step(-1),
      Key::Tab if state.contains(gdk::ModifierType::SHIFT_MASK) => self.step(-1),
      Key::Right | Key::Tab => self.step(1),
      _ => return false,
    }
    true
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn rows_names_and_filters() {
    let imgs = load_rows("/a/tokyo-night.png\t/c/1.jpg\n/b/tokyo-night.png\t/c/2.jpg\n/a/catppuccin_latte.jpg\n\n");
    assert_eq!(imgs.len(), 2);
    assert_eq!(imgs[1].thumbnail, "/a/catppuccin_latte.jpg");
    assert_eq!(label_for("/x/tokyo-night_dark.png"), "Tokyo Night Dark");
    assert_eq!(name_for("/x/.hidden"), ".hidden");
    assert!(matches(&imgs[0], "night") && matches(&imgs[0], "Tokyo N") && !matches(&imgs[1], "night"));
  }
}
