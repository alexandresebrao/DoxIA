#!/bin/bash

# FedorAI branding only: About, screensaver and terminal logos, the terminal greeting,
# /etc/motd, the login session name and the SDDM logo. Leaves the bar layout
# (~/.config/omarchy/shell.json), plugins, themes and Hyprland config alone, so it is
# the safe way to rebrand a machine after a git pull:
#
#   bash ~/.local/share/omarchy/fedora/install-branding.sh

set -euo pipefail

if (( EUID == 0 )); then
  echo "Run as your user, not root." >&2
  exit 1
fi

OMARCHY_PATH="$HOME/.local/share/omarchy"
branding="$HOME/.config/omarchy/branding"
backup_suffix=".bak-omarchy-$(date +%Y%m%d%H%M%S)"

echo "==> Logos (About, screensaver, terminal)"
mkdir -p "$branding"
for file in about.txt screensaver.txt logo.ansi; do
  if [[ -f $branding/$file ]] && ! cmp -s "$branding/$file" "$OMARCHY_PATH/fedora/personal/omarchy/branding/$file"; then
    cp -a "$branding/$file" "$branding/$file$backup_suffix"
    echo "  backed up $branding/$file -> $branding/$file$backup_suffix"
  fi
  cp "$OMARCHY_PATH/fedora/personal/omarchy/branding/$file" "$branding/$file"
done

echo "==> Terminal greeting"
mkdir -p "$HOME/.bashrc.d"
ln -sfn "$OMARCHY_PATH/fedora/fedorai/greeting.sh" "$HOME/.bashrc.d/fedorai.sh"
if ! grep -q 'bashrc.d' "$HOME/.bashrc" 2>/dev/null; then
  printf '\nfor rc in ~/.bashrc.d/*; do [[ -f $rc ]] && . "$rc"; done; unset rc\n' >> "$HOME/.bashrc"
fi

echo "==> /etc/motd, session name and login screen logo (sudo)"
sudo bash -s "$OMARCHY_PATH" "$(rpm -E %fedora)" <<'ROOT'
set -euo pipefail
omarchy_path=$1
sed "s/@FEDORA@/$2/" "$omarchy_path/fedora/fedorai/motd" > /etc/motd
chmod 644 /etc/motd
install -Dm644 "$omarchy_path/default/wayland-sessions/omarchy.desktop" /usr/share/wayland-sessions/omarchy.desktop
if [[ -d /usr/share/sddm/themes/omarchy ]]; then
  install -m644 "$omarchy_path"/default/sddm/omarchy/{logo.png,metadata.desktop} /usr/share/sddm/themes/omarchy/
fi
ROOT

echo "Done. Open a new terminal or run omarchy-launch-about to see it."
