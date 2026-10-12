-- LiquidOceanIA: vidro líquido claro. Cantos arredondados, blur atrás de janelas e
-- da casca translúcida (barra, menus, notificações) e um brilho azul suave na janela
-- ativa. O resto (gaps, animações) segue o padrão do Omarchy.

local active_border_color = { colors = { "rgba(1f6bffee)", "rgba(2fd4ffee)" }, angle = 45 }
local inactive_border_color = "rgba(1b2a3d24)"

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
      range = 16,
      render_power = 3,
      color = "rgba(1f7ae02e)",
      color_inactive = "rgba(1b2a3d1a)",
    },

    blur = {
      enabled = true,
      size = 9,
      passes = 3,
      noise = 0.02,
      contrast = 1.0,
      brightness = 1.0,
      vibrancy = 0.15,
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

-- Em VM (sem GPU de verdade, o Hyprland desenha no llvmpipe) o blur de 3
-- passadas deixa tudo lerdo: uma passada curta, sem vibrancy, ainda dá o fosco.
if o.shell_succeeds("systemd-detect-virt --vm --quiet") then
  hl.config({
    decoration = {
      shadow = { range = 10 },
      blur = {
        size = 4,
        passes = 1,
        vibrancy = 0,
        noise = 0,
      },
    },
  })
end
