-- Esconde a barra flutuante "... está compartilhando sua tela" do Chrome; o
-- indicador fica na barra (plugin klab.screenshare, clique para parar).
o.window({ title = ".*(está compartilhando|is sharing) .*" }, {
  workspace = "special:screenshare silent",
  no_initial_focus = true,
  no_anim = true,
})
