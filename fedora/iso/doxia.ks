# DoxIA installer kickstart (build.sh fills in @COMMIT@ and embeds it in the ISO).
#
# The DoxIA wizard (wizard/doxia-installer.py) runs from %pre and asks for the
# language, keyboard, disk, account and computer name; it writes them to
# /tmp/doxia/answers.ks, included below. Anaconda then installs without its own
# interface (cmdline) while the wizard shows the progress. The %post clones this
# repository at the commit the ISO was built from and runs the same scripts as a
# manual install, minus the personal setup.

cmdline
# Without it, cmdline mode ends at "Press ENTER to quit" on tty1 and never reboots.
# The last %post holds Anaconda until the wizard's Restart button is clicked.
reboot --eject

%pre --erroronfail --log=/tmp/doxia-pre.log
/usr/libexec/doxia-installer/doxia-installer-start
%end

%include /tmp/doxia/answers.ks

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

repo=https://github.com/alexandresebrao/DoxIA.git
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

# Last: tell the wizard the installation is done, then hold Anaconda (which reboots
# as soon as it finishes, see reboot above) until the Restart button is clicked.
%post --nochroot
# Keep the network joined in the wizard (Wi-Fi password included) on the installed system
target=/mnt/sysroot/etc/NetworkManager/system-connections
mkdir -p "$target"
for connection in /etc/NetworkManager/system-connections/*.nmconnection; do
  [[ -e $connection ]] || continue
  cp -n "$connection" "$target/"
  chmod 600 "$target/$(basename "$connection")"
done

touch /tmp/doxia/installed
while [[ ! -e /tmp/doxia/reboot ]]; do
  sleep 1
done
%end
