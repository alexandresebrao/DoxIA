import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

BarIndicator {
  id: root

  // The state comes from the file the notification server keeps it in, whichever
  // server that is: Quickshell's own service or doxia-shell, which takes the
  // notifications over and turns that service off (fedora/doxia/rs/shell).
  property bool dnd: false

  FileView {
    path: Quickshell.env("HOME") + "/.local/state/omarchy/notifications.json"
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: {
      try {
        root.dnd = JSON.parse(text()).dnd === true
      } catch (e) {
        root.dnd = false
      }
    }
    onLoadFailed: root.dnd = false
  }

  // Sempre visível para o estado ficar claro: permitindo = sino branco sem
  // traço; bloqueado = sino cortado em cinza.
  active: true
  activeText: dnd ? "󰂛" : "󰂚"
  activeTooltipText: dnd ? "Permitir notificações" : "Silenciar notificações"
  foreground: dnd ? Qt.darker(bar ? bar.barForeground : Color.foreground, 1.7)
                  : (bar ? bar.barForeground : Color.foreground)

  // omarchy-shell reaches whichever server is running.
  onPressed: function() {
    Quickshell.execDetached(["omarchy-toggle-notification-silencing"])
  }
}
