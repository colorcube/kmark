//! Detection and lossless conversion of foreign table formats (Unicode terminal box tables,
//! ASCII `+---+---+` / `psql` grids, CSV / semicolon-CSV, TSV, and clipboard HTML `<table>`)
//! into formatted GFM Markdown tables (`#324`).

use std::ops::Range;

use kmark_buffer::{Rope, Selection};
use kmark_syntax::{Block, BlockKind, ColumnAlign, looks_like_table_row, parse_table_delimiter};

use crate::commands::EditableTable;

/// Parsed foreign table ready to be formatted as GFM Markdown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportedTable {
    pub cols: usize,
    pub alignments: Vec<ColumnAlign>,
    /// Row 0 is the table header; `rows[1..]` are body rows.
    pub rows: Vec<Vec<String>>,
}

impl ImportedTable {
    /// Formats the imported table as a padded GFM Markdown table block (without trailing newline).
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let editable = EditableTable::new(self.cols, self.alignments.clone(), self.rows.clone());
        editable.format_markdown()
    }
}

/// Attempts to parse `raw` as a Unicode box-drawing table, ASCII grid / `psql` table, TSV, or CSV
/// and returns the formatted Markdown table string.
#[must_use]
pub fn convert_text_to_markdown_table(raw: &str) -> Option<String> {
    parse_foreign_table(raw).map(|tbl| tbl.to_markdown())
}

/// Attempts to parse an HTML snippet containing a `<table>…</table>` (e.g. from browser or
/// LibreOffice Calc/Writer clipboard) into a formatted Markdown table string.
#[must_use]
pub fn convert_html_to_markdown_table(html: &str) -> Option<String> {
    parse_html_table(html).map(|tbl| tbl.to_markdown())
}

/// Parses `raw` into an [`ImportedTable`] if it matches any supported structured table format.
#[must_use]
pub fn parse_foreign_table(raw: &str) -> Option<ImportedTable> {
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let trimmed = normalized.trim_matches('\n');
    if trimmed.trim().is_empty() {
        return None;
    }

    // 1. Already a GFM Markdown table? Re-parse and normalize it cleanly so explicit conversion is idempotent.
    if let Some(md_tbl) = try_reparse_existing_markdown_table(trimmed) {
        return Some(md_tbl);
    }

    // 2. Unicode box-drawing (`┌─┬─┐`, `│ … │`, `├─┼─┤`, `└─┴─┘`), ASCII `+---+---+`, or `psql` tables
    if let Some(box_tbl) = parse_box_or_grid_table(trimmed) {
        return Some(box_tbl);
    }

    // 3. Delimited text tables: TSV (`\t`), semicolon CSV (`;`), comma CSV (`,`)
    if let Some(delim_tbl) = parse_delimited_table(trimmed) {
        return Some(delim_tbl);
    }

    None
}

/// Locates the document byte range of a convertible foreign table either from an explicit
/// selection or automatically around the collapsed caret (`#324`), returning `(range, markdown)`.
#[must_use]
pub fn find_and_convert_table_at_selection(
    rope: &Rope,
    blocks: &[Block],
    selection: Selection,
) -> Option<(Range<usize>, String)> {
    if !selection.is_empty() {
        let sel = selection.range();
        let raw = rope.slice_to_string(sel.clone());
        if let Some(md) = convert_text_to_markdown_table(&raw) {
            // Trim a trailing newline from the replaced range so existing line breaks stay intact.
            let mut end = sel.end;
            if end > sel.start && rope.byte_at(end.saturating_sub(1)) == Some(b'\n') {
                end = end.saturating_sub(1);
            }
            return Some((sel.start..end, md));
        }

        // Forgiving fallback: expand partial-line selection to full logical lines.
        let start_lc = rope.byte_to_line_col(sel.start);
        let end_probe =
            if sel.end > sel.start && rope.byte_at(sel.end.saturating_sub(1)) == Some(b'\n') {
                sel.end.saturating_sub(1)
            } else {
                sel.end
            };
        let end_lc = rope.byte_to_line_col(end_probe);
        let full_start = rope.line_byte_range(start_lc.line).start;
        let full_end = rope.line_byte_range(end_lc.line).end;
        let expanded = rope.slice_to_string(full_start..full_end);
        if let Some(md) = convert_text_to_markdown_table(&expanded) {
            return Some((full_start..full_end, md));
        }
        return None;
    }

    let caret = rope.clamp_char_boundary(selection.head);
    // Do not auto-grab inside fenced code blocks when nothing is selected.
    for blk in blocks {
        if caret >= blk.byte_range.start
            && caret <= blk.byte_range.end
            && matches!(blk.kind, BlockKind::CodeFence { .. })
        {
            return None;
        }
    }

    let caret_line = rope.byte_to_line_col(caret).line;
    let total_lines = rope.line_count();
    if total_lines == 0 {
        return None;
    }

    // 1. Try expanding a Unicode/ASCII box or grid table around `caret_line` (tolerating blank lines
    //    between rows as often produced when copying from terminals or web UIs!).
    if let Some((first_l, last_l)) = find_box_table_line_span(rope, caret_line, total_lines) {
        let byte_start = rope.line_byte_range(first_l).start;
        let byte_end = rope.line_byte_range(last_l).end;
        let slice = rope.slice_to_string(byte_start..byte_end);
        if let Some(md) = parse_box_or_grid_table(&slice).map(|t| t.to_markdown()) {
            return Some((byte_start..byte_end, md));
        }
    }

    // 2. Try contiguous non-blank lines around `caret_line` (CSV / TSV / `psql` block).
    let cur_txt = line_str(rope, caret_line);
    if cur_txt.trim().is_empty() {
        return None;
    }
    let mut top = caret_line;
    while top > 0 {
        let prev = line_str(rope, top.saturating_sub(1));
        if !is_candidate_delimited_line(&prev) {
            break;
        }
        top = top.saturating_sub(1);
    }
    let mut bot = caret_line;
    while bot.saturating_add(1) < total_lines {
        let next = line_str(rope, bot.saturating_add(1));
        if !is_candidate_delimited_line(&next) {
            break;
        }
        bot = bot.saturating_add(1);
    }
    if bot > top {
        let byte_start = rope.line_byte_range(top).start;
        let byte_end = rope.line_byte_range(bot).end;
        let slice = rope.slice_to_string(byte_start..byte_end);
        if let Some(tbl) = parse_foreign_table(&slice) {
            // Require stricter confidence when nothing is selected so two-line comma prose never converts.
            if is_confident_unselected_table(&slice, &tbl) {
                return Some((byte_start..byte_end, tbl.to_markdown()));
            }
        }
    }

    None
}

fn line_str(rope: &Rope, line_idx: usize) -> String {
    rope.slice_to_string(rope.line_byte_range(line_idx))
}

fn is_candidate_delimited_line(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return false;
    }
    if t.starts_with('#') || t.starts_with("```") || t.starts_with("~~~") || t.starts_with('>') {
        return false;
    }
    true
}

fn is_confident_unselected_table(raw: &str, tbl: &ImportedTable) -> bool {
    if tbl.cols < 2 || tbl.rows.is_empty() {
        return false;
    }
    // Box/grid/TSV/semicolon tables are always high-confidence.
    if raw.contains('\t')
        || raw.contains(';')
        || raw.chars().any(is_unicode_vertical_bar)
        || raw.lines().any(|l| is_box_border_line(l.trim()))
    {
        return true;
    }
    // For unquoted comma-only blocks without selection, require either >= 3 columns or >= 3 rows
    // with compact cell lengths so normal two-line comma sentences are not mistaken for CSV.
    if tbl.cols >= 3 || tbl.rows.len() >= 3 {
        let max_cell = tbl
            .rows
            .iter()
            .flat_map(|r| r.iter())
            .map(|c| c.chars().count())
            .max()
            .unwrap_or(0);
        return max_cell <= 80;
    }
    let max_cell = tbl
        .rows
        .iter()
        .flat_map(|r| r.iter())
        .map(|c| c.chars().count())
        .max()
        .unwrap_or(0);
    max_cell <= 32
}

fn find_box_table_line_span(
    rope: &Rope,
    caret_line: usize,
    total_lines: usize,
) -> Option<(usize, usize)> {
    let cur = line_str(rope, caret_line);
    let cur_is_box = is_box_member_line(&cur);
    let bridged_blank = cur.trim().is_empty()
        && caret_line > 0
        && caret_line.saturating_add(1) < total_lines
        && is_box_member_line(&line_str(rope, caret_line.saturating_sub(1)))
        && is_box_member_line(&line_str(rope, caret_line.saturating_add(1)));

    if !cur_is_box && !bridged_blank {
        return None;
    }

    let mut top = if cur_is_box {
        caret_line
    } else {
        caret_line.saturating_sub(1)
    };
    loop {
        let top_txt = line_str(rope, top);
        if is_top_box_border(top_txt.trim()) || top == 0 {
            break;
        }
        if let Some(prev_box) = prev_box_line_across_blanks(rope, top) {
            let prev_txt = line_str(rope, prev_box);
            if is_bottom_box_border(prev_txt.trim()) {
                break;
            }
            top = prev_box;
        } else {
            break;
        }
    }

    let mut bot = if cur_is_box {
        caret_line
    } else {
        caret_line.saturating_add(1)
    };
    loop {
        let bot_txt = line_str(rope, bot);
        if is_bottom_box_border(bot_txt.trim()) || bot.saturating_add(1) >= total_lines {
            break;
        }
        if let Some(next_box) = next_box_line_across_blanks(rope, bot, total_lines) {
            let next_txt = line_str(rope, next_box);
            if is_top_box_border(next_txt.trim()) {
                break;
            }
            bot = next_box;
        } else {
            break;
        }
    }

    if bot >= top { Some((top, bot)) } else { None }
}

fn prev_box_line_across_blanks(rope: &Rope, from_line: usize) -> Option<usize> {
    let mut step = 1usize;
    while step <= 3 && from_line >= step {
        let idx = from_line.saturating_sub(step);
        let s = line_str(rope, idx);
        if s.trim().is_empty() {
            step = step.saturating_add(1);
            continue;
        }
        return is_box_member_line(&s).then_some(idx);
    }
    None
}

fn next_box_line_across_blanks(rope: &Rope, from_line: usize, total_lines: usize) -> Option<usize> {
    let mut step = 1usize;
    while step <= 3 && from_line.saturating_add(step) < total_lines {
        let idx = from_line.saturating_add(step);
        let s = line_str(rope, idx);
        if s.trim().is_empty() {
            step = step.saturating_add(1);
            continue;
        }
        return is_box_member_line(&s).then_some(idx);
    }
    None
}

fn try_reparse_existing_markdown_table(raw: &str) -> Option<ImportedTable> {
    let lines: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.len() < 2 {
        return None;
    }
    if !looks_like_table_row(lines.first().copied().unwrap_or("")) {
        return None;
    }
    let aligns = parse_table_delimiter(lines.get(1).copied().unwrap_or(""))?;
    if aligns.is_empty() {
        return None;
    }
    // Reject if any line uses Unicode box borders (`┌`, `├`, `└`, `│`), which belong to `parse_box_or_grid_table`.
    if raw
        .chars()
        .any(|c| is_unicode_vertical_bar(c) || is_unicode_box_border_char(c))
    {
        return None;
    }
    let rope = Rope::from_str(raw);
    let blocks = kmark_syntax::parse_blocks(&rope);
    if blocks.len() == 1
        && let BlockKind::Table(ref tbl) = blocks[0].kind
    {
        let editable = EditableTable::from_ast(&rope, tbl);
        return Some(ImportedTable {
            cols: editable.cols(),
            alignments: editable.alignments().to_vec(),
            rows: editable.rows().to_vec(),
        });
    }
    None
}

// ============================================================================
// 1. Unicode Box-Drawing, ASCII `+---+---+`, and `psql` Tables
// ============================================================================

fn is_unicode_vertical_bar(ch: char) -> bool {
    matches!(ch, '│' | '┃' | '║' | '╎' | '┇' | '┊' | '︱')
}

fn is_unicode_box_border_char(ch: char) -> bool {
    matches!(
        ch,
        '─' | '━'
            | '═'
            | '┌'
            | '┍'
            | '┎'
            | '┏'
            | '╒'
            | '╓'
            | '╔'
            | '╭'
            | '┐'
            | '┑'
            | '┒'
            | '┓'
            | '╕'
            | '╖'
            | '╗'
            | '╮'
            | '└'
            | '┕'
            | '┖'
            | '┗'
            | '╘'
            | '╙'
            | '╚'
            | '╰'
            | '┘'
            | '┙'
            | '┚'
            | '┛'
            | '╛'
            | '╜'
            | '╝'
            | '╯'
            | '├'
            | '┝'
            | '┞'
            | '┟'
            | '┠'
            | '┡'
            | '┢'
            | '┣'
            | '╞'
            | '╟'
            | '╠'
            | '┤'
            | '┥'
            | '┦'
            | '┧'
            | '┨'
            | '┩'
            | '┪'
            | '┫'
            | '╡'
            | '╢'
            | '╣'
            | '┬'
            | '┭'
            | '┮'
            | '┯'
            | '┰'
            | '┱'
            | '┲'
            | '┳'
            | '╤'
            | '╥'
            | '╦'
            | '┴'
            | '┵'
            | '┶'
            | '┷'
            | '┸'
            | '┹'
            | '┺'
            | '┻'
            | '╧'
            | '╨'
            | '╩'
            | '┼'
            | '┽'
            | '┾'
            | '┿'
            | '╀'
            | '╁'
            | '╂'
            | '╃'
            | '╄'
            | '╅'
            | '╆'
            | '╇'
            | '╈'
            | '╉'
            | '╊'
            | '╋'
            | '╪'
            | '╫'
            | '╬'
    )
}

fn is_top_box_border(trimmed: &str) -> bool {
    is_box_border_line(trimmed)
        && trimmed
            .chars()
            .next()
            .is_some_and(|c| matches!(c, '┌' | '┍' | '┎' | '┏' | '╒' | '╓' | '╔' | '╭'))
}

fn is_bottom_box_border(trimmed: &str) -> bool {
    is_box_border_line(trimmed)
        && trimmed
            .chars()
            .next()
            .is_some_and(|c| matches!(c, '└' | '┕' | '┖' | '┗' | '╘' | '╙' | '╚' | '╰'))
}

fn is_box_border_line(trimmed: &str) -> bool {
    if trimmed.is_empty() {
        return false;
    }
    let mut horiz_strokes = 0usize;
    let mut junction_or_corner = 0usize;
    for ch in trimmed.chars() {
        if matches!(ch, '─' | '━' | '═' | '-' | '=') {
            horiz_strokes = horiz_strokes.saturating_add(1);
        } else if is_unicode_box_border_char(ch)
            || is_unicode_vertical_bar(ch)
            || matches!(ch, '+' | '|')
        {
            junction_or_corner = junction_or_corner.saturating_add(1);
        } else if ch == ' ' || ch == ':' {
            // Allow spaces and alignment colons in separator lines
        } else {
            return false;
        }
    }
    // Require at least 3 horizontal strokes AND at least 1 junction/corner (`+`, `┼`, `┬`, `|`, etc.)
    // so a plain Markdown horizontal rule `---` is never misclassified as a multi-column box border!
    horiz_strokes >= 3 && junction_or_corner >= 1
}

fn is_box_member_line(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return false;
    }
    if is_box_border_line(t) || t.chars().any(is_unicode_vertical_bar) {
        return true;
    }
    // ASCII grid data line (`| a | b |` or `psql` `a | b` when paired with `+---+` / `---+---`)
    t.contains('|') && !t.starts_with('>')
}

fn is_psql_row_count_footer(trimmed: &str) -> bool {
    // Matches `(2 rows)`, `(1 row)`, `(2 Zeilen)`, `(1 Zeile)` at the end of `psql` output.
    if !(trimmed.starts_with('(') && trimmed.ends_with(')')) {
        return false;
    }
    let inner = trimmed
        .get(1..trimmed.len().saturating_sub(1))
        .unwrap_or("")
        .trim();
    let Some((num_part, word_part)) = inner.split_once(' ') else {
        return false;
    };
    !num_part.is_empty()
        && num_part.chars().all(|c| c.is_ascii_digit())
        && matches!(
            word_part.trim().to_ascii_lowercase().as_str(),
            "row" | "rows" | "zeile" | "zeilen"
        )
}

fn parse_box_or_grid_table(raw: &str) -> Option<ImportedTable> {
    let non_empty: Vec<&str> = raw
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .collect();
    if non_empty.len() < 2 {
        return None;
    }

    let has_border = non_empty.iter().any(|l| is_box_border_line(l.trim()));
    let has_unicode_bars = non_empty
        .iter()
        .all(|l| is_box_border_line(l.trim()) || l.chars().any(is_unicode_vertical_bar));

    if !has_border && !has_unicode_bars {
        return None;
    }

    // Group physical data lines into "bands" separated by horizontal border lines.
    let mut bands: Vec<Vec<Vec<String>>> = Vec::new();
    let mut current_band: Vec<Vec<String>> = Vec::new();
    let mut internal_separators = 0usize;

    for (idx, &line) in non_empty.iter().enumerate() {
        let trimmed = line.trim();
        if idx.saturating_add(1) == non_empty.len() && is_psql_row_count_footer(trimmed) {
            continue;
        }
        if is_box_border_line(trimmed) {
            if !current_band.is_empty() {
                bands.push(std::mem::take(&mut current_band));
                // Count separators that actually sit *after* a non-empty band (ignoring bottom border later).
                internal_separators = internal_separators.saturating_add(1);
            }
            continue;
        }
        let cells = split_box_data_line(trimmed)?;
        if cells.is_empty() {
            return None;
        }
        current_band.push(cells);
    }

    if !current_band.is_empty() {
        bands.push(current_band);
    } else {
        // Last line was a bottom border (`└───┴───┘` or `+---+---+`), so decrement trailing border count.
        internal_separators = internal_separators.saturating_sub(1);
    }

    if bands.is_empty() {
        return None;
    }

    let cols = bands
        .iter()
        .flat_map(|b| b.iter().map(Vec::len))
        .max()
        .unwrap_or(0);
    if cols < 2 {
        return None;
    }

    // Ensure every physical row has a consistent column count (within +/- 0 for boxed tables).
    let consistent = bands.iter().flat_map(|b| b.iter()).all(|r| r.len() == cols);
    if !consistent {
        return None;
    }

    let mut logical_rows: Vec<Vec<String>> = Vec::new();
    if bands.len() == 1 {
        // Borderless or outer-frame-only Unicode box table (`┌──┐ │ a │ b │ │ c │ d │ └──┘`):
        // every physical line is a separate logical row.
        for row in &bands[0] {
            logical_rows.push(normalize_row_cells(row, cols));
        }
    } else if internal_separators >= 2 {
        // Row-bordered grid table (like `rich`, Claude/CLI box tables, `tabulate(..., tablefmt="grid")`,
        // and Ordo #324!): EVERY band between horizontal separators is ONE logical row, and multiple
        // physical lines within a band are word-wrapped continuations of that row's cells!
        for band in &bands {
            logical_rows.push(merge_wrapped_band_cells(band, cols));
        }
    } else {
        // `internal_separators == 1` (`sqlite3 -box`, `mysql`, `psql`):
        // Band 0 is the header (merged if multi-line header), Band 1 holds individual body rows!
        logical_rows.push(merge_wrapped_band_cells(&bands[0], cols));
        for band in bands.iter().skip(1) {
            for row in band {
                logical_rows.push(normalize_row_cells(row, cols));
            }
        }
    }

    if logical_rows.is_empty() {
        return None;
    }

    Some(ImportedTable {
        cols,
        alignments: vec![ColumnAlign::Left; cols],
        rows: logical_rows,
    })
}

fn split_box_data_line(trimmed: &str) -> Option<Vec<String>> {
    let uses_unicode_bar = trimmed.chars().any(is_unicode_vertical_bar);
    if uses_unicode_bar {
        let parts: Vec<&str> = trimmed.split(is_unicode_vertical_bar).collect();
        if parts.len() < 2 {
            return None;
        }
        let starts_bar = trimmed.chars().next().is_some_and(is_unicode_vertical_bar);
        let ends_bar = trimmed
            .chars()
            .next_back()
            .is_some_and(is_unicode_vertical_bar);
        let start_idx = usize::from(starts_bar);
        let end_idx = if ends_bar {
            parts.len().saturating_sub(1)
        } else {
            parts.len()
        };
        if end_idx <= start_idx {
            return None;
        }
        return Some(
            parts
                .get(start_idx..end_idx)?
                .iter()
                .map(|s| sanitize_cell_for_markdown(s))
                .collect(),
        );
    }

    if !trimmed.contains('|') {
        return None;
    }
    // Split on unescaped ASCII `|`
    let mut raw_parts: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut chars = trimmed.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' && chars.peek() == Some(&'|') {
            cur.push('\\');
            if let Some(pipe) = chars.next() {
                cur.push(pipe);
            }
        } else if ch == '|' {
            raw_parts.push(std::mem::take(&mut cur));
        } else {
            cur.push(ch);
        }
    }
    raw_parts.push(cur);

    let starts_pipe = trimmed.starts_with('|');
    let ends_pipe = trimmed.ends_with('|') && !trimmed.ends_with("\\|");
    let start_idx = usize::from(starts_pipe);
    let end_idx = if ends_pipe {
        raw_parts.len().saturating_sub(1)
    } else {
        raw_parts.len()
    };
    if end_idx <= start_idx {
        return None;
    }
    Some(
        raw_parts
            .get(start_idx..end_idx)?
            .iter()
            .map(|s| sanitize_cell_for_markdown(s))
            .collect(),
    )
}

fn merge_wrapped_band_cells(band: &[Vec<String>], cols: usize) -> Vec<String> {
    let mut merged = vec![String::new(); cols];
    for phys_row in band {
        for (c, target) in merged.iter_mut().enumerate() {
            let piece = phys_row.get(c).map_or("", String::as_str).trim();
            if piece.is_empty() {
                continue;
            }
            if !target.is_empty() {
                // Join hyphenated line wraps cleanly or with a single space.
                target.push(' ');
            }
            target.push_str(piece);
        }
    }
    merged
}

fn normalize_row_cells(row: &[String], cols: usize) -> Vec<String> {
    let mut out = Vec::with_capacity(cols);
    for c in 0..cols {
        out.push(
            row.get(c)
                .map_or_else(String::new, |s| sanitize_cell_for_markdown(s)),
        );
    }
    out
}

// ============================================================================
// 2. RFC-4180 TSV / Semicolon-CSV / Comma-CSV Parser
// ============================================================================

fn parse_delimited_table(raw: &str) -> Option<ImportedTable> {
    for delim in ['\t', ';', ','] {
        if !raw.contains(delim) {
            continue;
        }
        if let Some(records) = parse_rfc4180_records(raw, delim)
            && validate_delimited_records(&records, delim)
        {
            let cols = records.first().map_or(0, Vec::len);
            let rows: Vec<Vec<String>> = records
                .into_iter()
                .map(|r| {
                    r.into_iter()
                        .map(|cell| sanitize_cell_for_markdown(&cell))
                        .collect()
                })
                .collect();
            return Some(ImportedTable {
                cols,
                alignments: vec![ColumnAlign::Left; cols],
                rows,
            });
        }
    }
    None
}

fn validate_delimited_records(records: &[Vec<String>], delim: char) -> bool {
    if records.len() < 2 {
        return false;
    }
    let Some(first) = records.first() else {
        return false;
    };
    let cols = first.len();
    if cols < 2 {
        return false;
    }
    if !records.iter().all(|r| r.len() == cols) {
        return false;
    }
    // Ensure at least one non-empty cell exists and reject accidental prose sentences on `,`.
    let any_non_empty = records.iter().flatten().any(|c| !c.trim().is_empty());
    if !any_non_empty {
        return false;
    }
    if delim == ',' && records.len() == 2 && cols == 2 {
        // Two lines that each happen to have one comma could be normal prose; reject if cells look
        // like long multi-word sentences (> 60 chars).
        let longest = records
            .iter()
            .flatten()
            .map(|c| c.trim().chars().count())
            .max()
            .unwrap_or(0);
        if longest > 60 {
            return false;
        }
    }
    true
}

fn parse_rfc4180_records(input: &str, delim: char) -> Option<Vec<Vec<String>>> {
    let mut records: Vec<Vec<String>> = Vec::new();
    let mut current_row: Vec<String> = Vec::new();
    let mut current_field = String::new();
    let mut in_quotes = false;
    let mut field_was_quoted = false;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    let _ = chars.next();
                    current_field.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                current_field.push(ch);
            }
            continue;
        }

        if ch == '"' && current_field.trim().is_empty() && !field_was_quoted {
            current_field.clear();
            in_quotes = true;
            field_was_quoted = true;
        } else if ch == delim {
            current_row.push(std::mem::take(&mut current_field));
            field_was_quoted = false;
        } else if ch == '\n' {
            current_row.push(std::mem::take(&mut current_field));
            field_was_quoted = false;
            // Skip completely blank lines (single empty unquoted field)
            let is_blank_row =
                current_row.len() == 1 && current_row.first().is_some_and(|s| s.trim().is_empty());
            if !is_blank_row {
                records.push(std::mem::take(&mut current_row));
            } else {
                current_row.clear();
            }
        } else {
            current_field.push(ch);
        }
    }

    if in_quotes {
        // Unclosed quote -> not a valid RFC-4180 table
        return None;
    }

    if !current_field.is_empty() || !current_row.is_empty() || field_was_quoted {
        current_row.push(current_field);
        let is_blank_row =
            current_row.len() == 1 && current_row.first().is_some_and(|s| s.trim().is_empty());
        if !is_blank_row {
            records.push(current_row);
        }
    }

    Some(records)
}

// ============================================================================
// 3. Clipboard HTML `<table>` Parser (Browser / LibreOffice Calc & Writer)
// ============================================================================

/// Parses the first `<table>…</table>` element in `html` into an [`ImportedTable`].
#[must_use]
pub fn parse_html_table(html: &str) -> Option<ImportedTable> {
    let lower = html.to_ascii_lowercase();
    let table_start = lower.find("<table")?;
    let table_close_rel = lower.get(table_start..)?.find("</table>")?;
    let table_html = html.get(table_start..table_start.saturating_add(table_close_rel))?;

    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut scan = 0usize;
    let table_lower = table_html.to_ascii_lowercase();

    while let Some(tr_rel) = table_lower.get(scan..)?.find("<tr") {
        let tr_open = scan.saturating_add(tr_rel);
        let Some(gt_rel) = table_html.get(tr_open..)?.find('>') else {
            break;
        };
        let tr_body_start = tr_open.saturating_add(gt_rel).saturating_add(1);
        let tr_end = table_lower
            .get(tr_body_start..)
            .and_then(|s| s.find("</tr>"))
            .map_or(table_html.len(), |r| tr_body_start.saturating_add(r));
        let tr_slice = table_html.get(tr_body_start..tr_end).unwrap_or("");
        let cells = extract_html_row_cells(tr_slice);
        if !cells.is_empty() {
            rows.push(cells);
        }
        scan = tr_end.saturating_add(5).min(table_html.len());
    }

    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    if rows.is_empty() || cols < 1 || (rows.len() == 1 && cols < 2) {
        return None;
    }
    for row in &mut rows {
        row.resize(cols, String::new());
    }

    Some(ImportedTable {
        cols,
        alignments: vec![ColumnAlign::Left; cols],
        rows,
    })
}

fn extract_html_row_cells(tr_html: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let lower = tr_html.to_ascii_lowercase();
    let mut pos = 0usize;

    while pos < lower.len() {
        let rest_lower = lower.get(pos..).unwrap_or("");
        let next_td = rest_lower.find("<td");
        let next_th = rest_lower.find("<th");
        let (cell_rel, close_tag) = match (next_td, next_th) {
            (Some(d), Some(h)) => {
                if d <= h {
                    (d, "</td>")
                } else {
                    (h, "</th>")
                }
            }
            (Some(d), None) => (d, "</td>"),
            (None, Some(h)) => (h, "</th>"),
            (None, None) => break,
        };
        let tag_start = pos.saturating_add(cell_rel);
        // Verify tag boundary (`<td>`, `<td ...>`, `<th>`, `<th ...>`, not `<thead`)
        let after_tag_name = lower.as_bytes().get(tag_start.saturating_add(3)).copied();
        if !matches!(
            after_tag_name,
            Some(b'>' | b' ' | b'\t' | b'\n' | b'\r' | b'/')
        ) {
            pos = tag_start.saturating_add(3);
            continue;
        }
        let Some(gt_rel) = tr_html.get(tag_start..).and_then(|s| s.find('>')) else {
            break;
        };
        let inner_start = tag_start.saturating_add(gt_rel).saturating_add(1);
        let inner_end = lower
            .get(inner_start..)
            .and_then(|s| s.find(close_tag))
            .map_or(tr_html.len(), |r| inner_start.saturating_add(r));
        let raw_inner = tr_html.get(inner_start..inner_end).unwrap_or("");
        let text = strip_html_tags_and_entities(raw_inner);
        cells.push(sanitize_cell_for_markdown(&text));
        pos = inner_end.saturating_add(close_tag.len()).min(tr_html.len());
    }

    cells
}

fn strip_html_tags_and_entities(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut tag_buf = String::new();

    for ch in html.chars() {
        if in_tag {
            if ch == '>' {
                in_tag = false;
                let t = tag_buf.trim().to_ascii_lowercase();
                if matches!(t.as_str(), "br" | "br/" | "br /" | "/p" | "/div" | "/li") {
                    out.push(' ');
                }
                tag_buf.clear();
            } else {
                tag_buf.push(ch);
            }
        } else if ch == '<' {
            in_tag = true;
            tag_buf.clear();
        } else {
            out.push(ch);
        }
    }

    decode_basic_html_entities(&out)
}

fn decode_basic_html_entities(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&#160;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// Normalizes cell whitespace and escapes unescaped `|` so the cell never breaks GFM table columns.
fn sanitize_cell_for_markdown(raw: &str) -> String {
    // 1. Replace newlines/tabs with spaces and collapse runs of whitespace cleanly.
    let collapsed: String = raw.split_whitespace().collect::<Vec<&str>>().join(" ");

    // 2. Escape any unescaped `|` as `\|`.
    let mut out = String::with_capacity(collapsed.len());
    let mut prev_backslash = false;
    for ch in collapsed.chars() {
        if ch == '|' && !prev_backslash {
            out.push_str("\\|");
        } else {
            out.push(ch);
        }
        prev_backslash = ch == '\\' && !prev_backslash;
    }
    out
}
