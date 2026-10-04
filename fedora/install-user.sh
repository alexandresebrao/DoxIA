#!/bin/bash

# Fedora port of Omarchy: per-user setup (run as your user, after install-system.sh).
# Seeds ~/.config the way Omarchy's /etc/skel would, but only with configs that
# don't change the KDE session. Existing files are backed up, never clobbered.
# The installer ISO runs it from its kickstart with --no-personal, before the
# user's first login.

set -euo pipefail

if (( EUID == 0 )); then
  echo "Run as your user, not root." >&2
  exit 1
fi

export OMARCHY_PATH="$HOME/.local/share/omarchy"
export PATH="$OMARCHY_PATH/bin:$PATH"
backup_suffix=".bak-omarchy-$(date +%Y%m%d%H%M%S)"
personal=true
[[ ${1:-} == "--no-personal" ]] && personal=false

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
# The terminal order (Kitty first) is system-wide, from install-system.sh, so
# omarchy-default-terminal's ~/.config/xdg-terminals.list can override it.

# Hyper-V guests: hyperv_drm prefers 1024x768 and has no hardware cursor plane,
# so the first login came up blurry (same check as default/sddm/hyprland.lua).
if [[ $(</sys/class/dmi/id/sys_vendor) == "Microsoft Corporation" &&
      $(</sys/class/dmi/id/product_name) == "Virtual Machine" ]] 2>/dev/null; then
  echo "==> Hyper-V: 1280x720 at scale 1 and a software cursor"
  sed -i -e 's/^local omarchy_gdk_scale = .*/local omarchy_gdk_scale = 1/' \
    -e 's/^local omarchy_monitor_scale = .*/local omarchy_monitor_scale = 1/' \
    -e 's/^\(hl.monitor({ output = "", mode = \)"preferred"/\1"1280x720@60"/' ~/.config/hypr/monitors.lua
  if ! grep -q 'no_hardware_cursors' ~/.config/hypr/input.lua; then
    printf '\n-- Hyper-V has no hardware cursor plane.\nhl.config({\n  cursor = {\n    no_hardware_cursors = 1,\n  },\n})\n' >> ~/.config/hypr/input.lua
  fi
fi

echo "==> DoxIA branding (About and screensaver logos, terminal greeting)"
mkdir -p ~/.config/omarchy/branding
[[ -f ~/.config/omarchy/branding/about.txt ]] || cp "$OMARCHY_PATH/icon.txt" ~/.config/omarchy/branding/about.txt
[[ -f ~/.config/omarchy/branding/screensaver.txt ]] || cp "$OMARCHY_PATH/logo.txt" ~/.config/omarchy/branding/screensaver.txt
mkdir -p ~/.bashrc.d
ln -sfn "$OMARCHY_PATH/fedora/doxia/greeting.sh" ~/.bashrc.d/doxia.sh

echo "==> DoxIA bar (∞ menu button, workspace icons, Now Playing, screen share, usage)"
bash "$OMARCHY_PATH/fedora/doxia/apply-bar"

echo "==> Run fedora/update.sh on every Atualizar (post-update hook)"
bash "$OMARCHY_PATH/fedora/doxia/install-update-hook"

echo "==> Screen share (portal config, Chrome's sharing bar hidden behind the bar button)"
seed fedora/doxia/hypr/xdph.conf ~/.config/hypr/xdph.conf
if ! grep -q "special:screenshare" ~/.config/hypr/hyprland.lua; then
  { echo; cat "$OMARCHY_PATH/fedora/doxia/hypr/screenshare-rule.lua"; } >> ~/.config/hypr/hyprland.lua
fi

echo "==> nano as the default editor (omarchy-launch-editor, \$EDITOR) instead of nvim"
mkdir -p ~/.local/state/omarchy/defaults
echo nano > ~/.local/state/omarchy/defaults/editor

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
# The shell draws its text and Nerd Font icons with "monospace", which Fedora maps
# to Noto Sans Mono; without this, Qt fills icons like the weather glyphs from other
# fonts. Same file omarchy-font-set writes, minus its shell restart.
if [[ ! -f ~/.config/fontconfig/fonts.conf ]]; then
  mkdir -p ~/.config/fontconfig
  cat > ~/.config/fontconfig/fonts.conf <<'XML'
<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "fonts.dtd">
<fontconfig>
  <match target="pattern">
    <test name="family" qual="any">
      <string>monospace</string>
    </test>
    <edit name="family" mode="prepend_first" binding="strong">
      <string>JetBrainsMono Nerd Font</string>
    </edit>
  </match>
</fontconfig>
XML
fi

echo "==> Marking Omarchy first-run/provisioning as done (Arch-only steps)"
# provision-user would reassign XDG Desktop/Templates, set Chromium as default
# browser, HEY as mailto handler and install a set of AI CLIs via mise.
omarchy-done mark finalize-user
omarchy-done mark first-run-user
mkdir -p ~/.local/state/omarchy/migrations
for migration in "$OMARCHY_PATH"/migrations/*.sh; do
  [[ -f $migration ]] && touch ~/.local/state/omarchy/migrations/"$(basename "$migration")"
done

echo "==> GTK apps in the theme's colors and red folder icons (DoxIA default)"
for tpl in "$OMARCHY_PATH"/fedora/personal/omarchy/themed/*.tpl; do
  seed "fedora/personal/omarchy/themed/$(basename "$tpl")" ~/.config/omarchy/themed/"$(basename "$tpl")"
done
gtk_css=~/.config/gtk-4.0/gtk.css
mkdir -p "$(dirname "$gtk_css")"
if ! grep -q 'current/theme/gtk.css' "$gtk_css" 2>/dev/null; then
  echo "@import url('file://$HOME/.local/state/omarchy/current/theme/gtk.css');" >> "$gtk_css"
fi
if ! grep -q 'nautilus-grid-view' "$gtk_css"; then
  cat >> "$gtk_css" <<'CSS'

/* Nautilus: grid icons 4% smaller (zoom only has ~25% steps). */
.nautilus-grid-view .nautilus-view-cell > :first-child {
  transform: scale(0.96);
}
CSS
fi
# No minimize/maximize/close or app icon in GTK title bars: Hyprland manages windows.
for ini in ~/.config/gtk-3.0/settings.ini ~/.config/gtk-4.0/settings.ini; do
  if [[ -f $ini ]] && grep -q '^gtk-decoration-layout=' "$ini"; then
    sed -i 's/^gtk-decoration-layout=.*/gtk-decoration-layout=:/' "$ini"
  fi
done
bash "$OMARCHY_PATH/themes/rhel-8/make-icons.sh"

echo "==> Theme (RHEL 8, the DoxIA default)"
mkdir -p ~/.config/omarchy/themes
if [[ ! -s ~/.local/state/omarchy/current/theme.name ]]; then
  OMARCHY_THEME_HEADLESS=1 omarchy-theme-set "rhel-8"
fi
mkdir -p ~/.config/btop/themes
ln -snf "$HOME/.local/state/omarchy/current/theme/btop.theme" ~/.config/btop/themes/current.theme
# Headless theme-set skips gsettings (no session bus in the installer): set the
# dark scheme, the theme's icons and the bare title bars through a throwaway bus.
dbus-run-session -- bash -c 'omarchy-theme-set-gnome
  gsettings set org.gnome.desktop.wm.preferences button-layout ":"'

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
# No user manager yet when the installer runs this; it reads the unit on first login.
if [[ -S ${XDG_RUNTIME_DIR:-/nonexistent}/bus ]]; then
  systemctl --user daemon-reload
fi

if ! grep -q omarchy-sleep-lock ~/.config/hypr/autostart.lua; then
  cat >> ~/.config/hypr/autostart.lua <<'EOF'

-- Fedora port: lock the screen before suspend (Omarchy enables this unit globally).
o.exec_on_start("systemctl --user start omarchy-sleep-lock.service")
EOF
fi

if $personal; then
  echo "==> Personal setup (plugins, bar layout, window rules)"
  bash "$OMARCHY_PATH/fedora/install-personal.sh"
fi

echo
echo "Done. Log out of KDE and pick \"DoxIA (Hyprland uwsm)\" on the login screen."
echo "Super + K shows the keybindings, Super + Space opens the menu."
