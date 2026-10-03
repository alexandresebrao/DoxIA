import QtQuick 2.0
import SddmComponents 2.0

// DoxIA login screen: a split layout with the brand, a large clock and the date on
// the left, the sign-in card on the right (pick an account, then type the password
// with the session chip under it), the power actions in the bottom right corner and
// the ∞ as a faint watermark bleeding off the bottom edge.
//
// omarchy-plymouth-set recolors #1a1b26 and #ffffff in this file to the theme's
// colors, so neither appears here: this screen keeps its own palette.
Rectangle {
  id: root
  width: 1280
  height: 800

  property string fontFamily: "Red Hat Text"
  property string displayFamily: "Red Hat Display"
  property color textColor: "#f2f2f2"
  property color dimColor: "#a9abae"
  property color accent: "#ee0000"
  property color accentHover: "#ff1a1a"
  property color cardColor: "#1a1c1f"
  property color cardBorder: "#33363a"
  property color fieldColor: "#121315"
  property color fieldBorder: "#4a4d51"
  property color chipColor: "#26292d"
  property color chipHover: "#30343a"
  property color errorColor: "#ff6c6c"

  property bool choosingUser: true
  property bool manualUser: false
  property bool loginFailed: false
  property bool sessionMenuOpen: false
  property int userIndex: userModel.lastIndex >= 0 ? userModel.lastIndex : 0
  property int sessionIndex: sessionModel.lastIndex >= 0 ? sessionModel.lastIndex : 0
  property Item selectedUser: users.count > 0 ? users.itemAt(userIndex) : null
  property Item selectedSession: sessionNames.count > 0 ? sessionNames.itemAt(sessionIndex) : null
  property string loginName: manualUser ? manualName.text : (selectedUser ? selectedUser.userName : "")
  property string shownName: manualUser ? manualName.text : (selectedUser ? selectedUser.displayName : "")

  gradient: Gradient {
    orientation: Gradient.Horizontal
    GradientStop { position: 0.0; color: "#101112" }
    GradientStop { position: 0.55; color: "#1d1f22" }
    GradientStop { position: 1.0; color: "#26292d" }
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

  // The ∞ watermark, cut by the bottom edge under the clock.
  Image {
    source: "infinity.png"
    width: root.width * 0.62
    height: sourceSize.width > 0 ? Math.round(width * sourceSize.height / sourceSize.width) : 0
    x: -width * 0.12
    y: root.height - height * 0.62
    opacity: 0.06
    smooth: true
    mipmap: true
  }

  // Clicking anywhere else closes the session menu.
  MouseArea {
    anchors.fill: parent
    enabled: root.sessionMenuOpen
    onClicked: root.sessionMenuOpen = false
  }

  // Left: brand on top, then the clock and the date.
  Image {
    id: brand
    source: "brand.png"
    height: 38
    width: sourceSize.height > 0 ? Math.round(height * sourceSize.width / sourceSize.height) : 0
    fillMode: Image.PreserveAspectFit
    smooth: true
    mipmap: true
    anchors.left: parent.left
    anchors.leftMargin: root.width * 0.07
    anchors.top: parent.top
    anchors.topMargin: 48
  }

  Column {
    anchors.left: brand.left
    anchors.verticalCenter: parent.verticalCenter
    spacing: 6

    Text {
      id: clock
      color: root.textColor
      font.family: root.displayFamily
      font.pixelSize: Math.round(root.height * 0.15)
      font.weight: Font.Light
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
      color: root.dimColor
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

  Text {
    anchors.left: brand.left
    anchors.bottom: parent.bottom
    anchors.bottomMargin: 36
    text: sddm.hostName
    color: root.dimColor
    opacity: 0.7
    font.family: root.fontFamily
    font.pixelSize: 13
  }

  // Right: the sign-in card.
  Rectangle {
    id: card
    width: 400
    height: (root.choosingUser ? userList.height : passwordStep.height) + 56
    anchors.right: parent.right
    anchors.rightMargin: root.width * 0.09
    anchors.verticalCenter: parent.verticalCenter
    radius: 18
    color: root.cardColor
    border.color: root.cardBorder
    border.width: 1

    // Step 1: pick an account.
    FocusScope {
      id: userList
      visible: root.choosingUser
      x: 28
      y: 28
      width: parent.width - 56
      height: userColumn.height
      focus: root.choosingUser

      Keys.onUpPressed: root.userIndex = Math.max(0, root.userIndex - 1)
      Keys.onDownPressed: root.userIndex = Math.min(users.count - 1, root.userIndex + 1)
      Keys.onReturnPressed: root.chooseUser(root.userIndex)
      Keys.onEnterPressed: root.chooseUser(root.userIndex)

      Column {
        id: userColumn
        width: parent.width
        spacing: 6

        Text {
          text: "Bem-vindo"
          color: root.textColor
          font.family: root.displayFamily
          font.pixelSize: 26
          font.bold: true
        }

        Text {
          text: "Escolha uma conta para entrar"
          color: root.dimColor
          font.family: root.fontFamily
          font.pixelSize: 14
        }

        Item { width: 1; height: 12 }

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
            height: 64

            Rectangle {
              anchors.fill: parent
              radius: 12
              color: userRow.current || rowMouse.containsMouse ? root.chipHover : root.chipColor
              border.width: userRow.current ? 1 : 0
              border.color: root.accent
            }

            Avatar {
              id: rowAvatar
              size: 44
              anchors.left: parent.left
              anchors.leftMargin: 12
              anchors.verticalCenter: parent.verticalCenter
              source: userRow.avatar
              initial: userRow.displayName
            }

            Text {
              anchors.left: rowAvatar.right
              anchors.leftMargin: 14
              anchors.right: arrow.left
              anchors.rightMargin: 8
              anchors.verticalCenter: parent.verticalCenter
              text: userRow.displayName
              color: root.textColor
              elide: Text.ElideRight
              font.family: root.fontFamily
              font.pixelSize: 17
              font.bold: true
            }

            Text {
              id: arrow
              anchors.right: parent.right
              anchors.rightMargin: 16
              anchors.verticalCenter: parent.verticalCenter
              text: "→"
              color: userRow.current ? root.accent : root.dimColor
              font.family: root.fontFamily
              font.pixelSize: 18
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

        Item { width: 1; height: 8 }

        Text {
          text: "Outro usuário…"
          color: notListedMouse.containsMouse ? root.textColor : root.dimColor
          font.family: root.fontFamily
          font.pixelSize: 14
          font.underline: notListedMouse.containsMouse

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
      x: 28
      y: 28
      width: parent.width - 56
      spacing: 12

      Item {
        width: parent.width
        height: 32

        RoundButton {
          id: back
          label: "←"
          anchors.left: parent.left
          onClicked: root.backToUsers()
        }

        Text {
          anchors.left: back.right
          anchors.leftMargin: 12
          anchors.verticalCenter: parent.verticalCenter
          text: root.manualUser ? "Outro usuário" : "Trocar de conta"
          color: root.dimColor
          font.family: root.fontFamily
          font.pixelSize: 13
        }
      }

      Avatar {
        visible: !root.manualUser
        size: 88
        anchors.horizontalCenter: parent.horizontalCenter
        source: root.selectedUser ? root.selectedUser.avatar : ""
        initial: root.shownName
      }

      Text {
        visible: !root.manualUser
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        text: root.shownName
        color: root.textColor
        elide: Text.ElideRight
        font.family: root.displayFamily
        font.pixelSize: 22
        font.bold: true
      }

      Item { width: 1; height: 4 }

      Field {
        visible: root.manualUser
        input: manualName
        placeholder: "Nome de usuário"

        TextInput {
          id: manualName
          anchors.fill: parent
          anchors.leftMargin: 16
          anchors.rightMargin: 16
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

      Field {
        input: password
        failed: root.loginFailed
        placeholder: "Senha"

        TextInput {
          id: password
          anchors.fill: parent
          anchors.leftMargin: 16
          anchors.rightMargin: 56
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

        // Submit, inside the field's right end.
        Rectangle {
          anchors.right: parent.right
          anchors.rightMargin: 6
          anchors.verticalCenter: parent.verticalCenter
          width: 36
          height: 36
          radius: 18
          color: submitMouse.containsMouse ? root.accentHover : root.accent

          Text {
            anchors.centerIn: parent
            text: "→"
            color: root.textColor
            font.family: root.fontFamily
            font.pixelSize: 18
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
        height: 18
        text: root.loginFailed ? "Senha incorreta. Tente novamente." : ""
        color: root.errorColor
        font.family: root.fontFamily
        font.pixelSize: 13
      }

      // Session chip: opens the session menu under it.
      Rectangle {
        id: sessionChip
        width: Math.min(parent.width, sessionRow.width + 28)
        height: 32
        radius: 16
        color: sessionMouse.containsMouse || root.sessionMenuOpen ? root.chipHover : root.chipColor

        Row {
          id: sessionRow
          anchors.centerIn: parent
          spacing: 8

          Text {
            text: "Sessão"
            color: root.dimColor
            font.family: root.fontFamily
            font.pixelSize: 12
            anchors.verticalCenter: parent.verticalCenter
          }
          Text {
            text: root.selectedSession ? root.selectedSession.sessionName : ""
            color: root.textColor
            font.family: root.fontFamily
            font.pixelSize: 12
            font.bold: true
            anchors.verticalCenter: parent.verticalCenter
          }
          Text {
            text: "▾"
            color: root.dimColor
            font.pixelSize: 11
            anchors.verticalCenter: parent.verticalCenter
          }
        }

        MouseArea {
          id: sessionMouse
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: {
            root.sessionMenuOpen = !root.sessionMenuOpen
            password.forceActiveFocus()
          }
        }
      }
    }
  }

  Popup {
    id: sessionMenu
    visible: root.sessionMenuOpen && !root.choosingUser
    x: card.x + 28
    y: card.y + card.height - 20
    width: 300
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

  // Bottom right: power actions, always visible.
  Row {
    anchors.right: card.right
    anchors.bottom: parent.bottom
    anchors.bottomMargin: 28
    spacing: 8

    PowerButton { label: "Suspender"; visible: sddm.canSuspend; onClicked: sddm.suspend() }
    PowerButton { label: "Reiniciar"; visible: sddm.canReboot; onClicked: sddm.reboot() }
    PowerButton { label: "Desligar"; icon: "power.png"; visible: sddm.canPowerOff; onClicked: sddm.powerOff() }
  }

  component Field: Rectangle {
    property Item input
    property bool failed: false
    property string placeholder: ""

    width: parent ? parent.width : 344
    height: 48
    radius: 24
    color: root.fieldColor
    border.width: input && input.activeFocus ? 2 : 1
    border.color: failed ? root.errorColor : (input && input.activeFocus ? root.accent : root.fieldBorder)

    Text {
      anchors.left: parent.left
      anchors.leftMargin: 16
      anchors.verticalCenter: parent.verticalCenter
      visible: parent.input && parent.input.text.length === 0
      text: parent.placeholder
      color: root.dimColor
      font.family: root.fontFamily
      font.pixelSize: 14
    }
  }

  component RoundButton: Rectangle {
    id: roundButton
    property string label: ""
    signal clicked()

    width: 32
    height: 32
    radius: 16
    color: roundMouse.containsMouse ? root.chipHover : root.chipColor

    Text {
      anchors.centerIn: parent
      text: roundButton.label
      color: root.textColor
      font.family: root.fontFamily
      font.pixelSize: 16
    }

    MouseArea {
      id: roundMouse
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: roundButton.clicked()
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
    border.color: root.cardBorder
    border.width: 1

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
    color: "#232528"
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
