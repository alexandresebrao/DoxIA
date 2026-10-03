import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.Commons
import qs.Ui

// Workspaces soltos, com ícone nos espaços fixos: fundo leve no hover e traço
// vermelho embaixo do ativo (estilo PatternFly, como o tema RHEL 8).
BarWidget {
  id: root
  moduleName: "doxia.workspaces"

  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family

  // Paleta do tema atual (colors.toml).
  property var pal: ({})
  function c(name, fallback) { return pal[name] || fallback }
  readonly property color cRed: c("red", "#ee0000")
  readonly property color cWhite: c("foreground", "#d8dde6")
  readonly property color cDim: c("dark_foreground", "#6b7590")
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

  readonly property real plateH: Math.round(barSize * 0.68)

  implicitWidth: vertical ? barSize : flow.implicitWidth + 8
  implicitHeight: vertical ? flow.implicitHeight + 8 : barSize

  Grid {
    id: flow
    anchors.centerIn: parent
    columns: root.vertical ? 1 : 0
    rows: root.vertical ? 0 : 1
    spacing: 4

    Repeater {
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
          anchors.fill: parent
          anchors.topMargin: Math.round(root.barSize * 0.14)
          anchors.bottomMargin: Math.round(root.barSize * 0.14)
          radius: 3
          color: root.cWhite
          opacity: plate.hovered && !plate.focused ? 0.08 : 0
          Behavior on opacity { NumberAnimation { duration: 120 } }
        }
        Rectangle {
          anchors.horizontalCenter: parent.horizontalCenter
          anchors.bottom: parent.bottom
          height: Math.max(2, Math.round(root.barSize * 0.09))
          width: plate.focused ? parent.width - 4 : 0
          color: root.cRed
          Behavior on width { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }
        }

        Text {
          anchors.centerIn: parent
          readonly property bool isIcon: root.workspaceIcons[plate.modelData] !== undefined
          text: root.workspaceLabel(plate.modelData)
          font.family: root.fontFamily
          font.pixelSize: Math.round(root.plateH * (isIcon ? 0.52 : 0.58))
          font.bold: plate.focused || plate.occupied
          color: plate.focused ? root.cBright : (plate.occupied ? root.cWhite : root.cDim)
        }

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
