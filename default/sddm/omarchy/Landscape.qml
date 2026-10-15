import QtQuick
import QtQuick.Shapes
import QtQuick.Effects

// The login screen's backdrop: a layered landscape, flat-shaded like an indie
// game's title screen. The sky, the light and the layers' tints follow the
// clock (night, dawn, day, sunset, dusk); the sun and the moon cross on an arc.
// Wind sways the pines and the grass in gusts, cloud shadows drift over the
// hills, each tree casts a shadow away from the sun, and leaves fly by day and
// fireflies glow at night.
//
// The weather comes from wttr.in (located by IP, refreshed every 20 minutes):
// cloud cover thickens the clouds and greys the sky, rain and snow fall slanted
// by the wind, fog thickens the haze, storms flash, and the real wind speed
// drives the sway. Offline (no network before login), the sky stays clear.
//
// theme.conf can pin the hour (hour=18.5), speed the day up to look at every
// light in a minute (timelapse=1440: a day per minute) or pin the weather
// (weather=clear|partly|cloudy|fog|rain|snow|storm).
Item {
  id: scene

  // Fractional hour, 0–24.
  property real hour: 12
  property real timelapse: 0

  // One clock for every motion, in seconds.
  property real t: 0
  NumberAnimation on t {
    from: 0
    to: 100000
    duration: 100000 * 1000
    loops: Animation.Infinite
  }

  // Wind: a steady breeze plus gusts that roll in every few seconds.
  readonly property real gust: Math.max(0, Math.sin(t * 0.23) * Math.sin(t * 0.61 + 1.3))
  readonly property real windBase: Math.min(1.6, 0.15 + windSpeed / 30)
  readonly property real wind: windBase * (0.45 + 0.55 * gust)

  Timer {
    interval: scene.timelapse > 0 ? 50 : 30000
    running: true
    repeat: true
    triggeredOnStart: true
    property real base: -1
    onTriggered: {
      var forced = parseFloat(config.hour)
      var lapse = parseFloat(config.timelapse)
      if (!isNaN(lapse) && lapse > 0) {
        scene.timelapse = lapse
        if (base < 0)
          base = isNaN(forced) ? 0 : forced
        scene.hour = (base + scene.t * lapse / 3600) % 24
      } else if (!isNaN(forced)) {
        scene.hour = forced
      } else {
        var now = new Date()
        scene.hour = now.getHours() + now.getMinutes() / 60
      }
    }
  }

  // ---- Weather -------------------------------------------------------------

  // clear, partly, cloudy, fog, rain, snow or storm.
  property string weather: "clear"
  // 0–1: how much of the sky the clouds take.
  property real cover: 0.15
  // km/h.
  property real windSpeed: 8
  // 0–1: rain or snow intensity.
  property real precip: 0

  readonly property bool raining: weather === "rain" || weather === "storm"
  readonly property bool snowing: weather === "snow"
  readonly property real fog: weather === "fog" ? 1 : (raining ? 0.35 : (snowing ? 0.4 : 0))
  // How grey the light is.
  readonly property real overcast: Math.min(1, Math.max(0, (cover - 0.3) / 0.7) * 0.8 + (raining ? 0.25 : 0) + (weather === "storm" ? 0.2 : 0) + fog * 0.3)

  // The first reading lands as is: animated, the clouds would swell in
  // front of the viewer. Later ones drift in over a minute, too slowly to see.
  property bool weatherKnown: false
  Behavior on cover { enabled: scene.weatherKnown; NumberAnimation { duration: 60000 } }
  Behavior on windSpeed { enabled: scene.weatherKnown; NumberAnimation { duration: 60000 } }
  Behavior on precip { enabled: scene.weatherKnown; NumberAnimation { duration: 60000 } }

  // wttr.in's WWO weather codes.
  function kindOf(code) {
    if ([200, 386, 389, 392, 395].indexOf(code) >= 0) return "storm"
    if ([179, 182, 185, 227, 230, 317, 320, 323, 326, 329, 332, 335, 338, 350, 362, 365, 368, 371, 374, 377].indexOf(code) >= 0) return "snow"
    if ([176, 263, 266, 281, 284, 293, 296, 299, 302, 305, 308, 311, 314, 353, 356, 359].indexOf(code) >= 0) return "rain"
    if ([143, 248, 260].indexOf(code) >= 0) return "fog"
    if ([119, 122].indexOf(code) >= 0) return "cloudy"
    if (code === 116) return "partly"
    return "clear"
  }

  function applyWeather(kind, coverPct, windKmh, precipMm) {
    weather = kind
    cover = Math.max(0, Math.min(1, coverPct / 100))
    windSpeed = windKmh
    precip = kind === "rain" || kind === "storm" || kind === "snow" ? Math.max(0.3, Math.min(1, precipMm / 4)) : 0
    weatherKnown = true
  }

  function fetchWeather() {
    var forced = config.weather
    if (forced && forced.length > 0) {
      var presets = {
        clear: [5, 6, 0], partly: [40, 12, 0], cloudy: [95, 14, 0], fog: [80, 3, 0],
        rain: [100, 18, 2], snow: [100, 10, 1.5], storm: [100, 38, 6]
      }
      var p = presets[forced] || presets.clear
      applyWeather(presets[forced] ? forced : "clear", p[0], p[1], p[2])
      return
    }
    var xhr = new XMLHttpRequest()
    xhr.onreadystatechange = function () {
      if (xhr.readyState !== XMLHttpRequest.DONE || xhr.status !== 200)
        return
      try {
        var now = JSON.parse(xhr.responseText).current_condition[0]
        applyWeather(kindOf(parseInt(now.weatherCode)), parseFloat(now.cloudcover), parseFloat(now.windspeedKmph), parseFloat(now.precipMM))
      } catch (e) {
        console.log("Landscape: weather unreadable: " + e)
      }
    }
    xhr.open("GET", "https://wttr.in/?format=j1&lang=pt")
    xhr.send()
  }

  Timer {
    interval: 20 * 60 * 1000
    running: true
    repeat: true
    triggeredOnStart: true
    onTriggered: scene.fetchWeather()
  }

  // ---- Light ---------------------------------------------------------------

  // Keyframes over the day: sky top and horizon, far ridge, near ridge, hills,
  // foreground, foliage, and how much daylight there is (shadows, stars).
  readonly property var keys: [
    { h: 0.0,  top: "#070b1d", hor: "#18234a", far: "#1b2547", near: "#141c38", hill: "#0f1630", fg: "#0a0f22", leaf: "#0c1424", day: 0.0 },
    { h: 4.8,  top: "#0b1230", hor: "#2a2f5c", far: "#232a52", near: "#1a2142", hill: "#131a36", fg: "#0c1226", leaf: "#0e1628", day: 0.0 },
    { h: 5.8,  top: "#29366b", hor: "#f19a7a", far: "#5a4f7d", near: "#3e3a63", hill: "#2c2d4e", fg: "#1d2036", leaf: "#1a2230", day: 0.25 },
    { h: 7.0,  top: "#5d8fd1", hor: "#fbd8b0", far: "#8ea3c7", near: "#5f7aa0", hill: "#4c7356", fg: "#2f4f37", leaf: "#26432f", day: 0.75 },
    { h: 12.0, top: "#3f84d6", hor: "#bfe2f6", far: "#9cb8d6", near: "#6f93b4", hill: "#5b8c5a", fg: "#3c6b3f", leaf: "#2f5a34", day: 1.0 },
    { h: 16.5, top: "#4f86cf", hor: "#f6dcae", far: "#a6aac4", near: "#7a8aa8", hill: "#6a8a52", fg: "#466a3a", leaf: "#365a2f", day: 0.9 },
    { h: 18.3, top: "#3b3a73", hor: "#ff8a52", far: "#8c5f7e", near: "#5e4766", hill: "#4a4a45", fg: "#2c3329", leaf: "#232c24", day: 0.45 },
    { h: 19.3, top: "#1d2150", hor: "#874f7c", far: "#3c3462", near: "#2a274c", hill: "#1f2238", fg: "#141828", leaf: "#121a26", day: 0.1 },
    { h: 20.5, top: "#0a0f26", hor: "#202850", far: "#1d2648", near: "#151d3a", hill: "#101831", fg: "#0b1024", leaf: "#0d1526", day: 0.0 },
    { h: 24.0, top: "#070b1d", hor: "#18234a", far: "#1b2547", near: "#141c38", hill: "#0f1630", fg: "#0a0f22", leaf: "#0c1424", day: 0.0 }
  ]

  function mix(a, b, k) {
    var c1 = Qt.color(a), c2 = Qt.color(b)
    return Qt.rgba(c1.r + (c2.r - c1.r) * k, c1.g + (c2.g - c1.g) * k, c1.b + (c2.b - c1.b) * k, 1)
  }

  function light(name) {
    var h = scene.hour
    for (var i = 0; i < keys.length - 1; i++) {
      var a = keys[i], b = keys[i + 1]
      if (h >= a.h && h <= b.h) {
        var k = (h - a.h) / (b.h - a.h)
        k = k * k * (3 - 2 * k)
        if (name === "day")
          return a.day + (b.day - a.day) * k
        return mix(a[name], b[name], k)
      }
    }
    return keys[0][name]
  }

  // Overcast: the colors drift toward their own grey and darken a little.
  function grey(c, k) {
    var l = 0.3 * c.r + 0.59 * c.g + 0.11 * c.b
    var g = Qt.rgba(l * 0.82, l * 0.86, l * 0.92, 1)
    return mix(c, g, k)
  }

  readonly property color skyTop: grey(light("top"), overcast * 0.85)
  readonly property color skyHorizon: grey(light("hor"), overcast * 0.8)
  readonly property color farColor: grey(light("far"), overcast * 0.6)
  readonly property color nearColor: grey(light("near"), overcast * 0.5)
  readonly property color hillColor: grey(light("hill"), overcast * 0.4)
  readonly property color fgColor: grey(light("fg"), overcast * 0.35)
  readonly property color leafColor: grey(light("leaf"), overcast * 0.35)
  readonly property real daylight: light("day") * (1 - overcast * 0.35)
  // Direct sun: shadows and the sun's glow fade under cloud.
  readonly property real sunlight: daylight * (1 - overcast)
  readonly property real night: 1 - Math.min(1, daylight * 3)

  // The sun from 6h to 18h, the moon from 18h to 6h, on the same arc.
  readonly property real sunPhase: (hour - 6) / 12
  readonly property real moonPhase: ((hour + 6) % 24) / 12
  function arcX(p) { return width * (0.08 + 0.84 * p) }
  function arcY(p) { return height * (0.62 - 0.5 * Math.sin(Math.PI * Math.max(0, Math.min(1, p)))) }
  readonly property bool sunUp: sunPhase > -0.04 && sunPhase < 1.04
  readonly property real sunX: arcX(sunPhase)
  // Shadows lean away from the sun and lengthen as it sinks.
  readonly property real shadowLean: sunUp ? Math.max(-1, Math.min(1, (width / 2 - sunX) / (width / 2))) : 0
  readonly property real shadowLength: sunUp ? 0.6 + 2.2 * (1 - Math.sin(Math.PI * Math.max(0.02, Math.min(0.98, sunPhase)))) : 0

  // Deterministic noise, so the scenery stays put between logins.
  function rnd(i) {
    var x = Math.sin(i * 127.1 + 311.7) * 43758.5453
    return x - Math.floor(x)
  }

  // A ridge line as polyline points from left to right, filled to the bottom.
  function ridge(seed, base, amp, steps, smooth) {
    var pts = [Qt.point(0, height)]
    for (var i = 0; i <= steps; i++) {
      var x = width * i / steps
      var y = base
      y -= amp * (0.55 * Math.sin(i / steps * Math.PI * (1.3 + rnd(seed) * 1.5) + rnd(seed + 1) * 6))
      y -= amp * (0.3 * Math.sin(i / steps * Math.PI * (4 + rnd(seed + 2) * 3) + rnd(seed + 3) * 6))
      if (!smooth)
        y -= amp * 0.25 * (rnd(seed * 31 + i) - 0.5)
      pts.push(Qt.point(x, y))
    }
    pts.push(Qt.point(width, height))
    return pts
  }

  // ---- Sky -----------------------------------------------------------------

  Rectangle {
    anchors.fill: parent
    gradient: Gradient {
      GradientStop { position: 0.0; color: scene.skyTop }
      GradientStop { position: 0.62; color: scene.skyHorizon }
      GradientStop { position: 1.0; color: scene.skyHorizon }
    }
  }

  // Stars, twinkling, gone by day.
  Repeater {
    model: 140
    Rectangle {
      readonly property real tw: 0.5 + 0.5 * Math.sin(scene.t * (0.8 + scene.rnd(index + 900) * 2.2) + index)
      x: scene.rnd(index + 500) * scene.width
      y: Math.pow(scene.rnd(index + 700), 1.6) * scene.height * 0.55
      width: scene.rnd(index + 800) > 0.92 ? 3 : 2
      height: width
      radius: width / 2
      color: "#fdf6e3"
      opacity: scene.night * (1 - scene.overcast) * (0.25 + 0.75 * tw) * (0.4 + 0.6 * scene.rnd(index + 600))
      visible: opacity > 0.01
    }
  }

  // The sun's glow, then the disc.
  Shape {
    visible: scene.sunUp
    anchors.fill: parent
    opacity: Math.min(1, scene.sunlight * 1.6)
    ShapePath {
      strokeWidth: 0
      fillGradient: RadialGradient {
        centerX: scene.sunX; centerY: scene.arcY(scene.sunPhase)
        focalX: centerX; focalY: centerY
        centerRadius: scene.height * 0.45
        GradientStop { position: 0.0; color: Qt.rgba(1, 0.92, 0.7, 0.55) }
        GradientStop { position: 0.25; color: Qt.rgba(1, 0.8, 0.55, 0.18) }
        GradientStop { position: 1.0; color: Qt.rgba(1, 0.8, 0.55, 0) }
      }
      PathRectangle { x: 0; y: 0; width: scene.width; height: scene.height }
    }
  }
  Rectangle {
    visible: scene.sunUp
    width: scene.height * 0.075
    height: width
    radius: width / 2
    x: scene.sunX - width / 2
    y: scene.arcY(scene.sunPhase) - height / 2
    color: scene.mix("#fff1c9", "#ffb36b", 1 - Math.sin(Math.PI * Math.max(0, Math.min(1, scene.sunPhase))))
    opacity: Math.max(0, 1 - scene.overcast * 1.6)
  }

  // The moon: a disc with a shadow bite.
  Item {
    visible: scene.moonPhase > -0.04 && scene.moonPhase < 1.04
    opacity: scene.night * Math.max(0, 1 - scene.overcast * 1.6)
    width: scene.height * 0.055
    height: width
    x: scene.arcX(scene.moonPhase) - width / 2
    y: scene.arcY(scene.moonPhase) - height / 2
    Rectangle { anchors.fill: parent; radius: width / 2; color: "#eef0f8" }
    Rectangle {
      width: parent.width; height: width; radius: width / 2
      x: parent.width * 0.32; y: -parent.height * 0.12
      color: scene.skyTop
      opacity: 0.92
    }
    Rectangle {
      anchors.centerIn: parent
      width: parent.width * 3.2; height: width; radius: width / 2
      color: "#dfe6ff"
      opacity: 0.06
    }
  }

  // Clouds: soft puffs drifting with the wind, lit by the sky.
  Repeater {
    model: 14
    Item {
      id: cloud
      visible: index < 2 + Math.round(scene.cover * 12)
      readonly property real speed: 6 + scene.rnd(index + 40) * 10
      readonly property real span: scene.width + width * 2
      readonly property int seed: index
      readonly property color tint: scene.mix(scene.mix(scene.skyHorizon, "#ffffff", 0.35 + 0.45 * scene.daylight), "#5d6470", scene.overcast * (scene.weather === "storm" ? 0.8 : 0.5))
      width: scene.width * (0.08 + scene.rnd(index + 41) * 0.07) * (1 + scene.cover * 0.9)
      height: width * 0.8
      x: ((scene.rnd(index + 42) * span + scene.t * speed) % span) - width
      y: scene.height * (0.03 + scene.rnd(index + 43) * (0.18 + scene.cover * 0.1))
      // Faded as one, so the puffs don't show through each other.
      opacity: Math.min(1, 0.12 + 0.73 * scene.daylight + scene.overcast * 0.5)
      layer.enabled: true
      Repeater {
        model: 4
        Rectangle {
          readonly property real s: 0.3 + scene.rnd(cloud.seed * 13 + index * 7 + 3) * 0.26
          width: cloud.width * s
          height: width
          radius: width / 2
          // Inside the cloud's box: its layer clips to it.
          x: Math.min(cloud.width - width, cloud.width * (0.04 + index * 0.17))
          y: Math.max(0, cloud.height - height * (0.62 + 0.3 * scene.rnd(cloud.seed * 5 + index)) - cloud.height * 0.04)
          color: cloud.tint
        }
      }
      Rectangle {
        width: cloud.width * 0.92
        height: cloud.height * 0.2
        radius: height / 2
        x: cloud.width * 0.04
        y: cloud.height - height
        color: cloud.tint
      }
    }
  }

  // ---- Land ----------------------------------------------------------------

  // Far ridge.
  Shape {
    anchors.fill: parent
    ShapePath {
      strokeWidth: 0
      fillColor: scene.farColor
      PathPolyline { path: scene.ridge(3, scene.height * 0.56, scene.height * 0.16, 90, false) }
    }
  }

  // Haze between the layers.
  Rectangle {
    width: scene.width
    height: scene.height * 0.2
    y: scene.height * 0.5
    gradient: Gradient {
      GradientStop { position: 0.0; color: Qt.rgba(scene.skyHorizon.r, scene.skyHorizon.g, scene.skyHorizon.b, 0) }
      GradientStop { position: 1.0; color: Qt.rgba(scene.skyHorizon.r, scene.skyHorizon.g, scene.skyHorizon.b, 0.45 + 0.4 * scene.fog) }
    }
  }

  // Near ridge.
  Shape {
    anchors.fill: parent
    ShapePath {
      strokeWidth: 0
      fillColor: scene.nearColor
      PathPolyline { path: scene.ridge(11, scene.height * 0.66, scene.height * 0.1, 70, false) }
    }
  }

  // Distant forest line on the near ridge.
  Repeater {
    model: 70
    Pine {
      readonly property real p: index / 70
      height: scene.height * (0.035 + scene.rnd(index + 1200) * 0.03)
      x: p * scene.width + scene.rnd(index + 1300) * 12
      y: scene.height * (0.645 + scene.rnd(index + 1400) * 0.05) - height
      color: scene.mix(scene.nearColor, scene.leafColor, 0.35)
      sway: 0.3
      phase: index * 0.37
    }
  }

  Rectangle {
    width: scene.width
    height: scene.height * 0.16
    y: scene.height * 0.62
    gradient: Gradient {
      GradientStop { position: 0.0; color: Qt.rgba(scene.skyHorizon.r, scene.skyHorizon.g, scene.skyHorizon.b, 0) }
      GradientStop { position: 1.0; color: Qt.rgba(scene.skyHorizon.r, scene.skyHorizon.g, scene.skyHorizon.b, 0.3 + 0.5 * scene.fog) }
    }
  }

  // Hills.
  Shape {
    anchors.fill: parent
    ShapePath {
      strokeWidth: 0
      fillColor: scene.hillColor
      PathPolyline { path: scene.ridge(23, scene.height * 0.76, scene.height * 0.07, 50, true) }
    }
  }

  // Foreground: a rise on the left, where the trees stand, sloping right.
  Shape {
    id: ground
    anchors.fill: parent
    ShapePath {
      strokeWidth: 0
      fillColor: scene.fgColor
      PathPolyline {
        path: {
          var pts = [Qt.point(0, scene.height)]
          for (var i = 0; i <= 60; i++) {
            var x = scene.width * i / 60
            pts.push(Qt.point(x, scene.groundY(x)))
          }
          pts.push(Qt.point(scene.width, scene.height))
          return pts
        }
      }
    }
  }

  function groundY(x) {
    var p = x / width
    return height * (0.8 + 0.08 * p - 0.035 * Math.sin(p * Math.PI * 1.6 + 0.4) - 0.012 * Math.sin(p * Math.PI * 6.3))
  }

  // Cloud shadows sliding over the hills and the foreground.
  Item {
    anchors.fill: parent
    clip: true
    opacity: 0.16 * scene.sunlight * Math.min(1, scene.cover * 3)
    visible: opacity > 0.005
    Repeater {
      model: 3
      Item {
        readonly property real span: scene.width + width
        width: scene.width * (0.3 + scene.rnd(index + 60) * 0.2)
        height: scene.height * 0.06
        x: ((scene.rnd(index + 61) * span + scene.t * (9 + index * 3)) % span) - width
        y: scene.height * (0.74 + index * 0.06)
        Rectangle {
          id: shadowBlob
          anchors.fill: parent
          radius: height / 2
          color: "#000814"
          visible: false
        }
        // Blurred to a soft-edged patch.
        MultiEffect {
          source: shadowBlob
          anchors.fill: parent
          anchors.margins: -64
          autoPaddingEnabled: true
          blurEnabled: true
          blurMax: 64
          blur: 1
        }
      }
    }
  }

  // Trees on the foreground rise, each with its shadow.
  Repeater {
    model: 14
    Item {
      id: tree
      // Clusters off the clock and the card: the left edge, the middle, the right edge.
      readonly property real p: index < 3 ? scene.rnd(index + 2000) * 0.05
                              : index < 7 ? 0.24 + scene.rnd(index + 2000) * 0.1
                              : index < 11 ? 0.68 + scene.rnd(index + 2000) * 0.1
                              : 0.93 + scene.rnd(index + 2000) * 0.06
      readonly property real h: scene.height * (0.13 + scene.rnd(index + 2100) * 0.14)
      readonly property real baseX: p * scene.width
      readonly property real baseY: scene.groundY(baseX) + 4

      // The shadow: a long thin wedge on the ground, away from the sun.
      Rectangle {
        readonly property real len: tree.h * (0.22 + 0.5 * scene.shadowLength * Math.abs(scene.shadowLean))
        width: len
        height: tree.h * 0.05
        radius: height / 2
        x: tree.baseX - width / 2 + scene.shadowLean * len * 0.45 + pine.lean * tree.h * 0.4
        y: tree.baseY - height * 0.6
        color: "#020611"
        opacity: 0.32 * scene.sunlight
        visible: opacity > 0.01
      }

      Pine {
        id: pine
        height: tree.h
        x: tree.baseX - width / 2
        y: tree.baseY - height
        color: scene.leafColor
        sway: 1
        phase: index * 1.9
      }
    }
  }

  // Grass along the foreground edge.
  Repeater {
    model: 150
    Rectangle {
      readonly property real gx: scene.rnd(index + 3000) * scene.width
      width: 2
      height: scene.height * (0.012 + scene.rnd(index + 3100) * 0.02)
      radius: 1
      x: gx
      y: scene.groundY(gx) - height + 3
      color: scene.mix(scene.fgColor, scene.leafColor, 0.6)
      transformOrigin: Item.Bottom
      rotation: (6 + 22 * scene.wind) * Math.sin(scene.t * 2.4 + gx * 0.02) * 0.5 + 10 * scene.wind
    }
  }

  // Leaves riding the gusts, by day.
  Repeater {
    model: 10
    Rectangle {
      readonly property real speed: 60 + scene.rnd(index + 4000) * 80
      readonly property real span: scene.width + 100
      readonly property real fx: ((scene.rnd(index + 4001) * span + scene.t * speed * (0.5 + scene.wind)) % span) - 50
      width: 7
      height: 4
      radius: 2
      x: fx
      y: scene.height * (0.55 + scene.rnd(index + 4002) * 0.35) + 30 * Math.sin(scene.t * 1.3 + index)
      rotation: scene.t * (120 + index * 20)
      color: index % 3 === 0 ? "#c9842f" : (index % 3 === 1 ? "#9fb34a" : "#d9a441")
      opacity: Math.min(1, scene.daylight * 1.5) * 0.85 * (scene.raining || scene.snowing ? 0 : 1)
      visible: opacity > 0.01
    }
  }

  // Fireflies, by night.
  Repeater {
    model: 24
    Rectangle {
      readonly property real glow: Math.max(0, Math.sin(scene.t * (0.7 + scene.rnd(index + 5000)) + index * 2.1))
      width: 4
      height: 4
      radius: 2
      x: scene.rnd(index + 5001) * scene.width * 0.6 + 40 * Math.sin(scene.t * 0.3 + index)
      y: scene.height * (0.72 + scene.rnd(index + 5002) * 0.22) + 20 * Math.cos(scene.t * 0.4 + index * 1.3)
      color: "#f6ff9e"
      opacity: scene.night * glow * (scene.raining || scene.snowing ? 0 : 1)
      visible: opacity > 0.01
      Rectangle {
        anchors.centerIn: parent
        width: 16; height: 16; radius: 8
        color: "#f6ff9e"
        opacity: 0.25
      }
    }
  }

  // Fog: a veil over the land.
  Rectangle {
    anchors.fill: parent
    visible: scene.fog > 0.01
    gradient: Gradient {
      GradientStop { position: 0.35; color: Qt.rgba(scene.skyHorizon.r, scene.skyHorizon.g, scene.skyHorizon.b, 0) }
      GradientStop { position: 0.75; color: Qt.rgba(scene.skyHorizon.r, scene.skyHorizon.g, scene.skyHorizon.b, 0.45 * scene.fog) }
      GradientStop { position: 1.0; color: Qt.rgba(scene.skyHorizon.r, scene.skyHorizon.g, scene.skyHorizon.b, 0.25 * scene.fog) }
    }
  }

  // Rain: streaks slanted by the wind.
  Repeater {
    model: 220
    Rectangle {
      readonly property real speed: 900 + scene.rnd(index + 6000) * 500
      readonly property real fall: (scene.rnd(index + 6001) * scene.height * 1.2 + scene.t * speed) % (scene.height * 1.2)
      visible: scene.raining && index < 220 * scene.precip
      width: 1.5
      height: 14 + scene.rnd(index + 6002) * 12
      radius: 1
      x: (scene.rnd(index + 6003) * scene.width * 1.3 - scene.width * 0.15 + fall * scene.windBase * 0.35) % (scene.width * 1.15)
      y: fall - scene.height * 0.1
      rotation: -scene.windBase * 18
      color: Qt.rgba(0.85, 0.9, 1, 0.35 + 0.25 * scene.rnd(index + 6004))
    }
  }

  // Snow: flakes drifting down, swaying.
  Repeater {
    model: 160
    Rectangle {
      readonly property real speed: 40 + scene.rnd(index + 7000) * 50
      readonly property real fall: (scene.rnd(index + 7001) * scene.height * 1.1 + scene.t * speed) % (scene.height * 1.1)
      visible: scene.snowing && index < 160 * scene.precip
      width: 2 + scene.rnd(index + 7002) * 4
      height: width
      radius: width / 2
      x: (scene.rnd(index + 7003) * scene.width + fall * scene.windBase * 0.25 + 18 * Math.sin(scene.t * 0.9 + index)) % scene.width
      y: fall - scene.height * 0.05
      color: "#ffffff"
      opacity: 0.6 + 0.4 * scene.rnd(index + 7004)
    }
  }

  // Lightning: a flash every so often in a storm.
  Rectangle {
    anchors.fill: parent
    color: "#e8eeff"
    readonly property real slot: Math.floor(scene.t / 6)
    readonly property real at: scene.t - slot * 6
    readonly property bool strikes: scene.rnd(slot + 8000) > 0.45
    opacity: scene.weather === "storm" && strikes ? Math.max(0, 0.55 * (1 - at * 6)) + Math.max(0, 0.35 * (1 - Math.abs(at - 0.25) * 12)) : 0
    visible: opacity > 0.01
  }

  // A pine: three stacked tiers and a trunk, swaying from its base.
  component Pine: Item {
    id: pineItem
    property color color: "#2f5a34"
    property real sway: 1
    property real phase: 0
    // Lean in radians-ish units, for the shadow to follow.
    readonly property real lean: sway * (0.02 + 0.05 * scene.wind) * Math.sin(scene.t * (1.1 + 0.2 * Math.sin(phase)) + phase) + sway * 0.03 * scene.wind

    width: height * 0.42
    transformOrigin: Item.Bottom
    rotation: lean * 57.3

    Rectangle {
      width: pineItem.width * 0.1
      height: pineItem.height * 0.14
      x: (pineItem.width - width) / 2
      y: pineItem.height - height
      color: Qt.darker(pineItem.color, 1.4)
    }

    Shape {
      anchors.fill: parent
      ShapePath {
        strokeWidth: 0
        fillColor: pineItem.color
        startX: pineItem.width * 0.5; startY: 0
        PathLine { x: pineItem.width * 0.78; y: pineItem.height * 0.34 }
        PathLine { x: pineItem.width * 0.64; y: pineItem.height * 0.34 }
        PathLine { x: pineItem.width * 0.9; y: pineItem.height * 0.6 }
        PathLine { x: pineItem.width * 0.72; y: pineItem.height * 0.6 }
        PathLine { x: pineItem.width * 1.0; y: pineItem.height * 0.88 }
        PathLine { x: 0; y: pineItem.height * 0.88 }
        PathLine { x: pineItem.width * 0.28; y: pineItem.height * 0.6 }
        PathLine { x: pineItem.width * 0.1; y: pineItem.height * 0.6 }
        PathLine { x: pineItem.width * 0.36; y: pineItem.height * 0.34 }
        PathLine { x: pineItem.width * 0.22; y: pineItem.height * 0.34 }
        PathLine { x: pineItem.width * 0.5; y: 0 }
      }
    }

    // The lit side: a lighter half facing the sun.
    Shape {
      anchors.fill: parent
      opacity: 0.18 * scene.sunlight
      ShapePath {
        strokeWidth: 0
        fillColor: "#fff3c4"
        startX: pineItem.width * 0.5; startY: 0
        PathLine { x: pineItem.width * (scene.sunX > pineItem.x ? 0.78 : 0.22); y: pineItem.height * 0.34 }
        PathLine { x: pineItem.width * 0.5; y: pineItem.height * 0.34 }
      }
    }
  }
}
