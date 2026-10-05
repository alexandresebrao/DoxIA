import QtQuick
import QtQuick.Controls
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui
import "UpdateModel.js" as Model

// Atualizar DoxIA: what an update would change, when each part installs, the
// update's progress and its preferences. Opened from Menu > Atualizar > DoxIA
// and the bar icon (omarchy-launch-update). The work is all in bin/: the
// window starts omarchy-update-run and follows the JSON files described in
// UpdateModel.js.
Item {
  id: root

  property var shell: null
  property bool closingFromHost: false

  readonly property string home: Quickshell.env("HOME")
  readonly property string runtimeDir: Quickshell.env("XDG_RUNTIME_DIR") || "/tmp"
  readonly property string statusPath: home + "/.cache/omarchy/update-status.json"
  readonly property string runPath: runtimeDir + "/omarchy-update/state.json"
  readonly property string lastPath: home + "/.local/state/omarchy/update-last.json"
  readonly property string logPath: "/tmp/omarchy-update.log"

  property var status: null
  property var run: null
  property var last: null
  property var prefs: ({ mode: "boot", autoremoveOrphans: true, snapshot: true, checkHours: 6, autoFlatpak: false })
  property bool checking: false
  property string tab: "updates"
  property bool logOpen: false
  property var logTail: []
  property bool starting: false

  // Semantic colors from the theme's palette (green/yellow), with fallbacks.
  property color okColor: "#5ba352"
  property color warnColor: "#f0ab00"
  readonly property color fg: Color.foreground
  readonly property color dim: Qt.rgba(fg.r, fg.g, fg.b, 0.6)
  readonly property color faint: Qt.rgba(fg.r, fg.g, fg.b, 0.12)
  readonly property string family: Style.font.family

  readonly property bool running: !!(run && run.running)
  readonly property bool waitingPassword: running && !!(run.askpass && run.askpass.waiting)
  readonly property bool lastUnseen: !!(last && last.seen === false && last.result && last.result !== "canceled"
    && (last.result !== "ok" || Date.now() / 1000 - (last.finishedAt || 0) < 3600))
  readonly property string view: running ? (waitingPassword ? "password" : "running")
    : (starting ? "running"
    : (Model.stagedPending(status) ? "staged"
    : (lastUnseen ? "result" : "idle")))

  // ---- plugin lifecycle ---------------------------------------------------
  function open(payloadJson) {
    closingFromHost = false
    window.visible = true
    // { "tab": "prefs" } opens on the preferences.
    var payload = Model.parse(payloadJson)
    tab = payload && payload.tab === "prefs" ? "prefs" : "updates"
    reloadAll()
    prefsProc.running = true
    if (!running && (!status || Date.now() / 1000 - (status.checkedAt || 0) > 600)) check(false)
  }

  function close() {
    closingFromHost = true
    window.visible = false
    closingFromHost = false
  }

  function requestClose() {
    if (shell && typeof shell.hide === "function") shell.hide("omarchy.update")
    else window.visible = false
  }

  // ---- actions ------------------------------------------------------------
  function reloadAll() {
    statusFile.reload()
    runFile.reload()
    lastFile.reload()
  }

  function check(refresh) {
    if (checkProc.running) return
    checking = true
    checkProc.command = refresh ? ["omarchy-update-check", "--refresh"] : ["omarchy-update-check"]
    checkProc.running = true
  }

  function start(mode) {
    markSeen()
    starting = true
    startTimeout.restart()
    Quickshell.execDetached(["omarchy-update-run", "start", mode])
  }

  // The password goes through stdin, never argv (visible in ps).
  property string pendingAnswer: ""
  function answer(password) {
    if (answerProc.running) return
    pendingAnswer = password
    passwordField.text = ""
    answerProc.running = true
  }

  function setPref(key, value) {
    var next = {}
    for (var k in prefs) next[k] = prefs[k]
    next[key] = value
    prefs = next
    Quickshell.execDetached(["omarchy-update-pref", "set", key, String(value)])
  }

  function markSeen() {
    if (!last || last.seen !== false) return
    var next = {}
    for (var k in last) next[k] = last[k]
    next.seen = true
    last = next
    Quickshell.execDetached(["sh", "-c", "f=\"$1\"; jq '.seen = true' \"$f\" > \"$f.tmp\" && mv \"$f.tmp\" \"$f\"", "sh", lastPath])
  }

  function reboot() { Quickshell.execDetached(["omarchy-system-reboot"]) }

  function openLog() {
    Quickshell.execDetached(["omarchy-launch-floating-terminal-with-presentation", "less", "-R", "+G", logPath])
  }

  // ---- data ---------------------------------------------------------------
  FileView {
    id: statusFile
    path: root.statusPath
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: root.status = Model.parse(text())
  }

  FileView {
    id: runFile
    path: root.runPath
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: {
      var r = Model.parse(text())
      root.run = r
      if (r && r.running) root.starting = false
    }
    onLoadFailed: root.run = null
  }

  FileView {
    id: lastFile
    path: root.lastPath
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: root.last = Model.parse(text())
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

  // Renames under the watch can slip past inotify: poll while a run is going.
  Timer {
    interval: 700
    repeat: true
    running: window.visible && (root.running || root.starting)
    onTriggered: { runFile.reload(); lastFile.reload() }
  }

  Timer {
    interval: 1000
    repeat: true
    running: window.visible && root.logOpen && (root.running || root.view === "result")
    triggeredOnStart: true
    onTriggered: if (!logProc.running) logProc.running = true
  }

  // A run that never reached omarchy-update-state begin (lock held, say).
  Timer {
    id: startTimeout
    interval: 8000
    onTriggered: { root.starting = false; root.check(false) }
  }

  Process {
    id: checkProc
    onExited: { root.checking = false; statusFile.reload() }
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
    id: answerProc
    command: ["omarchy-update-run", "answer"]
    stdinEnabled: true
    onStarted: {
      write(root.pendingAnswer + "\n")
      root.pendingAnswer = ""
      stdinEnabled = false
    }
    onExited: { stdinEnabled = true; runFile.reload() }
  }

  Process {
    id: logProc
    command: ["tail", "-c", "6000", root.logPath]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.logTail = Model.logLines(text, 9)
    }
  }

  onViewChanged: {
    if (view === "password") Qt.callLater(function() { passwordField.text = ""; passwordField.forceActiveFocus() })
    if (view === "idle" && window.visible) statusFile.reload()
  }

  // ---- window ---------------------------------------------------------------
  FloatingWindow {
    id: window
    title: "Atualizar DoxIA"
    color: Color.background
    implicitWidth: 560
    implicitHeight: 620
    minimumSize: Qt.size(480, 520)

    onVisibleChanged: {
      if (!visible) {
        if (root.view === "result") root.markSeen()
        root.logOpen = false
        if (!root.closingFromHost && root.shell && typeof root.shell.hide === "function") root.shell.hide("omarchy.update")
      }
    }

    FocusScope {
      anchors.fill: parent
      focus: true
      Keys.onEscapePressed: root.requestClose()

      // Header: icon, title, tabs
      Item {
        id: header
        anchors { left: parent.left; right: parent.right; top: parent.top }
        height: Style.space(64)

        Rectangle {
          id: headerIcon
          width: Style.space(36); height: width
          radius: Style.cornerRadius > 0 ? Style.space(6) : 0
          color: root.faint
          anchors { left: parent.left; leftMargin: Style.space(18); verticalCenter: parent.verticalCenter }
          Text {
            textFormat: Text.PlainText
            anchors.centerIn: parent
            text: root.tab === "prefs" ? ""
              : root.view === "result" && root.last && root.last.result === "ok" ? ""
              : root.view === "result" ? ""
              : root.view === "staged" ? ""
              : root.view === "password" ? "" : ""
            color: root.view === "result" && root.last && root.last.result === "ok" ? root.okColor
              : root.view === "result" ? Color.urgent
              : root.view === "staged" ? root.warnColor : Color.accent
            font.family: root.family
            font.pixelSize: Style.font.heading
          }
        }

        Column {
          anchors { left: headerIcon.right; leftMargin: Style.space(12); right: tabs.left; rightMargin: Style.space(12); verticalCenter: parent.verticalCenter }
          spacing: Style.space(2)
          Text {
            textFormat: Text.PlainText
            width: parent.width
            elide: Text.ElideRight
            text: root.tab === "prefs" ? "Preferências" : root.headerTitle()
            color: root.fg
            font.family: root.family
            font.pixelSize: Style.font.heading
            font.bold: true
          }
          Text {
            textFormat: Text.PlainText
            width: parent.width
            elide: Text.ElideRight
            text: root.tab === "prefs" ? "~/.config/omarchy/update.json" : root.headerSubtitle()
            color: root.dim
            font.family: root.family
            font.pixelSize: Style.font.bodySmall
          }
        }

        Row {
          id: tabs
          anchors { right: parent.right; rightMargin: Style.space(18); verticalCenter: parent.verticalCenter }
          spacing: Style.space(2)
          Button {
            text: "Atualizações"
            selected: root.tab === "updates"
            bordered: true
            fontSize: Style.font.bodySmall
            onClicked: root.tab = "updates"
          }
          Button {
            text: "Preferências"
            selected: root.tab === "prefs"
            bordered: true
            fontSize: Style.font.bodySmall
            onClicked: { root.tab = "prefs"; prefsProc.running = true }
          }
        }
      }

      Rectangle { id: headerRule; anchors { left: parent.left; right: parent.right; top: header.bottom } height: 1; color: root.faint }

      // Body
      Flickable {
        id: body
        anchors { left: parent.left; right: parent.right; top: headerRule.bottom; bottom: footerRule.top }
        contentHeight: bodyColumn.implicitHeight + Style.space(32)
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar { policy: body.contentHeight > body.height ? ScrollBar.AsNeeded : ScrollBar.AlwaysOff }

        Column {
          id: bodyColumn
          x: Style.space(18)
          y: Style.space(16)
          width: body.width - Style.space(36)
          spacing: Style.space(12)

          // ---------- updates: idle ----------
          Column {
            visible: root.tab === "updates" && root.view === "idle"
            width: parent.width
            spacing: Style.space(10)

            SourceRow {
              glyph: ""
              title: Model.systemCount(root.status) > 0 ? "Sistema · " + Model.plural(Model.systemCount(root.status), "pacote", "pacotes") : "Sistema"
              subtitle: Model.systemCount(root.status) > 0 ? Model.systemNames(root.status, 4) : "em dia"
              chip: Model.systemCount(root.status) === 0 ? "EM DIA" : (root.prefs.mode === "boot" ? "NO REINÍCIO" : "AGORA")
              chipColor: Model.systemCount(root.status) === 0 ? root.dim : (root.prefs.mode === "boot" ? root.warnColor : root.okColor)
              active: Model.systemCount(root.status) > 0
            }
            SourceRow {
              glyph: ""
              title: Model.flatpakCount(root.status) > 0 ? "Flatpak · " + Model.plural(Model.flatpakCount(root.status), "item", "itens") : "Flatpak"
              subtitle: Model.flatpakCount(root.status) > 0 ? Model.flatpakNames(root.status, 3) : "em dia"
              chip: Model.flatpakCount(root.status) > 0 ? "NA HORA" : "EM DIA"
              chipColor: Model.flatpakCount(root.status) > 0 ? root.okColor : root.dim
              active: Model.flatpakCount(root.status) > 0
            }
            SourceRow {
              glyph: ""
              title: Model.doxiaBehind(root.status) > 0 ? "DoxIA · " + Model.plural(Model.doxiaBehind(root.status), "novidade", "novidades") : "DoxIA"
              subtitle: Model.doxiaBehind(root.status) > 0 ? root.status.doxia.commits[0]
                : ((root.status && root.status.doxia && root.status.doxia.head ? root.status.doxia.head + " · " : "") + "em dia")
              chip: Model.doxiaBehind(root.status) > 0 ? "NA HORA" : "EM DIA"
              chipColor: Model.doxiaBehind(root.status) > 0 ? root.okColor : root.dim
              active: Model.doxiaBehind(root.status) > 0
            }

            Banner {
              visible: !root.checking && root.status !== null
              tone: Model.systemCount(root.status) > 0 && root.prefs.mode === "boot" ? "warn" : (Model.anything(root.status) ? "ok" : "plain")
              glyph: tone === "warn" ? "" : ""
              text: Model.systemCount(root.status) > 0
                ? (root.prefs.mode === "boot"
                  ? "Os pacotes do sistema serão baixados agora e instalados no próximo reinício. Você pode continuar usando o computador."
                  : "Os pacotes do sistema serão instalados agora. Feche os apps que estiverem sendo atualizados.")
                : Model.onlyFlatpak(root.status)
                  ? "Só apps Flatpak. Eles são atualizados agora, sem senha e sem reiniciar."
                  : Model.doxiaBehind(root.status) > 0
                    ? "Só novidades do DoxIA. São aplicadas agora, sem reiniciar."
                    : "Tudo em dia."
            }

            Banner {
              visible: !!(root.status && root.status.errors && root.status.errors.length)
              tone: "error"
              glyph: ""
              text: root.status && root.status.errors ? root.status.errors.join(" ") : ""
            }

            Banner {
              visible: root.checking || root.status === null
              tone: "plain"
              glyph: ""
              spinning: true
              text: "Procurando atualizações…"
            }
          }

          // ---------- updates: password ----------
          Column {
            visible: root.tab === "updates" && root.view === "password"
            width: parent.width
            spacing: Style.space(10)

            Caption { text: "Será executado como root:" }
            CommandBox {
              lines: root.run && root.run.rootCommands ? root.run.rootCommands : []
            }
            Caption { text: "Senha de " + Quickshell.env("USER") }
            TextField {
              id: passwordField
              width: parent.width
              password: true
              onAccepted: if (text.length) root.answer(text)
            }
            Text {
              textFormat: Text.PlainText
              visible: root.run && root.run.askpass && root.run.askpass.attempt > 1
              text: "Senha incorreta. Tente de novo."
              color: Color.urgent
              font.family: root.family
              font.pixelSize: Style.font.bodySmall
            }
          }

          // ---------- updates: running ----------
          Column {
            visible: root.tab === "updates" && root.view === "running"
            width: parent.width
            spacing: Style.space(12)

            ProgressLine { value: root.run ? (root.run.progress || 0) / 100 : 0 }

            StepList { steps: root.run && root.run.steps ? root.run.steps : [] }

            LogBox {}
          }

          // ---------- updates: staged ----------
          Column {
            visible: root.tab === "updates" && root.view === "staged"
            width: parent.width
            spacing: Style.space(12)

            StepList {
              steps: [
                { id: "a", label: "Pacotes baixados e testados", state: "done", detail: Model.stagedPackages(root.status).length ? String(Model.stagedPackages(root.status).length) : "" },
                { id: "b", label: "Instalar durante o reinício", state: "todo", detail: "" },
                { id: "c", label: "Migrações e configuração do DoxIA", state: "todo", detail: "" },
                { id: "d", label: "Remover pacotes órfãos", state: root.prefs.autoremoveOrphans ? "todo" : "skip", detail: root.prefs.autoremoveOrphans ? "auto" : "desligado" }
              ]
            }

            CommandBox {
              commands: false
              visible: Model.stagedPackages(root.status).length > 0
              lines: {
                var p = Model.stagedPackages(root.status)
                var out = p.slice(0, 6)
                if (p.length > 6) out.push("+ " + (p.length - 6) + " outros")
                return out
              }
            }

            Banner {
              tone: "warn"
              glyph: ""
              text: "A atualização é instalada na próxima vez que você reiniciar ou desligar, antes de voltar para o login. Até lá, continue usando normalmente."
            }
          }

          // ---------- updates: result ----------
          Column {
            visible: root.tab === "updates" && root.view === "result"
            width: parent.width
            spacing: Style.space(12)

            Banner {
              visible: !!(root.last && root.last.result !== "ok")
              tone: "error"
              glyph: ""
              text: Model.failureText(root.last)
            }

            StepList {
              steps: root.last && root.last.steps ? root.last.steps.filter(function(s) { return s.state !== "todo" }) : []
            }

            Banner {
              visible: !!(root.last && root.last.reboot)
              tone: "warn"
              glyph: ""
              text: (root.last && root.last.reboot ? root.last.reboot : "") + ". Reinicie para usar a versão nova."
            }

            LogBox { visible: !!(root.last && root.last.result !== "ok" && root.last.mode !== "boot") }
          }

          // ---------- preferences ----------
          Column {
            visible: root.tab === "prefs"
            width: parent.width
            spacing: Style.space(10)

            Caption { text: "QUANDO APLICAR OS PACOTES DO SISTEMA"; font.letterSpacing: 1.2; font.bold: true }
            RadioCard {
              checked: root.prefs.mode === "boot"
              title: "No próximo reinício"
              description: "Baixa agora e instala com o sistema parado (dnf offline), mais seguro e sem apps abertos usando bibliotecas trocadas. Flatpak e DoxIA continuam sendo aplicados na hora."
              onPicked: root.setPref("mode", "boot")
            }
            RadioCard {
              checked: root.prefs.mode === "now"
              title: "Agora, com o sistema rodando"
              description: "Instala na hora e só pede reinício se o kernel ou o Hyprland mudarem."
              onPicked: root.setPref("mode", "now")
            }

            Caption { text: "LIMPEZA"; font.letterSpacing: 1.2; font.bold: true; topPadding: Style.space(6) }
            Toggle {
              width: parent.width
              label: "Remover pacotes órfãos automaticamente"
              description: "Roda dnf autoremove no fim da atualização, sem perguntar."
              checked: root.prefs.autoremoveOrphans === true
              onClicked: root.setPref("autoremoveOrphans", !checked)
            }
            Toggle {
              width: parent.width
              enabled: !root.status || root.status.snapper !== false
              opacity: enabled ? 1 : 0.5
              label: "Criar snapshot antes de atualizar"
              description: enabled ? "Snapper. Dá para voltar pelo menu de boot se algo quebrar." : "O Snapper não está instalado."
              checked: root.prefs.snapshot === true && enabled
              onClicked: if (enabled) root.setPref("snapshot", !checked)
            }

            Caption { text: "VERIFICAÇÃO"; font.letterSpacing: 1.2; font.bold: true; topPadding: Style.space(6) }
            Dropdown {
              width: parent.width
              label: "Procurar atualizações"
              value: String(root.prefs.checkHours)
              options: [
                { value: "1", label: "a cada hora" },
                { value: "3", label: "a cada 3 horas" },
                { value: "6", label: "a cada 6 horas" },
                { value: "12", label: "a cada 12 horas" },
                { value: "24", label: "uma vez por dia" }
              ]
              onChanged: function(v) { root.setPref("checkHours", parseInt(v)) }
            }
            Toggle {
              width: parent.width
              label: "Atualizar Flatpaks sozinho"
              description: "Em segundo plano, sem abrir esta janela. Uma notificação avisa quando terminar."
              checked: root.prefs.autoFlatpak === true
              onClicked: root.setPref("autoFlatpak", !checked)
            }
          }
        }
      }

      Rectangle { id: footerRule; anchors { left: parent.left; right: parent.right; bottom: footer.top } height: 1; color: root.faint; visible: footer.visible }

      // Footer: actions for the current view
      Item {
        id: footer
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
        height: root.tab === "updates" ? Style.space(56) : 0
        visible: root.tab === "updates"

        Row {
          anchors { left: parent.left; leftMargin: Style.space(18); verticalCenter: parent.verticalCenter }
          spacing: Style.space(8)

          Button {
            visible: root.view === "idle"
            iconText: ""
            text: "Verificar"
            bordered: true
            iconSpinning: root.checking
            onClicked: root.check(true)
          }
          Button {
            visible: root.view === "staged"
            text: "Cancelar atualização"
            bordered: true
            onClicked: Quickshell.execDetached(["omarchy-update-run", "cancel-staged"])
          }
          Button {
            visible: root.view === "result" && root.last && root.last.result !== "ok" && root.last.mode !== "boot"
            text: "Ver log"
            bordered: true
            onClicked: root.openLog()
          }
          Text {
            textFormat: Text.PlainText
            visible: root.view === "running"
            anchors.verticalCenter: parent.verticalCenter
            text: "Pode fechar: o ícone da barra mostra o progresso."
            color: root.dim
            font.family: root.family
            font.pixelSize: Style.font.bodySmall
          }
        }

        Row {
          anchors { right: parent.right; rightMargin: Style.space(18); verticalCenter: parent.verticalCenter }
          spacing: Style.space(8)
          layoutDirection: Qt.RightToLeft

          // idle
          PrimaryButton {
            visible: root.view === "idle" && !root.checking && Model.anything(root.status)
            text: Model.systemCount(root.status) > 0
              ? (root.prefs.mode === "boot" ? "Preparar para o reinício" : "Atualizar agora")
              : "Atualizar agora"
            onClicked: {
              if (Model.systemCount(root.status) > 0) root.start(root.prefs.mode)
              else if (Model.onlyFlatpak(root.status)) root.start("flatpak")
              else root.start("now")
            }
          }
          Button {
            visible: root.view === "idle" && !root.checking && Model.systemCount(root.status) > 0
            text: root.prefs.mode === "boot" ? "Atualizar agora mesmo" : "Instalar no reinício"
            bordered: true
            onClicked: root.start(root.prefs.mode === "boot" ? "now" : "boot")
          }

          // password
          PrimaryButton {
            visible: root.view === "password"
            text: "Continuar"
            onClicked: if (passwordField.text.length) root.answer(passwordField.text)
          }
          Button {
            visible: root.view === "password"
            text: "Cancelar"
            bordered: true
            onClicked: Quickshell.execDetached(["omarchy-update-run", "stop"])
          }

          // running
          Button {
            visible: root.view === "running"
            text: "Ocultar"
            bordered: true
            onClicked: root.requestClose()
          }

          // staged
          PrimaryButton {
            visible: root.view === "staged"
            text: "Reiniciar agora"
            onClicked: root.reboot()
          }
          Button {
            visible: root.view === "staged"
            text: "Fechar"
            bordered: true
            onClicked: root.requestClose()
          }

          // result
          PrimaryButton {
            visible: root.view === "result"
            text: root.last && root.last.reboot ? "Reiniciar agora" : (root.last && root.last.result !== "ok" ? "Tentar de novo" : "Fechar")
            onClicked: {
              if (root.last && root.last.reboot) { root.markSeen(); root.reboot() }
              else if (root.last && root.last.result !== "ok") { root.markSeen(); root.check(true) }
              else { root.markSeen(); root.requestClose() }
            }
          }
          Button {
            visible: root.view === "result" && !!(root.last && (root.last.reboot || root.last.result !== "ok"))
            text: "Fechar"
            bordered: true
            onClicked: { root.markSeen(); root.requestClose() }
          }
        }
      }
    }
  }

  function headerTitle() {
    if (view === "password") return "Confirme sua senha"
    if (view === "running") {
      var s = Model.currentStep(run)
      return s ? s.label : "Preparando…"
    }
    if (view === "staged") return "Pronto para instalar"
    if (view === "result") {
      if (!last) return ""
      if (last.result !== "ok") return "A atualização não terminou"
      return last.mode === "boot" ? "Sistema atualizado" : (last.mode === "flatpak" ? "Apps atualizados" : "Tudo atualizado")
    }
    if (checking && !status) return "Procurando atualizações"
    var n = Model.updatesCount(status)
    return n > 0 ? Model.plural(n, "atualização", "atualizações") : "Tudo em dia"
  }

  function headerSubtitle() {
    if (view === "password") return run && run.mode === "boot" ? "Para instalar no próximo reinício" : "Uma vez, para toda a atualização"
    if (view === "running") {
      var s = Model.currentStep(run)
      var parts = []
      if (s && s.detail) parts.push(s.detail)
      if (run && run.startedAt) parts.push(Model.duration(Date.now() / 1000 - run.startedAt))
      return parts.join(" · ")
    }
    if (view === "staged") {
      var n = Model.stagedPackages(status).length
      return (n ? Model.plural(n, "pacote", "pacotes") + " · " : "") + "preparado às " + Model.clock(status.staged.stagedAt)
    }
    if (view === "result" && last) {
      var bits = []
      var sum = last.summary || {}
      if (sum.packages) bits.push(Model.plural(sum.packages, "pacote", "pacotes"))
      if (last.startedAt && last.finishedAt) bits.push(Model.duration(last.finishedAt - last.startedAt))
      bits.push("às " + Model.clock(last.finishedAt))
      return bits.join(" · ")
    }
    return checking ? "Verificando…" : "Verificado " + Model.ago(status ? status.checkedAt : 0)
  }

  // ---- components -----------------------------------------------------------
  component Caption: Text {
    textFormat: Text.PlainText
    color: root.dim
    font.family: root.family
    font.pixelSize: Style.font.caption
  }

  component PrimaryButton: Button {
    background: Color.accent
    foreground: Color.background
    accent: Color.background
  }

  component SourceRow: Rectangle {
    id: src
    property string glyph: ""
    property string title: ""
    property string subtitle: ""
    property string chip: ""
    property color chipColor: root.dim
    property bool active: true
    width: parent ? parent.width : 0
    implicitHeight: Math.max(Style.space(52), srcText.implicitHeight + Style.space(18))
    color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.03)
    border.color: root.faint
    border.width: 1
    radius: Style.cornerRadius > 0 ? Style.space(6) : 0
    opacity: active ? 1 : 0.55

    Rectangle {
      id: srcIcon
      width: Style.space(30); height: width
      radius: Style.cornerRadius > 0 ? Style.space(5) : 0
      color: root.faint
      anchors { left: parent.left; leftMargin: Style.space(12); verticalCenter: parent.verticalCenter }
      Text { textFormat: Text.PlainText; anchors.centerIn: parent; text: src.glyph; color: root.fg; font.family: root.family; font.pixelSize: Style.font.title }
    }
    Column {
      id: srcText
      anchors { left: srcIcon.right; leftMargin: Style.space(12); right: srcChip.left; rightMargin: Style.space(10); verticalCenter: parent.verticalCenter }
      spacing: Style.space(2)
      Text { textFormat: Text.PlainText; width: parent.width; elide: Text.ElideRight; text: src.title; color: root.fg; font.family: root.family; font.pixelSize: Style.font.body; font.bold: true }
      Text { textFormat: Text.PlainText; width: parent.width; elide: Text.ElideRight; text: src.subtitle; color: root.dim; font.family: root.family; font.pixelSize: Style.font.bodySmall }
    }
    Text {
      textFormat: Text.PlainText
      id: srcChip
      anchors { right: parent.right; rightMargin: Style.space(12); verticalCenter: parent.verticalCenter }
      text: src.chip
      color: src.chipColor
      font.family: root.family
      font.pixelSize: Style.font.caption
      font.bold: true
      font.letterSpacing: 0.8
    }
  }

  component Banner: Rectangle {
    id: banner
    property string tone: "plain"   // plain | ok | warn | error
    property string glyph: ""
    property string text: ""
    property bool spinning: false
    readonly property color toneColor: tone === "ok" ? root.okColor : tone === "warn" ? root.warnColor : tone === "error" ? Color.urgent : root.fg
    width: parent ? parent.width : 0
    implicitHeight: bannerText.implicitHeight + Style.space(20)
    color: Qt.rgba(toneColor.r, toneColor.g, toneColor.b, tone === "plain" ? 0.04 : 0.09)
    border.color: Qt.rgba(toneColor.r, toneColor.g, toneColor.b, tone === "plain" ? 0.12 : 0.3)
    border.width: 1
    radius: Style.cornerRadius > 0 ? Style.space(6) : 0

    Text {
      textFormat: Text.PlainText
      id: bannerGlyph
      anchors { left: parent.left; leftMargin: Style.space(12); top: parent.top; topMargin: Style.space(10) }
      text: banner.glyph
      color: banner.toneColor
      font.family: root.family
      font.pixelSize: Style.font.body
      RotationAnimation on rotation { running: banner.spinning && banner.visible; from: 0; to: 360; duration: 1200; loops: Animation.Infinite }
    }
    Text {
      textFormat: Text.PlainText
      id: bannerText
      anchors { left: bannerGlyph.right; leftMargin: Style.space(10); right: parent.right; rightMargin: Style.space(12); top: parent.top; topMargin: Style.space(10) }
      text: banner.text
      wrapMode: Text.WordWrap
      color: root.fg
      font.family: root.family
      font.pixelSize: Style.font.bodySmall
      lineHeight: 1.15
    }
  }

  component CommandBox: Rectangle {
    id: cmdBox
    property var lines: []
    property bool commands: true
    width: parent ? parent.width : 0
    implicitHeight: cmdCol.implicitHeight + Style.space(16)
    color: Qt.darker(Color.background, 1.3)
    border.color: root.faint
    border.width: 1
    radius: Style.cornerRadius > 0 ? Style.space(5) : 0
    Column {
      id: cmdCol
      x: Style.space(10); y: Style.space(8)
      width: parent.width - Style.space(20)
      spacing: Style.space(3)
      Repeater {
        model: cmdBox.lines
        delegate: Text {
          required property var modelData
          width: cmdCol.width
          wrapMode: Text.WrapAnywhere
          text: (cmdBox.commands ? "<font color='" + Color.accent + "'>#</font> " : "") + String(modelData).replace(/&/g, "&amp;").replace(/</g, "&lt;")
          textFormat: Text.StyledText
          color: root.fg
          font.family: root.family
          font.pixelSize: Style.font.bodySmall
        }
      }
    }
  }

  component ProgressLine: Rectangle {
    property real value: 0
    width: parent ? parent.width : 0
    height: Style.space(6)
    radius: height / 2
    color: root.faint
    Rectangle {
      width: parent.width * Math.max(0, Math.min(1, parent.value))
      height: parent.height
      radius: parent.radius
      color: Color.accent
      Behavior on width { NumberAnimation { duration: 400; easing.type: Easing.OutCubic } }
    }
  }

  component StepList: Column {
    id: stepList
    property var steps: []
    width: parent ? parent.width : 0
    spacing: Style.space(2)
    Repeater {
      model: stepList.steps
      delegate: Item {
        required property var modelData
        width: stepList.width
        height: Style.space(26)
        readonly property string st: modelData.state || "todo"

        Rectangle {
          id: dot
          width: Style.space(14); height: width; radius: width / 2
          anchors { left: parent.left; verticalCenter: parent.verticalCenter }
          color: st === "done" ? root.okColor : st === "fail" ? Color.urgent : "transparent"
          border.width: st === "done" || st === "fail" ? 0 : 2
          border.color: st === "run" ? Color.accent : Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.3)
          Text {
            textFormat: Text.PlainText
            anchors.centerIn: parent
            visible: st === "done" || st === "fail"
            text: st === "done" ? "" : ""
            color: Color.background
            font.family: root.family
            font.pixelSize: Style.font.caption
          }
          SequentialAnimation on opacity {
            running: st === "run"
            loops: Animation.Infinite
            NumberAnimation { to: 0.35; duration: 600 }
            NumberAnimation { to: 1; duration: 600 }
            onRunningChanged: if (!running) dot.opacity = 1
          }
        }
        Text {
          textFormat: Text.PlainText
          anchors { left: dot.right; leftMargin: Style.space(10); right: stepDetail.left; rightMargin: Style.space(8); verticalCenter: parent.verticalCenter }
          elide: Text.ElideRight
          text: modelData.label
          color: st === "run" ? root.fg : st === "todo" || st === "skip" ? root.dim : root.fg
          font.family: root.family
          font.pixelSize: Style.font.body
          font.bold: st === "run"
          font.strikeout: st === "skip"
        }
        Text {
          textFormat: Text.PlainText
          id: stepDetail
          anchors { right: parent.right; verticalCenter: parent.verticalCenter }
          text: modelData.detail ? String(modelData.detail) : (st === "done" && modelData.seconds > 1 ? Model.duration(modelData.seconds) : "")
          color: root.dim
          font.family: root.family
          font.pixelSize: Style.font.caption
        }
      }
    }
  }

  component LogBox: Column {
    width: parent ? parent.width : 0
    spacing: Style.space(6)
    Button {
      text: (root.logOpen ? "▾ " : "▸ ") + "Saída"
      fontSize: Style.font.bodySmall
      onClicked: root.logOpen = !root.logOpen
    }
    Rectangle {
      visible: root.logOpen
      width: parent.width
      implicitHeight: logText.implicitHeight + Style.space(16)
      color: Qt.darker(Color.background, 1.3)
      border.color: root.faint
      border.width: 1
      radius: Style.cornerRadius > 0 ? Style.space(5) : 0
      Text {
        textFormat: Text.PlainText
        id: logText
        x: Style.space(10); y: Style.space(8)
        width: parent.width - Style.space(20)
        text: root.logTail.length ? root.logTail.join("\n") : "…"
        elide: Text.ElideRight
        maximumLineCount: 9
        wrapMode: Text.NoWrap
        color: root.dim
        font.family: root.family
        font.pixelSize: Style.font.caption
      }
    }
  }

  component RadioCard: Rectangle {
    id: radio
    property bool checked: false
    property string title: ""
    property string description: ""
    signal picked()
    width: parent ? parent.width : 0
    implicitHeight: radioText.implicitHeight + Style.space(20)
    color: checked ? Qt.rgba(Color.accent.r, Color.accent.g, Color.accent.b, 0.07) : Qt.rgba(root.fg.r, root.fg.g, root.fg.b, radioMouse.containsMouse ? 0.05 : 0.02)
    border.color: checked ? Qt.rgba(Color.accent.r, Color.accent.g, Color.accent.b, 0.6) : root.faint
    border.width: 1
    radius: Style.cornerRadius > 0 ? Style.space(6) : 0

    Rectangle {
      id: radioDot
      width: Style.space(14); height: width; radius: width / 2
      anchors { left: parent.left; leftMargin: Style.space(12); top: parent.top; topMargin: Style.space(12) }
      color: "transparent"
      border.width: 2
      border.color: radio.checked ? Color.accent : Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.35)
      Rectangle {
        anchors.centerIn: parent
        visible: radio.checked
        width: parent.width - Style.space(8); height: width; radius: width / 2
        color: Color.accent
      }
    }
    Column {
      id: radioText
      anchors { left: radioDot.right; leftMargin: Style.space(10); right: parent.right; rightMargin: Style.space(12); top: parent.top; topMargin: Style.space(10) }
      spacing: Style.space(3)
      Text { textFormat: Text.PlainText; width: parent.width; text: radio.title; color: root.fg; font.family: root.family; font.pixelSize: Style.font.body; font.bold: true }
      Text { textFormat: Text.PlainText; width: parent.width; text: radio.description; wrapMode: Text.WordWrap; color: root.dim; font.family: root.family; font.pixelSize: Style.font.bodySmall }
    }
    MouseArea {
      id: radioMouse
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: radio.picked()
    }
  }
}
