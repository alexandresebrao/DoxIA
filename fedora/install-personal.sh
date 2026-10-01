#!/bin/bash

# Fedora port of Omarchy: personal setup, run at the end of install-user.sh.
# Restores my plugins (FortiVPN, Java servers, Now Playing), bar layout, default
# agent and Hyprland window rules from fedora/personal. Existing files are backed up.
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

echo "==> Bar layout and default agent"
seed omarchy/shell.json "$HOME/.config/omarchy/shell.json"
seed omarchy/defaults/agent "$HOME/.config/omarchy/defaults/agent"

echo "==> Hyprland window rules"
if ! grep -q "xwaylandvideobridge" ~/.config/hypr/hyprland.lua 2>/dev/null; then
  { echo; cat "$personal/hypr/window-rules.lua"; } >> ~/.config/hypr/hyprland.lua
fi

echo "==> FortiVPN (openfortivpn + polkit helper)"
rpm -q openfortivpn &>/dev/null || sudo dnf install -y openfortivpn
sudo "$HOME/.config/omarchy/plugins/klab.fortivpn/setup-system.sh"

if [[ ! -d /usr/local/java/tomcat || ! -d $HOME/.sdkman/candidates/java/8.0.192-oracle ]]; then
  echo
  echo "Note: the Java servers widget expects Tomcat in /usr/local/java/tomcat, JBoss in"
  echo "/usr/local/java/jboss-4.0.2 and the Oracle JDK 8 at ~/.sdkman/candidates/java/8.0.192-oracle."
fi

echo "Add VPN profiles from the FortiVPN widget; credentials stay in /etc/openfortivpn."
