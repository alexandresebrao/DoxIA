import QtQuick
import qs.Commons
import qs.Ui

// Several first-party panel widgets packed into one bar icon. Members keep
// their own popups (their bar buttons are hidden); KeyboardPanel sees
// `tabGroup` on the owner and draws a tab strip above the content, so the
// popups read as tabs of a single panel anchored under the group icon.
//
//   { "id": "omarchy.group", "members": ["omarchy.power", "omarchy.monitor",
//     { "id": "omarchy.audio", "someSetting": true }],
//     "titles": ["Bateria", "Tela", "Som"],
//     "icon": "omarchy.audio",    // member id (its live glyph) or a glyph
//     "wheel": "omarchy.audio",   // member that receives scroll on the icon
//     "rightClick": "omarchy.audio" }  // member that receives right-click
Item {
  id: root

  property QtObject bar: null
  property string moduleName: "omarchy.group"
  property var settings: ({})

  readonly property bool isTabGroup: true
  readonly property bool vertical: bar ? bar.vertical === true : false

  // Member id → panel directory next to this one.
  readonly property var knownMembers: ({
    "omarchy.audio": { dir: "audio", title: "Som" },
    "omarchy.bluetooth": { dir: "bluetooth", title: "Bluetooth" },
    "omarchy.monitor": { dir: "monitor", title: "Tela" },
    "omarchy.network": { dir: "network", title: "Wi-Fi" },
    "omarchy.power": { dir: "power", title: "Bateria" },
    "omarchy.tailscale": { dir: "tailscale", title: "Tailscale" }
  })

  readonly property var memberEntries: {
    // Settings arrive as Qt sequences, not JS arrays: index by length.
    var raw = settings && settings.members ? settings.members : []
    var titles = settings && settings.titles ? settings.titles : []
    var out = []
    for (var i = 0; i < raw.length; i++) {
      var entry = typeof raw[i] === "string" ? { id: raw[i] } : raw[i]
      if (!entry || !knownMembers[entry.id]) continue
      var memberSettings = {}
      for (var k in entry) if (k !== "id") memberSettings[k] = entry[k]
      out.push({
        id: entry.id,
        title: titles[out.length] || knownMembers[entry.id].title,
        source: Qt.resolvedUrl("../" + knownMembers[entry.id].dir + "/Panel.qml"),
        settings: memberSettings
      })
    }
    return out
  }

  // Loaded member panels in layout order (only the visible ones count as tabs).
  property var members: []
  property int lastIndex: 0

  readonly property var tabs: members.filter(function(m) { return m && m.visible && m.implicitWidth > 0 })
  readonly property bool opened: {
    for (var i = 0; i < members.length; i++) if (members[i] && members[i].opened) return true
    return false
  }

  function rebuildMembers() {
    var next = []
    for (var i = 0; i < repeater.count; i++) {
      var loader = repeater.itemAt(i)
      if (loader && loader.item) next.push(loader.item)
    }
    members = next
  }

  function indexOfTab(member) { return tabs.indexOf(member) }

  function memberById(id) {
    for (var i = 0; i < members.length; i++)
      if (memberEntries[i] && memberEntries[i].id === id) return members[i]
    return null
  }

  function buttonOf(member) {
    if (!member) return null
    for (var i = 0; i < member.children.length; i++) {
      var c = member.children[i]
      if (c && typeof c.triggerPress === "function" && c.text !== undefined) return c
    }
    return null
  }

  function titleOf(member) {
    var i = members.indexOf(member)
    return i >= 0 && memberEntries[i] ? memberEntries[i].title : ""
  }

  // The member's own bar glyph (its WidgetButton child), so tabs show live
  // battery/volume/signal icons.
  function glyphOf(member) {
    var c = buttonOf(member)
    if (!c) return ""
    var t = String(c.text)
    var sp = t.lastIndexOf(" ")
    return sp >= 0 ? t.substr(sp + 1) : t
  }

  readonly property string iconSetting: settings && settings.icon ? String(settings.icon) : ""
  readonly property string groupGlyph: {
    if (iconSetting && !knownMembers[iconSetting]) return iconSetting
    var m = iconSetting ? memberById(iconSetting) : tabs[0]
    return glyphOf(m)
  }

  function selectTab(index) {
    var t = tabs
    if (index < 0 || index >= t.length) return
    lastIndex = index
    if (!t[index].opened) t[index].open()
  }

  // Tab/Shift+Tab inside a member popup: walk the tabs, then hand off to the
  // neighbouring bar panel once past either end.
  function cycle(from, direction) {
    var t = tabs
    var i = t.indexOf(from)
    var next = i + (direction < 0 ? -1 : 1)
    if (i >= 0 && next >= 0 && next < t.length) {
      selectTab(next)
      return true
    }
    if (bar && typeof bar.switchPanelFrom === "function" && bar.switchPanelFrom(root, direction)) return true
    selectTab((next + t.length) % t.length)
    return true
  }

  function noteOpened(member) {
    var i = tabs.indexOf(member)
    if (i >= 0) lastIndex = i
  }

  // Panel-like API so the bar's Tab navigation can land on the group.
  function open() { selectTab(Math.min(lastIndex, tabs.length - 1)) }
  function close() {
    for (var i = 0; i < members.length; i++) if (members[i] && members[i].opened) members[i].close()
  }
  function toggle() { opened ? close() : open() }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  // Members fill the group invisibly: only their popups and IPC are used.
  Repeater {
    id: repeater
    model: root.memberEntries

    Loader {
      required property var modelData
      required property int index
      anchors.fill: parent
      source: modelData.source
      onLoaded: {
        item.tabGroup = root
        item.bar = Qt.binding(function() { return root.bar })
        item.settings = modelData.settings
        var b = root.buttonOf(item)
        if (b) { b.visible = false; b.enabled = false }
        root.rebuildMembers()
      }
    }
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.groupGlyph
    tooltipText: ""
    onPressed: function(b) {
      var target = b === Qt.RightButton && root.settings ? root.buttonOf(root.memberById(root.settings.rightClick)) : null
      if (target) target.pressed(b)
      else if (b === Qt.LeftButton) root.toggle()
    }
    onWheelMoved: function(delta) {
      var target = root.settings ? root.buttonOf(root.memberById(root.settings.wheel)) : null
      if (target) target.wheelMoved(delta)
    }
  }

  Connections {
    target: root
    function onMemberEntriesChanged() { Qt.callLater(root.rebuildMembers) }
  }
}
