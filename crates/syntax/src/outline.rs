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
            let raw = rope.slice_to_string(content_range.clone());
            let clean = strip_inline_syntax(&raw);
            let title = if clean.trim().is_empty() {
                format!("(Überschrift H{level})")
            } else {
                clean.trim().to_owned()
            };
            let slug = heading_slug(&title);
            entries.push(OutlineEntry {
                level,
                title,
                slug,
                line: block.line_range.start,
                byte_offset: content_range.start,
            });
        }
    }
    entries
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
