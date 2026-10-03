#!/bin/bash

# Brings this machine up to date with the DoxIA repo: pulls it, then reapplies the
# RHEL 8 theme (the DoxIA default), branding, the salsicha screensaver, the icon font, menu extensions, default
# agent and editor, screen-share and Hyprland window rules, /etc/motd, the login session
# name and the SDDM theme. The top bar is left alone: ~/.config/omarchy/shell.json and
# the bar plugins/panels in ~/.config/omarchy/plugins are never touched.
#
# Everything it replaces or moves aside goes to ~/.local/state/omarchy/doxia-backup-<date>/.
#
#   bash ~/.local/share/omarchy/fedora/update.sh            # git pull + apply
#   bash ~/.local/share/omarchy/fedora/update.sh --no-pull  # apply only

set -euo pipefail

if (( EUID == 0 )); then
  echo "Run as your user, not root." >&2
  exit 1
fi

export OMARCHY_PATH="$HOME/.local/share/omarchy"
export PATH="$OMARCHY_PATH/bin:$PATH"
personal="$OMARCHY_PATH/fedora/personal"
backup_dir="$HOME/.local/state/omarchy/doxia-backup-$(date +%Y%m%d%H%M%S)"

# Moves a path into the backup dir, keeping its place relative to $HOME.
backup() {
  local rel=${1#"$HOME"/}
  mkdir -p "$backup_dir/$(dirname "$rel")"
  mv "$1" "$backup_dir/$rel"
  echo "  backup: ~/$rel"
}

# Copies src over dest, backing up dest first unless it is already identical.
place() {
  local src=$1 dest=$2
  if [[ -e $dest ]] && diff -rq "$src" "$dest" >/dev/null 2>&1; then
    return 0
  fi
  if [[ -e $dest || -L $dest ]]; then
    backup "$dest"
  fi
  mkdir -p "$(dirname "$dest")"
  cp -a "$src" "$dest"
}

# The old salsicha hook patched bin/omarchy-screensaver in place. The repo now runs
# fedora/doxia/screensaver-salsicha itself, so drop that patch (before the pull, so
# it can't clash with it) and retire the hook.
sed -i '/screensaver-salsicha  # salsicha$/,+1d' "$OMARCHY_PATH/bin/omarchy-screensaver"
if [[ -e $HOME/.config/omarchy/hooks/post-update.d/screensaver-salsicha ]]; then
  backup "$HOME/.config/omarchy/hooks/post-update.d/screensaver-salsicha"
fi

if [[ ${1:-} != "--no-pull" ]]; then
  echo "==> git pull"
  # Local edits (like a hook-patched bin/) are stashed around the pull and put back.
  git -C "$OMARCHY_PATH" pull --rebase --autostash
  # Run the freshly pulled copy of this script.
  exec bash "$OMARCHY_PATH/fedora/update.sh" --no-pull
fi

echo "==> Icon font (∞ glyph)"
mkdir -p "$HOME/.local/share/fonts/omarchy"
cp -f "$OMARCHY_PATH/default/fonts/omarchy/omarchy.ttf" "$HOME/.local/share/fonts/omarchy/"
fc-cache -f "$HOME/.local/share/fonts/omarchy" >/dev/null
# The font in the repo already carries the ∞, so the hook that re-added it is obsolete.
if [[ -e $HOME/.config/omarchy/hooks/post-update.d/infinito-glifo ]]; then
  backup "$HOME/.config/omarchy/hooks/post-update.d/infinito-glifo"
fi

echo "==> Branding (About, screensaver and terminal logos, terminal greeting)"
for file in about.txt screensaver.txt logo.ansi; do
  place "$personal/omarchy/branding/$file" "$HOME/.config/omarchy/branding/$file"
done
mkdir -p "$HOME/.bashrc.d"
# The greeting used to be linked as fedorai.sh, before the FedorAI → DoxIA rename.
rm -f "$HOME/.bashrc.d/fedorai.sh"
ln -sfn "$OMARCHY_PATH/fedora/doxia/greeting.sh" "$HOME/.bashrc.d/doxia.sh"
if ! grep -q 'bashrc.d' "$HOME/.bashrc" 2>/dev/null; then
  printf '\nfor rc in ~/.bashrc.d/*; do [[ -f $rc ]] && . "$rc"; done; unset rc\n' >> "$HOME/.bashrc"
fi

echo "==> Menu extensions, default agent and editor"
place "$personal/omarchy/extensions/omarchy-menu.jsonc" "$HOME/.config/omarchy/extensions/omarchy-menu.jsonc"
place "$personal/omarchy/defaults/agent" "$HOME/.config/omarchy/defaults/agent"
mkdir -p "$HOME/.local/state/omarchy/defaults"
echo nano > "$HOME/.local/state/omarchy/defaults/editor"

echo "==> Screen share picker and Hyprland window rules"
place "$personal/hypr/xdph.conf" "$HOME/.config/hypr/xdph.conf"
if ! grep -q "xwaylandvideobridge" "$HOME/.config/hypr/hyprland.lua" 2>/dev/null; then
  { echo; cat "$personal/hypr/window-rules.lua"; } >> "$HOME/.config/hypr/hyprland.lua"
fi
if ! grep -q "special:screenshare" "$HOME/.config/hypr/hyprland.lua" 2>/dev/null; then
  { echo; cat "$personal/hypr/screenshare-rule.lua"; } >> "$HOME/.config/hypr/hyprland.lua"
fi

echo "==> Theme (RHEL 8, the DoxIA default)"
omarchy-pkg-add redhat-display-fonts redhat-text-fonts papirus-icon-theme-dark git
# Copies under ~/.config/omarchy/themes shadow the themes shipped in the repo, so move
# them all aside and let the repo's rhel-8 be the one in use.
for theme in "$HOME"/.config/omarchy/themes/*; do
  [[ -e $theme ]] && backup "$theme"
done
for tpl in "$personal"/omarchy/themed/*.tpl; do
  place "$tpl" "$HOME/.config/omarchy/themed/$(basename "$tpl")"
done
[[ -d $HOME/.local/share/icons/Papirus-Tela-Red ]] || bash "$OMARCHY_PATH/themes/rhel-8/make-icons.sh"
gsettings set org.gnome.desktop.wm.preferences button-layout ':'
for ini in "$HOME/.config/gtk-3.0/settings.ini" "$HOME/.config/gtk-4.0/settings.ini"; do
  if [[ -f $ini ]] && grep -q '^gtk-decoration-layout=' "$ini"; then
    sed -i 's/^gtk-decoration-layout=.*/gtk-decoration-layout=:/' "$ini"
  fi
done
gtk_css="$HOME/.config/gtk-4.0/gtk.css"
mkdir -p "$(dirname "$gtk_css")"
if ! grep -q 'current/theme/gtk.css' "$gtk_css" 2>/dev/null; then
  echo "@import url('file://$HOME/.local/state/omarchy/current/theme/gtk.css');" >> "$gtk_css"
fi
if [[ -n ${HYPRLAND_INSTANCE_SIGNATURE:-} ]]; then
  omarchy-theme-set rhel-8
else
  OMARCHY_THEME_HEADLESS=1 omarchy-theme-set rhel-8
fi

echo "==> Dev-link authorization for the checkout (sudo)"
# omarchy-plymouth-set only trusts a user-owned checkout named this way, the format
# omarchy dev link writes; older installs wrote a bare OMARCHY_PATH= line.
expected_conf="export OMARCHY_PATH=\"$OMARCHY_PATH\""
if [[ $(cat /etc/omarchy.conf 2>/dev/null) != "$expected_conf" ]]; then
  printf '%s\n' "$expected_conf" | sudo tee /etc/omarchy.conf >/dev/null
fi

echo "==> Boot splash (Fedora's spinner with the DoxIA watermark)"
# Fedora's BGRT theme (firmware logo and spinner) with the ∞ DOXIA mark where it
# shows the Fedora one. The initramfs rebuild is slow, so it only runs on a change.
plymouth_theme=/usr/share/plymouth/themes/doxia
if omarchy-cmd-present plymouth-set-default-theme &&
  { ! cmp -s "$OMARCHY_PATH/default/plymouth/doxia/watermark.png" "$plymouth_theme/watermark.png" ||
    ! cmp -s "$OMARCHY_PATH/default/plymouth/doxia/doxia.plymouth" "$plymouth_theme/doxia.plymouth" ||
    [[ $(plymouth-set-default-theme) != "doxia" ]]; }; then
  omarchy-pkg-add plymouth-theme-spinner
  sudo bash -s "$OMARCHY_PATH" <<'ROOT'
set -euo pipefail
theme=/usr/share/plymouth/themes/doxia
install -d "$theme"
cp /usr/share/plymouth/themes/spinner/*.png "$theme/"
install -m644 "$1"/default/plymouth/doxia/{doxia.plymouth,watermark.png} "$theme/"
restorecon -R "$theme" 2>/dev/null || true
plymouth-set-default-theme doxia
# The theme used to be installed as fedorai, before the FedorAI → DoxIA rename.
rm -rf /usr/share/plymouth/themes/fedorai
dracut -f --regenerate-all
ROOT
fi

echo "==> System: /etc/motd, About screen, session name and login screen (sudo)"
sudo bash -s "$OMARCHY_PATH" "$(rpm -E %fedora)" <<'ROOT'
set -euo pipefail
omarchy_path=$1
{ echo; sed 's/^/  /' "$omarchy_path/fedora/doxia/brand.ansi"; echo; sed "s/@FEDORA@/$2/" "$omarchy_path/fedora/doxia/motd"; } > /etc/motd
chmod 644 /etc/motd
mkdir -p /etc/fastfetch
ln -sfn "$omarchy_path/fedora/fastfetch/config.jsonc" /etc/fastfetch/config.jsonc
install -Dm644 "$omarchy_path/default/wayland-sessions/omarchy.desktop" /usr/share/wayland-sessions/omarchy.desktop
install -d /usr/share/sddm/themes/omarchy /etc/sddm.conf.d
install -m644 "$omarchy_path"/default/sddm/omarchy/* /usr/share/sddm/themes/omarchy/
install -m644 "$omarchy_path/default/sddm/hyprland.lua" /usr/share/sddm/hyprland.lua
install -m644 "$omarchy_path"/etc/sddm.conf.d/*.conf /etc/sddm.conf.d/
restorecon -R /usr/share/sddm /etc/sddm.conf.d 2>/dev/null || true
ROOT

if [[ -d $backup_dir ]]; then
  echo "Replaced files were saved in $backup_dir"
fi
echo "Done. The top bar (shell.json and plugins) was left as it was."
