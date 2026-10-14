// omarchy.bluetooth (shell/plugins/panels/bluetooth): the Bluetooth icon
// (off, on, connected), only with an adapter. Its panel: the switch
// (omarchy-bluetooth-power) over rotating phrases, then the connected
// devices, the paired ones and, while it searches (it does while open),
// those around; a click connects, pairs or disconnects
// (omarchy-bluetooth-device), and paired ones can be forgotten. A device
// that connects with an audio output becomes the default output. IPC
// target "omarchy.bluetooth": open, close, show, hide, toggle.

use super::super::member::Member;
use super::super::panel::Panel;
use super::super::ui::{self, CursorRow};
use super::super::widget::{Button, Kind, Slot};
use super::super::Ctx;
use crate::bluez::{self, Device, Snapshot};
use crate::pipewire;
use gtk::prelude::*;
use gtk::{gdk, glib, pango};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

const PHRASES: [&str; 8] = [
  "Desembaraçando fios", "Transmitindo vikings", "Pareando mistérios", "Reunindo fones", "Domando rádios", "Invocando caixas de som", "Negociando codecs", "Polindo pacotes",
];

pub fn icon(snap: &Snapshot) -> &'static str {
  match &snap.adapter {
    None => "",
    Some(a) if !a.powered => "\u{f00b2}",
    _ if snap.devices.iter().any(|d| d.connected && bluez::has_human_name(d)) => "\u{f00b1}",
    _ => "\u{f00af}",
  }
}

// (connected, paired, around), each by name.
pub fn lists(snap: &Snapshot) -> (Vec<Device>, Vec<Device>, Vec<Device>) {
  let (mut c, mut k, mut d) = (Vec::new(), Vec::new(), Vec::new());
  for dev in snap.devices.iter().filter(|d| bluez::has_human_name(d)) {
    if dev.connected {
      c.push(dev.clone());
    } else if dev.known() {
      k.push(dev.clone());
    } else {
      d.push(dev.clone());
    }
  }
  for l in [&mut c, &mut k, &mut d] {
    l.sort_by_key(|x| x.label().to_lowercase());
  }
  (c, k, d)
}

fn normalized(a: &str) -> String {
  a.to_lowercase().chars().filter(|c| c.is_ascii_hexdigit()).collect()
}

// The PipeWire output of a Bluetooth device: its address or name in the
// node's names.
fn audio_sink(dev: &Device) -> Option<pipewire::Node> {
  let snap = pipewire::get().snapshot();
  let addr = normalized(&dev.address);
  let label = dev.label().to_lowercase();
  snap.sinks().into_iter().find(|n| {
    let text = [n.name.as_str(), &n.description, &n.nick, n.prop("api.bluez5.address"), n.prop("device.description"), n.prop("device.alias")].join(" ").to_lowercase();
    (!addr.is_empty() && normalized(&text).contains(&addr)) || (!label.is_empty() && text.contains(&label))
  }).cloned()
}

pub fn build(ctx: &Rc<Ctx>) -> Member {
  let space = { let s = ctx.style.clone(); move |px: f64| s.space(px) };
  let bz = bluez::get();
  let button = Button::new(ctx, Kind::Icon, "");
  let slot = Slot::new(ctx, &button.widget);
  let panel = Panel::new(ctx, &button.widget, &slot, space(380.0));
  let pending: Rc<RefCell<HashMap<String, &'static str>>> = Rc::default();
  let phrase = Rc::new(std::cell::Cell::new(0usize));
  let cursor = ui::Cursor::new();
  let body = ui::vbox(space(14.0));
  let scroll = gtk::ScrolledWindow::new();
  scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
  scroll.set_propagate_natural_height(true);
  scroll.set_max_content_height(space(560.0));
  scroll.set_child(Some(&body));
  panel.content.append(&scroll);

  let toggle_power: Rc<dyn Fn()> = {
    let bz = bz.clone();
    Rc::new(move || {
      let on = bz.snapshot().adapter.as_ref().is_some_and(|a| a.powered);
      crate::run::argv(&["omarchy-bluetooth-power", if on { "off" } else { "on" }]);
    })
  };
  let device_action: Rc<dyn Fn(&Device, &'static str, &'static str)> = {
    let (pending, bz) = (pending.clone(), bz.clone());
    Rc::new(move |dev: &Device, action: &'static str, state: &'static str| {
      pending.borrow_mut().insert(dev.address.clone(), state);
      crate::run::argv(&["omarchy-bluetooth-device", action, &dev.address]);
      // Gone after 20 s whatever happened.
      let (pending, addr, bz) = (pending.clone(), dev.address.clone(), bz.clone());
      glib::timeout_add_local_once(Duration::from_secs(20), move || {
        pending.borrow_mut().remove(&addr);
        bz.refresh();
      });
    })
  };

  let render: Rc<dyn Fn()> = {
    let (bz, b, body, cursor, panel_w, pending, phrase) = (bz.clone(), button.clone(), body.clone(), cursor.clone(), Rc::downgrade(&panel), pending.clone(), phrase.clone());
    let (toggle_power, device_action) = (toggle_power.clone(), device_action.clone());
    let space = space.clone();
    Rc::new(move || {
      let snap = bz.snapshot();
      let ic = icon(&snap);
      b.set_text(ic);
      b.widget.set_visible(snap.adapter.is_some());
      // Pending actions that came true, and the audio switch on connect.
      {
        let mut p = pending.borrow_mut();
        let done: Vec<(String, &'static str)> = p
          .iter()
          .filter(|(addr, state)| {
            let d = snap.devices.iter().find(|d| d.address == **addr);
            match **state {
              "connecting" => d.is_some_and(|d| d.connected),
              "disconnecting" => d.is_some_and(|d| !d.connected),
              _ => d.is_none_or(|d| !d.known()),
            }
          })
          .map(|(a, s)| (a.clone(), *s))
          .collect();
        for (addr, state) in done {
          p.remove(&addr);
          if state == "connecting" {
            if let Some(dev) = snap.devices.iter().find(|d| d.address == addr).cloned() {
              // The output shows up a moment after the connection.
              let tries = Rc::new(std::cell::Cell::new(0));
              glib::timeout_add_local(Duration::from_millis(500), move || {
                if let Some(sink) = audio_sink(&dev) {
                  pipewire::set_default(sink.id);
                  crate::run::argv(&["omarchy-audio-output-set-default", &sink.id.to_string(), &sink.name]);
                  return glib::ControlFlow::Break;
                }
                tries.set(tries.get() + 1);
                if tries.get() >= 8 { glib::ControlFlow::Break } else { glib::ControlFlow::Continue }
              });
            }
          }
        }
      }
      let Some(panel) = panel_w.upgrade() else { return };
      if !panel.is_open() {
        return;
      }
      crate::popup::clear(&body);
      cursor.clear();
      let adapter = snap.adapter.clone();
      let powered = adapter.as_ref().is_some_and(|a| a.powered);
      let hero = ui::hbox(space(14.0));
      let hi = ui::label(ic, "panel-hero-icon");
      hi.set_opacity(if powered { 1.0 } else { 0.5 });
      hero.append(&hi);
      let labels = ui::vbox(space(2.0));
      labels.set_hexpand(true);
      labels.set_valign(gtk::Align::Center);
      let t = ui::label("Bluetooth", "panel-hero-title");
      t.set_xalign(0.0);
      labels.append(&t);
      let status = match &adapter {
        None => "Sem adaptador".to_string(),
        Some(a) if !a.powered => "Desligado".to_string(),
        _ => PHRASES[phrase.get() % PHRASES.len()].to_string(),
      };
      let s = ui::label(&status.to_uppercase(), "panel-hero-subtitle");
      s.set_xalign(0.0);
      labels.append(&s);
      hero.append(&labels);
      if adapter.is_some() {
        let tp = toggle_power.clone();
        let sw = ui::state_switch(powered, move || tp());
        sw.set_tooltip_text(Some(if powered { "Desligar Bluetooth" } else { "Ligar Bluetooth" }));
        hero.append(&sw);
        let tp = toggle_power.clone();
        cursor.add(CursorRow { widget: sw.upcast(), activate: Some(Rc::new(move || tp())), adjust: None, mute: None });
      }
      body.append(&hero);

      let (connected, known, around) = lists(&snap);
      let discovering = adapter.as_ref().is_some_and(|a| a.discovering);
      let row = |dev: &Device, section: &str| {
        let r = ui::hbox(space(10.0));
        r.append(&ui::label(if dev.connected { "\u{f00b1}" } else { "\u{f00af}" }, "network-signal"));
        let names = ui::vbox(0);
        names.set_hexpand(true);
        let n = ui::label(&if dev.label().is_empty() { "Dispositivo".into() } else { dev.label() }, if dev.connected { "network-ssid bold" } else { "network-ssid" });
        n.set_xalign(0.0);
        n.set_ellipsize(pango::EllipsizeMode::End);
        names.append(&n);
        let p = pending.borrow().get(&dev.address).copied();
        let status = match p {
          Some("forgetting") => "Esquecendo…".to_string(),
          Some("disconnecting") => "Desconectando…".to_string(),
          Some("connecting") => "Conectando…".to_string(),
          _ if dev.connected && section != "connected" => "Conectado".to_string(),
          _ => dev.battery.map(|b| format!("{b}%")).unwrap_or_default(),
        };
        if !status.is_empty() {
          let st = ui::label(&status, "network-status");
          st.set_xalign(0.0);
          names.append(&st);
        }
        r.append(&names);
        if dev.known() {
          let (d2, da) = (dev.clone(), device_action.clone());
          r.append(&ui::action("\u{f0159}", "Esquecer", move || da(&d2, "forget", "forgetting")));
        }
        let activate: Rc<dyn Fn()> = {
          let (d2, da) = (dev.clone(), device_action.clone());
          Rc::new(move || {
            if d2.connected {
              da(&d2, "disconnect", "disconnecting");
            } else if d2.known() {
              da(&d2, "connect", "connecting");
            } else {
              da(&d2, "pair", "connecting");
            }
          })
        };
        let a2 = activate.clone();
        let w = ui::row(&r, move |_| a2());
        w.set_tooltip_text(Some(if dev.connected { "Desconectar" } else if section == "discovered" { "Parear" } else { "Conectar" }));
        if dev.connected {
          w.add_css_class("selected");
        }
        cursor.add(CursorRow { widget: w.clone().upcast(), activate: Some(activate), adjust: None, mute: None });
        w
      };
      if !connected.is_empty() {
        body.append(&ui::separator());
        let col = ui::vbox(space(4.0));
        col.append(&ui::section("Conectados"));
        for d in &connected {
          col.append(&row(d, "connected"));
        }
        body.append(&col);
      }
      let show_around = discovering && !around.is_empty();
      if !known.is_empty() || show_around {
        body.append(&ui::separator());
        let col = ui::vbox(space(4.0));
        if !known.is_empty() {
          col.append(&ui::section("Pareados"));
          for d in &known {
            col.append(&row(d, "known"));
          }
        }
        if show_around {
          let h = ui::section("Disponíveis");
          if !known.is_empty() {
            h.set_margin_top(space(8.0));
          }
          col.append(&h);
          for d in &around {
            col.append(&row(d, "discovered"));
          }
        }
        body.append(&col);
      }
      if connected.is_empty() && known.is_empty() && !show_around {
        let msg = match &adapter {
          None => "Nenhum adaptador Bluetooth",
          Some(a) if !a.powered => "Ligue o Bluetooth para procurar",
          _ => "Procurando dispositivos…",
        };
        let l = ui::label(msg, "tray-manage-empty");
        l.set_xalign(0.0);
        body.append(&l);
      }
      panel.relayout();
    })
  };
  render();
  let (r, w) = (render.clone(), button.widget.downgrade());
  bz.connect(move || {
    if w.upgrade().is_some() {
      r();
    }
  });
  {
    // Searching while open (tried every second until it runs), stopped
    // when it closes if we started it; the phrase turns every 2.8 s.
    let (bz2, panel_w, c, r, phrase) = (bz.clone(), Rc::downgrade(&panel), cursor.clone(), render.clone(), phrase.clone());
    let started = Rc::new(std::cell::Cell::new(false));
    let s2 = started.clone();
    panel.connect_open(move || {
      c.reset();
      r();
      let (bz2, panel_w, r, phrase, started) = (bz2.clone(), panel_w.clone(), r.clone(), phrase.clone(), s2.clone());
      glib::spawn_future_local(async move {
        let mut tick = 0u32;
        while panel_w.upgrade().is_some_and(|p| p.is_open()) {
          let a = bz2.snapshot().adapter.clone();
          if a.as_ref().is_some_and(|a| a.powered && !a.discovering) {
            bz2.set_discovering(true);
            started.set(true);
          }
          glib::timeout_future(Duration::from_secs(1)).await;
          tick += 1;
          if tick % 3 == 0 {
            phrase.set(phrase.get() + 1);
            r();
          }
        }
      });
    });
    let bz2 = bz.clone();
    panel.connect_close(move || {
      if started.replace(false) {
        bz2.set_discovering(false);
      }
    });
    let (c, tp) = (cursor.clone(), toggle_power.clone());
    panel.connect_key(move |k, _| match k {
      gdk::Key::b | gdk::Key::B => {
        tp();
        true
      }
      _ => c.key(k),
    });
  }
  if ctx.first_screen {
    if let Some(bar) = ctx.bar.upgrade() {
      let p = Rc::downgrade(&panel);
      bar.add_ipc("omarchy.bluetooth", Box::new(move |method, _| {
        let panel = p.upgrade()?;
        match method {
          "open" | "show" => panel.open(),
          "close" | "hide" => panel.close(),
          "toggle" => panel.toggle(),
          _ => return None,
        }
        Some("ok".into())
      }));
    }
  }
  Member { id: "omarchy.bluetooth".into(), title: "Bluetooth".into(), button, slot, panel, wheel: None, right: None }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn icon_and_lists() {
    let mut snap = Snapshot::default();
    assert_eq!(icon(&snap), "");
    snap.adapter = Some(bluez::Adapter { path: "/org/bluez/hci0".into(), powered: true, discovering: false });
    snap.devices = vec![
      Device { alias: "Fone".into(), address: "AA".into(), connected: true, paired: true, ..Default::default() },
      Device { alias: "Caixa".into(), address: "BB".into(), paired: true, ..Default::default() },
      Device { alias: "11:22:33:44:55:66".into(), ..Default::default() },
      Device { alias: "TV".into(), address: "CC".into(), ..Default::default() },
    ];
    assert_eq!(icon(&snap), "\u{f00b1}");
    let (c, k, d) = lists(&snap);
    assert_eq!((c.len(), k.len(), d.len()), (1, 1, 1));
  }
}
