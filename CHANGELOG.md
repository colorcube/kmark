# Changelog

All notable changes to **KMark** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.3] - 2026-11-20

### Added
- **Format Markdown Document (`Format -> Format Markdown Document` / `Markdown neu formatieren`, `Ctrl+Shift+F`, `kmark-cli format`)**:
  - Losslessly normalizes blank lines around headings (`MD022`), lists (`MD032`), fenced code blocks (`MD031`), blockquotes, and tables (`MD058`), aligns GFM table columns, renumbers ordered lists, collapses duplicate blank lines (`MD012`), and strips accidental trailing spaces while keeping YAML frontmatter and code-fence contents untouched (fully undoable with `Ctrl+Z`).
  - Applying `H1`–`H6` (`Ctrl+1`–`Ctrl+6` / toolbar buttons) now automatically ensures a blank line before (unless on line 1) and after the heading.

### Changed
- **Paragraph & Heading Toolbar Icon Buttons**: Replaced the toolbar heading dropdown with 7 checkable, HiDPI vector icon buttons (`¶`, `H1`–`H6`) highlighting the active block level (`Ctrl+0`–`Ctrl+6`).
- **Typographic Paragraph Spacing & WYSIWYG `Enter` / `Shift+Enter`**: Paragraphs and list boundaries now render with clear vertical paragraph spacing (`~0.58×` line height) without a dead blank cursor row between paragraphs; in Formatted (`WYSIWYG`) mode, `Enter` starts a new paragraph (`\n\n`) while `Shift+Enter` inserts a soft-spaced line break (`  \n`) inside the same paragraph.

### Fixed
- **Unindented Paragraphs After Lists (`#316`)**: Only indented lines (`>= 2` columns) continue a list item; unindented column-0 text after a list (`Absatz vorher\n- Aufzählung\nAbsatz danach`) starts a new paragraph directly without requiring a blank line (`Shift+Enter` inside a list item inserts an indented continuation line; `Enter` on an empty nested item outdents one level).

## [0.1.2] - 2026-11-18

### Added
- **Soft Wrap & Horizontal Scrolling in Source / TXT Mode**: Long lines follow *Dynamic Word Wrap* (`F10`); with wrap off the view scrolls horizontally (scrollbar, `Shift`+wheel, caret follow).
- **Ardoise Appearance Presets**: *Ardoise (Light)* and *Ardoise (Dark)*, adapted from the MIT-licensed Obsidian theme by generalentropy.
- **Heading & Inline-Code Colors**: Optional text colors for H1–H3 and inline code in *Settings -> Colors* (right-click resets to the text color).
- Double-clicking the empty tab bar area opens a new document.
- **External File Modification Detection & Kate-Style Reload**:
  - Watches all open files and their parent directories via `QFileSystemWatcher` (surviving atomic `rename` saves by `git`, `vim`, `sed -i`, or sync tools) and re-verifies open tabs on window activation and tab switches without false alarms on identical-content rewrites.
  - Displays a non-modal Kate-style inline notification banner above the editor viewport with **Reload** (*Neu laden*), **Overwrite** (*Überschreiben*) / **Save Again** (*Erneut speichern*), and **Ignore** (*Ignorieren*), highlighting edit conflicts and deleted/moved files with tab warning badges.
  - Added `File -> Reload from Disk` (`F5`), recording the pre-reload buffer in `DocumentHistory` so every reload remains undoable with `Ctrl+Z`.

### Fixed
- Bold/italic text containing an inline code span is now rendered correctly instead of showing literal `**`.
- Indented and lazy continuation lines stay inside their list item instead of becoming separate paragraphs.

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
