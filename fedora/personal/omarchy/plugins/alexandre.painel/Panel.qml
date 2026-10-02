import QtQuick
import Quickshell
import qs.Commons
import qs.Ui

// Raio na barra que abre o Painel Rápido (~/.local/bin/painel-rapido).
// Clique esquerdo: tela inicial; clique direito: direto no chamado SRE.
BarWidget {
  id: root
  moduleName: "alexandre.painel"

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: String.fromCodePoint(0xF140B)
    tooltipText: "Painel Rápido — botão direito: chamado SRE"
    onPressed: function(buttonCode) {
      var home = Quickshell.env("HOME")
      Quickshell.execDetached(buttonCode === Qt.RightButton
        ? [home + "/.local/bin/painel-rapido", "sre"]
        : [home + "/.local/bin/painel-rapido"])
    }
  }
}
