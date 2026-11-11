#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Randomized property-based test suite exercising `Session` editing commands,
//! armed inline formatting, smart `Enter`/`Tab`/`Backspace`, GFM table mutations,
//! `Replace All`, viewport layout, and complete `Undo`/`Redo` history roundtrips.

use kmark_engine::{Command, KeyCode, KeyModifiers, SearchOptions, Session};
use kmark_layout::ViewMode;
use kmark_syntax::{AlertKind, ColumnAlign, ImageSize, extract_outline, parse_blocks};

#[derive(Clone, Copy)]
struct SplitMix64(u64);

impl SplitMix64 {
    const fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn gen_range(&mut self, upper_exclusive: usize) -> usize {
        if upper_exclusive <= 1 {
            return 0;
        }
        (self.next_u64() as usize) % upper_exclusive
    }
}

const TYPED_CHUNKS: &[&str] = &[
    "a",
    "Klar",
    " ",
    "Übersicht",
    "TODO",
    "42 €",
    "🚀",
    "Zeile eins\nZeile zwei",
    "日本語",
];

fn assert_session_invariants(session: &Session, ctx: &str) {
    let doc = session.active_doc().expect("active_doc must exist");
    assert!(
        doc.rope.is_char_boundary(doc.selection.anchor),
        "{ctx}: selection.anchor {} not on char boundary (len={})",
        doc.selection.anchor,
        doc.rope.len_bytes()
    );
    assert!(
        doc.rope.is_char_boundary(doc.selection.head),
        "{ctx}: selection.head {} not on char boundary (len={})",
        doc.selection.head,
        doc.rope.len_bytes()
    );

    let full_blocks = parse_blocks(&doc.rope);
    assert_eq!(
        doc.blocks, full_blocks,
        "{ctx}: doc.blocks diverged from full parse_blocks"
    );
    let full_outline = extract_outline(&doc.rope, &full_blocks);
    assert_eq!(
        doc.outline, full_outline,
        "{ctx}: doc.outline diverged from extract_outline"
    );
}

#[test]
fn proptest_session_commands_and_undo_redo_roundtrip_across_200_sequences() {
    for seed in 0..200_u64 {
        let mut rng = SplitMix64::new(0x5E55_1042_0000_0000 ^ seed);
        let mut session = Session::new_headless();
        assert!(!session.active_doc().unwrap().is_dirty());

        let steps = 40 + rng.gen_range(25);
        for step in 0..steps {
            let ctx = format!("seed={seed} step={step}");

            match rng.gen_range(28) {
                0..=5 => {
                    let chunk = TYPED_CHUNKS[rng.gen_range(TYPED_CHUNKS.len())];
                    session.insert_text(chunk);
                }
                6 => session.execute_command(Command::ToggleBold),
                7 => session.execute_command(Command::ToggleItalic),
                8 => session.execute_command(Command::ToggleStrike),
                9 => session.execute_command(Command::ToggleHighlight),
                10 => session.execute_command(Command::ToggleInlineCode),
                11 => {
                    let lvl = rng.gen_range(7) as u8;
                    session.execute_command(Command::SetHeading(lvl));
                }
                12 => session.execute_command(Command::ToggleBulletList),
                13 => session.execute_command(Command::ToggleOrderedList),
                14 => session.execute_command(Command::ToggleTaskList),
                15 => session.execute_command(Command::ToggleBlockQuote),
                16 => {
                    let kind = match rng.gen_range(5) {
                        0 => AlertKind::Note,
                        1 => AlertKind::Tip,
                        2 => AlertKind::Important,
                        3 => AlertKind::Warning,
                        _ => AlertKind::Caution,
                    };
                    session.execute_command(Command::InsertAlert(kind));
                }
                17 => session.execute_command(Command::InsertHorizontalRule),
                18 => session.execute_command(Command::InsertCodeFence {
                    language: "rust".to_owned(),
                }),
                19 => session.execute_command(Command::InsertTable {
                    cols: 2 + rng.gen_range(3),
                    rows: 1 + rng.gen_range(3),
                }),
                20 => {
                    let tbl_cmd = match rng.gen_range(7) {
                        0 => Command::TableInsertRowAbove,
                        1 => Command::TableInsertRowBelow,
                        2 => Command::TableDeleteRow,
                        3 => Command::TableInsertColLeft,
                        4 => Command::TableInsertColRight,
                        5 => Command::TableDeleteCol,
                        _ => Command::TableSetAlign(ColumnAlign::Right),
                    };
                    session.execute_command(tbl_cmd);
                }
                21 => session.execute_command(Command::InsertImage {
                    alt: "Bild".to_owned(),
                    url: "demo.png".to_owned(),
                    caption: Some("Abb. 1".to_owned()),
                    size: Some(ImageSize::Percent(50)),
                }),
                22 => {
                    let mode = match rng.gen_range(4) {
                        0 => ViewMode::Wysiwyg,
                        1 => ViewMode::LivePreview,
                        2 => ViewMode::Source,
                        _ => ViewMode::PlainText,
                    };
                    session.execute_command(Command::SetViewMode(mode));
                }
                23 => {
                    session.set_search_query("TODO", SearchOptions::default());
                    let _ = session.replace_all_matches("## TODO");
                }
                24 => {
                    // Random jump or selection drag across arbitrary byte offsets
                    let len = session.active_doc().unwrap().rope.len_bytes();
                    let a = session
                        .active_doc()
                        .unwrap()
                        .rope
                        .clamp_char_boundary(rng.gen_range(len + 1));
                    let b = session
                        .active_doc()
                        .unwrap()
                        .rope
                        .clamp_char_boundary(rng.gen_range(len + 1));
                    if let Some(tab) = session.active_doc_mut() {
                        tab.selection = kmark_buffer::Selection::new(a, b);
                    }
                }
                25 => {
                    if rng.gen_range(2) == 0 {
                        session.execute_command(Command::Undo);
                    } else {
                        session.execute_command(Command::Redo);
                    }
                }
                _ => {
                    let key = match rng.gen_range(10) {
                        0 => KeyCode::Enter,
                        1 => KeyCode::Backspace,
                        2 => KeyCode::Delete,
                        3 => KeyCode::Tab,
                        4 => KeyCode::Backtab,
                        5 => KeyCode::Left,
                        6 => KeyCode::Right,
                        7 => KeyCode::Up,
                        8 => KeyCode::Down,
                        _ => KeyCode::Home,
                    };
                    let mods = KeyModifiers {
                        shift: rng.gen_range(4) == 0,
                        ctrl: rng.gen_range(6) == 0,
                        alt: false,
                    };
                    session.handle_key(key, mods);
                }
            }

            assert_session_invariants(&session, &ctx);

            // Periodically exercise full layout frame building & scroll virtualization cache!
            if step % 8 == 0 {
                let frame = session
                    .render_viewport_fallback(860.0, 540.0)
                    .expect("render_viewport_fallback");
                assert!(frame.total_height.is_finite() && frame.total_height > 0.0);
            }
        }

        // Capture final text after the random sequence, then rewind ALL Undo steps back to empty,
        // and replay ALL Redo steps forward to verify lossless history symmetry!
        let final_text = session.active_doc().unwrap().rope.to_string_lossless();

        let mut undo_steps = 0usize;
        while session.toolbar_state().can_undo {
            session.execute_command(Command::Undo);
            assert_session_invariants(&session, &format!("seed={seed} undo={undo_steps}"));
            undo_steps += 1;
            assert!(
                undo_steps <= 10_000,
                "seed={seed}: Undo loop did not terminate"
            );
        }

        assert_eq!(
            session.active_doc().unwrap().rope.to_string_lossless(),
            "",
            "seed={seed}: full Undo rewind did not return to initial empty buffer"
        );
        assert!(
            !session.active_doc().unwrap().is_dirty(),
            "seed={seed}: document still marked dirty after full Undo rewind"
        );

        for redo_idx in 0..undo_steps {
            assert!(
                session.toolbar_state().can_redo,
                "seed={seed}: expected can_redo at step {redo_idx}/{undo_steps}"
            );
            session.execute_command(Command::Redo);
            assert_session_invariants(&session, &format!("seed={seed} redo={redo_idx}"));
        }
        assert_eq!(
            session.active_doc().unwrap().rope.to_string_lossless(),
            final_text,
            "seed={seed}: replaying {undo_steps} Redo steps did not restore exact document"
        );

        // If the random sequence itself ended mid-Undo, drain the remaining redo tail too.
        let mut tail_redos = 0usize;
        while session.toolbar_state().can_redo {
            session.execute_command(Command::Redo);
            assert_session_invariants(&session, &format!("seed={seed} tail_redo={tail_redos}"));
            tail_redos += 1;
            assert!(
                tail_redos <= 10_000,
                "seed={seed}: Redo tail loop did not terminate"
            );
        }
    }
}
