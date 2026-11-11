#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Randomized property-based test suite verifying that incremental reparsing
//! (`reparse_changed_rope_with_min_lines` and `reparse_changed_rope`) produces the
//! exact same `Vec<Block>` and `Vec<OutlineEntry>` as a full `parse_blocks` across
//! arbitrary single-site edits, fence openings/closings, Setext/ATX transitions,
//! GFM tables, alerts, and scattered `Replace All` edits.

use kmark_buffer::Rope;
use kmark_syntax::{
    Block, extract_outline, parse_blocks, reparse_changed_rope, reparse_changed_rope_with_min_lines,
};

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

/// Snippets without raw code-fence delimiters (used heavily so multi-block edits
/// stay on the incremental window splice + tail-offset shift path rather than
/// triggering the code-fence full-parse fallback).
const NON_FENCE_SNIPPETS: &[&str] = &[
    "",
    "\n",
    "\n\n",
    "# ",
    "## Überschrift\n\n",
    "### TODO Kapitel\n\n",
    "===\n",
    "---\n",
    "***\n",
    "Normaler Absatz mit **fett**, *kursiv*, `code` und TODO Marker.\n\n",
    "- Listenpunkt A\n- [ ] Aufgabe offen\n- [x] Aufgabe erledigt\n\n",
    "1. Erster Schritt\n2. Zweiter Schritt\n\n",
    "> Einfaches Zitat\n\n",
    "> [!NOTE] Hinweis\n> Mehrzeiliger Callout-Inhalt.\n\n",
    "> [!WARNING] Warnung\n\n",
    "| Spalte A | Spalte B | Wert |\n| :--- | :---: | ---: |\n| Eins | Zwei | 42 € |\n\n",
    "| Zusatz | Zeile | 99 |\n",
    "![Diagramm](docs/arch.png \"Abb. 1\"){width=60%}\n\n",
];

/// Full snippet set including code fences (opened and unclosed) to also verify
/// seamless fallback transitions whenever code fences enter or leave the edit window.
const ALL_SNIPPETS: &[&str] = &[
    "",
    "\n",
    "\n\n",
    "# ",
    "## Überschrift\n",
    "### TODO Kapitel\n\n",
    "===\n",
    "---\n",
    "***\n",
    "Normaler Absatz mit **fett**, *kursiv*, `code` und TODO Marker.\n",
    "- Listenpunkt A\n- [ ] Aufgabe offen\n- [x] Aufgabe erledigt\n",
    "1. Erster Schritt\n2. Zweiter Schritt\n",
    "> Einfaches Zitat\n",
    "> [!NOTE] Hinweis\n> Mehrzeiliger Callout-Inhalt.\n",
    "> [!WARNING] Warnung\n",
    "```rust\nfn demo() {\n    // # Kein Heading im Codefence!\n}\n```\n",
    "```\n",
    "~~~\n",
    "| Spalte A | Spalte B | Wert |\n| :--- | :---: | ---: |\n| Eins | Zwei | 42 € |\n",
    "| Zusatz | Zeile | 99 |\n",
    "![Diagramm](docs/arch.png \"Abb. 1\"){width=60%}\n",
];

fn assert_block_invariants(rope: &Rope, blocks: &[Block], ctx: &str) {
    let total_bytes = rope.len_bytes();
    let total_lines = rope.line_count();

    let mut prev_byte_end = 0usize;
    let mut prev_line_end = 0usize;
    for (idx, blk) in blocks.iter().enumerate() {
        assert!(
            blk.byte_range.start <= blk.byte_range.end,
            "{ctx}: block[{idx}] inverted byte_range {:?}",
            blk.byte_range
        );
        assert!(
            blk.byte_range.end <= total_bytes,
            "{ctx}: block[{idx}] byte_range {:?} exceeds len_bytes {total_bytes}",
            blk.byte_range
        );
        assert!(
            rope.is_char_boundary(blk.byte_range.start)
                && rope.is_char_boundary(blk.byte_range.end),
            "{ctx}: block[{idx}] byte_range {:?} not on char boundary",
            blk.byte_range
        );
        assert!(
            blk.byte_range.start >= prev_byte_end,
            "{ctx}: block[{idx}] overlaps previous block ({:?} < {prev_byte_end})",
            blk.byte_range
        );
        assert!(
            blk.line_range.start <= blk.line_range.end && blk.line_range.end <= total_lines,
            "{ctx}: block[{idx}] invalid line_range {:?} (total_lines={total_lines})",
            blk.line_range
        );
        assert!(
            blk.line_range.start >= prev_line_end,
            "{ctx}: block[{idx}] line_range {:?} overlaps prev_line_end {prev_line_end}",
            blk.line_range
        );
        prev_byte_end = blk.byte_range.end;
        prev_line_end = blk.line_range.end;
    }
    if !blocks.is_empty() {
        assert_eq!(
            blocks[0].line_range.start, 0,
            "{ctx}: first block line != 0"
        );
        assert_eq!(
            prev_line_end, total_lines,
            "{ctx}: blocks do not cover all lines"
        );
    }
}

fn apply_random_markdown_edit(
    rope: &mut Rope,
    rng: &mut SplitMix64,
    allow_fences: bool,
    bounded_deletion: bool,
) {
    let palette = if allow_fences {
        ALL_SNIPPETS
    } else {
        NON_FENCE_SNIPPETS
    };

    match rng.gen_range(8) {
        // Scattered multi-site `Replace All` across the document (like Bug #1)
        0 => {
            let text = rope.to_string_lossless();
            let (needle, replacement) = match rng.gen_range(5) {
                0 => ("SPECIAL_TODO", "# SPECIAL_TODO"),
                1 => ("# SPECIAL_TODO", "SPECIAL_TODO"),
                2 => ("TODO", "# TODO"),
                3 => ("```", ""),
                _ => ("- ", "1. "),
            };
            if text.contains(needle) {
                let replaced = text.replace(needle, replacement);
                *rope = Rope::from_str(&replaced);
            } else {
                let pos = rope.clamp_char_boundary(rng.gen_range(rope.len_bytes() + 1));
                rope.replace_range(
                    pos..pos,
                    "\nSPECIAL_TODO Erster Punkt\n\nNormaler Text\n\nSPECIAL_TODO Zweiter Punkt\n",
                );
            }
        }
        // Localized single-site edit (span <= 64 bytes) so `BlankLines` walls before/after
        // stay untouched and exercise window splicing + tail block shifting directly
        1..=5 => {
            let a = rope.clamp_char_boundary(rng.gen_range(rope.len_bytes() + 1));
            let max_del = rng.gen_range(64);
            let b = rope.clamp_char_boundary(a.saturating_add(max_del).min(rope.len_bytes()));
            let ins = palette[rng.gen_range(palette.len())];
            rope.replace_range(a..b, ins);
        }
        // Multi-block insertion / deletion / replacement
        _ => {
            let a = rope.clamp_char_boundary(rng.gen_range(rope.len_bytes() + 1));
            let b = if bounded_deletion {
                let max_del = rng.gen_range(600);
                rope.clamp_char_boundary(a.saturating_add(max_del).min(rope.len_bytes()))
            } else {
                rope.clamp_char_boundary(rng.gen_range(rope.len_bytes() + 1))
            };
            let range = a.min(b)..a.max(b);
            let snippet_count = rng.gen_range(4);
            let mut ins = String::new();
            for _ in 0..snippet_count {
                ins.push_str(palette[rng.gen_range(palette.len())]);
            }
            rope.replace_range(range, &ins);
        }
    }
}

#[test]
fn proptest_incremental_parser_matches_full_parse_with_zero_line_threshold() {
    let mut incremental_hits = 0usize;
    let mut total_steps = 0usize;

    for seed in 0..300_u64 {
        let mut rng = SplitMix64::new(0xFA57_BA5E_0000_0000 ^ seed);
        let allow_fences = seed % 3 == 0;
        let palette = if allow_fences {
            ALL_SNIPPETS
        } else {
            NON_FENCE_SNIPPETS
        };

        // Build a diverse multi-block Markdown document separated by blank lines.
        let mut init = String::new();
        let init_chunks = 10 + rng.gen_range(22);
        for _ in 0..init_chunks {
            init.push_str(palette[rng.gen_range(palette.len())]);
        }

        let mut rope = Rope::from_str(&init);
        let mut incremental_blocks = parse_blocks(&rope);
        assert_block_invariants(&rope, &incremental_blocks, &format!("seed={seed} init"));

        let steps = 35 + rng.gen_range(20);
        for step in 0..steps {
            let old_rope = rope.clone();
            apply_random_markdown_edit(&mut rope, &mut rng, allow_fences, false);

            let used_incremental =
                reparse_changed_rope_with_min_lines(&old_rope, &rope, &mut incremental_blocks, 0);
            if used_incremental {
                incremental_hits += 1;
            }
            total_steps += 1;

            let full_blocks = parse_blocks(&rope);
            let ctx = format!("seed={seed} step={step} inc={used_incremental}");
            assert_block_invariants(&rope, &incremental_blocks, &ctx);
            assert_eq!(
                incremental_blocks,
                full_blocks,
                "{ctx}: incremental blocks diverged from full parse!\nDocument:\n{}",
                rope.to_string_lossless()
            );
            assert_eq!(
                extract_outline(&rope, &incremental_blocks),
                extract_outline(&rope, &full_blocks),
                "{ctx}: outline diverged"
            );
        }
    }

    assert!(
        incremental_hits >= 5_000,
        "expected incremental window path to execute at least 5,000 times, got {incremental_hits} / {total_steps}"
    );
}

#[test]
fn proptest_production_reparse_changed_rope_above_300_lines_matches_full_parse() {
    let mut incremental_hits = 0usize;
    let mut total_steps = 0usize;

    for seed in 0..80_u64 {
        let mut rng = SplitMix64::new(0xFB9E_0300_0000_0000 ^ seed);
        let allow_fences = seed % 4 == 0;

        // Build a ~480-line document so production `reparse_changed_rope` (min_lines = 300)
        // genuinely exercises the incremental splice + tail-offset shift path.
        let mut init = String::with_capacity(28_000);
        for section in 0..115_usize {
            let marker = if section % 25 == 5 {
                "SPECIAL_TODO "
            } else {
                ""
            };
            init.push_str(&format!(
                "## Abschnitt {section}\n\n{marker}Absatz {section} mit Text und **Formatierung**.\n\n"
            ));
            if section % 7 == 0 {
                init.push_str(NON_FENCE_SNIPPETS[rng.gen_range(NON_FENCE_SNIPPETS.len())]);
                init.push('\n');
            }
        }

        let mut rope = Rope::from_str(&init);
        assert!(
            rope.line_count() > 450,
            "initial line count {} must exceed MIN_INCREMENTAL_PARSE_LINES (300)",
            rope.line_count()
        );

        let mut prod_blocks = parse_blocks(&rope);
        let mut probe_blocks = prod_blocks.clone();

        let steps = 25;
        for step in 0..steps {
            let old_rope = rope.clone();
            apply_random_markdown_edit(&mut rope, &mut rng, allow_fences, true);

            // Production entrypoint (`MIN_INCREMENTAL_PARSE_LINES = 300`)
            reparse_changed_rope(&old_rope, &rope, &mut prod_blocks);
            // Instrumented probe with identical `min_lines = 300` threshold
            let used_incremental =
                reparse_changed_rope_with_min_lines(&old_rope, &rope, &mut probe_blocks, 300);
            if used_incremental {
                incremental_hits += 1;
            }
            total_steps += 1;

            let full_blocks = parse_blocks(&rope);
            let ctx = format!("large seed={seed} step={step} inc={used_incremental}");
            assert_block_invariants(&rope, &prod_blocks, &ctx);
            assert_eq!(prod_blocks, probe_blocks, "{ctx}: prod vs probe mismatch");
            assert_eq!(
                prod_blocks, full_blocks,
                "{ctx}: production incremental blocks diverged from full parse!"
            );
            assert_eq!(
                extract_outline(&rope, &prod_blocks),
                extract_outline(&rope, &full_blocks),
                "{ctx}: outline diverged"
            );
        }
    }

    assert!(
        incremental_hits >= 1_200,
        "expected production >300-line incremental path to execute at least 1,200 times, got {incremental_hits} / {total_steps}"
    );
}
