use std::ops::Range;

use crate::rope::Rope;

/// Cursor and selection state in UTF-8 byte offsets within a [`Rope`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Selection {
    /// Fixed end of selection when dragging or Shift-moving.
    pub anchor: usize,
    /// Active caret offset.
    pub head: usize,
}

impl Selection {
    #[must_use]
    pub const fn collapsed(offset: usize) -> Self {
        Self {
            anchor: offset,
            head: offset,
        }
    }

    #[must_use]
    pub const fn new(anchor: usize, head: usize) -> Self {
        Self { anchor, head }
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.anchor == self.head
    }

    #[must_use]
    pub fn start(self) -> usize {
        self.anchor.min(self.head)
    }

    #[must_use]
    pub fn end(self) -> usize {
        self.anchor.max(self.head)
    }

    #[must_use]
    pub fn len_bytes(self) -> usize {
        self.end().saturating_sub(self.start())
    }

    #[must_use]
    pub fn range(self) -> Range<usize> {
        self.start()..self.end()
    }

    #[must_use]
    pub fn clamp_to(self, rope: &Rope) -> Self {
        Self {
            anchor: rope.clamp_char_boundary(self.anchor),
            head: rope.clamp_char_boundary(self.head),
        }
    }
}

/// Classification of an edit used to coalesce consecutive typing or backspacing into one undo step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditGroup {
    Typing,
    Backspace,
    Discrete,
}

#[derive(Clone, Debug)]
struct HistoryEntry {
    before_rope: Rope,
    before_selection: Selection,
    after_rope: Rope,
    after_selection: Selection,
    group: EditGroup,
}

/// Infallible snapshot-based Undo/Redo stack leveraging `O(1)` persistent [`Rope`] clones.
#[derive(Clone, Debug)]
pub struct DocumentHistory {
    undo_stack: Vec<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>,
    max_entries: usize,
    coalescing_broken: bool,
}

impl Default for DocumentHistory {
    fn default() -> Self {
        Self::new(500)
    }
}

impl DocumentHistory {
    #[must_use]
    pub fn new(max_entries: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_entries: max_entries.max(16),
            coalescing_broken: true,
        }
    }

    pub fn break_coalescing(&mut self) {
        self.coalescing_broken = true;
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.coalescing_broken = true;
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn record(
        &mut self,
        before_rope: Rope,
        before_selection: Selection,
        after_rope: Rope,
        after_selection: Selection,
        group: EditGroup,
    ) {
        if before_rope == after_rope {
            return;
        }
        self.redo_stack.clear();

        let can_merge = !self.coalescing_broken
            && matches!(group, EditGroup::Typing | EditGroup::Backspace)
            && self.undo_stack.last().is_some_and(|prev| {
                prev.group == group && prev.after_selection == before_selection
            });

        if can_merge {
            if let Some(last) = self.undo_stack.last_mut() {
                last.after_rope = after_rope;
                last.after_selection = after_selection;
            }
        } else {
            if self.undo_stack.len() >= self.max_entries {
                self.undo_stack.remove(0);
            }
            self.undo_stack.push(HistoryEntry {
                before_rope,
                before_selection,
                after_rope,
                after_selection,
                group,
            });
        }

        self.coalescing_broken = group == EditGroup::Discrete;
    }

    pub fn undo(&mut self) -> Option<(Rope, Selection)> {
        let entry = self.undo_stack.pop()?;
        let out = (entry.before_rope.clone(), entry.before_selection);
        self.redo_stack.push(entry);
        self.coalescing_broken = true;
        Some(out)
    }

    pub fn redo(&mut self) -> Option<(Rope, Selection)> {
        let entry = self.redo_stack.pop()?;
        let out = (entry.after_rope.clone(), entry.after_selection);
        self.undo_stack.push(entry);
        self.coalescing_broken = true;
        Some(out)
    }
}
