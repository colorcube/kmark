use std::ops::Range;

use kmark_buffer::{Rope, Selection};
use kmark_layout::{HeadingMarkStyle, ViewMode};
use kmark_syntax::{
    AlertKind, Block, BlockKind, ColumnAlign, ImageSize, TableBlock, TableRow,
    format_markdown_image, parse_atx_heading, parse_blocks,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ArmedStyle {
    Bold,
    Italic,
    Strike,
    Highlight,
    Code,
}

impl ArmedStyle {
    #[must_use]
    pub const fn delimiter(self) -> &'static str {
        match self {
            Self::Bold => "**",
            Self::Italic => "*",
            Self::Strike => "~~",
            Self::Highlight => "==",
            Self::Code => "`",
        }
    }
}

/// Every user-invocable editor command (dispatched identically from KDE menus, toolbars, shortcuts, FFI, and CLI).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Command {
    Undo,
    Redo,
    SelectAll,
    ToggleBold,
    ToggleItalic,
    ToggleStrike,
    ToggleHighlight,
    ToggleInlineCode,
    SetHeading(u8),
    ToggleBulletList,
    ToggleOrderedList,
    ToggleTaskList,
    ToggleBlockQuote,
    InsertAlert(AlertKind),
    SetCalloutKind {
        open_line_start: Option<usize>,
        kind: Option<AlertKind>,
    },
    InsertHorizontalRule,
    InsertCodeFence {
        language: String,
    },
    SetCodeFenceLanguage {
        open_line_start: Option<usize>,
        language: String,
    },
    InsertLink {
        text: String,
        url: String,
    },
    InsertImage {
        alt: String,
        url: String,
        caption: Option<String>,
        size: Option<ImageSize>,
    },
    UpdateImage {
        range: Range<usize>,
        alt: String,
        url: String,
        caption: Option<String>,
        size: Option<ImageSize>,
    },
    InsertTable {
        cols: usize,
        rows: usize,
    },
    TableInsertRowAbove,
    TableInsertRowBelow,
    TableDeleteRow,
    TableInsertColLeft,
    TableInsertColRight,
    TableDeleteCol,
    TableSetAlign(ColumnAlign),
    SetViewMode(ViewMode),
    SetHeadingMarks(HeadingMarkStyle),
    ToggleLineNumbers,
    ToggleWordWrap,
    ToggleFocusMode,
    ToggleTypewriterMode,
    ZoomIn,
    ZoomOut,
    ZoomReset,
}

#[derive(Clone, Copy, Debug)]
pub struct CommandDescriptor {
    pub id: &'static str,
    pub label_de: &'static str,
    pub shortcut: &'static str,
    pub icon_theme: &'static str,
    pub category_de: &'static str,
}

pub const COMMAND_CATALOG: &[CommandDescriptor] = &[
    CommandDescriptor {
        id: "edit.undo",
        label_de: "Rückgängig",
        shortcut: "Ctrl+Z",
        icon_theme: "edit-undo",
        category_de: "Bearbeiten",
    },
    CommandDescriptor {
        id: "edit.redo",
        label_de: "Wiederherstellen",
        shortcut: "Ctrl+Shift+Z",
        icon_theme: "edit-redo",
        category_de: "Bearbeiten",
    },
    CommandDescriptor {
        id: "format.bold",
        label_de: "Fett",
        shortcut: "Ctrl+B",
        icon_theme: "format-text-bold",
        category_de: "Format",
    },
    CommandDescriptor {
        id: "format.italic",
        label_de: "Kursiv",
        shortcut: "Ctrl+I",
        icon_theme: "format-text-italic",
        category_de: "Format",
    },
    CommandDescriptor {
        id: "format.strike",
        label_de: "Durchgestrichen",
        shortcut: "Ctrl+Shift+X",
        icon_theme: "format-text-strikethrough",
        category_de: "Format",
    },
    CommandDescriptor {
        id: "format.highlight",
        label_de: "Textmarker",
        shortcut: "Ctrl+Shift+H",
        icon_theme: "format-text-color",
        category_de: "Format",
    },
    CommandDescriptor {
        id: "format.code",
        label_de: "Inline-Code",
        shortcut: "Ctrl+E",
        icon_theme: "format-text-code",
        category_de: "Format",
    },
    CommandDescriptor {
        id: "format.heading.0",
        label_de: "Absatz (Normaltext)",
        shortcut: "Ctrl+0",
        icon_theme: "format-text-Clear",
        category_de: "Format",
    },
    CommandDescriptor {
        id: "format.heading.1",
        label_de: "Überschrift 1",
        shortcut: "Ctrl+1",
        icon_theme: "format-text-uppercase",
        category_de: "Format",
    },
    CommandDescriptor {
        id: "format.heading.2",
        label_de: "Überschrift 2",
        shortcut: "Ctrl+2",
        icon_theme: "format-text-uppercase",
        category_de: "Format",
    },
    CommandDescriptor {
        id: "format.heading.3",
        label_de: "Überschrift 3",
        shortcut: "Ctrl+3",
        icon_theme: "format-text-uppercase",
        category_de: "Format",
    },
    CommandDescriptor {
        id: "format.list.bullet",
        label_de: "Aufzählungsliste",
        shortcut: "Ctrl+Shift+U",
        icon_theme: "format-list-unordered",
        category_de: "Format",
    },
    CommandDescriptor {
        id: "format.list.ordered",
        label_de: "Nummerierte Liste",
        shortcut: "Ctrl+Shift+O",
        icon_theme: "format-list-ordered",
        category_de: "Format",
    },
    CommandDescriptor {
        id: "format.list.task",
        label_de: "Aufgabenliste",
        shortcut: "Ctrl+Shift+T",
        icon_theme: "view-task",
        category_de: "Format",
    },
    CommandDescriptor {
        id: "format.blockquote",
        label_de: "Zitatblock",
        shortcut: "Ctrl+Shift+Q",
        icon_theme: "format-text-blockquote",
        category_de: "Format",
    },
    CommandDescriptor {
        id: "insert.link",
        label_de: "Link einfügen…",
        shortcut: "Ctrl+K",
        icon_theme: "insert-link",
        category_de: "Einfügen",
    },
    CommandDescriptor {
        id: "insert.image",
        label_de: "Bild einfügen…",
        shortcut: "Ctrl+Shift+I",
        icon_theme: "insert-image",
        category_de: "Einfügen",
    },
    CommandDescriptor {
        id: "insert.code_fence",
        label_de: "Codeblock einfügen",
        shortcut: "Ctrl+Shift+C",
        icon_theme: "code-context",
        category_de: "Einfügen",
    },
    CommandDescriptor {
        id: "format.code_fence_lang",
        label_de: "Codeblock-Sprache ändern…",
        shortcut: "Ctrl+Alt+L",
        icon_theme: "code-context",
        category_de: "Format",
    },
    CommandDescriptor {
        id: "insert.hr",
        label_de: "Trennlinie einfügen",
        shortcut: "",
        icon_theme: "insert-horizontal-rule",
        category_de: "Einfügen",
    },
    CommandDescriptor {
        id: "table.insert",
        label_de: "Tabelle einfügen…",
        shortcut: "",
        icon_theme: "insert-table",
        category_de: "Tabelle",
    },
    CommandDescriptor {
        id: "table.row.above",
        label_de: "Zeile oberhalb einfügen",
        shortcut: "",
        icon_theme: "edit-table-insert-row-above",
        category_de: "Tabelle",
    },
    CommandDescriptor {
        id: "table.row.below",
        label_de: "Zeile unterhalb einfügen",
        shortcut: "",
        icon_theme: "edit-table-insert-row-below",
        category_de: "Tabelle",
    },
    CommandDescriptor {
        id: "table.row.delete",
        label_de: "Zeile löschen",
        shortcut: "",
        icon_theme: "edit-table-delete-row",
        category_de: "Tabelle",
    },
    CommandDescriptor {
        id: "table.col.left",
        label_de: "Spalte links einfügen",
        shortcut: "",
        icon_theme: "edit-table-insert-column-left",
        category_de: "Tabelle",
    },
    CommandDescriptor {
        id: "table.col.right",
        label_de: "Spalte rechts einfügen",
        shortcut: "",
        icon_theme: "edit-table-insert-column-right",
        category_de: "Tabelle",
    },
    CommandDescriptor {
        id: "table.col.delete",
        label_de: "Spalte löschen",
        shortcut: "",
        icon_theme: "edit-table-delete-column",
        category_de: "Tabelle",
    },
    CommandDescriptor {
        id: "view.mode.wysiwyg",
        label_de: "Formatiert (WYSIWYG)",
        shortcut: "Ctrl+Alt+1",
        icon_theme: "view-preview",
        category_de: "Ansicht",
    },
    CommandDescriptor {
        id: "view.mode.live",
        label_de: "Live-Vorschau",
        shortcut: "Ctrl+Alt+2",
        icon_theme: "view-split-left-right",
        category_de: "Ansicht",
    },
    CommandDescriptor {
        id: "view.mode.source",
        label_de: "Markdown-Quelltext",
        shortcut: "Ctrl+Alt+3",
        icon_theme: "text-x-markdown",
        category_de: "Ansicht",
    },
    CommandDescriptor {
        id: "view.mode.plaintext",
        label_de: "Reintext (TXT)",
        shortcut: "Ctrl+Alt+4",
        icon_theme: "text-plain",
        category_de: "Ansicht",
    },
    CommandDescriptor {
        id: "view.line_numbers",
        label_de: "Zeilennummern anzeigen",
        shortcut: "F11",
        icon_theme: "view-list-numeric",
        category_de: "Ansicht",
    },
];

pub enum InlineOutcome {
    Edited(Rope, Selection),
    ToggleArm(ArmedStyle),
}

/// Toggles an inline style (`**`, `*`, `~~`, `==`, `` ` ``) over `selection`, a touched word, or arms it.
#[must_use]
pub fn apply_inline_toggle(rope: &Rope, selection: Selection, style: ArmedStyle) -> InlineOutcome {
    let delim = style.delimiter();
    let dlen = delim.len();

    if !selection.is_empty() {
        let range = selection.range();
        let selected_text = rope.slice_to_string(range.clone());

        // Case 1a: Selected text itself begins and ends with `delim`
        if selected_text.len() >= dlen.saturating_mul(2)
            && selected_text.starts_with(delim)
            && selected_text.ends_with(delim)
            && let Some(inner) = selected_text.get(dlen..selected_text.len().saturating_sub(dlen))
        {
            let mut next = rope.clone();
            next.replace_range(range.clone(), inner);
            return InlineOutcome::Edited(
                next,
                Selection::new(range.start, range.start.saturating_add(inner.len())),
            );
        }

        // Case 1b: Delimiters immediately surround `range` outside the selection
        if range.start >= dlen && range.end.saturating_add(dlen) <= rope.len_bytes() {
            let before = rope.slice_to_string(range.start.saturating_sub(dlen)..range.start);
            let after = rope.slice_to_string(range.end..range.end.saturating_add(dlen));
            if before == delim && after == delim {
                let mut next = rope.clone();
                let outer = range.start.saturating_sub(dlen)..range.end.saturating_add(dlen);
                next.replace_range(outer.clone(), &selected_text);
                let new_start = outer.start;
                return InlineOutcome::Edited(
                    next,
                    Selection::new(new_start, new_start.saturating_add(selected_text.len())),
                );
            }
        }

        // Otherwise wrap selection with `delim`
        let wrapped = format!("{delim}{selected_text}{delim}");
        let mut next = rope.clone();
        next.replace_range(range.clone(), &wrapped);
        let inner_start = range.start.saturating_add(dlen);
        let inner_end = inner_start.saturating_add(selected_text.len());
        return InlineOutcome::Edited(next, Selection::new(inner_start, inner_end));
    }

    // Collapsed caret: check if caret is inside an already-wrapped word or touches a word
    let caret = rope.clamp_char_boundary(selection.head);

    // Check if caret sits right between empty delimiters `**|**` -> remove empty delimiters!
    if caret >= dlen && caret.saturating_add(dlen) <= rope.len_bytes() {
        let before = rope.slice_to_string(caret.saturating_sub(dlen)..caret);
        let after = rope.slice_to_string(caret..caret.saturating_add(dlen));
        if before == delim && after == delim {
            let mut next = rope.clone();
            let start = caret.saturating_sub(dlen);
            next.replace_range(start..caret.saturating_add(dlen), "");
            return InlineOutcome::Edited(next, Selection::collapsed(start));
        }
    }

    if let Some(word_range) = rope.word_range_at(caret) {
        let word = rope.slice_to_string(word_range.clone());
        let rel_caret = caret.saturating_sub(word_range.start);

        // Is this word already immediately surrounded by `delim`?
        if word_range.start >= dlen && word_range.end.saturating_add(dlen) <= rope.len_bytes() {
            let before =
                rope.slice_to_string(word_range.start.saturating_sub(dlen)..word_range.start);
            let after = rope.slice_to_string(word_range.end..word_range.end.saturating_add(dlen));
            if before == delim && after == delim {
                let mut next = rope.clone();
                let outer_start = word_range.start.saturating_sub(dlen);
                next.replace_range(outer_start..word_range.end.saturating_add(dlen), &word);
                return InlineOutcome::Edited(
                    next,
                    Selection::collapsed(outer_start.saturating_add(rel_caret)),
                );
            }
        }

        // Wrap touched word
        let wrapped = format!("{delim}{word}{delim}");
        let mut next = rope.clone();
        next.replace_range(word_range.clone(), &wrapped);
        let new_caret = word_range
            .start
            .saturating_add(dlen)
            .saturating_add(rel_caret);
        return InlineOutcome::Edited(next, Selection::collapsed(new_caret));
    }

    // Outside any word -> arm formatting without polluting the buffer with `****`!
    InlineOutcome::ToggleArm(style)
}

/// Sets ATX heading level (`0` = paragraph, `1..=6` = `#..######`). Toggles back to `0` if already at `level`.
#[must_use]
pub fn apply_heading(rope: &Rope, selection: Selection, level: u8) -> (Rope, Selection) {
    let clamped_level = level.min(6);
    let lc = rope.byte_to_line_col(selection.head);
    let line_range = rope.line_byte_range(lc.line);
    let line_text = rope.slice_to_string(line_range.clone());

    let (existing_level, prefix_bytes) =
        if let Some((lvl, content_start)) = parse_atx_heading(&line_text) {
            (lvl, content_start)
        } else {
            (0u8, 0usize)
        };

    let target_level = if clamped_level > 0 && existing_level == clamped_level {
        0
    } else {
        clamped_level
    };

    let clean_body = line_text.get(prefix_bytes..).unwrap_or(&line_text);
    let new_prefix = if target_level == 0 {
        String::new()
    } else {
        format!("{} ", "#".repeat(usize::from(target_level)))
    };

    let new_line = format!("{new_prefix}{clean_body}");
    let mut next = rope.clone();
    next.replace_range(line_range.clone(), &new_line);

    let old_rel = selection.head.saturating_sub(line_range.start);
    let body_rel = old_rel.saturating_sub(prefix_bytes);
    let new_caret = line_range
        .start
        .saturating_add(new_prefix.len())
        .saturating_add(body_rel)
        .min(line_range.start.saturating_add(new_line.len()));
    let clamped_caret = next.clamp_char_boundary(new_caret);

    (next, Selection::collapsed(clamped_caret))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinePrefixKind {
    Bullet,
    Ordered,
    Task,
    BlockQuote,
}

#[must_use]
pub fn toggle_line_prefix(
    rope: &Rope,
    selection: Selection,
    kind: LinePrefixKind,
) -> (Rope, Selection) {
    let start_line = rope.byte_to_line_col(selection.start()).line;
    let end_line = rope.byte_to_line_col(selection.end()).line;

    let mut lines = Vec::new();
    for line_idx in start_line..=end_line {
        lines.push(rope.line_text(line_idx));
    }

    let all_already_match = lines.iter().filter(|l| !l.trim().is_empty()).all(|l| {
        let t = l.trim_start();
        match kind {
            LinePrefixKind::Task => {
                t.starts_with("- [ ] ") || t.starts_with("- [x] ") || t.starts_with("- [X] ")
            }
            LinePrefixKind::Bullet => {
                (t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ "))
                    && !(t.starts_with("- [ ] ")
                        || t.starts_with("- [x] ")
                        || t.starts_with("- [X] "))
            }
            LinePrefixKind::Ordered => strip_ordered_marker(t).is_some(),
            LinePrefixKind::BlockQuote => t.starts_with('>'),
        }
    });

    let mut transformed = Vec::with_capacity(lines.len());
    for (idx, line) in lines.iter().enumerate() {
        let indent_len = line.len().saturating_sub(line.trim_start().len());
        let indent = line.get(..indent_len).unwrap_or("");
        let trimmed = line.get(indent_len..).unwrap_or("");
        let stripped = strip_any_list_or_quote_prefix(trimmed);

        if all_already_match {
            transformed.push(format!("{indent}{stripped}"));
        } else {
            let prefix = match kind {
                LinePrefixKind::Bullet => "- ".to_owned(),
                LinePrefixKind::Ordered => format!("{}. ", idx.saturating_add(1)),
                LinePrefixKind::Task => "- [ ] ".to_owned(),
                LinePrefixKind::BlockQuote => "> ".to_owned(),
            };
            transformed.push(format!("{indent}{prefix}{stripped}"));
        }
    }

    let full_start = rope.line_byte_range(start_line).start;
    let full_end = rope.line_byte_range(end_line).end;
    let replacement = transformed.join("\n");

    let mut next = rope.clone();
    next.replace_range(full_start..full_end, &replacement);
    let caret = next.clamp_char_boundary(full_start.saturating_add(replacement.len()));
    (next, Selection::collapsed(caret))
}

#[must_use]
pub fn strip_ordered_marker(s: &str) -> Option<&str> {
    let digits = s.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let rest = s.get(digits..)?;
    if let Some(after_dot) = rest.strip_prefix(". ") {
        Some(after_dot)
    } else {
        rest.strip_prefix(") ")
    }
}

fn strip_any_list_or_quote_prefix(trimmed: &str) -> &str {
    for task_pfx in ["- [ ] ", "- [x] ", "- [X] ", "* [ ] ", "* [x] "] {
        if let Some(rest) = trimmed.strip_prefix(task_pfx) {
            return rest;
        }
    }
    for bullet_pfx in ["- ", "* ", "+ "] {
        if let Some(rest) = trimmed.strip_prefix(bullet_pfx) {
            return rest;
        }
    }
    if let Some(rest) = strip_ordered_marker(trimmed) {
        return rest;
    }
    if let Some(rest) = trimmed.strip_prefix("> ") {
        return rest;
    }
    if let Some(rest) = trimmed.strip_prefix('>') {
        return rest;
    }
    trimmed
}

#[must_use]
pub fn insert_alert_block(rope: &Rope, selection: Selection, kind: AlertKind) -> (Rope, Selection) {
    let tag = kind.tag_str();
    let lc = rope.byte_to_line_col(selection.head);
    let line_range = rope.line_byte_range(lc.line);
    let line_text = rope.slice_to_string(line_range.clone());
    let prefix_nl = if line_text.trim().is_empty() {
        ""
    } else {
        "\n\n"
    };
    let snippet = format!("{prefix_nl}> [!{tag}]\n> ");
    let insert_at = if line_text.trim().is_empty() {
        line_range.start
    } else {
        line_range.end
    };
    let mut next = rope.clone();
    next.replace_range(insert_at..line_range.end, &snippet);
    let caret = insert_at.saturating_add(snippet.len());
    (next, Selection::collapsed(caret))
}

/// Switches the [`AlertKind`] of an existing `BlockQuote` / Callout block (identified by
/// `open_line_start` from a badge click or containing `selection.head`), preserving indentation,
/// quote depth, and optional custom title. Passing `kind = None` converts a Callout back to a
/// plain blockquote.
#[must_use]
pub fn set_callout_kind(
    rope: &Rope,
    blocks: &[Block],
    selection: Selection,
    open_line_start: Option<usize>,
    kind: Option<AlertKind>,
) -> Option<(Rope, Selection)> {
    for block in blocks {
        let BlockKind::BlockQuote {
            alert,
            ref custom_title,
            ref lines,
        } = block.kind
        else {
            continue;
        };
        let Some(first_ql) = lines.first() else {
            continue;
        };

        let matched = match open_line_start {
            Some(target) => {
                first_ql.marker_range.start == target || block.byte_range.start == target
            }
            None => {
                selection.head >= block.byte_range.start && selection.head <= block.byte_range.end
            }
        };
        if !matched {
            continue;
        }

        let raw_marker = rope.slice_to_string(first_ql.marker_range.clone());
        let marker_prefix = if raw_marker.ends_with(' ') {
            raw_marker
        } else {
            format!("{} ", raw_marker.trim_end())
        };

        let (replace_range, replacement) = match (alert, kind) {
            (Some(_), Some(new_kind)) => {
                let tag = new_kind.tag_str();
                let new_open = if let Some(title) = custom_title
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    format!("{marker_prefix}[!{tag}] {title}")
                } else {
                    format!("{marker_prefix}[!{tag}]")
                };
                (
                    first_ql.marker_range.start..first_ql.content_range.end,
                    new_open,
                )
            }
            (Some(_), None) => {
                if let Some(title) = custom_title
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    (
                        first_ql.marker_range.start..first_ql.content_range.end,
                        format!("{marker_prefix}{title}"),
                    )
                } else if let Some(second_ql) = lines.get(1) {
                    (
                        first_ql.marker_range.start..second_ql.marker_range.start,
                        String::new(),
                    )
                } else {
                    (
                        first_ql.marker_range.start..first_ql.content_range.end,
                        marker_prefix,
                    )
                }
            }
            (None, Some(new_kind)) => {
                let tag = new_kind.tag_str();
                let insert_header = format!("{marker_prefix}[!{tag}]\n");
                (
                    first_ql.marker_range.start..first_ql.marker_range.start,
                    insert_header,
                )
            }
            (None, None) => return None,
        };

        let old_len = replace_range.len();
        let new_len = replacement.len();
        let mut next = rope.clone();
        next.replace_range(replace_range.clone(), &replacement);

        let adjust = |pos: usize| -> usize {
            if pos <= replace_range.start {
                pos
            } else if pos <= replace_range.end {
                replace_range.start.saturating_add(new_len)
            } else if new_len >= old_len {
                pos.saturating_add(new_len - old_len)
            } else {
                pos.saturating_sub(old_len - new_len)
            }
        };
        let next_sel = Selection::new(adjust(selection.anchor), adjust(selection.head));
        return Some((next, next_sel));
    }
    None
}

#[must_use]
pub fn insert_horizontal_rule(rope: &Rope, selection: Selection) -> (Rope, Selection) {
    let lc = rope.byte_to_line_col(selection.head);
    let line_range = rope.line_byte_range(lc.line);
    let line_text = rope.slice_to_string(line_range.clone());
    let (replace_range, snippet) = if line_text.trim().is_empty() {
        (line_range, "---\n\n".to_owned())
    } else {
        (line_range.end..line_range.end, "\n\n---\n\n".to_owned())
    };
    let start = replace_range.start;
    let mut next = rope.clone();
    next.replace_range(replace_range, &snippet);
    (
        next,
        Selection::collapsed(start.saturating_add(snippet.len())),
    )
}

#[must_use]
pub fn insert_code_fence(rope: &Rope, selection: Selection, language: &str) -> (Rope, Selection) {
    let lang = language.trim();
    if !selection.is_empty() {
        let range = selection.range();
        let body = rope.slice_to_string(range.clone());
        let snippet = format!("```{lang}\n{body}\n```");
        let mut next = rope.clone();
        next.replace_range(range.clone(), &snippet);
        let caret = range
            .start
            .saturating_add(3)
            .saturating_add(lang.len())
            .saturating_add(1);
        return (next, Selection::collapsed(caret));
    }

    let lc = rope.byte_to_line_col(selection.head);
    let line_range = rope.line_byte_range(lc.line);
    let line_text = rope.slice_to_string(line_range.clone());
    let (at, lead) = if line_text.trim().is_empty() {
        (line_range, "")
    } else {
        (line_range.end..line_range.end, "\n\n")
    };
    let start = at.start;
    let open = format!("{lead}```{lang}\n");
    let full = format!("{open}\n```\n");
    let mut next = rope.clone();
    next.replace_range(at, &full);
    (next, Selection::collapsed(start.saturating_add(open.len())))
}

/// Updates the info-string language of an existing `CodeFence` block (either identified by
/// `open_line_start` from a badge click or containing `selection.head`).
#[must_use]
pub fn set_code_fence_language(
    rope: &Rope,
    blocks: &[Block],
    selection: Selection,
    open_line_start: Option<usize>,
    language: &str,
) -> Option<(Rope, Selection)> {
    let clean_lang = language.trim();
    for block in blocks {
        let BlockKind::CodeFence {
            fence_char,
            fence_len,
            ref open_line_range,
            ..
        } = block.kind
        else {
            continue;
        };

        let matched = match open_line_start {
            Some(target) => open_line_range.start == target,
            None => {
                selection.head >= block.byte_range.start && selection.head <= block.byte_range.end
            }
        };
        if !matched {
            continue;
        }

        let old_line = rope.slice_to_string(open_line_range.clone());
        let indent_len = old_line
            .bytes()
            .take_while(|b| *b == b' ' || *b == b'\t')
            .count();
        let indent = old_line.get(..indent_len).unwrap_or("");
        let fence_str: String =
            std::iter::repeat_n(char::from(fence_char), fence_len.max(3)).collect();
        let new_open = format!("{indent}{fence_str}{clean_lang}");

        let old_len = open_line_range.len();
        let new_len = new_open.len();
        let mut next = rope.clone();
        next.replace_range(open_line_range.clone(), &new_open);

        let adjust = |pos: usize| -> usize {
            if pos <= open_line_range.start {
                pos
            } else if pos <= open_line_range.end {
                open_line_range.start.saturating_add(new_len)
            } else if new_len >= old_len {
                pos.saturating_add(new_len - old_len)
            } else {
                pos.saturating_sub(old_len - new_len)
            }
        };
        let next_sel = Selection::new(adjust(selection.anchor), adjust(selection.head));
        return Some((next, next_sel));
    }
    None
}

#[must_use]
pub fn insert_link(rope: &Rope, selection: Selection, text: &str, url: &str) -> (Rope, Selection) {
    let range = selection.range();
    let label = if !text.is_empty() {
        text.to_owned()
    } else if !selection.is_empty() {
        rope.slice_to_string(range.clone())
    } else {
        "Link".to_owned()
    };
    let target = if url.is_empty() { "https://" } else { url };
    let md = format!("[{label}]({target})");
    let mut next = rope.clone();
    next.replace_range(range.clone(), &md);
    let caret = range.start.saturating_add(1).saturating_add(label.len());
    (next, Selection::collapsed(caret))
}

#[must_use]
pub fn insert_image(
    rope: &Rope,
    selection: Selection,
    alt: &str,
    url: &str,
    caption: Option<&str>,
    size: Option<ImageSize>,
) -> (Rope, Selection) {
    let range = selection.range();
    let label = if !alt.is_empty() {
        alt.to_owned()
    } else if !selection.is_empty() {
        rope.slice_to_string(range.clone())
    } else {
        "Bild".to_owned()
    };
    let md = format_markdown_image(&label, url, caption, size);
    let mut next = rope.clone();
    next.replace_range(range.clone(), &md);
    (
        next,
        Selection::collapsed(range.start.saturating_add(md.len())),
    )
}

#[must_use]
pub fn update_image(
    rope: &Rope,
    selection: Selection,
    range: Range<usize>,
    alt: &str,
    url: &str,
    caption: Option<&str>,
    size: Option<ImageSize>,
) -> Option<(Rope, Selection)> {
    if range.start > range.end || range.end > rope.len_bytes() {
        return None;
    }
    let start = rope.clamp_char_boundary(range.start);
    let end = rope.clamp_char_boundary(range.end);
    let md = format_markdown_image(alt, url, caption, size);
    let old_len = end.saturating_sub(start);
    let new_len = md.len();

    let mut next = rope.clone();
    next.replace_range(start..end, &md);

    let adjust = |off: usize| -> usize {
        if off >= end {
            off.saturating_sub(old_len).saturating_add(new_len)
        } else if off > start {
            start.saturating_add(new_len)
        } else {
            off
        }
    };
    let next_sel = Selection::new(adjust(selection.anchor), adjust(selection.head)).clamp_to(&next);
    Some((next, next_sel))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableRowLoc {
    Header,
    Delimiter,
    Body(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableCursor {
    pub row: TableRowLoc,
    pub col: usize,
}

impl TableCursor {
    /// Unified data row index (`0` = header, `1..=N` = body rows). Delimiter snaps to `0`.
    #[must_use]
    pub const fn data_row_index(self) -> usize {
        match self.row {
            TableRowLoc::Header | TableRowLoc::Delimiter => 0,
            TableRowLoc::Body(idx) => idx.saturating_add(1),
        }
    }
}

#[must_use]
pub fn locate_table_cursor(
    blocks: &[Block],
    caret: usize,
) -> Option<(&Block, &TableBlock, TableCursor)> {
    for block in blocks {
        if caret < block.byte_range.start || caret > block.byte_range.end {
            continue;
        }
        let BlockKind::Table(table) = &block.kind else {
            continue;
        };
        if caret >= table.header_row.byte_range.start && caret <= table.header_row.byte_range.end {
            let col = find_col_in_row(&table.header_row, caret);
            return Some((
                block,
                table,
                TableCursor {
                    row: TableRowLoc::Header,
                    col,
                },
            ));
        }
        if caret >= table.delimiter_range.start && caret <= table.delimiter_range.end {
            return Some((
                block,
                table,
                TableCursor {
                    row: TableRowLoc::Delimiter,
                    col: 0,
                },
            ));
        }
        for (b_idx, row) in table.body_rows.iter().enumerate() {
            if caret >= row.byte_range.start && caret <= row.byte_range.end {
                let col = find_col_in_row(row, caret);
                return Some((
                    block,
                    table,
                    TableCursor {
                        row: TableRowLoc::Body(b_idx),
                        col,
                    },
                ));
            }
        }
    }
    None
}

fn find_col_in_row(row: &TableRow, caret: usize) -> usize {
    for (idx, cell) in row.cells.iter().enumerate() {
        if caret <= cell.raw_range.end {
            return idx;
        }
    }
    row.cells.len().saturating_sub(1)
}

#[derive(Clone, Debug)]
struct EditableTable {
    cols: usize,
    alignments: Vec<ColumnAlign>,
    /// Row 0 is header, rows `1..` are body rows.
    rows: Vec<Vec<String>>,
}

impl EditableTable {
    fn from_ast(rope: &Rope, table: &TableBlock) -> Self {
        let cols = table.col_count.max(1);
        let mut alignments = table.alignments.clone();
        alignments.resize(cols, ColumnAlign::Left);

        let mut rows = Vec::with_capacity(table.body_rows.len().saturating_add(1));
        rows.push(extract_row_cells(rope, &table.header_row, cols));
        for br in &table.body_rows {
            rows.push(extract_row_cells(rope, br, cols));
        }
        Self {
            cols,
            alignments,
            rows,
        }
    }

    fn format_markdown(&self) -> String {
        let mut col_widths = vec![3usize; self.cols];
        for row in &self.rows {
            for (c, val) in row.iter().enumerate() {
                if let Some(w) = col_widths.get_mut(c) {
                    *w = (*w).max(val.chars().count().max(1));
                }
            }
        }

        let mut out = String::new();
        if let Some(header) = self.rows.first() {
            append_formatted_row(&mut out, header, &col_widths);
            out.push('\n');
        }
        // Delimiter row
        out.push('|');
        for c in 0..self.cols {
            let w = col_widths.get(c).copied().unwrap_or(3).max(3);
            let align = self.alignments.get(c).copied().unwrap_or(ColumnAlign::Left);
            out.push(' ');
            match align {
                ColumnAlign::Left => {
                    for _ in 0..w {
                        out.push('-');
                    }
                }
                ColumnAlign::Center => {
                    out.push(':');
                    for _ in 0..w.saturating_sub(2).max(1) {
                        out.push('-');
                    }
                    out.push(':');
                }
                ColumnAlign::Right => {
                    for _ in 0..w.saturating_sub(1).max(2) {
                        out.push('-');
                    }
                    out.push(':');
                }
            }
            out.push_str(" |");
        }

        for body_row in self.rows.iter().skip(1) {
            out.push('\n');
            append_formatted_row(&mut out, body_row, &col_widths);
        }
        out
    }
}

fn extract_row_cells(rope: &Rope, row: &TableRow, cols: usize) -> Vec<String> {
    let mut vals = Vec::with_capacity(cols);
    for c in 0..cols {
        if let Some(cell) = row.cells.get(c) {
            vals.push(rope.slice_to_string(cell.trimmed_range.clone()));
        } else {
            vals.push(String::new());
        }
    }
    vals
}

fn append_formatted_row(out: &mut String, cells: &[String], widths: &[usize]) {
    out.push('|');
    for (c, val) in cells.iter().enumerate() {
        let w = widths.get(c).copied().unwrap_or(3);
        let char_len = val.chars().count();
        out.push(' ');
        out.push_str(val);
        for _ in 0..w.saturating_sub(char_len) {
            out.push(' ');
        }
        out.push_str(" |");
    }
}

fn commit_editable_table(
    rope: &Rope,
    block_range: Range<usize>,
    editable: &EditableTable,
    target_data_row: usize,
    target_col: usize,
) -> (Rope, Selection) {
    let formatted = editable.format_markdown();
    let mut next = rope.clone();
    // Note: `block_range` from `parse_blocks` covers up to the end of the last table line (or +newline).
    // Trim trailing newline mismatch by replacing up to the exact non-newline end of the table block.
    let mut clean_end = block_range.end;
    if clean_end > block_range.start && rope.byte_at(clean_end.saturating_sub(1)) == Some(b'\n') {
        clean_end = clean_end.saturating_sub(1);
    }
    next.replace_range(block_range.start..clean_end, &formatted);

    let reparsed = parse_blocks(&next);
    for blk in &reparsed {
        if blk.byte_range.start <= block_range.start
            && blk.byte_range.end >= block_range.start.saturating_add(4)
            && let BlockKind::Table(tbl) = &blk.kind
        {
            let row_ref = if target_data_row == 0 {
                Some(&tbl.header_row)
            } else {
                tbl.body_rows.get(target_data_row.saturating_sub(1))
            };
            if let Some(r) = row_ref
                && let Some(cell) = r.cells.get(target_col.min(r.cells.len().saturating_sub(1)))
            {
                return (next, Selection::collapsed(cell.trimmed_range.end));
            }
        }
    }
    (next, Selection::collapsed(block_range.start))
}

#[must_use]
pub fn insert_new_table(
    rope: &Rope,
    selection: Selection,
    cols: usize,
    body_rows: usize,
) -> (Rope, Selection) {
    insert_new_table_localized(rope, selection, cols, body_rows, true)
}

#[must_use]
pub fn insert_new_table_localized(
    rope: &Rope,
    selection: Selection,
    cols: usize,
    body_rows: usize,
    german: bool,
) -> (Rope, Selection) {
    let c_count = cols.clamp(1, 16);
    let r_count = body_rows.clamp(1, 64);

    let prefix_lbl = if german { "Spalte" } else { "Column" };
    let mut header = Vec::with_capacity(c_count);
    for c in 0..c_count {
        header.push(format!("{prefix_lbl} {}", c.saturating_add(1)));
    }
    let mut rows = Vec::with_capacity(r_count.saturating_add(1));
    rows.push(header);
    for _ in 0..r_count {
        rows.push(vec![String::new(); c_count]);
    }

    let editable = EditableTable {
        cols: c_count,
        alignments: vec![ColumnAlign::Left; c_count],
        rows,
    };
    let md = editable.format_markdown();

    let lc = rope.byte_to_line_col(selection.head);
    let line_range = rope.line_byte_range(lc.line);
    let line_text = rope.slice_to_string(line_range.clone());
    let (replace_range, prefix, suffix) = if line_text.trim().is_empty() {
        (line_range, "", "\n")
    } else {
        (line_range.end..line_range.end, "\n\n", "\n")
    };
    let insert_pos = replace_range.start.saturating_add(prefix.len());
    let full = format!("{prefix}{md}{suffix}");

    let mut next = rope.clone();
    next.replace_range(replace_range, &full);

    let blocks = parse_blocks(&next);
    for blk in &blocks {
        if blk.byte_range.start <= insert_pos
            && blk.byte_range.end > insert_pos
            && let BlockKind::Table(tbl) = &blk.kind
            && let Some(cell0) = tbl.header_row.cells.first()
        {
            return (
                next,
                Selection::new(cell0.trimmed_range.start, cell0.trimmed_range.end),
            );
        }
    }
    (next, Selection::collapsed(insert_pos))
}

#[derive(Clone, Copy, Debug)]
pub enum TableOp {
    InsertRowAbove,
    InsertRowBelow,
    DeleteRow,
    InsertColLeft,
    InsertColRight,
    DeleteCol,
    SetAlign(ColumnAlign),
}

#[must_use]
pub fn apply_table_op(
    rope: &Rope,
    blocks: &[Block],
    selection: Selection,
    op: TableOp,
) -> Option<(Rope, Selection)> {
    let (block, table, cursor) = locate_table_cursor(blocks, selection.head)?;
    let mut editable = EditableTable::from_ast(rope, table);
    let cur_row = cursor.data_row_index();
    let cur_col = cursor.col.min(editable.cols.saturating_sub(1));

    match op {
        TableOp::InsertRowAbove => {
            let insert_idx = cur_row.max(1);
            editable
                .rows
                .insert(insert_idx, vec![String::new(); editable.cols]);
            Some(commit_editable_table(
                rope,
                block.byte_range.clone(),
                &editable,
                insert_idx,
                cur_col,
            ))
        }
        TableOp::InsertRowBelow => {
            let insert_idx = cur_row.saturating_add(1).min(editable.rows.len());
            editable
                .rows
                .insert(insert_idx, vec![String::new(); editable.cols]);
            Some(commit_editable_table(
                rope,
                block.byte_range.clone(),
                &editable,
                insert_idx,
                cur_col,
            ))
        }
        TableOp::DeleteRow => {
            if editable.rows.len() <= 1 {
                let mut next = rope.clone();
                next.replace_range(block.byte_range.clone(), "");
                return Some((next, Selection::collapsed(block.byte_range.start)));
            }
            let remove_idx = if cur_row == 0 {
                if editable.rows.len() > 1 { 1 } else { 0 }
            } else {
                cur_row.min(editable.rows.len().saturating_sub(1))
            };
            editable.rows.remove(remove_idx);
            let next_row = cur_row.min(editable.rows.len().saturating_sub(1));
            Some(commit_editable_table(
                rope,
                block.byte_range.clone(),
                &editable,
                next_row,
                cur_col,
            ))
        }
        TableOp::InsertColLeft => {
            editable.cols = editable.cols.saturating_add(1);
            editable.alignments.insert(cur_col, ColumnAlign::Left);
            for (r_idx, row) in editable.rows.iter_mut().enumerate() {
                let val = if r_idx == 0 {
                    "Neu".to_owned()
                } else {
                    String::new()
                };
                row.insert(cur_col.min(row.len()), val);
            }
            Some(commit_editable_table(
                rope,
                block.byte_range.clone(),
                &editable,
                cur_row,
                cur_col,
            ))
        }
        TableOp::InsertColRight => {
            let insert_col = cur_col.saturating_add(1).min(editable.cols);
            editable.cols = editable.cols.saturating_add(1);
            editable.alignments.insert(insert_col, ColumnAlign::Left);
            for (r_idx, row) in editable.rows.iter_mut().enumerate() {
                let val = if r_idx == 0 {
                    "Neu".to_owned()
                } else {
                    String::new()
                };
                row.insert(insert_col.min(row.len()), val);
            }
            Some(commit_editable_table(
                rope,
                block.byte_range.clone(),
                &editable,
                cur_row,
                insert_col,
            ))
        }
        TableOp::DeleteCol => {
            if editable.cols <= 1 {
                let mut next = rope.clone();
                next.replace_range(block.byte_range.clone(), "");
                return Some((next, Selection::collapsed(block.byte_range.start)));
            }
            editable.cols = editable.cols.saturating_sub(1);
            if cur_col < editable.alignments.len() {
                editable.alignments.remove(cur_col);
            }
            for row in &mut editable.rows {
                if cur_col < row.len() {
                    row.remove(cur_col);
                }
            }
            let next_col = cur_col.min(editable.cols.saturating_sub(1));
            Some(commit_editable_table(
                rope,
                block.byte_range.clone(),
                &editable,
                cur_row,
                next_col,
            ))
        }
        TableOp::SetAlign(align) => {
            if let Some(slot) = editable.alignments.get_mut(cur_col) {
                *slot = align;
            }
            Some(commit_editable_table(
                rope,
                block.byte_range.clone(),
                &editable,
                cur_row,
                cur_col,
            ))
        }
    }
}
