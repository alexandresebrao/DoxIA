#!/bin/bash

# Fedora port of Omarchy: personal setup, run at the end of install-user.sh.
# Restores my plugins (FortiVPN, Java servers, Now Playing, screen share, Xi bar),
# FedorAI branding, bar layout, menu extensions, default agent,
# screen-share picker config and Hyprland window rules from fedora/personal. Existing files are backed up.
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
for tpl in "$personal"/omarchy/themed/*.tpl; do
  seed "omarchy/themed/$(basename "$tpl")" "$HOME/.config/omarchy/themed/$(basename "$tpl")"
done
omarchy-pkg-add redhat-display-fonts redhat-text-fonts papirus-icon-theme-dark git

# Sem minimizar/maximizar/fechar nem ícone do app nas barras de título GTK: o
# Hyprland cuida das janelas.
gsettings set org.gnome.desktop.wm.preferences button-layout ':'
for ini in "$HOME/.config/gtk-3.0/settings.ini" "$HOME/.config/gtk-4.0/settings.ini"; do
  if [[ -f $ini ]] && grep -q '^gtk-decoration-layout=' "$ini"; then
    sed -i 's/^gtk-decoration-layout=.*/gtk-decoration-layout=:/' "$ini"
  fi
done

echo "==> Screen share (Hyprland picker, stop button in the bar)"
seed hypr/xdph.conf "$HOME/.config/hypr/xdph.conf"
omarchy-pkg-add wtype jq
systemctl --user try-restart xdg-desktop-portal-hyprland || true

echo "==> Screensaver effects (ttfx, no Fedora package)"
if ! command -v ttfx >/dev/null; then
  omarchy-pkg-add cargo
  cargo install --git https://github.com/omacom/ttfx --tag v0.5.0 --root "$HOME/.local"
fi

echo "==> Bar layout, menu extensions, default agent and editor"
seed omarchy/shell.json "$HOME/.config/omarchy/shell.json"
seed omarchy/extensions/omarchy-menu.jsonc "$HOME/.config/omarchy/extensions/omarchy-menu.jsonc"
seed omarchy/defaults/agent "$HOME/.config/omarchy/defaults/agent"
# Nano como editor padrão (omarchy-launch-editor / $EDITOR), no lugar do nvim.
mkdir -p "$HOME/.local/state/omarchy/defaults"
echo nano > "$HOME/.local/state/omarchy/defaults/editor"

echo "==> Hyprland window rules"
if ! grep -q "xwaylandvideobridge" ~/.config/hypr/hyprland.lua 2>/dev/null; then
  { echo; cat "$personal/hypr/window-rules.lua"; } >> ~/.config/hypr/hyprland.lua
fi
if ! grep -q "special:screenshare" ~/.config/hypr/hyprland.lua 2>/dev/null; then
  { echo; cat "$personal/hypr/screenshare-rule.lua"; } >> ~/.config/hypr/hyprland.lua
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
# Pastas do Tela em vermelho sobre o Papirus-Dark (Papirus-Tela-Red, gerado no $HOME).
bash "$OMARCHY_PATH/themes/rhel-8/make-icons.sh"
# GTK4/libadwaita (Nautilus) com as cores do tema, via themed/gtk.css.tpl.
gtk_css="$HOME/.config/gtk-4.0/gtk.css"
mkdir -p "$(dirname "$gtk_css")"
if ! grep -q 'current/theme/gtk.css' "$gtk_css" 2>/dev/null; then
  echo "@import url('file://$HOME/.local/state/omarchy/current/theme/gtk.css');" >> "$gtk_css"
fi
if ! grep -q 'nautilus-grid-view' "$gtk_css"; then
  cat >> "$gtk_css" <<'CSS'

/* Nautilus: ícones da grade 4% menores (o zoom só tem degraus de ~25%). */
.nautilus-grid-view .nautilus-view-cell > :first-child {
  transform: scale(0.96);
}
CSS
fi
omarchy-theme-set rhel-8
