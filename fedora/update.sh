#!/bin/bash

# Brings this machine up to date with the DoxIA repo: pulls it, then reapplies the
# DoxIA theme (the default), branding, nvm and SDKMAN!, zsh with Oh My Zsh, the salsicha screensaver, the icon font, menu extensions, default
# agent and editor, screen-share and Hyprland window rules, /etc/motd, the login session
# name, the SDDM theme, the GRUB boot menu theme, ONLYOFFICE (instead of LibreOffice), Thunderbird as the default mail client and the DoxIA bar widgets (fedora/doxia/apply-bar: the
# DoxIA plugins, the services panel included, are refreshed and the missing widgets
# added; the rest of the bar stays as it is).
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
doxia="$OMARCHY_PATH/fedora/doxia"
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

echo "==> Kitty as the default terminal (foot and chafa are no longer used)"
omarchy-pkg-add kitty
omarchy-pkg-drop foot chafa
# Older installs pinned foot in ~/.config/hyprland-xdg-terminals.list, which beats
# both the system order and a terminal picked with omarchy-default-terminal.
if [[ -e $HOME/.config/hyprland-xdg-terminals.list ]]; then
  backup "$HOME/.config/hyprland-xdg-terminals.list"
fi
[[ -e $HOME/.config/kitty ]] || cp -a "$OMARCHY_PATH/config/kitty" "$HOME/.config/kitty"

echo "==> Volume keys and power profiles (pactl, tuned-ppd)"
omarchy-pkg-add pulseaudio-utils tuned-ppd
if ! systemctl is-enabled --quiet tuned-ppd.service || ! systemctl is-active --quiet tuned-ppd.service; then
  sudo systemctl enable --now tuned.service tuned-ppd.service
fi

echo "==> FortiVPN bar widget (openfortivpn)"
omarchy-pkg-add openfortivpn

echo "==> GNOME Shell out (ISO installs got it through the yaru-theme metapackage)"
# Only Yaru's icons are used. The keyring PAM module came in as one of GNOME
# Shell's dependencies but unlocks the keyring at the SDDM login, so it is marked
# as wanted before dnf drops the rest (GDM, Mutter, GNOME Settings...). So are
# malcontent (its polkit policy lets flatpak read the parental controls; without
# it every flatpak dies with "Not allowed to query parental controls data") and
# tuned/tuned-ppd, installed just above, which dnf would otherwise sweep away too.
if omarchy-pkg-present yaru-theme; then
  keep=()
  for pkg in gnome-keyring gnome-keyring-pam yaru-icon-theme malcontent tuned tuned-ppd; do
    omarchy-pkg-present "$pkg" && keep+=("$pkg")
  done
  sudo dnf mark user -y "${keep[@]}" >/dev/null
  omarchy-pkg-drop yaru-theme gnome-shell-theme-yaru gnome-shell-extension-user-theme gnome-shell
fi
# Installs updated before that fix lost malcontent along with GNOME Shell.
omarchy-pkg-add malcontent

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
  place "$doxia/branding/$file" "$HOME/.config/omarchy/branding/$file"
done
mkdir -p "$HOME/.bashrc.d"
# The greeting used to be linked as fedorai.sh, before the FedorAI → DoxIA rename.
rm -f "$HOME/.bashrc.d/fedorai.sh"
ln -sfn "$OMARCHY_PATH/fedora/doxia/greeting.sh" "$HOME/.bashrc.d/doxia.sh"
if ! grep -q 'bashrc.d' "$HOME/.bashrc" 2>/dev/null; then
  printf '\nfor rc in ~/.bashrc.d/*; do [[ -f $rc ]] && . "$rc"; done; unset rc\n' >> "$HOME/.bashrc"
fi

echo "==> nvm and SDKMAN! (Node.js and Java versions)"
omarchy-pkg-add zip unzip
bash "$OMARCHY_PATH/fedora/doxia/install-dev-tools"
# ~/.bashrc.d/dev-tools.sh loads nvm now: drop the lines nvm's own installer
# appended to ~/.bashrc, which would load it a second time.
if grep -qxF 'export NVM_DIR="$HOME/.config/nvm"' "$HOME/.bashrc" 2>/dev/null; then
  mkdir -p "$backup_dir"
  cp -a "$HOME/.bashrc" "$backup_dir/.bashrc"
  echo "  backup: ~/.bashrc"
  sed -i -e '\|^export NVM_DIR="$HOME/.config/nvm"$|d' \
    -e '\|^\[ -s "$NVM_DIR/nvm.sh" \] && \\. "$NVM_DIR/nvm.sh"|d' \
    -e '\|^\[ -s "$NVM_DIR/bash_completion" \] && \\. "$NVM_DIR/bash_completion"|d' "$HOME/.bashrc"
fi

echo "==> zsh with Oh My Zsh and the DoxIA theme (folder, git, Node.js and Java versions)"
omarchy-pkg-add zsh
bash "$OMARCHY_PATH/fedora/doxia/install-zsh"
if [[ $(getent passwd "$USER" | cut -d: -f7) != */zsh ]]; then
  echo "  zsh as the login shell (sudo; takes effect at the next login)"
  sudo usermod -s /usr/bin/zsh "$USER"
fi

echo "==> Menu extensions, default agent and editor"
place "$doxia/omarchy-menu.jsonc" "$HOME/.config/omarchy/extensions/omarchy-menu.jsonc"
place "$doxia/default-agent" "$HOME/.config/omarchy/defaults/agent"
mkdir -p "$HOME/.local/state/omarchy/defaults"
echo nano > "$HOME/.local/state/omarchy/defaults/editor"

echo "==> User units (Bluetooth pairing agent, crash and migration notifiers, internal monitor recovery)"
bash "$OMARCHY_PATH/fedora/doxia/install-user-units"

# Chrome and Claude Code come with the ISO; older installs get them once, so
# removing either sticks. A failure only warns, and the next Atualizar retries.
echo "==> Google Chrome (once)"
if ! omarchy-done check doxia-chrome; then
  if bash "$OMARCHY_PATH/fedora/doxia/install-chrome"; then
    omarchy-done mark doxia-chrome
  else
    echo "  Chrome setup failed; the next Atualizar will retry."
  fi
fi

echo "==> Claude Code (once)"
if ! omarchy-done check doxia-claude; then
  if bash "$OMARCHY_PATH/fedora/doxia/install-claude"; then
    omarchy-done mark doxia-claude
  else
    echo "  Claude Code was not installed; the next Atualizar will retry."
  fi
fi

echo "==> Voxtype dictation (once: Remover > Ditado must stick)"
# Installs made before the ISO set it up never got it (the ISO skips the first-run
# invitation). A failed download only warns, and the next Atualizar retries.
if ! omarchy-done check doxia-voxtype; then
  if omarchy-pkg-present voxtype && [[ -f $HOME/.config/voxtype/config.toml ]]; then
    omarchy-done mark doxia-voxtype
  elif bash "$OMARCHY_PATH/fedora/doxia/install-voxtype"; then
    omarchy-done mark doxia-voxtype
  else
    echo "  Voxtype setup failed; the next Atualizar will retry."
  fi
fi

echo "==> Run on every Atualizar (omarchy-update's post-update hook)"
bash "$OMARCHY_PATH/fedora/doxia/install-update-hook"

echo "==> DoxIA bar widgets (∞ menu button, workspace icons, Now Playing, screen share, usage, services)"
bash "$OMARCHY_PATH/fedora/doxia/apply-bar"

echo "==> Screen share picker and Hyprland window rules"
place "$OMARCHY_PATH/fedora/doxia/hypr/xdph.conf" "$HOME/.config/hypr/xdph.conf"
if ! grep -q "xwaylandvideobridge" "$HOME/.config/hypr/hyprland.lua" 2>/dev/null; then
  { echo; cat "$doxia/hypr/window-rules.lua"; } >> "$HOME/.config/hypr/hyprland.lua"
fi
if ! grep -q "org.omarchy.services-log" "$HOME/.config/hypr/hyprland.lua" 2>/dev/null; then
  { echo; cat "$doxia/hypr/services-rule.lua"; } >> "$HOME/.config/hypr/hyprland.lua"
fi
if ! grep -q "org.doxia.Painel" "$HOME/.config/hypr/hyprland.lua" 2>/dev/null; then
  { echo; cat "$doxia/hypr/painel-rule.lua"; } >> "$HOME/.config/hypr/hyprland.lua"
fi
if ! grep -q "special:screenshare" "$HOME/.config/hypr/hyprland.lua" 2>/dev/null; then
  { echo; cat "$OMARCHY_PATH/fedora/doxia/hypr/screenshare-rule.lua"; } >> "$HOME/.config/hypr/hyprland.lua"
fi
if ! grep -q "scroll_focus" "$HOME/.config/hypr/bindings.lua" 2>/dev/null; then
  { echo; cat "$doxia/hypr/scroll-focus.lua"; } >> "$HOME/.config/hypr/bindings.lua"
fi
if ! grep -q "doxia.iniciar" "$HOME/.config/hypr/bindings.lua" 2>/dev/null; then
  { echo; cat "$doxia/hypr/iniciar.lua"; } >> "$HOME/.config/hypr/bindings.lua"
fi

echo "==> Theme (DoxIA, the default)"
omarchy-pkg-add redhat-display-fonts redhat-text-fonts papirus-icon-theme-dark git
# Reapplied only while DoxIA (or no theme) is in use: the Atualizar menu runs
# this after every update, and a theme picked by hand must survive it.
current_theme=$(cat "$HOME/.local/state/omarchy/current/theme.name" 2>/dev/null || true)
reapply_theme=false
# rhel-8 is the same theme under its old name, before the Red Hat marks came out of it.
[[ -z $current_theme || $current_theme == "doxia" || $current_theme == "rhel-8" ]] && reapply_theme=true
# Copies under ~/.config/omarchy/themes shadow the themes shipped in the repo, so move
# them all aside and let the repo's doxia be the one in use.
if $reapply_theme; then
  for theme in "$HOME"/.config/omarchy/themes/*; do
    [[ -e $theme ]] && backup "$theme"
  done
fi
for tpl in "$doxia"/themed/*.tpl; do
  place "$tpl" "$HOME/.config/omarchy/themed/$(basename "$tpl")"
done
[[ -d $HOME/.local/share/icons/Papirus-Tela-Red ]] || bash "$OMARCHY_PATH/themes/doxia/make-icons.sh"
bash "$doxia/lucide/make-theme"
mkdir -p "$HOME/.config/omarchy/hooks/theme-set.d"
cp "$doxia/hooks/theme-set.d/lucide-icons" "$HOME/.config/omarchy/hooks/theme-set.d/"
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
if ! $reapply_theme; then
  echo "  keeping the current theme ($current_theme)"
elif [[ -n ${HYPRLAND_INSTANCE_SIGNATURE:-} ]]; then
  omarchy-theme-set doxia
else
  OMARCHY_THEME_HEADLESS=1 omarchy-theme-set doxia
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

echo "==> Boot menu (graphical GRUB with the DoxIA theme, sudo)"
# Regenerates grub.cfg only when the theme or its settings changed.
sudo bash "$OMARCHY_PATH/fedora/doxia/install-grub-theme" "$OMARCHY_PATH"

echo "==> Office suite: ONLYOFFICE from Flathub in Brazilian Portuguese instead of LibreOffice (sudo)"
# Removing LibreOffice also drops the JDK only it needed: java and node come from
# SDKMAN! and nvm (fedora/doxia/install-dev-tools).
sudo bash -s <<'ROOT'
set -euo pipefail
office_rpms=$(rpm -qa --qf '%{NAME}\n' 'libreoffice*' unoconv)
if [[ -n $office_rpms ]]; then
  dnf remove -y $office_rpms
fi
# Brazilian Portuguese whatever the system's language: ONLYOFFICE takes its interface
# language from LANG, and ships the pt-BR translation and spell checker itself. Its
# Qt and GTK default to IBus inside Flatpak, whose portal fails without ibus-daemon
# (which DoxIA doesn't run) and uwsm's fumon reports that as a failed unit: use
# their built-in input (dead keys and compose included), as they fall back to anyway.
flatpak override --system --env=LANG=pt_BR.UTF-8 --env=LANGUAGE=pt_BR \
  --env=QT_IM_MODULE=compose --env=GTK_IM_MODULE=gtk-im-context-simple org.onlyoffice.desktopeditors
flatpak install -y --noninteractive --or-update flathub org.onlyoffice.desktopeditors
ROOT

echo "==> Mail: Thunderbird from Flathub as every user's default mail client (sudo)"
sudo flatpak install -y --noninteractive --or-update flathub org.mozilla.Thunderbird
sudo install -Dm644 "$OMARCHY_PATH/fedora/doxia/mimeapps.list" /etc/xdg/mimeapps.list

echo "==> System: /etc/motd, About screen, session name, login screen and browser theme color (sudo)"
sudo bash -s "$OMARCHY_PATH" "$(rpm -E %fedora)" <<'ROOT'
set -euo pipefail
omarchy_path=$1
{ echo; sed 's/^/  /' "$omarchy_path/fedora/doxia/brand.ansi"; echo; sed "s/@FEDORA@/$2/" "$omarchy_path/fedora/doxia/motd"; } > /etc/motd
chmod 644 /etc/motd
mkdir -p /etc/fastfetch
ln -sfn "$omarchy_path/fedora/fastfetch/config.jsonc" /etc/fastfetch/config.jsonc
ln -sfn "$omarchy_path/fedora/bin/fastfetch" /usr/local/bin/fastfetch
install -Dm644 "$omarchy_path/etc/xdg/kitty/kitty.conf" /etc/xdg/kitty/kitty.conf
install -Dm644 "$omarchy_path/default/xdg-terminal-exec/hyprland-xdg-terminals.list" \
  /usr/share/xdg-terminal-exec/hyprland-xdg-terminals.list
install -Dm755 "$omarchy_path/fedora/doxia/kernel-install/95-doxia-title.install" \
  /etc/kernel/install.d/95-doxia-title.install
/etc/kernel/install.d/95-doxia-title.install retitle
install -Dm644 "$omarchy_path/default/wayland-sessions/omarchy.desktop" /usr/share/wayland-sessions/omarchy.desktop
install -d /usr/share/sddm/themes/omarchy /etc/sddm.conf.d
install -m644 "$omarchy_path"/default/sddm/omarchy/* /usr/share/sddm/themes/omarchy/
install -m644 "$omarchy_path/default/sddm/hyprland.lua" /usr/share/sddm/hyprland.lua
install -m644 "$omarchy_path"/etc/sddm.conf.d/*.conf /etc/sddm.conf.d/
restorecon -R /usr/share/sddm /etc/sddm.conf.d 2>/dev/null || true
bash "$omarchy_path/fedora/doxia/install-browser-policy" "$omarchy_path"
ROOT

if [[ -d $backup_dir ]]; then
  echo "Replaced files were saved in $backup_dir"
fi
echo "Done."
