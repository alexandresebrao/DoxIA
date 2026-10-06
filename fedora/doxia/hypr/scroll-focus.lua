-- SUPER + roda do mouse: no layout scrolling (SUPER+L) troca o foco entre
-- janelas; nos demais layouts troca de workspace, como no padrão do omarchy.
local function scroll_focus(forward)
  return function()
    local workspace = hl.get_active_workspace()
    if workspace and workspace.tiled_layout == "scrolling" then
      hl.dispatch(hl.dsp.focus({ direction = forward and "r" or "l" }))
    else
      hl.dispatch(hl.dsp.focus({ workspace = forward and "e+1" or "e-1" }))
    end
  end
end

hl.unbind("SUPER + mouse_down")
hl.unbind("SUPER + mouse_up")
o.bind("SUPER + mouse_down", "Focus next window (scrolling) or workspace", scroll_focus(true))
o.bind("SUPER + mouse_up", "Focus previous window (scrolling) or workspace", scroll_focus(false))
o.bind("SUPER + mouse_right", "Focus next window (scrolling) or workspace", scroll_focus(true))
o.bind("SUPER + mouse_left", "Focus previous window (scrolling) or workspace", scroll_focus(false))

-- Touchpad: rolagem com dedos não aciona atalhos de scroll, então três dedos
-- para os lados trocam o foco para a janela à esquerda/direita.
hl.gesture({ fingers = 3, direction = "left", action = function() hl.dispatch(hl.dsp.focus({ direction = "l" })) end })
hl.gesture({ fingers = 3, direction = "right", action = function() hl.dispatch(hl.dsp.focus({ direction = "r" })) end })
