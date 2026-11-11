//! `kmark-syntax` (L1): Exact-offset Markdown block parser, 2D table AST,
//! inline span/marker tokenizer, outline extractor, and HTML5 exporter.

pub mod blocks;
pub mod html;
pub mod inline;
pub mod outline;

pub use blocks::{
    AlertKind, Block, BlockKind, ColumnAlign, QuoteLine, TableBlock, TableCell, TableRow,
    TaskState, is_blank, looks_like_table_row, parse_atx_heading, parse_blockquote_prefix,
    parse_blocks, parse_blocks_in_line_range, parse_table_delimiter, parse_table_row,
    reparse_changed_rope, reparse_changed_rope_with_min_lines, reparse_incremental,
};
pub use html::export_html_document;
pub use inline::{
    ImageSize, InlineToken, InlineTokenKind, StyleFlags, active_styles_at, format_markdown_image,
    parse_inline_tokens,
};
pub use outline::{OutlineEntry, extract_outline, heading_slug, strip_inline_syntax};

#[cfg(test)]
mod tests {
    use kmark_buffer::Rope;

    use super::*;

    #[test]
    fn parses_headings_lists_alerts_and_tables_with_exact_byte_offsets() {
        let md = "\
# Haupttitel

> [!TIP] Schneller Einstieg
> Direkt im formatierten Text schreiben.

- [x] Erledigt
- [ ] Offen

| Spalte A | Mitte | Rechts |
| --- | :---: | ---: |
| **Fett** | Text | 42 € |
";
        let rope = Rope::from_str(md);
        let blocks = parse_blocks(&rope);

        let outline = extract_outline(&rope, &blocks);
        assert_eq!(outline.len(), 1);
        assert_eq!(outline[0].title, "Haupttitel");
        assert_eq!(outline[0].slug, "haupttitel");

        let has_alert = blocks.iter().any(|b| {
            matches!(
                b.kind,
                BlockKind::BlockQuote {
                    alert: Some(AlertKind::Tip),
                    ..
                }
            )
        });
        assert!(has_alert);

        let table = blocks
            .iter()
            .find_map(|b| match &b.kind {
                BlockKind::Table(t) => Some(t),
                _ => None,
            })
            .unwrap();
        assert_eq!(table.col_count, 3);
        assert_eq!(
            table.alignments,
            vec![ColumnAlign::Left, ColumnAlign::Center, ColumnAlign::Right]
        );
        assert_eq!(
            rope.slice_to_string(table.body_rows[0].cells[0].trimmed_range.clone()),
            "**Fett**"
        );
        assert_eq!(
            rope.slice_to_string(table.body_rows[0].cells[2].trimmed_range.clone()),
            "42 €"
        );
    }

    #[test]
    fn inline_tokens_cover_every_byte_and_track_styles() {
        let line = "Normal **fett *und kursiv*** sowie ==markiert== und [Link](https://kde.org).";
        let tokens = parse_inline_tokens(100, line);
        assert_eq!(tokens.first().unwrap().range.start, 100);
        assert_eq!(tokens.last().unwrap().range.end, 100 + line.len());
        for pair in tokens.windows(2) {
            assert_eq!(pair[0].range.end, pair[1].range.start);
        }

        // Caret inside `fett`
        let bold_pos = 100 + line.find("fett").unwrap() + 2;
        assert!(active_styles_at(100, line, bold_pos).contains(StyleFlags::BOLD));

        // Caret inside `und kursiv`
        let bi_pos = 100 + line.find("kursiv").unwrap() + 2;
        let st = active_styles_at(100, line, bi_pos);
        assert!(st.contains(StyleFlags::BOLD));
        assert!(st.contains(StyleFlags::ITALIC));
    }

    #[test]
    fn parses_and_formats_image_captions_and_relative_sizes() {
        let md = format_markdown_image(
            "Architektur",
            "docs/arch.png",
            Some("Abb. 1: Schichtenmodell (L0–L5) mit \"Zitaten\""),
            Some(ImageSize::Percent(50)),
        );
        assert_eq!(
            md,
            "![Architektur](docs/arch.png \"Abb. 1: Schichtenmodell (L0–L5) mit \\\"Zitaten\\\"\"){width=50%}"
        );

        let tokens = parse_inline_tokens(0, &md);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].range, 0..md.len());
        assert_eq!(
            tokens[0].kind,
            InlineTokenKind::Image {
                alt: "Architektur".to_owned(),
                src: "docs/arch.png".to_owned(),
                caption: Some("Abb. 1: Schichtenmodell (L0–L5) mit \"Zitaten\"".to_owned()),
                size: Some(ImageSize::Percent(50)),
            }
        );

        let rope = Rope::from_str(&md);
        let blocks = parse_blocks(&rope);
        let html = export_html_document("Test", &rope, &blocks);
        assert!(html.contains("class=\"kmark-caption\""));
        assert!(html.contains("width:50%"));
    }

    #[test]
    fn incremental_reparse_matches_full_parse_exactly() {
        let mut src = String::new();
        for i in 0..100 {
            src.push_str(&format!(
                "## Kapitel {i}\n\nAbsatz in Kapitel {i} mit **Text**.\n\n- Punkt A\n- Punkt B\n\n"
            ));
        }
        let old_rope = Rope::from_str(&src);
        let mut blocks = parse_blocks(&old_rope);

        let mid = old_rope.line_start_byte(350);
        let mut new_rope = old_rope.clone();
        new_rope.replace_range(mid..mid, "Zusatzzeile!\nUnd noch eine ");

        reparse_incremental(&old_rope, &new_rope, &mut blocks, mid);
        let full = parse_blocks(&new_rope);
        assert_eq!(blocks, full);
    }
}
