import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.UPower
import qs.Commons
import qs.Ui

// Uso de CPU, memória e bateria em ícones que enchem: um chip com barra
// vertical (CPU), um pente de memória e um raio (bateria, só descarregando ou
// abaixo de 100%, todo preenchido quando carregando). Preenchimento na cor de
// destaque do tema e dourado acima de 75%; o percentual fica no tooltip.
BarWidget {
  id: root
  moduleName: "doxia.usage"

  readonly property int intervalSec: Math.max(1, parseInt(setting("intervalSec", 2), 10) || 2)
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family

  // Paleta do tema atual (colors.toml).
  property var pal: ({})
  function c(name, fallback) { return pal[name] || fallback }
  readonly property color cFill: c("accent", "#ee0000")
  readonly property color cYellow: c("yellow", "#f2c230")
  readonly property color cRed: c("red", "#e5484d")
  readonly property color cText: c("foreground", "#d8dde6")

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

  // Ícones do tamanho dos outros ícones da barra.
  readonly property real unit: Style.bar.iconCanvas
  readonly property color cStroke: bar && bar.barForeground ? bar.barForeground : cText
  readonly property bool showBattery: hasBattery && (UPower.onBattery || bat < 0.995)

  implicitWidth: vertical ? barSize : row.implicitWidth + 12
  implicitHeight: barSize
  visible: !vertical

  // Um ícone desenhado com preenchimento proporcional a `value` (0–1).
  // `shape` escolhe o desenho: "cpu" (chip com barra vertical), "mem" (pente
  // de memória enchendo da esquerda) e "bolt" (raio enchendo de baixo, todo
  // preenchido quando carregando).
  component Gauge: Canvas {
    id: gauge
    property string shape: "cpu"
    property real value: 0
    property real shown: value
    property bool solid: false
    property color fill: root.cFill
    Behavior on shown { NumberAnimation { duration: 400; easing.type: Easing.OutCubic } }
    Behavior on fill { ColorAnimation { duration: 300 } }

    anchors.verticalCenter: parent ? parent.verticalCenter : undefined
    width: Math.round(root.unit * (shape === "mem" ? 1.35 : (shape === "bolt" ? 0.8 : 1)))
    height: Math.round(root.unit)
    antialiasing: true

    onShownChanged: requestPaint()
    onFillChanged: requestPaint()
    onSolidChanged: requestPaint()
    Connections {
      target: root
      function onCStrokeChanged() { gauge.requestPaint() }
    }

    function rrect(ctx, x, y, w, h, r) {
      ctx.beginPath()
      ctx.moveTo(x + r, y)
      ctx.arcTo(x + w, y, x + w, y + h, r)
      ctx.arcTo(x + w, y + h, x, y + h, r)
      ctx.arcTo(x, y + h, x, y, r)
      ctx.arcTo(x, y, x + w, y, r)
      ctx.closePath()
    }

    onPaint: {
      var ctx = getContext("2d")
      var w = width, h = height
      var lw = Math.max(1, h / 12)
      var v = Math.max(0, Math.min(1, shown))
      ctx.reset()
      ctx.lineWidth = lw
      ctx.lineCap = "round"
      ctx.lineJoin = "round"
      ctx.strokeStyle = root.cStroke
      ctx.fillStyle = fill

      if (shape === "cpu") {
        var b = h * 0.2, s = h - 2 * b
        rrect(ctx, b, b, s, s, h * 0.12)
        ctx.stroke()
        ctx.beginPath()
        for (var i = 0; i < 3; i++) {
          var p = b + s * (0.25 + i * 0.25)
          ctx.moveTo(p, b); ctx.lineTo(p, b * 0.35)
          ctx.moveTo(p, h - b); ctx.lineTo(p, h - b * 0.35)
          ctx.moveTo(b, p); ctx.lineTo(b * 0.35, p)
          ctx.moveTo(w - b, p); ctx.lineTo(w - b * 0.35, p)
        }
        ctx.stroke()
        // Barra vertical dentro do chip, enchendo de baixo.
        var bw = s * 0.36, bh = s - 2 * lw - s * 0.24
        var bx = (w - bw) / 2, by = b + lw + s * 0.12
        ctx.globalAlpha = 0.18
        ctx.fillStyle = root.cStroke
        rrect(ctx, bx, by, bw, bh, bw * 0.3); ctx.fill()
        ctx.globalAlpha = 1
        ctx.fillStyle = fill
        var fh = Math.max(bw * 0.6, bh * v)
        rrect(ctx, bx, by + bh - fh, bw, fh, bw * 0.3); ctx.fill()
      } else if (shape === "mem") {
        var top = h * 0.24, bot = h * 0.66
        rrect(ctx, lw / 2, top, w - lw, bot - top, h * 0.08)
        ctx.stroke()
        // Contatos embaixo, com o chanfro fora do centro como num pente real.
        ctx.beginPath()
        var n = 6
        for (var k = 0; k < n; k++) {
          if (k === 2) continue
          var px = lw + (w - 2 * lw) * (k + 0.5) / n
          ctx.moveTo(px, bot); ctx.lineTo(px, h * 0.82)
        }
        ctx.stroke()
        var ix = lw * 1.8, iy = top + lw * 1.8
        var iw = w - 2 * ix, ih = bot - top - 3.6 * lw
        ctx.globalAlpha = 0.18
        ctx.fillStyle = root.cStroke
        rrect(ctx, ix, iy, iw, ih, ih * 0.3); ctx.fill()
        ctx.globalAlpha = 1
        ctx.fillStyle = fill
        rrect(ctx, ix, iy, Math.max(ih * 0.8, iw * v), ih, ih * 0.3); ctx.fill()
      } else {
        // Raio (Lucide zap) numa grade 18×20.
        var pts = [[10, 0], [0, 12], [9, 12], [8, 20], [18, 8], [9, 8]]
        var sx = (w - lw * 2) / 18, sy = (h - lw * 2) / 20
        ctx.beginPath()
        for (var j = 0; j < pts.length; j++) {
          var X = lw + pts[j][0] * sx, Y = lw + pts[j][1] * sy
          if (j === 0) ctx.moveTo(X, Y); else ctx.lineTo(X, Y)
        }
        ctx.closePath()
        ctx.save()
        ctx.clip()
        var level = solid ? 1 : v
        ctx.fillRect(0, h - h * level, w, h * level)
        ctx.restore()
        ctx.strokeStyle = solid ? fill : root.cStroke
        ctx.stroke()
      }
    }
  }

  Row {
    id: row
    anchors.centerIn: parent
    height: parent.height
    spacing: Style.space(10)

    Gauge { shape: "cpu"; value: root.cpu; fill: root.cpu > 0.75 ? root.cYellow : root.cFill }
    Gauge { shape: "mem"; value: root.mem; fill: root.mem > 0.75 ? root.cYellow : root.cFill }
    // Só aparece descarregando ou abaixo de 100%; preenchido quando carregando.
    Gauge {
      visible: root.showBattery
      shape: "bolt"
      value: root.bat
      solid: root.batPlugged
      fill: root.batPlugged ? root.cFill : (root.bat < 0.2 ? root.cRed : (root.bat < 0.35 ? root.cYellow : root.cFill))
    }
  }

  MouseArea {
    anchors.fill: parent
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor
    onClicked: if (root.bar) root.bar.run("xdg-terminal-exec btop")
    onContainsMouseChanged: {
      if (!root.bar) return
      if (containsMouse)
        root.bar.showTooltip(root, "CPU: " + Math.round(root.cpu * 100) + "%\n"
          + "Memória: " + root.memUsedGb.toFixed(1) + " / " + root.memTotalGb.toFixed(1) + " GiB"
          + (root.hasBattery ? "\nBateria: " + Math.round(root.bat * 100) + "%" + (root.batCharging ? " (carregando)" : (UPower.onBattery ? "" : " (na tomada)")) : ""))
      else root.bar.hideTooltip(root)
    }
  }
}
