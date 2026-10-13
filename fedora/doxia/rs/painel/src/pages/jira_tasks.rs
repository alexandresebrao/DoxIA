// The issues of a JQL search, grouped by status category and status, with a
// filter (Ctrl+F) and a refresh (Ctrl+R).

use super::{Page, ctrl, is_key};
use crate::app::AppRef;
use crate::config::{self, str_of};
use crate::net;
use crate::ui::{self, Frame};
use gtk::gdk;
use gtk::prelude::*;
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;

const CATEGORIES: [(&str, &str, &str); 3] =
  [("indeterminate", "Em andamento", "c-accent"), ("new", "A fazer", "c-warn"), ("done", "Concluídas", "c-ok")];
const DEFAULT_JQL: &str = "assignee = currentUser() AND statusCategory != Done ORDER BY updated DESC";

pub struct JiraTasks {
  app: AppRef,
  frame: Frame,
  site: String,
  jql: String,
  search: gtk::SearchEntry,
  list: gtk::Box,
  refresh: gtk::Button,
  status: (gtk::Box, gtk::Label, gtk::Label, gtk::Label),
  issues: RefCell<Vec<Value>>,
  loading: RefCell<bool>,
}

// "2026-10-05T11:02:33.000-0300" -> "há 3 h".
fn time_ago(iso: &str) -> String {
  let Ok(then) = gtk::glib::DateTime::from_iso8601(iso, None) else { return String::new() };
  let Ok(now) = gtk::glib::DateTime::now_utc() else { return String::new() };
  let mins = now.difference(&then).as_minutes();
  match mins {
    ..60 => format!("há {} min", mins.max(1)),
    60..1440 => format!("há {} h", mins / 60),
    _ if mins / 1440 == 1 => "ontem".into(),
    _ => format!("há {} dias", mins / 1440),
  }
}

impl JiraTasks {
  pub fn new(app: AppRef, cfg: &Value, action: &Value) -> Rc<JiraTasks> {
    let frame = Frame::new(str_of(action, "title"), str_of(action, "subtitle"), ui::G_LIST);
    super::wire_back(&frame.back, &app);
    frame.footer.set_visible(false);
    let site = config::jira_site(cfg, action);
    let jql = match str_of(action, "jql") {
      "" => DEFAULT_JQL,
      j => j,
    }
    .to_string();

    let refresh = ui::icon_button(ui::G_REFRESH, "Atualizar (Ctrl+R)");
    frame.actions.append(&refresh);
    let web = ui::icon_button(ui::G_GLOBE, "Abrir a busca no Jira");
    let url = format!("{site}/issues/?jql={}", net::quote(&jql));
    web.connect_clicked(move |_| ui::open_url(&url));
    frame.actions.append(&web);

    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Filtrar por chave, projeto ou título"));
    search.add_css_class("field");
    frame.body.append(&search);
    let status = ui::status_page(ui::G_LIST, "Carregando…", "");
    frame.body.append(&status.0);
    let list = ui::vbox(14);
    frame.body.append(&list);

    let me = Rc::new(JiraTasks {
      app,
      frame,
      site,
      jql,
      search,
      list,
      refresh,
      status,
      issues: RefCell::default(),
      loading: RefCell::new(false),
    });
    let weak = Rc::downgrade(&me);
    me.refresh.connect_clicked(move |_| {
      if let Some(me) = weak.upgrade() {
        me.load();
      }
    });
    let weak = Rc::downgrade(&me);
    me.search.connect_search_changed(move |_| {
      if let Some(me) = weak.upgrade() {
        me.render();
      }
    });
    me
  }

  fn show_status(&self, title: &str, description: &str) {
    self.status.2.set_text(title);
    self.status.3.set_text(description);
    self.status.0.set_visible(true);
    self.list.set_visible(false);
  }

  fn load(self: &Rc<Self>) {
    if *self.loading.borrow() {
      return;
    }
    let Some(app) = self.app.upgrade() else { return };
    let creds = config::load_credentials(&self.site);
    if creds.token.is_empty() {
      let weak = Rc::downgrade(self);
      app.push_credentials(&self.site, Some(Box::new(move || if let Some(me) = weak.upgrade() { me.load() })));
      return;
    }
    *self.loading.borrow_mut() = true;
    self.refresh.set_sensitive(false);
    if self.issues.borrow().is_empty() {
      self.show_status("Carregando…", "");
    }
    let (site, jql) = (self.site.clone(), self.jql.clone());
    let me = self.clone();
    ui::background(
      move || net::jira_search(&site, &creds, &jql),
      move |result| {
        *me.loading.borrow_mut() = false;
        me.refresh.set_sensitive(true);
        match result {
          Err(e) => {
            if e.contains("401") {
              if let Some(app) = me.app.upgrade() {
                let weak = Rc::downgrade(&me);
                app.push_credentials(&me.site, Some(Box::new(move || if let Some(me) = weak.upgrade() { me.load() })));
              }
            }
            me.show_status("Não foi possível buscar as tarefas", &e);
          }
          Ok(issues) => {
            *me.issues.borrow_mut() = issues;
            me.render();
          }
        }
      },
    );
  }

  fn render(self: &Rc<Self>) {
    ui::clear(&self.list);
    let query = self.search.text().trim().to_lowercase();
    let issues: Vec<Value> = self
      .issues
      .borrow()
      .iter()
      .filter(|i| {
        let f = &i["fields"];
        query.is_empty()
          || format!("{} {} {}", str_of(i, "key"), str_of(&f["project"], "name"), str_of(f, "summary"))
            .to_lowercase()
            .contains(&query)
      })
      .cloned()
      .collect();
    if issues.is_empty() {
      if query.is_empty() {
        self.show_status("Nenhuma tarefa", "");
      } else {
        self.show_status("Nada encontrado", &format!("Nenhuma tarefa com “{query}”"));
      }
      return;
    }
    self.status.0.set_visible(false);
    self.list.set_visible(true);

    for (category, title, color) in CATEGORIES {
      let in_cat: Vec<&Value> =
        issues.iter().filter(|i| str_of(&i["fields"]["status"]["statusCategory"], "key") == category).collect();
      if in_cat.is_empty() {
        continue;
      }
      let group = ui::vbox(6);
      group.append(&ui::label(&format!("{}  ·  {}", title.to_uppercase(), in_cat.len()), &["caption", "section"]));
      // Statuses in the order they first appear (the search's order).
      let mut by_status: Vec<(String, Vec<&Value>)> = Vec::new();
      for issue in in_cat {
        let name = str_of(&issue["fields"]["status"], "name").to_string();
        match by_status.iter_mut().find(|(n, _)| *n == name) {
          Some((_, list)) => list.push(issue),
          None => by_status.push((name, vec![issue])),
        }
      }
      for (name, items) in by_status {
        group.append(&self.status_group(&name, color, &items, category != "done" || !query.is_empty()));
      }
      self.list.append(&group);
    }
  }

  fn status_group(self: &Rc<Self>, name: &str, color: &str, items: &[&Value], expanded: bool) -> gtk::Box {
    let card = ui::vbox(0);
    card.add_css_class("status-group");
    let head = ui::hbox(10);
    head.add_css_class("status-head");
    head.append(&ui::label("●", &[color]));
    let text = ui::vbox(1);
    text.set_hexpand(true);
    text.append(&ui::label(name, &["row-title"]));
    let n = items.len();
    text.append(&ui::label(&format!("{n} tarefa{}", if n > 1 { "s" } else { "" }), &["row-sub"]));
    head.append(&text);
    let chevron = ui::label(if expanded { "\u{f078}" } else { ui::G_NEXT }, &["glyph-body", "c-dim"]);
    chevron.set_valign(gtk::Align::Center);
    head.append(&chevron);
    card.append(&head);

    let revealer = gtk::Revealer::new();
    revealer.set_reveal_child(expanded);
    let rows = ui::vbox(0);
    for issue in items {
      rows.append(&self.issue_row(issue));
    }
    revealer.set_child(Some(&rows));
    card.append(&revealer);
    ui::on_click(&head, move || {
      let open = !revealer.reveals_child();
      revealer.set_reveal_child(open);
      chevron.set_text(if open { "\u{f078}" } else { ui::G_NEXT });
    });
    card
  }

  fn issue_row(self: &Rc<Self>, issue: &Value) -> gtk::Box {
    let f = &issue["fields"];
    let key = str_of(issue, "key").to_string();
    let url = format!("{}/browse/{key}", self.site);
    let mut meta = vec![str_of(&f["project"], "name").to_string(), str_of(&f["issuetype"], "name").to_string()];
    if f["priority"].is_object() {
      meta.push(str_of(&f["priority"], "name").to_string());
    }
    meta.push(time_ago(str_of(f, "updated")));
    let meta: Vec<String> = meta.into_iter().filter(|m| !m.is_empty()).collect();

    let row = ui::hbox(10);
    row.add_css_class("issue-row");
    row.set_tooltip_text(Some("Abrir no Jira"));
    let text = ui::vbox(2);
    text.set_hexpand(true);
    let title = ui::wrapped(&format!("{key}  {}", str_of(f, "summary")), &["issue-title"]);
    title.set_lines(2);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    text.append(&title);
    text.append(&ui::label(&meta.join("  ·  "), &["row-sub"]));
    row.append(&text);

    let copy_btn = ui::icon_button(ui::G_COPY, "Copiar link");
    copy_btn.add_css_class("plain");
    let (u, k, app) = (url.clone(), key.clone(), self.app.clone());
    copy_btn.connect_clicked(move |_| {
      ui::copy(&u);
      if let Some(app) = app.upgrade() {
        app.toasts.show(&format!("Link de {k} copiado"), 3);
      }
    });
    row.append(&copy_btn);
    ui::on_click(&text, move || ui::open_url(&url));
    row
  }
}

impl Page for JiraTasks {
  fn widget(&self) -> gtk::Widget {
    self.frame.root.clone().upcast()
  }

  fn shown(self: Rc<Self>) {
    self.load();
  }

  fn key(&self, key: gdk::Key, mods: gdk::ModifierType) -> bool {
    if ctrl(mods) && is_key(key, 'r') {
      self.refresh.emit_clicked();
      return true;
    }
    if ctrl(mods) && is_key(key, 'f') {
      self.search.grab_focus();
      return true;
    }
    false
  }
}
