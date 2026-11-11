use std::ops::Range;

use kmark_buffer::{Rope, Selection};
use kmark_syntax::{
    AlertKind, Block, BlockKind, ColumnAlign, ImageSize, InlineTokenKind, StyleFlags, TableBlock,
    TableRow, parse_inline_tokens,
};

use crate::measurer::{FontMetrics, FontRole, MeasuredRun, TextMeasurer};
use crate::theme::{Rgba, ThemePalette};

/// Primary editor mode — all four modes operate directly over the single [`Rope`] without buffer conversion.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ViewMode {
    /// Formatted WYSIWYG: Markdown delimiters stay hidden even on the active line.
    #[default]
    Wysiwyg,
    /// Live Preview: Formatted everywhere except the block/line containing the active caret.
    LivePreview,
    /// Raw Markdown source code with syntax coloring and line numbers.
    Source,
    /// Plain text mode (Notepad / KWrite style) with line numbers and zero Markdown styling.
    PlainText,
}

impl ViewMode {
    #[must_use]
    pub const fn label_de(self) -> &'static str {
        match self {
            Self::Wysiwyg => "Formatiert",
            Self::LivePreview => "Live-Vorschau",
            Self::Source => "Quelltext",
            Self::PlainText => "Reintext (TXT)",
        }
    }

    #[must_use]
    pub const fn label_en(self) -> &'static str {
        match self {
            Self::Wysiwyg => "Formatted",
            Self::LivePreview => "Live Preview",
            Self::Source => "Source Code",
            Self::PlainText => "Plain Text (TXT)",
        }
    }

    #[must_use]
    pub const fn label(self, german: bool) -> &'static str {
        if german {
            self.label_de()
        } else {
            self.label_en()
        }
    }
}

/// Visual indicator drawn to the left of formatted headings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HeadingMarkStyle {
    /// MDNote's signature slanted indicator bars (`1..=6` bars).
    #[default]
    SlantedBars,
    /// Muted `#` prefix label.
    HashSymbol,
    /// No left heading mark.
    None,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LayoutConfig {
    pub view_mode: ViewMode,
    pub heading_marks: HeadingMarkStyle,
    pub base_font_px: f32,
    pub line_spacing_scale: f32,
    pub word_wrap: bool,
    pub show_line_numbers: bool,
    pub focus_mode: bool,
    pub typewriter_mode: bool,
    pub max_prose_width_px: f32,
    pub ui_german: bool,
    pub palette: ThemePalette,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            view_mode: ViewMode::Wysiwyg,
            heading_marks: HeadingMarkStyle::SlantedBars,
            base_font_px: 16.0,
            line_spacing_scale: 1.42,
            word_wrap: true,
            show_line_numbers: false,
            focus_mode: false,
            typewriter_mode: false,
            max_prose_width_px: 880.0,
            ui_german: true,
            palette: ThemePalette::breeze_light(),
        }
    }
}

impl LayoutConfig {
    #[must_use]
    pub fn effective_line_numbers(&self) -> bool {
        self.show_line_numbers || matches!(self.view_mode, ViewMode::Source | ViewMode::PlainText)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct RectF {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl RectF {
    #[must_use]
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    #[must_use]
    pub fn contains(self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

/// Flat drawing command list rendered by Qt's `QPainter` in `EditorViewport::paintEvent`.
#[derive(Clone, Debug)]
pub enum DrawCmd {
    FillRect {
        rect: RectF,
        color: Rgba,
        radius: f32,
    },
    StrokeRect {
        rect: RectF,
        color: Rgba,
        stroke_width: f32,
        radius: f32,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: Rgba,
        width: f32,
    },
    HeadingBars {
        x: f32,
        top_y: f32,
        height: f32,
        level: u8,
        color: Rgba,
    },
    Checkbox {
        rect: RectF,
        checked: bool,
    },
    Text {
        x: f32,
        baseline_y: f32,
        text: String,
        role: FontRole,
        size_px: f32,
        bold: bool,
        italic: bool,
        color: Rgba,
    },
    Image {
        rect: RectF,
        src: String,
    },
}

#[derive(Clone, Debug)]
pub struct PositionedGlyphSpan {
    pub doc_range: Range<usize>,
    pub x: f32,
    pub width: f32,
    pub measured: MeasuredRun,
    pub is_hidden_marker: bool,
    pub link_url: Option<String>,
}

#[derive(Clone, Debug)]
pub struct VisualRow {
    pub y_doc: f32,
    pub height: f32,
    pub baseline_offset: f32,
    pub line_idx: usize,
    pub doc_range: Range<usize>,
    pub default_x: f32,
    pub content_width: f32,
    pub spans: Vec<PositionedGlyphSpan>,
}

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

/// Complete frame output produced for the visible viewport (`[scroll_y .. scroll_y + viewport_h]`).
#[derive(Clone, Debug)]
pub struct ViewportFrame {
    pub viewport_w: f32,
    pub viewport_h: f32,
    pub scroll_y: f32,
    pub total_height: f32,
    pub gutter_w: f32,
    pub content_x: f32,
    pub content_w: f32,
    /// Caret rectangle in **viewport** coordinates (ready for `QPainter` and `Qt::ImCursorRectangle`).
    pub caret_viewport_rect: RectF,
    /// Caret rectangle in **document** coordinates (used for auto-scroll-to-caret).
    pub caret_doc_rect: RectF,
    pub draw_cmds: Vec<DrawCmd>,
    pub visual_rows: Vec<VisualRow>,
    pub checkboxes: Vec<CheckboxHitBox>,
    pub code_lang_badges: Vec<CodeLangHitBox>,
    pub callout_badges: Vec<CalloutBadgeHitBox>,
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

        if self.visual_rows.is_empty() {
            return HitTarget::Text {
                byte_offset: 0,
                link_url: None,
            };
        }

        // Pick closest visual row containing `doc_y` (or nearest row above/below).
        let mut chosen = &self.visual_rows[0];
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

const HEADING_SCALES: [f32; 7] = [1.0, 1.90, 1.55, 1.30, 1.15, 1.05, 1.0];

/// Builds a viewport-virtualized [`ViewportFrame`] for `(rope, blocks, selection, search_matches)`.
#[allow(clippy::too_many_arguments)]
pub fn build_viewport_frame(
    rope: &Rope,
    blocks: &[Block],
    selection: Selection,
    ime_preedit: Option<&str>,
    search_matches: &[Range<usize>],
    active_match_idx: Option<usize>,
    config: &LayoutConfig,
    viewport_w: f32,
    viewport_h: f32,
    scroll_y: f32,
    measurer: &mut dyn TextMeasurer,
) -> ViewportFrame {
    let vw = viewport_w.max(240.0);
    let vh = viewport_h.max(120.0);
    let base_metrics = measurer.font_metrics(FontRole::SansBody, config.base_font_px, false, false);
    let default_line_h =
        (config.base_font_px * config.line_spacing_scale).max(base_metrics.line_height);

    let gutter_w = if config.effective_line_numbers() {
        let digits = rope.line_count().max(1).ilog10().saturating_add(1).max(3) as f32;
        (digits * config.base_font_px * 0.62 + 24.0).round()
    } else {
        0.0
    };

    let avail_w = (vw - gutter_w).max(160.0);
    let pad_x = 28.0f32;
    let prose_w = if matches!(config.view_mode, ViewMode::Wysiwyg | ViewMode::LivePreview)
        && config.max_prose_width_px > 200.0
    {
        (avail_w - pad_x * 2.0).clamp(140.0, config.max_prose_width_px)
    } else {
        (avail_w - pad_x * 2.0).max(140.0)
    };
    let content_x = if matches!(config.view_mode, ViewMode::Wysiwyg | ViewMode::LivePreview)
        && avail_w > prose_w + pad_x * 2.0
    {
        gutter_w + ((avail_w - prose_w) * 0.5).round()
    } else {
        gutter_w + pad_x
    };

    let mut draw_cmds = Vec::with_capacity(256);
    // Canvas background
    draw_cmds.push(DrawCmd::FillRect {
        rect: RectF::new(0.0, 0.0, vw, vh),
        color: config.palette.canvas_bg,
        radius: 0.0,
    });

    if matches!(config.view_mode, ViewMode::PlainText | ViewMode::Source) {
        return build_line_based_frame(
            rope,
            selection,
            ime_preedit,
            search_matches,
            active_match_idx,
            config,
            vw,
            vh,
            scroll_y,
            gutter_w,
            content_x,
            prose_w,
            default_line_h,
            draw_cmds,
            measurer,
        );
    }

    // Formatted (WYSIWYG / LivePreview) block-indexed virtualized layout!
    let caret_byte = selection.head;
    let caret_line = rope.byte_to_line_col(caret_byte).line;

    // Step 1: fast height estimates for all blocks; full layout for visible window + caret block.
    let overscan = vh * 0.75 + 240.0;
    let win_top = (scroll_y - overscan).max(0.0);
    let win_bot = scroll_y + vh + overscan;

    let mut y_cursor = 20.0f32;
    let mut visual_rows: Vec<VisualRow> = Vec::new();
    let mut checkboxes: Vec<CheckboxHitBox> = Vec::new();
    let mut code_lang_badges: Vec<CodeLangHitBox> = Vec::new();
    let mut callout_badges: Vec<CalloutBadgeHitBox> = Vec::new();
    let mut bg_cmds: Vec<DrawCmd> = Vec::new();
    let mut fg_cmds: Vec<DrawCmd> = Vec::new();

    for block in blocks {
        let est_h = estimate_block_height(block, default_line_h);
        let contains_caret = caret_byte >= block.byte_range.start
            && caret_byte <= block.byte_range.end
            || (caret_line >= block.line_range.start && caret_line < block.line_range.end);
        let near_viewport = y_cursor + est_h >= win_top && y_cursor <= win_bot;

        if !near_viewport && !contains_caret {
            y_cursor += est_h;
            continue;
        }

        let reveal_raw = config.view_mode == ViewMode::LivePreview && contains_caret;
        let block_start_y = y_cursor;

        y_cursor = layout_single_block(
            rope,
            block,
            y_cursor,
            content_x,
            prose_w,
            default_line_h,
            reveal_raw,
            caret_line,
            config,
            scroll_y,
            &mut bg_cmds,
            &mut fg_cmds,
            &mut visual_rows,
            &mut checkboxes,
            &mut code_lang_badges,
            &mut callout_badges,
            measurer,
        );

        // Optional Focus Mode: dim non-active blocks!
        if config.focus_mode && !contains_caret {
            let block_h = (y_cursor - block_start_y).max(0.0);
            if block_h > 0.0 {
                fg_cmds.push(DrawCmd::FillRect {
                    rect: RectF::new(gutter_w, block_start_y - scroll_y, vw - gutter_w, block_h),
                    color: config.palette.canvas_bg.with_alpha(150),
                    radius: 0.0,
                });
            }
        }
    }

    if blocks.is_empty() {
        visual_rows.push(VisualRow {
            y_doc: y_cursor,
            height: default_line_h,
            baseline_offset: base_metrics.ascent,
            line_idx: 0,
            doc_range: 0..0,
            default_x: content_x,
            content_width: 0.0,
            spans: Vec::new(),
        });
        y_cursor += default_line_h;
    }

    let total_height = (y_cursor + 64.0).max(vh);

    // Search highlights & selection highlights behind text
    emit_range_highlights(
        &visual_rows,
        search_matches,
        active_match_idx,
        selection,
        scroll_y,
        &config.palette,
        &mut bg_cmds,
    );

    // Caret calculation
    let caret_doc_rect =
        compute_caret_rect(&visual_rows, caret_byte, content_x, 20.0, default_line_h);
    let caret_viewport_rect = RectF::new(
        caret_doc_rect.x,
        caret_doc_rect.y - scroll_y,
        2.0,
        caret_doc_rect.h,
    );

    // Optional IME preedit underline overlay at caret
    if let Some(preedit) = ime_preedit.filter(|s| !s.is_empty()) {
        let m = measurer.measure_run(
            preedit,
            FontRole::SansBody,
            config.base_font_px,
            false,
            false,
        );
        let py = caret_viewport_rect.y;
        fg_cmds.push(DrawCmd::FillRect {
            rect: RectF::new(
                caret_viewport_rect.x,
                py,
                m.total_width + 4.0,
                caret_viewport_rect.h,
            ),
            color: config.palette.canvas_bg,
            radius: 2.0,
        });
        fg_cmds.push(DrawCmd::Text {
            x: caret_viewport_rect.x + 1.0,
            baseline_y: py + m.metrics.ascent,
            text: preedit.to_owned(),
            role: FontRole::SansBody,
            size_px: config.base_font_px,
            bold: false,
            italic: false,
            color: config.palette.text_fg,
        });
        fg_cmds.push(DrawCmd::Line {
            x1: caret_viewport_rect.x,
            y1: py + caret_viewport_rect.h - 1.0,
            x2: caret_viewport_rect.x + m.total_width + 2.0,
            y2: py + caret_viewport_rect.h - 1.0,
            color: config.palette.accent,
            width: 2.0,
        });
    } else if selection.is_empty() {
        fg_cmds.push(DrawCmd::FillRect {
            rect: caret_viewport_rect,
            color: config.palette.caret,
            radius: 0.0,
        });
    }

    draw_cmds.extend(bg_cmds);
    draw_cmds.extend(fg_cmds);

    if gutter_w > 0.0 {
        emit_gutter(
            &visual_rows,
            vw,
            vh,
            gutter_w,
            scroll_y,
            config,
            &mut draw_cmds,
            measurer,
        );
    }

    ViewportFrame {
        viewport_w: vw,
        viewport_h: vh,
        scroll_y,
        total_height,
        gutter_w,
        content_x,
        content_w: prose_w,
        caret_viewport_rect,
        caret_doc_rect,
        draw_cmds,
        visual_rows,
        checkboxes,
        code_lang_badges,
        callout_badges,
    }
}

fn estimate_block_height(block: &Block, line_h: f32) -> f32 {
    let lines = block.line_range.len().max(1) as f32;
    match block.kind {
        BlockKind::Heading { level, .. } => {
            let scale = HEADING_SCALES
                .get(usize::from(level))
                .copied()
                .unwrap_or(1.0);
            line_h * scale + 14.0
        }
        BlockKind::Table(ref t) => (1.0 + t.body_rows.len() as f32) * (line_h + 12.0) + 16.0,
        BlockKind::ThematicBreak => 24.0,
        _ => lines * line_h + 8.0,
    }
}

#[allow(clippy::too_many_arguments)]
fn layout_single_block(
    rope: &Rope,
    block: &Block,
    mut y: f32,
    content_x: f32,
    prose_w: f32,
    default_line_h: f32,
    reveal_raw: bool,
    caret_line: usize,
    config: &LayoutConfig,
    scroll_y: f32,
    bg_cmds: &mut Vec<DrawCmd>,
    fg_cmds: &mut Vec<DrawCmd>,
    visual_rows: &mut Vec<VisualRow>,
    checkboxes: &mut Vec<CheckboxHitBox>,
    code_lang_badges: &mut Vec<CodeLangHitBox>,
    callout_badges: &mut Vec<CalloutBadgeHitBox>,
    measurer: &mut dyn TextMeasurer,
) -> f32 {
    match block.kind {
        BlockKind::BlankLines => {
            for l_idx in block.line_range.clone() {
                let r = rope.line_byte_range(l_idx);
                visual_rows.push(VisualRow {
                    y_doc: y,
                    height: default_line_h * 0.75,
                    baseline_offset: default_line_h * 0.58,
                    line_idx: l_idx,
                    doc_range: r,
                    default_x: content_x,
                    content_width: 0.0,
                    spans: Vec::new(),
                });
                y += default_line_h * 0.75;
            }
            y
        }
        BlockKind::ThematicBreak => {
            let r = rope.line_byte_range(block.line_range.start);
            if reveal_raw {
                layout_inline_line(
                    rope,
                    block.line_range.start,
                    r,
                    y,
                    content_x,
                    prose_w,
                    config.base_font_px,
                    false,
                    true,
                    config.palette.muted_fg,
                    config,
                    scroll_y,
                    bg_cmds,
                    fg_cmds,
                    visual_rows,
                    measurer,
                )
            } else {
                let mid_y = y + 12.0;
                fg_cmds.push(DrawCmd::Line {
                    x1: content_x,
                    y1: mid_y - scroll_y,
                    x2: content_x + prose_w,
                    y2: mid_y - scroll_y,
                    color: config.palette.rule_line,
                    width: 1.5,
                });
                visual_rows.push(VisualRow {
                    y_doc: y,
                    height: 24.0,
                    baseline_offset: 16.0,
                    line_idx: block.line_range.start,
                    doc_range: r,
                    default_x: content_x,
                    content_width: prose_w,
                    spans: Vec::new(),
                });
                y + 24.0
            }
        }
        BlockKind::Heading {
            level,
            ref prefix_range,
            ref content_range,
            ref setext_underline,
        } => {
            y += 8.0;
            let scale = HEADING_SCALES
                .get(usize::from(level))
                .copied()
                .unwrap_or(1.0);
            let h_size = (config.base_font_px * scale).round();
            let full_line = prefix_range.start..content_range.end;
            let start_y = y;

            let text_x = match config.heading_marks {
                HeadingMarkStyle::SlantedBars if !reveal_raw => {
                    content_x + (f32::from(level) * 4.5 + 12.0)
                }
                HeadingMarkStyle::HashSymbol if !reveal_raw => {
                    let hashes = "#".repeat(usize::from(level));
                    let m = measurer.measure_run(
                        &hashes,
                        FontRole::SansBody,
                        h_size * 0.85,
                        true,
                        false,
                    );
                    fg_cmds.push(DrawCmd::Text {
                        x: content_x,
                        baseline_y: (y - scroll_y) + m.metrics.ascent + 2.0,
                        text: hashes,
                        role: FontRole::SansBody,
                        size_px: h_size * 0.85,
                        bold: true,
                        italic: false,
                        color: config.palette.muted_fg.with_alpha(150),
                    });
                    content_x + m.total_width + 8.0
                }
                _ => content_x,
            };

            let target_range = if reveal_raw {
                full_line.clone()
            } else {
                content_range.clone()
            };

            let first_row_idx = visual_rows.len();
            y = layout_inline_line(
                rope,
                block.line_range.start,
                target_range,
                y,
                text_x,
                (prose_w - (text_x - content_x)).max(80.0),
                h_size,
                true,
                reveal_raw,
                config.palette.text_fg,
                config,
                scroll_y,
                bg_cmds,
                fg_cmds,
                visual_rows,
                measurer,
            );

            // Prepend hidden prefix span onto first visual row so caret offsets inside `### ` resolve cleanly!
            if !reveal_raw
                && prefix_range.end > prefix_range.start
                && let Some(first_row) = visual_rows.get_mut(first_row_idx)
            {
                first_row.doc_range.start = prefix_range.start;
                first_row.spans.insert(
                    0,
                    PositionedGlyphSpan {
                        doc_range: prefix_range.clone(),
                        x: text_x,
                        width: 0.0,
                        measured: MeasuredRun {
                            total_width: 0.0,
                            metrics: measurer.font_metrics(FontRole::SansBody, h_size, true, false),
                            grapheme_ends: Vec::new(),
                        },
                        is_hidden_marker: true,
                        link_url: None,
                    },
                );
            }

            if config.heading_marks == HeadingMarkStyle::SlantedBars
                && !reveal_raw
                && let Some(first_row) = visual_rows.get(first_row_idx)
            {
                let bar_h = (h_size * 0.82).max(12.0);
                let top_y = first_row.y_doc + first_row.baseline_offset - bar_h * 0.88;
                fg_cmds.push(DrawCmd::HeadingBars {
                    x: content_x,
                    top_y: top_y - scroll_y,
                    height: bar_h,
                    level,
                    color: config.palette.accent.with_alpha(190),
                });
            }

            if reveal_raw && let Some(u_range) = setext_underline.clone() {
                y = layout_inline_line(
                    rope,
                    block.line_range.start.saturating_add(1),
                    u_range,
                    y,
                    content_x,
                    prose_w,
                    config.base_font_px,
                    false,
                    true,
                    config.palette.muted_fg,
                    config,
                    scroll_y,
                    bg_cmds,
                    fg_cmds,
                    visual_rows,
                    measurer,
                );
            }

            if level <= 2 {
                fg_cmds.push(DrawCmd::Line {
                    x1: content_x,
                    y1: (y + 3.0) - scroll_y,
                    x2: content_x + prose_w,
                    y2: (y + 3.0) - scroll_y,
                    color: config.palette.rule_line,
                    width: 1.0,
                });
                y += 8.0;
            } else {
                let _ = start_y;
                y += 4.0;
            }
            y
        }
        BlockKind::Paragraph { ref lines } => {
            for &(l_idx, ref l_range) in lines {
                let raw_here = config.view_mode == ViewMode::LivePreview && l_idx == caret_line;
                y = layout_inline_line(
                    rope,
                    l_idx,
                    l_range.clone(),
                    y,
                    content_x,
                    prose_w,
                    config.base_font_px,
                    false,
                    raw_here,
                    config.palette.text_fg,
                    config,
                    scroll_y,
                    bg_cmds,
                    fg_cmds,
                    visual_rows,
                    measurer,
                );
            }
            y + 4.0
        }
        BlockKind::ListItem {
            indent_spaces,
            ordered,
            ref marker_range,
            ref task,
            ref content_range,
            line_idx,
        } => {
            let raw_here = config.view_mode == ViewMode::LivePreview && line_idx == caret_line;
            let indent_px = (indent_spaces as f32) * (config.base_font_px * 0.55);
            let marker_slot = 24.0f32;
            let task_slot = if task.is_some() { 22.0f32 } else { 0.0f32 };

            if raw_here {
                let full_range = marker_range.start..content_range.end;
                return layout_inline_line(
                    rope,
                    line_idx,
                    full_range,
                    y,
                    content_x + indent_px,
                    (prose_w - indent_px).max(80.0),
                    config.base_font_px,
                    false,
                    true,
                    config.palette.text_fg,
                    config,
                    scroll_y,
                    bg_cmds,
                    fg_cmds,
                    visual_rows,
                    measurer,
                ) + 2.0;
            }

            let body_x = content_x + indent_px + marker_slot + task_slot;
            let first_idx = visual_rows.len();
            let text_color = if task.as_ref().is_some_and(|t| t.checked) {
                config.palette.muted_fg
            } else {
                config.palette.text_fg
            };

            let next_y = layout_inline_line(
                rope,
                line_idx,
                content_range.clone(),
                y,
                body_x,
                (prose_w - (body_x - content_x)).max(80.0),
                config.base_font_px,
                false,
                false,
                text_color,
                config,
                scroll_y,
                bg_cmds,
                fg_cmds,
                visual_rows,
                measurer,
            );

            if let Some(first_row) = visual_rows.get_mut(first_idx) {
                first_row.doc_range.start = marker_range.start;
                let baseline_v = (first_row.y_doc - scroll_y) + first_row.baseline_offset;
                let label = match ordered {
                    Some(n) => format!("{n}."),
                    None => "•".to_owned(),
                };
                fg_cmds.push(DrawCmd::Text {
                    x: content_x + indent_px + 4.0,
                    baseline_y: baseline_v,
                    text: label,
                    role: FontRole::SansBody,
                    size_px: config.base_font_px,
                    bold: ordered.is_none(),
                    italic: false,
                    color: config.palette.accent,
                });

                if let Some(ts) = task {
                    let box_sz = (config.base_font_px * 0.95).clamp(13.0, 20.0);
                    let cb_y_doc = first_row.y_doc + (first_row.height - box_sz) * 0.5;
                    let cb_x = content_x + indent_px + marker_slot;
                    checkboxes.push(CheckboxHitBox {
                        rect_doc: RectF::new(cb_x, cb_y_doc, box_sz, box_sz),
                        bracket_range: ts.bracket_range.clone(),
                        checked: ts.checked,
                    });
                    fg_cmds.push(DrawCmd::Checkbox {
                        rect: RectF::new(cb_x, cb_y_doc - scroll_y, box_sz, box_sz),
                        checked: ts.checked,
                    });
                }
            }
            next_y + 2.0
        }
        BlockKind::BlockQuote {
            alert,
            ref custom_title,
            ref lines,
        } => {
            y += 4.0;
            let box_top = y;
            let bar_color =
                alert.map_or(config.palette.quote_bar, |a| config.palette.alert_color(a));
            let inner_x = content_x + 16.0;
            let inner_w = (prose_w - 24.0).max(80.0);

            let mut badge_reserve_w = 0.0f32;
            let mut min_alert_h = default_line_h;
            if let Some(kind) = alert {
                let badge_px = (config.base_font_px * 0.80).max(10.5);
                let badge_label = format!("{} ▾", kind.title(config.ui_german));
                let bm =
                    measurer.measure_run(&badge_label, FontRole::SansBody, badge_px, true, false);
                let bw = bm.total_width + 14.0;
                let bh = (bm.metrics.line_height + 4.0).max(20.0);
                let bx = (content_x + prose_w - bw - 8.0).max(content_x + 4.0);
                let by_doc = box_top + 2.0;
                badge_reserve_w = bw + 12.0;
                min_alert_h = (bh + 8.0).max(default_line_h);

                let badge_rect_v = RectF::new(bx, by_doc - scroll_y, bw, bh);
                fg_cmds.push(DrawCmd::FillRect {
                    rect: badge_rect_v,
                    color: config.palette.canvas_bg.with_alpha(225),
                    radius: 5.0,
                });
                fg_cmds.push(DrawCmd::StrokeRect {
                    rect: badge_rect_v,
                    color: bar_color.with_alpha(150),
                    stroke_width: 1.0,
                    radius: 5.0,
                });
                let badge_base = (by_doc - scroll_y)
                    + (bh - (bm.metrics.ascent + bm.metrics.descent)) * 0.5
                    + bm.metrics.ascent;
                fg_cmds.push(DrawCmd::Text {
                    x: bx + 7.0,
                    baseline_y: badge_base,
                    text: badge_label,
                    role: FontRole::SansBody,
                    size_px: badge_px,
                    bold: true,
                    italic: false,
                    color: bar_color,
                });
                let open_line_start = lines
                    .first()
                    .map_or(block.byte_range.start, |ql| ql.marker_range.start);
                callout_badges.push(CalloutBadgeHitBox {
                    rect_doc: RectF::new(bx, by_doc, bw, bh),
                    open_line_start,
                    kind,
                });
            }

            if let Some(kind) = alert
                && !reveal_raw
            {
                let title = custom_title
                    .as_deref()
                    .unwrap_or_else(|| kind.title(config.ui_german));
                let badge = format!("{title}  ");
                let m = measurer.measure_run(
                    &badge,
                    FontRole::SansBody,
                    config.base_font_px,
                    true,
                    false,
                );
                let h = default_line_h;
                fg_cmds.push(DrawCmd::Text {
                    x: inner_x,
                    baseline_y: (y - scroll_y) + m.metrics.ascent + 2.0,
                    text: badge,
                    role: FontRole::SansBody,
                    size_px: config.base_font_px,
                    bold: true,
                    italic: false,
                    color: bar_color,
                });
                if let Some(first_ql) = lines.first() {
                    visual_rows.push(VisualRow {
                        y_doc: y,
                        height: h,
                        baseline_offset: m.metrics.ascent + 2.0,
                        line_idx: first_ql.line_idx,
                        doc_range: first_ql.marker_range.start..first_ql.content_range.end,
                        default_x: inner_x,
                        content_width: m.total_width,
                        spans: Vec::new(),
                    });
                }
                y += h;
            }

            let skip = usize::from(alert.is_some() && !reveal_raw);
            for (line_pos, ql) in lines.iter().enumerate().skip(skip) {
                let raw_line = reveal_raw
                    || (config.view_mode == ViewMode::LivePreview && ql.line_idx == caret_line);
                let indent = f32::from(ql.depth.saturating_sub(1)) * 14.0;
                let range = if raw_line {
                    ql.marker_range.start..ql.content_range.end
                } else {
                    ql.content_range.clone()
                };
                let right_pad = if line_pos == 0 { badge_reserve_w } else { 0.0 };
                y = layout_inline_line(
                    rope,
                    ql.line_idx,
                    range,
                    y,
                    inner_x + indent,
                    (inner_w - indent - right_pad).max(60.0),
                    config.base_font_px,
                    false,
                    raw_line,
                    if alert.is_some() {
                        config.palette.text_fg
                    } else {
                        config.palette.quote_fg
                    },
                    config,
                    scroll_y,
                    bg_cmds,
                    fg_cmds,
                    visual_rows,
                    measurer,
                );
            }

            let box_h = (y - box_top + 4.0).max(min_alert_h);
            if let Some(kind) = alert {
                bg_cmds.push(DrawCmd::FillRect {
                    rect: RectF::new(content_x, box_top - 2.0 - scroll_y, prose_w, box_h),
                    color: config.palette.alert_color(kind).with_alpha(20),
                    radius: 4.0,
                });
            }
            fg_cmds.push(DrawCmd::FillRect {
                rect: RectF::new(
                    content_x,
                    box_top - 2.0 - scroll_y,
                    if alert.is_some() { 4.0 } else { 3.0 },
                    box_h,
                ),
                color: bar_color,
                radius: 1.5,
            });
            y + 8.0
        }
        BlockKind::CodeFence {
            ref language,
            ref open_line_range,
            ref body_lines,
            ref close_line_range,
            ..
        } => {
            y += 6.0;
            let box_top = y;
            let code_px = (config.base_font_px * 0.90).max(10.0);
            let pad = 10.0f32;

            // Interactive top-right language picker badge (`rust ▾` / `Code ▾`)
            let badge_px = (code_px * 0.84).max(10.5);
            let badge_label = if language.is_empty() {
                "Code ▾".to_owned()
            } else {
                format!("{language} ▾")
            };
            let bm = measurer.measure_run(&badge_label, FontRole::SansBody, badge_px, true, false);
            let bw = bm.total_width + 14.0;
            let bh = (bm.metrics.line_height + 4.0).max(20.0);
            let bx = (content_x + prose_w - bw - 8.0).max(content_x + 4.0);
            let by_doc = box_top + 6.0;

            if reveal_raw {
                y += pad * 0.6;
                y = layout_plain_code_line(
                    rope,
                    block.line_range.start,
                    open_line_range.clone(),
                    y,
                    content_x + pad,
                    code_px,
                    config.palette.muted_fg,
                    scroll_y,
                    fg_cmds,
                    visual_rows,
                    measurer,
                );
            } else {
                // Leave clean header clearance so even long first lines never collide with the badge.
                y += bh + 8.0;
            }

            let badge_rect_v = RectF::new(bx, by_doc - scroll_y, bw, bh);
            fg_cmds.push(DrawCmd::FillRect {
                rect: badge_rect_v,
                color: config.palette.canvas_bg.with_alpha(220),
                radius: 5.0,
            });
            fg_cmds.push(DrawCmd::StrokeRect {
                rect: badge_rect_v,
                color: config.palette.gutter_border,
                stroke_width: 1.0,
                radius: 5.0,
            });
            let badge_base = (by_doc - scroll_y)
                + (bh - (bm.metrics.ascent + bm.metrics.descent)) * 0.5
                + bm.metrics.ascent;
            fg_cmds.push(DrawCmd::Text {
                x: bx + 7.0,
                baseline_y: badge_base,
                text: badge_label,
                role: FontRole::SansBody,
                size_px: badge_px,
                bold: true,
                italic: false,
                color: if language.is_empty() {
                    config.palette.muted_fg
                } else {
                    config.palette.accent
                },
            });
            code_lang_badges.push(CodeLangHitBox {
                rect_doc: RectF::new(bx, by_doc, bw, bh),
                open_line_start: open_line_range.start,
                language: language.clone(),
            });

            for &(l_idx, ref r) in body_lines {
                y = layout_plain_code_line(
                    rope,
                    l_idx,
                    r.clone(),
                    y,
                    content_x + pad,
                    code_px,
                    config.palette.text_fg,
                    scroll_y,
                    fg_cmds,
                    visual_rows,
                    measurer,
                );
            }

            if reveal_raw && let Some(close_r) = close_line_range.clone() {
                let close_idx = block.line_range.end.saturating_sub(1);
                y = layout_plain_code_line(
                    rope,
                    close_idx,
                    close_r,
                    y,
                    content_x + pad,
                    code_px,
                    config.palette.muted_fg,
                    scroll_y,
                    fg_cmds,
                    visual_rows,
                    measurer,
                );
            }

            y += pad * 0.7;
            bg_cmds.push(DrawCmd::FillRect {
                rect: RectF::new(content_x, box_top - scroll_y, prose_w, y - box_top),
                color: config.palette.code_bg,
                radius: 6.0,
            });
            bg_cmds.push(DrawCmd::StrokeRect {
                rect: RectF::new(content_x, box_top - scroll_y, prose_w, y - box_top),
                color: config.palette.gutter_border,
                stroke_width: 1.0,
                radius: 6.0,
            });
            y + 8.0
        }
        BlockKind::Table(ref tbl) => layout_table_2d(
            rope,
            tbl,
            y,
            content_x,
            prose_w,
            reveal_raw,
            caret_line,
            config,
            scroll_y,
            bg_cmds,
            fg_cmds,
            visual_rows,
            measurer,
        ),
    }
}

/// Computes both `(min_word_width, natural_width)` of a table cell using the exact inline token
/// fonts, sizes, and `<kbd>` / `` `code` `` pill paddings as [`layout_inline_line`].
///
/// - `min_word_width` (`min-content`): width of the widest unbreakable inline run (single word,
///   contiguous formatted word across markers, or atomic `<kbd>` / `` `code` `` pill).
/// - `natural_width` (`max-content`): single-line width without wrapping.
fn measure_inline_cell_widths(
    txt: &str,
    font_px: f32,
    base_bold: bool,
    reveal_raw: bool,
    measurer: &mut dyn TextMeasurer,
) -> (f32, f32) {
    if txt.is_empty() {
        return (0.0, 0.0);
    }
    let tokens = parse_inline_tokens(0, txt);
    let mut total = 0.0f32;
    let mut max_run = 0.0f32;
    let mut cur_run = 0.0f32;
    for tok in tokens {
        let slice = txt.get(tok.range.clone()).unwrap_or("");
        match tok.kind {
            InlineTokenKind::SyntaxMarker if !reveal_raw => {}
            InlineTokenKind::Image {
                ref alt,
                ref src,
                ref caption,
                size,
            } if !reveal_raw => {
                let img_w = if let Some((iw, ih)) = measurer.image_dimensions(src)
                    && iw > 0.0
                    && ih > 0.0
                {
                    let base_w = match size {
                        Some(ImageSize::Percent(pct)) => {
                            (220.0 * (f32::from(pct) / 100.0)).max(24.0)
                        }
                        Some(ImageSize::Pixels(px)) => f32::from(px).clamp(24.0, 360.0),
                        None => {
                            let scale = (220.0 / iw).min(140.0 / ih).min(1.0);
                            (iw * scale).max(24.0)
                        }
                    };
                    let cap_w = caption
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map_or(0.0, |cap| {
                            measurer
                                .measure_run(
                                    cap,
                                    FontRole::SansBody,
                                    (font_px * 0.88).max(11.0),
                                    false,
                                    true,
                                )
                                .total_width
                                .min(260.0)
                        });
                    base_w.max(cap_w) + 4.0
                } else {
                    let badge = format_image_badge(alt, src, caption.as_deref(), size);
                    let m = measurer.measure_run(
                        &badge,
                        FontRole::SansBody,
                        font_px * 0.92,
                        false,
                        false,
                    );
                    m.total_width + 16.0
                };
                total += img_w;
                cur_run += img_w;
                max_run = max_run.max(cur_run);
            }
            _ => {
                let is_marker = matches!(tok.kind, InlineTokenKind::SyntaxMarker);
                let is_code = tok.style.contains(StyleFlags::CODE);
                let is_kbd = tok.style.contains(StyleFlags::KBD);
                let role = if is_code || is_kbd {
                    FontRole::Monospace
                } else {
                    FontRole::SansBody
                };
                let run_px = if is_kbd && !is_marker {
                    font_px * 0.86
                } else if is_code {
                    font_px * 0.90
                } else {
                    font_px
                };
                let bold = base_bold || tok.style.contains(StyleFlags::BOLD);
                let italic = tok.style.contains(StyleFlags::ITALIC);
                let m = measurer.measure_run(slice, role, run_px, bold, italic);
                if (is_kbd || is_code) && !is_marker {
                    let pill_extra = if is_kbd { 15.0 } else { 11.0 };
                    let pill_w = m.total_width + pill_extra;
                    total += pill_w;
                    cur_run += pill_w;
                    max_run = max_run.max(cur_run);
                } else {
                    total += m.total_width;
                    if !slice.chars().any(char::is_whitespace) {
                        cur_run += m.total_width;
                        max_run = max_run.max(cur_run);
                    } else {
                        let mut seg_start: Option<usize> = None;
                        for (idx, ch) in slice.char_indices() {
                            if ch.is_whitespace() {
                                if let Some(s) = seg_start.take()
                                    && let Some(part) = slice.get(s..idx)
                                {
                                    cur_run += measurer
                                        .measure_run(part, role, run_px, bold, italic)
                                        .total_width;
                                }
                                max_run = max_run.max(cur_run);
                                cur_run = 0.0;
                            } else if seg_start.is_none() {
                                seg_start = Some(idx);
                            }
                        }
                        if let Some(s) = seg_start
                            && let Some(part) = slice.get(s..)
                        {
                            cur_run += measurer
                                .measure_run(part, role, run_px, bold, italic)
                                .total_width;
                            max_run = max_run.max(cur_run);
                        }
                    }
                }
            }
        }
    }
    max_run = max_run.max(cur_run);
    (max_run, total)
}

fn shift_draw_cmds_in_bands(cmds: &mut [DrawCmd], bands: &[(f32, f32, f32)]) {
    for cmd in cmds {
        let cmd_vy = match cmd {
            DrawCmd::FillRect { rect, .. }
            | DrawCmd::StrokeRect { rect, .. }
            | DrawCmd::Checkbox { rect, .. }
            | DrawCmd::Image { rect, .. } => rect.y + rect.h * 0.5,
            DrawCmd::Line { y1, y2, .. } => (*y1 + *y2) * 0.5,
            DrawCmd::HeadingBars { top_y, height, .. } => *top_y + *height * 0.5,
            DrawCmd::Text { baseline_y, .. } => *baseline_y - 2.0,
        };
        let mut applied = 0.0f32;
        for &(vy_top, vy_bot, shift) in bands {
            if cmd_vy >= vy_top - 1.5 && cmd_vy <= vy_bot + 1.5 {
                applied = shift;
                break;
            }
        }
        if applied.abs() > f32::EPSILON {
            match cmd {
                DrawCmd::FillRect { rect, .. }
                | DrawCmd::StrokeRect { rect, .. }
                | DrawCmd::Checkbox { rect, .. }
                | DrawCmd::Image { rect, .. } => {
                    rect.x += applied;
                }
                DrawCmd::Line { x1, x2, .. } => {
                    *x1 += applied;
                    *x2 += applied;
                }
                DrawCmd::HeadingBars { x, .. } | DrawCmd::Text { x, .. } => {
                    *x += applied;
                }
            }
        }
    }
}

/// First-class 2D Table Grid Layout directly over `Rope` cell byte slices!
#[allow(clippy::too_many_arguments)]
fn layout_table_2d(
    rope: &Rope,
    tbl: &TableBlock,
    mut y: f32,
    content_x: f32,
    prose_w: f32,
    reveal_raw: bool,
    caret_line: usize,
    config: &LayoutConfig,
    scroll_y: f32,
    bg_cmds: &mut Vec<DrawCmd>,
    fg_cmds: &mut Vec<DrawCmd>,
    visual_rows: &mut Vec<VisualRow>,
    measurer: &mut dyn TextMeasurer,
) -> f32 {
    if reveal_raw && caret_line == tbl.delimiter_line_idx {
        // Only if the user specifically placed the caret on the `|---|---|` delimiter row in Live mode,
        // show raw table lines so the delimiter itself can be edited directly.
        let mut cur_y = y + 4.0;
        for (_, row) in tbl.all_data_rows() {
            if row.line_idx == tbl.header_row.line_idx {
                cur_y = layout_plain_code_line(
                    rope,
                    row.line_idx,
                    row.byte_range.clone(),
                    cur_y,
                    content_x,
                    config.base_font_px * 0.92,
                    config.palette.text_fg,
                    scroll_y,
                    fg_cmds,
                    visual_rows,
                    measurer,
                );
                cur_y = layout_plain_code_line(
                    rope,
                    tbl.delimiter_line_idx,
                    tbl.delimiter_range.clone(),
                    cur_y,
                    content_x,
                    config.base_font_px * 0.92,
                    config.palette.muted_fg,
                    scroll_y,
                    fg_cmds,
                    visual_rows,
                    measurer,
                );
            } else {
                cur_y = layout_plain_code_line(
                    rope,
                    row.line_idx,
                    row.byte_range.clone(),
                    cur_y,
                    content_x,
                    config.base_font_px * 0.92,
                    config.palette.text_fg,
                    scroll_y,
                    fg_cmds,
                    visual_rows,
                    measurer,
                );
            }
        }
        return cur_y + 6.0;
    }

    let cols = tbl.col_count.max(1);
    let cell_pad_x = 12.0f32;
    let cell_pad_y = 6.0f32;
    let min_col_w = 64.0f32;
    // Cap per-column `min-content` so one giant unbreakable URL cannot starve sibling columns.
    let max_min_col_w = (prose_w * 0.42).max(min_col_w);

    let mut min_w = vec![min_col_w; cols];
    let mut natural_w = vec![min_col_w; cols];
    for (row_idx, row) in tbl.all_data_rows() {
        let is_header = row_idx == 0;
        let raw_cell = config.view_mode == ViewMode::LivePreview && row.line_idx == caret_line;
        for (c, cell) in row.cells.iter().take(cols).enumerate() {
            let txt = rope.slice_to_string(cell.trimmed_range.clone());
            let (cell_min_w, cell_nat_w) = measure_inline_cell_widths(
                &txt,
                config.base_font_px,
                is_header,
                raw_cell,
                measurer,
            );
            let padded_min = (cell_min_w.ceil() + cell_pad_x * 2.0 + 8.0).min(max_min_col_w);
            let padded_nat = cell_nat_w.ceil() + cell_pad_x * 2.0 + 8.0;
            min_w[c] = min_w[c].max(padded_min);
            natural_w[c] = natural_w[c].max(padded_nat);
        }
    }
    for c in 0..cols {
        natural_w[c] = natural_w[c].max(min_w[c]);
    }

    let sum_nat: f32 = natural_w.iter().sum();
    let sum_min: f32 = min_w.iter().sum();
    let mut col_widths: Vec<f32> = if sum_nat <= prose_w {
        natural_w
    } else if sum_min >= prose_w {
        let scale = prose_w / sum_min.max(1.0);
        min_w.into_iter().map(|w| w * scale).collect()
    } else {
        // CSS-style `min-content` + damped flex distribution:
        // Guarantee each column at least its unbreakable word width (`min_w`) so compact
        // single-word columns (e.g. "Bereich") never get squished mid-word, then share remaining
        // width across multi-word columns using damped demand (`need^0.75`).
        let mut widths = min_w;
        let mut remaining = prose_w - sum_min;
        for _ in 0..3 {
            if remaining <= 0.5 {
                break;
            }
            let weights: Vec<f32> = widths
                .iter()
                .zip(&natural_w)
                .map(|(&cur, &nat)| {
                    let need = (nat - cur).max(0.0);
                    if need > 0.5 { need.powf(0.75) } else { 0.0 }
                })
                .collect();
            let weight_sum: f32 = weights.iter().sum();
            if weight_sum <= f32::EPSILON {
                break;
            }
            let mut granted_total = 0.0f32;
            for c in 0..cols {
                if weights[c] > 0.0 {
                    let share = remaining * (weights[c] / weight_sum);
                    let add = share.min(natural_w[c] - widths[c]);
                    widths[c] += add;
                    granted_total += add;
                }
            }
            if granted_total <= 0.1 {
                break;
            }
            remaining = (remaining - granted_total).max(0.0);
        }
        widths
    };
    let raw_table_w: f32 = col_widths.iter().sum();
    if raw_table_w > prose_w && raw_table_w > 1.0 {
        let norm = prose_w / raw_table_w;
        for cw in &mut col_widths {
            *cw *= norm;
        }
    }
    let table_w: f32 = col_widths.iter().sum();

    y += 6.0;
    let table_top = y;

    let render_row = |row: &TableRow,
                      is_header: bool,
                      row_top: f32,
                      bg_cmds: &mut Vec<DrawCmd>,
                      fg_cmds: &mut Vec<DrawCmd>,
                      visual_rows: &mut Vec<VisualRow>,
                      measurer: &mut dyn TextMeasurer|
     -> f32 {
        let raw_cell = config.view_mode == ViewMode::LivePreview && row.line_idx == caret_line;
        let mut col_x = content_x;
        let mut max_cell_bottom =
            row_top + config.base_font_px * config.line_spacing_scale + cell_pad_y * 2.0;

        for (c, &cw) in col_widths.iter().enumerate() {
            let align = tbl.alignments.get(c).copied().unwrap_or_default();
            let inner_w = (cw - cell_pad_x * 2.0).max(24.0);
            if let Some(cell) = row.cells.get(c) {
                let first_sub_idx = visual_rows.len();
                let bg_start = bg_cmds.len();
                let fg_start = fg_cmds.len();
                let cell_end_y = layout_inline_line(
                    rope,
                    row.line_idx,
                    cell.trimmed_range.clone(),
                    row_top + cell_pad_y,
                    col_x + cell_pad_x,
                    inner_w,
                    config.base_font_px,
                    is_header,
                    raw_cell,
                    config.palette.text_fg,
                    config,
                    scroll_y,
                    bg_cmds,
                    fg_cmds,
                    visual_rows,
                    measurer,
                );
                // Apply column alignment (`Center` / `Right`) synchronously to spans, bg pills (<kbd>/code), and fg runs!
                if align != ColumnAlign::Left {
                    let mut bands: Vec<(f32, f32, f32)> = Vec::new();
                    for vrow in visual_rows.iter_mut().skip(first_sub_idx) {
                        let extra = (inner_w - vrow.content_width).max(0.0);
                        let shift = match align {
                            ColumnAlign::Center => extra * 0.5,
                            ColumnAlign::Right => extra,
                            ColumnAlign::Left => 0.0,
                        };
                        vrow.default_x += shift;
                        for s in &mut vrow.spans {
                            s.x += shift;
                        }
                        let vy_top = vrow.y_doc - scroll_y;
                        bands.push((vy_top, vy_top + vrow.height, shift));
                    }
                    if let Some(slice) = bg_cmds.get_mut(bg_start..) {
                        shift_draw_cmds_in_bands(slice, &bands);
                    }
                    if let Some(slice) = fg_cmds.get_mut(fg_start..) {
                        shift_draw_cmds_in_bands(slice, &bands);
                    }
                }
                max_cell_bottom = max_cell_bottom.max(cell_end_y + cell_pad_y);
            }
            col_x += cw;
        }

        let row_h = max_cell_bottom - row_top;
        if is_header {
            bg_cmds.insert(
                0,
                DrawCmd::FillRect {
                    rect: RectF::new(content_x, row_top - scroll_y, table_w, row_h),
                    color: config.palette.table_header_bg,
                    radius: 0.0,
                },
            );
        }
        // Horizontal row divider
        fg_cmds.push(DrawCmd::Line {
            x1: content_x,
            y1: max_cell_bottom - scroll_y,
            x2: content_x + table_w,
            y2: max_cell_bottom - scroll_y,
            color: config.palette.table_border,
            width: if is_header { 1.5 } else { 1.0 },
        });
        max_cell_bottom
    };

    y = render_row(
        &tbl.header_row,
        true,
        y,
        bg_cmds,
        fg_cmds,
        visual_rows,
        measurer,
    );
    for row in &tbl.body_rows {
        y = render_row(row, false, y, bg_cmds, fg_cmds, visual_rows, measurer);
    }

    // Vertical column dividers and outer border
    let mut vx = content_x;
    for &cw in col_widths.iter().take(cols.saturating_sub(1)) {
        vx += cw;
        fg_cmds.push(DrawCmd::Line {
            x1: vx,
            y1: table_top - scroll_y,
            x2: vx,
            y2: y - scroll_y,
            color: config.palette.table_border,
            width: 1.0,
        });
    }
    fg_cmds.push(DrawCmd::StrokeRect {
        rect: RectF::new(content_x, table_top - scroll_y, table_w, y - table_top),
        color: config.palette.table_border,
        stroke_width: 1.0,
        radius: 3.0,
    });

    y + 8.0
}

#[allow(clippy::too_many_arguments)]
fn layout_inline_line(
    rope: &Rope,
    line_idx: usize,
    doc_range: Range<usize>,
    mut y: f32,
    start_x: f32,
    max_w: f32,
    font_px: f32,
    base_bold: bool,
    reveal_raw: bool,
    base_color: Rgba,
    config: &LayoutConfig,
    scroll_y: f32,
    bg_cmds: &mut Vec<DrawCmd>,
    fg_cmds: &mut Vec<DrawCmd>,
    visual_rows: &mut Vec<VisualRow>,
    measurer: &mut dyn TextMeasurer,
) -> f32 {
    let raw_text = rope.slice_to_string(doc_range.clone());
    let base_m = measurer.font_metrics(FontRole::SansBody, font_px, base_bold, false);
    let row_h = (font_px * config.line_spacing_scale).max(base_m.line_height);
    let baseline_off = (row_h - (base_m.ascent + base_m.descent)) * 0.5 + base_m.ascent;

    if raw_text.is_empty() {
        visual_rows.push(VisualRow {
            y_doc: y,
            height: row_h,
            baseline_offset: baseline_off,
            line_idx,
            doc_range,
            default_x: start_x,
            content_width: 0.0,
            spans: Vec::new(),
        });
        return y + row_h;
    }

    let tokens = parse_inline_tokens(doc_range.start, &raw_text);

    // Split tokens at whitespace boundaries when word_wrap is enabled so long lines wrap cleanly!
    let mut cur_x = start_x;
    let mut cur_spans: Vec<PositionedGlyphSpan> = Vec::new();
    let mut sub_start = doc_range.start;
    let mut emitted_any_row = false;

    let flush_row = |y_pos: f32,
                     end_x: f32,
                     s_byte: usize,
                     e_byte: usize,
                     spans: Vec<PositionedGlyphSpan>,
                     visual_rows: &mut Vec<VisualRow>| {
        visual_rows.push(VisualRow {
            y_doc: y_pos,
            height: row_h,
            baseline_offset: baseline_off,
            line_idx,
            doc_range: s_byte..e_byte.max(s_byte),
            default_x: start_x,
            content_width: (end_x - start_x).max(0.0),
            spans,
        });
    };

    for tok in tokens {
        let rel_s = tok.range.start.saturating_sub(doc_range.start);
        let rel_e = tok.range.end.saturating_sub(doc_range.start);
        let slice = raw_text.get(rel_s..rel_e).unwrap_or("");

        match tok.kind {
            InlineTokenKind::SyntaxMarker if !reveal_raw => {
                cur_spans.push(PositionedGlyphSpan {
                    doc_range: tok.range,
                    x: cur_x,
                    width: 0.0,
                    measured: MeasuredRun {
                        total_width: 0.0,
                        metrics: base_m,
                        grapheme_ends: Vec::new(),
                    },
                    is_hidden_marker: true,
                    link_url: None,
                });
            }
            InlineTokenKind::Image {
                ref alt,
                ref src,
                ref caption,
                size,
            } if !reveal_raw => {
                if let Some((nat_w, nat_h)) = measurer.image_dimensions(src)
                    && nat_w > 0.0
                    && nat_h > 0.0
                {
                    if !cur_spans.is_empty() {
                        let prev_end = tok.range.start;
                        let taken = std::mem::take(&mut cur_spans);
                        flush_row(y, cur_x, sub_start, prev_end, taken, visual_rows);
                        y += row_h;
                    }
                    let fit_w = max_w.max(48.0);
                    let (draw_w, draw_h) = match size {
                        Some(ImageSize::Percent(pct)) => {
                            let frac = (f32::from(pct) / 100.0).clamp(0.05, 1.0);
                            let w = (fit_w * frac).max(24.0);
                            let h = (w * (nat_h / nat_w)).max(24.0);
                            (w, h)
                        }
                        Some(ImageSize::Pixels(px)) => {
                            let w = f32::from(px).min(fit_w).max(24.0);
                            let h = (w * (nat_h / nat_w)).max(24.0);
                            (w, h)
                        }
                        None => {
                            let scale = (fit_w / nat_w).min(380.0 / nat_h).min(1.0);
                            ((nat_w * scale).max(24.0), (nat_h * scale).max(24.0))
                        }
                    };

                    let rect_v = RectF::new(start_x, (y - scroll_y) + 5.0, draw_w, draw_h);
                    fg_cmds.push(DrawCmd::Image {
                        rect: rect_v,
                        src: src.clone(),
                    });
                    fg_cmds.push(DrawCmd::StrokeRect {
                        rect: rect_v,
                        color: config.palette.table_border,
                        stroke_width: 1.0,
                        radius: 4.0,
                    });

                    let mut cap_extra_h = 0.0f32;
                    let mut effective_w = draw_w;
                    if let Some(cap) = caption.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                        let cap_px = (font_px * 0.88).max(11.0);
                        let cap_wrap_w = draw_w.max((fit_w * 0.55).min(fit_w));
                        let (cap_lines, cap_m) =
                            wrap_caption_lines(cap, cap_wrap_w, cap_px, measurer);
                        let cap_line_h = (cap_px * 1.34).max(cap_m.line_height);
                        let cap_base_off =
                            (cap_line_h - (cap_m.ascent + cap_m.descent)) * 0.5 + cap_m.ascent;
                        let cap_top_vy = (y - scroll_y) + 5.0 + draw_h + 5.0;
                        for (idx, (line_str, line_w)) in cap_lines.iter().enumerate() {
                            effective_w = effective_w.max(*line_w);
                            let cx = if *line_w <= draw_w {
                                start_x + (draw_w - *line_w) * 0.5
                            } else {
                                start_x
                            };
                            fg_cmds.push(DrawCmd::Text {
                                x: cx,
                                baseline_y: cap_top_vy + (idx as f32) * cap_line_h + cap_base_off,
                                text: line_str.clone(),
                                role: FontRole::SansBody,
                                size_px: cap_px,
                                bold: false,
                                italic: true,
                                color: config.palette.muted_fg,
                            });
                        }
                        if !cap_lines.is_empty() {
                            cap_extra_h = 5.0 + (cap_lines.len() as f32) * cap_line_h + 2.0;
                        }
                    }

                    let img_row_h = draw_h + 10.0 + cap_extra_h;
                    let img_span = PositionedGlyphSpan {
                        doc_range: tok.range.clone(),
                        x: start_x,
                        width: effective_w,
                        measured: MeasuredRun {
                            total_width: effective_w,
                            metrics: base_m,
                            grapheme_ends: vec![(slice.len(), effective_w)],
                        },
                        is_hidden_marker: false,
                        link_url: Some(src.clone()),
                    };
                    visual_rows.push(VisualRow {
                        y_doc: y,
                        height: img_row_h,
                        baseline_offset: img_row_h - 4.0,
                        line_idx,
                        doc_range: tok.range.clone(),
                        default_x: start_x,
                        content_width: effective_w,
                        spans: vec![img_span],
                    });
                    emitted_any_row = true;
                    y += img_row_h;
                    sub_start = tok.range.end;
                    cur_x = start_x;
                    continue;
                }

                let badge = format_image_badge(alt, src, caption.as_deref(), size);
                let m =
                    measurer.measure_run(&badge, FontRole::SansBody, font_px * 0.92, false, false);
                let bw = m.total_width + 14.0;
                if config.word_wrap && cur_x > start_x && (cur_x - start_x) + bw > max_w {
                    let prev_end = tok.range.start;
                    let taken = std::mem::take(&mut cur_spans);
                    flush_row(y, cur_x, sub_start, prev_end, taken, visual_rows);
                    emitted_any_row = true;
                    y += row_h;
                    cur_x = start_x;
                    sub_start = prev_end;
                }
                let rect_v = RectF::new(cur_x, (y - scroll_y) + 2.0, bw, row_h - 4.0);
                bg_cmds.push(DrawCmd::FillRect {
                    rect: rect_v,
                    color: config.palette.inline_code_bg,
                    radius: 4.0,
                });
                bg_cmds.push(DrawCmd::StrokeRect {
                    rect: rect_v,
                    color: config.palette.gutter_border,
                    stroke_width: 1.0,
                    radius: 4.0,
                });
                fg_cmds.push(DrawCmd::Text {
                    x: cur_x + 7.0,
                    baseline_y: (y - scroll_y) + baseline_off,
                    text: badge,
                    role: FontRole::SansBody,
                    size_px: font_px * 0.92,
                    bold: false,
                    italic: false,
                    color: config.palette.link_fg,
                });
                cur_spans.push(PositionedGlyphSpan {
                    doc_range: tok.range,
                    x: cur_x,
                    width: bw,
                    measured: MeasuredRun {
                        total_width: bw,
                        metrics: base_m,
                        grapheme_ends: vec![(slice.len(), bw)],
                    },
                    is_hidden_marker: false,
                    link_url: Some(src.clone()),
                });
                cur_x += bw + 2.0;
            }
            _ => {
                let is_marker = matches!(tok.kind, InlineTokenKind::SyntaxMarker);
                let is_code = tok.style.contains(StyleFlags::CODE);
                let is_kbd = tok.style.contains(StyleFlags::KBD);
                let role = if is_code || is_kbd {
                    FontRole::Monospace
                } else {
                    FontRole::SansBody
                };
                let run_px = if is_kbd && !is_marker {
                    font_px * 0.86
                } else if is_code {
                    font_px * 0.90
                } else {
                    font_px
                };
                let bold = base_bold || tok.style.contains(StyleFlags::BOLD);
                let italic = tok.style.contains(StyleFlags::ITALIC);
                let link_url = match &tok.kind {
                    InlineTokenKind::LinkText { url } => Some(url.clone()),
                    _ => None,
                };
                let fg_color = if is_marker {
                    config.palette.muted_fg
                } else if tok.style.contains(StyleFlags::LINK) {
                    config.palette.link_fg
                } else {
                    base_color
                };

                // Unified pill path for `<kbd>` and inline `` `code` `` whenever the pill fits on a single line!
                if (is_kbd || is_code) && !is_marker {
                    let outer = if is_kbd { 1.5f32 } else { 1.0f32 };
                    let pad_x = if is_kbd { 6.0f32 } else { 4.5f32 };
                    let m_whole = measurer.measure_run(slice, role, run_px, bold, italic);
                    let total_pill_w = m_whole.total_width + (pad_x + outer) * 2.0;

                    if !config.word_wrap || total_pill_w <= max_w {
                        if config.word_wrap
                            && cur_x > start_x
                            && (cur_x - start_x) + total_pill_w > max_w
                        {
                            let taken = std::mem::take(&mut cur_spans);
                            flush_row(y, cur_x, sub_start, tok.range.start, taken, visual_rows);
                            emitted_any_row = true;
                            y += row_h;
                            cur_x = start_x;
                            sub_start = tok.range.start;
                        }

                        let vy = y - scroll_y;
                        let box_x = cur_x + outer;
                        let box_w = m_whole.total_width + pad_x * 2.0;
                        let pill_rect = RectF::new(box_x, vy + 2.0, box_w, row_h - 4.0);

                        if is_kbd {
                            bg_cmds.push(DrawCmd::FillRect {
                                rect: pill_rect,
                                color: config.palette.inline_code_bg,
                                radius: 4.5,
                            });
                            bg_cmds.push(DrawCmd::StrokeRect {
                                rect: pill_rect,
                                color: config.palette.table_border,
                                stroke_width: 1.0,
                                radius: 4.5,
                            });
                            // Subtle keycap bottom bevel line
                            bg_cmds.push(DrawCmd::Line {
                                x1: box_x + 3.0,
                                y1: vy + row_h - 2.2,
                                x2: box_x + box_w - 3.0,
                                y2: vy + row_h - 2.2,
                                color: config.palette.table_border,
                                width: 1.4,
                            });
                        } else {
                            bg_cmds.push(DrawCmd::FillRect {
                                rect: pill_rect,
                                color: config.palette.inline_code_bg,
                                radius: 4.0,
                            });
                        }

                        let text_x = box_x + pad_x;
                        fg_cmds.push(DrawCmd::Text {
                            x: text_x,
                            baseline_y: vy + baseline_off,
                            text: slice.to_owned(),
                            role,
                            size_px: run_px,
                            bold,
                            italic,
                            color: fg_color,
                        });

                        let w = m_whole.total_width;
                        cur_spans.push(PositionedGlyphSpan {
                            doc_range: tok.range.clone(),
                            x: text_x,
                            width: w,
                            measured: m_whole,
                            is_hidden_marker: false,
                            link_url: link_url.clone(),
                        });
                        cur_x += total_pill_w;
                        continue;
                    }
                }

                for (seg_rel, seg_str) in split_wrap_chunks(slice) {
                    let seg_base = tok.range.start.saturating_add(seg_rel);
                    let m_full = measurer.measure_run(seg_str, role, run_px, bold, italic);
                    let pad_x = if (is_kbd || is_code) && !is_marker {
                        3.5f32
                    } else {
                        0.0f32
                    };

                    if config.word_wrap
                        && cur_x > start_x
                        && !seg_str.trim().is_empty()
                        && (cur_x - start_x) + m_full.total_width + pad_x * 2.0 > max_w
                    {
                        let taken = std::mem::take(&mut cur_spans);
                        flush_row(y, cur_x, sub_start, seg_base, taken, visual_rows);
                        emitted_any_row = true;
                        y += row_h;
                        cur_x = start_x;
                        sub_start = seg_base;
                    }

                    // Grapheme-level fallback wrap if a single unbreakable word/URL exceeds `max_w`!
                    let sub_pieces: Vec<(usize, &str)> = if config.word_wrap
                        && !seg_str.trim().is_empty()
                        && m_full.total_width + pad_x * 2.0 > max_w
                    {
                        split_piece_by_width(seg_str, &m_full, (max_w - pad_x * 2.0).max(16.0))
                    } else {
                        vec![(0, seg_str)]
                    };

                    for (p_idx, (p_rel, p_str)) in sub_pieces.into_iter().enumerate() {
                        let p_start = seg_base.saturating_add(p_rel);
                        let p_end = p_start.saturating_add(p_str.len());
                        if p_idx > 0 {
                            let taken = std::mem::take(&mut cur_spans);
                            flush_row(y, cur_x, sub_start, p_start, taken, visual_rows);
                            emitted_any_row = true;
                            y += row_h;
                            cur_x = start_x;
                            sub_start = p_start;
                        }
                        let m = measurer.measure_run(p_str, role, run_px, bold, italic);
                        let vy = y - scroll_y;
                        let draw_x = cur_x + pad_x;

                        if tok.style.contains(StyleFlags::HIGHLIGHT) {
                            bg_cmds.push(DrawCmd::FillRect {
                                rect: RectF::new(
                                    cur_x,
                                    vy + 2.0,
                                    m.total_width + pad_x * 2.0,
                                    row_h - 4.0,
                                ),
                                color: config.palette.highlight_bg,
                                radius: 3.0,
                            });
                        }
                        if is_code && !is_marker {
                            bg_cmds.push(DrawCmd::FillRect {
                                rect: RectF::new(
                                    cur_x,
                                    vy + 2.0,
                                    m.total_width + pad_x * 2.0,
                                    row_h - 4.0,
                                ),
                                color: config.palette.inline_code_bg,
                                radius: 3.5,
                            });
                        }
                        if is_kbd && !is_marker {
                            let kr = RectF::new(
                                cur_x,
                                vy + 2.0,
                                m.total_width + pad_x * 2.0,
                                row_h - 4.0,
                            );
                            bg_cmds.push(DrawCmd::FillRect {
                                rect: kr,
                                color: config.palette.inline_code_bg,
                                radius: 4.0,
                            });
                            bg_cmds.push(DrawCmd::StrokeRect {
                                rect: kr,
                                color: config.palette.table_border,
                                stroke_width: 1.0,
                                radius: 4.0,
                            });
                        }

                        fg_cmds.push(DrawCmd::Text {
                            x: draw_x,
                            baseline_y: vy + baseline_off,
                            text: p_str.to_owned(),
                            role,
                            size_px: run_px,
                            bold,
                            italic,
                            color: fg_color,
                        });

                        if tok.style.contains(StyleFlags::LINK) && !is_marker {
                            fg_cmds.push(DrawCmd::Line {
                                x1: draw_x,
                                y1: vy + baseline_off + 2.0,
                                x2: draw_x + m.total_width,
                                y2: vy + baseline_off + 2.0,
                                color: config.palette.link_fg.with_alpha(180),
                                width: 1.0,
                            });
                        }
                        if tok.style.contains(StyleFlags::STRIKE) && !is_marker {
                            let strike_y = vy + baseline_off - font_px * 0.28;
                            fg_cmds.push(DrawCmd::Line {
                                x1: draw_x,
                                y1: strike_y,
                                x2: draw_x + m.total_width,
                                y2: strike_y,
                                color: fg_color,
                                width: 1.2,
                            });
                        }

                        let w = m.total_width;
                        cur_spans.push(PositionedGlyphSpan {
                            doc_range: p_start..p_end,
                            x: draw_x,
                            width: w,
                            measured: m,
                            is_hidden_marker: false,
                            link_url: link_url.clone(),
                        });
                        cur_x += w + pad_x * 2.0;
                    }
                }
            }
        }
    }

    if emitted_any_row && cur_spans.is_empty() && sub_start >= doc_range.end {
        return y;
    }

    flush_row(y, cur_x, sub_start, doc_range.end, cur_spans, visual_rows);
    y + row_h
}

fn split_piece_by_width<'a>(
    s: &'a str,
    measured: &MeasuredRun,
    limit_w: f32,
) -> Vec<(usize, &'a str)> {
    let mut pieces = Vec::new();
    let mut piece_start_b = 0usize;
    let mut piece_start_x = 0.0f32;
    let mut last_end_b = 0usize;
    let mut last_end_x = 0.0f32;

    for &(end_b, end_x) in &measured.grapheme_ends {
        if end_x - piece_start_x > limit_w && last_end_b > piece_start_b {
            if let Some(sub) = s.get(piece_start_b..last_end_b) {
                pieces.push((piece_start_b, sub));
            }
            piece_start_b = last_end_b;
            piece_start_x = last_end_x;
        }
        last_end_b = end_b;
        last_end_x = end_x;
    }

    if piece_start_b < s.len()
        && let Some(tail) = s.get(piece_start_b..)
    {
        pieces.push((piece_start_b, tail));
    }
    if pieces.is_empty() {
        pieces.push((0, s));
    }
    pieces
}

fn split_wrap_chunks(s: &str) -> Vec<(usize, &str)> {
    let mut chunks = Vec::new();
    let mut start = 0usize;
    let mut in_ws: Option<bool> = None;
    for (idx, ch) in s.char_indices() {
        let ws = ch.is_whitespace();
        match in_ws {
            None => in_ws = Some(ws),
            Some(prev_ws) if prev_ws && !ws => {
                if let Some(piece) = s.get(start..idx) {
                    chunks.push((start, piece));
                }
                start = idx;
                in_ws = Some(false);
            }
            Some(_) => {
                in_ws = Some(ws);
            }
        }
    }
    if start < s.len()
        && let Some(tail) = s.get(start..)
    {
        chunks.push((start, tail));
    }
    chunks
}

fn format_image_badge(
    alt: &str,
    src: &str,
    caption: Option<&str>,
    size: Option<ImageSize>,
) -> String {
    let base = if alt.is_empty() { src } else { alt };
    let mut out = format!("🖼 {base}");
    match size {
        Some(ImageSize::Percent(pct)) => out.push_str(&format!(" [{pct}%]")),
        Some(ImageSize::Pixels(px)) => out.push_str(&format!(" [{px}px]")),
        None => {}
    }
    if let Some(cap) = caption.map(str::trim).filter(|s| !s.is_empty()) {
        out.push_str(&format!(" — “{cap}”"));
    }
    out
}

fn wrap_caption_lines(
    text: &str,
    max_w: f32,
    font_px: f32,
    measurer: &mut dyn TextMeasurer,
) -> (Vec<(String, f32)>, FontMetrics) {
    let limit = max_w.max(36.0);
    let base_m = measurer.font_metrics(FontRole::SansBody, font_px, false, true);
    let mut lines: Vec<(String, f32)> = Vec::new();
    let mut cur_line = String::new();
    let mut cur_w = 0.0f32;

    for (_off, chunk) in split_wrap_chunks(text) {
        let m = measurer.measure_run(chunk, FontRole::SansBody, font_px, false, true);
        if cur_line.is_empty() && m.total_width <= limit {
            cur_line.push_str(chunk);
            cur_w = m.total_width;
        } else if !cur_line.is_empty() && cur_w + m.total_width <= limit {
            cur_line.push_str(chunk);
            cur_w += m.total_width;
        } else {
            if !cur_line.is_empty() {
                let trimmed = cur_line.trim_end().to_owned();
                let tw = measurer
                    .measure_run(&trimmed, FontRole::SansBody, font_px, false, true)
                    .total_width;
                lines.push((trimmed, tw));
                cur_line.clear();
                cur_w = 0.0;
            }
            let leading_trimmed = chunk.trim_start();
            if leading_trimmed.is_empty() {
                continue;
            }
            let tm =
                measurer.measure_run(leading_trimmed, FontRole::SansBody, font_px, false, true);
            if tm.total_width <= limit {
                cur_line.push_str(leading_trimmed);
                cur_w = tm.total_width;
            } else {
                for (_sub_off, sub) in split_piece_by_width(leading_trimmed, &tm, limit) {
                    let sm = measurer.measure_run(sub, FontRole::SansBody, font_px, false, true);
                    lines.push((sub.to_owned(), sm.total_width));
                }
            }
        }
    }

    if !cur_line.is_empty() {
        let trimmed = cur_line.trim_end().to_owned();
        if !trimmed.is_empty() {
            let tw = measurer
                .measure_run(&trimmed, FontRole::SansBody, font_px, false, true)
                .total_width;
            lines.push((trimmed, tw));
        }
    }

    (lines, base_m)
}

#[allow(clippy::too_many_arguments)]
fn layout_plain_code_line(
    rope: &Rope,
    line_idx: usize,
    doc_range: Range<usize>,
    y: f32,
    start_x: f32,
    code_px: f32,
    color: Rgba,
    scroll_y: f32,
    fg_cmds: &mut Vec<DrawCmd>,
    visual_rows: &mut Vec<VisualRow>,
    measurer: &mut dyn TextMeasurer,
) -> f32 {
    let txt = rope.slice_to_string(doc_range.clone());
    let m = measurer.measure_run(&txt, FontRole::Monospace, code_px, false, false);
    let h = m.metrics.line_height.max(code_px * 1.38);
    let base_off = (h - (m.metrics.ascent + m.metrics.descent)) * 0.5 + m.metrics.ascent;
    if !txt.is_empty() {
        fg_cmds.push(DrawCmd::Text {
            x: start_x,
            baseline_y: (y - scroll_y) + base_off,
            text: txt,
            role: FontRole::Monospace,
            size_px: code_px,
            bold: false,
            italic: false,
            color,
        });
    }
    let w = m.total_width;
    let spans = if doc_range.is_empty() {
        Vec::new()
    } else {
        vec![PositionedGlyphSpan {
            doc_range: doc_range.clone(),
            x: start_x,
            width: w,
            measured: m,
            is_hidden_marker: false,
            link_url: None,
        }]
    };
    visual_rows.push(VisualRow {
        y_doc: y,
        height: h,
        baseline_offset: base_off,
        line_idx,
        doc_range,
        default_x: start_x,
        content_width: w,
        spans,
    });
    y + h
}

/// Fast `O(visible_lines)` virtualizer for `ViewMode::Source` and `ViewMode::PlainText`.
/// Handles million-line TXT/MD files in `< 0.3 ms`!
#[allow(clippy::too_many_arguments)]
fn build_line_based_frame(
    rope: &Rope,
    selection: Selection,
    ime_preedit: Option<&str>,
    search_matches: &[Range<usize>],
    active_match_idx: Option<usize>,
    config: &LayoutConfig,
    vw: f32,
    vh: f32,
    scroll_y: f32,
    gutter_w: f32,
    content_x: f32,
    prose_w: f32,
    line_h: f32,
    mut draw_cmds: Vec<DrawCmd>,
    measurer: &mut dyn TextMeasurer,
) -> ViewportFrame {
    let total_lines = rope.line_count();
    let pad_top = 16.0f32;
    let role = if config.view_mode == ViewMode::Source {
        FontRole::Monospace
    } else {
        FontRole::SansBody
    };

    let first_vis = ((scroll_y - pad_top).max(0.0) / line_h).floor() as usize;
    let vis_count = (vh / line_h).ceil() as usize + 6;
    let end_vis = (first_vis.saturating_add(vis_count)).min(total_lines);
    let caret_line = rope.byte_to_line_col(selection.head).line;

    let mut visual_rows = Vec::with_capacity(vis_count + 2);
    let mut bg_cmds = Vec::new();
    let mut fg_cmds = Vec::new();

    // Ensure caret line is also shaped if off-screen so `caret_doc_rect` stays exact.
    let mut lines_to_build: Vec<usize> = (first_vis..end_vis).collect();
    if caret_line < total_lines && !lines_to_build.contains(&caret_line) {
        lines_to_build.push(caret_line);
        lines_to_build.sort_unstable();
    }

    for l_idx in lines_to_build {
        let y_doc = pad_top + (l_idx as f32) * line_h;
        let r = rope.line_byte_range(l_idx);
        // Cap single-line shaping window to 4096 bytes for multi-MB single-line resilience!
        let capped_end = if r.len() > 4096 {
            rope.clamp_char_boundary(r.start.saturating_add(4096))
        } else {
            r.end
        };
        let txt = rope.slice_to_string(r.start..capped_end);
        let is_heading_src =
            config.view_mode == ViewMode::Source && txt.trim_start().starts_with('#');
        let color = if is_heading_src {
            config.palette.accent
        } else {
            config.palette.text_fg
        };
        let m = measurer.measure_run(&txt, role, config.base_font_px, is_heading_src, false);
        let base_off = (line_h - (m.metrics.ascent + m.metrics.descent)) * 0.5 + m.metrics.ascent;

        if y_doc + line_h >= scroll_y - 32.0 && y_doc <= scroll_y + vh + 32.0 && !txt.is_empty() {
            fg_cmds.push(DrawCmd::Text {
                x: content_x,
                baseline_y: (y_doc - scroll_y) + base_off,
                text: txt,
                role,
                size_px: config.base_font_px,
                bold: is_heading_src,
                italic: false,
                color,
            });
        }
        let w = m.total_width;
        let spans = if r.is_empty() {
            Vec::new()
        } else {
            vec![PositionedGlyphSpan {
                doc_range: r.start..capped_end,
                x: content_x,
                width: w,
                measured: m,
                is_hidden_marker: false,
                link_url: None,
            }]
        };
        visual_rows.push(VisualRow {
            y_doc,
            height: line_h,
            baseline_offset: base_off,
            line_idx: l_idx,
            doc_range: r,
            default_x: content_x,
            content_width: w,
            spans,
        });
    }

    let total_height = (pad_top + (total_lines as f32) * line_h + 64.0).max(vh);
    emit_range_highlights(
        &visual_rows,
        search_matches,
        active_match_idx,
        selection,
        scroll_y,
        &config.palette,
        &mut bg_cmds,
    );

    let caret_doc_rect =
        compute_caret_rect(&visual_rows, selection.head, content_x, pad_top, line_h);
    let caret_viewport_rect = RectF::new(
        caret_doc_rect.x,
        caret_doc_rect.y - scroll_y,
        2.0,
        caret_doc_rect.h,
    );

    if let Some(preedit) = ime_preedit.filter(|s| !s.is_empty()) {
        let m = measurer.measure_run(preedit, role, config.base_font_px, false, false);
        let py = caret_viewport_rect.y;
        fg_cmds.push(DrawCmd::FillRect {
            rect: RectF::new(
                caret_viewport_rect.x,
                py,
                m.total_width + 4.0,
                caret_viewport_rect.h,
            ),
            color: config.palette.canvas_bg,
            radius: 2.0,
        });
        fg_cmds.push(DrawCmd::Text {
            x: caret_viewport_rect.x + 1.0,
            baseline_y: py + m.metrics.ascent,
            text: preedit.to_owned(),
            role,
            size_px: config.base_font_px,
            bold: false,
            italic: false,
            color: config.palette.text_fg,
        });
    } else if selection.is_empty() {
        fg_cmds.push(DrawCmd::FillRect {
            rect: caret_viewport_rect,
            color: config.palette.caret,
            radius: 0.0,
        });
    }

    draw_cmds.extend(bg_cmds);
    draw_cmds.extend(fg_cmds);
    if gutter_w > 0.0 {
        emit_gutter(
            &visual_rows,
            vw,
            vh,
            gutter_w,
            scroll_y,
            config,
            &mut draw_cmds,
            measurer,
        );
    }

    ViewportFrame {
        viewport_w: vw,
        viewport_h: vh,
        scroll_y,
        total_height,
        gutter_w,
        content_x,
        content_w: prose_w,
        caret_viewport_rect,
        caret_doc_rect,
        draw_cmds,
        visual_rows,
        checkboxes: Vec::new(),
        code_lang_badges: Vec::new(),
        callout_badges: Vec::new(),
    }
}

fn compute_caret_rect(
    visual_rows: &[VisualRow],
    caret_byte: usize,
    default_x: f32,
    default_y: f32,
    default_h: f32,
) -> RectF {
    for row in visual_rows {
        if caret_byte >= row.doc_range.start && caret_byte <= row.doc_range.end {
            // Check spans inside row first (handles multi-column table cells sharing `line_idx`!).
            for span in &row.spans {
                if caret_byte >= span.doc_range.start && caret_byte <= span.doc_range.end {
                    let rel = caret_byte.saturating_sub(span.doc_range.start);
                    let x = if span.is_hidden_marker {
                        span.x
                    } else {
                        span.x + span.measured.x_at_rel_byte(rel)
                    };
                    return RectF::new(x, row.y_doc, 2.0, row.height);
                }
            }
            if row.spans.is_empty()
                && row.doc_range.start == row.doc_range.end
                && caret_byte == row.doc_range.start
            {
                return RectF::new(row.default_x, row.y_doc, 2.0, row.height);
            }
        }
    }
    // Second pass: relaxed match on row range when caret sits between spans (e.g. empty heading body).
    for row in visual_rows {
        if caret_byte >= row.doc_range.start && caret_byte <= row.doc_range.end {
            let x = row.spans.last().map_or(row.default_x, |s| s.x + s.width);
            return RectF::new(x, row.y_doc, 2.0, row.height);
        }
    }
    RectF::new(default_x, default_y, 2.0, default_h)
}

fn emit_range_highlights(
    visual_rows: &[VisualRow],
    search_matches: &[Range<usize>],
    active_match_idx: Option<usize>,
    selection: Selection,
    scroll_y: f32,
    palette: &ThemePalette,
    bg_cmds: &mut Vec<DrawCmd>,
) {
    let paint_range = |range: Range<usize>, color: Rgba, out: &mut Vec<DrawCmd>| {
        if range.is_empty() {
            return;
        }
        for row in visual_rows {
            if range.end <= row.doc_range.start || range.start > row.doc_range.end {
                continue;
            }
            for span in &row.spans {
                if span.is_hidden_marker
                    || range.end <= span.doc_range.start
                    || range.start >= span.doc_range.end
                {
                    continue;
                }
                let s_rel = range.start.max(span.doc_range.start) - span.doc_range.start;
                let e_rel = range.end.min(span.doc_range.end) - span.doc_range.start;
                let x0 = span.x + span.measured.x_at_rel_byte(s_rel);
                let x1 = span.x + span.measured.x_at_rel_byte(e_rel);
                if x1 > x0 {
                    out.push(DrawCmd::FillRect {
                        rect: RectF::new(x0, row.y_doc - scroll_y, x1 - x0, row.height),
                        color,
                        radius: 2.0,
                    });
                }
            }
        }
    };

    for (idx, m) in search_matches.iter().enumerate() {
        let col = if active_match_idx == Some(idx) {
            palette.search_current_bg
        } else {
            palette.search_match_bg
        };
        paint_range(m.clone(), col, bg_cmds);
    }

    if !selection.is_empty() {
        paint_range(selection.range(), palette.selection_bg, bg_cmds);
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_gutter(
    visual_rows: &[VisualRow],
    _vw: f32,
    vh: f32,
    gutter_w: f32,
    scroll_y: f32,
    config: &LayoutConfig,
    draw_cmds: &mut Vec<DrawCmd>,
    measurer: &mut dyn TextMeasurer,
) {
    draw_cmds.push(DrawCmd::FillRect {
        rect: RectF::new(0.0, 0.0, gutter_w, vh),
        color: config.palette.gutter_bg,
        radius: 0.0,
    });
    draw_cmds.push(DrawCmd::Line {
        x1: gutter_w,
        y1: 0.0,
        x2: gutter_w,
        y2: vh,
        color: config.palette.gutter_border,
        width: 1.0,
    });

    let num_px = (config.base_font_px * 0.82).max(10.0);
    let mut last_drawn_line: Option<usize> = None;
    for row in visual_rows {
        if last_drawn_line == Some(row.line_idx) {
            continue;
        }
        last_drawn_line = Some(row.line_idx);
        let vy = row.y_doc - scroll_y;
        if vy + row.height < 0.0 || vy > vh {
            continue;
        }
        let label = format!("{}", row.line_idx.saturating_add(1));
        let m = measurer.measure_run(&label, FontRole::Monospace, num_px, false, false);
        draw_cmds.push(DrawCmd::Text {
            x: (gutter_w - m.total_width - 10.0).max(4.0),
            baseline_y: vy + row.baseline_offset,
            text: label,
            role: FontRole::Monospace,
            size_px: num_px,
            bold: false,
            italic: false,
            color: config.palette.gutter_fg,
        });
    }
}
