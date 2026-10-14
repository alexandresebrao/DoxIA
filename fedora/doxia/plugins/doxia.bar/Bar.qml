import QtQuick

// A barra é do doxia-shell (fedora/doxia/rs/shell/src/bar): selecionada em
// shell.json (bar.id), esta toma o lugar da barra do Quickshell sem desenhar
// nada. O que o Quickshell lê da barra fica no "escondida".
Item {
  property var omarchyPath
  property var shell
  property var manifest
  property var barWidgetRegistry
  property var pluginRegistry
  property var barConfig

  readonly property bool barHidden: true
  readonly property int barSize: 0
  readonly property string position: "top"
  readonly property bool vertical: false
  readonly property string fontFamily: ""
}
