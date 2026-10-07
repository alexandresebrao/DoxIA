import QtQuick
import Quickshell
import qs.Commons
import qs.Ui

// Raio na barra que abre o Painel Rápido (fedora/doxia/painel, doxia-painel no PATH).
// Clique esquerdo: tela inicial; clique direito: a ação "favorite" do painel.json.
BarWidget {
  id: root
  moduleName: "doxia.painel"

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: String.fromCodePoint(0xF140B)
    tooltipText: "Painel Rápido — botão direito: ação favorita"
    onPressed: function(buttonCode) {
      var painel = Quickshell.env("OMARCHY_PATH") + "/fedora/doxia/painel"
      Quickshell.execDetached(buttonCode === Qt.RightButton ? [painel, "--favorito"] : [painel])
    }
  }
}
