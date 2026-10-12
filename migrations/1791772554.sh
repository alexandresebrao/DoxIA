echo "Switch from the DoxIA theme to LiquidIA, the new default"

# LiquidIA replaced DoxIA as the default theme. Installs still on DoxIA (or on
# rhel-8, its old name) are on it because it was the default, not by choice, so
# move them over once. Anyone who picks DoxIA again afterwards keeps it.
theme_name_path="$HOME/.local/state/omarchy/current/theme.name"

[[ -s $theme_name_path ]] || exit 0

theme_name=$(<"$theme_name_path")
[[ $theme_name == "doxia" || $theme_name == "rhel-8" ]] || exit 0

if [[ -n ${HYPRLAND_INSTANCE_SIGNATURE:-} ]]; then
  omarchy-theme-set "liquidia"
else
  OMARCHY_THEME_HEADLESS=1 omarchy-theme-set "liquidia"
fi
