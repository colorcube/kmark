use std::ops::Range;

use kmark_buffer::Rope;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AlertKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

impl AlertKind {
    #[must_use]
    pub const fn title_de(self) -> &'static str {
        match self {
            Self::Note => "Hinweis",
            Self::Tip => "Tipp",
            Self::Important => "Wichtig",
            Self::Warning => "Warnung",
            Self::Caution => "Vorsicht",
        }
    }

    #[must_use]
    pub const fn title_en(self) -> &'static str {
        match self {
            Self::Note => "Note",
            Self::Tip => "Tip",
            Self::Important => "Important",
            Self::Warning => "Warning",
            Self::Caution => "Caution",
        }
    }

    #[must_use]
    pub const fn title(self, german: bool) -> &'static str {
        if german {
            self.title_de()
        } else {
            self.title_en()
        }
    }

    #[must_use]
    pub const fn tag_str(self) -> &'static str {
        match self {
            Self::Note => "NOTE",
            Self::Tip => "TIP",
            Self::Important => "IMPORTANT",
            Self::Warning => "WARNING",
            Self::Caution => "CAUTION",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ColumnAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskState {
    pub checked: bool,
    /// Exact byte range of `[ ]` or `[x]` in the document.
    pub bracket_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuoteLine {
    pub line_idx: usize,
    pub depth: u8,
    pub marker_range: Range<usize>,
    pub content_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableCell {
    /// Full interior range between pipes (excluding `|`).
    pub raw_range: Range<usize>,
    /// Trimmed content range inside cell padding spaces.
    pub trimmed_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableRow {
    pub line_idx: usize,
    pub byte_range: Range<usize>,
    pub cells: Vec<TableCell>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableBlock {
    pub col_count: usize,
    pub alignments: Vec<ColumnAlign>,
    pub header_row: TableRow,
    pub delimiter_line_idx: usize,
    pub delimiter_range: Range<usize>,
    pub body_rows: Vec<TableRow>,
}

impl TableBlock {
    pub fn all_data_rows(&self) -> impl Iterator<Item = (usize, &TableRow)> {
        std::iter::once((0usize, &self.header_row)).chain(
            self.body_rows
                .iter()
                .enumerate()
                .map(|(i, r)| (i.saturating_add(1), r)),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockKind {
    BlankLines,
    Heading {
        level: u8,
        prefix_range: Range<usize>,
        content_range: Range<usize>,
        setext_underline: Option<Range<usize>>,
    },
    Paragraph {
        lines: Vec<(usize, Range<usize>)>,
    },
    ThematicBreak,
    CodeFence {
        fence_char: u8,
        fence_len: usize,
        language: String,
        open_line_range: Range<usize>,
        body_lines: Vec<(usize, Range<usize>)>,
        close_line_range: Option<Range<usize>>,
    },
    BlockQuote {
        alert: Option<AlertKind>,
        custom_title: Option<String>,
        lines: Vec<QuoteLine>,
    },
    ListItem {
        indent_spaces: usize,
        ordered: Option<u32>,
        marker_range: Range<usize>,
        task: Option<TaskState>,
        content_range: Range<usize>,
        line_idx: usize,
    },
    Table(TableBlock),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub byte_range: Range<usize>,
    pub line_range: Range<usize>,
    pub kind: BlockKind,
}

/// Parses a [`Rope`] into sequential, non-overlapping [`Block`]s with exact document byte offsets.
#[must_use]
pub fn parse_blocks(rope: &Rope) -> Vec<Block> {
    parse_blocks_in_line_range(rope, 0..rope.line_count())
}

/// Parses only the logical lines within `line_window` (`start_line..end_line`), returning [`Block`]s
/// with exact document byte offsets and global line indices.
#[must_use]
pub fn parse_blocks_in_line_range(rope: &Rope, line_window: Range<usize>) -> Vec<Block> {
    let clamped_end = line_window.end.min(rope.line_count());
    let clamped_start = line_window.start.min(clamped_end);
    let total_lines = clamped_end.saturating_sub(clamped_start);
    let mut lines: Vec<(Range<usize>, String)> = Vec::with_capacity(total_lines);
    for idx in clamped_start..clamped_end {
        let r = rope.line_byte_range(idx);
        let s = rope.slice_to_string(r.clone());
        lines.push((r, s));
    }

    let mut blocks = parse_window_lines(&lines);
    if clamped_start > 0
        && let Ok(line_delta) = isize::try_from(clamped_start)
    {
        for blk in &mut blocks {
            blk.shift_offsets(0, line_delta);
        }
    }
    blocks
}

fn parse_window_lines(lines: &[(Range<usize>, String)]) -> Vec<Block> {
    let total_lines = lines.len();
    let mut blocks = Vec::new();
    let mut i = 0usize;

    while i < total_lines {
        let (ref line_range, ref line_text) = lines[i];

        // 1. Consecutive blank lines
        if is_blank(line_text) {
            let start_i = i;
            let start_b = line_range.start;
            let mut end_b = line_range.end;
            i = i.saturating_add(1);
            while i < total_lines && is_blank(&lines[i].1) {
                end_b = lines[i].0.end;
                i = i.saturating_add(1);
            }
            blocks.push(Block {
                byte_range: start_b..end_b,
                line_range: start_i..i,
                kind: BlockKind::BlankLines,
            });
            continue;
        }

        // 2. Code fence (``` or ~~~)
        if let Some((fence_char, fence_len, lang)) = parse_fence_open(line_text) {
            let start_i = i;
            let open_range = line_range.clone();
            let mut end_b = open_range.end;
            let mut body_lines = Vec::new();
            let mut close_range = None;
            i = i.saturating_add(1);
            while i < total_lines {
                let (ref r, ref t) = lines[i];
                end_b = r.end;
                if is_fence_close(t, fence_char, fence_len) {
                    close_range = Some(r.clone());
                    i = i.saturating_add(1);
                    break;
                }
                body_lines.push((i, r.clone()));
                i = i.saturating_add(1);
            }
            blocks.push(Block {
                byte_range: open_range.start..end_b,
                line_range: start_i..i,
                kind: BlockKind::CodeFence {
                    fence_char,
                    fence_len,
                    language: lang,
                    open_line_range: open_range,
                    body_lines,
                    close_line_range: close_range,
                },
            });
            continue;
        }

        // 3. ATX Heading (# .. ######)
        if let Some((level, prefix_len)) = parse_atx_heading(line_text) {
            let prefix_range = line_range.start..line_range.start.saturating_add(prefix_len);
            let content_range = prefix_range.end..line_range.end;
            blocks.push(Block {
                byte_range: line_range.clone(),
                line_range: i..i.saturating_add(1),
                kind: BlockKind::Heading {
                    level,
                    prefix_range,
                    content_range,
                    setext_underline: None,
                },
            });
            i = i.saturating_add(1);
            continue;
        }

        // 4. Thematic break (---, ***, ___)
        if is_thematic_break(line_text) {
            blocks.push(Block {
                byte_range: line_range.clone(),
                line_range: i..i.saturating_add(1),
                kind: BlockKind::ThematicBreak,
            });
            i = i.saturating_add(1);
            continue;
        }

        // 5. BlockQuote / GitHub Alert / Obsidian Callout
        if let Some((depth, marker_len)) = parse_blockquote_prefix(line_text) {
            let start_i = i;
            let start_b = line_range.start;
            let mut end_b = line_range.end;
            let mut q_lines = Vec::new();
            let first_content = line_text.get(marker_len..).unwrap_or("");
            let (alert, custom_title) = parse_alert_marker(first_content);

            q_lines.push(QuoteLine {
                line_idx: i,
                depth,
                marker_range: line_range.start..line_range.start.saturating_add(marker_len),
                content_range: line_range.start.saturating_add(marker_len)..line_range.end,
            });
            i = i.saturating_add(1);
            while i < total_lines {
                let (ref r, ref t) = lines[i];
                if let Some((d, mlen)) = parse_blockquote_prefix(t) {
                    end_b = r.end;
                    q_lines.push(QuoteLine {
                        line_idx: i,
                        depth: d,
                        marker_range: r.start..r.start.saturating_add(mlen),
                        content_range: r.start.saturating_add(mlen)..r.end,
                    });
                    i = i.saturating_add(1);
                } else {
                    break;
                }
            }
            blocks.push(Block {
                byte_range: start_b..end_b,
                line_range: start_i..i,
                kind: BlockKind::BlockQuote {
                    alert,
                    custom_title,
                    lines: q_lines,
                },
            });
            continue;
        }

        // 6. GFM Table (header row + delimiter row `| --- | :---: |`)
        if i.saturating_add(1) < total_lines
            && looks_like_table_row(line_text)
            && let Some(alignments) = parse_table_delimiter(&lines[i.saturating_add(1)].1)
        {
            let header_row = parse_table_row(i, line_range.clone(), line_text);
            if !header_row.cells.is_empty() && header_row.cells.len() == alignments.len() {
                let col_count = alignments.len();
                let start_i = i;
                let start_b = line_range.start;
                let delim_idx = i.saturating_add(1);
                let delim_range = lines[delim_idx].0.clone();
                let mut end_b = delim_range.end;
                let mut body_rows = Vec::new();
                i = i.saturating_add(2);
                while i < total_lines {
                    let (ref r, ref t) = lines[i];
                    if is_blank(t) || !looks_like_table_row(t) {
                        break;
                    }
                    let mut row = parse_table_row(i, r.clone(), t);
                    // Pad missing trailing cells to `col_count` pointing safely at row end.
                    while row.cells.len() < col_count {
                        row.cells.push(TableCell {
                            raw_range: r.end..r.end,
                            trimmed_range: r.end..r.end,
                        });
                    }
                    end_b = r.end;
                    body_rows.push(row);
                    i = i.saturating_add(1);
                }
                blocks.push(Block {
                    byte_range: start_b..end_b,
                    line_range: start_i..i,
                    kind: BlockKind::Table(TableBlock {
                        col_count,
                        alignments,
                        header_row,
                        delimiter_line_idx: delim_idx,
                        delimiter_range: delim_range,
                        body_rows,
                    }),
                });
                continue;
            }
        }

        // 7. List item (unordered `-`, `*`, `+`, ordered `1.`, `1)`, task `- [ ]`, `- [x]`)
        if let Some(item) = parse_list_item(i, line_range.clone(), line_text) {
            blocks.push(Block {
                byte_range: line_range.clone(),
                line_range: i..i.saturating_add(1),
                kind: item,
            });
            i = i.saturating_add(1);
            continue;
        }

        // 8. Setext Heading (`Title\n====` or `Title\n----`)
        if i.saturating_add(1) < total_lines
            && let Some(level) = parse_setext_underline(&lines[i.saturating_add(1)].1)
        {
            let u_range = lines[i.saturating_add(1)].0.clone();
            blocks.push(Block {
                byte_range: line_range.start..u_range.end,
                line_range: i..i.saturating_add(2),
                kind: BlockKind::Heading {
                    level,
                    prefix_range: line_range.start..line_range.start,
                    content_range: line_range.clone(),
                    setext_underline: Some(u_range),
                },
            });
            i = i.saturating_add(2);
            continue;
        }

        // 9. Paragraph (collect contiguous plain paragraph lines)
        let start_i = i;
        let start_b = line_range.start;
        let mut end_b = line_range.end;
        let mut p_lines = vec![(i, line_range.clone())];
        i = i.saturating_add(1);
        while i < total_lines {
            let (ref r, ref t) = lines[i];
            if is_blank(t)
                || parse_fence_open(t).is_some()
                || parse_atx_heading(t).is_some()
                || is_thematic_break(t)
                || parse_blockquote_prefix(t).is_some()
                || parse_list_item(i, r.clone(), t).is_some()
                || (i.saturating_add(1) < total_lines
                    && looks_like_table_row(t)
                    && parse_table_delimiter(&lines[i.saturating_add(1)].1).is_some())
            {
                break;
            }
            // If current line in `p_lines` (when single) is followed by setext underline, break before setext.
            if let Some(level) = parse_setext_underline(t)
                && p_lines.len() == 1
            {
                let (_, first_r) = p_lines.remove(0);
                blocks.push(Block {
                    byte_range: first_r.start..r.end,
                    line_range: start_i..i.saturating_add(1),
                    kind: BlockKind::Heading {
                        level,
                        prefix_range: first_r.start..first_r.start,
                        content_range: first_r,
                        setext_underline: Some(r.clone()),
                    },
                });
                i = i.saturating_add(1);
                break;
            }
            end_b = r.end;
            p_lines.push((i, r.clone()));
            i = i.saturating_add(1);
        }
        if !p_lines.is_empty() {
            blocks.push(Block {
                byte_range: start_b..end_b,
                line_range: start_i..i,
                kind: BlockKind::Paragraph { lines: p_lines },
            });
        }
    }

    blocks
}

#[must_use]
pub fn is_blank(line: &str) -> bool {
    line.bytes().all(|b| b == b' ' || b == b'\t')
}

fn leading_spaces(line: &str) -> usize {
    line.bytes().take_while(|&b| b == b' ').count()
}

fn parse_fence_open(line: &str) -> Option<(u8, usize, String)> {
    let lead = leading_spaces(line);
    if lead > 3 {
        return None;
    }
    let rest = line.get(lead..)?;
    let first = *rest.as_bytes().first()?;
    if first != b'`' && first != b'~' {
        return None;
    }
    let fence_len = rest.bytes().take_while(|&b| b == first).count();
    if fence_len < 3 {
        return None;
    }
    let info = rest.get(fence_len..)?.trim();
    if first == b'`' && info.contains('`') {
        return None;
    }
    Some((first, fence_len, info.to_owned()))
}

fn is_fence_close(line: &str, fence_char: u8, min_len: usize) -> bool {
    let lead = leading_spaces(line);
    if lead > 3 {
        return false;
    }
    let Some(rest) = line.get(lead..) else {
        return false;
    };
    let count = rest.bytes().take_while(|&b| b == fence_char).count();
    count >= min_len && rest.get(count..).is_some_and(is_blank)
}

#[must_use]
pub fn parse_atx_heading(line: &str) -> Option<(u8, usize)> {
    let lead = leading_spaces(line);
    if lead > 3 {
        return None;
    }
    let rest = line.get(lead..)?;
    let hashes = rest.bytes().take_while(|&b| b == b'#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let after = rest.get(hashes..)?;
    if after.is_empty() {
        return Some((hashes as u8, lead.saturating_add(hashes)));
    }
    if !after.starts_with(' ') && !after.starts_with('\t') {
        return None;
    }
    let ws = after
        .bytes()
        .take_while(|&b| b == b' ' || b == b'\t')
        .count();
    Some((hashes as u8, lead.saturating_add(hashes).saturating_add(ws)))
}

fn parse_setext_underline(line: &str) -> Option<u8> {
    let lead = leading_spaces(line);
    if lead > 3 {
        return None;
    }
    let trimmed = line.get(lead..)?.trim_end_matches([' ', '\t']);
    if trimmed.len() < 2 {
        return None;
    }
    if trimmed.bytes().all(|b| b == b'=') {
        Some(1)
    } else if trimmed.bytes().all(|b| b == b'-') {
        Some(2)
    } else {
        None
    }
}

#[must_use]
pub fn is_thematic_break(line: &str) -> bool {
    let lead = leading_spaces(line);
    if lead > 3 {
        return false;
    }
    let Some(rest) = line.get(lead..) else {
        return false;
    };
    let mut marker: Option<u8> = None;
    let mut count = 0usize;
    for b in rest.bytes() {
        if b == b' ' || b == b'\t' {
            continue;
        }
        if b != b'-' && b != b'*' && b != b'_' {
            return false;
        }
        match marker {
            None => {
                marker = Some(b);
                count = 1;
            }
            Some(m) if m == b => {
                count = count.saturating_add(1);
            }
            Some(_) => return false,
        }
    }
    count >= 3
}

#[must_use]
pub fn parse_blockquote_prefix(line: &str) -> Option<(u8, usize)> {
    let lead = leading_spaces(line);
    if lead > 3 {
        return None;
    }
    let bytes = line.as_bytes();
    let mut pos = lead;
    let mut depth = 0u8;
    while pos < bytes.len() {
        let sp = bytes
            .get(pos..)
            .map_or(0, |s| s.iter().take_while(|&&b| b == b' ').count());
        if sp > 3 {
            break;
        }
        if bytes.get(pos.saturating_add(sp)).copied() == Some(b'>') {
            depth = depth.saturating_add(1);
            pos = pos.saturating_add(sp).saturating_add(1);
            if bytes.get(pos).copied() == Some(b' ') {
                pos = pos.saturating_add(1);
            }
        } else {
            break;
        }
    }
    (depth > 0).then_some((depth, pos))
}

fn parse_alert_marker(first_line_body: &str) -> (Option<AlertKind>, Option<String>) {
    let trimmed = first_line_body.trim();
    let Some(rest) = trimmed.strip_prefix("[!") else {
        return (None, None);
    };
    let Some(close_idx) = rest.find(']') else {
        return (None, None);
    };
    let tag = rest
        .get(..close_idx)
        .unwrap_or("")
        .trim()
        .to_ascii_uppercase();
    let after = rest
        .get(close_idx.saturating_add(1)..)
        .unwrap_or("")
        .trim_start_matches(['+', '-'])
        .trim();
    let kind = match tag.as_str() {
        "NOTE" | "INFO" | "ABSTRACT" | "SUMMARY" => Some(AlertKind::Note),
        "TIP" | "HINT" | "SUCCESS" => Some(AlertKind::Tip),
        "IMPORTANT" | "QUESTION" => Some(AlertKind::Important),
        "WARNING" | "ATTENTION" => Some(AlertKind::Warning),
        "CAUTION" | "DANGER" | "BUG" | "FAILURE" | "ERROR" => Some(AlertKind::Caution),
        _ => None,
    };
    let title = (!after.is_empty()).then(|| after.to_owned());
    (kind, title)
}

fn parse_list_item(line_idx: usize, line_range: Range<usize>, line: &str) -> Option<BlockKind> {
    let mut indent = 0usize;
    let mut byte_idx = 0usize;
    for b in line.bytes() {
        match b {
            b' ' => {
                indent = indent.saturating_add(1);
                byte_idx = byte_idx.saturating_add(1);
            }
            b'\t' => {
                indent = indent.saturating_add(4);
                byte_idx = byte_idx.saturating_add(1);
            }
            _ => break,
        }
    }
    let rest = line.get(byte_idx..)?;
    let first = *rest.as_bytes().first()?;

    let (ordered, marker_end_rel) = if matches!(first, b'-' | b'*' | b'+') {
        let after = rest.get(1..)?;
        if !after.starts_with(' ') && !after.starts_with('\t') {
            return None;
        }
        let ws = after
            .bytes()
            .take_while(|&b| b == b' ' || b == b'\t')
            .count();
        (None, byte_idx.saturating_add(1).saturating_add(ws))
    } else if first.is_ascii_digit() {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if !(1..=9).contains(&digits) {
            return None;
        }
        let delim = *rest.as_bytes().get(digits)?;
        if delim != b'.' && delim != b')' {
            return None;
        }
        let after = rest.get(digits.saturating_add(1)..)?;
        if !after.starts_with(' ') && !after.starts_with('\t') {
            return None;
        }
        let num = rest.get(..digits)?.parse::<u32>().ok()?;
        let ws = after
            .bytes()
            .take_while(|&b| b == b' ' || b == b'\t')
            .count();
        (
            Some(num),
            byte_idx
                .saturating_add(digits)
                .saturating_add(1)
                .saturating_add(ws),
        )
    } else {
        return None;
    };

    // Check optional GFM task box `[ ] ` or `[x] ` / `[X] `
    let after_marker = line.get(marker_end_rel..).unwrap_or("");
    let (task, content_start_rel) = if after_marker.len() >= 3
        && after_marker.starts_with('[')
        && after_marker.as_bytes().get(2) == Some(&b']')
        && matches!(after_marker.as_bytes().get(1), Some(b' ' | b'x' | b'X'))
        && after_marker
            .as_bytes()
            .get(3)
            .is_none_or(|&b| b == b' ' || b == b'\t')
    {
        let checked = after_marker.as_bytes().get(1) != Some(&b' ');
        let bracket_start = line_range.start.saturating_add(marker_end_rel);
        let bracket_end = bracket_start.saturating_add(3);
        let trailing_ws = after_marker.get(3..).map_or(0, |s| {
            s.bytes().take_while(|&b| b == b' ' || b == b'\t').count()
        });
        (
            Some(TaskState {
                checked,
                bracket_range: bracket_start..bracket_end,
            }),
            marker_end_rel.saturating_add(3).saturating_add(trailing_ws),
        )
    } else {
        (None, marker_end_rel)
    };

    Some(BlockKind::ListItem {
        indent_spaces: indent,
        ordered,
        marker_range: line_range.start..line_range.start.saturating_add(marker_end_rel),
        task,
        content_range: line_range.start.saturating_add(content_start_rel)..line_range.end,
        line_idx,
    })
}

#[must_use]
pub fn looks_like_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains('|') && !trimmed.is_empty()
}

#[must_use]
pub fn parse_table_delimiter(line: &str) -> Option<Vec<ColumnAlign>> {
    if !looks_like_table_row(line) {
        return None;
    }
    let fake_row = parse_table_row(0, 0..line.len(), line);
    if fake_row.cells.is_empty() {
        return None;
    }
    let mut aligns = Vec::with_capacity(fake_row.cells.len());
    for cell in &fake_row.cells {
        let spec = line.get(cell.trimmed_range.clone())?;
        if spec.is_empty() {
            return None;
        }
        let left_colon = spec.starts_with(':');
        let right_colon = spec.ends_with(':');
        let core = spec
            .strip_prefix(':')
            .unwrap_or(spec)
            .strip_suffix(':')
            .unwrap_or(spec);
        // Handle single `:` cleanly
        let dashes = spec.trim_matches(':');
        if dashes.is_empty() || !dashes.bytes().all(|b| b == b'-') {
            let _ = core;
            return None;
        }
        let align = match (left_colon, right_colon) {
            (true, true) => ColumnAlign::Center,
            (false, true) => ColumnAlign::Right,
            _ => ColumnAlign::Left,
        };
        aligns.push(align);
    }
    Some(aligns)
}

#[must_use]
pub fn parse_table_row(line_idx: usize, line_range: Range<usize>, line: &str) -> TableRow {
    let bytes = line.as_bytes();
    let mut pipe_positions = Vec::new();
    let mut escaped = false;
    let mut in_code = false;
    for (idx, &b) in bytes.iter().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        if b == b'\\' {
            escaped = true;
            continue;
        }
        if b == b'`' {
            in_code = !in_code;
            continue;
        }
        if b == b'|' && !in_code {
            pipe_positions.push(idx);
        }
    }

    let lead_ws = line
        .bytes()
        .take_while(|&b| b == b' ' || b == b'\t')
        .count();
    let trail_end = line.trim_end_matches([' ', '\t']).len();

    let mut seg_bounds: Vec<(usize, usize)> = Vec::new();
    if pipe_positions.is_empty() {
        return TableRow {
            line_idx,
            byte_range: line_range,
            cells: Vec::new(),
        };
    }

    let first_pipe = pipe_positions[0];
    if first_pipe > lead_ws {
        seg_bounds.push((lead_ws, first_pipe));
    }
    for win in pipe_positions.windows(2) {
        if let [p0, p1] = *win {
            seg_bounds.push((p0.saturating_add(1), p1));
        }
    }
    if let Some(&last_pipe) = pipe_positions.last()
        && last_pipe.saturating_add(1) < trail_end
    {
        seg_bounds.push((last_pipe.saturating_add(1), trail_end));
    }

    let mut cells = Vec::with_capacity(seg_bounds.len());
    for (s_rel, e_rel) in seg_bounds {
        let raw_slice = line.get(s_rel..e_rel).unwrap_or("");
        let left_pad = raw_slice
            .bytes()
            .take_while(|&b| b == b' ' || b == b'\t')
            .count();
        let right_trimmed_len = raw_slice.trim_end_matches([' ', '\t']).len();
        let (ts_rel, te_rel) = if left_pad >= right_trimmed_len {
            // Empty cell: place editable caret stop right after one leading space (if present).
            let anchor = s_rel.saturating_add(left_pad.min(1));
            (anchor, anchor)
        } else {
            (
                s_rel.saturating_add(left_pad),
                s_rel.saturating_add(right_trimmed_len),
            )
        };
        cells.push(TableCell {
            raw_range: line_range.start.saturating_add(s_rel)
                ..line_range.start.saturating_add(e_rel),
            trimmed_range: line_range.start.saturating_add(ts_rel)
                ..line_range.start.saturating_add(te_rel),
        });
    }

    TableRow {
        line_idx,
        byte_range: line_range,
        cells,
    }
}

fn shift_usize(val: usize, delta: isize) -> usize {
    val.saturating_add_signed(delta)
}

fn shift_range(range: &mut Range<usize>, delta: isize) {
    range.start = shift_usize(range.start, delta);
    range.end = shift_usize(range.end, delta);
}

fn shift_table_row(row: &mut TableRow, byte_delta: isize, line_delta: isize) {
    row.line_idx = shift_usize(row.line_idx, line_delta);
    shift_range(&mut row.byte_range, byte_delta);
    for cell in &mut row.cells {
        shift_range(&mut cell.raw_range, byte_delta);
        shift_range(&mut cell.trimmed_range, byte_delta);
    }
}

impl Block {
    /// Shifts all byte offsets and line indices inside `self` by `(byte_delta, line_delta)`.
    pub fn shift_offsets(&mut self, byte_delta: isize, line_delta: isize) {
        shift_range(&mut self.byte_range, byte_delta);
        shift_range(&mut self.line_range, line_delta);

        match &mut self.kind {
            BlockKind::BlankLines | BlockKind::ThematicBreak => {}
            BlockKind::Heading {
                prefix_range,
                content_range,
                setext_underline,
                ..
            } => {
                shift_range(prefix_range, byte_delta);
                shift_range(content_range, byte_delta);
                if let Some(u) = setext_underline {
                    shift_range(u, byte_delta);
                }
            }
            BlockKind::Paragraph { lines } => {
                for (l_idx, r) in lines {
                    *l_idx = shift_usize(*l_idx, line_delta);
                    shift_range(r, byte_delta);
                }
            }
            BlockKind::CodeFence {
                open_line_range,
                body_lines,
                close_line_range,
                ..
            } => {
                shift_range(open_line_range, byte_delta);
                for (l_idx, r) in body_lines {
                    *l_idx = shift_usize(*l_idx, line_delta);
                    shift_range(r, byte_delta);
                }
                if let Some(close) = close_line_range {
                    shift_range(close, byte_delta);
                }
            }
            BlockKind::BlockQuote { lines, .. } => {
                for ql in lines {
                    ql.line_idx = shift_usize(ql.line_idx, line_delta);
                    shift_range(&mut ql.marker_range, byte_delta);
                    shift_range(&mut ql.content_range, byte_delta);
                }
            }
            BlockKind::ListItem {
                marker_range,
                task,
                content_range,
                line_idx,
                ..
            } => {
                *line_idx = shift_usize(*line_idx, line_delta);
                shift_range(marker_range, byte_delta);
                shift_range(content_range, byte_delta);
                if let Some(t) = task {
                    shift_range(&mut t.bracket_range, byte_delta);
                }
            }
            BlockKind::Table(tbl) => {
                shift_table_row(&mut tbl.header_row, byte_delta, line_delta);
                tbl.delimiter_line_idx = shift_usize(tbl.delimiter_line_idx, line_delta);
                shift_range(&mut tbl.delimiter_range, byte_delta);
                for br in &mut tbl.body_rows {
                    shift_table_row(br, byte_delta, line_delta);
                }
            }
        }
    }
}

/// Minimum document line count before incremental window reparsing is used instead of full parse.
const MIN_INCREMENTAL_PARSE_LINES: usize = 300;

/// Maximum changed span size (bytes) eligible for single-window incremental reparsing.
const MAX_INCREMENTAL_WINDOW_BYTES: usize = 16_384;

/// Maximum window line span eligible for incremental reparsing before falling back to full parse.
const MAX_INCREMENTAL_WINDOW_LINES: usize = 512;

/// Incrementally updates `blocks` after `old_rope` becomes `new_rope` by computing the exact
/// changed byte span via [`Rope::diff_byte_range`].
///
/// Handles both single-caret typing (`O(log N)`) and multi-site edits such as *Replace All*:
/// - Every modified line between the first and last changed byte lies strictly inside the
///   reparsed blank-line-bounded window.
/// - Blocks after the untouched trailing `BlankLines` wall are shifted by exact `(byte_delta, line_delta)`.
/// - Falls back to full [`parse_blocks`] when code fences are involved or when a scattered edit
///   spans more than [`MAX_INCREMENTAL_WINDOW_LINES`] lines / [`MAX_INCREMENTAL_WINDOW_BYTES`] bytes.
pub fn reparse_changed_rope(old_rope: &Rope, new_rope: &Rope, blocks: &mut Vec<Block>) {
    if blocks.is_empty() || new_rope.line_count() <= MIN_INCREMENTAL_PARSE_LINES {
        *blocks = parse_blocks(new_rope);
        return;
    }

    let Some((old_edit_range, new_edit_range)) = old_rope.diff_byte_range(new_rope) else {
        return;
    };

    if old_edit_range.len() > MAX_INCREMENTAL_WINDOW_BYTES
        || new_edit_range.len() > MAX_INCREMENTAL_WINDOW_BYTES
    {
        *blocks = parse_blocks(new_rope);
        return;
    }

    let Ok(old_b) = isize::try_from(old_rope.len_bytes()) else {
        *blocks = parse_blocks(new_rope);
        return;
    };
    let Ok(new_b) = isize::try_from(new_rope.len_bytes()) else {
        *blocks = parse_blocks(new_rope);
        return;
    };
    let Ok(old_l) = isize::try_from(old_rope.line_count()) else {
        *blocks = parse_blocks(new_rope);
        return;
    };
    let Ok(new_l) = isize::try_from(new_rope.line_count()) else {
        *blocks = parse_blocks(new_rope);
        return;
    };

    let byte_delta = new_b.wrapping_sub(old_b);
    let line_delta = new_l.wrapping_sub(old_l);

    let old_start_line = old_rope.byte_to_line_col(old_edit_range.start).line;
    let old_end_line = old_rope.byte_to_line_col(old_edit_range.end).line;
    let safe_before_line = old_start_line.saturating_sub(1);
    let safe_after_line = (old_end_line.saturating_add(2)).min(old_rope.line_count());

    // Expand backward until `first_idx == 0` or `blocks[first_idx]` is a completely untouched `BlankLines` wall.
    let mut first_idx = blocks
        .partition_point(|b| b.line_range.end <= safe_before_line)
        .min(blocks.len().saturating_sub(1));
    while first_idx > 0
        && !(matches!(blocks[first_idx].kind, BlockKind::BlankLines)
            && blocks[first_idx].line_range.end <= safe_before_line)
    {
        first_idx = first_idx.saturating_sub(1);
    }

    // Expand forward until `end_idx == blocks.len()` or `blocks[end_idx - 1]` is an untouched `BlankLines` wall.
    let mut end_idx = blocks
        .partition_point(|b| b.line_range.start < safe_after_line)
        .max(first_idx.saturating_add(1))
        .min(blocks.len());
    while end_idx < blocks.len()
        && !(matches!(
            blocks[end_idx.saturating_sub(1)].kind,
            BlockKind::BlankLines
        ) && blocks[end_idx.saturating_sub(1)].line_range.start >= safe_after_line)
    {
        end_idx = end_idx.saturating_add(1);
    }

    let win_line_start = blocks[first_idx].line_range.start;
    let old_win_line_end = blocks[end_idx.saturating_sub(1)].line_range.end;
    let new_win_line_end = shift_usize(old_win_line_end, line_delta).min(new_rope.line_count());
    if new_win_line_end < win_line_start
        || new_win_line_end.saturating_sub(win_line_start) > MAX_INCREMENTAL_WINDOW_LINES
    {
        *blocks = parse_blocks(new_rope);
        return;
    }

    // If old window contains a CodeFence, fall back to full parse in case fences were opened/closed.
    if blocks[first_idx..end_idx]
        .iter()
        .any(|b| matches!(b.kind, BlockKind::CodeFence { .. }))
    {
        *blocks = parse_blocks(new_rope);
        return;
    }

    let replacement = parse_blocks_in_line_range(new_rope, win_line_start..new_win_line_end);
    if replacement
        .iter()
        .any(|b| matches!(b.kind, BlockKind::CodeFence { .. }))
    {
        *blocks = parse_blocks(new_rope);
        return;
    }

    for tail_blk in &mut blocks[end_idx..] {
        tail_blk.shift_offsets(byte_delta, line_delta);
    }
    blocks.splice(first_idx..end_idx, replacement);
}

/// Incrementally updates `blocks` after `old_rope` becomes `new_rope`.
///
/// Kept for API compatibility; ignores `_edit_hint_byte` in favor of [`reparse_changed_rope`]'s
/// exact B-tree diff window so multi-site edits cannot go out of sync.
pub fn reparse_incremental(
    old_rope: &Rope,
    new_rope: &Rope,
    blocks: &mut Vec<Block>,
    _edit_hint_byte: usize,
) {
    reparse_changed_rope(old_rope, new_rope, blocks);
}
