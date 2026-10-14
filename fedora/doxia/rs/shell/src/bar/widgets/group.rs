// omarchy.group (shell/plugins/panels/group/Panel.qml): several panel
// widgets behind one icon. Their own icons are hidden; their panels open
// under the group's, with a tab strip on top (each tab the member's live
// glyph and title). The entry says which member's glyph the icon shows
// ("icon", or a glyph of its own), which takes the wheel ("wheel"), the
// horizontal wheel ("hwheel") and the right click ("rightClick"). Wheeling
// a member other than the icon's shows its glyph for five seconds.
//
//   {"id": "omarchy.group", "members": ["omarchy.power", "omarchy.monitor",
//    "omarchy.audio"], "icon": "omarchy.monitor", "wheel": "omarchy.monitor"}

use super::super::config::setting_str;
use super::super::member::Member;
use super::super::ui;
use super::super::widget::{self, Button, Kind, Slot};
use super::super::Ctx;
use gtk::glib;
use gtk::prelude::*;
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

const TITLES: [(&str, &str); 6] = [
  ("omarchy.audio", "Som"),
  ("omarchy.bluetooth", "Bluetooth"),
  ("omarchy.monitor", "Tela"),
  ("omarchy.network", "Wi-Fi"),
  ("omarchy.power", "Bateria"),
  ("omarchy.tailscale", "Tailscale"),
];

pub fn build(ctx: &Rc<Ctx>, entry: &Value, make: impl Fn(&str, &Value) -> Option<Member>) -> Option<gtk::Widget> {
  let raw = entry["members"].as_array().cloned().unwrap_or_default();
  let titles = entry["titles"].as_array().cloned().unwrap_or_default();
  let mut members: Vec<Rc<Member>> = Vec::new();
  for m in &raw {
    let id = match m {
      Value::String(s) => s.clone(),
      Value::Object(o) => o.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
      _ => continue,
    };
    let Some(default_title) = TITLES.iter().find(|(i, _)| *i == id).map(|(_, t)| *t) else { continue };
    let Some(mut member) = make(&id, m) else { continue };
    member.title = titles.get(members.len()).and_then(Value::as_str).unwrap_or(default_title).to_string();
    members.push(Rc::new(member));
  }
  if members.is_empty() {
    return None;
  }
  let icon_setting = setting_str(entry, "icon", "");
  let wheel_id = setting_str(entry, "wheel", "");
  let hwheel_id = setting_str(entry, "hwheel", "");
  let right_id = setting_str(entry, "rightClick", "");
  let by_id = {
    let members = members.clone();
    move |id: &str| members.iter().find(|m| m.id == id).cloned()
  };

  let button = Button::new(ctx, Kind::Icon, "");
  let slot = Slot::new(ctx, &button.widget);
  let last = Rc::new(Cell::new(0usize));
  let flash = Rc::new(RefCell::new(String::new()));
  let flash_gen = Rc::new(Cell::new(0u64));

  // The icon: the flashed member's glyph, the chosen one's, a glyph of the
  // entry's own, or the first member's.
  let paint: Rc<dyn Fn()> = {
    let (b, members, flash, icon_setting) = (button.clone(), members.clone(), flash.clone(), icon_setting.clone());
    Rc::new(move || {
      let shown = if flash.borrow().is_empty() { icon_setting.clone() } else { flash.borrow().clone() };
      let glyph = if !shown.is_empty() && !TITLES.iter().any(|(i, _)| *i == shown) {
        shown
      } else {
        let m = if shown.is_empty() { members.first().cloned() } else { members.iter().find(|m| m.id == shown).cloned() };
        m.map(|m| m.button.label.text().to_string()).unwrap_or_default()
      };
      b.set_text(&glyph);
    })
  };
  for m in &members {
    // The member's own icon is not on the bar, but still tracks its state:
    // its text changing repaints the group's.
    let p = paint.clone();
    m.button.label.connect_label_notify(move |_| p());
    m.panel.set_anchor(&button.widget);
    m.panel.set_slot(&slot);
  }
  paint();

  // Tabs over each member's panel.
  let select: Rc<dyn Fn(usize)> = {
    let (members, last) = (members.clone(), last.clone());
    Rc::new(move |i: usize| {
      if let Some(m) = members.get(i) {
        last.set(i);
        m.panel.open();
      }
    })
  };
  for (index, m) in members.iter().enumerate() {
    let (tabs, members_c, select_tab, last) = (m.panel.tabs.clone(), members.clone(), select.clone(), last.clone());
    let space = { let s = ctx.style.clone(); move |px: f64| s.space(px) };
    let render = move || {
      last.set(index);
      crate::popup::clear(&tabs);
      // Members with nothing to show (no battery) have no tab.
      let shown = members_c.iter().filter(|m| m.button.widget.is_visible()).count();
      tabs.set_visible(shown > 1);
      for (i, other) in members_c.iter().enumerate() {
        if !other.button.widget.is_visible() {
          continue;
        }
        let t = ui::hbox(space(6.0));
        t.set_halign(gtk::Align::Center);
        t.append(&ui::label(&other.button.label.text(), "panel-tab-glyph"));
        t.append(&ui::label(&other.title, if i == index { "panel-tab-title current" } else { "panel-tab-title" }));
        let holder = ui::hbox(0);
        holder.add_css_class("panel-tab");
        if i == index {
          holder.add_css_class("current");
        }
        t.set_hexpand(true);
        holder.append(&t);
        holder.set_cursor_from_name(Some("pointer"));
        let sel = select_tab.clone();
        widget::on_press(&holder, move |_| {
          if i != index {
            sel(i);
          }
        });
        tabs.append(&holder);
      }
    };
    m.panel.connect_open(render);
    // Tab and Shift+Tab walk the tabs that show.
    let (sel, members_t) = (select.clone(), members.clone());
    m.panel.connect_tab(move |back| {
      let n = members_t.len();
      let mut i = index;
      for _ in 0..n {
        i = if back { (i + n - 1) % n } else { (i + 1) % n };
        if members_t[i].button.widget.is_visible() {
          break;
        }
      }
      sel(i);
    });
  }

  // Clicks, the wheel and the right click.
  {
    let (members, last, by_id) = (members.clone(), last.clone(), by_id.clone());
    button.on_press(move |b| {
      if b == 3 {
        if let Some(f) = by_id(&right_id).and_then(|m| m.right.clone()) {
          f();
        }
        return;
      }
      let open = members.iter().find(|m| m.panel.is_open()).cloned();
      match open {
        Some(m) => m.panel.close(),
        None => {
          let i = last.get().min(members.len() - 1);
          let pick = if members[i].button.widget.is_visible() { Some(i) } else { members.iter().position(|m| m.button.widget.is_visible()) };
          if let Some(i) = pick {
            members[i].panel.open();
          }
        }
      }
    });
  }
  let flash_member = {
    let (flash, generation, paint, icon_setting) = (flash.clone(), flash_gen.clone(), paint.clone(), icon_setting.clone());
    move |id: &str| {
      if id == icon_setting {
        flash.borrow_mut().clear();
        paint();
        return;
      }
      *flash.borrow_mut() = id.to_string();
      paint();
      let g = generation.get() + 1;
      generation.set(g);
      let (flash, generation, paint) = (flash.clone(), generation.clone(), paint.clone());
      glib::timeout_add_local_once(Duration::from_secs(5), move || {
        if generation.get() == g {
          flash.borrow_mut().clear();
          paint();
        }
      });
    }
  };
  {
    let (by_id, flash_member) = (by_id.clone(), flash_member.clone());
    let (wheel_id, hwheel_id) = (wheel_id.clone(), hwheel_id.clone());
    button.on_wheel(move |dx, dy| {
      let horizontal = dx.abs() > dy.abs();
      let id = if horizontal && !hwheel_id.is_empty() { &hwheel_id } else { &wheel_id };
      let Some(m) = by_id(id) else { return };
      let Some(f) = m.wheel.clone() else { return };
      if horizontal && !hwheel_id.is_empty() {
        // Fingers to the right turn it up.
        f(0.0, -dx);
      } else if dy != 0.0 {
        f(0.0, dy);
      } else {
        return;
      }
      flash_member(id);
    });
  }
  // SUPER + wheel over the icon (mango binds it and asks here).
  if ctx.first_screen && !hwheel_id.is_empty() {
    if let Some(bar) = ctx.bar.upgrade() {
      let hovered = Rc::new(Cell::new(false));
      let motion = gtk::EventControllerMotion::new();
      let (h1, h2) = (hovered.clone(), hovered.clone());
      motion.connect_enter(move |_, _, _| h1.set(true));
      motion.connect_leave(move |_| h2.set(false));
      button.widget.add_controller(motion);
      let by_id = by_id.clone();
      bar.add_ipc("omarchy.group", Box::new(move |method, args| {
        if method != "superWheel" {
          return None;
        }
        if hovered.get() {
          if let Some(f) = by_id(&hwheel_id).and_then(|m| m.wheel.clone()) {
            f(0.0, if args.first().map(String::as_str) == Some("down") { 1.0 } else { -1.0 });
          }
        }
        Some("ok".into())
      }));
    }
  }
  // The members live with the group's icon.
  let keep = members.clone();
  button.widget.connect_destroy(move |_| {
    let _ = &keep;
  });
  Some(slot.widget.clone().upcast())
}
