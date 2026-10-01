import QtQuick
import Quickshell.Io
import qs.Ui

BarIndicator {
  id: root

  property string state: "idle"
  property string icon: ""

  active: state === "recording"
  activeText: icon
  inactiveText: "󰍬"
  activeTooltipText: state === "recording" ? "Gravando ditado" : (state === "transcribing" ? "Transcrevendo…" : state)
  inactiveTooltipText: "Ditar"

  function update(raw) {
    var data = extractData(raw)

    state = String(data.alt || data.class || "idle")
    if (state === "recording") icon = "󰍬"
    else if (state === "transcribing") icon = "󰔟"
    else icon = ""
  }

  Process {
    command: ["bash", "-c", "omarchy-voxtype-status"]
    running: true
    stdout: SplitParser {
      onRead: function(data) { root.update(data) }
    }
  }

  onPressed: function() {
    if (!root.bar) return
    root.bar.run("omarchy-voxtype-config")
  }
}
