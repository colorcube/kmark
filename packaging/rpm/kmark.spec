Name:           kmark
Version:        0.2.0
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
* Sat Nov 21 2026 René Fritz <rf@colorcube.de> - 0.2.0-1
- Automatic bare URL and autolink recognition across all view modes with Ctrl+Click, tooltip, and context menu
- Convert console box-drawing, CSV, TSV, and clipboard HTML tables to Markdown tables (Ctrl+Alt+T / Ctrl+Alt+V)

* Fri Nov 20 2026 René Fritz <rf@colorcube.de> - 0.1.3-1
- Checkable Paragraph and H1-H6 toolbar icon buttons
- Typographic paragraph spacing and WYSIWYG Enter vs Shift+Enter
- Lossless Markdown document formatter (Ctrl+Shift+F and kmark-cli format)
- Unindented column-0 paragraphs after lists

* Wed Nov 18 2026 René Fritz <rf@colorcube.de> - 0.1.2-1
- Soft wrap / horizontal scrolling in Source and TXT mode
- Ardoise appearance presets and heading/inline-code colors
- External file change detection with reload
- Fix emphasis containing inline code and list continuation lines

* Thu Nov 12 2026 René Fritz <rf@colorcube.de> - 0.1.1-1
- Open files as tabs in existing instance via single-instance IPC (-n/--new-window override)

* Wed Nov 11 2026 René Fritz <rf@colorcube.de> - 0.1.0-1
- Initial release 0.1.0 for Linux and KDE Plasma
