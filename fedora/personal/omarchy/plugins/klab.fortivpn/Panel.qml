import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

// Bar icon + popup for openfortivpn profiles. Each profile is
// /etc/openfortivpn/<name>.conf, run by the packaged openfortivpn@<name>.service
// unit. Profiles are read and written only through the root-owned helper that
// setup-system.sh installs; passwords go in over stdin and never come back.
//
// Views: "main" (connect to a profile), "list" (the gear: manage profiles)
// and "edit" (new/edit form).
Panel {
  id: root
  moduleName: "klab.fortivpn"
  ipcTarget: "klab.fortivpn"
  manageIpc: false

  readonly property string vpnGlyph: String.fromCodePoint(0xF0582)
  readonly property string lockGlyph: String.fromCodePoint(0xF099D)
  readonly property string cogGlyph: String.fromCodePoint(0xF0493)
  readonly property string plusGlyph: String.fromCodePoint(0xF0415)
  readonly property string pencilGlyph: String.fromCodePoint(0xF03EB)
  readonly property string trashGlyph: String.fromCodePoint(0xF01B4)
  readonly property string backGlyph: String.fromCodePoint(0xF004D)

  readonly property int refreshIntervalSec: Math.max(2, parseInt(setting("refreshIntervalSec", 10), 10) || 10)
  readonly property string pluginDir: String(Qt.resolvedUrl(".")).replace(/^file:\/\//, "").replace(/\/$/, "")
  readonly property string helper: "/usr/local/libexec/omarchy-fortivpn-helper"

  // Last status.sh snapshot.
  property bool helperInstalled: true
  property string activeProfile: ""
  property string activeState: ""
  property string focusProfile: ""
  property string focusState: ""
  property int externalPid: 0
  property double since: 0
  property string iface: ""
  property string ipAddress: ""
  property string peer: ""
  property int routes: 0
  property string unitError: ""
  property string certDigest: ""
  property bool loaded: false

  property var profiles: []
  property bool profilesLoaded: false

  // "" while following the real state, "up"/"down" while a toggle settles.
  property string pending: ""
  property string actionError: ""
  property double now: Date.now() / 1000

  property string view: "main"
  property string confirmDelete: ""
  property string editOriginal: ""
  property bool editHasPassword: false
  property string formError: ""

  readonly property bool connected: ipAddress !== ""
  readonly property bool external: externalPid > 0 && activeProfile === ""
  // With Type=simple the unit is "active" as soon as openfortivpn starts; the
  // tunnel is only up once the ppp interface has an address.
  readonly property bool starting: activeState === "activating" || (activeState === "active" && !connected)
  readonly property bool transitioning: pending !== "" || starting || activeState === "deactivating"
  readonly property string currentProfile: activeProfile !== "" ? activeProfile
    : (focusProfile !== "" && hasProfile(focusProfile) ? focusProfile
    : (profiles.length > 0 ? profiles[0].name : ""))
  readonly property bool switchOn: pending === "up" || (pending === "" && (connected || starting))
  readonly property string errorText: actionError !== "" ? actionError
    : (focusState === "failed" || starting ? unitError : "")

  readonly property color foreground: bar ? bar.foreground : Color.foreground
  readonly property color urgent: bar ? bar.urgent : Color.urgent
  readonly property color dim: Qt.darker(foreground, 1.55)
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family

  readonly property string statusText: {
    if (pending === "up" || starting) return "Conectando a " + currentProfile + "…"
    if (pending === "down" || activeState === "deactivating") return "Desconectando…"
    if (connected) return (external ? "Conectado (manual)" : "Conectado · " + activeProfile) + " · " + formatDuration(now - since)
    if (focusState === "failed") return "Falha em " + focusProfile
    return "Desconectado"
  }

  function hasProfile(name) {
    for (var i = 0; i < profiles.length; i++) if (profiles[i].name === name) return true
    return false
  }

  function profileState(name) {
    if (name !== activeProfile) return (name === focusProfile && focusState === "failed") ? "failed" : ""
    if (activeState === "active" && connected) return "connected"
    if (starting) return "activating"
    return activeState
  }

  function formatDuration(seconds) {
    if (!since || seconds < 0) return ""
    var s = Math.floor(seconds)
    var h = Math.floor(s / 3600)
    var m = Math.floor((s % 3600) / 60)
    if (h > 0) return h + "h " + (m < 10 ? "0" : "") + m + "m"
    if (m > 0) return m + "m " + (s % 60 < 10 ? "0" : "") + (s % 60) + "s"
    return (s % 60) + "s"
  }

  // --- status -------------------------------------------------------------

  function refresh() {
    if (statusProc.running) return
    statusProc.command = ["bash", pluginDir + "/status.sh", pendingProfile]
    statusProc.running = true
  }

  property string pendingProfile: ""

  function applyStatus(json) {
    var s
    try { s = JSON.parse(json) } catch (e) { return }
    var wasConnected = connected
    var wasInstalled = helperInstalled
    helperInstalled = !!s.helperInstalled
    activeProfile = s.activeProfile || ""
    activeState = s.activeState || ""
    focusProfile = s.focus || ""
    focusState = s.focusState || ""
    externalPid = s.externalPid || 0
    since = s.since || 0
    iface = s.iface || ""
    ipAddress = s.ip || ""
    peer = s.peer || ""
    routes = s.routes || 0
    unitError = s.error || ""
    certDigest = s.cert || ""

    if (pending === "up" && ((connected && activeState === "active") || focusState === "failed")) pending = ""
    if (pending === "down" && !connected && activeProfile === "" && externalPid === 0) pending = ""
    if (pending === "") pendingProfile = ""

    if (loaded && wasConnected !== connected) {
      if (connected) notify("VPN conectada", (activeProfile || "manual") + " · " + ipAddress)
      else notify("VPN desconectada", focusState === "failed" ? unitError : "")
    }
    if (helperInstalled && (!wasInstalled || !profilesLoaded)) refreshProfiles()
    loaded = true
  }

  function notify(title, body) {
    Quickshell.execDetached(["omarchy-notification-send", "-g", vpnGlyph, title, body || ""])
  }

  // --- profiles -----------------------------------------------------------

  function refreshProfiles() {
    if (!helperInstalled || profilesProc.running) return
    profilesProc.command = ["pkexec", helper, "list"]
    profilesProc.running = true
  }

  function applyProfiles(json) {
    var list
    try { list = JSON.parse(json) } catch (e) { return }
    if (!Array.isArray(list)) return
    list.sort(function(a, b) { return a.name.localeCompare(b.name) })
    profiles = list
    profilesLoaded = true
  }

  function findProfile(name) {
    for (var i = 0; i < profiles.length; i++) if (profiles[i].name === name) return profiles[i]
    return null
  }

  function startEdit(name) {
    var p = name ? findProfile(name) : null
    editOriginal = p ? p.name : ""
    editHasPassword = p ? !!p.hasPassword : false
    formError = ""
    nameField.text = p ? p.name : ""
    hostField.text = p ? p.host : ""
    portField.text = p ? (p.port || "443") : "443"
    userField.text = p ? p.username : ""
    passField.text = ""
    realmField.text = p ? p.realm : ""
    certField.text = p ? (p.trustedCerts || []).join(", ") : ""
    dnsToggle.checked = p ? p.setDns !== false : true
    routesToggle.checked = p ? p.setRoutes !== false : true
    acceptRemoteToggle.checked = p ? !!p.acceptRemote : true
    view = "edit"
    Qt.callLater(function() { (p ? hostField : nameField).forceActiveFocus() })
  }

  function saveProfile() {
    var name = nameField.text.trim()
    if (!/^[A-Za-z0-9_-]{1,32}$/.test(name)) { formError = "Nome: use letras, números, - ou _ (até 32)."; return }
    if (hostField.text.trim() === "") { formError = "Informe o host do gateway."; return }
    if (userField.text.trim() === "") { formError = "Informe o usuário."; return }
    if (!editHasPassword && passField.text === "") { formError = "Informe a senha."; return }
    formError = ""
    saveProc.payload = JSON.stringify({
      host: hostField.text.trim(),
      port: portField.text.trim(),
      username: userField.text.trim(),
      password: passField.text,
      realm: realmField.text.trim(),
      trustedCerts: certField.text.trim(),
      setDns: dnsToggle.checked,
      setRoutes: routesToggle.checked,
      acceptRemote: acceptRemoteToggle.checked
    })
    saveProc.command = ["pkexec", helper, "save", name].concat(editOriginal !== "" && editOriginal !== name ? [editOriginal] : [])
    saveProc.running = true
  }

  function deleteProfile(name) {
    if (confirmDelete !== name) { confirmDelete = name; return }
    confirmDelete = ""
    runAction(["pkexec", helper, "delete", name], true)
  }

  function trustCertificate() {
    if (certDigest === "" || focusProfile === "") return
    var name = focusProfile
    actionError = ""
    trustProc.profile = name
    trustProc.command = ["pkexec", helper, "trust", name, certDigest]
    trustProc.running = true
  }

  // --- connect / disconnect ----------------------------------------------

  function connectProfile(name) {
    if (!name) return
    actionError = ""
    pending = "up"
    pendingProfile = name
    runAction(["bash", pluginDir + "/vpnctl.sh", "connect", name], false)
  }

  function disconnect() {
    actionError = ""
    pending = "down"
    if (external) runAction(["bash", pluginDir + "/vpnctl.sh", "kill", String(externalPid)], false)
    else runAction(["bash", pluginDir + "/vpnctl.sh", "disconnect"], false)
  }

  function toggleVpn() {
    if (actionProc.running) return
    if (switchOn) disconnect()
    else if (currentProfile !== "") connectProfile(currentProfile)
    else { open(); view = "list" }
  }

  function activateProfile(name) {
    if (actionProc.running) return
    var state = profileState(name)
    if (state === "connected" || state === "activating") disconnect()
    else connectProfile(name)
  }

  function runAction(cmd, reloadProfiles) {
    if (actionProc.running) return
    actionProc.reloadProfiles = reloadProfiles
    actionProc.command = cmd
    actionProc.running = true
    pollTimer.restart()
  }

  function back() {
    confirmDelete = ""
    if (view === "edit") view = "list"
    else if (view === "list") view = "main"
    else close()
    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  Component.onCompleted: refresh()
  onOpenedChanged: if (opened) {
    view = "main"
    confirmDelete = ""
    now = Date.now() / 1000
    refresh()
    refreshProfiles()
    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }

  Process {
    id: statusProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.applyStatus(String(text || "").trim())
    }
  }

  Process {
    id: profilesProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.applyProfiles(String(text || "").trim())
    }
  }

  Process {
    id: saveProc
    property string payload: ""
    stdinEnabled: true
    onStarted: {
      write(payload + "\n")
      payload = ""
    }
    stderr: StdioCollector { id: saveErr; waitForEnd: true }
    onExited: function(exitCode) {
      if (exitCode !== 0) {
        root.formError = String(saveErr.text || "").trim() || "Não foi possível salvar (código " + exitCode + ")"
        return
      }
      root.view = "list"
      root.refreshProfiles()
      Qt.callLater(function() { keyCatcher.forceActiveFocus() })
    }
  }

  Process {
    id: trustProc
    property string profile: ""
    stderr: StdioCollector { id: trustErr; waitForEnd: true }
    onExited: function(exitCode) {
      if (exitCode !== 0) root.actionError = String(trustErr.text || "").trim() || "Não foi possível salvar o certificado"
      else root.connectProfile(profile)
      root.refreshProfiles()
    }
  }

  Process {
    id: actionProc
    property bool reloadProfiles: false
    stderr: StdioCollector { id: actionErr; waitForEnd: true }
    onExited: function(exitCode) {
      if (exitCode !== 0) {
        root.pending = ""
        root.actionError = String(actionErr.text || "").trim() || "Comando falhou (código " + exitCode + ")"
      }
      if (reloadProfiles) root.refreshProfiles()
      root.refresh()
    }
  }

  Timer {
    id: pollTimer
    interval: root.transitioning ? 1000 : root.refreshIntervalSec * 1000
    running: true
    repeat: true
    onTriggered: root.refresh()
  }

  // A start that never reaches "active" (e.g. a gateway that stops
  // answering) must not leave the switch spinning forever.
  Timer {
    interval: 60000
    running: root.pending !== ""
    onTriggered: root.pending = ""
  }

  Timer {
    interval: 1000
    running: root.opened && root.connected
    repeat: true
    onTriggered: root.now = Date.now() / 1000
  }

  IpcHandler {
    target: root.ipcTarget
    function open(): void { root.open() }
    function close(): void { root.close() }
    function show(): void { root.open() }
    function hide(): void { root.close() }
    function toggle(): void { root.toggle() }
    function settings(): void { root.open(); Qt.callLater(function() { root.view = "list" }) }
    function newProfile(): void { root.open(); Qt.callLater(function() { root.startEdit("") }) }
    function connect(name: string): string { root.connectProfile(name || root.currentProfile); return "ok" }
    function disconnect(): string { root.disconnect(); return "ok" }
    function toggleVpn(): string { root.toggleVpn(); return "ok" }
    function status(): string { return root.statusText }
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.connected ? root.lockGlyph : root.vpnGlyph
    foreground: root.connected ? root.barForeground : Qt.darker(root.barForeground, 1.55)
    opacity: root.transitioning ? 0.6 : 1.0
    tooltipText: "FortiVPN: " + root.statusText

    onPressed: function(buttonCode) {
      if (buttonCode === Qt.RightButton) root.toggleVpn()
      else root.toggle()
    }
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(360))
    contentHeight: panel.fittedContentHeight(column.implicitHeight, Style.space(620))

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onCloseRequested: root.back()
      onTabRequested: function(direction) { if (root.view === "main") root.switchPanel(direction) }
      onTextKey: function(t) {
        if (root.view !== "main") return
        if (t === "c" || t === "C" || t === " ") root.toggleVpn()
        else if (t === "r" || t === "R") { root.refresh(); root.refreshProfiles() }
        else if (t === "s" || t === "S") root.view = "list"
      }

      Column {
        id: column
        width: parent.width
        spacing: Style.space(12)

        // ================= main =================

        RowLayout {
          visible: root.view === "main"
          width: parent.width
          spacing: Style.space(8)

          PanelHero {
            Layout.fillWidth: true
            title: "FortiVPN"
            meta: root.statusText
            foreground: root.foreground
            fontFamily: root.fontFamily
            iconOpacity: root.connected ? 1.0 : 0.5
            iconComponent: Component {
              Text {
                text: root.connected ? root.lockGlyph : root.vpnGlyph
                color: root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.display
              }
            }
          }

          PanelActionButton {
            iconText: root.cogGlyph
            tooltipText: "Configurar conexões"
            foreground: root.foreground
            fontFamily: root.fontFamily
            Layout.alignment: Qt.AlignVCenter
            onClicked: { root.confirmDelete = ""; root.view = "list" }
          }

          ToggleSwitch {
            id: powerSwitch
            visible: root.currentProfile !== "" || root.connected
            checked: root.switchOn
            busy: root.transitioning || actionProc.running
            foreground: root.foreground
            Layout.alignment: Qt.AlignVCenter
            onToggled: root.toggleVpn()

            PanelToolTip {
              visible: powerSwitch.containsMouse
              text: root.switchOn ? "Desconectar" : "Conectar a " + root.currentProfile
              fontFamily: root.fontFamily
            }
          }
        }

        SetupNotice { visible: root.view === "main" && !root.helperInstalled }

        Text {
          textFormat: Text.PlainText
          visible: root.view === "main" && root.errorText !== ""
          width: parent.width
          text: root.errorText
          color: root.urgent
          font.family: root.fontFamily
          font.pixelSize: Style.font.bodySmall
          wrapMode: Text.WordWrap
        }

        Column {
          visible: root.view === "main" && root.certDigest !== "" && root.focusState !== "active"
          width: parent.width
          spacing: Style.spacing.labelGap

          Text {
            textFormat: Text.PlainText
            width: parent.width
            text: "O certificado do gateway não é confiável:\n" + root.certDigest
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            wrapMode: Text.WrapAnywhere
          }

          Button {
            text: "Confiar neste certificado e conectar"
            foreground: root.foreground
            fontFamily: root.fontFamily
            bordered: true
            enabled: !trustProc.running
            onClicked: root.trustCertificate()
          }
        }

        Text {
          textFormat: Text.PlainText
          visible: root.view === "main" && root.external
          width: parent.width
          text: "Sessão iniciada fora do painel (PID " + root.externalPid + "). Desconectar pedirá sua senha."
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          wrapMode: Text.WordWrap
        }

        Column {
          visible: root.view === "main" && root.connected
          width: parent.width
          spacing: Style.spacing.labelGap

          PanelSeparator { foreground: root.foreground }
          InfoPair { label: "Interface"; value: root.iface }
          InfoPair { label: "Endereço"; value: root.ipAddress }
          InfoPair { label: "Peer"; value: root.peer }
          InfoPair { label: "Rotas"; value: String(root.routes) }
        }

        Column {
          visible: root.view === "main" && root.helperInstalled
          width: parent.width
          spacing: Style.space(6)

          PanelSeparator { foreground: root.foreground }

          PanelSectionHeader {
            text: "CONEXÕES"
            foreground: root.foreground
            fontFamily: root.fontFamily
          }

          Text {
            textFormat: Text.PlainText
            visible: root.profilesLoaded && root.profiles.length === 0
            width: parent.width
            text: "Nenhuma conexão cadastrada."
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
          }

          Button {
            visible: root.profilesLoaded && root.profiles.length === 0
            text: "Nova conexão"
            iconText: root.plusGlyph
            foreground: root.foreground
            fontFamily: root.fontFamily
            bordered: true
            onClicked: root.startEdit("")
          }

          Repeater {
            model: root.view === "main" ? root.profiles : []
            ProfileRow {
              required property var modelData
              width: parent.width
              profile: modelData
              onActivated: root.activateProfile(modelData.name)
            }
          }
        }

        // ================= list (gear) =================

        ViewHeader {
          visible: root.view === "list"
          title: "Conexões"
          actionGlyph: root.plusGlyph
          actionTooltip: "Nova conexão"
          actionVisible: root.helperInstalled
          onAction: root.startEdit("")
        }

        SetupNotice { visible: root.view === "list" && !root.helperInstalled }

        Text {
          textFormat: Text.PlainText
          visible: root.view === "list" && root.helperInstalled && root.profiles.length === 0
          width: parent.width
          text: "Nenhuma conexão ainda. Use + para criar a primeira."
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.body
          wrapMode: Text.WordWrap
        }

        Text {
          textFormat: Text.PlainText
          visible: root.view === "list" && root.actionError !== ""
          width: parent.width
          text: root.actionError
          color: root.urgent
          font.family: root.fontFamily
          font.pixelSize: Style.font.bodySmall
          wrapMode: Text.WordWrap
        }

        Column {
          visible: root.view === "list"
          width: parent.width
          spacing: Style.space(6)

          Repeater {
            model: root.view === "list" ? root.profiles : []
            ManageRow {
              required property var modelData
              width: parent.width
              profile: modelData
            }
          }
        }

        // ================= edit form =================

        ViewHeader {
          visible: root.view === "edit"
          title: root.editOriginal === "" ? "Nova conexão" : "Editar " + root.editOriginal
        }

        GridLayout {
          visible: root.view === "edit"
          width: parent.width
          columns: 2
          columnSpacing: Style.space(10)
          rowSpacing: Style.space(8)

          FieldLabel { text: "Nome" }
          FormField { id: nameField; placeholderText: "empresa"; KeyNavigation.tab: hostField }

          FieldLabel { text: "Gateway" }
          FormField { id: hostField; placeholderText: "vpn.empresa.com.br"; KeyNavigation.tab: portField }

          FieldLabel { text: "Porta" }
          FormField {
            id: portField
            placeholderText: "443"
            validator: IntValidator { bottom: 1; top: 65535 }
            KeyNavigation.tab: userField
          }

          FieldLabel { text: "Usuário" }
          FormField { id: userField; KeyNavigation.tab: passField }

          FieldLabel { text: "Senha" }
          FormField {
            id: passField
            password: true
            placeholderText: root.editHasPassword ? "manter a atual" : ""
            KeyNavigation.tab: realmField
          }

          FieldLabel { text: "Realm" }
          FormField { id: realmField; placeholderText: "opcional"; KeyNavigation.tab: certField }

          FieldLabel { text: "Certificado" }
          FormField { id: certField; placeholderText: "sha256 confiável (opcional)"; KeyNavigation.tab: nameField }
        }

        Toggle {
          id: dnsToggle
          visible: root.view === "edit"
          width: parent.width
          label: "Usar DNS da VPN"
          description: "Aplica os servidores DNS enviados pelo gateway."
          foreground: root.foreground
          fontFamily: root.fontFamily
          onClicked: checked = !checked
        }

        Toggle {
          id: routesToggle
          visible: root.view === "edit"
          width: parent.width
          label: "Configurar rotas"
          description: "Adiciona as rotas enviadas pelo gateway."
          foreground: root.foreground
          fontFamily: root.fontFamily
          onClicked: checked = !checked
        }

        Toggle {
          id: acceptRemoteToggle
          visible: root.view === "edit"
          width: parent.width
          label: "Aceitar IP remoto (pppd ≥ 2.5)"
          description: "Equivale a --pppd-accept-remote; necessário no pppd 2.5 ou mais novo."
          foreground: root.foreground
          fontFamily: root.fontFamily
          onClicked: checked = !checked
        }

        Text {
          textFormat: Text.PlainText
          visible: root.view === "edit" && root.formError !== ""
          width: parent.width
          text: root.formError
          color: root.urgent
          font.family: root.fontFamily
          font.pixelSize: Style.font.bodySmall
          wrapMode: Text.WordWrap
        }

        Row {
          visible: root.view === "edit"
          anchors.right: parent.right
          spacing: Style.space(8)

          Button {
            text: "Cancelar"
            foreground: root.foreground
            fontFamily: root.fontFamily
            onClicked: root.back()
          }

          Button {
            text: saveProc.running ? "Salvando…" : "Salvar"
            foreground: root.foreground
            fontFamily: root.fontFamily
            bordered: true
            enabled: !saveProc.running
            onClicked: root.saveProfile()
          }
        }
      }
    }
  }

  // ================= components =================

  component SetupNotice: Column {
    width: parent.width
    spacing: Style.spacing.labelGap

    Text {
      textFormat: Text.PlainText
      width: parent.width
      text: "Falta a configuração de sistema. Rode uma vez no terminal:"
      color: root.urgent
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      wrapMode: Text.WordWrap
    }

    Text {
      textFormat: Text.PlainText
      width: parent.width
      text: "sudo " + root.pluginDir + "/setup-system.sh"
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      wrapMode: Text.WrapAnywhere
    }
  }

  component ViewHeader: RowLayout {
    id: viewHeader
    property string title: ""
    property string actionGlyph: ""
    property string actionTooltip: ""
    property bool actionVisible: true
    signal action()

    width: parent.width
    spacing: Style.space(8)

    PanelActionButton {
      iconText: root.backGlyph
      tooltipText: "Voltar"
      foreground: root.foreground
      fontFamily: root.fontFamily
      onClicked: root.back()
    }

    Text {
      textFormat: Text.PlainText
      Layout.fillWidth: true
      text: viewHeader.title
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.title
      font.bold: true
      elide: Text.ElideRight
    }

    PanelActionButton {
      visible: viewHeader.actionGlyph !== "" && viewHeader.actionVisible
      iconText: viewHeader.actionGlyph
      tooltipText: viewHeader.actionTooltip
      foreground: root.foreground
      fontFamily: root.fontFamily
      onClicked: viewHeader.action()
    }
  }

  component ProfileRow: CursorSurface {
    id: profileRow
    property var profile: null
    readonly property string state: profile ? root.profileState(profile.name) : ""
    signal activated()

    hasCursor: rowMouse.containsMouse
    current: state === "connected"
    foreground: root.foreground
    implicitHeight: profileContent.implicitHeight + Style.spacing.rowPaddingX

    MouseArea {
      id: rowMouse
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: profileRow.activated()
    }

    RowLayout {
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      anchors.leftMargin: Style.space(10)
      anchors.rightMargin: Style.space(10)
      spacing: Style.space(8)

      Text {
        text: profileRow.state === "connected" ? root.lockGlyph : root.vpnGlyph
        color: profileRow.state === "failed" ? root.urgent : root.foreground
        opacity: profileRow.state === "" ? 0.5 : 1.0
        font.family: root.fontFamily
        font.pixelSize: Style.font.icon
      }

      ColumnLayout {
        id: profileContent
        Layout.fillWidth: true
        spacing: Style.space(1)

        Text {
          textFormat: Text.PlainText
          Layout.fillWidth: true
          text: profileRow.profile ? profileRow.profile.name : ""
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.body
          elide: Text.ElideRight
        }

        Text {
          textFormat: Text.PlainText
          Layout.fillWidth: true
          text: profileRow.profile ? profileRow.profile.username + " @ " + profileRow.profile.host + ":" + (profileRow.profile.port || "443") : ""
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          elide: Text.ElideRight
        }
      }

      Text {
        textFormat: Text.PlainText
        text: {
          if (profileRow.state === "connected") return "Conectado"
          if (profileRow.state === "activating") return "Conectando…"
          if (profileRow.state === "deactivating") return "Saindo…"
          if (profileRow.state === "failed") return "Falhou"
          return rowMouse.containsMouse ? "Conectar" : ""
        }
        color: profileRow.state === "failed" ? root.urgent : root.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
      }
    }
  }

  component ManageRow: CursorSurface {
    id: manageRow
    property var profile: null
    readonly property string name: profile ? profile.name : ""
    readonly property bool confirming: root.confirmDelete === name

    hasCursor: manageMouse.containsMouse
    foreground: root.foreground
    implicitHeight: manageContent.implicitHeight + Style.spacing.rowPaddingX

    MouseArea {
      id: manageMouse
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: root.startEdit(manageRow.name)
    }

    RowLayout {
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      anchors.leftMargin: Style.space(10)
      anchors.rightMargin: Style.space(6)
      spacing: Style.space(6)

      ColumnLayout {
        id: manageContent
        Layout.fillWidth: true
        spacing: Style.space(1)

        Text {
          textFormat: Text.PlainText
          Layout.fillWidth: true
          text: manageRow.name
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.body
          elide: Text.ElideRight
        }

        Text {
          textFormat: Text.PlainText
          Layout.fillWidth: true
          text: manageRow.confirming ? "Clique na lixeira de novo para apagar"
            : (manageRow.profile ? manageRow.profile.host + ":" + (manageRow.profile.port || "443") : "")
          color: manageRow.confirming ? root.urgent : root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          elide: Text.ElideRight
        }
      }

      PanelActionButton {
        iconText: root.pencilGlyph
        tooltipText: "Editar"
        foreground: root.foreground
        fontFamily: root.fontFamily
        onClicked: root.startEdit(manageRow.name)
      }

      PanelActionButton {
        iconText: root.trashGlyph
        tooltipText: manageRow.confirming ? "Confirmar exclusão" : "Apagar"
        foreground: manageRow.confirming ? root.urgent : root.foreground
        hoverColor: root.urgent
        fontFamily: root.fontFamily
        enabled: !actionProc.running
        onClicked: root.deleteProfile(manageRow.name)
      }
    }
  }

  component FieldLabel: Text {
    textFormat: Text.PlainText
    color: root.foreground
    opacity: 0.6
    font.family: root.fontFamily
    font.pixelSize: Style.font.bodySmall
    Layout.alignment: Qt.AlignVCenter
  }

  component FormField: TextField {
    Layout.fillWidth: true
    font.family: root.fontFamily
    font.pixelSize: Style.font.body
    foreground: root.foreground
    horizontalPadding: Style.spacing.controlGap
    verticalPadding: Style.spacing.controlPaddingY
    onAccepted: root.saveProfile()
    Keys.onEscapePressed: root.back()
  }

  component InfoPair: RowLayout {
    property string label: ""
    property string value: ""

    width: parent.width
    spacing: Style.space(8)

    Text {
      textFormat: Text.PlainText
      text: parent.label
      color: root.foreground
      opacity: 0.6
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
    }
    Item { Layout.fillWidth: true }
    Text {
      textFormat: Text.PlainText
      text: parent.value
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      elide: Text.ElideRight
      horizontalAlignment: Text.AlignRight
      Layout.maximumWidth: parent.width * 0.7
    }
  }
}
