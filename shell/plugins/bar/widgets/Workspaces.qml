import QtQuick
import QtQuick.Layouts
import Quickshell
import qs.Commons
import qs.Ui

BarWidget {
  id: root
  moduleName: "omarchy.workspaces"

  // mango tags are per output, so each bar shows its own monitor's.
  readonly property var window: root.QsWindow.window
  readonly property string monitorName: root.window && root.window.screen ? String(root.window.screen.name || "") : ""
  readonly property var tags: Compositor.tagsFor(root.monitorName.length > 0 ? root.monitorName : Compositor.focusedMonitor)

  function tagByIndex(index) {
    for (var i = 0; i < root.tags.length; i++) {
      if (root.tags[i].index === index) return root.tags[i]
    }

    return null
  }

  // 1..5 always, plus any later tag holding a window or in view.
  function workspaceIds() {
    var ids = [1, 2, 3, 4, 5]

    for (var i = 0; i < root.tags.length; i++) {
      var tag = root.tags[i]
      if (tag.index > 0 && tag.index <= 10 && (tag.client_count > 0 || tag.is_active) && ids.indexOf(tag.index) === -1) ids.push(tag.index)
    }

    ids.sort(function(left, right) { return left - right })
    return ids
  }

  function focusWorkspace(id) {
    if (root.monitorName) Compositor.dispatch("focusmon", root.monitorName)
    Compositor.dispatch("view", id)
  }

  readonly property real trailingGap: root.vertical ? 0 : Style.spaceReal(1.5)

  implicitWidth: grid.implicitWidth + trailingGap
  implicitHeight: grid.implicitHeight

  GridLayout {
    id: grid
    anchors.fill: parent
    anchors.rightMargin: root.trailingGap
    columns: root.vertical ? 1 : root.workspaceIds().length
    columnSpacing: root.vertical ? 0 : Style.space(1)
    rowSpacing: root.vertical ? Style.space(2) : 0

    Repeater {
      model: root.workspaceIds()

      WidgetButton {
        required property int modelData

        readonly property var tag: root.tagByIndex(modelData)
        readonly property bool occupied: tag !== null && tag.client_count > 0
        readonly property bool focused: tag !== null && tag.is_active

        bar: root.bar
        text: focused ? "\uDB85\uDCFB" : (modelData === 10 ? "0" : String(modelData))
        opacity: occupied || focused ? 1 : 0.5
        horizontalMargin: 6
        verticalPadding: 6
        fixedWidth: root.vertical ? root.barSize : Style.space(20)
        fixedHeight: root.barSize
        onPressed: function() { root.focusWorkspace(modelData) }
      }
    }
  }
}
