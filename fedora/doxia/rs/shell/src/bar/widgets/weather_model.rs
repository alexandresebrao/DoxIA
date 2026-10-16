// Weather data, as shell/plugins/panels/weather/Model.js: the configured
// location (weather.json, owned by omarchy-weather-location), wttr.in's
// report and Open-Meteo's forecast (current conditions, three days and
// the hourly series), the units and the Nerd Font weather glyphs.

use serde_json::Value;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Location {
  pub name: String,
  pub latitude: Option<f64>,
  pub longitude: Option<f64>,
}

fn num(v: &Value) -> Option<f64> {
  match v {
    Value::Number(n) => n.as_f64(),
    Value::String(s) => s.trim().parse().ok(),
    _ => None,
  }
}

pub fn parse_location_file(raw: &str) -> Location {
  let Ok(data) = serde_json::from_str::<Value>(raw) else { return Location::default() };
  let (lat, lon) = (num(&data["latitude"]), num(&data["longitude"]));
  let both = lat.is_some() && lon.is_some();
  Location {
    name: data["name"].as_str().unwrap_or("").trim().to_string(),
    latitude: if both { lat } else { None },
    longitude: if both { lon } else { None },
  }
}

fn encode(s: &str) -> String {
  gtk::glib::Uri::escape_string(s, None, false).to_string()
}

// The wttr.in path: coordinates, else the encoded name, else empty (the
// location from the IP address).
pub fn wttr_query(loc: &Location) -> String {
  match (loc.latitude, loc.longitude) {
    (Some(lat), Some(lon)) => format!("{lat},{lon}"),
    _ if !loc.name.is_empty() => encode(&loc.name),
    _ => String::new(),
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Suggestion {
  pub name: String,
  pub description: String,
  pub latitude: f64,
  pub longitude: f64,
}

pub fn parse_geocoding(raw: &str) -> Vec<Suggestion> {
  let Ok(data) = serde_json::from_str::<Value>(raw) else { return Vec::new() };
  data["results"]
    .as_array()
    .into_iter()
    .flatten()
    .filter_map(|r| {
      let name = r["name"].as_str()?.to_string();
      let (latitude, longitude) = (r["latitude"].as_f64()?, r["longitude"].as_f64()?);
      let region: Vec<&str> = [r["admin1"].as_str(), r["country"].as_str()].into_iter().flatten().filter(|s| !s.is_empty()).collect();
      Some(Suggestion { name, description: region.join(", "), latitude, longitude })
    })
    .collect()
}

pub fn rounded(v: &Value) -> String {
  num(v).map(|n| format!("{}", n.round() as i64)).unwrap_or_default()
}

pub fn c_to_f(c: f64) -> f64 {
  c * 9.0 / 5.0 + 32.0
}

pub fn locale_imperial(locale: &str) -> bool {
  let l = locale.replace('.', "_");
  let starts = |p: &str| l.starts_with(p) && l[p.len()..].chars().next().is_none_or(|c| "_.-".contains(c));
  starts("en_US") || starts("en-US") || starts("en_LR") || starts("en-LR") || starts("my")
}

pub fn country_imperial(country: &str) -> Option<bool> {
  let c = country.trim().replace(['.', '_', '-'], " ").to_lowercase();
  if c.is_empty() {
    return None;
  }
  Some(matches!(c.as_str(), "us" | "usa" | "united states" | "united states of america" | "liberia" | "myanmar" | "burma"))
}

pub fn use_imperial(unit: &str, locale: &str, country: &str) -> bool {
  match unit.trim().to_lowercase().as_str() {
    "imperial" => true,
    "metric" => false,
    _ => country_imperial(country).unwrap_or_else(|| locale_imperial(locale)),
  }
}

// Current conditions, metric and imperial, from either source.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Current {
  pub temp_c: String,
  pub temp_f: String,
  pub feels_c: String,
  pub feels_f: String,
  pub wind_kmh: String,
  pub wind_mph: String,
  pub humidity: String,
  pub icon: String,
}

pub fn open_meteo_current(report: &Value) -> Option<Current> {
  let c = &report["current"];
  let t = num(&c["temperature_2m"])?;
  let feels = num(&c["apparent_temperature"]);
  let wind = num(&c["wind_speed_10m"]);
  let code = num(&c["weather_code"]).map(|n| n as i64);
  let night = num(&c["is_day"]) == Some(0.0);
  let r = |v: Option<f64>| v.map(|n| format!("{}", n.round() as i64)).unwrap_or_default();
  Some(Current {
    temp_c: r(Some(t)),
    temp_f: r(Some(c_to_f(t))),
    feels_c: r(feels),
    feels_f: r(feels.map(c_to_f)),
    wind_kmh: r(wind),
    wind_mph: r(wind.map(|w| w * 0.621371)),
    humidity: r(num(&c["relative_humidity_2m"])),
    icon: code.map(|c| icon_open_meteo(c, night)).unwrap_or_default(),
  })
}

pub fn wttr_current(report: &Value) -> Option<Current> {
  let c = report["current_condition"].get(0)?;
  Some(Current {
    temp_c: rounded(&c["temp_C"]),
    temp_f: rounded(&c["temp_F"]),
    feels_c: rounded(&c["FeelsLikeC"]),
    feels_f: rounded(&c["FeelsLikeF"]),
    wind_kmh: rounded(&c["windspeedKmph"]),
    wind_mph: rounded(&c["windspeedMiles"]),
    humidity: rounded(&c["humidity"]),
    icon: num(&c["weatherCode"]).map(|n| icon_code(n as i64, false)).unwrap_or_default(),
  })
}

// The icon for now: Open-Meteo's, unless its model says dry while wttr.in's
// observation says it is raining or snowing (the model often ends a rain
// hours before it does); then wttr.in's, with Open-Meteo's day or night.
pub fn current_icon(meteo: Option<&Value>, wttr: Option<&Value>) -> String {
  let c = meteo.map(|m| &m["current"]);
  let code = c.and_then(|c| num(&c["weather_code"])).map(|n| n as i64);
  let night = c.and_then(|c| num(&c["is_day"])) == Some(0.0);
  let observed = wttr.and_then(|w| w["current_condition"].get(0)).and_then(|o| num(&o["weatherCode"])).map(|n| n as i64);
  match (code, observed) {
    (Some(code), Some(obs)) if code < 51 && wet_code(obs) => icon_code(obs, night),
    (Some(code), _) => icon_open_meteo(code, night),
    (None, Some(obs)) => icon_code(obs, night),
    (None, None) => String::new(),
  }
}

// wttr.in (WWO) codes with rain, drizzle, sleet, snow or thunder.
fn wet_code(code: i64) -> bool {
  !matches!(code, 113 | 116 | 119 | 122 | 143 | 248 | 260)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Day {
  pub date: String,
  pub max_c: String,
  pub min_c: String,
  pub max_f: String,
  pub min_f: String,
  pub icon: String,
}

// The next three days after today, from Open-Meteo, else from wttr.in.
pub fn forecast_days(wttr: Option<&Value>, meteo: Option<&Value>, today: &str) -> Vec<Day> {
  let mut out = Vec::new();
  if let Some(d) = meteo.map(|m| &m["daily"]) {
    let times = d["time"].as_array().cloned().unwrap_or_default();
    for (i, t) in times.iter().enumerate() {
      let date = t.as_str().unwrap_or("");
      if date.get(..10).unwrap_or("") <= today || out.len() >= 3 {
        continue;
      }
      let max = num(&d["temperature_2m_max"][i]);
      let min = num(&d["temperature_2m_min"][i]);
      let r = |v: Option<f64>| v.map(|n| format!("{}", n.round() as i64)).unwrap_or_default();
      out.push(Day {
        date: date.into(),
        max_c: r(max),
        min_c: r(min),
        max_f: r(max.map(c_to_f)),
        min_f: r(min.map(c_to_f)),
        icon: num(&d["weather_code"][i]).map(|c| icon_open_meteo(c as i64, false)).unwrap_or_default(),
      });
    }
  }
  if !out.is_empty() {
    return out;
  }
  for d in wttr.and_then(|w| w["weather"].as_array()).into_iter().flatten() {
    let date = d["date"].as_str().unwrap_or("");
    if date.get(..10).unwrap_or("") <= today || out.len() >= 3 {
      continue;
    }
    // The hour nearest noon stands for the day.
    let icon = d["hourly"]
      .as_array()
      .and_then(|h| h.iter().min_by_key(|e| (num(&e["time"]).unwrap_or(0.0) as i64 - 1200).abs()))
      .and_then(|e| num(&e["weatherCode"]))
      .map(|c| icon_code(c as i64, false))
      .unwrap_or_default();
    out.push(Day {
      date: date.into(),
      max_c: rounded(&d["maxtempC"]),
      min_c: rounded(&d["mintempC"]),
      max_f: rounded(&d["maxtempF"]),
      min_f: rounded(&d["mintempF"]),
      icon,
    });
  }
  out
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hour {
  pub time: String,
  pub hour: u32,
  pub temp_c: Option<f64>,
  pub precip_mm: f64,
  pub probability: Option<f64>,
}

// The next 24 hours from the report's current hour, or the whole of `date`.
pub fn hourly(report: Option<&Value>, date: &str) -> Vec<Hour> {
  let Some(r) = report else { return Vec::new() };
  let h = &r["hourly"];
  let Some(times) = h["time"].as_array() else { return Vec::new() };
  let now = r["current"]["time"].as_str().unwrap_or("").get(..13).unwrap_or("").to_string();
  let start = times.iter().position(|t| {
    let t = t.as_str().unwrap_or("");
    if !date.is_empty() { t.get(..10) == Some(date) } else { now.is_empty() || t.get(..13) == Some(now.as_str()) }
  });
  let Some(start) = start else { return Vec::new() };
  let mut out = Vec::new();
  for (j, t) in times.iter().enumerate().skip(start) {
    let time = t.as_str().unwrap_or("").to_string();
    if out.len() >= 24 || (!date.is_empty() && time.get(..10) != Some(date)) {
      break;
    }
    out.push(Hour {
      hour: time.get(11..13).and_then(|s| s.parse().ok()).unwrap_or(0),
      time,
      temp_c: num(&h["temperature_2m"][j]),
      precip_mm: num(&h["precipitation"][j]).unwrap_or(0.0),
      probability: num(&h["precipitation_probability"][j]),
    });
  }
  out
}

pub fn format_precip(mm: f64, imperial: bool) -> String {
  // pt_BR writes a decimal comma.
  let text = if imperial { format!("{:.2} in", mm / 25.4) } else { format!("{mm:.1} mm") };
  text.replace('.', ",")
}

pub fn icon_open_meteo(code: i64, night: bool) -> String {
  let wttr = match code {
    0 => 113,
    1 | 2 => 116,
    3 => 119,
    45 | 48 => 143,
    51 | 53 | 55 | 56 | 57 | 61 => 266,
    63 | 65 | 66 | 67 | 80 | 81 | 82 => 308,
    71 | 73 | 75 | 77 | 85 | 86 => 338,
    95 | 96 | 99 => 389,
    _ => 119,
  };
  icon_code(wttr, night)
}

pub fn icon_code(code: i64, night: bool) -> String {
  let g = match code {
    113 => if night { '\u{e32b}' } else { '\u{e30d}' },
    116 => if night { '\u{e32e}' } else { '\u{e302}' },
    119 | 122 => '\u{e33d}',
    143 | 248 | 260 => if night { '\u{e346}' } else { '\u{e313}' },
    176 | 263 | 353 => if night { '\u{e333}' } else { '\u{e308}' },
    179 | 227 | 230 | 323 | 326 | 368 => if night { '\u{e327}' } else { '\u{e30a}' },
    182 | 185 | 281 | 284 | 311 | 314 | 317 | 320 | 350 | 362 | 365 | 374 | 377 => '\u{e3ad}',
    200 | 386 | 389 | 392 | 395 => '\u{e31d}',
    266 | 293 | 296 | 299 | 302 | 305 | 308 | 356 | 359 => '\u{e318}',
    329 | 332 | 335 | 338 | 371 => '\u{e31a}',
    _ => '\u{e33d}',
  };
  g.to_string()
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn locations_units_and_forecast() {
    let loc = parse_location_file(r#"{"name": " Curitiba ", "latitude": "-25.4", "longitude": -49.3}"#);
    assert_eq!(loc.name, "Curitiba");
    assert_eq!(wttr_query(&loc), "-25.4,-49.3");
    assert_eq!(wttr_query(&parse_location_file(r#"{"name": "São Paulo"}"#)), "S%C3%A3o%20Paulo");
    assert_eq!(wttr_query(&parse_location_file("")), "");
    assert!(!use_imperial("", "pt_BR.UTF-8", "Brazil"));
    assert!(use_imperial("", "en_US.UTF-8", ""));
    assert!(use_imperial("imperial", "pt_BR", "Brazil"));
    let meteo = json!({"daily": {"time": ["2026-10-13", "2026-10-14"], "temperature_2m_max": [20.4, 22.6], "temperature_2m_min": [10, 11.5], "weather_code": [0, 63]},
      "current": {"time": "2026-10-13T21:00", "temperature_2m": 18.6, "apparent_temperature": 17.2, "wind_speed_10m": 9.4, "relative_humidity_2m": 80, "weather_code": 0, "is_day": 0},
      "hourly": {"time": ["2026-10-13T20:00", "2026-10-13T21:00", "2026-10-14T00:00"], "temperature_2m": [19, 18, 15], "precipitation": [0, 0.4, 1.2], "precipitation_probability": [0, 30, 60]}});
    let days = forecast_days(None, Some(&meteo), "2026-10-13");
    assert_eq!((days.len(), days[0].max_c.as_str(), days[0].min_c.as_str()), (1, "23", "12"));
    let cur = open_meteo_current(&meteo).unwrap();
    assert_eq!((cur.temp_c.as_str(), cur.icon.as_str()), ("19", "\u{e32b}"));
    let hours = hourly(Some(&meteo), "");
    assert_eq!((hours.len(), hours[0].hour), (2, 21));
    assert_eq!(hourly(Some(&meteo), "2026-10-14").len(), 1);
    assert_eq!(format_precip(1.25, false), "1,2 mm");
    assert_eq!(parse_geocoding(r#"{"results": [{"name": "Lapa", "latitude": 1, "longitude": 2, "admin1": "PR", "country": "Brasil"}]}"#)[0].description, "PR, Brasil");
  }

  #[test]
  fn observed_rain_wins_over_a_dry_model() {
    let meteo = json!({"current": {"weather_code": 3, "is_day": 0}});
    let raining = json!({"current_condition": [{"weatherCode": "353"}]});
    let cloudy = json!({"current_condition": [{"weatherCode": "119"}]});
    assert_eq!(current_icon(Some(&meteo), Some(&raining)), icon_code(353, true));
    assert_eq!(current_icon(Some(&meteo), Some(&cloudy)), icon_open_meteo(3, true));
    let storm = json!({"current": {"weather_code": 95, "is_day": 1}});
    assert_eq!(current_icon(Some(&storm), Some(&raining)), icon_open_meteo(95, false));
    assert_eq!(current_icon(None, Some(&raining)), icon_code(353, false));
    assert_eq!(current_icon(None, None), "");
  }
}
