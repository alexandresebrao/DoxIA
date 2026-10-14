// The network panel's data (shell/plugins/panels/network/Model.js):
// omarchy-network-status's key/value lines, the throughput and ping windows,
// nmcli's Wi-Fi list, and the formatting.

use std::collections::HashMap;

pub fn wifi_icon(strength: i32) -> &'static str {
  const ICONS: [&str; 5] = ["\u{f092f}", "\u{f091f}", "\u{f0922}", "\u{f0925}", "\u{f0928}"];
  ICONS[((strength as f64 / 20.0).ceil() as i64 - 1).clamp(0, 4) as usize]
}

pub fn connection_icon(kind: &str, strength: i32) -> &'static str {
  match kind {
    "wifi" => wifi_icon(strength),
    "ethernet" => "\u{f0200}",
    _ => "\u{f092e}",
  }
}

pub fn format_speed(mbps: &str) -> String {
  let v: i64 = mbps.trim().parse().unwrap_or(0);
  if v <= 0 {
    return String::new();
  }
  if v >= 1000 {
    let g = v as f64 / 1000.0;
    if v % 1000 == 0 { format!("{}gbit", v / 1000) } else { format!("{g:.1}gbit") }
  } else {
    format!("{v}mbit")
  }
}

// iw prints non-ASCII SSID bytes as \xNN.
pub fn decode_ssid(raw: &str) -> String {
  let bytes = raw.as_bytes();
  let mut out = Vec::new();
  let mut i = 0;
  while i < bytes.len() {
    if bytes[i] == b'\\' && bytes.get(i + 1) == Some(&b'x') {
      if let Some(b) = raw.get(i + 2..i + 4).and_then(|h| u8::from_str_radix(h, 16).ok()) {
        out.push(b);
        i += 4;
        continue;
      }
    }
    out.push(bytes[i]);
    i += 1;
  }
  String::from_utf8(out).unwrap_or_else(|_| raw.to_string())
}

pub fn parse_key_value(raw: &str) -> HashMap<String, String> {
  raw
    .lines()
    .filter_map(|l| l.split_once('\t'))
    .filter(|(k, _)| !k.is_empty())
    .map(|(k, v)| (k.to_string(), if k == "ssid" { decode_ssid(v) } else { v.trim().to_string() }))
    .collect()
}

// (current band, selected band, available) from omarchy-network-band.
pub fn parse_band(raw: &str) -> (String, String, Vec<String>) {
  let kv = parse_key_value(raw);
  let available = kv.get("available").map(|a| a.split_whitespace().map(String::from).collect()).unwrap_or_default();
  (kv.get("band").cloned().unwrap_or_default(), kv.get("selected").cloned().filter(|s| !s.is_empty()).unwrap_or_else(|| "auto".into()), available)
}

pub fn band_label(band: &str) -> String {
  match band {
    "auto" => "Auto".into(),
    "" => String::new(),
    b => format!("{b}ghz"),
  }
}

pub fn band_title(selected: &str, current: &str) -> String {
  let label = band_label(current);
  if selected != "auto" || label.is_empty() { "BANDA DO WI-FI".into() } else { format!("BANDA DO WI-FI: {}", label.to_uppercase()) }
}

pub fn format_bytes(n: f64) -> String {
  let n = if n.is_finite() && n > 0.0 { n } else { 0.0 };
  if n < 1024.0 {
    format!("{} B", n.round())
  } else if n < 1024.0 * 1024.0 {
    format!("{:.1} KB", n / 1024.0)
  } else if n < 1024.0 * 1024.0 * 1024.0 {
    format!("{:.1} MB", n / (1024.0 * 1024.0))
  } else {
    format!("{:.2} GB", n / (1024.0 * 1024.0 * 1024.0))
  }
}

pub fn format_ping(ms: Option<f64>, has_samples: bool) -> String {
  if !has_samples {
    return "--".into();
  }
  match ms {
    Some(v) if v >= 0.0 => {
      if v > 0.0 && v < 10.0 { format!("{v:.1} ms") } else { format!("{v:.0} ms") }
    }
    _ => "Tempo esgotado".into(),
  }
}

// The last `window` samples (None: lost); the average of the last
// `average` answered ones, and the share lost.
#[derive(Clone, Debug, Default)]
pub struct Pings {
  pub iface: String,
  pub router: Vec<Option<f64>>,
  pub internet: Vec<Option<f64>>,
}

fn sample(v: Option<&String>) -> Option<Option<f64>> {
  let v = v?;
  Some(v.trim().parse::<f64>().ok().filter(|x| x.is_finite() && *x >= 0.0))
}

impl Pings {
  pub fn push(&mut self, info: &HashMap<String, String>, window: usize) {
    let iface = info.get("iface").cloned().unwrap_or_default();
    if iface.is_empty() || iface != self.iface {
      self.router.clear();
      self.internet.clear();
    }
    self.iface = iface;
    for (list, key) in [(&mut self.router, "router_ping_ms"), (&mut self.internet, "internet_ping_ms")] {
      match sample(info.get(key)) {
        Some(s) => {
          list.push(s);
          while list.len() > window {
            list.remove(0);
          }
        }
        None => list.clear(),
      }
    }
  }

  pub fn average(list: &[Option<f64>], last: usize) -> Option<f64> {
    let tail = &list[list.len().saturating_sub(last)..];
    let got: Vec<f64> = tail.iter().flatten().copied().collect();
    (!got.is_empty()).then(|| got.iter().sum::<f64>() / got.len() as f64)
  }

  pub fn loss(list: &[Option<f64>]) -> i64 {
    if list.is_empty() {
      return 0;
    }
    ((list.iter().filter(|s| s.is_none()).count() as f64 / list.len() as f64) * 100.0).round() as i64
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Wifi {
  pub ssid: String,
  pub signal: i32,
  pub security: String,
  pub connected: bool,
  pub known: bool,
}

impl Wifi {
  // Open and OWE (Enhanced Open) ask for nothing.
  pub fn needs_credentials(&self) -> bool {
    let s = self.security.trim();
    !(s.is_empty() || s == "--" || s.contains("OWE"))
  }

  pub fn enterprise(&self) -> bool {
    self.security.contains("802.1X")
  }
}

// `nmcli -t -f IN-USE,SSID,SIGNAL,SECURITY dev wifi list`, with the saved
// Wi-Fi connections' names: one row per SSID (the strongest), hidden ones
// out, connected first, then known, then by signal.
pub fn parse_wifi(list: &str, known: &[String]) -> Vec<Wifi> {
  let mut out: Vec<Wifi> = Vec::new();
  for line in list.lines() {
    // Terse output escapes ':' in values as '\:'.
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
      match c {
        '\\' => {
          if let Some(n) = chars.next() {
            cur.push(n);
          }
        }
        ':' => fields.push(std::mem::take(&mut cur)),
        _ => cur.push(c),
      }
    }
    fields.push(cur);
    if fields.len() < 4 || fields[1].trim().is_empty() {
      continue;
    }
    let net = Wifi {
      connected: fields[0] == "*",
      ssid: fields[1].clone(),
      signal: fields[2].parse().unwrap_or(0),
      security: fields[3].clone(),
      known: known.contains(&fields[1]),
    };
    match out.iter_mut().find(|n| n.ssid == net.ssid) {
      Some(n) => {
        if net.connected || (!n.connected && net.signal > n.signal) {
          *n = net;
        }
      }
      None => out.push(net),
    }
  }
  out.sort_by(|a, b| b.connected.cmp(&a.connected).then(b.known.cmp(&a.known)).then(b.signal.cmp(&a.signal)));
  out
}

pub fn section_title(nets: &[Wifi], i: usize) -> &'static str {
  let Some(n) = nets.get(i) else { return "" };
  if n.known && i == 0 {
    "REDES CONHECIDAS"
  } else if !n.known && (i == 0 || nets[i - 1].known) {
    "OUTRAS REDES"
  } else {
    ""
  }
}

// What nmcli's error says, in the panel's words.
pub fn failure_reason(stderr: &str, needs_credentials: bool) -> &'static str {
  let e = stderr.to_lowercase();
  if needs_credentials && (e.contains("secrets were required") || e.contains("no secrets")) {
    "Senha necessária"
  } else if needs_credentials && (e.contains("timeout") || e.contains("authentication")) {
    "Senha incorreta"
  } else if e.contains("not found") || e.contains("no network") {
    "Rede perdida"
  } else {
    "Não foi possível conectar"
  }
}

// The secret goes through stdin into nmcli's editor: argv is readable by
// anyone in /proc. $1 the SSID; for 802.1X $2 the identity.
pub const PSK_SCRIPT: &str = "IFS= read -r pw; u=$(uuidgen);\
 nmcli connection add type wifi con-name \"$1\" ssid \"$1\" connection.uuid \"$u\" wifi-sec.key-mgmt wpa-psk >/dev/null\
 && printf 'set wifi-sec.psk %s\\nsave\\nquit\\n' \"$pw\" | nmcli connection edit uuid \"$u\" >/dev/null\
 && nmcli connection up uuid \"$u\"\
 || { nmcli connection delete uuid \"$u\" >/dev/null 2>&1; false; }";

pub const ENTERPRISE_SCRIPT: &str = "u=$(uuidgen); IFS= read -r pw;\
 nmcli connection add type wifi con-name \"$1\" ssid \"$1\" connection.uuid \"$u\"\
 wifi-sec.key-mgmt wpa-eap 802-1x.eap peap 802-1x.phase2-auth mschapv2\
 802-1x.identity \"$2\" 802-1x.auth-timeout 8 >/dev/null\
 && printf 'set 802-1x.password %s\\nsave\\nquit\\n' \"$pw\" | nmcli connection edit uuid \"$u\" >/dev/null\
 && nmcli connection up uuid \"$u\"\
 || { nmcli connection delete uuid \"$u\" >/dev/null 2>&1; false; }";

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn network_data() {
    assert_eq!(wifi_icon(100), "\u{f0928}");
    assert_eq!(wifi_icon(0), "\u{f092f}");
    assert_eq!(format_speed("10000"), "10gbit");
    assert_eq!(format_speed("2500"), "2.5gbit");
    assert_eq!(format_speed("100"), "100mbit");
    assert_eq!(decode_ssid("Caf\\xc3\\xa9"), "Café");
    let nets = parse_wifi("*:Casa:80:WPA2\n :Casa:40:WPA2\n :Rua\\:1:60:\n :Vizinho:90:WPA2 802.1X\n : :10:WPA2\n", &["Casa".into()]);
    assert_eq!(nets.iter().map(|n| n.ssid.as_str()).collect::<Vec<_>>(), ["Casa", "Vizinho", "Rua:1"]);
    assert!(nets[0].connected && nets[0].known);
    assert!(!nets[2].needs_credentials() && nets[1].enterprise());
    assert_eq!(section_title(&nets, 0), "REDES CONHECIDAS");
    assert_eq!(section_title(&nets, 1), "OUTRAS REDES");
    assert_eq!(section_title(&nets, 2), "");
    let mut p = Pings::default();
    let mut info = parse_key_value("iface\tens160\nrouter_ping_ms\t0.2\ninternet_ping_ms\t14\n");
    p.push(&info, 24);
    info.insert("internet_ping_ms".into(), "timeout".into());
    p.push(&info, 24);
    assert_eq!(Pings::loss(&p.internet), 50);
    assert_eq!(Pings::average(&p.internet, 5), Some(14.0));
    assert_eq!(format_ping(Some(0.185), true), "0.2 ms");
    assert_eq!(format_bytes(375200501.0), "357.8 MB");
    assert_eq!(parse_band("band\t5\nselected\tauto\navailable\t2.4 5\n"), ("5".into(), "auto".into(), vec!["2.4".into(), "5".into()]));
    assert_eq!(band_title("auto", "5"), "BANDA DO WI-FI: 5GHZ");
  }
}
