import QtQuick 2.0
import SddmComponents 2.0

// DoxIA login screen, like a game's title screen: an animated landscape that
// follows the clock and the weather (Landscape.qml), the brand and a large
// clock on the left, and the sign-in floating in the lower third with no card:
// the account (arrows switch between accounts), the password on a thin line,
// then "Outro usuário" and the session; the power actions in the bottom right
// corner.
//
// omarchy-plymouth-set recolors the upstream greeter's background and text hex
// codes (see bin/omarchy-plymouth-set) to the theme's colors. Neither appears in
// this file, not even in a comment, so this screen keeps its own palette.
Rectangle {
  id: root
  width: 1280
  height: 800
  color: "black"

  property string fontFamily: "Red Hat Text"
  property string displayFamily: "Red Hat Display"
  property color textColor: "#f2f2f2"
  property color dimColor: "#c9ccd1"
  property color accent: "#ee0000"
  property color accentHover: "#ff1a1a"
  property color lineColor: Qt.rgba(1, 1, 1, 0.45)
  property color chipHover: Qt.rgba(1, 1, 1, 0.14)
  property color errorColor: "#ff8a8a"

  property bool manualUser: false
  property bool loginFailed: false
  property bool sessionMenuOpen: false
  property int userIndex: userModel.lastIndex >= 0 ? userModel.lastIndex : 0
  property int sessionIndex: sessionModel.lastIndex >= 0 ? sessionModel.lastIndex : 0
  property Item selectedUser: users.count > 0 ? users.itemAt(userIndex) : null
  property Item selectedSession: sessionNames.count > 0 ? sessionNames.itemAt(sessionIndex) : null
  property string loginName: manualUser ? manualName.text : (selectedUser ? selectedUser.userName : "")
  property string shownName: manualUser ? manualName.text : (selectedUser ? selectedUser.displayName : "")

  function switchUser(step) {
    if (users.count < 2)
      return
    userIndex = (userIndex + step + users.count) % users.count
    manualUser = false
    loginFailed = false
    password.text = ""
    password.forceActiveFocus()
  }

  function chooseManualUser() {
    manualUser = true
    loginFailed = false
    password.text = ""
    manualName.text = ""
    manualName.forceActiveFocus()
  }

  function backToList() {
    manualUser = false
    sessionMenuOpen = false
    password.text = ""
    loginFailed = false
    password.forceActiveFocus()
  }

  function login() {
    if (loginName.length > 0)
      sddm.login(loginName, password.text, sessionIndex)
  }

  Connections {
    target: sddm
    function onLoginFailed() {
      root.loginFailed = true
      password.text = ""
      password.forceActiveFocus()
      shake.restart()
    }
    function onLoginSucceeded() {
      root.loginFailed = false
    }
  }

  // Accounts and session names, readable by index.
  Item {
    visible: false
    Repeater {
      id: users
      model: userModel
      Item {
        property string userName: model.name
        property string displayName: model.realName || model.name
        // SDDM's stock face is a dark silhouette lost on the background:
        // show the initial instead.
        property string avatar: (model.icon || "").indexOf("/.face.icon") === -1 || (model.icon || "").indexOf("/sddm/faces/") === -1 ? (model.icon || "") : ""
        property bool needsPassword: model.needsPassword
      }
    }
    Repeater {
      id: sessionNames
      model: sessionModel
      Item { property string sessionName: model.name }
    }
  }

  Landscape {
    anchors.fill: parent
  }

  // Shade under the clock and under the sign-in, so the text reads on a
  // bright daytime sky and on the grass.
  Rectangle {
    anchors.fill: parent
    gradient: Gradient {
      orientation: Gradient.Horizontal
      GradientStop { position: 0.0; color: Qt.rgba(0, 0, 0, 0.38) }
      GradientStop { position: 0.38; color: Qt.rgba(0, 0, 0, 0.06) }
      GradientStop { position: 1.0; color: Qt.rgba(0, 0, 0, 0.0) }
    }
  }
  Rectangle {
    width: parent.width
    height: parent.height * 0.45
    anchors.bottom: parent.bottom
    gradient: Gradient {
      GradientStop { position: 0.0; color: Qt.rgba(0, 0, 0, 0) }
      GradientStop { position: 1.0; color: Qt.rgba(0, 0, 0, 0.5) }
    }
  }

  // Clicking anywhere else closes the session menu.
  MouseArea {
    anchors.fill: parent
    enabled: root.sessionMenuOpen
    onClicked: root.sessionMenuOpen = false
  }

  // Top left: the brand, small.
  Image {
    id: brand
    source: "brand.png"
    height: 26
    opacity: 0.85
    width: sourceSize.height > 0 ? Math.round(height * sourceSize.width / sourceSize.height) : 0
    fillMode: Image.PreserveAspectFit
    smooth: true
    mipmap: true
    anchors.left: parent.left
    anchors.leftMargin: root.width * 0.07
    anchors.top: parent.top
    anchors.topMargin: 48
  }

  // Left: the clock and the date.
  Column {
    anchors.left: brand.left
    anchors.top: parent.top
    anchors.topMargin: root.height * 0.3
    spacing: 6

    Text {
      id: clock
      color: root.textColor
      font.family: root.displayFamily
      font.pixelSize: Math.round(root.height * 0.15)
      font.weight: Font.Light
      style: Text.Raised
      styleColor: Qt.rgba(0, 0, 0, 0.35)
    }

    Rectangle {
      width: 56
      height: 4
      radius: 2
      color: root.accent
    }

    Item { width: 1; height: 6 }

    Text {
      id: date
      color: root.textColor
      opacity: 0.85
      style: Text.Raised
      styleColor: Qt.rgba(0, 0, 0, 0.35)
      font.family: root.displayFamily
      font.pixelSize: 22
    }

    Timer {
      interval: 1000
      running: true
      repeat: true
      triggeredOnStart: true
      onTriggered: {
        var now = new Date()
        clock.text = now.toLocaleString(Qt.locale("pt_BR"), "HH:mm")
        date.text = now.toLocaleString(Qt.locale("pt_BR"), "dddd, d 'de' MMMM")
      }
    }
  }

  // The sign-in, floating in the lower third.
  Column {
    id: signIn
    width: 380
    anchors.horizontalCenter: parent.horizontalCenter
    anchors.bottom: parent.bottom
    anchors.bottomMargin: root.height * 0.1
    spacing: 14

    // Shakes on a wrong password.
    transform: Translate { id: shakeOffset }
    SequentialAnimation {
      id: shake
      NumberAnimation { target: shakeOffset; property: "x"; to: -10; duration: 50 }
      NumberAnimation { target: shakeOffset; property: "x"; to: 10; duration: 70 }
      NumberAnimation { target: shakeOffset; property: "x"; to: -6; duration: 60 }
      NumberAnimation { target: shakeOffset; property: "x"; to: 0; duration: 50 }
    }

    // The account, with arrows when there is more than one.
    Item {
      visible: !root.manualUser
      width: parent.width
      height: 96

      Avatar {
        id: avatar
        size: 88
        anchors.centerIn: parent
        source: root.selectedUser ? root.selectedUser.avatar : ""
        initial: root.shownName
      }

      ArrowButton {
        visible: users.count > 1
        label: "‹"
        anchors.right: avatar.left
        anchors.rightMargin: 28
        anchors.verticalCenter: parent.verticalCenter
        onClicked: root.switchUser(-1)
      }

      ArrowButton {
        visible: users.count > 1
        label: "›"
        anchors.left: avatar.right
        anchors.leftMargin: 28
        anchors.verticalCenter: parent.verticalCenter
        onClicked: root.switchUser(1)
      }
    }

    Text {
      visible: !root.manualUser
      width: parent.width
      horizontalAlignment: Text.AlignHCenter
      text: root.shownName
      color: root.textColor
      elide: Text.ElideRight
      style: Text.Raised
      styleColor: Qt.rgba(0, 0, 0, 0.35)
      font.family: root.displayFamily
      font.pixelSize: 24
      font.bold: true
    }

    // Someone not in the list: the name on its own line.
    Line {
      visible: root.manualUser
      input: manualName
      placeholder: "Nome de usuário"

      TextInput {
        id: manualName
        anchors.fill: parent
        anchors.rightMargin: 44
        verticalAlignment: TextInput.AlignVCenter
        horizontalAlignment: TextInput.AlignHCenter
        color: root.textColor
        selectionColor: root.accent
        font.family: root.fontFamily
        font.pixelSize: 18
        clip: true

        Keys.onReturnPressed: password.forceActiveFocus()
        Keys.onEnterPressed: password.forceActiveFocus()
        Keys.onEscapePressed: root.backToList()
      }
    }

    Line {
      input: password
      failed: root.loginFailed
      placeholder: root.selectedUser && !root.selectedUser.needsPassword && !root.manualUser ? "Enter para entrar" : "Senha"

      TextInput {
        id: password
        anchors.fill: parent
        anchors.leftMargin: 44
        anchors.rightMargin: 44
        verticalAlignment: TextInput.AlignVCenter
        horizontalAlignment: TextInput.AlignHCenter
        echoMode: TextInput.Password
        passwordCharacter: "●"
        color: root.textColor
        selectionColor: root.accent
        font.family: root.fontFamily
        font.pixelSize: 16
        font.letterSpacing: 3
        clip: true
        focus: true

        onTextChanged: root.loginFailed = false

        Keys.onPressed: function (event) {
          if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            root.sessionMenuOpen = false
            root.login()
            event.accepted = true
          } else if (event.key === Qt.Key_Escape) {
            if (root.sessionMenuOpen)
              root.sessionMenuOpen = false
            else if (root.manualUser)
              root.backToList()
            else
              password.text = ""
            event.accepted = true
          } else if (event.key === Qt.Key_Tab) {
            root.sessionMenuOpen = !root.sessionMenuOpen
            event.accepted = true
          } else if (root.sessionMenuOpen && event.key === Qt.Key_Up) {
            root.sessionIndex = Math.max(0, root.sessionIndex - 1)
            event.accepted = true
          } else if (root.sessionMenuOpen && event.key === Qt.Key_Down) {
            root.sessionIndex = Math.min(sessionNames.count - 1, root.sessionIndex + 1)
            event.accepted = true
          } else if (!root.manualUser && password.text.length === 0 && event.key === Qt.Key_Left) {
            root.switchUser(-1)
            event.accepted = true
          } else if (!root.manualUser && password.text.length === 0 && event.key === Qt.Key_Right) {
            root.switchUser(1)
            event.accepted = true
          }
        }
      }

      // Submit, at the line's right end.
      Rectangle {
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        width: 34
        height: 34
        radius: 17
        color: submitMouse.containsMouse ? root.accentHover : root.accent
        opacity: password.text.length > 0 || submitMouse.containsMouse ? 1 : 0.75

        Text {
          anchors.centerIn: parent
          text: "→"
          color: root.textColor
          font.family: root.fontFamily
          font.pixelSize: 17
          font.bold: true
        }

        MouseArea {
          id: submitMouse
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: root.login()
        }
      }
    }

    Text {
      width: parent.width
      height: 16
      horizontalAlignment: Text.AlignHCenter
      text: root.loginFailed ? "Senha incorreta. Tente novamente." : ""
      color: root.errorColor
      style: Text.Raised
      styleColor: Qt.rgba(0, 0, 0, 0.4)
      font.family: root.fontFamily
      font.pixelSize: 13
    }

    // "Outro usuário" · the session.
    Row {
      anchors.horizontalCenter: parent.horizontalCenter
      spacing: 6

      LinkText {
        text: root.manualUser ? "Voltar às contas" : "Outro usuário"
        onClicked: root.manualUser ? root.backToList() : root.chooseManualUser()
      }

      Text {
        text: "·"
        color: root.dimColor
        font.family: root.fontFamily
        font.pixelSize: 13
        anchors.verticalCenter: parent.verticalCenter
      }

      LinkText {
        text: "Sessão: " + (root.selectedSession ? root.selectedSession.sessionName : "") + "  ▾"
        active: root.sessionMenuOpen
        onClicked: {
          root.sessionMenuOpen = !root.sessionMenuOpen
          password.forceActiveFocus()
        }
      }
    }
  }

  Popup {
    id: sessionMenu
    visible: root.sessionMenuOpen
    width: 260
    x: (root.width - width) / 2
    y: signIn.y + signIn.height - height - 40
    entries: {
      var list = []
      for (var i = 0; i < sessionNames.count; i++) {
        list.push({
          label: sessionNames.itemAt(i).sessionName,
          enabled: true,
          checked: i === root.sessionIndex,
          index: i
        })
      }
      return list
    }
    onPicked: function (entry) {
      root.sessionIndex = entry.index
      root.sessionMenuOpen = false
      password.forceActiveFocus()
    }
  }

  Text {
    anchors.left: brand.left
    anchors.bottom: parent.bottom
    anchors.bottomMargin: 30
    text: sddm.hostName
    color: root.dimColor
    opacity: 0.7
    font.family: root.fontFamily
    font.pixelSize: 13
  }

  // Bottom right: power actions, always visible.
  Row {
    anchors.right: parent.right
    anchors.rightMargin: root.width * 0.04
    anchors.bottom: parent.bottom
    anchors.bottomMargin: 24
    spacing: 4

    PowerButton { label: "Suspender"; visible: sddm.canSuspend; onClicked: sddm.suspend() }
    PowerButton { label: "Reiniciar"; visible: sddm.canReboot; onClicked: sddm.reboot() }
    PowerButton { label: "Desligar"; icon: "power.png"; visible: sddm.canPowerOff; onClicked: sddm.powerOff() }
  }

  // A text field drawn as a thin line, like a game menu's.
  component Line: Item {
    property Item input
    property bool failed: false
    property string placeholder: ""

    width: parent ? parent.width : 380
    height: 44

    Text {
      anchors.centerIn: parent
      visible: parent.input && parent.input.text.length === 0
      text: parent.placeholder
      color: root.dimColor
      opacity: 0.8
      style: Text.Raised
      styleColor: Qt.rgba(0, 0, 0, 0.35)
      font.family: root.fontFamily
      font.pixelSize: 15
    }

    Rectangle {
      anchors.bottom: parent.bottom
      anchors.horizontalCenter: parent.horizontalCenter
      width: parent.input && parent.input.activeFocus ? parent.width : parent.width * 0.8
      height: parent.input && parent.input.activeFocus ? 2 : 1
      color: parent.failed ? root.errorColor : (parent.input && parent.input.activeFocus ? root.textColor : root.lineColor)
      Behavior on width { NumberAnimation { duration: 180; easing.type: Easing.OutCubic } }
    }
  }

  component ArrowButton: Rectangle {
    id: arrowButton
    property string label: ""
    signal clicked()

    width: 36
    height: 36
    radius: 18
    color: arrowMouse.containsMouse ? root.chipHover : "transparent"

    Text {
      anchors.centerIn: parent
      anchors.verticalCenterOffset: -2
      text: arrowButton.label
      color: root.textColor
      font.family: root.fontFamily
      font.pixelSize: 28
    }

    MouseArea {
      id: arrowMouse
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: arrowButton.clicked()
    }
  }

  component LinkText: Text {
    id: link
    property bool active: false
    signal clicked()

    color: linkMouse.containsMouse || active ? root.textColor : root.dimColor
    style: Text.Raised
    styleColor: Qt.rgba(0, 0, 0, 0.35)
    font.family: root.fontFamily
    font.pixelSize: 13
    font.underline: linkMouse.containsMouse

    MouseArea {
      id: linkMouse
      anchors.fill: parent
      anchors.margins: -6
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: link.clicked()
    }
  }

  component PowerButton: Rectangle {
    id: powerButton
    property string label: ""
    property string icon: ""
    signal clicked()

    width: powerRow.width + 24
    height: 32
    radius: 16
    color: powerMouse.containsMouse ? root.chipHover : "transparent"

    Row {
      id: powerRow
      anchors.centerIn: parent
      spacing: 6

      Image {
        visible: powerButton.icon !== ""
        source: powerButton.icon
        width: 16
        height: 16
        smooth: true
        mipmap: true
        anchors.verticalCenter: parent.verticalCenter
      }
      Text {
        text: powerButton.label
        color: powerMouse.containsMouse ? root.textColor : root.dimColor
        style: Text.Raised
        styleColor: Qt.rgba(0, 0, 0, 0.35)
        font.family: root.fontFamily
        font.pixelSize: 13
        anchors.verticalCenter: parent.verticalCenter
      }
    }

    MouseArea {
      id: powerMouse
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: powerButton.clicked()
    }
  }

  component Popup: Rectangle {
    id: popup
    property var entries: []
    signal picked(var entry)

    z: 20
    height: popupColumn.height + 12
    radius: 12
    color: Qt.rgba(0.08, 0.09, 0.11, 0.85)
    border.color: Qt.rgba(1, 1, 1, 0.14)
    border.width: 1

    Column {
      id: popupColumn
      anchors.centerIn: parent
      width: parent.width - 12

      Repeater {
        model: popup.entries

        Item {
          width: popupColumn.width
          height: 36
          opacity: modelData.enabled ? 1 : 0.4

          Rectangle {
            anchors.fill: parent
            radius: 8
            color: root.textColor
            opacity: entryMouse.containsMouse && modelData.enabled ? 0.08 : 0
          }

          Text {
            anchors.left: parent.left
            anchors.leftMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            text: (modelData.checked ? "✓  " : "    ") + modelData.label
            color: root.textColor
            font.family: root.fontFamily
            font.pixelSize: 14
          }

          MouseArea {
            id: entryMouse
            anchors.fill: parent
            hoverEnabled: true
            enabled: modelData.enabled
            cursorShape: Qt.PointingHandCursor
            onClicked: {
              popup.picked(modelData)
              if (modelData.action)
                modelData.action()
            }
          }
        }
      }
    }
  }

  component Avatar: Item {
    property int size: 64
    property string source: ""
    property string initial: ""

    width: size
    height: size

    // A soft ring, so the avatar stands off the landscape.
    Rectangle {
      anchors.centerIn: parent
      width: parent.size + 10
      height: width
      radius: width / 2
      color: "transparent"
      border.width: 2
      border.color: Qt.rgba(1, 1, 1, 0.5)
    }

    Rectangle {
      anchors.fill: parent
      radius: width / 2
      color: Qt.rgba(0.1, 0.11, 0.13, 0.7)
      visible: face.status !== Image.Ready
    }

    Text {
      anchors.centerIn: parent
      visible: face.status !== Image.Ready
      text: parent.initial.charAt(0).toUpperCase()
      color: root.textColor
      font.family: root.fontFamily
      font.pixelSize: parent.size * 0.42
      font.bold: true
    }

    Image {
      id: face
      anchors.fill: parent
      source: parent.source
      sourceSize.width: parent.size
      sourceSize.height: parent.size
      fillMode: Image.PreserveAspectCrop
      visible: status === Image.Ready
    }
  }

  Component.onCompleted: password.forceActiveFocus()
}
