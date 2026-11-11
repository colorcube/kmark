cmake_minimum_required(VERSION 3.22)
project(kmark VERSION 0.1.0 DESCRIPTION "Native KDE Plasma WYSIWYG Markdown & Text Editor" LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)

include(GNUInstallDirs)

option(KMARK_CARGO_OFFLINE "Pass --offline to Cargo (for sandboxed/offline builds)" OFF)
option(KMARK_BUILD_CLI "Build and install headless kmark-cli utility" ON)

# 1. Qt6 (Required)
find_package(Qt6 6.4 REQUIRED COMPONENTS Core Gui Widgets PrintSupport)

# 2. KDE Frameworks 6 (Optional — enables KXmlGuiWindow, KAboutData, standard KDE shortcuts/dialogs)
find_package(KF6 6.0 QUIET COMPONENTS XmlGui ConfigWidgets CoreAddons I18n)
if(KF6XmlGui_FOUND AND KF6CoreAddons_FOUND)
    message(STATUS "KMark: KDE Frameworks 6 found — building with native KXmlGuiWindow integration")
    set(KMARK_USE_KF6 TRUE)
else()
    message(STATUS "KMark: KF6 dev packages not found — building in pure Qt6/Breeze mode")
    set(KMARK_USE_KF6 FALSE)
endif()

# 3. Rust FFI static library (crates/ffi -> libkmark_ffi.a) & optional kmark-cli
find_program(CARGO_EXECUTABLE cargo REQUIRED)

if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    set(CARGO_PROFILE_ARGS "")
    set(CARGO_TARGET_SUBDIR "debug")
else()
    set(CARGO_PROFILE_ARGS "--release")
    set(CARGO_TARGET_SUBDIR "release")
endif()

set(CARGO_EXTRA_ARGS "")
if(KMARK_CARGO_OFFLINE)
    list(APPEND CARGO_EXTRA_ARGS "--offline")
endif()

set(KMARK_CARGO_TARGET_DIR "${CMAKE_BINARY_DIR}/cargo-target")
set(KMARK_FFI_LIB "${KMARK_CARGO_TARGET_DIR}/${CARGO_TARGET_SUBDIR}/libkmark_ffi.a")
set(KMARK_CLI_BIN "${KMARK_CARGO_TARGET_DIR}/${CARGO_TARGET_SUBDIR}/kmark-cli")

file(GLOB_RECURSE KMARK_RUST_SOURCES CONFIGURE_DEPENDS
    "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml"
    "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.lock"
    "${CMAKE_CURRENT_SOURCE_DIR}/crates/*.rs"
    "${CMAKE_CURRENT_SOURCE_DIR}/crates/*.toml"
    "${CMAKE_CURRENT_SOURCE_DIR}/apps/*.rs"
    "${CMAKE_CURRENT_SOURCE_DIR}/apps/*.toml"
)

if(KMARK_BUILD_CLI)
    set(CARGO_PKGS -p kmark-ffi -p kmark-cli)
    set(CARGO_OUTPUTS "${KMARK_FFI_LIB}" "${KMARK_CLI_BIN}")
else()
    set(CARGO_PKGS -p kmark-ffi)
    set(CARGO_OUTPUTS "${KMARK_FFI_LIB}")
endif()

add_custom_command(
    OUTPUT ${CARGO_OUTPUTS}
    COMMAND "${CARGO_EXECUTABLE}" build ${CARGO_PKGS} ${CARGO_PROFILE_ARGS} ${CARGO_EXTRA_ARGS} --target-dir "${KMARK_CARGO_TARGET_DIR}"
    DEPENDS ${KMARK_RUST_SOURCES}
    WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
    COMMENT "Building safe Rust engine (kmark-ffi / kmark-cli)…"
    USES_TERMINAL
)

add_custom_target(kmark_rust_ffi ALL DEPENDS ${CARGO_OUTPUTS})

# 4. Native Qt6 / KDE Plasma GUI executable
add_executable(kmark
    qt-shell/src/main.cpp
    qt-shell/src/I18n.h
    qt-shell/src/PaletteBridge.h
    qt-shell/src/AppSettings.h
    qt-shell/src/AppSettings.cpp
    qt-shell/src/SettingsDialog.h
    qt-shell/src/SettingsDialog.cpp
    qt-shell/src/EditorViewport.h
    qt-shell/src/EditorViewport.cpp
    qt-shell/src/MainWindow.h
    qt-shell/src/MainWindow.cpp
    qt-shell/resources/kmark.qrc
)

add_dependencies(kmark kmark_rust_ffi)

target_include_directories(kmark PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/crates/ffi/include"
    "${CMAKE_CURRENT_SOURCE_DIR}/qt-shell/src"
)

target_compile_options(kmark PRIVATE -Wall -Wextra -Wpedantic)

target_link_libraries(kmark PRIVATE
    "${KMARK_FFI_LIB}"
    Qt6::Core
    Qt6::Gui
    Qt6::Widgets
    Qt6::PrintSupport
    dl
    pthread
    m
)

if(KMARK_USE_KF6)
    target_compile_definitions(kmark PRIVATE KMARK_HAVE_KF6=1)
    target_link_libraries(kmark PRIVATE
        KF6::XmlGui
        KF6::ConfigWidgets
        KF6::CoreAddons
        KF6::I18n
    )
endif()

# 5. Desktop, Icon, KXmlGui & AppStream Installation Rules
install(TARGETS kmark RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}")
if(KMARK_BUILD_CLI)
    install(PROGRAMS "${KMARK_CLI_BIN}" DESTINATION "${CMAKE_INSTALL_BINDIR}")
endif()
install(FILES qt-shell/resources/de.colorcube.kmark.desktop
        DESTINATION "${CMAKE_INSTALL_DATADIR}/applications")
install(FILES qt-shell/resources/icons/de.colorcube.kmark.svg
        DESTINATION "${CMAKE_INSTALL_DATADIR}/icons/hicolor/scalable/apps")
install(FILES qt-shell/resources/kmarkui.rc
        DESTINATION "${CMAKE_INSTALL_DATADIR}/kxmlgui5/kmark")
install(FILES packaging/de.colorcube.kmark.metainfo.xml
        DESTINATION "${CMAKE_INSTALL_DATADIR}/metainfo")

# 6. CPack Packaging (.deb, .rpm, .tar.gz)
set(CPACK_PACKAGE_NAME "kmark")
set(CPACK_PACKAGE_VENDOR "colorcube")
set(CPACK_PACKAGE_VERSION "${PROJECT_VERSION}")
set(CPACK_PACKAGE_DESCRIPTION_SUMMARY "Native KDE Plasma WYSIWYG Markdown & Text Editor")
set(CPACK_PACKAGE_CONTACT "René Fritz <rf@colorcube.de>")
set(CPACK_RESOURCE_FILE_LICENSE "${CMAKE_CURRENT_SOURCE_DIR}/LICENSE")
set(CPACK_RESOURCE_FILE_README "${CMAKE_CURRENT_SOURCE_DIR}/README.md")
set(CPACK_PACKAGING_INSTALL_PREFIX "/usr")
set(CPACK_GENERATOR "TGZ;DEB;RPM")

# Debian / Ubuntu (.deb)
set(CPACK_DEBIAN_FILE_NAME DEB-DEFAULT)
set(CPACK_DEBIAN_PACKAGE_SECTION "editors")
set(CPACK_DEBIAN_PACKAGE_PRIORITY "optional")
set(CPACK_DEBIAN_PACKAGE_SHLIBDEPS ON)
set(CPACK_DEBIAN_PACKAGE_HOMEPAGE "https://github.com/colorcube/kmark")

# Fedora / openSUSE / RHEL (.rpm)
set(CPACK_RPM_FILE_NAME RPM-DEFAULT)
set(CPACK_RPM_PACKAGE_LICENSE "GPL-3.0-or-later")
set(CPACK_RPM_PACKAGE_GROUP "Applications/Editors")
set(CPACK_RPM_PACKAGE_URL "https://github.com/colorcube/kmark")
set(CPACK_RPM_PACKAGE_AUTOREQPROV ON)
list(APPEND CPACK_RPM_EXCLUDE_FROM_AUTO_FILELIST_ADDITION
    "/usr/share/applications"
    "/usr/share/icons"
    "/usr/share/icons/hicolor"
    "/usr/share/icons/hicolor/scalable"
    "/usr/share/icons/hicolor/scalable/apps"
    "/usr/share/metainfo"
    "/usr/share/kxmlgui5"
)

include(CPack)
