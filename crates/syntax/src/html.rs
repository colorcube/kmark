use kmark_buffer::Rope;

use crate::blocks::{Block, BlockKind, ColumnAlign};
use crate::inline::{InlineTokenKind, StyleFlags, parse_inline_tokens};
use crate::outline::heading_slug;

/// Exports a parsed Markdown document to a standalone, print-ready HTML5 document.
#[must_use]
pub fn export_html_document(title: &str, rope: &Rope, blocks: &[Block]) -> String {
    let mut body = String::with_capacity(rope.len_bytes().saturating_mul(2).max(512));

    let mut i = 0usize;
    while i < blocks.len() {
        match blocks[i].kind {
            BlockKind::BlankLines => {
                i = i.saturating_add(1);
            }
            BlockKind::Heading {
                level,
                ref content_range,
                ..
            } => {
                let raw = rope.slice_to_string(content_range.clone());
                let inner = render_inline_html(&raw);
                let slug = heading_slug(&crate::outline::strip_inline_syntax(&raw));
                let lvl = level.clamp(1, 6);
                body.push_str(&format!(
                    "<h{lvl} id=\"{}\">{inner}</h{lvl}>\n",
                    escape_html(&slug)
                ));
                i = i.saturating_add(1);
            }
            BlockKind::Paragraph { ref lines } => {
                body.push_str("<p>");
                for (idx, (_, r)) in lines.iter().enumerate() {
                    if idx > 0 {
                        body.push('\n');
                    }
                    let raw = rope.slice_to_string(r.clone());
                    body.push_str(&render_inline_html(&raw));
                }
                body.push_str("</p>\n");
                i = i.saturating_add(1);
            }
            BlockKind::ThematicBreak => {
                body.push_str("<hr>\n");
                i = i.saturating_add(1);
            }
            BlockKind::CodeFence {
                ref language,
                ref body_lines,
                ..
            } => {
                let class_attr = if language.is_empty() {
                    String::new()
                } else {
                    format!(" class=\"language-{}\"", escape_html(language))
                };
                body.push_str(&format!("<pre><code{class_attr}>"));
                for (_, r) in body_lines {
                    body.push_str(&escape_html(&rope.slice_to_string(r.clone())));
                    body.push('\n');
                }
                body.push_str("</code></pre>\n");
                i = i.saturating_add(1);
            }
            BlockKind::BlockQuote {
                alert,
                ref custom_title,
                ref lines,
            } => {
                if let Some(kind) = alert {
                    let heading = custom_title.as_deref().unwrap_or_else(|| kind.title_de());
                    body.push_str(&format!(
                        "<blockquote class=\"alert\"><strong>{}</strong><br>",
                        escape_html(heading)
                    ));
                    for ql in lines.iter().skip(1) {
                        body.push_str(&render_inline_html(
                            &rope.slice_to_string(ql.content_range.clone()),
                        ));
                        body.push_str("<br>");
                    }
                    body.push_str("</blockquote>\n");
                } else {
                    body.push_str("<blockquote><p>");
                    for (idx, ql) in lines.iter().enumerate() {
                        if idx > 0 {
                            body.push_str("<br>");
                        }
                        body.push_str(&render_inline_html(
                            &rope.slice_to_string(ql.content_range.clone()),
                        ));
                    }
                    body.push_str("</p></blockquote>\n");
                }
                i = i.saturating_add(1);
            }
            BlockKind::ListItem { ordered, .. } => {
                let is_ordered = ordered.is_some();
                let tag = if is_ordered { "ol" } else { "ul" };
                body.push_str(&format!("<{tag}>\n"));
                while i < blocks.len() {
                    if let BlockKind::ListItem {
                        ordered: item_ord,
                        ref task,
                        ref content_range,
                        ..
                    } = blocks[i].kind
                        && item_ord.is_some() == is_ordered
                    {
                        body.push_str("  <li>");
                        if let Some(ts) = task {
                            let checked = if ts.checked { " checked" } else { "" };
                            body.push_str(&format!("<input type=\"checkbox\" disabled{checked}> "));
                        }
                        body.push_str(&render_inline_html(
                            &rope.slice_to_string(content_range.clone()),
                        ));
                        body.push_str("</li>\n");
                        i = i.saturating_add(1);
                    } else {
                        break;
                    }
                }
                body.push_str(&format!("</{tag}>\n"));
            }
            BlockKind::Table(ref tbl) => {
                body.push_str("<table>\n<thead>\n<tr>");
                for (col, cell) in tbl.header_row.cells.iter().enumerate() {
                    let align = align_style(tbl.alignments.get(col).copied().unwrap_or_default());
                    let content =
                        render_inline_html(&rope.slice_to_string(cell.trimmed_range.clone()));
                    body.push_str(&format!("<th{align}>{content}</th>"));
                }
                body.push_str("</tr>\n</thead>\n<tbody>\n");
                for row in &tbl.body_rows {
                    body.push_str("<tr>");
                    for (col, cell) in row.cells.iter().enumerate() {
                        let align =
                            align_style(tbl.alignments.get(col).copied().unwrap_or_default());
                        let content =
                            render_inline_html(&rope.slice_to_string(cell.trimmed_range.clone()));
                        body.push_str(&format!("<td{align}>{content}</td>"));
                    }
                    body.push_str("</tr>\n");
                }
                body.push_str("</tbody>\n</table>\n");
                i = i.saturating_add(1);
            }
        }
    }

    format!(
        "<!DOCTYPE html>\n<html lang=\"de\">\n<head>\n<meta charset=\"utf-8\">\n<title>{}</title>\n\
         <style>\n\
         body {{ font-family: 'Noto Sans', sans-serif; max-width: 52rem; margin: 2rem auto; padding: 0 1.5rem; line-height: 1.6; color: #232629; }}\n\
         pre, code {{ font-family: 'Hack', 'JetBrains Mono', monospace; background: #f4f5f7; border-radius: 4px; }}\n\
         pre {{ padding: 0.85rem 1rem; overflow-x: auto; }}\n\
         code {{ padding: 0.1rem 0.3rem; }}\n\
         blockquote {{ margin: 1rem 0; padding: 0.4rem 1rem; border-left: 4px solid #3daee9; background: #f8fafc; }}\n\
         table {{ border-collapse: collapse; width: 100%; margin: 1rem 0; }}\n\
         th, td {{ border: 1px solid #bdc3c7; padding: 0.45rem 0.7rem; }}\n\
         th {{ background: #eff0f1; }}\n\
         mark {{ background: #fff3a3; padding: 0 0.15rem; }}\n\
         kbd {{ border: 1px solid #888; border-radius: 4px; padding: 0.05rem 0.35rem; font-size: 0.9em; background: #f7f7f7; }}\n\
         .kmark-figure {{ display: inline-block; max-width: 100%; margin: 0.4rem 0; vertical-align: top; }}\n\
         .kmark-figure img {{ display: block; max-width: 100%; height: auto; border-radius: 4px; }}\n\
         .kmark-caption {{ display: block; text-align: center; font-size: 0.9em; font-style: italic; color: #5c616c; margin-top: 0.35rem; }}\n\
         </style>\n</head>\n<body>\n{body}</body>\n</html>\n",
        escape_html(title)
    )
}

fn align_style(align: ColumnAlign) -> &'static str {
    match align {
        ColumnAlign::Left => "",
        ColumnAlign::Center => " style=\"text-align:center\"",
        ColumnAlign::Right => " style=\"text-align:right\"",
    }
}

fn render_inline_html(raw: &str) -> String {
    let tokens = parse_inline_tokens(0, raw);
    let mut out = String::with_capacity(raw.len());
    for tok in tokens {
        let slice = raw.get(tok.range.clone()).unwrap_or("");
        match tok.kind {
            InlineTokenKind::SyntaxMarker => {}
            InlineTokenKind::Image {
                alt,
                src,
                caption,
                size,
            } => {
                let size_style = match size {
                    Some(crate::inline::ImageSize::Percent(pct)) => {
                        format!(" style=\"width:{pct}%;max-width:100%;height:auto;\"")
                    }
                    Some(crate::inline::ImageSize::Pixels(px)) => {
                        format!(" style=\"width:{px}px;max-width:100%;height:auto;\"")
                    }
                    None => " style=\"max-width:100%;height:auto;\"".to_owned(),
                };
                if let Some(cap) = caption.as_deref().filter(|s| !s.trim().is_empty()) {
                    let fig_style = match size {
                        Some(crate::inline::ImageSize::Percent(pct)) => {
                            format!(" style=\"width:{pct}%;max-width:100%;\"")
                        }
                        Some(crate::inline::ImageSize::Pixels(px)) => {
                            format!(" style=\"width:{px}px;max-width:100%;\"")
                        }
                        None => String::new(),
                    };
                    let img_style = if size.is_some() {
                        " style=\"width:100%;height:auto;\""
                    } else {
                        " style=\"max-width:100%;height:auto;\""
                    };
                    out.push_str(&format!(
                        "<span class=\"kmark-figure\"{fig_style}><img src=\"{}\" alt=\"{}\" title=\"{}\"{img_style}><span class=\"kmark-caption\">{}</span></span>",
                        escape_html(&src),
                        escape_html(&alt),
                        escape_html(cap),
                        escape_html(cap)
                    ));
                } else {
                    out.push_str(&format!(
                        "<img src=\"{}\" alt=\"{}\"{size_style}>",
                        escape_html(&src),
                        escape_html(&alt)
                    ));
                }
            }
            InlineTokenKind::LinkText { url } => {
                let styled = wrap_styles(&escape_html(slice), tok.style);
                out.push_str(&format!("<a href=\"{}\">{styled}</a>", escape_html(&url)));
            }
            InlineTokenKind::Text => {
                out.push_str(&wrap_styles(&escape_html(slice), tok.style));
            }
        }
    }
    out
}

fn wrap_styles(escaped: &str, style: StyleFlags) -> String {
    let mut res = escaped.to_owned();
    if style.contains(StyleFlags::CODE) {
        res = format!("<code>{res}</code>");
    }
    if style.contains(StyleFlags::KBD) {
        res = format!("<kbd>{res}</kbd>");
    }
    if style.contains(StyleFlags::HIGHLIGHT) {
        res = format!("<mark>{res}</mark>");
    }
    if style.contains(StyleFlags::STRIKE) {
        res = format!("<del>{res}</del>");
    }
    if style.contains(StyleFlags::ITALIC) {
        res = format!("<em>{res}</em>");
    }
    if style.contains(StyleFlags::BOLD) {
        res = format!("<strong>{res}</strong>");
    }
    res
}

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}
