// omarchy.agents (shell/plugins/agents): AI coding subscriptions' usage, as
// omarchy-agent-usage-update writes it to
// ~/.local/state/omarchy/agents/usage/<agent>.json (every
// "refreshIntervalSec", limits again on opening and 30 s after a record
// advises a retry). The icon turns to the bar's "active" color near a
// limit; a right click starts an agent, a middle click shows the next one.
// Its panel: the provider (switchable when there are several), its
// balance, rate-limit windows with their reset, tokens by day and by
// model. The settings' "providers.<id>.enabled": false leaves one out.
// Not carried over: the cross-machine sync ("syncMode").

use super::super::config::setting;
use super::super::panel::Panel;
use super::super::ui;
use super::super::widget::{Button, Kind, Slot};
use super::super::Ctx;
use gtk::prelude::*;
use gtk::{gdk, gio, glib, pango};
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

fn usage_dir() -> String {
  let state = std::env::var("XDG_STATE_HOME").unwrap_or_else(|_| format!("{}/.local/state", std::env::var("HOME").unwrap_or_default()));
  format!("{state}/omarchy/agents/usage")
}

fn num(v: &Value) -> f64 {
  v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())).unwrap_or(0.0)
}

pub fn token_count(n: f64) -> String {
  if n >= 1e9 {
    format!("{:.1}B", n / 1e9)
  } else if n >= 1e6 {
    format!("{:.1}M", n / 1e6)
  } else if n >= 1e3 {
    format!("{:.1}K", n / 1e3)
  } else {
    format!("{}", n as i64)
  }
}

// "claude-opus-4-1-20250805" -> "Opus 4.1".
pub fn model_name(id: &str) -> String {
  if id.is_empty() {
    return "Unknown".into();
  }
  let mut name = id.strip_prefix("claude-").unwrap_or(id).to_string();
  if let Some(i) = name.rfind('-') {
    if name[i + 1..].len() == 8 && name[i + 1..].bytes().all(|b| b.is_ascii_digit()) {
      name.truncate(i);
    }
  }
  let mut words: Vec<String> = Vec::new();
  let mut version: Vec<&str> = Vec::new();
  for part in name.split('-').filter(|p| !p.is_empty()) {
    if part.starts_with(|c: char| c.is_ascii_digit()) {
      version.push(part);
      continue;
    }
    if !version.is_empty() {
      words.push(version.join("."));
      version.clear();
    }
    words.push(match part {
      "gpt" => "GPT".into(),
      "deepseek" => "DeepSeek".into(),
      w => {
        let mut c = w.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
      }
    });
  }
  if !version.is_empty() {
    words.push(version.join("."));
  }
  if words.is_empty() { "Unknown".into() } else { words.join(" ") }
}

fn window_is_long(t: &str) -> bool {
  ["week", "7-day", "seven", "month", "30-day"].iter().any(|w| t.contains(w))
}

pub fn window_title(label: &str) -> String {
  let t = label.to_lowercase();
  if t.contains("month") {
    "Monthly".into()
  } else if window_is_long(&t) {
    "Weekly".into()
  } else if t.contains("session") || t.chars().any(|c| c.is_ascii_digit()) {
    "Session".into()
  } else {
    let plain = label.split('(').next().unwrap_or("").trim();
    if plain.is_empty() { "Limit".into() } else { plain.into() }
  }
}

pub fn duration(ms: f64) -> String {
  if ms <= 0.0 {
    return "now".into();
  }
  let minutes = (ms / 60000.0).floor() as i64;
  let (hours, days) = (minutes / 60, minutes / 1440);
  if days > 0 {
    format!("{days}d {}h", hours % 24)
  } else if hours > 0 {
    format!("{hours}h {}m", minutes % 60)
  } else {
    format!("{}m", minutes.max(1))
  }
}

fn has_data(p: &Value) -> bool {
  ["totalPrompts", "totalSessions", "activeDays", "todayPrompts", "todaySessions"].iter().any(|k| num(&p[*k]) > 0.0)
    || p["limits"].as_array().is_some_and(|l| !l.is_empty())
    || p["balance"]["remaining"].as_f64().is_some_and(|r| r >= 0.0)
}

fn limits(p: &Value) -> Vec<(String, f64, String)> {
  p["limits"]
    .as_array()
    .into_iter()
    .flatten()
    .filter_map(|l| {
      let percent = l["percent"].as_f64()?;
      (percent >= 0.0).then(|| {
        let title = l["title"].as_str().filter(|t| !t.is_empty()).map(String::from).unwrap_or_else(|| window_title(l["label"].as_str().unwrap_or("")));
        (title, percent, l["resetsAt"].as_str().unwrap_or("").to_string())
      })
    })
    .collect()
}

fn alarming(p: &Value) -> bool {
  let limit = limits(p).iter().any(|(_, pc, _)| *pc >= 0.9);
  let b = &p["balance"];
  let funded = num(&b["funded"]);
  limit || (funded > 0.0 && num(&b["remaining"]) / funded <= 0.1)
}

fn read_providers(entry: &Value) -> Vec<Value> {
  let Ok(dir) = std::fs::read_dir(usage_dir()) else { return Vec::new() };
  let mut files: Vec<_> = dir.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
  files.sort();
  files
    .iter()
    .filter_map(|f| serde_json::from_str::<Value>(&std::fs::read_to_string(f).ok()?).ok())
    .filter(|p| {
      let id = p["id"].as_str().unwrap_or("");
      setting(entry, "providers").and_then(|ps| ps[id]["enabled"].as_bool()) != Some(false)
    })
    .filter(has_data)
    .collect()
}

fn update_command(entry: &Value, kind: &str, ids: &[String]) -> Vec<String> {
  let mut c = vec!["omarchy-agent-usage-update".to_string()];
  match kind {
    "force" => c.push("--force".into()),
    "limits" => c.push("--limits-only".into()),
    _ => {}
  }
  if let Some(Value::Object(ps)) = setting(entry, "providers") {
    for (id, v) in ps {
      if v["enabled"].as_bool() == Some(false) {
        c.push("--except".into());
        c.push(id.clone());
      }
    }
  }
  c.extend(ids.iter().cloned());
  c
}

fn now_ms() -> f64 {
  glib::real_time() as f64 / 1000.0
}

fn reset_ms(at: &str) -> f64 {
  glib::DateTime::from_iso8601(at, None).map(|d| d.to_unix() as f64 * 1000.0 - now_ms()).unwrap_or(-1.0)
}

pub fn build(ctx: &Rc<Ctx>, entry: &Value) -> gtk::Widget {
  let space = { let s = ctx.style.clone(); move |px: f64| s.space(px) };
  // [font] display: the hero mark's size, as in Panel.qml.
  let display = (ctx.style.font_base * 2.0).round() as i32;
  let entry = Rc::new(entry.clone());
  let interval = setting(&entry, "refreshIntervalSec").map(num).unwrap_or(900.0).max(30.0) as u64;
  let button = Button::new(ctx, Kind::Icon, "\u{f16a3}");
  let slot = Slot::new(ctx, &button.widget);
  let panel = Panel::new(ctx, &button.widget, &slot, space(380.0));
  let providers: Rc<RefCell<Vec<Value>>> = Rc::default();
  let selected = Rc::new(RefCell::new(String::new()));
  let body = ui::vbox(space(12.0));
  let scroll = gtk::ScrolledWindow::new();
  scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
  scroll.set_propagate_natural_height(true);
  scroll.set_max_content_height(space(700.0));
  scroll.set_child(Some(&body));
  panel.content.append(&scroll);
  let surface_light = {
    let f = doxia_theme::surface_card("popups").fill;
    0.2126 * f[0] + 0.7152 * f[1] + 0.0722 * f[2] >= 0.5
  };

  // Updates: one at a time; the next kind asked for waits.
  let running = Rc::new(Cell::new(false));
  let queued = Rc::new(RefCell::new(None::<(String, Vec<String>)>));
  let update: Rc<RefCell<std::rc::Weak<dyn Fn(&str, Vec<String>)>>> =
    Rc::new(RefCell::new(std::rc::Weak::<Box<dyn Fn(&str, Vec<String>)>>::new() as std::rc::Weak<dyn Fn(&str, Vec<String>)>));
  let run_update: Rc<dyn Fn(&str, Vec<String>)> = {
    let (entry, running, queued, update) = (entry.clone(), running.clone(), queued.clone(), update.clone());
    Rc::new(move |kind: &str, ids: Vec<String>| {
      if running.replace(true) {
        let mut q = queued.borrow_mut();
        if kind == "force" || q.is_none() {
          *q = Some((kind.to_string(), ids));
        }
        return;
      }
      let cmd = update_command(&entry, kind, &ids);
      let (running, queued, update) = (running.clone(), queued.clone(), update.clone());
      glib::spawn_future_local(async move {
        let args: Vec<&str> = cmd.iter().map(String::as_str).collect();
        let _ = crate::watch::output(&args).await;
        running.set(false);
        let next = queued.borrow_mut().take();
        if let (Some((k, ids)), Some(f)) = (next, update.borrow().upgrade()) {
          f(&k, ids);
        }
      });
    })
  };
  *update.borrow_mut() = Rc::downgrade(&run_update);

  let render: Rc<dyn Fn()> = {
    let (providers, selected, body, b, panel_w) = (providers.clone(), selected.clone(), body.clone(), button.clone(), Rc::downgrade(&panel));
    let (run_update, space) = (run_update.clone(), space.clone());
    let again: Rc<RefCell<std::rc::Weak<dyn Fn()>>> = Rc::new(RefCell::new(std::rc::Weak::<Box<dyn Fn()>>::new() as std::rc::Weak<dyn Fn()>));
    let again2 = again.clone();
    let f: Rc<dyn Fn()> = Rc::new(move || {
      let list = providers.borrow().clone();
      b.widget.set_visible(!list.is_empty());
      let index = list.iter().position(|p| p["id"].as_str() == Some(selected.borrow().as_str())).unwrap_or(0);
      let p = list.get(index).cloned();
      b.set_active(p.as_ref().is_some_and(alarming));
      let Some(panel) = panel_w.upgrade() else { return };
      if !panel.is_open() {
        return;
      }
      crate::popup::clear(&body);
      let Some(p) = p else {
        let t = ui::label("No AI coding subscriptions found.\nAgents show up here once you've used them.", "dim");
        t.set_justify(gtk::Justification::Center);
        t.set_margin_top(space(24.0));
        body.append(&t);
        panel.relayout();
        return;
      };
      // Hero: the provider's mark (a light variant on light cards).
      let hero = ui::hbox(space(14.0));
      let id = p["id"].as_str().unwrap_or("").to_string();
      let assets = format!("{}/shell/plugins/agents/assets", std::env::var("OMARCHY_PATH").unwrap_or_default());
      let mut candidates = Vec::new();
      if surface_light {
        candidates.push(format!("{assets}/{id}-light.svg"));
      }
      candidates.push(format!("{assets}/{id}.svg"));
      let mark = candidates.iter().find(|c| std::path::Path::new(c).exists()).and_then(|c| gdk::Texture::from_filename(c).ok());
      match mark {
        Some(t) => {
          // An Image holds the size; a Picture would take the SVG's own.
          let pic = gtk::Image::from_paintable(Some(&t));
          pic.set_pixel_size(display);
          pic.set_valign(gtk::Align::Center);
          hero.append(&pic);
        }
        None => hero.append(&ui::label("\u{f16a3}", "panel-hero-icon")),
      }
      let labels = ui::vbox(space(2.0));
      labels.set_valign(gtk::Align::Center);
      let t = ui::label(p["name"].as_str().unwrap_or(&id), "panel-hero-title");
      t.set_xalign(0.0);
      labels.append(&t);
      let status = p["usageStatusText"].as_str().unwrap_or("");
      let tier = p["tierLabel"].as_str().unwrap_or("");
      let meta = if !status.is_empty() {
        status.to_string()
      } else if tier.is_empty() {
        "Subscription".into()
      } else {
        let mut c = tier.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
      };
      let m = ui::label(&meta.to_uppercase(), "panel-hero-subtitle");
      m.set_xalign(0.0);
      labels.append(&m);
      hero.append(&labels);
      body.append(&hero);
      if list.len() > 1 {
        let switch = ui::hbox(space(6.0));
        switch.set_homogeneous(true);
        for (i, q) in list.iter().enumerate() {
          let (sel, qid, a) = (selected.clone(), q["id"].as_str().unwrap_or("").to_string(), again2.clone());
          let btn = ui::button(q["name"].as_str().unwrap_or(""), false, move || {
            *sel.borrow_mut() = qid.clone();
            if let Some(f) = a.borrow().upgrade() {
              f();
            }
          });
          btn.add_css_class("scale");
          if i == index {
            btn.add_css_class("selected");
          }
          switch.append(&btn);
        }
        body.append(&switch);
      }
      if !status.is_empty() {
        let help = ui::text(p["authHelpText"].as_str().unwrap_or(""));
        help.add_css_class("agents-help");
        body.append(&help);
      }
      let text = doxia_theme::controls().text;
      let meter = |value: f64, alarm: bool, alpha: f64| {
        let rail = ui::Rail::new(space(4.0).max(4), if alarm { doxia_theme::palette().red } else { [text[0], text[1], text[2], alpha] });
        rail.widget.add_css_class(if alarm { "agents-meter alarm" } else { "agents-meter" });
        rail.set(value.clamp(0.0, 1.0));
        rail
      };
      let limit_list = limits(&p);
      let b_v = &p["balance"];
      let has_balance = b_v["remaining"].as_f64().is_some_and(|r| r >= 0.0);
      if has_balance || !limit_list.is_empty() {
        body.append(&ui::separator());
      }
      let money = |v: f64, cur: &str| {
        let prefix = match cur.to_uppercase().as_str() {
          "USD" | "" => "$".to_string(),
          "EUR" => "€".into(),
          "GBP" => "£".into(),
          c => format!("{c} "),
        };
        format!("{prefix}{v:.2}")
      };
      if has_balance {
        let col = ui::vbox(space(10.0));
        col.append(&ui::section("Balance"));
        let cur = b_v["currency"].as_str().unwrap_or("USD");
        let (remaining, funded) = (num(&b_v["remaining"]), num(&b_v["funded"]));
        let alarm = funded > 0.0 && remaining / funded <= 0.1;
        let row = gtk::CenterBox::new();
        row.set_start_widget(Some(&ui::label("Prepaid credits", "agents-row")));
        row.set_end_widget(Some(&ui::label(&money(remaining, cur), if alarm { "agents-value alarm" } else { "agents-value" })));
        col.append(&row);
        if funded > 0.0 {
          col.append(&meter(remaining / funded, alarm, 1.0).widget);
          let mut d = format!("{} spent of {} funded", money(num(&b_v["spent"]), cur), money(funded, cur));
          if b_v["estimated"].as_bool() == Some(true) {
            d.push_str(" · estimated");
          }
          let dl = ui::label(&d, "agents-caption");
          dl.set_xalign(0.0);
          col.append(&dl);
        }
        body.append(&col);
      }
      if !limit_list.is_empty() {
        let col = ui::vbox(space(10.0));
        col.append(&ui::section("Limits"));
        for (title, percent, reset) in limit_list {
          let alarm = percent >= 0.9;
          let r = ui::vbox(space(6.0));
          let head = gtk::CenterBox::new();
          head.set_start_widget(Some(&ui::label(&title, "agents-row")));
          head.set_end_widget(Some(&ui::label(&format!("{}%", (percent * 100.0).round()), if alarm { "agents-value alarm" } else { "agents-value" })));
          r.append(&head);
          r.append(&meter(percent, alarm, 1.0).widget);
          let ms = reset_ms(&reset);
          if ms > 0.0 {
            let l = ui::label(&format!("Resets in {}", duration(ms)), "agents-caption");
            l.set_xalign(0.0);
            r.append(&l);
          }
          col.append(&r);
        }
        body.append(&col);
      }
      // Tokens by day.
      let days = p["recentDays"].as_array().cloned().unwrap_or_default();
      if !days.is_empty() {
        body.append(&ui::separator());
        let col = ui::vbox(space(6.0));
        col.append(&ui::section("Tokens by day"));
        let peak = days.iter().map(|d| num(&d["messageCount"])).fold(1.0, f64::max);
        let today = glib::DateTime::now_local().ok().and_then(|d| d.format("%Y-%m-%d").ok()).map(|s| s.to_string()).unwrap_or_default();
        const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        for d in &days {
          let date = d["date"].as_str().unwrap_or("");
          let is_today = date == today;
          let count = num(&d["messageCount"]);
          let parts: Vec<i32> = date.split('-').filter_map(|x| x.parse().ok()).collect();
          let dt = (parts.len() == 3).then(|| glib::DateTime::from_local(parts[0], parts[1], parts[2], 12, 0, 0.0).ok()).flatten();
          let name = if is_today { "Today".to_string() } else { dt.as_ref().map(|d| DAYS[(d.day_of_week() % 7) as usize].to_string()).unwrap_or_else(|| date.to_string()) };
          let r = ui::hbox(space(8.0));
          let l = ui::label(&name, if is_today { "agents-day today" } else { "agents-day" });
          l.set_size_request(space(52.0), -1);
          l.set_xalign(0.0);
          r.append(&l);
          let rail = meter(count / peak, false, if is_today { 1.0 } else { 0.55 });
          rail.widget.set_valign(gtk::Align::Center);
          r.append(&rail.widget);
          let v = ui::label(&token_count(count), if is_today { "agents-day-value today" } else { "agents-day-value" });
          v.set_size_request(space(52.0), -1);
          v.set_xalign(1.0);
          r.append(&v);
          let mut tip = format!("{} · {} tokens", dt.as_ref().map(|d| format!("{} {}/{}", DAYS[(d.day_of_week() % 7) as usize], d.month(), d.day_of_month())).unwrap_or_else(|| date.to_string()), token_count(count));
          if is_today && p["hasPromptStats"].as_bool() != Some(false) {
            tip.push_str(&format!(" · {} prompts · {} sessions", num(&p["todayPrompts"]), num(&p["todaySessions"])));
          }
          r.set_tooltip_text(Some(&tip));
          col.append(&r);
        }
        body.append(&col);
      }
      // Tokens by model, the top four.
      let mut models: Vec<(String, f64, [f64; 4])> = p["modelUsage"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(id, b)| {
          let parts = [num(&b["inputTokens"]), num(&b["outputTokens"]), num(&b["cacheReadInputTokens"]), num(&b["cacheCreationInputTokens"])];
          (model_name(id), parts.iter().sum(), parts)
        })
        .collect();
      models.sort_by(|a, b| b.1.total_cmp(&a.1));
      models.truncate(4);
      if !models.is_empty() {
        body.append(&ui::separator());
        let col = ui::vbox(space(6.0));
        col.append(&ui::section("Tokens by model"));
        let top = models[0].1.max(1.0);
        for (name, total, parts) in models {
          let row = gtk::Overlay::new();
          row.add_css_class("agents-model");
          let bar = gtk::DrawingArea::new();
          let share = total / top;
          bar.set_draw_func(move |w, cr, width, height| {
            let fg = w.color();
            super::super::widget::rounded(cr, 0.0, 0.0, width as f64 * share.clamp(0.0, 1.0), height as f64, 4.0);
            cr.set_source_rgba(fg.red() as f64, fg.green() as f64, fg.blue() as f64, 0.14);
            let _ = cr.fill();
          });
          row.set_child(Some(&bar));
          let line = gtk::CenterBox::new();
          line.set_margin_start(space(8.0));
          line.set_margin_end(space(8.0));
          line.set_margin_top(space(4.0));
          line.set_margin_bottom(space(4.0));
          let n = ui::label(&name, "agents-model-name");
          n.set_ellipsize(pango::EllipsizeMode::End);
          line.set_start_widget(Some(&n));
          line.set_end_widget(Some(&ui::label(&token_count(total), "agents-model-value")));
          row.add_overlay(&line);
          row.set_measure_overlay(&line, true);
          row.set_tooltip_text(Some(&format!(
            "In {} · out {} · cache read {} · cache write {}",
            token_count(parts[0]),
            token_count(parts[1]),
            token_count(parts[2]),
            token_count(parts[3])
          )));
          col.append(&row);
        }
        body.append(&col);
      }
      let _ = &run_update;
      panel.relayout();
    });
    *again.borrow_mut() = Rc::downgrade(&f);
    let keep = again.clone();
    button.widget.connect_destroy(move |_| {
      let _ = &keep;
    });
    f
  };

  // Records: read again when the directory changes; a record advising a
  // retry asks for limits in 30 s.
  let reload: Rc<dyn Fn()> = {
    let (entry, providers, render, run_update) = (entry.clone(), providers.clone(), render.clone(), run_update.clone());
    let retry_pending = Rc::new(Cell::new(false));
    Rc::new(move || {
      let list = read_providers(&entry);
      let retry: Vec<String> = list.iter().filter(|p| p["retryAdvised"].as_bool() == Some(true)).filter_map(|p| p["id"].as_str().map(String::from)).collect();
      *providers.borrow_mut() = list;
      render();
      if !retry.is_empty() && !retry_pending.replace(true) {
        let (ru, rp) = (run_update.clone(), retry_pending.clone());
        glib::timeout_add_local_once(Duration::from_secs(30), move || {
          rp.set(false);
          ru("limits", retry);
        });
      }
    })
  };
  reload();
  let _ = std::fs::create_dir_all(usage_dir());
  if let Ok(mon) = gio::File::for_path(usage_dir()).monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
    let r = reload.clone();
    mon.connect_changed(move |_, _, _, ev| {
      if matches!(ev, gio::FileMonitorEvent::ChangesDoneHint | gio::FileMonitorEvent::Deleted | gio::FileMonitorEvent::Created | gio::FileMonitorEvent::MovedIn | gio::FileMonitorEvent::Renamed) {
        r();
      }
    });
    button.widget.connect_destroy(move |_| {
      let _ = &mon;
    });
  }
  if ctx.first_screen {
    run_update("normal", Vec::new());
    let (ru, w) = (run_update.clone(), button.widget.downgrade());
    glib::timeout_add_seconds_local(interval as u32, move || {
      if w.upgrade().is_none() {
        return glib::ControlFlow::Break;
      }
      ru("normal", Vec::new());
      glib::ControlFlow::Continue
    });
  }
  let next: Rc<dyn Fn()> = {
    let (providers, selected, render) = (providers.clone(), selected.clone(), render.clone());
    Rc::new(move || {
      let list = providers.borrow();
      if list.is_empty() {
        return;
      }
      let i = list.iter().position(|p| p["id"].as_str() == Some(selected.borrow().as_str())).unwrap_or(0);
      *selected.borrow_mut() = list[(i + 1) % list.len()]["id"].as_str().unwrap_or("").to_string();
      drop(list);
      render();
    })
  };
  {
    let (ru, r) = (run_update.clone(), render.clone());
    panel.connect_open(move || {
      r();
      ru("limits", Vec::new());
    });
    let (ru, providers2, selected2, r) = (run_update.clone(), providers.clone(), selected.clone(), render.clone());
    panel.connect_key(move |k, _| {
      match k {
        gdk::Key::r | gdk::Key::R | gdk::Key::Return | gdk::Key::KP_Enter => ru("force", Vec::new()),
        gdk::Key::Left | gdk::Key::Right | gdk::Key::h | gdk::Key::l => {
          let list = providers2.borrow().clone();
          if list.is_empty() {
            return true;
          }
          let i = list.iter().position(|p| p["id"].as_str() == Some(selected2.borrow().as_str())).unwrap_or(0) as i64;
          let d = if matches!(k, gdk::Key::Left | gdk::Key::h) { -1 } else { 1 };
          let j = (i + d).rem_euclid(list.len() as i64) as usize;
          *selected2.borrow_mut() = list[j]["id"].as_str().unwrap_or("").to_string();
          r();
        }
        _ => return false,
      }
      true
    });
  }
  let (p, nx, pw) = (panel.clone(), next.clone(), Rc::downgrade(&panel));
  button.on_press(move |b| match b {
    3 => {
      crate::run::shell("omarchy-agent --pick");
      if let Some(p) = pw.upgrade() {
        p.close();
      }
    }
    2 => nx(),
    _ => p.toggle(),
  });
  if ctx.first_screen {
    if let Some(bar) = ctx.bar.upgrade() {
      let (p, ru, nx) = (Rc::downgrade(&panel), run_update.clone(), next.clone());
      bar.add_ipc("omarchy.agents", Box::new(move |method, _| {
        let panel = p.upgrade()?;
        match method {
          "open" | "show" => panel.open(),
          "close" | "hide" => panel.close(),
          "toggle" => panel.toggle(),
          "refresh" => ru("force", Vec::new()),
          "next" => nx(),
          _ => return None,
        }
        Some("ok".into())
      }));
    }
  }
  slot.widget.clone().upcast()
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn names_counts_and_windows() {
    assert_eq!(model_name("claude-opus-4-1-20250805"), "Opus 4.1");
    assert_eq!(model_name("gpt-5-codex"), "GPT 5 Codex");
    assert_eq!(token_count(597831149.0), "597.8M");
    assert_eq!(token_count(950.0), "950");
    assert_eq!(window_title("Session (5-hour)"), "Session");
    assert_eq!(window_title("Weekly (7-day)"), "Weekly");
    assert_eq!(duration(3_700_000.0), "1h 1m");
    assert!(alarming(&json!({"limits": [{"percent": 0.95}]})));
    assert!(!has_data(&json!({"limits": [], "totalPrompts": 0})));
    let e = json!({"providers": {"codex": {"enabled": false}}});
    assert_eq!(update_command(&e, "limits", &[]), ["omarchy-agent-usage-update", "--limits-only", "--except", "codex"]);
  }
}
