//! Lossless, idempotent Markdown document formatter (`Format -> Format Markdown Document`).
//!
//! Enforces canonical block spacing (`MD022` blanks around headings, `MD031` blanks around code
//! fences, `MD032` blanks around lists, `MD058` blanks around tables), normalizes heading/list/quote
//! marker spacing and ordered list numbering, aligns GFM table columns, collapses duplicate blank
//! lines (`MD012`), strips accidental trailing spaces while preserving CommonMark hard line breaks,
//! and leaves fenced code block contents and YAML frontmatter completely untouched.

use kmark_buffer::{Rope, Selection};
use kmark_syntax::{Block, BlockKind, parse_blocks_in_line_range};

use crate::commands::EditableTable;

/// Formats `rope` into clean, canonical CommonMark / GFM Markdown while preserving code blocks,
/// YAML frontmatter, and approximate caret location.
///
/// Returns `(formatted_rope, mapped_selection, changed)`.
#[must_use]
pub fn format_markdown_document(rope: &Rope, selection: Selection) -> (Rope, Selection, bool) {
    let total_lines = rope.line_count();
    if rope.len_bytes() == 0 {
        return (rope.clone(), Selection::collapsed(0), false);
    }

    let frontmatter_end_line = detect_yaml_frontmatter_end(rope, total_lines);
    let body_start_line = frontmatter_end_line.unwrap_or(0);
    let blocks = parse_blocks_in_line_range(rope, body_start_line..total_lines);

    let has_any_content = frontmatter_end_line.is_some()
        || blocks
            .iter()
            .any(|b| !matches!(b.kind, BlockKind::BlankLines));
    if !has_any_content {
        let changed = rope.len_bytes() > 0;
        return (Rope::new(), Selection::collapsed(0), changed);
    }

    let mut out = String::with_capacity(rope.len_bytes().saturating_add(64));
    let mut mapped_caret: Option<usize> = None;
    let caret_head = selection.head;

    if let Some(fm_end) = frontmatter_end_line {
        let fm_start_b = rope.line_start_byte(0);
        let fm_end_b = rope.line_byte_range(fm_end.saturating_sub(1)).end;
        let fm_raw = rope.slice_to_string(fm_start_b..fm_end_b);
        if caret_head <= fm_end_b {
            mapped_caret = Some(caret_head.min(fm_raw.len()));
        }
        out.push_str(&fm_raw);
    }

    let mut prev_was_blank = false;
    let mut prev_list_ordered: Option<bool> = None;
    let mut ordered_counters: Vec<(usize, u32)> = Vec::new();

    for blk in &blocks {
        if matches!(blk.kind, BlockKind::BlankLines) {
            if mapped_caret.is_none()
                && caret_head >= blk.byte_range.start
                && caret_head <= blk.byte_range.end
            {
                mapped_caret = Some(out.len());
            }
            prev_was_blank = true;
            continue;
        }

        let is_list_item = matches!(blk.kind, BlockKind::ListItem { .. });
        if !is_list_item {
            ordered_counters.clear();
            prev_list_ordered = None;
        }

        // Determine separator before this block (`\n` within the same list, `\n\n` between blocks)
        if !out.is_empty() {
            let tight_same_list = if let BlockKind::ListItem {
                indent_spaces,
                ordered,
                ..
            } = blk.kind
            {
                if let Some(prev_ord) = prev_list_ordered {
                    if !prev_was_blank {
                        true
                    } else if indent_spaces == 0 && prev_ord != ordered.is_some() {
                        // Separate top-level bullet list from top-level ordered list across a blank line
                        ordered_counters.clear();
                        false
                    } else {
                        true
                    }
                } else {
                    false
                }
            } else {
                false
            };

            if tight_same_list {
                out.push('\n');
            } else {
                out.push_str("\n\n");
            }
        }

        let out_block_start = out.len();
        format_single_block(rope, blk, &mut ordered_counters, &mut out);
        let out_block_len = out.len().saturating_sub(out_block_start);

        if mapped_caret.is_none()
            && caret_head >= blk.byte_range.start
            && caret_head <= blk.byte_range.end
        {
            let rel = caret_head.saturating_sub(blk.byte_range.start);
            mapped_caret = Some(out_block_start.saturating_add(rel.min(out_block_len)));
        }

        if let BlockKind::ListItem { ordered, .. } = blk.kind {
            prev_list_ordered = Some(ordered.is_some());
        }
        prev_was_blank = false;
    }

    if !out.ends_with('\n') {
        out.push('\n');
    }

    let next_rope = Rope::from_str(&out);
    let changed = &next_rope != rope;
    if !changed {
        return (rope.clone(), selection, false);
    }

    let target_byte = mapped_caret.unwrap_or_else(|| next_rope.len_bytes().saturating_sub(1));
    let clamped = next_rope.clamp_char_boundary(target_byte.min(next_rope.len_bytes()));
    (next_rope, Selection::collapsed(clamped), true)
}

/// Detects optional YAML frontmatter (`---\nkey: val\n---`) starting at line 0.
/// Returns the exclusive end line index (`close_line + 1`) when valid frontmatter is present.
fn detect_yaml_frontmatter_end(rope: &Rope, total_lines: usize) -> Option<usize> {
    if total_lines < 3 {
        return None;
    }
    if rope.line_text(0).trim_end_matches([' ', '\t']) != "---" {
        return None;
    }
    let second = rope.line_text(1);
    let second_trimmed = second.trim_start();
    if second_trimmed.is_empty() || (!second.contains(':') && !second_trimmed.starts_with('#')) {
        return None;
    }
    let scan_limit = total_lines.min(200);
    for idx in 2..scan_limit {
        let t = rope.line_text(idx);
        let trimmed_end = t.trim_end_matches([' ', '\t']);
        if trimmed_end == "---" || trimmed_end == "..." {
            return Some(idx.saturating_add(1));
        }
    }
    None
}

fn format_single_block(
    rope: &Rope,
    blk: &Block,
    ordered_counters: &mut Vec<(usize, u32)>,
    out: &mut String,
) {
    match blk.kind {
        BlockKind::BlankLines => {}
        BlockKind::ThematicBreak => {
            out.push_str("---");
        }
        BlockKind::Heading {
            level,
            ref content_range,
            ..
        } => {
            let hashes = "#".repeat(usize::from(level.clamp(1, 6)));
            let raw_title = rope.slice_to_string(content_range.clone());
            let trimmed = raw_title.trim();
            out.push_str(&hashes);
            if !trimmed.is_empty() {
                out.push(' ');
                out.push_str(trimmed);
            }
        }
        BlockKind::Paragraph { ref lines } => {
            let last_idx = lines.len().saturating_sub(1);
            for (idx, (_, r)) in lines.iter().enumerate() {
                if idx > 0 {
                    out.push('\n');
                }
                let raw = rope.slice_to_string(r.clone());
                out.push_str(&clean_prose_line(&raw, idx == last_idx));
            }
        }
        BlockKind::ListItem {
            indent_spaces,
            ordered,
            ref marker_range,
            ref task,
            ref content_range,
            ref continuation_lines,
            ..
        } => {
            while ordered_counters
                .last()
                .is_some_and(|&(ind, _)| ind > indent_spaces)
            {
                ordered_counters.pop();
            }

            let indent_str = " ".repeat(indent_spaces);
            let raw_marker = rope.slice_to_string(marker_range.clone());
            let trimmed_marker = raw_marker.trim();

            let marker_prefix = if ordered.is_some() {
                let sep = if trimmed_marker.ends_with(')') {
                    ')'
                } else {
                    '.'
                };
                let num = if let Some(&mut (ind, ref mut next_n)) = ordered_counters.last_mut()
                    && ind == indent_spaces
                {
                    let current = *next_n;
                    *next_n = next_n.saturating_add(1);
                    current
                } else {
                    ordered_counters.push((indent_spaces, 2));
                    1
                };
                format!("{indent_str}{num}{sep} ")
            } else {
                if ordered_counters
                    .last()
                    .is_some_and(|&(ind, _)| ind == indent_spaces)
                {
                    ordered_counters.pop();
                }
                let bullet = match trimmed_marker.as_bytes().first().copied() {
                    Some(b'*') => '*',
                    Some(b'+') => '+',
                    _ => '-',
                };
                format!("{indent_str}{bullet} ")
            };

            let task_prefix = match task {
                Some(ts) if ts.checked => "[x] ",
                Some(_) => "[ ] ",
                None => "",
            };

            let full_prefix = format!("{marker_prefix}{task_prefix}");
            let cont_indent_cols = full_prefix.len().max(2);
            let has_cont = !continuation_lines.is_empty();

            out.push_str(&full_prefix);
            let first_body = rope.slice_to_string(content_range.clone());
            out.push_str(&clean_prose_line(&first_body, !has_cont));

            let last_cont = continuation_lines.len().saturating_sub(1);
            let cont_pad = " ".repeat(cont_indent_cols);
            for (c_idx, (_, c_range)) in continuation_lines.iter().enumerate() {
                out.push('\n');
                out.push_str(&cont_pad);
                let c_raw = rope.slice_to_string(c_range.clone());
                out.push_str(&clean_prose_line(&c_raw, c_idx == last_cont));
            }
        }
        BlockKind::BlockQuote {
            alert,
            ref custom_title,
            ref lines,
        } => {
            let last_idx = lines.len().saturating_sub(1);
            for (idx, ql) in lines.iter().enumerate() {
                if idx > 0 {
                    out.push('\n');
                }
                let chevrons = ">".repeat(usize::from(ql.depth.max(1)));
                if idx == 0
                    && let Some(kind) = alert
                {
                    out.push_str(&chevrons);
                    out.push_str(" [!");
                    out.push_str(kind.tag_str());
                    out.push(']');
                    if let Some(title) = custom_title.as_deref().map(str::trim)
                        && !title.is_empty()
                    {
                        out.push(' ');
                        out.push_str(title);
                    }
                    continue;
                }

                let raw = rope.slice_to_string(ql.content_range.clone());
                let cleaned = clean_prose_line(&raw, idx == last_idx);
                out.push_str(&chevrons);
                if !cleaned.is_empty() {
                    out.push(' ');
                    out.push_str(&cleaned);
                }
            }
        }
        BlockKind::CodeFence {
            fence_char,
            fence_len,
            ref language,
            ref body_lines,
            ..
        } => {
            let ch = char::from(if fence_char == b'~' { b'~' } else { b'`' });
            let fence: String = std::iter::repeat_n(ch, fence_len.max(3)).collect();
            out.push_str(&fence);
            let lang = language.trim();
            if !lang.is_empty() {
                out.push_str(lang);
            }
            for (_, r) in body_lines {
                out.push('\n');
                // Preserve code fence body lines 100% verbatim!
                out.push_str(&rope.slice_to_string(r.clone()));
            }
            out.push('\n');
            out.push_str(&fence);
        }
        BlockKind::Table(ref table) => {
            let editable = EditableTable::from_ast(rope, table);
            out.push_str(&editable.format_markdown());
        }
    }
}

/// Strips accidental trailing spaces/tabs while preserving CommonMark hard line breaks (`  ` or `\`)
/// on non-final lines of a multi-line paragraph, list item, or blockquote.
fn clean_prose_line(raw: &str, is_last_in_block: bool) -> String {
    if is_last_in_block {
        return raw.trim_end_matches([' ', '\t']).to_owned();
    }
    if raw.ends_with('\\') {
        return raw.to_owned();
    }
    let trimmed = raw.trim_end_matches([' ', '\t']);
    let trailing_spaces = raw
        .get(trimmed.len()..)
        .map_or(0, |tail| tail.bytes().filter(|&b| b == b' ').count());
    if trailing_spaces >= 2 {
        format!("{trimmed}  ")
    } else {
        trimmed.to_owned()
    }
}
