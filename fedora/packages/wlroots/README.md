# wlroots (DoxIA build)

Fedora's wlroots 0.20.2 with `skip-dmabuf-import-check-on-vmwgfx.patch`. On
VMware (vmwgfx) the DMA-BUF import check in `wlr_linux_dmabuf_v1` closes the
imported handle with `drmCloseBufferHandle()`, which fails with EINVAL there, so
every GL client buffer is rejected ("importing the supplied dmabufs failed"):
the SDDM greeter shows only the cursor and Quickshell gives up. The patch skips
that advisory check when the main device is vmwgfx; the renderer still imports
for real.

The other files are Fedora's (spec from the 0.20.2-1.fc44 source RPM). The
release is `1%{?dist}.doxia1`, so it sorts newer than Fedora's own build.

`fedora/doxia/install-wlroots` installs the RPMs from this repository's release
`wlroots-<version>` only on VMware guests, and version-locks them so a Fedora
update does not swap them out. On any other machine it removes the lock.

## Shipping a new build

```bash
sudo dnf install rpmdevtools
sudo dnf builddep fedora/packages/wlroots/wlroots.spec

top=$(mktemp -d)
mkdir -p "$top/SOURCES"
cp fedora/packages/wlroots/{*.patch,*.gpg,examples.meson.build} "$top/SOURCES/"
spectool -g -C "$top/SOURCES" fedora/packages/wlroots/wlroots.spec
rpmbuild --define "_topdir $top" -bb fedora/packages/wlroots/wlroots.spec

gh release create wlroots-<version> \
  "$top"/RPMS/x86_64/wlroots-{,devel-}<version>.fc*.doxia1.x86_64.rpm \
  --target fedora --latest=false
```

Then set `version` in `fedora/doxia/install-wlroots`. When Fedora ships a newer
wlroots, rebase the spec on its source RPM and keep the patch.
