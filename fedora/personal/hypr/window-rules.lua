-- Google Chat (web app) sempre sozinho no workspace 3, ocupando a tela toda.
o.window("^chrome-chat\\.google\\.com__-Default$", { workspace = "3" })

-- Esconde a ponte de vídeo Xwayland (janela preta) num workspace especial, sem foco.
o.window("xwaylandvideobridge", {
  workspace = "special:xwaylandvideobridge silent",
  no_initial_focus = true,
  no_focus = true,
  no_anim = true,
  opacity = "0.0 override",
})

-- Janela de log do painel de serviços (plugin alexandre.services): flutuante e larga.
o.window("org.omarchy.services-log", { float = true })
o.window("org.omarchy.services-log", { center = true })
o.window("org.omarchy.services-log", { size = { 1100, 620 } })
