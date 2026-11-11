/// UI language selector shared between engine built-in documents and the Qt/KDE shell.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum UiLanguage {
    English,
    #[default]
    German,
}

impl UiLanguage {
    #[must_use]
    pub const fn is_german(self) -> bool {
        matches!(self, Self::German)
    }

    #[must_use]
    pub fn untitled_title(self, index_num: usize) -> String {
        match self {
            Self::German => format!("Unbenannt {index_num}"),
            Self::English => format!("Untitled {index_num}"),
        }
    }

    #[must_use]
    pub const fn quick_guide_title(self) -> &'static str {
        match self {
            Self::German => "Markdown-Kurzreferenz",
            Self::English => "Markdown Quick Reference",
        }
    }

    #[must_use]
    pub const fn quick_guide_markdown(self) -> &'static str {
        match self {
            Self::German => MARKDOWN_GUIDE_DE,
            Self::English => MARKDOWN_GUIDE_EN,
        }
    }
}

/// Built-in German Markdown quick-reference and live feature showcase (`Hilfe -> Markdown-Kurzreferenz`).
pub const MARKDOWN_GUIDE_DE: &str = r#"# Willkommen bei KMark

**KMark** ist ein schneller, nativer **KDE-Plasma-Markdown- & Texteditor** in Rust und Qt6/KF6. Alle vier Ansichtsmodi arbeiten verlustfrei auf *demselben* UTF-8-Textpuffer:

1. **Formatiert (WYSIWYG)** — Direkt im gesetzten Dokument schreiben; Markdown-Steuerzeichen bleiben unsichtbar.
2. **Live-Vorschau** — Formatierte Darstellung im gesamten Dokument, während der aktive Absatz die Markdown-Syntax einblendet.
3. **Quelltext** — Klassische Markdown-Ansicht mit Syntaxhervorhebung und Zeilennummern (`F11`).
4. **Reintext (TXT)** — Schneller Editor-Modus ohne Markdown-Formatierung wie KWrite/Kate.

---

## Wichtige Tastenkürzel

| Aktion | Tastenkürzel | Markdown-Syntax |
| --- | :---: | --- |
| **Fett** | <kbd>Ctrl+B</kbd> | `**Text**` |
| *Kursiv* | <kbd>Ctrl+I</kbd> | `*Text*` |
| ~~Durchgestrichen~~ | <kbd>Ctrl+Shift+X</kbd> | `~~Text~~` |
| ==Hervorhebung== | <kbd>Ctrl+Shift+H</kbd> | `==Text==` |
| `Inline-Code` | <kbd>Ctrl+E</kbd> | `` `Code` `` |
| Überschrift 1–6 | <kbd>Ctrl+1</kbd> … <kbd>Ctrl+6</kbd> | `# ` … `###### ` |
| Absatz (Normaltext) | <kbd>Ctrl+0</kbd> | Entfernt `#` |
| Link einfügen | <kbd>Ctrl+K</kbd> | `[Titel](https://kde.org)` |
| Suchen / Ersetzen | <kbd>Ctrl+F</kbd> / <kbd>Ctrl+R</kbd> | Kate-Suchleiste unten |

> [!TIP] Direkt lostippen & Bilder per Drag & Drop einfügen
> Drückst du außerhalb eines Wortes <kbd>Ctrl+B</kbd>, merkt sich KMark den Stil („Armed Formatting“). Bilder aus Dolphin lassen sich direkt per **Drag & Drop** in den Editor ziehen!

## Aufgabenlisten & Aufzählungen

- [x] Persistenter Copy-on-Write-`Rope`-Textpuffer (`O(log N)`)
- [x] Echtes 2D-Tabellen-Grid direkt über Byte-Bereiche des Puffers
- [ ] Klick auf das Sprach-Badge `rust ▾` rechts oben im Codeblock ausprobieren

> [!NOTE] Hinweis zu Tabellen & Codeblöcken
> Mit <kbd>Tab</kbd> und <kbd>Shift+Tab</kbd> springst du von Zelle zu Zelle. Die Sprache eines Codeblocks änderst du im Formatierten Modus einfach per Klick auf das Badge rechts oben (z. B. `rust ▾`).

## Codeblöcke

```rust
fn begruessung(name: &str) -> String {
    format!("Hallo {name}, viel Freude beim Schreiben in KDE Plasma!")
}
```
"#;

/// Built-in English Markdown quick-reference and live feature showcase (`Help -> Markdown Quick Reference`).
pub const MARKDOWN_GUIDE_EN: &str = r#"# Welcome to KMark

**KMark** is a fast, native **KDE Plasma Markdown & Plain-Text Editor** powered by a safe Rust engine and Qt6/KF6. All four view modes operate losslessly on the *exact same* UTF-8 buffer:

1. **Formatted (WYSIWYG)** — Write directly inside clean typography while Markdown delimiters stay hidden.
2. **Live Preview** — Typeset rendering across the document while revealing raw Markdown on the active line.
3. **Source Code** — Classic monospace Markdown editor with heading highlights and line numbers (`F11`).
4. **Plain Text (TXT)** — Distraction-free KWrite/Kate-style plain-text editing at million-line scale.

---

## Essential Shortcuts

| Action | Shortcut | Markdown Syntax |
| --- | :---: | --- |
| **Bold** | <kbd>Ctrl+B</kbd> | `**Text**` |
| *Italic* | <kbd>Ctrl+I</kbd> | `*Text*` |
| ~~Strikethrough~~ | <kbd>Ctrl+Shift+X</kbd> | `~~Text~~` |
| ==Highlight== | <kbd>Ctrl+Shift+H</kbd> | `==Text==` |
| `Inline Code` | <kbd>Ctrl+E</kbd> | `` `Code` `` |
| Heading 1–6 | <kbd>Ctrl+1</kbd> … <kbd>Ctrl+6</kbd> | `# ` … `###### ` |
| Paragraph (Body) | <kbd>Ctrl+0</kbd> | Strips `#` prefix |
| Insert Link | <kbd>Ctrl+K</kbd> | `[Title](https://kde.org)` |
| Find / Replace | <kbd>Ctrl+F</kbd> / <kbd>Ctrl+R</kbd> | Bottom Kate search bar |

> [!TIP] Armed Formatting & Image Drag-and-Drop
> Press <kbd>Ctrl+B</kbd> before typing a word — KMark arms the style without inserting empty `****` markers. Drop images straight from Dolphin onto the canvas to embed and preview them inline!

## Task Lists & Outlines

- [x] Persistent copy-on-write `Rope` text buffer (`O(log N)` edits)
- [x] Native 2D table grid projected directly over buffer byte slices
- [ ] Click the `rust ▾` badge on the code block below to switch its language

> [!NOTE] Tables & Code Block Languages
> Press <kbd>Tab</kbd> and <kbd>Shift+Tab</kbd> to move across table cells (pressing <kbd>Tab</kbd> in the last cell appends a new row). Click the top-right language badge (`rust ▾`) on any code block to change its language!

## Fenced Code Blocks

```rust
fn greet(user: &str) -> String {
    format!("Hello {user}, enjoy writing Markdown natively on KDE Plasma!")
}
```
"#;
