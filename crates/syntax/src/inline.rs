use std::ops::Range;

/// Bitset of active inline styles on a text token.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct StyleFlags(pub u16);

impl StyleFlags {
    pub const NONE: Self = Self(0);
    pub const BOLD: Self = Self(1 << 0);
    pub const ITALIC: Self = Self(1 << 1);
    pub const STRIKE: Self = Self(1 << 2);
    pub const HIGHLIGHT: Self = Self(1 << 3);
    pub const CODE: Self = Self(1 << 4);
    pub const LINK: Self = Self(1 << 5);
    pub const KBD: Self = Self(1 << 6);
    pub const MUTED: Self = Self(1 << 7);

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// Optional width specification for inline Markdown images (`{width=50%}` or `{width=320px}`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ImageSize {
    /// Relative width in percent of the available content column (`5..=100`).
    Percent(u16),
    /// Explicit width in logical pixels (`16..=4096`).
    Pixels(u16),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InlineTokenKind {
    /// Visible content glyphs.
    Text,
    /// Markdown delimiter (`**`, `*`, `~~`, `==`, `` ` ``, `[`, `](...)`, `<kbd>`, `</kbd>`).
    /// Hidden (`0 px`) in WYSIWYG mode, visible as muted syntax on raw/live/source lines.
    SyntaxMarker,
    /// Link label text carrying its target URL for Ctrl+Click navigation.
    LinkText { url: String },
    /// Inline image `![alt](src "caption"){width=50%}` — rendered as image + caption in WYSIWYG.
    Image {
        alt: String,
        src: String,
        caption: Option<String>,
        size: Option<ImageSize>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineToken {
    /// Absolute document byte range (`base_offset + rel_start .. base_offset + rel_end`).
    pub range: Range<usize>,
    pub kind: InlineTokenKind,
    pub style: StyleFlags,
}

/// Tokenizes `text` (which starts at `base_offset` in the document) into contiguous,
/// non-overlapping [`InlineToken`]s covering every byte of `text`.
#[must_use]
pub fn parse_inline_tokens(base_offset: usize, text: &str) -> Vec<InlineToken> {
    let mut out = Vec::new();
    parse_inline_recursive(base_offset, text, StyleFlags::NONE, &mut out);
    merge_adjacent_text_tokens(&mut out);
    out
}

/// Queries which [`StyleFlags`] are active at `caret_offset` inside `(line_start, line_text)`.
#[must_use]
pub fn active_styles_at(line_start: usize, line_text: &str, caret_offset: usize) -> StyleFlags {
    let tokens = parse_inline_tokens(line_start, line_text);
    for tok in &tokens {
        if matches!(
            tok.kind,
            InlineTokenKind::Text | InlineTokenKind::LinkText { .. }
        ) && caret_offset >= tok.range.start
            && caret_offset <= tok.range.end
        {
            return tok.style;
        }
    }
    StyleFlags::NONE
}

fn parse_inline_recursive(
    base: usize,
    text: &str,
    inherited: StyleFlags,
    out: &mut Vec<InlineToken>,
) {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut pos = 0usize;
    let mut plain_start = 0usize;

    let flush_plain = |from: usize, to: usize, out: &mut Vec<InlineToken>| {
        if to > from {
            out.push(InlineToken {
                range: base.saturating_add(from)..base.saturating_add(to),
                kind: InlineTokenKind::Text,
                style: inherited,
            });
        }
    };

    while pos < len {
        let b = bytes[pos];

        // 1. Backslash escape `\*`
        if b == b'\\'
            && pos.saturating_add(1) < len
            && bytes[pos.saturating_add(1)].is_ascii_punctuation()
        {
            flush_plain(plain_start, pos, out);
            out.push(InlineToken {
                range: base.saturating_add(pos)..base.saturating_add(pos.saturating_add(1)),
                kind: InlineTokenKind::SyntaxMarker,
                style: inherited.union(StyleFlags::MUTED),
            });
            let escaped_char_end = next_char_end(text, pos.saturating_add(1));
            out.push(InlineToken {
                range: base.saturating_add(pos.saturating_add(1))
                    ..base.saturating_add(escaped_char_end),
                kind: InlineTokenKind::Text,
                style: inherited,
            });
            pos = escaped_char_end;
            plain_start = pos;
            continue;
        }

        // 2. Inline code span `` `...` ``
        if b == b'`' {
            let ticks = bytes
                .get(pos..)
                .map_or(0, |s| s.iter().take_while(|&&c| c == b'`').count());
            if let Some(close_rel) =
                find_closing_backticks(text.get(pos.saturating_add(ticks)..).unwrap_or(""), ticks)
            {
                flush_plain(plain_start, pos, out);
                let content_start = pos.saturating_add(ticks);
                let content_end = content_start.saturating_add(close_rel);
                let close_end = content_end.saturating_add(ticks);

                out.push(InlineToken {
                    range: base.saturating_add(pos)..base.saturating_add(content_start),
                    kind: InlineTokenKind::SyntaxMarker,
                    style: inherited.union(StyleFlags::MUTED),
                });
                if content_end > content_start {
                    out.push(InlineToken {
                        range: base.saturating_add(content_start)..base.saturating_add(content_end),
                        kind: InlineTokenKind::Text,
                        style: inherited.union(StyleFlags::CODE),
                    });
                }
                out.push(InlineToken {
                    range: base.saturating_add(content_end)..base.saturating_add(close_end),
                    kind: InlineTokenKind::SyntaxMarker,
                    style: inherited.union(StyleFlags::MUTED),
                });
                pos = close_end;
                plain_start = pos;
                continue;
            }
        }

        // 3. Inline image `![alt](src "caption"){width=50%}`
        if b == b'!'
            && bytes.get(pos.saturating_add(1)) == Some(&b'[')
            && let Some((label_end, url_start, url_end, paren_end)) =
                parse_bracket_paren(text, pos.saturating_add(1))
        {
            flush_plain(plain_start, pos, out);
            let alt = unescape_markdown_brackets(
                text.get(pos.saturating_add(2)..label_end).unwrap_or(""),
            );
            let inner = text.get(url_start..url_end).unwrap_or("");
            let (src, caption) = parse_image_dest_and_title(inner);
            let (size, attr_len) = text
                .get(paren_end..)
                .and_then(parse_image_attr_block)
                .map_or((None, 0), |(sz, len)| (Some(sz), len));
            let total_end = paren_end.saturating_add(attr_len);
            out.push(InlineToken {
                range: base.saturating_add(pos)..base.saturating_add(total_end),
                kind: InlineTokenKind::Image {
                    alt,
                    src,
                    caption,
                    size,
                },
                style: inherited,
            });
            pos = total_end;
            plain_start = pos;
            continue;
        }

        // 4. Inline link `[label](url)`
        if b == b'['
            && let Some((label_end, url_start, url_end, total_end)) = parse_bracket_paren(text, pos)
        {
            flush_plain(plain_start, pos, out);
            let url = text.get(url_start..url_end).unwrap_or("").trim().to_owned();
            let label_start = pos.saturating_add(1);

            out.push(InlineToken {
                range: base.saturating_add(pos)..base.saturating_add(label_start),
                kind: InlineTokenKind::SyntaxMarker,
                style: inherited.union(StyleFlags::MUTED),
            });
            if label_end > label_start {
                out.push(InlineToken {
                    range: base.saturating_add(label_start)..base.saturating_add(label_end),
                    kind: InlineTokenKind::LinkText { url },
                    style: inherited.union(StyleFlags::LINK),
                });
            }
            out.push(InlineToken {
                range: base.saturating_add(label_end)..base.saturating_add(total_end),
                kind: InlineTokenKind::SyntaxMarker,
                style: inherited.union(StyleFlags::MUTED),
            });
            pos = total_end;
            plain_start = pos;
            continue;
        }

        // 5. `<kbd>...</kbd>`
        if b == b'<'
            && text.get(pos..).is_some_and(|s| {
                s.len() >= 11 && s.get(..5).is_some_and(|h| h.eq_ignore_ascii_case("<kbd>"))
            })
            && let Some(after_open) = text.get(pos.saturating_add(5)..)
            && let Some(close_rel) = find_ascii_case_insensitive(after_open, "</kbd>")
            && close_rel > 0
        {
            flush_plain(plain_start, pos, out);
            let inner_start = pos.saturating_add(5);
            let inner_end = inner_start.saturating_add(close_rel);
            let close_end = inner_end.saturating_add(6);

            out.push(InlineToken {
                range: base.saturating_add(pos)..base.saturating_add(inner_start),
                kind: InlineTokenKind::SyntaxMarker,
                style: inherited.union(StyleFlags::MUTED),
            });
            out.push(InlineToken {
                range: base.saturating_add(inner_start)..base.saturating_add(inner_end),
                kind: InlineTokenKind::Text,
                style: inherited.union(StyleFlags::KBD),
            });
            out.push(InlineToken {
                range: base.saturating_add(inner_end)..base.saturating_add(close_end),
                kind: InlineTokenKind::SyntaxMarker,
                style: inherited.union(StyleFlags::MUTED),
            });
            pos = close_end;
            plain_start = pos;
            continue;
        }

        // 6. Paired symmetric delimiters: `***`, `**`, `*`, `~~`, `==`
        if let Some((delim, added_style)) = match_paired_delimiter(text, pos)
            && let Some(close_rel) = find_closing_delimiter(
                text.get(pos.saturating_add(delim.len())..).unwrap_or(""),
                delim,
            )
        {
            flush_plain(plain_start, pos, out);
            let inner_start = pos.saturating_add(delim.len());
            let inner_end = inner_start.saturating_add(close_rel);
            let close_end = inner_end.saturating_add(delim.len());

            out.push(InlineToken {
                range: base.saturating_add(pos)..base.saturating_add(inner_start),
                kind: InlineTokenKind::SyntaxMarker,
                style: inherited.union(StyleFlags::MUTED),
            });
            if let Some(inner_str) = text.get(inner_start..inner_end) {
                parse_inline_recursive(
                    base.saturating_add(inner_start),
                    inner_str,
                    inherited.union(added_style),
                    out,
                );
            }
            out.push(InlineToken {
                range: base.saturating_add(inner_end)..base.saturating_add(close_end),
                kind: InlineTokenKind::SyntaxMarker,
                style: inherited.union(StyleFlags::MUTED),
            });
            pos = close_end;
            plain_start = pos;
            continue;
        }

        pos = next_char_end(text, pos);
    }

    flush_plain(plain_start, len, out);
}

fn next_char_end(text: &str, pos: usize) -> usize {
    let mut next = pos.saturating_add(1);
    while next < text.len() && !text.is_char_boundary(next) {
        next = next.saturating_add(1);
    }
    next.min(text.len())
}

fn find_closing_backticks(after: &str, ticks: usize) -> Option<usize> {
    let bytes = after.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            let run = bytes
                .get(i..)
                .map_or(0, |s| s.iter().take_while(|&&c| c == b'`').count());
            if run == ticks {
                return Some(i);
            }
            i = i.saturating_add(run);
        } else {
            i = i.saturating_add(1);
        }
    }
    None
}

/// Formats an inline Markdown image cleanly with optional CommonMark `"caption"` and
/// Pandoc/MkDocs/Zensical `{width=…}` size attribute.
#[must_use]
pub fn format_markdown_image(
    alt: &str,
    src: &str,
    caption: Option<&str>,
    size: Option<ImageSize>,
) -> String {
    let mut clean_alt = String::with_capacity(alt.len());
    for ch in alt.chars() {
        match ch {
            '\r' | '\n' => clean_alt.push(' '),
            ']' => clean_alt.push_str("\\]"),
            _ => clean_alt.push(ch),
        }
    }

    let trimmed_src = src.trim();
    let clean_src: String = trimmed_src
        .chars()
        .map(|c| if c == '\r' || c == '\n' { ' ' } else { c })
        .collect();
    let needs_angles = !(clean_src.starts_with('<') && clean_src.ends_with('>'))
        && (clean_src.contains(' ') || clean_src.contains('(') || clean_src.contains(')'));
    let formatted_dest = if needs_angles {
        format!("<{clean_src}>")
    } else {
        clean_src
    };

    let mut out = format!("![{clean_alt}]({formatted_dest}");

    if let Some(cap) = caption.map(str::trim).filter(|s| !s.is_empty()) {
        out.push_str(" \"");
        for ch in cap.chars() {
            match ch {
                '\r' | '\n' => out.push(' '),
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                _ => out.push(ch),
            }
        }
        out.push('"');
    }

    out.push(')');

    match size {
        Some(ImageSize::Percent(pct)) => {
            let clamped = pct.clamp(5, 100);
            out.push_str(&format!("{{width={clamped}%}}"));
        }
        Some(ImageSize::Pixels(px)) => {
            let clamped = px.clamp(16, 4096);
            out.push_str(&format!("{{width={clamped}px}}"));
        }
        None => {}
    }

    out
}

fn unescape_markdown_brackets(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            } else {
                out.push('\\');
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn parse_image_dest_and_title(inner: &str) -> (String, Option<String>) {
    let trimmed = inner.trim();
    if trimmed.is_empty() {
        return (String::new(), None);
    }

    // Case 1: `<destination>` optionally followed by `"caption"` or `'caption'`
    if let Some(after_lt) = trimmed.strip_prefix('<')
        && let Some(gt_pos) = after_lt.find('>')
    {
        let dest = after_lt.get(..gt_pos).unwrap_or("").trim().to_owned();
        let rest = after_lt
            .get(gt_pos.saturating_add(1)..)
            .unwrap_or("")
            .trim();
        let caption = parse_quoted_title(rest);
        return (dest, caption);
    }

    // Case 2: Trailing `"caption"` or `'caption'` separated by whitespace from destination
    if let Some((dest_part, cap)) = split_trailing_quoted_title(trimmed) {
        return (dest_part.trim().to_owned(), cap);
    }

    (trimmed.to_owned(), None)
}

fn parse_quoted_title(s: &str) -> Option<String> {
    if s.len() < 2 {
        return None;
    }
    let quote = s.as_bytes().first().copied()?;
    if (quote != b'"' && quote != b'\'') || s.as_bytes().last().copied() != Some(quote) {
        return None;
    }
    let inner = s.get(1..s.len().saturating_sub(1))?;
    let unescaped = unescape_quoted_title(inner);
    let clean = unescaped.trim();
    if clean.is_empty() {
        None
    } else {
        Some(clean.to_owned())
    }
}

fn split_trailing_quoted_title(trimmed: &str) -> Option<(&str, Option<String>)> {
    let bytes = trimmed.as_bytes();
    let quote = *bytes.last()?;
    if quote != b'"' && quote != b'\'' {
        return None;
    }
    // Scan forward from first whitespace to find an unescaped opening quote whose matching close is `bytes.len() - 1`
    for (idx, ch) in trimmed.char_indices() {
        if (ch == ' ' || ch == '\t')
            && let Some(after_ws) = trimmed.get(idx..)
        {
            let ws_len = after_ws.len().saturating_sub(after_ws.trim_start().len());
            let q_open = idx.saturating_add(ws_len);
            if bytes.get(q_open) == Some(&quote) && q_open.saturating_add(1) < bytes.len() {
                // Verify no unescaped closing `quote` appears before the very end
                let mut k = q_open.saturating_add(1);
                let mut valid = true;
                while k < bytes.len().saturating_sub(1) {
                    if bytes[k] == b'\\' {
                        k = k.saturating_add(2);
                        continue;
                    }
                    if bytes[k] == quote {
                        valid = false;
                        break;
                    }
                    k = k.saturating_add(1);
                }
                if valid {
                    let dest = trimmed.get(..idx)?;
                    let raw_title =
                        trimmed.get(q_open.saturating_add(1)..bytes.len().saturating_sub(1))?;
                    let cap = unescape_quoted_title(raw_title);
                    let clean_cap = cap.trim();
                    let opt_cap = (!clean_cap.is_empty()).then(|| clean_cap.to_owned());
                    return Some((dest, opt_cap));
                }
            }
        }
    }
    None
}

fn unescape_quoted_title(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            } else {
                out.push('\\');
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn parse_image_attr_block(after_paren: &str) -> Option<(ImageSize, usize)> {
    let body = after_paren.strip_prefix('{')?;
    let close_rel = body.find('}')?;
    if close_rel > 80 {
        return None;
    }
    let inside = body.get(..close_rel)?.trim();
    if inside.is_empty() || inside.contains('\n') {
        return None;
    }
    let consumed = close_rel.saturating_add(2);

    // Support `{50%}` shorthand or `{width=50%}` / `{ width="50%" }` / `{width: 320px}`
    for token in inside.split_whitespace() {
        let val_raw = if let Some(rest) = token.strip_prefix("width=") {
            rest
        } else if let Some(rest) = token.strip_prefix("width:") {
            rest
        } else if token.ends_with('%') && token.len() >= 2 {
            token
        } else {
            continue;
        };
        let val = val_raw.trim_matches(|c: char| c == '"' || c == '\'' || c == ',' || c == ';');
        if let Some(pct_str) = val.strip_suffix('%')
            && let Ok(pct) = pct_str.trim().parse::<u16>()
        {
            return Some((ImageSize::Percent(pct.clamp(5, 100)), consumed));
        }
        if let Some(px_str) = val.strip_suffix("px")
            && let Ok(px) = px_str.trim().parse::<u16>()
        {
            return Some((ImageSize::Pixels(px.clamp(16, 4096)), consumed));
        }
        if let Ok(px) = val.trim().parse::<u16>() {
            return Some((ImageSize::Pixels(px.clamp(16, 4096)), consumed));
        }
    }
    None
}

fn parse_bracket_paren(text: &str, open_bracket: usize) -> Option<(usize, usize, usize, usize)> {
    let bytes = text.as_bytes();
    let mut i = open_bracket.saturating_add(1);
    let mut depth = 1usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                i = i.saturating_add(2);
                continue;
            }
            b'`' => return None,
            b'[' => depth = depth.saturating_add(1),
            b']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        i = i.saturating_add(1);
    }
    if depth != 0 || bytes.get(i.saturating_add(1)) != Some(&b'(') {
        return None;
    }
    let label_end = i;
    let url_start = i.saturating_add(2);
    let mut j = url_start;
    let mut pdepth = 1usize;
    let mut in_quote: Option<u8> = None;
    let mut in_angles = false;

    while j < bytes.len() {
        let c = bytes[j];
        if c == b'\\' {
            j = j.saturating_add(2);
            continue;
        }
        if let Some(q) = in_quote {
            if c == q {
                in_quote = None;
            }
            j = j.saturating_add(1);
            continue;
        }
        if in_angles {
            if c == b'>' {
                in_angles = false;
            }
            j = j.saturating_add(1);
            continue;
        }
        match c {
            b'<' if j == url_start || bytes.get(j.saturating_sub(1)) == Some(&b' ') => {
                in_angles = true;
            }
            b'"' | b'\''
                if j > url_start
                    && matches!(bytes.get(j.saturating_sub(1)), Some(b' ' | b'\t')) =>
            {
                in_quote = Some(c);
            }
            b'(' => pdepth = pdepth.saturating_add(1),
            b')' => {
                pdepth = pdepth.saturating_sub(1);
                if pdepth == 0 {
                    return Some((label_end, url_start, j, j.saturating_add(1)));
                }
            }
            _ => {}
        }
        j = j.saturating_add(1);
    }
    None
}

fn find_ascii_case_insensitive(haystack: &str, needle_lower: &str) -> Option<usize> {
    let nlen = needle_lower.len();
    if haystack.len() < nlen {
        return None;
    }
    for i in 0..=haystack.len().saturating_sub(nlen) {
        if haystack
            .as_bytes()
            .get(i..i.saturating_add(nlen))
            .is_some_and(|w| w.eq_ignore_ascii_case(needle_lower.as_bytes()))
        {
            return Some(i);
        }
    }
    None
}

fn match_paired_delimiter(text: &str, pos: usize) -> Option<(&'static str, StyleFlags)> {
    let rest = text.get(pos..)?;
    let candidates: &[(&str, StyleFlags)] = &[
        ("***", StyleFlags::BOLD.union(StyleFlags::ITALIC)),
        ("**", StyleFlags::BOLD),
        ("~~", StyleFlags::STRIKE),
        ("==", StyleFlags::HIGHLIGHT),
        ("*", StyleFlags::ITALIC),
    ];
    for &(delim, flags) in candidates {
        if let Some(after) = rest.strip_prefix(delim) {
            // Opener must not be followed by whitespace or empty.
            if after.is_empty() || after.starts_with(' ') || after.starts_with('\t') {
                continue;
            }
            // Single `*` must not match inside `**`.
            if delim == "*" && after.starts_with('*') {
                continue;
            }
            return Some((delim, flags));
        }
    }
    None
}

fn find_closing_delimiter(after_open: &str, delim: &str) -> Option<usize> {
    let bytes = after_open.as_bytes();
    let dbytes = delim.as_bytes();
    let mut i = 0usize;
    while i.saturating_add(dbytes.len()) <= bytes.len() {
        if bytes[i] == b'\\' {
            i = i.saturating_add(2);
            continue;
        }
        if bytes[i] == b'`' {
            return None;
        }
        if bytes.get(i..i.saturating_add(dbytes.len())) == Some(dbytes) {
            if i == 0 {
                return None;
            }
            let prev = bytes[i.saturating_sub(1)];
            if prev != b' ' && prev != b'\t' {
                if delim == "*" && bytes.get(i.saturating_add(1)) == Some(&b'*') {
                    // Single `*` closer must not steal from a `**` run.
                    let run = bytes
                        .get(i..)
                        .map_or(2, |s| s.iter().take_while(|&&c| c == b'*').count());
                    i = i.saturating_add(run);
                    continue;
                }
                if delim == "**" && bytes.get(i.saturating_add(2)) == Some(&b'*') {
                    // In `**bold *and italic***`, let the inner `*` take the first `*` and close `**` on the last two!
                    let run = bytes
                        .get(i..)
                        .map_or(3, |s| s.iter().take_while(|&&c| c == b'*').count());
                    return Some(i.saturating_add(run.saturating_sub(2)));
                }
                return Some(i);
            }
        }
        i = i.saturating_add(1);
    }
    None
}

fn merge_adjacent_text_tokens(tokens: &mut Vec<InlineToken>) {
    let mut write = 0usize;
    for read in 0..tokens.len() {
        if write > 0 {
            let can_merge = matches!(
                (&tokens[write - 1].kind, &tokens[read].kind),
                (InlineTokenKind::Text, InlineTokenKind::Text)
            ) && tokens[write - 1].style == tokens[read].style
                && tokens[write - 1].range.end == tokens[read].range.start;
            if can_merge {
                tokens[write - 1].range.end = tokens[read].range.end;
                continue;
            }
        }
        if write != read {
            tokens[write] = tokens[read].clone();
        }
        write = write.saturating_add(1);
    }
    tokens.truncate(write);
}
