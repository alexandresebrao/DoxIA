// The widgets a layout entry can name. An id the bar does not know yet
// takes no space.

mod calendar;
pub mod clock;
mod doxia;
mod media;
mod system;
mod tray;
mod usage;
mod weather;
mod weather_model;

use super::config::entry_id;
use super::Ctx;
use serde_json::Value;
use std::rc::Rc;

pub fn build(ctx: &Rc<Ctx>, entry: &Value) -> Option<gtk::Widget> {
  match entry_id(entry).as_str() {
    "omarchy.clock" => Some(clock::build(ctx, entry)),
    "doxia.emblem" => Some(doxia::emblem(ctx)),
    "doxia.workspaces" => Some(doxia::workspaces(ctx)),
    "doxia.screenshare" => Some(doxia::screenshare(ctx)),
    "doxia.usage" => usage::build(ctx, entry),
    "doxia.media" => Some(media::build(ctx, entry)),
    "omarchy.weather" => Some(weather::build(ctx, entry)),
    "omarchy.tray" => Some(tray::build(ctx, entry)),
    "omarchy.keyboard-layout" => Some(system::keyboard_layout(ctx)),
    "omarchy.system-update" => Some(system::system_update(ctx)),
    _ => None,
  }
}
