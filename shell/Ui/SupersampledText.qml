import QtQuick

// Draws a sibling Text (sourceItem) supersampled: the glyph is rendered at
// `factor`x and mipmapped down, so small icons on low-DPI screens get soft
// edges instead of stair steps. The capture extends `pad` px past the Text's
// box because icon glyphs often overhang their line box.
ShaderEffectSource {
  id: root

  property real pad: 4
  property real factor: 3

  visible: sourceItem ? sourceItem.visible : false
  x: sourceItem ? sourceItem.x - pad : 0
  y: sourceItem ? sourceItem.y - pad : 0
  width: sourceItem ? sourceItem.width + pad * 2 : 0
  height: sourceItem ? sourceItem.height + pad * 2 : 0
  rotation: sourceItem ? sourceItem.rotation : 0
  opacity: sourceItem ? sourceItem.opacity : 1
  sourceRect: Qt.rect(-pad, -pad, width, height)
  textureSize: Qt.size(Math.ceil(width * factor), Math.ceil(height * factor))
  hideSource: true
  smooth: true
  mipmap: true
}
