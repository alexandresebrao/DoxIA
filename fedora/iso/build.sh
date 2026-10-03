#!/bin/bash

# Builds the DoxIA installer ISO: Fedora's netinstall (Anaconda) rebuilt with lorax
# as "DoxIA", with the generic logos instead of Fedora's, the DoxIA installer theme
# and doxia.ks embedded. Installing needs internet: packages come from Fedora's
# mirrors and %post clones this repository at the commit the ISO was built from.
#
#   sudo bash ~/.local/share/omarchy/fedora/iso/build.sh [output dir]
#
# Needs lorax (dnf install lorax lorax-templates-generic) and ~6 GB free.

set -euo pipefail

if (( EUID != 0 )); then
  echo "Run with sudo: sudo bash $0" >&2
  exit 1
fi

iso_dir=$(cd "$(dirname "$0")" && pwd)
omarchy_path=$(cd "$iso_dir/../.." && pwd)
release=$(rpm -E %fedora)
arch=$(uname -m)
out=$(realpath -m "${1:-$PWD/doxia-iso}")
volid="DoxIA-$release-$arch"
iso="$out/DoxIA-$release-$arch.iso"

commit=$(git -C "$omarchy_path" rev-parse HEAD)
if ! git -C "$omarchy_path" branch -r --contains "$commit" | grep -q 'origin/fedora'; then
  echo "Commit $commit is not on origin/fedora: push it first, the installer clones it from GitHub." >&2
  exit 1
fi
if [[ -n $(git -C "$omarchy_path" status --porcelain) ]]; then
  echo "Warning: uncommitted changes are not part of the ISO (it installs $commit)." >&2
fi

mirror=https://dl.fedoraproject.org/pub/fedora/linux
work=$(mktemp -d "${TMPDIR:-/var/tmp}/doxia-iso.XXXXXX")
trap 'rm -rf "$work"' EXIT
mkdir -p "$out"

echo "==> Building the installer image with lorax (Fedora $release, $arch)"
lorax \
  --product DoxIA --version "$release" --release "$release" --isfinal \
  --volid "$volid" \
  --source "$mirror/releases/$release/Everything/$arch/os/" \
  --source "$mirror/updates/$release/Everything/$arch/" \
  --skip-branding --installpkgs generic-logos --installpkgs fedora-release \
  --add-template "$iso_dir/doxia-theme.tmpl" \
  --add-template-var "themedir=$iso_dir/anaconda" \
  --logfile "$out/lorax.log" --tmp "$work" \
  "$work/lorax"

echo "==> Embedding the kickstart (installs $commit)"
sed "s/@COMMIT@/$commit/" "$iso_dir/doxia.ks" > "$work/doxia.ks"
rm -f "$iso"
mkksiso --ks "$work/doxia.ks" --cmdline "inst.profile=doxia" \
  "$work/lorax/images/boot.iso" "$iso"

if [[ -n ${SUDO_USER:-} ]]; then
  chown "$SUDO_USER:" "$out" "$iso" "$out/lorax.log"
fi
echo
echo "Done: $iso ($(du -h "$iso" | cut -f1))"
