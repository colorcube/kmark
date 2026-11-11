use crate::sample::UiLanguage;

impl UiLanguage {
    #[must_use]
    pub const fn complete_reference_title(self) -> &'static str {
        match self {
            Self::German => "Markdown-Langreferenz",
            Self::English => "Markdown Complete Reference",
        }
    }

    #[must_use]
    pub const fn complete_reference_markdown(self) -> &'static str {
        match self {
            Self::German => MARKDOWN_REFERENCE_DE,
            Self::English => MARKDOWN_REFERENCE_EN,
        }
    }
}

/// Comprehensive German Markdown reference manual (`Hilfe -> Markdown-Langreferenz`, `Shift+F1`).
pub const MARKDOWN_REFERENCE_DE: &str = r#"# Markdown-Langreferenz — KMark Handbuch

Dieses Nachschlagewerk dokumentiert die vollständige **CommonMark- und GitHub-Flavored-Markdown-(GFM)-Syntax**, die von **KMark** unterstützt wird, sowie alle Komfortfunktionen des Editors im **Formatierten Modus (WYSIWYG)** und in der **Live-Vorschau**.

---

## 1. Grundprinzip: Ein Puffer, vier Ansichten

KMark speichert jedes Dokument als reinen Textpuffer (`Rope`). Zwischen den vier Ansichten kannst du jederzeit ohne Konvertierungsverluste wechseln:

| Modus | Kurzwahl | Verhalten |
| :--- | :---: | :--- |
| **Formatiert (WYSIWYG)** | <kbd>Ctrl+Alt+1</kbd> | Alle Steuerzeichen (`**`, `#`, `` ` ``) sind ausgeblendet; direktes Schreiben im Layout. |
| **Live-Vorschau** | <kbd>Ctrl+Alt+2</kbd> | Wie Formatiert, blendet jedoch in der aktiven Cursor-Zeile die Roh-Syntax ein. |
| **Quelltext** | <kbd>Ctrl+Alt+3</kbd> | Monospace-Quelltextansicht mit Überschriften-Hervorhebung und Zeilennummern. |
| **Reintext (TXT)** | <kbd>Ctrl+Alt+4</kbd> | Reiner Texteditor ohne Markdown-Interpretation (Standard für `.txt`, `.log`, `.ini`). |

---

## 2. Überschriften & Dokumentgliederung

KMark unterstützt sowohl **ATX-Überschriften** (`#` bis `######`) als auch zweizeilige **Setext-Überschriften** (`===` für Ebene 1, `---` für Ebene 2). Alle Überschriften erscheinen automatisch im Dock **Gliederung** (<kbd>F9</kbd>).

| Ebene | Markdown-Eingabe | Tastenkürzel |
| :--- | :--- | :---: |
| Überschrift 1 | `# Haupttitel` | <kbd>Ctrl+1</kbd> |
| Überschrift 2 | `## Abschnitt` | <kbd>Ctrl+2</kbd> |
| Überschrift 3 | `### Unterabschnitt` | <kbd>Ctrl+3</kbd> |
| Überschrift 4 | `#### Detailpunkt` | <kbd>Ctrl+4</kbd> |
| Überschrift 5 | `##### Notiztitel` | <kbd>Ctrl+5</kbd> |
| Überschrift 6 | `###### Kleintitel` | <kbd>Ctrl+6</kbd> |
| Normaler Absatz | *(entfernt führende `#`)* | <kbd>Ctrl+0</kbd> |

> [!TIP] Schräge Ebenen-Balken
> Unter `Einstellungen -> Überschriften-Markierung` wählst du zwischen den schrägen Ebenen-Balken (`1..6` Striche links neben der Überschrift), dezenten `#`-Symbolen oder gar keiner Markierung.

---

## 3. Absätze, Zeilenumbrüche & Trennlinien

- **Absatzwechsel**: Eine **Leerzeile** zwischen zwei Textblöcken erzeugt einen neuen Absatz.
- **Zeilenumbruch**: Im Editor bricht <kbd>Enter</kbd> direkt in die nächste Zeile um; in Listen und Zitaten setzt <kbd>Enter</kbd> das Präfix automatisch fort.
- **Horizontale Trennlinie**: Drei oder mehr Bindestriche `---`, Sternchen `***` oder Unterstriche `___` allein auf einer Zeile erzeugen eine horizontale Linie (`Format -> Trennlinie`).

---

## 4. Inline-Formatierungen & Typografie

Inline-Stile lassen sich beliebig verschachteln — etwa `**fett und *gleichzeitig kursiv***` (**fett und *gleichzeitig kursiv***).

| Darstellung | Syntax | Tastenkürzel | Besonderheit in KMark |
| :--- | :--- | :---: | :--- |
| **Fettdruck** | `**Wort**` oder `__Wort__` | <kbd>Ctrl+B</kbd> | Unterstützt *Armed Formatting* vor dem Tippen |
| *Kursivschrift* | `*Wort*` oder `_Wort_` | <kbd>Ctrl+I</kbd> | Auch innerhalb von **Fettdruck** kombinierbar |
| ***Fett & Kursiv*** | `***Wort***` | <kbd>Ctrl+B</kbd>, <kbd>Ctrl+I</kbd> | Dreifache Begrenzer werden sauber aufgelöst |
| ~~Durchgestrichen~~ | `~~Wort~~` | <kbd>Ctrl+Shift+X</kbd> | GFM-Erweiterung für erledigte/verworfene Punkte |
| ==Textmarker== | `==Wort==` | <kbd>Ctrl+Shift+H</kbd> | Farbige Texthervorhebung (Extended Markdown) |
| `Inline-Code` | `` `Befehl` `` | <kbd>Ctrl+E</kbd> | Monospace-Pille für Befehle, Pfade & Symbole |
| Tastenkappe <kbd>Ctrl+S</kbd> | `<kbd>Ctrl+S</kbd>` | — | Gerenderte Tastenkappe mit Rahmen & Fase |
| Maskierte Zeichen \* \# \[ | `\*`, `\#`, `\[`, `` \` `` | — | Backslash `\` verhindert die Markdown-Deutung |

---

## 5. Links & Bilder (inkl. Drag & Drop)

### Hyperlinks
- **Standard-Link**: `[KDE Community](https://kde.org)` → [KDE Community](https://kde.org) (<kbd>Ctrl+K</kbd>)
- **Öffnen im Browser**: Halte <kbd>Ctrl</kbd> gedrückt und klicke auf den farbigen Linktext.

### Bilder einfügen, Bildunterschriften & Größe
- **Grundsyntax**: `![Alternativtext](pfad/zum/bild.png)` (<kbd>Ctrl+Shift+I</kbd>)
- **Bildunterschrift (Caption)**: `![Alternativtext](pfad/zum/bild.png "Abbildung 1: Beschreibung")` — wird im Formatierten Modus und in der Live-Vorschau kursiv zentriert direkt unter dem Bild dargestellt.
- **Relative Bildgröße**: `![Alternativtext](pfad/zum/bild.png "Bildunterschrift"){width=50%}` — skaliert das Bild proportional relativ zur Textspaltenbreite (z. B. `25%`, `50%`, `75%`, `100%`).
- **Kontextmenü per Rechtsklick auf ein Bild**:
  - *Bildunterschrift hinzufügen… / bearbeiten… / entfernen*
  - *Bild ersetzen…* (unter Beibehaltung von Unterschrift & Größe)
  - *Größe* (`Automatisch`, `25 %`, `33 %`, `50 %`, `66 %`, `75 %`, `100 %` oder benutzerdefinierte Prozentangabe)
- **Drag & Drop**: Ziehe Bilddateien (`.png`, `.jpg`, `.jpeg`, `.svg`, `.webp`, `.gif`, `.bmp`) aus **Dolphin** direkt an die gewünschte Stelle im Dokument:
  - Ist die aktuelle Markdown-Datei bereits gespeichert, trägt KMark automatisch einen **sauberen relativen Pfad** ein (z. B. `![diagramm](bilder/diagramm.png)`).
  - Ziehe `.md`- oder `.txt`-Dateien auf das Fenster, um sie direkt als neuen Tab zu öffnen.

---

## 6. Listen & Interaktive Aufgabenlisten

### Aufzählungslisten (<kbd>Ctrl+Shift+U</kbd>)
Beginne Zeilen mit `- `, `* ` oder `+ `. Mit <kbd>Tab</kbd> rückst du Unterpunkte um zwei Leerzeichen ein, mit <kbd>Shift+Tab</kbd> wieder aus:

- Hauptpunkt A
  - Unterpunkt A.1
  - Unterpunkt A.2
- Hauptpunkt B

### Nummerierte Listen (<kbd>Ctrl+Shift+O</kbd>)
1. Erster Schritt (<kbd>Enter</kbd> zählt automatisch mit `2. ` weiter)
2. Zweiter Schritt (nochmaliges <kbd>Enter</kbd> auf einer leeren Nummer beendet die Liste)

### Aufgabenlisten / Checklisten (<kbd>Ctrl+Shift+T</kbd>)
- [x] Entwurf verfassen (`- [x] `)
- [ ] Korrektur lesen (`- [ ] ` — im Formatierten Modus direkt per Mausklick abhakbar!)

---

## 7. Zitate & Hinweisblöcke (GitHub / KDE Alerts)

Klassische Zitate beginnen mit `> ` (<kbd>Ctrl+Shift+Q</kbd>) und können mit `>> ` verschachtelt werden:

> Gute Werkzeuge treten beim Schreiben in den Hintergrund und lassen den Gedanken den Vortritt.

Zusätzlich unterstützt KMark alle fünf farbigen **GFM-Hinweisblöcke** (`Format -> Hinweisblock einfügen`), wahlweise mit eigenem Titel in derselben Zeile. Über das **klickbare Typ-Badge rechts oben** (z. B. `Hinweis ▾`, analog zur Sprachauswahl bei Codeblöcken) wechselst du jederzeit zwischen allen fünf Callout-Typen oder wandelst den Block zurück in ein normales Zitat:

> [!NOTE]
> Hintergrundinformationen, Kontext oder ergänzende Hinweise zum Abschnitt.

> [!TIP] Eigener Tipp-Titel
> Praktische Abkürzungen, Best Practices oder Empfehlungen für den Alltag.

> [!IMPORTANT]
> Wichtige Voraussetzungen, die zwingend beachtet werden sollten.

> [!WARNING]
> Warnung vor möglichen Stolpersteinen oder Kompatibilitätsproblemen.

> [!CAUTION]
> Kritische Aktionen mit Risiko von Datenverlust.

---

## 8. Codeblöcke & Sprachauswahl im Formatierten Modus

Umzäunte Codeblöcke werden mit drei Backticks ```` ``` ```` oder drei Tilden `~~~` eingeleitet (<kbd>Ctrl+Shift+C</kbd>):

```python
def fibonacci(n: int) -> list[int]:
    a, b = 0, 1
    ergebnis = []
    for _ in range(n):
        ergebnis.append(a)
        a, b = b, a + b
    return ergebnis
```

### So wählst du die Code-Sprache im Formatierten Modus aus
1. **Per Klick auf das Sprach-Badge**: Rechts oben in jedem Codeblock siehst du eine klickbare Schaltfläche (z. B. `python ▾` oder `Code ▾`, wenn noch keine Sprache vergeben ist). Ein Klick öffnet ein Auswahlmenü mit gängigen Sprachen (`rust`, `python`, `bash`, `cpp`, `json`, `toml`, `yaml`, `html`, `sql`, …) sowie **Eigene Sprache…**.
2. **Über das Menü / Tastenkürzel**: Setze den Cursor in den Codeblock und wähle `Format -> Codeblock-Sprache ändern…` (<kbd>Ctrl+Alt+L</kbd>).
3. **In der Live-Vorschau**: Bewege den Cursor in den Codeblock — die erste Zeile ```` ```sprache ```` wird sichtbar und kann auch direkt getippt werden.

---

## 9. GFM-Tabellen (2D-Grid-Bearbeitung)

Tabellen bestehen aus einer Kopfzeile, einer Ausrichtungszeile (`:---` links, `:---:` zentriert, `---:` rechts) und beliebig vielen Datenzeilen:

| Spalte links (`:---`) | Zentriert (`:---:`) | Rechtsbündig (`---:`) |
| :--- | :---: | ---: |
| Rust `L0`–`L4` | <kbd>Tab</kbd> | 0,86 ms |
| Qt6 / KF6 `L5` | <kbd>Shift+Tab</kbd> | 60 fps |

- **Navigation**: <kbd>Tab</kbd> springt zur nächsten Zelle, <kbd>Shift+Tab</kbd> zur vorherigen.
- **Neue Zeile anfügen**: <kbd>Tab</kbd> in der letzten Zelle unten rechts hängt automatisch eine neue Tabellenzeile an.
- **Struktur-Befehle**: Über das Menü `Tabelle` fügst du Zeilen/Spalten ein, löschst sie oder änderst die Spaltenausrichtung — KMark formatiert das zugrunde liegende Markdown-Rohgitter dabei automatisch sauber ausgerichtet neu!

---

## 10. Übersicht aller Tastenkürzel

| Bereich | Funktion | Tastenkürzel |
| :--- | :--- | :---: |
| **Datei** | Neu / Öffnen / Zuletzt geöffnete Dateien / Speichern / Speichern unter | <kbd>Ctrl+N</kbd> / <kbd>Ctrl+O</kbd> / Menü / <kbd>Ctrl+S</kbd> / <kbd>Ctrl+Shift+S</kbd> |
| **Datei** | Drucken / Tab schließen / Beenden | <kbd>Ctrl+P</kbd> / <kbd>Ctrl+W</kbd> / <kbd>Ctrl+Q</kbd> |
| **Bearbeiten** | Rückgängig / Wiederherstellen | <kbd>Ctrl+Z</kbd> / <kbd>Ctrl+Shift+Z</kbd> |
| **Bearbeiten** | Suchen / Suchen & Ersetzen | <kbd>Ctrl+F</kbd> / <kbd>Ctrl+R</kbd> |
| **Ansicht** | Formatiert / Live-Vorschau / Quelltext / Reintext | <kbd>Ctrl+Alt+1</kbd> … <kbd>Ctrl+Alt+4</kbd> |
| **Ansicht** | Ordnerbaum (inkl. *Aktive Datei aufklappen* oben rechts) / Gliederung / Zeilennummern | <kbd>F7</kbd> / <kbd>F9</kbd> / <kbd>F11</kbd> |
| **Ansicht** | Zeilenumbruch / Fokus-Modus / Schreibmaschinen-Modus | <kbd>F10</kbd> / <kbd>Ctrl+Shift+F</kbd> / <kbd>Ctrl+Shift+M</kbd> |
| **Ansicht** | Schrift vergrößern / verkleinern / zurücksetzen | <kbd>Ctrl++</kbd> / <kbd>Ctrl+-</kbd> / <kbd>Ctrl+)</kbd> |
| **Format** | Fett / Kursiv / Durchgestrichen / Textmarker / Inline-Code | <kbd>Ctrl+B</kbd> / <kbd>Ctrl+I</kbd> / <kbd>Ctrl+Shift+X</kbd> / <kbd>Ctrl+Shift+H</kbd> / <kbd>Ctrl+E</kbd> |
| **Format** | Codeblock einfügen / Codeblock-Sprache ändern | <kbd>Ctrl+Shift+C</kbd> / <kbd>Ctrl+Alt+L</kbd> |
| **Hilfe** | Markdown-Kurzreferenz / Markdown-Langreferenz | <kbd>F1</kbd> / <kbd>Shift+F1</kbd> |
"#;

/// Comprehensive English Markdown reference manual (`Help -> Markdown Complete Reference`, `Shift+F1`).
pub const MARKDOWN_REFERENCE_EN: &str = r#"# Markdown Complete Reference — KMark Manual

This reference covers the full **CommonMark and GitHub Flavored Markdown (GFM)** syntax supported by **KMark**, together with its interactive **Formatted (WYSIWYG)** and **Live Preview** editing workflows.

---

## 1. Core Principle: One Buffer, Four Views

Every KMark tab is backed by a single persistent UTF-8 `Rope` text buffer. You can switch between all four views instantaneously with zero conversion loss:

| View Mode | Shortcut | Behavior |
| :--- | :---: | :--- |
| **Formatted (WYSIWYG)** | <kbd>Ctrl+Alt+1</kbd> | Hides Markdown delimiters (`**`, `#`, `` ` ``) so you write directly in clean typography. |
| **Live Preview** | <kbd>Ctrl+Alt+2</kbd> | Formatted everywhere except the active cursor block/line, which reveals raw syntax. |
| **Source Code** | <kbd>Ctrl+Alt+3</kbd> | Monospace Markdown source editor with heading highlights and line numbers. |
| **Plain Text (TXT)** | <kbd>Ctrl+Alt+4</kbd> | Pure KWrite/Kate-style plain-text mode (automatic default for `.txt`, `.log`, `.ini`). |

---

## 2. Headings & Document Outline

KMark supports both **ATX headings** (`#` through `######`) and two-line **Setext headings** (`===` for H1, `---` for H2). Every heading is indexed live in the **Outline** dock (<kbd>F9</kbd>).

| Level | Markdown Input | Shortcut |
| :--- | :--- | :---: |
| Heading 1 | `# Document Title` | <kbd>Ctrl+1</kbd> |
| Heading 2 | `## Major Section` | <kbd>Ctrl+2</kbd> |
| Heading 3 | `### Subsection` | <kbd>Ctrl+3</kbd> |
| Heading 4 | `#### Sub-subsection` | <kbd>Ctrl+4</kbd> |
| Heading 5 | `##### Minor Heading` | <kbd>Ctrl+5</kbd> |
| Heading 6 | `###### Caption Heading` | <kbd>Ctrl+6</kbd> |
| Body Paragraph | *(removes leading `#`)* | <kbd>Ctrl+0</kbd> |

> [!TIP] Heading Indicator Styles
> Use `Settings -> Heading Indicators` to choose between slanted level bars (`1..6` bars beside each heading), subtle `#` prefixes, or no margin indicator.

---

## 3. Paragraphs, Line Breaks & Horizontal Rules

- **Paragraphs**: Separate paragraphs with one or more **blank lines**.
- **Smart Continuation**: Pressing <kbd>Enter</kbd> inside bullet lists, numbered lists, task lists, or blockquotes continues the structure automatically; pressing <kbd>Enter</kbd> again on an empty list item exits the list cleanly.
- **Horizontal Rules**: Place `---`, `***`, or `___` alone on a line (`Format -> Horizontal Rule`).

---

## 4. Inline Formatting & Typography

Inline styles can be nested freely — for example `**bold and *italic* together**` (**bold and *italic* together**).

| Appearance | Syntax | Shortcut | KMark Behavior |
| :--- | :--- | :---: | :--- |
| **Bold** | `**Word**` or `__Word__` | <kbd>Ctrl+B</kbd> | Supports *Armed Formatting* before typing |
| *Italic* | `*Word*` or `_Word_` | <kbd>Ctrl+I</kbd> | Nests cleanly inside **bold** |
| ***Bold & Italic*** | `***Word***` | <kbd>Ctrl+B</kbd>, <kbd>Ctrl+I</kbd> | Triple asterisks resolve inner/outer spans |
| ~~Strikethrough~~ | `~~Word~~` | <kbd>Ctrl+Shift+X</kbd> | GFM strikethrough extension |
| ==Highlight== | `==Word==` | <kbd>Ctrl+Shift+H</kbd> | Marker highlight pill |
| `Inline Code` | `` `command` `` | <kbd>Ctrl+E</kbd> | Monospace code pill for identifiers & paths |
| Keycap <kbd>Ctrl+S</kbd> | `<kbd>Ctrl+S</kbd>` | — | Typeset hardware keycap with border & bevel |
| Escaped chars \* \# \[ | `\*`, `\#`, `\[`, `` \` `` | — | Backslash `\` suppresses Markdown parsing |

---

## 5. Links & Images (Including Drag & Drop)

### Hyperlinks
- **Standard Link**: `[KDE Community](https://kde.org)` → [KDE Community](https://kde.org) (<kbd>Ctrl+K</kbd>)
- **Open in Browser**: Hold <kbd>Ctrl</kbd> and click any formatted link.

### Embedding Images, Captions & Relative Sizes
- **Basic Syntax**: `![Alt text](path/to/image.png)` (<kbd>Ctrl+Shift+I</kbd>)
- **Image Caption**: `![Alt text](path/to/image.png "Figure 1: Architecture overview")` — rendered centered in italic directly underneath the image in Formatted and Live Preview modes.
- **Relative Image Size**: `![Alt text](path/to/image.png "Caption"){width=50%}` — scales the image proportionally relative to the column width (`25%`, `50%`, `75%`, `100%`, etc.).
- **Right-Click Image Context Menu**:
  - *Add Caption… / Edit Caption… / Remove Caption*
  - *Replace Image…* (preserving caption and relative size)
  - *Size* (`Automatic`, `25%`, `33%`, `50%`, `66%`, `75%`, `100%`, or custom percentage)
- **Drag & Drop**: Drag image files (`.png`, `.jpg`, `.jpeg`, `.svg`, `.webp`, `.gif`, `.bmp`) from **Dolphin** or any file manager directly onto the editor canvas:
  - When the active Markdown document is saved on disk, KMark automatically computes a **clean relative path** (e.g. `![diagram](assets/diagram.png)`).
  - Dropping `.md` or `.txt` files onto the window opens them in a new tab.

---

## 6. Lists & Interactive Task Lists

### Bullet Lists (<kbd>Ctrl+Shift+U</kbd>)
Start lines with `- `, `* `, or `+ `. Press <kbd>Tab</kbd> to indent nested items by two spaces and <kbd>Shift+Tab</kbd> to outdent:

- Main topic A
  - Sub-item A.1
  - Sub-item A.2
- Main topic B

### Ordered Lists (<kbd>Ctrl+Shift+O</kbd>)
1. First step (pressing <kbd>Enter</kbd> automatically inserts `2. `)
2. Second step (pressing <kbd>Enter</kbd> on an empty item exits the list)

### Task Lists (<kbd>Ctrl+Shift+T</kbd>)
- [x] Draft specification (`- [x] `)
- [ ] Review pull request (`- [ ] ` — click checkboxes directly in Formatted mode!)

---

## 7. Blockquotes & Callout Alerts (GFM)

Standard blockquotes start with `> ` (<kbd>Ctrl+Shift+Q</kbd>) and nest with `>> `:

> Good tools disappear while you write and let your ideas take center stage.

KMark also supports all five color-coded **GFM Alert Callouts** (`Format -> Insert Callout`), with optional custom titles on the opening line. Click the **top-right Callout type badge** (e.g. `Note ▾`, matching fenced code blocks) to switch between Callout types or convert back to a plain blockquote:

> [!NOTE]
> Useful context, background notes, or implementation details.

> [!TIP] Custom Tip Title
> Productivity shortcuts, best practices, and helpful workflows.

> [!IMPORTANT]
> Essential requirements that readers should not overlook.

> [!WARNING]
> Potential pitfalls, breaking changes, or compatibility caveats.

> [!CAUTION]
> High-risk operations that may cause data loss.

---

## 8. Fenced Code Blocks & Selecting the Language in Formatted Mode

Fenced code blocks open and close with triple backticks ```` ``` ```` or tildes `~~~` (<kbd>Ctrl+Shift+C</kbd>):

```python
def fibonacci(n: int) -> list[int]:
    a, b = 0, 1
    out = []
    for _ in range(n):
        out.append(a)
        a, b = b, a + b
    return out
```

### Selecting a Code Block's Language in Formatted (WYSIWYG) Mode
1. **Click the Language Badge**: Every fenced code block displays an interactive pill badge in its top-right corner (e.g. `python ▾`, or `Code ▾` when untagged). Click it to pick from common languages (`rust`, `python`, `bash`, `cpp`, `json`, `toml`, `yaml`, `html`, `sql`, …) or choose **Custom Language…**.
2. **Menu / Shortcut**: Place the cursor inside any code block and trigger `Format -> Code Block Language…` (<kbd>Ctrl+Alt+L</kbd>).
3. **Live Preview**: Move the caret into the code block to reveal and edit ```` ```language ```` directly.

---

## 9. GFM Pipe Tables (2D Grid Editing)

Tables consist of a header row, an alignment delimiter row (`:---` left, `:---:` center, `---:` right), and body rows:

| Left Column (`:---`) | Centered (`:---:`) | Right-Aligned (`---:`) |
| :--- | :---: | ---: |
| Rust `L0`–`L4` | <kbd>Tab</kbd> | 0.86 ms |
| Qt6 / KF6 `L5` | <kbd>Shift+Tab</kbd> | 60 fps |

- **Cell Navigation**: <kbd>Tab</kbd> advances to the next cell; <kbd>Shift+Tab</kbd> moves back.
- **Auto-Append Row**: Pressing <kbd>Tab</kbd> in the bottom-right cell automatically appends a fresh row.
- **Table Menu**: Insert/delete rows and columns or change column alignment via the `Table` menu — KMark cleanly reformats the underlying Markdown pipe table automatically!

---

## 10. Complete Keyboard Shortcut Reference

| Category | Action | Shortcut |
| :--- | :--- | :---: |
| **File** | New / Open / Open Recent / Save / Save As | <kbd>Ctrl+N</kbd> / <kbd>Ctrl+O</kbd> / Menu / <kbd>Ctrl+S</kbd> / <kbd>Ctrl+Shift+S</kbd> |
| **File** | Print / Close Tab / Quit | <kbd>Ctrl+P</kbd> / <kbd>Ctrl+W</kbd> / <kbd>Ctrl+Q</kbd> |
| **Edit** | Undo / Redo | <kbd>Ctrl+Z</kbd> / <kbd>Ctrl+Shift+Z</kbd> |
| **Edit** | Find / Find & Replace | <kbd>Ctrl+F</kbd> / <kbd>Ctrl+R</kbd> |
| **View** | Formatted / Live Preview / Source / Plain Text | <kbd>Ctrl+Alt+1</kbd> … <kbd>Ctrl+Alt+4</kbd> |
| **View** | Folder Tree (incl. top-right *Select Opened File*) / Outline / Line Numbers | <kbd>F7</kbd> / <kbd>F9</kbd> / <kbd>F11</kbd> |
| **View** | Word Wrap / Focus Mode / Typewriter Mode | <kbd>F10</kbd> / <kbd>Ctrl+Shift+F</kbd> / <kbd>Ctrl+Shift+M</kbd> |
| **View** | Zoom In / Zoom Out / Reset Zoom | <kbd>Ctrl++</kbd> / <kbd>Ctrl+-</kbd> / <kbd>Ctrl+)</kbd> |
| **Format** | Bold / Italic / Strikethrough / Highlight / Inline Code | <kbd>Ctrl+B</kbd> / <kbd>Ctrl+I</kbd> / <kbd>Ctrl+Shift+X</kbd> / <kbd>Ctrl+Shift+H</kbd> / <kbd>Ctrl+E</kbd> |
| **Format** | Insert Code Block / Change Code Block Language | <kbd>Ctrl+Shift+C</kbd> / <kbd>Ctrl+Alt+L</kbd> |
| **Help** | Markdown Quick Reference / Complete Reference | <kbd>F1</kbd> / <kbd>Shift+F1</kbd> |
"#;
