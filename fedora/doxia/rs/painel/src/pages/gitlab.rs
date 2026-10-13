// Starts a GitLab pipeline with variables, remembering the last combinations.
// Ctrl+Enter starts it.

use super::{Page, ctrl, is_enter};
use crate::app::AppRef;
use crate::config::{self, Combo, str_of};
use crate::net;
use crate::ui::{self, Field, Frame};
use gtk::gdk;
use gtk::prelude::*;
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;

pub struct Pipeline {
  app: AppRef,
  frame: Frame,
  id: String,
  host: String,
  project: String,
  web_url: String,
  default_ref: String,
  // (key, label, input)
  inputs: Vec<(String, String, Field)>,
  // ref plus the required variables: what a recent combination must have.
  keys: Vec<String>,
  branches: RefCell<Vec<String>>,
  model: gtk::StringList,
  branch: gtk::DropDown,
  recent: gtk::Box,
  send: gtk::Button,
}

impl Pipeline {
  pub fn new(app: AppRef, action: &Value) -> Rc<Pipeline> {
    let title = str_of(action, "title");
    let frame = Frame::new(title, str_of(action, "subtitle"), "\u{f04b}");
    super::wire_back(&frame.back, &app);
    let host = match str_of(action, "host") {
      "" => "gitlab.com",
      h => h,
    }
    .to_string();
    let project = config::text_of(action, "project");
    let web_url = match str_of(action, "projectUrl") {
      "" => format!("https://{host}/{project}"),
      u => u.to_string(),
    };
    let web = ui::icon_button(ui::G_GLOBE, "Abrir pipelines no GitLab");
    let pipelines = format!("{web_url}/-/pipelines");
    web.connect_clicked(move |_| ui::open_url(&pipelines));
    frame.actions.append(&web);

    let form_title = match str_of(action, "formTitle") {
      "" => title,
      t => t,
    };
    frame.body.append(&ui::label(&form_title.to_uppercase(), &["caption", "section"]));
    let description = str_of(action, "formDescription");
    if !description.is_empty() {
      frame.body.append(&ui::wrapped(description, &["desc"]));
    }

    let branch_box = ui::vbox(6);
    branch_box.append(&ui::label("Branch", &["caption"]));
    let model = gtk::StringList::new(&[]);
    let branch = gtk::DropDown::new(Some(model.clone()), None::<gtk::Expression>);
    branch.set_expression(Some(gtk::PropertyExpression::new(gtk::StringObject::static_type(), None::<gtk::Expression>, "string")));
    branch.set_enable_search(true);
    branch.add_css_class("choice");
    branch.add_css_class("wide");
    branch_box.append(&branch);
    frame.body.append(&branch_box);

    let mut inputs = Vec::new();
    let mut keys = vec!["ref".to_string()];
    let mut default = Combo::new();
    let default_ref = match str_of(action, "defaultRef") {
      "" => "main",
      r => r,
    }
    .to_string();
    default.insert("ref".into(), Value::String(default_ref.clone()));
    for var in action["variables"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
      let key = str_of(var, "key").to_string();
      let label = match str_of(var, "label") {
        "" => key.clone(),
        l => l.to_string(),
      };
      let (row, input) = ui::field(&label, str_of(var, "hint"), false, 0);
      frame.body.append(&row);
      if var["required"].as_bool().unwrap_or(true) {
        keys.push(key.clone());
      }
      default.insert(key.clone(), Value::String(config::text_of(var, "default")));
      inputs.push((key, label, input));
    }

    let recent = ui::vbox(6);
    frame.body.append(&recent);

    let send = ui::button(
      match str_of(action, "submitLabel") {
        "" => "Iniciar pipeline",
        s => s,
      },
      true,
    );
    send.set_tooltip_text(Some("Ctrl+Enter"));
    frame.add(&send);

    let id = str_of(action, "id").to_string();
    let me = Rc::new(Pipeline {
      app,
      frame,
      id,
      host,
      project,
      web_url,
      default_ref,
      inputs,
      keys,
      branches: RefCell::default(),
      model,
      branch,
      recent,
      send,
    });
    let weak = Rc::downgrade(&me);
    me.send.connect_clicked(move |_| {
      if let Some(me) = weak.upgrade() {
        me.on_send();
      }
    });
    let first = config::load_recent(&me.id, &me.keys).into_iter().next().unwrap_or(default);
    me.fill(&first);
    me.refresh_recent();
    me
  }

  fn selected_branch(&self) -> String {
    self.branch.selected_item().and_downcast::<gtk::StringObject>().map(|s| s.string().to_string()).unwrap_or_default()
  }

  fn select_branch(&self, reference: &str) {
    if reference.is_empty() {
      return;
    }
    let pos = self.branches.borrow().iter().position(|b| b == reference);
    let pos = pos.unwrap_or_else(|| {
      self.branches.borrow_mut().push(reference.to_string());
      self.model.append(reference);
      self.branches.borrow().len() - 1
    });
    self.branch.set_selected(pos as u32);
  }

  fn fill(&self, combo: &Combo) {
    self.select_branch(combo.get("ref").and_then(Value::as_str).unwrap_or(""));
    for (key, _, input) in &self.inputs {
      input.set_value(combo.get(key).and_then(Value::as_str).unwrap_or(""));
    }
  }

  fn load_branches(self: &Rc<Self>) {
    if self.branches.borrow().len() > 1 {
      return;
    }
    let current = self.selected_branch();
    let (host, project) = (self.host.clone(), self.project.clone());
    let me = self.clone();
    ui::background(
      move || net::gitlab_branches(&host, &project),
      move |result| match result {
        Err(e) => me.frame.set_status(&format!("Não deu para listar as branches: {e}")),
        Ok(names) => {
          let n = me.model.n_items();
          let refs: Vec<&str> = names.iter().map(String::as_str).collect();
          me.model.splice(0, n, &refs);
          *me.branches.borrow_mut() = names;
          me.select_branch(if current.is_empty() { &me.default_ref } else { &current });
        }
      },
    );
  }

  fn refresh_recent(self: &Rc<Self>) {
    ui::clear(&self.recent);
    let recent = config::load_recent(&self.id, &self.keys);
    self.recent.set_visible(!recent.is_empty());
    if recent.is_empty() {
      return;
    }
    let caption = ui::label("RECENTES", &["caption", "section"]);
    caption.set_margin_top(6);
    self.recent.append(&caption);
    for combo in recent {
      let row = ui::hbox(12);
      row.add_css_class("action-row");
      let text = ui::vbox(2);
      text.set_hexpand(true);
      text.append(&ui::label(combo.get("ref").and_then(Value::as_str).unwrap_or(""), &["row-title"]));
      let subtitle: Vec<&str> = self
        .inputs
        .iter()
        .filter_map(|(k, ..)| combo.get(k).and_then(Value::as_str).filter(|v| !v.is_empty()))
        .collect();
      let s = ui::label(&subtitle.join(" · "), &["row-sub"]);
      s.set_ellipsize(gtk::pango::EllipsizeMode::End);
      text.append(&s);
      row.append(&text);
      let paste = ui::label(ui::G_PASTE, &["glyph-body", "c-dim"]);
      paste.set_valign(gtk::Align::Center);
      row.append(&paste);
      let weak = Rc::downgrade(self);
      ui::on_click(&row, move || {
        if let Some(me) = weak.upgrade() {
          me.fill(&combo);
        }
      });
      self.recent.append(&row);
    }
  }

  fn on_send(self: &Rc<Self>) {
    if !self.send.is_sensitive() {
      return;
    }
    let mut combo = Combo::new();
    combo.insert("ref".into(), Value::String(self.selected_branch()));
    for (key, _, input) in &self.inputs {
      combo.insert(key.clone(), Value::String(input.value()));
    }
    let label = |k: &str| {
      if k == "ref" {
        "Branch".to_string()
      } else {
        self.inputs.iter().find(|(key, ..)| key == k).map(|(_, l, _)| l.clone()).unwrap_or_default()
      }
    };
    let missing: Vec<String> =
      self.keys.iter().filter(|k| combo.get(*k).and_then(Value::as_str).unwrap_or("").is_empty()).map(|k| label(k)).collect();
    if !missing.is_empty() {
      self.frame.set_status(&format!("Faltando: {}", missing.join(", ")));
      return;
    }
    self.send.set_sensitive(false);
    self.frame.set_status("Iniciando…");
    let reference = self.selected_branch();
    let variables: Vec<(String, String)> = self
      .inputs
      .iter()
      .map(|(k, _, i)| (k.clone(), i.value()))
      .filter(|(_, v)| !v.is_empty())
      .collect();
    let (host, project) = (self.host.clone(), self.project.clone());
    let me = self.clone();
    let r = reference.clone();
    ui::background(
      move || net::gitlab_pipeline(&host, &project, &r, &variables),
      move |result| {
        me.send.set_sensitive(true);
        match result {
          Err(e) => me.frame.set_status(&e),
          Ok(pipeline) => {
            config::save_recent(&me.id, &me.keys, &combo);
            me.refresh_recent();
            me.frame.set_status("");
            let pid = config::text_of(&pipeline, "id");
            let url = match str_of(&pipeline, "web_url") {
              "" => format!("{}/-/pipelines/{pid}", me.web_url),
              u => u.to_string(),
            };
            if let Some(app) = me.app.upgrade() {
              app.notify_link(&format!("Pipeline #{} iniciada ({reference})", if pid.is_empty() { "?" } else { &pid }), &url);
            }
          }
        }
      },
    );
  }
}

impl Page for Pipeline {
  fn widget(&self) -> gtk::Widget {
    self.frame.root.clone().upcast()
  }

  fn shown(self: Rc<Self>) {
    self.load_branches();
  }

  fn key(&self, key: gdk::Key, mods: gdk::ModifierType) -> bool {
    if ctrl(mods) && is_enter(key) {
      self.send.emit_clicked();
      return true;
    }
    false
  }
}
