// Quickshell services doxia-shell has taken over. A Quickshell service holds
// its D-Bus resources (org.freedesktop.Notifications, the session's polkit
// agent) while it runs; listing it in shell.json's disabledPlugins keeps it
// from loading. Quickshell, which watches the file, unloads a running one but
// may keep its bus name until it restarts, so doxia-shell waits in line and
// retries rather than restarting it (omarchy-restart-shell can lock the
// session). fedora/doxia/apply-bar takes the entries back out on machines
// without doxia-shell.

use serde_json::Value;
use std::path::PathBuf;

// True when shell.json changed (the service was on until now).
pub fn disable(id: &str) -> bool {
  edit(|obj| {
    let list = obj.entry("disabledPlugins").or_insert_with(|| Value::Array(vec![]));
    let Some(list) = list.as_array_mut() else { return false };
    if list.iter().any(|v| v.as_str() == Some(id)) {
      return false;
    }
    list.push(Value::String(id.into()));
    true
  })
}

// Quickshell always loads a bar: bar.id picks the doxia.bar plugin, which
// draws nothing, in place of its own (back to it while the plugin is not
// installed). True when shell.json changed.
pub fn take_bar() -> bool {
  edit(|obj| {
    let Some(bar) = obj.get_mut("bar").and_then(Value::as_object_mut) else { return false };
    if bar.get("id").and_then(Value::as_str) == Some("doxia.bar") {
      return false;
    }
    bar.insert("id".into(), Value::String("doxia.bar".into()));
    true
  })
}

// Changes shell.json when `change` says it did.
fn edit(change: impl FnOnce(&mut serde_json::Map<String, Value>) -> bool) -> bool {
  let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
  let path = home.join(".config/omarchy/shell.json");
  let Ok(text) = std::fs::read_to_string(&path) else { return false };
  let Ok(mut config) = serde_json::from_str::<Value>(&text) else { return false };
  let Some(obj) = config.as_object_mut() else { return false };
  if !change(obj) {
    return false;
  }
  let Ok(out) = serde_json::to_string_pretty(&config) else { return false };
  let tmp = path.with_extension("json.tmp");
  std::fs::write(&tmp, out + "\n").is_ok() && std::fs::rename(&tmp, &path).is_ok()
}
