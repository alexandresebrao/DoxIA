import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Services.Mpris
import Quickshell.Services.Pipewire
import qs.Commons
import qs.Ui

// Now-playing bar widget: title in the bar, and a popup with album art, a
// seekable progress bar and full controls. Talks to MPRIS players directly,
// so it works with Spotify and anything else that speaks MPRIS.
Panel {
  id: root
  moduleName: "alexandre.media"
  ipcTarget: "alexandre.media"
  manageIpc: false

  readonly property string gPlay: String.fromCodePoint(0xF040A)
  readonly property string gPause: String.fromCodePoint(0xF03E4)
  readonly property string gPrev: String.fromCodePoint(0xF04AE)
  readonly property string gNext: String.fromCodePoint(0xF04AD)
  readonly property string gShuffle: String.fromCodePoint(0xF049D)
  readonly property string gRepeat: String.fromCodePoint(0xF0456)
  readonly property string gRepeatOne: String.fromCodePoint(0xF0458)
  readonly property string gMusic: String.fromCodePoint(0xF075A)
  readonly property string gOpen: String.fromCodePoint(0xF03CC)
  readonly property string gVolume: String.fromCodePoint(0xF057E)
  readonly property string gVolumeLow: String.fromCodePoint(0xF057F)
  readonly property string gMute: String.fromCodePoint(0xF075F)

  readonly property real maxLabelWidth: Math.max(60, parseInt(setting("maxLabelWidth", 180), 10) || 180)

  // playerctld only proxies the other players; listing it would show every
  // track twice.
  readonly property var players: {
    var all = Mpris.players ? Mpris.players.values : []
    var out = []
    for (var i = 0; i < all.length; i++) {
      var p = all[i]
      if (String(p.dbusName || "").indexOf("playerctld") === -1) out.push(p)
    }
    return out
  }
  property string selectedName: ""
  readonly property var player: {
    var i
    for (i = 0; i < players.length; i++) if (players[i].dbusName === selectedName) return players[i]
    for (i = 0; i < players.length; i++) if (players[i].isPlaying) return players[i]
    for (i = 0; i < players.length; i++) if (players[i].trackTitle) return players[i]
    return players.length > 0 ? players[0] : null
  }

  readonly property bool hasMedia: player !== null && (player.trackTitle !== "" || player.trackArtist !== "")
  readonly property bool playing: player !== null && player.isPlaying
  readonly property string title: player ? (player.trackTitle || "") : ""
  readonly property string artist: player ? (player.trackArtist || "") : ""
  readonly property string album: player ? (player.trackAlbum || "") : ""
  readonly property string artUrl: player ? (player.trackArtUrl || "") : ""
  readonly property real length: player && player.lengthSupported ? player.length : 0
  readonly property bool canSeek: player !== null && player.canSeek && player.positionSupported && length > 0

  readonly property color foreground: bar ? bar.foreground : Color.foreground
  readonly property color dim: Qt.darker(foreground, 1.55)
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family

  function formatTime(seconds) {
    var s = Math.max(0, Math.floor(seconds || 0))
    var h = Math.floor(s / 3600)
    var m = Math.floor((s % 3600) / 60)
    var sec = s % 60
    var mm = h > 0 && m < 10 ? "0" + m : String(m)
    return (h > 0 ? h + ":" : "") + mm + ":" + (sec < 10 ? "0" : "") + sec
  }

  function playerLabel(p) {
    if (!p) return ""
    var name = p.identity || p.desktopEntry || ""
    return name.charAt(0).toUpperCase() + name.slice(1)
  }

  // Volume do player: o stream dele no PipeWire (o mesmo que o painel de
  // áudio mexe). O volume MPRIS do Spotify pode vir desatualizado, então só é
  // lido quando não há stream encontrado (mas é sempre escrito em setVolume).
  readonly property var playbackStreams: {
    var all = Pipewire.nodes ? Pipewire.nodes.values : []
    var out = []
    for (var i = 0; i < all.length; i++) {
      var n = all[i]
      if (n && n.isStream && n.isSink === true && n.audio) out.push(n)
    }
    return out
  }
  PwObjectTracker { objects: root.opened ? root.playbackStreams : [] }

  function streamMatchesPlayer(node, p) {
    if (!node || !node.ready || !node.properties || !p) return false
    var props = node.properties
    var keys = [String(p.identity || "").toLowerCase(), String(p.desktopEntry || "").toLowerCase()]
    var names = [props["application.name"], props["application.process.binary"], props["node.name"]]
    for (var k = 0; k < keys.length; k++) {
      if (!keys[k]) continue
      for (var j = 0; j < names.length; j++) {
        var name = String(names[j] || "").toLowerCase()
        if (name && (name.indexOf(keys[k]) !== -1 || keys[k].indexOf(name) !== -1)) return true
      }
    }
    return false
  }

  readonly property var playerStreams: {
    var out = []
    for (var i = 0; i < playbackStreams.length; i++)
      if (streamMatchesPlayer(playbackStreams[i], player)) out.push(playbackStreams[i])
    return out
  }
  readonly property bool hasStreamVolume: playerStreams.length > 0
  readonly property bool hasVolume: hasStreamVolume || (player !== null && player.volumeSupported)
  readonly property real volume: hasStreamVolume ? playerStreams[0].audio.volume
    : (player && player.volumeSupported ? player.volume : 0)
  readonly property bool muted: hasStreamVolume && playerStreams[0].audio.muted

  function setVolume(v) {
    v = Math.max(0, Math.min(1, v))
    // O Spotify reaplica o volume interno (MPRIS) no stream a cada troca de
    // faixa, então ele também precisa ser atualizado, não só o stream.
    if (player && player.volumeSupported && player.canControl) player.volume = v
    for (var i = 0; i < playerStreams.length; i++) playerStreams[i].audio.volume = v
  }

  function toggleMute() {
    if (!hasStreamVolume) return
    var m = !muted
    for (var i = 0; i < playerStreams.length; i++) playerStreams[i].audio.muted = m
  }

  function playPause() { if (player && player.canTogglePlaying) player.togglePlaying() }
  function next() { if (player && player.canGoNext) player.next() }
  function previous() { if (player && player.canGoPrevious) player.previous() }

  function seekBy(delta) {
    if (!canSeek) return
    player.position = Math.max(0, Math.min(length, player.position + delta))
  }

  function toggleShuffle() {
    if (player && player.shuffleSupported) player.shuffle = !player.shuffle
  }

  function cycleLoop() {
    if (!player || !player.loopSupported) return
    if (player.loopState === MprisLoopState.None) player.loopState = MprisLoopState.Playlist
    else if (player.loopState === MprisLoopState.Playlist) player.loopState = MprisLoopState.Track
    else player.loopState = MprisLoopState.None
  }

  // Always shown: with no player it is a music glyph that launches Spotify.
  implicitWidth: row.implicitWidth + Style.space(14)
  implicitHeight: bar ? bar.barSize : Style.bar.sizeHorizontal

  onOpenedChanged: if (opened) {
    labelText.x = 0
    if (player) player.positionChanged()
    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }

  // MPRIS only reports position on seeks, so poll it while the popup shows it.
  Timer {
    interval: 500
    running: root.opened && root.playing
    repeat: true
    onTriggered: if (root.player) root.player.positionChanged()
  }

  IpcHandler {
    target: root.ipcTarget
    function open(): void { root.open() }
    function close(): void { root.close() }
    function show(): void { root.open() }
    function hide(): void { root.close() }
    function toggle(): void { root.toggle() }
    function playPause(): void { root.playPause() }
    function next(): void { root.next() }
    function previous(): void { root.previous() }
  }

  // ---------------- bar ----------------

  Row {
    id: row
    anchors.centerIn: parent
    spacing: Style.space(6)

    Text {
      id: glyph
      textFormat: Text.PlainText
      anchors.verticalCenter: parent.verticalCenter
      text: !root.hasMedia ? root.gMusic : (root.playing ? root.gPause : root.gPlay)
      color: root.playing ? root.barForeground : Qt.darker(root.barForeground, 1.5)
      font.family: root.fontFamily
      font.pixelSize: Style.font.body
    }

    Item {
      id: scrollClip
      // Largura fixa: títulos curtos não encolhem o widget, então trocar de
      // faixa não empurra o resto da barra.
      width: root.maxLabelWidth
      height: glyph.height
      clip: true
      anchors.verticalCenter: parent.verticalCenter
      visible: !(root.bar && root.bar.vertical) && root.hasMedia && root.title !== ""

      Text {
        id: labelText
        textFormat: Text.PlainText
        text: root.title + (root.artist ? "  ·  " + root.artist : "")
        color: root.barForeground
        font.family: root.fontFamily
        font.pixelSize: Style.font.body
        anchors.verticalCenter: parent.verticalCenter
        // Com a largura fixa, títulos curtos cabem inteiros; mesmo assim o
        // letreiro gira sempre que está tocando. Pausado (ou com o popup
        // aberto mostrando o título completo) ele estaciona no início.
        readonly property bool scrolling: root.playing && !root.opened && scrollClip.visible
        width: scrolling ? implicitWidth : scrollClip.width
        elide: scrolling ? Text.ElideNone : Text.ElideRight
        onTextChanged: {
          x = 0
          if (marquee.running) marquee.restart()
        }

        NumberAnimation on x {
          id: marquee
          running: labelText.scrolling
          loops: Animation.Infinite
          // Velocidade constante (~40 px/s), independente do tamanho do título.
          duration: Math.max(4000, (scrollClip.width + labelText.implicitWidth) * 25)
          from: scrollClip.width
          to: -labelText.implicitWidth
          onRunningChanged: if (!running) labelText.x = 0
        }
      }
    }
  }

  MouseArea {
    id: barMouse
    anchors.fill: parent
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor
    acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
    onClicked: function(mouse) {
      if (!root.hasMedia) {
        if (mouse.button === Qt.LeftButton && root.bar) root.bar.run("omarchy-launch-spotify")
        return
      }
      if (mouse.button === Qt.RightButton) root.playPause()
      else if (mouse.button === Qt.MiddleButton) root.next()
      else root.toggle()
    }
    onEntered: if (root.bar && !root.opened) root.bar.showTooltip(root, root.hasMedia ? root.title + (root.artist ? " — " + root.artist : "") : "Abrir Spotify")
    onExited: if (root.bar) root.bar.hideTooltip(root)
  }

  // ---------------- popup ----------------

  KeyboardPanel {
    id: panel
    anchorItem: root
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(320))
    contentHeight: panel.fittedContentHeight(column.implicitHeight, Style.space(640))

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }
      onActivateRequested: root.playPause()
      onMoveRequested: function(dx, dy) {
        if (dx !== 0) root.seekBy(dx * 5)
        else if (dy < 0) root.previous()
        else if (dy > 0) root.next()
      }
      onTextKey: function(t) {
        if (t === " " || t === "k") root.playPause()
        else if (t === "n") root.next()
        else if (t === "p") root.previous()
        else if (t === "s") root.toggleShuffle()
        else if (t === "r") root.cycleLoop()
        else if (t === "+" || t === "=") root.setVolume(root.volume + 0.05)
        else if (t === "-") root.setVolume(root.volume - 0.05)
        else if (t === "m") root.toggleMute()
      }

      Column {
        id: column
        width: parent.width
        spacing: Style.space(12)

        // Album art
        BorderSurface {
          width: parent.width
          height: width
          radius: Style.spacing.labelGap
          color: Style.normalFillFor(root.foreground, Color.accent)
          borderSpec: Border.controlSpec("normal", root.foreground, Color.accent)

          Image {
            id: art
            anchors.fill: parent
            anchors.margins: Style.space(2)
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            cache: true
            sourceSize.width: 640
            sourceSize.height: 640
            source: root.artUrl
            visible: status === Image.Ready
          }

          Text {
            anchors.centerIn: parent
            visible: !art.visible
            text: root.gMusic
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.displayLarge * 2
          }
        }

        // Track info
        Column {
          width: parent.width
          spacing: Style.space(2)

          Text {
            textFormat: Text.PlainText
            width: parent.width
            text: root.title || "Nada tocando"
            color: root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.heading
            font.bold: true
            elide: Text.ElideRight
          }

          Text {
            textFormat: Text.PlainText
            width: parent.width
            // Sempre ocupa a linha (mesmo vazia) para o popup não mudar de altura.
            text: root.artist || " "
            color: Qt.darker(root.foreground, 1.25)
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
            elide: Text.ElideRight
          }

          Text {
            textFormat: Text.PlainText
            width: parent.width
            opacity: root.album !== "" && root.album !== root.artist ? 1 : 0
            text: root.album || " "
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            elide: Text.ElideRight
          }
        }

        // Progress
        Column {
          width: parent.width
          spacing: Style.space(4)
          // Fica no layout mesmo sem duração conhecida (troca de faixa, rádio)
          // para a altura do popup não oscilar.
          opacity: root.length > 0 ? 1 : 0

          PanelSlider {
            id: progress
            width: parent.width
            bar: root.bar
            minimum: 0
            maximum: Math.max(1, root.length)
            step: 1
            value: root.player ? root.player.position : 0
            enabled: root.canSeek
            onReleased: function(v) { if (root.canSeek) root.player.position = v }
          }

          RowLayout {
            width: parent.width

            Text {
              textFormat: Text.PlainText
              text: root.formatTime(progress.dragging ? progress.liveValue : (root.player ? root.player.position : 0))
              color: root.dim
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
            }
            Item { Layout.fillWidth: true }
            Text {
              textFormat: Text.PlainText
              text: root.formatTime(root.length)
              color: root.dim
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
            }
          }
        }

        // Controls
        RowLayout {
          width: parent.width
          spacing: Style.space(4)

          ControlButton {
            iconText: root.gShuffle
            tooltipText: root.player && root.player.shuffle ? "Aleatório: ligado" : "Aleatório: desligado"
            visible: root.player !== null && root.player.shuffleSupported
            toggle: true
            lit: root.player !== null && root.player.shuffle
            onClicked: root.toggleShuffle()
          }
          Item { Layout.fillWidth: true }
          ControlButton {
            iconText: root.gPrev
            tooltipText: "Anterior"
            enabled: root.player !== null && root.player.canGoPrevious
            onClicked: root.previous()
          }
          ControlButton {
            iconText: root.playing ? root.gPause : root.gPlay
            tooltipText: root.playing ? "Pausar" : "Tocar"
            iconSize: Style.font.iconLarge * 1.4
            enabled: root.player !== null && root.player.canTogglePlaying
            onClicked: root.playPause()
          }
          ControlButton {
            iconText: root.gNext
            tooltipText: "Próxima"
            enabled: root.player !== null && root.player.canGoNext
            onClicked: root.next()
          }
          Item { Layout.fillWidth: true }
          ControlButton {
            iconText: root.player && root.player.loopState === MprisLoopState.Track ? root.gRepeatOne : root.gRepeat
            tooltipText: !root.player ? "" : (root.player.loopState === MprisLoopState.Track ? "Repetir: faixa"
              : (root.player.loopState === MprisLoopState.Playlist ? "Repetir: tudo" : "Repetir: desligado"))
            visible: root.player !== null && root.player.loopSupported
            toggle: true
            lit: root.player !== null && root.player.loopState !== MprisLoopState.None
            onClicked: root.cycleLoop()
          }
        }

        // Volume do player
        RowLayout {
          width: parent.width
          spacing: Style.space(8)
          // Fica no layout mesmo sem volume para a altura do popup não mudar.
          opacity: root.hasVolume ? 1 : 0
          enabled: root.hasVolume

          Text {
            textFormat: Text.PlainText
            text: root.muted || root.volume <= 0 ? root.gMute : (root.volume < 0.5 ? root.gVolumeLow : root.gVolume)
            color: root.muted ? root.dim : root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.title
            Layout.preferredWidth: Style.space(22)
            horizontalAlignment: Text.AlignHCenter

            MouseArea {
              anchors.fill: parent
              cursorShape: root.hasStreamVolume ? Qt.PointingHandCursor : Qt.ArrowCursor
              onClicked: root.toggleMute()
            }
          }

          PanelSlider {
            id: volumeSlider
            Layout.fillWidth: true
            bar: root.bar
            minimum: 0
            maximum: 1
            step: 0.05
            value: root.volume
            opacity: root.muted ? 0.5 : 1.0
            onMoved: function(v) { root.setVolume(v) }
            onReleased: function(v) { root.setVolume(v) }
            onRightClicked: root.toggleMute()
          }

          Text {
            textFormat: Text.PlainText
            text: Math.round((volumeSlider.dragging ? volumeSlider.liveValue : root.volume) * 100) + "%"
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            font.bold: true
            Layout.preferredWidth: Style.space(36)
            horizontalAlignment: Text.AlignRight
          }
        }

        // Source + open app
        RowLayout {
          width: parent.width
          spacing: Style.space(6)

          Text {
            textFormat: Text.PlainText
            Layout.fillWidth: true
            text: root.playerLabel(root.player)
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            elide: Text.ElideRight
          }

          PanelActionButton {
            opacity: root.player !== null && root.player.canRaise ? 1 : 0
            enabled: opacity > 0
            iconText: root.gOpen
            tooltipText: "Abrir " + root.playerLabel(root.player)
            foreground: root.foreground
            fontFamily: root.fontFamily
            onClicked: { root.player.raise(); root.close() }
          }
        }

        // Other players
        Column {
          width: parent.width
          spacing: Style.space(4)
          visible: root.players.length > 1

          PanelSeparator { foreground: root.foreground }

          Repeater {
            model: root.players

            CursorSurface {
              id: sourceRow
              required property var modelData
              readonly property bool selected: root.player === modelData

              width: parent.width
              implicitHeight: sourceInner.implicitHeight + Style.space(10)
              hasCursor: sourceMouse.containsMouse
              current: selected
              foreground: root.foreground

              RowLayout {
                id: sourceInner
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                anchors.leftMargin: Style.space(8)
                anchors.rightMargin: Style.space(8)
                spacing: Style.space(8)

                Text {
                  text: sourceRow.modelData.isPlaying ? root.gPause : root.gPlay
                  color: root.foreground
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.body
                }

                Text {
                  textFormat: Text.PlainText
                  Layout.fillWidth: true
                  text: (sourceRow.modelData.trackTitle || root.playerLabel(sourceRow.modelData))
                    + (sourceRow.modelData.trackTitle ? "  ·  " + root.playerLabel(sourceRow.modelData) : "")
                  color: root.foreground
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.bodySmall
                  font.bold: sourceRow.selected
                  elide: Text.ElideRight
                }
              }

              MouseArea {
                id: sourceMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.selectedName = sourceRow.modelData.dbusName
              }
            }
          }
        }
      }
    }
  }

  // Só o ícone: sem fundo nem borda em nenhum estado. Botões de alternar
  // (aleatório/repetir) ficam brancos ligados e cinza desligados; o hover
  // só clareia/aumenta o ícone.
  component ControlButton: Button {
    id: ctl
    property bool lit: false
    property bool toggle: false
    foreground: ctl.hot ? Qt.lighter(root.foreground, 1.25) : ((!ctl.toggle || ctl.lit) ? root.foreground : root.dim)
    fontFamily: root.fontFamily
    iconSize: Style.font.iconLarge
    horizontalPadding: Style.spacing.controlPaddingX
    verticalPadding: Style.spacing.controlPaddingY
    color: "transparent"
    borderSpec: Border.none()
    scale: ctl.hot && ctl.enabled ? 1.12 : 1.0
    Behavior on scale { NumberAnimation { duration: 120 } }
    opacity: enabled ? 1.0 : 0.4
  }
}
