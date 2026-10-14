// omarchy.network (shell/plugins/panels/network): the connection's icon
// (Wi-Fi signal, cable, or none), kept current by `nmcli monitor`. Its
// panel: the connection (SSID or Ethernet, with the link speed) in rotating
// phrases, the Wi-Fi QR code, the speed test and the Wi-Fi switch; ping,
// loss, throughput and totals, IP and gateway (a click copies them); the
// Wi-Fi band (omarchy-network-band); the DNS provider (omarchy-dns); and
// the Wi-Fi networks to connect to, disconnect or forget, asking for the
// password (and an identity on 802.1X) in place. IPC target
// "omarchy.network": open, close, show, hide, toggle, toggleNetwork,
// showQr, speedTest.

use super::super::member::Member;
use super::super::panel::Panel;
use super::super::ui::{self, CursorRow};
use super::super::widget::{self, Button, Kind, Slot};
use super::super::Ctx;
use super::network_model as m;
use gtk::prelude::*;
use gtk::{gdk, gio, glib, pango};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

const PHRASES: [&str; 7] = ["Ligando bits", "Manuseando pacotes", "Ordenando quadros", "Carregando bytes", "Roteando migalhas", "Contando colisões", "Curvando a luz"];

#[derive(Default)]
struct State {
  info: HashMap<String, String>,
  // From the short status: kind, label, signal.
  kind: String,
  signal: i32,
  pings: m::Pings,
  rx: Option<(f64, f64, Instant, String)>,
  down: f64,
  up: f64,
  band: (String, String, Vec<String>),
  pending_band: String,
  dns: String,
  wifi: Vec<m::Wifi>,
  wifi_device: bool,
  wifi_enabled: bool,
  scanning: bool,
  action: Option<(String, &'static str)>,
  failure: Option<(String, String)>,
  password_for: String,
  phrase: usize,
}

async fn run_with_stdin(argv: &[&str], input: &str) -> (bool, String) {
  let args: Vec<&std::ffi::OsStr> = argv.iter().map(|s| s.as_ref()).collect();
  let Ok(child) = gio::Subprocess::newv(&args, gio::SubprocessFlags::STDIN_PIPE | gio::SubprocessFlags::STDOUT_SILENCE | gio::SubprocessFlags::STDERR_PIPE) else {
    return (false, String::new());
  };
  let res = child.communicate_utf8_future(Some(input.to_string())).await;
  let err = res.ok().and_then(|(_, e)| e).map(|s| s.to_string()).unwrap_or_default();
  (child.is_successful(), err)
}

async fn run(argv: &[&str]) -> (bool, String) {
  run_with_stdin(argv, "").await
}

pub fn build(ctx: &Rc<Ctx>) -> Member {
  let space = { let s = ctx.style.clone(); move |px: f64| s.space(px) };
  let button = Button::new(ctx, Kind::Icon, "\u{f092e}");
  let slot = Slot::new(ctx, &button.widget);
  let panel = Panel::new(ctx, &button.widget, &slot, space(440.0));
  let st = Rc::new(RefCell::new(State { dns: "DHCP".into(), ..State::default() }));
  let cursor = ui::Cursor::new();
  let bar = ctx.bar.clone();

  let scroll = gtk::ScrolledWindow::new();
  scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
  scroll.set_propagate_natural_height(true);
  scroll.set_max_content_height(space(640.0));
  let body = ui::vbox(space(14.0));
  scroll.set_child(Some(&body));
  panel.content.append(&scroll);

  // The password being typed lives here, not in the state the panel is
  // drawn from, so a redraw never loses what was typed.
  let password = Rc::new(RefCell::new((String::new(), String::new())));
  let again: Rc<RefCell<std::rc::Weak<dyn Fn()>>> = Rc::new(RefCell::new(std::rc::Weak::<Box<dyn Fn()>>::new() as std::rc::Weak<dyn Fn()>));
  let redraw = {
    let a = again.clone();
    move || {
      if let Some(f) = a.borrow().upgrade() {
        f();
      }
    }
  };

  // ---- Reading.
  let read_status: Rc<dyn Fn()> = {
    let (st, b, redraw) = (st.clone(), button.clone(), redraw.clone());
    Rc::new(move || {
      let (st, b, redraw) = (st.clone(), b.clone(), redraw.clone());
      glib::spawn_future_local(async move {
        let short = crate::watch::output(&["omarchy-network-status"]).await.unwrap_or_default();
        let parts: Vec<&str> = short.trim_end().split('\t').collect();
        let kind = parts.first().filter(|k| !k.is_empty()).unwrap_or(&"disconnected").to_string();
        let signal = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(-1);
        let label = parts.get(1).unwrap_or(&"").to_string();
        let devs = crate::watch::output(&["nmcli", "-t", "-f", "TYPE", "dev"]).await.unwrap_or_default();
        let radio = crate::watch::output(&["nmcli", "radio", "wifi"]).await.unwrap_or_default();
        {
          let mut s = st.borrow_mut();
          s.kind = kind.clone();
          s.signal = signal;
          s.wifi_device = devs.lines().any(|l| l.trim() == "wifi");
          s.wifi_enabled = radio.trim() == "enabled";
        }
        b.set_text(m::connection_icon(&kind, signal));
        b.set_tooltip(&label);
        redraw();
      });
    })
  };
  let read_details: Rc<dyn Fn(bool)> = {
    let (st, redraw) = (st.clone(), redraw.clone());
    Rc::new(move |scan: bool| {
      let (st, redraw) = (st.clone(), redraw.clone());
      glib::spawn_future_local(async move {
        let raw = crate::watch::output(&["omarchy-network-status", "--verbose"]).await.unwrap_or_default();
        let info = m::parse_key_value(&raw);
        let band = crate::watch::output(&["omarchy-network-band"]).await.unwrap_or_default();
        let dns = crate::watch::output(&["bash", "-c", "omarchy-dns"]).await.unwrap_or_default();
        {
          let mut s = st.borrow_mut();
          let pending = !s.pending_band.is_empty();
          if !(pending && !info.contains_key("iface")) {
            // Throughput from the counters' change since the last read.
            let iface = info.get("iface").cloned().unwrap_or_default();
            let rx = info.get("rx_bytes").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
            let tx = info.get("tx_bytes").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
            let now = Instant::now();
            if let Some((prx, ptx, t, pif)) = s.rx.clone() {
              let dt = now.duration_since(t).as_secs_f64();
              if pif == iface && dt > 0.0 {
                s.down = ((rx - prx) / dt).max(0.0);
                s.up = ((tx - ptx) / dt).max(0.0);
              }
            }
            s.rx = Some((rx, tx, now, iface));
            s.pings.push(&info, 24);
            s.info = info;
          }
          let parsed = m::parse_band(&band);
          if !(pending && parsed.2.is_empty()) {
            s.band = parsed;
          }
          if !dns.trim().is_empty() {
            s.dns = dns.trim().to_string();
          }
        }
        redraw();
        if scan {
          st.borrow_mut().scanning = true;
          redraw();
          let _ = run(&["nmcli", "dev", "wifi", "rescan"]).await;
          glib::timeout_future(Duration::from_millis(1500)).await;
        }
        let list = crate::watch::output(&["nmcli", "-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "dev", "wifi", "list"]).await.unwrap_or_default();
        let saved = crate::watch::output(&["nmcli", "-t", "-f", "NAME,TYPE", "connection", "show"]).await.unwrap_or_default();
        let known: Vec<String> = saved.lines().filter_map(|l| l.rsplit_once(':')).filter(|(_, t)| t.contains("wireless")).map(|(n, _)| n.replace("\\:", ":")).collect();
        {
          let mut s = st.borrow_mut();
          s.wifi = m::parse_wifi(&list, &known);
          s.scanning = false;
        }
        redraw();
      });
    })
  };
  read_status();
  // Changes as NetworkManager reports them, and every 30 s besides.
  {
    let (r, pending) = (read_status.clone(), Rc::new(Cell::new(false)));
    let w = button.widget.downgrade();
    crate::watch::lines(&["nmcli", "monitor"], move |_| {
      if w.upgrade().is_none() || pending.replace(true) {
        return;
      }
      let (r, pending) = (r.clone(), pending.clone());
      glib::timeout_add_local_once(Duration::from_millis(400), move || {
        pending.set(false);
        r();
      });
    });
    let r = read_status.clone();
    let w = button.widget.downgrade();
    glib::timeout_add_seconds_local(30, move || {
      if w.upgrade().is_none() {
        return glib::ControlFlow::Break;
      }
      r();
      glib::ControlFlow::Continue
    });
  }

  // ---- Actions.
  let toggle_wifi: Rc<dyn Fn()> = {
    let (st, rs) = (st.clone(), read_status.clone());
    Rc::new(move || {
      let on = st.borrow().wifi_enabled;
      let rs = rs.clone();
      glib::spawn_future_local(async move {
        let _ = run(&["nmcli", "radio", "wifi", if on { "off" } else { "on" }]).await;
        rs();
      });
    })
  };
  let summon = |plugin: &'static str, payload: Rc<dyn Fn() -> String>| -> Rc<dyn Fn()> {
    let p = Rc::downgrade(&panel);
    Rc::new(move || {
      if let Some(p) = p.upgrade() {
        p.close();
      }
      crate::run::argv(&["omarchy-shell", "shell", "summon", plugin, &payload()]);
    })
  };
  let show_qr = {
    let st = st.clone();
    summon(
      "omarchy.wifiqr",
      Rc::new(move || {
        let s = st.borrow();
        let mut p = serde_json::Map::new();
        if s.info.get("type").map(String::as_str) == Some("wifi") {
          if let Some(i) = s.info.get("iface") {
            p.insert("iface".into(), i.clone().into());
          }
          if let Some(ssid) = s.info.get("ssid") {
            p.insert("ssid".into(), ssid.clone().into());
          }
        }
        serde_json::Value::Object(p).to_string()
      }),
    )
  };
  let speed_test = {
    let st = st.clone();
    summon(
      "omarchy.speedtest",
      Rc::new(move || {
        let s = st.borrow();
        let c = match s.info.get("type").map(String::as_str) {
          Some("wifi") => s.info.get("ssid").cloned().unwrap_or_else(|| "Wi-Fi".into()),
          Some("ethernet") => "Ethernet".into(),
          _ => String::new(),
        };
        if c.is_empty() { "{}".into() } else { serde_json::json!({ "connection": c }).to_string() }
      }),
    )
  };
  // A Wi-Fi action, with its "…ando" state and a 30 s limit.
  let act: Rc<dyn Fn(String, &'static str, Vec<String>, String)> = {
    let (st, rd, redraw) = (st.clone(), read_details.clone(), redraw.clone());
    Rc::new(move |ssid: String, kind: &'static str, argv: Vec<String>, secret: String| {
      if st.borrow().action.is_some() {
        return;
      }
      {
        let mut s = st.borrow_mut();
        s.action = Some((ssid.clone(), kind));
        s.failure = None;
      }
      redraw();
      let (st, rd, redraw) = (st.clone(), rd.clone(), redraw.clone());
      glib::spawn_future_local(async move {
        let args: Vec<&str> = argv.iter().map(String::as_str).collect();
        let started = Instant::now();
        let (ok, err) = run_with_stdin(&args, &(secret + "\n")).await;
        {
          let mut s = st.borrow_mut();
          s.action = None;
          if ok {
            s.password_for.clear();
          } else {
            let needs = s.wifi.iter().find(|n| n.ssid == ssid).map(|n| n.needs_credentials()).unwrap_or(true);
            let reason = if started.elapsed() > Duration::from_secs(30) {
              match kind {
                "connect" => "Tempo esgotado ao conectar",
                "disconnect" => "Tempo esgotado ao desconectar",
                _ => "Tempo esgotado ao esquecer",
              }
            } else {
              m::failure_reason(&err, needs)
            };
            s.failure = Some((ssid.clone(), reason.to_string()));
            // A missing or wrong password asks again.
            if kind == "connect" && needs && (reason == "Senha necessária" || reason == "Senha incorreta") {
              s.password_for = ssid.clone();
            }
          }
        }
        redraw();
        rd(false);
      });
    })
  };
  let set_dns: Rc<dyn Fn(&str)> = {
    let (st, p) = (st.clone(), Rc::downgrade(&panel));
    Rc::new(move |provider: &str| {
      if provider == "Custom" {
        crate::run::shell("omarchy-launch-floating-terminal-with-presentation 'omarchy-dns Custom'");
      } else {
        let (st, provider) = (st.clone(), provider.to_string());
        glib::spawn_future_local(async move {
          let (ok, _) = run(&["bash", "-c", &format!("omarchy-dns '{provider}'")]).await;
          if ok {
            st.borrow_mut().dns = provider;
          }
        });
      }
      if let Some(p) = p.upgrade() {
        p.close();
      }
    })
  };
  let set_band: Rc<dyn Fn(String)> = {
    let (st, rd, redraw) = (st.clone(), read_details.clone(), redraw.clone());
    Rc::new(move |band: String| {
      if !st.borrow().pending_band.is_empty() {
        return;
      }
      st.borrow_mut().pending_band = band.clone();
      redraw();
      let (st, rd) = (st.clone(), rd.clone());
      glib::spawn_future_local(async move {
        let (ok, _) = run(&["omarchy-network-band", &band]).await;
        {
          let mut s = st.borrow_mut();
          if ok {
            s.band.1 = band;
          }
          s.pending_band.clear();
        }
        rd(false);
      });
    })
  };

  // ---- Drawing the panel.
  let render: Rc<dyn Fn()> = {
    let (st, body, cursor, panel_w, password) = (st.clone(), body.clone(), cursor.clone(), Rc::downgrade(&panel), password.clone());
    let (toggle_wifi, show_qr, speed_test, act, set_dns, set_band) = (toggle_wifi.clone(), show_qr.clone(), speed_test.clone(), act.clone(), set_dns.clone(), set_band.clone());
    let space = space.clone();
    let again = again.clone();
    Rc::new(move || {
      let Some(panel) = panel_w.upgrade() else { return };
      if !panel.is_open() {
        return;
      }
      let s = st.borrow();
      crate::popup::clear(&body);
      cursor.clear();
      let info = &s.info;
      let itype = info.get("type").map(String::as_str).unwrap_or("");
      let connected_wifi = s.wifi.iter().find(|n| n.connected).cloned();

      // Hero.
      let hero = ui::hbox(space(14.0));
      hero.append(&ui::label(m::connection_icon(&s.kind, s.signal), "panel-hero-icon"));
      let labels = ui::vbox(space(2.0));
      labels.set_hexpand(true);
      labels.set_valign(gtk::Align::Center);
      let title = match itype {
        "wifi" => info.get("ssid").cloned().unwrap_or_else(|| "Wi-Fi".into()),
        "ethernet" => "Ethernet".into(),
        _ => info.get("iface").cloned().unwrap_or_else(|| if s.kind == "disconnected" { "Desconectado".into() } else { "Sem conexão".into() }),
      };
      let detail = if itype == "ethernet" { m::format_speed(info.get("speed").map(String::as_str).unwrap_or("")) } else { String::new() };
      let t = ui::label(&if detail.is_empty() { title } else { format!("{title} ({detail})") }, "panel-hero-title");
      t.set_xalign(0.0);
      t.set_ellipsize(pango::EllipsizeMode::End);
      labels.append(&t);
      let phrase = PHRASES[s.phrase % PHRASES.len()].to_uppercase();
      let meta = match itype {
        "wifi" if connected_wifi.is_some() => phrase,
        "ethernet" => phrase,
        _ if s.kind == "disconnected" => "NÃO CONECTADO".into(),
        _ => String::new(),
      };
      let mt = ui::label(&meta, "panel-hero-subtitle");
      mt.set_xalign(0.0);
      mt.set_visible(!meta.is_empty());
      labels.append(&mt);
      hero.append(&labels);
      let can_share = itype == "wifi" && connected_wifi.as_ref().is_some_and(|n| !n.enterprise());
      if can_share {
        let q = show_qr.clone();
        let a = ui::action("\u{f0432}", "Mostrar QR code", move || q());
        hero.append(&a);
        let q = show_qr.clone();
        cursor.add(CursorRow { widget: a.upcast(), activate: Some(Rc::new(move || q())), adjust: None, mute: None });
      }
      if info.contains_key("iface") {
        let q = speed_test.clone();
        let a = ui::action("\u{f04c5}", "Testar velocidade", move || q());
        hero.append(&a);
        let q = speed_test.clone();
        cursor.add(CursorRow { widget: a.upcast(), activate: Some(Rc::new(move || q())), adjust: None, mute: None });
      }
      if s.wifi_device {
        let tw = toggle_wifi.clone();
        let sw = gtk::Switch::new();
        sw.set_state(s.wifi_enabled);
        sw.set_valign(gtk::Align::Center);
        sw.set_tooltip_text(Some(if s.wifi_enabled { "Desligar Wi-Fi" } else { "Ligar Wi-Fi" }));
        sw.connect_state_set(move |_, _| {
          tw();
          glib::Propagation::Stop
        });
        hero.append(&sw);
        let tw = toggle_wifi.clone();
        cursor.add(CursorRow { widget: sw.upcast(), activate: Some(Rc::new(move || tw())), adjust: None, mute: None });
      }
      body.append(&hero);

      // Details.
      if info.contains_key("iface") {
        let grid = gtk::Grid::new();
        grid.set_column_spacing(space(20.0) as u32);
        grid.set_row_spacing(space(4.0) as u32);
        grid.set_column_homogeneous(true);
        let has_ping = !s.pings.internet.is_empty();
        let has_transfer = info.contains_key("rx_bytes");
        let total = |k: &str| if has_transfer { m::format_bytes(info.get(k).and_then(|v| v.parse().ok()).unwrap_or(0.0)) } else { "--".into() };
        let rate = |v: f64| if has_transfer { format!("{}/s", m::format_bytes(v)) } else { "--".into() };
        let rows: [(&str, String, bool); 8] = [
          ("Ping", m::format_ping(m::Pings::average(&s.pings.internet, 5), has_ping), false),
          ("Perda de pacotes", if has_ping { format!("{}%", m::Pings::loss(&s.pings.internet)) } else { "--".into() }, false),
          ("Recebendo", rate(s.down), false),
          ("Enviando", rate(s.up), false),
          ("Baixado", total("rx_bytes"), false),
          ("Enviado", total("tx_bytes"), false),
          ("Endereço IP", info.get("ip").cloned().unwrap_or_else(|| "--".into()), true),
          ("Gateway", info.get("gateway").cloned().unwrap_or_else(|| "--".into()), true),
        ];
        for (i, (label, value, copy)) in rows.into_iter().enumerate() {
          let b = ui::hbox(space(8.0));
          let l = ui::label(label, "power-info-label");
          l.set_hexpand(true);
          l.set_xalign(0.0);
          b.append(&l);
          let v = ui::label(&value, "power-info-value");
          if copy && value != "--" {
            v.add_css_class("copyable");
            v.set_cursor_from_name(Some("pointer"));
            v.set_tooltip_text(Some(if label == "Gateway" { "Copiar gateway" } else { "Copiar IP" }));
            widget::on_press(&v, move |_| {
              if let Some(d) = gdk::Display::default() {
                d.clipboard().set_text(&value);
              }
            });
          }
          b.append(&v);
          grid.attach(&b, (i % 2) as i32, (i / 2) as i32, 1, 1);
        }
        body.append(&grid);
      }

      // The Wi-Fi band.
      let (current, selected, available) = &s.band;
      let effective = if s.pending_band.is_empty() { selected.clone() } else { s.pending_band.clone() };
      let pinned = effective != "auto";
      if (s.kind == "wifi" || !s.pending_band.is_empty()) && (available.len() > 1 || pinned) {
        body.append(&ui::separator());
        let col = ui::vbox(space(8.0));
        let head = gtk::CenterBox::new();
        head.set_start_widget(Some(&ui::label(&m::band_title(&effective, current), "panel-section")));
        let auto = ui::hbox(space(8.0));
        auto.append(&ui::label("AUTOMÁTICO", "panel-section"));
        let sb = set_band.clone();
        let cur = current.clone();
        let sw = gtk::Switch::new();
        sw.set_state(!pinned);
        sw.set_tooltip_text(Some(&if pinned { "Deixar o Wi-Fi escolher a banda".to_string() } else { format!("Manter em {}", m::band_label(current)) }));
        let pinned_now = pinned;
        sw.connect_state_set(move |_, _| {
          if pinned_now {
            sb("auto".into());
          } else if !cur.is_empty() {
            sb(cur.clone());
          }
          glib::Propagation::Stop
        });
        auto.append(&sw);
        head.set_end_widget(Some(&auto));
        col.append(&head);
        if pinned {
          let pills = ui::hbox(space(6.0));
          pills.set_homogeneous(true);
          for b in available {
            let sb = set_band.clone();
            let b2 = b.clone();
            let btn = ui::button(&m::band_label(b), false, move || sb(b2.clone()));
            btn.set_tooltip_text(Some(&format!("Manter em {}", m::band_label(b))));
            btn.add_css_class("scale");
            if *b == effective {
              btn.add_css_class("selected");
            }
            pills.append(&btn);
          }
          col.append(&pills);
        }
        body.append(&col);
      }

      // DNS.
      body.append(&ui::separator());
      let col = ui::vbox(space(8.0));
      col.append(&ui::section("Provedor de DNS"));
      let pills = ui::hbox(space(6.0));
      pills.set_homogeneous(true);
      for (provider, tip) in [
        ("DHCP", "Usar DNS do DHCP"),
        ("Cloudflare", "Usar DNS da Cloudflare"),
        ("Google", "Usar DNS do Google"),
        ("Custom", "Definir servidores DNS personalizados"),
      ] {
        let sd = set_dns.clone();
        let btn = ui::button(provider, false, move || sd(provider));
        btn.set_tooltip_text(Some(tip));
        btn.add_css_class("scale");
        if s.dns == provider {
          btn.add_css_class("selected");
        }
        pills.append(&btn);
        let b2 = btn.clone();
        cursor.add(CursorRow { widget: btn.upcast(), activate: Some(Rc::new(move || b2.emit_clicked())), adjust: None, mute: None });
      }
      col.append(&pills);
      body.append(&col);

      // Wi-Fi networks.
      if s.wifi_device {
        body.append(&ui::separator());
        if s.scanning {
          body.append(&ui::section("Procurando redes Wi-Fi…"));
        }
        let col = ui::vbox(space(4.0));
        for (i, net) in s.wifi.iter().enumerate() {
          let title = m::section_title(&s.wifi, i);
          if !title.is_empty() {
            let h = ui::label(title, "panel-section");
            h.set_xalign(0.0);
            h.set_margin_top(if i == 0 { 0 } else { space(8.0) });
            col.append(&h);
          }
          let busy = s.action.as_ref().filter(|(ssid, _)| *ssid == net.ssid).map(|(_, k)| *k);
          let failed = s.failure.as_ref().filter(|(ssid, _)| *ssid == net.ssid).map(|(_, r)| r.clone());
          let status = match (busy, &failed) {
            (Some("connect"), _) => "Conectando…".to_string(),
            (Some("disconnect"), _) => "Desconectando…".to_string(),
            (Some(_), _) => "Esquecendo…".to_string(),
            (None, Some(r)) => r.clone(),
            _ if net.connected => "Conectado".to_string(),
            _ => String::new(),
          };
          let r = ui::hbox(space(10.0));
          r.append(&ui::label(m::wifi_icon(net.signal), "network-signal"));
          let names = ui::vbox(0);
          names.set_hexpand(true);
          let n = ui::label(if net.ssid.is_empty() { "Oculta" } else { &net.ssid }, if net.connected { "network-ssid bold" } else { "network-ssid" });
          n.set_xalign(0.0);
          n.set_ellipsize(pango::EllipsizeMode::End);
          names.append(&n);
          if !status.is_empty() {
            let st_l = ui::label(&status, if failed.is_some() { "network-status failed" } else { "network-status" });
            st_l.set_xalign(0.0);
            names.append(&st_l);
          }
          r.append(&names);
          let can_forget = net.known && !net.connected;
          if can_forget {
            let ssid = net.ssid.clone();
            let a = act.clone();
            let forget = ui::action("\u{f0159}", "Esquecer rede", move || a(ssid.clone(), "forget", vec!["nmcli".into(), "connection".into(), "delete".into(), "id".into(), ssid.clone()], String::new()));
            r.append(&forget);
          } else if net.needs_credentials() {
            r.append(&ui::label("\u{f033e}", "network-lock"));
          }
          let activate: Rc<dyn Fn()> = {
            let (net, act, st2, again) = (net.clone(), act.clone(), st.clone(), again.clone());
            Rc::new(move || {
              if net.connected {
                act(net.ssid.clone(), "disconnect", vec!["nmcli".into(), "connection".into(), "down".into(), "id".into(), net.ssid.clone()], String::new());
              } else if net.needs_credentials() && !net.known {
                st2.borrow_mut().password_for = net.ssid.clone();
                if let Some(f) = again.borrow().upgrade() {
                  f();
                }
              } else {
                act(net.ssid.clone(), "connect", vec!["nmcli".into(), "dev".into(), "wifi".into(), "connect".into(), net.ssid.clone()], String::new());
              }
            })
          };
          let a2 = activate.clone();
          let row = ui::row(&r, move |_| a2());
          if net.connected {
            row.add_css_class("selected");
          }
          col.append(&row);
          cursor.add(CursorRow { widget: row.upcast(), activate: Some(activate), adjust: None, mute: None });
          // The password, typed in place.
          if s.password_for == net.ssid && busy.is_none() {
            let prompt = ui::vbox(space(6.0));
            prompt.set_margin_start(space(30.0));
            let identity = gtk::Entry::new();
            identity.add_css_class("field");
            identity.set_placeholder_text(Some("Identidade (usuario@dominio)"));
            identity.set_visible(net.enterprise());
            identity.set_text(&password.borrow().0);
            let pass = gtk::PasswordEntry::new();
            pass.add_css_class("field");
            pass.set_show_peek_icon(true);
            pass.set_property("placeholder-text", "Senha");
            pass.set_text(&password.borrow().1);
            let pw2 = password.clone();
            identity.connect_changed(move |e| pw2.borrow_mut().0 = e.text().to_string());
            let pw2 = password.clone();
            pass.connect_changed(move |e| pw2.borrow_mut().1 = e.text().to_string());
            let submit: Rc<dyn Fn()> = {
              let (net, act, password) = (net.clone(), act.clone(), password.clone());
              Rc::new(move || {
                let (id, secret) = password.borrow().clone();
                if secret.is_empty() {
                  return;
                }
                password.borrow_mut().1.clear();
                if net.enterprise() {
                  act(net.ssid.clone(), "connect", vec!["bash".into(), "-c".into(), m::ENTERPRISE_SCRIPT.into(), "nmcli-eap".into(), net.ssid.clone(), id], secret);
                } else {
                  act(net.ssid.clone(), "connect", vec!["bash".into(), "-c".into(), m::PSK_SCRIPT.into(), "nmcli-psk".into(), net.ssid.clone()], secret);
                }
              })
            };
            let s2 = submit.clone();
            pass.connect_activate(move |_| s2());
            let line = ui::hbox(space(6.0));
            pass.set_hexpand(true);
            line.append(&pass);
            let s2 = submit.clone();
            line.append(&ui::action("\u{f012c}", "Conectar", move || s2()));
            prompt.append(&identity);
            prompt.append(&line);
            // Escape drops the prompt, not the panel.
            let keys = gtk::EventControllerKey::new();
            let (st2, again2) = (st.clone(), again.clone());
            keys.connect_key_pressed(move |_, k, _, _| {
              if k == gdk::Key::Escape {
                st2.borrow_mut().password_for.clear();
                if let Some(f) = again2.borrow().upgrade() {
                  f();
                }
                return glib::Propagation::Stop;
              }
              glib::Propagation::Proceed
            });
            prompt.add_controller(keys);
            col.append(&prompt);
            let focus = if net.enterprise() && identity.text().is_empty() { identity.upcast::<gtk::Widget>() } else { pass.upcast::<gtk::Widget>() };
            glib::idle_add_local_once(move || {
              focus.grab_focus();
            });
          }
        }
        body.append(&col);
      }
      drop(s);
      panel.relayout();
    })
  };
  *again.borrow_mut() = Rc::downgrade(&render);

  {
    let (rd, rs, c, st2, panel_w, render2) = (read_details.clone(), read_status.clone(), cursor.clone(), st.clone(), Rc::downgrade(&panel), render.clone());
    panel.connect_open(move || {
      c.reset();
      {
        let mut s = st2.borrow_mut();
        s.rx = None;
        s.pings = m::Pings::default();
        s.password_for.clear();
      }
      rs();
      rd(true);
      render2();
      // Details every 1.5 s, the phrase every 2.8 s, while open.
      let (rd, st2, panel_w, render2) = (rd.clone(), st2.clone(), panel_w.clone(), render2.clone());
      glib::spawn_future_local(async move {
        let mut tick = 0u32;
        loop {
          glib::timeout_future(Duration::from_millis(1400)).await;
          if !panel_w.upgrade().is_some_and(|p| p.is_open()) {
            break;
          }
          tick += 1;
          if tick % 2 == 0 {
            st2.borrow_mut().phrase += 1;
          }
          // Not while a password is being typed: the prompt would redraw.
          if st2.borrow().password_for.is_empty() {
            rd(false);
          } else {
            let _ = &render2;
          }
        }
      });
    });
    let c = cursor.clone();
    let (rd, tw) = (read_details.clone(), toggle_wifi.clone());
    panel.connect_key(move |k, _| match k {
      gdk::Key::r | gdk::Key::R => {
        rd(true);
        true
      }
      gdk::Key::w | gdk::Key::W => {
        tw();
        true
      }
      _ => c.key(k),
    });
  }
  if ctx.first_screen {
    if let Some(bar) = bar.upgrade() {
      let (p, tw, qr, sp) = (Rc::downgrade(&panel), toggle_wifi.clone(), show_qr.clone(), speed_test.clone());
      bar.add_ipc("omarchy.network", Box::new(move |method, _| {
        let panel = p.upgrade()?;
        match method {
          "open" | "show" => panel.open(),
          "close" | "hide" => panel.close(),
          "toggle" => panel.toggle(),
          "toggleNetwork" => tw(),
          "showQr" => qr(),
          "speedTest" => sp(),
          _ => return None,
        }
        Some("ok".into())
      }));
    }
  }
  Member { id: "omarchy.network".into(), title: "Wi-Fi".into(), button, slot, panel, wheel: None, right: None }
}
