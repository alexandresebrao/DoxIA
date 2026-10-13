pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// What the shell needs from mango, read off its IPC (mmsg). Each watch streams
// one JSON object per line: the current state first, then every change. A
// watch that ends (mango restarting, the socket not up yet) is started again
// after a short pause, so the shell recovers without a restart of its own.
QtObject {
  id: root

  // Tags per output: { "Virtual-1": [{ index, is_active, is_urgent, client_count, layout }] }
  property var tags: ({})
  // The output holding the focused window, or under the pointer when no window
  // has focus. Empty until mango reports one.
  property string focusedMonitor: ""
  // The active keymap's description, e.g. "Portuguese (Brazil)".
  property string keyboardLayout: ""

  // A monitor's tags, or an empty list for one mango hasn't reported.
  function tagsFor(monitor) {
    return root.tags[String(monitor || "")] || []
  }

  // Run a mango function: dispatch("view", 3), dispatch("focusid", "client,4").
  function dispatch(func) {
    var call = [String(func)]
    for (var i = 1; i < arguments.length; i++) call.push(String(arguments[i]))
    Quickshell.execDetached(["mmsg", "dispatch", call.join(",")])
  }

  function parse(line) {
    try {
      return JSON.parse(String(line || ""))
    } catch (e) {
      return null
    }
  }

  function applyTags(json) {
    if (!json || !Array.isArray(json.all_tags)) return
    var next = {}
    for (var i = 0; i < json.all_tags.length; i++) {
      var entry = json.all_tags[i]
      next[String(entry.monitor)] = entry.tags || []
    }
    root.tags = next
  }

  function applyFocusedClient(json) {
    if (json && json.monitor) {
      root.focusedMonitor = String(json.monitor)
      return
    }
    // No focused window: ask which output the pointer is on.
    cursorProc.running = true
  }

  property Process tagsWatch: Process {
    command: ["mmsg", "watch", "all-tags"]
    running: true
    stdout: SplitParser { onRead: function(line) { root.applyTags(root.parse(line)) } }
    onExited: restartTimer.start()
  }

  property Process focusWatch: Process {
    command: ["mmsg", "watch", "focusing-client"]
    running: true
    stdout: SplitParser { onRead: function(line) { root.applyFocusedClient(root.parse(line)) } }
    onExited: restartTimer.start()
  }

  property Process layoutWatch: Process {
    command: ["mmsg", "watch", "keyboardlayout"]
    running: true
    stdout: SplitParser {
      onRead: function(line) {
        var json = root.parse(line)
        if (json && json.layout !== undefined) root.keyboardLayout = String(json.layout)
      }
    }
    onExited: restartTimer.start()
  }

  property Process cursorProc: Process {
    command: ["mmsg", "get", "cursorpos"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var json = root.parse(text)
        if (json && json.monitor) root.focusedMonitor = String(json.monitor)
      }
    }
  }

  property Timer restartTimer: Timer {
    interval: 2000
    onTriggered: {
      if (!root.tagsWatch.running) root.tagsWatch.running = true
      if (!root.focusWatch.running) root.focusWatch.running = true
      if (!root.layoutWatch.running) root.layoutWatch.running = true
    }
  }
}
