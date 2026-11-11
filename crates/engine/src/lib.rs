//! `kmark-engine` (L3): Multi-tab document session, command registry, armed inline
//! formatting, smart structural editing, 2D table manipulation, find/replace, and crash recovery.

pub mod commands;
pub mod input;
pub mod recovery;
pub mod reference;
pub mod sample;
pub mod search;
pub mod session;

pub use commands::{ArmedStyle, COMMAND_CATALOG, Command, CommandDescriptor};
pub use input::{KeyCode, KeyModifiers};
pub use kmark_syntax::ImageSize;
pub use recovery::{RecoveryDraft, RecoveryStore};
pub use reference::{MARKDOWN_REFERENCE_DE, MARKDOWN_REFERENCE_EN};
pub use sample::{MARKDOWN_GUIDE_DE, MARKDOWN_GUIDE_EN, UiLanguage};
pub use search::{SearchOptions, SearchState};
pub use session::{
    ActiveToolbarState, DocumentTab, EngineError, ImageAtPoint, LayoutOptionsUpdate, PointerAction,
    RenderStateStamp, Session,
};

#[cfg(test)]
mod tests {
    use kmark_layout::ViewMode;
    use kmark_syntax::ColumnAlign;

    use super::*;

    #[test]
    fn armed_bold_formatting_never_inserts_empty_asterisks() {
        let mut session = Session::new_headless();
        session.execute_command(Command::ToggleBold);

        // Buffer is STILL completely empty until the user actually types a character!
        assert_eq!(session.active_doc().unwrap().rope.to_string_lossless(), "");
        assert!(session.toolbar_state().bold);

        session.insert_text("K");
        assert_eq!(
            session.active_doc().unwrap().rope.to_string_lossless(),
            "**K**"
        );
        session.insert_text("lar");
        assert_eq!(
            session.active_doc().unwrap().rope.to_string_lossless(),
            "**Klar**"
        );
    }

    #[test]
    fn smart_list_continuation_and_exit_on_empty_item() {
        let mut session = Session::new_headless();
        session.insert_text("1. Erster Punkt");
        session.handle_key(KeyCode::Enter, KeyModifiers::default());
        assert_eq!(
            session.active_doc().unwrap().rope.to_string_lossless(),
            "1. Erster Punkt\n2. "
        );

        // Pressing Enter again on empty `2. ` exits the list cleanly!
        session.handle_key(KeyCode::Enter, KeyModifiers::default());
        assert_eq!(
            session.active_doc().unwrap().rope.to_string_lossless(),
            "1. Erster Punkt\n"
        );
    }

    #[test]
    fn table_insert_tab_navigation_and_alignment_preserve_markdown() {
        let mut session = Session::new_headless();
        session.execute_command(Command::InsertTable { cols: 2, rows: 1 });
        session.insert_text("Produkt");
        session.handle_key(KeyCode::Tab, KeyModifiers::default());
        session.insert_text("Preis");
        session.handle_key(KeyCode::Tab, KeyModifiers::default());
        session.insert_text("KMark");
        session.handle_key(KeyCode::Tab, KeyModifiers::default());
        session.insert_text("0 €");

        // Align second column right
        session.execute_command(Command::TableSetAlign(ColumnAlign::Right));
        let md = session.active_doc().unwrap().rope.to_string_lossless();
        assert!(md.contains("Produkt"));
        assert!(md.contains("0 €"));
        assert!(md.contains("---:"));
        assert_eq!(md.lines().count(), 3);

        // Pressing Tab at the very end of the table appends a brand new row!
        session.handle_key(KeyCode::Tab, KeyModifiers::default());
        let md_after = session.active_doc().unwrap().rope.to_string_lossless();
        assert_eq!(md_after.lines().count(), 4);
    }

    #[test]
    fn wysiwyg_left_right_skips_hidden_markers_and_undo_restores_all() {
        let mut session = Session::new_headless();
        session.insert_text("Vor **Fett** Nach");
        session.execute_command(Command::SetViewMode(ViewMode::Wysiwyg));

        // Walk all the way left from the end: caret never lands on a half-marker byte
        for _ in 0..20 {
            session.handle_key(KeyCode::Left, KeyModifiers::default());
            let head = session.active_doc().unwrap().selection.head;
            assert!(session.active_doc().unwrap().rope.is_char_boundary(head));
        }

        session.execute_command(Command::Undo);
        assert_eq!(session.active_doc().unwrap().rope.to_string_lossless(), "");
        session.execute_command(Command::Redo);
        assert_eq!(
            session.active_doc().unwrap().rope.to_string_lossless(),
            "Vor **Fett** Nach"
        );
    }

    #[test]
    fn code_fence_language_command_and_complete_reference_tabs_work() {
        let mut session = Session::new_headless();
        session.execute_command(Command::InsertCodeFence {
            language: "rust".to_owned(),
        });
        session.insert_text("let x = 42;");
        assert!(session.toolbar_state().in_code_block);
        assert_eq!(session.toolbar_state().code_language, "rust");

        session.execute_command(Command::SetCodeFenceLanguage {
            open_line_start: None,
            language: "python".to_owned(),
        });
        assert_eq!(session.toolbar_state().code_language, "python");
        assert!(
            session
                .active_doc()
                .unwrap()
                .rope
                .to_string_lossless()
                .starts_with("```python\n")
        );

        session.set_ui_language(UiLanguage::English);
        let idx_en = session.open_reference_tab();
        assert!(session.tabs()[idx_en].outline.len() >= 8);

        session.set_ui_language(UiLanguage::German);
        let idx_de = session.open_reference_tab();
        assert!(session.tabs()[idx_de].outline.len() >= 8);
    }

    #[test]
    fn image_context_commands_caption_replace_and_size_work_with_undo() {
        let mut session = Session::new_headless();
        session.execute_command(Command::InsertImage {
            alt: "Foto".to_owned(),
            url: "alt.png".to_owned(),
            caption: None,
            size: None,
        });
        assert_eq!(
            session.active_doc().unwrap().rope.to_string_lossless(),
            "![Foto](alt.png)"
        );

        let _ = session.render_viewport_fallback(900.0, 600.0);
        // Find the rendered image badge in the viewport and verify `image_at` hits it!
        let row = &session
            .active_doc()
            .unwrap()
            .last_frame
            .as_ref()
            .unwrap()
            .visual_rows[0];
        let hit = session
            .image_at(row.spans[0].x + 5.0, row.y_doc + 5.0)
            .expect("expected image_at hit");
        assert_eq!(hit.src, "alt.png");
        assert_eq!(hit.caption, None);
        assert_eq!(hit.size, None);

        // 1. Add caption & set 50% relative size
        session.execute_command(Command::UpdateImage {
            range: hit.range,
            alt: hit.alt,
            url: hit.src,
            caption: Some("Abbildung 1: Übersicht".to_owned()),
            size: Some(ImageSize::Percent(50)),
        });
        let updated = session.active_doc().unwrap().rope.to_string_lossless();
        assert_eq!(
            updated,
            "![Foto](alt.png \"Abbildung 1: Übersicht\"){width=50%}"
        );

        // 2. Replace image while keeping caption & size
        session.execute_command(Command::UpdateImage {
            range: 0..updated.len(),
            alt: "Foto".to_owned(),
            url: "neu.png".to_owned(),
            caption: Some("Abbildung 1: Übersicht".to_owned()),
            size: Some(ImageSize::Percent(50)),
        });
        assert_eq!(
            session.active_doc().unwrap().rope.to_string_lossless(),
            "![Foto](neu.png \"Abbildung 1: Übersicht\"){width=50%}"
        );

        // 3. Undo reverts cleanly step by step
        session.execute_command(Command::Undo);
        assert_eq!(
            session.active_doc().unwrap().rope.to_string_lossless(),
            "![Foto](alt.png \"Abbildung 1: Übersicht\"){width=50%}"
        );
        session.execute_command(Command::Undo);
        assert_eq!(
            session.active_doc().unwrap().rope.to_string_lossless(),
            "![Foto](alt.png)"
        );
    }

    #[test]
    fn callout_type_switcher_preserves_custom_title_and_supports_plain_quote_and_undo() {
        let mut session = Session::new_headless();
        session.insert_text("> [!NOTE] Wichtiges Detail\n> Zweite Zeile.");

        let frame = session.render_viewport_fallback(900.0, 600.0).unwrap();
        assert_eq!(frame.callout_badges.len(), 1);
        let badge_rect = frame.callout_badges[0].rect_doc;

        let action = session.handle_pointer_press(
            badge_rect.x + 4.0,
            badge_rect.y + 4.0,
            1,
            KeyModifiers::default(),
        );
        assert_eq!(
            action,
            PointerAction::EditCalloutKind {
                open_line_start: 0,
                current_kind: kmark_syntax::AlertKind::Note,
            }
        );

        // Switch from NOTE -> WARNING while preserving custom title
        session.execute_command(Command::SetCalloutKind {
            open_line_start: Some(0),
            kind: Some(kmark_syntax::AlertKind::Warning),
        });
        assert_eq!(
            session.active_doc().unwrap().rope.to_string_lossless(),
            "> [!WARNING] Wichtiges Detail\n> Zweite Zeile."
        );

        // Convert Callout to plain blockquote
        session.execute_command(Command::SetCalloutKind {
            open_line_start: Some(0),
            kind: None,
        });
        assert_eq!(
            session.active_doc().unwrap().rope.to_string_lossless(),
            "> Wichtiges Detail\n> Zweite Zeile."
        );

        // Undo restores WARNING callout
        session.execute_command(Command::Undo);
        assert_eq!(
            session.active_doc().unwrap().rope.to_string_lossless(),
            "> [!WARNING] Wichtiges Detail\n> Zweite Zeile."
        );
    }

    #[test]
    fn replace_all_across_400_paragraphs_keeps_blocks_outline_and_tables_exact() {
        for target_indices in [[5usize, 130, 260, 385], [50usize, 80, 110, 140]] {
            let mut md = String::new();
            for i in 0..400 {
                if target_indices.contains(&i) {
                    md.push_str(&format!("TODO Abschnitt {i}\n\n"));
                } else {
                    md.push_str(&format!(
                        "Absatz Nummer {i} mit normalem Fließtext ohne Treffer.\n\n"
                    ));
                }
            }
            md.push_str("| Spalte A | Spalte B |\n| --- | --- |\n| Zelle 1 | Zelle 2 |\n");

            let mut session = Session::new_headless();
            if let Some(tab) = session.active_doc_mut() {
                let rope = kmark_buffer::Rope::from_str(&md);
                tab.saved_rope = rope.clone();
                tab.rope = rope;
                tab.saved_revision = Some(tab.revision);
                tab.reparse();
            }
            assert!(!session.active_doc().unwrap().is_dirty());

            session.set_search_query("TODO", SearchOptions::default());
            let replaced = session.replace_all_matches("# TODO");
            assert_eq!(replaced, 4);

            let doc = session.active_doc().unwrap();
            assert!(doc.is_dirty());
            assert_eq!(
                doc.outline.len(),
                4,
                "all 4 scattered headings must appear in outline"
            );
            assert_eq!(doc.blocks, kmark_syntax::parse_blocks(&doc.rope));

            // Verify table byte ranges at the end of the document remain 100% accurate for table ops!
            let tbl_pos = doc.rope.to_string_lossless().find("Zelle 1").unwrap();
            session.jump_to_byte_offset(tbl_pos);
            session.execute_command(Command::TableInsertRowBelow);

            let doc_after_tbl = session.active_doc().unwrap();
            assert_eq!(
                doc_after_tbl.blocks,
                kmark_syntax::parse_blocks(&doc_after_tbl.rope)
            );
            let table_count = doc_after_tbl
                .blocks
                .iter()
                .filter(|b| {
                    matches!(b.kind, kmark_syntax::BlockKind::Table(ref t) if t.body_rows.len() == 2)
                })
                .count();
            assert_eq!(table_count, 1);

            // Two Undos return cleanly to the original saved state (`is_dirty() == false` in O(1)).
            session.execute_command(Command::Undo);
            session.execute_command(Command::Undo);
            assert!(!session.active_doc().unwrap().is_dirty());
        }
    }

    #[test]
    fn crash_recovery_persists_all_dirty_tabs_restores_orphans_and_cleans_up() {
        let dir = tempfile::tempdir().unwrap();
        let rec_dir = dir.path().join("recovery");
        let existing_file = dir.path().join("article.md");
        std::fs::write(&existing_file, "# Saved On Disk\n\nOriginal paragraph.\n").unwrap();

        // Simulate Session A (crashed PID 999_999_991, guaranteed dead) with TWO dirty tabs:
        // Tab 0: opened from `existing_file` with unsaved edits (and NOT the active tab at flush time!)
        // Tab 1: untitled new tab with unsaved notes (active tab)
        {
            let mut crashed = Session::new_headless();
            crashed.recovery = RecoveryStore::with_directory_and_pid(&rec_dir, 999_999_991);
            crashed.open_file(&existing_file).unwrap();
            crashed.jump_to_byte_offset(crashed.active_doc().unwrap().rope.len_bytes());
            crashed.insert_text("\nUnsaved crash paragraph in file tab.\n");

            crashed.new_empty_tab();
            crashed.insert_text("# Scratch Idea\n\nUnsaved untitled tab.");

            // Flush must persist BOTH dirty tabs, not only the active one!
            crashed.flush_recovery_draft();
            let draft_files: Vec<_> = std::fs::read_dir(&rec_dir)
                .unwrap()
                .flatten()
                .map(|e| e.path())
                .collect();
            assert_eq!(
                draft_files.len(),
                2,
                "both dirty tabs must be written to recovery store"
            );
            // Drop `crashed` WITHOUT calling `clear_all_recovery_drafts()` to simulate SIGKILL/crash.
        }

        // Simulate Session B starting up afterwards:
        let mut restarted = Session::new_headless();
        restarted.recovery = RecoveryStore::with_directory_and_pid(&rec_dir, std::process::id());
        let recovered = restarted.recover_crash_drafts();
        assert_eq!(recovered, 2);
        assert_eq!(restarted.tab_count(), 2);

        // Both restored tabs are marked dirty, keep their paths/titles/contents, and dead-PID files are gone!
        restarted.set_active_tab(0);
        let t0 = restarted.active_doc().unwrap();
        assert!(t0.is_dirty());
        assert_eq!(
            t0.path.as_deref(),
            Some(std::fs::canonicalize(&existing_file).unwrap().as_path())
        );
        assert!(
            t0.rope
                .to_string_lossless()
                .contains("Unsaved crash paragraph")
        );

        restarted.set_active_tab(1);
        let t1 = restarted.active_doc().unwrap();
        assert!(t1.is_dirty());
        assert!(t1.path.is_none());
        assert_eq!(
            t1.rope.to_string_lossless(),
            "# Scratch Idea\n\nUnsaved untitled tab."
        );

        // Saving Tab 0 writes to the original file and removes its draft; clean shutdown removes remaining drafts.
        restarted.set_active_tab(0);
        restarted.save_active_tab().unwrap();
        assert!(
            std::fs::read_to_string(&existing_file)
                .unwrap()
                .contains("Unsaved crash paragraph")
        );

        restarted.clear_all_recovery_drafts();
        let leftover: Vec<_> = std::fs::read_dir(&rec_dir).unwrap().flatten().collect();
        assert!(
            leftover.is_empty(),
            "recovery directory must be empty after clean shutdown"
        );
    }
}
