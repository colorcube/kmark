# Changelog

All notable changes to **KMark** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2026-11-12

### Added
- **Single-Instance Tab Opening**:
  - Opening additional `.md` / `.txt` files from Dolphin (`Exec=kmark %U`) or the command line now opens them as tabs in the already-running KMark window and activates the window on both Wayland (`XDG_ACTIVATION_TOKEN`) and X11.
  - Added `-n` / `--new-window` CLI flag, a *New Window* / *Neues Fenster* desktop action in `de.colorcube.kmark.desktop`, and a *Settings -> Editor & View* toggle (*Open files in existing window as tabs*).

## [0.1.0] - 2026-11-11

### Added
- **Single-Buffer Projected Editor (`L0`–`L5`)**:
  - Persistent Copy-on-Write UTF-8 `Rope` (`kmark-buffer`) with encoding detection (`UTF-8`, `UTF-16 LE/BE`, `Windows-1252`), atomic file writes, and coalesced Undo/Redo.
  - Incremental CommonMark + GitHub Flavored Markdown parser (`kmark-syntax`) with exact byte offsets, 2D table AST, heading outline, and standalone HTML5 export.
  - Virtualized 2D layout engine (`kmark-layout`) supporting **Formatted (`WYSIWYG`)**, **Live Preview**, **Markdown Source**, and **Plain Text (`TXT`)** modes.
  - Multi-tab document session (`kmark-engine`) with armed inline formatting, smart list/quote continuation, RegEx Find & Replace, and crash recovery drafts.
  - Native C++20 Qt6 / KDE Frameworks 6 desktop application (`qt-shell`) with `KXmlGuiWindow`, Breeze Light/Dark palette sync, dockable *Outline* & *Folder Tree*, and PDF/CUPS print preview.
- **Clickable Code Block Language & Callout Type Selectors**:
  - Top-right `rust ▾` / `Code ▾` badge on every fenced code block in Formatted and Live Preview modes (`Ctrl+Alt+L`), plus a matching clickable `Note ▾` / `Hinweis ▾` badge on every Callout (`[!NOTE]`, `[!TIP]`, `[!IMPORTANT]`, `[!WARNING]`, `[!CAUTION]`) to switch Callout types or convert to a plain blockquote.
- **Folder Tree Reveal & Open Recent Files**:
  - Full filesystem navigation (`↑` Parent, `~` Home, context menu *Set as Root Folder*) and a PhpStorm-style top-right **Select Opened File** action in the Folder Tree dock (`F7`), plus persistent **Open Recent** (`File -> Open Recent`) history.
- **Unified Settings Dialog, Appearance Presets & Persistent State**:
  - Persistent `QSettings` (`~/.config/colorcube/kmark.conf`) storage for fonts, colors, heading indicator style, default view mode, word wrap, line numbers, focus/typewriter modes, recent files, folder tree root, and window/dock state.
  - Unified KDE Settings Dialog (`Settings -> Configure KMark…`, `Ctrl+Shift+,`) with live WYSIWYG preview card, body/code font selectors, **Heading Indicators** (*Slanted Bars*, *`#` Hash*, *None*), and 8 customizable editor color swatches.
  - 6 built-in Appearance Presets (*System (KDE Breeze)*, *Modern*, *Classic* / *Klassisch*, *Calm* / *Ruhig*, *Dark (Night Studio)*, *Technical*) plus custom user presets selectable directly from `Settings -> Appearance Presets`.
- **Contextual Table Toolbar & Balanced 2D Table Column Sizing**:
  - Automatically displays table row, column, and alignment actions in the main toolbar whenever the caret enters a Markdown table, highlighting the active column alignment (`Left` / `Center` / `Right`).
  - `min-content`-aware column width solver with damped flex distribution so single-word columns (e.g. *Bereich* / *Scope*) never break mid-word alongside wide text columns.
- **Inline Images, Captions, Relative Sizing & Context Menu**:
  - Drop images or paste clipboard screenshots directly into the viewport; supports CommonMark image captions (`![alt](img.png "Caption")`), relative widths (`{width=50%}`), HTML5 figure/caption export, and a right-click context menu (*Add/Edit Caption*, *Replace Image*, *Size*).
- **Bilingual UI & Built-in Documentation (`English` & `Deutsch`)**:
  - Automatic KDE/system locale detection, **Markdown Quick Reference** (`F1`), and **Markdown Complete Reference** (`Shift+F1`).
- **Local Install & Linux Release Packaging**:
  - `scripts/install-local.sh`, AppStream metadata, CPack/native `.deb`, `.rpm`, and `.tar.gz` packaging, plus GitHub Actions CI & Release workflows.
