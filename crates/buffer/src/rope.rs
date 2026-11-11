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
        self.to_string_lossless() == other.to_string_lossless()
    }
}

impl Eq for Rope {}

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

fn build_tree_from_nodes(mut nodes: Vec<Arc<Node>>) -> Arc<Node> {
    if nodes.is_empty() {
        return Node::leaf(String::new());
    }
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
        nodes = next_level;
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
