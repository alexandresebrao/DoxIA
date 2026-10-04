#!/bin/bash

# Fedora port of Omarchy: personal setup, run at the end of install-user.sh.
# Restores my plugins (FortiVPN, Java servers, panel, services), DoxIA branding, bar layout,
# menu extensions, default agent and Hyprland window rules from fedora/personal. Existing files are backed up.
# Can be re-run on its own to reapply them:
#
#   bash ~/.local/share/omarchy/fedora/install-personal.sh

set -euo pipefail

if (( EUID == 0 )); then
  echo "Run as your user, not root." >&2
  exit 1
fi

export OMARCHY_PATH="$HOME/.local/share/omarchy"
export PATH="$OMARCHY_PATH/bin:$PATH"
personal="$OMARCHY_PATH/fedora/personal"
backup_suffix=".bak-omarchy-$(date +%Y%m%d%H%M%S)"

seed() {
  local src="$personal/$1" dest="$2"
  if [[ -e $dest || -L $dest ]]; then
    mv "$dest" "$dest$backup_suffix"
    echo "  backed up $dest -> $dest$backup_suffix"
  fi
  mkdir -p "$(dirname "$dest")"
  cp -a "$src" "$dest"
}

echo "==> Plugins"
for plugin in "$personal"/omarchy/plugins/*/; do
  name=$(basename "$plugin")
  seed "omarchy/plugins/$name" "$HOME/.config/omarchy/plugins/$name"
done

echo "==> Branding"
seed omarchy/branding "$HOME/.config/omarchy/branding"
omarchy-pkg-add redhat-display-fonts redhat-text-fonts


echo "==> Screensaver effects (ttfx, no Fedora package)"
if ! command -v ttfx >/dev/null; then
  omarchy-pkg-add cargo
  cargo install --git https://github.com/omacom/ttfx --tag v0.5.0 --root "$HOME/.local"
fi

echo "==> Bar layout, menu extensions and default agent"
seed omarchy/shell.json "$HOME/.config/omarchy/shell.json"
seed omarchy/extensions/omarchy-menu.jsonc "$HOME/.config/omarchy/extensions/omarchy-menu.jsonc"
seed omarchy/defaults/agent "$HOME/.config/omarchy/defaults/agent"

echo "==> Hyprland window rules"
if ! grep -q "xwaylandvideobridge" ~/.config/hypr/hyprland.lua 2>/dev/null; then
  { echo; cat "$personal/hypr/window-rules.lua"; } >> ~/.config/hypr/hyprland.lua
fi

echo "==> Painel de serviços (svc no PATH e skill /servicos do Claude Code)"
mkdir -p "$HOME/.local/bin"
ln -sfn "$HOME/.config/omarchy/plugins/alexandre.services/svc" "$HOME/.local/bin/svc"
seed claude/skills/servicos "$HOME/.claude/skills/servicos"
if ! grep -q "org.omarchy.services-log" ~/.config/hypr/hyprland.lua 2>/dev/null; then
  { echo; echo "-- Janela de log do painel de serviços (plugin alexandre.services): flutuante e larga."
    grep -F 'org.omarchy.services-log' "$personal/hypr/window-rules.lua"; } >> ~/.config/hypr/hyprland.lua
fi

echo "==> FortiVPN (openfortivpn + polkit helper)"
omarchy-pkg-add openfortivpn
sudo "$HOME/.config/omarchy/plugins/alexandre.fortivpn/setup-system.sh"

if [[ ! -d /usr/local/java/tomcat || ! -d $HOME/.sdkman/candidates/java/8.0.192-oracle ]]; then
  echo
  echo "Note: the Java servers widget expects Tomcat in /usr/local/java/tomcat, JBoss in"
  echo "/usr/local/java/jboss-4.0.2 and the Oracle JDK 8 at ~/.sdkman/candidates/java/8.0.192-oracle."
fi

echo "Add VPN profiles from the FortiVPN widget; credentials stay in /etc/openfortivpn."

echo "==> Theme"
# As cores do GTK, as pastas vermelhas e os ajustes do Nautilus já vêm do install-user.sh.
omarchy-theme-set rhel-8
