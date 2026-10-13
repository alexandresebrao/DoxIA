// E-mail and API token for a Jira site, tested against /myself before they
// are saved.

use super::{Page, is_enter};
use crate::app::AppRef;
use crate::config;
use crate::net;
use crate::ui::{self, Frame};
use gtk::gdk;
use gtk::prelude::*;
use std::rc::Rc;

const TOKEN_URL: &str = "https://id.atlassian.com/manage-profile/security/api-tokens";

pub struct Credentials {
  app: AppRef,
  frame: Frame,
  site: String,
  email: gtk::Entry,
  token: gtk::PasswordEntry,
  save: gtk::Button,
}

impl Credentials {
  pub fn new(app: AppRef, site: &str) -> Rc<Credentials> {
    let host = site.split("://").nth(1).unwrap_or(site).split('/').next().unwrap_or(site);
    let frame = Frame::new("Credenciais do Jira", &format!("Acesso ao {host}"), ui::G_KEY);
    let creds = config::load_credentials(site);

    frame.body.append(&ui::wrapped(
      &format!(
        "Usado só para agir no Jira em seu nome. Fica salvo em {} (visível só para você).",
        config::tilde(&config::credentials_file())
      ),
      &["desc"],
    ));
    let (email_box, email) = ui::field("E-mail da conta Atlassian", "", false, 0);
    let ui::Field::Line(email) = email else { unreachable!() };
    email.set_text(&creds.email);
    frame.body.append(&email_box);

    let token_box = ui::vbox(6);
    token_box.append(&ui::label("API token", &["caption"]));
    let token = gtk::PasswordEntry::new();
    token.set_show_peek_icon(true);
    token.add_css_class("field");
    token.set_text(&creds.token);
    token_box.append(&token);
    frame.body.append(&token_box);

    let link = ui::hbox(12);
    link.add_css_class("action-row");
    let text = ui::vbox(2);
    text.set_hexpand(true);
    text.append(&ui::label("Gerar um API token", &["row-title"]));
    text.append(&ui::label("id.atlassian.com → Segurança → API tokens", &["row-sub"]));
    link.append(&text);
    let arrow = ui::label(ui::G_EXTERNAL, &["glyph-body", "c-dim"]);
    arrow.set_valign(gtk::Align::Center);
    link.append(&arrow);
    ui::on_click(&link, || ui::open_url(TOKEN_URL));
    frame.body.append(&link);

    let save = ui::button("Testar e salvar", true);
    frame.add(&save);

    super::wire_back(&frame.back, &app);
    let me = Rc::new(Credentials { app, frame, site: site.to_string(), email, token, save });
    let weak = Rc::downgrade(&me);
    me.save.connect_clicked(move |_| {
      if let Some(me) = weak.upgrade() {
        me.on_save();
      }
    });
    me
  }

  fn on_save(self: &Rc<Self>) {
    let creds = config::Credentials { email: self.email.text().trim().to_string(), token: self.token.text().trim().to_string() };
    if creds.email.is_empty() || creds.token.is_empty() {
      self.frame.set_status("Preencha e-mail e token.");
      return;
    }
    self.save.set_sensitive(false);
    self.frame.set_status("Testando…");
    let site = self.site.clone();
    let me = self.clone();
    let c = creds.clone();
    ui::background(
      move || net::jira(&site, &c, "GET", "/rest/api/3/myself", None),
      move |result| {
        me.save.set_sensitive(true);
        match result {
          Err(e) => me.frame.set_status(&e),
          Ok(user) => {
            if let Err(e) = config::save_credentials(&me.site, &creds) {
              me.frame.set_status(&format!("Não deu para salvar: {e}"));
              return;
            }
            let name = config::str_of(&user, "displayName");
            me.frame.set_status(&format!("Conectado como {}.", if name.is_empty() { &creds.email } else { name }));
            if let Some(app) = me.app.upgrade() {
              app.credentials_saved();
            }
          }
        }
      },
    );
  }
}

impl Page for Credentials {
  fn widget(&self) -> gtk::Widget {
    self.frame.root.clone().upcast()
  }

  fn shown(self: Rc<Self>) {
    if self.email.text().is_empty() { self.email.grab_focus() } else { self.token.grab_focus() };
  }

  fn key(&self, key: gdk::Key, _mods: gdk::ModifierType) -> bool {
    if is_enter(key) && self.save.is_sensitive() {
      self.save.emit_clicked();
      return true;
    }
    false
  }
}
