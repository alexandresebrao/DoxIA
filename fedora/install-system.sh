#!/bin/bash

# Fedora port of Omarchy: system-level setup (run with sudo).
# Installs Hyprland + Quickshell + Omarchy runtime deps side by side with KDE.
# Nothing here removes or reconfigures KDE/Plasma.

set -euo pipefail

if (( EUID != 0 )); then
  echo "Run with sudo: sudo bash $0" >&2
  exit 1
fi

target_user=${SUDO_USER:?run through sudo so the target user is known}
target_home=$(getent passwd "$target_user" | cut -d: -f6)
omarchy_path="$target_home/.local/share/omarchy"

if [[ ! -d $omarchy_path/bin ]]; then
  echo "Omarchy checkout not found at $omarchy_path" >&2
  exit 1
fi

echo "==> Enabling COPR repositories"
dnf install -y dnf-plugins-core
dnf copr disable -y sdegler/hyprland 2>/dev/null || true
dnf copr enable -y mineiro/hyprland
dnf copr enable -y errornointernet/quickshell
dnf copr enable -y brycensranch/gpu-screen-recorder-git

echo "==> Installing packages"
packages=(
  # Compositor, session, portals
  hyprland hyprland-guiutils hyprpicker hyprsunset
  xdg-desktop-portal-hyprland xdg-desktop-portal-gtk uwsm sddm

  # Omarchy shell (bar, menu, notifications, OSD, lock, polkit) runs on Quickshell
  quickshell qt6-qtwayland qt6-qtimageformats qt6-qtsvg qt6-qt5compat qt6-qtmultimedia
  gtk4-layer-shell

  # Terminal + default apps
  foot nautilus nautilus-python sushi gnome-disk-utility evince-thumbnailer imv mpv mpv-mpris flatpak
  gnome-keyring yaru-icon-theme yaru-theme

  # Wayland/desktop utilities used by omarchy-* commands
  brightnessctl pamixer playerctl wireplumber wl-clipboard wtype grim slurp cliphist
  gpu-screen-recorder libnotify upower udiskie xdg-user-dirs xdg-terminal-exec
  bluez-tools NetworkManager-tui wiremix

  # CLI toolbox used by Omarchy scripts and TUIs
  btop fastfetch gum jq socat inotify-tools fzf util-linux-script eza zoxide ripgrep fd-find bat tmux
  ImageMagick vips-tools ffmpegthumbnailer qrencode zbar tesseract tldr inxi plocate
  python3-gobject tree-sitter-cli nss-mdns

  # Fonts
  google-noto-sans-fonts google-noto-emoji-fonts google-noto-sans-cjk-fonts
  fontawesome-fonts-all jetbrains-mono-fonts
)
dnf install -y --setopt=install_weak_deps=False "${packages[@]}"

# Fedora's own quickshell (0.2.x) and uwsm can win over the COPR builds on first
# install; Omarchy 4 needs quickshell >= 0.3.1.
dnf upgrade -y --refresh quickshell uwsm

# Document viewer from Flathub instead of the RPM; evince-thumbnailer above keeps
# PDF thumbnails in Nautilus.
echo "==> Installing Flatpak apps"
flatpak remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install -y --noninteractive flathub org.gnome.Evince

# Fedora ships tuned-ppd (same D-Bus API as power-profiles-daemon, which conflicts
# with it) but no powerprofilesctl, which Omarchy's power menu relies on.
if [[ ! -x /usr/bin/powerprofilesctl ]]; then
  install -Dm755 "$omarchy_path/fedora/bin/powerprofilesctl" /usr/local/bin/powerprofilesctl
fi

echo "==> Linking Omarchy into /usr/share/omarchy"
# Many Omarchy scripts reference /usr/share/omarchy directly (Arch package path).
if [[ -e /usr/share/omarchy && ! -L /usr/share/omarchy ]]; then
  echo "/usr/share/omarchy exists and is not a symlink; leaving it alone" >&2
else
  ln -sfn "$omarchy_path" /usr/share/omarchy
fi

# Same mechanism as `omarchy dev link`: OMARCHY_PATH points at the checkout.
# In the format omarchy dev link writes, which omarchy-plymouth-set checks before
# trusting a user-owned checkout.
printf 'export OMARCHY_PATH="%s"\n' "$omarchy_path" > /etc/omarchy.conf

echo "==> DoxIA About screen (fastfetch) and MOTD"
mkdir -p /etc/fastfetch
ln -sfn "$omarchy_path/fedora/fastfetch/config.jsonc" /etc/fastfetch/config.jsonc
{ echo; sed 's/^/  /' "$omarchy_path/fedora/doxia/brand.ansi"; echo; sed "s/@FEDORA@/$(rpm -E %fedora)/" "$omarchy_path/fedora/doxia/motd"; } > /etc/motd

echo "==> Installing Omarchy session entry"
install -Dm644 "$omarchy_path/default/wayland-sessions/omarchy.desktop" \
  /usr/share/wayland-sessions/omarchy.desktop

echo "==> Login screen (SDDM with the DoxIA theme on a Hyprland greeter)"
install -d /usr/share/sddm/themes/omarchy /etc/sddm.conf.d
install -m644 "$omarchy_path"/default/sddm/omarchy/* /usr/share/sddm/themes/omarchy/
install -m644 "$omarchy_path/default/sddm/hyprland.lua" /usr/share/sddm/hyprland.lua
install -m644 "$omarchy_path"/etc/sddm.conf.d/*.conf /etc/sddm.conf.d/
restorecon -R /usr/share/sddm /etc/sddm.conf.d 2>/dev/null || true
# The theme preselects SDDM's last user and session, which a fresh SDDM doesn't
# have yet (the Arch ISO seeds it): start on the DoxIA session.
if [[ ! -s /var/lib/sddm/state.conf ]]; then
  install -d -o sddm -g sddm /var/lib/sddm
  printf '[Last]\nUser=%s\nSession=/usr/share/wayland-sessions/omarchy.desktop\n' "$target_user" > /var/lib/sddm/state.conf
  chown sddm:sddm /var/lib/sddm/state.conf
fi
# Replaces whichever display manager the Fedora spin enabled (Plasma Login, GDM).
systemctl enable --force sddm.service
# The ISO installs from @core, where Anaconda picks multi-user.target before SDDM exists.
systemctl set-default graphical.target

echo "==> Configuring lock screen PAM (Fedora variant of omarchy-apply-lock)"
cat > /etc/pam.d/omarchy-lock-password <<'EOF'
auth       required                    pam_faillock.so preauth silent deny=10 unlock_time=120
-auth      [success=2 default=ignore]  pam_systemd_home.so
auth       [success=1 default=bad]     pam_unix.so try_first_pass nullok
auth       [default=die]               pam_faillock.so authfail deny=10 unlock_time=120
auth       optional                    pam_permit.so
auth       required                    pam_env.so
auth       required                    pam_faillock.so authsucc
account    include                     system-auth
EOF

if [[ -x /usr/bin/fprintd-list ]] && /usr/bin/fprintd-list "$target_user" 2>/dev/null | grep -qi finger; then
  cat > /etc/pam.d/omarchy-lock-fingerprint <<'EOF'
auth       required                    pam_fprintd.so
account    include                     system-auth
EOF
fi

echo "==> Nautilus action icons for Yaru (from install/config/theme-system.sh)"
mkdir -p /usr/share/icons/Yaru/scalable/actions
ln -snf /usr/share/icons/Adwaita/symbolic/actions/go-previous-symbolic.svg \
  /usr/share/icons/Yaru/scalable/actions/go-previous-symbolic.svg
ln -snf /usr/share/icons/Adwaita/symbolic/actions/go-next-symbolic.svg \
  /usr/share/icons/Yaru/scalable/actions/go-next-symbolic.svg
gtk-update-icon-cache /usr/share/icons/Yaru &>/dev/null || true

echo
echo "System setup done. Now run (as $target_user, no sudo):"
echo "  bash $omarchy_path/fedora/install-user.sh"
