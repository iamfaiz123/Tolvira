//! # Dev Toolbox Document Engine (`document-engine`)
//!
//! Office documents are ZIP + XML; this engine reads them with `zip` +
//! `quick-xml` into a format-neutral model (headings, paragraphs with run
//! formatting, lists, tables, code, page breaks) and renders text, HTML,
//! Markdown or JSON. It writes DOCX (from Markdown, text or any readable
//! document) and fills `{{placeholder}}` templates. Page-faithful rendering
//! (DOCX/PPTX → PDF/images) is out of scope for WASM without a layout engine:
//! `to_html` output can be sent to `pdf-engine.from_html`.
//!
//! | Action | Tool(s) |
//! |---|---|
//! | `inspect` | docx-metadata-viewer, pptx-metadata-viewer |
//! | `to_text` | docx-to-text, rtf-to-text |
//! | `to_html` | docx-to-html, rtf-to-html, docx-to-pdf / pptx-to-pdf (→ pdf.from_html) |
//! | `to_markdown` | docx-to-markdown |
//! | `fill_template` | docx-template-fill |
//! | `from_markdown` | markdown-to-docx |
//! | `convert` | odt-to-docx (and any readable → docx) |
//! | `pptx_slides` | pptx-metadata-viewer, pptx-to-images (text outline; rendering → backend) |

pub mod ebook;
pub mod model;
pub mod ooxml;
pub mod rtf;

/// JSON value → plain text (strings unquoted).
pub fn ooxml_value_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

use engine_shared::{EngineError, EngineResult};
use model::{Block, Doc, Inline};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde::Deserialize;
use serde_json::{json, Value};

/// Document input: a base64 file and/or inline text, with the original file name.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct FileInput {
    /// File bytes (base64 on the wire).
    #[serde(with = "engine_shared::b64::option")]
    pub file: Option<Vec<u8>>,
    /// Inline text for RTF / Markdown inputs.
    pub text: Option<String>,
    /// Original file name (used for format detection and output names).
    pub name: Option<String>,
}

fn base_name(input: &FileInput, fallback: &str) -> String {
    input
        .name
        .as_deref()
        .map(|n| n.rsplit('/').next().unwrap_or(n))
        .and_then(|n| {
            n.rsplit_once('.')
                .map(|(b, _)| b.to_string())
                .or_else(|| Some(n.to_string()))
        })
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

/// Loads any supported input into the model. Returns (doc, format).
pub fn load(input: &FileInput) -> EngineResult<(Doc, &'static str)> {
    if let Some(t) = input.text.as_deref().filter(|t| !t.trim().is_empty()) {
        if t.trim_start().starts_with("{\\rtf") {
            return Ok((rtf::parse(t)?, "rtf"));
        }
        return Ok((markdown_to_doc(t), "markdown"));
    }
    let bytes = input
        .file
        .as_deref()
        .filter(|b| !b.is_empty())
        .ok_or_else(|| EngineError::empty("file"))?;
    if bytes.starts_with(b"{\\rtf") {
        return Ok((rtf::parse(&String::from_utf8_lossy(bytes))?, "rtf"));
    }
    let mut z = ooxml::open(bytes)?;
    match ooxml::detect(&mut z) {
        ooxml::Kind::Docx => Ok((ooxml::read_docx(&mut z)?, "docx")),
        ooxml::Kind::Pptx => Ok((ooxml::read_pptx(&mut z)?.0, "pptx")),
        ooxml::Kind::Odt | ooxml::Kind::Odp => Ok((ooxml::read_odt(&mut z)?, "odt")),
        ooxml::Kind::Xlsx | ooxml::Kind::Ods => Err(EngineError::unsupported_format(
            "Spreadsheets are handled by the spreadsheet engine",
        )),
        ooxml::Kind::Unknown => Err(EngineError::unsupported_format(
            "Unrecognised package (expected DOCX, PPTX, ODT or RTF)",
        )),
    }
}

/// Markdown → model via pulldown-cmark.
pub fn markdown_to_doc(md: &str) -> Doc {
    let mut doc = Doc::default();
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(md, opts);
    let mut inlines: Vec<Inline> = Vec::new();
    let mut style = Inline::default();
    let mut heading: Option<u8> = None;
    let mut list_stack: Vec<bool> = Vec::new();
    let mut in_item = false;
    let mut in_code = false;
    let mut code = String::new();
    let mut in_quote = false;
    let mut table: Option<Vec<Vec<String>>> = None;
    let mut row: Vec<String> = Vec::new();
    let mut cell = String::new();
    let mut in_cell = false;
    let flush = |doc: &mut Doc,
                 inlines: &mut Vec<Inline>,
                 heading: &mut Option<u8>,
                 list_stack: &[bool],
                 in_item: bool,
                 in_quote: bool| {
        if inlines.is_empty() {
            return;
        }
        let items = std::mem::take(inlines);
        if let Some(h) = heading.take() {
            doc.blocks.push(Block::Heading(h, items));
        } else if let (true, Some(ordered)) = (in_item, list_stack.last().copied()) {
            doc.blocks
                .push(Block::ListItem((list_stack.len() - 1) as u8, ordered, items));
        } else if in_quote {
            doc.blocks.push(Block::Quote(items));
        } else {
            doc.blocks.push(Block::Paragraph(items));
        }
    };
    for ev in parser {
        match ev {
            Event::Start(Tag::Heading { level, .. }) => {
                flush(&mut doc, &mut inlines, &mut heading, &list_stack, in_item, in_quote);
                heading = Some(match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                });
            },
            Event::End(TagEnd::Heading(_)) | Event::End(TagEnd::Paragraph) => {
                flush(&mut doc, &mut inlines, &mut heading, &list_stack, in_item, in_quote)
            },
            Event::Start(Tag::List(start)) => list_stack.push(start.is_some()),
            Event::End(TagEnd::List(_)) => {
                list_stack.pop();
            },
            Event::Start(Tag::Item) => in_item = true,
            Event::End(TagEnd::Item) => {
                flush(&mut doc, &mut inlines, &mut heading, &list_stack, in_item, in_quote);
                in_item = false;
            },
            Event::Start(Tag::BlockQuote(_)) => in_quote = true,
            Event::End(TagEnd::BlockQuote(_)) => in_quote = false,
            Event::Start(Tag::CodeBlock(_)) => {
                in_code = true;
                code.clear();
            },
            Event::End(TagEnd::CodeBlock) => {
                in_code = false;
                doc.blocks.push(Block::Code(code.trim_end_matches('\n').to_string()));
            },
            Event::Start(Tag::Emphasis) => style.italic = true,
            Event::End(TagEnd::Emphasis) => style.italic = false,
            Event::Start(Tag::Strong) => style.bold = true,
            Event::End(TagEnd::Strong) => style.bold = false,
            Event::Start(Tag::Strikethrough) => style.strike = true,
            Event::End(TagEnd::Strikethrough) => style.strike = false,
            Event::Start(Tag::Link { dest_url, .. }) => style.link = Some(dest_url.to_string()),
            Event::End(TagEnd::Link) => style.link = None,
            Event::Start(Tag::Image { dest_url, .. }) => {
                doc.images += 1;
                inlines.push(Inline {
                    text: format!("[image: {dest_url}]"),
                    ..Default::default()
                });
            },
            Event::Start(Tag::Table(_)) => table = Some(Vec::new()),
            Event::End(TagEnd::Table) => {
                if let Some(t) = table.take() {
                    doc.blocks.push(Block::Table(t));
                }
            },
            Event::Start(Tag::TableHead) | Event::Start(Tag::TableRow) => row.clear(),
            Event::End(TagEnd::TableHead) | Event::End(TagEnd::TableRow) => {
                if let Some(t) = table.as_mut() {
                    t.push(std::mem::take(&mut row));
                }
            },
            Event::Start(Tag::TableCell) => {
                in_cell = true;
                cell.clear();
            },
            Event::End(TagEnd::TableCell) => {
                in_cell = false;
                row.push(std::mem::take(&mut cell));
            },
            Event::Rule => doc.blocks.push(Block::Rule),
            Event::Text(t) => {
                if in_code {
                    code.push_str(&t);
                } else if in_cell {
                    cell.push_str(&t);
                } else {
                    inlines.push(Inline { text: t.to_string(), ..style.clone() });
                }
            },
            Event::Code(t) => {
                if in_cell {
                    cell.push_str(&t);
                } else {
                    inlines.push(Inline {
                        text: t.to_string(),
                        code: true,
                        ..style.clone()
                    });
                }
            },
            Event::SoftBreak => inlines.push(Inline { text: " ".into(), ..style.clone() }),
            Event::HardBreak => inlines.push(Inline { text: "\n".into(), ..style.clone() }),
            Event::Html(h) | Event::InlineHtml(h) => {
                let stripped: String = strip_tags(&h);
                if !stripped.trim().is_empty() {
                    inlines.push(Inline { text: stripped, ..style.clone() });
                }
            },
            _ => {},
        }
    }
    flush(&mut doc, &mut inlines, &mut heading, &list_stack, in_item, in_quote);
    doc
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {},
        }
    }
    out
}

fn file_bytes(input: &FileInput) -> EngineResult<&[u8]> {
    input
        .file
        .as_deref()
        .filter(|b| !b.is_empty())
        .ok_or_else(|| EngineError::empty("file"))
}

fn docx_json(bytes: Vec<u8>, name: &str) -> Value {
    json!({
        "file": engine_shared::b64::encode(&bytes),
        "filename": format!("{name}.docx"),
        "mime": "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "bytes": bytes.len()
    })
}

/// Every action understood by the document engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum DocAction {
    /// Format, metadata, outline and statistics.
    Inspect {
        /// Input document.
        #[serde(flatten)]
        input: FileInput,
    },
    /// Plain-text extraction.
    ToText {
        /// Input document.
        #[serde(flatten)]
        input: FileInput,
    },
    /// Structural HTML conversion.
    ToHtml {
        /// Input document.
        #[serde(flatten)]
        input: FileInput,
        /// Wrap in a full `<html>` page.
        #[serde(default = "default_true")]
        full_page: bool,
    },
    /// Markdown conversion.
    ToMarkdown {
        /// Input document.
        #[serde(flatten)]
        input: FileInput,
    },
    /// Structural JSON (blocks, inlines, metadata).
    ToJson {
        /// Input document.
        #[serde(flatten)]
        input: FileInput,
    },
    /// Replaces `{{placeholder}}` tokens in a DOCX (body, headers, footers).
    FillTemplate {
        /// Input DOCX.
        #[serde(flatten)]
        input: FileInput,
        /// Placeholder → value map.
        values: serde_json::Map<String, Value>,
        /// Opening delimiter (default `{{`).
        #[serde(default)]
        open: String,
        /// Closing delimiter (default `}}`).
        #[serde(default)]
        close: String,
    },
    /// Builds a DOCX from Markdown.
    FromMarkdown {
        /// Markdown source.
        markdown: String,
        /// Document title.
        #[serde(default)]
        title: String,
        /// Output file name (without extension).
        #[serde(default)]
        name: String,
    },
    /// Converts between DOCX/ODT/RTF/Markdown and DOCX/HTML/Markdown/text.
    Convert {
        /// Input document.
        #[serde(flatten)]
        input: FileInput,
        /// Target format (`docx`, `html`, `markdown`, `txt`).
        #[serde(default)]
        to: String,
    },
    /// Slide-by-slide text outline of a PPTX.
    PptxSlides {
        /// Input presentation.
        #[serde(flatten)]
        input: FileInput,
    },
    // ebook catalog engine
    /// EPUB metadata, chapters and statistics.
    EpubInspect {
        /// Input EPUB.
        #[serde(flatten)]
        input: FileInput,
    },
    /// Extracts one EPUB chapter.
    EpubChapter {
        /// Input EPUB.
        #[serde(flatten)]
        input: FileInput,
        /// 0-based chapter index in spine order.
        #[serde(default)]
        index: usize,
        /// `html` (default), or `raw` for the original XHTML.
        #[serde(default)]
        format: String,
    },
    /// Flattens an EPUB into one HTML document.
    EpubToHtml {
        /// Input EPUB.
        #[serde(flatten)]
        input: FileInput,
        /// Prepend a table of contents.
        #[serde(default = "default_true")]
        include_toc: bool,
    },
    /// Converts an EPUB to HTML/text/Markdown/DOCX.
    EpubConvert {
        /// Input EPUB.
        #[serde(flatten)]
        input: FileInput,
        /// Target format (`html` default, `txt`, `markdown`, `docx`).
        #[serde(default)]
        to: String,
    },
    /// Extracts the EPUB cover image.
    EpubCover {
        /// Input EPUB.
        #[serde(flatten)]
        input: FileInput,
    },
    /// Validates EPUB structure.
    EpubValidate {
        /// Input EPUB.
        #[serde(flatten)]
        input: FileInput,
    },
    /// Rewrites Dublin Core metadata in an EPUB.
    EpubSetMetadata {
        /// Input EPUB.
        #[serde(flatten)]
        input: FileInput,
        /// Metadata field → new value (`title`, `creator`, `language`, `publisher`, `description`,
        /// `identifier`, `date`).
        values: serde_json::Map<String, Value>,
    },
    /// Reads MOBI/AZW headers and EXTH metadata.
    MobiMetadata {
        /// Input MOBI file.
        #[serde(flatten)]
        input: FileInput,
    },
    /// Always returns `E_UNSUPPORTED`: page-faithful rendering needs LibreOffice on the backend.
    Render {
        /// Catalog tool id, echoed back in the guidance.
        #[serde(default)]
        tool: String,
    },
}

fn default_true() -> bool {
    true
}

/// Dispatches a [`DocAction`] to its implementation.
pub fn handle_action(action: DocAction) -> EngineResult<Value> {
    match action {
        DocAction::Inspect { input } => {
            let (doc, format) = load(&input)?;
            Ok(json!({
                "format": format,
                "filename": input.name,
                "metadata": doc.metadata,
                "stats": doc.stats(),
                "outline": doc.outline(),
                "warnings": doc.warnings
            }))
        },
        DocAction::ToText { input } => {
            let (doc, format) = load(&input)?;
            Ok(json!({
                "text": doc.to_text(),
                "format": format,
                "stats": doc.stats(),
                "warnings": doc.warnings
            }))
        },
        DocAction::ToHtml { input, full_page } => {
            let (doc, format) = load(&input)?;
            let title = doc
                .metadata
                .get("title")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| base_name(&input, "Document"));
            Ok(json!({
                "html": doc.to_html(full_page, &title),
                "format": format,
                "stats": doc.stats(),
                "next_step": "Send `html` to pdf-engine action `from_html` for a PDF",
                "fidelity": "Structure and inline formatting are preserved; page layout, fonts, \
                             images and positioning are not"
            }))
        },
        DocAction::ToMarkdown { input } => {
            let (doc, format) = load(&input)?;
            Ok(json!({ "markdown": doc.to_markdown(), "format": format, "stats": doc.stats() }))
        },
        DocAction::ToJson { input } => {
            let (doc, format) = load(&input)?;
            Ok(json!({
                "blocks": doc.to_json(),
                "metadata": doc.metadata,
                "format": format,
                "stats": doc.stats()
            }))
        },
        DocAction::FillTemplate { input, values, open, close } => {
            let bytes = input
                .file
                .as_deref()
                .filter(|b| !b.is_empty())
                .ok_or_else(|| EngineError::empty("file"))?;
            if values.is_empty() {
                return Err(EngineError::empty("values"));
            }
            let (o, c) = (
                if open.is_empty() { "{{" } else { open.as_str() },
                if close.is_empty() { "}}" } else { close.as_str() },
            );
            let (out, report) = ooxml::fill_template(bytes, &values, (o, c))?;
            let mut v = docx_json(out, &format!("{}_filled", base_name(&input, "template")));
            v["report"] = report;
            Ok(v)
        },
        DocAction::FromMarkdown { markdown, title, name } => {
            engine_shared::error::require_text(&markdown, "markdown")?;
            let doc = markdown_to_doc(&markdown);
            let title = if title.is_empty() {
                doc.blocks
                    .iter()
                    .find_map(|b| {
                        if let Block::Heading(_, i) = b {
                            Some(model::plain(i))
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "Document".into())
            } else {
                title
            };
            let bytes = ooxml::write_docx(&doc, &title)?;
            let mut v = docx_json(bytes, if name.is_empty() { "document" } else { &name });
            v["stats"] = doc.stats();
            Ok(v)
        },
        DocAction::Convert { input, to } => {
            let (doc, format) = load(&input)?;
            let name = base_name(&input, "document");
            match to.to_ascii_lowercase().as_str() {
                "" | "docx" => {
                    let title = doc
                        .metadata
                        .get("title")
                        .and_then(Value::as_str)
                        .unwrap_or(&name)
                        .to_string();
                    let bytes = ooxml::write_docx(&doc, &title)?;
                    let mut v = docx_json(bytes, &name);
                    v["from_format"] = json!(format);
                    v["stats"] = doc.stats();
                    Ok(v)
                },
                "html" => Ok(json!({ "html": doc.to_html(true, &name), "from_format": format })),
                "markdown" | "md" => Ok(json!({
                    "markdown": doc.to_markdown(),
                    "from_format": format
                })),
                "txt" | "text" => Ok(json!({ "text": doc.to_text(), "from_format": format })),
                "pdf" => Err(EngineError::unsupported(
                    "Use `to_html` here and then pdf-engine `from_html`; direct DOCX→PDF needs a \
                     layout engine (backend LibreOffice for full fidelity)",
                )),
                other => Err(EngineError::unsupported_format(format!(
                    "Cannot convert to `{other}` (docx, html, markdown, text)"
                ))),
            }
        },
        DocAction::PptxSlides { input } => {
            let bytes = input
                .file
                .as_deref()
                .filter(|b| !b.is_empty())
                .ok_or_else(|| EngineError::empty("file"))?;
            let mut z = ooxml::open(bytes)?;
            if ooxml::detect(&mut z) != ooxml::Kind::Pptx {
                return Err(EngineError::unsupported_format("Not a PPTX presentation"));
            }
            let (doc, slides) = ooxml::read_pptx(&mut z)?;
            Ok(json!({
                "slides": slides,
                "slide_count": slides.len(),
                "metadata": doc.metadata,
                "images": doc.images,
                "outline_html": doc.to_html(true, "Slides"),
                "rendering": "Slide images need a renderer (backend LibreOffice); this is the \
                              text outline"
            }))
        },
        DocAction::EpubInspect { input } => ebook::inspect(file_bytes(&input)?),
        DocAction::EpubChapter { input, index, format } => {
            ebook::chapter(file_bytes(&input)?, index, &format)
        },
        DocAction::EpubToHtml { input, include_toc } => {
            let (html, merged, epub) = ebook::to_html(file_bytes(&input)?, include_toc)?;
            Ok(json!({
                "html": html,
                "metadata": epub.metadata,
                "chapters": epub.spine.len(),
                "stats": merged.stats(),
                "next_step": "Send `html` to pdf-engine action `from_html` for a PDF"
            }))
        },
        DocAction::EpubConvert { input, to } => {
            let (html, merged, epub) = ebook::to_html(file_bytes(&input)?, false)?;
            let name = base_name(&input, "book");
            match to.to_ascii_lowercase().as_str() {
                "" | "html" => Ok(json!({
                    "html": html,
                    "filename": format!("{name}.html"),
                    "metadata": epub.metadata
                })),
                "txt" | "text" => Ok(json!({
                    "text": merged.to_text(),
                    "filename": format!("{name}.txt")
                })),
                "markdown" | "md" => Ok(json!({
                    "markdown": merged.to_markdown(),
                    "filename": format!("{name}.md")
                })),
                "docx" => {
                    let title = epub
                        .metadata
                        .get("title")
                        .map(ooxml_value_text)
                        .unwrap_or_else(|| name.clone());
                    let mut v = docx_json(ooxml::write_docx(&merged, &title)?, &name);
                    v["metadata"] = json!(epub.metadata);
                    Ok(v)
                },
                "pdf" => {
                    Err(EngineError::unsupported("Use `epub_to_html` then pdf-engine `from_html`"))
                },
                "mobi" | "azw3" | "kfx" => Err(EngineError::unsupported(
                    "Kindle formats need Amazon's KindleGen/Calibre; export HTML/DOCX here and \
                     convert on the backend",
                )),
                other => Err(EngineError::unsupported_format(format!(
                    "Cannot convert to `{other}` (html, text, markdown, docx)"
                ))),
            }
        },
        DocAction::EpubCover { input } => ebook::cover(file_bytes(&input)?),
        DocAction::EpubValidate { input } => ebook::validate(file_bytes(&input)?),
        DocAction::EpubSetMetadata { input, values } => {
            let (out, changed) = ebook::set_metadata(file_bytes(&input)?, &values)?;
            Ok(json!({
                "file": engine_shared::b64::encode(&out),
                "filename": format!("{}_edited.epub", base_name(&input, "book")),
                "mime": "application/epub+zip",
                "bytes": out.len(),
                "changed": changed
            }))
        },
        DocAction::MobiMetadata { input } => ebook::mobi_metadata(file_bytes(&input)?),
        DocAction::Render { tool } => Err(EngineError::unsupported(format!(
            "{} requires page rendering (fonts, layout, images) that is not available in WASM; \
             use `to_html` + pdf-engine `from_html` for a structural PDF or the backend \
             LibreOffice service for full fidelity",
            if tool.is_empty() { "Rendering" } else { tool.as_str() }
        ))),
    }
}

engine_shared::export_engine!(handle_action, DocAction);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_docx_roundtrip() {
        let md = "# Title\n\nSome **bold** and *em* text with `code` and a \
                  [link](https://x.y).\n\n- a\n- b\n  - nested\n\n1. one\n2. two\n\n> \
                  quote\n\n```\nlet x = 1;\n```\n\n| h1 | h2 |\n|---|---|\n| 1 | 2 |\n\n---\n";
        let doc = markdown_to_doc(md);
        assert!(matches!(&doc.blocks[0], Block::Heading(1, _)));
        assert!(doc.blocks.iter().any(|b| matches!(b, Block::ListItem(1, false, _))));
        assert!(doc.blocks.iter().any(|b| matches!(b, Block::ListItem(0, true, _))));
        assert!(doc.blocks.iter().any(|b| matches!(b, Block::Quote(_))));
        assert!(doc
            .blocks
            .iter()
            .any(|b| matches!(b, Block::Table(t) if t.len() == 2)));
        let bytes = ooxml::write_docx(&doc, "T").unwrap();
        let (back, fmt) = load(&FileInput {
            file: Some(bytes),
            text: None,
            name: Some("t.docx".into()),
        })
        .unwrap();
        assert_eq!(fmt, "docx");
        let md2 = back.to_markdown();
        assert!(
            md2.contains("# Title") && md2.contains("**bold**") && md2.contains("| h1 | h2 |"),
            "{md2}"
        );
        assert!(md2.contains("- a") && md2.contains("1. one"), "{md2}");
        assert!(load(&FileInput::default()).is_err());
        assert!(
            matches!(load(&FileInput { file: Some(b"garbage".to_vec()), ..Default::default() }), Err(e) if e.code == engine_shared::error::ErrorCode::UnsupportedFormat)
        );
        let (r, f) = load(&FileInput {
            text: Some(r"{\rtf1 Hello \b RTF\b0\par}".into()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(f, "rtf");
        assert!(r.to_text().contains("Hello RTF"));
    }
}
