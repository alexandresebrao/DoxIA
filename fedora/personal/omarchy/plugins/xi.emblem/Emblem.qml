import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

// Botão do menu Omarchy desenhado como a cabeça do Xi: V-fin amarela com a
// câmera verde no vértice, seguida do nome.
BarWidget {
  id: root
  moduleName: "xi.emblem"

  readonly property bool showLabel: setting("showLabel", false) === true
  readonly property string label: String(setting("label", "Alexandre"))
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family
  readonly property color foreground: bar ? bar.barForeground : Color.foreground

  // Paleta do tema atual (colors.toml), com as cores do Xi como fallback.
  property var pal: ({})
  function c(name, fallback) { return pal[name] || fallback }
  readonly property color cYellow: c("highlight", c("yellow", "#f2c230"))
  readonly property color cGreen: c("bright_green", "#7ef0a8")

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
      // Recarrega o emblem.png do tema (o diretório foi trocado).
      themeEmblem.source = ""
      themeEmblem.source = root.emblemPath
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

  // Um tema pode trazer emblem.png; sem ele, desenha a V-fin do Xi.
  readonly property string emblemPath: "file://" + Quickshell.env("HOME") + "/.local/state/omarchy/current/theme/emblem.png"

  readonly property real unit: Math.max(14, barSize - 8)
  // Tamanho do emblem.png em relação à altura da V-fin.
  readonly property real emblemScale: 0.78
  implicitWidth: vertical ? barSize : row.implicitWidth + 14
  implicitHeight: barSize

  Row {
    id: row
    anchors.centerIn: parent
    spacing: 5

    Image {
      id: themeEmblem
      visible: status === Image.Ready
      source: root.emblemPath
      cache: false
      height: Math.round(root.unit * root.emblemScale)
      width: visible ? implicitWidth * height / Math.max(1, implicitHeight) : 0
      sourceSize.height: root.unit * 2
      fillMode: Image.PreserveAspectFit
      smooth: true
      mipmap: true
      anchors.verticalCenter: parent.verticalCenter
      opacity: mouse.containsMouse ? 1 : 0.9
      Behavior on opacity { NumberAnimation { duration: 160 } }
    }

    Canvas {
      id: art
      visible: themeEmblem.status !== Image.Ready
      width: visible ? root.unit * 1.05 : 0
      height: root.unit
      anchors.verticalCenter: parent.verticalCenter
      property real glow: mouse.containsMouse ? 1 : 0
      Behavior on glow { NumberAnimation { duration: 160 } }
      onGlowChanged: requestPaint()
      Connections {
        target: root
        function onPalChanged() { art.requestPaint() }
      }

      onPaint: {
        var ctx = getContext("2d")
        var h = height
        ctx.reset()

        // V-fin
        var fw = h * 1.05, t = h * 0.17
        ctx.shadowColor = root.cYellow
        ctx.shadowBlur = 2 + glow * 6
        ctx.fillStyle = root.cYellow
        ctx.beginPath()
        ctx.moveTo(0, h * 0.08)
        ctx.lineTo(t * 1.3, h * 0.08)
        ctx.lineTo(fw / 2, h * 0.70)
        ctx.lineTo(fw - t * 1.3, h * 0.08)
        ctx.lineTo(fw, h * 0.08)
        ctx.lineTo(fw / 2, h * 0.95)
        ctx.closePath()
        ctx.fill()

        // câmera
        ctx.shadowColor = root.cGreen
        ctx.shadowBlur = 3 + glow * 6
        ctx.fillStyle = root.cGreen
        ctx.beginPath()
        ctx.ellipse(fw / 2 - h * 0.11, h * 0.40, h * 0.22, h * 0.13)
        ctx.fill()
      }
    }

    Text {
      visible: root.showLabel && !root.vertical
      anchors.verticalCenter: parent.verticalCenter
      text: root.label
      color: mouse.containsMouse ? root.cYellow : root.foreground
      opacity: mouse.containsMouse ? 1 : 0.75
      font.family: root.fontFamily
      font.pixelSize: Math.round(root.barSize * 0.42)
      font.bold: true
      font.letterSpacing: 1
      Behavior on color { ColorAnimation { duration: 160 } }
    }
  }

  MouseArea {
    id: mouse
    anchors.fill: parent
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor
    acceptedButtons: Qt.LeftButton | Qt.RightButton
    onClicked: function(event) {
      if (!root.bar) return
      if (event.button === Qt.RightButton) root.bar.run("xdg-terminal-exec")
      else root.bar.run("omarchy-shell shell toggle omarchy.menu '{\"menu\":\"root\"}'")
    }
    onContainsMouseChanged: {
      if (!root.bar) return
      if (containsMouse) root.bar.showTooltip(root, "Menu")
      else root.bar.hideTooltip(root)
    }
  }
}
