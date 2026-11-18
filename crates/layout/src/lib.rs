//! `kmark-layout` (L2): Single-Buffer Projected Layout Engine (WYSIWYG, Live Preview,
//! Source, Plain Text), 2D Table Grid layout, hit-testing, and Qt `QPainter` DisplayList.

pub mod context;
pub mod engine;
pub mod hit_test;
mod inline_layout;
pub mod line_wrap;
pub mod measurer;
mod table_layout;
pub mod theme;

pub use context::ViewportRequest;
pub use engine::{
    BlockHeightCache, DrawCmd, HeadingMarkStyle, LayoutConfig, PositionedGlyphSpan, RectF,
    ViewMode, ViewportFrame, VisualRow, build_viewport_frame, build_viewport_frame_cached,
};
pub use hit_test::{CalloutBadgeHitBox, CheckboxHitBox, CodeLangHitBox, HitTarget};
pub use line_wrap::LineWrapCache;
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
            &ViewportRequest {
                rope: &rope,
                blocks: &blocks,
                selection: Selection::collapsed(bold_pos),
                ime_preedit: None,
                search_matches: &[],
                active_match_idx: None,
                config: &cfg,
                viewport_w: 960.0,
                viewport_h: 700.0,
                scroll_y: 0.0,
                scroll_x: 0.0,
            },
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
            &ViewportRequest {
                rope: &rope,
                blocks: &blocks,
                selection: Selection::collapsed(bold_pos),
                ime_preedit: None,
                search_matches: &[],
                active_match_idx: None,
                config: &cfg,
                viewport_w: 960.0,
                viewport_h: 700.0,
                scroll_y: 0.0,
                scroll_x: 0.0,
            },
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
    fn list_continuation_lines_align_under_item_text() {
        // Regression #293: indented continuation lines were a separate paragraph drawn with
        // visible indentation at the paragraph margin.
        let md = "-   **Fett:**  \n    Folgezeile im Punkt.\n\nAbsatz.\n";
        let rope = Rope::from_str(md);
        let blocks = parse_blocks(&rope);
        let mut measurer = FallbackMeasurer;
        let cfg = LayoutConfig {
            view_mode: ViewMode::Wysiwyg,
            ..LayoutConfig::default()
        };
        let render = |caret: usize, measurer: &mut FallbackMeasurer| {
            build_viewport_frame(
                &ViewportRequest {
                    rope: &rope,
                    blocks: &blocks,
                    selection: Selection::collapsed(caret),
                    ime_preedit: None,
                    search_matches: &[],
                    active_match_idx: None,
                    config: &cfg,
                    viewport_w: 960.0,
                    viewport_h: 700.0,
                    scroll_y: 0.0,
                    scroll_x: 0.0,
                },
                measurer,
            )
        };

        let text_x = |frame: &ViewportFrame, needle: &str| {
            frame.draw_cmds.iter().find_map(|c| match c {
                DrawCmd::Text { text, x, .. } if text.contains(needle) => Some(*x),
                _ => None,
            })
        };
        let cont_pos = md.find("Folgezeile").unwrap();
        let frame = render(md.find("Absatz").unwrap(), &mut measurer);
        let fett_x = text_x(&frame, "Fett").unwrap();
        let cont_x = text_x(&frame, "Folgezeile").unwrap();
        let para_x = text_x(&frame, "Absatz").unwrap();
        assert!((fett_x - cont_x).abs() < 0.5, "{fett_x} vs {cont_x}");
        assert!(cont_x > para_x + 10.0);

        // Caret in the hidden indentation sits at the start of the visible continuation text.
        let at_text = render(cont_pos, &mut measurer).caret_doc_rect;
        let at_indent = render(cont_pos - 4, &mut measurer).caret_doc_rect;
        assert!(
            (at_text.x - at_indent.x).abs() < 0.5,
            "{at_text:?} vs {at_indent:?}"
        );
        assert!((at_text.y - at_indent.y).abs() < 0.5);
    }

    fn source_frame(
        rope: &Rope,
        caret: usize,
        word_wrap: bool,
        scroll_x: f32,
        cache: &mut BlockHeightCache,
    ) -> ViewportFrame {
        let cfg = LayoutConfig {
            view_mode: ViewMode::Source,
            word_wrap,
            ..LayoutConfig::default()
        };
        build_viewport_frame_cached(
            &ViewportRequest {
                rope,
                blocks: &[],
                selection: Selection::collapsed(caret),
                ime_preedit: None,
                search_matches: &[],
                active_match_idx: None,
                config: &cfg,
                viewport_w: 600.0,
                viewport_h: 400.0,
                scroll_y: 0.0,
                scroll_x,
            },
            &mut FallbackMeasurer,
            Some(cache),
        )
    }

    fn text_cmds(frame: &ViewportFrame) -> Vec<(f32, f32, String)> {
        frame
            .draw_cmds
            .iter()
            .filter_map(|c| match c {
                DrawCmd::Text {
                    x,
                    baseline_y,
                    text,
                    ..
                } if !text.trim().is_empty() && text.parse::<u32>().is_err() => {
                    Some((*x, *baseline_y, text.clone()))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn source_mode_soft_wraps_long_lines() {
        // Regression #288: long lines in Source/TXT mode ran off the right edge.
        let md = format!("kurz\n{}ENDE\nletzte\n", "Wort ".repeat(60));
        let rope = Rope::from_str(&md);
        let mut cache = BlockHeightCache::new();
        let end = md.find("ENDE").unwrap() + 4;
        let frame = source_frame(&rope, end, true, 0.0, &mut cache);

        let texts = text_cmds(&frame);
        let long_rows: Vec<_> = texts
            .iter()
            .filter(|t| t.2.contains("Wort") || t.2.contains("ENDE"))
            .collect();
        assert!(long_rows.len() >= 3, "{texts:?}");
        // All rows start at the same x and stack downwards.
        assert!(
            long_rows
                .windows(2)
                .all(|w| w[0].0 == w[1].0 && w[1].1 > w[0].1)
        );
        // Nothing overflows the viewport, no horizontal scrolling.
        assert!((frame.total_width - frame.viewport_w).abs() < 0.5);
        // The line below follows the wrapped rows; the caret sits on the last wrapped row.
        let last = texts.iter().find(|t| t.2 == "letzte").unwrap();
        assert!(last.1 > long_rows.last().unwrap().1);
        let last_row_y = frame
            .visual_rows
            .iter()
            .filter(|r| r.line_idx == 1)
            .map(|r| r.y_doc)
            .fold(f32::MIN, f32::max);
        assert!(
            (frame.caret_doc_rect.y - last_row_y).abs() < 0.5,
            "{frame:?}"
        );
        // Gutter-free: one row per wrapped segment, exact document height.
        let rows_for_line = frame.visual_rows.iter().filter(|r| r.line_idx == 1).count();
        assert_eq!(rows_for_line, long_rows.len());
    }

    #[test]
    fn source_mode_without_wrap_scrolls_horizontally() {
        let md = format!("{}ENDE\n", "Wort ".repeat(60));
        let rope = Rope::from_str(&md);
        let mut cache = BlockHeightCache::new();
        let end = md.find("ENDE").unwrap() + 4;

        let unscrolled = source_frame(&rope, end, false, 0.0, &mut cache);
        assert!(unscrolled.total_width > unscrolled.viewport_w + 200.0);
        let base_x = text_cmds(&unscrolled)[0].0;

        let frame = source_frame(&rope, end, false, 300.0, &mut cache);
        assert!((frame.scroll_x - 300.0).abs() < f32::EPSILON);
        let shifted_x = text_cmds(&frame)[0].0;
        assert!((base_x - shifted_x - 300.0).abs() < 0.5);
        // Caret and hit-testing agree in scrolled viewport coordinates.
        assert!(
            (frame.caret_viewport_rect.x - (unscrolled.caret_viewport_rect.x - 300.0)).abs() < 0.5
        );
        let hit = frame.hit_test(
            frame.caret_viewport_rect.x,
            frame.caret_viewport_rect.y + 4.0,
        );
        assert!(
            matches!(hit, HitTarget::Text { byte_offset, .. } if byte_offset == end),
            "{hit:?}"
        );

        // With word wrap on, a stale scroll_x is ignored.
        let wrapped = source_frame(&rope, end, true, 300.0, &mut cache);
        assert!(wrapped.scroll_x.abs() < f32::EPSILON);
    }

    #[test]
    fn hit_testing_maps_click_on_checkbox_and_code_badge() {
        let md = "- [ ] Klick mich\n\n```rust\nlet x = 1;\n```\n";
        let rope = Rope::from_str(md);
        let blocks = parse_blocks(&rope);
        let mut measurer = FallbackMeasurer;
        let cfg = LayoutConfig::default();

        let frame = build_viewport_frame(
            &ViewportRequest {
                rope: &rope,
                blocks: &blocks,
                selection: Selection::collapsed(0),
                ime_preedit: None,
                search_matches: &[],
                active_match_idx: None,
                config: &cfg,
                viewport_w: 900.0,
                viewport_h: 600.0,
                scroll_y: 0.0,
                scroll_x: 0.0,
            },
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
            &ViewportRequest {
                rope: &rope,
                blocks: &blocks,
                selection: Selection::collapsed(0),
                ime_preedit: None,
                search_matches: &[],
                active_match_idx: None,
                config: &cfg,
                viewport_w: 960.0,
                viewport_h: 600.0,
                scroll_y: 0.0,
                scroll_x: 0.0,
            },
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
            &ViewportRequest {
                rope: &rope,
                blocks: &blocks,
                selection: Selection::collapsed(0),
                ime_preedit: None,
                search_matches: &[],
                active_match_idx: None,
                config: &cfg,
                viewport_w: 960.0,
                viewport_h: 600.0,
                scroll_y: 0.0,
                scroll_x: 0.0,
            },
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
            &ViewportRequest {
                rope: &rope,
                blocks: &blocks,
                selection: Selection::collapsed(0),
                ime_preedit: None,
                search_matches: &[],
                active_match_idx: None,
                config: &cfg,
                viewport_w: 900.0,
                viewport_h: 600.0,
                scroll_y: 0.0,
                scroll_x: 0.0,
            },
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
            &ViewportRequest {
                rope: &rope,
                blocks: &blocks,
                selection: Selection::collapsed(0),
                ime_preedit: None,
                search_matches: &[],
                active_match_idx: None,
                config: &cfg,
                viewport_w: 820.0,
                viewport_h: 600.0,
                scroll_y: 0.0,
                scroll_x: 0.0,
            },
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
                &ViewportRequest {
                    rope: &rope,
                    blocks: &blocks,
                    selection: Selection::collapsed(0),
                    ime_preedit: None,
                    search_matches: &[],
                    active_match_idx: None,
                    config: &cfg,
                    viewport_w: 920.0,
                    viewport_h: 680.0,
                    scroll_y,
                    scroll_x: 0.0,
                },
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
                &ViewportRequest {
                    rope: &rope,
                    blocks: &blocks,
                    selection: Selection::collapsed(0),
                    ime_preedit: None,
                    search_matches: &[],
                    active_match_idx: None,
                    config: &cfg,
                    viewport_w: 920.0,
                    viewport_h: 680.0,
                    scroll_y: probe_scroll,
                    scroll_x: 0.0,
                },
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

    #[test]
    fn cached_block_heights_stay_stable_across_mid_document_edits() {
        let mut md = String::new();
        let long_sentence = "Ein umgebrochener Fließtextabsatz mit **Fettschrift**, *Kursivschrift* und `Code`, dessen exakte Höhe gecacht bleibt. ";
        for i in 0..80 {
            md.push_str(&format!(
                "## Kapitel {i}\n\n{}\n\n",
                long_sentence.repeat(4)
            ));
        }

        let mut rope = Rope::from_str(&md);
        let mut blocks = parse_blocks(&rope);
        let mut measurer = FallbackMeasurer;
        let cfg = LayoutConfig::default();
        let mut height_cache = BlockHeightCache::new();

        // Measure all blocks once by scrolling top -> bottom.
        let mut scroll_y = 0.0f32;
        let mut full_measured_h = 0.0f32;
        while scroll_y <= 30_000.0 {
            let f = build_viewport_frame_cached(
                &ViewportRequest {
                    rope: &rope,
                    blocks: &blocks,
                    selection: Selection::collapsed(0),
                    ime_preedit: None,
                    search_matches: &[],
                    active_match_idx: None,
                    config: &cfg,
                    viewport_w: 900.0,
                    viewport_h: 640.0,
                    scroll_y,
                    scroll_x: 0.0,
                },
                &mut measurer,
                Some(&mut height_cache),
            );
            full_measured_h = f.total_height;
            if scroll_y > f.total_height {
                break;
            }
            scroll_y += 500.0;
        }

        // Edit a single character at mid-document (which does not add a wrap line) and verify
        // off-screen prefix and suffix blocks retain their exact measured heights immediately!
        let mid = rope.clamp_char_boundary(rope.len_bytes() / 2);
        let old_rope = rope.clone();
        rope.replace_range(mid..mid, "x");
        kmark_syntax::reparse_changed_rope(&old_rope, &rope, &mut blocks);

        let after_edit_frame = build_viewport_frame_cached(
            &ViewportRequest {
                rope: &rope,
                blocks: &blocks,
                selection: Selection::collapsed(mid + 1),
                ime_preedit: None,
                search_matches: &[],
                active_match_idx: None,
                config: &cfg,
                viewport_w: 900.0,
                viewport_h: 640.0,
                scroll_y: full_measured_h * 0.5,
                scroll_x: 0.0,
            },
            &mut measurer,
            Some(&mut height_cache),
        );

        let scrolled_top_frame = build_viewport_frame_cached(
            &ViewportRequest {
                rope: &rope,
                blocks: &blocks,
                selection: Selection::collapsed(mid + 1),
                ime_preedit: None,
                search_matches: &[],
                active_match_idx: None,
                config: &cfg,
                viewport_w: 900.0,
                viewport_h: 640.0,
                scroll_y: 0.0,
                scroll_x: 0.0,
            },
            &mut measurer,
            Some(&mut height_cache),
        );

        assert!(
            (after_edit_frame.total_height - scrolled_top_frame.total_height).abs() < 0.01,
            "total_height drifted after mid-document edit ({} vs {})",
            after_edit_frame.total_height,
            scrolled_top_frame.total_height
        );
    }
}
