import QtQuick
import Quickshell
import Quickshell.Services.Pipewire
import qs.Commons
import qs.Ui

// Bolinha vermelha que pisca enquanto o portal wlr transmite a tela
// (nó "xdg-desktop-portal-wlr", Video/Source, no PipeWire). Some quando não há compartilhamento.
// O clique roda stop.sh, que aperta o "Parar de compartilhar" da barra
// flutuante do Chrome (escondida pela regra em default/mango/windows.conf).
BarWidget {
  id: root
  moduleName: "doxia.screenshare"

  readonly property var nodes: Pipewire.nodes ? Pipewire.nodes.values : []
  readonly property int streamCount: {
    var n = 0
    for (var i = 0; i < nodes.length; i++)
      if (nodes[i] && String(nodes[i].name || "") === "xdg-desktop-portal-wlr") n++
    return n
  }
  readonly property bool sharing: streamCount > 0
  readonly property string pluginDir: String(Qt.resolvedUrl(".")).replace(/^file:\/\//, "").replace(/\/$/, "")

  visible: sharing
  implicitWidth: sharing ? button.implicitWidth : 0
  implicitHeight: sharing ? button.implicitHeight : 0

  // A barra flutuante do Chrome às vezes ganha o título depois de abrir e
  // escapa da regra do mango (que só vale na abertura); aqui ela é recolhida
  // de novo para a tag especial.
  function hideChromeBar() {
    Quickshell.execDetached(["sh", "-c",
      "mmsg get all-clients | jq -r '.clients[] | select(.title | test(\"(está compartilhando|is sharing) \")) "
      + "| select(.is_visible) | .id' | while read id; do "
      + "mmsg dispatch tag_special_silent \"client,$id\"; done"])
  }

  onSharingChanged: if (sharing) hideTimer.restart()

  Timer {
    id: hideTimer
    interval: 700
    repeat: true
    property int left: 6
    onRunningChanged: if (running) left = 6
    onTriggered: { root.hideChromeBar(); if (--left <= 0) stop() }
  }

  function stopSharing() {
    Quickshell.execDetached(["sh", root.pluginDir + "/stop.sh"])
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: String.fromCodePoint(0xF044A)
    foreground: "#e5252a"
    tooltipText: "Compartilhando a tela — clique para parar"
    onPressed: function(buttonCode) { root.stopSharing() }

    SequentialAnimation on opacity {
      running: root.sharing
      loops: Animation.Infinite
      NumberAnimation { to: 0.35; duration: 900; easing.type: Easing.InOutSine }
      NumberAnimation { to: 1.0; duration: 900; easing.type: Easing.InOutSine }
      onRunningChanged: if (!running) button.opacity = 1.0
    }
  }
}
