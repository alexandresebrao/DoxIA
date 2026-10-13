Name:           scenefx
Version:        0.5
Release:        3%{?dist}.doxia2
Summary:        Drop-in replacement for the wlroots scene API with eye-candy effects

License:        MIT
URL:            https://github.com/wlrfx/scenefx
Source0:        %{url}/archive/refs/tags/%{version}.tar.gz
# DoxIA: vmwgfx (VMware) has no XBGR/ABGR DMA-BUFs for the effect buffers
Patch0:         fallback-to-bgra-effect-buffers-on-vmwgfx.patch

BuildRequires:  gcc
BuildRequires:  gcc-c++
BuildRequires:  meson >= 0.60.0
BuildRequires:  ninja-build
BuildRequires:  pkgconfig

# Target the 0.20 development package
BuildRequires:  pkgconfig(wlroots-0.20)

BuildRequires:  pkgconfig(wayland-server) >= 1.22.0
BuildRequires:  pkgconfig(wayland-protocols) >= 1.27
BuildRequires:  pkgconfig(libdrm) >= 2.4.114
BuildRequires:  pkgconfig(pixman-1) >= 0.42.0
BuildRequires:  pkgconfig(libinput)
BuildRequires:  pkgconfig(xkbcommon)
BuildRequires:  scdoc >= 1.9.2

%description
A drop-in replacement for the wlroots scene API that allows wayland compositors to render surfaces with eye-candy effects like blur, shadows, and rounded corners.

%package devel
Summary:        Development files for %{name}
Requires:       %{name}%{?_isa} = %{version}-%{release}

%description devel
The %{name}-devel package contains libraries and header files for
developing applications that use %{name}.

%prep
%autosetup -p1 -n scenefx-%{version}

%build
%meson
%meson_build

%install
%meson_install

%files
%license LICENSE
%{_libdir}/libscenefx-%{version}.so

%files devel
%{_includedir}/scenefx-%{version}/
%{_libdir}/pkgconfig/scenefx-%{version}.pc

%changelog
* Tue Oct 13 2026 Alexandre Sebrão <xandizitxu@hotmail.com> - 0.5-3.doxia2
- Also fall back to ARGB8888/XRGB8888 when ABGR/XBGR is listed but can't
  be allocated (vmwgfx rejects it for scanout): gbm_bo_create failed every frame

* Tue Oct 13 2026 Alexandre Sebrão <xandizitxu@hotmail.com> - 0.5-3.doxia1
- Fall back to ARGB8888/XRGB8888 effect buffers when XBGR/ABGR DMA-BUFs
  are not supported (vmwgfx): blur failed and logged an error every frame

* Fri Jul 10 2026 kaleedtc <kaleed@tutanota.com> - 0.5-3
- Update to upstream release 0.5
- Update %files and %files devel sections to use %{version} macro
- Fix missing file errors for libscenefx 0.5 shared libraries
