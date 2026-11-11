//! `kmark-layout` (L2): Single-Buffer Projected Layout Engine (WYSIWYG, Live Preview,
//! Source, Plain Text), 2D Table Grid layout, hit-testing, and Qt `QPainter` DisplayList.

pub mod engine;
pub mod measurer;
pub mod theme;

pub use engine::{
    BlockHeightCache, CalloutBadgeHitBox, CheckboxHitBox, CodeLangHitBox, DrawCmd,
    HeadingMarkStyle, HitTarget, LayoutConfig, PositionedGlyphSpan, RectF, ViewMode, ViewportFrame,
    VisualRow, build_viewport_frame, build_viewport_frame_cached,
};
pub use measurer::{FallbackMeasurer, FontMetrics, FontRole, MeasuredRun, TextMeasurer};
pub use theme::{Rgba, ThemePalette};

#[cfg(test)]
mod tests {
    use kmark_buffer::{Rope, Selection};
    use kmark_syntax::parse_blocks;

    use super::*;

    #[test]
    fn wysiwyg_hides_markers_while_live_reveals_caret_block() {
        let md = "# Überschrift\n\nHier ist **fetter** Text und\n- [ ] Aufgabe eins\n";
        let rope = Rope::from_str(md);
        let blocks = parse_blocks(&rope);
        let mut measurer = FallbackMeasurer;

        // Caret on paragraph line in WYSIWYG mode: `**` is hidden (`0 px`)!
        let bold_pos = md.find("fetter").unwrap();
        let mut cfg = LayoutConfig {
            view_mode: ViewMode::Wysiwyg,
            ..LayoutConfig::default()
        };

        let frame = build_viewport_frame(
            &rope,
            &blocks,
            Selection::collapsed(bold_pos),
            None,
            &[],
            None,
            &cfg,
            960.0,
            700.0,
            0.0,
            &mut measurer,
        );

        let drawn_texts: Vec<&str> = frame
            .draw_cmds
            .iter()
            .filter_map(|c| match c {
                DrawCmd::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(drawn_texts.iter().any(|t| t.contains("fetter")));
        assert!(!drawn_texts.iter().any(|t| t.contains("**")));
        assert_eq!(frame.checkboxes.len(), 1);

        // Switch to LivePreview on the same caret position: `**` appears on the active line!
        cfg.view_mode = ViewMode::LivePreview;
        let live_frame = build_viewport_frame(
            &rope,
            &blocks,
            Selection::collapsed(bold_pos),
            None,
            &[],
            None,
            &cfg,
            960.0,
            700.0,
            0.0,
            &mut measurer,
        );
        let live_texts: Vec<&str> = live_frame
            .draw_cmds
            .iter()
            .filter_map(|c| match c {
                DrawCmd::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(live_texts.iter().any(|t| t.contains("**")));
        // Heading block (not containing caret) still hides `#`!
        assert!(!live_texts.iter().any(|t| t.starts_with("# ")));
    }

    #[test]
    fn hit_testing_maps_click_on_checkbox_and_code_badge() {
        let md = "- [ ] Klick mich\n\n```rust\nlet x = 1;\n```\n";
        let rope = Rope::from_str(md);
        let blocks = parse_blocks(&rope);
        let mut measurer = FallbackMeasurer;
        let cfg = LayoutConfig::default();

        let frame = build_viewport_frame(
            &rope,
            &blocks,
            Selection::collapsed(0),
            None,
            &[],
            None,
            &cfg,
            900.0,
            600.0,
            0.0,
            &mut measurer,
        );

        let cb = &frame.checkboxes[0];
        let hit = frame.hit_test(cb.rect_doc.x + 3.0, cb.rect_doc.y + 3.0);
        assert!(matches!(hit, HitTarget::Checkbox { checked: false, .. }));

        assert_eq!(frame.code_lang_badges.len(), 1);
        let badge = &frame.code_lang_badges[0];
        let b_hit = frame.hit_test(badge.rect_doc.x + 4.0, badge.rect_doc.y + 4.0);
        assert_eq!(
            b_hit,
            HitTarget::CodeLanguageBadge {
                open_line_start: badge.open_line_start,
                language: "rust".to_owned(),
            }
        );
    }

    #[test]
    fn table_fits_monospace_code_cells_and_centers_kbd_keycaps() {
        let md = "| Element | Tastenkürzel | Markdown-Syntax |\n| :--- | :---: | :--- |\n| [Link](https://kde.org) | <kbd>Ctrl+B</kbd> | `[Titel](https://kde.org)` |\n";
        let rope = Rope::from_str(md);
        let blocks = parse_blocks(&rope);
        let mut measurer = FallbackMeasurer;
        let cfg = LayoutConfig::default();

        let frame = build_viewport_frame(
            &rope,
            &blocks,
            Selection::collapsed(0),
            None,
            &[],
            None,
            &cfg,
            960.0,
            600.0,
            0.0,
            &mut measurer,
        );

        // Outer table border is the widest StrokeRect in the frame.
        let table_rect = frame
            .draw_cmds
            .iter()
            .filter_map(|c| match c {
                DrawCmd::StrokeRect { rect, .. } => Some(*rect),
                _ => None,
            })
            .max_by(|a, b| a.w.partial_cmp(&b.w).unwrap())
            .unwrap();

        // Check `[Titel](https://kde.org)` monospace text & pill stay strictly inside `table_rect`!
        let (code_x, code_w) = frame
            .draw_cmds
            .iter()
            .find_map(|c| match c {
                DrawCmd::Text {
                    x,
                    text,
                    role,
                    size_px,
                    bold,
                    italic,
                    ..
                } if text == "[Titel](https://kde.org)" => {
                    let m = measurer.measure_run(text, *role, *size_px, *bold, *italic);
                    Some((*x, m.total_width))
                }
                _ => None,
            })
            .unwrap();
        assert!(
            code_x + code_w + 4.5 <= table_rect.x + table_rect.w + 0.5,
            "inline code cell overflowed table border: text_end={} > table_end={}",
            code_x + code_w + 4.5,
            table_rect.x + table_rect.w
        );

        // Check `<kbd>Ctrl+B</kbd>` keycap StrokeRect is horizontally centered around `"Ctrl+B"`!
        let (kbd_text_x, kbd_text_w) = frame
            .draw_cmds
            .iter()
            .find_map(|c| match c {
                DrawCmd::Text {
                    x,
                    text,
                    role,
                    size_px,
                    bold,
                    italic,
                    ..
                } if text == "Ctrl+B" => {
                    let m = measurer.measure_run(text, *role, *size_px, *bold, *italic);
                    Some((*x, m.total_width))
                }
                _ => None,
            })
            .unwrap();

        // Keycap StrokeRect has radius ~4.5
        let kbd_box = frame
            .draw_cmds
            .iter()
            .find_map(|c| match c {
                DrawCmd::StrokeRect { rect, radius, .. } if (*radius - 4.5).abs() < 0.2 => {
                    Some(*rect)
                }
                _ => None,
            })
            .unwrap();

        let text_center = kbd_text_x + kbd_text_w * 0.5;
        let box_center = kbd_box.x + kbd_box.w * 0.5;
        assert!(
            (text_center - box_center).abs() < 0.5,
            "kbd box center ({box_center}) misaligned with text center ({text_center})"
        );
        assert!(
            kbd_box.w >= kbd_text_w + 11.5,
            "kbd box too narrow: box_w={} vs text_w={}",
            kbd_box.w,
            kbd_text_w
        );
    }

    #[test]
    fn renders_image_with_relative_percentage_width_and_caption() {
        struct MockImageMeasurer(FallbackMeasurer);
        impl TextMeasurer for MockImageMeasurer {
            fn font_metrics(
                &mut self,
                role: FontRole,
                size_px: f32,
                bold: bool,
                italic: bool,
            ) -> FontMetrics {
                self.0.font_metrics(role, size_px, bold, italic)
            }
            fn measure_run(
                &mut self,
                text: &str,
                role: FontRole,
                size_px: f32,
                bold: bool,
                italic: bool,
            ) -> MeasuredRun {
                self.0.measure_run(text, role, size_px, bold, italic)
            }
            fn image_dimensions(&mut self, _src: &str) -> Option<(f32, f32)> {
                Some((800.0, 400.0))
            }
        }

        let md = "![Demo](foto.png \"Abbildung 1: Beispielbild\"){width=50%}\n";
        let rope = Rope::from_str(md);
        let blocks = parse_blocks(&rope);
        let mut measurer = MockImageMeasurer(FallbackMeasurer);
        let cfg = LayoutConfig::default();

        let frame = build_viewport_frame(
            &rope,
            &blocks,
            Selection::collapsed(0),
            None,
            &[],
            None,
            &cfg,
            960.0,
            600.0,
            0.0,
            &mut measurer,
        );

        let img_rect = frame
            .draw_cmds
            .iter()
            .find_map(|c| match c {
                DrawCmd::Image { rect, src } if src == "foto.png" => Some(*rect),
                _ => None,
            })
            .unwrap();

        let expected_w = frame.content_w * 0.5;
        assert!(
            (img_rect.w - expected_w).abs() < 1.0,
            "expected 50% width ({expected_w}), got {}",
            img_rect.w
        );
        assert!(
            (img_rect.h - expected_w * 0.5).abs() < 1.0,
            "expected preserved 2:1 aspect ratio, got h={}",
            img_rect.h
        );

        let cap_y = frame
            .draw_cmds
            .iter()
            .find_map(|c| match c {
                DrawCmd::Text {
                    baseline_y,
                    text,
                    italic,
                    ..
                } if text == "Abbildung 1: Beispielbild" && *italic => Some(*baseline_y),
                _ => None,
            })
            .unwrap();
        assert!(cap_y > img_rect.y + img_rect.h);
    }

    #[test]
    fn hit_testing_maps_callout_badge() {
        let md = "> [!WARNING] Eigener Titel\n> Bitte vor dem Löschen prüfen.\n";
        let rope = Rope::from_str(md);
        let blocks = parse_blocks(&rope);
        let mut measurer = FallbackMeasurer;
        let cfg = LayoutConfig::default();

        let frame = build_viewport_frame(
            &rope,
            &blocks,
            Selection::collapsed(0),
            None,
            &[],
            None,
            &cfg,
            900.0,
            600.0,
            0.0,
            &mut measurer,
        );

        assert_eq!(frame.callout_badges.len(), 1);
        let badge = &frame.callout_badges[0];
        let hit = frame.hit_test(badge.rect_doc.x + 4.0, badge.rect_doc.y + 4.0);
        assert_eq!(
            hit,
            HitTarget::CalloutBadge {
                open_line_start: 0,
                kind: kmark_syntax::AlertKind::Warning,
            }
        );
    }

    #[test]
    fn table_preserves_single_word_columns_under_wide_neighbors() {
        let md = concat!(
            "| Bereich | Funktion | Tastenkürzel |\n",
            "| :--- | :--- | :--- |\n",
            "| **Bearbeiten** | Fett / Kursiv / Durchgestrichen / Textmarker / Inline-Code | <kbd>Ctrl+B</kbd> <kbd>Ctrl+I</kbd> <kbd>Alt+Shift+5</kbd> <kbd>Ctrl+Shift+H</kbd> <kbd>Ctrl+`</kbd> |\n",
        );
        let rope = Rope::from_str(md);
        let blocks = parse_blocks(&rope);
        let mut measurer = FallbackMeasurer;
        let cfg = LayoutConfig::default();

        let frame = build_viewport_frame(
            &rope,
            &blocks,
            Selection::collapsed(0),
            None,
            &[],
            None,
            &cfg,
            820.0,
            600.0,
            0.0,
            &mut measurer,
        );

        // "Bearbeiten" in column 0 must appear as a single unbroken text draw command, never wrapped mid-word!
        let has_full_bearbeiten = frame
            .draw_cmds
            .iter()
            .any(|c| matches!(c, DrawCmd::Text { text, bold: true, .. } if text == "Bearbeiten"));
        assert!(
            has_full_bearbeiten,
            "single-word column cell '**Bearbeiten**' was squished and broken across lines"
        );
    }

    #[test]
    fn cached_block_heights_eliminate_scroll_jumps_and_scrollbar_breathing() {
        let mut md = String::new();
        let long_sentence = "Dies ist ein längerer Absatz mit automatischem Zeilenumbruch, fetten **Begriffen**, `Inline-Code` und realistischer Satzlänge für den Scroll-Test. ";
        for i in 0..120 {
            if i % 15 == 0 {
                md.push_str(&format!("## Abschnitt {i}\n\n"));
            }
            md.push_str(&long_sentence.repeat(5));
            md.push_str("\n\n");
        }

        let rope = Rope::from_str(&md);
        let blocks = parse_blocks(&rope);
        let mut measurer = FallbackMeasurer;
        let cfg = LayoutConfig::default();
        let mut height_cache = BlockHeightCache::new();

        // Scroll progressively from top to bottom so every block passes through the viewport/overscan.
        let mut scroll_y = 0.0f32;
        let mut last_total_height = 0.0f32;
        while scroll_y <= 36_000.0 {
            let f = build_viewport_frame_cached(
                &rope,
                &blocks,
                Selection::collapsed(0),
                None,
                &[],
                None,
                &cfg,
                920.0,
                680.0,
                scroll_y,
                &mut measurer,
                Some(&mut height_cache),
            );
            last_total_height = f.total_height;
            if scroll_y > f.total_height {
                break;
            }
            scroll_y += 500.0;
        }

        // Now scroll across arbitrary positions (bottom, middle, top, random offsets) and verify:
        // 1) `total_height` is 100% invariant (zero scrollbar breathing),
        // 2) every visible row's `y_doc` for a given `line_idx` stays bit-for-bit identical as blocks enter/leave overscan!
        let mut recorded_line_y = std::collections::HashMap::new();
        for &probe_scroll in &[
            last_total_height * 0.85,
            9500.0,
            4200.0,
            1300.0,
            0.0,
            6100.0,
        ] {
            let f = build_viewport_frame_cached(
                &rope,
                &blocks,
                Selection::collapsed(0),
                None,
                &[],
                None,
                &cfg,
                920.0,
                680.0,
                probe_scroll,
                &mut measurer,
                Some(&mut height_cache),
            );
            assert!(
                (f.total_height - last_total_height).abs() < 0.01,
                "scrollbar total_height breathed from {last_total_height} to {} at scroll_y={probe_scroll}",
                f.total_height
            );
            for row in &f.visual_rows {
                let prev = recorded_line_y
                    .entry((row.line_idx, row.doc_range.start))
                    .or_insert(row.y_doc);
                assert!(
                    (*prev - row.y_doc).abs() < 0.01,
                    "visual row at line {} jumped from y={} to y={}",
                    row.line_idx,
                    *prev,
                    row.y_doc
                );
            }
        }
    }
}
