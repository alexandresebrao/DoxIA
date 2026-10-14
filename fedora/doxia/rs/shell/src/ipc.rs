// org.doxia.Shell.Call(target, method, args) -> answer: the same calls the
// Quickshell IPC takes (`omarchy-shell <target> <method> [args...]`), for the
// targets doxia-shell has taken over. bin/omarchy-shell asks here first and
// goes on to Quickshell on "Target not found." or when nobody answers, which
// is what lets the pieces move one at a time.

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub const PATH: &str = "/org/doxia/Shell/Ipc";

const XML: &str = r#"<node>
  <interface name="org.doxia.Shell">
    <method name="Call">
      <arg name="target" type="s" direction="in"/>
      <arg name="method" type="s" direction="in"/>
      <arg name="args" type="as" direction="in"/>
      <arg name="answer" type="s" direction="out"/>
    </method>
  </interface>
</node>"#;

// None from a handler: no such method.
pub type Handler = Box<dyn Fn(&str, &[String]) -> Option<String>>;

#[derive(Default)]
pub struct Ipc {
  targets: RefCell<HashMap<String, Handler>>,
}

impl Ipc {
  pub fn add(&self, target: &str, handler: Handler) {
    self.targets.borrow_mut().insert(target.to_string(), handler);
  }

  // A call from inside the shell (a bar widget showing the OSD).
  pub fn invoke(&self, target: &str, method: &str, args: &[String]) -> String {
    self.call(target, method, args)
  }

  fn call(&self, target: &str, method: &str, args: &[String]) -> String {
    match self.targets.borrow().get(target) {
      None => "Target not found.".into(),
      Some(h) => h(method, args).unwrap_or_else(|| "Function not found.".into()),
    }
  }

  pub fn register(self: &Rc<Self>, conn: &gio::DBusConnection) {
    let info = gio::DBusNodeInfo::for_xml(XML).expect("ipc xml");
    let iface = info.lookup_interface("org.doxia.Shell").expect("ipc interface");
    let me = self.clone();
    let result = conn
      .register_object(PATH, &iface)
      .method_call(move |_conn, _sender, _path, _iface, method, params, invocation| {
        if method != "Call" {
          invocation.return_dbus_error("org.freedesktop.DBus.Error.UnknownMethod", method);
          return;
        }
        let Some((target, name, args)) = params.get::<(String, String, Vec<String>)>() else {
          invocation.return_dbus_error("org.freedesktop.DBus.Error.InvalidArgs", "expected (ssas)");
          return;
        };
        let answer = me.call(&target, &name, &args);
        invocation.return_value(Some(&(answer,).to_variant()));
      })
      .build();
    if let Err(e) = result {
      glib::g_warning!("doxia-shell", "IPC not registered: {e}");
    }
  }
}
