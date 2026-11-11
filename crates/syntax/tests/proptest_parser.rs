#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Randomized property-based test suite verifying that `reparse_changed_rope`
//! produces the exact same `Vec<Block>` and `Vec<OutlineEntry>` as a full
//! `parse_blocks` across arbitrary single-site edits, fence openings/closings,
//! Setext/ATX transitions, GFM tables, alerts, and scattered `Replace All` edits.

use kmark_buffer::Rope;
use kmark_syntax::{Block, extract_outline, parse_blocks, reparse_changed_rope};

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

const MD_SNIPPETS: &[&str] = &[
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

#[test]
fn proptest_incremental_parser_matches_full_parse_across_350_sequences() {
    for seed in 0..350_u64 {
        let mut rng = SplitMix64::new(0xFA57_BA5E_0000_0000 ^ seed);

        // Build a diverse initial multi-block Markdown document.
        let mut init = String::new();
        let init_chunks = 6 + rng.gen_range(20);
        for _ in 0..init_chunks {
            init.push_str(MD_SNIPPETS[rng.gen_range(MD_SNIPPETS.len())]);
        }

        let mut rope = Rope::from_str(&init);
        let mut incremental_blocks = parse_blocks(&rope);
        assert_block_invariants(&rope, &incremental_blocks, &format!("seed={seed} init"));

        let steps = 35 + rng.gen_range(20);
        for step in 0..steps {
            let old_rope = rope.clone();

            match rng.gen_range(8) {
                // Scattered multi-site `Replace All` across the whole document (like Bug #1!)
                0 | 1 => {
                    let text = rope.to_string_lossless();
                    let (needle, replacement) = match rng.gen_range(5) {
                        0 => ("TODO", "# TODO"),
                        1 => ("# ", ""),
                        2 => ("\n\n", "\n---\n"),
                        3 => ("```", ""),
                        _ => ("- ", "1. "),
                    };
                    if text.contains(needle) {
                        let replaced = text.replace(needle, replacement);
                        rope = Rope::from_str(&replaced);
                    } else {
                        let pos = rope.clamp_char_boundary(rng.gen_range(rope.len_bytes() + 1));
                        rope.replace_range(
                            pos..pos,
                            "\nTODO Erster Punkt\n\nNormaler Text\n\nTODO Zweiter Punkt\n",
                        );
                    }
                }
                // Arbitrary single-range insertion / deletion / replacement (including mid-fence & mid-table)
                _ => {
                    let a = rope.clamp_char_boundary(rng.gen_range(rope.len_bytes() + 1));
                    let b = rope.clamp_char_boundary(rng.gen_range(rope.len_bytes() + 1));
                    let range = a.min(b)..a.max(b);
                    let snippet_count = rng.gen_range(4);
                    let mut ins = String::new();
                    for _ in 0..snippet_count {
                        ins.push_str(MD_SNIPPETS[rng.gen_range(MD_SNIPPETS.len())]);
                    }
                    rope.replace_range(range, &ins);
                }
            }

            reparse_changed_rope(&old_rope, &rope, &mut incremental_blocks);
            let full_blocks = parse_blocks(&rope);

            let ctx = format!("seed={seed} step={step}");
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
}
