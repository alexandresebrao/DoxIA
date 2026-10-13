import QtQuick
import Quickshell
import Quickshell.Io
import qs.Ui
import qs.Commons
import "KeyboardLayoutModel.js" as KeyboardLayoutModel

BarWidget {
  id: root
  moduleName: "omarchy.keyboard-layout"

  // mango keeps one keymap for the whole seat and streams its description
  // ("Portuguese (Brazil)") whenever it changes, so there is no per-device
  // reading to reconcile.
  readonly property string layoutFull: Compositor.keyboardLayout
  // Nothing to read or switch on the single-layout install most people run, so
  // the widget ships on the bar and stays out of the way until there are two.
  property bool multipleLayouts: false
  // Short language code per layout description ("English (US)": "en"), read from
  // xkb's own table rather than maintained by hand.
  property var layoutBriefs: ({})
  readonly property string layoutLabel: KeyboardLayoutModel.shortLabel(layoutFull, layoutBriefs)

  function cycleLayout() {
    Compositor.dispatch("switch_keyboard_layout")
  }

  Component.onCompleted: {
    briefsProc.running = true
    layoutsProc.running = true
  }

  // A reload that adds a layout raises no event of its own, but switching to
  // it does, so re-read the list whenever the active layout moves.
  onLayoutFullChanged: if (!layoutsProc.running) layoutsProc.running = true

  Process {
    id: layoutsProc
    command: ["omarchy-mango-option", "xkb_rules_layout"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.multipleLayouts = String(text || "").indexOf(",") !== -1
    }
  }

  // The table only changes when xkb data is upgraded, so read it at startup and
  // leave it alone. The bar is built per monitor, so this runs once per widget.
  // The exotic rulesets cover layouts like trans (IPA) that ship in the same xkb
  // package and set just as well, so load them or those labels lose their code.
  Process {
    id: briefsProc
    command: ["xkbcli", "list", "--load-exotic"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.layoutBriefs = KeyboardLayoutModel.layoutBriefs(text)
    }
  }

  visible: layoutLabel !== "" && multipleLayouts
  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.layoutLabel
    fontSize: Style.font.caption
    horizontalMargin: 6
    tooltipText: root.layoutFull
    onPressed: function() { root.cycleLayout() }
  }
}
