// omarchy.tray (shell/plugins/bar/widgets/Tray.qml): the tray's icons. The
// pinned ones always show; the rest sit in a drawer that slides open while
// the pointer is over its chevron. A click activates an item (its menu, for
// menu-only items), a middle click is its secondary action, a right click
// its menu, drawn here with its submenus drilled into in place; the wheel
// scrolls it. A right click on the chevron opens the list where icons are
// pinned or hidden (the entry's "pinned" and "hidden" lists in shell.json).
// LocalSend's icon never shows (Share > Receive covers it).

use super::super::config::{self, setting};
use super::super::panel::Panel;
use super::super::ui;
use super::super::widget::{self, Button, Kind, Slot};
use super::super::Ctx;
use crate::sni::{self, Item, MenuEntry};
use gtk::prelude::*;
use gtk::{glib, pango};
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

fn ids(entry: &Value, key: &str) -> Vec<String> {
  setting(entry, key).and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default()
}

fn named(item: &Item, name: &str) -> bool {
  [&item.id, &item.title, &item.tooltip].iter().any(|s| s.to_lowercase().contains(name))
}

pub fn build(ctx: &Rc<Ctx>, entry: &Value) -> gtk::Widget {
  let vertical = ctx.vertical();
  let orientation = if vertical { gtk::Orientation::Vertical } else { gtk::Orientation::Horizontal };
  let slot_size = ctx.style.icon_slot;
  let space = { let s = ctx.style.clone(); move |px: f64| s.space(px) };
  let entry = Rc::new(RefCell::new(entry.clone()));
  let tray = sni::get();

  let root = gtk::Box::new(orientation, 0);
  root.add_css_class("bar-tray");
  // The drawer: the chevron, then the clipped strip of icons.
  let drawer = gtk::Box::new(orientation, 0);
  // Pointing towards the bar's center: left, or up on a side bar.
  let chevron = Button::new(ctx, Kind::Icon, if vertical { "\u{f077}" } else { "\u{f053}" });
  let clip = gtk::ScrolledWindow::new();
  clip.set_policy(gtk::PolicyType::External, gtk::PolicyType::External);
  clip.set_propagate_natural_height(true);
  let strip = gtk::Box::new(orientation, 0);
  clip.set_child(Some(&strip));
  drawer.append(&chevron.widget);
  drawer.append(&clip);
  let pinned = gtk::Box::new(orientation, 0);
  root.append(&drawer);
  root.append(&pinned);
  let slot = Slot::new(ctx, &root);

  // ---- The drawer's reveal: 600 ms, out cubic.
  let reveal = Rc::new(Cell::new(0.0f64));
  let target = Rc::new(Cell::new(0.0f64));
  let extent = Rc::new(Cell::new(0));
  let apply = {
    let (clip, reveal, extent) = (clip.clone(), reveal.clone(), extent.clone());
    move || {
      let shown = (extent.get() as f64 * reveal.get()).round() as i32;
      if vertical {
        clip.set_size_request(-1, shown);
        clip.set_max_content_height(shown.max(0));
      } else {
        clip.set_size_request(shown, -1);
        clip.set_max_content_width(shown.max(0));
      }
      clip.set_visible(shown > 0);
      // Slid in from the bar's center side: the end of the strip shows first.
      let adj = if vertical { clip.vadjustment() } else { clip.hadjustment() };
      adj.set_value(adj.upper() - adj.page_size());
    }
  };
  let animate = {
    let (reveal, target, apply) = (reveal.clone(), target.clone(), apply.clone());
    let running = Rc::new(Cell::new(false));
    move |to: f64| {
      target.set(to);
      if running.replace(true) {
        return;
      }
      let (reveal, target, apply, running) = (reveal.clone(), target.clone(), apply.clone(), running.clone());
      let (start, from) = (Instant::now(), reveal.get());
      let from_cell = Rc::new(Cell::new(from));
      let started = Rc::new(Cell::new(start));
      let last_target = Rc::new(Cell::new(to));
      glib::timeout_add_local(Duration::from_millis(16), move || {
        if last_target.get() != target.get() {
          last_target.set(target.get());
          from_cell.set(reveal.get());
          started.set(Instant::now());
        }
        let t = (started.get().elapsed().as_secs_f64() / 0.6).min(1.0);
        let eased = 1.0 - (1.0 - t).powi(3);
        reveal.set(from_cell.get() + (target.get() - from_cell.get()) * eased);
        apply();
        if t >= 1.0 {
          running.set(false);
          glib::ControlFlow::Break
        } else {
          glib::ControlFlow::Continue
        }
      });
    }
  };
  let hover = gtk::EventControllerMotion::new();
  let a = animate.clone();
  hover.connect_enter(move |_, _, _| a(1.0));
  let a = animate.clone();
  hover.connect_leave(move |_| a(0.0));
  drawer.add_controller(hover);

  // ---- The menu of an item, with its submenus.
  let menu_panel = Panel::new(ctx, &root, &slot, space(232.0));
  menu_panel.content.add_css_class("tray-menu");
  let menu_box = ui::vbox(0);
  let menu_scroll = gtk::ScrolledWindow::new();
  menu_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
  menu_scroll.set_propagate_natural_height(true);
  menu_scroll.set_max_content_height(space(420.0));
  menu_scroll.set_child(Some(&menu_box));
  menu_panel.content.append(&menu_scroll);
  let menu_item: Rc<RefCell<Option<Item>>> = Rc::default();
  let menu_stack: Rc<RefCell<Vec<(String, MenuEntry)>>> = Rc::default();
  let render_menu: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::default();
  {
    let (menu_box, menu_item, menu_stack, rm, panel_w) = (menu_box.clone(), menu_item.clone(), menu_stack.clone(), Rc::downgrade(&render_menu), Rc::downgrade(&menu_panel));
    let tray = tray.clone();
    let space = space.clone();
    let f: Rc<dyn Fn()> = Rc::new(move || {
      crate::popup::clear(&menu_box);
      let stack = menu_stack.borrow().clone();
      let Some((_, level)) = stack.last().cloned() else { return };
      let at_root = stack.len() == 1;
      let item = menu_item.borrow().clone();
      if !at_root {
        let back = ui::hbox(0);
        back.append(&ui::label("\u{2039}", "tray-menu-mark"));
        let t = ui::label(&stack.last().unwrap().0, "tray-menu-text");
        t.set_xalign(0.0);
        t.set_ellipsize(pango::EllipsizeMode::End);
        back.append(&t);
        let (ms, rm) = (menu_stack.clone(), rm.clone());
        let row = ui::row(&back, move |_| {
          ms.borrow_mut().pop();
          if let Some(f) = rm.upgrade().and_then(|r| r.borrow().clone()) {
            f();
          }
        });
        row.add_css_class("tray-menu-row");
        menu_box.append(&row);
        menu_box.append(&rule(&space));
      }
      let title = item.as_ref().map(|i| if i.title.is_empty() { i.id.clone() } else { i.title.clone() }).unwrap_or_default();
      for (i, e) in level.children.iter().enumerate() {
        // The app's own name as the first submenu, and a leading rule, are
        // dropped at the top.
        if at_root && ((i == 0 && !e.children.is_empty() && e.label.to_lowercase() == title.to_lowercase()) || (e.separator && i <= 1)) {
          continue;
        }
        if e.separator {
          menu_box.append(&rule(&space));
          continue;
        }
        let r = ui::hbox(0);
        r.append(&ui::label(if e.toggle && e.checked { "\u{f00c}" } else { "" }, "tray-menu-mark"));
        let t = ui::label(&e.label, "tray-menu-text");
        t.set_xalign(0.0);
        t.set_hexpand(true);
        t.set_ellipsize(pango::EllipsizeMode::End);
        r.append(&t);
        if !e.children.is_empty() {
          r.append(&ui::label("\u{203a}", "tray-menu-sub"));
        }
        let (e2, ms, rm, tray, item, panel_w) = (e.clone(), menu_stack.clone(), rm.clone(), tray.clone(), item.clone(), panel_w.clone());
        let row = ui::row(&r, move |_| {
          if !e2.enabled {
            return;
          }
          if !e2.children.is_empty() {
            ms.borrow_mut().push((e2.label.clone(), e2.clone()));
            if let (Some(item), Some(tray)) = (item.clone(), Some(tray.clone())) {
              let id = e2.id;
              glib::spawn_future_local(async move { tray.about_to_show(&item, id).await });
            }
            if let Some(f) = rm.upgrade().and_then(|r| r.borrow().clone()) {
              f();
            }
          } else {
            if let Some(item) = &item {
              tray.menu_click(item, e2.id);
            }
            if let Some(p) = panel_w.upgrade() {
              p.close();
            }
          }
        });
        row.add_css_class("tray-menu-row");
        if !e.enabled {
          row.set_opacity(0.45);
        }
        menu_box.append(&row);
      }
      if let Some(p) = panel_w.upgrade() {
        p.relayout();
      }
    });
    *render_menu.borrow_mut() = Some(f);
  }
  let open_menu: Rc<dyn Fn(Item)> = {
    let (tray, menu_item, menu_stack, rm, panel) = (tray.clone(), menu_item.clone(), menu_stack.clone(), render_menu.clone(), menu_panel.clone());
    Rc::new(move |item: Item| {
      if item.menu.is_empty() {
        tray.context_menu(&item);
        return;
      }
      let (tray, menu_item, menu_stack, rm, panel) = (tray.clone(), menu_item.clone(), menu_stack.clone(), rm.clone(), panel.clone());
      glib::spawn_future_local(async move {
        let Some(root) = tray.menu(&item).await else { return };
        *menu_item.borrow_mut() = Some(item);
        *menu_stack.borrow_mut() = vec![(String::new(), root)];
        if let Some(f) = rm.borrow().clone() {
          f();
        }
        panel.open();
      });
    })
  };

  // ---- Managing: pin or hide each icon.
  let manage = Panel::new(ctx, &chevron.widget, &slot, space(300.0));
  let manage_box = ui::vbox(space(8.0));
  manage.content.append(&manage_box);
  let render_manage: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::default();

  // ---- Painting the icons.
  let paint: Rc<dyn Fn()> = {
    let (strip, pinned_box, root, entry, extent, apply, tray, open_menu) =
      (strip.clone(), pinned.clone(), root.clone(), entry.clone(), extent.clone(), apply.clone(), tray.clone(), open_menu.clone());
    let (drawer, rmg) = (drawer.clone(), Rc::downgrade(&render_manage));
    let size = space(12.0);
    let scale = ctx.window.scale_factor();
    Rc::new(move || {
      let e = entry.borrow().clone();
      let (pins, hidden) = (ids(&e, "pinned"), ids(&e, "hidden"));
      let items: Vec<Item> = tray.items().into_iter().filter(|i| i.status != "Passive" && !named(i, "localsend")).collect();
      crate::popup::clear(&strip);
      crate::popup::clear(&pinned_box);
      let (mut drawer_count, mut pinned_count) = (0, 0);
      for item in &items {
        if hidden.contains(&item.id) {
          continue;
        }
        let w = tray_icon(item, size, scale, slot_size, vertical);
        let (t, it, om) = (tray.clone(), item.clone(), open_menu.clone());
        widget::on_press(&w, move |b| match b {
          3 => om(it.clone()),
          2 => t.secondary_activate(&it),
          _ if it.only_menu => om(it.clone()),
          _ => t.activate(&it),
        });
        let (t, it) = (tray.clone(), item.clone());
        widget::on_wheel(&w, move |_, dy| t.scroll(&it, if dy < 0.0 { 120 } else { -120 }));
        if pins.contains(&item.id) {
          pinned_box.append(&w);
          pinned_count += 1;
        } else {
          strip.append(&w);
          drawer_count += 1;
        }
      }
      extent.set(drawer_count * slot_size);
      drawer.set_visible(!items.is_empty());
      root.set_visible(pinned_count > 0 || drawer_count > 0);
      apply();
      if let Some(f) = rmg.upgrade().and_then(|r| r.borrow().clone()) {
        f();
      }
    })
  };
  {
    let (manage_box, entry, tray, p) = (manage_box.clone(), entry.clone(), tray.clone(), Rc::downgrade(&paint));
    let manage_w = Rc::downgrade(&manage);
    let space = space.clone();
    let scale = ctx.window.scale_factor();
    let f: Rc<dyn Fn()> = Rc::new(move || {
      crate::popup::clear(&manage_box);
      let title = ui::label("Tray icons", "tray-manage-title");
      title.set_xalign(0.0);
      manage_box.append(&title);
      let hint = ui::text("Pinned icons stay visible. Hidden icons never show.");
      hint.add_css_class("dim");
      manage_box.append(&hint);
      let items: Vec<Item> = tray.items().into_iter().filter(|i| i.status != "Passive" && !named(i, "localsend")).collect();
      if items.is_empty() {
        manage_box.append(&ui::label("No tray items reporting.", "tray-manage-empty"));
      }
      let e = entry.borrow().clone();
      let (pins, hidden) = (ids(&e, "pinned"), ids(&e, "hidden"));
      for item in items {
        let row = ui::hbox(space(10.0));
        let icon = gtk::Picture::new();
        icon.set_size_request(16, 16);
        icon.set_paintable(sni::icon_paintable(&item.icon, 16, scale).as_ref());
        row.append(&icon);
        let name = ui::label(&item.display_name(), "tray-manage-name");
        name.set_hexpand(true);
        name.set_xalign(0.0);
        name.set_ellipsize(pango::EllipsizeMode::End);
        row.append(&name);
        let toggle_list = |key: &'static str, other: &'static str| {
          let (entry, id, p) = (entry.clone(), item.id.clone(), p.clone());
          move || {
            let mut e = entry.borrow_mut();
            if !e.is_object() {
              *e = serde_json::json!({ "id": config::entry_id(&e) });
            }
            let mut list = ids(&e, key);
            let mut rest = ids(&e, other);
            if let Some(i) = list.iter().position(|x| *x == id) {
              list.remove(i);
            } else {
              list.push(id.clone());
              rest.retain(|x| *x != id);
            }
            e[key] = list.into();
            e[other] = rest.into();
            let written = e.clone();
            drop(e);
            config::update_entry(&written);
            if let Some(f) = p.upgrade() {
              f();
            }
          }
        };
        let hide = ui::button(&format!("\u{f06e} {}", if hidden.contains(&item.id) { "Show" } else { "Hide" }), false, toggle_list("hidden", "pinned"));
        let pin = ui::button(&format!("\u{f08d} {}", if pins.contains(&item.id) { "Unpin" } else { "Pin" }), false, toggle_list("pinned", "hidden"));
        hide.add_css_class("small");
        pin.add_css_class("small");
        row.append(&hide);
        row.append(&pin);
        manage_box.append(&row);
      }
      if let Some(m) = manage_w.upgrade() {
        m.relayout();
      }
    });
    *render_manage.borrow_mut() = Some(f);
  }
  let keep = (render_menu.clone(), render_manage.clone(), paint.clone());
  root.connect_destroy(move |_| {
    let _ = &keep;
  });
  paint();
  let (p, w) = (paint.clone(), root.downgrade());
  tray.connect(move || {
    if w.upgrade().is_some() {
      p();
    }
  });
  let m = manage.clone();
  chevron.on_press(move |b| {
    if b == 3 {
      m.toggle();
    }
  });
  slot.widget.clone().upcast()
}

fn rule(space: &impl Fn(f64) -> i32) -> gtk::Box {
  let b = ui::separator();
  b.add_css_class("tray-menu-rule");
  b.set_margin_top(space(5.0));
  b.set_margin_bottom(space(5.0));
  b.set_margin_start(space(10.0));
  b.set_margin_end(space(10.0));
  b
}

fn tray_icon(item: &Item, size: i32, scale: i32, slot: i32, vertical: bool) -> gtk::Box {
  let holder = gtk::Box::new(gtk::Orientation::Horizontal, 0);
  holder.add_css_class("tray-item");
  holder.set_size_request(slot, slot);
  holder.set_cursor_from_name(Some("pointer"));
  holder.set_tooltip_text(Some(&item.tooltip_text()));
  let pic = gtk::Picture::new();
  pic.set_size_request(size, size);
  pic.set_can_shrink(true);
  pic.set_halign(gtk::Align::Center);
  pic.set_valign(gtk::Align::Center);
  pic.set_hexpand(true);
  pic.set_paintable(sni::icon_paintable(&item.icon, size, scale).as_ref());
  if sni::symbolic(&item.icon) {
    pic.add_css_class("symbolic");
  }
  let _ = vertical;
  holder.append(&pic);
  holder
}
