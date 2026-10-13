// A Jira Service Management request form, with an optional "Melhorar com
// Claude" (Ctrl+E) that rewrites one field. Ctrl+Enter sends.

use super::{Page, ctrl, is_enter, is_key};
use crate::app::AppRef;
use crate::config::{self, str_of};
use crate::net;
use crate::ui::{self, Field, Frame};
use gtk::gdk;
use gtk::prelude::*;
use serde_json::{Map, Value};
use std::rc::Rc;

pub struct JiraRequest {
  app: AppRef,
  frame: Frame,
  action: Value,
  site: String,
  // (key, label, required, input)
  fields: Vec<(String, String, bool, Field)>,
  claude: Option<Value>,
  claude_btn: Option<gtk::Button>,
  spinner: gtk::Spinner,
  send: gtk::Button,
}

impl JiraRequest {
  pub fn new(app: AppRef, cfg: &Value, action: &Value) -> Rc<JiraRequest> {
    let title = str_of(action, "title");
    let frame = Frame::new(title, str_of(action, "subtitle"), ui::G_WARN);
    super::wire_back(&frame.back, &app);
    let portal = str_of(action, "portalUrl").to_string();
    if !portal.is_empty() {
      let b = ui::icon_button(ui::G_GLOBE, "Abrir no portal");
      b.connect_clicked(move |_| ui::open_url(&portal));
      frame.actions.append(&b);
    }

    let form_title = match str_of(action, "formTitle") {
      "" => title,
      t => t,
    };
    frame.body.append(&ui::label(&form_title.to_uppercase(), &["caption", "section"]));
    let description = str_of(action, "formDescription");
    if !description.is_empty() {
      frame.body.append(&ui::wrapped(description, &["desc"]));
    }

    let mut fields = Vec::new();
    for f in action["fields"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
      let key = str_of(f, "key").to_string();
      let label = str_of(f, "label").to_string();
      let multiline = f["multiline"].as_bool().unwrap_or(false);
      let (row, input) = ui::field(&label, str_of(f, "hint"), multiline, 180);
      frame.body.append(&row);
      fields.push((key, label, f["required"].as_bool().unwrap_or(true), input));
    }

    let claude = action.get("claude").filter(|c| fields.iter().any(|(k, ..)| k == str_of(c, "field"))).cloned();
    let spinner = gtk::Spinner::new();
    spinner.set_visible(false);
    frame.add(&spinner);
    let claude_btn = claude.as_ref().map(|c| {
      let label = match str_of(c, "label") {
        "" => "Melhorar com Claude",
        l => l,
      };
      let tip = match str_of(c, "tooltip") {
        "" => "Reescrever o texto com o Claude",
        t => t,
      };
      let b = ui::glyph_button(ui::G_MAGIC, label);
      b.set_tooltip_text(Some(&format!("{tip} (Ctrl+E)")));
      frame.add(&b);
      b
    });
    let send = ui::button(
      match str_of(action, "submitLabel") {
        "" => "Abrir chamado",
        s => s,
      },
      true,
    );
    send.set_tooltip_text(Some("Ctrl+Enter"));
    frame.add(&send);

    let me = Rc::new(JiraRequest {
      app,
      frame,
      action: action.clone(),
      site: config::jira_site(cfg, action),
      fields,
      claude,
      claude_btn,
      spinner,
      send,
    });
    let weak = Rc::downgrade(&me);
    me.send.connect_clicked(move |_| {
      if let Some(me) = weak.upgrade() {
        me.on_send();
      }
    });
    if let Some(b) = &me.claude_btn {
      let weak = Rc::downgrade(&me);
      b.connect_clicked(move |_| {
        if let Some(me) = weak.upgrade() {
          me.on_improve();
        }
      });
    }
    me
  }

  fn input(&self, key: &str) -> Option<&Field> {
    self.fields.iter().find(|(k, ..)| k == key).map(|(.., f)| f)
  }

  fn set_busy(&self, busy: bool) {
    self.send.set_sensitive(!busy);
    if let Some(b) = &self.claude_btn {
      b.set_sensitive(!busy);
    }
    self.spinner.set_visible(busy);
    self.spinner.set_spinning(busy);
    if let Some(f) = self.claude.as_ref().and_then(|c| self.input(str_of(c, "field"))) {
      f.set_editable(!busy);
    }
  }

  fn on_improve(self: &Rc<Self>) {
    let (Some(spec), Some(btn)) = (self.claude.clone(), &self.claude_btn) else { return };
    if !btn.is_sensitive() {
      return;
    }
    let key = str_of(&spec, "field").to_string();
    let target = self.input(&key).unwrap().clone();
    let original = target.value();
    if original.is_empty() {
      let label = self.fields.iter().find(|(k, ..)| *k == key).map(|(_, l, ..)| l.to_lowercase()).unwrap_or_default();
      self.frame.set_status(&format!("Escreva {label} primeiro"));
      target.grab_focus();
      return;
    }
    let context: Vec<(String, String)> =
      self.fields.iter().filter(|(k, ..)| *k != key).map(|(_, l, _, f)| (l.clone(), f.value())).collect();
    self.set_busy(true);
    self.frame.set_status("Claude escrevendo…");
    let me = self.clone();
    let text = original.clone();
    ui::background(
      move || net::claude_rewrite(&spec, &context, &text),
      move |result| {
        me.set_busy(false);
        match result {
          Err(e) => me.frame.set_status(&e),
          Ok(text) => {
            me.frame.set_status("Revise o texto antes de enviar.");
            target.set_value(&text);
            if let Some(app) = me.app.upgrade() {
              let t = target.clone();
              app.toasts.show_with("Texto reescrito pelo Claude", 10, Some("Desfazer"), Some(move || t.set_value(&original)));
            }
          }
        }
      },
    );
  }

  fn on_send(self: &Rc<Self>) {
    if !self.send.is_sensitive() {
      return;
    }
    let values: Vec<(String, String, bool, String)> =
      self.fields.iter().map(|(k, l, r, f)| (k.clone(), l.clone(), *r, f.value())).collect();
    let missing: Vec<&str> = values.iter().filter(|(_, _, r, v)| *r && v.is_empty()).map(|(_, l, ..)| l.as_str()).collect();
    if !missing.is_empty() {
      self.frame.set_status(&format!("Faltando: {}", missing.join(", ")));
      return;
    }
    let Some(app) = self.app.upgrade() else { return };
    let creds = config::load_credentials(&self.site);
    if creds.token.is_empty() {
      let weak = Rc::downgrade(self);
      app.push_credentials(&self.site, Some(Box::new(move || if let Some(me) = weak.upgrade() { me.on_send() })));
      return;
    }
    self.send.set_sensitive(false);
    self.frame.set_status("Enviando…");
    let body: Map<String, Value> =
      values.into_iter().filter(|(.., v)| !v.is_empty()).map(|(k, .., v)| (k, Value::String(v))).collect();
    let (site, action) = (self.site.clone(), self.action.clone());
    let me = self.clone();
    ui::background(
      move || net::jira_create_request(&site, &creds, &action, Value::Object(body)),
      move |result| {
        me.send.set_sensitive(true);
        let Some(app) = me.app.upgrade() else { return };
        match result {
          Err(e) => {
            me.frame.set_status(&e);
            if e.contains("401") {
              let weak = Rc::downgrade(&me);
              app.push_credentials(&me.site, Some(Box::new(move || if let Some(me) = weak.upgrade() { me.on_send() })));
            }
          }
          Ok(issue) => {
            let key = match str_of(&issue, "issueKey") {
              "" => "?",
              k => k,
            };
            let url = match issue["_links"]["web"].as_str() {
              Some(u) if !u.is_empty() => u.to_string(),
              _ => format!("{}/browse/{key}", me.site),
            };
            for (.., f) in &me.fields {
              f.set_value("");
            }
            me.frame.set_status("");
            app.notify_link(&format!("Chamado {key} criado"), &url);
          }
        }
      },
    );
  }
}

impl Page for JiraRequest {
  fn widget(&self) -> gtk::Widget {
    self.frame.root.clone().upcast()
  }

  fn shown(self: Rc<Self>) {
    if let Some((.., f)) = self.fields.first() {
      f.grab_focus();
    }
  }

  fn key(&self, key: gdk::Key, mods: gdk::ModifierType) -> bool {
    if ctrl(mods) && is_enter(key) {
      self.send.emit_clicked();
      return true;
    }
    if ctrl(mods) && is_key(key, 'e') && self.claude_btn.is_some() {
      self.claude_btn.as_ref().unwrap().emit_clicked();
      return true;
    }
    false
  }
}
