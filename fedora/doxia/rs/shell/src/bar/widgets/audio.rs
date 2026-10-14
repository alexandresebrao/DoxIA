// omarchy.audio (shell/plugins/panels/audio): the output's icon; the wheel
// steps the volume by 5% with the OSD, a right click mutes everything. Its
// panel: the volume's name ("Sussurro" .. "Sala de concerto") over a switch
// that mutes output and input, the output slider and its devices, the
// input slider and its devices, and each app's stream with its own slider.
// The volume slider moves the sink omarchy-audio-output-sink names (the
// speaker tuning's filter, when there is one); devices
// omarchy-audio-sink-availability reports as unplugged are left out.

use super::super::member::Member;
use super::super::panel::Panel;
use super::super::ui::{self, CursorRow};
use super::super::widget::{Button, Kind, Slot};
use super::super::Ctx;
use crate::pipewire::{self, Node, Snapshot};
use gtk::prelude::*;
use gtk::{glib, pango};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

pub fn volume_name(volume: f64, muted: bool) -> &'static str {
  if muted {
    return "Mudo";
  }
  match (volume * 100.0).round() as i64 {
    0 => "Silenciado",
    p if p >= 100 => "Sala de concerto",
    p if p >= 85 => "Modo festa",
    p if p >= 70 => "No talo",
    p if p >= 50 => "Ritmo constante",
    p if p >= 30 => "Som ambiente",
    p if p >= 15 => "Murmúrio",
    _ => "Sussurro",
  }
}

pub fn friendly_device(text: &str) -> String {
  let mut l = text.trim().to_string();
  for prefix in ["sof-soundwire ", "built-in audio ", "builtin audio "] {
    if l.to_lowercase().starts_with(prefix) {
      l = l[prefix.len()..].to_string();
    }
  }
  for suffix in [" Output", " Input"] {
    if l.ends_with(suffix) {
      l.truncate(l.len() - suffix.len());
    }
  }
  l.replace("Microphones", "Microphone")
}

pub fn node_label(n: &Node) -> String {
  let nick = friendly_device(if !n.nick.is_empty() { &n.nick } else { n.prop("device.profile.description") });
  if !nick.is_empty() {
    return nick;
  }
  let d = friendly_device(if !n.description.is_empty() { &n.description } else { &n.name });
  if d.is_empty() { "Desconhecido".into() } else { d }
}

fn blob(n: &Node, keys: &[&str]) -> String {
  let mut parts = vec![n.name.clone(), n.description.clone(), n.nick.clone()];
  parts.extend(keys.iter().map(|k| n.prop(k).to_string()));
  parts.join(" ").to_lowercase()
}

pub fn is_headphones(n: &Node) -> bool {
  let b = blob(n, &["device.icon-name", "device.icon_name", "device.product.name", "node.description", "node.nick"]);
  ["headphone", "headset", "earbud", "earphone", "airpod"].iter().any(|w| b.contains(w))
}

pub fn sink_glyph(n: &Node) -> &'static str {
  if is_headphones(n) {
    return "\u{f02cb}";
  }
  let b = blob(n, &["device.icon-name", "device.icon_name", "device.product.name"]);
  if b.contains("bluetooth") {
    "\u{f00af}"
  } else if b.contains("hdmi") || b.contains("display") {
    "\u{f0379}"
  } else {
    "\u{f04c3}"
  }
}

pub fn source_glyph(n: &Node) -> &'static str {
  let b = blob(n, &["device.icon-name", "device.icon_name"]);
  if b.contains("headset") {
    "\u{f02cb}"
  } else if b.contains("bluetooth") {
    "\u{f00af}"
  } else if b.contains("webcam") || b.contains("camera") {
    "\u{f0100}"
  } else {
    "\u{f036c}"
  }
}

// An app stream's name; a generic "audio-src" takes the name of the one
// MPRIS player it can only be.
pub fn stream_label(n: &Node, players: &[String]) -> String {
  let raw = [n.prop("application.name"), n.description.as_str(), n.prop("media.name"), n.prop("node.name"), n.name.as_str()]
    .into_iter()
    .find(|s| !s.is_empty())
    .unwrap_or("")
    .to_string();
  let friendly = |s: &str| if s.eq_ignore_ascii_case("spotify") { "Spotify".to_string() } else { s.to_string() };
  if raw.eq_ignore_ascii_case("audio-src") && players.len() == 1 {
    return friendly(&players[0]);
  }
  let out = friendly(&raw);
  if out.is_empty() { "Transmissão".into() } else { out }
}

fn output_icon(sink: Option<&Node>, volume: f64, muted: bool) -> &'static str {
  let Some(s) = sink.filter(|s| s.has_volume) else { return "\u{eee8}" };
  if is_headphones(s) {
    return "\u{f02cb}";
  }
  if muted {
    return "\u{eee8}";
  }
  if volume >= 0.67 {
    "\u{f028}"
  } else if volume >= 0.34 {
    "\u{f027}"
  } else if volume > 0.0 {
    "\u{f026}"
  } else {
    "\u{eee8}"
  }
}

pub fn parse_availability(raw: &str) -> HashMap<String, bool> {
  raw.lines()
    .filter_map(|l| {
      let mut p = l.trim().split('\t');
      Some((p.next()?.to_string(), p.next()? != "0"))
    })
    .collect()
}

struct Audio {
  volume_sink: RefCell<String>,
  availability: RefCell<Option<HashMap<String, bool>>>,
}

impl Audio {
  // The sink the volume moves: the tuned one when there is one.
  fn volume_sink(&self, snap: &Snapshot) -> Option<Node> {
    let name = self.volume_sink.borrow().clone();
    if !name.is_empty() {
      if let Some(n) = snap.nodes.iter().find(|n| n.is_sink() && n.name == name && n.has_volume) {
        return Some(n.clone());
      }
    }
    snap.sink().cloned()
  }

  fn sinks(&self, snap: &Snapshot) -> Vec<Node> {
    let avail = self.availability.borrow();
    let mut list: Vec<Node> = snap
      .sinks()
      .into_iter()
      .filter(|n| avail.as_ref().is_none_or(|a| a.get(&n.name) != Some(&false)))
      .cloned()
      .collect();
    if let Some(s) = snap.sink() {
      if !list.iter().any(|n| n.id == s.id) {
        list.insert(0, s.clone());
      }
    }
    list
  }
}

fn refresh_volume_sink(audio: &Rc<Audio>) {
  let a = audio.clone();
  glib::spawn_future_local(async move {
    let out = crate::watch::output(&["omarchy-audio-output-sink"]).await.unwrap_or_default();
    *a.volume_sink.borrow_mut() = out.trim().to_string();
  });
}

pub fn build(ctx: &Rc<Ctx>) -> Member {
  let space = { let s = ctx.style.clone(); move |px: f64| s.space(px) };
  let pw = pipewire::get();
  let audio = Rc::new(Audio { volume_sink: RefCell::default(), availability: RefCell::default() });
  refresh_volume_sink(&audio);
  {
    let a = audio.clone();
    glib::timeout_add_seconds_local(15, move || {
      refresh_volume_sink(&a);
      glib::ControlFlow::Continue
    });
  }
  let button = Button::new(ctx, Kind::Icon, "\u{eee8}");
  let slot = Slot::new(ctx, &button.widget);
  let panel = Panel::new(ctx, &button.widget, &slot, space(380.0));
  panel.content.add_css_class("audio");
  let cursor = ui::Cursor::new();

  let scroll = gtk::ScrolledWindow::new();
  scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
  scroll.set_propagate_natural_height(true);
  scroll.set_max_content_height(space(560.0));
  let body = ui::vbox(space(14.0));
  scroll.set_child(Some(&body));
  panel.content.append(&scroll);

  let toggle_all = {
    let (pw, a) = (pw.clone(), audio.clone());
    move || {
      let snap = pw.snapshot();
      let (out, inp) = (a.volume_sink(&snap), snap.source().cloned());
      let audible = out.as_ref().is_some_and(|o| !o.muted) || inp.as_ref().is_some_and(|i| !i.muted);
      if let Some(o) = out {
        pipewire::set_mute(o.id, audible);
      }
      if let Some(i) = inp {
        pipewire::set_mute(i.id, audible);
      }
    }
  };

  // The panel is built again only when its devices or streams change;
  // values (volumes, mutes, the default) update in place, so a slider being
  // dragged is never swapped out from under the pointer.
  let structure = Rc::new(RefCell::new(String::new()));
  let updaters: Rc<RefCell<Vec<Box<dyn Fn(&Snapshot)>>>> = Rc::default();
  let paint: Rc<dyn Fn()> = {
    let (pw, a, b, body, cursor, panel_w) = (pw.clone(), audio.clone(), button.clone(), body.clone(), cursor.clone(), Rc::downgrade(&panel));
    let (toggle_all, structure, updaters) = (toggle_all.clone(), structure.clone(), updaters.clone());
    let space = space.clone();
    Rc::new(move || {
      let snap = pw.snapshot();
      let out = a.volume_sink(&snap);
      let (volume, muted) = out.as_ref().map(|o| (o.volume, o.muted)).unwrap_or((0.0, false));
      b.set_text(output_icon(snap.sink(), volume, muted));
      b.set_tooltip(&format!("{}%", (volume * 100.0).round()));
      let Some(panel) = panel_w.upgrade() else { return };
      if !panel.is_open() {
        structure.borrow_mut().clear();
        return;
      }
      let streams: Vec<Node> = snap.playback().into_iter().filter(|n| n.has_volume && !n.name.starts_with("omarchy_speaker_tuning")).cloned().collect();
      let sinks = a.sinks(&snap);
      let mut sources: Vec<Node> = snap.sources().into_iter().cloned().collect();
      if let Some(s) = snap.source() {
        if !sources.iter().any(|n| n.id == s.id) {
          sources.insert(0, s.clone());
        }
      }
      let key = format!(
        "{:?}|{:?}|{:?}|{:?}|{:?}",
        sinks.iter().map(|n| (n.id, node_label(n))).collect::<Vec<_>>(),
        sources.iter().map(|n| (n.id, node_label(n))).collect::<Vec<_>>(),
        streams.iter().map(|n| n.id).collect::<Vec<_>>(),
        snap.source().map(|s| s.id),
        out.as_ref().map(|o| o.id),
      );
      if *structure.borrow() != key {
        *structure.borrow_mut() = key;
        updaters.borrow_mut().clear();
        crate::popup::clear(&body);
        cursor.clear();
        build_panel(&body, &cursor, &updaters, &snap, &a, &pw, &sinks, &sources, &streams, &toggle_all, &space);
        panel.relayout();
      }
      for u in updaters.borrow().iter() {
        u(&snap);
      }
    })
  };
  paint();
  let (p, w) = (paint.clone(), button.widget.downgrade());
  pw.connect(move || {
    if w.upgrade().is_some() {
      p();
    }
  });
  {
    let (p, a, c) = (paint.clone(), audio.clone(), cursor.clone());
    let panel_w = Rc::downgrade(&panel);
    panel.connect_open(move || {
      c.reset();
      // Unplugged outputs, read again every 5 s while open.
      let (p, a, panel_w) = (p.clone(), a.clone(), panel_w.clone());
      p();
      glib::spawn_future_local(async move {
        while panel_w.upgrade().is_some_and(|p| p.is_open()) {
          let raw = crate::watch::output(&["omarchy-audio-sink-availability"]).await.unwrap_or_default();
          *a.availability.borrow_mut() = Some(parse_availability(&raw));
          p();
          glib::timeout_future(Duration::from_secs(5)).await;
        }
      });
    });
    let c = cursor.clone();
    panel.connect_key(move |k, _| c.key(k));
  }

  // The wheel: 5% a step, with the OSD.
  let wheel: Rc<dyn Fn(f64, f64)> = {
    let (pw, a, bar) = (pw.clone(), audio.clone(), ctx.bar.clone());
    Rc::new(move |_, dy| {
      if dy == 0.0 {
        return;
      }
      let snap = pw.snapshot();
      let Some(o) = a.volume_sink(&snap) else { return };
      let v = (o.volume + if dy < 0.0 { 0.05 } else { -0.05 }).clamp(0.0, 1.0);
      pipewire::set_volume(o.id, v);
      if let Some(bar) = bar.upgrade() {
        bar.osd(output_icon(snap.sink(), v, o.muted), v * 100.0);
      }
    })
  };
  Member { id: "omarchy.audio".into(), title: "Som".into(), button, slot, panel, wheel: Some(wheel), right: Some(Rc::new(toggle_all)) }
}

type Updaters = Rc<RefCell<Vec<Box<dyn Fn(&Snapshot)>>>>;

fn node_in(snap: &Snapshot, id: u32) -> Option<&Node> {
  snap.nodes.iter().find(|n| n.id == id)
}

fn pct(v: f64) -> String {
  format!("{}%", (v * 100.0).round())
}

#[allow(clippy::too_many_arguments)]
fn build_panel(
  body: &gtk::Box,
  cursor: &Rc<ui::Cursor>,
  updaters: &Updaters,
  snap: &Snapshot,
  audio: &Rc<Audio>,
  pw: &Rc<pipewire::Pipewire>,
  sinks: &[Node],
  sources: &[Node],
  streams: &[Node],
  toggle_all: &(impl Fn() + Clone + 'static),
  space: &(impl Fn(f64) -> i32 + Clone + 'static),
) {
  let up = |f: Box<dyn Fn(&Snapshot)>| updaters.borrow_mut().push(f);
  let out_id = audio.volume_sink(snap).map(|o| o.id);

  // Hero: the icon, "Áudio" over the volume's name, the mute-all switch.
  let hero = ui::hbox(space(14.0));
  let hi = ui::label("", "panel-hero-icon");
  hero.append(&hi);
  let labels = ui::vbox(space(2.0));
  labels.set_hexpand(true);
  labels.set_valign(gtk::Align::Center);
  let t = ui::label("Áudio", "panel-hero-title");
  t.set_xalign(0.0);
  labels.append(&t);
  let name = ui::label("", "panel-hero-subtitle");
  name.set_xalign(0.0);
  labels.append(&name);
  hero.append(&labels);
  let ta = toggle_all.clone();
  // A click toggles; the state follows PipeWire through the updater.
  let sw = ui::state_switch(false, move || ta());
  sw.set_focus_on_click(false);
  hero.append(&sw);
  body.append(&hero);
  let ta = toggle_all.clone();
  cursor.add(CursorRow { widget: sw.clone().upcast(), activate: Some(Rc::new(move || ta())), adjust: None, mute: None });
  {
    let (hi, name, sw, a) = (hi.clone(), name.clone(), sw.clone(), audio.clone());
    up(Box::new(move |snap| {
      let out = out_id.and_then(|id| node_in(snap, id)).cloned().or_else(|| a.volume_sink(snap));
      let (v, m) = out.as_ref().map(|o| (o.volume, o.muted)).unwrap_or((0.0, false));
      hi.set_text(output_icon(snap.sink(), v, m));
      hi.set_opacity(if m { 0.5 } else { 1.0 });
      name.set_text(&volume_name(v, m).to_uppercase());
      let audible = (out.is_some() && !m) || snap.source().is_some_and(|s| !s.muted);
      ui::set_switch(&sw, audible);
      sw.set_tooltip_text(Some(if audible { "Silenciar" } else { "Ativar som" }));
    }));
  }
  body.append(&ui::separator());

  let section_head = |title: &str| {
    let head = gtk::CenterBox::new();
    head.set_start_widget(Some(&ui::section(title)));
    let p = ui::label("", "audio-percent");
    p.set_margin_end(space(6.0));
    head.set_end_widget(Some(&p));
    (head, p)
  };
  // A volume slider for node `id`, its percent label updated alongside.
  let volume_row = |id: u32, max: f64, pct_label: &gtk::Label| {
    let slider = ui::Slider::new(0.0, max, 0.05, space);
    slider.widget.set_margin_start(space(6.0));
    slider.widget.set_margin_end(space(6.0));
    let row = ui::hbox(0);
    row.add_css_class("audio-slider-row");
    row.append(&slider.widget);
    let p = pct_label.clone();
    slider.connect_moved(move |v| {
      p.set_text(&pct(v));
      pipewire::set_volume(id, v);
    });
    let pw2 = pw.clone();
    slider.connect_right_click(move || {
      if let Some(n) = node_in(&pw2.snapshot(), id) {
        pipewire::set_mute(id, !n.muted);
      }
    });
    let (sl, p) = (slider.clone(), pct_label.clone());
    up(Box::new(move |snap| {
      if let Some(n) = node_in(snap, id) {
        sl.set(n.volume);
        sl.widget.set_opacity(if n.muted { 0.5 } else { 1.0 });
        if !sl.dragging() {
          p.set_text(&pct(n.volume));
        }
        p.set_opacity(if n.muted { 0.5 } else { 1.0 });
      }
    }));
    let (pw_a, pw_m) = (pw.clone(), pw.clone());
    let mute: Rc<dyn Fn()> = Rc::new(move || {
      if let Some(n) = node_in(&pw_m.snapshot(), id) {
        pipewire::set_mute(id, !n.muted);
      }
    });
    cursor.add(CursorRow {
      widget: row.clone().upcast(),
      activate: Some(mute.clone()),
      adjust: Some(Rc::new(move |d| {
        if let Some(n) = node_in(&pw_a.snapshot(), id) {
          pipewire::set_volume(id, (n.volume + d).clamp(0.0, max));
        }
      })),
      mute: Some(mute),
    });
    row
  };
  // A device: a click makes it the default.
  let device = |n: &Node, glyph: &str, default_cmd: &'static str, is_default: fn(&Snapshot) -> Option<u32>| {
    let (r, label) = device_row(glyph, &node_label(n), space);
    let (id, nm) = (n.id, n.name.clone());
    let activate: Rc<dyn Fn()> = Rc::new(move || {
      pipewire::set_default(id);
      crate::run::argv(&[default_cmd, &id.to_string(), &nm]);
    });
    let act = activate.clone();
    let row = ui::row(&r, move |_| act());
    let rw = row.clone();
    up(Box::new(move |snap| {
      let on = is_default(snap) == Some(id);
      if on { rw.add_css_class("selected") } else { rw.remove_css_class("selected") }
      if on { label.add_css_class("bold") } else { label.remove_css_class("bold") }
    }));
    cursor.add(CursorRow { widget: row.clone().upcast(), activate: Some(activate), adjust: None, mute: None });
    row
  };

  // Output.
  let col = ui::vbox(space(6.0));
  let (head, out_pct) = section_head("Saída");
  col.append(&head);
  if let Some(id) = out_id {
    col.append(&volume_row(id, 1.0, &out_pct));
  }
  for n in sinks {
    col.append(&device(n, sink_glyph(n), "omarchy-audio-output-set-default", |s| s.sink().map(|n| n.id)));
  }
  body.append(&col);

  // Input.
  if !sources.is_empty() {
    body.append(&ui::separator());
    let col = ui::vbox(space(6.0));
    let (head, in_pct) = section_head("Entrada");
    col.append(&head);
    if let Some(src) = snap.source() {
      col.append(&volume_row(src.id, 1.0, &in_pct));
    }
    for n in sources {
      col.append(&device(n, source_glyph(n), "omarchy-audio-input-set-default", |s| s.source().map(|n| n.id)));
    }
    body.append(&col);
  }

  // App streams.
  if !streams.is_empty() {
    body.append(&ui::separator());
    let col = ui::vbox(space(10.0));
    col.append(&ui::section("Fontes"));
    let players: Vec<String> = crate::mpris::get().players().iter().map(|p| p.identity()).filter(|s| !s.is_empty()).collect();
    for n in streams {
      let item = ui::vbox(space(2.0));
      item.add_css_class("audio-stream");
      let top = ui::hbox(space(8.0));
      let id = n.id;
      let pw_m = pw.clone();
      let mute_icon = ui::action("\u{f057e}", "", move || {
        if let Some(n) = node_in(&pw_m.snapshot(), id) {
          pipewire::set_mute(id, !n.muted);
        }
      });
      mute_icon.set_size_request(space(22.0), -1);
      top.append(&mute_icon);
      let l = ui::label(&stream_label(n, &players), "audio-stream-name");
      l.set_hexpand(true);
      l.set_xalign(0.0);
      l.set_ellipsize(pango::EllipsizeMode::End);
      top.append(&l);
      let p = ui::label("", "audio-percent");
      p.set_size_request(space(36.0), -1);
      p.set_xalign(1.0);
      top.append(&p);
      item.append(&top);
      item.append(&volume_row(id, 1.5, &p));
      let mi = mute_icon.clone();
      up(Box::new(move |snap| {
        if let Some(n) = node_in(snap, id) {
          mi.set_text(if n.muted { "\u{f075f}" } else { "\u{f057e}" });
          mi.set_opacity(if n.muted { 0.5 } else { 1.0 });
        }
      }));
      col.append(&item);
    }
    body.append(&col);
  }
}

fn device_row(glyph: &str, text: &str, space: &impl Fn(f64) -> i32) -> (gtk::Box, gtk::Label) {
  let r = ui::hbox(space(8.0));
  let g = ui::label(glyph, "audio-device-glyph");
  g.set_size_request(space(22.0), -1);
  r.append(&g);
  let l = ui::label(text, "audio-device");
  l.set_xalign(0.0);
  l.set_hexpand(true);
  l.set_ellipsize(pango::EllipsizeMode::End);
  r.append(&l);
  (r, l)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn names_and_labels() {
    assert_eq!(volume_name(0.4, false), "Som ambiente");
    assert_eq!(volume_name(1.0, false), "Sala de concerto");
    assert_eq!(volume_name(0.4, true), "Mudo");
    assert_eq!(friendly_device("Built-in Audio Analog Stereo Output"), "Analog Stereo");
    let mut n = Node { nick: "ES1371 DAC2/ADC".into(), ..Default::default() };
    assert_eq!(node_label(&n), "ES1371 DAC2/ADC");
    n.props.insert("device.product.name".into(), "USB Headset".into());
    assert!(is_headphones(&n));
    let mut s = Node::default();
    s.props.insert("application.name".into(), "audio-src".into());
    assert_eq!(stream_label(&s, &["Spotify".into()]), "Spotify");
    assert_eq!(parse_availability("a\t1\nb\t0\n").get("b"), Some(&false));
  }
}
