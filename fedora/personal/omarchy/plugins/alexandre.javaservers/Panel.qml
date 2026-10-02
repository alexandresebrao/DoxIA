import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

// Ícone do Java na barra: claro com algum servidor no ar, escuro com tudo
// parado. O popup tem play/stop do Tomcat e do JBoss. Todo o controle fica em
// ctl.py, que sobe os servidores com o JDK Oracle 8 do SDKMAN.
Panel {
  id: root
  moduleName: "alexandre.javaservers"
  ipcTarget: "alexandre.javaservers"
  manageIpc: false

  readonly property string javaGlyph: String.fromCodePoint(0xE256)
  // Na barra (13px) o logo do Java fica ralo perto dos outros ícones; o
  // servidor preenchido tem o mesmo peso. O logo fica no topo do popup.
  readonly property string serverGlyph: String.fromCodePoint(0xF048B)
  readonly property string playGlyph: String.fromCodePoint(0xF040A)
  readonly property string stopGlyph: String.fromCodePoint(0xF04DB)
  readonly property string logGlyph: String.fromCodePoint(0xF0219)

  readonly property int refreshIntervalSec: Math.max(2, parseInt(setting("refreshIntervalSec", 5), 10) || 5)
  readonly property string pluginDir: String(Qt.resolvedUrl(".")).replace(/^file:\/\//, "").replace(/\/$/, "")

  // Último snapshot do ctl.py status.
  property var services: []
  property bool javaOk: true
  property double now: Date.now() / 1000

  // Ações em andamento por serviço: { tomcat: "start" | "stop" }.
  property var pending: ({})
  property var pendingSince: ({})

  readonly property bool busy: Object.keys(pending).length > 0
    || services.some(function(s) { return s.estado === "iniciando" })
  readonly property int runningCount: services.filter(function(s) { return s.pid > 0 }).length
  readonly property bool anyRunning: runningCount > 0

  readonly property color foreground: bar ? bar.foreground : Color.foreground
  readonly property color urgent: bar ? bar.urgent : Color.urgent
  readonly property color dim: Qt.darker(foreground, 1.55)
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family

  readonly property string statusText: {
    if (!javaOk) return "JDK Oracle 8 não encontrado"
    if (runningCount === 0) return "Tudo parado"
    return services.filter(function(s) { return s.pid > 0 })
      .map(function(s) { return s.nome + " " + stateLabel(s) }).join(" · ")
  }

  function stateLabel(s) {
    var p = pending[s.id]
    if (p === "stop") return "parando…"
    if (p === "start" && s.pid === 0) return "iniciando…"
    if (s.estado === "iniciando") return "iniciando…"
    return s.estado
  }

  function formatDuration(seconds) {
    var s = Math.max(0, Math.floor(seconds || 0))
    var h = Math.floor(s / 3600)
    var m = Math.floor((s % 3600) / 60)
    if (h > 0) return h + "h " + (m < 10 ? "0" : "") + m + "m"
    if (m > 0) return m + "m " + (s % 60 < 10 ? "0" : "") + (s % 60) + "s"
    return (s % 60) + "s"
  }

  function detailText(s) {
    if (s.pid === 0) return pending[s.id] === "start" ? "Subindo com JDK Oracle 8…" : s.url
    return "PID " + s.pid + " · " + formatDuration(s.uptime) + " · " + s.memoriaMb + " MB"
  }

  function refresh() {
    if (statusProc.running) return
    statusProc.command = ["python3", pluginDir + "/ctl.py", "status"]
    statusProc.running = true
  }

  function applyStatus(text) {
    if (!text) return
    try {
      var data = JSON.parse(text)
      services = data.servicos || []
      javaOk = !!data.javaOk
      now = Date.now() / 1000
      // A ação some quando o status confirma o resultado (ou após 90s), para
      // o popup nunca ficar preso em "Aguarde…".
      services.forEach(function(sv) {
        var action = pending[sv.id]
        if (!action) return
        var done = (action === "start" && sv.pid > 0) || (action === "stop" && sv.pid === 0)
        if (done || now - (pendingSince[sv.id] || now) > 90) root.setPending(sv.id, "")
      })
    } catch (e) {
      console.warn("alexandre.javaservers: status inválido", e)
    }
  }

  function setPending(id, action) {
    var p = Object.assign({}, pending)
    var since = Object.assign({}, pendingSince)
    if (action) { p[id] = action; since[id] = Date.now() / 1000 }
    else { delete p[id]; delete since[id] }
    pendingSince = since
    pending = p
  }

  function procFor(id) { return id === "tomcat" ? tomcatProc : jbossProc }

  function run(id, action) {
    var proc = procFor(id)
    if (proc.running) return
    setPending(id, action)
    proc.serviceId = id
    proc.command = ["python3", pluginDir + "/ctl.py", action, id]
    proc.running = true
    refresh()
  }

  function toggleService(s) {
    if (pending[s.id]) return
    run(s.id, s.pid > 0 ? "stop" : "start")
  }

  function startAll() { services.forEach(function(s) { if (s.pid === 0) run(s.id, "start") }) }
  function stopAll() { services.forEach(function(s) { if (s.pid > 0) run(s.id, "stop") }) }

  // O terminal só lê o arquivo de log: fechar a janela ou dar Ctrl+C não
  // afeta o servidor, que grava no arquivo pela própria sessão.
  function openLog(s) {
    Quickshell.execDetached(["setsid", "uwsm-app", "--", "xdg-terminal-exec",
      "bash", "-c",
      'printf "\\e]2;Log %s\\a" "$2"; printf "\\e[2m== %s · fechar esta janela NÃO para o servidor ==\\e[0m\\n" "$1"; exec tail -n +1 -F "$1"',
      "log", s.log, s.nome])
    close()
  }

  // Ciclo branco → vermelho → branco do LED de "no ar" (3 s).
  property real ledPhase: 0
  SequentialAnimation on ledPhase {
    running: root.opened && root.anyRunning
    loops: Animation.Infinite
    NumberAnimation { to: 1.0; duration: 1500; easing.type: Easing.InOutSine }
    NumberAnimation { to: 0.0; duration: 1500; easing.type: Easing.InOutSine }
  }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  Component.onCompleted: refresh()
  onOpenedChanged: if (opened) {
    refresh()
    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }

  Process {
    id: statusProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.applyStatus(String(text || "").trim())
    }
  }

  component ActionProc: Process {
    property string serviceId: ""
    stderr: StdioCollector { id: err; waitForEnd: true }
    onExited: function(exitCode) {
      if (exitCode !== 0)
        Quickshell.execDetached(["omarchy-notification-send", "-g", root.javaGlyph,
          "Falha no " + serviceId, String(err.text || "").trim() || ("código " + exitCode)])
      root.setPending(serviceId, "")
      root.refresh()
    }
  }

  ActionProc { id: tomcatProc }
  ActionProc { id: jbossProc }

  Timer {
    interval: root.busy || root.opened ? 1500 : root.refreshIntervalSec * 1000
    running: true
    repeat: true
    onTriggered: root.refresh()
  }

  IpcHandler {
    target: root.ipcTarget
    function open(): void { root.open() }
    function close(): void { root.close() }
    function show(): void { root.open() }
    function hide(): void { root.close() }
    function toggle(): void { root.toggle() }
    function start(id: string): string { root.run(id, "start"); return "ok" }
    function stop(id: string): string { root.run(id, "stop"); return "ok" }
    function startAll(): string { root.startAll(); return "ok" }
    function stopAll(): string { root.stopAll(); return "ok" }
    function log(id: string): string {
      var s = root.services.filter(function(x) { return x.id === id })[0]
      if (!s) return "serviço desconhecido"
      root.openLog(s)
      return "ok"
    }
    function status(): string { return root.statusText }
    function debug(): string {
      return JSON.stringify({ anyRunning: root.anyRunning, busy: root.busy, pending: root.pending,
        opacity: button.opacity, fg: String(button.foreground), barFg: String(root.barForeground) })
    }
  }

  // ---------------- barra ----------------

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.serverGlyph
    foreground: root.anyRunning ? root.barForeground : Qt.darker(root.barForeground, 1.55)
    tooltipText: "Servidores Java: " + root.statusText
    onPressed: function(buttonCode) { root.toggle() }
  }

  // ---------------- popup ----------------

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(380))
    contentHeight: panel.fittedContentHeight(column.implicitHeight, Style.space(520))

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }
      onTextKey: function(t) {
        if (t === "t" || t === "T") { if (root.services[0]) root.toggleService(root.services[0]) }
        else if (t === "j" || t === "J") { if (root.services[1]) root.toggleService(root.services[1]) }
      }

      Column {
        id: column
        width: parent.width
        spacing: Style.space(12)

        PanelHero {
          width: parent.width
          title: "Servidores Java"
          meta: root.javaOk ? "JDK Oracle 8.0.192" : "JDK Oracle 8 não encontrado no SDKMAN"
          foreground: root.foreground
          fontFamily: root.fontFamily
          iconOpacity: root.anyRunning ? 1.0 : 0.5
          iconComponent: Component {
            Text {
              text: root.javaGlyph
              color: root.foreground
              font.family: root.fontFamily
              font.pixelSize: Style.font.display
            }
          }
        }

        PanelSeparator { width: parent.width; foreground: root.foreground }

        Repeater {
          model: root.services

          CursorSurface {
            id: row
            required property var modelData
            readonly property bool up: modelData.pid > 0
            readonly property string action: root.pending[modelData.id] || ""
            readonly property bool transitioning: action !== "" || modelData.estado === "iniciando"

            width: column.width
            implicitHeight: rowInner.implicitHeight + Style.space(14)
            hasCursor: rowMouse.containsMouse
            foreground: root.foreground

            MouseArea {
              id: rowMouse
              anchors.fill: parent
              hoverEnabled: true
              acceptedButtons: Qt.NoButton
            }

            RowLayout {
              id: rowInner
              anchors.left: parent.left
              anchors.right: parent.right
              anchors.verticalCenter: parent.verticalCenter
              anchors.leftMargin: Style.space(10)
              anchors.rightMargin: Style.space(6)
              spacing: Style.space(8)

              Rectangle {
                Layout.alignment: Qt.AlignVCenter
                width: Style.space(9)
                height: width
                radius: width / 2
                color: row.up ? root.foreground : "transparent"
                border.width: 1
                border.color: row.up ? root.foreground : root.dim
                SequentialAnimation on opacity {
                  running: row.transitioning
                  loops: Animation.Infinite
                  NumberAnimation { to: 0.25; duration: 500 }
                  NumberAnimation { to: 1.0; duration: 500 }
                  onRunningChanged: if (!running) parent.opacity = 1.0
                }

                // No ar: alterna branco ↔ vermelho, como um LED de atividade.
                Rectangle {
                  id: led
                  anchors.fill: parent
                  radius: parent.radius
                  color: "#d62a2f"
                  // Fase vem do painel: o status recria as linhas a cada
                  // refresh e uma animação local reiniciaria no branco.
                  opacity: row.up && !row.transitioning ? root.ledPhase : 0
                }
              }

              Column {
                Layout.fillWidth: true
                spacing: Style.space(2)

                Text {
                  textFormat: Text.PlainText
                  width: parent.width
                  text: row.modelData.nome + "  ·  " + root.stateLabel(row.modelData)
                  color: root.foreground
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.body
                  font.bold: row.up
                  elide: Text.ElideRight
                }

                Text {
                  textFormat: Text.PlainText
                  width: parent.width
                  text: root.detailText(row.modelData)
                  color: root.dim
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.caption
                  elide: Text.ElideRight
                }
              }

              Button {
                Layout.alignment: Qt.AlignVCenter
                iconText: root.logGlyph
                text: "Log"
                tooltipText: "Ver o output em tempo real (fechar não para o servidor)"
                foreground: root.foreground
                fontFamily: root.fontFamily
                fontSize: Style.font.bodySmall
                onClicked: root.openLog(row.modelData)
              }

              // Só o ícone, sem fundo nem borda; o hover clareia/aumenta.
              Button {
                id: toggleBtn
                Layout.alignment: Qt.AlignVCenter
                iconText: row.up ? root.stopGlyph : root.playGlyph
                tooltipText: row.action !== "" ? "Aguarde…" : (row.up ? "Parar " : "Iniciar ") + row.modelData.nome
                foreground: toggleBtn.hot ? Qt.lighter(root.foreground, 1.25) : root.foreground
                fontFamily: root.fontFamily
                iconSize: Style.font.iconLarge
                color: "transparent"
                borderSpec: Border.none()
                scale: toggleBtn.hot && toggleBtn.enabled ? 1.12 : 1.0
                Behavior on scale { NumberAnimation { duration: 120 } }
                enabled: row.action === "" && root.javaOk
                opacity: enabled ? 1.0 : 0.4
                onClicked: root.toggleService(row.modelData)
              }
            }
          }
        }

        PanelSeparator { width: parent.width; foreground: root.foreground }

        RowLayout {
          width: parent.width
          spacing: Style.space(8)

          Text {
            textFormat: Text.PlainText
            Layout.fillWidth: true
            text: "T / J alterna Tomcat / JBoss"
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            elide: Text.ElideRight
          }

          Button {
            text: "Iniciar todos"
            foreground: root.foreground
            fontFamily: root.fontFamily
            fontSize: Style.font.bodySmall
            visible: root.runningCount < root.services.length
            enabled: root.javaOk
            onClicked: root.startAll()
          }

          Button {
            text: "Parar todos"
            foreground: root.foreground
            fontFamily: root.fontFamily
            fontSize: Style.font.bodySmall
            visible: root.runningCount > 0
            onClicked: root.stopAll()
          }
        }
      }
    }
  }
}
