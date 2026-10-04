import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

// Painel de serviços de desenvolvimento. Todo o trabalho fica no script `svc`
// ao lado deste arquivo (units transitórias do systemd do usuário); aqui só
// lemos `svc status` e disparamos ações. A config é ~/.config/omarchy/services.json.
Panel {
  id: root
  moduleName: "alexandre.services"
  ipcTarget: "alexandre.services"

  readonly property string pluginDir: String(Qt.resolvedUrl(".")).replace(/^file:\/\//, "").replace(/\/$/, "")
  readonly property string svc: pluginDir + "/svc"
  readonly property string configPath: Quickshell.env("HOME") + "/.config/omarchy/services.json"

  property bool hasConfig: true
  property var services: []
  // O Repeater usa só a lista de ids (estável entre polls) para não recriar as
  // linhas a cada atualização, o que fecharia o popup de branch aberto.
  property var serviceIds: []
  property var byId: ({})
  property var branches: ({})
  property var runtimes: ({ node: [], java: [] })
  property real now: Date.now() / 1000

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  readonly property color runningColor: "#5fb878"
  readonly property int runningCount: services.filter(function(s) { return s.state === "running" }).length
  readonly property bool anyBusy: services.some(function(s) { return s.state === "busy" })
  readonly property bool anyFailed: services.some(function(s) { return s.state === "failed" || s.taskFailed })

  function refresh() {
    if (!statusProc.running) statusProc.running = true
  }

  function run(args) {
    Quickshell.execDetached([root.svc].concat(args))
    refreshSoon.restart()
  }

  function uptime(since) {
    if (!since) return ""
    var s = Math.max(0, Math.floor(root.now - since))
    if (s < 60) return s + "s"
    if (s < 3600) return Math.floor(s / 60) + "min"
    if (s < 86400) return Math.floor(s / 3600) + "h" + (Math.floor(s / 60) % 60 ? String(Math.floor(s / 60) % 60).padStart(2, "0") : "")
    return Math.floor(s / 86400) + "d"
  }

  function stateLabel(s) {
    if (s.state === "busy") return s.task + "…"
    if (s.state === "running") return "rodando " + uptime(s.since) + (s.port ? " · :" + s.port : "")
    if (s.state === "stopping") return "parando…"
    if (s.state === "failed") return "caiu — veja o log"
    if (s.taskFailed) return "build falhou — veja o log"
    return "parado"
  }

  function stateText(s) {
    return stateLabel(s) + (s.dirty ? " · alterações locais" : "")
  }

  function runtimeOptions(type, current, hint) {
    var list = (root.runtimes[type] || []).slice()
    if (current && list.indexOf(current) < 0) list.unshift(current)
    var opts = [{ value: "", label: "auto" + (hint ? " · " + hint : "") }]
    list.forEach(function(v) { opts.push({ value: v, label: v }) })
    return opts
  }

  function branchOptions(id, current) {
    var list = (root.branches[id] || []).slice()
    if (current && list.indexOf(current) < 0) list.unshift(current)
    return list
  }

  onOpenedChanged: {
    if (!opened) return
    refresh()
    if (!branchProc.running) branchProc.running = true
    if (!runtimesProc.running) runtimesProc.running = true
  }

  Process {
    id: runtimesProc
    command: [root.svc, "runtimes"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try { root.runtimes = JSON.parse(text) } catch (e) {}
      }
    }
  }

  Process {
    id: statusProc
    command: [root.svc, "status"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try {
          var data = JSON.parse(text)
          root.hasConfig = data.config
          var list = data.services || []
          var map = {}
          list.forEach(function(s) { map[s.id] = s })
          var ids = list.map(function(s) { return s.id })
          if (JSON.stringify(ids) !== JSON.stringify(root.serviceIds)) root.serviceIds = ids
          root.byId = map
          root.services = list
          root.now = Date.now() / 1000
        } catch (e) {}
      }
    }
  }

  Process {
    id: branchProc
    command: [root.svc, "branches-all"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try { root.branches = JSON.parse(text) } catch (e) {}
      }
    }
  }

  Timer { id: refreshSoon; interval: 400; onTriggered: root.refresh() }

  Timer {
    interval: root.opened || root.anyBusy ? 1500 : 6000
    running: true
    repeat: true
    triggeredOnStart: true
    onTriggered: root.refresh()
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    // Branco com algo rodando, cinza com tudo parado; em falha ganha um
    // triângulo de alerta no canto inferior esquerdo (failBadge).
    text: "󰒋"
    foreground: root.runningCount > 0 || root.anyBusy
      ? root.bar.foreground
      : Qt.rgba(root.bar.foreground.r, root.bar.foreground.g, root.bar.foreground.b, 0.4)
    tooltipText: !root.hasConfig ? "Serviços: sem config"
      : (root.anyFailed ? "Algum serviço caiu ou o build falhou — " : "")
        + root.runningCount + " de " + root.services.length + " serviços rodando"
    onPressed: function(b) { root.toggle() }

    SequentialAnimation on opacity {
      running: root.anyBusy
      loops: Animation.Infinite
      NumberAnimation { to: 0.4; duration: 700; easing.type: Easing.InOutSine }
      NumberAnimation { to: 1.0; duration: 700; easing.type: Easing.InOutSine }
      onRunningChanged: if (!running) button.opacity = 1.0
    }
  }

  // Em falha, uma bolinha vermelha no canto superior direito do servidor,
  // com contorno da cor da barra para separar do ícone.
  Rectangle {
    id: failBadge
    visible: root.anyFailed
    readonly property real glyph: button.glyphPaintedWidth > 0 ? button.glyphPaintedWidth : button.width * 0.5
    width: Math.max(7, Math.round(glyph * 0.4)) + 2
    height: width
    radius: width / 2
    color: "#e5252a"
    border.width: 1.5
    border.color: Color.bar.background
    x: Math.round(button.x + (button.width + glyph) / 2 - width * 0.6)
    y: Math.round(button.y + (button.height - glyph) / 2 - height * 0.35)
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(470))
    contentHeight: panel.fittedContentHeight(column.implicitHeight)

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }

      Column {
        id: column
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        spacing: Style.space(12)

        // ---------- Cabeçalho ----------
        Item {
          width: parent.width
          implicitHeight: Math.max(heroIcon.implicitHeight, heroLabels.implicitHeight)

          Text {
            id: heroIcon
            text: "󰒋"
            color: root.bar.foreground
            font.family: root.bar.fontFamily
            font.pixelSize: Style.font.display
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
          }

          Column {
            id: heroLabels
            anchors.left: heroIcon.right
            anchors.leftMargin: Style.space(14)
            anchors.right: heroActions.left
            anchors.verticalCenter: parent.verticalCenter
            spacing: Style.space(2)

            Text {
              text: "Serviços"
              color: root.bar.foreground
              font.family: root.bar.fontFamily
              font.pixelSize: Style.font.title
              font.bold: true
            }
            Text {
              text: (root.services.length === 0 ? "NENHUM CONFIGURADO"
                : root.runningCount + " DE " + root.services.length + " RODANDO")
              color: Qt.darker(root.bar.foreground, 1.4)
              font.family: root.bar.fontFamily
              font.pixelSize: Style.font.caption
              font.bold: true
              font.letterSpacing: 1.2
            }
          }

          Row {
            id: heroActions
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: Style.space(2)

            PanelActionButton {
              iconText: "󰏫"
              tooltipText: "Editar services.json"
              foreground: root.bar.foreground
              fontFamily: root.bar.fontFamily
              onClicked: {
                Quickshell.execDetached(["omarchy-launch-editor", root.configPath])
                root.close()
              }
            }
            PanelActionButton {
              iconText: "󰑐"
              tooltipText: "Atualizar branches e versões"
              foreground: root.bar.foreground
              fontFamily: root.bar.fontFamily
              onClicked: {
                root.refresh()
                if (!branchProc.running) branchProc.running = true
                if (!runtimesProc.running) runtimesProc.running = true
              }
            }
          }
        }

        PanelSeparator { foreground: root.bar.foreground }

        Text {
          visible: root.services.length === 0
          width: parent.width
          wrapMode: Text.WordWrap
          text: "Nenhum serviço ainda. No Claude Code, use /servicos para adicionar um projeto node ou java."
          color: root.bar.foreground
          opacity: 0.7
          font.family: root.bar.fontFamily
          font.pixelSize: Style.font.bodySmall
        }

        Repeater {
          model: root.serviceIds
          delegate: ServiceRow {
            required property var modelData
            service: root.byId[modelData] || ({})
            width: column.width
          }
        }
      }
    }
  }

  component ServiceRow: Column {
    id: row
    property var service: ({})
    readonly property string serviceId: service.id || ""
    readonly property string branch: service.branch || ""
    readonly property string runtime: service.runtime || ""
    readonly property string runtimeHint: service.runtimeHint || ""
    readonly property string serviceType: service.type || ""
    readonly property bool running: service.state === "running"
    readonly property bool busy: service.state === "busy" || service.state === "stopping"
    readonly property color dotColor: service.state === "running" ? root.runningColor
      : service.state === "failed" ? Color.urgent
      : busy ? Color.accent
      : Qt.rgba(root.bar.foreground.r, root.bar.foreground.g, root.bar.foreground.b, 0.3)

    spacing: Style.space(6)
    bottomPadding: Style.space(6)

    // Linha 1: estado · nome · tipo ........ ações
    Item {
      width: parent.width
      implicitHeight: Math.max(nameCol.implicitHeight, actions.implicitHeight)

      Rectangle {
        id: dot
        width: Style.space(9); height: width; radius: width / 2
        color: row.dotColor
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter

        SequentialAnimation on opacity {
          running: row.busy
          loops: Animation.Infinite
          NumberAnimation { to: 0.25; duration: 600 }
          NumberAnimation { to: 1.0; duration: 600 }
          onRunningChanged: if (!running) dot.opacity = 1.0
        }
      }

      Column {
        id: nameCol
        anchors.left: dot.right
        anchors.leftMargin: Style.space(10)
        anchors.right: actions.left
        anchors.rightMargin: Style.space(8)
        anchors.verticalCenter: parent.verticalCenter
        spacing: Style.space(1)

        Row {
          spacing: Style.space(8)
          Text {
            text: row.service.name || row.serviceId
            color: root.bar.foreground
            font.family: root.bar.fontFamily
            font.pixelSize: Style.font.body
            font.bold: true
          }
          Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            visible: !!row.service.type
            implicitWidth: typeText.implicitWidth + Style.space(10)
            implicitHeight: typeText.implicitHeight + Style.space(2)
            radius: Style.cornerRadius
            color: "transparent"
            border.width: 1
            border.color: Qt.rgba(root.bar.foreground.r, root.bar.foreground.g, root.bar.foreground.b, 0.35)
            Text {
              id: typeText
              anchors.centerIn: parent
              text: row.service.type || ""
              color: root.bar.foreground
              opacity: 0.75
              font.family: root.bar.fontFamily
              font.pixelSize: Style.font.caption
            }
          }
        }

        Text {
          width: parent.width
          elide: Text.ElideRight
          readonly property bool bad: row.service.state === "failed" || (row.service.taskFailed === true && row.service.state !== "running")
          text: root.stateText(row.service)
          color: bad ? Color.urgent : root.bar.foreground
          opacity: bad ? 1 : 0.6
          font.family: root.bar.fontFamily
          font.pixelSize: Style.font.caption
        }
      }

      Row {
        id: actions
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: Style.space(2)

        PanelActionButton {
          iconText: row.running ? "󰓛" : "󰐊"
          tooltipText: row.running ? "Parar" : "Iniciar"
          foreground: row.running ? Color.urgent : root.runningColor
          fontFamily: root.bar.fontFamily
          enabled: !row.busy
          opacity: enabled ? 1 : 0.35
          onClicked: root.run([row.running ? "stop" : "start", row.service.id])
        }
        PanelActionButton {
          iconText: "󰜉"
          tooltipText: "Reiniciar"
          foreground: root.bar.foreground
          fontFamily: root.bar.fontFamily
          visible: row.running
          onClicked: root.run(["restart", row.service.id])
        }
        PanelActionButton {
          iconText: "󱌢"
          tooltipText: "Rodar os passos de build (sem trocar de branch)"
          foreground: root.bar.foreground
          fontFamily: root.bar.fontFamily
          visible: row.service.hasBuild === true
          enabled: !row.busy
          opacity: enabled ? 1 : 0.35
          onClicked: root.run(["build", row.service.id])
        }
        PanelActionButton {
          iconText: "󰆍"
          tooltipText: "Log"
          foreground: row.service.taskFailed ? Color.urgent : root.bar.foreground
          fontFamily: root.bar.fontFamily
          onClicked: { root.run(["log", row.service.id]); root.close() }
        }
      }
    }

    // Linha 2: branch · versão do runtime (nvm / sdkman)
    Row {
      id: line2
      x: dot.width + Style.space(10)
      width: parent.width - x
      spacing: Style.space(6)

      readonly property real runtimeWidth: Style.space(150)
      readonly property real iconWidth: Style.space(16)

      Text {
        visible: row.service.git === true
        width: line2.iconWidth
        anchors.verticalCenter: parent.verticalCenter
        text: "󰘬"
        color: root.bar.foreground
        opacity: 0.6
        font.family: root.bar.fontFamily
        font.pixelSize: Style.font.body
      }

      SearchableDropdown {
        visible: row.service.git === true
        anchors.verticalCenter: parent.verticalCenter
        width: line2.width - line2.runtimeWidth - line2.iconWidth * 2 - line2.spacing * 3
        showLabel: false
        enabled: !row.busy
        opacity: enabled ? 1 : 0.5
        fontFamily: root.bar.fontFamily
        placeholderText: "Buscar branch…"
        emptyText: "Nenhuma branch"
        value: row.branch
        options: root.branchOptions(row.serviceId, row.branch)
        onChanged: function(v) {
          if (v && v !== row.branch) root.run(["checkout", row.serviceId, v])
          // O dropdown sobrescreve value; volta a seguir a branch real do repo.
          value = Qt.binding(function() { return row.branch })
        }
      }

      Item {
        visible: row.service.git !== true
        width: line2.width - line2.runtimeWidth - line2.iconWidth - line2.spacing * 2
        height: 1
      }

      Text {
        width: line2.iconWidth
        anchors.verticalCenter: parent.verticalCenter
        horizontalAlignment: Text.AlignHCenter
        text: row.service.type === "java" ? "󰬷" : "󰎙"
        color: root.bar.foreground
        opacity: 0.6
        font.family: root.bar.fontFamily
        font.pixelSize: Style.font.body
      }

      Dropdown {
        anchors.verticalCenter: parent.verticalCenter
        width: line2.runtimeWidth
        showLabel: false
        enabled: !row.busy
        opacity: enabled ? 1 : 0.5
        fontFamily: root.bar.fontFamily
        value: row.runtime
        options: root.runtimeOptions(row.serviceType, row.runtime, row.runtimeHint)
        onChanged: function(v) {
          if (v !== row.runtime) root.run(["runtime", row.serviceId, v])
          value = Qt.binding(function() { return row.runtime })
        }
      }
    }
  }
}
