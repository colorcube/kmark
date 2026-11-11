#!/usr/bin/env bash
# Build a Debian/Ubuntu .deb package from a compiled CMake install tree.
# Uses dpkg-deb when available, or standard GNU ar + tar (deb 2.0 format) on non-Debian hosts.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
BUILD_DIR="${BUILD_DIR:-${PROJECT_DIR}/build-pkg}"
OUT_DIR="${OUT_DIR:-${PROJECT_DIR}/dist}"
VERSION="${VERSION:-0.1.1}"

detect_deb_arch() {
    if command -v dpkg >/dev/null 2>&1; then
        dpkg --print-architecture
        return
    fi
    case "$(uname -m)" in
        x86_64)  echo "amd64" ;;
        aarch64) echo "arm64" ;;
        *)       uname -m ;;
    esac
}
ARCH="${ARCH:-$(detect_deb_arch)}"

WORK_DIR="$(mktemp -d)"
trap 'rm -rf "${WORK_DIR}"' EXIT

STAGE_DIR="${WORK_DIR}/root"
mkdir -p "${STAGE_DIR}"
DESTDIR="${STAGE_DIR}" cmake --install "${BUILD_DIR}" --prefix /usr >/dev/null

INSTALLED_KB="$(du -sk "${STAGE_DIR}/usr" | awk '{print $1}')"
mkdir -p "${OUT_DIR}"
DEB_FILE="${OUT_DIR}/kmark_${VERSION}_${ARCH}.deb"
rm -f "${DEB_FILE}"

CONTROL_CONTENT="Package: kmark
Version: ${VERSION}
Section: editors
Priority: optional
Architecture: ${ARCH}
Installed-Size: ${INSTALLED_KB}
Maintainer: René Fritz <rf@colorcube.de>
Homepage: https://github.com/colorcube/kmark
Depends: libc6, libstdc++6, libqt6core6 | libqt6core6t64, libqt6gui6 | libqt6gui6t64, libqt6widgets6 | libqt6widgets6t64, libqt6printsupport6 | libqt6printsupport6t64, libqt6network6 | libqt6network6t64
Description: Native WYSIWYG Markdown and plain-text editor for KDE Plasma
 KMark combines distraction-free WYSIWYG Markdown typography with native KDE
 Plasma integration (Qt6 / KDE Frameworks 6) over a memory-safe Rust Rope and
 virtualized 2D layout engine.
"

if command -v dpkg-deb >/dev/null 2>&1; then
    mkdir -p "${STAGE_DIR}/DEBIAN"
    printf '%s' "${CONTROL_CONTENT}" > "${STAGE_DIR}/DEBIAN/control"
    chmod 0644 "${STAGE_DIR}/DEBIAN/control"
    dpkg-deb --root-owner-group --build "${STAGE_DIR}" "${DEB_FILE}"
else
    CTRL_DIR="${WORK_DIR}/control"
    mkdir -p "${CTRL_DIR}"
    printf '%s' "${CONTROL_CONTENT}" > "${CTRL_DIR}/control"
    (cd "${STAGE_DIR}" && find usr -type f -exec md5sum {} +) > "${CTRL_DIR}/md5sums"
    chmod 0644 "${CTRL_DIR}/control" "${CTRL_DIR}/md5sums"

    printf '2.0\n' > "${WORK_DIR}/debian-binary"
    tar --numeric-owner --owner=0 --group=0 -C "${CTRL_DIR}" -czf "${WORK_DIR}/control.tar.gz" ./control ./md5sums
    tar --numeric-owner --owner=0 --group=0 -C "${STAGE_DIR}" -czf "${WORK_DIR}/data.tar.gz" ./usr
    ar rcs "${DEB_FILE}" "${WORK_DIR}/debian-binary" "${WORK_DIR}/control.tar.gz" "${WORK_DIR}/data.tar.gz"
fi

echo "Built Debian package: ${DEB_FILE}"
