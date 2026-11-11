use kmark_buffer::{EditGroup, Rope, Selection};
use kmark_layout::{ViewMode, ViewportFrame};
use kmark_syntax::{Block, BlockKind, InlineTokenKind, parse_blocks, parse_inline_tokens};

use crate::commands::{
    ArmedStyle, TableOp, TableRowLoc, apply_table_op, locate_table_cursor, strip_ordered_marker,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyCode {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Backspace,
    Delete,
    Enter,
    Tab,
    Backtab,
    Escape,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyModifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// Inserts committed text (`ch` or pasted text), honoring active `armed_styles`.
#[must_use]
pub fn handle_text_insert(
    rope: &Rope,
    selection: Selection,
    armed_styles: &mut Vec<ArmedStyle>,
    text: &str,
) -> Option<(Rope, Selection, EditGroup)> {
    if text.is_empty() {
        return None;
    }

    if !armed_styles.is_empty() && !text.contains('\n') {
        let mut open = String::new();
        let mut close = String::new();
        for style in armed_styles.iter() {
            open.push_str(style.delimiter());
        }
        for style in armed_styles.iter().rev() {
            close.push_str(style.delimiter());
        }
        armed_styles.clear();

        let range = selection.range();
        let wrapped = format!("{open}{text}{close}");
        let mut next = rope.clone();
        next.replace_range(range.clone(), &wrapped);
        let caret = range
            .start
            .saturating_add(open.len())
            .saturating_add(text.len());
        return Some((next, Selection::collapsed(caret), EditGroup::Discrete));
    }

    armed_styles.clear();
    let range = selection.range();
    let mut next = rope.clone();
    next.replace_range(range.clone(), text);
    let caret = range.start.saturating_add(text.len());
    let group = if selection.is_empty() && text.chars().count() <= 2 && !text.contains('\n') {
        EditGroup::Typing
    } else {
        EditGroup::Discrete
    };
    Some((next, Selection::collapsed(caret), group))
}

/// Smart `Enter`: table row step, list/task/quote continuation (and exit on empty item!), or newline.
#[must_use]
pub fn handle_enter(
    rope: &Rope,
    blocks: &[Block],
    selection: Selection,
    modifiers: KeyModifiers,
    view_mode: ViewMode,
) -> (Rope, Selection, EditGroup) {
    if !matches!(view_mode, ViewMode::PlainText) {
        // 1. Inside a 2D TableBlock
        if let Some((_, table, cursor)) = locate_table_cursor(blocks, selection.head) {
            if modifiers.shift {
                let range = selection.range();
                let mut next = rope.clone();
                next.replace_range(range.clone(), "<br>");
                return (
                    next,
                    Selection::collapsed(range.start.saturating_add(4)),
                    EditGroup::Discrete,
                );
            }
            // Move down one cell in the same column, or append a new row if on the last row!
            let col = cursor.col.min(table.col_count.saturating_sub(1));
            match cursor.row {
                TableRowLoc::Header | TableRowLoc::Delimiter => {
                    if let Some(first_body) = table.body_rows.first()
                        && let Some(cell) = first_body.cells.get(col)
                    {
                        return (
                            rope.clone(),
                            Selection::new(cell.trimmed_range.start, cell.trimmed_range.end),
                            EditGroup::Discrete,
                        );
                    }
                }
                TableRowLoc::Body(b_idx) => {
                    if let Some(next_row) = table.body_rows.get(b_idx.saturating_add(1))
                        && let Some(cell) = next_row.cells.get(col)
                    {
                        return (
                            rope.clone(),
                            Selection::new(cell.trimmed_range.start, cell.trimmed_range.end),
                            EditGroup::Discrete,
                        );
                    }
                }
            }
            if let Some((next_rope, next_sel)) =
                apply_table_op(rope, blocks, selection, TableOp::InsertRowBelow)
            {
                return (next_rope, next_sel, EditGroup::Discrete);
            }
        }

        // 2. Smart list & blockquote continuation on collapsed selection
        if selection.is_empty() && !modifiers.shift {
            let lc = rope.byte_to_line_col(selection.head);
            let line_range = rope.line_byte_range(lc.line);
            let line = rope.slice_to_string(line_range.clone());

            if let Some((prefix_len, next_prefix, is_empty_item)) = analyze_continuation_line(&line)
            {
                if is_empty_item {
                    // Pressing Enter on an empty `- ` / `1. ` / `> ` exits the list/quote cleanly!
                    let mut next = rope.clone();
                    next.replace_range(
                        line_range.start..line_range.start.saturating_add(prefix_len),
                        "",
                    );
                    return (
                        next,
                        Selection::collapsed(line_range.start),
                        EditGroup::Discrete,
                    );
                }

                // Only continue if caret is at or after the marker prefix
                if selection.head >= line_range.start.saturating_add(prefix_len) {
                    let inserted = format!("\n{next_prefix}");
                    let mut next = rope.clone();
                    next.replace_range(selection.head..selection.head, &inserted);
                    return (
                        next,
                        Selection::collapsed(selection.head.saturating_add(inserted.len())),
                        EditGroup::Discrete,
                    );
                }
            }
        }
    }

    // Standard newline preserving leading spaces in code fences / source mode
    let range = selection.range();
    let lc = rope.byte_to_line_col(range.start);
    let line_range = rope.line_byte_range(lc.line);
    let line_prefix = rope.slice_to_string(line_range.start..range.start);
    let leading_ws: String =
        if matches!(view_mode, ViewMode::Source) || is_in_code_fence(blocks, range.start) {
            line_prefix
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .collect()
        } else {
            String::new()
        };

    let inserted = format!("\n{leading_ws}");
    let mut next = rope.clone();
    next.replace_range(range.clone(), &inserted);
    (
        next,
        Selection::collapsed(range.start.saturating_add(inserted.len())),
        EditGroup::Discrete,
    )
}

fn is_in_code_fence(blocks: &[Block], byte_offset: usize) -> bool {
    blocks.iter().any(|b| {
        byte_offset >= b.byte_range.start
            && byte_offset <= b.byte_range.end
            && matches!(b.kind, BlockKind::CodeFence { .. })
    })
}

/// Returns `(current_prefix_byte_len, next_line_prefix, is_item_body_empty)`.
fn analyze_continuation_line(line: &str) -> Option<(usize, String, bool)> {
    let indent_bytes = line.bytes().take_while(|b| *b == b' ').count();
    let indent = line.get(..indent_bytes)?;
    let rest = line.get(indent_bytes..)?;

    // Task list
    for marker in ["- [ ] ", "- [x] ", "- [X] ", "* [ ] ", "* [x] "] {
        if let Some(body) = rest.strip_prefix(marker) {
            let bullet = marker.get(..1).unwrap_or("-");
            let cur_len = indent_bytes.saturating_add(marker.len());
            let next_pfx = format!("{indent}{bullet} [ ] ");
            return Some((cur_len, next_pfx, body.trim().is_empty()));
        }
    }

    // Unordered list
    for marker in ["- ", "* ", "+ "] {
        if let Some(body) = rest.strip_prefix(marker) {
            let cur_len = indent_bytes.saturating_add(marker.len());
            let next_pfx = format!("{indent}{marker}");
            return Some((cur_len, next_pfx, body.trim().is_empty()));
        }
    }

    // Ordered list (`1. ` or `1) `)
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0
        && digits <= 9
        && let Some(num_str) = rest.get(..digits)
        && let Ok(num) = num_str.parse::<u64>()
        && let Some(after_digits) = rest.get(digits..)
    {
        for sep in [". ", ") "] {
            if let Some(body) = after_digits.strip_prefix(sep) {
                let cur_len = indent_bytes
                    .saturating_add(digits)
                    .saturating_add(sep.len());
                let next_pfx = format!("{indent}{}{sep}", num.saturating_add(1));
                return Some((cur_len, next_pfx, body.trim().is_empty()));
            }
        }
    }

    // Blockquote (`> ` or `>> `)
    if rest.starts_with('>') {
        let mut q_bytes = 0usize;
        let bytes = rest.as_bytes();
        while q_bytes < bytes.len() && (bytes[q_bytes] == b'>' || bytes[q_bytes] == b' ') {
            q_bytes = q_bytes.saturating_add(1);
        }
        let marker = rest.get(..q_bytes)?;
        let body = rest.get(q_bytes..)?;
        if !body.starts_with("[!") {
            return Some((
                indent_bytes.saturating_add(q_bytes),
                format!("{indent}{marker}"),
                body.trim().is_empty(),
            ));
        }
    }

    None
}

/// Smart `Tab` / `Shift+Tab`: navigates table cells (creating a new row at table end!) or indents lists/lines.
#[must_use]
pub fn handle_tab(
    rope: &Rope,
    blocks: &[Block],
    selection: Selection,
    backwards: bool,
) -> (Rope, Selection, Option<EditGroup>) {
    // 1. Inside a TableBlock -> jump between cells!
    if let Some((_, table, cursor)) = locate_table_cursor(blocks, selection.head) {
        let total_rows = table.body_rows.len().saturating_add(1);
        let cur_r = cursor.data_row_index();
        let cur_c = cursor.col.min(table.col_count.saturating_sub(1));

        if !backwards {
            let (next_r, next_c) = if cur_c.saturating_add(1) < table.col_count {
                (cur_r, cur_c.saturating_add(1))
            } else {
                (cur_r.saturating_add(1), 0)
            };

            if next_r < total_rows {
                let row_ref = if next_r == 0 {
                    Some(&table.header_row)
                } else {
                    table.body_rows.get(next_r.saturating_sub(1))
                };
                if let Some(r) = row_ref
                    && let Some(cell) = r.cells.get(next_c)
                {
                    return (
                        rope.clone(),
                        Selection::new(cell.trimmed_range.start, cell.trimmed_range.end),
                        None,
                    );
                }
            } else if let Some((new_rope, new_sel)) =
                apply_table_op(rope, blocks, selection, TableOp::InsertRowBelow)
            {
                // Move caret to column 0 of the newly appended row
                let reparsed = parse_blocks(&new_rope);
                if let Some((_, new_tbl, _)) = locate_table_cursor(&reparsed, new_sel.head)
                    && let Some(last_row) = new_tbl.body_rows.last()
                    && let Some(first_cell) = last_row.cells.first()
                {
                    return (
                        new_rope,
                        Selection::collapsed(first_cell.trimmed_range.start),
                        Some(EditGroup::Discrete),
                    );
                }
                return (new_rope, new_sel, Some(EditGroup::Discrete));
            }
        } else {
            let (prev_r, prev_c) = if cur_c > 0 {
                (cur_r, cur_c.saturating_sub(1))
            } else if cur_r > 0 {
                (cur_r.saturating_sub(1), table.col_count.saturating_sub(1))
            } else {
                (0, 0)
            };
            let row_ref = if prev_r == 0 {
                Some(&table.header_row)
            } else {
                table.body_rows.get(prev_r.saturating_sub(1))
            };
            if let Some(r) = row_ref
                && let Some(cell) = r.cells.get(prev_c)
            {
                return (
                    rope.clone(),
                    Selection::new(cell.trimmed_range.start, cell.trimmed_range.end),
                    None,
                );
            }
        }
    }

    // 2. List item indent / outdent (2 spaces)
    let lc = rope.byte_to_line_col(selection.head);
    let line_range = rope.line_byte_range(lc.line);
    let line_text = rope.slice_to_string(line_range.clone());
    let trimmed = line_text.trim_start();
    let is_list_line = trimmed.starts_with("- ")
        || trimmed.starts_with("* ")
        || trimmed.starts_with("+ ")
        || strip_ordered_marker(trimmed).is_some();

    if is_list_line {
        let mut next = rope.clone();
        if !backwards {
            next.replace_range(line_range.start..line_range.start, "  ");
            return (
                next,
                Selection::collapsed(selection.head.saturating_add(2)),
                Some(EditGroup::Discrete),
            );
        }
        let removable = line_text.bytes().take(2).take_while(|b| *b == b' ').count();
        if removable > 0 {
            next.replace_range(
                line_range.start..line_range.start.saturating_add(removable),
                "",
            );
            return (
                next,
                Selection::collapsed(selection.head.saturating_sub(removable)),
                Some(EditGroup::Discrete),
            );
        }
        return (rope.clone(), selection, None);
    }

    // 3. Regular indentation (`4` spaces)
    let mut next = rope.clone();
    if !backwards {
        let range = selection.range();
        next.replace_range(range.clone(), "    ");
        (
            next,
            Selection::collapsed(range.start.saturating_add(4)),
            Some(EditGroup::Discrete),
        )
    } else {
        let removable = line_text.bytes().take(4).take_while(|b| *b == b' ').count();
        if removable > 0 {
            next.replace_range(
                line_range.start..line_range.start.saturating_add(removable),
                "",
            );
            (
                next,
                Selection::collapsed(
                    selection
                        .head
                        .saturating_sub(removable)
                        .max(line_range.start),
                ),
                Some(EditGroup::Discrete),
            )
        } else {
            (rope.clone(), selection, None)
        }
    }
}

/// Horizontal caret step (`Left` / `Right`), skipping hidden `SyntaxMarker` runs in `ViewMode::Wysiwyg`.
#[must_use]
pub fn step_horizontal(
    rope: &Rope,
    pos: usize,
    forward: bool,
    by_word: bool,
    view_mode: ViewMode,
) -> usize {
    if by_word {
        return if forward {
            rope.next_word_offset(pos)
        } else {
            rope.prev_word_offset(pos)
        };
    }

    let mut next = if forward {
        rope.next_grapheme_offset(pos)
    } else {
        rope.prev_grapheme_offset(pos)
    };

    if matches!(view_mode, ViewMode::Wysiwyg) && next != pos {
        let lc = rope.byte_to_line_col(next);
        let line_range = rope.line_byte_range(lc.line);
        let line_text = rope.slice_to_string(line_range.clone());
        let tokens = parse_inline_tokens(line_range.start, &line_text);

        for _ in 0..8 {
            let jumped = tokens.iter().find_map(|tok| {
                if tok.kind == InlineTokenKind::SyntaxMarker
                    && next > tok.range.start
                    && next < tok.range.end
                {
                    Some(if forward {
                        tok.range.end
                    } else {
                        tok.range.start
                    })
                } else {
                    None
                }
            });
            if let Some(target) = jumped {
                next = target;
            } else {
                break;
            }
        }
    }

    next
}

/// Smart `Backspace` / `Delete`, cleaning up empty paired delimiters in `Wysiwyg` mode.
#[must_use]
pub fn handle_delete_key(
    rope: &Rope,
    selection: Selection,
    forward: bool,
    by_word: bool,
    view_mode: ViewMode,
) -> Option<(Rope, Selection, EditGroup)> {
    if !selection.is_empty() {
        let range = selection.range();
        let mut next = rope.clone();
        next.replace_range(range.clone(), "");
        return Some((next, Selection::collapsed(range.start), EditGroup::Discrete));
    }

    let caret = rope.clamp_char_boundary(selection.head);
    if !forward && caret == 0 {
        return None;
    }
    if forward && caret >= rope.len_bytes() {
        return None;
    }

    // If caret is between an empty pair like `**|**` or `*|*` or `` `|` ``, delete both delimiters!
    if !forward && !by_word {
        for delim in ["**", "~~", "==", "*", "`"] {
            let dl = delim.len();
            if caret >= dl && caret.saturating_add(dl) <= rope.len_bytes() {
                let left = rope.slice_to_string(caret.saturating_sub(dl)..caret);
                let right = rope.slice_to_string(caret..caret.saturating_add(dl));
                if left == delim && right == delim {
                    let mut next = rope.clone();
                    let start = caret.saturating_sub(dl);
                    next.replace_range(start..caret.saturating_add(dl), "");
                    return Some((next, Selection::collapsed(start), EditGroup::Backspace));
                }
            }
        }
    }

    let other = step_horizontal(rope, caret, forward, by_word, view_mode);
    let del_range = caret.min(other)..caret.max(other);
    if del_range.is_empty() {
        return None;
    }

    let mut next = rope.clone();
    next.replace_range(del_range.clone(), "");
    let group = if !forward && !by_word {
        EditGroup::Backspace
    } else {
        EditGroup::Discrete
    };
    Some((next, Selection::collapsed(del_range.start), group))
}

/// Visual vertical/line boundary movement (`Up`, `Down`, `Home`, `End`, `PageUp`, `PageDown`).
#[must_use]
pub fn move_with_frame(
    rope: &Rope,
    frame: Option<&ViewportFrame>,
    current: usize,
    preferred_x: Option<f32>,
    key: KeyCode,
) -> (usize, Option<f32>) {
    match key {
        KeyCode::Home => {
            let lc = rope.byte_to_line_col(current);
            let lr = rope.line_byte_range(lc.line);
            (lr.start, None)
        }
        KeyCode::End => {
            let lc = rope.byte_to_line_col(current);
            let lr = rope.line_byte_range(lc.line);
            (lr.end, None)
        }
        KeyCode::Up => {
            if let Some(f) = frame {
                let (off, x) = f.move_vertical(current, preferred_x, -1);
                (off, Some(x))
            } else {
                let lc = rope.byte_to_line_col(current);
                if lc.line == 0 {
                    (0, None)
                } else {
                    (
                        rope.line_col_to_byte(lc.line.saturating_sub(1), lc.col_bytes),
                        None,
                    )
                }
            }
        }
        KeyCode::Down => {
            if let Some(f) = frame {
                let (off, x) = f.move_vertical(current, preferred_x, 1);
                (off, Some(x))
            } else {
                let lc = rope.byte_to_line_col(current);
                (
                    rope.line_col_to_byte(lc.line.saturating_add(1), lc.col_bytes),
                    None,
                )
            }
        }
        KeyCode::PageUp => {
            if let Some(f) = frame {
                let mut pos = current;
                let mut px = preferred_x;
                for _ in 0..18 {
                    let (n, x) = f.move_vertical(pos, px, -1);
                    if n == pos {
                        break;
                    }
                    pos = n;
                    px = Some(x);
                }
                (pos, px)
            } else {
                let lc = rope.byte_to_line_col(current);
                (
                    rope.line_col_to_byte(lc.line.saturating_sub(20), lc.col_bytes),
                    None,
                )
            }
        }
        KeyCode::PageDown => {
            if let Some(f) = frame {
                let mut pos = current;
                let mut px = preferred_x;
                for _ in 0..18 {
                    let (n, x) = f.move_vertical(pos, px, 1);
                    if n == pos {
                        break;
                    }
                    pos = n;
                    px = Some(x);
                }
                (pos, px)
            } else {
                let lc = rope.byte_to_line_col(current);
                (
                    rope.line_col_to_byte(lc.line.saturating_add(20), lc.col_bytes),
                    None,
                )
            }
        }
        _ => (current, preferred_x),
    }
}
