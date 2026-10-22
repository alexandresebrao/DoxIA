# mangowm (DoxIA build)

mango 0.18.0 with `mango-dmabuf-null.patch`: without a DRM render node (Hyper-V,
llvmpipe) `wlr_linux_dmabuf_v1_create_with_renderer()` returns NULL and mango
crashed passing it to `wlr_scene_set_linux_dmabuf_v1()`.

`fedora/doxia/install-mango` installs the RPM from this repository's release
`mangowm-<version>`. scenefx is the DoxIA build too (`fedora/packages/scenefx`);
the kaleedtc/mangowm COPR is no longer used.

## Shipping a new build

```bash
# Build deps (scenefx-devel is in this repository's scenefx-<version> release);
# spectool is in rpmdevtools
sudo dnf install rpmdevtools
sudo dnf builddep fedora/packages/mango/mangowm.spec

top=$(mktemp -d)
mkdir -p "$top/SOURCES"
cp fedora/packages/mango/mango-dmabuf-null.patch "$top/SOURCES/"
spectool -g -C "$top/SOURCES" fedora/packages/mango/mangowm.spec
rpmbuild --define "_topdir $top" -bb fedora/packages/mango/mangowm.spec

gh release create mangowm-<version> "$top"/RPMS/x86_64/mangowm-<version>.fc*.x86_64.rpm \
  --target fedora --latest=false
```

Then set `version` in `fedora/doxia/install-mango`. A release for a new Fedora
version needs its own `fc<N>` RPM uploaded to the same release.
