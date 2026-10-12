import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.UPower
import qs.Commons
import qs.Ui

// Uso de CPU, memória e bateria: ícone (chip, memória, raio) com uma barra
// vertical à direita que enche com a porcentagem. A bateria só aparece
// descarregando ou abaixo de 100%, com o raio preenchido quando carregando.
// Preenchimento na cor de destaque do tema e dourado acima de 75%; o percentual
// exato fica no tooltip de cada medidor.
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

  // Ícone que identifica o medidor e, à direita, uma barra vertical que enche
  // de baixo com a porcentagem.
  component Meter: Row {
    id: meter
    property string icon: ""
    property real value: 0
    property color fill: root.cFill
    property color iconColor: root.cStroke
    property real iconRotation: 0
    // Tooltip deste medidor, com o valor de quando o mouse chega nele.
    property string tip: ""
    // A barra só mostra o tooltip enquanto o alvo diz que está com o mouse.
    readonly property bool tooltipHovered: root.hoveredMeter === meter
    // Centrado na altura da barra.
    y: parent ? Math.round((parent.height - height) / 2) : 0
    spacing: Style.space(3)

    // Caixa do tamanho do desenho: os ícones Lucide da fonte DoxIA Shell passam
    // da largura do caractere mono e não ficam centrados na linha do Text, então
    // o desenho é medido e centrado (girado de lado, largura e altura trocam).
    Item {
      id: iconBox
      readonly property bool sideways: meter.iconRotation % 180 !== 0
      readonly property rect ink: glyphMetrics.tightBoundingRect
      // Centro do desenho em relação ao centro do Text, antes de girar: a
      // linha do Text inclui a descida da fonte, então o desenho fica alto.
      readonly property real dx: ink.x + ink.width / 2 - glyph.implicitWidth / 2
      readonly property real dy: glyph.baselineOffset + ink.y + ink.height / 2 - glyph.implicitHeight / 2
      anchors.verticalCenter: parent.verticalCenter
      width: Math.ceil(sideways ? ink.height : ink.width)
      height: Math.round(Style.bar.iconFont * 1.2)

      TextMetrics {
        id: glyphMetrics
        font: glyph.font
        text: glyph.text
      }

      Text {
        id: glyph
        anchors.centerIn: parent
        anchors.horizontalCenterOffset: iconBox.sideways ? -iconBox.dy : -iconBox.dx
        anchors.verticalCenterOffset: iconBox.sideways ? iconBox.dx : -iconBox.dy
        text: meter.icon
        rotation: meter.iconRotation
        color: meter.iconColor
        font.family: root.fontFamily
        font.pixelSize: Style.bar.iconFont
      }
      SupersampledText { sourceItem: glyph }
    }

    Rectangle {
      anchors.verticalCenter: parent.verticalCenter
      width: Math.max(3, Math.round(root.unit * 0.24))
      height: Math.round(root.unit * 0.95)
      radius: width / 2
      color: Qt.rgba(root.cStroke.r, root.cStroke.g, root.cStroke.b, 0.16)

      Rectangle {
        anchors.bottom: parent.bottom
        width: parent.width
        radius: parent.radius
        height: Math.max(width, parent.height * Math.max(0, Math.min(1, meter.value)))
        color: meter.fill
        Behavior on height { NumberAnimation { duration: 400; easing.type: Easing.OutCubic } }
        Behavior on color { ColorAnimation { duration: 300 } }
      }
    }
  }

  Row {
    id: row
    anchors.centerIn: parent
    height: parent.height
    spacing: Style.space(6)

    // cpu e memory-stick do Lucide (cod-chip e fa-memory na fonte DoxIA Shell).
    Meter {
      icon: String.fromCodePoint(0xEC19)
      value: root.cpu
      fill: root.cpu > 0.75 ? root.cYellow : root.cFill
      tip: "CPU: " + Math.round(root.cpu * 100) + "%"
    }
    // Pente em pé, com os contatos para a direita.
    Meter {
      icon: String.fromCodePoint(0xEFC5)
      iconRotation: -90
      value: root.mem
      fill: root.mem > 0.75 ? root.cYellow : root.cFill
      tip: "Memória: " + Math.round(root.mem * 100) + "% (" + root.memUsedGb.toFixed(1) + " / " + root.memTotalGb.toFixed(1) + " GiB)"
    }
    // Só aparece descarregando ou abaixo de 100%. Raio cheio (md-flash, zap
    // cheio) carregando, em contorno (md-lightning_bolt, zap) descarregando.
    Meter {
      visible: root.showBattery
      icon: String.fromCodePoint(root.batPlugged ? 0xF0241 : 0xF140B)
      iconColor: root.batPlugged ? root.cFill : root.cStroke
      value: root.bat
      fill: root.bat < 0.2 ? root.cRed : (root.bat < 0.35 ? root.cYellow : root.cFill)
      tip: "Bateria: " + Math.round(root.bat * 100) + "%" + (root.batCharging ? " (carregando)" : (UPower.onBattery ? "" : " (na tomada)"))
    }
  }

  // Medidor sob o mouse (null nos vãos entre eles), dono do tooltip.
  property Item hoveredMeter: null
  property Item tipMeter: null
  onHoveredMeterChanged: {
    if (!bar) return
    if (tipMeter) bar.hideTooltip(tipMeter)
    tipMeter = hoveredMeter
    // Depois do tooltipHovered do medidor se atualizar: a barra confere ele.
    if (hoveredMeter) Qt.callLater(function() {
      if (root.tipMeter) root.bar.showTooltip(root.tipMeter, root.tipMeter.tip)
    })
  }

  // Clique em qualquer medidor abre o btop; o tooltip é de cada medidor.
  MouseArea {
    anchors.fill: parent
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor
    onPositionChanged: function(mouse) {
      var p = mapToItem(row, mouse.x, mouse.y)
      var m = row.childAt(p.x, p.y)
      root.hoveredMeter = m && m.tip !== undefined ? m : null
    }
    onExited: root.hoveredMeter = null
    onClicked: if (root.bar) root.bar.run("xdg-terminal-exec btop")
  }
}
