#!/bin/bash

# Copies my current plugins, branding, bar layout, menu extensions,
# screen-share picker config and default agent from ~/.config back into fedora/personal so they can
# be committed. Backups (*.bak*) are left out. Hyprland window rules live in
# fedora/personal/hypr/window-rules.lua and are edited there by hand.

set -euo pipefail

personal="$HOME/.local/share/omarchy/fedora/personal"

for plugin in "$HOME"/.config/omarchy/plugins/*/; do
  name=$(basename "$plugin")
  rm -rf "$personal/omarchy/plugins/$name"
  cp -a "$plugin" "$personal/omarchy/plugins/$name"
done

rm -rf "$personal/omarchy/branding"
cp -a "$HOME/.config/omarchy/branding" "$personal/omarchy/branding"

cp "$HOME/.config/hypr/xdph.conf" "$personal/hypr/xdph.conf"

mkdir -p "$personal/omarchy/themed"
find "$HOME/.config/omarchy/themed" -maxdepth 1 -name '*.tpl' -exec cp {} "$personal/omarchy/themed/" \;

find "$personal" -name __pycache__ -prune -exec rm -rf {} +
find "$personal" -name '*.bak*' -exec rm -rf {} +

cp "$HOME/.config/omarchy/shell.json" "$personal/omarchy/shell.json"
cp "$HOME/.config/omarchy/defaults/agent" "$personal/omarchy/defaults/agent"
mkdir -p "$personal/omarchy/extensions"
cp "$HOME/.config/omarchy/extensions/omarchy-menu.jsonc" "$personal/omarchy/extensions/omarchy-menu.jsonc"

git -C "$personal" status --short -- .
