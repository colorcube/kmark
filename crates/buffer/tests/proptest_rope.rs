#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Randomized property-based test suite comparing `Rope` operations, B-tree
//! splits/merges, line/column indexing, UTF-8 boundary clamping, hashing, and
//! `diff_byte_range` against a `String` reference model.

use kmark_buffer::Rope;

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

const FRAGMENTS: &[&str] = &[
    "",
    "a",
    "aaaa",
    " ",
    "\n",
    "\r\n",
    "\n\n",
    "# Überschrift ÄÖÜß\n",
    "ä",
    "¤",
    "€",
    "日本語と한글",
    "🚀✨",
    "👨‍👩‍👧‍👦",
    "e\u{0301}",
    "| Spalte 1 | Spalte 2 |\n| --- | ---: |\n",
    "```rust\nfn main() {}\n```\n",
    "> [!NOTE] Hinweis mit **Fettdruck**\n",
];

fn clamp_str_char_boundary(s: &str, offset: usize) -> usize {
    let mut pos = offset.min(s.len());
    while pos > 0 && !s.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

fn fnv1a_bytes(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

fn random_payload(rng: &mut SplitMix64) -> String {
    match rng.gen_range(10) {
        0 => String::new(),
        1..=6 => {
            let count = 1 + rng.gen_range(4);
            let mut s = String::new();
            for _ in 0..count {
                s.push_str(FRAGMENTS[rng.gen_range(FRAGMENTS.len())]);
            }
            s
        }
        7..=8 => {
            // Multi-hundred-byte chunk to cross leaf split/coalesce thresholds (`MAX_LEAF` / `MIN_LEAF`).
            let count = 12 + rng.gen_range(28);
            let mut s = String::new();
            for _ in 0..count {
                s.push_str(FRAGMENTS[rng.gen_range(FRAGMENTS.len())]);
            }
            s
        }
        _ => {
            // Multi-kilobyte burst forcing B-tree height growth.
            let count = 60 + rng.gen_range(80);
            let mut s = String::new();
            for _ in 0..count {
                s.push_str(FRAGMENTS[rng.gen_range(FRAGMENTS.len())]);
            }
            s
        }
    }
}

fn verify_rope_invariants(rope: &Rope, reference: &str, rng: &mut SplitMix64, ctx: &str) {
    assert_eq!(rope.len_bytes(), reference.len(), "{ctx}: len_bytes");
    assert_eq!(rope.is_empty(), reference.is_empty(), "{ctx}: is_empty");
    assert_eq!(
        rope.len_chars(),
        reference.chars().count(),
        "{ctx}: len_chars"
    );
    assert_eq!(
        rope.len_utf16(),
        reference.encode_utf16().count(),
        "{ctx}: len_utf16"
    );
    let expected_newlines = reference.bytes().filter(|&b| b == b'\n').count();
    assert_eq!(
        rope.line_count(),
        expected_newlines + 1,
        "{ctx}: line_count"
    );

    assert_eq!(
        rope.to_string_lossless(),
        reference,
        "{ctx}: to_string_lossless"
    );
    let streamed: String = rope.chunks().collect();
    assert_eq!(streamed, reference, "{ctx}: chunks()");

    // Probe random arbitrary byte offsets (including mid-UTF-8 continuation bytes and past-EOF).
    let max_probe = reference.len() + 8;
    for _ in 0..6 {
        let probe = rng.gen_range(max_probe + 1);
        let expected_clamped = clamp_str_char_boundary(reference, probe);
        assert_eq!(
            rope.clamp_char_boundary(probe),
            expected_clamped,
            "{ctx}: clamp_char_boundary({probe})"
        );
        let expected_is_boundary = probe <= reference.len() && reference.is_char_boundary(probe);
        assert_eq!(
            rope.is_char_boundary(probe),
            expected_is_boundary,
            "{ctx}: is_char_boundary({probe})"
        );
        assert_eq!(
            rope.byte_at(probe),
            reference.as_bytes().get(probe).copied(),
            "{ctx}: byte_at({probe})"
        );
        let expected_char = if probe >= reference.len() {
            None
        } else {
            reference[expected_clamped..].chars().next()
        };
        assert_eq!(
            rope.char_at(probe),
            expected_char,
            "{ctx}: char_at({probe})"
        );

        // Random slice & hash probe
        let probe2 = rng.gen_range(max_probe + 1);
        let s = clamp_str_char_boundary(reference, probe.min(probe2));
        let e = clamp_str_char_boundary(reference, probe.max(probe2)).max(s);
        assert_eq!(
            rope.slice_to_string(probe.min(probe2)..probe.max(probe2)),
            &reference[s..e],
            "{ctx}: slice_to_string"
        );
        assert_eq!(
            rope.hash_byte_range(s..e),
            fnv1a_bytes(&reference.as_bytes()[s..e]),
            "{ctx}: hash_byte_range({s}..{e})"
        );

        // Line / column roundtrip
        let lc = rope.byte_to_line_col(probe);
        let expected_line = reference.as_bytes()[..expected_clamped]
            .iter()
            .filter(|&&b| b == b'\n')
            .count();
        assert_eq!(lc.line, expected_line, "{ctx}: byte_to_line_col.line");
        let roundtrip = rope.line_col_to_byte(lc.line, lc.col_bytes);
        assert_eq!(
            roundtrip, expected_clamped,
            "{ctx}: line_col_to_byte roundtrip"
        );
    }

    // Spot-check logical lines against manual `\n` splitting.
    let ref_lines: Vec<&str> = reference.split('\n').collect();
    assert_eq!(rope.line_count(), ref_lines.len(), "{ctx}: split('\\n')");
    for _ in 0..4 {
        let line_idx = rng.gen_range(ref_lines.len());
        assert_eq!(
            rope.line_text(line_idx),
            ref_lines[line_idx],
            "{ctx}: line_text({line_idx})"
        );
    }
}

#[test]
fn proptest_rope_matches_string_across_400_random_edit_sequences() {
    for seed in 0..400_u64 {
        let mut rng = SplitMix64::new(0xC0DE_CAFE_0000_0000 ^ seed);
        let mut rope = Rope::new();
        let mut reference = String::new();

        let steps = 45 + rng.gen_range(25);
        for step in 0..steps {
            let prev_rope = rope.clone();
            let prev_ref = reference.clone();

            // Pick arbitrary raw byte endpoints (deliberately including non-char boundaries!)
            let len_plus = reference.len() + 4;
            let raw_a = rng.gen_range(len_plus + 1);
            let raw_b = rng.gen_range(len_plus + 1);

            let (raw_range, inserted) = match rng.gen_range(10) {
                // Pure deletion (including occasional full-buffer clear to exercise coalescing)
                0..=2 => {
                    if rng.gen_range(12) == 0 {
                        (0..reference.len(), String::new())
                    } else {
                        (raw_a.min(raw_b)..raw_a.max(raw_b), String::new())
                    }
                }
                // Pure insertion
                3..=6 => (raw_a..raw_a, random_payload(&mut rng)),
                // Replacement
                _ => (raw_a.min(raw_b)..raw_a.max(raw_b), random_payload(&mut rng)),
            };

            let start = clamp_str_char_boundary(&reference, raw_range.start);
            let end = clamp_str_char_boundary(&reference, raw_range.end).max(start);
            reference.replace_range(start..end, &inserted);
            rope.replace_range(raw_range, &inserted);

            let ctx = format!("seed={seed} step={step}");
            verify_rope_invariants(&rope, &reference, &mut rng, &ctx);

            // Verify `PartialEq` and `diff_byte_range` against `prev_rope`!
            if prev_ref == reference {
                assert_eq!(prev_rope, rope, "{ctx}: PartialEq on identical content");
                assert_eq!(
                    prev_rope.diff_byte_range(&rope),
                    None,
                    "{ctx}: diff_byte_range must be None when unchanged"
                );
            } else {
                assert_ne!(prev_rope, rope, "{ctx}: PartialEq on changed content");
                let (old_r, new_r) = prev_rope
                    .diff_byte_range(&rope)
                    .unwrap_or_else(|| panic!("{ctx}: expected Some diff_byte_range"));
                assert_eq!(
                    old_r.start, new_r.start,
                    "{ctx}: diff prefix start must match"
                );
                assert!(
                    prev_rope.is_char_boundary(old_r.start)
                        && prev_rope.is_char_boundary(old_r.end),
                    "{ctx}: old_r not on UTF-8 boundary: {old_r:?}"
                );
                assert!(
                    rope.is_char_boundary(new_r.start) && rope.is_char_boundary(new_r.end),
                    "{ctx}: new_r not on UTF-8 boundary: {new_r:?}"
                );

                // Applying the computed diff window to `prev_ref` must reconstruct `reference`!
                let mut reconstructed = prev_ref.clone();
                reconstructed.replace_range(old_r, &reference[new_r]);
                assert_eq!(
                    reconstructed, reference,
                    "{ctx}: diff_byte_range window failed to reconstruct new text"
                );
            }
        }

        // Also verify freshly built single-allocation Rope equals fragmented multi-edit Rope.
        let fresh_rope = Rope::from_str(&reference);
        assert_eq!(
            rope, fresh_rope,
            "seed={seed}: fragmented Rope != fresh Rope"
        );
        assert_eq!(rope.diff_byte_range(&fresh_rope), None);
    }
}
