import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui
import "../../update/UpdateModel.js" as Model

// Bar icon for Atualizar DoxIA. Shows while there is something to update, an
// update is running or waiting for the reboot, or the last one failed; its
// popup summarizes that and opens the update window (plugins/update). It also
// runs the periodic check (omarchy-update-check, every "checkHours"), the
// background Flatpak update when "autoFlatpak" is on, and the notification
// when an update finishes.
Panel {
  id: root
  moduleName: "omarchy.system-update"
  manageIpc: false

  readonly property string home: Quickshell.env("HOME")
  readonly property string runtimeDir: Quickshell.env("XDG_RUNTIME_DIR") || "/tmp"

  property var status: null
  property var run: null
  property var last: null
  property var prefs: ({ checkHours: 6, autoFlatpak: false })
  property color okColor: "#5ba352"
  property color warnColor: "#f0ab00"

  readonly property bool running: !!(run && run.running)
  readonly property bool waitingPassword: running && !!(run.askpass && run.askpass.waiting)
  readonly property bool staged: Model.stagedPending(status)
  readonly property bool failed: !!(last && last.seen === false && last.result === "failed")
  readonly property bool updateAvailable: Model.anything(status)
  readonly property string phase: running ? (waitingPassword ? "password" : "running")
    : staged ? "staged" : failed ? "failed" : updateAvailable ? "available" : "none"

  visible: phase !== "none"
  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  function refresh() { statusFile.reload(); runFile.reload(); lastFile.reload() }
  function clear() { refresh() }

  function check() {
    if (checkProc.running) return
    prefsProc.running = true
    checkProc.running = true
  }

  function broadcast(method) {
    var items = bar && typeof bar.moduleWidgets === "function" ? bar.moduleWidgets(moduleName) : [root]
    for (var i = 0; i < items.length; i++)
      if (items[i] && typeof items[i][method] === "function") items[i][method]()
  }

  function openWindow() {
    root.close()
    Quickshell.execDetached(["omarchy-launch-update"])
  }

  // One toast per finished run; `notified` keeps the window's `seen` for itself.
  function notifyResult() {
    if (!last || last.seen !== false || last.notified || !last.finishedAt) return
    if (last.result !== "ok" && last.result !== "failed") return
    if (Date.now() / 1000 - last.finishedAt > 86400) return
    var sum = last.summary || {}
    var body = []
    if (sum.packages) body.push(Model.plural(sum.packages, "pacote", "pacotes"))
    if (sum.migrations) body.push(Model.plural(sum.migrations, "migração", "migrações"))
    if (sum.orphans) body.push(Model.plural(sum.orphans, "órfão removido", "órfãos removidos"))
    if (last.reboot) body.push(last.reboot + ": reinicie")
    if (last.result === "ok") {
      Quickshell.execDetached(["omarchy-notification-send", "-g", "",
        last.mode === "flatpak" ? "Apps Flatpak atualizados" : "DoxIA atualizado",
        body.length ? body.join(" · ") : "Sem pendências"])
    } else {
      Quickshell.execDetached(["omarchy-notification-send", "-u", "critical", "-g", "",
        "A atualização não terminou", Model.failureText(last), "--exec", "omarchy-launch-update"])
    }
    var path = home + "/.local/state/omarchy/update-last.json"
    Quickshell.execDetached(["sh", "-c", "f=\"$1\"; jq '.notified = true' \"$f\" > \"$f.tmp\" && mv \"$f.tmp\" \"$f\"", "sh", path])
  }

  IpcHandler {
    target: "omarchy.system-update"
    function refresh(): void { root.broadcast("refresh") }
    function clear(): void { root.broadcast("clear") }
    function check(): void { root.check() }
  }

  FileView {
    id: statusFile
    path: root.home + "/.cache/omarchy/update-status.json"
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: root.status = Model.parse(text())
  }

  FileView {
    id: runFile
    path: root.runtimeDir + "/omarchy-update/state.json"
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: root.run = Model.parse(text())
    onLoadFailed: root.run = null
  }

  FileView {
    id: lastFile
    path: root.home + "/.local/state/omarchy/update-last.json"
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: { root.last = Model.parse(text()); root.notifyResult() }
  }

  FileView {
    path: Color.currentThemePath + "/colors.toml"
    printErrors: false
    onLoaded: {
      var t = String(text() || "")
      var g = t.match(/^green\s*=\s*"(#[0-9a-fA-F]{6})"/m)
      var y = t.match(/^yellow\s*=\s*"(#[0-9a-fA-F]{6})"/m)
      if (g) root.okColor = g[1]
      if (y) root.warnColor = y[1]
    }
  }

  onRunningChanged: if (!running) { statusFile.reload(); lastFile.reload() }

  Timer {
    interval: 1000
    repeat: true
    running: root.running
    onTriggered: { runFile.reload(); lastFile.reload() }
  }

  Process {
    id: prefsProc
    command: ["omarchy-update-pref", "json"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var p = Model.parse(text)
        if (p) root.prefs = p
      }
    }
  }

  Process {
    id: checkProc
    command: ["omarchy-update-check"]
    onExited: {
      statusFile.reload()
      if (root.prefs.autoFlatpak && !root.running && Model.flatpakCount(root.status) > 0)
        Quickshell.execDetached(["omarchy-update-run", "start", "flatpak"])
    }
  }

  Timer {
    interval: Math.max(1, root.prefs.checkHours || 6) * 3600000
    running: true
    repeat: true
    triggeredOnStart: true
    onTriggered: root.check()
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.phase === "failed" ? "" : ""
    foreground: root.phase === "staged" ? root.warnColor
      : root.phase === "failed" ? Color.urgent
      : root.phase === "running" || root.phase === "password" ? Color.accent
      : root.barForeground
    slotSize: Style.bar.statusSlot
    fontSize: Style.font.caption
    tooltipText: root.phase === "running" ? "Atualizando o DoxIA…"
      : root.phase === "password" ? "A atualização espera sua senha"
      : root.phase === "staged" ? "Atualização pronta: instala no próximo reinício"
      : root.phase === "failed" ? "A última atualização não terminou"
      : "Atualizações do DoxIA disponíveis"
    onPressed: root.toggle()

    RotationAnimation on textRotation {
      running: root.phase === "running"
      from: 0; to: 360; duration: 1600
      loops: Animation.Infinite
      onRunningChanged: if (!running) button.textRotation = 0
    }
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(300))
    contentHeight: panel.fittedContentHeight(column.implicitHeight)

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }

      Column {
        id: column
        anchors { left: parent.left; right: parent.right; top: parent.top }
        spacing: Style.space(12)

        Item {
          width: parent.width
          implicitHeight: Math.max(heroIcon.implicitHeight, heroLabels.implicitHeight)

          Text {
            textFormat: Text.PlainText
            id: heroIcon
            text: button.text
            color: button.foreground
            font.family: root.bar ? root.bar.fontFamily : Style.font.family
            font.pixelSize: Style.font.display
            anchors { left: parent.left; verticalCenter: parent.verticalCenter }
          }

          Column {
            id: heroLabels
            anchors { left: heroIcon.right; leftMargin: Style.space(14); right: parent.right; verticalCenter: parent.verticalCenter }
            spacing: Style.space(2)
            Text {
              textFormat: Text.PlainText
              width: parent.width
              elide: Text.ElideRight
              text: root.phase === "running" ? (Model.currentStep(root.run) ? Model.currentStep(root.run).label : "Atualizando…")
                : root.phase === "password" ? "Aguardando senha"
                : root.phase === "staged" ? "Instala no reinício"
                : root.phase === "failed" ? "A atualização não terminou"
                : Model.plural(Model.updatesCount(root.status), "atualização", "atualizações")
              color: root.barForeground
              font.family: root.bar ? root.bar.fontFamily : Style.font.family
              font.pixelSize: Style.font.title
              font.bold: true
            }
            Text {
              textFormat: Text.PlainText
              width: parent.width
              elide: Text.ElideRight
              text: root.phase === "running" ? (root.run.progress || 0) + "%"
                : root.phase === "staged" ? Model.plural(Model.stagedPackages(root.status).length, "pacote pronto", "pacotes prontos")
                : root.phase === "available" ? "VERIFICADO " + Model.ago(root.status ? root.status.checkedAt : 0).toUpperCase()
                : ""
              visible: text !== ""
              color: Qt.darker(root.barForeground, 1.4)
              font.family: root.bar ? root.bar.fontFamily : Style.font.family
              font.pixelSize: Style.font.caption
              font.bold: true
              font.letterSpacing: 1.2
            }
          }
        }

        Rectangle {
          visible: root.phase === "running"
          width: parent.width
          height: Style.space(5)
          radius: height / 2
          color: Qt.rgba(root.barForeground.r, root.barForeground.g, root.barForeground.b, 0.12)
          Rectangle {
            width: parent.width * Math.min(1, (root.run && root.run.progress ? root.run.progress : 0) / 100)
            height: parent.height
            radius: parent.radius
            color: Color.accent
            Behavior on width { NumberAnimation { duration: 400 } }
          }
        }

        Text {
          textFormat: Text.PlainText
          width: parent.width
          wrapMode: Text.WordWrap
          visible: text !== ""
          text: {
            if (root.phase === "available") {
              var lines = []
              if (Model.systemCount(root.status)) lines.push("Sistema: " + Model.systemNames(root.status, 3))
              if (Model.flatpakCount(root.status)) lines.push("Flatpak: " + Model.flatpakNames(root.status, 2))
              if (Model.doxiaBehind(root.status)) lines.push("DoxIA: " + Model.plural(Model.doxiaBehind(root.status), "novidade", "novidades"))
              return lines.join("\n")
            }
            if (root.phase === "staged") return "A atualização já foi baixada e será instalada quando você reiniciar ou desligar."
            if (root.phase === "password") return "Abra a janela para confirmar a senha."
            if (root.phase === "failed") return Model.failureText(root.last)
            if (root.phase === "running") {
              var s = Model.currentStep(root.run)
              return s && s.detail ? s.detail : ""
            }
            return ""
          }
          color: root.barForeground
          opacity: 0.8
          font.family: root.bar ? root.bar.fontFamily : Style.font.family
          font.pixelSize: Style.font.bodySmall
          lineHeight: 1.2
        }

        Row {
          anchors.right: parent.right
          spacing: Style.space(8)

          Button {
            visible: root.phase === "available"
            text: "Depois"
            bordered: true
            onClicked: root.close()
          }
          Button {
            visible: root.phase === "staged"
            text: "Abrir"
            bordered: true
            onClicked: root.openWindow()
          }
          Button {
            text: root.phase === "available" ? "Atualizar" : root.phase === "staged" ? "Reiniciar" : "Abrir"
            background: Color.accent
            foreground: Color.background
            accent: Color.background
            onClicked: {
              if (root.phase === "staged") { root.close(); Quickshell.execDetached(["omarchy-system-reboot"]) }
              else root.openWindow()
            }
          }
        }
      }
    }
  }
}
