-- LiquidIA: vidro líquido. Cantos arredondados, blur atrás de janelas e da
-- casca translúcida (barra, menus, notificações) e um brilho laranja na janela
-- ativa. O resto (gaps, animações) segue o padrão do Omarchy.

local active_border_color = { colors = { "rgba(ff2a2aee)", "rgba(ff9a1fee)" }, angle = 45 }
local inactive_border_color = "rgba(ffffff1f)"

hl.config({
  general = {
    col = {
      active_border = active_border_color,
      inactive_border = inactive_border_color,
    },
  },

  decoration = {
    rounding = 14,
    rounding_power = 3,

    active_opacity = 0.97,
    inactive_opacity = 0.9,

    shadow = {
      enabled = true,
      range = 28,
      render_power = 3,
      color = "rgba(ff4a1a55)",
      color_inactive = "rgba(00000066)",
    },

    blur = {
      enabled = true,
      size = 9,
      passes = 3,
      noise = 0.02,
      contrast = 1.0,
      brightness = 0.9,
      vibrancy = 0.35,
      vibrancy_darkness = 0.2,
      popups = true,
      new_optimizations = true,
      xray = false,
    },
  },

  group = {
    col = {
      border_active = active_border_color,
      border_inactive = inactive_border_color,
    },

    groupbar = {
      gradient_rounding = 8,
    },
  },
})

-- A casca do Omarchy é layer-shell: blur atrás dela, ignorando os pixels
-- quase transparentes (sombras e cantos) para não borrar um retângulo.
hl.layer_rule({
  match = { namespace = "^omarchy-(bar|menu|notifications|osd|clipboard|emojis|keyboard-panel|polkit|reminders|network-qr)$" },
  blur = true,
  ignore_alpha = 0.2,
})
