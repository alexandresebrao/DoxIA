#!/bin/bash

# Fedora port of Omarchy: per-user setup (run as your user, after install-system.sh).
# Seeds ~/.config the way Omarchy's /etc/skel would, but only with configs that
# don't change the KDE session. Existing files are backed up, never clobbered.

set -euo pipefail

if (( EUID == 0 )); then
  echo "Run as your user, not root." >&2
  exit 1
fi

export OMARCHY_PATH="$HOME/.local/share/omarchy"
export PATH="$OMARCHY_PATH/bin:$PATH"
backup_suffix=".bak-omarchy-$(date +%Y%m%d%H%M%S)"

seed() {
  local src="$OMARCHY_PATH/$1" dest="$2"
  if [[ -e $dest || -L $dest ]]; then
    mv "$dest" "$dest$backup_suffix"
    echo "  backed up $dest -> $dest$backup_suffix"
  fi
  mkdir -p "$(dirname "$dest")"
  cp -a "$src" "$dest"
}

echo "==> Seeding Omarchy configs into ~/.config"
# Skipped on purpose (they would also change KDE): autostart, fcitx5, wireplumber,
# git, chromium, environment.d.
for item in hypr omarchy foot alacritty ghostty kitty btop imv lazygit tmux starship.toml; do
  seed "config/$item" "$HOME/.config/$item"
done
# xdg-terminal-exec reads this only when XDG_CURRENT_DESKTOP=Hyprland.
seed default/xdg-terminal-exec/hyprland-xdg-terminals.list "$HOME/.config/hyprland-xdg-terminals.list"

echo "==> ChapeuOS branding (About and screensaver logos)"
mkdir -p ~/.config/omarchy/branding
[[ -f ~/.config/omarchy/branding/about.txt ]] || cp "$OMARCHY_PATH/icon.txt" ~/.config/omarchy/branding/about.txt
[[ -f ~/.config/omarchy/branding/screensaver.txt ]] || cp "$OMARCHY_PATH/logo.txt" ~/.config/omarchy/branding/screensaver.txt

echo "==> uwsm session environment"
mkdir -p ~/.config/uwsm/env.d
ln -sfn "$OMARCHY_PATH/default/uwsm/env.d/10-omarchy" ~/.config/uwsm/env.d/10-omarchy

echo "==> Fonts (JetBrainsMono Nerd Font + Omarchy glyph font)"
font_dir="$HOME/.local/share/fonts"
mkdir -p "$font_dir/JetBrainsMonoNerd" "$font_dir/omarchy"
if ! fc-list | grep -q "JetBrainsMono Nerd Font"; then
  tmp=$(mktemp -d)
  curl -fsSL -o "$tmp/jbm.tar.xz" \
    https://github.com/ryanoasis/nerd-fonts/releases/latest/download/JetBrainsMono.tar.xz
  tar -xJf "$tmp/jbm.tar.xz" -C "$font_dir/JetBrainsMonoNerd" --wildcards '*.ttf'
  rm -rf "$tmp"
fi
cp -f "$OMARCHY_PATH/default/fonts/omarchy/omarchy.ttf" "$font_dir/omarchy/"
fc-cache -f "$font_dir" >/dev/null

echo "==> Marking Omarchy first-run/provisioning as done (Arch-only steps)"
# provision-user would reassign XDG Desktop/Templates, set Chromium as default
# browser, HEY as mailto handler and install a set of AI CLIs via mise.
omarchy-done mark finalize-user
omarchy-done mark first-run-user
mkdir -p ~/.local/state/omarchy/migrations
for migration in "$OMARCHY_PATH"/migrations/*.sh; do
  [[ -f $migration ]] && touch ~/.local/state/omarchy/migrations/"$(basename "$migration")"
done

echo "==> Theme (Tokyo Night)"
mkdir -p ~/.config/omarchy/themes
if [[ ! -s ~/.local/state/omarchy/current/theme.name ]]; then
  OMARCHY_THEME_HEADLESS=1 omarchy-theme-set "Tokyo Night"
fi
mkdir -p ~/.config/btop/themes
ln -snf "$HOME/.local/state/omarchy/current/theme/btop.theme" ~/.config/btop/themes/current.theme

echo "==> XCompose"
if [[ ! -f ~/.XCompose ]]; then
  OMARCHY_USER_NAME=$(git config --global user.name || true) \
  OMARCHY_USER_EMAIL=$(git config --global user.email || true) \
    bash -c 'source "$OMARCHY_PATH/install/user/xcompose.sh"'
fi

echo "==> Lock-on-sleep user unit (started only from Hyprland, never in KDE)"
mkdir -p ~/.config/systemd/user
sed -e "s|/usr/bin/omarchy-|$OMARCHY_PATH/bin/omarchy-|g" \
    -e '/^\[Install\]/,$d' \
  "$OMARCHY_PATH/default/systemd/user/omarchy-sleep-lock.service" \
  > ~/.config/systemd/user/omarchy-sleep-lock.service
systemctl --user daemon-reload

if ! grep -q omarchy-sleep-lock ~/.config/hypr/autostart.lua; then
  cat >> ~/.config/hypr/autostart.lua <<'EOF'

-- Fedora port: lock the screen before suspend (Omarchy enables this unit globally).
o.exec_on_start("systemctl --user start omarchy-sleep-lock.service")
EOF
fi

echo "==> Personal setup (plugins, bar layout, window rules)"
bash "$OMARCHY_PATH/fedora/install-personal.sh"

echo
echo "Done. Log out of KDE and pick \"ChapeuOS (Hyprland uwsm)\" on the login screen."
echo "Super + K shows the keybindings, Super + Space opens the menu."
