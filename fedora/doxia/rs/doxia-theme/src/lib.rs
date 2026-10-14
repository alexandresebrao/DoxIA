// doxia-theme: the shell's look for the DoxIA GTK apps. Each app adds the
// class "doxia" to its windows and hands its own stylesheet to Styler.
//
// The look comes from the current theme's palette (colors.toml), its control
// tokens ([controls]) and font size ([font]) in shell.toml, and the corner
// rounding mango uses: the same three the shell reads (shell/Commons/Color.qml
// and Style.qml).

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

  // Qt.lighter: the HSV value times the factor; past full value, the
  // overflow comes out of the saturation.
  fn lighter(self, factor: f64) -> Rgb {
    let (r, g, b) = (self.0, self.1, self.2);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let (mut s, mut v) = (if max > 0.0 { (max - min) / max } else { 0.0 }, max * factor);
    if v > 1.0 {
      s = (s - (v - 1.0)).max(0.0);
      v = 1.0;
    }
    // Hue from the original channels; rebuild at the new s and v.
    let d = max - min;
    let h = if d <= 0.0 {
      0.0
    } else if max == r {
      ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
      (b - r) / d + 2.0
    } else {
      (r - g) / d + 4.0
    };
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match h as u32 {
      0 => (c, x, 0.0),
      1 => (x, c, 0.0),
      2 => (0.0, c, x),
      3 => (0.0, x, c),
      4 => (x, 0.0, c),
      _ => (c, 0.0, x),
    };
    let m = v - c;
    Rgb(r1 + m, g1 + m, b1 + m)
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

// A # starts a comment only outside quotes: "#1f7ae0" is a color.
fn strip_comment(line: &str) -> &str {
  let mut quoted = false;
  for (i, c) in line.char_indices() {
    match c {
      '"' => quoted = !quoted,
      '#' if !quoted => return &line[..i],
      _ => {}
    }
  }
  line
}

// Hyprland-style colors from shell.toml: "#rrggbb", "rgba(rrggbbaa)" or
// "rgb(rrggbb)", as CSS.
fn css_color(token: &str) -> Option<String> {
  let t = token.trim();
  if let Some(hex) = t.strip_prefix("rgba(").and_then(|r| r.strip_suffix(')')) {
    let rgb = Rgb::hex(&hex[..6.min(hex.len())])?;
    let a = hex.get(6..8).and_then(|a| u8::from_str_radix(a, 16).ok()).map(|a| a as f64 / 255.0).unwrap_or(1.0);
    return Some(rgb.alpha(a));
  }
  if let Some(hex) = t.strip_prefix("rgb(").and_then(|r| r.strip_suffix(')')) {
    return Rgb::hex(hex).map(Rgb::css);
  }
  Rgb::hex(t).filter(|_| t.starts_with('#')).map(Rgb::css)
}

// A border value as a CSS image: a gradient for "c1 c2 45deg", a flat one for
// a single color. Section.key values point at another entry (the popups use
// "hyprland.active-border").
fn border_image(shell: &Toml, value: &str, fallback: &str, depth: u8) -> String {
  if let Some((section, key)) = value.split_once('.').filter(|(s, _)| !s.contains(['(', '#', ' '])) {
    if depth < 4 {
      if let Some(v) = shell.get(section, key) {
        return border_image(shell, v, fallback, depth + 1);
      }
    }
  }
  let mut colors = Vec::new();
  let mut angle = None;
  for part in value.split_whitespace() {
    if let Some(deg) = part.strip_suffix("deg") {
      angle = deg.parse::<f64>().ok();
    } else if let Some(c) = css_color(part) {
      colors.push(c);
    }
  }
  match colors.len() {
    0 => format!("linear-gradient({fallback}, {fallback})"),
    1 => format!("linear-gradient({0}, {0})", colors[0]),
    // Hyprland measures the angle from the x axis, CSS from the top.
    _ => format!("linear-gradient({}deg, {})", 90.0 - angle.unwrap_or(0.0), colors.join(", ")),
  }
}

impl Toml {
  fn read(path: &PathBuf) -> Toml {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut section = String::new();
    let mut entries = Vec::new();
    for line in text.lines() {
      let line = strip_comment(line).trim();
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

// A mango option as omarchy-mango-option reads it. The script takes over a
// tenth of a second and every card and panel asks, so an answer is kept for
// two seconds: a startup building them all runs it once, and a changed
// config is still seen by the next theme reload.
fn mango_option(name: &str) -> Option<String> {
  thread_local! {
    static CACHE: std::cell::RefCell<std::collections::HashMap<String, (std::time::Instant, Option<String>)>> = Default::default();
  }
  let fresh = CACHE.with(|c| c.borrow().get(name).filter(|(at, _)| at.elapsed() < std::time::Duration::from_secs(2)).map(|(_, v)| v.clone()));
  if let Some(value) = fresh {
    return value;
  }
  let value = Command::new("omarchy-mango-option")
    .arg(name)
    .output()
    .ok()
    .and_then(|o| String::from_utf8(o.stdout).ok())
    .map(|s| s.trim().to_string());
  CACHE.with(|c| c.borrow_mut().insert(name.to_string(), (std::time::Instant::now(), value.clone())));
  value
}

fn rounding() -> u32 {
  mango_option("border_radius").and_then(|s| s.parse().ok()).unwrap_or(0)
}

// Half of mango's outer gap: what the bar and popups keep off the edges.
fn outer_gap() -> i32 {
  mango_option("gap_outer_horizontal").and_then(|s| s.parse::<f64>().ok()).map(|g| (g / 2.0).round() as i32).unwrap_or(5)
}

// The base controls, then the app's own stylesheet, then the color
// utilities, all with the theme's values filled in.
pub fn css(app_css: &str) -> String {
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

  let popup_text = shell.get("popups", "text").and_then(css_color).unwrap_or_else(|| p.fg.css());
  // [notifications]: a translucent card (mango blurs behind it), its text, a
  // dimmer body and the border.
  let notif_text_rgb = shell.get("notifications", "text").and_then(|t| Rgb::hex(t)).unwrap_or(p.fg);
  let notif_bg_rgb = shell.get("notifications", "background").and_then(|t| Rgb::hex(t)).unwrap_or(p.bg);
  let notif_alpha = shell.num("notifications", "background-alpha").unwrap_or(1.0);
  let notif_border = border_image(&shell, shell.get("notifications", "border").unwrap_or(""), &p.accent.css(), 0);
  // [polkit]: Color.qml's polkit group (scrim, card, borders, error colors).
  let section_rgb = |section: &str, key: &str, fallback: Rgb| shell.get(section, key).and_then(|t| Rgb::hex(t)).unwrap_or(fallback);
  let pk_alpha = shell.num("polkit", "background-alpha").unwrap_or(1.0);
  let pk_border_alpha = shell.num("polkit", "border-alpha").unwrap_or(1.0);
  let pk_scrim = section_rgb("polkit", "scrim", p.bg).alpha(shell.num("polkit", "scrim-alpha").unwrap_or(0.5));
  let pk_border = border_image(&shell, shell.get("polkit", "border").unwrap_or(""), &p.accent.alpha(pk_border_alpha), 0);
  let pk_border_error = border_image(&shell, shell.get("polkit", "border-error").unwrap_or(""), &p.urgent.alpha(pk_border_alpha), 0);
  // [lock]: Color.qml's lock group (the password field over the wallpaper).
  let lk_border_alpha = shell.num("lock", "border-alpha").unwrap_or(1.0);
  let lk_bg = section_rgb("lock", "background", p.bg).alpha(shell.num("lock", "background-alpha").unwrap_or(0.8));
  let lk_placeholder = shell.get("lock", "placeholder").and_then(css_color).unwrap_or_else(|| p.fg.alpha(0.66));
  let lk_border_active =
    border_image(&shell, shell.get("lock", "border-active").unwrap_or(""), &p.accent.alpha(lk_border_alpha), 0);
  let lk_border_error =
    border_image(&shell, shell.get("lock", "border-error").unwrap_or(""), &p.urgent.alpha(lk_border_alpha), 0);
  let lk_selection = section_rgb("lock", "selection", p.accent).alpha(shell.num("lock", "selection-alpha").unwrap_or(0.45));
  let popup_border = border_image(&shell, shell.get("popups", "border").unwrap_or(""), &p.accent.css(), 0);
  // [menu]: Color.qml's menu group; the card itself is drawn (menu_card).
  let menu_scrim = section_rgb("menu", "scrim", p.bg).alpha(shell.num("menu", "scrim-alpha").unwrap_or(0.5));
  let menu_text = shell.get("menu", "text").and_then(css_color).unwrap_or_else(|| p.fg.css());
  let menu_sel_bg =
    section_rgb("menu", "selected-background", p.fg).alpha(shell.num("menu", "selected-background-alpha").unwrap_or(0.08));
  let menu_sel_text = shell.get("menu", "selected-text").and_then(css_color).unwrap_or_else(|| p.accent.css());
  // The Start menu (doxia.iniciar): the menu's colors in Windows 98 relief.
  let face = section_rgb("menu", "background", p.bg);
  let face_alpha = shell.num("menu", "background-alpha").unwrap_or(1.0);
  let menu_border = border_image(
    &shell,
    shell.get("menu", "border").unwrap_or(""),
    &p.fg.alpha(shell.num("menu", "border-alpha").unwrap_or(1.0)),
    0,
  );

  // The bar ([bar], [tooltip]); its sizes come from bar_style().
  let bar_text = shell.get("bar", "text").and_then(css_color).unwrap_or_else(|| p.fg.css());
  let bar_active = shell.get("bar", "active").and_then(css_color).unwrap_or_else(|| p.urgent.css());
  let tip_bg = section_rgb("tooltip", "background", p.bg).alpha(shell.num("tooltip", "background-alpha").unwrap_or(1.0));
  let tip_text = shell.get("tooltip", "text").and_then(css_color).unwrap_or_else(|| p.fg.css());
  let tip_border = section_rgb("tooltip", "border", p.fg).alpha(shell.num("tooltip", "border-alpha").unwrap_or(1.0));
  let bar = bar_style();

  let vars = [
    ("$BAR_TEXT", bar_text),
    ("$BAR_ACTIVE", bar_active),
    ("$BAR_ICON_FONT_NUMBER", format!("{}px", (bar.icon_font as f64 * 1.1).round())),
    ("$BAR_ICON_FONT", format!("{}px", bar.icon_font)),
    ("$TIP_BG", tip_bg),
    ("$TIP_TEXT", tip_text),
    ("$TIP_BORDER", tip_border),
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
    ("$FS_DISPLAY_LARGE", format!("{}px", font("display-large", 2.333))),
    ("$FS_ICON_LARGE", format!("{}px", font("icon-large", 1.5))),
    ("$POPUP_FILL", p.bg.alpha(0.97)),
    ("$POPUP_TEXT", popup_text),
    ("$POPUP_TRACK", Rgb::hex(&shell.get("popups", "text").unwrap_or("").replace('#', "")).unwrap_or(p.fg).alpha(0.45)),
    ("$POPUP_BORDER", popup_border),
    ("$NOTIF_BG", notif_bg_rgb.alpha(notif_alpha)),
    ("$NOTIF_TEXT", notif_text_rgb.css()),
    ("$NOTIF_BODY", notif_text_rgb.darker(1.15).css()),
    ("$NOTIF_DIM", notif_text_rgb.darker(1.4).css()),
    ("$NOTIF_BORDER", notif_border),
    ("$PK_BG", section_rgb("polkit", "background", p.bg).alpha(pk_alpha)),
    ("$PK_TEXT", section_rgb("polkit", "text", p.fg).css()),
    ("$PK_TEXT_ERROR", section_rgb("polkit", "text-error", p.urgent).css()),
    ("$PK_ACCENT", section_rgb("polkit", "accent", p.accent).css()),
    ("$PK_SCRIM", pk_scrim),
    ("$PK_BORDER_ERROR", pk_border_error),
    ("$PK_BORDER", pk_border),
    ("$MENU_SCRIM", menu_scrim),
    ("$MENU_TEXT", menu_text),
    ("$MENU_SEL_BG", menu_sel_bg),
    ("$MENU_SEL_TEXT", menu_sel_text),
    ("$IN_FACE", face.alpha(face_alpha)),
    ("$IN_LIGHT", face.lighter(2.2).alpha(face_alpha)),
    ("$IN_MID", face.lighter(1.6).alpha(face_alpha)),
    ("$IN_SHADOW", face.darker(1.5).alpha(face_alpha)),
    ("$IN_FIELD", face.darker(2.2).alpha(face_alpha)),
    ("$IN_BANNER_TOP", face.darker(1.4).alpha(face_alpha)),
    ("$IN_BANNER_BOTTOM", p.accent.darker(1.45).css()),
    ("$IN_BORDER", menu_border),
    ("$LK_BG", lk_bg),
    ("$LK_TEXT", section_rgb("lock", "text", p.fg).css()),
    ("$LK_PLACEHOLDER", lk_placeholder),
    ("$LK_TEXT_ERROR", section_rgb("lock", "text-error", p.urgent).css()),
    ("$LK_BORDER_ACTIVE", lk_border_active),
    ("$LK_BORDER_ERROR", lk_border_error),
    ("$LK_SELECTION", lk_selection),
    ("$FS_LOCK_FIELD", format!("{}px", (font("heading", 1.333) * 1.125).round())),
    ("$FS_LOCK_DOTS", format!("{}px", (font("heading", 1.333) * 1.33).round())),
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
  let mut out = [include_str!("base.css"), app_css, include_str!("colors.css")].join("\n");
  for (k, v) in vars {
    out = out.replace(k, &v);
  }
  out
}

// "#rrggbb", "rgba(rrggbbaa)" or "rgb(rrggbb)" as RGBA, for cairo.
fn rgba_of(token: &str) -> Option<[f64; 4]> {
  let t = token.trim();
  let (hex, alpha) = if let Some(h) = t.strip_prefix("rgba(").and_then(|r| r.strip_suffix(')')) {
    (h, h.get(6..8).and_then(|a| u8::from_str_radix(a, 16).ok()).map(|a| a as f64 / 255.0).unwrap_or(1.0))
  } else if let Some(h) = t.strip_prefix("rgb(").and_then(|r| r.strip_suffix(')')) {
    (h, 1.0)
  } else if t.starts_with('#') {
    (t, 1.0)
  } else {
    return None;
  };
  let Rgb(r, g, b) = Rgb::hex(&hex[..6.min(hex.len())])?;
  Some([r, g, b, alpha])
}

// A border for cairo: its colors (one is flat) and Hyprland's angle, which
// runs from the x axis.
#[derive(Clone, Debug, PartialEq)]
pub struct Paint {
  pub colors: Vec<[f64; 4]>,
  pub angle: f64,
}

fn paint(shell: &Toml, value: &str, fallback: [f64; 4], opacity: f64, depth: u8) -> Paint {
  if let Some((section, key)) = value.split_once('.').filter(|(s, _)| !s.contains(['(', '#', ' '])) {
    if depth < 4 {
      if let Some(v) = shell.get(section, key) {
        return paint(shell, v, fallback, opacity, depth + 1);
      }
    }
  }
  let mut colors = Vec::new();
  let mut angle = 0.0;
  for part in value.split_whitespace() {
    if let Some(deg) = part.strip_suffix("deg") {
      angle = deg.parse().unwrap_or(0.0);
    } else if let Some(c) = rgba_of(part) {
      colors.push(c);
    }
  }
  if colors.is_empty() {
    colors.push(fallback);
  }
  for c in &mut colors {
    c[3] *= opacity;
  }
  Paint { colors, angle }
}

// A card the shell draws as a BorderSurface: fill, border and corners.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
  pub fill: [f64; 4],
  pub border: Paint,
  pub border_width: f64,
  pub radius: f64,
}

// The [menu] card of Color.qml and Border.qml: background with its alpha,
// the border (usually hyprland.active-border-foreground) at border-alpha,
// 2px wide, mango's corner rounding.
pub fn menu_card() -> Card {
  surface_card("menu")
}

// A shell.toml section's card: [menu], [popups] (the bar's panels) or
// [tooltip]. Each falls back as Color.qml and the QML that draws it do:
// the menu's border to the foreground, 2px; the popups' to the accent, 2px
// at the spacing scale; the tooltip's to the foreground, 1px.
pub fn surface_card(section: &str) -> Card {
  let dir = theme_dir();
  let colors = dir.join("colors.toml");
  let shell = Toml::read(&dir.join("shell.toml"));
  let fg = color_value(&colors, "foreground").unwrap_or(Rgb(0.79, 0.8, 0.8));
  let bg = color_value(&colors, "background").unwrap_or(Rgb(0.063, 0.075, 0.082));
  let accent = color_value(&colors, "accent").unwrap_or(fg);
  let Rgb(r, g, b) = shell.get(section, "background").and_then(Rgb::hex).unwrap_or(bg);
  let fill = [r, g, b, shell.num(section, "background-alpha").unwrap_or(1.0)];
  let (border_default, width_default) = match section {
    "popups" => (accent, (2.0 * spacing_scale(&shell)).round().max(1.0)),
    "tooltip" => (fg, 1.0),
    _ => (fg, 2.0),
  };
  let border_alpha = shell.num(section, "border-alpha").unwrap_or(1.0);
  let Rgb(br, bg_, bb) = border_default;
  let border = paint(&shell, shell.get(section, "border").unwrap_or(""), [br, bg_, bb, 1.0], border_alpha, 0);
  let border_width = shell.num(section, "border-width").unwrap_or(width_default).max(0.0);
  Card { fill, border, border_width, radius: rounding() as f64 }
}

// Style.qml's effectiveSpacingScale: [spacing] scale, times the font scale
// unless scale-with-font is false.
fn spacing_scale(shell: &Toml) -> f64 {
  let base = shell.num("font", "base-size").filter(|n| *n >= 1.0).unwrap_or(12.0);
  let with_font = shell.get("spacing", "scale-with-font") != Some("false");
  shell.num("spacing", "scale").unwrap_or(1.0) * if with_font { base / 12.0 } else { 1.0 }
}

// The bar's look (Style.qml's bar sizes, Color.qml's bar colors): the cross
// size for a horizontal and a vertical bar, the icon slot, the canvas and
// font of its glyphs and the status slot, each scaled with the font unless
// [bar] scale-with-font is false; its fill, text and "active" colors; the
// corner rounding and the spacing scale.
#[derive(Clone, Debug, PartialEq)]
pub struct BarStyle {
  pub size_horizontal: i32,
  pub size_vertical: i32,
  pub icon_slot: i32,
  pub icon_canvas: i32,
  pub icon_font: i32,
  pub status_slot: i32,
  pub background: [f64; 4],
  pub text: [f64; 4],
  pub active: [f64; 4],
  pub accent: [f64; 4],
  pub radius: f64,
  pub spacing: f64,
  pub gaps_out: i32,
  pub font_body: f64,
  pub font_caption: f64,
  // [font] base-size: what omarchy-display-text-size sets.
  pub font_base: f64,
}

impl BarStyle {
  // Style.space: a spacing length at the scale, at least 1px.
  pub fn space(&self, px: f64) -> i32 {
    if px <= 0.0 { 0 } else { ((px * self.spacing).round() as i32).max(1) }
  }
}

// The theme's palette (colors.toml) as RGBA, for what cairo draws.
#[derive(Clone, Debug, PartialEq)]
pub struct Colors {
  pub foreground: [f64; 4],
  pub background: [f64; 4],
  pub accent: [f64; 4],
  pub red: [f64; 4],
  pub green: [f64; 4],
  pub yellow: [f64; 4],
  pub blue: [f64; 4],
}

pub fn palette() -> Colors {
  let colors = theme_dir().join("colors.toml");
  let get = |key: &str, fallback: &str| {
    let Rgb(r, g, b) = color_value(&colors, key).or_else(|| Rgb::hex(fallback)).unwrap();
    [r, g, b, 1.0]
  };
  let fg = get("foreground", "#c9cccc");
  Colors {
    foreground: fg,
    background: get("background", "#101315"),
    accent: color_value(&colors, "accent").map(|Rgb(r, g, b)| [r, g, b, 1.0]).unwrap_or_else(|| get("red", "#ee0000")),
    red: get("red", "#e5484d"),
    green: get("green", "#5ba352"),
    yellow: get("yellow", "#f2c230"),
    blue: get("blue", "#1f6bff"),
  }
}

// The [controls] state fills and the popups' text and background, for what
// cairo draws in the bar's panels (sliders, rails).
#[derive(Clone, Debug, PartialEq)]
pub struct Controls {
  pub selected_fill: [f64; 4],
  pub hover_fill: [f64; 4],
  pub text: [f64; 4],
  pub background: [f64; 4],
}

pub fn controls() -> Controls {
  let dir = theme_dir();
  let colors = dir.join("colors.toml");
  let shell_path = dir.join("shell.toml");
  let shell = Toml::read(&shell_path);
  let fg = color_value(&colors, "foreground").unwrap_or(Rgb(0.79, 0.8, 0.8));
  let bg = color_value(&colors, "background").unwrap_or(Rgb(0.063, 0.075, 0.082));
  let accent = color_value(&colors, "accent").unwrap_or(fg);
  let urgent = color_value(&colors, "red").unwrap_or(Rgb(0.65, 0.33, 0.33));
  let token = |key: &str, fallback: Rgb| -> Rgb {
    match controls_color(&shell_path, key).as_deref().map(str::trim) {
      Some("foreground" | "text") => fg,
      Some("accent") => accent,
      Some("urgent") => urgent,
      Some("background") => bg,
      Some(v) => Rgb::hex(v).unwrap_or(fallback),
      None => fallback,
    }
  };
  let alpha = |key: &str, fallback: f64| shell.num("controls", key).unwrap_or(fallback).clamp(0.0, 1.0);
  let selected = token("selected-color", fg);
  let hover = token("hover-cursor-color", fg);
  let text = shell.get("popups", "text").and_then(rgba_of).unwrap_or([fg.0, fg.1, fg.2, 1.0]);
  Controls {
    selected_fill: [selected.0, selected.1, selected.2, alpha("selected-fill-alpha", 0.18)],
    hover_fill: [hover.0, hover.1, hover.2, alpha("hover-cursor-fill-alpha", 0.08)],
    text,
    background: [bg.0, bg.1, bg.2, 1.0],
  }
}

pub fn bar_style() -> BarStyle {
  let dir = theme_dir();
  let colors = dir.join("colors.toml");
  let shell = Toml::read(&dir.join("shell.toml"));
  let fg = color_value(&colors, "foreground").unwrap_or(Rgb(0.79, 0.8, 0.8));
  let bg = color_value(&colors, "background").unwrap_or(Rgb(0.063, 0.075, 0.082));
  let accent = color_value(&colors, "accent").unwrap_or(fg);
  let urgent = color_value(&colors, "red").or_else(|| color_value(&colors, "color1")).unwrap_or(Rgb(0.65, 0.33, 0.33));
  let base = shell.num("font", "base-size").filter(|n| *n >= 1.0).unwrap_or(12.0);
  let font_scale = (base / 12.0).max(1.0 / 12.0);
  let scale = if shell.get("bar", "scale-with-font") == Some("false") { 1.0 } else { font_scale };
  let token = |key: &str, fallback: f64| {
    let n = shell.num("bar", key).filter(|n| *n > 0.0).unwrap_or(fallback);
    ((n * scale).round() as i32).max(1)
  };
  let rgba = |c: Rgb, a: f64| [c.0, c.1, c.2, a];
  let pick = |key: &str, fallback: Rgb| shell.get("bar", key).and_then(rgba_of).unwrap_or(rgba(fallback, 1.0));
  let Rgb(r, g, b) = shell.get("bar", "background").and_then(Rgb::hex).unwrap_or(bg);
  let font = |key: &str, mult: f64| shell.num("font", key).filter(|n| *n > 0.0).unwrap_or((base * mult).round().max(1.0));
  let gaps_out = outer_gap();
  BarStyle {
    size_horizontal: token("size-horizontal", 26.0),
    size_vertical: token("size-vertical", 28.0),
    icon_slot: token("icon-slot", 27.0),
    icon_canvas: token("icon-canvas", 16.0),
    icon_font: token("icon-font", 13.0),
    status_slot: token("status-slot", 21.0),
    background: [r, g, b, shell.num("bar", "background-alpha").unwrap_or(1.0)],
    text: pick("text", fg),
    active: pick("active", urgent),
    accent: rgba(accent, 1.0),
    radius: rounding() as f64,
    spacing: spacing_scale(&shell),
    gaps_out,
    font_body: font("body", 1.0),
    font_caption: font("caption", 0.833),
    font_base: base,
  }
}

// The image picker's colors (Color.qml's imagePicker): the scrim, the
// labels, the selected and other slices' borders, and the background that
// dims the other slices and outlines the labels.
#[derive(Clone, Debug, PartialEq)]
pub struct ImagePicker {
  pub scrim: [f64; 4],
  pub text: [f64; 4],
  pub selected_border: [f64; 4],
  pub unselected_border: [f64; 4],
  pub background: [f64; 4],
}

pub fn image_picker() -> ImagePicker {
  let dir = theme_dir();
  let colors = dir.join("colors.toml");
  let shell = Toml::read(&dir.join("shell.toml"));
  let fg = color_value(&colors, "foreground").unwrap_or(Rgb(0.79, 0.8, 0.8));
  let bg = color_value(&colors, "background").unwrap_or(Rgb(0.063, 0.075, 0.082));
  let accent = color_value(&colors, "accent").unwrap_or(fg);
  let composed = |key: &str, fallback: Rgb, alpha: f64| {
    let Rgb(r, g, b) = shell.get("image-picker", key).and_then(Rgb::hex).unwrap_or(fallback);
    [r, g, b, shell.num("image-picker", &format!("{key}-alpha")).unwrap_or(alpha)]
  };
  ImagePicker {
    scrim: composed("scrim", bg, 0.5),
    text: shell.get("image-picker", "text").and_then(rgba_of).unwrap_or([fg.0, fg.1, fg.2, 1.0]),
    selected_border: composed("selected-border", accent, 1.0),
    unselected_border: composed("unselected-border", fg, 0.28),
    background: [bg.0, bg.1, bg.2, 1.0],
  }
}

// Keeps one app's CSS in place and current: reload() rebuilds it when the
// theme files changed (a theme switch), and says whether it did.
pub struct Styler {
  provider: gtk::CssProvider,
  app_css: &'static str,
  seen: std::cell::RefCell<Vec<Option<SystemTime>>>,
}

impl Styler {
  pub fn install(app_css: &'static str) -> Styler {
    let provider = gtk::CssProvider::new();
    if let Some(display) = gtk::gdk::Display::default() {
      gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_USER);
    }
    let styler = Styler { provider, app_css, seen: Default::default() };
    styler.reload(true);
    styler
  }

  pub fn reload(&self, force: bool) -> bool {
    let fp = fingerprint();
    if !force && *self.seen.borrow() == fp {
      return false;
    }
    *self.seen.borrow_mut() = fp;
    self.provider.load_from_string(&css(self.app_css));
    true
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn comments_stop_at_quotes() {
    assert_eq!(strip_comment(r##"text = "#1b2a3d" # navy"##).trim(), r##"text = "#1b2a3d""##);
  }

  #[test]
  fn borders_follow_references_and_gradients() {
    let shell = Toml(vec![
      ("hyprland".into(), "active-border".into(), "rgba(1f6bffee) rgba(2fd4ffee) 45deg".into()),
      ("popups".into(), "border".into(), "hyprland.active-border".into()),
    ]);
    assert_eq!(
      border_image(&shell, "hyprland.active-border", "#000000", 0),
      "linear-gradient(45deg, rgba(31,107,255,0.933), rgba(47,212,255,0.933))"
    );
    assert_eq!(border_image(&shell, "#1f7ae0", "#000000", 0), "linear-gradient(#1f7ae0, #1f7ae0)");
  }

  #[test]
  fn lighter_matches_qt() {
    // QColor("#120a0a").lighter(220) is #271616 and #ff0000's lighter(150)
    // #ff7f7f: Qt truncates to integers, this rounds.
    assert_eq!(Rgb::hex("#120a0a").unwrap().lighter(2.2).css(), "#281616");
    assert_eq!(Rgb::hex("#ff0000").unwrap().lighter(1.5).css(), "#ff8080");
  }

  #[test]
  fn paints_resolve_references_and_scale_alpha() {
    let shell = Toml(vec![("hyprland".into(), "active-border-foreground".into(), "rgba(ff000080) rgb(00ff00) 45deg".into())]);
    let p = paint(&shell, "hyprland.active-border-foreground", [0.0; 4], 0.5, 0);
    assert_eq!(p.angle, 45.0);
    assert_eq!(p.colors, vec![[1.0, 0.0, 0.0, 128.0 / 255.0 * 0.5], [0.0, 1.0, 0.0, 0.5]]);
    assert_eq!(paint(&shell, "", [1.0, 1.0, 1.0, 1.0], 1.0, 0).colors, vec![[1.0; 4]]);
  }
}

// Where shell surfaces sit relative to the bar: its edge (shell.json
// bar.position), how far a surface must stay from that edge (the bar's size,
// from shell.toml [bar], plus the gap) and the plain gap elsewhere, which is
// half of mango's outer gap (Style.qml's gapsOut).
pub struct Placement {
  pub bar_edge: String,
  pub clearance: i32,
  pub gap: i32,
}

pub fn placement() -> Placement {
  let home = std::env::var("HOME").unwrap_or_default();
  let bar_edge = std::fs::read_to_string(PathBuf::from(&home).join(".config/omarchy/shell.json"))
    .ok()
    .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
    .and_then(|v| v["bar"]["position"].as_str().map(String::from))
    .unwrap_or_else(|| "top".into());
  let shell = Toml::read(&theme_dir().join("shell.toml"));
  let vertical = bar_edge == "left" || bar_edge == "right";
  let base = shell.num("font", "base-size").filter(|n| *n >= 1.0).unwrap_or(12.0);
  let scale = if shell.get("bar", "scale-with-font") == Some("false") { 1.0 } else { base / 12.0 };
  let size = if vertical {
    shell.num("bar", "size-vertical").unwrap_or(28.0)
  } else {
    shell.num("bar", "size-horizontal").unwrap_or(26.0)
  };
  let gap = outer_gap();
  Placement { bar_edge, clearance: (size * scale).round() as i32 + gap, gap }
}
