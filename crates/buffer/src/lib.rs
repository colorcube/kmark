//! `kmark-buffer` (L0): Persistent copy-on-write B-tree rope, text encodings,
//! atomic file persistence, and infallible undo/redo history.

pub mod encoding;
pub mod history;
pub mod rope;

pub use encoding::{DecodedDocument, IoError, LineEnding, TextEncoding};
pub use history::{DocumentHistory, EditGroup, Selection};
pub use rope::{LineCol, Rope, Summary};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rope_basic_insert_delete_and_lines() {
        let mut r = Rope::from_str("# Titel\nErste Zeile\nZweite Zeile");
        assert_eq!(r.line_count(), 3);
        assert_eq!(r.line_text(0), "# Titel");
        assert_eq!(r.line_text(1), "Erste Zeile");
        assert_eq!(r.line_text(2), "Zweite Zeile");

        let lc = r.byte_to_line_col(9);
        assert_eq!(lc.line, 1);
        assert_eq!(lc.col_bytes, 1);

        r.replace_range(8..13, "Neue");
        assert_eq!(r.line_text(1), "Neue Zeile");
    }

    #[test]
    fn rope_large_document_and_unicode_safety() {
        let mut src = String::new();
        for i in 0..2_500 {
            src.push_str(&format!(
                "## Abschnitt {i} — ÄÖÜß 한글 日本語 🚀\nText in Zeile {i}.\n"
            ));
        }
        let mut rope = Rope::from_str(&src);
        assert_eq!(rope.line_count(), 5_001);
        let snapshot = rope.clone();

        let mid = rope.line_start_byte(2_500);
        rope.replace_range(mid..mid, "**Eingefügt** ");
        assert!(rope.line_text(2_500).starts_with("**Eingefügt** "));
        // Persistent snapshot stays untouched.
        assert!(!snapshot.line_text(2_500).starts_with("**Eingefügt** "));

        // Clamping inside multi-byte UTF-8 characters never panics.
        for probe in mid..mid + 25 {
            let aligned = rope.clamp_char_boundary(probe);
            assert!(rope.is_char_boundary(aligned));
        }
    }

    #[test]
    fn encoding_roundtrip_and_atomic_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("probe.md");
        let content = "# Grußformel\nPreis: 42 € — „Zitat“\n";

        for enc in TextEncoding::all() {
            encoding::write_file_atomic(&path, content, *enc, LineEnding::CrLf).unwrap();
            let loaded = encoding::read_file_auto(&path).unwrap();
            assert_eq!(loaded.text, content);
            assert_eq!(loaded.line_ending, LineEnding::CrLf);
        }
    }

    #[test]
    fn history_typing_coalesces_until_broken() {
        let mut hist = DocumentHistory::default();
        let mut rope = Rope::new();
        let mut sel = Selection::collapsed(0);

        for ch in ["H", "a", "l", "l", "o"] {
            let before_r = rope.clone();
            let before_s = sel;
            rope.replace_range(sel.head..sel.head, ch);
            sel = Selection::collapsed(sel.head + ch.len());
            hist.record(before_r, before_s, rope.clone(), sel, EditGroup::Typing);
        }
        assert_eq!(rope.to_string_lossless(), "Hallo");

        // All 5 keystrokes coalesce into a single undo step!
        let (undone, undone_sel) = hist.undo().unwrap();
        assert_eq!(undone.to_string_lossless(), "");
        assert_eq!(undone_sel, Selection::collapsed(0));
        assert!(!hist.can_undo());

        let (redone, _) = hist.redo().unwrap();
        assert_eq!(redone.to_string_lossless(), "Hallo");
    }

    #[cfg(unix)]
    #[test]
    fn atomic_save_preserves_existing_permissions_and_applies_umask_to_new_files() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();

        // 1. Brand-new file must NOT stay 0600 from NamedTempFile; it must include group/other read
        //    under normal umasks (or match 0666 & !umask).
        let fresh = dir.path().join("fresh.md");
        encoding::write_file_atomic(&fresh, "hello\n", TextEncoding::Utf8, LineEnding::Lf).unwrap();
        let fresh_mode = fs::metadata(&fresh).unwrap().permissions().mode() & 0o777;
        assert_ne!(
            fresh_mode, 0o600,
            "new file unexpectedly kept NamedTempFile 0600 mode"
        );

        // 2. Overwriting existing files preserves exact permission bits (e.g. 0644 and 0755).
        for expected_mode in [0o644_u32, 0o640_u32, 0o755_u32] {
            let file_path = dir.path().join(format!("mode_{expected_mode:o}.md"));
            fs::write(&file_path, "before\n").unwrap();
            fs::set_permissions(&file_path, fs::Permissions::from_mode(expected_mode)).unwrap();

            encoding::write_file_atomic(
                &file_path,
                "after atomic save\n",
                TextEncoding::Utf8,
                LineEnding::Lf,
            )
            .unwrap();

            let actual_mode = fs::metadata(&file_path).unwrap().permissions().mode() & 0o777;
            assert_eq!(actual_mode, expected_mode);
            assert_eq!(
                fs::read_to_string(&file_path).unwrap(),
                "after atomic save\n"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn atomic_save_follows_symlinks_without_replacing_link() {
        use std::fs;
        use std::os::unix::fs::{PermissionsExt, symlink};

        let dir = tempfile::tempdir().unwrap();
        let target_dir = dir.path().join("dotfiles");
        let link_dir = dir.path().join("home");
        fs::create_dir_all(&target_dir).unwrap();
        fs::create_dir_all(&link_dir).unwrap();

        let real_file = target_dir.join("notes.md");
        fs::write(&real_file, "# Old\n").unwrap();
        fs::set_permissions(&real_file, fs::Permissions::from_mode(0o644)).unwrap();

        // Chained relative symlink: link2.md -> link1.md -> ../dotfiles/notes.md
        let link1 = link_dir.join("link1.md");
        let link2 = link_dir.join("link2.md");
        symlink("../dotfiles/notes.md", &link1).unwrap();
        symlink("link1.md", &link2).unwrap();

        encoding::write_file_atomic(
            &link2,
            "# Updated via symlink\n",
            TextEncoding::Utf8,
            LineEnding::Lf,
        )
        .unwrap();

        assert!(
            fs::symlink_metadata(&link1)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(
            fs::symlink_metadata(&link2)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            fs::read_to_string(&real_file).unwrap(),
            "# Updated via symlink\n"
        );
        assert_eq!(
            fs::metadata(&real_file).unwrap().permissions().mode() & 0o777,
            0o644
        );

        // Dangling symlink pointing to a not-yet-created target file:
        let dangling_link = link_dir.join("dangling.md");
        let new_target = target_dir.join("created_through_link.md");
        symlink("../dotfiles/created_through_link.md", &dangling_link).unwrap();

        encoding::write_file_atomic(
            &dangling_link,
            "created!\n",
            TextEncoding::Utf8,
            LineEnding::Lf,
        )
        .unwrap();
        assert!(
            fs::symlink_metadata(&dangling_link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_to_string(&new_target).unwrap(), "created!\n");
    }
}
