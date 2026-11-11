#!/usr/bin/env bash
# Install KMark locally into ${PREFIX:-$HOME/.local} with desktop entry,
# scalable SVG icon, AppStream metainfo, KXmlGui RC, and cache refresh.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
PREFIX="${PREFIX:-${HOME}/.local}"
BUILD_DIR="${BUILD_DIR:-${PROJECT_DIR}/build-local}"

echo "==> Building KMark (Release) in ${BUILD_DIR} ..."
cmake -S "${PROJECT_DIR}" -B "${BUILD_DIR}" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="${PREFIX}"
cmake --build "${BUILD_DIR}" -j "$(nproc)"

echo "==> Installing KMark into ${PREFIX} ..."
rm -f "${PREFIX}/share/applications/org.kde.kmark.desktop" \
      "${PREFIX}/share/icons/hicolor/scalable/apps/org.kde.kmark.svg" \
      "${PREFIX}/share/metainfo/org.kde.kmark.metainfo.xml"
cmake --install "${BUILD_DIR}"

# Refresh Freedesktop & KDE Plasma caches so KMark appears immediately in KRunner / Kickoff
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "${PREFIX}/share/applications" >/dev/null 2>&1 || true
fi

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "${PREFIX}/share/icons/hicolor" >/dev/null 2>&1 || true
fi

if command -v kbuildsycoca6 >/dev/null 2>&1; then
    kbuildsycoca6 --noincremental >/dev/null 2>&1 || true
fi

echo ""
echo "KMark installed successfully into ${PREFIX}!"
echo "  Binary:   ${PREFIX}/bin/kmark"
echo "  CLI:      ${PREFIX}/bin/kmark-cli"
echo "  Desktop:  ${PREFIX}/share/applications/de.colorcube.kmark.desktop"
echo "  Icon:     ${PREFIX}/share/icons/hicolor/scalable/apps/de.colorcube.kmark.svg"
echo ""
if [[ ":${PATH}:" != *":${PREFIX}/bin:"* ]]; then
    echo "NOTE: Add ${PREFIX}/bin to your PATH to launch 'kmark' from any terminal."
fi
