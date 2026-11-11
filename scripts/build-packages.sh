#!/usr/bin/env bash
# Build release distribution packages (.tar.gz, .deb, .rpm) into dist/
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
BUILD_DIR="${BUILD_DIR:-${PROJECT_DIR}/build-pkg}"
DIST_DIR="${DIST_DIR:-${PROJECT_DIR}/dist}"
CARGO_OFFLINE="${KMARK_CARGO_OFFLINE:-OFF}"

mkdir -p "${DIST_DIR}"

echo "==> Configuring CMake release build in ${BUILD_DIR} ..."
cmake -S "${PROJECT_DIR}" -B "${BUILD_DIR}" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX=/usr \
    -DKMARK_CARGO_OFFLINE="${CARGO_OFFLINE}"

echo "==> Compiling KMark + kmark-cli ..."
cmake --build "${BUILD_DIR}" -j "$(nproc)"

# 1. Portable Linux binary tarball (.tar.gz)
echo "==> Building TGZ package via CPack ..."
cpack --config "${BUILD_DIR}/CPackConfig.cmake" -G TGZ -B "${DIST_DIR}"

# 2. Debian / Ubuntu (.deb) via CPack or portable ar/tar builder
echo "==> Building DEB package ..."
if command -v dpkg-deb >/dev/null 2>&1; then
    cpack --config "${BUILD_DIR}/CPackConfig.cmake" -G DEB -B "${DIST_DIR}" || \
        BUILD_DIR="${BUILD_DIR}" OUT_DIR="${DIST_DIR}" "${PROJECT_DIR}/packaging/deb/build-deb.sh"
elif command -v ar >/dev/null 2>&1; then
    BUILD_DIR="${BUILD_DIR}" OUT_DIR="${DIST_DIR}" "${PROJECT_DIR}/packaging/deb/build-deb.sh"
else
    echo "    [skip] Neither dpkg-deb nor ar found; skipping .deb build."
fi

# 3. Fedora / RHEL / openSUSE (.rpm) if rpmbuild is installed
if command -v rpmbuild >/dev/null 2>&1; then
    echo "==> Building RPM package via CPack ..."
    cpack --config "${BUILD_DIR}/CPackConfig.cmake" -G RPM -B "${DIST_DIR}"
else
    echo "    [skip] rpmbuild not found on host; RPM spec ready at packaging/rpm/kmark.spec (and built in CI)."
fi

# Remove temporary _CPack_Packages staging directory inside dist/
rm -rf "${DIST_DIR}/_CPack_Packages"

# Generate SHA256SUMS
(
    cd "${DIST_DIR}"
    shopt -s nullglob
    files=( *.tar.gz *.deb *.rpm )
    if (( ${#files[@]} > 0 )); then
        sha256sum "${files[@]}" > SHA256SUMS
    fi
)

echo ""
echo "==> Release artifacts in ${DIST_DIR}:"
ls -lh "${DIST_DIR}"
