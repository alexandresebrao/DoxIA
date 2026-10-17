// The notification bell (doxia.notifications, or the indicators' "Dnd"), like KDE's: a badge while
// notifications arrived unseen, the struck bell while they are silenced. A
// click lists the history (newest first; a click on one acts like its popup
// would, a right click or its × drops it), with do-not-disturb and "Limpar
// tudo" on top; a right click on the bell silences.

use super::super::panel::Panel;
use super::super::ui;
use super::super::widget::{Button, Kind, Slot};
use super::super::Ctx;
use crate::notify::model::Entry;
use gtk::glib;
use gtk::prelude::*;
use std::rc::Rc;

// "agora", "há 5 min", "14:32", "ontem 14:32", "12/10 14:32".
fn when(timestamp: i64) -> String {
  let Ok(then) = glib::DateTime::from_unix_local(timestamp / 1000) else { return String::new() };
  let Ok(now) = glib::DateTime::now_local() else { return String::new() };
  let minutes = (now.to_unix() - then.to_unix()) / 60;
  let clock = then.format("%H:%M").map(|s| s.to_string()).unwrap_or_default();
  let day = |d: &glib::DateTime| (d.year(), d.day_of_year());
  if minutes < 1 {
    "agora".into()
  } else if minutes < 60 {
    format!("há {minutes} min")
  } else if day(&then) == day(&now) {
    clock
  } else if now.add_days(-1).is_ok_and(|y| day(&y) == day(&then)) {
    format!("ontem {clock}")
  } else {
    format!("{} {clock}", then.format("%d/%m").map(|s| s.to_string()).unwrap_or_default())
  }
}

fn app_name(entry: &Entry) -> String {
  match entry.app.as_str() {
    "" | "omarchy-action" | "notify-send" => "DoxIA".into(),
    app => app.into(),
  }
}

// doxia.notifications: the bell on its own.
pub fn build(ctx: &Rc<Ctx>) -> Option<gtk::Widget> {
  let b = Button::new(ctx, Kind::Status, "\u{f009a}");
  let shown = b.clone();
  let slot = bell(ctx, &b, move |_, text, tip| {
    shown.set_text(text);
    shown.set_tooltip(tip);
  })?;
  Some(slot.widget.clone().upcast())
}

// The bell's slot, around the indicator button `b`.
pub fn bell(ctx: &Rc<Ctx>, b: &Rc<Button>, set: impl Fn(bool, &str, &str) + 'static) -> Option<Rc<Slot>> {
  let n = crate::notify::get()?;
  let space = { let s = ctx.style.clone(); move |px: f64| s.space(px) };
  let slot = Slot::new(ctx, &b.widget);
  let panel = Panel::new(ctx, &b.widget, &slot, space(400.0));

  let header = ui::hbox(space(8.0));
  let title = ui::section("Notificações");
  title.set_hexpand(true);
  header.append(&title);
  let quiet = ui::label("Não perturbe", "panel-text dim");
  header.append(&quiet);
  let dnd = {
    let n = n.clone();
    ui::toggle(n.dnd(), move |on| {
      if on != n.dnd() {
        n.set_dnd(on);
      }
    })
  };
  header.append(&dnd);
  let clear = {
    let n = n.clone();
    ui::button("Limpar tudo", false, move || n.clear_history())
  };
  let body = ui::vbox(space(12.0));
  body.append(&header);
  let list = ui::vbox(space(10.0));
  let scroll = gtk::ScrolledWindow::new();
  scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
  scroll.set_propagate_natural_height(true);
  scroll.set_max_content_height(space(600.0));
  scroll.set_child(Some(&list));
  body.append(&scroll);
  clear.set_halign(gtk::Align::End);
  body.append(&clear);
  panel.content.append(&body);

  let render: Rc<dyn Fn()> = {
    let (n, list, clear, dnd, panel_w) = (Rc::downgrade(&n), list.clone(), clear.clone(), dnd.clone(), Rc::downgrade(&panel));
    Rc::new(move || {
      let Some(n) = n.upgrade() else { return };
      let silenced = n.dnd();
      let unread = n.unread();
      let (glyph, tip) = if silenced {
        ("\u{f009b}", "Notificações silenciadas".to_string())
      } else if unread > 0 {
        ("\u{f116b}", if unread == 1 { "1 notificação nova".into() } else { format!("{unread} notificações novas") })
      } else {
        ("\u{f009a}", "Notificações".into())
      };
      set(true, glyph, &tip);
      if dnd.is_active() != silenced {
        dnd.set_active(silenced);
      }

      let open = panel_w.upgrade().is_some_and(|p| p.is_open());
      if !open {
        return;
      }
      while let Some(child) = list.first_child() {
        list.remove(&child);
      }
      let entries = n.history();
      clear.set_visible(!entries.is_empty());
      if entries.is_empty() {
        let empty = ui::label("Nenhuma notificação", "panel-text dim");
        empty.set_margin_top(12);
        empty.set_margin_bottom(12);
        list.append(&empty);
      }
      for entry in entries {
        list.append(&row(&n, &entry, &panel_w));
      }
    })
  };
  {
    let r = Rc::downgrade(&render);
    n.connect_changed(move || {
      if let Some(r) = r.upgrade() {
        r();
      }
    });
  }
  {
    let (r, n) = (render.clone(), Rc::downgrade(&n));
    panel.connect_open(move || {
      r();
      if let Some(n) = n.upgrade() {
        n.mark_seen();
      }
    });
  }
  // The history's age labels ("há 5 min") move on while it is open.
  {
    let (r, p) = (Rc::downgrade(&render), Rc::downgrade(&panel));
    glib::timeout_add_seconds_local(60, move || {
      let (Some(r), Some(p)) = (r.upgrade(), p.upgrade()) else { return glib::ControlFlow::Break };
      if p.is_open() {
        r();
      }
      glib::ControlFlow::Continue
    });
  }
  render();

  {
    let (p, n) = (panel.clone(), n.clone());
    b.on_press(move |button| {
      if button == 3 {
        n.set_dnd(!n.dnd());
      } else {
        p.toggle();
      }
    });
  }
  if ctx.first_screen {
    let p = Rc::downgrade(&panel);
    n.set_history_opener(move || {
      if let Some(p) = p.upgrade() {
        p.open();
      }
    });
  }
  let keep = (render, panel);
  slot.widget.connect_destroy(move |_| {
    let _ = &keep;
  });
  Some(slot)
}

fn row(n: &Rc<crate::notify::Notifications>, entry: &Entry, panel: &std::rc::Weak<Panel>) -> gtk::Box {
  let item = ui::vbox(4);
  let head = ui::hbox(8);
  let app = ui::label(&app_name(entry), "panel-section");
  app.set_xalign(0.0);
  app.set_hexpand(true);
  app.set_ellipsize(gtk::pango::EllipsizeMode::End);
  head.append(&app);
  head.append(&ui::label(&when(entry.timestamp), "panel-text dim"));
  {
    let (n, e) = (Rc::downgrade(n), entry.clone());
    head.append(&ui::action("\u{f0156}", "Remover", move || {
      if let Some(n) = n.upgrade() {
        n.forget(&e);
      }
    }));
  }
  item.append(&head);

  let card = crate::notify::history_card(entry);
  card.set_size_request(-1, -1);
  card.set_halign(gtk::Align::Fill);
  let click = gtk::GestureClick::new();
  click.set_button(0);
  {
    let (n, e, p) = (Rc::downgrade(n), entry.clone(), panel.clone());
    click.connect_released(move |g, _, _, _| {
      let Some(n) = n.upgrade() else { return };
      if g.current_button() == gtk::gdk::BUTTON_SECONDARY {
        n.forget(&e);
      } else {
        if let Some(p) = p.upgrade() {
          p.close();
        }
        n.activate(&e);
      }
    });
  }
  card.add_controller(click);
  item.append(&card);
  item
}
