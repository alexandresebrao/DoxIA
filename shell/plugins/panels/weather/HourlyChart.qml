import QtQuick
import qs.Commons
import "Model.js" as Model

// Hourly forecast: the temperature line over the rain bars, sharing one hour
// axis. Two stacked plots rather than one with two y-scales, since °C and mm
// have nothing in common. Only the extremes are labeled; hovering an hour
// shows its values in the readout row, which doubles as the legend.
Item {
  id: chart

  property var series: []
  property bool useImperial: false
  // True when the series starts now (first point labeled "Agora").
  property bool startsNow: true
  property color foreground: "white"
  property color surface: "black"
  property color tempColor: Color.accent
  property color rainColor: "#2b9af3"
  property string fontFamily: ""
  property int hoverIndex: -1

  readonly property real tempPlotHeight: Style.space(56)
  readonly property real rainPlotHeight: Style.space(34)
  readonly property real labelRoom: Style.space(18)
  readonly property real axisRoom: Style.space(18)
  readonly property int labelPx: Style.font.caption
  readonly property string decimalPoint: Qt.locale().decimalPoint

  readonly property var temps: series.map(function(point) { return Model.chartTemp(point.tempC, useImperial) })
  readonly property real totalRainMm: series.reduce(function(sum, point) { return sum + point.precipMm }, 0)

  implicitHeight: readout.height + Style.space(8) + canvas.height

  function tempText(value) {
    return value === null || value === undefined ? "—" : Math.round(value) + "°"
  }

  function rainText(mm) {
    return Model.formatPrecip(mm, useImperial, decimalPoint)
  }

  function extremes() {
    var lo = -1, hi = -1
    for (var i = 0; i < temps.length; ++i) {
      if (temps[i] === null) continue
      if (lo < 0 || temps[i] < temps[lo]) lo = i
      if (hi < 0 || temps[i] > temps[hi]) hi = i
    }
    return { lo: lo, hi: hi }
  }

  onSeriesChanged: { hoverIndex = -1; canvas.requestPaint() }
  onHoverIndexChanged: canvas.requestPaint()
  onUseImperialChanged: canvas.requestPaint()
  onForegroundChanged: canvas.requestPaint()
  onTempColorChanged: canvas.requestPaint()
  onRainColorChanged: canvas.requestPaint()
  onWidthChanged: canvas.requestPaint()

  // ---- Readout: the hovered hour, or a summary of the whole range.
  Row {
    id: readout
    width: parent.width
    spacing: Style.space(14)
    readonly property var point: chart.hoverIndex >= 0 ? chart.series[chart.hoverIndex] : null

    Text {
      visible: !!readout.point
      textFormat: Text.PlainText
      text: readout.point ? readout.point.hour + "h" : ""
      color: Qt.darker(chart.foreground, 1.4)
      font.family: chart.fontFamily
      font.pixelSize: Style.font.bodySmall
    }

    Row {
      spacing: Style.space(6)
      Rectangle {
        width: Style.space(10); height: 2; radius: 1
        color: chart.tempColor
        anchors.verticalCenter: parent.verticalCenter
      }
      Text {
        textFormat: Text.PlainText
        text: {
          if (readout.point) return chart.tempText(chart.temps[chart.hoverIndex])
          var e = chart.extremes()
          return e.lo < 0 ? "—" : chart.tempText(chart.temps[e.lo]) + " a " + chart.tempText(chart.temps[e.hi])
        }
        color: chart.foreground
        font.family: chart.fontFamily
        font.pixelSize: Style.font.bodySmall
      }
    }

    Row {
      spacing: Style.space(6)
      Rectangle {
        width: Style.space(8); height: Style.space(8); radius: 2
        color: chart.rainColor
        anchors.verticalCenter: parent.verticalCenter
      }
      Text {
        textFormat: Text.PlainText
        text: {
          if (!readout.point) return chart.totalRainMm > 0 ? "chuva " + chart.rainText(chart.totalRainMm) + " no total" : "sem chuva prevista"
          var text = "chuva " + chart.rainText(readout.point.precipMm)
          return readout.point.probability === null ? text : text + " (" + Math.round(readout.point.probability) + "%)"
        }
        color: chart.foreground
        font.family: chart.fontFamily
        font.pixelSize: Style.font.bodySmall
      }
    }
  }

  Canvas {
    id: canvas
    anchors.top: readout.bottom
    anchors.topMargin: Style.space(8)
    width: parent.width
    height: chart.labelRoom + chart.tempPlotHeight + chart.labelRoom
            + chart.labelRoom + chart.rainPlotHeight + chart.axisRoom

    onPaint: {
      var ctx = getContext("2d")
      ctx.reset()
      var n = chart.series.length
      if (n === 0) return

      var slot = width / n
      function xAt(i) { return slot * (i + 0.5) }
      var font = chart.labelPx + "px '" + chart.fontFamily + "'"

      // Temperature scale: at least 4 degrees tall so a flat day stays flat.
      var e = chart.extremes()
      var lo = e.lo >= 0 ? chart.temps[e.lo] : 0
      var hi = e.hi >= 0 ? chart.temps[e.hi] : 1
      if (hi - lo < 4) { var mid = (hi + lo) / 2; lo = mid - 2; hi = mid + 2 }
      var tTop = chart.labelRoom
      var tBottom = tTop + chart.tempPlotHeight
      function yAt(t) { return tBottom - (t - lo) / (hi - lo) * chart.tempPlotHeight }

      // Rain scale: 2 mm/h fills the plot, so a drizzle reads as one.
      var rainTop = tBottom + chart.labelRoom + chart.labelRoom
      var rainBottom = rainTop + chart.rainPlotHeight
      var maxMm = 0, maxRainIndex = -1
      for (var r = 0; r < n; ++r) {
        if (chart.series[r].precipMm > maxMm) { maxMm = chart.series[r].precipMm; maxRainIndex = r }
      }
      var rainScale = Math.max(2, maxMm)

      // Crosshair through both plots.
      if (chart.hoverIndex >= 0) {
        ctx.strokeStyle = Qt.rgba(chart.foreground.r, chart.foreground.g, chart.foreground.b, 0.25)
        ctx.lineWidth = 1
        ctx.beginPath()
        ctx.moveTo(Math.round(xAt(chart.hoverIndex)) + 0.5, tTop - 4)
        ctx.lineTo(Math.round(xAt(chart.hoverIndex)) + 0.5, rainBottom)
        ctx.stroke()
      }

      // Rain bars: thin, rounded at the top, anchored to the baseline.
      var barWidth = Math.max(3, Math.min(slot - 2, slot * 0.6))
      for (var b = 0; b < n; ++b) {
        var mm = chart.series[b].precipMm
        if (mm <= 0) continue
        var h = Math.max(2, mm / rainScale * chart.rainPlotHeight)
        var x0 = xAt(b) - barWidth / 2
        var radius = Math.min(4, barWidth / 2, h)
        ctx.fillStyle = chart.rainColor
        ctx.globalAlpha = chart.hoverIndex >= 0 && chart.hoverIndex !== b ? 0.55 : 1
        ctx.beginPath()
        ctx.moveTo(x0, rainBottom)
        ctx.lineTo(x0, rainBottom - h + radius)
        ctx.arcTo(x0, rainBottom - h, x0 + radius, rainBottom - h, radius)
        ctx.lineTo(x0 + barWidth - radius, rainBottom - h)
        ctx.arcTo(x0 + barWidth, rainBottom - h, x0 + barWidth, rainBottom - h + radius, radius)
        ctx.lineTo(x0 + barWidth, rainBottom)
        ctx.closePath()
        ctx.fill()
      }
      ctx.globalAlpha = 1

      // Rain baseline (recessive) and the wettest hour's amount.
      ctx.strokeStyle = Qt.rgba(chart.foreground.r, chart.foreground.g, chart.foreground.b, 0.15)
      ctx.lineWidth = 1
      ctx.beginPath()
      ctx.moveTo(0, Math.round(rainBottom) + 0.5)
      ctx.lineTo(width, Math.round(rainBottom) + 0.5)
      ctx.stroke()

      ctx.font = font
      ctx.textBaseline = "alphabetic"
      function label(text, x, y, color) {
        var w = ctx.measureText(text).width
        var left = Math.max(0, Math.min(width - w, x - w / 2))
        ctx.fillStyle = color
        ctx.fillText(text, left, y)
      }
      if (maxRainIndex >= 0) {
        var barTop = rainBottom - Math.max(2, maxMm / rainScale * chart.rainPlotHeight)
        label(chart.rainText(maxMm), xAt(maxRainIndex), barTop - 5, Qt.darker(chart.foreground, 1.2))
      }

      // Temperature line, broken where an hour has no value.
      ctx.strokeStyle = chart.tempColor
      ctx.lineWidth = 2
      ctx.lineJoin = "round"
      ctx.lineCap = "round"
      ctx.beginPath()
      var drawing = false
      for (var t = 0; t < n; ++t) {
        if (chart.temps[t] === null) { drawing = false; continue }
        if (drawing) ctx.lineTo(xAt(t), yAt(chart.temps[t]))
        else ctx.moveTo(xAt(t), yAt(chart.temps[t]))
        drawing = true
      }
      ctx.stroke()

      // Markers with a ring in the panel's background, so they sit on the line.
      function marker(i) {
        ctx.beginPath()
        ctx.arc(xAt(i), yAt(chart.temps[i]), 4, 0, Math.PI * 2)
        ctx.fillStyle = chart.tempColor
        ctx.fill()
        ctx.lineWidth = 2
        ctx.strokeStyle = chart.surface
        ctx.stroke()
      }
      if (e.hi >= 0) {
        marker(e.hi)
        label(chart.tempText(chart.temps[e.hi]), xAt(e.hi), yAt(chart.temps[e.hi]) - 9, chart.foreground)
      }
      if (e.lo >= 0 && e.lo !== e.hi) {
        marker(e.lo)
        label(chart.tempText(chart.temps[e.lo]), xAt(e.lo), yAt(chart.temps[e.lo]) + 9 + chart.labelPx, chart.foreground)
      }
      if (chart.hoverIndex >= 0 && chart.temps[chart.hoverIndex] !== null) marker(chart.hoverIndex)

      // Hour axis: every third hour, "Agora" for the current one.
      var axisY = rainBottom + chart.axisRoom - 4
      var muted = Qt.darker(chart.foreground, 1.5)
      if (chart.startsNow) label("Agora", xAt(0), axisY, muted)
      for (var a = 0; a < n; ++a) {
        if (chart.startsNow && a < 2) continue
        if (chart.series[a].hour % 3 !== 0) continue
        label(chart.series[a].hour + "h", xAt(a), axisY, muted)
      }
    }

    MouseArea {
      anchors.fill: parent
      hoverEnabled: true
      onPositionChanged: function(mouse) {
        var n = chart.series.length
        chart.hoverIndex = n === 0 ? -1 : Math.max(0, Math.min(n - 1, Math.floor(mouse.x / (width / n))))
      }
      onExited: chart.hoverIndex = -1
    }
  }
}
