//! Inline Markdown token shaping, word/grapheme wrapping, inline/block images, and code lines.

use kmark_syntax::{ImageSize, InlineTokenKind, StyleFlags, parse_inline_tokens};

use crate::context::{
    CodeLineSpec, DEFAULT_IMAGE_MAX_H, InlineLineSpec, LayoutCtx, MIN_IMAGE_DIM_PX,
};
use crate::engine::{DrawCmd, PositionedGlyphSpan, RectF, VisualRow};
use crate::measurer::{FontMetrics, FontRole, MeasuredRun, TextMeasurer};

pub(crate) fn layout_inline_line(ctx: &mut LayoutCtx<'_>, spec: InlineLineSpec) -> f32 {
    let InlineLineSpec {
        line_idx,
        doc_range,
        mut y,
        start_x,
        max_w,
        font_px,
        base_bold,
        reveal_raw,
        base_color,
    } = spec;

    let raw_text = ctx.rope.slice_to_string(doc_range.clone());
    let base_m = ctx
        .measurer
        .font_metrics(FontRole::SansBody, font_px, base_bold, false);
    let row_h = (font_px * ctx.config.line_spacing_scale).max(base_m.line_height);
    let baseline_off = (row_h - (base_m.ascent + base_m.descent)) * 0.5 + base_m.ascent;

    if raw_text.is_empty() {
        ctx.visual_rows.push(VisualRow {
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
                if let Some((nat_w, nat_h)) = ctx.measurer.image_dimensions(src)
                    && nat_w > 0.0
                    && nat_h > 0.0
                {
                    if !cur_spans.is_empty() {
                        let prev_end = tok.range.start;
                        let taken = std::mem::take(&mut cur_spans);
                        flush_row(y, cur_x, sub_start, prev_end, taken, &mut ctx.visual_rows);
                        y += row_h;
                    }
                    let fit_w = max_w.max(48.0);
                    let (draw_w, draw_h) = match size {
                        Some(ImageSize::Percent(pct)) => {
                            let frac = (f32::from(pct) / 100.0).clamp(0.05, 1.0);
                            let w = (fit_w * frac).max(MIN_IMAGE_DIM_PX);
                            let h = (w * (nat_h / nat_w)).max(MIN_IMAGE_DIM_PX);
                            (w, h)
                        }
                        Some(ImageSize::Pixels(px)) => {
                            let w = f32::from(px).min(fit_w).max(MIN_IMAGE_DIM_PX);
                            let h = (w * (nat_h / nat_w)).max(MIN_IMAGE_DIM_PX);
                            (w, h)
                        }
                        None => {
                            let scale = (fit_w / nat_w).min(DEFAULT_IMAGE_MAX_H / nat_h).min(1.0);
                            (
                                (nat_w * scale).max(MIN_IMAGE_DIM_PX),
                                (nat_h * scale).max(MIN_IMAGE_DIM_PX),
                            )
                        }
                    };

                    let rect_v = RectF::new(start_x, (y - ctx.scroll_y) + 5.0, draw_w, draw_h);
                    ctx.fg_cmds.push(DrawCmd::Image {
                        rect: rect_v,
                        src: src.clone(),
                    });
                    ctx.fg_cmds.push(DrawCmd::StrokeRect {
                        rect: rect_v,
                        color: ctx.config.palette.table_border,
                        stroke_width: 1.0,
                        radius: 4.0,
                    });

                    let mut cap_extra_h = 0.0f32;
                    let mut effective_w = draw_w;
                    if let Some(cap) = caption.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                        let cap_px = (font_px * 0.88).max(11.0);
                        let cap_wrap_w = draw_w.max((fit_w * 0.55).min(fit_w));
                        let (cap_lines, cap_m) =
                            wrap_caption_lines(cap, cap_wrap_w, cap_px, ctx.measurer);
                        let cap_line_h = (cap_px * 1.34).max(cap_m.line_height);
                        let cap_base_off =
                            (cap_line_h - (cap_m.ascent + cap_m.descent)) * 0.5 + cap_m.ascent;
                        let cap_top_vy = (y - ctx.scroll_y) + 5.0 + draw_h + 5.0;
                        for (idx, (line_str, line_w)) in cap_lines.iter().enumerate() {
                            effective_w = effective_w.max(*line_w);
                            let cx = if *line_w <= draw_w {
                                start_x + (draw_w - *line_w) * 0.5
                            } else {
                                start_x
                            };
                            ctx.fg_cmds.push(DrawCmd::Text {
                                x: cx,
                                baseline_y: cap_top_vy + (idx as f32) * cap_line_h + cap_base_off,
                                text: line_str.clone(),
                                role: FontRole::SansBody,
                                size_px: cap_px,
                                bold: false,
                                italic: true,
                                color: ctx.config.palette.muted_fg,
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
                    ctx.visual_rows.push(VisualRow {
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
                let m = ctx.measurer.measure_run(
                    &badge,
                    FontRole::SansBody,
                    font_px * 0.92,
                    false,
                    false,
                );
                let bw = m.total_width + 14.0;
                if ctx.config.word_wrap && cur_x > start_x && (cur_x - start_x) + bw > max_w {
                    let prev_end = tok.range.start;
                    let taken = std::mem::take(&mut cur_spans);
                    flush_row(y, cur_x, sub_start, prev_end, taken, &mut ctx.visual_rows);
                    emitted_any_row = true;
                    y += row_h;
                    cur_x = start_x;
                    sub_start = prev_end;
                }
                let rect_v = RectF::new(cur_x, (y - ctx.scroll_y) + 2.0, bw, row_h - 4.0);
                ctx.bg_cmds.push(DrawCmd::FillRect {
                    rect: rect_v,
                    color: ctx.config.palette.inline_code_bg,
                    radius: 4.0,
                });
                ctx.bg_cmds.push(DrawCmd::StrokeRect {
                    rect: rect_v,
                    color: ctx.config.palette.gutter_border,
                    stroke_width: 1.0,
                    radius: 4.0,
                });
                ctx.fg_cmds.push(DrawCmd::Text {
                    x: cur_x + 7.0,
                    baseline_y: (y - ctx.scroll_y) + baseline_off,
                    text: badge,
                    role: FontRole::SansBody,
                    size_px: font_px * 0.92,
                    bold: false,
                    italic: false,
                    color: ctx.config.palette.link_fg,
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
                    ctx.config.palette.muted_fg
                } else if tok.style.contains(StyleFlags::LINK) {
                    ctx.config.palette.link_fg
                } else {
                    base_color
                };

                // Unified pill path for `<kbd>` and inline `` `code` `` whenever the pill fits on a single line!
                if (is_kbd || is_code) && !is_marker {
                    let outer = if is_kbd { 1.5f32 } else { 1.0f32 };
                    let pad_x = if is_kbd { 6.0f32 } else { 4.5f32 };
                    let m_whole = ctx.measurer.measure_run(slice, role, run_px, bold, italic);
                    let total_pill_w = m_whole.total_width + (pad_x + outer) * 2.0;

                    if !ctx.config.word_wrap || total_pill_w <= max_w {
                        if ctx.config.word_wrap
                            && cur_x > start_x
                            && (cur_x - start_x) + total_pill_w > max_w
                        {
                            let taken = std::mem::take(&mut cur_spans);
                            flush_row(
                                y,
                                cur_x,
                                sub_start,
                                tok.range.start,
                                taken,
                                &mut ctx.visual_rows,
                            );
                            emitted_any_row = true;
                            y += row_h;
                            cur_x = start_x;
                            sub_start = tok.range.start;
                        }

                        let vy = y - ctx.scroll_y;
                        let box_x = cur_x + outer;
                        let box_w = m_whole.total_width + pad_x * 2.0;
                        let pill_rect = RectF::new(box_x, vy + 2.0, box_w, row_h - 4.0);

                        if is_kbd {
                            ctx.bg_cmds.push(DrawCmd::FillRect {
                                rect: pill_rect,
                                color: ctx.config.palette.inline_code_bg,
                                radius: 4.5,
                            });
                            ctx.bg_cmds.push(DrawCmd::StrokeRect {
                                rect: pill_rect,
                                color: ctx.config.palette.table_border,
                                stroke_width: 1.0,
                                radius: 4.5,
                            });
                            // Subtle keycap bottom bevel line
                            ctx.bg_cmds.push(DrawCmd::Line {
                                x1: box_x + 3.0,
                                y1: vy + row_h - 2.2,
                                x2: box_x + box_w - 3.0,
                                y2: vy + row_h - 2.2,
                                color: ctx.config.palette.table_border,
                                width: 1.4,
                            });
                        } else {
                            ctx.bg_cmds.push(DrawCmd::FillRect {
                                rect: pill_rect,
                                color: ctx.config.palette.inline_code_bg,
                                radius: 4.0,
                            });
                        }

                        let text_x = box_x + pad_x;
                        ctx.fg_cmds.push(DrawCmd::Text {
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
                    let m_full = ctx
                        .measurer
                        .measure_run(seg_str, role, run_px, bold, italic);
                    let pad_x = if (is_kbd || is_code) && !is_marker {
                        3.5f32
                    } else {
                        0.0f32
                    };

                    if ctx.config.word_wrap
                        && cur_x > start_x
                        && !seg_str.trim().is_empty()
                        && (cur_x - start_x) + m_full.total_width + pad_x * 2.0 > max_w
                    {
                        let taken = std::mem::take(&mut cur_spans);
                        flush_row(y, cur_x, sub_start, seg_base, taken, &mut ctx.visual_rows);
                        emitted_any_row = true;
                        y += row_h;
                        cur_x = start_x;
                        sub_start = seg_base;
                    }

                    // Grapheme-level fallback wrap if a single unbreakable word/URL exceeds `max_w`!
                    let sub_pieces: Vec<(usize, &str)> = if ctx.config.word_wrap
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
                            flush_row(y, cur_x, sub_start, p_start, taken, &mut ctx.visual_rows);
                            emitted_any_row = true;
                            y += row_h;
                            cur_x = start_x;
                            sub_start = p_start;
                        }
                        let m = ctx.measurer.measure_run(p_str, role, run_px, bold, italic);
                        let vy = y - ctx.scroll_y;
                        let draw_x = cur_x + pad_x;

                        if tok.style.contains(StyleFlags::HIGHLIGHT) {
                            ctx.bg_cmds.push(DrawCmd::FillRect {
                                rect: RectF::new(
                                    cur_x,
                                    vy + 2.0,
                                    m.total_width + pad_x * 2.0,
                                    row_h - 4.0,
                                ),
                                color: ctx.config.palette.highlight_bg,
                                radius: 3.0,
                            });
                        }
                        if is_code && !is_marker {
                            ctx.bg_cmds.push(DrawCmd::FillRect {
                                rect: RectF::new(
                                    cur_x,
                                    vy + 2.0,
                                    m.total_width + pad_x * 2.0,
                                    row_h - 4.0,
                                ),
                                color: ctx.config.palette.inline_code_bg,
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
                            ctx.bg_cmds.push(DrawCmd::FillRect {
                                rect: kr,
                                color: ctx.config.palette.inline_code_bg,
                                radius: 4.0,
                            });
                            ctx.bg_cmds.push(DrawCmd::StrokeRect {
                                rect: kr,
                                color: ctx.config.palette.table_border,
                                stroke_width: 1.0,
                                radius: 4.0,
                            });
                        }

                        ctx.fg_cmds.push(DrawCmd::Text {
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
                            ctx.fg_cmds.push(DrawCmd::Line {
                                x1: draw_x,
                                y1: vy + baseline_off + 2.0,
                                x2: draw_x + m.total_width,
                                y2: vy + baseline_off + 2.0,
                                color: ctx.config.palette.link_fg.with_alpha(180),
                                width: 1.0,
                            });
                        }
                        if tok.style.contains(StyleFlags::STRIKE) && !is_marker {
                            let strike_y = vy + baseline_off - font_px * 0.28;
                            ctx.fg_cmds.push(DrawCmd::Line {
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

    flush_row(
        y,
        cur_x,
        sub_start,
        doc_range.end,
        cur_spans,
        &mut ctx.visual_rows,
    );
    y + row_h
}

pub(crate) fn split_piece_by_width<'a>(
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

pub(crate) fn split_wrap_chunks(s: &str) -> Vec<(usize, &str)> {
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

pub(crate) fn format_image_badge(
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

pub(crate) fn wrap_caption_lines(
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

pub(crate) fn layout_plain_code_line(ctx: &mut LayoutCtx<'_>, spec: CodeLineSpec) -> f32 {
    let CodeLineSpec {
        line_idx,
        doc_range,
        y,
        start_x,
        code_px,
        color,
    } = spec;

    let txt = ctx.rope.slice_to_string(doc_range.clone());
    let m = ctx
        .measurer
        .measure_run(&txt, FontRole::Monospace, code_px, false, false);
    let h = m.metrics.line_height.max(code_px * 1.38);
    let base_off = (h - (m.metrics.ascent + m.metrics.descent)) * 0.5 + m.metrics.ascent;
    if !txt.is_empty() {
        ctx.fg_cmds.push(DrawCmd::Text {
            x: start_x,
            baseline_y: (y - ctx.scroll_y) + base_off,
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
    ctx.visual_rows.push(VisualRow {
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
