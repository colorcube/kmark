//! Shared layout constants, viewport request bundle, and per-frame `LayoutCtx`.

use std::ops::Range;

use kmark_buffer::{Rope, Selection};
use kmark_syntax::Block;

use crate::engine::{DrawCmd, LayoutConfig, VisualRow};
use crate::hit_test::{CalloutBadgeHitBox, CheckboxHitBox, CodeLangHitBox};
use crate::measurer::TextMeasurer;
use crate::theme::Rgba;

/// Relative font-size multipliers for `H1`..=`H6` (index `0` = body text).
pub(crate) const HEADING_SCALES: [f32; 7] = [1.0, 1.90, 1.55, 1.30, 1.15, 1.05, 1.0];

/// Upper bound on cached `(BlockContentKey -> height)` entries before eviction reset.
pub(crate) const MAX_CACHED_BLOCK_HEIGHTS: usize = 16_384;

/// Minimum clamped viewport dimensions (px) so tiny/uninitialized widgets never divide by zero.
pub(crate) const MIN_VIEWPORT_W: f32 = 240.0;
pub(crate) const MIN_VIEWPORT_H: f32 = 120.0;

/// Horizontal canvas margin (px) around the prose column.
pub(crate) const CANVAS_PAD_X: f32 = 28.0;
/// Minimum clamped prose column width (px).
pub(crate) const MIN_PROSE_W: f32 = 140.0;

/// Top document padding (px) in formatted (`WYSIWYG` / `LivePreview`) and line-based modes.
pub(crate) const FORMATTED_TOP_PAD_Y: f32 = 20.0;
pub(crate) const LINE_MODE_TOP_PAD_Y: f32 = 16.0;
/// Bottom overscroll breathing room (px) below the final block/line.
pub(crate) const DOC_BOTTOM_OVERSCROLL_Y: f32 = 64.0;

/// Off-screen virtualization overscan window (`vh * OVERSCAN_VIEWPORT_FRAC + OVERSCAN_EXTRA_PX`).
pub(crate) const OVERSCAN_VIEWPORT_FRAC: f32 = 0.75;
pub(crate) const OVERSCAN_EXTRA_PX: f32 = 240.0;

/// Maximum byte slice shaped per single line in `Source` / `PlainText` mode so multi-MB single-line
/// logs never stall frame generation.
pub(crate) const MAX_SINGLE_LINE_SHAPE_BYTES: usize = 4_096;

/// Caret bar width in logical pixels.
pub(crate) const CARET_WIDTH_PX: f32 = 2.0;

/// Table layout cell padding and minimum column constraints (px).
pub(crate) const TABLE_CELL_PAD_X: f32 = 12.0;
pub(crate) const TABLE_CELL_PAD_Y: f32 = 6.0;
pub(crate) const TABLE_MIN_COL_W: f32 = 64.0;
pub(crate) const TABLE_MAX_MIN_COL_FRAC: f32 = 0.42;

/// Default maximum height (px) for unsized inline images.
pub(crate) const DEFAULT_IMAGE_MAX_H: f32 = 380.0;
pub(crate) const MIN_IMAGE_DIM_PX: f32 = 24.0;

/// Input parameters for building a virtualized [`crate::engine::ViewportFrame`].
pub struct ViewportRequest<'a> {
    pub rope: &'a Rope,
    pub blocks: &'a [Block],
    pub selection: Selection,
    pub ime_preedit: Option<&'a str>,
    pub search_matches: &'a [Range<usize>],
    pub active_match_idx: Option<usize>,
    pub config: &'a LayoutConfig,
    pub viewport_w: f32,
    pub viewport_h: f32,
    pub scroll_y: f32,
}

/// Resolved canvas & column geometry for a single viewport frame.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FrameGeometry {
    pub vw: f32,
    pub vh: f32,
    pub gutter_w: f32,
    pub content_x: f32,
    pub prose_w: f32,
    pub default_line_h: f32,
}

/// Mutable per-frame layout accumulator passed through block, inline, and table layout passes.
pub(crate) struct LayoutCtx<'a> {
    pub rope: &'a Rope,
    pub config: &'a LayoutConfig,
    pub content_x: f32,
    pub prose_w: f32,
    pub default_line_h: f32,
    pub scroll_y: f32,
    pub bg_cmds: Vec<DrawCmd>,
    pub fg_cmds: Vec<DrawCmd>,
    pub visual_rows: Vec<VisualRow>,
    pub checkboxes: Vec<CheckboxHitBox>,
    pub code_lang_badges: Vec<CodeLangHitBox>,
    pub callout_badges: Vec<CalloutBadgeHitBox>,
    pub measurer: &'a mut dyn TextMeasurer,
}

/// Parameters for shaping and wrapping one Markdown inline line or table cell.
pub(crate) struct InlineLineSpec {
    pub line_idx: usize,
    pub doc_range: Range<usize>,
    pub y: f32,
    pub start_x: f32,
    pub max_w: f32,
    pub font_px: f32,
    pub base_bold: bool,
    pub reveal_raw: bool,
    pub base_color: Rgba,
}

/// Parameters for shaping a single monospace code/raw line without inline Markdown styling.
pub(crate) struct CodeLineSpec {
    pub line_idx: usize,
    pub doc_range: Range<usize>,
    pub y: f32,
    pub start_x: f32,
    pub code_px: f32,
    pub color: Rgba,
}
