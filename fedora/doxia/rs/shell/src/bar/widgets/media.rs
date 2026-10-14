// doxia.media (fedora/doxia/plugins/doxia.media/Panel.qml): what plays, in
// the bar, the title running as a marquee in a fixed width while it plays;
// with nothing playing, a music glyph that opens Spotify. Its panel has the
// album art, the track, a seekable progress bar, the controls, the
// player's volume (its PipeWire stream, which the audio panel moves too)
// and, with several music players (browsers are left out), the choice
// between them. Right click plays or pauses, middle click skips. A window
// taking the focus shows its title there for five seconds. IPC target
// "doxia.media" (open, close, show, hide, toggle, playPause, next, previous).

use super::super::config::setting;
use super::super::panel::Panel;
use super::super::ui;
use super::super::widget::{self, Slot};
use super::super::Ctx;
use crate::mango;
use crate::mpris::{self, Loop, Player};
use crate::pipewire;
use gtk::gdk_pixbuf::Pixbuf;
use gtk::prelude::*;
use gtk::{gdk, gio, glib, pango};
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

const PLAY: &str = "\u{F040A}";
const PAUSE: &str = "\u{F03E4}";
const PREV: &str = "\u{F04AE}";
const NEXT: &str = "\u{F04AD}";
const SHUFFLE: &str = "\u{F049D}";
const REPEAT: &str = "\u{F0456}";
const REPEAT_ONE: &str = "\u{F0458}";
const MUSIC: &str = "\u{F075A}";
const OPEN: &str = "\u{F03CC}";
const VOLUME: &str = "\u{F057E}";
const VOLUME_LOW: &str = "\u{F057F}";
const MUTE: &str = "\u{F075F}";

pub fn format_time(seconds: f64) -> String {
  let s = seconds.max(0.0).floor() as u64;
  let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
  if h > 0 { format!("{h}:{m:02}:{sec:02}") } else { format!("{m}:{sec:02}") }
}

// The player's playback streams: their app name, binary or node name and
// the player's identity or desktop entry contain one another.
pub fn stream_matches(node: &pipewire::Node, identity: &str, desktop: &str) -> bool {
  let keys = [identity.to_lowercase(), desktop.to_lowercase()];
  let names = [node.prop("application.name"), node.prop("application.process.binary"), node.prop("node.name")];
  keys.iter().filter(|k| !k.is_empty()).any(|k| {
    names.iter().map(|n| n.to_lowercase()).filter(|n| !n.is_empty()).any(|n| n.contains(k.as_str()) || k.contains(n.as_str()))
  })
}

// The music players: browsers publish one per tab with sound (a video, a
// call), which is not what this widget is for.
const BROWSERS: [&str; 9] = ["chromium", "chrome", "firefox", "brave", "msedge", "edge", "vivaldi", "opera", "plasma-browser-integration"];

fn is_browser(p: &Player) -> bool {
  let bus = p.bus_name.trim_start_matches("org.mpris.MediaPlayer2.").to_lowercase();
  let entry = p.desktop_entry().to_lowercase();
  BROWSERS.iter().any(|b| bus.starts_with(b) || entry.contains(b))
}

fn music_players() -> Vec<Rc<Player>> {
  mpris::get().players().into_iter().filter(|p| !is_browser(p)).collect()
}

struct Media {
  selected: RefCell<String>,
  player: RefCell<Option<Rc<Player>>>,
  position: Cell<f64>,
  // The volume just set, shown until PipeWire and MPRIS catch up with it.
  wanted: Cell<Option<(f64, Instant)>>,
}

impl Media {
  fn player(&self) -> Option<Rc<Player>> {
    self.player.borrow().clone()
  }

  fn streams(&self) -> Vec<pipewire::Node> {
    let Some(p) = self.player() else { return Vec::new() };
    let (identity, desktop) = (p.raw_identity(), p.desktop_entry());
    pipewire::get().snapshot().playback().into_iter().filter(|n| n.has_volume && stream_matches(n, &identity, &desktop)).cloned().collect()
  }

  // The stream's volume when there is one: Spotify's MPRIS volume lags.
  fn volume(&self) -> Option<(f64, bool)> {
    let wanted = self.wanted.get().filter(|(_, at)| at.elapsed() < Duration::from_millis(1500)).map(|(v, _)| v);
    if let Some(s) = self.streams().first() {
      return Some((wanted.unwrap_or(s.volume), s.muted));
    }
    self.player().and_then(|p| p.volume()).map(|v| (wanted.unwrap_or(v), false))
  }

  fn set_volume(&self, v: f64) {
    let v = v.clamp(0.0, 1.0);
    self.wanted.set(Some((v, Instant::now())));
    // Spotify puts its own (MPRIS) volume back on the stream at every track,
    // so that one moves too.
    if let Some(p) = self.player().filter(|p| p.volume().is_some() && p.can("CanControl")) {
      p.set_volume(v);
    }
    for s in self.streams() {
      pipewire::set_volume(s.id, v);
    }
  }

  fn toggle_mute(&self) {
    let streams = self.streams();
    let Some(first) = streams.first() else { return };
    let mute = !first.muted;
    for s in &streams {
      pipewire::set_mute(s.id, mute);
    }
  }

  fn play_pause(&self) {
    if let Some(p) = self.player().filter(|p| p.can("CanPlay") || p.can("CanPause")) {
      p.play_pause();
    }
  }

  fn next(&self) {
    if let Some(p) = self.player().filter(|p| p.can("CanGoNext")) {
      p.next();
    }
  }

  fn previous(&self) {
    if let Some(p) = self.player().filter(|p| p.can("CanGoPrevious")) {
      p.previous();
    }
  }

  fn can_seek(&self) -> bool {
    self.player().is_some_and(|p| p.can("CanSeek") && p.length() > 0.0)
  }

  fn seek_by(&self, delta: f64) {
    if let Some(p) = self.player().filter(|_| self.can_seek()) {
      let to = (self.position.get() + delta).clamp(0.0, p.length());
      p.set_position(to);
      self.position.set(to);
    }
  }

  fn toggle_shuffle(&self) {
    if let Some(p) = self.player() {
      if let Some(on) = p.shuffle() {
        p.set_shuffle(!on);
      }
    }
  }

  fn cycle_loop(&self) {
    if let Some(p) = self.player() {
      match p.loop_state() {
        Some(Loop::None) => p.set_loop(Loop::Playlist),
        Some(Loop::Playlist) => p.set_loop(Loop::Track),
        Some(Loop::Track) => p.set_loop(Loop::None),
        None => {}
      }
    }
  }
}

pub fn build(ctx: &Rc<Ctx>, entry: &Value) -> gtk::Widget {
  let space = { let s = ctx.style.clone(); move |px: f64| s.space(px) };
  let max_label = setting(entry, "maxLabelWidth").and_then(|v| v.as_f64().or_else(|| v.as_str()?.parse().ok())).unwrap_or(180.0).max(60.0);
  let vertical = ctx.vertical();
  let media = Rc::new(Media { selected: RefCell::default(), player: RefCell::default(), position: Cell::new(0.0), wanted: Cell::new(None) });

  // The bar: the glyph, then the title in a fixed-width clip.
  let row = ui::hbox(space(6.0));
  row.add_css_class("bar-media");
  row.set_cursor_from_name(Some("pointer"));
  let pad = space(7.0);
  if vertical { row.set_size_request(ctx.bar_size(), -1) } else { row.set_size_request(-1, ctx.bar_size()) }
  row.set_margin_start(pad);
  row.set_margin_end(pad);
  let glyph = ui::label(MUSIC, "bar-media-glyph");
  glyph.set_valign(gtk::Align::Center);
  row.append(&glyph);
  let marquee = gtk::DrawingArea::new();
  marquee.set_content_width(max_label as i32);
  marquee.set_valign(gtk::Align::Fill);
  marquee.add_css_class("bar-media-title");
  row.append(&marquee);
  let slot = Slot::new(ctx, &row);
  let panel = Panel::new(ctx, &row, &slot, space(320.0));

  // A window taking the focus: its title over the player for five seconds,
  // the two crossing with a blur (style.css).
  if !vertical {
    let focus = gtk::Label::new(None);
    focus.add_css_class("bar-media-focus");
    focus.set_ellipsize(pango::EllipsizeMode::End);
    focus.set_max_width_chars(((max_label + 2.0 * pad as f64) / 7.0).round() as i32);
    focus.set_can_target(false);
    slot.widget.add_overlay(&focus);
    slot.widget.set_measure_overlay(&focus, true);
    let last = Rc::new(Cell::new(None::<u64>));
    let generation = Rc::new(Cell::new(0u64));
    let (row_w, focus_w) = (row.downgrade(), focus.downgrade());
    mango::get().connect(move || {
      let Some((id, title)) = mango::get().focused_client() else { return };
      // The first one heard (the shell starting) only sets where we are.
      if last.replace(Some(id)).is_none_or(|was| was == id) || title.is_empty() {
        return;
      }
      let (Some(row), Some(focus)) = (row_w.upgrade(), focus_w.upgrade()) else { return };
      focus.set_text(&title);
      focus.add_css_class("shown");
      row.add_css_class("away");
      let g = generation.get() + 1;
      generation.set(g);
      let (generation, row_w, focus_w) = (generation.clone(), row_w.clone(), focus_w.clone());
      glib::timeout_add_local_once(Duration::from_secs(5), move || {
        if generation.get() != g {
          return;
        }
        let (Some(row), Some(focus)) = (row_w.upgrade(), focus_w.upgrade()) else { return };
        focus.remove_css_class("shown");
        row.remove_css_class("away");
        // Its width goes once the blur is over.
        glib::timeout_add_local_once(Duration::from_millis(400), move || {
          if generation.get() == g {
            focus.set_text("");
          }
        });
      });
    });
  }

  // The marquee: parked at the start, ellipsized, unless it plays with the
  // panel shut; then it runs at about 40 px/s from the right edge.
  let text = Rc::new(RefCell::new(String::new()));
  let running_since = Rc::new(Cell::new(None::<Instant>));
  let (t, rs) = (text.clone(), running_since.clone());
  marquee.set_draw_func(move |area, cr, w, h| {
    let layout = area.create_pango_layout(Some(&t.borrow()));
    let (tw, th) = layout.pixel_size();
    let (w, tw) = (w as f64, tw as f64);
    let y = ((h - th) / 2) as f64;
    let fg = area.color();
    cr.set_source_rgba(fg.red() as f64, fg.green() as f64, fg.blue() as f64, fg.alpha() as f64);
    match rs.get() {
      Some(start) => {
        let duration = ((w + tw) * 25.0).max(4000.0) / 1000.0;
        let phase = (start.elapsed().as_secs_f64() / duration).fract();
        let x = w - phase * (w + tw);
        cr.rectangle(0.0, 0.0, w, h as f64);
        cr.clip();
        cr.move_to(x, y);
        pangocairo::functions::show_layout(cr, &layout);
      }
      None => {
        layout.set_width((w * pango::SCALE as f64) as i32);
        layout.set_ellipsize(pango::EllipsizeMode::End);
        cr.move_to(0.0, y);
        pangocairo::functions::show_layout(cr, &layout);
      }
    }
  });

  // ---- The panel.
  let body = ui::vbox(space(12.0));
  body.add_css_class("media-panel");
  let art_frame = gtk::Overlay::new();
  art_frame.add_css_class("media-art");
  let art = gtk::Picture::new();
  art.set_content_fit(gtk::ContentFit::Cover);
  art.set_can_shrink(true);
  let art_size = space(320.0) - 0;
  art_frame.set_size_request(art_size, art_size);
  let art_fallback = ui::label(MUSIC, "media-art-glyph");
  art_frame.set_child(Some(&art_fallback));
  art.set_margin_top(space(2.0));
  art.set_margin_bottom(space(2.0));
  art.set_margin_start(space(2.0));
  art.set_margin_end(space(2.0));
  art_frame.add_overlay(&art);
  body.append(&art_frame);
  let info = ui::vbox(space(2.0));
  let title = ui::label("", "media-title");
  let artist = ui::label("", "media-artist");
  let album = ui::label("", "media-album");
  for l in [&title, &artist, &album] {
    l.set_xalign(0.0);
    l.set_ellipsize(pango::EllipsizeMode::End);
    info.append(l);
  }
  body.append(&info);
  let progress_box = ui::vbox(space(4.0));
  let progress = ui::Slider::new(0.0, 1.0, 1.0, &space);
  progress_box.append(&progress.widget);
  let times = gtk::CenterBox::new();
  let elapsed = ui::label("0:00", "media-time");
  let total = ui::label("0:00", "media-time");
  times.set_start_widget(Some(&elapsed));
  times.set_end_widget(Some(&total));
  progress_box.append(&times);
  body.append(&progress_box);

  let controls = gtk::CenterBox::new();
  let control = |glyph: &str, tip: &str, big: bool| {
    let l = ui::label(glyph, if big { "media-control big" } else { "media-control" });
    l.set_tooltip_text(Some(tip));
    l.set_cursor_from_name(Some("pointer"));
    l
  };
  let shuffle = control(SHUFFLE, "", false);
  let prev = control(PREV, "Anterior", false);
  let play = control(PLAY, "Tocar", true);
  let next = control(NEXT, "Próxima", false);
  let repeat = control(REPEAT, "", false);
  let middle = ui::hbox(space(4.0));
  middle.append(&prev);
  middle.append(&play);
  middle.append(&next);
  controls.set_start_widget(Some(&shuffle));
  controls.set_center_widget(Some(&middle));
  controls.set_end_widget(Some(&repeat));
  body.append(&controls);

  let volume_row = ui::hbox(space(8.0));
  let vol_icon = ui::label(VOLUME, "media-volume-icon");
  vol_icon.set_size_request(space(22.0), -1);
  vol_icon.set_cursor_from_name(Some("pointer"));
  let volume = ui::Slider::new(0.0, 1.0, 0.05, &space);
  let vol_text = ui::label("0%", "media-volume-text");
  vol_text.set_size_request(space(36.0), -1);
  vol_text.set_xalign(1.0);
  volume_row.append(&vol_icon);
  volume_row.append(&volume.widget);
  volume_row.append(&vol_text);
  body.append(&volume_row);

  let source_row = ui::hbox(space(6.0));
  let source = ui::label("", "media-source");
  source.set_hexpand(true);
  source.set_xalign(0.0);
  source.set_ellipsize(pango::EllipsizeMode::End);
  source_row.append(&source);
  let open_app = {
    let (m, p) = (media.clone(), Rc::downgrade(&panel));
    ui::action(OPEN, "", move || {
      if let Some(pl) = m.player() {
        pl.raise();
      }
      if let Some(p) = p.upgrade() {
        p.close();
      }
    })
  };
  source_row.append(&open_app);
  body.append(&source_row);
  let others = ui::vbox(space(4.0));
  body.append(&others);
  panel.content.append(&body);

  // ---- Painting from the player's state.
  let art_url = Rc::new(RefCell::new(String::new()));
  // Picking another player repaints: paint, reached from its own rows.
  let repaint: Rc<RefCell<std::rc::Weak<dyn Fn()>>> = Rc::new(RefCell::new(std::rc::Weak::<Box<dyn Fn()>>::new() as std::rc::Weak<dyn Fn()>));
  let paint: Rc<dyn Fn()> = {
    let repaint = Rc::downgrade(&repaint);
    let media = media.clone();
    let (glyph, text, marquee, row, panel_w) = (glyph.clone(), text.clone(), marquee.clone(), row.clone(), Rc::downgrade(&panel));
    let running_since = running_since.clone();
    let (title, artist, album, total, art, art_url) = (title.clone(), artist.clone(), album.clone(), total.clone(), art.clone(), art_url.clone());
    let (shuffle, prev, play, next, repeat) = (shuffle.clone(), prev.clone(), play.clone(), next.clone(), repeat.clone());
    let (progress, progress_box, volume, vol_icon, vol_text, volume_row) = (progress.clone(), progress_box.clone(), volume.clone(), vol_icon.clone(), vol_text.clone(), volume_row.clone());
    let (source, open_app, others) = (source.clone(), open_app.clone(), others.clone());
    Rc::new(move || {
      let player = mpris::pick_from(&music_players(), &media.selected.borrow());
      *media.player.borrow_mut() = player.clone();
      let (t, a) = player.as_ref().map(|p| (p.title(), p.artist())).unwrap_or_default();
      let has_media = !t.is_empty() || !a.is_empty();
      let playing = player.as_ref().is_some_and(|p| p.playing());
      let open = panel_w.upgrade().is_some_and(|p| p.is_open());
      glyph.set_text(if !has_media { MUSIC } else if playing { PAUSE } else { PLAY });
      if playing { glyph.remove_css_class("dim") } else { glyph.add_css_class("dim") }
      let label = if a.is_empty() { t.clone() } else { format!("{t}  ·  {a}") };
      marquee.set_visible(!vertical && has_media && !t.is_empty());
      if *text.borrow() != label {
        *text.borrow_mut() = label;
        if running_since.get().is_some() {
          running_since.set(Some(Instant::now()));
        }
      }
      let should_run = playing && !open && marquee.is_visible();
      if should_run && running_since.get().is_none() {
        running_since.set(Some(Instant::now()));
        let (rs, m) = (running_since.clone(), marquee.downgrade());
        glib::timeout_add_local(Duration::from_millis(33), move || {
          let Some(m) = m.upgrade() else { return glib::ControlFlow::Break };
          if rs.get().is_none() {
            m.queue_draw();
            return glib::ControlFlow::Break;
          }
          m.queue_draw();
          glib::ControlFlow::Continue
        });
      } else if !should_run {
        running_since.set(None);
      }
      marquee.queue_draw();
      row.set_tooltip_text(Some(&if has_media {
        if a.is_empty() { t.clone() } else { format!("{t} — {a}") }
      } else {
        "Abrir Spotify".into()
      }));

      // The panel.
      title.set_text(if t.is_empty() { "Nada tocando" } else { &t });
      artist.set_text(if a.is_empty() { " " } else { &a });
      let al = player.as_ref().map(|p| p.album()).unwrap_or_default();
      album.set_text(if al.is_empty() { " " } else { &al });
      album.set_opacity(if !al.is_empty() && al != a { 1.0 } else { 0.0 });
      let length = player.as_ref().map(|p| p.length()).unwrap_or(0.0);
      progress_box.set_opacity(if length > 0.0 { 1.0 } else { 0.0 });
      total.set_text(&format_time(length));
      let url = player.as_ref().map(|p| p.art_url()).unwrap_or_default();
      if *art_url.borrow() != url {
        *art_url.borrow_mut() = url.clone();
        load_art(&art, &url, art_size);
      }
      let can = |w: &str| player.as_ref().is_some_and(|p| p.can(w));
      play.set_text(if playing { PAUSE } else { PLAY });
      play.set_tooltip_text(Some(if playing { "Pausar" } else { "Tocar" }));
      play.set_opacity(if can("CanPlay") || can("CanPause") { 1.0 } else { 0.4 });
      prev.set_opacity(if can("CanGoPrevious") { 1.0 } else { 0.4 });
      next.set_opacity(if can("CanGoNext") { 1.0 } else { 0.4 });
      let sh = player.as_ref().and_then(|p| p.shuffle());
      shuffle.set_visible(sh.is_some());
      shuffle.set_tooltip_text(Some(if sh == Some(true) { "Aleatório: ligado" } else { "Aleatório: desligado" }));
      if sh == Some(true) { shuffle.remove_css_class("off") } else { shuffle.add_css_class("off") }
      let lp = player.as_ref().and_then(|p| p.loop_state());
      repeat.set_visible(lp.is_some());
      repeat.set_text(if lp == Some(Loop::Track) { REPEAT_ONE } else { REPEAT });
      repeat.set_tooltip_text(Some(match lp {
        Some(Loop::Track) => "Repetir: faixa",
        Some(Loop::Playlist) => "Repetir: tudo",
        _ => "Repetir: desligado",
      }));
      if lp.is_some_and(|l| l != Loop::None) { repeat.remove_css_class("off") } else { repeat.add_css_class("off") }
      progress.set(media.position.get() / length.max(1.0));
      let vol = media.volume();
      volume_row.set_opacity(if vol.is_some() { 1.0 } else { 0.0 });
      volume_row.set_sensitive(vol.is_some());
      let (v, muted) = vol.unwrap_or((0.0, false));
      volume.set(v);
      volume.widget.set_opacity(if muted { 0.5 } else { 1.0 });
      vol_icon.set_text(if muted || v <= 0.0 { MUTE } else if v < 0.5 { VOLUME_LOW } else { VOLUME });
      if muted { vol_icon.add_css_class("dim") } else { vol_icon.remove_css_class("dim") }
      if !volume.dragging() {
        vol_text.set_text(&format!("{}%", (v * 100.0).round()));
      }
      let name = player.as_ref().map(|p| p.identity()).unwrap_or_default();
      source.set_text(&name);
      open_app.set_opacity(if player.as_ref().is_some_and(|p| p.can_raise()) { 1.0 } else { 0.0 });
      open_app.set_tooltip_text(Some(&format!("Abrir {name}")));

      // Other players, to pick from.
      crate::popup::clear(&others);
      let players = music_players();
      others.set_visible(players.len() > 1);
      if players.len() > 1 {
        others.append(&ui::separator());
        for p in players {
          let r = ui::hbox(8);
          r.append(&ui::label(if p.playing() { PAUSE } else { PLAY }, "media-other-glyph"));
          let name = p.identity();
          let tt = p.title();
          let l = ui::label(&if tt.is_empty() { name.clone() } else { format!("{tt}  ·  {name}") }, "media-other");
          l.set_ellipsize(pango::EllipsizeMode::End);
          l.set_xalign(0.0);
          l.set_hexpand(true);
          r.append(&l);
          let selected = player.as_ref().is_some_and(|q| q.bus_name == p.bus_name);
          let m = media.clone();
          let bus = p.bus_name.clone();
          let rp = repaint.clone();
          let row = ui::row(&r, move |_| {
            *m.selected.borrow_mut() = bus.clone();
            if let Some(f) = rp.upgrade().and_then(|r| r.borrow().upgrade()) {
              f();
            }
          });
          if selected {
            row.add_css_class("selected");
            l.add_css_class("bold");
          }
          others.append(&row);
        }
      }
      if let Some(p) = panel_w.upgrade() {
        p.relayout();
      }
    })
  };
  *repaint.borrow_mut() = Rc::downgrade(&paint);
  // Kept with the widget.
  let keep = repaint.clone();
  row.connect_destroy(move |_| {
    let _ = &keep;
  });
  paint();
  let (p, w) = (paint.clone(), row.downgrade());
  mpris::get().connect(move || {
    if w.upgrade().is_some() {
      p();
    }
  });
  let (p, w) = (paint.clone(), row.downgrade());
  pipewire::get().connect(move || {
    if w.upgrade().is_some() {
      p();
    }
  });

  // Position: asked every half second while the panel shows it.
  let tick = {
    let (media, progress, elapsed, panel_w) = (media.clone(), progress.clone(), elapsed.clone(), Rc::downgrade(&panel));
    move || {
      let (media, progress, elapsed, panel_w) = (media.clone(), progress.clone(), elapsed.clone(), panel_w.clone());
      glib::spawn_future_local(async move {
        let Some(p) = media.player() else { return };
        let pos = p.position().await;
        media.position.set(pos);
        if panel_w.upgrade().is_some_and(|p| p.is_open()) && !progress.dragging() {
          progress.set(pos / p.length().max(1.0));
          elapsed.set_text(&format_time(pos));
        }
      });
    }
  };
  {
    let (tick, panel_w, p) = (tick.clone(), Rc::downgrade(&panel), paint.clone());
    panel.connect_open(move || {
      tick();
      p();
      let (tick, panel_w) = (tick.clone(), panel_w.clone());
      glib::timeout_add_local(Duration::from_millis(500), move || {
        if !panel_w.upgrade().is_some_and(|p| p.is_open()) {
          return glib::ControlFlow::Break;
        }
        tick();
        glib::ControlFlow::Continue
      });
    });
    let p = paint.clone();
    panel.connect_close(move || p());
  }
  {
    let (m, e) = (media.clone(), elapsed.clone());
    progress.connect_moved(move |f| {
      if let Some(p) = m.player() {
        e.set_text(&format_time(f * p.length()));
      }
    });
    let m = media.clone();
    progress.connect_released(move |f| {
      if let Some(p) = m.player().filter(|_| m.can_seek()) {
        p.set_position(f * p.length());
        m.position.set(f * p.length());
      }
    });
    let (m, vt) = (media.clone(), vol_text.clone());
    volume.connect_moved(move |v| {
      vt.set_text(&format!("{}%", (v * 100.0).round()));
      m.set_volume(v);
    });
    let m = media.clone();
    volume.connect_released(move |v| m.set_volume(v));
    let m = media.clone();
    volume.connect_right_click(move || m.toggle_mute());
    let m = media.clone();
    widget::on_press(&vol_icon, move |_| m.toggle_mute());
    for (w, f) in [
      (&shuffle, Box::new({ let m = media.clone(); move || m.toggle_shuffle() }) as Box<dyn Fn()>),
      (&prev, Box::new({ let m = media.clone(); move || m.previous() })),
      (&play, Box::new({ let m = media.clone(); move || m.play_pause() })),
      (&next, Box::new({ let m = media.clone(); move || m.next() })),
      (&repeat, Box::new({ let m = media.clone(); move || m.cycle_loop() })),
    ] {
      widget::on_press(w, move |_| f());
    }
  }

  // Keys in the panel.
  {
    let m = media.clone();
    panel.connect_key(move |key, _| {
      use gdk::Key;
      match key {
        Key::Return | Key::KP_Enter | Key::space | Key::k => m.play_pause(),
        Key::Left | Key::h => m.seek_by(-5.0),
        Key::Right | Key::l => m.seek_by(5.0),
        Key::Up => m.previous(),
        Key::Down | Key::j => m.next(),
        Key::n => m.next(),
        Key::p => m.previous(),
        Key::s => m.toggle_shuffle(),
        Key::r => m.cycle_loop(),
        Key::plus | Key::equal => m.set_volume(m.volume().map(|v| v.0).unwrap_or(0.0) + 0.05),
        Key::minus => m.set_volume(m.volume().map(|v| v.0).unwrap_or(0.0) - 0.05),
        Key::m => m.toggle_mute(),
        _ => return false,
      }
      true
    });
  }

  // Clicks on the bar.
  {
    let (m, p) = (media.clone(), panel.clone());
    widget::on_press(&row, move |b| {
      let has_media = m.player().is_some_and(|p| !p.title().is_empty() || !p.artist().is_empty());
      if !has_media {
        if b == 1 {
          crate::run::shell("omarchy-launch-spotify");
        }
        return;
      }
      match b {
        3 => m.play_pause(),
        2 => m.next(),
        _ => p.toggle(),
      }
    });
  }

  // IPC, from the first screen's bar.
  if ctx.first_screen {
    if let Some(bar) = ctx.bar.upgrade() {
      let (m, p) = (media.clone(), Rc::downgrade(&panel));
      bar.add_ipc("doxia.media", Box::new(move |method, _| {
        let panel = p.upgrade()?;
        match method {
          "open" | "show" => panel.open(),
          "close" | "hide" => panel.close(),
          "toggle" => panel.toggle(),
          "playPause" => m.play_pause(),
          "next" => m.next(),
          "previous" => m.previous(),
          _ => return None,
        }
        Some("ok".into())
      }));
    }
  }
  slot.widget.clone().upcast()
}

fn load_art(picture: &gtk::Picture, url: &str, size: i32) {
  picture.set_paintable(None::<&gdk::Paintable>);
  if url.is_empty() {
    return;
  }
  let (weak, file) = (picture.downgrade(), gio::File::for_uri(url));
  glib::spawn_future_local(async move {
    let Ok(stream) = file.read_future(glib::Priority::DEFAULT).await else { return };
    let Ok(pixbuf) = Pixbuf::from_stream_at_scale_future(&stream, size * 2, size * 2, true).await else { return };
    if let Some(p) = weak.upgrade() {
      p.set_paintable(Some(&gdk::Texture::for_pixbuf(&pixbuf)));
    }
  });
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn times_and_stream_matching() {
    assert_eq!(format_time(65.4), "1:05");
    assert_eq!(format_time(3725.0), "1:02:05");
    let mut n = pipewire::Node::default();
    n.props.insert("application.name".into(), "spotify".into());
    assert!(stream_matches(&n, "Spotify", ""));
    assert!(!stream_matches(&n, "Firefox", "firefox"));
  }
}
