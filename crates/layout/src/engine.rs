//! Core viewport frame types, persistent `BlockHeightCache`, virtualized block dispatch,
//! `O(visible_lines)` plain/source virtualizer, highlights, caret, and line-number gutter.

use std::ops::Range;

use kmark_buffer::{Rope, Selection};
use kmark_syntax::{AlertKind, Block, BlockKind};

use crate::context::{
    CANVAS_PAD_X, CARET_WIDTH_PX, CodeLineSpec, DOC_BOTTOM_OVERSCROLL_Y, FORMATTED_TOP_PAD_Y,
    FrameGeometry, HEADING_SCALES, InlineLineSpec, LINE_MODE_TOP_PAD_Y, LayoutCtx,
    MAX_CACHED_BLOCK_HEIGHTS, MAX_SINGLE_LINE_SHAPE_BYTES, MIN_PROSE_W, MIN_VIEWPORT_H,
    MIN_VIEWPORT_W, OVERSCAN_EXTRA_PX, OVERSCAN_VIEWPORT_FRAC, ViewportRequest,
};
use crate::hit_test::{CalloutBadgeHitBox, CheckboxHitBox, CodeLangHitBox};
use crate::inline_layout::{layout_inline_line, layout_plain_code_line};
use crate::measurer::{FontRole, MeasuredRun, TextMeasurer};
use crate::table_layout::layout_table_2d;
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
    /// Signature slanted indicator bars (`1..=6` bars).
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct HeightCacheConfigKey {
    prose_w_tenths: u32,
    default_line_h_tenths: u32,
    base_font_tenths: u32,
    line_spacing_hundredths: u32,
    word_wrap: bool,
    heading_marks_tag: u8,
    ui_german: bool,
}

impl HeightCacheConfigKey {
    fn from_layout(prose_w: f32, default_line_h: f32, config: &LayoutConfig) -> Self {
        let marks_tag = match config.heading_marks {
            HeadingMarkStyle::SlantedBars => 0,
            HeadingMarkStyle::HashSymbol => 1,
            HeadingMarkStyle::None => 2,
        };
        Self {
            prose_w_tenths: (prose_w.max(1.0) * 10.0).round() as u32,
            default_line_h_tenths: (default_line_h.max(1.0) * 10.0).round() as u32,
            base_font_tenths: (config.base_font_px.max(1.0) * 10.0).round() as u32,
            line_spacing_hundredths: (config.line_spacing_scale.max(0.5) * 100.0).round() as u32,
            word_wrap: config.word_wrap,
            heading_marks_tag: marks_tag,
            ui_german: config.ui_german,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct BlockContentKey {
    kind_tag: u8,
    byte_len: usize,
    content_hash: u64,
}

impl BlockContentKey {
    fn from_block(rope: &Rope, block: &Block) -> Self {
        let kind_tag = match block.kind {
            BlockKind::BlankLines => 0,
            BlockKind::ThematicBreak => 1,
            BlockKind::Heading { level, .. } => 10u8.saturating_add(level),
            BlockKind::Paragraph { .. } => 20,
            BlockKind::ListItem {
                ordered, ref task, ..
            } => 30u8
                .saturating_add(u8::from(ordered.is_some()))
                .saturating_add(if task.is_some() { 2 } else { 0 }),
            BlockKind::BlockQuote { alert, .. } => {
                40u8.saturating_add(alert.map_or(0, |a| match a {
                    AlertKind::Note => 1,
                    AlertKind::Tip => 2,
                    AlertKind::Important => 3,
                    AlertKind::Warning => 4,
                    AlertKind::Caution => 5,
                }))
            }
            BlockKind::CodeFence { .. } => 50,
            BlockKind::Table(_) => 60,
        };
        Self {
            kind_tag,
            byte_len: block.byte_range.len(),
            content_hash: rope.hash_byte_range(block.byte_range.clone()),
        }
    }
}

/// Persistent per-tab cache of exact measured formatted block heights so off-screen blocks
/// retain their exact heights across scrolling and single-site edits (eliminating scroll jumps
/// and scrollbar breathing).
#[derive(Clone, Debug, Default)]
pub struct BlockHeightCache {
    config_key: Option<HeightCacheConfigKey>,
    last_rope: Option<Rope>,
    by_index: Vec<Option<f32>>,
    by_content: std::collections::HashMap<BlockContentKey, f32>,
}

impl BlockHeightCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.config_key = None;
        self.last_rope = None;
        self.by_index.clear();
        self.by_content.clear();
    }

    fn prepare(
        &mut self,
        rope: &Rope,
        block_count: usize,
        prose_w: f32,
        default_line_h: f32,
        config: &LayoutConfig,
    ) {
        let next_cfg = HeightCacheConfigKey::from_layout(prose_w, default_line_h, config);
        if self.config_key != Some(next_cfg) {
            self.config_key = Some(next_cfg);
            self.last_rope = Some(rope.clone());
            self.by_index.clear();
            self.by_index.resize(block_count, None);
            self.by_content.clear();
            return;
        }

        let same_rope = self
            .last_rope
            .as_ref()
            .is_some_and(|prev| prev.ptr_eq(rope));
        if !same_rope || self.by_index.len() != block_count {
            self.last_rope = Some(rope.clone());
            self.by_index.clear();
            self.by_index.resize(block_count, None);
        }
    }

    fn lookup(&mut self, block_idx: usize, rope: &Rope, block: &Block) -> Option<f32> {
        if let Some(Some(h)) = self.by_index.get(block_idx).copied() {
            return Some(h);
        }
        let key = BlockContentKey::from_block(rope, block);
        if let Some(&h) = self.by_content.get(&key) {
            if let Some(slot) = self.by_index.get_mut(block_idx) {
                *slot = Some(h);
            }
            return Some(h);
        }
        None
    }

    fn record(&mut self, block_idx: usize, rope: &Rope, block: &Block, height: f32) {
        if let Some(slot) = self.by_index.get_mut(block_idx) {
            *slot = Some(height);
        }
        if self.by_content.len() >= MAX_CACHED_BLOCK_HEIGHTS {
            self.by_content.clear();
        }
        let key = BlockContentKey::from_block(rope, block);
        self.by_content.insert(key, height);
    }
}

/// Builds a viewport-virtualized [`ViewportFrame`] without a persistent height cache.
pub fn build_viewport_frame(
    req: &ViewportRequest<'_>,
    measurer: &mut dyn TextMeasurer,
) -> ViewportFrame {
    build_viewport_frame_cached(req, measurer, None)
}

/// Builds a viewport-virtualized [`ViewportFrame`] backed by an optional persistent [`BlockHeightCache`]
/// so measured block heights remain completely invariant during scrolling.
pub fn build_viewport_frame_cached(
    req: &ViewportRequest<'_>,
    measurer: &mut dyn TextMeasurer,
    mut height_cache: Option<&mut BlockHeightCache>,
) -> ViewportFrame {
    let vw = req.viewport_w.max(MIN_VIEWPORT_W);
    let vh = req.viewport_h.max(MIN_VIEWPORT_H);
    let base_metrics =
        measurer.font_metrics(FontRole::SansBody, req.config.base_font_px, false, false);
    let default_line_h =
        (req.config.base_font_px * req.config.line_spacing_scale).max(base_metrics.line_height);

    let gutter_w = if req.config.effective_line_numbers() {
        let digits = req
            .rope
            .line_count()
            .max(1)
            .ilog10()
            .saturating_add(1)
            .max(3) as f32;
        (digits * req.config.base_font_px * 0.62 + 24.0).round()
    } else {
        0.0
    };

    let avail_w = (vw - gutter_w).max(160.0);
    let prose_w = if matches!(
        req.config.view_mode,
        ViewMode::Wysiwyg | ViewMode::LivePreview
    ) && req.config.max_prose_width_px > 200.0
    {
        (avail_w - CANVAS_PAD_X * 2.0).clamp(MIN_PROSE_W, req.config.max_prose_width_px)
    } else {
        (avail_w - CANVAS_PAD_X * 2.0).max(MIN_PROSE_W)
    };
    let content_x = if matches!(
        req.config.view_mode,
        ViewMode::Wysiwyg | ViewMode::LivePreview
    ) && avail_w > prose_w + CANVAS_PAD_X * 2.0
    {
        gutter_w + ((avail_w - prose_w) * 0.5).round()
    } else {
        gutter_w + CANVAS_PAD_X
    };

    let geom = FrameGeometry {
        vw,
        vh,
        gutter_w,
        content_x,
        prose_w,
        default_line_h,
    };

    let mut draw_cmds = Vec::with_capacity(256);
    draw_cmds.push(DrawCmd::FillRect {
        rect: RectF::new(0.0, 0.0, vw, vh),
        color: req.config.palette.canvas_bg,
        radius: 0.0,
    });

    if matches!(req.config.view_mode, ViewMode::PlainText | ViewMode::Source) {
        return build_line_based_frame(req, geom, draw_cmds, measurer);
    }

    if let Some(cache) = height_cache.as_deref_mut() {
        cache.prepare(
            req.rope,
            req.blocks.len(),
            prose_w,
            default_line_h,
            req.config,
        );
    }

    // Formatted (WYSIWYG / LivePreview) block-indexed virtualized layout!
    let caret_byte = req.selection.head;
    let caret_line = req.rope.byte_to_line_col(caret_byte).line;

    // Step 1: cached exact heights (or wrap-aware estimates for never-seen blocks);
    // full layout for visible + overscan window + active caret block.
    let overscan = vh * OVERSCAN_VIEWPORT_FRAC + OVERSCAN_EXTRA_PX;
    let win_top = (req.scroll_y - overscan).max(0.0);
    let win_bot = req.scroll_y + vh + overscan;

    let mut y_cursor = FORMATTED_TOP_PAD_Y;
    let mut ctx = LayoutCtx {
        rope: req.rope,
        config: req.config,
        content_x,
        prose_w,
        default_line_h,
        scroll_y: req.scroll_y,
        bg_cmds: Vec::new(),
        fg_cmds: Vec::new(),
        visual_rows: Vec::new(),
        checkboxes: Vec::new(),
        code_lang_badges: Vec::new(),
        callout_badges: Vec::new(),
        measurer,
    };

    for (block_idx, block) in req.blocks.iter().enumerate() {
        let contains_caret = caret_byte >= block.byte_range.start
            && caret_byte <= block.byte_range.end
            || (caret_line >= block.line_range.start && caret_line < block.line_range.end);
        let reveal_raw = req.config.view_mode == ViewMode::LivePreview && contains_caret;
        let cacheable_formatted = !reveal_raw;

        let cached_h = if cacheable_formatted {
            height_cache
                .as_deref_mut()
                .and_then(|c| c.lookup(block_idx, req.rope, block))
        } else {
            None
        };
        let known_or_est_h = cached_h.unwrap_or_else(|| {
            estimate_block_height(
                block,
                prose_w,
                req.config.base_font_px,
                default_line_h,
                req.config.word_wrap,
            )
        });
        let near_viewport = y_cursor + known_or_est_h >= win_top && y_cursor <= win_bot;

        if !near_viewport && !contains_caret {
            y_cursor += known_or_est_h;
            continue;
        }

        let block_start_y = y_cursor;
        y_cursor = layout_single_block(&mut ctx, block, y_cursor, reveal_raw, caret_line);

        let measured_h = (y_cursor - block_start_y).max(0.0);
        if cacheable_formatted && let Some(cache) = height_cache.as_deref_mut() {
            cache.record(block_idx, req.rope, block, measured_h);
        }

        // Optional Focus Mode: dim non-active blocks!
        if req.config.focus_mode && !contains_caret && measured_h > 0.0 {
            ctx.fg_cmds.push(DrawCmd::FillRect {
                rect: RectF::new(
                    gutter_w,
                    block_start_y - req.scroll_y,
                    vw - gutter_w,
                    measured_h,
                ),
                color: req.config.palette.canvas_bg.with_alpha(150),
                radius: 0.0,
            });
        }
    }

    if req.blocks.is_empty() {
        ctx.visual_rows.push(VisualRow {
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

    let total_height = (y_cursor + DOC_BOTTOM_OVERSCROLL_Y).max(vh);

    // Search highlights & selection highlights behind text
    emit_range_highlights(
        &ctx.visual_rows,
        req.search_matches,
        req.active_match_idx,
        req.selection,
        req.scroll_y,
        &req.config.palette,
        &mut ctx.bg_cmds,
    );

    // Caret calculation
    let caret_doc_rect = compute_caret_rect(
        &ctx.visual_rows,
        caret_byte,
        content_x,
        FORMATTED_TOP_PAD_Y,
        default_line_h,
    );
    let caret_viewport_rect = RectF::new(
        caret_doc_rect.x,
        caret_doc_rect.y - req.scroll_y,
        CARET_WIDTH_PX,
        caret_doc_rect.h,
    );

    // Optional IME preedit underline overlay at caret
    if let Some(preedit) = req.ime_preedit.filter(|s| !s.is_empty()) {
        let m = ctx.measurer.measure_run(
            preedit,
            FontRole::SansBody,
            req.config.base_font_px,
            false,
            false,
        );
        let py = caret_viewport_rect.y;
        ctx.fg_cmds.push(DrawCmd::FillRect {
            rect: RectF::new(
                caret_viewport_rect.x,
                py,
                m.total_width + 4.0,
                caret_viewport_rect.h,
            ),
            color: req.config.palette.canvas_bg,
            radius: 2.0,
        });
        ctx.fg_cmds.push(DrawCmd::Text {
            x: caret_viewport_rect.x + 1.0,
            baseline_y: py + m.metrics.ascent,
            text: preedit.to_owned(),
            role: FontRole::SansBody,
            size_px: req.config.base_font_px,
            bold: false,
            italic: false,
            color: req.config.palette.text_fg,
        });
        ctx.fg_cmds.push(DrawCmd::Line {
            x1: caret_viewport_rect.x,
            y1: py + caret_viewport_rect.h - 1.0,
            x2: caret_viewport_rect.x + m.total_width + 2.0,
            y2: py + caret_viewport_rect.h - 1.0,
            color: req.config.palette.accent,
            width: 2.0,
        });
    } else if req.selection.is_empty() {
        ctx.fg_cmds.push(DrawCmd::FillRect {
            rect: caret_viewport_rect,
            color: req.config.palette.caret,
            radius: 0.0,
        });
    }

    let LayoutCtx {
        bg_cmds,
        fg_cmds,
        visual_rows,
        checkboxes,
        code_lang_badges,
        callout_badges,
        measurer,
        ..
    } = ctx;

    draw_cmds.extend(bg_cmds);
    draw_cmds.extend(fg_cmds);

    if gutter_w > 0.0 {
        emit_gutter(
            &visual_rows,
            &geom,
            req.config,
            req.scroll_y,
            &mut draw_cmds,
            measurer,
        );
    }

    ViewportFrame {
        viewport_w: vw,
        viewport_h: vh,
        scroll_y: req.scroll_y,
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

fn estimate_wrapped_visual_lines(
    byte_len: usize,
    avail_w: f32,
    avg_char_px: f32,
    word_wrap: bool,
) -> f32 {
    if !word_wrap || byte_len == 0 {
        return 1.0;
    }
    let chars_per_line = (avail_w.max(80.0) / avg_char_px.max(4.0)).max(10.0);
    // Account for ~8% word-wrap slack at spaces and occasional multi-byte UTF-8 characters.
    let approx_chars = (byte_len as f32) * 0.96;
    ((approx_chars * 1.08) / chars_per_line).ceil().max(1.0)
}

fn estimate_block_height(
    block: &Block,
    prose_w: f32,
    base_font_px: f32,
    line_h: f32,
    word_wrap: bool,
) -> f32 {
    let avg_char_px = (base_font_px * 0.54).max(4.5);
    match block.kind {
        BlockKind::BlankLines => (block.line_range.len().max(1) as f32) * (line_h * 0.75),
        BlockKind::ThematicBreak => 24.0,
        BlockKind::Heading {
            level,
            ref content_range,
            ..
        } => {
            let scale = HEADING_SCALES
                .get(usize::from(level))
                .copied()
                .unwrap_or(1.0);
            let h_px = (base_font_px * scale).round();
            let h_line = line_h * scale;
            let wrapped = estimate_wrapped_visual_lines(
                content_range.len(),
                prose_w - CANVAS_PAD_X,
                h_px * 0.57,
                word_wrap,
            );
            let pad = if level <= 2 { 16.0 } else { 12.0 };
            wrapped * h_line + pad
        }
        BlockKind::Paragraph { ref lines } => {
            let mut visual_lines = 0.0f32;
            for (_, r) in lines {
                visual_lines +=
                    estimate_wrapped_visual_lines(r.len(), prose_w, avg_char_px, word_wrap);
            }
            visual_lines.max(1.0) * line_h + 4.0
        }
        BlockKind::ListItem {
            indent_spaces,
            ref task,
            ref content_range,
            ..
        } => {
            let indent_px = (indent_spaces as f32) * (base_font_px * 0.55);
            let slot_px = 24.0 + if task.is_some() { 22.0 } else { 0.0 };
            let avail = (prose_w - indent_px - slot_px).max(80.0);
            let wrapped =
                estimate_wrapped_visual_lines(content_range.len(), avail, avg_char_px, word_wrap);
            wrapped * line_h + 2.0
        }
        BlockKind::BlockQuote {
            alert, ref lines, ..
        } => {
            let inner_w = (prose_w - 24.0).max(80.0);
            let mut visual_lines = if alert.is_some() { 1.0 } else { 0.0 };
            let skip = usize::from(alert.is_some());
            for ql in lines.iter().skip(skip) {
                let indent = f32::from(ql.depth.saturating_sub(1)) * 14.0;
                visual_lines += estimate_wrapped_visual_lines(
                    ql.content_range.len(),
                    (inner_w - indent).max(60.0),
                    avg_char_px,
                    word_wrap,
                );
            }
            visual_lines.max(1.0) * line_h + 12.0
        }
        BlockKind::CodeFence { ref body_lines, .. } => {
            let code_line_h = line_h * 0.92;
            28.0 + (body_lines.len().max(1) as f32) * code_line_h + 18.0
        }
        BlockKind::Table(ref t) => (1.0 + t.body_rows.len() as f32) * (line_h + 12.0) + 16.0,
    }
}

fn layout_single_block(
    ctx: &mut LayoutCtx<'_>,
    block: &Block,
    mut y: f32,
    reveal_raw: bool,
    caret_line: usize,
) -> f32 {
    let content_x = ctx.content_x;
    let prose_w = ctx.prose_w;
    let default_line_h = ctx.default_line_h;
    let scroll_y = ctx.scroll_y;
    let base_font_px = ctx.config.base_font_px;

    match block.kind {
        BlockKind::BlankLines => {
            for l_idx in block.line_range.clone() {
                let r = ctx.rope.line_byte_range(l_idx);
                ctx.visual_rows.push(VisualRow {
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
            let r = ctx.rope.line_byte_range(block.line_range.start);
            if reveal_raw {
                layout_inline_line(
                    ctx,
                    InlineLineSpec {
                        line_idx: block.line_range.start,
                        doc_range: r,
                        y,
                        start_x: content_x,
                        max_w: prose_w,
                        font_px: base_font_px,
                        base_bold: false,
                        reveal_raw: true,
                        base_color: ctx.config.palette.muted_fg,
                    },
                )
            } else {
                let mid_y = y + 12.0;
                ctx.fg_cmds.push(DrawCmd::Line {
                    x1: content_x,
                    y1: mid_y - scroll_y,
                    x2: content_x + prose_w,
                    y2: mid_y - scroll_y,
                    color: ctx.config.palette.rule_line,
                    width: 1.5,
                });
                ctx.visual_rows.push(VisualRow {
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
            let h_size = (base_font_px * scale).round();
            let full_line = prefix_range.start..content_range.end;

            let text_x = match ctx.config.heading_marks {
                HeadingMarkStyle::SlantedBars if !reveal_raw => {
                    content_x + (f32::from(level) * 4.5 + 12.0)
                }
                HeadingMarkStyle::HashSymbol if !reveal_raw => {
                    let hashes = "#".repeat(usize::from(level));
                    let m = ctx.measurer.measure_run(
                        &hashes,
                        FontRole::SansBody,
                        h_size * 0.85,
                        true,
                        false,
                    );
                    ctx.fg_cmds.push(DrawCmd::Text {
                        x: content_x,
                        baseline_y: (y - scroll_y) + m.metrics.ascent + 2.0,
                        text: hashes,
                        role: FontRole::SansBody,
                        size_px: h_size * 0.85,
                        bold: true,
                        italic: false,
                        color: ctx.config.palette.muted_fg.with_alpha(150),
                    });
                    content_x + m.total_width + 8.0
                }
                _ => content_x,
            };

            let target_range = if reveal_raw {
                full_line
            } else {
                content_range.clone()
            };

            let first_row_idx = ctx.visual_rows.len();
            let text_fg = ctx.config.palette.text_fg;
            y = layout_inline_line(
                ctx,
                InlineLineSpec {
                    line_idx: block.line_range.start,
                    doc_range: target_range,
                    y,
                    start_x: text_x,
                    max_w: (prose_w - (text_x - content_x)).max(80.0),
                    font_px: h_size,
                    base_bold: true,
                    reveal_raw,
                    base_color: text_fg,
                },
            );

            // Prepend hidden prefix span onto first visual row so caret offsets inside `### ` resolve cleanly!
            if !reveal_raw
                && prefix_range.end > prefix_range.start
                && let Some(first_row) = ctx.visual_rows.get_mut(first_row_idx)
            {
                let metrics = ctx
                    .measurer
                    .font_metrics(FontRole::SansBody, h_size, true, false);
                first_row.doc_range.start = prefix_range.start;
                first_row.spans.insert(
                    0,
                    PositionedGlyphSpan {
                        doc_range: prefix_range.clone(),
                        x: text_x,
                        width: 0.0,
                        measured: MeasuredRun {
                            total_width: 0.0,
                            metrics,
                            grapheme_ends: Vec::new(),
                        },
                        is_hidden_marker: true,
                        link_url: None,
                    },
                );
            }

            if ctx.config.heading_marks == HeadingMarkStyle::SlantedBars
                && !reveal_raw
                && let Some(first_row) = ctx.visual_rows.get(first_row_idx)
            {
                let bar_h = (h_size * 0.82).max(12.0);
                let top_y = first_row.y_doc + first_row.baseline_offset - bar_h * 0.88;
                ctx.fg_cmds.push(DrawCmd::HeadingBars {
                    x: content_x,
                    top_y: top_y - scroll_y,
                    height: bar_h,
                    level,
                    color: ctx.config.palette.accent.with_alpha(190),
                });
            }

            if reveal_raw && let Some(u_range) = setext_underline.clone() {
                let muted_fg = ctx.config.palette.muted_fg;
                y = layout_inline_line(
                    ctx,
                    InlineLineSpec {
                        line_idx: block.line_range.start.saturating_add(1),
                        doc_range: u_range,
                        y,
                        start_x: content_x,
                        max_w: prose_w,
                        font_px: base_font_px,
                        base_bold: false,
                        reveal_raw: true,
                        base_color: muted_fg,
                    },
                );
            }

            if level <= 2 {
                ctx.fg_cmds.push(DrawCmd::Line {
                    x1: content_x,
                    y1: (y + 3.0) - scroll_y,
                    x2: content_x + prose_w,
                    y2: (y + 3.0) - scroll_y,
                    color: ctx.config.palette.rule_line,
                    width: 1.0,
                });
                y += 8.0;
            } else {
                y += 4.0;
            }
            y
        }
        BlockKind::Paragraph { ref lines } => {
            let text_fg = ctx.config.palette.text_fg;
            for &(l_idx, ref l_range) in lines {
                let raw_here = ctx.config.view_mode == ViewMode::LivePreview && l_idx == caret_line;
                y = layout_inline_line(
                    ctx,
                    InlineLineSpec {
                        line_idx: l_idx,
                        doc_range: l_range.clone(),
                        y,
                        start_x: content_x,
                        max_w: prose_w,
                        font_px: base_font_px,
                        base_bold: false,
                        reveal_raw: raw_here,
                        base_color: text_fg,
                    },
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
            let raw_here = ctx.config.view_mode == ViewMode::LivePreview && line_idx == caret_line;
            let indent_px = (indent_spaces as f32) * (base_font_px * 0.55);
            let marker_slot = 24.0f32;
            let task_slot = if task.is_some() { 22.0f32 } else { 0.0f32 };

            if raw_here {
                let text_fg = ctx.config.palette.text_fg;
                return layout_inline_line(
                    ctx,
                    InlineLineSpec {
                        line_idx,
                        doc_range: marker_range.start..content_range.end,
                        y,
                        start_x: content_x + indent_px,
                        max_w: (prose_w - indent_px).max(80.0),
                        font_px: base_font_px,
                        base_bold: false,
                        reveal_raw: true,
                        base_color: text_fg,
                    },
                ) + 2.0;
            }

            let body_x = content_x + indent_px + marker_slot + task_slot;
            let first_idx = ctx.visual_rows.len();
            let text_color = if task.as_ref().is_some_and(|t| t.checked) {
                ctx.config.palette.muted_fg
            } else {
                ctx.config.palette.text_fg
            };

            let next_y = layout_inline_line(
                ctx,
                InlineLineSpec {
                    line_idx,
                    doc_range: content_range.clone(),
                    y,
                    start_x: body_x,
                    max_w: (prose_w - (body_x - content_x)).max(80.0),
                    font_px: base_font_px,
                    base_bold: false,
                    reveal_raw: false,
                    base_color: text_color,
                },
            );

            if let Some(first_row) = ctx.visual_rows.get_mut(first_idx) {
                first_row.doc_range.start = marker_range.start;
                let baseline_v = (first_row.y_doc - scroll_y) + first_row.baseline_offset;
                let label = match ordered {
                    Some(n) => format!("{n}."),
                    None => "•".to_owned(),
                };
                ctx.fg_cmds.push(DrawCmd::Text {
                    x: content_x + indent_px + 4.0,
                    baseline_y: baseline_v,
                    text: label,
                    role: FontRole::SansBody,
                    size_px: base_font_px,
                    bold: ordered.is_none(),
                    italic: false,
                    color: ctx.config.palette.accent,
                });

                if let Some(ts) = task {
                    let box_sz = (base_font_px * 0.95).clamp(13.0, 20.0);
                    let cb_y_doc = first_row.y_doc + (first_row.height - box_sz) * 0.5;
                    let cb_x = content_x + indent_px + marker_slot;
                    ctx.checkboxes.push(CheckboxHitBox {
                        rect_doc: RectF::new(cb_x, cb_y_doc, box_sz, box_sz),
                        bracket_range: ts.bracket_range.clone(),
                        checked: ts.checked,
                    });
                    ctx.fg_cmds.push(DrawCmd::Checkbox {
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
            let bar_color = alert.map_or(ctx.config.palette.quote_bar, |a| {
                ctx.config.palette.alert_color(a)
            });
            let inner_x = content_x + 16.0;
            let inner_w = (prose_w - 24.0).max(80.0);

            let mut badge_reserve_w = 0.0f32;
            let mut min_alert_h = default_line_h;
            if let Some(kind) = alert {
                let badge_px = (base_font_px * 0.80).max(10.5);
                let badge_label = format!("{} ▾", kind.title(ctx.config.ui_german));
                let bm = ctx.measurer.measure_run(
                    &badge_label,
                    FontRole::SansBody,
                    badge_px,
                    true,
                    false,
                );
                let bw = bm.total_width + 14.0;
                let bh = (bm.metrics.line_height + 4.0).max(20.0);
                let bx = (content_x + prose_w - bw - 8.0).max(content_x + 4.0);
                let by_doc = box_top + 2.0;
                badge_reserve_w = bw + 12.0;
                min_alert_h = (bh + 8.0).max(default_line_h);

                let badge_rect_v = RectF::new(bx, by_doc - scroll_y, bw, bh);
                ctx.fg_cmds.push(DrawCmd::FillRect {
                    rect: badge_rect_v,
                    color: ctx.config.palette.canvas_bg.with_alpha(225),
                    radius: 5.0,
                });
                ctx.fg_cmds.push(DrawCmd::StrokeRect {
                    rect: badge_rect_v,
                    color: bar_color.with_alpha(150),
                    stroke_width: 1.0,
                    radius: 5.0,
                });
                let badge_base = (by_doc - scroll_y)
                    + (bh - (bm.metrics.ascent + bm.metrics.descent)) * 0.5
                    + bm.metrics.ascent;
                ctx.fg_cmds.push(DrawCmd::Text {
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
                ctx.callout_badges.push(CalloutBadgeHitBox {
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
                    .unwrap_or_else(|| kind.title(ctx.config.ui_german));
                let badge = format!("{title}  ");
                let m =
                    ctx.measurer
                        .measure_run(&badge, FontRole::SansBody, base_font_px, true, false);
                let h = default_line_h;
                ctx.fg_cmds.push(DrawCmd::Text {
                    x: inner_x,
                    baseline_y: (y - scroll_y) + m.metrics.ascent + 2.0,
                    text: badge,
                    role: FontRole::SansBody,
                    size_px: base_font_px,
                    bold: true,
                    italic: false,
                    color: bar_color,
                });
                if let Some(first_ql) = lines.first() {
                    ctx.visual_rows.push(VisualRow {
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
            let quote_body_color = if alert.is_some() {
                ctx.config.palette.text_fg
            } else {
                ctx.config.palette.quote_fg
            };
            for (line_pos, ql) in lines.iter().enumerate().skip(skip) {
                let raw_line = reveal_raw
                    || (ctx.config.view_mode == ViewMode::LivePreview && ql.line_idx == caret_line);
                let indent = f32::from(ql.depth.saturating_sub(1)) * 14.0;
                let range = if raw_line {
                    ql.marker_range.start..ql.content_range.end
                } else {
                    ql.content_range.clone()
                };
                let right_pad = if line_pos == 0 { badge_reserve_w } else { 0.0 };
                y = layout_inline_line(
                    ctx,
                    InlineLineSpec {
                        line_idx: ql.line_idx,
                        doc_range: range,
                        y,
                        start_x: inner_x + indent,
                        max_w: (inner_w - indent - right_pad).max(60.0),
                        font_px: base_font_px,
                        base_bold: false,
                        reveal_raw: raw_line,
                        base_color: quote_body_color,
                    },
                );
            }

            let box_h = (y - box_top + 4.0).max(min_alert_h);
            if let Some(kind) = alert {
                ctx.bg_cmds.push(DrawCmd::FillRect {
                    rect: RectF::new(content_x, box_top - 2.0 - scroll_y, prose_w, box_h),
                    color: ctx.config.palette.alert_color(kind).with_alpha(20),
                    radius: 4.0,
                });
            }
            ctx.fg_cmds.push(DrawCmd::FillRect {
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
            let code_px = (base_font_px * 0.90).max(10.0);
            let pad = 10.0f32;

            // Interactive top-right language picker badge (`rust ▾` / `Code ▾`)
            let badge_px = (code_px * 0.84).max(10.5);
            let badge_label = if language.is_empty() {
                "Code ▾".to_owned()
            } else {
                format!("{language} ▾")
            };
            let bm =
                ctx.measurer
                    .measure_run(&badge_label, FontRole::SansBody, badge_px, true, false);
            let bw = bm.total_width + 14.0;
            let bh = (bm.metrics.line_height + 4.0).max(20.0);
            let bx = (content_x + prose_w - bw - 8.0).max(content_x + 4.0);
            let by_doc = box_top + 6.0;

            let muted_fg = ctx.config.palette.muted_fg;
            let text_fg = ctx.config.palette.text_fg;

            if reveal_raw {
                y += pad * 0.6;
                y = layout_plain_code_line(
                    ctx,
                    CodeLineSpec {
                        line_idx: block.line_range.start,
                        doc_range: open_line_range.clone(),
                        y,
                        start_x: content_x + pad,
                        code_px,
                        color: muted_fg,
                    },
                );
            } else {
                // Leave clean header clearance so even long first lines never collide with the badge.
                y += bh + 8.0;
            }

            let badge_rect_v = RectF::new(bx, by_doc - scroll_y, bw, bh);
            ctx.fg_cmds.push(DrawCmd::FillRect {
                rect: badge_rect_v,
                color: ctx.config.palette.canvas_bg.with_alpha(220),
                radius: 5.0,
            });
            ctx.fg_cmds.push(DrawCmd::StrokeRect {
                rect: badge_rect_v,
                color: ctx.config.palette.gutter_border,
                stroke_width: 1.0,
                radius: 5.0,
            });
            let badge_base = (by_doc - scroll_y)
                + (bh - (bm.metrics.ascent + bm.metrics.descent)) * 0.5
                + bm.metrics.ascent;
            ctx.fg_cmds.push(DrawCmd::Text {
                x: bx + 7.0,
                baseline_y: badge_base,
                text: badge_label,
                role: FontRole::SansBody,
                size_px: badge_px,
                bold: true,
                italic: false,
                color: if language.is_empty() {
                    muted_fg
                } else {
                    ctx.config.palette.accent
                },
            });
            ctx.code_lang_badges.push(CodeLangHitBox {
                rect_doc: RectF::new(bx, by_doc, bw, bh),
                open_line_start: open_line_range.start,
                language: language.clone(),
            });

            for &(l_idx, ref r) in body_lines {
                y = layout_plain_code_line(
                    ctx,
                    CodeLineSpec {
                        line_idx: l_idx,
                        doc_range: r.clone(),
                        y,
                        start_x: content_x + pad,
                        code_px,
                        color: text_fg,
                    },
                );
            }

            if reveal_raw && let Some(close_r) = close_line_range.clone() {
                let close_idx = block.line_range.end.saturating_sub(1);
                y = layout_plain_code_line(
                    ctx,
                    CodeLineSpec {
                        line_idx: close_idx,
                        doc_range: close_r,
                        y,
                        start_x: content_x + pad,
                        code_px,
                        color: muted_fg,
                    },
                );
            }

            y += pad * 0.7;
            ctx.bg_cmds.push(DrawCmd::FillRect {
                rect: RectF::new(content_x, box_top - scroll_y, prose_w, y - box_top),
                color: ctx.config.palette.code_bg,
                radius: 6.0,
            });
            ctx.bg_cmds.push(DrawCmd::StrokeRect {
                rect: RectF::new(content_x, box_top - scroll_y, prose_w, y - box_top),
                color: ctx.config.palette.gutter_border,
                stroke_width: 1.0,
                radius: 6.0,
            });
            y + 8.0
        }
        BlockKind::Table(ref tbl) => layout_table_2d(ctx, tbl, y, reveal_raw, caret_line),
    }
}

/// Fast `O(visible_lines)` virtualizer for `ViewMode::Source` and `ViewMode::PlainText`.
/// Handles million-line TXT/MD files in `< 0.3 ms`!
fn build_line_based_frame(
    req: &ViewportRequest<'_>,
    geom: FrameGeometry,
    mut draw_cmds: Vec<DrawCmd>,
    measurer: &mut dyn TextMeasurer,
) -> ViewportFrame {
    let FrameGeometry {
        vw,
        vh,
        gutter_w,
        content_x,
        prose_w,
        default_line_h: line_h,
    } = geom;
    let total_lines = req.rope.line_count();
    let pad_top = LINE_MODE_TOP_PAD_Y;
    let role = if req.config.view_mode == ViewMode::Source {
        FontRole::Monospace
    } else {
        FontRole::SansBody
    };

    let first_vis = ((req.scroll_y - pad_top).max(0.0) / line_h).floor() as usize;
    let vis_count = (vh / line_h).ceil() as usize + 6;
    let end_vis = (first_vis.saturating_add(vis_count)).min(total_lines);
    let caret_line = req.rope.byte_to_line_col(req.selection.head).line;

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
        let r = req.rope.line_byte_range(l_idx);
        let capped_end = if r.len() > MAX_SINGLE_LINE_SHAPE_BYTES {
            req.rope
                .clamp_char_boundary(r.start.saturating_add(MAX_SINGLE_LINE_SHAPE_BYTES))
        } else {
            r.end
        };
        let txt = req.rope.slice_to_string(r.start..capped_end);
        let is_heading_src =
            req.config.view_mode == ViewMode::Source && txt.trim_start().starts_with('#');
        let color = if is_heading_src {
            req.config.palette.accent
        } else {
            req.config.palette.text_fg
        };
        let m = measurer.measure_run(&txt, role, req.config.base_font_px, is_heading_src, false);
        let base_off = (line_h - (m.metrics.ascent + m.metrics.descent)) * 0.5 + m.metrics.ascent;

        if y_doc + line_h >= req.scroll_y - 32.0
            && y_doc <= req.scroll_y + vh + 32.0
            && !txt.is_empty()
        {
            fg_cmds.push(DrawCmd::Text {
                x: content_x,
                baseline_y: (y_doc - req.scroll_y) + base_off,
                text: txt,
                role,
                size_px: req.config.base_font_px,
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

    let total_height = (pad_top + (total_lines as f32) * line_h + DOC_BOTTOM_OVERSCROLL_Y).max(vh);
    emit_range_highlights(
        &visual_rows,
        req.search_matches,
        req.active_match_idx,
        req.selection,
        req.scroll_y,
        &req.config.palette,
        &mut bg_cmds,
    );

    let caret_doc_rect =
        compute_caret_rect(&visual_rows, req.selection.head, content_x, pad_top, line_h);
    let caret_viewport_rect = RectF::new(
        caret_doc_rect.x,
        caret_doc_rect.y - req.scroll_y,
        CARET_WIDTH_PX,
        caret_doc_rect.h,
    );

    if let Some(preedit) = req.ime_preedit.filter(|s| !s.is_empty()) {
        let m = measurer.measure_run(preedit, role, req.config.base_font_px, false, false);
        let py = caret_viewport_rect.y;
        fg_cmds.push(DrawCmd::FillRect {
            rect: RectF::new(
                caret_viewport_rect.x,
                py,
                m.total_width + 4.0,
                caret_viewport_rect.h,
            ),
            color: req.config.palette.canvas_bg,
            radius: 2.0,
        });
        fg_cmds.push(DrawCmd::Text {
            x: caret_viewport_rect.x + 1.0,
            baseline_y: py + m.metrics.ascent,
            text: preedit.to_owned(),
            role,
            size_px: req.config.base_font_px,
            bold: false,
            italic: false,
            color: req.config.palette.text_fg,
        });
    } else if req.selection.is_empty() {
        fg_cmds.push(DrawCmd::FillRect {
            rect: caret_viewport_rect,
            color: req.config.palette.caret,
            radius: 0.0,
        });
    }

    draw_cmds.extend(bg_cmds);
    draw_cmds.extend(fg_cmds);
    if gutter_w > 0.0 {
        emit_gutter(
            &visual_rows,
            &geom,
            req.config,
            req.scroll_y,
            &mut draw_cmds,
            measurer,
        );
    }

    ViewportFrame {
        viewport_w: vw,
        viewport_h: vh,
        scroll_y: req.scroll_y,
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
                    return RectF::new(x, row.y_doc, CARET_WIDTH_PX, row.height);
                }
            }
            if row.spans.is_empty()
                && row.doc_range.start == row.doc_range.end
                && caret_byte == row.doc_range.start
            {
                return RectF::new(row.default_x, row.y_doc, CARET_WIDTH_PX, row.height);
            }
        }
    }
    // Second pass: relaxed match on row range when caret sits between spans (e.g. empty heading body).
    for row in visual_rows {
        if caret_byte >= row.doc_range.start && caret_byte <= row.doc_range.end {
            let x = row.spans.last().map_or(row.default_x, |s| s.x + s.width);
            return RectF::new(x, row.y_doc, CARET_WIDTH_PX, row.height);
        }
    }
    RectF::new(default_x, default_y, CARET_WIDTH_PX, default_h)
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

fn emit_gutter(
    visual_rows: &[VisualRow],
    geom: &FrameGeometry,
    config: &LayoutConfig,
    scroll_y: f32,
    draw_cmds: &mut Vec<DrawCmd>,
    measurer: &mut dyn TextMeasurer,
) {
    let gutter_w = geom.gutter_w;
    let vh = geom.vh;
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
