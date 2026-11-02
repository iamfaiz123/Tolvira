//! Format-neutral document model plus renderers (text / HTML / Markdown / JSON).

use serde_json::{json, Value};

/// A run of text with inline formatting.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Inline {
    /// Text content.
    pub text: String,
    /// Bold.
    pub bold: bool,
    /// Italic.
    pub italic: bool,
    /// Underlined.
    pub underline: bool,
    /// Strikethrough.
    pub strike: bool,
    /// Inline code / monospace.
    pub code: bool,
    /// Hyperlink target.
    pub link: Option<String>,
}

/// A block-level element of a [`Doc`].
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// Heading with level 1–6.
    Heading(u8, Vec<Inline>),
    /// Paragraph.
    Paragraph(Vec<Inline>),
    /// (level, ordered, inlines)
    ListItem(u8, bool, Vec<Inline>),
    /// Table as rows of cell text.
    Table(Vec<Vec<String>>),
    /// Pre-formatted code block.
    Code(String),
    /// Block quote.
    Quote(Vec<Inline>),
    /// Explicit page break.
    PageBreak,
    /// Horizontal rule.
    Rule,
}

/// Format-neutral document.
#[derive(Debug, Clone, Default)]
pub struct Doc {
    /// Blocks in document order.
    pub blocks: Vec<Block>,
    /// Metadata (title, author, dates, …).
    pub metadata: serde_json::Map<String, Value>,
    /// Number of embedded images encountered.
    pub images: usize,
    /// Non-fatal problems found while reading.
    pub warnings: Vec<String>,
}

/// Concatenates the plain text of inline runs.
pub fn plain(inlines: &[Inline]) -> String {
    inlines.iter().map(|i| i.text.as_str()).collect()
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn inline_html(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for i in inlines {
        let mut t = esc(&i.text).replace('\n', "<br>");
        if i.code {
            t = format!("<code>{t}</code>");
        }
        if i.bold {
            t = format!("<strong>{t}</strong>");
        }
        if i.italic {
            t = format!("<em>{t}</em>");
        }
        if i.underline {
            t = format!("<u>{t}</u>");
        }
        if i.strike {
            t = format!("<s>{t}</s>");
        }
        if let Some(l) = &i.link {
            t = format!("<a href=\"{}\">{t}</a>", esc(l));
        }
        out.push_str(&t);
    }
    out
}

fn inline_md(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for i in inlines {
        if i.text.is_empty() {
            continue;
        }
        let leading = i.text.len() - i.text.trim_start().len();
        let trailing = i.text.len() - i.text.trim_end().len();
        let core = i.text.trim();
        if core.is_empty() {
            out.push_str(&i.text);
            continue;
        }
        let mut t = if i.code {
            format!("`{core}`")
        } else {
            core.replace('*', "\\*").replace('_', "\\_")
        };
        if i.bold && i.italic {
            t = format!("***{t}***");
        } else if i.bold {
            t = format!("**{t}**");
        } else if i.italic {
            t = format!("*{t}*");
        }
        if i.strike {
            t = format!("~~{t}~~");
        }
        if let Some(l) = &i.link {
            t = format!("[{t}]({l})");
        }
        out.push_str(&" ".repeat(leading));
        out.push_str(&t);
        out.push_str(&" ".repeat(trailing));
    }
    out
}

impl Doc {
    /// Renders as plain text.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for b in &self.blocks {
            match b {
                Block::Heading(_, i) | Block::Paragraph(i) | Block::Quote(i) => {
                    out.push_str(&plain(i));
                    out.push_str("\n\n");
                },
                Block::ListItem(level, ordered, i) => {
                    out.push_str(&"  ".repeat(*level as usize));
                    out.push_str(if *ordered { "1. " } else { "• " });
                    out.push_str(&plain(i));
                    out.push('\n');
                },
                Block::Table(rows) => {
                    for r in rows {
                        out.push_str(&r.join("\t"));
                        out.push('\n');
                    }
                    out.push('\n');
                },
                Block::Code(c) => {
                    out.push_str(c);
                    out.push_str("\n\n");
                },
                Block::PageBreak => out.push_str("\x0c\n"),
                Block::Rule => out.push_str("----\n\n"),
            }
        }
        out.trim_end().to_string() + "\n"
    }

    /// Renders as HTML (fragment or full page with `title`).
    pub fn to_html(&self, full_page: bool, title: &str) -> String {
        let mut body = String::new();
        let mut list_stack: Vec<(u8, bool)> = Vec::new();
        let close_lists = |body: &mut String, stack: &mut Vec<(u8, bool)>, to_level: i32| {
            while let Some((lvl, ordered)) = stack.last().copied() {
                if lvl as i32 >= to_level {
                    body.push_str(if ordered { "</ol>\n" } else { "</ul>\n" });
                    stack.pop();
                } else {
                    break;
                }
            }
        };
        for b in &self.blocks {
            if !matches!(b, Block::ListItem(..)) {
                close_lists(&mut body, &mut list_stack, 0);
            }
            match b {
                Block::Heading(l, i) => {
                    body.push_str(&format!("<h{l}>{}</h{l}>\n", inline_html(i)))
                },
                Block::Paragraph(i) => body.push_str(&format!("<p>{}</p>\n", inline_html(i))),
                Block::Quote(i) => {
                    body.push_str(&format!("<blockquote><p>{}</p></blockquote>\n", inline_html(i)))
                },
                Block::ListItem(level, ordered, i) => {
                    // Close deeper lists, open missing ones.
                    close_lists(&mut body, &mut list_stack, *level as i32 + 1);
                    if list_stack.last().map(|(l, _)| *l) != Some(*level) {
                        body.push_str(if *ordered { "<ol>\n" } else { "<ul>\n" });
                        list_stack.push((*level, *ordered));
                    }
                    body.push_str(&format!("<li>{}</li>\n", inline_html(i)));
                },
                Block::Table(rows) => {
                    body.push_str("<table>\n");
                    for (ri, r) in rows.iter().enumerate() {
                        let tag = if ri == 0 { "th" } else { "td" };
                        body.push_str(&format!(
                            "<tr>{}</tr>\n",
                            r.iter()
                                .map(|c| format!("<{tag}>{}</{tag}>", esc(c)))
                                .collect::<String>()
                        ));
                    }
                    body.push_str("</table>\n");
                },
                Block::Code(c) => body.push_str(&format!("<pre><code>{}</code></pre>\n", esc(c))),
                Block::PageBreak => {
                    body.push_str("<div style=\"page-break-after:always\"></div>\n")
                },
                Block::Rule => body.push_str("<hr>\n"),
            }
        }
        close_lists(&mut body, &mut list_stack, 0);
        if full_page {
            format!("<!DOCTYPE html>\n<html><head><meta \
                     charset=\"utf-8\"><title>{}</title><style>body{{font-family:Georgia,serif;max-width:800px;margin:2rem auto;line-height:1.5}}table{{border-collapse:collapse}}td,th{{border:1px solid #999;padding:4px 8px}}pre{{background:#f4f4f4;padding:8px}}</style></head><body>\n{body}</body></html>\n", esc(title))
        } else {
            body
        }
    }

    /// Renders as Markdown.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        for b in &self.blocks {
            match b {
                Block::Heading(l, i) => out.push_str(&format!(
                    "{} {}\n\n",
                    "#".repeat(*l as usize),
                    inline_md(i).trim()
                )),
                Block::Paragraph(i) => out.push_str(&format!("{}\n\n", inline_md(i).trim())),
                Block::Quote(i) => out.push_str(&format!("> {}\n\n", inline_md(i).trim())),
                Block::ListItem(level, ordered, i) => out.push_str(&format!(
                    "{}{} {}\n",
                    "  ".repeat(*level as usize),
                    if *ordered { "1." } else { "-" },
                    inline_md(i).trim()
                )),
                Block::Table(rows) => {
                    if let Some(head) = rows.first() {
                        let escape = |c: &str| c.replace('|', "\\|").replace('\n', " ");
                        out.push_str(&format!(
                            "| {} |\n",
                            head.iter().map(|c| escape(c)).collect::<Vec<_>>().join(" | ")
                        ));
                        out.push_str(&format!(
                            "| {} |\n",
                            head.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")
                        ));
                        for r in rows.iter().skip(1) {
                            out.push_str(&format!(
                                "| {} |\n",
                                r.iter().map(|c| escape(c)).collect::<Vec<_>>().join(" | ")
                            ));
                        }
                        out.push('\n');
                    }
                },
                Block::Code(c) => out.push_str(&format!("```\n{c}\n```\n\n")),
                Block::PageBreak => out.push_str("\n---\n\n"),
                Block::Rule => out.push_str("---\n\n"),
            }
        }
        // Separate consecutive list blocks from following paragraphs.
        out.replace("\n\n\n", "\n\n").trim_end().to_string() + "\n"
    }

    /// Heading outline as JSON.
    pub fn outline(&self) -> Value {
        Value::Array(
            self.blocks
                .iter()
                .filter_map(|b| {
                    if let Block::Heading(l, i) = b {
                        Some(json!({ "level": l, "text": plain(i) }))
                    } else {
                        None
                    }
                })
                .collect(),
        )
    }

    /// Word/character/paragraph/heading/table counts.
    pub fn stats(&self) -> Value {
        let text = self.to_text();
        let words = text.split_whitespace().count();
        json!({
            "blocks": self.blocks.len(),
            "paragraphs": self.blocks.iter().filter(|b| matches!(b, Block::Paragraph(_))).count(),
            "headings": self.blocks.iter().filter(|b| matches!(b, Block::Heading(..))).count(),
            "list_items": self.blocks.iter().filter(|b| matches!(b, Block::ListItem(..))).count(),
            "tables": self.blocks.iter().filter(|b| matches!(b, Block::Table(_))).count(),
            "images": self.images,
            "words": words,
            "characters": text.chars().count(),
            "reading_time_minutes": ((words as f64 / 200.0) * 10.0).round() / 10.0,
        })
    }

    /// Structural JSON representation.
    pub fn to_json(&self) -> Value {
        Value::Array(
            self.blocks
                .iter()
                .map(|b| match b {
                    Block::Heading(l, i) => json!({
                        "type": "heading",
                        "level": l,
                        "text": plain(i),
                        "runs": runs_json(i)
                    }),
                    Block::Paragraph(i) => json!({
                        "type": "paragraph",
                        "text": plain(i),
                        "runs": runs_json(i)
                    }),
                    Block::Quote(i) => json!({ "type": "quote", "text": plain(i) }),
                    Block::ListItem(l, o, i) => json!({
                        "type": "list_item",
                        "level": l,
                        "ordered": o,
                        "text": plain(i)
                    }),
                    Block::Table(rows) => json!({ "type": "table", "rows": rows }),
                    Block::Code(c) => json!({ "type": "code", "text": c }),
                    Block::PageBreak => json!({ "type": "page_break" }),
                    Block::Rule => json!({ "type": "rule" }),
                })
                .collect(),
        )
    }
}

fn runs_json(i: &[Inline]) -> Value {
    Value::Array(
        i.iter()
            .map(|r| {
                json!({
                    "text": r.text,
                    "bold": r.bold,
                    "italic": r.italic,
                    "underline": r.underline,
                    "strike": r.strike,
                    "code": r.code,
                    "link": r.link
                })
            })
            .collect(),
    )
}
