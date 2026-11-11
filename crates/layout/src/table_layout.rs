//! First-class 2D GFM table column width solver, cell wrapping, alignment shifting, and grid borders.

use kmark_syntax::{
    ColumnAlign, ImageSize, InlineTokenKind, StyleFlags, TableBlock, TableRow, parse_inline_tokens,
};

use crate::context::{
    CodeLineSpec, InlineLineSpec, LayoutCtx, MIN_IMAGE_DIM_PX, TABLE_CELL_PAD_X, TABLE_CELL_PAD_Y,
    TABLE_MAX_MIN_COL_FRAC, TABLE_MIN_COL_W,
};
use crate::engine::{DrawCmd, RectF, ViewMode};
use crate::inline_layout::{format_image_badge, layout_inline_line, layout_plain_code_line};
use crate::measurer::{FontRole, TextMeasurer};

struct TableGridSpec<'a> {
    tbl: &'a TableBlock,
    col_widths: &'a [f32],
    table_w: f32,
    caret_line: usize,
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
                            (220.0 * (f32::from(pct) / 100.0)).max(MIN_IMAGE_DIM_PX)
                        }
                        Some(ImageSize::Pixels(px)) => f32::from(px).clamp(MIN_IMAGE_DIM_PX, 360.0),
                        None => {
                            let scale = (220.0 / iw).min(140.0 / ih).min(1.0);
                            (iw * scale).max(MIN_IMAGE_DIM_PX)
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

fn render_table_row(
    ctx: &mut LayoutCtx<'_>,
    grid: &TableGridSpec<'_>,
    row: &TableRow,
    is_header: bool,
    row_top: f32,
) -> f32 {
    let raw_cell = ctx.config.view_mode == ViewMode::LivePreview && row.line_idx == grid.caret_line;
    let mut col_x = ctx.content_x;
    let mut max_cell_bottom =
        row_top + ctx.config.base_font_px * ctx.config.line_spacing_scale + TABLE_CELL_PAD_Y * 2.0;

    for (c, &cw) in grid.col_widths.iter().enumerate() {
        let align = grid.tbl.alignments.get(c).copied().unwrap_or_default();
        let inner_w = (cw - TABLE_CELL_PAD_X * 2.0).max(24.0);
        if let Some(cell) = row.cells.get(c) {
            let first_sub_idx = ctx.visual_rows.len();
            let bg_start = ctx.bg_cmds.len();
            let fg_start = ctx.fg_cmds.len();
            let cell_end_y = layout_inline_line(
                ctx,
                InlineLineSpec {
                    line_idx: row.line_idx,
                    doc_range: cell.trimmed_range.clone(),
                    y: row_top + TABLE_CELL_PAD_Y,
                    start_x: col_x + TABLE_CELL_PAD_X,
                    max_w: inner_w,
                    font_px: ctx.config.base_font_px,
                    base_bold: is_header,
                    reveal_raw: raw_cell,
                    base_color: ctx.config.palette.text_fg,
                },
            );
            // Apply column alignment (`Center` / `Right`) synchronously to spans, bg pills (<kbd>/code), and fg runs!
            if align != ColumnAlign::Left {
                let mut bands: Vec<(f32, f32, f32)> = Vec::new();
                for vrow in ctx.visual_rows.iter_mut().skip(first_sub_idx) {
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
                    let vy_top = vrow.y_doc - ctx.scroll_y;
                    bands.push((vy_top, vy_top + vrow.height, shift));
                }
                if let Some(slice) = ctx.bg_cmds.get_mut(bg_start..) {
                    shift_draw_cmds_in_bands(slice, &bands);
                }
                if let Some(slice) = ctx.fg_cmds.get_mut(fg_start..) {
                    shift_draw_cmds_in_bands(slice, &bands);
                }
            }
            max_cell_bottom = max_cell_bottom.max(cell_end_y + TABLE_CELL_PAD_Y);
        }
        col_x += cw;
    }

    let row_h = max_cell_bottom - row_top;
    if is_header {
        ctx.bg_cmds.insert(
            0,
            DrawCmd::FillRect {
                rect: RectF::new(ctx.content_x, row_top - ctx.scroll_y, grid.table_w, row_h),
                color: ctx.config.palette.table_header_bg,
                radius: 0.0,
            },
        );
    }
    // Horizontal row divider
    ctx.fg_cmds.push(DrawCmd::Line {
        x1: ctx.content_x,
        y1: max_cell_bottom - ctx.scroll_y,
        x2: ctx.content_x + grid.table_w,
        y2: max_cell_bottom - ctx.scroll_y,
        color: ctx.config.palette.table_border,
        width: if is_header { 1.5 } else { 1.0 },
    });
    max_cell_bottom
}

/// First-class 2D Table Grid Layout directly over `Rope` cell byte slices!
pub(crate) fn layout_table_2d(
    ctx: &mut LayoutCtx<'_>,
    tbl: &TableBlock,
    mut y: f32,
    reveal_raw: bool,
    caret_line: usize,
) -> f32 {
    if reveal_raw && caret_line == tbl.delimiter_line_idx {
        // Only if the user specifically placed the caret on the `|---|---|` delimiter row in Live mode,
        // show raw table lines so the delimiter itself can be edited directly.
        let mut cur_y = y + 4.0;
        let code_px = ctx.config.base_font_px * 0.92;
        let start_x = ctx.content_x;
        let text_fg = ctx.config.palette.text_fg;
        let muted_fg = ctx.config.palette.muted_fg;
        for (_, row) in tbl.all_data_rows() {
            if row.line_idx == tbl.header_row.line_idx {
                cur_y = layout_plain_code_line(
                    ctx,
                    CodeLineSpec {
                        line_idx: row.line_idx,
                        doc_range: row.byte_range.clone(),
                        y: cur_y,
                        start_x,
                        code_px,
                        color: text_fg,
                    },
                );
                cur_y = layout_plain_code_line(
                    ctx,
                    CodeLineSpec {
                        line_idx: tbl.delimiter_line_idx,
                        doc_range: tbl.delimiter_range.clone(),
                        y: cur_y,
                        start_x,
                        code_px,
                        color: muted_fg,
                    },
                );
            } else {
                cur_y = layout_plain_code_line(
                    ctx,
                    CodeLineSpec {
                        line_idx: row.line_idx,
                        doc_range: row.byte_range.clone(),
                        y: cur_y,
                        start_x,
                        code_px,
                        color: text_fg,
                    },
                );
            }
        }
        return cur_y + 6.0;
    }

    let cols = tbl.col_count.max(1);
    let prose_w = ctx.prose_w;
    // Cap per-column `min-content` so one giant unbreakable URL cannot starve sibling columns.
    let max_min_col_w = (prose_w * TABLE_MAX_MIN_COL_FRAC).max(TABLE_MIN_COL_W);

    let mut min_w = vec![TABLE_MIN_COL_W; cols];
    let mut natural_w = vec![TABLE_MIN_COL_W; cols];
    for (row_idx, row) in tbl.all_data_rows() {
        let is_header = row_idx == 0;
        let raw_cell = ctx.config.view_mode == ViewMode::LivePreview && row.line_idx == caret_line;
        for (c, cell) in row.cells.iter().take(cols).enumerate() {
            let txt = ctx.rope.slice_to_string(cell.trimmed_range.clone());
            let (cell_min_w, cell_nat_w) = measure_inline_cell_widths(
                &txt,
                ctx.config.base_font_px,
                is_header,
                raw_cell,
                ctx.measurer,
            );
            let padded_min = (cell_min_w.ceil() + TABLE_CELL_PAD_X * 2.0 + 8.0).min(max_min_col_w);
            let padded_nat = cell_nat_w.ceil() + TABLE_CELL_PAD_X * 2.0 + 8.0;
            if let Some(mw) = min_w.get_mut(c) {
                *mw = mw.max(padded_min);
            }
            if let Some(nw) = natural_w.get_mut(c) {
                *nw = nw.max(padded_nat);
            }
        }
    }
    for (nw, &mw) in natural_w.iter_mut().zip(&min_w) {
        *nw = nw.max(mw);
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
            for ((w_slot, &nat), &wt) in widths.iter_mut().zip(&natural_w).zip(&weights) {
                if wt > 0.0 {
                    let share = remaining * (wt / weight_sum);
                    let add = share.min(nat - *w_slot);
                    *w_slot += add;
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
    let grid = TableGridSpec {
        tbl,
        col_widths: &col_widths,
        table_w,
        caret_line,
    };

    y = render_table_row(ctx, &grid, &tbl.header_row, true, y);
    for row in &tbl.body_rows {
        y = render_table_row(ctx, &grid, row, false, y);
    }

    // Vertical column dividers and outer border
    let mut vx = ctx.content_x;
    for &cw in col_widths.iter().take(cols.saturating_sub(1)) {
        vx += cw;
        ctx.fg_cmds.push(DrawCmd::Line {
            x1: vx,
            y1: table_top - ctx.scroll_y,
            x2: vx,
            y2: y - ctx.scroll_y,
            color: ctx.config.palette.table_border,
            width: 1.0,
        });
    }
    ctx.fg_cmds.push(DrawCmd::StrokeRect {
        rect: RectF::new(
            ctx.content_x,
            table_top - ctx.scroll_y,
            table_w,
            y - table_top,
        ),
        color: ctx.config.palette.table_border,
        stroke_width: 1.0,
        radius: 3.0,
    });

    y + 8.0
}
