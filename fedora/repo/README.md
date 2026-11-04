# DoxIA dnf repository

The DoxIA packages (mango, scenefx, uwsm, gpu-screen-recorder, DoxMail) as a
dnf repository on GitHub Pages: `https://alexandresebrao.github.io/DoxIA/`.
With it, `dnf upgrade` and `dnf system-upgrade` see them like any other
package; moving to a new Fedora release swaps them in the same transaction
instead of stopping on their old dependencies (DoxMail on the previous EDS,
gpu-screen-recorder on the previous ffmpeg).

- `build` writes the site: one repository per Fedora release
  (`fedora/<release>/x86_64/`), with the metadata of every RPM in this
  repository's releases. Each package points at its release asset
  (`createrepo_c --baseurl`), so the site holds metadata only. wlroots (VMware
  only, version-locked by `install-wlroots`) and debug packages are left out.
- `.github/workflows/doxia-dnf-repo.yml` runs it and publishes it whenever a
  release is published, edited or deleted (and by hand).
- `doxia.repo` is what `fedora/doxia/install-repo` puts in
  `/etc/yum.repos.d` (from `install-system.sh` and `update.sh`).

## Signing

The RPMs are not signed; `repomd.xml` is, and it carries the sha256 of every
RPM, so dnf (`repo_gpgcheck=1`) checks the packages through it. Clients trust
`RPM-GPG-KEY-doxia` from the checkout, not the copy on the site. The secret key
is the `DOXIA_REPO_GPG_KEY` secret of this repository; `build` refuses any
other key. `install-repo` enables nothing until `RPM-GPG-KEY-doxia` exists.

## A new Fedora release

Build and upload its `fc<N>` RPMs to the same releases (see each package's
README in `fedora/packages`), add `<N>` to the `build` call in the workflow,
and run it. Then `dnf system-upgrade download --releasever=<N>` finds them.

## Testing locally

```bash
fedora/repo/build /tmp/doxia-site 45   # unsigned without DOXIA_REPO_GPG_KEY
```
