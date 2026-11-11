# AGENTS.md — Architektur- & Entwicklungsleitfaden für KMark

**KMark** ist ein schneller, nativer Markdown- und Reintext-Editor für **KDE Plasma / Linux**, inspiriert von der Single-Buffer-WYSIWYG-Bedienung von **MDNote** und der klassischen KDE-Architektur von **Kate / KWrite**.

## 1. Schichtenarchitektur (`L0` -> `L5`)

```text
crates/
  buffer       L0 Persistenter Copy-on-Write B-Baum-Rope (O(log n) Edits, O(1) Snapshots),
                  Zeilen-/UTF-16-/Graphem-Navigation, Kodierungen (UTF-8/BOM/UTF-16/CP1252),
                  atomares Speichern & fehlertolerante Undo/Redo-Historie
  syntax       L1 Inkrementeller Markdown-Block-Scanner (Paragraph, Heading, CodeFence,
                  BlockQuote/Alert, List, Table, Rule) + Inline-Span-Parser + Gliederung + HTML-Export
  layout       L2 Single-Buffer-Projektion (Formatiert/WYSIWYG, Live-Zeile, Quelltext, TXT),
                  2D-Tabellen-Grid-Layout direkt über Rope-Slices, Viewport-Virtualisierung,
                  abstrakter TextMeasurer + flache DisplayList für QPainter
  engine       L3 Session, Dokument-Tabs, Command-Registry, reine Markdown-/Tabellen-Kommandos,
                  Armed-Formatting an leerer Selektion, Suchen/Ersetzen & Crash-Recovery
  ffi          L4 Dünne C-ABI-Brücke (`libkmark_ffi.a` + `kmark_ffi.h`) mit `catch_unwind`-Schutz
apps/
  kmark-cli    Headless-CLI für Diagnose, Gliederung, HTML-Export, Kommandoliste & Latenz-Benchmarks
qt-shell/      L5 Native KDE / Qt6 C++-Oberfläche (`KXmlGuiWindow` / `QMainWindow` + `QPainter`-Viewport)
```

## 2. Goldene Regeln

1. **Niemals abstürzen (`unsafe_code = "forbid"` in allen Kern-Crates `L0`–`L3` sowie `kmark-cli`):**
   - Produktivcode verbietet `unwrap()`, `expect()`, `panic!`, `todo!`, `unimplemented!` und `unreachable!`.
   - Alle String-Zugriffe beachten UTF-8-Zeichengrenzen (`get()`, `clamp_char_boundary()`).
   - `kmark-ffi` kapselt als einziges Crate `extern "C"`-Aufrufe und fängt jeden Aufruf mit `std::panic::catch_unwind` ab.
2. **Ein einziger Text-Buffer (kein verlustbehafteter AST-Roundtrip):**
   - Alle vier Ansichtsmodi (*Formatiert*, *Live*, *Quelltext*, *TXT*) arbeiten direkt auf demselben `Rope`.
   - Moduswechsel sind $O(1)$ ohne Textkonvertierung.
3. **Kate-Architektur für 100 % KDE-Integration:**
   - Fensterrahmen, Menüleiste, Werkzeugleisten, andockbare Seitenleisten (`QDockWidget`), Kurzbefehle, Dateidialoge, Druckvorschau und IME (`QInputMethod`) sind echtes **Qt6 / KDE Frameworks 6**.
   - Der Editor-Viewport zeichnet die aus Rust berechnete `DisplayList` direkt per `QPainter` und nutzt `QFontMetricsF` / `QTextLayout` über den `TextMeasurer`-Callback für pixelgenaue Übereinstimmung von Layout und Darstellung.
