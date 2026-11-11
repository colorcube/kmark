#!/usr/bin/env bash
# Remove a local KMark installation from ${PREFIX:-$HOME/.local}.
set -euo pipefail

PREFIX="${PREFIX:-${HOME}/.local}"

echo "==> Removing KMark from ${PREFIX} ..."
rm -f "${PREFIX}/bin/kmark"
rm -f "${PREFIX}/bin/kmark-cli"
rm -f "${PREFIX}/share/applications/de.colorcube.kmark.desktop" "${PREFIX}/share/applications/org.kde.kmark.desktop"
rm -f "${PREFIX}/share/icons/hicolor/scalable/apps/de.colorcube.kmark.svg" "${PREFIX}/share/icons/hicolor/scalable/apps/org.kde.kmark.svg"
rm -f "${PREFIX}/share/metainfo/de.colorcube.kmark.metainfo.xml" "${PREFIX}/share/metainfo/org.kde.kmark.metainfo.xml"
rm -rf "${PREFIX}/share/kxmlgui5/kmark"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "${PREFIX}/share/applications" >/dev/null 2>&1 || true
fi

if command -v kbuildsycoca6 >/dev/null 2>&1; then
    kbuildsycoca6 --noincremental >/dev/null 2>&1 || true
fi

echo "KMark uninstalled from ${PREFIX}."
