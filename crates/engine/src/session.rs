use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use kmark_buffer::encoding::{read_file_auto, write_file_atomic};
use kmark_buffer::{
    DocumentHistory, EditGroup, IoError, LineEnding, Rope, Selection, TextEncoding,
};
use kmark_layout::{
    BlockHeightCache, FallbackMeasurer, HeadingMarkStyle, HitTarget, LayoutConfig, TextMeasurer,
    ThemePalette, ViewMode, ViewportFrame, ViewportRequest, build_viewport_frame_cached,
};
use kmark_syntax::{
    AlertKind, Block, BlockKind, ColumnAlign, ImageSize, InlineTokenKind, OutlineEntry, StyleFlags,
    active_styles_at, export_html_document, extract_outline, parse_blocks, parse_inline_tokens,
    reparse_changed_rope, update_outline_changed_rope,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::commands::{
    ArmedStyle, Command, InlineOutcome, LinePrefixKind, TableOp, apply_heading,
    apply_inline_toggle, apply_table_op, insert_alert_block, insert_code_fence,
    insert_horizontal_rule, insert_image, insert_link, insert_new_table_localized,
    locate_table_cursor, set_callout_kind, set_code_fence_language, toggle_line_prefix,
    update_image,
};
use crate::input::{
    KeyCode, KeyModifiers, handle_delete_key, handle_enter, handle_tab, handle_text_insert,
    move_with_frame, step_horizontal,
};
use crate::recovery::{RecoveryDraft, RecoveryStore};
use crate::sample::UiLanguage;
use crate::search::{SearchOptions, SearchState};

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error(transparent)]
    Io(#[from] IoError),
    #[error("No file path set for active tab")]
    MissingFilePath,
    #[error("Invalid tab index: {0}")]
    InvalidTab(usize),
}

#[derive(Clone, Debug)]
pub struct ActiveToolbarState {
    pub heading_level: u8,
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub highlight: bool,
    pub inline_code: bool,
    pub in_table: bool,
    pub table_col_align: ColumnAlign,
    pub in_code_block: bool,
    pub code_language: String,
    pub in_blockquote: bool,
    pub in_bullet_list: bool,
    pub in_ordered_list: bool,
    pub in_task_list: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub is_dirty: bool,
    pub line_1based: usize,
    pub col_1based: usize,
    pub char_count: usize,
    pub word_count: usize,
    pub encoding: TextEncoding,
    pub line_ending: LineEnding,
    pub view_mode: ViewMode,
}

/// Metadata of an inline Markdown image hit-tested at viewport coordinates `(vx, vy)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageAtPoint {
    pub range: Range<usize>,
    pub alt: String,
    pub src: String,
    pub caption: Option<String>,
    pub size: Option<ImageSize>,
}

/// Bundled layout settings update passed to [`Session::set_layout_options`].
#[derive(Clone, Copy, Debug)]
pub struct LayoutOptionsUpdate {
    pub view_mode: ViewMode,
    pub heading_marks: HeadingMarkStyle,
    pub word_wrap: bool,
    pub show_line_numbers: bool,
    pub focus_mode: bool,
    pub typewriter_mode: bool,
    pub base_font_px: f32,
    pub line_spacing_scale: f32,
    pub max_prose_width_px: f32,
}

impl Default for ActiveToolbarState {
    fn default() -> Self {
        Self {
            heading_level: 0,
            bold: false,
            italic: false,
            strike: false,
            highlight: false,
            inline_code: false,
            in_table: false,
            table_col_align: ColumnAlign::Left,
            in_code_block: false,
            code_language: String::new(),
            in_blockquote: false,
            in_bullet_list: false,
            in_ordered_list: false,
            in_task_list: false,
            can_undo: false,
            can_redo: false,
            is_dirty: false,
            line_1based: 1,
            col_1based: 1,
            char_count: 0,
            word_count: 0,
            encoding: TextEncoding::Utf8,
            line_ending: LineEnding::Lf,
            view_mode: ViewMode::Wysiwyg,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PointerAction {
    Handled,
    OpenExternalUrl(String),
    EditCodeLanguage {
        open_line_start: usize,
        current_language: String,
    },
    EditCalloutKind {
        open_line_start: usize,
        current_kind: AlertKind,
    },
}

pub struct DocumentTab {
    pub id: u64,
    pub path: Option<PathBuf>,
    pub title: String,
    pub rope: Rope,
    pub saved_rope: Rope,
    pub revision: u64,
    pub saved_revision: Option<u64>,
    pub encoding: TextEncoding,
    pub line_ending: LineEnding,
    pub view_mode: ViewMode,
    pub selection: Selection,
    pub preferred_x: Option<f32>,
    pub scroll_y: f32,
    pub armed_styles: Vec<ArmedStyle>,
    pub ime_preedit: Option<String>,
    pub history: DocumentHistory,
    pub blocks: Vec<Block>,
    pub outline: Vec<OutlineEntry>,
    pub height_cache: BlockHeightCache,
    pub last_frame: Option<ViewportFrame>,
    pub word_count: usize,
    pub needs_scroll_to_caret: bool,
}

/// Number of lines per slice when computing full-document word counts so multi-MiB
/// files never allocate a single contiguous full-document `String`.
const WORD_COUNT_BATCH_LINES: usize = 512;

fn count_words_in_lines(rope: &Rope, line_range: Range<usize>) -> usize {
    let end = line_range.end.min(rope.line_count());
    let start = line_range.start.min(end);
    if start >= end {
        return 0;
    }
    let byte_start = rope.line_start_byte(start);
    let byte_end = rope.line_full_byte_range(end.saturating_sub(1)).end;
    rope.slice_to_string(byte_start..byte_end)
        .unicode_words()
        .count()
}

fn count_all_words_batched(rope: &Rope) -> usize {
    let total_lines = rope.line_count();
    let mut words = 0usize;
    let mut start = 0usize;
    while start < total_lines {
        let end = start
            .saturating_add(WORD_COUNT_BATCH_LINES)
            .min(total_lines);
        words = words.saturating_add(count_words_in_lines(rope, start..end));
        start = end;
    }
    words
}

impl DocumentTab {
    #[must_use]
    pub fn new_empty(id: u64, index_num: usize) -> Self {
        Self::new_empty_localized(id, index_num, UiLanguage::German)
    }

    #[must_use]
    pub fn new_empty_localized(id: u64, index_num: usize, lang: UiLanguage) -> Self {
        let rope = Rope::new();
        let mut tab = Self {
            id,
            path: None,
            title: lang.untitled_title(index_num),
            saved_rope: rope.clone(),
            rope,
            revision: 0,
            saved_revision: Some(0),
            encoding: TextEncoding::Utf8,
            line_ending: LineEnding::Lf,
            view_mode: ViewMode::Wysiwyg,
            selection: Selection::collapsed(0),
            preferred_x: None,
            scroll_y: 0.0,
            armed_styles: Vec::new(),
            ime_preedit: None,
            history: DocumentHistory::default(),
            blocks: Vec::new(),
            outline: Vec::new(),
            height_cache: BlockHeightCache::new(),
            last_frame: None,
            word_count: 0,
            needs_scroll_to_caret: false,
        };
        tab.reparse();
        tab
    }

    #[must_use]
    pub fn from_markdown_text(id: u64, title: impl Into<String>, content: &str) -> Self {
        let rope = Rope::from_str(content);
        let mut tab = Self {
            id,
            path: None,
            title: title.into(),
            saved_rope: rope.clone(),
            rope,
            revision: 0,
            saved_revision: Some(0),
            encoding: TextEncoding::Utf8,
            line_ending: LineEnding::Lf,
            view_mode: ViewMode::Wysiwyg,
            selection: Selection::collapsed(0),
            preferred_x: None,
            scroll_y: 0.0,
            armed_styles: Vec::new(),
            ime_preedit: None,
            history: DocumentHistory::default(),
            blocks: Vec::new(),
            outline: Vec::new(),
            height_cache: BlockHeightCache::new(),
            last_frame: None,
            word_count: 0,
            needs_scroll_to_caret: false,
        };
        tab.reparse();
        tab
    }

    pub fn reparse(&mut self) {
        self.selection = self.selection.clamp_to(&self.rope);
        self.last_frame = None;
        self.blocks = parse_blocks(&self.rope);
        self.outline = extract_outline(&self.rope, &self.blocks);
        self.word_count = count_all_words_batched(&self.rope);
    }

    #[must_use]
    pub fn is_dirty(&self) -> bool {
        if self.saved_revision == Some(self.revision) || self.rope.ptr_eq(&self.saved_rope) {
            return false;
        }
        self.rope != self.saved_rope
    }

    #[must_use]
    pub fn selected_text(&self) -> String {
        if self.selection.is_empty() {
            String::new()
        } else {
            self.rope.slice_to_string(self.selection.range())
        }
    }

    #[must_use]
    pub fn export_html(&self) -> String {
        export_html_document(&self.title, &self.rope, &self.blocks)
    }

    fn apply_incremental_rope_transition(
        &mut self,
        old_rope: &Rope,
        old_range: Range<usize>,
        new_range: Range<usize>,
    ) {
        self.revision = self.revision.wrapping_add(1);
        self.last_frame = None;
        reparse_changed_rope(old_rope, &self.rope, &mut self.blocks);
        update_outline_changed_rope(old_rope, &self.rope, &self.blocks, &mut self.outline);

        let old_line_start = old_rope.byte_to_line_col(old_range.start).line;
        let old_line_end = old_rope
            .byte_to_line_col(old_range.end)
            .line
            .saturating_add(1);
        let new_line_start = self.rope.byte_to_line_col(new_range.start).line;
        let new_line_end = self
            .rope
            .byte_to_line_col(new_range.end)
            .line
            .saturating_add(1);
        let old_words = count_words_in_lines(old_rope, old_line_start..old_line_end);
        let new_words = count_words_in_lines(&self.rope, new_line_start..new_line_end);
        self.word_count = self
            .word_count
            .saturating_sub(old_words)
            .saturating_add(new_words);
    }

    pub fn commit_edit(&mut self, next_rope: Rope, next_sel: Selection, group: EditGroup) {
        let clamped_sel = next_sel.clamp_to(&next_rope);
        let diff = self.rope.diff_byte_range(&next_rope);
        if diff.is_none() && self.selection == clamped_sel {
            return;
        }
        let before_r = self.rope.clone();
        let before_s = self.selection;
        self.rope = next_rope;
        self.selection = clamped_sel;
        self.preferred_x = None;
        self.needs_scroll_to_caret = true;

        if let Some((old_range, new_range)) = diff {
            self.apply_incremental_rope_transition(&before_r, old_range, new_range);
        }

        self.history
            .record(before_r, before_s, self.rope.clone(), self.selection, group);
    }
}

/// Maximum automatic synchronous recovery draft size (4 MB) so huge files stay sub-millisecond.
const MAX_RECOVERY_DRAFT_BYTES: usize = 4 * 1024 * 1024;

/// Multi-document editor session driving both the native KDE/Qt6 shell and `kmark-cli`.
pub struct Session {
    tabs: Vec<DocumentTab>,
    active_tab: usize,
    next_tab_id: u64,
    untitled_counter: usize,
    pub ui_language: UiLanguage,
    pub layout_config: LayoutConfig,
    pub search: SearchState,
    pub recovery: RecoveryStore,
    flushed_revisions: std::collections::HashMap<u64, u64>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    #[must_use]
    pub fn new() -> Self {
        let mut s = Self {
            tabs: Vec::new(),
            active_tab: 0,
            next_tab_id: 1,
            untitled_counter: 1,
            ui_language: UiLanguage::German,
            layout_config: LayoutConfig::default(),
            search: SearchState::default(),
            recovery: RecoveryStore::xdg_default(),
            flushed_revisions: std::collections::HashMap::new(),
        };
        let _ = s.new_empty_tab();
        s
    }

    #[must_use]
    pub fn new_headless() -> Self {
        let mut s = Self::new();
        s.recovery = RecoveryStore::disabled();
        s
    }

    pub fn set_ui_language(&mut self, lang: UiLanguage) {
        self.ui_language = lang;
        self.layout_config.ui_german = lang.is_german();
        for tab in &mut self.tabs {
            if tab.path.is_none()
                && let Some(num_str) = tab
                    .title
                    .strip_prefix("Unbenannt ")
                    .or_else(|| tab.title.strip_prefix("Untitled "))
                && let Ok(n) = num_str.parse::<usize>()
            {
                tab.title = lang.untitled_title(n);
            }
        }
    }

    fn alloc_tab_id(&mut self) -> u64 {
        let id = self.next_tab_id;
        self.next_tab_id = self.next_tab_id.saturating_add(1);
        id
    }

    pub fn new_empty_tab(&mut self) -> usize {
        let id = self.alloc_tab_id();
        let num = self.untitled_counter;
        self.untitled_counter = self.untitled_counter.saturating_add(1);
        let mut tab = DocumentTab::new_empty_localized(id, num, self.ui_language);
        tab.view_mode = self.layout_config.view_mode;
        self.tabs.push(tab);
        let idx = self.tabs.len().saturating_sub(1);
        self.active_tab = idx;
        self.sync_search_for_active_tab();
        idx
    }

    fn open_builtin_doc(&mut self, title: &str, markdown: &str) -> usize {
        if let Some(existing) = self
            .tabs
            .iter()
            .position(|t| t.path.is_none() && t.title == title && !t.is_dirty())
        {
            self.active_tab = existing;
            self.sync_search_for_active_tab();
            return existing;
        }
        let id = self.alloc_tab_id();
        let mut tab = DocumentTab::from_markdown_text(id, title, markdown);
        if self.layout_config.view_mode != ViewMode::PlainText {
            tab.view_mode = self.layout_config.view_mode;
        }
        // Replace single untouched empty tab if present
        if self.tabs.len() == 1
            && let Some(first) = self.tabs.first()
            && first.path.is_none()
            && !first.is_dirty()
            && first.rope.is_empty()
        {
            self.tabs[0] = tab;
            self.active_tab = 0;
            self.sync_search_for_active_tab();
            return 0;
        }
        self.tabs.push(tab);
        let idx = self.tabs.len().saturating_sub(1);
        self.active_tab = idx;
        self.sync_search_for_active_tab();
        idx
    }

    pub fn open_guide_tab(&mut self) -> usize {
        let lang = self.ui_language;
        self.open_builtin_doc(lang.quick_guide_title(), lang.quick_guide_markdown())
    }

    pub fn open_reference_tab(&mut self) -> usize {
        let lang = self.ui_language;
        self.open_builtin_doc(
            lang.complete_reference_title(),
            lang.complete_reference_markdown(),
        )
    }

    pub fn open_file(&mut self, path: impl AsRef<Path>) -> Result<usize, EngineError> {
        let canonical_or_given = path.as_ref().to_path_buf();
        // Focus already-open tab if path matches
        if let Some(existing_idx) = self
            .tabs
            .iter()
            .position(|t| t.path.as_deref() == Some(canonical_or_given.as_path()))
        {
            self.active_tab = existing_idx;
            self.sync_search_for_active_tab();
            return Ok(existing_idx);
        }

        let decoded = read_file_auto(&canonical_or_given)?;
        let title = canonical_or_given
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("Dokument")
            .to_owned();
        let path_mode = default_view_mode_for_path(&canonical_or_given);
        let view_mode = if path_mode == ViewMode::PlainText {
            ViewMode::PlainText
        } else if self.layout_config.view_mode != ViewMode::PlainText {
            self.layout_config.view_mode
        } else {
            ViewMode::Wysiwyg
        };
        let id = self.alloc_tab_id();
        let rope = Rope::from_str(&decoded.text);
        let mut tab = DocumentTab {
            id,
            path: Some(canonical_or_given),
            title,
            saved_rope: rope.clone(),
            rope,
            revision: 0,
            saved_revision: Some(0),
            encoding: decoded.encoding,
            line_ending: decoded.line_ending,
            view_mode,
            selection: Selection::collapsed(0),
            preferred_x: None,
            scroll_y: 0.0,
            armed_styles: Vec::new(),
            ime_preedit: None,
            history: DocumentHistory::default(),
            blocks: Vec::new(),
            outline: Vec::new(),
            height_cache: BlockHeightCache::new(),
            last_frame: None,
            word_count: 0,
            needs_scroll_to_caret: false,
        };
        tab.reparse();

        // Replace single untouched empty startup tab
        if self.tabs.len() == 1
            && let Some(first) = self.tabs.first()
            && first.path.is_none()
            && !first.is_dirty()
            && first.rope.is_empty()
        {
            self.tabs[0] = tab;
            self.active_tab = 0;
            self.sync_search_for_active_tab();
            return Ok(0);
        }

        self.tabs.push(tab);
        let idx = self.tabs.len().saturating_sub(1);
        self.active_tab = idx;
        self.sync_search_for_active_tab();
        Ok(idx)
    }

    pub fn save_active_tab(&mut self) -> Result<(), EngineError> {
        let Some(tab) = self.tabs.get_mut(self.active_tab) else {
            return Err(EngineError::InvalidTab(self.active_tab));
        };
        let Some(path) = tab.path.clone() else {
            return Err(EngineError::MissingFilePath);
        };
        let content = tab.rope.to_string_lossless();
        write_file_atomic(&path, &content, tab.encoding, tab.line_ending)?;
        tab.saved_rope = tab.rope.clone();
        tab.saved_revision = Some(tab.revision);
        self.flushed_revisions.remove(&tab.id);
        self.recovery.remove_draft(tab.id);
        Ok(())
    }

    pub fn save_active_tab_as(&mut self, path: impl AsRef<Path>) -> Result<(), EngineError> {
        let target = path.as_ref().to_path_buf();
        let Some(tab) = self.tabs.get_mut(self.active_tab) else {
            return Err(EngineError::InvalidTab(self.active_tab));
        };
        let content = tab.rope.to_string_lossless();
        write_file_atomic(&target, &content, tab.encoding, tab.line_ending)?;
        if let Some(name) = target.file_name().and_then(|n| n.to_str()) {
            tab.title = name.to_owned();
        }
        tab.path = Some(target);
        tab.saved_rope = tab.rope.clone();
        tab.saved_revision = Some(tab.revision);
        self.flushed_revisions.remove(&tab.id);
        self.recovery.remove_draft(tab.id);
        Ok(())
    }

    pub fn close_tab(&mut self, idx: usize) {
        if let Some(removed) = (idx < self.tabs.len()).then(|| self.tabs.remove(idx)) {
            self.flushed_revisions.remove(&removed.id);
            self.recovery.remove_draft(removed.id);
        }
        if self.tabs.is_empty() {
            let _ = self.new_empty_tab();
        } else if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len().saturating_sub(1);
        }
        self.sync_search_for_active_tab();
    }

    pub fn set_active_tab(&mut self, idx: usize) {
        if idx < self.tabs.len() {
            self.active_tab = idx;
            self.sync_search_for_active_tab();
        }
    }

    #[must_use]
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    #[must_use]
    pub fn active_tab_index(&self) -> usize {
        self.active_tab
    }

    #[must_use]
    pub fn tabs(&self) -> &[DocumentTab] {
        &self.tabs
    }

    #[must_use]
    pub fn active_doc(&self) -> Option<&DocumentTab> {
        self.tabs.get(self.active_tab)
    }

    pub fn active_doc_mut(&mut self) -> Option<&mut DocumentTab> {
        self.tabs.get_mut(self.active_tab)
    }

    pub fn set_palette(&mut self, palette: ThemePalette) {
        self.layout_config.palette = palette;
    }

    #[must_use]
    pub const fn layout_config(&self) -> &LayoutConfig {
        &self.layout_config
    }

    pub fn set_layout_options(&mut self, opts: LayoutOptionsUpdate) {
        let mode_changed = self.layout_config.view_mode != opts.view_mode;
        self.layout_config.view_mode = opts.view_mode;
        self.layout_config.heading_marks = opts.heading_marks;
        self.layout_config.word_wrap = opts.word_wrap;
        self.layout_config.show_line_numbers = opts.show_line_numbers;
        self.layout_config.focus_mode = opts.focus_mode;
        self.layout_config.typewriter_mode = opts.typewriter_mode;
        self.layout_config.base_font_px = opts.base_font_px.clamp(10.0, 40.0);
        self.layout_config.line_spacing_scale = opts.line_spacing_scale.clamp(1.10, 2.20);
        self.layout_config.max_prose_width_px = opts.max_prose_width_px.clamp(500.0, 2400.0);

        if mode_changed
            && let Some(tab) = self.tabs.get_mut(self.active_tab)
            && tab.view_mode != ViewMode::PlainText
        {
            tab.view_mode = opts.view_mode;
            tab.armed_styles.clear();
            tab.last_frame = None;
            tab.needs_scroll_to_caret = true;
        }
    }

    pub fn set_scroll_y(&mut self, scroll_y: f32) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.scroll_y = scroll_y.max(0.0);
        }
    }

    pub fn scroll_by_pixels(&mut self, delta_y: f32) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            let max_scroll = tab
                .last_frame
                .as_ref()
                .map_or(f32::MAX, |f| (f.total_height - f.viewport_h * 0.5).max(0.0));
            tab.scroll_y = (tab.scroll_y + delta_y).clamp(0.0, max_scroll);
        }
    }

    pub fn jump_to_byte_offset(&mut self, byte_offset: usize) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            let clamped = tab.rope.clamp_char_boundary(byte_offset);
            tab.selection = Selection::collapsed(clamped);
            tab.armed_styles.clear();
            tab.preferred_x = None;
            tab.needs_scroll_to_caret = true;
            tab.history.break_coalescing();
        }
    }

    /// Flushes recovery drafts for **all** dirty tabs (skipping unchanged revisions and files > 4 MB).
    pub fn flush_recovery_draft(&mut self) {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());

        for tab in &self.tabs {
            if !tab.is_dirty() {
                if self.flushed_revisions.remove(&tab.id).is_some() {
                    self.recovery.remove_draft(tab.id);
                }
                continue;
            }
            if tab.rope.len_bytes() > MAX_RECOVERY_DRAFT_BYTES {
                continue;
            }
            if self.flushed_revisions.get(&tab.id) == Some(&tab.revision) {
                continue;
            }
            let draft = RecoveryDraft {
                tab_id: tab.id,
                original_path: tab.path.clone(),
                title: tab.title.clone(),
                content: tab.rope.to_string_lossless(),
                timestamp_unix_secs: ts,
                cursor_byte: tab.selection.head,
            };
            if self.recovery.save_draft(&draft).is_ok() {
                self.flushed_revisions.insert(tab.id, tab.revision);
            }
        }
    }

    /// Removes all recovery draft files belonging to the current session on clean exit.
    pub fn clear_all_recovery_drafts(&mut self) {
        self.flushed_revisions.clear();
        self.recovery.clear_session_drafts();
    }

    /// Loads orphaned crash drafts from non-running KMark processes, restores them into dirty tabs,
    /// re-saves them under the current session PID, and removes the stale orphan files.
    pub fn recover_crash_drafts(&mut self) -> usize {
        let orphans = self.recovery.load_orphaned_drafts();
        if orphans.is_empty() {
            return 0;
        }

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let mut recovered_count = 0usize;

        for (orphan_path, draft) in orphans {
            let rope = Rope::from_str(&draft.content);
            let mut saved_rope = Rope::new();
            let mut encoding = TextEncoding::Utf8;
            let mut line_ending = LineEnding::Lf;
            let mut view_mode = self.layout_config.view_mode;

            if let Some(ref orig_path) = draft.original_path {
                if default_view_mode_for_path(orig_path) == ViewMode::PlainText {
                    view_mode = ViewMode::PlainText;
                }
                if let Ok(decoded) = read_file_auto(orig_path) {
                    saved_rope = Rope::from_str(&decoded.text);
                    encoding = decoded.encoding;
                    line_ending = decoded.line_ending;
                    if rope == saved_rope {
                        let _ = std::fs::remove_file(&orphan_path);
                        continue;
                    }
                }
            } else if rope.is_empty() {
                let _ = std::fs::remove_file(&orphan_path);
                continue;
            }

            let id = self.alloc_tab_id();
            let sel = Selection::collapsed(draft.cursor_byte).clamp_to(&rope);
            let mut tab = DocumentTab {
                id,
                path: draft.original_path.clone(),
                title: draft.title,
                saved_rope,
                rope,
                revision: 1,
                saved_revision: None,
                encoding,
                line_ending,
                view_mode,
                selection: sel,
                preferred_x: None,
                scroll_y: 0.0,
                armed_styles: Vec::new(),
                ime_preedit: None,
                history: DocumentHistory::default(),
                blocks: Vec::new(),
                outline: Vec::new(),
                height_cache: BlockHeightCache::new(),
                last_frame: None,
                word_count: 0,
                needs_scroll_to_caret: false,
            };
            tab.reparse();

            // Adopt draft under current session PID before removing old dead-PID file.
            let adopted = RecoveryDraft {
                tab_id: id,
                original_path: tab.path.clone(),
                title: tab.title.clone(),
                content: draft.content,
                timestamp_unix_secs: ts,
                cursor_byte: tab.selection.head,
            };
            if self.recovery.save_draft(&adopted).is_ok() {
                self.flushed_revisions.insert(id, tab.revision);
            }
            let _ = std::fs::remove_file(&orphan_path);

            if self.tabs.len() == 1
                && let Some(first) = self.tabs.first()
                && first.path.is_none()
                && !first.is_dirty()
                && first.rope.is_empty()
            {
                self.tabs[0] = tab;
                self.active_tab = 0;
            } else {
                self.tabs.push(tab);
                self.active_tab = self.tabs.len().saturating_sub(1);
            }

            recovered_count = recovered_count.saturating_add(1);
        }

        if recovered_count > 0 {
            self.sync_search_for_active_tab();
        }
        recovered_count
    }

    fn sync_search_for_active_tab(&mut self) {
        if let Some(tab) = self.tabs.get(self.active_tab) {
            self.search.recompute(&tab.rope, tab.selection.head);
        }
    }

    pub fn set_search_query(&mut self, query: &str, options: SearchOptions) {
        self.search.query = query.to_owned();
        self.search.options = options;
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            self.search.recompute(&tab.rope, tab.selection.start());
            if let Some(idx) = self.search.active_index
                && let Some(m) = self.search.matches.get(idx)
            {
                tab.selection = Selection::new(m.start, m.end);
                tab.needs_scroll_to_caret = true;
            }
        }
    }

    pub fn find_next(&mut self) -> bool {
        let Some(m) = self.search.step_next() else {
            return false;
        };
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.selection = Selection::new(m.start, m.end);
            tab.needs_scroll_to_caret = true;
        }
        true
    }

    pub fn find_prev(&mut self) -> bool {
        let Some(m) = self.search.step_prev() else {
            return false;
        };
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.selection = Selection::new(m.start, m.end);
            tab.needs_scroll_to_caret = true;
        }
        true
    }

    pub fn replace_current_match(&mut self, replacement: &str) -> bool {
        let Some(tab) = self.tabs.get_mut(self.active_tab) else {
            return false;
        };
        let Some((next_rope, next_sel, group)) =
            self.search.replace_current(&tab.rope, replacement)
        else {
            return false;
        };
        tab.commit_edit(next_rope, next_sel, group);
        let hint = tab.selection.head;
        self.search.recompute(&tab.rope, hint);
        if let Some(idx) = self.search.active_index
            && let Some(m) = self.search.matches.get(idx)
        {
            tab.selection = Selection::new(m.start, m.end);
            tab.needs_scroll_to_caret = true;
        }
        true
    }

    pub fn replace_all_matches(&mut self, replacement: &str) -> usize {
        let Some(tab) = self.tabs.get_mut(self.active_tab) else {
            return 0;
        };
        let Some((next_rope, next_sel, count)) = self.search.replace_all(&tab.rope, replacement)
        else {
            return 0;
        };
        tab.commit_edit(next_rope, next_sel, EditGroup::Discrete);
        self.search.recompute(&tab.rope, tab.selection.head);
        count
    }

    pub fn execute_command(&mut self, cmd: Command) {
        match cmd {
            Command::SetViewMode(mode) => {
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    tab.view_mode = mode;
                    tab.armed_styles.clear();
                    tab.last_frame = None;
                    tab.needs_scroll_to_caret = true;
                }
                self.layout_config.view_mode = mode;
                return;
            }
            Command::SetHeadingMarks(style) => {
                self.layout_config.heading_marks = style;
                return;
            }
            Command::ToggleLineNumbers => {
                self.layout_config.show_line_numbers = !self.layout_config.show_line_numbers;
                return;
            }
            Command::ToggleWordWrap => {
                self.layout_config.word_wrap = !self.layout_config.word_wrap;
                return;
            }
            Command::ToggleFocusMode => {
                self.layout_config.focus_mode = !self.layout_config.focus_mode;
                return;
            }
            Command::ToggleTypewriterMode => {
                self.layout_config.typewriter_mode = !self.layout_config.typewriter_mode;
                return;
            }
            Command::ZoomIn => {
                self.layout_config.base_font_px =
                    (self.layout_config.base_font_px + 1.5).clamp(10.0, 40.0);
                return;
            }
            Command::ZoomOut => {
                self.layout_config.base_font_px =
                    (self.layout_config.base_font_px - 1.5).clamp(10.0, 40.0);
                return;
            }
            Command::ZoomReset => {
                self.layout_config.base_font_px = 16.0;
                return;
            }
            _ => {}
        }

        let is_german = self.ui_language.is_german();
        let Some(tab) = self.tabs.get_mut(self.active_tab) else {
            return;
        };

        match cmd {
            Command::Undo => {
                tab.armed_styles.clear();
                if let Some((prev_rope, prev_sel)) = tab.history.undo() {
                    let old_r = std::mem::replace(&mut tab.rope, prev_rope);
                    tab.selection = prev_sel.clamp_to(&tab.rope);
                    tab.needs_scroll_to_caret = true;
                    if let Some((old_range, new_range)) = old_r.diff_byte_range(&tab.rope) {
                        tab.apply_incremental_rope_transition(&old_r, old_range, new_range);
                    }
                }
            }
            Command::Redo => {
                tab.armed_styles.clear();
                if let Some((next_rope, next_sel)) = tab.history.redo() {
                    let old_r = std::mem::replace(&mut tab.rope, next_rope);
                    tab.selection = next_sel.clamp_to(&tab.rope);
                    tab.needs_scroll_to_caret = true;
                    if let Some((old_range, new_range)) = old_r.diff_byte_range(&tab.rope) {
                        tab.apply_incremental_rope_transition(&old_r, old_range, new_range);
                    }
                }
            }
            Command::SelectAll => {
                tab.armed_styles.clear();
                tab.selection = Selection::new(0, tab.rope.len_bytes());
                tab.history.break_coalescing();
            }
            Command::ToggleBold => Self::apply_inline_on_tab(tab, ArmedStyle::Bold),
            Command::ToggleItalic => Self::apply_inline_on_tab(tab, ArmedStyle::Italic),
            Command::ToggleStrike => Self::apply_inline_on_tab(tab, ArmedStyle::Strike),
            Command::ToggleHighlight => Self::apply_inline_on_tab(tab, ArmedStyle::Highlight),
            Command::ToggleInlineCode => Self::apply_inline_on_tab(tab, ArmedStyle::Code),
            Command::SetHeading(lvl) => {
                tab.armed_styles.clear();
                let (r, s) = apply_heading(&tab.rope, tab.selection, lvl);
                tab.commit_edit(r, s, EditGroup::Discrete);
            }
            Command::ToggleBulletList => {
                tab.armed_styles.clear();
                let (r, s) = toggle_line_prefix(&tab.rope, tab.selection, LinePrefixKind::Bullet);
                tab.commit_edit(r, s, EditGroup::Discrete);
            }
            Command::ToggleOrderedList => {
                tab.armed_styles.clear();
                let (r, s) = toggle_line_prefix(&tab.rope, tab.selection, LinePrefixKind::Ordered);
                tab.commit_edit(r, s, EditGroup::Discrete);
            }
            Command::ToggleTaskList => {
                tab.armed_styles.clear();
                let (r, s) = toggle_line_prefix(&tab.rope, tab.selection, LinePrefixKind::Task);
                tab.commit_edit(r, s, EditGroup::Discrete);
            }
            Command::ToggleBlockQuote => {
                tab.armed_styles.clear();
                let (r, s) =
                    toggle_line_prefix(&tab.rope, tab.selection, LinePrefixKind::BlockQuote);
                tab.commit_edit(r, s, EditGroup::Discrete);
            }
            Command::InsertAlert(kind) => {
                tab.armed_styles.clear();
                let inside_alert = tab.blocks.iter().any(|b| {
                    matches!(b.kind, BlockKind::BlockQuote { alert: Some(_), .. })
                        && tab.selection.head >= b.byte_range.start
                        && tab.selection.head <= b.byte_range.end
                });
                if inside_alert
                    && let Some((r, s)) =
                        set_callout_kind(&tab.rope, &tab.blocks, tab.selection, None, Some(kind))
                {
                    tab.commit_edit(r, s, EditGroup::Discrete);
                } else {
                    let (r, s) = insert_alert_block(&tab.rope, tab.selection, kind);
                    tab.commit_edit(r, s, EditGroup::Discrete);
                }
            }
            Command::SetCalloutKind {
                open_line_start,
                kind,
            } => {
                tab.armed_styles.clear();
                if let Some((r, s)) =
                    set_callout_kind(&tab.rope, &tab.blocks, tab.selection, open_line_start, kind)
                {
                    tab.commit_edit(r, s, EditGroup::Discrete);
                }
            }
            Command::InsertHorizontalRule => {
                tab.armed_styles.clear();
                let (r, s) = insert_horizontal_rule(&tab.rope, tab.selection);
                tab.commit_edit(r, s, EditGroup::Discrete);
            }
            Command::InsertCodeFence { language } => {
                tab.armed_styles.clear();
                let (r, s) = insert_code_fence(&tab.rope, tab.selection, &language);
                tab.commit_edit(r, s, EditGroup::Discrete);
            }
            Command::SetCodeFenceLanguage {
                open_line_start,
                language,
            } => {
                tab.armed_styles.clear();
                if let Some((r, s)) = set_code_fence_language(
                    &tab.rope,
                    &tab.blocks,
                    tab.selection,
                    open_line_start,
                    &language,
                ) {
                    tab.commit_edit(r, s, EditGroup::Discrete);
                }
            }
            Command::InsertLink { text, url } => {
                tab.armed_styles.clear();
                let (r, s) = insert_link(&tab.rope, tab.selection, &text, &url);
                tab.commit_edit(r, s, EditGroup::Discrete);
            }
            Command::InsertImage {
                alt,
                url,
                caption,
                size,
            } => {
                tab.armed_styles.clear();
                let (r, s) = insert_image(
                    &tab.rope,
                    tab.selection,
                    &alt,
                    &url,
                    caption.as_deref(),
                    size,
                );
                tab.commit_edit(r, s, EditGroup::Discrete);
            }
            Command::UpdateImage {
                range,
                alt,
                url,
                caption,
                size,
            } => {
                tab.armed_styles.clear();
                if let Some((r, s)) = update_image(
                    &tab.rope,
                    tab.selection,
                    range,
                    &alt,
                    &url,
                    caption.as_deref(),
                    size,
                ) {
                    tab.commit_edit(r, s, EditGroup::Discrete);
                }
            }
            Command::InsertTable { cols, rows } => {
                tab.armed_styles.clear();
                let (r, s) =
                    insert_new_table_localized(&tab.rope, tab.selection, cols, rows, is_german);
                tab.commit_edit(r, s, EditGroup::Discrete);
            }
            Command::TableInsertRowAbove => {
                Self::exec_table_op(tab, TableOp::InsertRowAbove);
            }
            Command::TableInsertRowBelow => {
                Self::exec_table_op(tab, TableOp::InsertRowBelow);
            }
            Command::TableDeleteRow => {
                Self::exec_table_op(tab, TableOp::DeleteRow);
            }
            Command::TableInsertColLeft => {
                Self::exec_table_op(tab, TableOp::InsertColLeft);
            }
            Command::TableInsertColRight => {
                Self::exec_table_op(tab, TableOp::InsertColRight);
            }
            Command::TableDeleteCol => {
                Self::exec_table_op(tab, TableOp::DeleteCol);
            }
            Command::TableSetAlign(align) => {
                Self::exec_table_op(tab, TableOp::SetAlign(align));
            }
            Command::SetViewMode(_)
            | Command::SetHeadingMarks(_)
            | Command::ToggleLineNumbers
            | Command::ToggleWordWrap
            | Command::ToggleFocusMode
            | Command::ToggleTypewriterMode
            | Command::ZoomIn
            | Command::ZoomOut
            | Command::ZoomReset => {}
        }

        if !self.search.query.is_empty()
            && let Some(t) = self.tabs.get(self.active_tab)
        {
            self.search.recompute(&t.rope, t.selection.head);
        }
    }

    fn apply_inline_on_tab(tab: &mut DocumentTab, style: ArmedStyle) {
        match apply_inline_toggle(&tab.rope, tab.selection, style) {
            InlineOutcome::Edited(r, s) => {
                tab.armed_styles.clear();
                tab.commit_edit(r, s, EditGroup::Discrete);
            }
            InlineOutcome::ToggleArm(arm) => {
                if let Some(pos) = tab.armed_styles.iter().position(|x| *x == arm) {
                    tab.armed_styles.remove(pos);
                } else {
                    tab.armed_styles.push(arm);
                }
            }
        }
    }

    fn exec_table_op(tab: &mut DocumentTab, op: TableOp) {
        tab.armed_styles.clear();
        if let Some((r, s)) = apply_table_op(&tab.rope, &tab.blocks, tab.selection, op) {
            tab.commit_edit(r, s, EditGroup::Discrete);
        }
    }

    /// Commits typed or pasted text (including multi-line clipboard paste).
    pub fn insert_text(&mut self, text: &str) {
        let normalized = if text.contains('\r') {
            text.replace("\r\n", "\n").replace('\r', "\n")
        } else {
            text.to_owned()
        };
        let Some(tab) = self.tabs.get_mut(self.active_tab) else {
            return;
        };
        tab.ime_preedit = None;
        if let Some((r, s, group)) =
            handle_text_insert(&tab.rope, tab.selection, &mut tab.armed_styles, &normalized)
        {
            tab.commit_edit(r, s, group);
        }
        if !self.search.query.is_empty()
            && let Some(t) = self.tabs.get(self.active_tab)
        {
            self.search.recompute(&t.rope, t.selection.head);
        }
    }

    /// Updates Wayland/X11 IME composition preedit string (`QInputMethodEvent`).
    pub fn set_ime_preedit(&mut self, preedit: Option<&str>) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.ime_preedit = preedit.filter(|s| !s.is_empty()).map(str::to_owned);
        }
    }

    /// Handles navigation and editing keys (`Left`, `Right`, `Up`, `Down`, `Enter`, `Tab`, `Backspace`, etc.).
    pub fn handle_key(&mut self, key: KeyCode, mods: KeyModifiers) -> bool {
        let Some(tab) = self.tabs.get_mut(self.active_tab) else {
            return false;
        };

        match key {
            KeyCode::Escape => {
                tab.armed_styles.clear();
                tab.ime_preedit = None;
                if !tab.selection.is_empty() {
                    tab.selection = Selection::collapsed(tab.selection.head);
                    return true;
                }
                return false;
            }
            KeyCode::Enter => {
                tab.armed_styles.clear();
                let (r, s, grp) =
                    handle_enter(&tab.rope, &tab.blocks, tab.selection, mods, tab.view_mode);
                tab.commit_edit(r, s, grp);
            }
            KeyCode::Tab | KeyCode::Backtab => {
                tab.armed_styles.clear();
                let backwards = key == KeyCode::Backtab || mods.shift;
                let (r, s, maybe_grp) =
                    handle_tab(&tab.rope, &tab.blocks, tab.selection, backwards);
                if let Some(grp) = maybe_grp {
                    tab.commit_edit(r, s, grp);
                } else {
                    tab.selection = s.clamp_to(&tab.rope);
                    tab.needs_scroll_to_caret = true;
                    tab.history.break_coalescing();
                }
            }
            KeyCode::Backspace | KeyCode::Delete => {
                tab.armed_styles.clear();
                let forward = key == KeyCode::Delete;
                if let Some((r, s, grp)) =
                    handle_delete_key(&tab.rope, tab.selection, forward, mods.ctrl, tab.view_mode)
                {
                    tab.commit_edit(r, s, grp);
                }
            }
            KeyCode::Left | KeyCode::Right => {
                tab.armed_styles.clear();
                tab.preferred_x = None;
                tab.history.break_coalescing();
                let forward = key == KeyCode::Right;
                let next_head = if !mods.shift && !tab.selection.is_empty() {
                    if forward {
                        tab.selection.end()
                    } else {
                        tab.selection.start()
                    }
                } else {
                    step_horizontal(
                        &tab.rope,
                        tab.selection.head,
                        forward,
                        mods.ctrl,
                        tab.view_mode,
                    )
                };
                tab.selection = if mods.shift {
                    Selection::new(tab.selection.anchor, next_head).clamp_to(&tab.rope)
                } else {
                    Selection::collapsed(next_head).clamp_to(&tab.rope)
                };
                tab.needs_scroll_to_caret = true;
            }
            KeyCode::Up
            | KeyCode::Down
            | KeyCode::Home
            | KeyCode::End
            | KeyCode::PageUp
            | KeyCode::PageDown => {
                tab.armed_styles.clear();
                tab.history.break_coalescing();
                let next_head = if mods.ctrl && key == KeyCode::Home {
                    tab.preferred_x = None;
                    0
                } else if mods.ctrl && key == KeyCode::End {
                    tab.preferred_x = None;
                    tab.rope.len_bytes()
                } else {
                    let (off, px) = move_with_frame(
                        &tab.rope,
                        tab.last_frame.as_ref(),
                        tab.selection.head,
                        tab.preferred_x,
                        key,
                    );
                    tab.preferred_x = px;
                    off
                };
                tab.selection = if mods.shift {
                    Selection::new(tab.selection.anchor, next_head).clamp_to(&tab.rope)
                } else {
                    Selection::collapsed(next_head).clamp_to(&tab.rope)
                };
                tab.needs_scroll_to_caret = true;
            }
        }

        if !self.search.query.is_empty()
            && let Some(t) = self.tabs.get(self.active_tab)
        {
            self.search.recompute(&t.rope, t.selection.head);
        }
        true
    }

    /// Handles mouse press (`click_count`: `1` = single, `2` = double word select, `3` = triple line select).
    pub fn handle_pointer_press(
        &mut self,
        vx: f32,
        vy: f32,
        click_count: u8,
        mods: KeyModifiers,
    ) -> PointerAction {
        let Some(tab) = self.tabs.get_mut(self.active_tab) else {
            return PointerAction::Handled;
        };
        tab.armed_styles.clear();
        tab.preferred_x = None;
        tab.history.break_coalescing();

        let Some(frame) = tab.last_frame.as_ref() else {
            return PointerAction::Handled;
        };

        match frame.hit_test(vx, vy) {
            HitTarget::Checkbox {
                bracket_range,
                checked,
            } => {
                let replacement = if checked { "[ ]" } else { "[x]" };
                let mut next = tab.rope.clone();
                next.replace_range(bracket_range, replacement);
                let keep_sel = tab.selection;
                tab.commit_edit(next, keep_sel, EditGroup::Discrete);
                tab.needs_scroll_to_caret = false;
                PointerAction::Handled
            }
            HitTarget::CodeLanguageBadge {
                open_line_start,
                language,
            } => PointerAction::EditCodeLanguage {
                open_line_start,
                current_language: language,
            },
            HitTarget::CalloutBadge {
                open_line_start,
                kind,
            } => PointerAction::EditCalloutKind {
                open_line_start,
                current_kind: kind,
            },
            HitTarget::Text {
                byte_offset,
                link_url,
            } => {
                if mods.ctrl
                    && let Some(url) = link_url
                {
                    return PointerAction::OpenExternalUrl(url);
                }
                let clamped = tab.rope.clamp_char_boundary(byte_offset);
                if click_count >= 3 {
                    let lc = tab.rope.byte_to_line_col(clamped);
                    let lr = tab.rope.line_byte_range(lc.line);
                    tab.selection = Selection::new(lr.start, lr.end);
                } else if click_count == 2 {
                    if let Some(wr) = tab.rope.word_range_at(clamped) {
                        tab.selection = Selection::new(wr.start, wr.end);
                    } else {
                        tab.selection = Selection::collapsed(clamped);
                    }
                } else if mods.shift {
                    tab.selection = Selection::new(tab.selection.anchor, clamped);
                } else {
                    tab.selection = Selection::collapsed(clamped);
                }
                PointerAction::Handled
            }
        }
    }

    pub fn handle_pointer_drag(&mut self, vx: f32, vy: f32) {
        let Some(tab) = self.tabs.get_mut(self.active_tab) else {
            return;
        };
        let Some(frame) = tab.last_frame.as_ref() else {
            return;
        };
        if let HitTarget::Text { byte_offset, .. } = frame.hit_test(vx, vy) {
            let clamped = tab.rope.clamp_char_boundary(byte_offset);
            tab.selection = Selection::new(tab.selection.anchor, clamped);
        }
    }

    /// Places the collapsed caret at the hit-tested text offset for Drag & Drop insertion.
    pub fn drop_cursor_at(&mut self, vx: f32, vy: f32) {
        let Some(tab) = self.tabs.get_mut(self.active_tab) else {
            return;
        };
        tab.armed_styles.clear();
        tab.history.break_coalescing();
        let Some(frame) = tab.last_frame.as_ref() else {
            return;
        };
        if let HitTarget::Text { byte_offset, .. } = frame.hit_test(vx, vy) {
            let clamped = tab.rope.clamp_char_boundary(byte_offset);
            tab.selection = Selection::collapsed(clamped);
        }
    }

    /// Returns `true` when the pointer hovers an interactive element (checkbox, code/callout badge, or `Ctrl+Link`).
    #[must_use]
    pub fn hover_interactive_at(&self, vx: f32, vy: f32, ctrl: bool) -> bool {
        let Some(tab) = self.active_doc() else {
            return false;
        };
        let Some(frame) = tab.last_frame.as_ref() else {
            return false;
        };
        match frame.hit_test(vx, vy) {
            HitTarget::Checkbox { .. }
            | HitTarget::CodeLanguageBadge { .. }
            | HitTarget::CalloutBadge { .. } => true,
            HitTarget::Text { link_url, .. } => ctrl && link_url.is_some(),
        }
    }

    /// Hit-tests whether `(vx, vy)` lands on an inline Markdown image (or its caption / raw syntax).
    #[must_use]
    pub fn image_at(&self, vx: f32, vy: f32) -> Option<ImageAtPoint> {
        let tab = self.active_doc()?;
        if tab.view_mode == ViewMode::PlainText {
            return None;
        }
        let frame = tab.last_frame.as_ref()?;
        let doc_x = vx;
        let doc_y = vy + frame.scroll_y;

        for row in &frame.visual_rows {
            if doc_y < row.y_doc || doc_y > row.y_doc + row.height {
                continue;
            }
            for span in &row.spans {
                if span.is_hidden_marker
                    || doc_x < span.x - 2.0
                    || doc_x > span.x + span.width + 2.0
                {
                    continue;
                }
                let rel_b = span.measured.rel_byte_at_x((doc_x - span.x).max(0.0));
                let hit_b = span.doc_range.start.saturating_add(rel_b);
                let line_range = tab.rope.line_byte_range(row.line_idx);
                let line_text = tab.rope.slice_to_string(line_range.clone());
                for tok in parse_inline_tokens(line_range.start, &line_text) {
                    if let InlineTokenKind::Image {
                        alt,
                        src,
                        caption,
                        size,
                    } = tok.kind
                        && (span.doc_range == tok.range
                            || (hit_b >= tok.range.start && hit_b <= tok.range.end))
                    {
                        return Some(ImageAtPoint {
                            range: tok.range,
                            alt,
                            src,
                            caption,
                            size,
                        });
                    }
                }
            }
        }
        None
    }

    /// Computes active toolbar & status bar state at the current cursor position.
    #[must_use]
    pub fn toolbar_state(&self) -> ActiveToolbarState {
        let Some(tab) = self.active_doc() else {
            return ActiveToolbarState::default();
        };

        let caret = tab.rope.clamp_char_boundary(tab.selection.head);
        let lc = tab.rope.byte_to_line_col(caret);
        let line_range = tab.rope.line_byte_range(lc.line);
        let line_text = tab.rope.slice_to_string(line_range.clone());
        let inline_flags = active_styles_at(line_range.start, &line_text, caret);

        let mut state = ActiveToolbarState {
            heading_level: 0,
            bold: inline_flags.contains(StyleFlags::BOLD)
                || tab.armed_styles.contains(&ArmedStyle::Bold),
            italic: inline_flags.contains(StyleFlags::ITALIC)
                || tab.armed_styles.contains(&ArmedStyle::Italic),
            strike: inline_flags.contains(StyleFlags::STRIKE)
                || tab.armed_styles.contains(&ArmedStyle::Strike),
            highlight: inline_flags.contains(StyleFlags::HIGHLIGHT)
                || tab.armed_styles.contains(&ArmedStyle::Highlight),
            inline_code: inline_flags.contains(StyleFlags::CODE)
                || tab.armed_styles.contains(&ArmedStyle::Code),
            in_table: false,
            table_col_align: ColumnAlign::Left,
            in_code_block: false,
            code_language: String::new(),
            in_blockquote: false,
            in_bullet_list: false,
            in_ordered_list: false,
            in_task_list: false,
            can_undo: tab.history.can_undo(),
            can_redo: tab.history.can_redo(),
            is_dirty: tab.is_dirty(),
            line_1based: lc.line.saturating_add(1),
            col_1based: lc.col_chars.saturating_add(1),
            char_count: tab.rope.len_chars(),
            word_count: tab.word_count,
            encoding: tab.encoding,
            line_ending: tab.line_ending,
            view_mode: tab.view_mode,
        };

        for block in &tab.blocks {
            if caret < block.byte_range.start || caret > block.byte_range.end {
                continue;
            }
            match &block.kind {
                BlockKind::Heading { level, .. } => {
                    if lc.line >= block.line_range.start && lc.line < block.line_range.end {
                        state.heading_level = *level;
                    }
                }
                BlockKind::Table(_) => {
                    if lc.line >= block.line_range.start && lc.line < block.line_range.end {
                        state.in_table = true;
                        if let Some((_, tbl, tc)) = locate_table_cursor(&tab.blocks, caret)
                            && let Some(align) = tbl.alignments.get(tc.col)
                        {
                            state.table_col_align = *align;
                        }
                    }
                }
                BlockKind::CodeFence { language, .. } => {
                    state.in_code_block = true;
                    state.code_language.clone_from(language);
                }
                BlockKind::BlockQuote { .. } => state.in_blockquote = true,
                BlockKind::ListItem {
                    ordered,
                    task,
                    line_idx,
                    ..
                } => {
                    if *line_idx == lc.line {
                        if task.is_some() {
                            state.in_task_list = true;
                        } else if ordered.is_some() {
                            state.in_ordered_list = true;
                        } else {
                            state.in_bullet_list = true;
                        }
                    }
                }
                BlockKind::BlankLines | BlockKind::Paragraph { .. } | BlockKind::ThematicBreak => {}
            }
        }
        state
    }

    /// Clears cached block heights across all open tabs (e.g. after a font family or image change).
    pub fn clear_layout_caches(&mut self) {
        for tab in &mut self.tabs {
            tab.height_cache.clear();
            tab.last_frame = None;
        }
    }

    /// Builds a virtualized [`ViewportFrame`] for the active tab, auto-scrolling to keep the caret visible.
    pub fn render_viewport(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        measurer: &mut dyn TextMeasurer,
    ) -> Option<&ViewportFrame> {
        let tab = self.tabs.get_mut(self.active_tab)?;
        self.layout_config.view_mode = tab.view_mode;

        let mut frame = build_viewport_frame_cached(
            &ViewportRequest {
                rope: &tab.rope,
                blocks: &tab.blocks,
                selection: tab.selection,
                ime_preedit: tab.ime_preedit.as_deref(),
                search_matches: &self.search.matches,
                active_match_idx: self.search.active_index,
                config: &self.layout_config,
                viewport_w,
                viewport_h,
                scroll_y: tab.scroll_y,
            },
            measurer,
            Some(&mut tab.height_cache),
        );

        // Auto-scroll caret into view (or center vertically in Typewriter mode!)
        if tab.needs_scroll_to_caret || self.layout_config.typewriter_mode {
            let caret_top = frame.caret_doc_rect.y;
            let caret_bot = caret_top + frame.caret_doc_rect.h;
            let margin = 36.0f32;
            let mut desired_scroll = tab.scroll_y;

            if self.layout_config.typewriter_mode && tab.needs_scroll_to_caret {
                desired_scroll = (caret_top - viewport_h * 0.45).max(0.0);
            } else if caret_top < tab.scroll_y + margin {
                desired_scroll = (caret_top - margin).max(0.0);
            } else if caret_bot > tab.scroll_y + viewport_h - margin {
                desired_scroll = (caret_bot - viewport_h + margin).max(0.0);
            }

            let max_scroll = (frame.total_height - viewport_h * 0.5).max(0.0);
            desired_scroll = desired_scroll.clamp(0.0, max_scroll);
            tab.needs_scroll_to_caret = false;

            if (desired_scroll - tab.scroll_y).abs() > 0.5 {
                tab.scroll_y = desired_scroll;
                frame = build_viewport_frame_cached(
                    &ViewportRequest {
                        rope: &tab.rope,
                        blocks: &tab.blocks,
                        selection: tab.selection,
                        ime_preedit: tab.ime_preedit.as_deref(),
                        search_matches: &self.search.matches,
                        active_match_idx: self.search.active_index,
                        config: &self.layout_config,
                        viewport_w,
                        viewport_h,
                        scroll_y: tab.scroll_y,
                    },
                    measurer,
                    Some(&mut tab.height_cache),
                );
            }
        }

        let _ = locate_table_cursor;
        tab.last_frame = Some(frame);
        tab.last_frame.as_ref()
    }

    pub fn render_viewport_fallback(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
    ) -> Option<&ViewportFrame> {
        let mut fallback = FallbackMeasurer;
        self.render_viewport(viewport_w, viewport_h, &mut fallback)
    }

    /// Captures all session/tab inputs that affect the rendered [`ViewportFrame`] so
    /// FFI callers can skip layout recomputation on cursor-blink or expose repaints.
    #[must_use]
    pub fn render_state_stamp(&self) -> Option<RenderStateStamp> {
        let tab = self.tabs.get(self.active_tab)?;
        Some(RenderStateStamp {
            tab_id: tab.id,
            revision: tab.revision,
            selection: tab.selection,
            ime_preedit: tab.ime_preedit.clone(),
            view_mode: tab.view_mode,
            scroll_y_bits: tab.scroll_y.to_bits(),
            needs_scroll_to_caret: tab.needs_scroll_to_caret,
            search_query: self.search.query.clone(),
            search_options: self.search.options,
            search_match_count: self.search.matches.len(),
            search_active_index: self.search.active_index,
            heading_marks: self.layout_config.heading_marks,
            base_font_bits: self.layout_config.base_font_px.to_bits(),
            line_spacing_bits: self.layout_config.line_spacing_scale.to_bits(),
            max_prose_width_bits: self.layout_config.max_prose_width_px.to_bits(),
            word_wrap: self.layout_config.word_wrap,
            show_line_numbers: self.layout_config.show_line_numbers,
            focus_mode: self.layout_config.focus_mode,
            typewriter_mode: self.layout_config.typewriter_mode,
            ui_german: self.layout_config.ui_german,
            palette: self.layout_config.palette,
        })
    }
}

/// Exact equality stamp covering all document, cursor, search, and theme inputs to [`Session::render_viewport`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderStateStamp {
    pub tab_id: u64,
    pub revision: u64,
    pub selection: Selection,
    pub ime_preedit: Option<String>,
    pub view_mode: ViewMode,
    pub scroll_y_bits: u32,
    pub needs_scroll_to_caret: bool,
    pub search_query: String,
    pub search_options: SearchOptions,
    pub search_match_count: usize,
    pub search_active_index: Option<usize>,
    pub heading_marks: HeadingMarkStyle,
    pub base_font_bits: u32,
    pub line_spacing_bits: u32,
    pub max_prose_width_bits: u32,
    pub word_wrap: bool,
    pub show_line_numbers: bool,
    pub focus_mode: bool,
    pub typewriter_mode: bool,
    pub ui_german: bool,
    pub palette: kmark_layout::ThemePalette,
}

fn default_view_mode_for_path(path: &Path) -> ViewMode {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return ViewMode::Wysiwyg;
    };
    match ext.to_ascii_lowercase().as_str() {
        "md" | "markdown" | "mdown" | "mkd" | "mdx" => ViewMode::Wysiwyg,
        _ => ViewMode::PlainText,
    }
}
