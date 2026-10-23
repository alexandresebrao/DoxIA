# dec05eba publishes snapshots, not tags: r<commits>.<hash> of the release.
%global snapshot r1528.1c01417

Name:           gpu-screen-recorder
Version:        6.1.5
Release:        1.doxia1%{?dist}
Summary:        A shadowplay-like screen recorder for Linux. The fastest screen recorder for Linux.
# The brycensranch/gpu-screen-recorder-git COPR's epoch, which installs from it
# still carry: without it every DoxIA build would sort older.
Epoch:          2

License:        GPL-3.0-only

URL:            https://git.dec05eba.com/%{name}/about

Source:         https://dec05eba.com/snapshot/%{name}.git.%{snapshot}.tar.gz

BuildRequires:  gcc
BuildRequires:  (gcc-g++ or gcc-c++)
BuildRequires:  pkgconfig(libva)
BuildRequires:  pkgconfig(libdrm)
BuildRequires:  pkgconfig(libva-drm)
BuildRequires:  vulkan-headers
BuildRequires:  pkgconfig(libcap)
BuildRequires:  pkgconfig(wayland-client)
BuildRequires:  pkgconfig(wayland-egl)
BuildRequires:  pkgconfig(libpulse)
BuildRequires:  (ffmpeg-free-devel or ffmpeg-devel or ffmpeg-7-mini-devel) 
BuildRequires:  pkgconfig(x11)
BuildRequires:  pkgconfig(xdamage)
BuildRequires:  pkgconfig(xcomposite)
BuildRequires:  pkgconfig(xrandr)
BuildRequires:  pkgconfig(xfixes)
BuildRequires:  meson
BuildRequires:  pkgconfig(dbus-1)
BuildRequires:  pkgconfig(libspa-0.2)
BuildRequires:  pkgconfig(libglvnd)
Requires(post): libcap


%description
Shadowplay like screen recorder for Linux. It is the fastest screen recorder for Linux.


%prep
%autosetup -c

%build
%meson -Dcapabilities=false
%meson_build


%install
%meson_install

%check
%meson_test

%post
setcap cap_sys_admin+ep %{_bindir}/gsr-kms-server

%files
%license LICENSE
%doc README.md
%{_datadir}/gpu-screen-recorder
%{_bindir}/gpu-screen-recorder
%{_bindir}/gsr-kms-server
%{_bindir}/gsr-cli
%{_includedir}/gsr/plugin.h
/usr/lib/systemd/user/%{name}.service
/usr/lib/modprobe.d/gsr-nvidia.conf
%{_mandir}/man1/gsr-kms-server.1*
%{_mandir}/man1/gpu-screen-recorder.1*
%{_mandir}/man1/gsr-cli.1*


%changelog
* Thu Oct 22 2026 Alexandre Sebrão <xandizitxu@hotmail.com> - 2:6.1.5-1.doxia1
- DoxIA build, from the brycensranch/gpu-screen-recorder-git COPR spec: 6.1.5
* Tue Aug 18 2026 Brycen G <brycengranville@outlook.com> - 6.0.1-1
- Update to 6.0.1
* Thu Sep 05 2024 Brycen G <brycengranville@outlook.com> - 5.3.3-1
- Update to 5.3.3
* Thu Sep 05 2024 Brycen G <brycengranville@outlook.com> - 4.3.3-3
- Update to 4.3.3
