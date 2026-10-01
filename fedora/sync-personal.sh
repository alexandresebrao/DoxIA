#!/bin/bash

# Copies my current plugins, bar layout and default agent from ~/.config back
# into fedora/personal so they can be committed. Hyprland window rules live in
# fedora/personal/hypr/window-rules.lua and are edited there by hand.

set -euo pipefail

personal="$HOME/.local/share/omarchy/fedora/personal"

for plugin in "$HOME"/.config/omarchy/plugins/*/; do
  name=$(basename "$plugin")
  rm -rf "$personal/omarchy/plugins/$name"
  cp -a "$plugin" "$personal/omarchy/plugins/$name"
done
find "$personal" -name __pycache__ -prune -exec rm -rf {} +

cp "$HOME/.config/omarchy/shell.json" "$personal/omarchy/shell.json"
cp "$HOME/.config/omarchy/defaults/agent" "$personal/omarchy/defaults/agent"

git -C "$personal" status --short -- .
