use std::ops::Range;

use kmark_buffer::{EditGroup, Rope, Selection};
use regex::RegexBuilder;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub use_regex: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SearchState {
    pub query: String,
    pub options: SearchOptions,
    pub matches: Vec<Range<usize>>,
    pub active_index: Option<usize>,
    pub regex_error: Option<String>,
}

impl SearchState {
    pub fn clear(&mut self) {
        self.query.clear();
        self.matches.clear();
        self.active_index = None;
        self.regex_error = None;
    }

    pub fn recompute(&mut self, rope: &Rope, caret_hint: usize) {
        self.matches.clear();
        self.active_index = None;
        self.regex_error = None;

        if self.query.is_empty() {
            return;
        }

        let pattern = if self.options.use_regex {
            if self.options.whole_word {
                format!(r"\b(?:{})\b", self.query)
            } else {
                self.query.clone()
            }
        } else {
            let escaped = regex::escape(&self.query);
            if self.options.whole_word {
                format!(r"\b{escaped}\b")
            } else {
                escaped
            }
        };

        let compiled = RegexBuilder::new(&pattern)
            .case_insensitive(!self.options.case_sensitive)
            .multi_line(true)
            .build();

        let regex = match compiled {
            Ok(re) => re,
            Err(err) => {
                self.regex_error = Some(err.to_string());
                return;
            }
        };

        let haystack = rope.to_string_lossless();
        for m in regex.find_iter(&haystack) {
            if m.start() < m.end() {
                self.matches.push(m.start()..m.end());
            }
        }

        if !self.matches.is_empty() {
            let idx = self
                .matches
                .iter()
                .position(|r| r.start >= caret_hint)
                .unwrap_or(0);
            self.active_index = Some(idx);
        }
    }

    pub fn step_next(&mut self) -> Option<Range<usize>> {
        if self.matches.is_empty() {
            self.active_index = None;
            return None;
        }
        let next = match self.active_index {
            Some(i) => (i.saturating_add(1)) % self.matches.len(),
            None => 0,
        };
        self.active_index = Some(next);
        self.matches.get(next).cloned()
    }

    pub fn step_prev(&mut self) -> Option<Range<usize>> {
        if self.matches.is_empty() {
            self.active_index = None;
            return None;
        }
        let prev = match self.active_index {
            Some(0) | None => self.matches.len().saturating_sub(1),
            Some(i) => i.saturating_sub(1),
        };
        self.active_index = Some(prev);
        self.matches.get(prev).cloned()
    }

    /// Replaces the currently active match and returns `(new_rope, new_selection, edit_group)`.
    #[must_use]
    pub fn replace_current(
        &self,
        rope: &Rope,
        replacement: &str,
    ) -> Option<(Rope, Selection, EditGroup)> {
        let idx = self.active_index?;
        let target = self.matches.get(idx)?.clone();
        let mut updated = rope.clone();
        updated.replace_range(target.clone(), replacement);
        let new_caret = target.start.saturating_add(replacement.len());
        Some((
            updated,
            Selection::collapsed(new_caret),
            EditGroup::Discrete,
        ))
    }

    /// Replaces all current matches in one atomic undo step and returns `(new_rope, new_selection, count)`.
    #[must_use]
    pub fn replace_all(&self, rope: &Rope, replacement: &str) -> Option<(Rope, Selection, usize)> {
        if self.matches.is_empty() {
            return None;
        }
        let mut updated = rope.clone();
        let count = self.matches.len();
        let mut last_caret = 0usize;
        // Apply from back to front so earlier byte offsets stay valid!
        for m in self.matches.iter().rev() {
            updated.replace_range(m.clone(), replacement);
            last_caret = m.start.saturating_add(replacement.len());
        }
        Some((updated, Selection::collapsed(last_caret), count))
    }
}
