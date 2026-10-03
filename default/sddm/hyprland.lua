-- Minimal Hyprland config for the SDDM Wayland greeter.
-- SDDM starts the greeter itself after the compositor is ready.
hl.config({
  misc = {
    disable_hyprland_logo = true,
    disable_splash_rendering = true,
    force_default_wallpaper = 0,
  },

  animations = {
    enabled = false,
  },
})

local function read(path)
  local file = io.open(path, "r")
  if not file then return "" end
  local value = file:read("*l") or ""
  file:close()
  return value
end

-- Hyper-V guests: hyperv_drm prefers 1024x768 and has no hardware cursor plane,
-- so the greeter came up blurry and without a pointer.
if read("/sys/class/dmi/id/sys_vendor") == "Microsoft Corporation"
  and read("/sys/class/dmi/id/product_name") == "Virtual Machine" then
  hl.monitor({ output = "", mode = "1280x720@60", position = "auto", scale = 1 })
  hl.config({ cursor = { no_hardware_cursors = 1 } })
end
