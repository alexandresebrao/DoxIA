-- Esconde a ponte de vídeo Xwayland (janela preta) num workspace especial, sem foco.
o.window("xwaylandvideobridge", {
  workspace = "special:xwaylandvideobridge silent",
  no_initial_focus = true,
  no_focus = true,
  no_anim = true,
  opacity = "0.0 override",
})
