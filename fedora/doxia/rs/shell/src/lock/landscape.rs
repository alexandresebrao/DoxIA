// The lock screen's backdrop, the login screen's landscape redrawn with cairo:
// default/sddm/omarchy/Landscape.qml, step by step (keep the two in step). A
// layered landscape, flat-shaded like an indie game's title screen. The sky,
// the light and the layers' tints follow the clock; the sun and the moon cross
// on an arc. Wind sways the pines and the grass in gusts, cloud shadows drift
// over the hills, each tree casts a shadow away from the sun, and leaves fly by
// day and fireflies glow at night. The weather is wttr.in's current conditions
// (the bar's weather fetches them): cloud cover, rain, snow, fog, storm, wind.

use gtk::cairo;
use serde_json::Value;
use std::f64::consts::PI;

type Rgb = (f64, f64, f64);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
  Clear,
  Partly,
  Cloudy,
  Fog,
  Rain,
  Snow,
  Storm,
}

// The weather the scene shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sky {
  pub kind: Kind,
  // 0–1: how much of the sky the clouds take.
  pub cover: f64,
  // km/h.
  pub wind_speed: f64,
  // 0–1: rain or snow intensity.
  pub precip: f64,
}

impl Default for Sky {
  // Offline: a clear sky.
  fn default() -> Sky {
    Sky { kind: Kind::Clear, cover: 0.15, wind_speed: 8.0, precip: 0.0 }
  }
}

fn num(v: &Value) -> Option<f64> {
  match v {
    Value::Number(n) => n.as_f64(),
    Value::String(s) => s.trim().parse().ok(),
    _ => None,
  }
}

// wttr.in's WWO weather codes.
pub fn kind_of(code: i64) -> Kind {
  match code {
    200 | 386 | 389 | 392 | 395 => Kind::Storm,
    179 | 182 | 185 | 227 | 230 | 317 | 320 | 323 | 326 | 329 | 332 | 335 | 338 | 350 | 362 | 365 | 368 | 371 | 374 | 377 => Kind::Snow,
    176 | 263 | 266 | 281 | 284 | 293 | 296 | 299 | 302 | 305 | 308 | 311 | 314 | 353 | 356 | 359 => Kind::Rain,
    143 | 248 | 260 => Kind::Fog,
    119 | 122 => Kind::Cloudy,
    116 => Kind::Partly,
    _ => Kind::Clear,
  }
}

// From wttr.in's current_condition entry.
pub fn sky_of(current: &Value) -> Option<Sky> {
  let kind = kind_of(num(&current["weatherCode"])? as i64);
  let wet = matches!(kind, Kind::Rain | Kind::Storm | Kind::Snow);
  let mm = num(&current["precipMM"]).unwrap_or(0.0);
  Some(Sky {
    kind,
    cover: (num(&current["cloudcover"]).unwrap_or(15.0) / 100.0).clamp(0.0, 1.0),
    wind_speed: num(&current["windspeedKmph"]).unwrap_or(8.0),
    precip: if wet { (mm / 4.0).clamp(0.3, 1.0) } else { 0.0 },
  })
}

// Deterministic noise, so the scenery stays put between locks.
pub fn rnd(i: f64) -> f64 {
  let x = (i * 127.1 + 311.7).sin() * 43758.5453;
  x - x.floor()
}

fn hex(s: &str) -> Rgb {
  let v = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0);
  (((v >> 16) & 255) as f64 / 255.0, ((v >> 8) & 255) as f64 / 255.0, (v & 255) as f64 / 255.0)
}

fn mix(a: Rgb, b: Rgb, k: f64) -> Rgb {
  (a.0 + (b.0 - a.0) * k, a.1 + (b.1 - a.1) * k, a.2 + (b.2 - a.2) * k)
}

// Overcast: the colors drift toward their own grey and darken a little.
fn grey(c: Rgb, k: f64) -> Rgb {
  let l = 0.3 * c.0 + 0.59 * c.1 + 0.11 * c.2;
  mix(c, (l * 0.82, l * 0.86, l * 0.92), k)
}

// Keyframes over the day: sky top and horizon, far ridge, near ridge, hills,
// foreground, foliage, and how much daylight there is (shadows, stars).
const KEYS: [(f64, [&str; 7], f64); 10] = [
  (0.0, ["#070b1d", "#18234a", "#1b2547", "#141c38", "#0f1630", "#0a0f22", "#0c1424"], 0.0),
  (4.8, ["#0b1230", "#2a2f5c", "#232a52", "#1a2142", "#131a36", "#0c1226", "#0e1628"], 0.0),
  (5.8, ["#29366b", "#f19a7a", "#5a4f7d", "#3e3a63", "#2c2d4e", "#1d2036", "#1a2230"], 0.25),
  (7.0, ["#5d8fd1", "#fbd8b0", "#8ea3c7", "#5f7aa0", "#4c7356", "#2f4f37", "#26432f"], 0.75),
  (12.0, ["#3f84d6", "#bfe2f6", "#9cb8d6", "#6f93b4", "#5b8c5a", "#3c6b3f", "#2f5a34"], 1.0),
  (16.5, ["#4f86cf", "#f6dcae", "#a6aac4", "#7a8aa8", "#6a8a52", "#466a3a", "#365a2f"], 0.9),
  (18.3, ["#3b3a73", "#ff8a52", "#8c5f7e", "#5e4766", "#4a4a45", "#2c3329", "#232c24"], 0.45),
  (19.3, ["#1d2150", "#874f7c", "#3c3462", "#2a274c", "#1f2238", "#141828", "#121a26"], 0.1),
  (20.5, ["#0a0f26", "#202850", "#1d2648", "#151d3a", "#101831", "#0b1024", "#0d1526"], 0.0),
  (24.0, ["#070b1d", "#18234a", "#1b2547", "#141c38", "#0f1630", "#0a0f22", "#0c1424"], 0.0),
];

// The light at `hour`: the 7 colors above and the daylight.
fn light(hour: f64) -> ([Rgb; 7], f64) {
  for w in KEYS.windows(2) {
    let (a, b) = (&w[0], &w[1]);
    if hour >= a.0 && hour <= b.0 {
      let mut k = (hour - a.0) / (b.0 - a.0);
      k = k * k * (3.0 - 2.0 * k);
      let colors = std::array::from_fn(|i| mix(hex(a.1[i]), hex(b.1[i]), k));
      return (colors, a.2 + (b.2 - a.2) * k);
    }
  }
  (KEYS[0].1.map(hex), KEYS[0].2)
}

// Everything the frame's drawing reads, worked out once.
pub struct Scene {
  pub w: f64,
  pub h: f64,
  pub t: f64,
  pub sky: Sky,
  wind_base: f64,
  wind: f64,
  raining: bool,
  snowing: bool,
  fog: f64,
  overcast: f64,
  sky_top: Rgb,
  sky_horizon: Rgb,
  far: Rgb,
  near: Rgb,
  hill: Rgb,
  fg: Rgb,
  leaf: Rgb,
  daylight: f64,
  sunlight: f64,
  night: f64,
  sun_phase: f64,
  moon_phase: f64,
  sun_up: bool,
  sun_x: f64,
  shadow_lean: f64,
  shadow_length: f64,
}

impl Scene {
  pub fn new(w: f64, h: f64, t: f64, hour: f64, sky: Sky) -> Scene {
    // Wind: a steady breeze plus gusts that roll in every few seconds.
    let gust = ((t * 0.23).sin() * (t * 0.61 + 1.3).sin()).max(0.0);
    let wind_base = (0.15 + sky.wind_speed / 30.0).min(1.6);
    let raining = matches!(sky.kind, Kind::Rain | Kind::Storm);
    let snowing = sky.kind == Kind::Snow;
    let fog = if sky.kind == Kind::Fog { 1.0 } else if raining { 0.35 } else if snowing { 0.4 } else { 0.0 };
    let storm = sky.kind == Kind::Storm;
    let overcast = (((sky.cover - 0.3) / 0.7).clamp(0.0, 1.0) * 0.8
      + if raining { 0.25 } else { 0.0 }
      + if storm { 0.2 } else { 0.0 }
      + fog * 0.3)
      .min(1.0);
    let ([top, hor, far, near, hill, fg, leaf], day) = light(hour);
    let daylight = day * (1.0 - overcast * 0.35);
    let sunlight = daylight * (1.0 - overcast);
    // The sun from 6h to 18h, the moon from 18h to 6h, on the same arc.
    let sun_phase = (hour - 6.0) / 12.0;
    let sun_up = sun_phase > -0.04 && sun_phase < 1.04;
    let sun_x = w * (0.08 + 0.84 * sun_phase);
    Scene {
      w,
      h,
      t,
      sky,
      wind_base,
      wind: wind_base * (0.45 + 0.55 * gust),
      raining,
      snowing,
      fog,
      overcast,
      sky_top: grey(top, overcast * 0.85),
      sky_horizon: grey(hor, overcast * 0.8),
      far: grey(far, overcast * 0.6),
      near: grey(near, overcast * 0.5),
      hill: grey(hill, overcast * 0.4),
      fg: grey(fg, overcast * 0.35),
      leaf: grey(leaf, overcast * 0.35),
      daylight,
      sunlight,
      night: 1.0 - (daylight * 3.0).min(1.0),
      sun_phase,
      moon_phase: ((hour + 6.0) % 24.0) / 12.0,
      sun_up,
      sun_x,
      // Shadows lean away from the sun and lengthen as it sinks.
      shadow_lean: if sun_up { ((w / 2.0 - sun_x) / (w / 2.0)).clamp(-1.0, 1.0) } else { 0.0 },
      shadow_length: if sun_up { 0.6 + 2.2 * (1.0 - (PI * sun_phase.clamp(0.02, 0.98)).sin()) } else { 0.0 },
    }
  }

  fn arc_x(&self, p: f64) -> f64 {
    self.w * (0.08 + 0.84 * p)
  }

  fn arc_y(&self, p: f64) -> f64 {
    self.h * (0.62 - 0.5 * (PI * p.clamp(0.0, 1.0)).sin())
  }

  // A ridge line from left to right.
  fn ridge(&self, seed: f64, base: f64, amp: f64, steps: u32, smooth: bool) -> Vec<(f64, f64)> {
    (0..=steps)
      .map(|i| {
        let f = i as f64 / steps as f64;
        let mut y = base;
        y -= amp * 0.55 * (f * PI * (1.3 + rnd(seed) * 1.5) + rnd(seed + 1.0) * 6.0).sin();
        y -= amp * 0.3 * (f * PI * (4.0 + rnd(seed + 2.0) * 3.0) + rnd(seed + 3.0) * 6.0).sin();
        if !smooth {
          y -= amp * 0.25 * (rnd(seed * 31.0 + i as f64) - 0.5);
        }
        (self.w * f, y)
      })
      .collect()
  }

  pub fn ground_y(&self, x: f64) -> f64 {
    let p = x / self.w;
    self.h * (0.8 + 0.08 * p - 0.035 * (p * PI * 1.6 + 0.4).sin() - 0.012 * (p * PI * 6.3).sin())
  }

  // A pine's lean, in radians, for its sway and its shadow.
  fn lean(&self, sway: f64, phase: f64) -> f64 {
    sway * (0.02 + 0.05 * self.wind) * (self.t * (1.1 + 0.2 * phase.sin()) + phase).sin() + sway * 0.03 * self.wind
  }
}

fn rgba(cr: &cairo::Context, c: Rgb, a: f64) {
  cr.set_source_rgba(c.0, c.1, c.2, a);
}

// Filled down to the bottom edge.
fn fill_line(cr: &cairo::Context, s: &Scene, pts: &[(f64, f64)], c: Rgb) {
  cr.move_to(0.0, s.h);
  for (x, y) in pts {
    cr.line_to(*x, *y);
  }
  cr.line_to(s.w, s.h);
  cr.close_path();
  rgba(cr, c, 1.0);
  let _ = cr.fill();
}

// A vertical band of the horizon color, from clear at the top to `alpha`.
fn haze(cr: &cairo::Context, s: &Scene, y: f64, height: f64, alpha: f64) {
  let g = cairo::LinearGradient::new(0.0, y, 0.0, y + height);
  let c = s.sky_horizon;
  g.add_color_stop_rgba(0.0, c.0, c.1, c.2, 0.0);
  g.add_color_stop_rgba(1.0, c.0, c.1, c.2, alpha);
  let _ = cr.set_source(&g);
  cr.rectangle(0.0, y, s.w, height);
  let _ = cr.fill();
}

fn circle(cr: &cairo::Context, cx: f64, cy: f64, r: f64) {
  cr.new_sub_path();
  cr.arc(cx, cy, r, 0.0, 2.0 * PI);
}

// A rectangle with fully rounded ends, lying or standing.
fn pill(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64) {
  let r = (h / 2.0).min(w / 2.0);
  cr.new_sub_path();
  if w >= h {
    cr.arc(x + w - r, y + r, r, -PI / 2.0, PI / 2.0);
    cr.arc(x + r, y + r, r, PI / 2.0, 3.0 * PI / 2.0);
  } else {
    cr.arc(x + r, y + r, r, PI, 2.0 * PI);
    cr.arc(x + r, y + h - r, r, 0.0, PI);
  }
  cr.close_path();
}

// A pine: three stacked tiers and a trunk, swaying from its base. (x, y) is
// its box's top left; the lit side faces the sun.
fn pine(cr: &cairo::Context, s: &Scene, x: f64, y: f64, height: f64, color: Rgb, lean: f64) {
  let w = height * 0.42;
  let _ = cr.save();
  cr.translate(x + w / 2.0, y + height);
  cr.rotate(lean);
  cr.translate(-w / 2.0, -height);
  rgba(cr, (color.0 / 1.4, color.1 / 1.4, color.2 / 1.4), 1.0);
  cr.rectangle((w - w * 0.1) / 2.0, height * 0.86, w * 0.1, height * 0.14);
  let _ = cr.fill();
  let tiers = [
    (0.5, 0.0),
    (0.78, 0.34),
    (0.64, 0.34),
    (0.9, 0.6),
    (0.72, 0.6),
    (1.0, 0.88),
    (0.0, 0.88),
    (0.28, 0.6),
    (0.1, 0.6),
    (0.36, 0.34),
    (0.22, 0.34),
  ];
  for (i, (px, py)) in tiers.iter().enumerate() {
    if i == 0 {
      cr.move_to(w * px, height * py);
    } else {
      cr.line_to(w * px, height * py);
    }
  }
  cr.close_path();
  rgba(cr, color, 1.0);
  let _ = cr.fill();
  if s.sunlight > 0.0 {
    cr.move_to(w * 0.5, 0.0);
    cr.line_to(w * if s.sun_x > x { 0.78 } else { 0.22 }, height * 0.34);
    cr.line_to(w * 0.5, height * 0.34);
    cr.close_path();
    rgba(cr, hex("#fff3c4"), 0.18 * s.sunlight);
    let _ = cr.fill();
  }
  let _ = cr.restore();
}

pub fn draw(cr: &cairo::Context, s: &Scene) {
  let (w, h, t) = (s.w, s.h, s.t);

  // ---- Sky ----
  let g = cairo::LinearGradient::new(0.0, 0.0, 0.0, h);
  for (at, c) in [(0.0, s.sky_top), (0.62, s.sky_horizon), (1.0, s.sky_horizon)] {
    g.add_color_stop_rgb(at, c.0, c.1, c.2);
  }
  let _ = cr.set_source(&g);
  let _ = cr.paint();

  // Stars, twinkling, gone by day.
  let starry = s.night * (1.0 - s.overcast);
  if starry > 0.0 {
    for i in 0..140 {
      let i = i as f64;
      let tw = 0.5 + 0.5 * (t * (0.8 + rnd(i + 900.0) * 2.2) + i).sin();
      let a = starry * (0.25 + 0.75 * tw) * (0.4 + 0.6 * rnd(i + 600.0));
      if a <= 0.01 {
        continue;
      }
      let size = if rnd(i + 800.0) > 0.92 { 3.0 } else { 2.0 };
      circle(cr, rnd(i + 500.0) * w + size / 2.0, rnd(i + 700.0).powf(1.6) * h * 0.55 + size / 2.0, size / 2.0);
      rgba(cr, hex("#fdf6e3"), a);
      let _ = cr.fill();
    }
  }

  // The sun's glow, then the disc.
  if s.sun_up {
    let (sx, sy) = (s.sun_x, s.arc_y(s.sun_phase));
    let glow = (s.sunlight * 1.6).min(1.0);
    if glow > 0.0 {
      let g = cairo::RadialGradient::new(sx, sy, 0.0, sx, sy, h * 0.45);
      g.add_color_stop_rgba(0.0, 1.0, 0.92, 0.7, 0.55 * glow);
      g.add_color_stop_rgba(0.25, 1.0, 0.8, 0.55, 0.18 * glow);
      g.add_color_stop_rgba(1.0, 1.0, 0.8, 0.55, 0.0);
      let _ = cr.set_source(&g);
      let _ = cr.paint();
    }
    let a = (1.0 - s.overcast * 1.6).max(0.0);
    if a > 0.0 {
      let c = mix(hex("#fff1c9"), hex("#ffb36b"), 1.0 - (PI * s.sun_phase.clamp(0.0, 1.0)).sin());
      circle(cr, sx, sy, h * 0.075 / 2.0);
      rgba(cr, c, a);
      let _ = cr.fill();
    }
  }

  // The moon: a disc with a shadow bite.
  let moon = s.night * (1.0 - s.overcast * 1.6).max(0.0);
  if s.moon_phase > -0.04 && s.moon_phase < 1.04 && moon > 0.0 {
    let size = h * 0.055;
    let (mx, my) = (s.arc_x(s.moon_phase) - size / 2.0, s.arc_y(s.moon_phase) - size / 2.0);
    cr.push_group();
    circle(cr, mx + size / 2.0, my + size / 2.0, size / 2.0);
    rgba(cr, hex("#eef0f8"), 1.0);
    let _ = cr.fill();
    circle(cr, mx + size * 0.32 + size / 2.0, my - size * 0.12 + size / 2.0, size / 2.0);
    rgba(cr, s.sky_top, 0.92);
    let _ = cr.fill();
    circle(cr, mx + size / 2.0, my + size / 2.0, size * 1.6);
    rgba(cr, hex("#dfe6ff"), 0.06);
    let _ = cr.fill();
    let _ = cr.pop_group_to_source();
    let _ = cr.paint_with_alpha(moon);
  }

  // Clouds: soft puffs drifting with the wind, lit by the sky.
  let storm = s.sky.kind == Kind::Storm;
  let tint = mix(
    mix(s.sky_horizon, (1.0, 1.0, 1.0), 0.35 + 0.45 * s.daylight),
    hex("#5d6470"),
    s.overcast * if storm { 0.8 } else { 0.5 },
  );
  let cloud_alpha = (0.12 + 0.73 * s.daylight + s.overcast * 0.5).min(1.0);
  let clouds = 2 + (s.sky.cover * 12.0).round() as usize;
  for i in 0..14usize.min(clouds) {
    let f = i as f64;
    let cw = w * (0.08 + rnd(f + 41.0) * 0.07) * (1.0 + s.sky.cover * 0.9);
    let ch = cw * 0.8;
    let span = w + cw * 2.0;
    let x = ((rnd(f + 42.0) * span + t * (6.0 + rnd(f + 40.0) * 10.0)) % span) - cw;
    let y = h * (0.03 + rnd(f + 43.0) * (0.18 + s.sky.cover * 0.1));
    // Faded as one, so the puffs don't show through each other.
    let _ = cr.save();
    cr.rectangle(x, y, cw, ch);
    cr.clip();
    cr.push_group();
    rgba(cr, tint, 1.0);
    for j in 0..4 {
      let j = j as f64;
      let size = cw * (0.3 + rnd(f * 13.0 + j * 7.0 + 3.0) * 0.26);
      let px = (cw * (0.04 + j * 0.17)).min(cw - size);
      let py = (ch - size * (0.62 + 0.3 * rnd(f * 5.0 + j)) - ch * 0.04).max(0.0);
      circle(cr, x + px + size / 2.0, y + py + size / 2.0, size / 2.0);
      let _ = cr.fill();
    }
    pill(cr, x + cw * 0.04, y + ch - ch * 0.2, cw * 0.92, ch * 0.2);
    let _ = cr.fill();
    let _ = cr.pop_group_to_source();
    let _ = cr.paint_with_alpha(cloud_alpha);
    let _ = cr.restore();
  }

  // ---- Land ----
  fill_line(cr, s, &s.ridge(3.0, h * 0.56, h * 0.16, 90, false), s.far);
  haze(cr, s, h * 0.5, h * 0.2, 0.45 + 0.4 * s.fog);
  fill_line(cr, s, &s.ridge(11.0, h * 0.66, h * 0.1, 70, false), s.near);

  // Distant forest line on the near ridge.
  let distant = mix(s.near, s.leaf, 0.35);
  for i in 0..70 {
    let f = i as f64;
    let ph = h * (0.035 + rnd(f + 1200.0) * 0.03);
    let x = f / 70.0 * w + rnd(f + 1300.0) * 12.0;
    let y = h * (0.645 + rnd(f + 1400.0) * 0.05) - ph;
    pine(cr, s, x, y, ph, distant, s.lean(0.3, f * 0.37));
  }

  haze(cr, s, h * 0.62, h * 0.16, 0.3 + 0.5 * s.fog);
  fill_line(cr, s, &s.ridge(23.0, h * 0.76, h * 0.07, 50, true), s.hill);

  // Foreground: a rise on the left, where the trees stand, sloping right.
  let ground: Vec<(f64, f64)> = (0..=60).map(|i| w * i as f64 / 60.0).map(|x| (x, s.ground_y(x))).collect();
  fill_line(cr, s, &ground, s.fg);

  // Cloud shadows sliding over the hills and the foreground: soft-edged
  // patches (the QML blurs a pill by 64 px).
  let shade = 0.16 * s.sunlight * (s.sky.cover * 3.0).min(1.0);
  if shade > 0.005 {
    for i in 0..3 {
      let f = i as f64;
      let bw = w * (0.3 + rnd(f + 60.0) * 0.2);
      let bh = h * 0.06;
      let span = w + bw;
      let x = ((rnd(f + 61.0) * span + t * (9.0 + f * 3.0)) % span) - bw;
      let y = h * (0.74 + f * 0.06);
      let _ = cr.save();
      cr.translate(x + bw / 2.0, y + bh / 2.0);
      cr.scale(bw / 2.0 + 48.0, bh / 2.0 + 48.0);
      let g = cairo::RadialGradient::new(0.0, 0.0, 0.0, 0.0, 0.0, 1.0);
      let c = hex("#000814");
      g.add_color_stop_rgba(0.0, c.0, c.1, c.2, shade);
      g.add_color_stop_rgba(0.55, c.0, c.1, c.2, shade * 0.8);
      g.add_color_stop_rgba(1.0, c.0, c.1, c.2, 0.0);
      let _ = cr.set_source(&g);
      circle(cr, 0.0, 0.0, 1.0);
      let _ = cr.fill();
      let _ = cr.restore();
    }
  }

  // Trees on the foreground rise, each with its shadow: clusters off the
  // clock and the sign-in (the left edge, the middle, the right edge).
  for i in 0..14 {
    let f = i as f64;
    let p = if i < 3 {
      rnd(f + 2000.0) * 0.05
    } else if i < 7 {
      0.24 + rnd(f + 2000.0) * 0.1
    } else if i < 11 {
      0.68 + rnd(f + 2000.0) * 0.1
    } else {
      0.93 + rnd(f + 2000.0) * 0.06
    };
    let th = h * (0.13 + rnd(f + 2100.0) * 0.14);
    let base_x = p * w;
    let base_y = s.ground_y(base_x) + 4.0;
    let lean = s.lean(1.0, f * 1.9);
    let shadow = 0.32 * s.sunlight;
    if shadow > 0.01 {
      let len = th * (0.22 + 0.5 * s.shadow_length * s.shadow_lean.abs());
      let sh = th * 0.05;
      pill(cr, base_x - len / 2.0 + s.shadow_lean * len * 0.45 + lean * th * 0.4, base_y - sh * 0.6, len, sh);
      rgba(cr, hex("#020611"), shadow);
      let _ = cr.fill();
    }
    pine(cr, s, base_x - th * 0.42 / 2.0, base_y - th, th, s.leaf, lean);
  }

  // Grass along the foreground edge.
  rgba(cr, mix(s.fg, s.leaf, 0.6), 1.0);
  for i in 0..150 {
    let f = i as f64;
    let gx = rnd(f + 3000.0) * w;
    let gh = h * (0.012 + rnd(f + 3100.0) * 0.02);
    let deg = (6.0 + 22.0 * s.wind) * (t * 2.4 + gx * 0.02).sin() * 0.5 + 10.0 * s.wind;
    let _ = cr.save();
    cr.translate(gx + 1.0, s.ground_y(gx) + 3.0);
    cr.rotate(deg.to_radians());
    pill(cr, -1.0, -gh, 2.0, gh);
    let _ = cr.fill();
    let _ = cr.restore();
  }

  let calm = !(s.raining || s.snowing);

  // Leaves riding the gusts, by day.
  let leaves = (s.daylight * 1.5).min(1.0) * 0.85;
  if calm && leaves > 0.01 {
    for i in 0..10 {
      let f = i as f64;
      let speed = 60.0 + rnd(f + 4000.0) * 80.0;
      let span = w + 100.0;
      let x = ((rnd(f + 4001.0) * span + t * speed * (0.5 + s.wind)) % span) - 50.0;
      let y = h * (0.55 + rnd(f + 4002.0) * 0.35) + 30.0 * (t * 1.3 + f).sin();
      let c = match i % 3 {
        0 => hex("#c9842f"),
        1 => hex("#9fb34a"),
        _ => hex("#d9a441"),
      };
      let _ = cr.save();
      cr.translate(x + 3.5, y + 2.0);
      cr.rotate((t * (120.0 + f * 20.0)).to_radians());
      pill(cr, -3.5, -2.0, 7.0, 4.0);
      rgba(cr, c, leaves);
      let _ = cr.fill();
      let _ = cr.restore();
    }
  }

  // Fireflies, by night.
  if calm && s.night > 0.0 {
    let c = hex("#f6ff9e");
    for i in 0..24 {
      let f = i as f64;
      let a = s.night * (t * (0.7 + rnd(f + 5000.0)) + f * 2.1).sin().max(0.0);
      if a <= 0.01 {
        continue;
      }
      let x = rnd(f + 5001.0) * w * 0.6 + 40.0 * (t * 0.3 + f).sin() + 2.0;
      let y = h * (0.72 + rnd(f + 5002.0) * 0.22) + 20.0 * (t * 0.4 + f * 1.3).cos() + 2.0;
      circle(cr, x, y, 8.0);
      rgba(cr, c, a * 0.25);
      let _ = cr.fill();
      circle(cr, x, y, 2.0);
      rgba(cr, c, a);
      let _ = cr.fill();
    }
  }

  // Fog: a veil over the land.
  if s.fog > 0.01 {
    let g = cairo::LinearGradient::new(0.0, 0.0, 0.0, h);
    let c = s.sky_horizon;
    g.add_color_stop_rgba(0.35, c.0, c.1, c.2, 0.0);
    g.add_color_stop_rgba(0.75, c.0, c.1, c.2, 0.45 * s.fog);
    g.add_color_stop_rgba(1.0, c.0, c.1, c.2, 0.25 * s.fog);
    let _ = cr.set_source(&g);
    let _ = cr.paint();
  }

  // Rain: streaks slanted by the wind.
  if s.raining {
    let drops = (220.0 * s.sky.precip).ceil() as usize;
    for i in 0..drops.min(220) {
      let f = i as f64;
      let speed = 900.0 + rnd(f + 6000.0) * 500.0;
      let fall = (rnd(f + 6001.0) * h * 1.2 + t * speed) % (h * 1.2);
      let len = 14.0 + rnd(f + 6002.0) * 12.0;
      let x = (rnd(f + 6003.0) * w * 1.3 - w * 0.15 + fall * s.wind_base * 0.35) % (w * 1.15);
      let y = fall - h * 0.1;
      let _ = cr.save();
      cr.translate(x + 0.75, y + len / 2.0);
      cr.rotate((-s.wind_base * 18.0).to_radians());
      pill(cr, -0.75, -len / 2.0, 1.5, len);
      cr.set_source_rgba(0.85, 0.9, 1.0, 0.35 + 0.25 * rnd(f + 6004.0));
      let _ = cr.fill();
      let _ = cr.restore();
    }
  }

  // Snow: flakes drifting down, swaying.
  if s.snowing {
    let flakes = (160.0 * s.sky.precip).ceil() as usize;
    for i in 0..flakes.min(160) {
      let f = i as f64;
      let speed = 40.0 + rnd(f + 7000.0) * 50.0;
      let fall = (rnd(f + 7001.0) * h * 1.1 + t * speed) % (h * 1.1);
      let size = 2.0 + rnd(f + 7002.0) * 4.0;
      let x = (rnd(f + 7003.0) * w + fall * s.wind_base * 0.25 + 18.0 * (t * 0.9 + f).sin()) % w;
      circle(cr, x + size / 2.0, fall - h * 0.05 + size / 2.0, size / 2.0);
      cr.set_source_rgba(1.0, 1.0, 1.0, 0.6 + 0.4 * rnd(f + 7004.0));
      let _ = cr.fill();
    }
  }

  // Lightning: a flash every so often in a storm.
  if storm {
    let slot = (t / 6.0).floor();
    let at = t - slot * 6.0;
    if rnd(slot + 8000.0) > 0.45 {
      let a = (0.55 * (1.0 - at * 6.0)).max(0.0) + (0.35 * (1.0 - (at - 0.25).abs() * 12.0)).max(0.0);
      if a > 0.01 {
        rgba(cr, hex("#e8eeff"), a);
        let _ = cr.paint();
      }
    }
  }

  // Shade under the clock and under the sign-in (Main.qml), so the text reads
  // on a bright daytime sky and on the grass.
  let g = cairo::LinearGradient::new(0.0, 0.0, w, 0.0);
  g.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, 0.38);
  g.add_color_stop_rgba(0.38, 0.0, 0.0, 0.0, 0.06);
  g.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.0);
  let _ = cr.set_source(&g);
  let _ = cr.paint();
  let g = cairo::LinearGradient::new(0.0, h * 0.55, 0.0, h);
  g.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, 0.0);
  g.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.5);
  let _ = cr.set_source(&g);
  cr.rectangle(0.0, h * 0.55, w, h * 0.45);
  let _ = cr.fill();
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn weather_and_light() {
    assert_eq!(kind_of(353), Kind::Rain);
    assert_eq!(kind_of(389), Kind::Storm);
    assert_eq!(kind_of(113), Kind::Clear);
    let sky = sky_of(&json!({"weatherCode": "353", "cloudcover": "100", "windspeedKmph": "6", "precipMM": "0.9"})).unwrap();
    assert_eq!((sky.kind, sky.cover, sky.precip), (Kind::Rain, 1.0, 0.3));
    let dry = sky_of(&json!({"weatherCode": "116", "cloudcover": "40", "windspeedKmph": "12", "precipMM": "0.4"})).unwrap();
    assert_eq!(dry.precip, 0.0);
    assert!(sky_of(&json!({})).is_none());
    // Noon is full daylight, midnight none; keyframe colors land exactly.
    assert_eq!(light(12.0).1, 1.0);
    assert_eq!(light(0.0).1, 0.0);
    assert_eq!(light(12.0).0[0], hex("#3f84d6"));
    let rain = Scene::new(1000.0, 500.0, 0.0, 12.0, sky);
    let clear = Scene::new(1000.0, 500.0, 0.0, 12.0, Sky::default());
    assert!(rain.overcast > 0.9 && clear.overcast == 0.0);
    assert!(rain.sunlight < clear.sunlight);
    assert!((0.0..1.0).contains(&rnd(42.0)));
  }
}
