Name:           kmark
Version:        0.1.1
Release:        1%{?dist}
Summary:        Native WYSIWYG Markdown and plain-text editor for KDE Plasma
License:        GPL-3.0-or-later
URL:            https://github.com/colorcube/kmark
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  cmake >= 3.22
BuildRequires:  gcc-c++
BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  qt6-qtbase-devel
BuildRequires:  kf6-kxmlgui-devel
BuildRequires:  kf6-kconfigwidgets-devel
BuildRequires:  kf6-kcoreaddons-devel
BuildRequires:  kf6-ki18n-devel

%description
KMark is a fast, native WYSIWYG Markdown and plain-text editor for Linux and
KDE Plasma. It combines a memory-safe Rust core (Copy-on-Write Rope, incremental
CommonMark/GFM parser, virtualized 2D layout engine) with a native Qt6 / KDE
Frameworks 6 shell.

%prep
%autosetup

%build
%cmake -DCMAKE_BUILD_TYPE=Release
%cmake_build

%install
%cmake_install

%files
%license LICENSE
%doc README.md CHANGELOG.md
%{_bindir}/kmark
%{_bindir}/kmark-cli
%{_datadir}/applications/de.colorcube.kmark.desktop
%{_datadir}/icons/hicolor/scalable/apps/de.colorcube.kmark.svg
%{_datadir}/kxmlgui5/kmark/kmarkui.rc
%{_datadir}/metainfo/de.colorcube.kmark.metainfo.xml

%changelog
* Thu Nov 12 2026 René Fritz <rf@colorcube.de> - 0.1.1-1
- Open files as tabs in existing instance via single-instance IPC (-n/--new-window override)

* Wed Nov 11 2026 René Fritz <rf@colorcube.de> - 0.1.0-1
- Initial release 0.1.0 for Linux and KDE Plasma
