//! Interactive hit-boxes, pointer hit-testing, and vertical visual-band caret navigation.

use std::ops::Range;

use kmark_syntax::AlertKind;

use crate::engine::{PositionedGlyphSpan, RectF, ViewportFrame, VisualRow};

#[derive(Clone, Debug)]
pub struct CheckboxHitBox {
    pub rect_doc: RectF,
    pub bracket_range: Range<usize>,
    pub checked: bool,
}

#[derive(Clone, Debug)]
pub struct CodeLangHitBox {
    pub rect_doc: RectF,
    pub open_line_start: usize,
    pub language: String,
}

#[derive(Clone, Debug)]
pub struct CalloutBadgeHitBox {
    pub rect_doc: RectF,
    pub open_line_start: usize,
    pub kind: AlertKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HitTarget {
    Checkbox {
        bracket_range: Range<usize>,
        checked: bool,
    },
    CodeLanguageBadge {
        open_line_start: usize,
        language: String,
    },
    CalloutBadge {
        open_line_start: usize,
        kind: AlertKind,
    },
    Text {
        byte_offset: usize,
        link_url: Option<String>,
    },
}

impl ViewportFrame {
    /// Hit-tests a click at viewport pixel coordinates `(vx, vy)`.
    #[must_use]
    pub fn hit_test(&self, vx: f32, vy: f32) -> HitTarget {
        let doc_y = vy + self.scroll_y;
        let doc_x = vx;

        for badge in &self.code_lang_badges {
            if badge.rect_doc.contains(doc_x, doc_y) {
                return HitTarget::CodeLanguageBadge {
                    open_line_start: badge.open_line_start,
                    language: badge.language.clone(),
                };
            }
        }

        for badge in &self.callout_badges {
            if badge.rect_doc.contains(doc_x, doc_y) {
                return HitTarget::CalloutBadge {
                    open_line_start: badge.open_line_start,
                    kind: badge.kind,
                };
            }
        }

        for cb in &self.checkboxes {
            if cb.rect_doc.contains(doc_x, doc_y) {
                return HitTarget::Checkbox {
                    bracket_range: cb.bracket_range.clone(),
                    checked: cb.checked,
                };
            }
        }

        let Some(first_row) = self.visual_rows.first() else {
            return HitTarget::Text {
                byte_offset: 0,
                link_url: None,
            };
        };

        // Pick closest visual row containing `doc_y` (or nearest row above/below).
        let mut chosen = first_row;
        let mut best_dist = f32::MAX;
        for row in &self.visual_rows {
            if doc_y >= row.y_doc && doc_y <= row.y_doc + row.height {
                // Inside vertical band: if multiple sub-rows share the same Y (e.g. table cells!),
                // pick the cell/row horizontally closest to `doc_x`.
                let row_left = row.spans.first().map_or(row.default_x, |s| s.x);
                let row_right = row.spans.last().map_or(row.default_x, |s| s.x + s.width);
                let h_dist = if doc_x < row_left {
                    row_left - doc_x
                } else if doc_x > row_right {
                    doc_x - row_right
                } else {
                    0.0
                };
                if h_dist < best_dist {
                    best_dist = h_dist;
                    chosen = row;
                }
            } else if best_dist == f32::MAX {
                let d = if doc_y < row.y_doc {
                    row.y_doc - doc_y
                } else {
                    doc_y - (row.y_doc + row.height)
                };
                if d < best_dist {
                    best_dist = d;
                    chosen = row;
                }
            }
        }
        // Fallback nearest vertical row if `doc_y` fell in inter-block spacing.
        if !(doc_y >= chosen.y_doc && doc_y <= chosen.y_doc + chosen.height) {
            let mut min_v = f32::MAX;
            for row in &self.visual_rows {
                let v = if doc_y < row.y_doc {
                    row.y_doc - doc_y
                } else if doc_y > row.y_doc + row.height {
                    doc_y - (row.y_doc + row.height)
                } else {
                    0.0
                };
                if v < min_v {
                    min_v = v;
                    chosen = row;
                }
            }
        }

        byte_offset_in_visual_row(chosen, doc_x)
    }

    /// Computes target byte offset when pressing `Up` (`delta = -1`) or `Down` (`delta = +1`).
    #[must_use]
    pub fn move_vertical(
        &self,
        current_caret: usize,
        preferred_x: Option<f32>,
        delta: i32,
    ) -> (usize, f32) {
        let target_x = preferred_x.unwrap_or(self.caret_doc_rect.x);
        if self.visual_rows.is_empty() {
            return (current_caret, target_x);
        }

        // Collect distinct vertical bands (`y_doc`) sorted ascending.
        let mut bands: Vec<f32> = Vec::new();
        for r in &self.visual_rows {
            if bands
                .last()
                .is_none_or(|&prev| (r.y_doc - prev).abs() > 0.5)
            {
                bands.push(r.y_doc);
            }
        }
        let cur_y = self.caret_doc_rect.y;
        let cur_band = bands
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                ((**a - cur_y).abs())
                    .partial_cmp(&((**b - cur_y).abs()))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map_or(0, |(idx, _)| idx);

        let next_band = if delta < 0 {
            cur_band.saturating_sub(delta.unsigned_abs() as usize)
        } else {
            (cur_band.saturating_add(delta as usize)).min(bands.len().saturating_sub(1))
        };
        let band_y = bands.get(next_band).copied().unwrap_or(cur_y) + 2.0;
        match self.hit_test(target_x, band_y - self.scroll_y) {
            HitTarget::Text { byte_offset, .. } => (byte_offset, target_x),
            HitTarget::Checkbox { bracket_range, .. } => (bracket_range.start, target_x),
            HitTarget::CodeLanguageBadge {
                open_line_start, ..
            }
            | HitTarget::CalloutBadge {
                open_line_start, ..
            } => (open_line_start, target_x),
        }
    }
}

fn byte_offset_in_visual_row(row: &VisualRow, doc_x: f32) -> HitTarget {
    let visible_spans: Vec<&PositionedGlyphSpan> =
        row.spans.iter().filter(|s| !s.is_hidden_marker).collect();
    if visible_spans.is_empty() {
        return HitTarget::Text {
            byte_offset: row.doc_range.start,
            link_url: None,
        };
    }
    if let Some(first) = visible_spans.first()
        && doc_x <= first.x
    {
        return HitTarget::Text {
            byte_offset: first.doc_range.start,
            link_url: first.link_url.clone(),
        };
    }
    for span in &visible_spans {
        if doc_x >= span.x && doc_x <= span.x + span.width {
            let rel = span.measured.rel_byte_at_x(doc_x - span.x);
            return HitTarget::Text {
                byte_offset: span
                    .doc_range
                    .start
                    .saturating_add(rel)
                    .min(span.doc_range.end),
                link_url: span.link_url.clone(),
            };
        }
    }
    if let Some(last) = visible_spans.last() {
        return HitTarget::Text {
            byte_offset: last.doc_range.end,
            link_url: last.link_url.clone(),
        };
    }
    HitTarget::Text {
        byte_offset: row.doc_range.end,
        link_url: None,
    }
}
