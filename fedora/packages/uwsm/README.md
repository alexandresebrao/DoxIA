# uwsm (DoxIA build)

uwsm runs the DoxIA session (mango and its units). Fedora doesn't package it;
it came from the mineiro/hyprland COPR, which DoxIA no longer enables. The spec
is that COPR's (from its 0.27.0-1.fc44 source RPM), on upstream 0.27.1.

`fedora/doxia/install-uwsm` installs the RPM from this repository's release
`uwsm-<version>`. It is noarch: one RPM per Fedora release.

## Shipping a new build

```bash
# Build deps; spectool is in rpmdevtools
sudo dnf install rpmdevtools
sudo dnf builddep fedora/packages/uwsm/uwsm.spec

top=$(mktemp -d)
mkdir -p "$top/SOURCES"
spectool -g -C "$top/SOURCES" fedora/packages/uwsm/uwsm.spec
rpmbuild --define "_topdir $top" -bb fedora/packages/uwsm/uwsm.spec

gh release create uwsm-<version> "$top"/RPMS/noarch/uwsm-<version>.fc*.noarch.rpm \
  --target fedora --latest=false
```

Then set `version` in `fedora/doxia/install-uwsm`. New versions:
github.com/Vladimir-csp/uwsm.
