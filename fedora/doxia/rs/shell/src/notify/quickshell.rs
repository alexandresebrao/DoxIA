// Quickshell's notification service holds org.freedesktop.Notifications while
// it runs. Listing it in shell.json's disabledPlugins keeps it from loading;
// Quickshell (which watches the file) unloads a running one, but the name
// stays taken until Quickshell restarts. fedora/doxia/apply-bar takes the
// entry back out on machines without doxia-shell, so they are never left
// without a server.

use serde_json::Value;
use std::path::PathBuf;

const ID: &str = "omarchy.notifications";

// True when shell.json changed (the service was on until now).
pub fn release_notifications() -> bool {
  let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
  let path = home.join(".config/omarchy/shell.json");
  let Ok(text) = std::fs::read_to_string(&path) else { return false };
  let Ok(mut config) = serde_json::from_str::<Value>(&text) else { return false };
  let Some(obj) = config.as_object_mut() else { return false };
  let list = obj.entry("disabledPlugins").or_insert_with(|| Value::Array(vec![]));
  let Some(list) = list.as_array_mut() else { return false };
  if list.iter().any(|v| v.as_str() == Some(ID)) {
    return false;
  }
  list.push(Value::String(ID.into()));
  let Ok(out) = serde_json::to_string_pretty(&config) else { return false };
  let tmp = path.with_extension("json.tmp");
  std::fs::write(&tmp, out + "\n").is_ok() && std::fs::rename(&tmp, &path).is_ok()
}
