import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.Commons
import qs.Ui

// Workspaces como quadrados com borda em degradê:
//   ativo   -> borda vermelho -> amarelo e brilho interno vermelho
//   ocupado -> borda branco -> gunmetal
//   vazio   -> borda gunmetal apagada
BarWidget {
  id: root
  moduleName: "xi.workspaces"

  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family

  property var pal: ({})
  function c(name, fallback) { return pal[name] || fallback }
  readonly property color cRed: c("red", "#e63946")
  // "highlight" (opcional no colors.toml) substitui o amarelo da V-fin.
  readonly property color cYellow: c("highlight", c("yellow", "#f2c230"))
  readonly property color cWhite: c("foreground", "#d8dde6")
  readonly property color cMuted: c("muted", "#3a4560")
  readonly property color cDim: c("dark_foreground", "#6b7590")
  readonly property color cBright: c("bright_foreground", "#f4f6fa")
  // workspace_style no colors.toml: "plates" (padrão, Xi) ou "underline"
  // (números soltos, ativo com traço de destaque embaixo, estilo PatternFly).
  readonly property bool underline: pal.workspace_style === "underline"

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
        var st = line.match(/^\s*workspace_style\s*=\s*"([a-z]+)"/)
        if (st) out.workspace_style = st[1]
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
    1: "\uf120", // terminais
    2: "\uf0ac", // navegador
    3: "\uf086", // chat
    4: "\uf121", // IDE
    5: "\uf0e0", // email
    6: "\uf1bc"  // Spotify
  })

  function workspaceLabel(id) {
    return workspaceIcons[id] || (id === 10 ? "0" : String(id))
  }

  function focusWorkspace(id) {
    if (!root.bar) return
    root.bar.run("hyprctl dispatch " + Util.shellQuote("hl.dsp.focus({ workspace = \"" + id + "\" })"))
  }

  readonly property real plateH: Math.round(barSize * 0.68)
  readonly property real plateW: plateH

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

        width: root.underline ? Math.round(root.plateW * 1.05) : root.plateW
        height: root.underline ? root.barSize : root.plateH

        // Estilo "underline": fundo leve no hover e traço no ativo.
        Rectangle {
          visible: root.underline
          anchors.fill: parent
          anchors.topMargin: Math.round(root.barSize * 0.14)
          anchors.bottomMargin: Math.round(root.barSize * 0.14)
          radius: 3
          color: root.cWhite
          opacity: plate.hovered && !plate.focused ? 0.08 : 0
          Behavior on opacity { NumberAnimation { duration: 120 } }
        }
        Rectangle {
          visible: root.underline
          anchors.horizontalCenter: parent.horizontalCenter
          anchors.bottom: parent.bottom
          height: Math.max(2, Math.round(root.barSize * 0.09))
          width: plate.focused ? parent.width - 4 : 0
          color: root.cRed
          Behavior on width { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }
        }

        Canvas {
          id: shape
          visible: !root.underline
          anchors.fill: parent
          property bool f: plate.focused
          property bool o: plate.occupied
          property bool hv: plate.hovered
          onFChanged: requestPaint()
          onOChanged: requestPaint()
          onHvChanged: requestPaint()

          // Ativo: o degradê da borda gira em volta da caixa e o brilho
          // interno pulsa de leve. Parado nos demais para não gastar CPU.
          property real phase: 0
          onPhaseChanged: if (f) requestPaint()
          // Uma volta a cada 7 s, redesenhando a 20 fps.
          Timer {
            running: shape.f && shape.visible
            interval: 50
            repeat: true
            onTriggered: shape.phase = (shape.phase + 50 / 7000) % 1
          }
          Connections {
            target: root
            function onPalChanged() { shape.requestPaint() }
          }

          onPaint: {
            var ctx = getContext("2d")
            var w = width, h = height, lw = f ? 1.6 : 1.2, r = 3
            var x = lw / 2, y = lw / 2, iw = w - lw, ih = h - lw
            ctx.reset()
            ctx.beginPath()
            ctx.moveTo(x + r, y)
            ctx.arcTo(x + iw, y, x + iw, y + ih, r)
            ctx.arcTo(x + iw, y + ih, x, y + ih, r)
            ctx.arcTo(x, y + ih, x, y, r)
            ctx.arcTo(x, y, x + iw, y, r)
            ctx.closePath()

            // Leve brilho interno vindo das bordas no ativo / hover.
            if (f || hv) {
              var tint = f ? root.cRed : root.cWhite
              var pulse = f ? 0.22 + 0.12 * (0.5 + 0.5 * Math.sin(phase * Math.PI * 4)) : 0.12
              var inner = ctx.createRadialGradient(w / 2, h / 2, 0, w / 2, h / 2, w * 0.75)
              inner.addColorStop(0.0, Qt.rgba(0, 0, 0, 0))
              inner.addColorStop(1.0, Qt.rgba(tint.r, tint.g, tint.b, pulse))
              ctx.fillStyle = inner
              ctx.fill()
            }

            // Borda em degradê: vermelho -> amarelo (V-fin) no ativo,
            // branco -> gunmetal no ocupado, gunmetal apagado no vazio.
            var g
            if (f) {
              var a = phase * Math.PI * 2, cx = w / 2, cy = h / 2, rr = w * 0.7
              g = ctx.createLinearGradient(cx - Math.cos(a) * rr, cy - Math.sin(a) * rr,
                                           cx + Math.cos(a) * rr, cy + Math.sin(a) * rr)
              g.addColorStop(0, root.cRed)
              g.addColorStop(0.5, root.cYellow)
              g.addColorStop(1, root.cRed)
            } else if (o) {
              g = ctx.createLinearGradient(0, 0, w, h)
              g.addColorStop(0, root.cWhite)
              g.addColorStop(1, root.cMuted)
            } else {
              g = ctx.createLinearGradient(0, 0, w, h)
              g.addColorStop(0, hv ? root.cDim : root.cMuted)
              g.addColorStop(1, Qt.rgba(root.cMuted.r, root.cMuted.g, root.cMuted.b, 0.35))
            }
            ctx.lineWidth = lw
            ctx.strokeStyle = g
            ctx.stroke()
          }
        }

        Text {
          anchors.centerIn: parent
          readonly property bool isIcon: root.workspaceIcons[plate.modelData] !== undefined
          text: root.workspaceLabel(plate.modelData)
          font.family: root.fontFamily
          font.pixelSize: Math.round(root.plateH * (isIcon ? 0.52 : 0.58))
          font.bold: plate.focused || plate.occupied
          color: root.underline
            ? (plate.focused ? root.cBright : (plate.occupied ? root.cWhite : root.cDim))
            : (plate.focused ? root.cYellow : (plate.occupied ? root.cWhite : root.cDim))
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
