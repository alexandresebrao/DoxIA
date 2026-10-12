import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.Commons
import qs.Ui

// Workspaces soltos, com ícone nos espaços fixos: fundo leve no hover e uma
// pílula de vidro na cor de destaque do tema que desliza até o ativo.
BarWidget {
  id: root
  moduleName: "doxia.workspaces"

  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family

  // Paleta do tema atual (colors.toml).
  property var pal: ({})
  function c(name, fallback) { return pal[name] || fallback }
  readonly property color cAccent: c("accent", c("red", "#ee0000"))
  readonly property color cWhite: bar && bar.barForeground ? bar.barForeground : c("foreground", "#d8dde6")
  readonly property color cDim: Qt.rgba(cWhite.r, cWhite.g, cWhite.b, 0.55)
  readonly property color cBright: c("bright_foreground", "#f4f6fa")

  FileView {
    id: paletteFile
    path: Quickshell.env("HOME") + "/.local/state/omarchy/current/theme/colors.toml"
    watchChanges: true
    onFileChanged: reload()
    // O tema é trocado substituindo o diretório; tenta de novo logo depois.
    onLoadFailed: paletteRetry.restart()
    onLoaded: {
      var out = {}
      String(text()).split("\n").forEach(function(line) {
        var m = line.match(/^\s*([a-z_]+)\s*=\s*"(#[0-9a-fA-F]{6})"/)
        if (m) out[m[1]] = m[2]
      })
      root.pal = out
    }
  }
  Timer {
    id: paletteRetry
    interval: 1000
    onTriggered: paletteFile.reload()
  }
  Connections {
    target: Color
    function onAccentChanged() { paletteFile.reload() }
  }

  function workspaceById(id) {
    var values = Hyprland.workspaces.values
    for (var i = 0; i < values.length; i++) if (values[i].id === id) return values[i]
    return null
  }

  function workspaceIds() {
    var ids = [1, 2, 3, 4, 5, 6]
    var values = Hyprland.workspaces.values
    for (var i = 0; i < values.length; i++) {
      var id = values[i].id
      if (id > 0 && id <= 10 && ids.indexOf(id) === -1) ids.push(id)
    }
    ids.sort(function(a, b) { return a - b })
    return ids
  }

  // Ícones (Nerd Font) dos espaços fixos; os demais mostram o número.
  readonly property var workspaceIcons: ({
    1: "", // terminais
    2: "", // navegador
    3: "", // chat
    4: "", // IDE
    5: "", // email
    6: ""  // Spotify
  })

  function workspaceLabel(id) {
    return workspaceIcons[id] || (id === 10 ? "0" : String(id))
  }

  function focusWorkspace(id) {
    if (!root.bar) return
    root.bar.run("hyprctl dispatch " + Util.shellQuote("hl.dsp.focus({ workspace = \"" + id + "\" })"))
  }

  readonly property real plateH: Math.round(barSize * 0.82)

  implicitWidth: vertical ? barSize : flow.implicitWidth + 8
  implicitHeight: vertical ? flow.implicitHeight + 8 : barSize

  readonly property int focusedIndex: {
    var ids = workspaceIds()
    return Hyprland.focusedWorkspace ? ids.indexOf(Hyprland.focusedWorkspace.id) : -1
  }
  readonly property Item focusedPlate: focusedIndex >= 0 ? plates.itemAt(focusedIndex) : null

  // Pílula do ativo: vidro liso tingido com o destaque e borda fina.
  Rectangle {
    id: pill
    visible: root.focusedPlate !== null
    x: root.focusedPlate ? flow.x + root.focusedPlate.x : 0
    y: root.focusedPlate ? flow.y + root.focusedPlate.y + (root.focusedPlate.height - height) / 2 : 0
    width: root.focusedPlate ? root.focusedPlate.width : 0
    height: root.plateH
    radius: height / 2
    border.width: 1
    border.color: Qt.rgba(root.cAccent.r, root.cAccent.g, root.cAccent.b, 0.3)
    color: Qt.rgba(root.cAccent.r, root.cAccent.g, root.cAccent.b, 0.18)
    Behavior on x { NumberAnimation { duration: 220; easing.type: Easing.OutCubic } }
    Behavior on y { NumberAnimation { duration: 220; easing.type: Easing.OutCubic } }
  }

  Grid {
    id: flow
    anchors.centerIn: parent
    columns: root.vertical ? 1 : 0
    rows: root.vertical ? 0 : 1
    spacing: 4

    Repeater {
      id: plates
      model: root.workspaceIds()

      Item {
        id: plate
        required property int modelData
        readonly property var workspace: root.workspaceById(modelData)
        readonly property bool occupied: workspace !== null && workspace.toplevels.values.length > 0
        readonly property bool focused: Hyprland.focusedWorkspace !== null && Hyprland.focusedWorkspace.id === modelData
        readonly property bool hovered: area.containsMouse

        width: Math.round(root.plateH * 1.05)
        height: root.barSize

        Rectangle {
          anchors.centerIn: parent
          width: parent.width
          height: root.plateH
          radius: height / 2
          color: root.cWhite
          opacity: plate.hovered && !plate.focused ? 0.1 : 0
          Behavior on opacity { NumberAnimation { duration: 120 } }
        }

        Text {
          id: label
          anchors.centerIn: parent
          readonly property bool isIcon: root.workspaceIcons[plate.modelData] !== undefined
          text: root.workspaceLabel(plate.modelData)
          font.family: root.fontFamily
          font.pixelSize: isIcon ? Style.bar.iconFont : Math.round(Style.bar.iconFont * 1.1)
          font.weight: plate.focused || plate.occupied ? Font.Medium : Font.Normal
          renderType: Text.CurveRendering
          color: plate.focused ? root.cAccent : (plate.occupied ? root.cWhite : root.cDim)
        }
        SupersampledText { sourceItem: label }

        MouseArea {
          id: area
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: root.focusWorkspace(plate.modelData)
        }
      }
    }
  }

  WheelHandler {
    onWheel: function(event) {
      root.focusWorkspace(event.angleDelta.y > 0 ? "e-1" : "e+1")
    }
  }
}
