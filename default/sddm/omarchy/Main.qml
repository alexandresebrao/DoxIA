import QtQuick 2.0
import SddmComponents 2.0

Rectangle {
  id: root
  width: 640
  height: 480
  color: "#1a1b26"

  // Only #1a1b26 (background), #ffffff (text) and #f7768e (failure) appear in
  // this file: omarchy-plymouth-set recolors the first two to the theme's.
  property string fontFamily: "JetBrainsMono Nerd Font"
  property bool choosingUser: true
  property bool loginFailed: false
  property bool sessionMenuOpen: false
  property int userIndex: userModel.lastIndex >= 0 ? userModel.lastIndex : 0
  property int sessionIndex: sessionModel.lastIndex >= 0 ? sessionModel.lastIndex : 0
  property Item selectedUser: users.count > 0 ? users.itemAt(userIndex) : null
  property Item selectedSession: sessionNames.count > 0 ? sessionNames.itemAt(sessionIndex) : null

  function chooseUser(index) {
    userIndex = index
    loginFailed = false
    password.text = ""
    choosingUser = false
    if (selectedUser && !selectedUser.needsPassword)
      login()
    else
      password.forceActiveFocus()
  }

  function backToUsers() {
    choosingUser = true
    sessionMenuOpen = false
    password.text = ""
    loginFailed = false
    userList.forceActiveFocus()
  }

  function login() {
    if (selectedUser)
      sddm.login(selectedUser.userName, password.text, sessionIndex)
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

  Column {
    anchors.centerIn: parent
    spacing: 40

    Image {
      id: logo
      source: "logo.png"
      width: Math.min(sourceSize.width, root.width * 0.8)
      height: sourceSize.width > 0 ? Math.round(width * sourceSize.height / sourceSize.width) : 0
      fillMode: Image.PreserveAspectFit
      anchors.horizontalCenter: parent.horizontalCenter
    }

    // Step 1: pick a user.
    FocusScope {
      id: userList
      visible: root.choosingUser
      width: 420
      height: userColumn.height
      anchors.horizontalCenter: parent.horizontalCenter
      focus: root.choosingUser

      Keys.onUpPressed: root.userIndex = Math.max(0, root.userIndex - 1)
      Keys.onDownPressed: root.userIndex = Math.min(users.count - 1, root.userIndex + 1)
      Keys.onReturnPressed: root.chooseUser(root.userIndex)
      Keys.onEnterPressed: root.chooseUser(root.userIndex)

      Column {
        id: userColumn
        width: parent.width
        spacing: 6

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
            height: 72

            Rectangle {
              anchors.fill: parent
              radius: 8
              color: "#ffffff"
              opacity: userRow.current ? 0.12 : (rowMouse.containsMouse ? 0.06 : 0)
            }

            Avatar {
              id: rowAvatar
              size: 48
              anchors.left: parent.left
              anchors.leftMargin: 14
              anchors.verticalCenter: parent.verticalCenter
              source: userRow.avatar
              initial: userRow.displayName
            }

            Text {
              anchors.left: rowAvatar.right
              anchors.leftMargin: 16
              anchors.right: parent.right
              anchors.rightMargin: 14
              anchors.verticalCenter: parent.verticalCenter
              text: userRow.displayName
              color: "#ffffff"
              elide: Text.ElideRight
              font.family: root.fontFamily
              font.pixelSize: 20
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
      }
    }

    // Step 2: password and session for the chosen user.
    Column {
      visible: !root.choosingUser
      spacing: 24
      anchors.horizontalCenter: parent.horizontalCenter

      Row {
        spacing: 16
        anchors.horizontalCenter: parent.horizontalCenter

        Avatar {
          size: 56
          anchors.verticalCenter: parent.verticalCenter
          source: root.selectedUser ? root.selectedUser.avatar : ""
          initial: root.selectedUser ? root.selectedUser.displayName : ""
        }

        Text {
          anchors.verticalCenter: parent.verticalCenter
          text: root.selectedUser ? root.selectedUser.displayName : ""
          color: "#ffffff"
          font.family: root.fontFamily
          font.pixelSize: 22
        }
      }

      Row {
        anchors.horizontalCenter: parent.horizontalCenter
        spacing: 15

        Image {
          source: root.loginFailed ? "lock-failed.png" : "lock.png"
          width: 34
          height: 38
          fillMode: Image.PreserveAspectFit
          anchors.verticalCenter: parent.verticalCenter
        }

        Item {
          width: entry.width
          height: entry.height

          Image {
            id: entry
            source: root.loginFailed ? "entry-failed.png" : "entry.png"
            anchors.centerIn: parent
          }

          Row {
            anchors.left: parent.left
            anchors.leftMargin: 20
            anchors.verticalCenter: parent.verticalCenter
            spacing: 5

            Repeater {
              model: Math.min(password.text.length, 21)

              Image {
                source: "bullet.png"
                width: 7
                height: 7
              }
            }
          }

          TextInput {
            id: password
            anchors.fill: parent
            anchors.leftMargin: 20
            anchors.rightMargin: 20
            verticalAlignment: TextInput.AlignVCenter
            echoMode: TextInput.Password
            font.family: root.fontFamily
            font.pixelSize: 24
            font.letterSpacing: 5
            passwordCharacter: "•"
            color: "transparent"
            selectionColor: "transparent"
            selectedTextColor: "transparent"
            cursorDelegate: Item {}

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
      }

      Item {
        width: parent.width
        height: Math.max(backLink.height, sessionLink.height)

        Text {
          id: backLink
          anchors.left: parent.left
          text: "← Trocar usuário"
          color: "#ffffff"
          opacity: backMouse.containsMouse ? 1 : 0.6
          font.family: root.fontFamily
          font.pixelSize: 15

          MouseArea {
            id: backMouse
            anchors.fill: parent
            anchors.margins: -6
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.backToUsers()
          }
        }

        Text {
          id: sessionLink
          anchors.right: parent.right
          text: (root.selectedSession ? root.selectedSession.sessionName : "") + (root.sessionMenuOpen ? "  ▴" : "  ▾")
          color: "#ffffff"
          opacity: sessionMouse.containsMouse || root.sessionMenuOpen ? 1 : 0.6
          font.family: root.fontFamily
          font.pixelSize: 15

          MouseArea {
            id: sessionMouse
            anchors.fill: parent
            anchors.margins: -6
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: {
              root.sessionMenuOpen = !root.sessionMenuOpen
              password.forceActiveFocus()
            }
          }
        }

        // Session menu, opening under its link.
        Rectangle {
          visible: root.sessionMenuOpen
          z: 10
          anchors.right: parent.right
          anchors.top: sessionLink.bottom
          anchors.topMargin: 10
          width: Math.max(260, sessionMenu.width + 16)
          height: sessionMenu.height + 16
          radius: 8
          color: "#1a1b26"
          border.color: "#ffffff"
          border.width: 1

          Column {
            id: sessionMenu
            anchors.centerIn: parent
            width: parent.width - 16

            Repeater {
              model: sessionModel

              Item {
                id: sessionRow
                width: sessionMenu.width
                height: 36

                Rectangle {
                  anchors.fill: parent
                  radius: 6
                  color: "#ffffff"
                  opacity: index === root.sessionIndex ? 0.14 : (sessionRowMouse.containsMouse ? 0.06 : 0)
                }

                Text {
                  anchors.left: parent.left
                  anchors.leftMargin: 12
                  anchors.verticalCenter: parent.verticalCenter
                  text: model.name
                  color: "#ffffff"
                  font.family: root.fontFamily
                  font.pixelSize: 15
                }

                MouseArea {
                  id: sessionRowMouse
                  anchors.fill: parent
                  hoverEnabled: true
                  cursorShape: Qt.PointingHandCursor
                  onClicked: {
                    root.sessionIndex = index
                    root.sessionMenuOpen = false
                    password.forceActiveFocus()
                  }
                }
              }
            }
          }
        }
      }
    }
  }

  component Avatar: Item {
    property int size: 48
    property string source: ""
    property string initial: ""

    width: size
    height: size

    Rectangle {
      anchors.fill: parent
      radius: width / 2
      color: "transparent"
      border.color: "#ffffff"
      border.width: 2
      visible: face.status !== Image.Ready
    }

    Text {
      anchors.centerIn: parent
      visible: face.status !== Image.Ready
      text: parent.initial.charAt(0).toUpperCase()
      color: "#ffffff"
      font.family: root.fontFamily
      font.pixelSize: parent.size * 0.45
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
