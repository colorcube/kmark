use std::ops::Range;
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

const MIN_LEAF_BYTES: usize = 512;
const MAX_LEAF_BYTES: usize = 1536;
const MIN_CHILDREN: usize = 4;
const MAX_CHILDREN: usize = 12;

/// Aggregated metrics stored at every node of the B-tree rope.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub bytes: usize,
    pub chars: usize,
    pub utf16_units: usize,
    pub newlines: usize,
}

impl Summary {
    #[must_use]
    pub fn of_str(text: &str) -> Self {
        let bytes = text.len();
        let mut chars = 0usize;
        let mut utf16_units = 0usize;
        let mut newlines = 0usize;
        for ch in text.chars() {
            chars = chars.saturating_add(1);
            utf16_units = utf16_units.saturating_add(ch.len_utf16());
            if ch == '\n' {
                newlines = newlines.saturating_add(1);
            }
        }
        Self {
            bytes,
            chars,
            utf16_units,
            newlines,
        }
    }

    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn add(self, other: Self) -> Self {
        Self {
            bytes: self.bytes.saturating_add(other.bytes),
            chars: self.chars.saturating_add(other.chars),
            utf16_units: self.utf16_units.saturating_add(other.utf16_units),
            newlines: self.newlines.saturating_add(other.newlines),
        }
    }
}

#[derive(Clone, Debug)]
enum Node {
    Leaf {
        text: String,
        summary: Summary,
    },
    Internal {
        children: Vec<Arc<Node>>,
        summary: Summary,
        height: u8,
    },
}

impl Node {
    fn leaf(text: String) -> Arc<Self> {
        let summary = Summary::of_str(&text);
        Arc::new(Self::Leaf { text, summary })
    }

    fn internal(children: Vec<Arc<Self>>) -> Arc<Self> {
        let mut summary = Summary::default();
        let mut child_height = 0u8;
        for child in &children {
            summary = summary.add(child.summary());
            child_height = child_height.max(child.height());
        }
        Arc::new(Self::Internal {
            children,
            summary,
            height: child_height.saturating_add(1),
        })
    }

    fn summary(&self) -> Summary {
        match self {
            Self::Leaf { summary, .. } | Self::Internal { summary, .. } => *summary,
        }
    }

    fn height(&self) -> u8 {
        match self {
            Self::Leaf { .. } => 0,
            Self::Internal { height, .. } => *height,
        }
    }
}

/// 0-indexed line and column position within a [`Rope`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LineCol {
    pub line: usize,
    pub col_bytes: usize,
    pub col_chars: usize,
}

/// Persistent copy-on-write B-tree rope over UTF-8 text.
///
/// Cloning a `Rope` is `O(1)` (`Arc` increment), enabling zero-cost undo snapshots
/// and background search snapshots while keeping edits `O(log N)`.
#[derive(Clone, Debug)]
pub struct Rope {
    root: Arc<Node>,
}

impl Default for Rope {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for Rope {
    fn eq(&self, other: &Self) -> bool {
        if Arc::ptr_eq(&self.root, &other.root) {
            return true;
        }
        if self.summary() != other.summary() {
            return false;
        }
        common_prefix_bytes(&self.root, &other.root) == self.len_bytes()
    }
}

impl Eq for Rope {}

/// Streaming iterator over non-empty UTF-8 leaf slices in a [`Rope`] (zero String allocations).
pub struct Chunks<'a> {
    stack: Vec<&'a Arc<Node>>,
}

impl<'a> Iterator for Chunks<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(node) = self.stack.pop() {
            match node.as_ref() {
                Node::Leaf { text, .. } => {
                    if !text.is_empty() {
                        return Some(text.as_str());
                    }
                }
                Node::Internal { children, .. } => {
                    for child in children.iter().rev() {
                        self.stack.push(child);
                    }
                }
            }
        }
        None
    }
}

impl Rope {
    #[must_use]
    pub fn new() -> Self {
        Self {
            root: Node::leaf(String::new()),
        }
    }

    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(text: &str) -> Self {
        if text.is_empty() {
            return Self::new();
        }
        let leaves = split_str_into_leaves(text);
        Self {
            root: build_tree_from_nodes(leaves),
        }
    }

    /// Returns `true` if `self` and `other` share the exact same root B-tree node (`O(1)`).
    #[must_use]
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.root, &other.root)
    }

    /// Iterates over leaf `&str` chunks from start to end without heap allocations.
    #[must_use]
    pub fn chunks(&self) -> Chunks<'_> {
        Chunks {
            stack: vec![&self.root],
        }
    }

    /// Computes the minimal UTF-8-aligned changed window `(old_range, new_range)` turning `self`
    /// into `other`, skipping shared Copy-on-Write B-tree subtrees in `O(log N)`.
    ///
    /// Returns `None` if `self` and `other` contain identical text.
    #[must_use]
    pub fn diff_byte_range(&self, other: &Self) -> Option<(Range<usize>, Range<usize>)> {
        if Arc::ptr_eq(&self.root, &other.root) {
            return None;
        }
        let old_len = self.len_bytes();
        let new_len = other.len_bytes();
        let raw_prefix = common_prefix_bytes(&self.root, &other.root);
        if raw_prefix == old_len && raw_prefix == new_len {
            return None;
        }

        let start = self.clamp_char_boundary(raw_prefix);
        let max_suffix = old_len.min(new_len).saturating_sub(start);
        let mut raw_suffix = common_suffix_bytes(&self.root, &other.root, max_suffix);

        while raw_suffix > 0
            && (!self.is_char_boundary(old_len.saturating_sub(raw_suffix))
                || !other.is_char_boundary(new_len.saturating_sub(raw_suffix)))
        {
            raw_suffix = raw_suffix.saturating_sub(1);
        }

        let old_end = old_len.saturating_sub(raw_suffix).max(start);
        let new_end = new_len.saturating_sub(raw_suffix).max(start);
        Some((start..old_end, start..new_end))
    }

    #[must_use]
    pub fn summary(&self) -> Summary {
        self.root.summary()
    }

    #[must_use]
    pub fn len_bytes(&self) -> usize {
        self.root.summary().bytes
    }

    #[must_use]
    pub fn len_chars(&self) -> usize {
        self.root.summary().chars
    }

    #[must_use]
    pub fn len_utf16(&self) -> usize {
        self.root.summary().utf16_units
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len_bytes() == 0
    }

    /// Number of logical lines (`>= 1` even when empty).
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.root.summary().newlines.saturating_add(1)
    }

    /// Clamps `offset` into `0..=len_bytes()` and snaps left onto a valid UTF-8 char boundary.
    #[must_use]
    pub fn clamp_char_boundary(&self, offset: usize) -> usize {
        let clamped = offset.min(self.len_bytes());
        clamp_node_char_boundary(&self.root, clamped)
    }

    /// Returns `true` if `offset` lies on a valid UTF-8 boundary (`0..=len_bytes()`).
    #[must_use]
    pub fn is_char_boundary(&self, offset: usize) -> bool {
        offset <= self.len_bytes() && self.clamp_char_boundary(offset) == offset
    }

    /// Extracts a UTF-8 `String` for `range` (clamped safely to valid char boundaries).
    #[must_use]
    pub fn slice_to_string(&self, range: Range<usize>) -> String {
        let start = self.clamp_char_boundary(range.start);
        let end = self.clamp_char_boundary(range.end).max(start);
        if start == end {
            return String::new();
        }
        let mut out = String::with_capacity(end.saturating_sub(start));
        collect_slice(&self.root, start, end, &mut out);
        out
    }

    #[must_use]
    pub fn to_string_lossless(&self) -> String {
        self.slice_to_string(0..self.len_bytes())
    }

    #[must_use]
    pub fn byte_at(&self, offset: usize) -> Option<u8> {
        if offset >= self.len_bytes() {
            return None;
        }
        byte_at_node(&self.root, offset)
    }

    #[must_use]
    pub fn char_at(&self, offset: usize) -> Option<char> {
        if offset >= self.len_bytes() {
            return None;
        }
        let aligned = self.clamp_char_boundary(offset);
        let end = (aligned.saturating_add(4)).min(self.len_bytes());
        // Walk up to 4 bytes safely to capture one full UTF-8 codepoint.
        let mut finish = end;
        while finish > aligned && !self.is_char_boundary(finish) {
            finish = finish.saturating_sub(1);
        }
        if finish == aligned {
            // If `end` landed inside a 4-byte char near EOF, `len_bytes` is always a boundary.
            finish = self.len_bytes().min(aligned.saturating_add(4));
        }
        self.slice_to_string(aligned..finish).chars().next()
    }

    /// Byte offset where 0-indexed `line_idx` begins.
    #[must_use]
    pub fn line_start_byte(&self, line_idx: usize) -> usize {
        if line_idx == 0 {
            return 0;
        }
        if line_idx >= self.line_count() {
            return self.len_bytes();
        }
        // Start of line `k` is 1 byte past the `(k - 1)`-th '\n'.
        find_nth_newline(&self.root, line_idx.saturating_sub(1))
            .map_or_else(|| self.len_bytes(), |nl_pos| nl_pos.saturating_add(1))
    }

    /// Byte range of 0-indexed `line_idx` **excluding** any trailing `\n`.
    #[must_use]
    pub fn line_byte_range(&self, line_idx: usize) -> Range<usize> {
        let count = self.line_count();
        if line_idx >= count {
            let end = self.len_bytes();
            return end..end;
        }
        let start = self.line_start_byte(line_idx);
        let end = if line_idx.saturating_add(1) < count {
            self.line_start_byte(line_idx.saturating_add(1))
                .saturating_sub(1)
        } else {
            self.len_bytes()
        };
        start..end.max(start)
    }

    /// Byte range of 0-indexed `line_idx` **including** any trailing `\n`.
    #[must_use]
    pub fn line_full_byte_range(&self, line_idx: usize) -> Range<usize> {
        let count = self.line_count();
        if line_idx >= count {
            let end = self.len_bytes();
            return end..end;
        }
        let start = self.line_start_byte(line_idx);
        let end = if line_idx.saturating_add(1) < count {
            self.line_start_byte(line_idx.saturating_add(1))
        } else {
            self.len_bytes()
        };
        start..end.max(start)
    }

    #[must_use]
    pub fn line_text(&self, line_idx: usize) -> String {
        self.slice_to_string(self.line_byte_range(line_idx))
    }

    /// Converts a byte offset into 0-indexed `(line, col_bytes, col_chars)`.
    #[must_use]
    pub fn byte_to_line_col(&self, byte_offset: usize) -> LineCol {
        let clamped = self.clamp_char_boundary(byte_offset);
        let line = count_newlines_before(&self.root, clamped);
        let line_start = self.line_start_byte(line);
        let col_bytes = clamped.saturating_sub(line_start);
        let prefix = self.slice_to_string(line_start..clamped);
        let col_chars = prefix.chars().count();
        LineCol {
            line,
            col_bytes,
            col_chars,
        }
    }

    /// Converts `(line, col_bytes)` into a valid byte offset inside that line.
    #[must_use]
    pub fn line_col_to_byte(&self, line: usize, col_bytes: usize) -> usize {
        let range = self.line_byte_range(line.min(self.line_count().saturating_sub(1)));
        let target = range.start.saturating_add(col_bytes).min(range.end);
        self.clamp_char_boundary(target)
    }

    /// Replaces `range` (byte offsets, safely clamped to UTF-8 char boundaries) with `inserted`.
    pub fn replace_range(&mut self, range: Range<usize>, inserted: &str) {
        let start = self.clamp_char_boundary(range.start);
        let end = self.clamp_char_boundary(range.end).max(start);
        if start == end && inserted.is_empty() {
            return;
        }
        if start == 0 && end == self.len_bytes() {
            *self = Self::from_str(inserted);
            return;
        }

        let left = split_prefix(&self.root, start);
        let right = split_suffix(&self.root, end);

        let mut merged = if let Some(l) = left {
            if inserted.is_empty() {
                l
            } else {
                let mid = build_tree_from_nodes(split_str_into_leaves(inserted));
                concat_trees(l, mid)
            }
        } else if inserted.is_empty() {
            Node::leaf(String::new())
        } else {
            build_tree_from_nodes(split_str_into_leaves(inserted))
        };

        if let Some(r) = right {
            merged = if merged.summary().bytes == 0 {
                r
            } else {
                concat_trees(merged, r)
            };
        }

        self.root = merged;
    }

    /// Moves one Unicode extended grapheme cluster backward from `byte_offset`.
    #[must_use]
    pub fn prev_grapheme_offset(&self, byte_offset: usize) -> usize {
        let pos = self.clamp_char_boundary(byte_offset);
        if pos == 0 {
            return 0;
        }
        // Inspect up to 256 bytes before `pos` (covers long ZWJ emoji sequences).
        let window_start = self.clamp_char_boundary(pos.saturating_sub(256));
        let window = self.slice_to_string(window_start..pos);
        window
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(rel, _)| window_start.saturating_add(rel))
    }

    /// Moves one Unicode extended grapheme cluster forward from `byte_offset`.
    #[must_use]
    pub fn next_grapheme_offset(&self, byte_offset: usize) -> usize {
        let pos = self.clamp_char_boundary(byte_offset);
        if pos >= self.len_bytes() {
            return self.len_bytes();
        }
        let raw_end = (pos.saturating_add(256)).min(self.len_bytes());
        let mut window_end = raw_end;
        while window_end < self.len_bytes() && !self.is_char_boundary(window_end) {
            window_end = window_end.saturating_add(1);
        }
        let window = self.slice_to_string(pos..window_end);
        window
            .graphemes(true)
            .next()
            .map_or(self.len_bytes(), |g| pos.saturating_add(g.len()))
    }

    /// Moves one word boundary backward (within the current or preceding line).
    #[must_use]
    pub fn prev_word_offset(&self, byte_offset: usize) -> usize {
        let pos = self.clamp_char_boundary(byte_offset);
        if pos == 0 {
            return 0;
        }
        let lc = self.byte_to_line_col(pos);
        let line_range = self.line_byte_range(lc.line);
        if pos == line_range.start {
            return self.prev_grapheme_offset(pos);
        }
        let line_prefix = self.slice_to_string(line_range.start..pos);
        let mut last_start = 0usize;
        for (idx, seg) in line_prefix.split_word_bound_indices() {
            if !seg.chars().all(char::is_whitespace) {
                last_start = idx;
            }
        }
        line_range.start.saturating_add(last_start)
    }

    /// Moves one word boundary forward (within the current or next line).
    #[must_use]
    pub fn next_word_offset(&self, byte_offset: usize) -> usize {
        let pos = self.clamp_char_boundary(byte_offset);
        if pos >= self.len_bytes() {
            return self.len_bytes();
        }
        let lc = self.byte_to_line_col(pos);
        let line_range = self.line_byte_range(lc.line);
        if pos >= line_range.end {
            return self.next_grapheme_offset(pos);
        }
        let line_suffix = self.slice_to_string(pos..line_range.end);
        for (idx, seg) in line_suffix.split_word_bound_indices() {
            if !seg.chars().all(char::is_whitespace) {
                return pos.saturating_add(idx).saturating_add(seg.len());
            }
        }
        line_range.end
    }

    /// Returns the non-whitespace word range covering `byte_offset`, if `byte_offset` touches or sits inside a word.
    #[must_use]
    pub fn word_range_at(&self, byte_offset: usize) -> Option<Range<usize>> {
        let pos = self.clamp_char_boundary(byte_offset);
        let lc = self.byte_to_line_col(pos);
        let line_range = self.line_byte_range(lc.line);
        let text = self.slice_to_string(line_range.clone());
        let rel = pos.saturating_sub(line_range.start);
        for (idx, word) in text.unicode_word_indices() {
            let end = idx.saturating_add(word.len());
            if rel >= idx && rel <= end {
                return Some(
                    line_range.start.saturating_add(idx)..line_range.start.saturating_add(end),
                );
            }
        }
        None
    }
}

fn find_good_split_point(s: &str, target: usize) -> usize {
    let capped = target.min(s.len());
    let mut boundary = capped;
    while boundary > 0 && !s.is_char_boundary(boundary) {
        boundary = boundary.saturating_sub(1);
    }
    // Prefer splitting right after a newline within [MIN_LEAF_BYTES..=boundary] for clean line locality.
    if let Some(sub) = s.get(MIN_LEAF_BYTES..boundary)
        && let Some(nl_rel) = sub.rfind('\n')
    {
        return MIN_LEAF_BYTES.saturating_add(nl_rel).saturating_add(1);
    }
    boundary.max(1)
}

fn split_str_into_leaves(mut remaining: &str) -> Vec<Arc<Node>> {
    let mut leaves = Vec::new();
    while !remaining.is_empty() {
        if remaining.len() <= MAX_LEAF_BYTES {
            leaves.push(Node::leaf(remaining.to_owned()));
            break;
        }
        let split_at = find_good_split_point(remaining, (MIN_LEAF_BYTES + MAX_LEAF_BYTES) / 2);
        let head = remaining.get(..split_at).unwrap_or(remaining);
        leaves.push(Node::leaf(head.to_owned()));
        remaining = remaining.get(split_at..).unwrap_or("");
    }
    if leaves.is_empty() {
        leaves.push(Node::leaf(String::new()));
    }
    leaves
}

fn coalesce_underfull_nodes(nodes: Vec<Arc<Node>>) -> Vec<Arc<Node>> {
    if nodes.len() <= 1 {
        return nodes;
    }
    let level_height = nodes.first().map_or(0, |n| n.height());
    if level_height == 0 {
        let mut merged: Vec<Arc<Node>> = Vec::with_capacity(nodes.len());
        for node in nodes {
            if node.summary().bytes == 0 {
                continue;
            }
            if let (Some(prev), Node::Leaf { text: rt, .. }) =
                (merged.last().cloned(), node.as_ref())
                && let Node::Leaf { text: lt, .. } = prev.as_ref()
                && (lt.len() < MIN_LEAF_BYTES || rt.len() < MIN_LEAF_BYTES)
                && lt.len().saturating_add(rt.len()) <= MAX_LEAF_BYTES
            {
                let mut combined = String::with_capacity(lt.len().saturating_add(rt.len()));
                combined.push_str(lt);
                combined.push_str(rt);
                let last_idx = merged.len().saturating_sub(1);
                merged[last_idx] = Node::leaf(combined);
                continue;
            }
            merged.push(node);
        }
        if merged.is_empty() {
            merged.push(Node::leaf(String::new()));
        }
        return merged;
    }

    // Internal nodes of uniform height >= 1: merge underfull (< MIN_CHILDREN) siblings with a neighbor.
    let mut out: Vec<Arc<Node>> = Vec::with_capacity(nodes.len());
    for node in nodes {
        let curr_len = match node.as_ref() {
            Node::Internal { children, .. } => children.len(),
            Node::Leaf { .. } => 1,
        };
        if let Some(prev) = out.last().cloned()
            && let (
                Node::Internal {
                    children: prev_ch, ..
                },
                Node::Internal {
                    children: curr_ch, ..
                },
            ) = (prev.as_ref(), node.as_ref())
            && (prev_ch.len() < MIN_CHILDREN || curr_len < MIN_CHILDREN)
        {
            let total = prev_ch.len().saturating_add(curr_ch.len());
            let mut combined = Vec::with_capacity(total);
            combined.extend(prev_ch.iter().cloned());
            combined.extend(curr_ch.iter().cloned());
            out.pop();
            if total <= MAX_CHILDREN {
                out.push(Node::internal(combined));
            } else {
                let mid = total / 2;
                let right = combined.split_off(mid);
                out.push(Node::internal(combined));
                out.push(Node::internal(right));
            }
            continue;
        }
        out.push(node);
    }
    out
}

fn build_tree_from_nodes(mut nodes: Vec<Arc<Node>>) -> Arc<Node> {
    if nodes.is_empty() {
        return Node::leaf(String::new());
    }
    nodes = coalesce_underfull_nodes(nodes);
    while nodes.len() > 1 {
        let mut next_level = Vec::with_capacity(nodes.len().div_ceil(MAX_CHILDREN));
        let mut idx = 0usize;
        while idx < nodes.len() {
            let rem = nodes.len().saturating_sub(idx);
            let take = if rem <= MAX_CHILDREN {
                rem
            } else if rem < MAX_CHILDREN + MIN_CHILDREN {
                rem / 2
            } else {
                MAX_CHILDREN
            };
            let chunk: Vec<Arc<Node>> = nodes
                .get(idx..idx.saturating_add(take))
                .unwrap_or(&[])
                .to_vec();
            next_level.push(Node::internal(chunk));
            idx = idx.saturating_add(take);
        }
        nodes = coalesce_underfull_nodes(next_level);
    }
    nodes
        .into_iter()
        .next()
        .unwrap_or_else(|| Node::leaf(String::new()))
}

fn clamp_node_char_boundary(node: &Node, offset: usize) -> usize {
    match node {
        Node::Leaf { text, .. } => {
            let mut b = offset.min(text.len());
            while b > 0 && !text.is_char_boundary(b) {
                b = b.saturating_sub(1);
            }
            b
        }
        Node::Internal { children, .. } => {
            let mut acc = 0usize;
            for child in children {
                let cb = child.summary().bytes;
                if offset < acc.saturating_add(cb) {
                    return acc.saturating_add(clamp_node_char_boundary(
                        child,
                        offset.saturating_sub(acc),
                    ));
                }
                acc = acc.saturating_add(cb);
            }
            acc
        }
    }
}

fn collect_slice(node: &Node, start: usize, end: usize, out: &mut String) {
    if start >= end {
        return;
    }
    match node {
        Node::Leaf { text, .. } => {
            let s = start.min(text.len());
            let e = end.min(text.len());
            if let Some(sub) = text.get(s..e) {
                out.push_str(sub);
            }
        }
        Node::Internal { children, .. } => {
            let mut acc = 0usize;
            for child in children {
                let cb = child.summary().bytes;
                let child_end = acc.saturating_add(cb);
                if start < child_end && end > acc {
                    let cs = start.saturating_sub(acc);
                    let ce = end.saturating_sub(acc).min(cb);
                    collect_slice(child, cs, ce, out);
                }
                acc = child_end;
                if acc >= end {
                    break;
                }
            }
        }
    }
}

fn byte_at_node(node: &Node, offset: usize) -> Option<u8> {
    match node {
        Node::Leaf { text, .. } => text.as_bytes().get(offset).copied(),
        Node::Internal { children, .. } => {
            let mut acc = 0usize;
            for child in children {
                let cb = child.summary().bytes;
                if offset < acc.saturating_add(cb) {
                    return byte_at_node(child, offset.saturating_sub(acc));
                }
                acc = acc.saturating_add(cb);
            }
            None
        }
    }
}

fn find_nth_newline(node: &Node, mut target_nl: usize) -> Option<usize> {
    if target_nl >= node.summary().newlines {
        return None;
    }
    match node {
        Node::Leaf { text, .. } => {
            for (idx, b) in text.bytes().enumerate() {
                if b == b'\n' {
                    if target_nl == 0 {
                        return Some(idx);
                    }
                    target_nl = target_nl.saturating_sub(1);
                }
            }
            None
        }
        Node::Internal { children, .. } => {
            let mut byte_acc = 0usize;
            for child in children {
                let c_sum = child.summary();
                if target_nl < c_sum.newlines {
                    return find_nth_newline(child, target_nl)
                        .map(|rel| byte_acc.saturating_add(rel));
                }
                target_nl = target_nl.saturating_sub(c_sum.newlines);
                byte_acc = byte_acc.saturating_add(c_sum.bytes);
            }
            None
        }
    }
}

fn count_newlines_before(node: &Node, byte_offset: usize) -> usize {
    if byte_offset == 0 {
        return 0;
    }
    if byte_offset >= node.summary().bytes {
        return node.summary().newlines;
    }
    match node {
        Node::Leaf { text, .. } => text
            .as_bytes()
            .get(..byte_offset.min(text.len()))
            .map_or(0, |slice| slice.iter().filter(|&&b| b == b'\n').count()),
        Node::Internal { children, .. } => {
            let mut byte_acc = 0usize;
            let mut nl_acc = 0usize;
            for child in children {
                let c_sum = child.summary();
                if byte_offset < byte_acc.saturating_add(c_sum.bytes) {
                    return nl_acc.saturating_add(count_newlines_before(
                        child,
                        byte_offset.saturating_sub(byte_acc),
                    ));
                }
                byte_acc = byte_acc.saturating_add(c_sum.bytes);
                nl_acc = nl_acc.saturating_add(c_sum.newlines);
            }
            nl_acc
        }
    }
}

fn split_prefix(node: &Arc<Node>, end_byte: usize) -> Option<Arc<Node>> {
    if end_byte == 0 {
        return None;
    }
    if end_byte >= node.summary().bytes {
        return Some(Arc::clone(node));
    }
    match node.as_ref() {
        Node::Leaf { text, .. } => text.get(..end_byte).map(|s| Node::leaf(s.to_owned())),
        Node::Internal { children, .. } => {
            let mut kept = Vec::new();
            let mut acc = 0usize;
            for child in children {
                let cb = child.summary().bytes;
                if end_byte >= acc.saturating_add(cb) {
                    kept.push(Arc::clone(child));
                } else if end_byte > acc {
                    if let Some(sub) = split_prefix(child, end_byte.saturating_sub(acc)) {
                        kept.push(sub);
                    }
                    break;
                } else {
                    break;
                }
                acc = acc.saturating_add(cb);
            }
            collapse_children(kept)
        }
    }
}

fn split_suffix(node: &Arc<Node>, start_byte: usize) -> Option<Arc<Node>> {
    if start_byte == 0 {
        return Some(Arc::clone(node));
    }
    if start_byte >= node.summary().bytes {
        return None;
    }
    match node.as_ref() {
        Node::Leaf { text, .. } => text.get(start_byte..).map(|s| Node::leaf(s.to_owned())),
        Node::Internal { children, .. } => {
            let mut kept = Vec::new();
            let mut acc = 0usize;
            for child in children {
                let cb = child.summary().bytes;
                let next_acc = acc.saturating_add(cb);
                if start_byte == acc {
                    kept.push(Arc::clone(child));
                } else if start_byte < next_acc {
                    if start_byte > acc {
                        if let Some(sub) = split_suffix(child, start_byte.saturating_sub(acc)) {
                            kept.push(sub);
                        }
                    } else {
                        kept.push(Arc::clone(child));
                    }
                }
                acc = next_acc;
            }
            collapse_children(kept)
        }
    }
}

fn collapse_children(children: Vec<Arc<Node>>) -> Option<Arc<Node>> {
    if children.is_empty() {
        return None;
    }
    if children.len() == 1 {
        return children.into_iter().next();
    }
    // Normalize heights before building internal nodes so all siblings share equal height.
    let mut leaves_or_balanced = Vec::new();
    let first_h = children.first().map_or(0, |n| n.height());
    let uniform = children.iter().all(|n| n.height() == first_h);
    if uniform {
        Some(build_tree_from_nodes(children))
    } else {
        for c in children {
            leaves_or_balanced.push(c);
        }
        let mut iter = leaves_or_balanced.into_iter();
        let mut acc = iter.next()?;
        for next in iter {
            acc = concat_trees(acc, next);
        }
        Some(acc)
    }
}

fn concat_trees(left: Arc<Node>, right: Arc<Node>) -> Arc<Node> {
    if left.summary().bytes == 0 {
        return right;
    }
    if right.summary().bytes == 0 {
        return left;
    }
    let lh = left.height();
    let rh = right.height();
    if lh == rh {
        // Coalesce tiny adjacent leaves to prevent degenerate leaf fragmentation during typing.
        if let (Node::Leaf { text: lt, .. }, Node::Leaf { text: rt, .. }) =
            (left.as_ref(), right.as_ref())
            && lt.len().saturating_add(rt.len()) <= MAX_LEAF_BYTES
        {
            let mut merged = String::with_capacity(lt.len().saturating_add(rt.len()));
            merged.push_str(lt);
            merged.push_str(rt);
            return Node::leaf(merged);
        }
        return Node::internal(vec![left, right]);
    }
    if lh > rh {
        if let Node::Internal { children, .. } = left.as_ref()
            && let Some((last, prefix)) = children.split_last()
        {
            let merged_tail = concat_trees(Arc::clone(last), right);
            let mut all = prefix.to_vec();
            if merged_tail.height() == last.height() {
                all.push(merged_tail);
            } else if let Node::Internal {
                children: tail_children,
                ..
            } = merged_tail.as_ref()
            {
                all.extend(tail_children.iter().cloned());
            } else {
                all.push(merged_tail);
            }
            return build_tree_from_nodes(all);
        }
    } else if let Node::Internal { children, .. } = right.as_ref()
        && let Some((first, suffix)) = children.split_first()
    {
        let merged_head = concat_trees(left, Arc::clone(first));
        let mut all = Vec::new();
        if merged_head.height() == first.height() {
            all.push(merged_head);
        } else if let Node::Internal {
            children: head_children,
            ..
        } = merged_head.as_ref()
        {
            all.extend(head_children.iter().cloned());
        } else {
            all.push(merged_head);
        }
        all.extend(suffix.iter().cloned());
        return build_tree_from_nodes(all);
    }
    Node::internal(vec![left, right])
}

fn next_non_empty_leaf_forward<'a>(stack: &mut Vec<&'a Arc<Node>>) -> Option<&'a [u8]> {
    while let Some(node) = stack.pop() {
        match node.as_ref() {
            Node::Leaf { text, .. } => {
                if !text.is_empty() {
                    return Some(text.as_bytes());
                }
            }
            Node::Internal { children, .. } => {
                for child in children.iter().rev() {
                    stack.push(child);
                }
            }
        }
    }
    None
}

fn next_non_empty_leaf_backward<'a>(stack: &mut Vec<&'a Arc<Node>>) -> Option<&'a [u8]> {
    while let Some(node) = stack.pop() {
        match node.as_ref() {
            Node::Leaf { text, .. } => {
                if !text.is_empty() {
                    return Some(text.as_bytes());
                }
            }
            Node::Internal { children, .. } => {
                for child in children {
                    stack.push(child);
                }
            }
        }
    }
    None
}

fn common_prefix_bytes(a: &Arc<Node>, b: &Arc<Node>) -> usize {
    let mut a_stack: Vec<&Arc<Node>> = vec![a];
    let mut b_stack: Vec<&Arc<Node>> = vec![b];
    let mut a_leaf: &[u8] = &[];
    let mut b_leaf: &[u8] = &[];
    let mut prefix = 0usize;

    loop {
        if a_leaf.is_empty() && b_leaf.is_empty() {
            while a_stack.last().is_some_and(|n| n.summary().bytes == 0) {
                a_stack.pop();
            }
            while b_stack.last().is_some_and(|n| n.summary().bytes == 0) {
                b_stack.pop();
            }
            let (Some(&na), Some(&nb)) = (a_stack.last(), b_stack.last()) else {
                break;
            };
            if Arc::ptr_eq(na, nb) {
                prefix = prefix.saturating_add(na.summary().bytes);
                a_stack.pop();
                b_stack.pop();
                continue;
            }
            if na.height() > nb.height() {
                a_stack.pop();
                if let Node::Internal { children, .. } = na.as_ref() {
                    for child in children.iter().rev() {
                        a_stack.push(child);
                    }
                }
                continue;
            }
            if nb.height() > na.height() {
                b_stack.pop();
                if let Node::Internal { children, .. } = nb.as_ref() {
                    for child in children.iter().rev() {
                        b_stack.push(child);
                    }
                }
                continue;
            }
            if na.height() > 0 {
                a_stack.pop();
                b_stack.pop();
                if let Node::Internal { children, .. } = na.as_ref() {
                    for child in children.iter().rev() {
                        a_stack.push(child);
                    }
                }
                if let Node::Internal { children, .. } = nb.as_ref() {
                    for child in children.iter().rev() {
                        b_stack.push(child);
                    }
                }
                continue;
            }
        }

        if a_leaf.is_empty() {
            let Some(next) = next_non_empty_leaf_forward(&mut a_stack) else {
                break;
            };
            a_leaf = next;
        }
        if b_leaf.is_empty() {
            let Some(next) = next_non_empty_leaf_forward(&mut b_stack) else {
                break;
            };
            b_leaf = next;
        }

        let n = a_leaf.len().min(b_leaf.len());
        if a_leaf[..n] == b_leaf[..n] {
            prefix = prefix.saturating_add(n);
            a_leaf = &a_leaf[n..];
            b_leaf = &b_leaf[n..];
        } else {
            let mismatch = a_leaf[..n]
                .iter()
                .zip(&b_leaf[..n])
                .position(|(x, y)| x != y)
                .unwrap_or(0);
            return prefix.saturating_add(mismatch);
        }
    }

    prefix
}

fn common_suffix_bytes(a: &Arc<Node>, b: &Arc<Node>, max_suffix: usize) -> usize {
    if max_suffix == 0 {
        return 0;
    }
    let mut a_stack: Vec<&Arc<Node>> = vec![a];
    let mut b_stack: Vec<&Arc<Node>> = vec![b];
    let mut a_leaf: &[u8] = &[];
    let mut b_leaf: &[u8] = &[];
    let mut suffix = 0usize;

    loop {
        let rem = max_suffix.saturating_sub(suffix);
        if rem == 0 {
            break;
        }

        if a_leaf.is_empty() && b_leaf.is_empty() {
            while a_stack.last().is_some_and(|n| n.summary().bytes == 0) {
                a_stack.pop();
            }
            while b_stack.last().is_some_and(|n| n.summary().bytes == 0) {
                b_stack.pop();
            }
            let (Some(&na), Some(&nb)) = (a_stack.last(), b_stack.last()) else {
                break;
            };
            if Arc::ptr_eq(na, nb) && na.summary().bytes <= rem {
                suffix = suffix.saturating_add(na.summary().bytes);
                a_stack.pop();
                b_stack.pop();
                continue;
            }
            if na.height() > nb.height() {
                a_stack.pop();
                if let Node::Internal { children, .. } = na.as_ref() {
                    for child in children {
                        a_stack.push(child);
                    }
                }
                continue;
            }
            if nb.height() > na.height() {
                b_stack.pop();
                if let Node::Internal { children, .. } = nb.as_ref() {
                    for child in children {
                        b_stack.push(child);
                    }
                }
                continue;
            }
            if na.height() > 0 {
                a_stack.pop();
                b_stack.pop();
                if let Node::Internal { children, .. } = na.as_ref() {
                    for child in children {
                        a_stack.push(child);
                    }
                }
                if let Node::Internal { children, .. } = nb.as_ref() {
                    for child in children {
                        b_stack.push(child);
                    }
                }
                continue;
            }
        }

        if a_leaf.is_empty() {
            let Some(next) = next_non_empty_leaf_backward(&mut a_stack) else {
                break;
            };
            a_leaf = next;
        }
        if b_leaf.is_empty() {
            let Some(next) = next_non_empty_leaf_backward(&mut b_stack) else {
                break;
            };
            b_leaf = next;
        }

        let n = a_leaf.len().min(b_leaf.len()).min(rem);
        let a_start = a_leaf.len().saturating_sub(n);
        let b_start = b_leaf.len().saturating_sub(n);
        let a_tail = &a_leaf[a_start..];
        let b_tail = &b_leaf[b_start..];
        if a_tail == b_tail {
            suffix = suffix.saturating_add(n);
            a_leaf = &a_leaf[..a_start];
            b_leaf = &b_leaf[..b_start];
        } else {
            let mismatch = a_tail
                .iter()
                .rev()
                .zip(b_tail.iter().rev())
                .position(|(x, y)| x != y)
                .unwrap_or(0);
            return suffix.saturating_add(mismatch);
        }
    }

    suffix
}
