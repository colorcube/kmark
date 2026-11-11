use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum FontRole {
    SansBody,
    Monospace,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub line_height: f32,
    pub space_width: f32,
}

#[derive(Clone, Debug)]
pub struct MeasuredRun {
    pub total_width: f32,
    pub metrics: FontMetrics,
    /// `(relative_byte_end, cumulative_x_px)` for each extended grapheme cluster in `text`.
    pub grapheme_ends: Vec<(usize, f32)>,
}

impl MeasuredRun {
    /// Finds the cumulative X position at `rel_byte` within the measured run.
    #[must_use]
    pub fn x_at_rel_byte(&self, rel_byte: usize) -> f32 {
        if rel_byte == 0 {
            return 0.0;
        }
        for &(end_b, x) in &self.grapheme_ends {
            if end_b >= rel_byte {
                return x;
            }
        }
        self.total_width
    }

    /// Snaps a local `local_x` coordinate (relative to run start) to the nearest grapheme byte boundary.
    #[must_use]
    pub fn rel_byte_at_x(&self, local_x: f32) -> usize {
        if local_x <= 0.0 || self.grapheme_ends.is_empty() {
            return 0;
        }
        let mut prev_b = 0usize;
        let mut prev_x = 0.0f32;
        for &(end_b, end_x) in &self.grapheme_ends {
            let mid = (prev_x + end_x) * 0.5;
            if local_x < mid {
                return prev_b;
            }
            if local_x <= end_x {
                return end_b;
            }
            prev_b = end_b;
            prev_x = end_x;
        }
        prev_b
    }
}

/// Abstract text shaping/measurement interface.
///
/// In Qt6, implemented via `QTextLayout` / `QFontMetricsF` (cached in Rust) so word-wrapping,
/// table columns, selection rects, and caret placement match `QPainter` to the exact subpixel.
/// In headless CLI and tests, [`FallbackMeasurer`] provides deterministic Unicode-aware metrics.
pub trait TextMeasurer {
    fn font_metrics(
        &mut self,
        role: FontRole,
        size_px: f32,
        bold: bool,
        italic: bool,
    ) -> FontMetrics;
    fn measure_run(
        &mut self,
        text: &str,
        role: FontRole,
        size_px: f32,
        bold: bool,
        italic: bool,
    ) -> MeasuredRun;

    /// Returns `(natural_width_px, natural_height_px)` for local images in WYSIWYG mode,
    /// or `None` when unavailable (e.g. remote URLs or headless fallback).
    fn image_dimensions(&mut self, _src: &str) -> Option<(f32, f32)> {
        None
    }
}

/// Pure-Rust deterministic text measurer used by unit tests, benchmarks, and `kmark-cli`.
#[derive(Clone, Debug, Default)]
pub struct FallbackMeasurer;

impl TextMeasurer for FallbackMeasurer {
    fn font_metrics(
        &mut self,
        role: FontRole,
        size_px: f32,
        _bold: bool,
        _italic: bool,
    ) -> FontMetrics {
        let s = size_px.max(6.0);
        let space_ratio = match role {
            FontRole::SansBody => 0.30,
            FontRole::Monospace => 0.60,
        };
        FontMetrics {
            ascent: s * 0.82,
            descent: s * 0.24,
            line_height: s * 1.38,
            space_width: s * space_ratio,
        }
    }

    fn measure_run(
        &mut self,
        text: &str,
        role: FontRole,
        size_px: f32,
        bold: bool,
        italic: bool,
    ) -> MeasuredRun {
        let metrics = self.font_metrics(role, size_px, bold, italic);
        let s = size_px.max(6.0);
        let weight_boost = if bold { 1.05 } else { 1.0 };
        let mut x = 0.0f32;
        let mut ends = Vec::new();

        for (rel_idx, g) in text.grapheme_indices(true) {
            let w = match role {
                FontRole::Monospace => {
                    let cols = UnicodeWidthStr::width(g).max(1) as f32;
                    cols * s * 0.60
                }
                FontRole::SansBody => {
                    if g == " " {
                        metrics.space_width
                    } else if g == "\t" {
                        metrics.space_width * 4.0
                    } else {
                        let cols = UnicodeWidthStr::width(g);
                        if cols >= 2 {
                            s * 1.0 * weight_boost
                        } else {
                            s * 0.54 * weight_boost
                        }
                    }
                }
            };
            x += w;
            ends.push((rel_idx.saturating_add(g.len()), x));
        }

        MeasuredRun {
            total_width: x,
            metrics,
            grapheme_ends: ends,
        }
    }
}
