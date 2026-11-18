//! Soft wrapping for the line-based view modes (*Source* and *Plain Text*).
//!
//! [`LineWrapCache`] keeps the number of visual rows for every logical line plus prefix sums, so
//! the first visible line for any `scroll_y` is found in `O(log N)` and an edit only re-estimates
//! the changed lines (via [`Rope::diff_byte_range`]). Off-screen lines use a width model
//! (monospace advance × Unicode display width, or an average glyph width for proportional
//! fonts); every line that is actually laid out replaces its estimate with the exact row count.

use kmark_buffer::Rope;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::measurer::FontRole;

/// Tab stop in character cells, matching the Qt shell's `QTextOption::setTabStopDistance`.
const TAB_CELLS: f32 = 4.0;

/// Returns the byte offsets where each visual row starts (always beginning with `0`).
///
/// `ends` holds `(grapheme_byte_end, cumulative_x)` pairs for `text`. Rows break after the last
/// whitespace that fits into `avail_w`; a word wider than a row is broken between graphemes.
/// Trailing whitespace may overhang the row edge so it never starts a new row on its own.
#[must_use]
pub fn wrap_row_starts(text: &str, ends: &[(usize, f32)], avail_w: f32) -> Vec<usize> {
    let mut starts = vec![0usize];
    if avail_w <= 0.0 {
        return starts;
    }
    let mut row_start_b = 0usize;
    let mut row_start_w = 0.0f32;
    let mut last_ws_break: Option<(usize, f32)> = None;
    let mut prev_b = 0usize;
    let mut prev_w = 0.0f32;

    for &(end_b, w) in ends {
        let g = text.get(prev_b..end_b).unwrap_or("");
        let is_ws = !g.is_empty() && g.chars().all(char::is_whitespace);
        if !is_ws {
            while w - row_start_w > avail_w && prev_b > row_start_b {
                let (break_b, break_w) = last_ws_break
                    .filter(|&(b, _)| b > row_start_b)
                    .unwrap_or((prev_b, prev_w));
                starts.push(break_b);
                row_start_b = break_b;
                row_start_w = break_w;
                last_ws_break = None;
            }
        } else {
            last_ws_break = Some((end_b, w));
        }
        prev_b = end_b;
        prev_w = w;
    }
    starts
}

/// Approximates `(grapheme_byte_end, cumulative_x)` pairs for `text` without a font measurer.
#[must_use]
pub fn approximate_grapheme_ends(text: &str, cell_w: f32) -> Vec<(usize, f32)> {
    let mut out = Vec::new();
    let mut x = 0.0f32;
    for (idx, g) in text.grapheme_indices(true) {
        let cells = if g == "\t" {
            TAB_CELLS
        } else {
            g.width().max(1) as f32
        };
        x += cells * cell_w;
        out.push((idx.saturating_add(g.len()), x));
    }
    out
}

/// Layout parameters that invalidate every cached row count when they change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LineWrapKey {
    avail_w_tenths: u32,
    cell_w_hundredths: u32,
    monospace: bool,
}

/// Per-tab cache of visual row counts for soft-wrapped line modes.
#[derive(Clone, Debug, Default)]
pub struct LineWrapCache {
    key: Option<LineWrapKey>,
    last_rope: Option<Rope>,
    rows: Vec<u32>,
    /// `prefix[i]` = total visual rows of lines `0..i` (`len = rows.len() + 1`).
    prefix: Vec<u64>,
    prefix_dirty: bool,
    cell_w: f32,
    avail_w: f32,
    shape_cap: usize,
    /// Widest unwrapped line seen so far (`role`, font size in tenths, width) for the
    /// horizontal scrollbar when word wrap is off.
    widest: Option<(FontRole, u32, f32)>,
}

impl LineWrapCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Synchronizes the cache with `rope` and the current wrap geometry.
    pub(crate) fn prepare(
        &mut self,
        rope: &Rope,
        avail_w: f32,
        cell_w: f32,
        role: FontRole,
        shape_cap: usize,
    ) {
        let key = LineWrapKey {
            avail_w_tenths: (avail_w.max(1.0) * 10.0).round() as u32,
            cell_w_hundredths: (cell_w.max(0.01) * 100.0).round() as u32,
            monospace: role == FontRole::Monospace,
        };
        self.cell_w = cell_w.max(0.01);
        self.avail_w = avail_w.max(1.0);
        self.shape_cap = shape_cap;

        if self.key != Some(key) || self.last_rope.is_none() {
            self.key = Some(key);
            self.last_rope = Some(rope.clone());
            self.rebuild_all(rope);
            return;
        }
        let Some(prev) = self.last_rope.replace(rope.clone()) else {
            self.rebuild_all(rope);
            return;
        };
        if prev.ptr_eq(rope) {
            return;
        }
        let Some((old_edit, new_edit)) = prev.diff_byte_range(rope) else {
            return;
        };
        let first = prev.byte_to_line_col(old_edit.start).line;
        let old_last = prev.byte_to_line_col(old_edit.end).line;
        let new_last = rope.byte_to_line_col(new_edit.end).line;
        if self.rows.len() != prev.line_count() || old_last >= self.rows.len() {
            self.rebuild_all(rope);
            return;
        }
        let replacement: Vec<u32> = (first..=new_last)
            .map(|l| self.estimate_rows(&rope.line_text(l)))
            .collect();
        self.rows.splice(first..=old_last, replacement);
        if self.rows.len() != rope.line_count() {
            self.rebuild_all(rope);
            return;
        }
        self.prefix_dirty = true;
    }

    fn rebuild_all(&mut self, rope: &Rope) {
        let text = rope.to_string_lossless();
        let rows: Vec<u32> = text.split('\n').map(|l| self.estimate_rows(l)).collect();
        self.rows = rows;
        self.prefix_dirty = true;
    }

    fn estimate_rows(&self, line: &str) -> u32 {
        let capped = if line.len() > self.shape_cap {
            let mut cut = self.shape_cap;
            while cut > 0 && !line.is_char_boundary(cut) {
                cut = cut.saturating_sub(1);
            }
            line.get(..cut).unwrap_or("")
        } else {
            line
        };
        if capped.is_empty() {
            return 1;
        }
        let ends = approximate_grapheme_ends(capped, self.cell_w);
        let n = wrap_row_starts(capped, &ends, self.avail_w).len();
        u32::try_from(n).unwrap_or(u32::MAX).max(1)
    }

    fn ensure_prefix(&mut self) {
        if !self.prefix_dirty && self.prefix.len() == self.rows.len().saturating_add(1) {
            return;
        }
        self.prefix.clear();
        self.prefix.reserve(self.rows.len().saturating_add(1));
        let mut acc = 0u64;
        self.prefix.push(0);
        for &r in &self.rows {
            acc = acc.saturating_add(u64::from(r));
            self.prefix.push(acc);
        }
        self.prefix_dirty = false;
    }

    /// Total number of visual rows in the document.
    pub(crate) fn total_rows(&mut self) -> u64 {
        self.ensure_prefix();
        self.prefix.last().copied().unwrap_or(0)
    }

    /// Visual row index at which logical line `line` begins.
    pub(crate) fn row_of_line(&mut self, line: usize) -> u64 {
        self.ensure_prefix();
        let idx = line.min(self.prefix.len().saturating_sub(1));
        self.prefix.get(idx).copied().unwrap_or(0)
    }

    /// Logical line containing visual row `row`.
    pub(crate) fn line_at_row(&mut self, row: u64) -> usize {
        self.ensure_prefix();
        // Last line whose starting row is `<= row`.
        self.prefix
            .partition_point(|&start| start <= row)
            .saturating_sub(1)
            .min(self.rows.len().saturating_sub(1))
    }

    /// Remembers the widest unwrapped line measured for `role` / `font_px`.
    pub(crate) fn note_widest_line(&mut self, role: FontRole, font_px: f32, width: f32) {
        let px_tenths = (font_px.max(1.0) * 10.0).round() as u32;
        self.widest = match self.widest {
            Some((r, px, w)) if r == role && px == px_tenths => Some((r, px, w.max(width))),
            _ => Some((role, px_tenths, width)),
        };
    }

    /// Widest unwrapped line remembered by [`Self::note_widest_line`].
    pub(crate) fn widest_line(&self) -> f32 {
        self.widest.map_or(0.0, |(_, _, w)| w)
    }

    /// Records the exact row count of a laid-out line; returns `true` if the estimate changed.
    pub(crate) fn record_exact(&mut self, line: usize, rows: u32) -> bool {
        let rows = rows.max(1);
        match self.rows.get_mut(line) {
            Some(slot) if *slot != rows => {
                *slot = rows;
                self.prefix_dirty = true;
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn starts(text: &str, avail: f32) -> Vec<usize> {
        let ends = approximate_grapheme_ends(text, 10.0);
        wrap_row_starts(text, &ends, avail)
    }

    #[test]
    fn wraps_after_whitespace_and_breaks_overlong_words() {
        // 10 px per cell, 60 px per row = 6 cells.
        assert_eq!(starts("abc def ghi", 60.0), vec![0, 4, 8]);
        assert_eq!(starts("abcdefghijkl", 60.0), vec![0, 6]);
        assert_eq!(starts("kurz", 60.0), vec![0]);
        // Trailing whitespace overhangs instead of opening an empty row.
        assert_eq!(starts("abcdef      ", 60.0), vec![0]);
    }

    #[test]
    fn cache_tracks_edits_and_row_lookup() {
        let mut rope = Rope::from_str("kurz\nabc def ghi\n\nende");
        let mut cache = LineWrapCache::new();
        cache.prepare(&rope, 60.0, 10.0, FontRole::Monospace, 4096);
        assert_eq!(cache.total_rows(), 1 + 3 + 1 + 1);
        assert_eq!(cache.row_of_line(2), 4);
        assert_eq!(cache.line_at_row(0), 0);
        assert_eq!(cache.line_at_row(3), 1);
        assert_eq!(cache.line_at_row(4), 2);
        assert_eq!(cache.line_at_row(99), 3);

        // Splitting line 1 into two lines only re-estimates the touched lines.
        rope.replace_range(8..8, "\n");
        cache.prepare(&rope, 60.0, 10.0, FontRole::Monospace, 4096);
        assert_eq!(cache.total_rows(), 1 + 1 + 2 + 1 + 1);

        assert!(cache.record_exact(2, 5));
        assert_eq!(cache.total_rows(), 1 + 1 + 5 + 1 + 1);
        assert!(!cache.record_exact(2, 5));
    }
}
