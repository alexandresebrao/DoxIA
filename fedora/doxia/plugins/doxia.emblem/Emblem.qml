import QtQuick
import Quickshell
import qs.Commons
import qs.Ui

// Botão do menu Omarchy com o emblem.png do tema atual (o ∞ do DoxIA); sem ele,
// o glifo do Omarchy.
BarWidget {
  id: root
  moduleName: "doxia.emblem"

  readonly property string emblemPath: "file://" + Quickshell.env("HOME") + "/.local/state/omarchy/current/theme/emblem.png"
  readonly property real unit: Math.max(14, barSize - 8)
  implicitWidth: vertical ? barSize : Math.max(emblem.width, fallback.implicitWidth) + 14
  implicitHeight: barSize

  // O tema é trocado substituindo o diretório: recarrega a imagem quando a cor muda.
  Connections {
    target: Color
    function onAccentChanged() {
      emblem.source = ""
      emblem.source = root.emblemPath
    }
  }

  Image {
    id: emblem
    anchors.centerIn: parent
    visible: status === Image.Ready
    source: root.emblemPath
    cache: false
    height: Math.round(root.unit * 0.78)
    width: visible ? implicitWidth * height / Math.max(1, implicitHeight) : 0
    sourceSize.height: root.unit * 2
    fillMode: Image.PreserveAspectFit
    smooth: true
    mipmap: true
    opacity: mouse.containsMouse ? 1 : 0.9
    Behavior on opacity { NumberAnimation { duration: 160 } }
  }

  Text {
    id: fallback
    anchors.centerIn: parent
    visible: !emblem.visible
    text: ""
    color: bar ? bar.barForeground : Color.foreground
    font.family: "omarchy"
    font.pixelSize: Math.round(root.barSize * 0.55)
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
