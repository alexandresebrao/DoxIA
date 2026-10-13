%global forgeurl https://github.com/mangowm/mango

Name:           mangowm
Version:        0.18.0
Release:        2.doxia1%{?dist}
Summary:        Practical and powerful Wayland compositor built on dwl

License:        GPL-3.0-or-later
URL:            %{forgeurl}

# We use the upstream tarball which already contains assets/
Source0:        %{forgeurl}/archive/refs/tags/%{version}.tar.gz
Patch0:         mango-dmabuf-null.patch

BuildRequires:  gcc
BuildRequires:  gcc-c++
BuildRequires:  meson
BuildRequires:  ninja-build
BuildRequires:  pkgconfig
BuildRequires:  wayland-devel
BuildRequires:  wayland-protocols-devel
BuildRequires:  wlroots-devel
BuildRequires:  libinput-devel
BuildRequires:  libdrm-devel
BuildRequires:  libxkbcommon-devel
BuildRequires:  pixman-devel
BuildRequires:  libseat-devel
BuildRequires:  pcre2-devel
BuildRequires:  scenefx-devel
BuildRequires:  cjson-devel
BuildRequires:  pango-devel
BuildRequires:  systemd-rpm-macros

# scenefx is resolved from the same Copr project
Requires:       scenefx

%description
Mango is a modern, lightweight, high-performance tiling Wayland compositor built on dwl, utilizing SceneFX for smooth animations and window effects.

%prep
%autosetup -p1 -n mango-%{version}

%build
%meson -Dxwayland=enabled
%meson_build

%install
%meson_install

%files
%license LICENSE*
%doc README.md
%{_bindir}/mango
%{_bindir}/mmsg
%{_bindir}/mangonag
%{_datadir}/wayland-sessions/mango.desktop
%{_datadir}/xdg-desktop-portal/mango-portals.conf
%{_mandir}/man1/mmsg.1*
%{_mandir}/man1/mangonag.1*
%dir %{_sysconfdir}/mango
%config(noreplace) %{_sysconfdir}/mango/config.conf
%config(noreplace) %{_sysconfdir}/mango/config.toml
%{_userunitdir}/mango-session.target

%changelog
* Fri Oct 09 2026 kaleedtc <kaleed@tutanota.com> - 0.18.0-2
- Update to upstream version 0.18.0
- Add missing mangonag binary, man page, and config.toml to %files
