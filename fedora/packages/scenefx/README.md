# scenefx (DoxIA build)

scenefx 0.5 from the kaleedtc/mangowm COPR with
`fallback-to-bgra-effect-buffers-on-vmwgfx.patch`. The effect framebuffers
(blur, shadows) are allocated as XBGR8888/ABGR8888, which vmwgfx (VMware)
either does not list or cannot allocate with the scanout flag wlroots' gbm
allocator always passes. Every frame then logged "gbm_bo_create failed" /
"Failed to create effect framebuffers" and the effects were skipped. The patch
falls back to XRGB8888/ARGB8888, which work everywhere; other machines never
reach the fallback.

The spec is the COPR's (from its 0.5-3.fc44 source RPM). The release is
`3%{?dist}.doxia2`, so it sorts newer than the COPR's own build.

`fedora/doxia/install-scenefx` installs the RPMs from this repository's release
`scenefx-<version>` only on VMware guests, and version-locks them so a COPR
update does not swap them out. On any other machine it removes the lock.

## Shipping a new build

```bash
sudo dnf install rpmdevtools
sudo dnf builddep fedora/packages/scenefx/scenefx.spec

top=$(mktemp -d)
mkdir -p "$top/SOURCES"
cp fedora/packages/scenefx/*.patch "$top/SOURCES/"
spectool -g -C "$top/SOURCES" fedora/packages/scenefx/scenefx.spec
rpmbuild --define "_topdir $top" -bb fedora/packages/scenefx/scenefx.spec

gh release create scenefx-<version> \
  "$top"/RPMS/x86_64/scenefx-{,devel-}<version>.fc*.doxia2.x86_64.rpm \
  --target fedora --latest=false
```

Then set `version` in `fedora/doxia/install-scenefx`. When the COPR ships a
newer scenefx, rebase the spec on its source RPM and keep the patch.
