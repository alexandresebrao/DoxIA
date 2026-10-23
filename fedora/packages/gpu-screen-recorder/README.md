# gpu-screen-recorder (DoxIA build)

The screen recording (`omarchy-capture-screenrecording`). Neither Fedora nor
RPM Fusion packages it; it came from the brycensranch/gpu-screen-recorder-git
COPR, which DoxIA no longer enables. The spec is that COPR's (from its
6.1.3-1.fc44 source RPM), keeping its epoch 2 so the DoxIA build sorts newer
than what installs from it carry. It links Fedora's ffmpeg-free, as the COPR's
did. `%post` gives `gsr-kms-server` cap_sys_admin: recording a monitor over
KMS asks for no password.

`fedora/doxia/install-gpu-screen-recorder` installs the RPM from this
repository's release `gpu-screen-recorder-<version>`.

## Shipping a new build

Upstream (git.dec05eba.com/gpu-screen-recorder) publishes no tags or release
tarballs, only snapshots named after the commit count and hash. The Flathub
manifest (github.com/flathub/com.dec05eba.gpu_screen_recorder) names the
snapshot of each release: set it as `snapshot` in the spec, with `Version`.

```bash
# Build deps; spectool is in rpmdevtools
sudo dnf install rpmdevtools
sudo dnf builddep fedora/packages/gpu-screen-recorder/gpu-screen-recorder.spec

top=$(mktemp -d)
mkdir -p "$top/SOURCES"
spectool -g -C "$top/SOURCES" fedora/packages/gpu-screen-recorder/gpu-screen-recorder.spec
rpmbuild --define "_topdir $top" -bb fedora/packages/gpu-screen-recorder/gpu-screen-recorder.spec

gh release create gpu-screen-recorder-<version> \
  "$top"/RPMS/x86_64/gpu-screen-recorder-<version>.fc*.x86_64.rpm \
  --target fedora --latest=false
```

Then set `version` in `fedora/doxia/install-gpu-screen-recorder`. A release
for a new Fedora version needs its own `fc<N>` RPM uploaded to the same
release.
