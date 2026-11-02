//! DOCX / PPTX / ODT readers, DOCX template filling and DOCX writer.

use crate::model::{Block, Doc, Inline};
use engine_shared::{EngineError, EngineResult};
use quick_xml::events::Event;
use quick_xml::Reader;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use zip::write::SimpleFileOptions;

/// In-memory ZIP archive used for OOXML / ODF containers.
pub type Archive = zip::ZipArchive<Cursor<Vec<u8>>>;

/// Opens a ZIP container, reporting corrupt or encrypted archives.
pub fn open(bytes: &[u8]) -> EngineResult<Archive> {
    engine_shared::error::require_bytes(bytes, "file")?;
    engine_shared::limits::check_size(bytes.len(), engine_shared::limits::MAX_FILE_BYTES, "file")?;
    if bytes.len() >= 8 && bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]) {
        return Err(EngineError::unsupported_format(
            "This is a legacy binary Office file (.doc/.ppt/.xls); save it as .docx/.pptx first",
        ));
    }
    if !bytes.starts_with(b"PK") {
        return Err(EngineError::unsupported_format(
            "Not an Office Open XML / OpenDocument package (expected a ZIP container)",
        ));
    }
    zip::ZipArchive::new(Cursor::new(bytes.to_vec()))
        .map_err(|e| EngineError::malformed(format!("Corrupt package: {e}")))
}

/// Reads one entry by name (`None` when missing).
pub fn read_entry(z: &mut Archive, name: &str) -> Option<Vec<u8>> {
    let mut f = z.by_name(name).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// Office container kind detected from the archive contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Word document (`.docx`).
    Docx,
    /// PowerPoint presentation (`.pptx`).
    Pptx,
    /// Excel workbook (`.xlsx`; handled by the spreadsheet engine).
    Xlsx,
    /// OpenDocument text (`.odt`).
    Odt,
    /// OpenDocument presentation (`.odp`).
    Odp,
    /// OpenDocument spreadsheet (`.ods`).
    Ods,
    /// Not an Office container.
    Unknown,
}

/// Detects the container kind from `[Content_Types].xml` / `mimetype`.
pub fn detect(z: &mut Archive) -> Kind {
    let names: Vec<String> = z.file_names().map(str::to_string).collect();
    if names.iter().any(|n| n == "word/document.xml") {
        Kind::Docx
    } else if names.iter().any(|n| n == "ppt/presentation.xml") {
        Kind::Pptx
    } else if names.iter().any(|n| n == "xl/workbook.xml") {
        Kind::Xlsx
    } else if names.iter().any(|n| n == "mimetype") {
        match read_entry(z, "mimetype")
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .unwrap_or_default()
            .as_str()
        {
            "application/vnd.oasis.opendocument.text" => Kind::Odt,
            "application/vnd.oasis.opendocument.presentation" => Kind::Odp,
            "application/vnd.oasis.opendocument.spreadsheet" => Kind::Ods,
            _ => Kind::Unknown,
        }
    } else if names.iter().any(|n| n == "content.xml") {
        Kind::Odt
    } else {
        Kind::Unknown
    }
}

fn attr(e: &quick_xml::events::BytesStart, name: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.local_name().as_ref() == name)
        .map(|a| String::from_utf8_lossy(&a.value).to_string())
}

fn local(e: &quick_xml::events::BytesStart) -> String {
    String::from_utf8_lossy(e.local_name().as_ref()).to_string()
}

/// Parses `word/_rels/document.xml.rels` into rId → Target.
fn relationships(z: &mut Archive, path: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Some(bytes) = read_entry(z, path) else {
        return out;
    };
    let mut r = Reader::from_reader(bytes.as_slice());
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local(&e) == "Relationship" => {
                if let (Some(id), Some(target)) = (attr(&e, b"Id"), attr(&e, b"Target")) {
                    out.insert(id, target);
                }
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {},
        }
    }
    out
}

/// Reads core (Dublin Core) and app properties.
pub fn core_metadata(z: &mut Archive) -> serde_json::Map<String, Value> {
    let mut meta = serde_json::Map::new();
    for path in ["docProps/core.xml", "docProps/app.xml", "meta.xml"] {
        let Some(bytes) = read_entry(z, path) else {
            continue;
        };
        let mut r = Reader::from_reader(bytes.as_slice());
        let mut current: Option<String> = None;
        loop {
            match r.read_event() {
                Ok(Event::Start(e)) => {
                    let n = local(&e);
                    if path == "meta.xml" {
                        if let (true, Some(name)) = (n == "user-defined", attr(&e, b"name")) {
                            current = Some(format!("user:{name}"));
                            continue;
                        }
                    }
                    current = Some(n);
                },
                Ok(Event::Text(t)) => {
                    if let Some(k) = &current {
                        let v = t.unescape().map(|s| s.to_string()).unwrap_or_default();
                        if !v.trim().is_empty()
                            && !matches!(
                                k.as_str(),
                                "coreProperties" | "Properties" | "document-meta" | "meta"
                            )
                        {
                            let key = match k.as_str() {
                                "creator" | "initial-creator" => "author".to_string(),
                                "lastModifiedBy" => "last_modified_by".into(),
                                "created" | "creation-date" => "created".into(),
                                "modified" | "date" => "modified".into(),
                                "Words" | "word-count" => "words".into(),
                                "Pages" | "page-count" => "pages".into(),
                                "Characters" | "character-count" => "characters".into(),
                                "Paragraphs" | "paragraph-count" => "paragraphs".into(),
                                "Application" | "generator" => "application".into(),
                                "AppVersion" => "app_version".into(),
                                "TotalTime" | "editing-duration" => "editing_time".into(),
                                "Slides" => "slides".into(),
                                "Company" => "company".into(),
                                "Template" => "template".into(),
                                other => other.to_lowercase(),
                            };
                            meta.entry(key).or_insert(Value::String(v.trim().to_string()));
                        }
                    }
                },
                Ok(Event::End(_)) => current = None,
                Ok(Event::Eof) | Err(_) => break,
                _ => {},
            }
        }
    }
    meta
}

// ------------------------------------------------------------------- DOCX --

struct ParaState {
    inlines: Vec<Inline>,
    style: String,
    num_level: Option<u8>,
    num_id: Option<String>,
}

/// Parses `word/document.xml` into a [`Doc`] (headings, lists, tables, runs, links, breaks).
pub fn read_docx(z: &mut Archive) -> EngineResult<Doc> {
    let bytes = read_entry(z, "word/document.xml")
        .ok_or_else(|| EngineError::malformed("word/document.xml is missing"))?;
    let rels = relationships(z, "word/_rels/document.xml.rels");
    let numbering = read_numbering(z);
    let mut doc = Doc {
        metadata: core_metadata(z),
        ..Default::default()
    };
    doc.images = z.file_names().filter(|n| n.starts_with("word/media/")).count();
    let mut r = Reader::from_reader(bytes.as_slice());
    let mut para: Option<ParaState> = None;
    let mut run: Option<Inline> = None;
    let mut in_rpr = false;
    let mut in_ppr = false;
    let mut link: Option<String> = None;
    let mut table: Option<Vec<Vec<String>>> = None;
    let mut row: Vec<String> = Vec::new();
    let mut cell: Option<String> = None;
    let mut in_deleted = false;
    let mut depth_tbl = 0;
    let flush_para = |doc: &mut Doc, p: ParaState| {
        let mut inlines = p.inlines;
        // Merge adjacent runs with identical formatting.
        let mut merged: Vec<Inline> = Vec::new();
        for i in inlines.drain(..) {
            if let Some(last) = merged.last_mut() {
                if last.bold == i.bold
                    && last.italic == i.italic
                    && last.underline == i.underline
                    && last.strike == i.strike
                    && last.code == i.code
                    && last.link == i.link
                {
                    last.text.push_str(&i.text);
                    continue;
                }
            }
            merged.push(i);
        }
        let text_empty = merged.iter().all(|i| i.text.trim().is_empty());
        let style = p.style.to_ascii_lowercase().replace([' ', '-', '_'], "");
        if let Some(lvl) = p.num_level {
            if !text_empty {
                let ordered = p
                    .num_id
                    .as_ref()
                    .and_then(|id| numbering.get(id))
                    .copied()
                    .unwrap_or(false);
                doc.blocks.push(Block::ListItem(lvl, ordered, merged));
            }
            return;
        }
        if let Some(l) = style
            .strip_prefix("heading")
            .or_else(|| style.strip_prefix("berschrift"))
            .and_then(|l| l.parse::<u8>().ok())
        {
            if !text_empty {
                doc.blocks.push(Block::Heading(l.clamp(1, 6), merged));
            }
        } else if style == "title" {
            if !text_empty {
                doc.blocks.push(Block::Heading(1, merged));
            }
        } else if style == "quote" || style == "intensequote" || style == "blockquote" {
            doc.blocks.push(Block::Quote(merged));
        } else if style.contains("code") || style == "htmlpreformatted" {
            doc.blocks.push(Block::Code(crate::model::plain(&merged)));
        } else if !text_empty {
            doc.blocks.push(Block::Paragraph(merged));
        }
    };
    loop {
        let ev = r
            .read_event()
            .map_err(|e| EngineError::malformed(format!("document.xml: {e}")))?;
        match ev {
            Event::Start(e) => {
                let n = local(&e);
                match n.as_str() {
                    "tbl" => {
                        depth_tbl += 1;
                        if depth_tbl == 1 {
                            table = Some(Vec::new());
                        }
                    },
                    "tr" if depth_tbl == 1 => row.clear(),
                    "tc" if depth_tbl == 1 => cell = Some(String::new()),
                    "p" => {
                        para = Some(ParaState {
                            inlines: Vec::new(),
                            style: String::new(),
                            num_level: None,
                            num_id: None,
                        })
                    },
                    "r" => run = Some(Inline { link: link.clone(), ..Default::default() }),
                    "rPr" => in_rpr = true,
                    "pPr" => in_ppr = true,
                    "hyperlink" => {
                        link = attr(&e, b"id")
                            .and_then(|id| rels.get(&id).cloned())
                            .or_else(|| attr(&e, b"anchor").map(|a| format!("#{a}")))
                    },
                    "del" => in_deleted = true,
                    "b" | "i" | "u" | "strike" | "dstrike" if in_rpr => {
                        apply_rpr(&mut run, &n, attr(&e, b"val"))
                    },
                    "pStyle" if in_ppr => {
                        if let (Some(p), Some(v)) = (para.as_mut(), attr(&e, b"val")) {
                            p.style = v;
                        }
                    },
                    "ilvl" if in_ppr => {
                        if let (Some(p), Some(v)) = (para.as_mut(), attr(&e, b"val")) {
                            p.num_level = v.parse().ok();
                        }
                    },
                    "numId" if in_ppr => {
                        if let (Some(p), Some(v)) = (para.as_mut(), attr(&e, b"val")) {
                            p.num_id = Some(v);
                            p.num_level.get_or_insert(0);
                        }
                    },
                    "rStyle" if in_rpr => {
                        if let (Some(rn), Some(v)) = (run.as_mut(), attr(&e, b"val")) {
                            if v.to_ascii_lowercase().contains("code") {
                                rn.code = true;
                            }
                        }
                    },
                    "rFonts" if in_rpr => {
                        if let (Some(rn), Some(v)) = (run.as_mut(), attr(&e, b"ascii")) {
                            if matches!(
                                v.as_str(),
                                "Consolas" | "Courier New" | "Courier" | "Menlo" | "Monaco"
                            ) {
                                rn.code = true;
                            }
                        }
                    },
                    _ => {},
                }
            },
            Event::Empty(e) => {
                let n = local(&e);
                match n.as_str() {
                    "b" | "i" | "u" | "strike" | "dstrike" if in_rpr => {
                        apply_rpr(&mut run, &n, attr(&e, b"val"))
                    },
                    "pStyle" if in_ppr => {
                        if let (Some(p), Some(v)) = (para.as_mut(), attr(&e, b"val")) {
                            p.style = v;
                        }
                    },
                    "ilvl" if in_ppr => {
                        if let (Some(p), Some(v)) = (para.as_mut(), attr(&e, b"val")) {
                            p.num_level = v.parse().ok();
                        }
                    },
                    "numId" if in_ppr => {
                        if let (Some(p), Some(v)) = (para.as_mut(), attr(&e, b"val")) {
                            p.num_id = Some(v);
                            p.num_level.get_or_insert(0);
                        }
                    },
                    "rFonts" if in_rpr => {
                        if let (Some(rn), Some(v)) = (run.as_mut(), attr(&e, b"ascii")) {
                            if matches!(
                                v.as_str(),
                                "Consolas" | "Courier New" | "Courier" | "Menlo" | "Monaco"
                            ) {
                                rn.code = true;
                            }
                        }
                    },
                    "rStyle" if in_rpr => {
                        if let (Some(rn), Some(v)) = (run.as_mut(), attr(&e, b"val")) {
                            if v.to_ascii_lowercase().contains("code") {
                                rn.code = true;
                            }
                        }
                    },
                    "tab" => push_text(&mut run, &mut cell, "\t"),
                    "br" => {
                        if attr(&e, b"type").as_deref() == Some("page") {
                            if let Some(p) = para.take() {
                                flush_para(&mut doc, p);
                            }
                            doc.blocks.push(Block::PageBreak);
                            para = Some(ParaState {
                                inlines: Vec::new(),
                                style: String::new(),
                                num_level: None,
                                num_id: None,
                            });
                        } else {
                            push_text(&mut run, &mut cell, "\n");
                        }
                    },
                    "drawing" | "pict" => push_text(&mut run, &mut cell, ""),
                    _ => {},
                }
            },
            Event::Text(t) => {
                if in_deleted {
                    continue;
                }
                let s = t.unescape().map(|c| c.to_string()).unwrap_or_default();
                push_text(&mut run, &mut cell, &s);
            },
            Event::End(e) => {
                let n = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                match n.as_str() {
                    "rPr" => in_rpr = false,
                    "pPr" => in_ppr = false,
                    "hyperlink" => link = None,
                    "del" => in_deleted = false,
                    "r" => {
                        if let (Some(rn), Some(p)) = (run.take(), para.as_mut()) {
                            if !rn.text.is_empty() {
                                p.inlines.push(rn);
                            }
                        }
                    },
                    "p" => {
                        if let Some(p) = para.take() {
                            if let Some(c) = cell.as_mut() {
                                let t = crate::model::plain(&p.inlines);
                                if !c.is_empty() && !t.is_empty() {
                                    c.push('\n');
                                }
                                c.push_str(&t);
                            } else {
                                flush_para(&mut doc, p);
                            }
                        }
                    },
                    "tc" if depth_tbl == 1 => row.push(cell.take().unwrap_or_default()),
                    "tr" if depth_tbl == 1 => {
                        if let Some(t) = table.as_mut() {
                            t.push(std::mem::take(&mut row));
                        }
                    },
                    "tbl" => {
                        depth_tbl -= 1;
                        if depth_tbl == 0 {
                            if let Some(t) = table.take() {
                                if !t.is_empty() {
                                    doc.blocks.push(Block::Table(t));
                                }
                            }
                        }
                    },
                    _ => {},
                }
            },
            Event::Eof => break,
            _ => {},
        }
    }
    if doc.blocks.is_empty() {
        doc.warnings.push("Document body contains no text".into());
    }
    Ok(doc)
}

fn apply_rpr(run: &mut Option<Inline>, tag: &str, val: Option<String>) {
    let Some(r) = run.as_mut() else { return };
    let on = !matches!(val.as_deref(), Some("0") | Some("false") | Some("none"));
    match tag {
        "b" => r.bold = on,
        "i" => r.italic = on,
        "u" => r.underline = on,
        "strike" | "dstrike" => r.strike = on,
        _ => {},
    }
}

fn push_text(run: &mut Option<Inline>, cell: &mut Option<String>, s: &str) {
    if let Some(r) = run.as_mut() {
        r.text.push_str(s);
    } else if let Some(c) = cell.as_mut() {
        c.push_str(s);
    }
}

/// numId → ordered? (from numbering.xml abstractNum level 0 numFmt).
fn read_numbering(z: &mut Archive) -> HashMap<String, bool> {
    let mut out = HashMap::new();
    let Some(bytes) = read_entry(z, "word/numbering.xml") else {
        return out;
    };
    let mut r = Reader::from_reader(bytes.as_slice());
    let mut abstract_fmt: HashMap<String, bool> = HashMap::new();
    let mut current_abstract: Option<String> = None;
    let mut level0 = false;
    let mut num_to_abstract: Vec<(String, String)> = Vec::new();
    let mut current_num: Option<String> = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match local(&e).as_str() {
                "abstractNum" => current_abstract = attr(&e, b"abstractNumId"),
                "lvl" => level0 = attr(&e, b"ilvl").as_deref() == Some("0"),
                "numFmt" if level0 => {
                    if let (Some(a), Some(v)) = (&current_abstract, attr(&e, b"val")) {
                        abstract_fmt.insert(a.clone(), v != "bullet" && v != "none");
                    }
                },
                "num" => current_num = attr(&e, b"numId"),
                "abstractNumId" => {
                    if let (Some(n), Some(v)) = (&current_num, attr(&e, b"val")) {
                        num_to_abstract.push((n.clone(), v));
                    }
                },
                _ => {},
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {},
        }
    }
    for (n, a) in num_to_abstract {
        out.insert(n, abstract_fmt.get(&a).copied().unwrap_or(false));
    }
    out
}

// ------------------------------------------------------------------- PPTX --

/// Parses every slide of a PPTX into a [`Doc`] plus per-slide JSON.
pub fn read_pptx(z: &mut Archive) -> EngineResult<(Doc, Vec<Value>)> {
    let mut names: Vec<String> = z
        .file_names()
        .filter(|n| {
            n.starts_with("ppt/slides/slide") && n.ends_with(".xml") && !n.contains("_rels")
        })
        .map(str::to_string)
        .collect();
    names.sort_by_key(|n| {
        n.trim_start_matches("ppt/slides/slide")
            .trim_end_matches(".xml")
            .parse::<u32>()
            .unwrap_or(0)
    });
    if names.is_empty() {
        return Err(EngineError::malformed("Presentation has no slides"));
    }
    let mut doc = Doc {
        metadata: core_metadata(z),
        ..Default::default()
    };
    doc.images = z.file_names().filter(|n| n.starts_with("ppt/media/")).count();
    let mut slides = Vec::new();
    for (idx, name) in names.iter().enumerate() {
        let bytes = read_entry(z, name).unwrap_or_default();
        let notes_name = name.replace("ppt/slides/slide", "ppt/notesSlides/notesSlide");
        let notes = read_entry(z, &notes_name)
            .map(|b| extract_a_text(&b).join("\n"))
            .unwrap_or_default();
        let paragraphs = extract_a_text(&bytes);
        let title = paragraphs.first().cloned().unwrap_or_default();
        doc.blocks.push(Block::Heading(
            2,
            vec![Inline {
                text: if title.is_empty() {
                    format!("Slide {}", idx + 1)
                } else {
                    title.clone()
                },
                ..Default::default()
            }],
        ));
        for p in paragraphs.iter().skip(1) {
            doc.blocks.push(Block::ListItem(
                0,
                false,
                vec![Inline { text: p.clone(), ..Default::default() }],
            ));
        }
        slides.push(json!({
            "number": idx + 1,
            "title": title,
            "text": paragraphs,
            "notes": notes,
            "word_count": paragraphs.iter().map(|p| p.split_whitespace().count()).sum::<usize>()
        }));
        doc.blocks.push(Block::PageBreak);
    }
    doc.blocks.pop();
    Ok((doc, slides))
}

/// Collects `a:p` paragraph texts (joining `a:t` runs) from a DrawingML part.
fn extract_a_text(bytes: &[u8]) -> Vec<String> {
    let mut r = Reader::from_reader(bytes);
    let mut out = Vec::new();
    let mut cur: Option<String> = None;
    let mut in_t = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(&e).as_str() {
                "p" => cur = Some(String::new()),
                "t" => in_t = true,
                _ => {},
            },
            Ok(Event::Empty(e)) if local(&e) == "br" => {
                if let Some(c) = cur.as_mut() {
                    c.push('\n');
                }
            },
            Ok(Event::Text(t)) if in_t => {
                if let Some(c) = cur.as_mut() {
                    c.push_str(&t.unescape().map(|s| s.to_string()).unwrap_or_default());
                }
            },
            Ok(Event::End(e)) => match String::from_utf8_lossy(e.local_name().as_ref()).as_ref() {
                "t" => in_t = false,
                "p" => {
                    if let Some(c) = cur.take() {
                        if !c.trim().is_empty() {
                            out.push(c.trim().to_string());
                        }
                    }
                },
                _ => {},
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {},
        }
    }
    out
}

// -------------------------------------------------------------------- ODT --

/// Parses `content.xml` of an ODT into a [`Doc`].
pub fn read_odt(z: &mut Archive) -> EngineResult<Doc> {
    let bytes = read_entry(z, "content.xml")
        .ok_or_else(|| EngineError::malformed("content.xml is missing"))?;
    let mut doc = Doc {
        metadata: core_metadata(z),
        ..Default::default()
    };
    doc.images = z.file_names().filter(|n| n.starts_with("Pictures/")).count();
    // Style name → (bold, italic) from automatic styles.
    let mut styles: HashMap<String, (bool, bool, bool)> = HashMap::new();
    {
        let mut r = Reader::from_reader(bytes.as_slice());
        let mut cur: Option<String> = None;
        loop {
            match r.read_event() {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match local(&e).as_str() {
                    "style" => cur = attr(&e, b"name"),
                    "text-properties" => {
                        if let Some(c) = &cur {
                            let bold =
                                attr(&e, b"font-weight").map(|v| v == "bold").unwrap_or(false);
                            let italic =
                                attr(&e, b"font-style").map(|v| v == "italic").unwrap_or(false);
                            let underline = attr(&e, b"text-underline-style")
                                .map(|v| v != "none")
                                .unwrap_or(false);
                            styles.insert(c.clone(), (bold, italic, underline));
                        }
                    },
                    _ => {},
                },
                Ok(Event::End(e)) if e.local_name().as_ref() == b"style" => cur = None,
                Ok(Event::Eof) | Err(_) => break,
                _ => {},
            }
        }
    }
    let mut r = Reader::from_reader(bytes.as_slice());
    let mut para: Option<(Vec<Inline>, Option<u8>)> = None;
    let mut span_stack: Vec<(bool, bool, bool)> = Vec::new();
    let mut list_depth: u8 = 0;
    let mut list_ordered: Vec<bool> = Vec::new();
    let mut link: Option<String> = None;
    let mut table: Option<Vec<Vec<String>>> = None;
    let mut row: Vec<String> = Vec::new();
    let mut cell: Option<String> = None;
    let mut in_body = false;
    loop {
        let ev = r
            .read_event()
            .map_err(|e| EngineError::malformed(format!("content.xml: {e}")))?;
        match ev {
            Event::Start(e) => {
                let n = local(&e);
                match n.as_str() {
                    "body" => in_body = true,
                    "h" if in_body => {
                        para = Some((
                            Vec::new(),
                            attr(&e, b"outline-level").and_then(|l| l.parse().ok()).or(Some(1)),
                        ))
                    },
                    "p" if in_body => para = Some((Vec::new(), None)),
                    "span" => span_stack.push(
                        attr(&e, b"style-name")
                            .and_then(|s| styles.get(&s).copied())
                            .unwrap_or((false, false, false)),
                    ),
                    "a" => link = attr(&e, b"href"),
                    "list" => {
                        list_depth += 1;
                        list_ordered.push(
                            attr(&e, b"style-name")
                                .map(|s| {
                                    s.to_ascii_lowercase().contains("number") || s.starts_with('N')
                                })
                                .unwrap_or(false),
                        );
                    },
                    "table" if in_body => table = Some(Vec::new()),
                    "table-row" => row.clear(),
                    "table-cell" => cell = Some(String::new()),
                    _ => {},
                }
            },
            Event::Empty(e) => match local(&e).as_str() {
                "tab" => push_odt_text(&mut para, &mut cell, "\t", &span_stack, &link),
                "line-break" => push_odt_text(&mut para, &mut cell, "\n", &span_stack, &link),
                "s" => {
                    let n = attr(&e, b"c").and_then(|c| c.parse::<usize>().ok()).unwrap_or(1);
                    push_odt_text(&mut para, &mut cell, &" ".repeat(n), &span_stack, &link);
                },
                "soft-page-break" => doc.blocks.push(Block::PageBreak),
                _ => {},
            },
            Event::Text(t) => {
                if in_body {
                    let s = t.unescape().map(|c| c.to_string()).unwrap_or_default();
                    push_odt_text(&mut para, &mut cell, &s, &span_stack, &link);
                }
            },
            Event::End(e) => {
                let n = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                match n.as_str() {
                    "span" => {
                        span_stack.pop();
                    },
                    "a" => link = None,
                    "list" => {
                        list_depth = list_depth.saturating_sub(1);
                        list_ordered.pop();
                    },
                    "h" | "p" => {
                        if let Some((inl, level)) = para.take() {
                            if let Some(c) = cell.as_mut() {
                                let t = crate::model::plain(&inl);
                                if !c.is_empty() && !t.is_empty() {
                                    c.push('\n');
                                }
                                c.push_str(&t);
                            } else if inl.iter().any(|i| !i.text.trim().is_empty()) {
                                if let Some(l) = level {
                                    doc.blocks.push(Block::Heading(l.clamp(1, 6), inl));
                                } else if list_depth > 0 {
                                    doc.blocks.push(Block::ListItem(
                                        list_depth - 1,
                                        list_ordered.last().copied().unwrap_or(false),
                                        inl,
                                    ));
                                } else {
                                    doc.blocks.push(Block::Paragraph(inl));
                                }
                            }
                        }
                    },
                    "table-cell" => row.push(cell.take().unwrap_or_default()),
                    "table-row" => {
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
                    _ => {},
                }
            },
            Event::Eof => break,
            _ => {},
        }
    }
    Ok(doc)
}

fn push_odt_text(
    para: &mut Option<(Vec<Inline>, Option<u8>)>,
    cell: &mut Option<String>,
    s: &str,
    spans: &[(bool, bool, bool)],
    link: &Option<String>,
) {
    if let Some((inl, _)) = para.as_mut() {
        let (b, i, u) = spans
            .iter()
            .fold((false, false, false), |acc, s| (acc.0 || s.0, acc.1 || s.1, acc.2 || s.2));
        inl.push(Inline {
            text: s.to_string(),
            bold: b,
            italic: i,
            underline: u,
            link: link.clone(),
            ..Default::default()
        });
    } else if let Some(c) = cell.as_mut() {
        c.push_str(s);
    }
}

// ------------------------------------------------------------ DOCX writer --

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn run_xml(i: &Inline) -> String {
    let mut rpr = String::new();
    if i.bold {
        rpr.push_str("<w:b/>");
    }
    if i.italic {
        rpr.push_str("<w:i/>");
    }
    if i.underline {
        rpr.push_str("<w:u w:val=\"single\"/>");
    }
    if i.strike {
        rpr.push_str("<w:strike/>");
    }
    if i.code {
        rpr.push_str("<w:rFonts w:ascii=\"Consolas\" w:hAnsi=\"Consolas\"/>");
    }
    if i.link.is_some() {
        rpr.push_str("<w:color w:val=\"0563C1\"/><w:u w:val=\"single\"/>");
    }
    let mut out = String::new();
    let parts: Vec<&str> = i.text.split('\n').collect();
    for (k, part) in parts.iter().enumerate() {
        if k > 0 {
            out.push_str("<w:r><w:br/></w:r>");
        }
        out.push_str(&format!(
            "<w:r>{}<w:t xml:space=\"preserve\">{}</w:t></w:r>",
            if rpr.is_empty() {
                String::new()
            } else {
                format!("<w:rPr>{rpr}</w:rPr>")
            },
            xml_escape(part)
        ));
    }
    if let Some(l) = &i.link {
        out.push_str(&format!(
            "<w:r><w:rPr><w:color w:val=\"808080\"/></w:rPr><w:t xml:space=\"preserve\"> \
             ({})</w:t></w:r>",
            xml_escape(l)
        ));
    }
    out
}

fn para_xml(style: Option<&str>, num: Option<(u8, bool)>, inlines: &[Inline]) -> String {
    let mut ppr = String::new();
    if let Some(s) = style {
        ppr.push_str(&format!("<w:pStyle w:val=\"{s}\"/>"));
    }
    if let Some((lvl, ordered)) = num {
        ppr.push_str(&format!(
            "<w:numPr><w:ilvl w:val=\"{lvl}\"/><w:numId w:val=\"{}\"/></w:numPr>",
            if ordered { 2 } else { 1 }
        ));
    }
    format!(
        "<w:p>{}{}</w:p>",
        if ppr.is_empty() {
            String::new()
        } else {
            format!("<w:pPr>{ppr}</w:pPr>")
        },
        inlines.iter().map(run_xml).collect::<String>()
    )
}

/// Serialises a [`Doc`] as a minimal valid DOCX.
pub fn write_docx(doc: &Doc, title: &str) -> EngineResult<Vec<u8>> {
    if doc.blocks.is_empty() {
        return Err(EngineError::empty("document"));
    }
    let mut body = String::new();
    for b in &doc.blocks {
        match b {
            Block::Heading(l, i) => {
                body.push_str(&para_xml(Some(&format!("Heading{}", l.clamp(&1, &6))), None, i))
            },
            Block::Paragraph(i) => body.push_str(&para_xml(None, None, i)),
            Block::Quote(i) => body.push_str(&para_xml(Some("Quote"), None, i)),
            Block::ListItem(l, o, i) => {
                body.push_str(&para_xml(Some("ListParagraph"), Some((*l, *o)), i))
            },
            Block::Code(c) => {
                for line in c.lines() {
                    body.push_str(&para_xml(
                        Some("Code"),
                        None,
                        &[Inline {
                            text: line.to_string(),
                            code: true,
                            ..Default::default()
                        }],
                    ));
                }
            },
            Block::Table(rows) => {
                let cols = rows.iter().map(Vec::len).max().unwrap_or(1);
                body.push_str(
                    "<w:tbl><w:tblPr><w:tblStyle w:val=\"TableGrid\"/><w:tblW w:w=\"0\" \
                     w:type=\"auto\"/><w:tblBorders><w:top w:val=\"single\" w:sz=\"4\" \
                     w:color=\"999999\"/><w:left w:val=\"single\" w:sz=\"4\" \
                     w:color=\"999999\"/><w:bottom w:val=\"single\" w:sz=\"4\" \
                     w:color=\"999999\"/><w:right w:val=\"single\" w:sz=\"4\" \
                     w:color=\"999999\"/><w:insideH w:val=\"single\" w:sz=\"4\" \
                     w:color=\"999999\"/><w:insideV w:val=\"single\" w:sz=\"4\" \
                     w:color=\"999999\"/></w:tblBorders></w:tblPr><w:tblGrid>",
                );
                for _ in 0..cols {
                    body.push_str(&format!("<w:gridCol w:w=\"{}\"/>", 9000 / cols.max(1)));
                }
                body.push_str("</w:tblGrid>");
                for (ri, r) in rows.iter().enumerate() {
                    body.push_str("<w:tr>");
                    for c in 0..cols {
                        let text = r.get(c).cloned().unwrap_or_default();
                        body.push_str(&format!(
                            "<w:tc><w:tcPr><w:tcW w:w=\"{}\" w:type=\"dxa\"/></w:tcPr>{}</w:tc>",
                            9000 / cols.max(1),
                            para_xml(
                                None,
                                None,
                                &[Inline {
                                    text,
                                    bold: ri == 0,
                                    ..Default::default()
                                }]
                            )
                        ));
                    }
                    body.push_str("</w:tr>");
                }
                body.push_str("</w:tbl>");
            },
            Block::PageBreak => body.push_str("<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>"),
            Block::Rule => body.push_str(
                "<w:p><w:pPr><w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\" w:space=\"1\" \
                 w:color=\"999999\"/></w:pBdr></w:pPr></w:p>",
            ),
        }
    }
    let document = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" \
                            standalone=\"yes\"?><w:document \
                            xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><w:body>{body}<w:sectPr><w:pgSz w:w=\"12240\" w:h=\"15840\"/><w:pgMar w:top=\"1440\" w:right=\"1440\" w:bottom=\"1440\" w:left=\"1440\" w:header=\"708\" w:footer=\"708\" w:gutter=\"0\"/></w:sectPr></w:body></w:document>");
    let styles = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:cs="Calibri"/><w:sz w:val="22"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="160" w:line="259" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:keepNext/><w:spacing w:before="240" w:after="120"/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:sz w:val="36"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:pPr><w:keepNext/><w:spacing w:before="200" w:after="100"/><w:outlineLvl w:val="1"/></w:pPr><w:rPr><w:b/><w:sz w:val="30"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading3"><w:name w:val="heading 3"/><w:basedOn w:val="Normal"/><w:pPr><w:keepNext/><w:outlineLvl w:val="2"/></w:pPr><w:rPr><w:b/><w:sz w:val="26"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading4"><w:name w:val="heading 4"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="3"/></w:pPr><w:rPr><w:b/><w:i/><w:sz w:val="24"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading5"><w:name w:val="heading 5"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="4"/></w:pPr><w:rPr><w:b/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading6"><w:name w:val="heading 6"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="5"/></w:pPr><w:rPr><w:i/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="ListParagraph"><w:name w:val="List Paragraph"/><w:basedOn w:val="Normal"/><w:pPr><w:ind w:left="720"/><w:spacing w:after="40"/></w:pPr></w:style><w:style w:type="paragraph" w:styleId="Quote"><w:name w:val="Quote"/><w:basedOn w:val="Normal"/><w:pPr><w:ind w:left="720"/></w:pPr><w:rPr><w:i/><w:color w:val="404040"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Code"><w:name w:val="Code"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:after="0"/><w:shd w:val="clear" w:color="auto" w:fill="F2F2F2"/></w:pPr><w:rPr><w:rFonts w:ascii="Consolas" w:hAnsi="Consolas"/><w:sz w:val="18"/></w:rPr></w:style><w:style w:type="table" w:styleId="TableGrid"><w:name w:val="Table Grid"/><w:tblPr><w:tblCellMar><w:left w:w="108" w:type="dxa"/><w:right w:w="108" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style></w:styles>"#;
    let mut numbering = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
    );
    for (abs, fmt, text) in [(0, "bullet", "\u{2022}"), (1, "decimal", "%1.")] {
        numbering.push_str(&format!(
            "<w:abstractNum w:abstractNumId=\"{abs}\"><w:multiLevelType \
             w:val=\"hybridMultilevel\"/>"
        ));
        for lvl in 0..9 {
            let t = if fmt == "bullet" {
                text.to_string()
            } else {
                format!("%{}.", lvl + 1)
            };
            numbering.push_str(&format!(
                "<w:lvl w:ilvl=\"{lvl}\"><w:start w:val=\"1\"/><w:numFmt \
                 w:val=\"{fmt}\"/><w:lvlText w:val=\"{t}\"/><w:lvlJc \
                 w:val=\"left\"/><w:pPr><w:ind w:left=\"{}\" w:hanging=\"360\"/></w:pPr>{}</w:lvl>",
                720 + lvl * 360,
                if fmt == "bullet" {
                    "<w:rPr><w:rFonts w:ascii=\"Symbol\" w:hAnsi=\"Symbol\" \
                     w:hint=\"default\"/></w:rPr>"
                } else {
                    ""
                }
            ));
        }
        numbering.push_str("</w:abstractNum>");
    }
    numbering.push_str(
        "<w:num w:numId=\"1\"><w:abstractNumId w:val=\"0\"/></w:num><w:num \
         w:numId=\"2\"><w:abstractNumId w:val=\"1\"/></w:num></w:numbering>",
    );
    let content_types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/></Types>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/></Relationships>"#;
    let doc_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/></Relationships>"#;
    let core = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" \
                        standalone=\"yes\"?><cp:coreProperties \
                        xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"><dc:title>{}</dc:title><dc:creator>Dev Toolbox</dc:creator></cp:coreProperties>", xml_escape(title));
    let app = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Application>Dev Toolbox Document Engine</Application></Properties>"#;
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, data) in [
        ("[Content_Types].xml", content_types.to_string()),
        ("_rels/.rels", rels.to_string()),
        ("word/document.xml", document),
        ("word/styles.xml", styles.to_string()),
        ("word/numbering.xml", numbering),
        ("word/_rels/document.xml.rels", doc_rels.to_string()),
        ("docProps/core.xml", core),
        ("docProps/app.xml", app.to_string()),
    ] {
        z.start_file(name, opts)
            .map_err(|e| EngineError::internal(e.to_string()))?;
        z.write_all(data.as_bytes())
            .map_err(|e| EngineError::internal(e.to_string()))?;
    }
    Ok(z.finish()
        .map_err(|e| EngineError::internal(e.to_string()))?
        .into_inner())
}

// ---------------------------------------------------------- template fill --

/// Replaces `{{key}}` placeholders in document, header and footer parts. Runs
/// that split a placeholder are merged paragraph-wise.
pub fn fill_template(
    bytes: &[u8],
    values: &serde_json::Map<String, Value>,
    delimiters: (&str, &str),
) -> EngineResult<(Vec<u8>, Value)> {
    let mut z = open(bytes)?;
    if detect(&mut z) != Kind::Docx {
        return Err(EngineError::unsupported_format("Template filling supports DOCX files"));
    }
    let names: Vec<String> = z.file_names().map(str::to_string).collect();
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut replaced: HashMap<String, usize> = HashMap::new();
    let mut unresolved: Vec<String> = Vec::new();
    for name in names {
        let data = read_entry(&mut z, &name).unwrap_or_default();
        let is_text_part = name == "word/document.xml"
            || (name.starts_with("word/header")
                || name.starts_with("word/footer")
                || name.starts_with("word/footnotes")
                || name.starts_with("word/endnotes"))
                && name.ends_with(".xml");
        let data = if is_text_part {
            fill_part(&data, values, delimiters, &mut replaced, &mut unresolved)?
        } else {
            data
        };
        let method = if name == "mimetype" {
            zip::CompressionMethod::Stored
        } else {
            zip::CompressionMethod::Deflated
        };
        out.start_file(&name, SimpleFileOptions::default().compression_method(method))
            .map_err(|e| EngineError::internal(e.to_string()))?;
        out.write_all(&data)
            .map_err(|e| EngineError::internal(e.to_string()))?;
    }
    unresolved.sort();
    unresolved.dedup();
    let report = json!({
        "replaced": replaced,
        "unresolved_placeholders": unresolved,
        "values_provided": values.len(),
        "values_unused": values.keys().filter(|k| !replaced.contains_key(*k)).collect::<Vec<_>>()
    });
    Ok((
        out.finish()
            .map_err(|e| EngineError::internal(e.to_string()))?
            .into_inner(),
        report,
    ))
}

fn fill_part(
    xml: &[u8],
    values: &serde_json::Map<String, Value>,
    (open_d, close_d): (&str, &str),
    replaced: &mut HashMap<String, usize>,
    unresolved: &mut Vec<String>,
) -> EngineResult<Vec<u8>> {
    let text =
        String::from_utf8(xml.to_vec()).map_err(|_| EngineError::malformed("Part is not UTF-8"))?;
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    // Process paragraph by paragraph.
    while let Some(start) = rest.find("<w:p>").or_else(|| rest.find("<w:p ")) {
        let Some(end_rel) = rest[start..].find("</w:p>") else {
            break;
        };
        let end = start + end_rel + "</w:p>".len();
        out.push_str(&rest[..start]);
        let para = &rest[start..end];
        out.push_str(&fill_paragraph(para, values, open_d, close_d, replaced, unresolved));
        rest = &rest[end..];
    }
    out.push_str(rest);
    Ok(out.into_bytes())
}

fn fill_paragraph(
    para: &str,
    values: &serde_json::Map<String, Value>,
    open_d: &str,
    close_d: &str,
    replaced: &mut HashMap<String, usize>,
    unresolved: &mut Vec<String>,
) -> String {
    // Collect <w:t> contents.
    let mut texts: Vec<(usize, usize, String)> = Vec::new(); // (start of content, end of content, content)
    let mut pos = 0;
    while let Some(s) = para[pos..].find("<w:t") {
        let tag_start = pos + s;
        let Some(gt) = para[tag_start..].find('>') else {
            break;
        };
        let content_start = tag_start + gt + 1;
        if para[tag_start..content_start].ends_with("/>") {
            pos = content_start;
            continue;
        }
        let Some(close) = para[content_start..].find("</w:t>") else {
            break;
        };
        let content_end = content_start + close;
        texts.push((content_start, content_end, para[content_start..content_end].to_string()));
        pos = content_end;
    }
    let joined: String = texts.iter().map(|t| t.2.as_str()).collect();
    let decoded = joined
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&amp;", "&");
    if !decoded.contains(open_d) {
        return para.to_string();
    }
    // Replace placeholders in the joined text.
    let mut result = String::new();
    let mut r = decoded.as_str();
    while let Some(s) = r.find(open_d) {
        result.push_str(&r[..s]);
        let after = &r[s + open_d.len()..];
        match after.find(close_d) {
            Some(e) => {
                let key = after[..e].trim();
                match values.get(key) {
                    Some(v) => {
                        result.push_str(&match v {
                            Value::String(s) => s.clone(),
                            Value::Null => String::new(),
                            other => other.to_string(),
                        });
                        *replaced.entry(key.to_string()).or_insert(0) += 1;
                    },
                    None => {
                        unresolved.push(key.to_string());
                        result.push_str(&r[s..s + open_d.len() + e + close_d.len()]);
                    },
                }
                r = &after[e + close_d.len()..];
            },
            None => {
                result.push_str(&r[s..]);
                r = "";
            },
        }
    }
    result.push_str(r);
    if result == decoded {
        return para.to_string();
    }
    // Put everything into the first <w:t>, empty the others.
    let mut out = String::new();
    let mut last = 0;
    for (i, (cs, ce, _)) in texts.iter().enumerate() {
        out.push_str(&para[last..*cs]);
        if i == 0 {
            out.push_str(
                &xml_escape(&result).replace('\n', "</w:t><w:br/><w:t xml:space=\"preserve\">"),
            );
        }
        last = *ce;
    }
    out.push_str(&para[last..]);
    // Ensure whitespace preservation on the first <w:t>.
    out.replacen("<w:t>", "<w:t xml:space=\"preserve\">", 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_doc() -> Doc {
        Doc {
            blocks: vec![
                Block::Heading(
                    1,
                    vec![Inline {
                        text: "Title".into(),
                        ..Default::default()
                    }],
                ),
                Block::Paragraph(vec![
                    Inline {
                        text: "Hello ".into(),
                        ..Default::default()
                    },
                    Inline {
                        text: "bold".into(),
                        bold: true,
                        ..Default::default()
                    },
                    Inline {
                        text: " and {{name}}".into(),
                        italic: true,
                        ..Default::default()
                    },
                ]),
                Block::ListItem(
                    0,
                    false,
                    vec![Inline { text: "one".into(), ..Default::default() }],
                ),
                Block::ListItem(
                    0,
                    true,
                    vec![Inline {
                        text: "first".into(),
                        ..Default::default()
                    }],
                ),
                Block::Table(vec![vec!["a".into(), "b".into()], vec!["1".into(), "2".into()]]),
                Block::Code("let x = 1;".into()),
                Block::PageBreak,
                Block::Paragraph(vec![Inline {
                    text: "End".into(),
                    link: Some("https://x.y".into()),
                    ..Default::default()
                }]),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn docx_roundtrip_and_template() {
        let bytes = write_docx(&sample_doc(), "T").unwrap();
        let mut z = open(&bytes).unwrap();
        assert_eq!(detect(&mut z), Kind::Docx);
        let doc = read_docx(&mut z).unwrap();
        assert!(matches!(&doc.blocks[0], Block::Heading(1, i) if i[0].text == "Title"));
        assert!(
            matches!(&doc.blocks[1], Block::Paragraph(i) if i.len() == 3 && i[1].bold && i[2].italic)
        );
        assert!(matches!(&doc.blocks[2], Block::ListItem(0, false, _)));
        assert!(matches!(&doc.blocks[3], Block::ListItem(0, true, _)));
        assert!(
            matches!(&doc.blocks[4], Block::Table(rows) if rows.len() == 2 && rows[1][1] == "2")
        );
        assert!(matches!(&doc.blocks[5], Block::Code(c) if c == "let x = 1;"));
        assert!(matches!(&doc.blocks[6], Block::PageBreak));
        assert_eq!(doc.metadata["title"], "T");
        let html = doc.to_html(false, "");
        assert!(html.contains("<strong>bold</strong>"));
        let md = doc.to_markdown();
        assert!(md.starts_with("# Title\n\nHello **bold** *and {{name}}*"));
        let mut values = serde_json::Map::new();
        values.insert("name".into(), json!("World & Co"));
        let (filled, report) = fill_template(&bytes, &values, ("{{", "}}")).unwrap();
        assert_eq!(report["replaced"]["name"], 1);
        let mut z2 = open(&filled).unwrap();
        let d2 = read_docx(&mut z2).unwrap();
        assert!(d2.to_text().contains("Hello bold and World & Co"), "{}", d2.to_text());
        assert!(open(b"not a zip").is_err());
        assert!(open(&[0xD0, 0xCF, 0x11, 0xE0, 0, 0, 0, 0]).is_err());
    }
}
