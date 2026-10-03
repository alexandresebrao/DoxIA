import QtQuick 2.0
import SddmComponents 2.0

// FedorAI login screen, modeled on the RHEL 8 GDM greeter: black top bar with the
// clock and power menu, the user list in the middle, a password step with Cancel /
// Sign in and a gear for the session, and the ∞ at the bottom where RHEL puts its logo.
//
// omarchy-plymouth-set recolors #1a1b26 and #ffffff in this file to the theme's
// colors, so neither appears here: this screen keeps its RHEL 8 palette.
Rectangle {
  id: root
  width: 1280
  height: 800

  property string fontFamily: "Red Hat Text"
  property color textColor: "#f2f2f2"
  property color dimColor: "#a9abae"
  property color accent: "#ee0000"
  property color accentHover: "#ff1a1a"
  property color fieldColor: "#1c1d1f"
  property color fieldBorder: "#55585c"
  property color buttonColor: "#3a3d41"
  property color buttonHover: "#46494e"
  property color errorColor: "#ff6c6c"

  property bool choosingUser: true
  property bool manualUser: false
  property bool loginFailed: false
  property bool sessionMenuOpen: false
  property bool powerMenuOpen: false
  property int userIndex: userModel.lastIndex >= 0 ? userModel.lastIndex : 0
  property int sessionIndex: sessionModel.lastIndex >= 0 ? sessionModel.lastIndex : 0
  property Item selectedUser: users.count > 0 ? users.itemAt(userIndex) : null
  property Item selectedSession: sessionNames.count > 0 ? sessionNames.itemAt(sessionIndex) : null
  property string loginName: manualUser ? manualName.text : (selectedUser ? selectedUser.userName : "")
  property string shownName: manualUser ? manualName.text : (selectedUser ? selectedUser.displayName : "")

  gradient: Gradient {
    GradientStop { position: 0.0; color: "#303338" }
    GradientStop { position: 1.0; color: "#1b1d20" }
  }

  function chooseUser(index) {
    userIndex = index
    manualUser = false
    loginFailed = false
    password.text = ""
    choosingUser = false
    if (selectedUser && !selectedUser.needsPassword)
      login()
    else
      password.forceActiveFocus()
  }

  function chooseManualUser() {
    manualUser = true
    loginFailed = false
    password.text = ""
    manualName.text = ""
    choosingUser = false
    manualName.forceActiveFocus()
  }

  function backToUsers() {
    choosingUser = true
    manualUser = false
    sessionMenuOpen = false
    password.text = ""
    loginFailed = false
    userList.forceActiveFocus()
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
    }
    function onLoginSucceeded() {
      root.loginFailed = false
    }
  }

  // Session names, readable by index outside a delegate.
  Item {
    visible: false
    Repeater {
      id: sessionNames
      model: sessionModel
      Item { property string sessionName: model.name }
    }
  }

  // Clicking anywhere else closes an open menu.
  MouseArea {
    anchors.fill: parent
    enabled: root.sessionMenuOpen || root.powerMenuOpen
    onClicked: {
      root.sessionMenuOpen = false
      root.powerMenuOpen = false
    }
  }

  // Top bar: clock in the middle, power menu on the right.
  Rectangle {
    id: topBar
    z: 5
    anchors.top: parent.top
    anchors.left: parent.left
    anchors.right: parent.right
    height: 34
    color: "#000000"

    Text {
      id: clock
      anchors.centerIn: parent
      color: root.textColor
      font.family: root.fontFamily
      font.pixelSize: 15
      font.bold: true

      function update() {
        var label = new Date().toLocaleString(Qt.locale("pt_BR"), "ddd d 'de' MMM  HH:mm")
        text = label.replace(/\./g, "")
      }

      Timer {
        interval: 1000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: clock.update()
      }
    }

    Rectangle {
      id: powerButton
      anchors.right: parent.right
      anchors.rightMargin: 8
      anchors.verticalCenter: parent.verticalCenter
      width: powerRow.width + 20
      height: 26
      radius: 13
      color: powerMouse.containsMouse || root.powerMenuOpen ? "#2a2a2a" : "transparent"

      Row {
        id: powerRow
        anchors.centerIn: parent
        spacing: 6

        Image {
          source: "power.png"
          width: 18
          height: 18
          smooth: true
          mipmap: true
          anchors.verticalCenter: parent.verticalCenter
        }
        Text {
          text: "▾"
          color: root.textColor
          font.pixelSize: 11
          anchors.verticalCenter: parent.verticalCenter
        }
      }

      MouseArea {
        id: powerMouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: {
          root.sessionMenuOpen = false
          root.powerMenuOpen = !root.powerMenuOpen
        }
      }
    }
  }

  Popup {
    id: powerMenu
    visible: root.powerMenuOpen
    anchors.top: topBar.bottom
    anchors.topMargin: 6
    anchors.right: parent.right
    anchors.rightMargin: 8
    width: 220
    entries: [
      { label: "Suspender", enabled: sddm.canSuspend, action: function () { sddm.suspend() } },
      { label: "Reiniciar", enabled: sddm.canReboot, action: function () { sddm.reboot() } },
      { label: "Desligar", enabled: sddm.canPowerOff, action: function () { sddm.powerOff() } }
    ]
    onPicked: root.powerMenuOpen = false
  }

  // Middle: user list, then the password step.
  Item {
    id: center
    width: 380
    height: root.choosingUser ? userList.height : passwordStep.height
    anchors.horizontalCenter: parent.horizontalCenter
    anchors.verticalCenter: parent.verticalCenter
    anchors.verticalCenterOffset: -20

    // Step 1: pick a user.
    FocusScope {
      id: userList
      visible: root.choosingUser
      width: parent.width
      height: userColumn.height
      focus: root.choosingUser

      Keys.onUpPressed: root.userIndex = Math.max(0, root.userIndex - 1)
      Keys.onDownPressed: root.userIndex = Math.min(users.count - 1, root.userIndex + 1)
      Keys.onReturnPressed: root.chooseUser(root.userIndex)
      Keys.onEnterPressed: root.chooseUser(root.userIndex)

      Column {
        id: userColumn
        width: parent.width
        spacing: 4

        Repeater {
          id: users
          model: userModel

          Item {
            id: userRow
            property string userName: model.name
            property string displayName: model.realName || model.name
            // SDDM's stock face is a dark silhouette lost on the background:
            // show the initial instead.
            property string avatar: (model.icon || "").indexOf("/.face.icon") === -1 || (model.icon || "").indexOf("/sddm/faces/") === -1 ? (model.icon || "") : ""
            property bool needsPassword: model.needsPassword
            property bool current: index === root.userIndex

            width: userColumn.width
            height: 84

            Rectangle {
              anchors.fill: parent
              radius: 10
              color: root.textColor
              opacity: userRow.current ? 0.10 : (rowMouse.containsMouse ? 0.05 : 0)
            }

            Avatar {
              id: rowAvatar
              size: 64
              anchors.left: parent.left
              anchors.leftMargin: 12
              anchors.verticalCenter: parent.verticalCenter
              source: userRow.avatar
              initial: userRow.displayName
            }

            Text {
              anchors.left: rowAvatar.right
              anchors.leftMargin: 18
              anchors.right: parent.right
              anchors.rightMargin: 12
              anchors.verticalCenter: parent.verticalCenter
              text: userRow.displayName
              color: root.textColor
              elide: Text.ElideRight
              font.family: root.fontFamily
              font.pixelSize: 19
              font.bold: true
            }

            MouseArea {
              id: rowMouse
              anchors.fill: parent
              hoverEnabled: true
              cursorShape: Qt.PointingHandCursor
              onClicked: root.chooseUser(index)
            }
          }
        }

        Item { width: 1; height: 14 }

        Text {
          text: "Não está na lista?"
          color: notListedMouse.containsMouse ? root.textColor : root.dimColor
          font.family: root.fontFamily
          font.pixelSize: 14
          font.underline: notListedMouse.containsMouse
          anchors.horizontalCenter: parent.horizontalCenter

          MouseArea {
            id: notListedMouse
            anchors.fill: parent
            anchors.margins: -6
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.chooseManualUser()
          }
        }
      }
    }

    // Step 2: password (and the user name, for someone not in the list).
    Column {
      id: passwordStep
      visible: !root.choosingUser
      width: parent.width
      spacing: 14

      Row {
        visible: !root.manualUser
        spacing: 18

        Avatar {
          size: 64
          anchors.verticalCenter: parent.verticalCenter
          source: root.selectedUser ? root.selectedUser.avatar : ""
          initial: root.shownName
        }

        Text {
          anchors.verticalCenter: parent.verticalCenter
          text: root.shownName
          color: root.textColor
          font.family: root.fontFamily
          font.pixelSize: 19
          font.bold: true
        }
      }

      Item { width: 1; height: 6 }

      Text {
        visible: root.manualUser
        text: "Nome de usuário"
        color: root.textColor
        font.family: root.fontFamily
        font.pixelSize: 14
      }

      Field {
        visible: root.manualUser
        input: manualName

        TextInput {
          id: manualName
          anchors.fill: parent
          anchors.leftMargin: 12
          anchors.rightMargin: 12
          verticalAlignment: TextInput.AlignVCenter
          color: root.textColor
          selectionColor: root.accent
          font.family: root.fontFamily
          font.pixelSize: 16
          clip: true

          Keys.onReturnPressed: password.forceActiveFocus()
          Keys.onEnterPressed: password.forceActiveFocus()
          Keys.onEscapePressed: root.backToUsers()
        }
      }

      Text {
        text: "Senha"
        color: root.textColor
        font.family: root.fontFamily
        font.pixelSize: 14
      }

      Field {
        input: password
        failed: root.loginFailed

        TextInput {
          id: password
          anchors.fill: parent
          anchors.leftMargin: 12
          anchors.rightMargin: 12
          verticalAlignment: TextInput.AlignVCenter
          echoMode: TextInput.Password
          passwordCharacter: "●"
          color: root.textColor
          selectionColor: root.accent
          font.family: root.fontFamily
          font.pixelSize: 14
          font.letterSpacing: 2
          clip: true

          onTextChanged: root.loginFailed = false

          Keys.onPressed: function (event) {
            if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
              root.sessionMenuOpen = false
              root.login()
              event.accepted = true
            } else if (event.key === Qt.Key_Escape) {
              if (root.sessionMenuOpen)
                root.sessionMenuOpen = false
              else
                root.backToUsers()
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
            }
          }
        }
      }

      Text {
        width: parent.width
        height: 20
        text: root.loginFailed ? "Desculpe, isso não funcionou. Tente novamente." : ""
        color: root.errorColor
        font.family: root.fontFamily
        font.pixelSize: 14
      }

      Item {
        width: parent.width
        height: 36

        Button {
          anchors.left: parent.left
          label: "Cancelar"
          onClicked: root.backToUsers()
        }

        Row {
          anchors.right: parent.right
          spacing: 10

          Button {
            id: gear
            icon: "gear.png"
            fixedWidth: 40
            highlighted: root.sessionMenuOpen
            onClicked: {
              root.powerMenuOpen = false
              root.sessionMenuOpen = !root.sessionMenuOpen
              password.forceActiveFocus()
            }
          }

          Button {
            label: "Entrar"
            primary: true
            onClicked: root.login()
          }
        }
      }

      Text {
        width: parent.width
        horizontalAlignment: Text.AlignRight
        text: root.selectedSession ? root.selectedSession.sessionName : ""
        color: root.dimColor
        font.family: root.fontFamily
        font.pixelSize: 12
      }
    }
  }

  // Session menu, opening under the gear.
  Popup {
    id: sessionMenu
    visible: root.sessionMenuOpen && !root.choosingUser
    x: center.x + center.width - width
    y: center.y + passwordStep.height + 6
    width: 280
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

  // Bottom: the ∞, where RHEL 8 shows its logo.
  Image {
    source: "infinito.png"
    height: 64
    width: sourceSize.height > 0 ? Math.round(height * sourceSize.width / sourceSize.height) : 0
    fillMode: Image.PreserveAspectFit
    smooth: true
    mipmap: true
    anchors.horizontalCenter: parent.horizontalCenter
    anchors.bottom: parent.bottom
    anchors.bottomMargin: 48
  }

  component Field: Rectangle {
    property Item input
    property bool failed: false

    width: parent ? parent.width : 380
    height: 40
    radius: 6
    color: root.fieldColor
    border.width: input && input.activeFocus ? 2 : 1
    border.color: failed ? root.errorColor : (input && input.activeFocus ? root.accent : root.fieldBorder)
  }

  component Button: Rectangle {
    id: button
    property string label: ""
    property string icon: ""
    property int labelSize: 14
    property int fixedWidth: 0
    property bool primary: false
    property bool highlighted: false
    signal clicked()

    width: fixedWidth > 0 ? fixedWidth : buttonText.implicitWidth + 36
    height: 36
    radius: 6
    color: primary ? (buttonMouse.containsMouse ? root.accentHover : root.accent)
                   : (buttonMouse.containsMouse || highlighted ? root.buttonHover : root.buttonColor)

    Image {
      visible: button.icon !== ""
      source: button.icon
      width: 20
      height: 20
      smooth: true
      mipmap: true
      anchors.centerIn: parent
    }

    Text {
      id: buttonText
      anchors.centerIn: parent
      text: button.label
      color: root.textColor
      font.family: root.fontFamily
      font.pixelSize: button.labelSize
      font.bold: button.primary
    }

    MouseArea {
      id: buttonMouse
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: button.clicked()
    }
  }

  component Popup: Rectangle {
    id: popup
    property var entries: []
    signal picked(var entry)

    z: 20
    height: popupColumn.height + 12
    radius: 10
    color: "#262729"
    border.color: "#3e4044"
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
            radius: 6
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

    Rectangle {
      anchors.fill: parent
      radius: width / 2
      color: "#4b4f55"
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

  Component.onCompleted: userList.forceActiveFocus()
}
