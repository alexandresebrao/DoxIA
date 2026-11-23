# scenefx (DoxIA build)

scenefx 0.5 from the kaleedtc/mangowm COPR with three patches.

`clip-blur-damage-to-output.patch` (backport of upstream main) clips the blur
damage to the output. Without it a blurred window partly off screen (the
Painel Rápido sliding in) puts negative coordinates in the output damage;
xdg-desktop-portal-wlr echoes them back through ext-image-copy-capture,
wlroots rejects them ("Invalid buffer damage coordinates") and the portal
exits, freezing any screen share. Drop it once a scenefx release has the fix.

`blur-null-transparency-mask-source.patch` (upstream 5c8fdc20d1, after 0.5):
`wlr_scene_blur_set_transparency_mask_source()` dereferenced the source even
when clearing it with NULL. mango sets the mask source on every layer-shell
blur. Drop it with the next scenefx release.

`fallback-to-bgra-effect-buffers-on-vmwgfx.patch`: The effect framebuffers
(blur, shadows) are allocated as XBGR8888/ABGR8888, which vmwgfx (VMware)
either does not list or cannot allocate with the scanout flag wlroots' gbm
allocator always passes. Every frame then logged "gbm_bo_create failed" /
"Failed to create effect framebuffers" and the effects were skipped. The patch
falls back to XRGB8888/ARGB8888, which work everywhere; other machines never
reach the fallback.

The spec is the COPR's (from its 0.5-3.fc44 source RPM). The release is
`3%{?dist}.doxia4`, so it sorts newer than the COPR's own build.

`fedora/doxia/install-scenefx` installs the RPMs from this repository's release
`scenefx-<version>` on every machine: it replaces the kaleedtc/mangowm COPR,
which DoxIA no longer enables. The vmwgfx patch only changes anything on VMware.

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
  "$top"/RPMS/x86_64/scenefx-{,devel-}<version>.fc*.doxia4.x86_64.rpm \
  --target fedora --latest=false
```

Then set `version` in `fedora/doxia/install-scenefx`. A new scenefx upstream
(github.com/wlrfx/scenefx) is ours to package now: bump the spec, keep the
patch, and check mango still builds against it.
