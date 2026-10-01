import QtQuick
import qs.Commons
import qs.Ui

BarIndicator {
  id: root

  readonly property var notificationService: bar?.shell?.firstPartyServiceFor("omarchy.notifications")
  readonly property bool dnd: notificationService ? notificationService.doNotDisturb : false

  // Sempre visível para o estado ficar claro: permitindo = sino branco sem
  // traço; bloqueado = sino cortado em cinza.
  active: true
  activeText: dnd ? "󰂛" : "󰂚"
  activeTooltipText: dnd ? "Permitir notificações" : "Silenciar notificações"
  foreground: dnd ? Qt.darker(bar ? bar.barForeground : Color.foreground, 1.7)
                  : (bar ? bar.barForeground : Color.foreground)

  onPressed: function() {
    if (root.notificationService) {
      root.notificationService.setDoNotDisturb(!root.notificationService.doNotDisturb)
    }
  }
}
