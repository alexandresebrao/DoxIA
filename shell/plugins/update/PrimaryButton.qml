import QtQuick
import qs.Commons

// The one filled action of the update window and the bar popup. Ui/Button
// swaps its fill for a translucent tint on hover, which loses a solid accent
// background; this keeps the accent and white bold text (about 6:1 on the
// darkened red) in every state.
Rectangle {
  id: root

  property string text: ""
  property string fontFamily: Style.font.family
  property real fontSize: Style.font.body
  signal clicked()

  readonly property color base: Qt.darker(Color.accent, 1.25)

  implicitWidth: label.implicitWidth + Style.spacing.controlPaddingX * 2 + Style.space(4)
  implicitHeight: label.implicitHeight + Style.spacing.controlPaddingY * 2 + Style.space(2)
  radius: Style.cornerRadius > 0 ? Style.space(4) : 0
  color: mouse.pressed ? Qt.darker(base, 1.2) : mouse.containsMouse ? Color.accent : base
  border.width: activeFocus ? 2 : 0
  border.color: "#ffffff"
  activeFocusOnTab: true

  Behavior on color { ColorAnimation { duration: 120 } }

  Keys.onReturnPressed: root.clicked()
  Keys.onSpacePressed: root.clicked()

  Text {
    id: label
    anchors.centerIn: parent
    textFormat: Text.PlainText
    text: root.text
    color: "#ffffff"
    font.family: root.fontFamily
    font.pixelSize: root.fontSize
    font.bold: true
  }

  MouseArea {
    id: mouse
    anchors.fill: parent
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor
    onClicked: root.clicked()
  }
}
