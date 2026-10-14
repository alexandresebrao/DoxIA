// The widgets a layout entry can name. An id the bar does not know yet
// takes no space.

mod agents;
mod audio;
mod bluetooth;
mod calendar;
pub mod clock;
mod doxia;
mod fortivpn;
mod group;
mod indicators;
mod media;
mod monitor;
mod network;
mod network_model;
mod power;
mod services;
mod system;
mod tray;
mod usage;
mod weather;
mod weather_model;

use super::config::entry_id;
use super::member::Member;
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
    "doxia.services" => Some(services::build(ctx)),
    "doxia.fortivpn" => Some(fortivpn::build(ctx, entry)),
    "omarchy.agents" => Some(agents::build(ctx, entry)),
    "omarchy.indicators" => Some(indicators::build(ctx, entry)),
    "omarchy.menu" => Some(system::menu_button(ctx)),
    "omarchy.workspaces" => Some(system::omarchy_workspaces(ctx)),
    "omarchy.keyboard-layout" => Some(system::keyboard_layout(ctx)),
    "omarchy.system-update" => Some(system::system_update(ctx)),
    id @ ("omarchy.audio" | "omarchy.power" | "omarchy.monitor" | "omarchy.network" | "omarchy.bluetooth") => member(ctx, id, entry).map(Member::standalone),
    "omarchy.group" => group::build(ctx, entry, |id, e| member(ctx, id, e)),
    _ => None,
  }
}

// A widget with a panel, alone on the bar or in a group.
fn member(ctx: &Rc<Ctx>, id: &str, entry: &Value) -> Option<Member> {
  match id {
    "omarchy.audio" => Some(audio::build(ctx)),
    "omarchy.power" => Some(power::build(ctx, entry)),
    "omarchy.monitor" => Some(monitor::build(ctx)),
    "omarchy.network" => Some(network::build(ctx)),
    "omarchy.bluetooth" => Some(bluetooth::build(ctx)),
    _ => None,
  }
}
