//! EPUB (2/3) reading, validation, metadata editing, cover extraction and
//! MOBI/AZW header metadata.

use crate::model::{Block, Doc, Inline};
use crate::ooxml::{open, read_entry, Archive};
use engine_shared::{EngineError, EngineResult};
use quick_xml::events::Event;
use quick_xml::Reader;
use serde_json::{json, Map, Value};
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;

fn attr(e: &quick_xml::events::BytesStart, name: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.local_name().as_ref() == name)
        .map(|a| String::from_utf8_lossy(&a.value).to_string())
}

fn local(e: &quick_xml::events::BytesStart) -> String {
    String::from_utf8_lossy(e.local_name().as_ref()).to_string()
}

fn join_path(base: &str, rel: &str) -> String {
    if rel.starts_with('/') {
        return rel.trim_start_matches('/').to_string();
    }
    let rel = rel.split(['#', '?']).next().unwrap_or(rel);
    let mut parts: Vec<&str> = base
        .rsplit_once('/')
        .map(|(d, _)| d.split('/').collect())
        .unwrap_or_default();
    for seg in rel.split('/') {
        match seg {
            "." | "" => {},
            ".." => {
                parts.pop();
            },
            s => parts.push(s),
        }
    }
    percent_decode(&parts.join("/"))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Parsed EPUB container structure.
pub struct Epub {
    /// Path of the OPF package document inside the ZIP.
    pub opf_path: String,
    /// Dublin Core metadata (title, creator, language, identifier, …).
    pub metadata: Map<String, Value>,
    /// Manifest items as `(id, resolved href, media type, properties)`.
    pub manifest: Vec<(String, String, String, String)>, // id, href(resolved), media type, properties
    /// Spine item ids in reading order.
    pub spine: Vec<String>, // idrefs
    /// Table of contents as `(depth, label, href)`.
    pub toc: Vec<(u8, String, String)>, // depth, label, href
    /// EPUB version (`2.0`, `3.0`, …).
    pub version: String,
    /// Href of the cover image, if declared.
    pub cover_href: Option<String>,
}

/// Parses `META-INF/container.xml`, the OPF package and the NCX/nav table of contents.
pub fn parse_epub(z: &mut Archive) -> EngineResult<Epub> {
    let container = read_entry(z, "META-INF/container.xml")
        .ok_or_else(|| EngineError::malformed("META-INF/container.xml is missing (not an EPUB)"))?;
    let mut opf_path = None;
    let mut r = Reader::from_reader(container.as_slice());
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local(&e) == "rootfile" => {
                opf_path = attr(&e, b"full-path")
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {},
        }
    }
    let opf_path =
        opf_path.ok_or_else(|| EngineError::malformed("container.xml has no rootfile"))?;
    let opf = read_entry(z, &opf_path).ok_or_else(|| {
        EngineError::malformed(format!("OPF `{opf_path}` referenced by container.xml is missing"))
    })?;
    let mut metadata = Map::new();
    let mut manifest = Vec::new();
    let mut spine = Vec::new();
    let mut version = String::from("2.0");
    let mut cover_id: Option<String> = None;
    let mut r = Reader::from_reader(opf.as_slice());
    let mut current_meta: Option<String> = None;
    let mut in_metadata = false;
    let mut refines: Vec<(String, String, String)> = Vec::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let n = local(&e);
                let empty = matches!(r.buffer_position(), _ if false);
                let _ = empty;
                match n.as_str() {
                    "package" => {
                        if let Some(v) = attr(&e, b"version") {
                            version = v;
                        }
                    },
                    "metadata" => in_metadata = true,
                    "meta" if in_metadata => {
                        if attr(&e, b"name").as_deref() == Some("cover") {
                            cover_id = attr(&e, b"content");
                        }
                        if let (Some(prop), Some(refine)) =
                            (attr(&e, b"property"), attr(&e, b"refines"))
                        {
                            refines.push((
                                refine.trim_start_matches('#').to_string(),
                                prop,
                                String::new(),
                            ));
                            current_meta = Some("__refine".into());
                        } else if let Some(prop) = attr(&e, b"property") {
                            current_meta = Some(prop.replace("dcterms:", ""));
                        }
                    },
                    "item" => {
                        if let (Some(id), Some(href)) = (attr(&e, b"id"), attr(&e, b"href")) {
                            manifest.push((
                                id,
                                join_path(&opf_path, &href),
                                attr(&e, b"media-type").unwrap_or_default(),
                                attr(&e, b"properties").unwrap_or_default(),
                            ));
                        }
                    },
                    "itemref" => {
                        if let Some(idref) = attr(&e, b"idref") {
                            if attr(&e, b"linear").as_deref() != Some("no") {
                                spine.push(idref);
                            }
                        }
                    },
                    _ if in_metadata => {
                        let key = match n.as_str() {
                            "creator" => "author".to_string(),
                            other => other.to_string(),
                        };
                        current_meta = Some(key);
                    },
                    _ => {},
                }
            },
            Ok(Event::Text(t)) => {
                if let Some(k) = &current_meta {
                    let v = t.unescape().map(|s| s.to_string()).unwrap_or_default();
                    if v.trim().is_empty() {
                        continue;
                    }
                    if k == "__refine" {
                        if let Some(last) = refines.last_mut() {
                            last.2 = v.trim().to_string();
                        }
                    } else {
                        match metadata.get_mut(k) {
                            Some(Value::Array(a)) => a.push(json!(v.trim())),
                            Some(existing) => {
                                let prev = existing.take();
                                *existing = json!([prev, v.trim()]);
                            },
                            None => {
                                metadata.insert(k.clone(), json!(v.trim()));
                            },
                        }
                    }
                }
            },
            Ok(Event::End(e)) => {
                let n = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if n == "metadata" {
                    in_metadata = false;
                }
                current_meta = None;
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(EngineError::malformed(format!("OPF parse error: {e}"))),
            _ => {},
        }
    }
    if !refines.is_empty() {
        metadata.insert(
            "refinements".into(),
            json!(refines
                .iter()
                .map(|(id, p, v)| json!({ "refines": id, "property": p, "value": v }))
                .collect::<Vec<_>>()),
        );
    }
    let cover_href = manifest
        .iter()
        .find(|m| m.3.contains("cover-image"))
        .or_else(|| cover_id.as_ref().and_then(|c| manifest.iter().find(|m| &m.0 == c)))
        .or_else(|| {
            manifest
                .iter()
                .find(|m| m.2.starts_with("image/") && m.1.to_ascii_lowercase().contains("cover"))
        })
        .map(|m| m.1.clone());
    // TOC: EPUB3 nav, fallback NCX.
    let mut toc = Vec::new();
    if let Some(nav) = manifest.iter().find(|m| m.3.contains("nav")) {
        if let Some(bytes) = read_entry(z, &nav.1) {
            toc = parse_nav(&bytes, &nav.1);
        }
    }
    if toc.is_empty() {
        if let Some(ncx) = manifest.iter().find(|m| m.2 == "application/x-dtbncx+xml") {
            if let Some(bytes) = read_entry(z, &ncx.1) {
                toc = parse_ncx(&bytes, &ncx.1);
            }
        }
    }
    Ok(Epub {
        opf_path,
        metadata,
        manifest,
        spine,
        toc,
        version,
        cover_href,
    })
}

fn parse_nav(bytes: &[u8], base: &str) -> Vec<(u8, String, String)> {
    let mut r = Reader::from_reader(bytes);
    let mut out = Vec::new();
    let mut in_nav = false;
    let mut depth: i32 = 0;
    let mut href: Option<String> = None;
    let mut label = String::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(&e).as_str() {
                "nav" => in_nav = attr(&e, b"type").map(|t| t.contains("toc")).unwrap_or(true),
                "ol" if in_nav => depth += 1,
                "a" if in_nav => {
                    href = attr(&e, b"href").map(|h| join_path(base, &h));
                    label.clear();
                },
                _ => {},
            },
            Ok(Event::Text(t)) if href.is_some() => {
                label.push_str(&t.unescape().map(|s| s.to_string()).unwrap_or_default())
            },
            Ok(Event::End(e)) => match String::from_utf8_lossy(e.local_name().as_ref()).as_ref() {
                "a" => {
                    if let Some(h) = href.take() {
                        out.push(((depth - 1).max(0) as u8, label.trim().to_string(), h));
                    }
                },
                "ol" if in_nav => depth -= 1,
                "nav" => in_nav = false,
                _ => {},
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {},
        }
    }
    out
}

fn parse_ncx(bytes: &[u8], base: &str) -> Vec<(u8, String, String)> {
    let mut r = Reader::from_reader(bytes);
    let mut out = Vec::new();
    let mut depth: i32 = 0;
    let mut label = String::new();
    let mut in_text = false;
    let mut pending: Vec<(u8, String)> = Vec::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(&e).as_str() {
                "navPoint" => {
                    depth += 1;
                    label.clear();
                },
                "text" => in_text = true,
                _ => {},
            },
            Ok(Event::Empty(e)) if local(&e) == "content" => {
                if let Some(src) = attr(&e, b"src") {
                    let lbl =
                        pending.pop().map(|p| p.1).unwrap_or_else(|| label.trim().to_string());
                    out.push(((depth - 1).max(0) as u8, lbl, join_path(base, &src)));
                }
            },
            Ok(Event::Text(t)) if in_text => {
                label.push_str(&t.unescape().map(|s| s.to_string()).unwrap_or_default())
            },
            Ok(Event::End(e)) => match String::from_utf8_lossy(e.local_name().as_ref()).as_ref() {
                "text" => {
                    in_text = false;
                    pending.push((depth as u8, label.trim().to_string()));
                    label.clear();
                },
                "navPoint" => depth -= 1,
                _ => {},
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {},
        }
    }
    out
}

/// XHTML chapter → document model (block-level structure with inline formatting).
pub fn xhtml_to_doc(bytes: &[u8]) -> Doc {
    let text = String::from_utf8_lossy(bytes);
    let mut r = Reader::from_str(&text);
    r.config_mut().check_end_names = false;
    let mut doc = Doc::default();
    let mut inlines: Vec<Inline> = Vec::new();
    let mut style_stack: Vec<Inline> = vec![Inline::default()];
    let mut heading: Option<u8> = None;
    let mut list_stack: Vec<bool> = Vec::new();
    let mut in_item = false;
    let mut in_pre = false;
    let mut in_skip = 0;
    let mut in_body = false;
    let mut table: Option<Vec<Vec<String>>> = None;
    let mut row: Vec<String> = Vec::new();
    let mut cell: Option<String> = None;
    let flush = |doc: &mut Doc,
                 inlines: &mut Vec<Inline>,
                 heading: &mut Option<u8>,
                 list_stack: &[bool],
                 in_item: bool,
                 in_pre: bool| {
        if inlines.iter().all(|i| i.text.trim().is_empty()) {
            inlines.clear();
            *heading = None;
            return;
        }
        let items = std::mem::take(inlines);
        if in_pre {
            doc.blocks.push(Block::Code(crate::model::plain(&items)));
        } else if let Some(h) = heading.take() {
            doc.blocks.push(Block::Heading(h, items));
        } else if let (true, Some(ordered)) = (in_item, list_stack.last().copied()) {
            doc.blocks
                .push(Block::ListItem((list_stack.len() - 1) as u8, ordered, items));
        } else {
            doc.blocks.push(Block::Paragraph(items));
        }
    };
    while let Ok(ev) = r.read_event() {
        match ev {
            Event::Start(e) => {
                let n = local(&e).to_ascii_lowercase();
                match n.as_str() {
                    "body" => in_body = true,
                    "script" | "style" | "head" | "svg" => in_skip += 1,
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        flush(&mut doc, &mut inlines, &mut heading, &list_stack, in_item, in_pre);
                        heading = n[1..].parse().ok();
                    },
                    "p" | "div" | "section" | "article" | "blockquote" | "figcaption" | "dt"
                    | "dd" => {
                        flush(&mut doc, &mut inlines, &mut heading, &list_stack, in_item, in_pre)
                    },
                    "br" => inlines.push(Inline {
                        text: "\n".into(),
                        ..style_stack.last().cloned().unwrap_or_default()
                    }),
                    "ul" => list_stack.push(false),
                    "ol" => list_stack.push(true),
                    "li" => {
                        flush(&mut doc, &mut inlines, &mut heading, &list_stack, in_item, in_pre);
                        in_item = true;
                    },
                    "pre" => {
                        flush(&mut doc, &mut inlines, &mut heading, &list_stack, in_item, in_pre);
                        in_pre = true;
                    },
                    "b" | "strong" => style_stack.push(Inline {
                        bold: true,
                        ..style_stack.last().cloned().unwrap_or_default()
                    }),
                    "i" | "em" | "cite" => style_stack.push(Inline {
                        italic: true,
                        ..style_stack.last().cloned().unwrap_or_default()
                    }),
                    "u" => style_stack.push(Inline {
                        underline: true,
                        ..style_stack.last().cloned().unwrap_or_default()
                    }),
                    "s" | "del" | "strike" => style_stack.push(Inline {
                        strike: true,
                        ..style_stack.last().cloned().unwrap_or_default()
                    }),
                    "code" | "tt" | "kbd" => style_stack.push(Inline {
                        code: true,
                        ..style_stack.last().cloned().unwrap_or_default()
                    }),
                    "a" => style_stack.push(Inline {
                        link: attr(&e, b"href"),
                        ..style_stack.last().cloned().unwrap_or_default()
                    }),
                    "span" | "small" | "sup" | "sub" | "q" | "abbr" | "mark" | "time" => {
                        style_stack.push(style_stack.last().cloned().unwrap_or_default())
                    },
                    "img" | "image" => doc.images += 1,
                    "table" => {
                        flush(&mut doc, &mut inlines, &mut heading, &list_stack, in_item, in_pre);
                        table = Some(Vec::new());
                    },
                    "tr" => row.clear(),
                    "td" | "th" => cell = Some(String::new()),
                    "hr" => doc.blocks.push(Block::Rule),
                    _ => {},
                }
            },
            Event::Empty(e) => match local(&e).to_ascii_lowercase().as_str() {
                "br" => inlines.push(Inline {
                    text: "\n".into(),
                    ..style_stack.last().cloned().unwrap_or_default()
                }),
                "img" | "image" => doc.images += 1,
                "hr" => doc.blocks.push(Block::Rule),
                _ => {},
            },
            Event::Text(t) => {
                if in_skip > 0 || !in_body {
                    continue;
                }
                let s = t
                    .unescape()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|_| String::from_utf8_lossy(&t).to_string());
                let s = if in_pre { s } else { s.replace(['\n', '\r', '\t'], " ") };
                if let Some(c) = cell.as_mut() {
                    c.push_str(&s);
                } else {
                    let mut inl = style_stack.last().cloned().unwrap_or_default();
                    inl.text = s;
                    inlines.push(inl);
                }
            },
            Event::End(e) => {
                let n = String::from_utf8_lossy(e.local_name().as_ref()).to_ascii_lowercase();
                match n.as_str() {
                    "script" | "style" | "head" | "svg" => in_skip = (in_skip - 1).max(0),
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "div" | "section"
                    | "article" | "blockquote" | "figcaption" | "dt" | "dd" => {
                        flush(&mut doc, &mut inlines, &mut heading, &list_stack, in_item, in_pre)
                    },
                    "li" => {
                        flush(&mut doc, &mut inlines, &mut heading, &list_stack, in_item, in_pre);
                        in_item = false;
                    },
                    "ul" | "ol" => {
                        list_stack.pop();
                    },
                    "pre" => {
                        flush(&mut doc, &mut inlines, &mut heading, &list_stack, in_item, in_pre);
                        in_pre = false;
                    },
                    "b" | "strong" | "i" | "em" | "cite" | "u" | "s" | "del" | "strike"
                    | "code" | "tt" | "kbd" | "a" | "span" | "small" | "sup" | "sub" | "q"
                    | "abbr" | "mark" | "time" => {
                        if style_stack.len() > 1 {
                            style_stack.pop();
                        }
                    },
                    "td" | "th" => row.push(
                        cell.take()
                            .unwrap_or_default()
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" "),
                    ),
                    "tr" => {
                        if let Some(t) = table.as_mut() {
                            t.push(std::mem::take(&mut row));
                        }
                    },
                    "table" => {
                        if let Some(t) = table.take() {
                            if !t.is_empty() {
                                doc.blocks.push(Block::Table(t));
                            }
                        }
                    },
                    "body" => in_body = false,
                    _ => {},
                }
            },
            Event::Eof => break,
            _ => {},
        }
    }
    flush(&mut doc, &mut inlines, &mut heading, &list_stack, in_item, in_pre);
    // Collapse runs of spaces produced by source line wrapping.
    for b in doc.blocks.iter_mut() {
        if let Block::Paragraph(i) | Block::Heading(_, i) | Block::ListItem(_, _, i) = b {
            for inl in i.iter_mut() {
                while inl.text.contains("  ") {
                    inl.text = inl.text.replace("  ", " ");
                }
            }
            if let Some(f) = i.first_mut() {
                f.text = f.text.trim_start().to_string();
            }
            if let Some(l) = i.last_mut() {
                l.text = l.text.trim_end().to_string();
            }
        }
    }
    doc
}

/// Reads every spine item into a [`Doc`] (`(href, document)`).
pub fn chapters(z: &mut Archive, epub: &Epub) -> Vec<(String, Doc)> {
    let mut out = Vec::new();
    for idref in &epub.spine {
        if let Some(item) = epub.manifest.iter().find(|m| &m.0 == idref) {
            if let Some(bytes) = read_entry(z, &item.1) {
                out.push((item.1.clone(), xhtml_to_doc(&bytes)));
            }
        }
    }
    out
}

/// Reports metadata, structure, chapter list and statistics of an EPUB.
pub fn inspect(bytes: &[u8]) -> EngineResult<Value> {
    let mut z = open(bytes)?;
    let epub = parse_epub(&mut z)?;
    let chs = chapters(&mut z, &epub);
    let mut total_words = 0usize;
    let chapter_json: Vec<Value> = chs
        .iter()
        .enumerate()
        .map(|(i, (href, d))| {
            let stats = d.stats();
            total_words += stats["words"].as_u64().unwrap_or(0) as usize;
            let title = epub
                .toc
                .iter()
                .find(|t| t.2 == *href)
                .map(|t| t.1.clone())
                .or_else(|| {
                    d.blocks.iter().find_map(|b| {
                        if let Block::Heading(_, i) = b {
                            Some(crate::model::plain(i))
                        } else {
                            None
                        }
                    })
                });
            json!({
                "index": i,
                "href": href,
                "title": title,
                "words": stats["words"],
                "paragraphs": stats["paragraphs"],
                "images": d.images
            })
        })
        .collect();
    let media: Map<String, Value> = epub.manifest.iter().fold(Map::new(), |mut m, item| {
        let k = item.2.split('/').next().unwrap_or("other").to_string();
        m.insert(k.clone(), json!(m.get(&k).and_then(Value::as_u64).unwrap_or(0) + 1));
        m
    });
    Ok(json!({
        "format": "epub",
        "version": epub.version,
        "metadata": epub.metadata,
        "opf": epub.opf_path,
        "chapters": chapter_json,
        "chapter_count": chs.len(),
        "words": total_words,
        "reading_time_minutes": (total_words as f64 / 220.0).round(),
        "toc": epub.toc.iter().map(|(d, l, h)| json!({ "depth": d, "label": l, "href": h })).collect::<Vec<_>>(),
        "manifest_items": epub.manifest.len(),
        "media_types": media,
        "cover": epub.cover_href,
        "file_bytes": bytes.len(),
    }))
}

/// Extracts one chapter as text/Markdown/HTML (`format = "raw"` returns the original XHTML).
pub fn chapter(bytes: &[u8], index: usize, format: &str) -> EngineResult<Value> {
    let mut z = open(bytes)?;
    let epub = parse_epub(&mut z)?;
    let chs = chapters(&mut z, &epub);
    let (href, doc) = chs.get(index).ok_or_else(|| {
        EngineError::not_found(format!("Chapter {index} does not exist ({} chapters)", chs.len()))
    })?;
    let raw = read_entry(&mut z, href).unwrap_or_default();
    Ok(json!({
        "index": index,
        "href": href,
        "text": doc.to_text(),
        "markdown": doc.to_markdown(),
        "html": if format == "raw" { String::from_utf8_lossy(&raw).to_string() } else { doc.to_html(false, "") },
        "stats": doc.stats()
    }))
}

/// Flattens the whole book into a single HTML document (optionally with a table of contents).
pub fn to_html(bytes: &[u8], include_toc: bool) -> EngineResult<(String, Doc, Epub)> {
    let mut z = open(bytes)?;
    let epub = parse_epub(&mut z)?;
    let chs = chapters(&mut z, &epub);
    let title = epub
        .metadata
        .get("title")
        .map(crate::ooxml_value_text)
        .unwrap_or_else(|| "Book".into());
    let mut merged = Doc {
        metadata: epub.metadata.clone(),
        ..Default::default()
    };
    let mut body = String::new();
    if include_toc && !epub.toc.is_empty() {
        body.push_str("<nav class=\"toc\"><h2>Contents</h2><ol>");
        for (_, l, h) in &epub.toc {
            body.push_str(&format!("<li><a href=\"#{}\">{}</a></li>", anchor(h), html_escape(l)));
        }
        body.push_str("</ol></nav>\n");
    }
    for (href, d) in &chs {
        body.push_str(&format!(
            "<section id=\"{}\">\n{}</section>\n",
            anchor(href),
            d.to_html(false, "")
        ));
        merged.blocks.extend(d.blocks.iter().cloned());
        merged.images += d.images;
    }
    let html = format!("<!DOCTYPE html>\n<html><head><meta \
                        charset=\"utf-8\"><title>{}</title><style>body{{font-family:Georgia,serif;max-width:720px;margin:2rem auto;line-height:1.6}}section{{page-break-before:always}}nav.toc{{page-break-before:auto}}</style></head><body>\n<h1>{}</h1>\n{body}</body></html>\n", html_escape(&title), html_escape(&title));
    Ok((html, merged, epub))
}

fn anchor(h: &str) -> String {
    h.chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Extracts the cover image (base64) with its media type.
pub fn cover(bytes: &[u8]) -> EngineResult<Value> {
    let mut z = open(bytes)?;
    let epub = parse_epub(&mut z)?;
    let href = epub
        .cover_href
        .clone()
        .ok_or_else(|| EngineError::not_found("No cover image declared in the EPUB manifest"))?;
    let data = read_entry(&mut z, &href).ok_or_else(|| {
        EngineError::malformed(format!("Cover `{href}` is missing from the package"))
    })?;
    let mime = epub
        .manifest
        .iter()
        .find(|m| m.1 == href)
        .map(|m| m.2.clone())
        .unwrap_or_else(|| "image/jpeg".into());
    Ok(json!({
        "file": engine_shared::b64::encode(&data),
        "filename": href.rsplit('/').next().unwrap_or("cover"),
        "mime": mime,
        "bytes": data.len(),
        "href": href
    }))
}

/// Validates container structure, manifest/spine consistency and missing resources.
pub fn validate(bytes: &[u8]) -> EngineResult<Value> {
    let mut z = open(bytes)?;
    let mut issues: Vec<Value> = Vec::new();
    let mut push = |sev: &str, code: &str, msg: String| {
        issues.push(json!({ "severity": sev, "code": code, "message": msg }))
    };
    let names: Vec<String> = z.file_names().map(str::to_string).collect();
    match names.first().map(String::as_str) {
        Some("mimetype") => {
            let f = z.by_index(0).map_err(|e| EngineError::malformed(e.to_string()))?;
            if f.compression() != zip::CompressionMethod::Stored {
                push(
                    "error",
                    "OCF-mimetype-compressed",
                    "`mimetype` must be stored uncompressed".into(),
                );
            }
            drop(f);
            let content = read_entry(&mut z, "mimetype").unwrap_or_default();
            if String::from_utf8_lossy(&content).trim() != "application/epub+zip" {
                push(
                    "error",
                    "OCF-mimetype-content",
                    "`mimetype` must contain exactly `application/epub+zip`".into(),
                );
            }
        },
        _ => push(
            "error",
            "OCF-mimetype-first",
            "`mimetype` must be the first entry of the container".into(),
        ),
    }
    let epub = match parse_epub(&mut z) {
        Ok(e) => e,
        Err(e) => {
            push("error", "OPF", e.message.clone());
            return Ok(json!({ "valid": false, "issues": issues }));
        },
    };
    for key in ["title", "identifier", "language"] {
        if !epub.metadata.contains_key(key) {
            push("error", "OPF-metadata", format!("Required dc:{key} is missing"));
        }
    }
    if !epub.metadata.contains_key("author") {
        push("warning", "OPF-metadata", "No dc:creator (author)".into());
    }
    if epub.version.starts_with('3') && !epub.manifest.iter().any(|m| m.3.contains("nav")) {
        push(
            "error",
            "EPUB3-nav",
            "EPUB 3 requires a navigation document (item with properties=\"nav\")".into(),
        );
    }
    if epub.version.starts_with('2')
        && !epub.manifest.iter().any(|m| m.2 == "application/x-dtbncx+xml")
    {
        push("error", "EPUB2-ncx", "EPUB 2 requires an NCX table of contents".into());
    }
    let mut ids = std::collections::HashSet::new();
    for (id, href, media, _) in &epub.manifest {
        if !ids.insert(id.clone()) {
            push("error", "OPF-duplicate-id", format!("Duplicate manifest id `{id}`"));
        }
        if !names.iter().any(|n| n == href) {
            push(
                "error",
                "OPF-missing-file",
                format!("Manifest item `{id}` points to `{href}` which is not in the package"),
            );
        }
        if media.is_empty() {
            push("error", "OPF-media-type", format!("Manifest item `{id}` has no media-type"));
        }
    }
    for idref in &epub.spine {
        if !epub.manifest.iter().any(|m| &m.0 == idref) {
            push("error", "OPF-spine", format!("Spine idref `{idref}` is not in the manifest"));
        }
    }
    if epub.spine.is_empty() {
        push("error", "OPF-spine-empty", "Spine has no linear items".into());
    }
    let manifest_hrefs: std::collections::HashSet<&String> =
        epub.manifest.iter().map(|m| &m.1).collect();
    for n in &names {
        if n.ends_with('/') || n == "mimetype" || n.starts_with("META-INF/") || *n == epub.opf_path
        {
            continue;
        }
        if !manifest_hrefs.contains(n) {
            push(
                "warning",
                "OPF-unlisted-file",
                format!("`{n}` is in the package but not in the manifest"),
            );
        }
    }
    for (_, l, h) in &epub.toc {
        if !names.iter().any(|n| n == h) {
            push("error", "NAV-broken-link", format!("TOC entry `{l}` points to missing `{h}`"));
        }
    }
    if epub.cover_href.is_none() {
        push("info", "cover", "No cover image declared".into());
    }
    // Chapter parse check.
    for idref in &epub.spine {
        if let Some(item) = epub.manifest.iter().find(|m| &m.0 == idref) {
            if let Some(b) = read_entry(&mut z, &item.1) {
                let mut r = Reader::from_reader(b.as_slice());
                loop {
                    match r.read_event() {
                        Ok(Event::Eof) => break,
                        Err(e) => {
                            push(
                                "warning",
                                "XHTML-wellformed",
                                format!("`{}` is not well-formed XML: {e}", item.1),
                            );
                            break;
                        },
                        _ => {},
                    }
                }
            }
        }
    }
    let errors = issues.iter().filter(|i| i["severity"] == "error").count();
    Ok(json!({
        "valid": errors == 0,
        "version": epub.version,
        "errors": errors,
        "warnings": issues.iter().filter(|i| i["severity"] == "warning").count(),
        "issues": issues,
        "manifest_items": epub.manifest.len(),
        "spine_items": epub.spine.len(),
        "note": "Structural checks (OCF container, OPF metadata/manifest/spine, navigation, \
                 well-formedness); not a full epubcheck schema validation"
    }))
}

/// Rewrites dc:* metadata in the OPF and returns a new EPUB.
pub fn set_metadata(bytes: &[u8], values: &Map<String, Value>) -> EngineResult<(Vec<u8>, Value)> {
    let mut z = open(bytes)?;
    let epub = parse_epub(&mut z)?;
    if values.is_empty() {
        return Err(EngineError::empty("values"));
    }
    let opf = String::from_utf8(read_entry(&mut z, &epub.opf_path).unwrap_or_default())
        .map_err(|_| EngineError::malformed("OPF is not UTF-8"))?;
    let mut out = opf.clone();
    let mut changed = Map::new();
    for (k, v) in values {
        let tag = match k.as_str() {
            "author" => "creator",
            other => other,
        };
        let val = crate::ooxml_value_text(v);
        let esc = val.replace('&', "&amp;").replace('<', "&lt;");
        // Replace first <dc:tag ...>…</dc:tag> or insert before </metadata>.
        let open_pat = format!("<dc:{tag}");
        if let Some(s) = out.find(&open_pat) {
            if let Some(gt) = out[s..].find('>') {
                let content_start = s + gt + 1;
                if let Some(e) = out[content_start..].find(&format!("</dc:{tag}>")) {
                    out.replace_range(content_start..content_start + e, &esc);
                    changed.insert(k.clone(), json!("updated"));
                    continue;
                }
            }
        }
        if let Some(p) = out.find("</metadata>") {
            out.insert_str(p, &format!("<dc:{tag}>{esc}</dc:{tag}>\n"));
            changed.insert(k.clone(), json!("added"));
        } else {
            return Err(EngineError::malformed("OPF has no <metadata> element"));
        }
    }
    if !out.contains("xmlns:dc=") {
        out =
            out.replacen("<metadata", "<metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\"", 1);
    }
    let names: Vec<String> = z.file_names().map(str::to_string).collect();
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for name in names {
        let data = if name == epub.opf_path {
            out.as_bytes().to_vec()
        } else {
            read_entry(&mut z, &name).unwrap_or_default()
        };
        let method = if name == "mimetype" {
            zip::CompressionMethod::Stored
        } else {
            zip::CompressionMethod::Deflated
        };
        w.start_file(&name, SimpleFileOptions::default().compression_method(method))
            .map_err(|e| EngineError::internal(e.to_string()))?;
        w.write_all(&data).map_err(|e| EngineError::internal(e.to_string()))?;
    }
    Ok((
        w.finish()
            .map_err(|e| EngineError::internal(e.to_string()))?
            .into_inner(),
        json!(changed),
    ))
}

// ------------------------------------------------------------------- MOBI --

/// Reads PalmDOC/MOBI headers and EXTH metadata (title, author, publisher, ISBN, …).
pub fn mobi_metadata(bytes: &[u8]) -> EngineResult<Value> {
    engine_shared::error::require_bytes(bytes, "file")?;
    if bytes.len() < 78 + 8 {
        return Err(EngineError::malformed("File is too small to be a MOBI/PRC"));
    }
    let ident = &bytes[60..68];
    let (kind, creator) = match ident {
        b"BOOKMOBI" => ("mobi", "MOBI"),
        b"TEXtREAd" => ("palmdoc", "PalmDOC"),
        _ => {
            return Err(EngineError::unsupported_format(format!(
                "Not a MOBI/PalmDOC file (type/creator `{}`)",
                String::from_utf8_lossy(ident)
            )))
        },
    };
    let name = String::from_utf8_lossy(&bytes[..32])
        .trim_end_matches('\0')
        .to_string();
    let be32 = |o: usize| -> u32 {
        if o + 4 <= bytes.len() {
            u32::from_be_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]])
        } else {
            0
        }
    };
    let be16 = |o: usize| -> u16 {
        if o + 2 <= bytes.len() {
            u16::from_be_bytes([bytes[o], bytes[o + 1]])
        } else {
            0
        }
    };
    let num_records = be16(76) as usize;
    let rec0 = be32(78) as usize;
    if rec0 + 16 > bytes.len() {
        return Err(EngineError::malformed("Record 0 offset is outside the file"));
    }
    let compression = match be16(rec0) {
        1 => "none",
        2 => "PalmDOC",
        17480 => "HUFF/CDIC",
        other => {
            return Ok(json!({
                "format": kind,
                "name": name,
                "compression_code": other,
                "note": "Unknown compression"
            }))
        },
    };
    let text_length = be32(rec0 + 4);
    let text_records = be16(rec0 + 8);
    let mut out = json!({
        "format": kind,
        "creator": creator,
        "name": name,
        "records": num_records,
        "compression": compression,
        "text_length": text_length,
        "text_records": text_records,
        "file_bytes": bytes.len()
    });
    if kind == "mobi" && &bytes[rec0 + 16..rec0 + 20] == b"MOBI" {
        let header_len = be32(rec0 + 20) as usize;
        let mobi_type = be32(rec0 + 24);
        let encoding = match be32(rec0 + 28) {
            1252 => "windows-1252",
            65001 => "utf-8",
            other => return Ok(json!({ "format": kind, "name": name, "encoding_code": other })),
        };
        let version = be32(rec0 + 36);
        let locale = be32(rec0 + 92);
        let full_name_off = be32(rec0 + 84) as usize;
        let full_name_len = be32(rec0 + 88) as usize;
        let full_name = if rec0 + full_name_off + full_name_len <= bytes.len() {
            String::from_utf8_lossy(
                &bytes[rec0 + full_name_off..rec0 + full_name_off + full_name_len],
            )
            .to_string()
        } else {
            String::new()
        };
        let exth_flags = be32(rec0 + 128);
        let mut exth = Map::new();
        if exth_flags & 0x40 != 0 {
            let exth_start = rec0 + 16 + header_len;
            if exth_start + 12 <= bytes.len() && &bytes[exth_start..exth_start + 4] == b"EXTH" {
                let count = be32(exth_start + 8) as usize;
                let mut p = exth_start + 12;
                for _ in 0..count.min(500) {
                    if p + 8 > bytes.len() {
                        break;
                    }
                    let rtype = be32(p);
                    let rlen = be32(p + 4) as usize;
                    if rlen < 8 || p + rlen > bytes.len() {
                        break;
                    }
                    let data = &bytes[p + 8..p + rlen];
                    let key = match rtype {
                        100 => "author",
                        101 => "publisher",
                        103 => "description",
                        104 => "isbn",
                        105 => "subject",
                        106 => "publish_date",
                        108 => "contributor",
                        109 => "rights",
                        112 => "source",
                        113 => "asin",
                        114 => "version_number",
                        118 => "retail_price",
                        119 => "retail_price_currency",
                        201 => "cover_offset",
                        202 => "thumb_offset",
                        501 => "cde_type",
                        503 => "updated_title",
                        504 => "asin_2",
                        524 => "language",
                        525 => "alignment",
                        535 => "creator_build",
                        _ => "",
                    };
                    if key.is_empty() {
                        p += rlen;
                        continue;
                    }
                    let val = if matches!(rtype, 201 | 202 | 114 | 125 | 131) && data.len() == 4 {
                        json!(u32::from_be_bytes([data[0], data[1], data[2], data[3]]))
                    } else {
                        json!(String::from_utf8_lossy(data).trim_end_matches('\0').to_string())
                    };
                    match exth.get_mut(key) {
                        Some(Value::Array(a)) => a.push(val),
                        Some(existing) => {
                            let prev = existing.take();
                            *existing = json!([prev, val]);
                        },
                        None => {
                            exth.insert(key.to_string(), val);
                        },
                    }
                    p += rlen;
                }
            }
        }
        out["mobi"] = json!({
            "header_length": header_len,
            "type": match mobi_type { 2 => "Mobipocket Book", 3 => "PalmDoc Book", 232 => "mobipocket (KF8)", 248 => "KF8", 257 => "News", 258 => "News feed", 259 => "News magazine", 513 => "PICS", 514 => "WORD", 515 => "XLS", 516 => "PPT", 517 => "TEXT", 518 => "HTML", _ => "unknown" },
            "type_code": mobi_type,
            "encoding": encoding,
            "version": version,
            "locale_code": locale,
            "full_name": full_name,
            "kf8": version >= 8 || mobi_type == 248,
            "drm": be32(rec0 + 168) != 0xFFFF_FFFF && be32(rec0 + 168) != 0
        });
        out["metadata"] = Value::Object(exth);
        out["title"] = json!(if full_name.is_empty() { name.clone() } else { full_name });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn sample_epub() -> Vec<u8> {
        sample_epub_with("c2")
    }

    pub fn sample_epub_with(second: &str) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let stored =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let deflated =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        w.start_file("mimetype", stored).unwrap();
        w.write_all(b"application/epub+zip").unwrap();
        let files = [
            ("META-INF/container.xml", r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#.to_string()),
            ("OEBPS/content.opf", r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">urn:uuid:1</dc:identifier><dc:title>Test Book</dc:title><dc:creator>Ada</dc:creator><dc:language>en</dc:language><meta name="cover" content="cover"/></metadata><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="c1" href="ch1.xhtml" media-type="application/xhtml+xml"/><item id="c2" href="text/ch2.xhtml" media-type="application/xhtml+xml"/><item id="cover" href="cover.png" media-type="image/png"/></manifest><spine><itemref idref="c1"/><itemref idref="SECOND"/></spine></package>"#.replace("SECOND", second)),
            ("OEBPS/nav.xhtml", r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><nav epub:type="toc" xmlns:epub="http://www.idpf.org/2007/ops"><ol><li><a href="ch1.xhtml">One</a></li><li><a href="text/ch2.xhtml">Two</a></li></ol></nav></body></html>"#.to_string()),
            ("OEBPS/ch1.xhtml", "<html><head><title>x</title></head><body><h1>Chapter \
                                 One</h1><p>Hello <b>bold</b> \
                                 world.</p><ul><li>a</li><li>b</li></ul></body></html>".to_string()),
            ("OEBPS/text/ch2.xhtml", "<html><body><h1>Chapter Two</h1><p>Second chapter text \
                                      here.</p></body></html>".to_string()),
        ];
        for (n, c) in files {
            w.start_file(n, deflated).unwrap();
            w.write_all(c.as_bytes()).unwrap();
        }
        w.start_file("OEBPS/cover.png", deflated).unwrap();
        w.write_all(&[0x89, b'P', b'N', b'G', 0, 0, 0, 0]).unwrap();
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn epub_roundtrip() {
        let bytes = sample_epub();
        let i = inspect(&bytes).unwrap();
        assert_eq!(i["metadata"]["title"], "Test Book");
        assert_eq!(i["chapter_count"], 2);
        assert_eq!(i["chapters"][1]["title"], "Two");
        assert_eq!(i["toc"][1]["href"], "OEBPS/text/ch2.xhtml");
        let c = chapter(&bytes, 0, "").unwrap();
        assert!(
            c["markdown"]
                .as_str()
                .unwrap()
                .starts_with("# Chapter One\n\nHello **bold** world."),
            "{}",
            c["markdown"]
        );
        assert!(c["markdown"].as_str().unwrap().contains("- a"));
        let (html, merged, _) = to_html(&bytes, true).unwrap();
        assert!(html.contains("<nav class=\"toc\">") && html.contains("Second chapter"));
        assert_eq!(
            merged
                .blocks
                .iter()
                .filter(|b| matches!(b, Block::Heading(1, _)))
                .count(),
            2
        );
        assert_eq!(cover(&bytes).unwrap()["mime"], "image/png");
        let v = validate(&bytes).unwrap();
        assert_eq!(v["valid"], true, "{v}");
        let mut vals = Map::new();
        vals.insert("title".into(), json!("New <Title>"));
        vals.insert("publisher".into(), json!("Pub"));
        let (out, changed) = set_metadata(&bytes, &vals).unwrap();
        assert_eq!(changed["title"], "updated");
        assert_eq!(changed["publisher"], "added");
        let i2 = inspect(&out).unwrap();
        assert_eq!(i2["metadata"]["title"], "New <Title>");
        assert_eq!(i2["metadata"]["publisher"], "Pub");
        assert!(validate(&out).unwrap()["valid"] == true);
        assert!(chapter(&bytes, 9, "").is_err());
        // Broken: missing spine target.
        let bad = sample_epub_with("zz");
        let v = validate(&bad).unwrap();
        assert_eq!(v["valid"], false);
        assert!(v["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["code"] == "OPF-spine"));
    }

    #[test]
    fn mobi_header() {
        let mut b = vec![0u8; 78];
        b[..4].copy_from_slice(b"Book");
        b[60..68].copy_from_slice(b"BOOKMOBI");
        b[76..78].copy_from_slice(&1u16.to_be_bytes());
        b.extend_from_slice(&(86u32).to_be_bytes()); // record 0 offset
        b.extend_from_slice(&[0, 0, 0, 0]);
        // record 0: palmdoc header (16) + MOBI header
        let mut rec = Vec::new();
        rec.extend_from_slice(&2u16.to_be_bytes());
        rec.extend_from_slice(&[0, 0]);
        rec.extend_from_slice(&1234u32.to_be_bytes());
        rec.extend_from_slice(&3u16.to_be_bytes());
        rec.extend_from_slice(&[0u8; 6]);
        let mut mobi = vec![0u8; 232];
        mobi[..4].copy_from_slice(b"MOBI");
        mobi[4..8].copy_from_slice(&232u32.to_be_bytes());
        mobi[8..12].copy_from_slice(&2u32.to_be_bytes());
        mobi[12..16].copy_from_slice(&65001u32.to_be_bytes());
        mobi[20..24].copy_from_slice(&6u32.to_be_bytes());
        mobi[112..116].copy_from_slice(&0x40u32.to_be_bytes());
        let exth_author = b"Ada";
        let mut exth = Vec::new();
        exth.extend_from_slice(b"EXTH");
        exth.extend_from_slice(&(12 + 8 + exth_author.len() as u32).to_be_bytes());
        exth.extend_from_slice(&1u32.to_be_bytes());
        exth.extend_from_slice(&100u32.to_be_bytes());
        exth.extend_from_slice(&(8 + exth_author.len() as u32).to_be_bytes());
        exth.extend_from_slice(exth_author);
        let full_name = b"My Full Title";
        let name_off = 16 + 232 + exth.len();
        mobi[68..72].copy_from_slice(&(name_off as u32).to_be_bytes());
        mobi[72..76].copy_from_slice(&(full_name.len() as u32).to_be_bytes());
        rec.extend_from_slice(&mobi);
        rec.extend_from_slice(&exth);
        rec.extend_from_slice(full_name);
        b.extend_from_slice(&rec);
        let m = mobi_metadata(&b).unwrap();
        assert_eq!(m["format"], "mobi");
        assert_eq!(m["compression"], "PalmDOC");
        assert_eq!(m["mobi"]["encoding"], "utf-8");
        assert_eq!(m["metadata"]["author"], "Ada");
        assert_eq!(m["title"], "My Full Title");
        assert!(mobi_metadata(
            b"not a mobi file at all, definitely not one of those ........................"
        )
        .is_err());
    }
}
