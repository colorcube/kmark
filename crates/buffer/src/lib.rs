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
}
