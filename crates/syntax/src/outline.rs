use kmark_buffer::Rope;

use crate::blocks::{Block, BlockKind};
use crate::inline::{InlineTokenKind, parse_inline_tokens};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OutlineEntry {
    pub level: u8,
    pub title: String,
    pub slug: String,
    pub line: usize,
    pub byte_offset: usize,
}

fn build_outline_entry(
    rope: &Rope,
    level: u8,
    content_range: &std::ops::Range<usize>,
    line: usize,
) -> OutlineEntry {
    let raw = rope.slice_to_string(content_range.clone());
    let clean = strip_inline_syntax(&raw);
    let title = if clean.trim().is_empty() {
        format!("(Überschrift H{level})")
    } else {
        clean.trim().to_owned()
    };
    let slug = heading_slug(&title);
    OutlineEntry {
        level,
        title,
        slug,
        line,
        byte_offset: content_range.start,
    }
}

/// Extracts document headings for the dockable KDE Outline tree and `#anchor` link jumps.
#[must_use]
pub fn extract_outline(rope: &Rope, blocks: &[Block]) -> Vec<OutlineEntry> {
    let mut entries = Vec::new();
    for block in blocks {
        if let BlockKind::Heading {
            level,
            ref content_range,
            ..
        } = block.kind
        {
            entries.push(build_outline_entry(
                rope,
                level,
                content_range,
                block.line_range.start,
            ));
        }
    }
    entries
}

/// Incrementally updates `outline` after `old_rope` becomes `new_rope` with `new_blocks`,
/// reusing existing `(title, slug)` allocations for untouched prefix and suffix headings
/// without slicing `new_rope` or re-tokenizing inline Markdown.
pub fn update_outline_changed_rope(
    old_rope: &Rope,
    new_rope: &Rope,
    new_blocks: &[Block],
    outline: &mut Vec<OutlineEntry>,
) {
    let Some((old_edit_range, new_edit_range)) = old_rope.diff_byte_range(new_rope) else {
        return;
    };

    let prev = std::mem::take(outline);
    let mut prev_iter = prev.into_iter().peekable();
    let mut next = Vec::with_capacity(prev_iter.len());

    for block in new_blocks {
        let BlockKind::Heading {
            level,
            ref content_range,
            ..
        } = block.kind
        else {
            continue;
        };

        // Case 1: Heading lies strictly inside the unchanged prefix (`< old_edit_range.start`).
        if block.byte_range.end < new_edit_range.start {
            while prev_iter
                .peek()
                .is_some_and(|e| e.byte_offset < content_range.start)
            {
                let _ = prev_iter.next();
            }
            if prev_iter
                .peek()
                .is_some_and(|e| e.byte_offset == content_range.start && e.level == level)
                && let Some(mut existing) = prev_iter.next()
            {
                existing.line = block.line_range.start;
                next.push(existing);
                continue;
            }
        }
        // Case 2: Heading lies strictly inside the unchanged suffix (`> new_edit_range.end`).
        else if block.byte_range.start > new_edit_range.end {
            let suffix_rel = content_range.start.saturating_sub(new_edit_range.end);
            let old_offset = old_edit_range.end.saturating_add(suffix_rel);
            while prev_iter.peek().is_some_and(|e| e.byte_offset < old_offset) {
                let _ = prev_iter.next();
            }
            if prev_iter
                .peek()
                .is_some_and(|e| e.byte_offset == old_offset && e.level == level)
                && let Some(mut existing) = prev_iter.next()
            {
                existing.line = block.line_range.start;
                existing.byte_offset = content_range.start;
                next.push(existing);
                continue;
            }
        }

        // Case 3: Heading overlaps the edited span (or entered/left a code fence).
        next.push(build_outline_entry(
            new_rope,
            level,
            content_range,
            block.line_range.start,
        ));
    }

    *outline = next;
}

/// Strips inline Markdown delimiters to produce readable heading titles.
#[must_use]
pub fn strip_inline_syntax(raw: &str) -> String {
    let tokens = parse_inline_tokens(0, raw);
    let mut out = String::with_capacity(raw.len());
    for tok in tokens {
        match tok.kind {
            InlineTokenKind::Text | InlineTokenKind::LinkText { .. } => {
                if let Some(sub) = raw.get(tok.range) {
                    out.push_str(sub);
                }
            }
            InlineTokenKind::Image { alt, .. } => out.push_str(&alt),
            InlineTokenKind::SyntaxMarker => {}
        }
    }
    out
}

/// GitHub-compatible heading anchor slug (`#mein-abschnitt`).
#[must_use]
pub fn heading_slug(title: &str) -> String {
    let mut slug = String::with_capacity(title.len());
    for ch in title.chars() {
        if ch.is_alphanumeric() {
            for lc in ch.to_lowercase() {
                slug.push(lc);
            }
        } else if ch == ' ' || ch == '-' || ch == '_' {
            slug.push('-');
        }
    }
    slug
}
