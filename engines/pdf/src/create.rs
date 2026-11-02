//! PDF generation: text, Markdown, HTML and images → PDF.
//!
//! A small block layout engine places paragraphs, headings, lists, code
//! blocks and rules on pages using the standard-14 fonts. Fidelity is
//! intentionally modest (no tables, floats or CSS); the goal is a clean,
//! predictable document that is generated fully client-side.

use crate::doc::{self, escape_pdf_string};
use crate::fonts::{encode_winansi, wrap_line, StdFont};
use crate::pages::PdfOutput;
use engine_shared::{limits, EngineError, EngineResult};
use lopdf::{Dictionary, Object, Stream};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde::Deserialize;
use serde_json::{json, Value};

/// Layout options shared by every text based generator.
#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct LayoutOptions {
    /// `A4`, `Letter`, `A4-landscape`, `210mm x 297mm`, ...
    pub page_size: String,
    /// `helvetica` (default), `times`, `courier`.
    pub font: String,
    /// Body font size in points.
    pub font_size: f64,
    /// Line height as a multiple of the font size.
    pub line_height: f64,
    /// Margins in points.
    pub margin_top: f64,
    /// Right margin in points.
    pub margin_right: f64,
    /// Bottom margin in points.
    pub margin_bottom: f64,
    /// Left margin in points.
    pub margin_left: f64,
    /// `left` (default), `center`, `right`, `justify` (treated as left).
    pub align: String,
    /// Document title written to the Info dictionary.
    pub title: Option<String>,
    /// Document author written to the Info dictionary.
    pub author: Option<String>,
    /// Add `Page n of N` footer.
    pub page_numbers: bool,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        LayoutOptions {
            page_size: "A4".into(),
            font: "helvetica".into(),
            font_size: 11.0,
            line_height: 1.4,
            margin_top: 56.0,
            margin_right: 56.0,
            margin_bottom: 56.0,
            margin_left: 56.0,
            align: "left".into(),
            title: None,
            author: None,
            page_numbers: false,
        }
    }
}

impl LayoutOptions {
    fn validate(&self) -> EngineResult<()> {
        engine_shared::error::require_range(self.font_size, 4.0, 96.0, "font_size")?;
        engine_shared::error::require_range(self.line_height, 0.8, 4.0, "line_height")?;
        for (m, name) in [
            (self.margin_top, "margin_top"),
            (self.margin_right, "margin_right"),
            (self.margin_bottom, "margin_bottom"),
            (self.margin_left, "margin_left"),
        ] {
            engine_shared::error::require_range(m, 0.0, 400.0, name)?;
        }
        Ok(())
    }
}

/// A laid-out block of content.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// Heading of the given level (1–6).
    Heading {
        /// Heading level.
        level: u8,
        /// Heading text.
        text: String,
    },
    /// Body paragraph.
    Paragraph(String),
    /// Bullet or numbered list item.
    ListItem {
        /// Item number for ordered lists, `None` for bullets.
        ordered: Option<u32>,
        /// Nesting depth (0 = top level).
        depth: u8,
        /// Item text.
        text: String,
    },
    /// Pre-formatted code block (monospace).
    Code(String),
    /// Block quote.
    Quote(String),
    /// Horizontal rule.
    Rule,
    /// Vertical spacer.
    Spacer,
}

struct Line {
    text: String,
    font: StdFont,
    size: f64,
    indent: f64,
    /// Extra vertical space *before* the line.
    space_before: f64,
    align: Align,
}

#[derive(Clone, Copy, PartialEq)]
enum Align {
    Left,
    Center,
    Right,
}

/// Converts blocks into pages and serialises the PDF.
pub fn render_blocks(blocks: &[Block], opts: &LayoutOptions) -> EngineResult<Value> {
    opts.validate()?;
    let (pw, ph) = doc::page_size_from_name(&opts.page_size)?;
    let content_w = pw - opts.margin_left - opts.margin_right;
    let content_h = ph - opts.margin_top - opts.margin_bottom;
    if content_w < 20.0 || content_h < 20.0 {
        return Err(EngineError::invalid("Margins leave no room for content on the page"));
    }
    let base_align = match opts.align.as_str() {
        "center" => Align::Center,
        "right" => Align::Right,
        _ => Align::Left,
    };
    let body = StdFont::resolve(&opts.font, false, false);
    let bold = StdFont::resolve(&opts.font, true, false);
    let italic = StdFont::resolve(&opts.font, false, true);
    let mono = StdFont::resolve("courier", false, false);
    let fs = opts.font_size;
    let lh = opts.line_height;

    // 1. Flatten blocks into lines.
    let mut lines: Vec<Line> = Vec::new();
    let mut substituted = 0usize;
    #[allow(clippy::too_many_arguments)]
    fn push_wrapped(
        lines: &mut Vec<Line>,
        substituted: &mut usize,
        content_w: f64,
        text: &str,
        font: StdFont,
        size: f64,
        indent: f64,
        space_before: f64,
        align: Align,
    ) {
        let (_, subs) = encode_winansi(text);
        *substituted += subs;
        let wrapped = wrap_line(font, size, text, content_w - indent);
        for (i, l) in wrapped.into_iter().enumerate() {
            lines.push(Line {
                text: l,
                font,
                size,
                indent,
                space_before: if i == 0 { space_before } else { 0.0 },
                align,
            });
        }
    }

    for block in blocks {
        match block {
            Block::Heading { level, text } => {
                let size = match level {
                    1 => fs * 2.0,
                    2 => fs * 1.6,
                    3 => fs * 1.3,
                    4 => fs * 1.15,
                    _ => fs * 1.0,
                };
                push_wrapped(
                    &mut lines,
                    &mut substituted,
                    content_w,
                    text,
                    bold,
                    size,
                    0.0,
                    size * 0.8,
                    base_align,
                );
            },
            Block::Paragraph(text) => {
                for (i, para) in text.split('\n').enumerate() {
                    push_wrapped(
                        &mut lines,
                        &mut substituted,
                        content_w,
                        para,
                        body,
                        fs,
                        0.0,
                        if i == 0 { fs * 0.6 } else { 0.0 },
                        base_align,
                    );
                }
            },
            Block::ListItem { ordered, depth, text } => {
                let indent = 18.0 + (*depth as f64) * 18.0;
                let marker = match ordered {
                    Some(n) => format!("{n}."),
                    None => "\u{2022}".to_string(),
                };
                let (_, subs) = encode_winansi(text);
                substituted += subs;
                let wrapped = wrap_line(body, fs, text, content_w - indent);
                for (i, l) in wrapped.into_iter().enumerate() {
                    lines.push(Line {
                        text: if i == 0 { format!("{marker}\u{1}{l}") } else { l },
                        font: body,
                        size: fs,
                        indent,
                        space_before: if i == 0 { fs * 0.25 } else { 0.0 },
                        align: Align::Left,
                    });
                }
            },
            Block::Code(code) => {
                for (i, l) in code.lines().enumerate() {
                    let expanded = l.replace('\t', "    ");
                    push_wrapped(
                        &mut lines,
                        &mut substituted,
                        content_w,
                        &expanded,
                        mono,
                        fs * 0.9,
                        12.0,
                        if i == 0 { fs * 0.6 } else { 0.0 },
                        Align::Left,
                    );
                }
                if code.is_empty() {
                    lines.push(Line {
                        text: String::new(),
                        font: mono,
                        size: fs * 0.9,
                        indent: 12.0,
                        space_before: 0.0,
                        align: Align::Left,
                    });
                }
            },
            Block::Quote(text) => {
                push_wrapped(
                    &mut lines,
                    &mut substituted,
                    content_w,
                    text,
                    italic,
                    fs,
                    24.0,
                    fs * 0.6,
                    Align::Left,
                );
            },
            Block::Rule => lines.push(Line {
                text: "\u{2}".into(),
                font: body,
                size: fs,
                indent: 0.0,
                space_before: fs * 0.5,
                align: Align::Left,
            }),
            Block::Spacer => lines.push(Line {
                text: String::new(),
                font: body,
                size: fs,
                indent: 0.0,
                space_before: 0.0,
                align: Align::Left,
            }),
        }
    }
    if lines.is_empty() {
        lines.push(Line {
            text: String::new(),
            font: body,
            size: fs,
            indent: 0.0,
            space_before: 0.0,
            align: Align::Left,
        });
    }

    // 2. Paginate.
    let mut pages: Vec<Vec<(f64, &Line)>> = Vec::new();
    let mut current: Vec<(f64, &Line)> = Vec::new();
    let mut y = ph - opts.margin_top;
    let top = ph - opts.margin_top;
    let bottom = opts.margin_bottom;
    for line in &lines {
        let advance = line.size * lh;
        let mut needed = advance + if current.is_empty() { 0.0 } else { line.space_before };
        if y - needed < bottom && !current.is_empty() {
            pages.push(std::mem::take(&mut current));
            y = top;
            needed = advance;
        }
        y -= needed;
        current.push((y, line));
        if pages.len() + 1 > limits::MAX_PDF_PAGES {
            return Err(EngineError::limit("Generated document exceeds the page limit"));
        }
    }

    pages.push(current);

    // 3. Emit.
    let (mut d, pages_id) = doc::new_document();
    let font_ids = [
        ("F1", body.add_to_document(&mut d)),
        ("F2", bold.add_to_document(&mut d)),
        ("F3", italic.add_to_document(&mut d)),
        ("F4", mono.add_to_document(&mut d)),
    ];
    let font_key = |f: StdFont| -> &'static str {
        if f == bold {
            "F2"
        } else if f == italic {
            "F3"
        } else if f == mono {
            "F4"
        } else {
            "F1"
        }
    };
    let mut fonts_dict = Dictionary::new();
    for (k, id) in &font_ids {
        fonts_dict.set(*k, Object::Reference(*id));
    }
    let mut resources = Dictionary::new();
    resources.set("Font", Object::Dictionary(fonts_dict));
    let resources_id = d.add_object(Object::Dictionary(resources));

    let total_pages = pages.len();
    for (pi, page_lines) in pages.iter().enumerate() {
        let mut content = String::new();
        for (y, line) in page_lines {
            if line.text == "\u{2}" {
                content.push_str(&format!(
                    "0.6 G 0.5 w {:.2} {:.2} m {:.2} {:.2} l S\n",
                    opts.margin_left,
                    y + line.size * 0.4,
                    pw - opts.margin_right,
                    y + line.size * 0.4
                ));
                continue;
            }
            let (marker, text) = match line.text.split_once('\u{1}') {
                Some((m, t)) => (Some(m.to_string()), t.to_string()),
                None => (None, line.text.clone()),
            };
            if let Some(m) = marker {
                let (mb, _) = encode_winansi(&m);
                content.push_str(&format!(
                    "BT /{} {:.2} Tf {:.2} {:.2} Td ({}) Tj ET\n",
                    font_key(line.font),
                    line.size,
                    opts.margin_left + line.indent - 14.0,
                    y,
                    String::from_utf8_lossy(&escape_pdf_string(&mb))
                ));
            }
            if text.is_empty() {
                continue;
            }
            let (bytes, _) = encode_winansi(&text);
            let width = line.font.text_width(&bytes, line.size);
            let x = match line.align {
                Align::Left => opts.margin_left + line.indent,
                Align::Center => {
                    opts.margin_left + line.indent + (content_w - line.indent - width) / 2.0
                },
                Align::Right => pw - opts.margin_right - width,
            };
            let escaped = escape_pdf_string(&bytes);
            content.push_str(&format!(
                "BT /{} {:.2} Tf {:.2} {:.2} Td (",
                font_key(line.font),
                line.size,
                x,
                y
            ));
            content.push_str(&String::from_utf8_lossy(&escaped));
            content.push_str(") Tj ET\n");
        }
        if opts.page_numbers {
            let label = format!("Page {} of {}", pi + 1, total_pages);
            let (bytes, _) = encode_winansi(&label);
            let w = body.text_width(&bytes, fs * 0.8);
            content.push_str(&format!(
                "BT /F1 {:.2} Tf {:.2} {:.2} Td ({}) Tj ET\n",
                fs * 0.8,
                (pw - w) / 2.0,
                opts.margin_bottom / 2.0,
                String::from_utf8_lossy(&escape_pdf_string(&bytes))
            ));
        }
        let content_id = d.add_object(Stream::new(Dictionary::new(), content.into_bytes()));
        let mut page = Dictionary::new();
        page.set("MediaBox", doc::Rect { x0: 0.0, y0: 0.0, x1: pw, y1: ph }.to_object());
        page.set("Contents", Object::Reference(content_id));
        page.set("Resources", Object::Reference(resources_id));
        doc::append_page(&mut d, pages_id, page);
    }

    // Info dictionary.
    let mut info = Dictionary::new();
    if let Some(t) = &opts.title {
        info.set("Title", doc::encode_text_string(t));
    }
    if let Some(a) = &opts.author {
        info.set("Author", doc::encode_text_string(a));
    }
    info.set("Producer", Object::string_literal("Dev Toolbox PDF Engine"));
    let info_id = d.add_object(Object::Dictionary(info));
    d.trailer.set("Info", Object::Reference(info_id));

    let bytes = doc::save(&mut d)?;
    Ok(json!({
        "output": PdfOutput::new(bytes, total_pages),
        "substituted_characters": substituted,
        "warnings": if substituted > 0 { vec![format!("{substituted} character(s) outside the \
                                                       WinAnsi repertoire were replaced with '?'")] } else { vec![] },
    }))
}

/// Plain text → blocks: blank lines separate paragraphs; single newlines are
/// preserved inside a paragraph.
pub fn text_to_blocks(text: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut para: Vec<&str> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            if !para.is_empty() {
                blocks.push(Block::Paragraph(para.join("\n")));
                para.clear();
            }
            blocks.push(Block::Spacer);
        } else {
            para.push(line.trim_end());
        }
    }
    if !para.is_empty() {
        blocks.push(Block::Paragraph(para.join("\n")));
    }
    // Collapse leading / repeated spacers.
    let mut out: Vec<Block> = Vec::new();
    for b in blocks {
        if b == Block::Spacer && (out.is_empty() || out.last() == Some(&Block::Spacer)) {
            continue;
        }
        out.push(b);
    }
    out
}

/// Markdown → blocks via pulldown-cmark.
pub fn markdown_to_blocks(md: &str) -> Vec<Block> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TABLES);
    let parser = Parser::new_ext(md, opts);

    let mut blocks = Vec::new();
    let mut text = String::new();
    let mut heading: Option<u8> = None;
    let mut list_stack: Vec<Option<u32>> = Vec::new();
    let mut in_code = false;
    let mut in_quote = 0usize;
    let mut in_item = false;
    let mut table_row: Vec<String> = Vec::new();
    let mut in_table = false;

    let flush_para = |text: &mut String, blocks: &mut Vec<Block>, in_quote: usize| {
        let t = text.trim().to_string();
        if !t.is_empty() {
            if in_quote > 0 {
                blocks.push(Block::Quote(t));
            } else {
                blocks.push(Block::Paragraph(t));
            }
        }
        text.clear();
    };

    for ev in parser {
        match ev {
            Event::Start(Tag::Heading { level, .. }) => {
                flush_para(&mut text, &mut blocks, in_quote);
                heading = Some(match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                });
            },
            Event::End(TagEnd::Heading(_)) => {
                blocks.push(Block::Heading {
                    level: heading.take().unwrap_or(1),
                    text: text.trim().to_string(),
                });
                text.clear();
            },
            Event::Start(Tag::Paragraph) => {
                if !in_item {
                    flush_para(&mut text, &mut blocks, in_quote);
                }
            },
            Event::End(TagEnd::Paragraph) => {
                if !in_item {
                    flush_para(&mut text, &mut blocks, in_quote);
                }
            },
            Event::Start(Tag::List(start)) => {
                flush_para(&mut text, &mut blocks, in_quote);
                list_stack.push(start.map(|s| s as u32));
            },
            Event::End(TagEnd::List(_)) => {
                list_stack.pop();
            },
            Event::Start(Tag::Item) => {
                in_item = true;
                text.clear();
            },
            Event::End(TagEnd::Item) => {
                in_item = false;
                let depth = list_stack.len().saturating_sub(1) as u8;
                let ordered = list_stack.last_mut().and_then(|s| {
                    s.inspect(|&n| {
                        *s = Some(n + 1);
                    })
                });
                blocks.push(Block::ListItem {
                    ordered,
                    depth,
                    text: text.trim().to_string(),
                });
                text.clear();
            },
            Event::Start(Tag::CodeBlock(_)) => {
                flush_para(&mut text, &mut blocks, in_quote);
                in_code = true;
            },
            Event::End(TagEnd::CodeBlock) => {
                in_code = false;
                blocks.push(Block::Code(text.trim_end_matches('\n').to_string()));
                text.clear();
            },
            Event::Start(Tag::BlockQuote(_)) => {
                flush_para(&mut text, &mut blocks, in_quote);
                in_quote += 1;
            },
            Event::End(TagEnd::BlockQuote(_)) => {
                flush_para(&mut text, &mut blocks, in_quote);
                in_quote = in_quote.saturating_sub(1);
            },
            Event::Start(Tag::Table(_)) => {
                flush_para(&mut text, &mut blocks, in_quote);
                in_table = true;
            },
            Event::End(TagEnd::Table) => in_table = false,
            Event::Start(Tag::TableHead) | Event::Start(Tag::TableRow) => table_row.clear(),
            Event::End(TagEnd::TableHead) | Event::End(TagEnd::TableRow) => {
                blocks.push(Block::Code(table_row.join(" | ")));
                table_row.clear();
            },
            Event::End(TagEnd::TableCell) => {
                table_row.push(text.trim().to_string());
                text.clear();
            },
            Event::Rule => {
                flush_para(&mut text, &mut blocks, in_quote);
                blocks.push(Block::Rule);
            },
            Event::Text(t) | Event::Code(t) => text.push_str(&t),
            Event::SoftBreak => text.push(' '),
            Event::HardBreak => text.push('\n'),
            Event::Html(h) | Event::InlineHtml(h) => {
                if !in_table {
                    text.push_str(&strip_tags(&h));
                }
            },
            Event::Start(Tag::Link { .. }) | Event::End(TagEnd::Link) => {},
            Event::Start(Tag::Image { dest_url, .. }) => {
                text.push_str(&format!("[image: {dest_url}]"))
            },
            _ => {},
        }
        let _ = in_code;
    }
    flush_para(&mut text, &mut blocks, in_quote);
    blocks
}

/// Very small HTML → blocks converter (no CSS). Handles headings,
/// paragraphs, lists, `<pre>`, `<blockquote>`, `<br>`, `<hr>` and drops
/// scripts/styles. Everything else is treated as inline text.
pub fn html_to_blocks(html: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut text = String::new();
    let mut heading: Option<u8> = None;
    let mut list_stack: Vec<Option<u32>> = Vec::new();
    let mut pre = 0usize;
    let mut quote = 0usize;
    let mut skip = 0usize;

    let flush = |text: &mut String,
                 blocks: &mut Vec<Block>,
                 heading: &mut Option<u8>,
                 quote: usize,
                 pre: usize| {
        let t = if pre > 0 {
            text.trim_matches('\n').to_string()
        } else {
            collapse_ws(text)
        };
        if !t.is_empty() {
            if let Some(l) = heading.take() {
                blocks.push(Block::Heading { level: l, text: t });
            } else if pre > 0 {
                blocks.push(Block::Code(t));
            } else if quote > 0 {
                blocks.push(Block::Quote(t));
            } else {
                blocks.push(Block::Paragraph(t));
            }
        }
        text.clear();
    };

    let mut rest = html;
    while let Some(start) = rest.find('<') {
        let before = &rest[..start];
        if skip == 0 {
            text.push_str(&decode_entities(before));
        }
        let after = &rest[start + 1..];
        let end = match after.find('>') {
            Some(e) => e,
            None => {
                rest = "";
                break;
            },
        };
        let raw_tag = &after[..end];
        rest = &after[end + 1..];
        if raw_tag.starts_with('!') || raw_tag.starts_with('?') {
            continue;
        }
        let closing = raw_tag.starts_with('/');
        let name: String = raw_tag
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        match (name.as_str(), closing) {
            ("script", false)
            | ("style", false)
            | ("head", false)
            | ("title", false)
            | ("noscript", false) => skip += 1,
            ("script", true)
            | ("style", true)
            | ("head", true)
            | ("title", true)
            | ("noscript", true) => skip = skip.saturating_sub(1),
            (h, false)
                if h.len() == 2
                    && h.starts_with('h')
                    && h[1..].chars().all(|c| ('1'..='6').contains(&c)) =>
            {
                flush(&mut text, &mut blocks, &mut heading, quote, pre);
                heading = Some(h[1..].parse().unwrap_or(1));
            },
            (h, true)
                if h.len() == 2
                    && h.starts_with('h')
                    && h[1..].chars().all(|c| ('1'..='6').contains(&c)) =>
            {
                flush(&mut text, &mut blocks, &mut heading, quote, pre);
            },
            (
                "p" | "div" | "section" | "article" | "header" | "footer" | "main" | "table" | "tr"
                | "td" | "th" | "dd" | "dt",
                _,
            ) => {
                flush(&mut text, &mut blocks, &mut heading, quote, pre);
            },
            ("br", _) => text.push('\n'),
            ("hr", _) => {
                flush(&mut text, &mut blocks, &mut heading, quote, pre);
                blocks.push(Block::Rule);
            },
            ("ul", false) => {
                flush(&mut text, &mut blocks, &mut heading, quote, pre);
                list_stack.push(None);
            },
            ("ol", false) => {
                flush(&mut text, &mut blocks, &mut heading, quote, pre);
                list_stack.push(Some(1));
            },
            ("ul" | "ol", true) => {
                flush(&mut text, &mut blocks, &mut heading, quote, pre);
                list_stack.pop();
            },
            ("li", false) => {
                flush(&mut text, &mut blocks, &mut heading, quote, pre);
            },
            ("li", true) => {
                let t = collapse_ws(&text);
                text.clear();
                let depth = list_stack.len().saturating_sub(1) as u8;
                let ordered = list_stack.last_mut().and_then(|s| {
                    s.inspect(|&n| {
                        *s = Some(n + 1);
                    })
                });
                if !t.is_empty() {
                    blocks.push(Block::ListItem { ordered, depth, text: t });
                }
            },
            ("pre", false) => {
                flush(&mut text, &mut blocks, &mut heading, quote, pre);
                pre += 1;
            },
            ("pre", true) => {
                flush(&mut text, &mut blocks, &mut heading, quote, pre);
                pre = pre.saturating_sub(1);
            },
            ("blockquote", false) => {
                flush(&mut text, &mut blocks, &mut heading, quote, pre);
                quote += 1;
            },
            ("blockquote", true) => {
                flush(&mut text, &mut blocks, &mut heading, quote, pre);
                quote = quote.saturating_sub(1);
            },
            ("img", false) => {
                if let Some(alt) = attr_value(raw_tag, "alt") {
                    text.push_str(&format!("[image: {alt}]"));
                }
            },
            _ => {},
        }
    }
    if skip == 0 {
        text.push_str(&decode_entities(rest));
    }
    flush(&mut text, &mut blocks, &mut heading, quote, pre);
    blocks
}

fn attr_value(tag: &str, attr: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let idx = lower.find(&format!("{attr}=")).map(|i| i + attr.len() + 1)?;
    let rest = &tag[idx..];
    let quote = rest.chars().next()?;
    if quote == '"' || quote == '\'' {
        let end = rest[1..].find(quote)?;
        Some(decode_entities(&rest[1..1 + end]))
    } else {
        Some(rest.split_whitespace().next().unwrap_or("").to_string())
    }
}

/// Collapses runs of whitespace into single spaces.
pub fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = true;
    for ch in s.chars() {
        if ch == '\n' {
            out.push('\n');
            prev_space = true;
        } else if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

/// Removes HTML tags, keeping the text between them.
pub fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {},
        }
    }
    decode_entities(&out)
}

/// Decodes common named, decimal and hex HTML entities.
pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        if let Some(semi) = after.find(';').filter(|&n| n <= 10) {
            let ent = &after[..semi];
            let decoded = match ent {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" | "#39" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                "copy" => Some('©'),
                "reg" => Some('®'),
                "trade" => Some('™'),
                "mdash" => Some('—'),
                "ndash" => Some('–'),
                "hellip" => Some('…'),
                "laquo" => Some('«'),
                "raquo" => Some('»'),
                "ldquo" => Some('“'),
                "rdquo" => Some('”'),
                "lsquo" => Some('‘'),
                "rsquo" => Some('’'),
                "bull" => Some('•'),
                "euro" => Some('€'),
                _ => {
                    if let Some(hex) = ent.strip_prefix("#x").or_else(|| ent.strip_prefix("#X")) {
                        u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
                    } else if let Some(dec) = ent.strip_prefix('#') {
                        dec.parse::<u32>().ok().and_then(char::from_u32)
                    } else {
                        None
                    }
                },
            };
            match decoded {
                Some(c) => {
                    out.push(c);
                    rest = &after[semi + 1..];
                },
                None => {
                    out.push('&');
                    rest = after;
                },
            }
        } else {
            out.push('&');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// Builds a PDF from raster images (one image per page).
///
/// JPEG data is embedded as-is (`DCTDecode`); every other format is decoded
/// and stored as Flate compressed RGB (or Gray) samples.
pub fn from_images(
    images: &[Vec<u8>],
    page_size: Option<&str>,
    margin: f64,
    fit: &str,
) -> EngineResult<Value> {
    limits::check_count(images.len(), limits::MAX_BATCH_FILES, "images")?;
    engine_shared::error::require_range(margin, 0.0, 300.0, "margin")?;
    let (mut d, pages_id) = doc::new_document();
    let mut info_pages = Vec::new();

    for (i, bytes) in images.iter().enumerate() {
        if bytes.is_empty() {
            return Err(EngineError::empty(&format!("image {}", i + 1)));
        }
        limits::check_size(bytes.len(), limits::MAX_FILE_BYTES, "image")?;
        let (xobj, w, h) = crate::images::image_xobject(bytes)
            .map_err(|e| EngineError::new(e.code, format!("Image {}: {}", i + 1, e.message)))?;
        let (pw, ph) = match page_size {
            None | Some("fit") | Some("auto") | Some("") => {
                (w as f64 + 2.0 * margin, h as f64 + 2.0 * margin)
            },
            Some(name) => doc::page_size_from_name(name)?,
        };
        let avail_w = (pw - 2.0 * margin).max(1.0);
        let avail_h = (ph - 2.0 * margin).max(1.0);
        let (dw, dh) = match fit {
            "stretch" => (avail_w, avail_h),
            "cover" => {
                let s = (avail_w / w as f64).max(avail_h / h as f64);
                (w as f64 * s, h as f64 * s)
            },
            _ => {
                let s = (avail_w / w as f64).min(avail_h / h as f64);
                (w as f64 * s, h as f64 * s)
            },
        };
        let x = (pw - dw) / 2.0;
        let y = (ph - dh) / 2.0;
        let xobj_id = d.add_object(xobj);
        let content = format!("q {dw:.3} 0 0 {dh:.3} {x:.3} {y:.3} cm /Im1 Do Q\n");
        let content_id = d.add_object(Stream::new(Dictionary::new(), content.into_bytes()));
        let mut xobjects = Dictionary::new();
        xobjects.set("Im1", Object::Reference(xobj_id));
        let mut resources = Dictionary::new();
        resources.set("XObject", Object::Dictionary(xobjects));
        let mut page = Dictionary::new();
        page.set("MediaBox", doc::Rect { x0: 0.0, y0: 0.0, x1: pw, y1: ph }.to_object());
        page.set("Contents", Object::Reference(content_id));
        page.set("Resources", Object::Dictionary(resources));
        doc::append_page(&mut d, pages_id, page);
        info_pages.push(json!({
            "image": i + 1,
            "width_px": w,
            "height_px": h,
            "page_width_pt": pw,
            "page_height_pt": ph
        }));
    }
    let bytes = doc::save(&mut d)?;
    Ok(json!({ "output": PdfOutput::new(bytes, images.len()), "pages": info_pages }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_blocks() {
        let b = text_to_blocks("Hello\nworld\n\n\nSecond para");
        assert_eq!(
            b,
            vec![
                Block::Paragraph("Hello\nworld".into()),
                Block::Spacer,
                Block::Paragraph("Second para".into())
            ]
        );
        assert!(text_to_blocks("").is_empty());
    }

    #[test]
    fn markdown_blocks() {
        let md = "# Title\n\nSome *text* here.\n\n- one\n- two\n\n1. first\n2. \
                  second\n\n```\ncode\n```\n\n> quote\n\n---\n";
        let b = markdown_to_blocks(md);
        assert_eq!(b[0], Block::Heading { level: 1, text: "Title".into() });
        assert_eq!(b[1], Block::Paragraph("Some text here.".into()));
        assert_eq!(
            b[2],
            Block::ListItem {
                ordered: None,
                depth: 0,
                text: "one".into()
            }
        );
        assert_eq!(
            b[4],
            Block::ListItem {
                ordered: Some(1),
                depth: 0,
                text: "first".into()
            }
        );
        assert_eq!(
            b[5],
            Block::ListItem {
                ordered: Some(2),
                depth: 0,
                text: "second".into()
            }
        );
        assert_eq!(b[6], Block::Code("code".into()));
        assert_eq!(b[7], Block::Quote("quote".into()));
        assert_eq!(b[8], Block::Rule);
    }

    #[test]
    fn html_blocks() {
        let html = "<html><head><title>x</title><style>p{}</style></head><body><h2>Head &amp; \
                    co</h2><p>Para \
                    <b>bold</b><br>line2</p><ul><li>a</li><li>b</li></ul><pre>code\n  x</pre><hr><script>alert(1)</script>tail</body></html>";
        let b = html_to_blocks(html);
        assert_eq!(b[0], Block::Heading { level: 2, text: "Head & co".into() });
        assert_eq!(b[1], Block::Paragraph("Para bold\nline2".into()));
        assert_eq!(
            b[2],
            Block::ListItem {
                ordered: None,
                depth: 0,
                text: "a".into()
            }
        );
        assert_eq!(b[4], Block::Code("code\n  x".into()));
        assert_eq!(b[5], Block::Rule);
        assert_eq!(b[6], Block::Paragraph("tail".into()));
        assert_eq!(decode_entities("&#65;&#x42;&nbsp;&bogus;"), "AB\u{a0}&bogus;");
    }

    #[test]
    fn render_paginates() {
        let long: Vec<Block> = (0..200)
            .map(|i| {
                Block::Paragraph(format!(
                    "Paragraph {i} with enough words to wrap around the line at least once or \
                     twice hopefully."
                ))
            })
            .collect();
        let opts = LayoutOptions {
            page_numbers: true,
            title: Some("T".into()),
            ..Default::default()
        };
        let out = render_blocks(&long, &opts).unwrap();
        assert!(out["output"]["pages"].as_u64().unwrap() > 3);
        let bytes = engine_shared::b64::decode(out["output"]["file"].as_str().unwrap()).unwrap();
        let d = doc::load(&bytes, None).unwrap();
        let txt = d.extract_text(&[1]).unwrap();
        assert!(txt.contains("Paragraph 0"));
        assert!(txt.contains("Page 1 of"));
        let bad = LayoutOptions { font_size: 0.0, ..Default::default() };
        assert!(render_blocks(&long, &bad).is_err());
        let bad2 = LayoutOptions {
            margin_left: 400.0,
            margin_right: 400.0,
            ..Default::default()
        };
        assert!(render_blocks(&long, &bad2).is_err());
        // Unicode outside WinAnsi is reported.
        let out =
            render_blocks(&[Block::Paragraph("漢字".into())], &LayoutOptions::default()).unwrap();
        assert_eq!(out["substituted_characters"], 2);
    }
}
