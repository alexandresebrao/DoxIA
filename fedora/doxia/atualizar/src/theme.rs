// The shell's look for a GTK window: the palette from the current theme's
// colors.toml, the control tokens ([controls]) and font size ([font]) from its
// shell.toml, and the corner rounding mango uses (the shell reads the same
// three, see shell/Commons/Color.qml and Style.qml).

use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;

#[derive(Clone, Copy, PartialEq)]
struct Rgb(f64, f64, f64);

impl Rgb {
  fn hex(s: &str) -> Option<Rgb> {
    let h = s.trim().trim_start_matches('#');
    if h.len() < 6 || !h.is_char_boundary(6) {
      return None;
    }
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok().map(|b| b as f64 / 255.0);
    Some(Rgb(byte(0)?, byte(2)?, byte(4)?))
  }

  fn css(self) -> String {
    let c = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", c(self.0), c(self.1), c(self.2))
  }

  fn alpha(self, a: f64) -> String {
    let c = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("rgba({},{},{},{:.3})", c(self.0), c(self.1), c(self.2), a.clamp(0.0, 1.0))
  }

  // Qt.darker: the HSV value divided by the factor.
  fn darker(self, factor: f64) -> Rgb {
    let max = self.0.max(self.1).max(self.2);
    if max <= 0.0 {
      return self;
    }
    let scale = (max / factor) / max;
    Rgb(self.0 * scale, self.1 * scale, self.2 * scale)
  }
}

struct Toml(Vec<(String, String, String)>);

impl Toml {
  fn read(path: &PathBuf) -> Toml {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut section = String::new();
    let mut entries = Vec::new();
    for line in text.lines() {
      let line = line.split('#').next().unwrap_or("").trim();
      if line.starts_with('[') && line.ends_with(']') {
        section = line[1..line.len() - 1].trim().to_string();
      } else if let Some((k, v)) = line.split_once('=') {
        entries.push((section.clone(), k.trim().to_string(), v.trim().trim_matches('"').to_string()));
      }
    }
    Toml(entries)
  }

  fn get(&self, section: &str, key: &str) -> Option<&str> {
    self.0.iter().rev().find(|(s, k, _)| s == section && k == key).map(|(_, _, v)| v.as_str())
  }

  fn num(&self, section: &str, key: &str) -> Option<f64> {
    self.get(section, key).and_then(|v| v.parse().ok())
  }
}

// Hash-colors inside "#1f6bff ..." lines are not comments: re-read the raw
// value for colors so the split above can't cut them.
fn color_value(path: &PathBuf, key: &str) -> Option<Rgb> {
  let text = std::fs::read_to_string(path).ok()?;
  text.lines().rev().find_map(|line| {
    let (k, v) = line.split_once('=')?;
    (k.trim() == key).then(|| Rgb::hex(v.trim().trim_matches('"')))?
  })
}

fn controls_color(path: &PathBuf, key: &str) -> Option<String> {
  let text = std::fs::read_to_string(path).ok()?;
  let mut in_controls = false;
  for line in text.lines() {
    let t = line.trim();
    if t.starts_with('[') {
      in_controls = t == "[controls]";
      continue;
    }
    if !in_controls {
      continue;
    }
    if let Some((k, v)) = t.split_once('=') {
      if k.trim() == key {
        return Some(v.trim().trim_matches('"').to_string());
      }
    }
  }
  None
}

pub fn theme_dir() -> PathBuf {
  let home = std::env::var("HOME").unwrap_or_default();
  PathBuf::from(home).join(".local/state/omarchy/current/theme")
}

// What the CSS depends on, to rebuild it only when the theme changes.
pub fn fingerprint() -> Vec<Option<SystemTime>> {
  let dir = theme_dir();
  let home = std::env::var("HOME").unwrap_or_default();
  [dir.join("colors.toml"), dir.join("shell.toml"), PathBuf::from(home).join(".config/mango/looknfeel.conf")]
    .iter()
    .map(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
    .collect()
}

struct Palette {
  fg: Rgb,
  bg: Rgb,
  accent: Rgb,
  urgent: Rgb,
  ok: Rgb,
  warn: Rgb,
}

// For Pango markup, which can't take CSS classes.
pub fn accent() -> String {
  color_value(&theme_dir().join("colors.toml"), "accent").unwrap_or(Rgb(0.79, 0.8, 0.8)).css()
}

fn rounding() -> u32 {
  Command::new("omarchy-mango-option")
    .arg("border_radius")
    .output()
    .ok()
    .and_then(|o| String::from_utf8(o.stdout).ok())
    .and_then(|s| s.trim().parse().ok())
    .unwrap_or(0)
}

pub fn css() -> String {
  let dir = theme_dir();
  let colors = dir.join("colors.toml");
  let shell_path = dir.join("shell.toml");
  let shell = Toml::read(&shell_path);

  let p = Palette {
    fg: color_value(&colors, "foreground").unwrap_or(Rgb(0.79, 0.8, 0.8)),
    bg: color_value(&colors, "background").unwrap_or(Rgb(0.063, 0.075, 0.082)),
    accent: color_value(&colors, "accent").unwrap_or(Rgb(0.79, 0.8, 0.8)),
    urgent: color_value(&colors, "red").or_else(|| color_value(&colors, "color1")).unwrap_or(Rgb(0.65, 0.33, 0.33)),
    ok: color_value(&colors, "green").unwrap_or(Rgb::hex("#5ba352").unwrap()),
    warn: color_value(&colors, "yellow").unwrap_or(Rgb::hex("#f0ab00").unwrap()),
  };

  // Control state tokens, as Style.qml resolves them.
  let token = |key: &str, fallback: Rgb| -> Rgb {
    match controls_color(&shell_path, key).as_deref().map(str::trim) {
      Some("foreground" | "text") => p.fg,
      Some("accent") => p.accent,
      Some("urgent") => p.urgent,
      Some("background") => p.bg,
      Some(v) => Rgb::hex(v).unwrap_or(fallback),
      None => fallback,
    }
  };
  let alpha = |key: &str, fallback: f64| shell.num("controls", key).unwrap_or(fallback).clamp(0.0, 1.0);
  let normal = token("normal-color", p.fg);
  let hover = token("hover-cursor-color", p.fg);
  let focus = token("focus-color", hover);
  let selected = token("selected-color", p.fg);
  let normal_border = token("normal-border", normal);
  let hover_border = token("hover-cursor-border", hover);
  let focus_border = token("focus-border", focus);

  let base = shell.num("font", "base-size").filter(|n| *n >= 1.0).unwrap_or(12.0);
  let font = |key: &str, mult: f64| shell.num("font", key).filter(|n| *n > 0.0).unwrap_or((base * mult).round().max(1.0));
  let scale = shell.num("spacing", "scale").unwrap_or(1.0) * (base / 12.0);
  let sp = |px: f64| format!("{}px", (px * scale).round());

  let radius = rounding();
  let r = |px: u32| if radius > 0 { format!("{}px", (px as f64 * scale).round()) } else { "0".into() };

  let vars = [
    ("$FG", p.fg.css()),
    ("$BG", p.bg.css()),
    ("$ACCENT", p.accent.css()),
    ("$URGENT", p.urgent.css()),
    ("$OK", p.ok.css()),
    ("$WARN", p.warn.css()),
    ("$DIM", p.fg.alpha(0.6)),
    ("$FAINT", p.fg.alpha(0.12)),
    ("$SURFACE", p.fg.alpha(0.03)),
    ("$INSET", p.bg.darker(1.3).css()),
    ("$PRIMARY_HOVER", p.accent.css()),
    ("$PRIMARY_PRESS", p.accent.darker(1.25).darker(1.2).css()),
    ("$PRIMARY", p.accent.darker(1.25).css()),
    ("$ACCENT_TINT", p.accent.alpha(0.07)),
    ("$ACCENT_EDGE", p.accent.alpha(0.6)),
    ("$DOT_EDGE", p.fg.alpha(0.3)),
    ("$RADIO_EDGE", p.fg.alpha(0.35)),
    ("$CARD_HOVER", p.fg.alpha(0.05)),
    ("$CARD", p.fg.alpha(0.02)),
    ("$OK_BG", p.ok.alpha(0.09)),
    ("$OK_EDGE", p.ok.alpha(0.3)),
    ("$WARN_BG", p.warn.alpha(0.09)),
    ("$WARN_EDGE", p.warn.alpha(0.3)),
    ("$ERR_BG", p.urgent.alpha(0.09)),
    ("$ERR_EDGE", p.urgent.alpha(0.3)),
    ("$PLAIN_BG", p.fg.alpha(0.04)),
    ("$PLAIN_EDGE", p.fg.alpha(0.12)),
    ("$NORMAL_FILL", normal.alpha(alpha("normal-fill-alpha", 0.04))),
    ("$NORMAL_BORDER", normal_border.alpha(alpha("normal-border-alpha", 0.4))),
    ("$HOVER_FILL", hover.alpha(alpha("hover-cursor-fill-alpha", 0.08))),
    ("$HOVER_BORDER", hover_border.alpha(alpha("hover-cursor-border-alpha", 0.25))),
    ("$FOCUS_FILL", focus.alpha(alpha("focus-fill-alpha", 0.08))),
    ("$FOCUS_BORDER", focus_border.alpha(alpha("focus-border-alpha", 0.25))),
    ("$SELECTED_FILL", selected.alpha(alpha("selected-fill-alpha", 0.18))),
    ("$SELECTED_TEXT", selected.css()),
    ("$PRESSED_FILL", hover.alpha(alpha("pressed-fill-alpha", 0.22))),
    ("$SELECTION", p.accent.alpha(alpha("selection-fill-alpha", 0.35))),
    ("$FS_CAPTION", format!("{}px", font("caption", 0.833))),
    ("$FS_SMALL", format!("{}px", font("body-small", 0.917))),
    ("$FS_BODY", format!("{}px", font("body", 1.0))),
    ("$FS_SUBTITLE", format!("{}px", font("subtitle", 1.083))),
    ("$FS_TITLE", format!("{}px", font("title", 1.167))),
    ("$FS_HEADING", format!("{}px", font("heading", 1.333))),
    ("$R_CTRL", if radius > 0 { format!("{radius}px") } else { "0".into() }),
    ("$R_CARD", r(6)),
    ("$R_ICON", r(5)),
    ("$R_BTN", r(4)),
    ("$SP2", sp(2.0)),
    ("$SP6", sp(6.0)),
    ("$SP7", sp(7.0)),
    ("$SP8", sp(8.0)),
    ("$SP9", sp(9.0)),
    ("$SP10", sp(10.0)),
    ("$SP12", sp(12.0)),
    ("$SP14", sp(14.0)),
    ("$SP18", sp(18.0)),
    ("$SP30", sp(30.0)),
    ("$SP36", sp(36.0)),
    ("$SP34", sp(34.0)),
  ];

  // Longest names first, so $PRIMARY never eats the start of $PRIMARY_HOVER.
  let mut vars = vars.to_vec();
  vars.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
  let mut out = include_str!("style.css").to_string();
  for (k, v) in vars {
    out = out.replace(k, &v);
  }
  out
}
