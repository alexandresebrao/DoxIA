import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.UPower
import qs.Commons
import qs.Ui

// Uso de CPU, memória e bateria (só quando há bateria) em barras de progresso
// finas com percentual, como o card de uso do web console do RHEL: trilho
// apagado, preenchimento na cor de destaque do tema e dourado acima de 75%.
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
  readonly property color cMuted: c("muted", "#3a4560")
  readonly property color cText: c("foreground", "#d8dde6")
  readonly property color cDim: c("dark_foreground", "#6b7590")

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
  // Raio sobre a barra da bateria quando o carregador está conectado.
  readonly property string boltGlyph: String.fromCodePoint(0xF140B)
  readonly property color barBackground: bar && bar.background !== undefined ? bar.background : "#050505"

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

  implicitWidth: vertical ? barSize : row.implicitWidth + 12
  implicitHeight: barSize
  visible: !vertical

  component Usage: Row {
    id: usage
    property string label: ""
    property real value: 0
    // Bateria: alerta quando o valor está baixo, não alto.
    property bool inverse: false
    property bool plugged: false
    readonly property real load: inverse ? 1 - value : value
    readonly property color fill: load > 0.75 ? root.cYellow : root.cFill
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

      Text {
        visible: usage.plugged
        anchors.centerIn: parent
        text: root.boltGlyph
        color: root.cText
        style: Text.Outline
        styleColor: root.barBackground
        font.family: root.fontFamily
        font.pixelSize: Math.round(root.barSize * 0.46)
      }
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
    spacing: 14

    Usage { label: "CPU"; value: root.cpu }
    Usage { label: "Mem"; value: root.mem }
    Usage { visible: root.hasBattery; label: "Bat"; value: root.bat; inverse: true; plugged: root.batPlugged }
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
