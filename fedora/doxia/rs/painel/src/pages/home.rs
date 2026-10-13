// The action list. Alt+1…9 opens the first nine; Ctrl+R rereads painel.json.

use super::{Page, ctrl, is_key};
use crate::app::AppRef;
use crate::config::{self, str_of};
use crate::ui::{self, Frame};
use gtk::gdk;
use gtk::prelude::*;
use serde_json::Value;
use std::rc::Rc;

pub struct Home {
  app: AppRef,
  frame: Frame,
  ids: Vec<String>,
}

fn default_icon(kind: &str) -> &'static str {
  match kind {
    "jira-request" => "dialog-warning-symbolic",
    "jira-tasks" => "view-list-symbolic",
    "gitlab-pipeline" => "media-playback-start-symbolic",
    "claude-text" => "document-edit-symbolic",
    "link" => "web-browser-symbolic",
    "command" => "utilities-terminal-symbolic",
    _ => "application-x-executable-symbolic",
  }
}

impl Home {
  pub fn new(app: AppRef, actions: &[Value], error: Option<String>) -> Rc<Home> {
    let file = config::tilde(&config::config_file());
    let frame = Frame::new("Painel Rápido", &file, ui::G_BOLT);
    frame.back.set_visible(false);
    frame.footer.set_visible(false);
    let reload = ui::icon_button(ui::G_REFRESH, "Recarregar painel.json (Ctrl+R)");
    frame.actions.append(&reload);

    if error.is_some() || actions.is_empty() {
      let (page, ..) = match &error {
        Some(e) => ui::status_page(ui::G_WARN, "Configuração inválida", &gtk::glib::markup_escape_text(e)),
        None => ui::status_page(
          ui::G_GRID,
          "Nenhuma ação ainda",
          "Peça ao Claude Code para criar uma: use a skill <b>/painel</b>, por exemplo\n“/painel adiciona um atalho para abrir chamado no Service Desk”.",
        ),
      };
      frame.body.append(&page);
    } else {
      frame.body.append(&ui::label("AÇÕES", &["caption", "section"]));
    }

    let me = Rc::new(Home { app, frame, ids: actions.iter().map(|a| str_of(a, "id").to_string()).collect() });

    let weak = me.app.clone();
    reload.connect_clicked(move |_| {
      if let Some(app) = weak.upgrade() {
        app.reload();
      }
    });

    for (i, action) in actions.iter().enumerate() {
      let row = ui::hbox(12);
      row.add_css_class("action-row");
      let icon = gtk::CenterBox::new();
      icon.add_css_class("source-icon");
      icon.set_valign(gtk::Align::Center);
      let name = match str_of(action, "icon") {
        "" => default_icon(str_of(action, "type")),
        n => n,
      };
      let image = gtk::Image::from_icon_name(name);
      image.set_pixel_size(16);
      icon.set_center_widget(Some(&image));
      row.append(&icon);

      let text = ui::vbox(2);
      text.set_hexpand(true);
      text.set_valign(gtk::Align::Center);
      let t = ui::label(str_of(action, "title"), &["row-title"]);
      t.set_ellipsize(gtk::pango::EllipsizeMode::End);
      text.append(&t);
      let sub = str_of(action, "subtitle");
      if !sub.is_empty() {
        let s = ui::label(sub, &["row-sub"]);
        s.set_ellipsize(gtk::pango::EllipsizeMode::End);
        text.append(&s);
      }
      row.append(&text);
      if i < 9 {
        let k = ui::label(&format!("Alt+{}", i + 1), &["kbd"]);
        k.set_valign(gtk::Align::Center);
        row.append(&k);
      }
      let page = config::has_page(str_of(action, "type"));
      let arrow = ui::label(if page { ui::G_NEXT } else { ui::G_EXTERNAL }, &["glyph-body", "c-dim"]);
      arrow.set_valign(gtk::Align::Center);
      row.append(&arrow);

      let id = str_of(action, "id").to_string();
      let weak = me.app.clone();
      ui::on_click(&row, move || {
        if let Some(app) = weak.upgrade() {
          app.open_action(&id, false);
        }
      });
      me.frame.body.append(&row);
    }
    me
  }
}

impl Page for Home {
  fn widget(&self) -> gtk::Widget {
    self.frame.root.clone().upcast()
  }

  fn key(&self, key: gdk::Key, mods: gdk::ModifierType) -> bool {
    let Some(app) = self.app.upgrade() else { return false };
    if ctrl(mods) && is_key(key, 'r') {
      app.reload();
      return true;
    }
    if mods.contains(gdk::ModifierType::ALT_MASK) {
      let digit = key.to_unicode().and_then(|c| c.to_digit(10)).filter(|d| (1..=9).contains(d));
      if let Some(id) = digit.and_then(|d| self.ids.get(d as usize - 1)) {
        app.open_action(id, false);
        return true;
      }
    }
    false
  }
}
