import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.UPower
import qs.Commons
import qs.Ui

// HUD de cockpit: GEN (uso de CPU), MEM (uso de memória) e BAT (carga da
// bateria, só quando há bateria) em barras de segmentos inclinados, verde ->
// amarelo -> vermelho conforme a carga (na bateria, conforme ela acaba).
BarWidget {
  id: root
  moduleName: "xi.reactor"

  readonly property int intervalSec: Math.max(1, parseInt(setting("intervalSec", 2), 10) || 2)
  readonly property int segments: Math.max(4, Math.min(16, parseInt(setting("segments", 8), 10) || 8))
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family

  property var pal: ({})
  function c(name, fallback) { return pal[name] || fallback }
  readonly property color cRed: c("red", "#e63946")
  readonly property color cYellow: c("yellow", "#f2c230")
  // "highlight" (opcional no colors.toml) substitui o amarelo dos rótulos.
  readonly property color cLabel: c("highlight", cYellow)
  readonly property color cGreen: c("green", "#5fd38d")
  readonly property color cCyan: c("cyan", "#4cc9f0")
  readonly property color cMuted: c("muted", "#3a4560")
  readonly property color cBlue: c("blue", "#4a7bd0")
  readonly property color cText: c("foreground", "#d8dde6")
  readonly property color cDim: c("dark_foreground", "#6b7590")
  // reactor_style no colors.toml: "hud" (padrão, Xi) ou "cockpit" (barras de
  // progresso finas com percentual, como o card de uso do web console do RHEL).
  readonly property bool cockpit: pal.reactor_style === "cockpit"

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
        var st = line.match(/^\s*reactor_style\s*=\s*"([a-z]+)"/)
        if (st) out.reactor_style = st[1]
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

  property real cpu: 0
  property real mem: 0
  property real memUsedGb: 0
  property real memTotalGb: 0
  property var lastCpu: null

  readonly property var battery: UPower.displayDevice
  readonly property bool hasBattery: !!(battery && battery.isPresent)
  readonly property real bat: hasBattery ? Math.max(0, Math.min(1, battery.percentage)) : 0
  readonly property bool batCharging: hasBattery && !UPower.onBattery && bat < 1
  readonly property bool batPlugged: hasBattery && !UPower.onBattery
  // Raio sobre o medidor quando o carregador está conectado.
  readonly property string boltGlyph: String.fromCodePoint(0xF140B)
  readonly property color barBackground: bar && bar.background !== undefined ? bar.background : "#050505"

  component Bolt: Text {
    anchors.centerIn: parent
    text: root.boltGlyph
    color: root.cText
    style: Text.Outline
    styleColor: root.barBackground
    font.family: root.fontFamily
    font.pixelSize: Math.round(root.barSize * 0.46)
  }

  function parse(out) {
    var lines = String(out || "").split("\n")
    var total = 0, avail = 0
    for (var i = 0; i < lines.length; i++) {
      var l = lines[i]
      if (l.indexOf("cpu ") === 0) {
        var n = l.trim().split(/\s+/).slice(1).map(Number)
        var idle = n[3] + (n[4] || 0)
        var sum = n.reduce(function(a, b) { return a + b }, 0)
        if (root.lastCpu) {
          var dt = sum - root.lastCpu.sum, di = idle - root.lastCpu.idle
          if (dt > 0) root.cpu = Math.max(0, Math.min(1, 1 - di / dt))
        }
        root.lastCpu = { sum: sum, idle: idle }
      } else if (l.indexOf("MemTotal:") === 0) {
        total = parseInt(l.split(/\s+/)[1], 10)
      } else if (l.indexOf("MemAvailable:") === 0) {
        avail = parseInt(l.split(/\s+/)[1], 10)
      }
    }
    if (total > 0) {
      root.mem = (total - avail) / total
      root.memUsedGb = (total - avail) / 1048576
      root.memTotalGb = total / 1048576
    }
  }

  Process {
    id: proc
    command: ["sh", "-c", "head -n1 /proc/stat; grep -E '^(MemTotal|MemAvailable):' /proc/meminfo"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.parse(text)
    }
  }

  Timer {
    interval: root.intervalSec * 1000
    running: true
    repeat: true
    triggeredOnStart: true
    onTriggered: if (!proc.running) proc.running = true
  }

  readonly property real segH: Math.round(barSize * 0.46)
  readonly property real segW: Math.max(3, Math.round(segH * 0.42))

  implicitWidth: vertical ? barSize : row.implicitWidth + 12
  implicitHeight: barSize
  visible: !vertical

  component Gauge: Row {
    id: gauge
    property string label: ""
    property real value: 0
    // Bateria: a cor acompanha o nível (vermelho quando está acabando).
    property bool inverse: false
    property bool plugged: false
    spacing: 4

    Text {
      anchors.verticalCenter: parent.verticalCenter
      text: gauge.label
      color: root.cLabel
      font.family: root.fontFamily
      font.pixelSize: Math.round(root.barSize * 0.36)
      font.bold: true
      font.letterSpacing: 1
    }

    Canvas {
      id: bars
      anchors.verticalCenter: parent.verticalCenter
      width: root.segments * (root.segW + 2) + root.segW * 0.6
      height: root.segH
      property real v: gauge.value
      property bool inv: gauge.inverse
      onVChanged: requestPaint()

      Bolt { visible: gauge.plugged }
      Connections {
        target: root
        function onPalChanged() { bars.requestPaint() }
      }

      onPaint: {
        var ctx = getContext("2d")
        var n = root.segments, sw = root.segW, h = height, sk = sw * 0.6
        var lit = Math.round(v * n)
        ctx.reset()
        for (var i = 0; i < n; i++) {
          var x = i * (sw + 2)
          var frac = (i + 1) / n
          var on = inv ? (v <= 0.15 ? root.cRed : (v <= 0.3 ? root.cYellow : root.cGreen))
                       : (frac > 0.85 ? root.cRed : (frac > 0.6 ? root.cYellow : root.cGreen))
          var col = i < lit ? on : root.cMuted
          ctx.globalAlpha = i < lit ? 1 : 0.55
          ctx.fillStyle = col
          ctx.beginPath()
          ctx.moveTo(x + sk, 0)
          ctx.lineTo(x + sk + sw, 0)
          ctx.lineTo(x + sw, h)
          ctx.lineTo(x, h)
          ctx.closePath()
          ctx.fill()
        }
      }
    }
  }

  // Barra de progresso PatternFly: trilho apagado, preenchimento (azul ou
  // usage_fill do colors.toml) que
  // vira dourado (>75%) e vermelho (>90%), percentual à direita.
  component Usage: Row {
    id: usage
    property string label: ""
    property real value: 0
    // Bateria: alerta quando o valor está baixo, não alto.
    property bool inverse: false
    property bool plugged: false
    readonly property real load: inverse ? 1 - value : value
    // Com usage_fill (ex.: vermelho), só o alerta dourado acima de 75%.
    readonly property color fill: root.pal.usage_fill
      ? (load > 0.75 ? root.cYellow : root.pal.usage_fill)
      : (load > 0.9 ? root.cRed : (load > 0.75 ? root.cYellow : root.cBlue))
    spacing: 6

    Text {
      anchors.verticalCenter: parent.verticalCenter
      text: usage.label
      color: root.cDim
      font.family: root.fontFamily
      font.pixelSize: Math.round(root.barSize * 0.38)
    }

    Rectangle {
      anchors.verticalCenter: parent.verticalCenter
      width: Math.round(root.barSize * 1.52)
      height: Math.max(3, Math.round(root.barSize * 0.16))
      radius: height / 2
      color: Qt.rgba(root.cMuted.r, root.cMuted.g, root.cMuted.b, 0.7)

      Rectangle {
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        height: parent.height
        radius: parent.radius
        width: Math.max(height, parent.width * usage.value)
        color: usage.fill
        Behavior on width { NumberAnimation { duration: 400; easing.type: Easing.OutCubic } }
        Behavior on color { ColorAnimation { duration: 300 } }
      }

      Bolt { visible: usage.plugged }
    }

    Text {
      anchors.verticalCenter: parent.verticalCenter
      width: pctMetrics.width
      horizontalAlignment: Text.AlignLeft
      text: Math.round(usage.value * 100) + "%"
      color: root.cText
      font.family: root.fontFamily
      font.pixelSize: Math.round(root.barSize * 0.38)
      TextMetrics {
        id: pctMetrics
        font.family: root.fontFamily
        font.pixelSize: Math.round(root.barSize * 0.38)
        text: "100%"
      }
    }
  }

  Row {
    id: row
    anchors.centerIn: parent
    spacing: root.cockpit ? 14 : 10

    Gauge { visible: !root.cockpit; label: "GEN"; value: root.cpu }
    Gauge { visible: !root.cockpit; label: "MEM"; value: root.mem }
    Usage { visible: root.cockpit; label: "CPU"; value: root.cpu }
    Usage { visible: root.cockpit; label: "Mem"; value: root.mem }
    Gauge { visible: !root.cockpit && root.hasBattery; label: "BAT"; value: root.bat; inverse: true; plugged: root.batPlugged }
    Usage { visible: root.cockpit && root.hasBattery; label: "Bat"; value: root.bat; inverse: true; plugged: root.batPlugged }
  }

  MouseArea {
    id: mouse
    anchors.fill: parent
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor
    onClicked: if (root.bar) root.bar.run("xdg-terminal-exec btop")
    onContainsMouseChanged: {
      if (!root.bar) return
      if (containsMouse)
        root.bar.showTooltip(root, (root.cockpit ? "CPU: " : "Gerador (CPU): ") + Math.round(root.cpu * 100) + "%\n"
          + (root.cockpit ? "Memória: " : "Carga (memória): ") + root.memUsedGb.toFixed(1) + " / " + root.memTotalGb.toFixed(1) + " GiB"
          + (root.hasBattery ? "\nBateria: " + Math.round(root.bat * 100) + "%" + (root.batCharging ? " (carregando)" : (UPower.onBattery ? "" : " (na tomada)")) : ""))
      else root.bar.hideTooltip(root)
    }
  }
}
