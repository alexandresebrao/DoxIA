# DoxIA installer kickstart (build.sh fills in @COMMIT@ and embeds it in the ISO).
#
# Only language, keyboard, time zone, sources and packages are set here: the
# installer still asks for the disk, network, the user account and root.
# The %post clones this repository at the commit the ISO was built from and
# runs the same scripts as a manual install, minus the personal setup.

lang pt_BR.UTF-8
keyboard --vckeymap=br-abnt2 --xlayouts=br
timezone America/Sao_Paulo --utc

url --mirrorlist=https://mirrors.fedoraproject.org/mirrorlist?repo=fedora-$releasever&arch=$basearch
repo --name=updates --mirrorlist=https://mirrors.fedoraproject.org/mirrorlist?repo=updates-released-f$releasever&arch=$basearch

%packages
@core
@standard
@hardware-support
@base-graphical
@fonts
@multimedia
@networkmanager-submodules
@printing
@guest-desktop-agents
git
plymouth-theme-spinner
redhat-display-fonts
redhat-text-fonts
# A Fedora remix ships the generic logos instead of Fedora's trademarks.
generic-logos
-fedora-logos
%end

%post --log=/root/doxia-install.log
set -euo pipefail

repo=https://github.com/alexandresebrao/ChapeuOS.git
commit=@COMMIT@

user=$(awk -F: '$3 >= 1000 && $3 < 60000 { print $1; exit }' /etc/passwd)
if [[ -z $user ]]; then
  echo "DoxIA: no user account was created in the installer, so the desktop was not set up."
  echo "Create one, then follow the README: $repo"
  exit 0
fi
home=$(getent passwd "$user" | cut -d: -f6)
checkout=$home/.local/share/omarchy

echo "==> Cloning DoxIA at $commit for $user"
runuser -u "$user" -- mkdir -p "$home/.local/share"
runuser -u "$user" -- git clone --branch fedora "$repo" "$checkout"
# Stay on the branch (update.sh pulls it) but at the commit the ISO was built from.
runuser -u "$user" -- git -C "$checkout" reset -q --hard "$commit"

SUDO_USER=$user bash "$checkout/fedora/install-system.sh"
runuser -l "$user" -c 'bash ~/.local/share/omarchy/fedora/install-user.sh --no-personal'

echo "==> Boot splash (Fedora's spinner with the DoxIA watermark)"
theme=/usr/share/plymouth/themes/doxia
install -d "$theme"
cp /usr/share/plymouth/themes/spinner/*.png "$theme/"
install -m644 "$checkout"/default/plymouth/doxia/{doxia.plymouth,watermark.png} "$theme/"
plymouth-set-default-theme doxia
dracut -f --regenerate-all
%end
