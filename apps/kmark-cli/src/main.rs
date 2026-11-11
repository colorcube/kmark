use std::env;
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use kmark_buffer::encoding::{read_file_auto, write_file_atomic};
use kmark_buffer::{LineEnding, Rope, TextEncoding};
use kmark_engine::{COMMAND_CATALOG, Session};
use kmark_layout::FallbackMeasurer;
use kmark_syntax::{BlockKind, export_html_document, extract_outline, parse_blocks};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let Some(sub) = args.first().map(String::as_str) else {
        print_usage();
        return ExitCode::SUCCESS;
    };

    let result = match sub {
        "info" => cmd_info(args.get(1).map(String::as_str)),
        "outline" => cmd_outline(args.get(1).map(String::as_str)),
        "html" => cmd_html(&args[1..]),
        "commands" => {
            cmd_commands();
            Ok(())
        }
        "bench" => {
            cmd_bench();
            Ok(())
        }
        "-h" | "--help" | "help" => {
            print_usage();
            Ok(())
        }
        other => Err(format!("Unbekannter Unterbefehl: '{other}'")),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("Fehler: {msg}");
            ExitCode::FAILURE
        }
    }
}

fn print_usage() {
    println!(
        "\
kmark-cli — KMark Headless-Werkzeug (L0–L3 Rust-Engine)

Verwendung:
  kmark-cli info <datei>               Dokument-Statistik, Kodierung & Blockstruktur
  kmark-cli outline <datei>            Überschriften-Gliederung (H1–H6) ausgeben
  kmark-cli html <datei> [-o out.html] Eigenständiges HTML5-Dokument exportieren
  kmark-cli commands                   Alle registrierten Editor-Befehle & KDE-Shortcuts
  kmark-cli bench                      Latenz-Benchmark auf 100.000 Zeilen (~4,5 MB)"
    );
}

fn cmd_info(path_arg: Option<&str>) -> Result<(), String> {
    let Some(path_str) = path_arg else {
        return Err("Bitte Dateipfad angeben: kmark-cli info <datei>".to_owned());
    };
    let decoded = read_file_auto(Path::new(path_str)).map_err(|e| e.to_string())?;
    let rope = Rope::from_str(&decoded.text);
    let blocks = parse_blocks(&rope);
    let outline = extract_outline(&rope, &blocks);

    let mut headings = 0usize;
    let mut tables = 0usize;
    let mut code_fences = 0usize;
    let mut lists = 0usize;
    for b in &blocks {
        match b.kind {
            BlockKind::Heading { .. } => headings = headings.saturating_add(1),
            BlockKind::Table(_) => tables = tables.saturating_add(1),
            BlockKind::CodeFence { .. } => code_fences = code_fences.saturating_add(1),
            BlockKind::ListItem { .. } => lists = lists.saturating_add(1),
            _ => {}
        }
    }

    println!("Datei:          {path_str}");
    println!("Kodierung:      {}", decoded.encoding.label());
    println!("Zeilenende:     {}", decoded.line_ending.label());
    println!("Bytes (UTF-8):  {}", rope.len_bytes());
    println!("Zeichen:        {}", rope.len_chars());
    println!("Zeilen:         {}", rope.line_count());
    println!("Blöcke gesamt:  {}", blocks.len());
    println!(
        "  Überschriften:{headings:>6} (Gliederungseinträge: {})",
        outline.len()
    );
    println!("  Tabellen:     {tables:>6}");
    println!("  Codeblöcke:   {code_fences:>6}");
    println!("  Listenzeilen: {lists:>6}");
    Ok(())
}

fn cmd_outline(path_arg: Option<&str>) -> Result<(), String> {
    let Some(path_str) = path_arg else {
        return Err("Bitte Dateipfad angeben: kmark-cli outline <datei>".to_owned());
    };
    let decoded = read_file_auto(Path::new(path_str)).map_err(|e| e.to_string())?;
    let rope = Rope::from_str(&decoded.text);
    let blocks = parse_blocks(&rope);
    let outline = extract_outline(&rope, &blocks);

    for item in outline {
        let indent = "  ".repeat(usize::from(item.level.saturating_sub(1)));
        println!(
            "{indent}H{}  [Zeile {:>5}]  {}  (#{})",
            item.level,
            item.line.saturating_add(1),
            item.title,
            item.slug
        );
    }
    Ok(())
}

fn cmd_html(args: &[String]) -> Result<(), String> {
    let Some(input_path) = args.first() else {
        return Err("Bitte Eingabedatei angeben: kmark-cli html <datei> [-o out.html]".to_owned());
    };
    let mut out_path: Option<&str> = None;
    let mut i = 1usize;
    while i < args.len() {
        if args.get(i).map(String::as_str) == Some("-o") {
            out_path = args.get(i.saturating_add(1)).map(String::as_str);
            i = i.saturating_add(2);
        } else {
            i = i.saturating_add(1);
        }
    }

    let path = Path::new(input_path);
    let decoded = read_file_auto(path).map_err(|e| e.to_string())?;
    let title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("KMark Dokument");
    let rope = Rope::from_str(&decoded.text);
    let blocks = parse_blocks(&rope);
    let html = export_html_document(title, &rope, &blocks);

    if let Some(dest) = out_path {
        write_file_atomic(Path::new(dest), &html, TextEncoding::Utf8, LineEnding::Lf)
            .map_err(|e| e.to_string())?;
        println!(
            "HTML5-Dokument geschrieben nach: {dest} ({} Bytes)",
            html.len()
        );
    } else {
        print!("{html}");
    }
    Ok(())
}

fn cmd_commands() {
    println!(
        "{:<24} {:<12} {:<16} {:<26} Bezeichnung",
        "ID", "Kategorie", "Tastenkürzel", "Breeze-Icon"
    );
    println!("{}", "-".repeat(98));
    for c in COMMAND_CATALOG {
        println!(
            "{:<24} {:<12} {:<16} {:<26} {}",
            c.id, c.category_de, c.shortcut, c.icon_theme, c.label_de
        );
    }
}

fn cmd_bench() {
    // Build a realistic ~100,000-line Markdown document (~4.5 MB)
    let mut doc = String::with_capacity(4_800_000);
    for sec in 0..5_000usize {
        doc.push_str(&format!(
            "## Kapitel {sec} — Systemarchitektur & Typografie\n\n\
             Dies ist ein **formatierter** Absatz mit *Kursivschrift*, `Inline-Code` und einem [KDE-Link](https://kde.org).\n\
             Weitere Zeile mit deutschen Umlauten ÄÖÜß sowie Unicode-Symbolen.\n\n\
             - [x] Erledigte Teilaufgabe {sec}\n\
             - [ ] Offene Teilaufgabe {sec}\n\n\
             | Merkmal | Status | Wert |\n\
             | --- | :---: | ---: |\n\
             | Puffer | Aktiv | {sec} |\n\n"
        ));
    }

    let t0 = Instant::now();
    let rope = Rope::from_str(&doc);
    let rope_ms = t0.elapsed().as_secs_f64() * 1000.0;

    let t1 = Instant::now();
    let blocks = parse_blocks(&rope);
    let outline = extract_outline(&rope, &blocks);
    let parse_ms = t1.elapsed().as_secs_f64() * 1000.0;

    let mut session = Session::new_headless();
    if let Some(tab) = session.active_doc_mut() {
        tab.rope = rope.clone();
        tab.saved_rope = rope.clone();
        tab.blocks = blocks;
        tab.outline = outline;
    }

    let mid_byte = rope.len_bytes() / 2;
    session.jump_to_byte_offset(mid_byte);

    let mut measurer = FallbackMeasurer;
    let t2 = Instant::now();
    let frame_cmds = session
        .render_viewport(1280.0, 800.0, &mut measurer)
        .map_or(0, |f| f.draw_cmds.len());
    let layout_ms = t2.elapsed().as_secs_f64() * 1000.0;

    // Benchmark single Rope edit + viewport frame render at mid-document
    let t3 = Instant::now();
    let mut rope_edit = rope.clone();
    let aligned_mid = rope_edit.clamp_char_boundary(mid_byte);
    rope_edit.replace_range(aligned_mid..aligned_mid, "x");
    let rope_keystroke_us = t3.elapsed().as_secs_f64() * 1_000_000.0;

    let t4 = Instant::now();
    session.insert_text("a");
    let _ = session.render_viewport(1280.0, 800.0, &mut measurer);
    let full_cycle_ms = t4.elapsed().as_secs_f64() * 1000.0;

    println!("KMark Großdokument-Benchmark");
    println!("============================");
    println!(
        "Dokumentgröße:             {:.2} MiB ({} Zeilen)",
        rope.len_bytes() as f64 / (1024.0 * 1024.0),
        rope.line_count()
    );
    println!("Rope-Aufbau (O(N)):        {rope_ms:>8.2} ms");
    println!("Vollständiger Block-Parse: {parse_ms:>8.2} ms");
    println!("Viewport-Layout (WYSIWYG): {layout_ms:>8.2} ms ({frame_cmds} DrawCmds)");
    println!("Rope-Einzeltaste (O(logN)):{rope_keystroke_us:>8.2} µs");
    println!("Kompletter Tipp-Zyklus:    {full_cycle_ms:>8.2} ms");
}
