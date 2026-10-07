import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import QtQuick
import QtQuick.Effects
import qs.Commons
import qs.Ui
import "IniciarModel.js" as Model

// Menu Iniciar à moda do Windows 98, nas cores do tema: faixa vertical com o
// nome, bordas em relevo e submenus em cascata que abrem ao passar o mouse.
// Digitar em qualquer ponto vira "Localizar"; "Executar..." roda um comando.
// Abre com `omarchy-shell shell toggle doxia.iniciar` (emblema da barra e
// SUPER+ALT+SPACE).
Item {
  id: root

  property string omarchyPath: Quickshell.env("OMARCHY_PATH")
  property var shell: null
  property var manifest: null

  readonly property var appLibrary: root.shell ? root.shell.appLibrary : null
  readonly property string pluginId: (root.manifest && root.manifest.id) || "doxia.iniciar"

  property bool opened: false
  // Uma coluna por nível de cascata: { items, parentIndex, mode, title }.
  property var levels: []
  // Item destacado em cada nível (-1 = nenhum).
  property var selections: []
  property int focusLevel: 0
  // Texto digitado em Localizar / Executar.
  property string typed: ""
  property string recentXml: ""
  // Ids escondidos do lançador (launcher.hides), só para o caminho sem a
  // biblioteca de apps do shell, que já os filtra.
  property var hiddenIds: ({})
  // Ícones monocromáticos das pastas e ações: nome -> arquivo *-symbolic.svg
  // (Papirus antes do Adwaita). Os apps mantêm as cores.
  property var symbolicIndex: ({})
  property var pendingSymbolic: ({})
  readonly property int maxLevels: 5

  // Paleta: os tokens [menu] do tema (fundo, texto, seleção e a borda do
  // Hyprland), como o menu Omarchy; o relevo do 98 vem por dentro da borda.
  readonly property color face: Color.menu.background
  readonly property color text: Color.menu.text
  readonly property color dimText: Qt.rgba(text.r, text.g, text.b, 0.55)
  readonly property color selectedBackground: Color.menu.selectedBackground
  readonly property color selectedText: Color.menu.selectedText
  readonly property color accentDark: Qt.darker(Color.accent, 1.45)
  readonly property var borderSpec: Border.surfaceSpec("menu", "border", Color.menu.border, 1)
  readonly property color bevelLight: Qt.lighter(face, 2.2)
  readonly property color bevelMid: Qt.lighter(face, 1.6)
  readonly property color bevelDark: "#000000"
  readonly property string fontFamily: Style.font.menuFamily

  readonly property int bevel: 3
  readonly property int pad: 3
  readonly property int bannerWidth: Style.space(30)
  readonly property int rootRowHeight: Style.space(38)
  readonly property int rowHeight: Style.space(26)
  readonly property int separatorHeight: Style.space(9)
  readonly property int rootWidth: Style.space(250)
  readonly property int subWidth: Style.space(260)
  readonly property int gap: Style.gapsOut + Style.space(2)

  function open(payloadJson) {
    if (root.appLibrary) root.appLibrary.refreshIcons()
    if (!symbolicScan.running && Object.keys(root.symbolicIndex).length === 0) symbolicScan.running = true
    recentFile.reload()
    root.typed = ""
    root.levels = [{ items: root.rootItems(), parentIndex: -1, mode: "menu", title: "" }]
    root.selections = [-1]
    root.focusLevel = 0
    root.opened = true
    // {"path": [2, 0]} já abre a cascata nesses itens (ex.: Programas > 1ª pasta).
    var payload = ({})
    try { payload = JSON.parse(payloadJson || "{}") } catch (e) {}
    var path = Array.isArray(payload.path) ? payload.path : []
    for (var i = 0; i < path.length && i < root.maxLevels - 1; i++) {
      root.setSelection(i, path[i])
      if (!root.expand(i, path[i], true)) break
    }
    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }

  function close() {
    root.opened = false
    hoverTimer.stop()
  }

  function dismiss() {
    root.close()
    if (root.shell && typeof root.shell.hide === "function") root.shell.hide(root.pluginId)
  }

  function toggle() {
    if (root.opened) root.dismiss()
    else root.open("{}")
  }

  // --- Ações -----------------------------------------------------------------

  function runCommand(command) {
    Quickshell.execDetached(["bash", "-lc", command])
  }

  // Sem a biblioteca de apps do shell, lança como ela: gtk-launch num scope
  // do uwsm (mantém o sufixo .desktop para ids como org.telegram.desktop).
  function launchApp(entry) {
    if (root.appLibrary) root.appLibrary.launch(entry.id, root.appLibrary.entryName(entry))
    else Quickshell.execDetached(["uwsm-app", "--", "gtk-launch", String(entry.id) + ".desktop"])
  }

  function symbolicFor(name, fallback) {
    var value = String(name || "")
    if (value.length === 0 || value.indexOf("/") !== -1) value = ""
    var hit = value ? (root.symbolicIndex[value.replace(/-symbolic$/, "") + "-symbolic"] || "") : ""
    if (!hit && fallback) hit = root.symbolicIndex[fallback + "-symbolic"] || ""
    return hit ? Util.fileUrl(hit) : ""
  }

  function iconFor(name) {
    if (root.appLibrary) return root.appLibrary.iconSource(name)
    return Quickshell.iconPath(String(name || "application-x-executable"), true)
  }

  function appItem(entry) {
    var name = String(entry.name || entry.id)
    return { label: name, icon: entry.icon, colored: true, run: function() { root.launchApp(entry) } }
  }

  function commandItem(label, icon, command) {
    return { label: label, icon: icon, run: function() { root.runCommand(command) } }
  }

  // --- Conteúdo dos menus ----------------------------------------------------

  // Com o emblema do tema (o ∞ do DoxIA) tingido como os ícones simbólicos.
  function omarchyMenuItem() {
    var item = commandItem("DoxIA Express", Color.currentThemePath + "/emblem.png", "omarchy-shell shell summon omarchy.menu '{\"menu\":\"root\"}'")
    item.tint = true
    return item
  }

  function rootItems() {
    return [
      root.omarchyMenuItem(),
      { sep: true },
      { label: "Programas", icon: "applications-other", sub: root.programItems },
      { label: "Documentos", icon: "folder-documents", sub: root.documentItems },
      { label: "Configurações", icon: "preferences-system", sub: root.settingsItems },
      { label: "Localizar", icon: "system-search", mode: "search" },
      commandItem("Ajuda", "help-browser", "omarchy-menu-keybindings"),
      { label: "Executar...", icon: "system-run", mode: "run" },
      { sep: true },
      { label: "Desligar...", icon: "system-shutdown", sub: root.powerItems }
    ]
  }

  // sortedEntries devolve linhas { entry, score }. A biblioteca do shell nem
  // sempre chega a plugins de terceiros; aí lê o DesktopEntries direto.
  function findApps(query) {
    if (root.appLibrary)
      return root.appLibrary.sortedEntries(query).map(function(row) { return row.entry })
    return Model.searchEntries(DesktopEntries.applications.values || [], query, root.hiddenIds)
  }

  function allApps() {
    return root.findApps("")
  }

  function programItems() {
    var groups = Model.groupByCategory(root.allApps())
    return groups.map(function(group) {
      return {
        label: group.label,
        icon: group.icon,
        sub: function() { return group.apps.map(root.appItem) }
      }
    })
  }

  function documentItems() {
    var items = [commandItem("Meus documentos", "folder-documents", "xdg-open \"$(xdg-user-dir DOCUMENTS)\""), { sep: true }]
    var recent = Model.recentDocuments(root.recentXml, 15)
    if (recent.length === 0) items.push({ label: "(Vazio)", disabled: true })
    for (var i = 0; i < recent.length; i++) {
      (function(doc) {
        items.push({ label: doc.name, icon: doc.icon, iconFallback: "text-x-generic", run: function() { Quickshell.execDetached(["xdg-open", doc.href]) } })
      })(recent[i])
    }
    return items
  }

  function settingsItems() {
    var items = [
      root.omarchyMenuItem(),
      commandItem("Tema", "preferences-desktop-appearance", "omarchy-menu toggle theme"),
      commandItem("Papel de parede", "preferences-desktop-wallpaper", "omarchy-menu toggle background"),
      commandItem("Atalhos de teclado", "preferences-desktop-keyboard-shortcuts", "omarchy-launch-config-editor \"$HOME/.config/hypr/bindings.lua\"")
    ]
    var settingsApps = root.allApps().filter(function(entry) {
      return Model.listOf(entry.categories).indexOf("Settings") !== -1
    })
    if (settingsApps.length > 0) items.push({ sep: true })
    return items.concat(settingsApps.map(root.appItem))
  }

  function powerItems() {
    return [
      commandItem("Bloquear", "system-lock-screen", "omarchy-system-lock"),
      commandItem("Suspender", "system-suspend", "systemctl suspend"),
      commandItem("Sair da sessão", "system-log-out", "omarchy-system-logout"),
      { sep: true },
      commandItem("Reiniciar", "system-reboot", "omarchy-system-reboot"),
      commandItem("Desligar", "system-shutdown", "omarchy-system-shutdown")
    ]
  }

  function searchItems(query) {
    var entries = root.findApps(query)
    var items = entries.slice(0, 14).map(root.appItem)
    if (items.length === 0) items.push({ label: "Nada encontrado", disabled: true })
    return items
  }

  // --- Navegação -------------------------------------------------------------

  function itemAt(level, index) {
    var l = root.levels[level]
    return l && index >= 0 && index < l.items.length ? l.items[index] : null
  }

  function selectable(item) {
    return item && !item.sep && !item.disabled
  }

  function setSelection(level, index) {
    var next = root.selections.slice(0, level + 1)
    while (next.length <= level) next.push(-1)
    next[level] = index
    // Ao entrar num submenu pelo mouse, o item pai continua destacado.
    for (var l = level; l > 0; l--) next[l - 1] = root.levels[l].parentIndex
    root.selections = next
  }

  function truncate(level) {
    if (root.levels.length > level + 1) {
      root.levels = root.levels.slice(0, level + 1)
      root.selections = root.selections.slice(0, level + 1)
    }
    if (root.focusLevel > level) root.focusLevel = level
    if (level === 0) root.typed = ""
  }

  function pushLevel(parentLevel, parentIndex, level) {
    root.truncate(parentLevel)
    level.parentIndex = parentIndex
    root.levels = root.levels.concat([level])
    root.selections = root.selections.concat([-1])
  }

  // Abre o submenu (ou Localizar/Executar) do item; com keyboard, já
  // destaca o primeiro item dele.
  function expand(level, index, keyboard) {
    var item = root.itemAt(level, index)
    if (!root.selectable(item)) return false
    if (item.sub) {
      var items = item.sub()
      if (items.length === 0) items = [{ label: "(Vazio)", disabled: true }]
      root.pushLevel(level, index, { items: items, mode: "menu", title: "" })
    } else if (item.mode === "search") {
      root.typed = ""
      root.pushLevel(level, index, { items: root.searchItems(""), mode: "search", title: "Localizar" })
    } else if (item.mode === "run") {
      root.typed = ""
      root.pushLevel(level, index, { items: [], mode: "run", title: "Executar" })
    } else {
      root.truncate(level)
      return false
    }
    if (keyboard) {
      root.focusLevel = level + 1
      root.selectFirst(level + 1)
    }
    return true
  }

  function activate(level, index) {
    var item = root.itemAt(level, index)
    if (!root.selectable(item)) return
    if (item.sub || item.mode) {
      root.expand(level, index, true)
      return
    }
    if (item.run) {
      item.run()
      root.dismiss()
    }
  }

  function selectFirst(level) {
    var l = root.levels[level]
    if (!l) return
    for (var i = 0; i < l.items.length; i++) {
      if (root.selectable(l.items[i])) {
        root.setSelection(level, i)
        return
      }
    }
    root.setSelection(level, -1)
  }

  function move(delta) {
    var level = root.focusLevel
    var l = root.levels[level]
    if (!l || l.items.length === 0) return
    var index = root.selections[level]
    for (var step = 0; step < l.items.length; step++) {
      index = index < 0 ? (delta > 0 ? 0 : l.items.length - 1) : (index + delta + l.items.length) % l.items.length
      if (root.selectable(l.items[index])) break
    }
    root.setSelection(level, index)
    root.truncate(level)
  }

  function setTyped(value) {
    root.typed = value
    var deepest = root.levels.length - 1
    root.focusLevel = deepest
    var l = root.levels[deepest]
    if (l && l.mode === "search") {
      var next = root.levels.slice()
      next[deepest] = { items: root.searchItems(value), parentIndex: l.parentIndex, mode: "search", title: l.title }
      root.levels = next
      root.selectFirst(deepest)
    }
  }

  // Digitar fora de Executar abre Localizar já com o texto.
  function startSearch(textValue) {
    var searchIndex = -1
    var items = root.levels[0].items
    for (var i = 0; i < items.length; i++) if (items[i].mode === "search") searchIndex = i
    root.setSelection(0, searchIndex)
    root.expand(0, searchIndex, true)
    root.setTyped(textValue)
  }

  function runTyped() {
    var command = root.typed.trim()
    if (!command) return
    Quickshell.execDetached(["uwsm-app", "--", "bash", "-lc", command])
    root.dismiss()
  }

  function deepestMode() {
    var l = root.levels[root.levels.length - 1]
    return l ? l.mode : "menu"
  }

  function handleKey(event) {
    var mode = root.deepestMode()
    var typing = mode === "search" || mode === "run"
    if (event.key === Qt.Key_Escape) {
      if (typing && root.typed) root.setTyped("")
      else if (root.levels.length > 1) {
        var parent = root.levels.length - 2
        root.truncate(parent)
        root.focusLevel = parent
      } else root.dismiss()
    } else if (typing && Util.editsFilter(event, root.typed)) {
      root.setTyped(Util.editedFilter(event, root.typed))
    } else if (event.key === Qt.Key_Up) {
      root.move(-1)
    } else if (event.key === Qt.Key_Down) {
      root.move(1)
    } else if (event.key === Qt.Key_Right) {
      var sel = root.selections[root.focusLevel]
      var item = root.itemAt(root.focusLevel, sel)
      if (item && (item.sub || item.mode)) root.expand(root.focusLevel, sel, true)
    } else if (event.key === Qt.Key_Left) {
      if (root.focusLevel > 0) {
        var up = root.focusLevel - 1
        root.truncate(up)
        root.focusLevel = up
      }
    } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
      if (mode === "run") root.runTyped()
      else root.activate(root.focusLevel, root.selections[root.focusLevel])
    } else if (event.text && event.text.length === 1 && event.text.charCodeAt(0) >= 32 && event.text.charCodeAt(0) !== 127
               && !(event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))) {
      if (typing) root.setTyped(root.typed + event.text)
      else root.startSearch(event.text)
    } else return
    event.accepted = true
  }

  // Altura de uma coluna e posição de cada linha (separadores são baixos).
  function rowHeightFor(level, item) {
    if (item && item.sep) return root.separatorHeight
    return level === 0 ? root.rootRowHeight : root.rowHeight
  }

  function rowY(level, index) {
    var l = root.levels[level]
    var y = 0
    for (var i = 0; l && i < index && i < l.items.length; i++) y += root.rowHeightFor(level, l.items[i])
    return y
  }

  function headerHeight(level) {
    var l = root.levels[level]
    return l && (l.mode === "search" || l.mode === "run") ? root.rowHeight + Style.space(8) : 0
  }

  function contentHeight(level) {
    var l = root.levels[level]
    if (!l) return 0
    var h = root.rowY(level, l.items.length) + root.headerHeight(level)
    if (l.mode === "run") h += root.rowHeight
    return h
  }

  // --- Dados -----------------------------------------------------------------

  FileView {
    id: recentFile
    path: Quickshell.env("HOME") + "/.local/share/recently-used.xbel"
    printErrors: false
    onLoaded: root.recentXml = text()
    onLoadFailed: root.recentXml = ""
  }

  Process {
    id: symbolicScan
    command: ["bash", "-c", "for d in \"$HOME/.local/share/icons\" /usr/share/icons/Papirus /usr/share/icons/Adwaita; do [[ -d $d ]] && find \"$d\" -path '*symbolic*' -name '*-symbolic.svg' 2>/dev/null; done"]
    stdout: SplitParser {
      onRead: function(line) {
        var file = line.slice(line.lastIndexOf("/") + 1).replace(/\.svg$/, "")
        if (file && root.pendingSymbolic[file] === undefined) root.pendingSymbolic[file] = line
      }
    }
    onStarted: root.pendingSymbolic = ({})
    onExited: root.symbolicIndex = root.pendingSymbolic
  }

  Component.onCompleted: symbolicScan.running = true

  FileView {
    path: root.omarchyPath + "/default/omarchy/launcher.hides"
    watchChanges: true
    printErrors: false
    onLoaded: root.hiddenIds = Model.hiddenIds(text())
    onFileChanged: reload()
  }

  Timer {
    id: hoverTimer
    interval: 280
    property int level: -1
    property int index: -1
    onTriggered: root.expand(level, index, false)
  }

  // --- Janela ----------------------------------------------------------------

  PanelWindow {
    id: panel
    visible: root.opened
    anchors { top: true; bottom: true; left: true; right: true }
    color: "transparent"
    WlrLayershell.namespace: "doxia-iniciar"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    // Respeita a área da barra: o menu nasce logo abaixo do emblema.
    exclusionMode: ExclusionMode.Normal

    MouseArea {
      anchors.fill: parent
      onClicked: root.dismiss()
    }

    Item {
      id: keyCatcher
      anchors.fill: parent
      focus: true
      Keys.onPressed: function(event) { root.handleKey(event) }
    }

    Repeater {
      model: root.maxLevels

      Item {
        id: column
        required property int index
        readonly property var level: root.levels[index] || null
        readonly property bool isRoot: index === 0
        readonly property int innerHeight: root.contentHeight(index)
        readonly property int fullHeight: Math.min(innerHeight + (root.bevel + root.pad) * 2, panel.height - root.gap * 2)
        readonly property Item previous: index > 0 ? columns.itemAt(index - 1) : null

        visible: root.opened && level !== null
        width: (isRoot ? root.rootWidth + root.bannerWidth : root.subWidth) + (root.bevel + root.pad) * 2
        height: fullHeight
        x: previous ? previous.x + previous.width - root.bevel * 2 : root.gap
        // Alinha a primeira linha do submenu com o item que o abriu.
        y: {
          if (!previous || !level) return root.gap
          var wanted = previous.y + root.bevel + root.pad + previous.listY + root.rowY(index - 1, level.parentIndex) - root.bevel - root.pad
          return Math.max(root.gap, Math.min(wanted, panel.height - root.gap - fullHeight))
        }
        readonly property int listY: (isRoot ? 0 : root.headerHeight(index)) - (list ? list.contentY : 0)

        Component.onCompleted: columns.register(index, column)

        MouseArea {
          anchors.fill: parent
          hoverEnabled: true
          onClicked: {}
        }

        // Borda do tema (a do Hyprland) e, por dentro, o relevo de 2px do 98:
        // claro em cima/esquerda, escuro embaixo/direita.
        BorderSurface {
          anchors.fill: parent
          radius: 0
          color: root.face
          borderSpec: root.borderSpec
        }
        Rectangle { x: 1; y: 1; width: parent.width - 2; height: parent.height - 2; color: root.bevelDark }
        Rectangle { x: 1; y: 1; width: parent.width - 3; height: parent.height - 3; color: root.bevelLight }
        Rectangle { x: 2; y: 2; width: parent.width - 4; height: parent.height - 4; color: Qt.darker(root.face, 1.5) }
        Rectangle { x: 2; y: 2; width: parent.width - 5; height: parent.height - 5; color: root.bevelMid }
        Rectangle { x: 3; y: 3; width: parent.width - 6; height: parent.height - 6; color: root.face }

        // Faixa vertical com o nome, como o "Windows98" do original.
        Rectangle {
          id: banner
          visible: column.isRoot
          x: root.bevel + root.pad
          y: root.bevel + root.pad
          width: root.bannerWidth
          height: column.height - (root.bevel + root.pad) * 2
          gradient: Gradient {
            GradientStop { position: 0.0; color: Qt.darker(root.face, 1.4) }
            GradientStop { position: 1.0; color: root.accentDark }
          }

          // Texto deitado, lido de baixo para cima.
          Item {
            width: banner.height
            height: banner.width
            anchors.centerIn: parent
            rotation: -90

            Row {
              anchors.left: parent.left
              anchors.leftMargin: Style.space(10)
              anchors.verticalCenter: parent.verticalCenter
              spacing: Style.space(2)

              Text {
                text: "DoxIA"
                color: "#ffffff"
                font.family: root.fontFamily
                font.pixelSize: Math.round(root.bannerWidth * 0.68)
                font.bold: true
              }
              Text {
                text: "98"
                color: Qt.lighter(Color.accent, 1.5)
                font.family: root.fontFamily
                font.pixelSize: Math.round(root.bannerWidth * 0.68)
              }
            }
          }
        }

        // Cabeçalho de Localizar/Executar: o texto digitado num campo afundado.
        Item {
          id: header
          visible: !column.isRoot && root.headerHeight(column.index) > 0
          x: root.bevel + root.pad
          y: root.bevel + root.pad
          width: column.width - (root.bevel + root.pad) * 2
          height: root.headerHeight(column.index)

          Text {
            id: headerLabel
            anchors.left: parent.left
            anchors.leftMargin: Style.space(6)
            anchors.verticalCenter: field.verticalCenter
            text: column.level && column.level.mode === "run" ? "Abrir:" : "Nome:"
            color: root.text
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
          }

          Rectangle {
            id: field
            anchors.left: headerLabel.right
            anchors.leftMargin: Style.space(6)
            anchors.right: parent.right
            anchors.rightMargin: Style.space(4)
            y: Style.space(3)
            height: root.rowHeight
            color: root.bevelLight
            Rectangle { x: 1; y: 1; width: parent.width - 1; height: parent.height - 1; color: Qt.darker(root.face, 2.2) }

            Text {
              anchors.left: parent.left
              anchors.leftMargin: Style.space(6)
              anchors.right: parent.right
              anchors.verticalCenter: parent.verticalCenter
              elide: Text.ElideLeft
              text: root.typed + (root.focusLevel === column.index ? "▏" : "")
              color: root.text
              font.family: root.fontFamily
              font.pixelSize: Style.font.body
            }
          }
        }

        Text {
          visible: column.level !== null && column.level.mode === "run"
          x: root.bevel + root.pad + Style.space(8)
          y: root.bevel + root.pad + root.headerHeight(column.index)
          height: root.rowHeight
          verticalAlignment: Text.AlignVCenter
          text: "Enter executa o comando"
          color: root.dimText
          font.family: root.fontFamily
          font.pixelSize: Style.font.bodySmall
        }

        ListView {
          id: list
          x: root.bevel + root.pad + (column.isRoot ? root.bannerWidth : 0)
          y: root.bevel + root.pad + root.headerHeight(column.index)
          width: column.width - (root.bevel + root.pad) * 2 - (column.isRoot ? root.bannerWidth : 0)
          height: column.height - (root.bevel + root.pad) * 2 - root.headerHeight(column.index)
          clip: true
          interactive: contentHeight > height
          boundsBehavior: Flickable.StopAtBounds
          model: column.level ? column.level.items : []
          currentIndex: root.selections[column.index] !== undefined ? root.selections[column.index] : -1
          highlightFollowsCurrentItem: false
          onCurrentIndexChanged: if (currentIndex >= 0) positionViewAtIndex(currentIndex, ListView.Contain)

          delegate: Item {
            id: row
            required property var modelData
            required property int index
            readonly property bool selected: root.selections[column.index] === index && root.selectable(modelData)
            width: list.width
            height: root.rowHeightFor(column.index, modelData)

            // Separador entalhado: linha escura sobre linha clara.
            Item {
              visible: row.modelData.sep === true
              anchors.fill: parent
              Rectangle { x: Style.space(4); y: parent.height / 2 - 1; width: parent.width - Style.space(8); height: 1; color: root.bevelDark }
              Rectangle { x: Style.space(4); y: parent.height / 2; width: parent.width - Style.space(8); height: 1; color: root.bevelMid }
            }

            Rectangle {
              visible: !row.modelData.sep
              anchors.fill: parent
              color: row.selected ? root.selectedBackground : "transparent"

              // Apps mantêm o ícone colorido. Pastas e ações: o simbólico tingido
              // com a cor do texto; sem ele, o colorido em cinza.
              Image {
                id: icon
                readonly property bool colored: row.modelData.colored === true
                readonly property string symbolic: colored ? "" : root.symbolicFor(row.modelData.icon, row.modelData.iconFallback)
                visible: colored && !!row.modelData.icon
                anchors.left: parent.left
                anchors.leftMargin: column.isRoot ? Style.space(8) : Style.space(5)
                anchors.verticalCenter: parent.verticalCenter
                width: column.isRoot ? Style.space(26) : Style.space(18)
                height: width
                sourceSize.width: width * 2
                sourceSize.height: height * 2
                source: !row.modelData.icon ? "" : (symbolic || root.iconFor(row.modelData.icon))
                asynchronous: true
                smooth: true
              }

              MultiEffect {
                anchors.fill: icon
                source: icon
                visible: !icon.colored && !!row.modelData.icon
                readonly property bool tinted: !!icon.symbolic || row.modelData.tint === true
                colorization: tinted ? 1.0 : 0.0
                colorizationColor: row.selected ? root.selectedText : root.text
                brightness: tinted ? 1.0 : 0.0
                saturation: tinted ? 0.0 : -1.0
                opacity: row.modelData.disabled ? 0.5 : 1.0
              }

              Text {
                anchors.left: parent.left
                anchors.leftMargin: (column.isRoot ? Style.space(8) + Style.space(26) : Style.space(5) + Style.space(18)) + Style.space(9)
                anchors.right: arrow.left
                anchors.rightMargin: Style.space(4)
                anchors.verticalCenter: parent.verticalCenter
                text: row.modelData.label || ""
                elide: Text.ElideRight
                color: row.modelData.disabled ? root.dimText : (row.selected ? root.selectedText : root.text)
                font.family: root.fontFamily
                font.pixelSize: column.isRoot ? Style.font.title : Style.font.body
                font.italic: row.modelData.disabled === true
              }

              Text {
                id: arrow
                anchors.right: parent.right
                anchors.rightMargin: Style.space(6)
                anchors.verticalCenter: parent.verticalCenter
                text: row.modelData.sub || row.modelData.mode === "search" ? "▶" : ""
                color: row.selected ? root.selectedText : root.text
                font.pixelSize: Math.round(Style.font.body * 0.7)
              }
            }

            MouseArea {
              anchors.fill: parent
              hoverEnabled: true
              enabled: root.selectable(row.modelData)
              onEntered: {
                root.focusLevel = column.index
                root.setSelection(column.index, row.index)
                hoverTimer.level = column.index
                hoverTimer.index = row.index
                hoverTimer.restart()
              }
              onClicked: {
                hoverTimer.stop()
                root.focusLevel = column.index
                root.setSelection(column.index, row.index)
                root.activate(column.index, row.index)
              }
            }
          }
        }
      }
    }

    // Repeater.itemAt não é reativo; guarda as colunas para o x/y em cascata.
    QtObject {
      id: columns
      property var items: []
      function register(index, item) {
        var next = items.slice()
        next[index] = item
        items = next
      }
      function itemAt(index) { return items[index] || null }
    }
  }
}
