// A free text box that Claude rewrites (Ctrl+Enter or Ctrl+E), with an
// optional extra instruction; Ctrl+Shift+C copies the text.

use super::{Page, ctrl, is_enter, is_key};
use crate::app::AppRef;
use crate::config::str_of;
use crate::net;
use crate::ui::{self, Field, Frame};
use gtk::gdk;
use gtk::prelude::*;
use serde_json::Value;
use std::rc::Rc;

const PROMPT: &str = "Você melhora textos. Reescreva o texto recebido deixando-o claro, correto e bem escrito: \
corrija ortografia, gramática e pontuação, melhore a fluidez e a organização, mantenha o \
sentido, os fatos, o idioma e o tom do original, sem inventar informações. Se houver uma \
instrução extra, siga-a. Responda só com o texto final, em texto puro, sem comentários, \
sem aspas e sem markdown.";

pub struct ClaudeText {
  app: AppRef,
  frame: Frame,
  spec: Value,
  text: Field,
  instruction: Option<Field>,
  spinner: gtk::Spinner,
  improve: gtk::Button,
  copy_btn: gtk::Button,
}

impl ClaudeText {
  pub fn new(app: AppRef, action: &Value) -> Rc<ClaudeText> {
    let frame = Frame::new(str_of(action, "title"), str_of(action, "subtitle"), ui::G_MAGIC);
    super::wire_back(&frame.back, &app);
    let mut spec = action.clone();
    if str_of(action, "prompt").is_empty() && str_of(action, "promptFile").is_empty() {
      spec["prompt"] = Value::String(PROMPT.into());
    }

    let title = match str_of(action, "formTitle") {
      "" => "Texto",
      t => t,
    };
    let description = match str_of(action, "formDescription") {
      "" => "Cole o texto e peça para o Claude melhorar.",
      d => d,
    };
    frame.body.append(&ui::wrapped(description, &["desc"]));
    let (row, text) = ui::field(title, "", true, 260);
    frame.body.append(&row);
    let instruction = if action["instruction"].as_bool().unwrap_or(true) {
      let (row, input) = ui::field("Instrução extra (opcional)", "ex.: mais formal, mais curto, em inglês", false, 0);
      frame.body.append(&row);
      Some(input)
    } else {
      None
    };

    let spinner = gtk::Spinner::new();
    spinner.set_visible(false);
    frame.add(&spinner);
    let copy_btn = ui::glyph_button(ui::G_COPY, "Copiar");
    copy_btn.set_tooltip_text(Some("Copiar o texto (Ctrl+Shift+C)"));
    frame.add(&copy_btn);
    let label = match str_of(action, "label") {
      "" => "Melhorar com Claude",
      l => l,
    };
    let improve = ui::button(label, true);
    improve.set_tooltip_text(Some("Reescrever o texto com o Claude (Ctrl+Enter)"));
    frame.add(&improve);

    let me = Rc::new(ClaudeText { app, frame, spec, text, instruction, spinner, improve, copy_btn });
    let weak = Rc::downgrade(&me);
    me.improve.connect_clicked(move |_| {
      if let Some(me) = weak.upgrade() {
        me.on_improve();
      }
    });
    let weak = Rc::downgrade(&me);
    me.copy_btn.connect_clicked(move |_| {
      if let Some(me) = weak.upgrade() {
        me.on_copy();
      }
    });
    me
  }

  fn set_busy(&self, busy: bool) {
    self.improve.set_sensitive(!busy);
    self.copy_btn.set_sensitive(!busy);
    self.text.set_editable(!busy);
    self.spinner.set_visible(busy);
    self.spinner.set_spinning(busy);
  }

  fn on_copy(&self) {
    let text = self.text.value();
    if text.is_empty() {
      self.frame.set_status("Nada para copiar");
      return;
    }
    ui::copy(&text);
    if let Some(app) = self.app.upgrade() {
      app.toasts.show("Texto copiado", 3);
    }
  }

  fn on_improve(self: &Rc<Self>) {
    if !self.improve.is_sensitive() {
      return;
    }
    let original = self.text.value();
    if original.is_empty() {
      self.frame.set_status("Cole um texto primeiro");
      self.text.grab_focus();
      return;
    }
    let extra = self.instruction.as_ref().map(Field::value).unwrap_or_default();
    let context = if extra.is_empty() { vec![] } else { vec![("Instrução extra".to_string(), extra)] };
    self.set_busy(true);
    self.frame.set_status("Claude escrevendo…");
    let (spec, text) = (self.spec.clone(), original.clone());
    let me = self.clone();
    ui::background(
      move || net::claude_rewrite(&spec, &context, &text),
      move |result| {
        me.set_busy(false);
        match result {
          Err(e) => me.frame.set_status(&e),
          Ok(text) => {
            me.frame.set_status("Revise o texto antes de usar.");
            me.text.set_value(&text);
            if let Some(app) = me.app.upgrade() {
              let t = me.text.clone();
              app.toasts.show_with("Texto reescrito pelo Claude", 10, Some("Desfazer"), Some(move || t.set_value(&original)));
            }
          }
        }
      },
    );
  }
}

impl Page for ClaudeText {
  fn widget(&self) -> gtk::Widget {
    self.frame.root.clone().upcast()
  }

  fn shown(self: Rc<Self>) {
    self.text.grab_focus();
  }

  fn key(&self, key: gdk::Key, mods: gdk::ModifierType) -> bool {
    if ctrl(mods) && (is_enter(key) || is_key(key, 'e')) {
      self.improve.emit_clicked();
      return true;
    }
    if ctrl(mods) && mods.contains(gdk::ModifierType::SHIFT_MASK) && is_key(key, 'c') {
      self.on_copy();
      return true;
    }
    false
  }
}
