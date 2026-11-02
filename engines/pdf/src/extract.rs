//! Text extraction and text based conversions: text, HTML, Markdown, DOCX
//! (text only) and document comparison.

use crate::doc::{self, PageSelection};
use engine_shared::{b64, EngineError, EngineResult};
use lopdf::Document;
use serde_json::{json, Value};
use similar::{ChangeTag, TextDiff};
use std::io::Write;

/// Extracts text for the selected pages. Pages whose fonts cannot be decoded
/// are reported in `warnings` instead of failing the whole call.
pub fn page_texts(d: &Document, pages: &[u32]) -> (Vec<(u32, String)>, Vec<String>) {
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    for &p in pages {
        match d.extract_text_with_limit(&[p], doc::MAX_DECOMPRESSED_STREAM) {
            Ok(t) => out.push((p, normalise_text(&t))),
            Err(e) => {
                warnings.push(format!("Page {p}: text could not be extracted ({e})"));
                out.push((p, String::new()));
            },
        }
    }
    (out, warnings)
}

fn normalise_text(t: &str) -> String {
    // lopdf separates text chunks with newlines; collapse runs of blank lines.
    let mut out = String::with_capacity(t.len());
    let mut blank = 0;
    for line in t.lines() {
        let l = line.trim_end();
        if l.trim().is_empty() {
            blank += 1;
            if blank <= 1 {
                out.push('\n');
            }
        } else {
            blank = 0;
            out.push_str(l);
            out.push('\n');
        }
    }
    out.trim_end().to_string()
}

/// Extracts text from the selected pages, grouping glyph runs into lines.
pub fn extract_text(
    bytes: &[u8],
    password: Option<&str>,
    pages: &PageSelection,
) -> EngineResult<Value> {
    let d = doc::load(bytes, password)?;
    let total = d.get_pages().len();
    let selected = pages.resolve(total)?;
    let (texts, warnings) = page_texts(&d, &selected);
    let full: Vec<String> = texts.iter().map(|(_, t)| t.clone()).collect();
    let words: usize = full.iter().map(|t| t.split_whitespace().count()).sum();
    let chars: usize = full.iter().map(|t| t.chars().count()).sum();
    Ok(json!({
        "text": full.join("\n\n"),
        "pages": texts.iter().map(|(p, t)| json!({ "page": p, "text": t })).collect::<Vec<_>>(),
        "total_pages": total,
        "words": words,
        "characters": chars,
        "has_text": chars > 0,
        "warnings": warnings,
    }))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Converts extracted text into a simple HTML document (one section per
/// page, paragraphs split on blank lines). Layout is not preserved.
pub fn to_html(
    bytes: &[u8],
    password: Option<&str>,
    pages: &PageSelection,
    title: Option<&str>,
) -> EngineResult<Value> {
    let d = doc::load(bytes, password)?;
    let total = d.get_pages().len();
    let selected = pages.resolve(total)?;
    let (texts, warnings) = page_texts(&d, &selected);
    let title = title
        .map(str::to_string)
        .or_else(|| metadata_title(&d))
        .unwrap_or_else(|| "Document".into());
    let mut html = String::new();
    html.push_str("<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>");
    html.push_str(&html_escape(&title));
    html.push_str("</title>\n<style>body{font-family:system-ui,sans-serif;max-width:800px;margin:2rem auto;line-height:1.5}section.page{border-bottom:1px solid #ddd;padding:1rem 0}h2.page-label{font-size:.8rem;color:#888;text-transform:uppercase}</style>\n</head>\n<body>\n");
    for (p, t) in &texts {
        html.push_str(&format!(
            "<section class=\"page\" id=\"page-{p}\">\n<h2 class=\"page-label\">Page {p}</h2>\n"
        ));
        for para in t.split("\n\n") {
            let para = para.trim();
            if para.is_empty() {
                continue;
            }
            html.push_str("<p>");
            html.push_str(&html_escape(para).replace('\n', "<br>\n"));
            html.push_str("</p>\n");
        }
        html.push_str("</section>\n");
    }
    html.push_str("</body>\n</html>\n");
    Ok(json!({
        "html": html,
        "pages": selected.len(),
        "warnings": warnings,
        "note": "Text only conversion; fonts, images and layout are not preserved."
    }))
}

fn metadata_title(d: &Document) -> Option<String> {
    d.trailer
        .get(b"Info")
        .ok()
        .and_then(|o| d.dereference(o).ok())
        .and_then(|(_, o)| o.as_dict().ok())
        .and_then(|i| i.get(b"Title").ok())
        .and_then(doc::text_string)
        .filter(|t| !t.trim().is_empty())
}

/// Converts extracted text to Markdown (paragraphs; lines that look like
/// headings – short, no trailing period, followed by a blank line – become `##`).
pub fn to_markdown(
    bytes: &[u8],
    password: Option<&str>,
    pages: &PageSelection,
) -> EngineResult<Value> {
    let d = doc::load(bytes, password)?;
    let total = d.get_pages().len();
    let selected = pages.resolve(total)?;
    let (texts, warnings) = page_texts(&d, &selected);
    let mut md = String::new();
    if let Some(t) = metadata_title(&d) {
        md.push_str(&format!("# {t}\n\n"));
    }
    for (i, (p, t)) in texts.iter().enumerate() {
        if i > 0 {
            md.push_str("\n---\n\n");
        }
        md.push_str(&format!("<!-- page {p} -->\n\n"));
        for para in t.split("\n\n") {
            let para = para.trim();
            if para.is_empty() {
                continue;
            }
            let lines: Vec<&str> = para.lines().collect();
            let looks_heading = lines.len() == 1
                && para.chars().count() < 80
                && !para.ends_with('.')
                && para.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
            if looks_heading {
                md.push_str(&format!("## {para}\n\n"));
            } else if lines.iter().all(|l| l.trim_start().starts_with(['-', '•', '*'])) {
                for l in lines {
                    md.push_str(&format!("- {}\n", l.trim_start_matches(['-', '•', '*', ' '])));
                }
                md.push('\n');
            } else {
                md.push_str(&para.replace('\n', " "));
                md.push_str("\n\n");
            }
        }
    }
    Ok(json!({
        "markdown": md.trim_end().to_string(),
        "pages": selected.len(),
        "warnings": warnings
    }))
}

/// Produces a minimal DOCX (WordprocessingML) containing the extracted text,
/// one paragraph per text line and a page break between PDF pages.
pub fn to_docx(bytes: &[u8], password: Option<&str>, pages: &PageSelection) -> EngineResult<Value> {
    let d = doc::load(bytes, password)?;
    let total = d.get_pages().len();
    let selected = pages.resolve(total)?;
    let (texts, warnings) = page_texts(&d, &selected);

    let mut body = String::new();
    for (i, (_, t)) in texts.iter().enumerate() {
        if i > 0 {
            body.push_str("<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>");
        }
        for para in t.split('\n') {
            body.push_str("<w:p><w:r><w:t xml:space=\"preserve\">");
            body.push_str(&xml_escape(para));
            body.push_str("</w:t></w:r></w:p>");
        }
    }
    let document_xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:document \
         xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{body}<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1440\" w:right=\"1440\" w:bottom=\"1440\" w:left=\"1440\"/></w:sectPr></w:body></w:document>"
    );
    let content_types = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types \
                         xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/></Types>";
    let rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships \
                xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/></Relationships>";
    let doc_rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships \
                    xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"></Relationships>";

    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in [
            ("[Content_Types].xml", content_types),
            ("_rels/.rels", rels),
            ("word/_rels/document.xml.rels", doc_rels),
            ("word/document.xml", &document_xml),
        ] {
            zip.start_file(name, opts)
                .map_err(|e| EngineError::internal(format!("DOCX write failed: {e}")))?;
            zip.write_all(data.as_bytes())
                .map_err(|e| EngineError::internal(format!("DOCX write failed: {e}")))?;
        }
        zip.finish()
            .map_err(|e| EngineError::internal(format!("DOCX write failed: {e}")))?;
    }
    let out = buf.into_inner();
    Ok(json!({
        "output": {
            "file": b64::encode(&out),
            "mime": "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "bytes": out.len()
        },
        "pages": selected.len(),
        "warnings": warnings,
        "note": "Text only conversion; formatting, images and layout are not preserved.",
    }))
}

fn xml_escape(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() || *c == '\t')
        .map(|c| match c {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            c => c.to_string(),
        })
        .collect()
}

/// Compares two PDFs: page counts, page sizes, metadata and a line level
/// text diff per page.
pub fn compare(
    a: &[u8],
    b: &[u8],
    password_a: Option<&str>,
    password_b: Option<&str>,
    max_changes: usize,
) -> EngineResult<Value> {
    let da = doc::load(a, password_a)
        .map_err(|e| EngineError::new(e.code, format!("File A: {}", e.message)))?;
    let db = doc::load(b, password_b)
        .map_err(|e| EngineError::new(e.code, format!("File B: {}", e.message)))?;
    let pa = da.get_pages().len();
    let pb = db.get_pages().len();
    let n = pa.max(pb);
    let (ta, wa) = page_texts(&da, &(1..=pa as u32).collect::<Vec<_>>());
    let (tb, wb) = page_texts(&db, &(1..=pb as u32).collect::<Vec<_>>());
    let mut pages = Vec::new();
    let mut total_changes = 0usize;
    let mut identical_pages = 0usize;
    let limit = if max_changes == 0 { 200 } else { max_changes };
    for i in 0..n {
        let xa = ta.get(i).map(|(_, t)| t.as_str()).unwrap_or("");
        let xb = tb.get(i).map(|(_, t)| t.as_str()).unwrap_or("");
        let diff = TextDiff::from_lines(xa, xb);
        let mut changes = Vec::new();
        let mut added = 0;
        let mut removed = 0;
        for c in diff.iter_all_changes() {
            match c.tag() {
                ChangeTag::Insert => {
                    added += 1;
                    if changes.len() < limit {
                        changes.push(json!({ "op": "+", "text": c.value().trim_end() }));
                    }
                },
                ChangeTag::Delete => {
                    removed += 1;
                    if changes.len() < limit {
                        changes.push(json!({ "op": "-", "text": c.value().trim_end() }));
                    }
                },
                ChangeTag::Equal => {},
            }
        }
        let size_a = da.get_pages().get(&(i as u32 + 1)).map(|&p| doc::media_box(&da, p));
        let size_b = db.get_pages().get(&(i as u32 + 1)).map(|&p| doc::media_box(&db, p));
        let same_size = match (size_a, size_b) {
            (Some(x), Some(y)) => {
                (x.width() - y.width()).abs() < 0.5 && (x.height() - y.height()).abs() < 0.5
            },
            _ => false,
        };
        let identical = added == 0 && removed == 0 && same_size && i < pa && i < pb;
        if identical {
            identical_pages += 1;
        }
        total_changes += added + removed;
        pages.push(json!({
            "page": i + 1,
            "in_a": i < pa,
            "in_b": i < pb,
            "identical_text": added == 0 && removed == 0,
            "same_size": same_size,
            "lines_added": added,
            "lines_removed": removed,
            "similarity": diff.ratio(),
            "changes": changes,
        }));
    }
    let meta_a = crate::meta::metadata(a, password_a)?["info"].clone();
    let meta_b = crate::meta::metadata(b, password_b)?["info"].clone();
    let mut warnings = wa;
    warnings.extend(wb);
    Ok(json!({
        "pages_a": pa,
        "pages_b": pb,
        "identical": total_changes == 0 && pa == pb && identical_pages == n,
        "identical_pages": identical_pages,
        "total_line_changes": total_changes,
        "pages": pages,
        "metadata_a": meta_a,
        "metadata_b": meta_b,
        "metadata_identical": meta_a == meta_b,
        "warnings": warnings,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::sample_pdf;

    #[test]
    fn text_html_markdown_docx() {
        let pdf = sample_pdf(&["Hello World", "Second Page"]);
        let t = extract_text(&pdf, None, &PageSelection::All).unwrap();
        assert!(t["text"].as_str().unwrap().contains("Hello World"));
        assert_eq!(t["pages"].as_array().unwrap().len(), 2);
        assert_eq!(t["has_text"], true);
        let h = to_html(&pdf, None, &PageSelection::List(vec![2]), Some("T")).unwrap();
        let html = h["html"].as_str().unwrap();
        assert!(
            html.contains("<title>T</title>")
                && html.contains("Second Page")
                && !html.contains("Hello")
        );
        let m = to_markdown(&pdf, None, &PageSelection::All).unwrap();
        assert!(m["markdown"].as_str().unwrap().contains("Hello World"));
        let dx = to_docx(&pdf, None, &PageSelection::All).unwrap();
        let bytes = b64::decode(dx["output"]["file"].as_str().unwrap()).unwrap();
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut s = String::new();
        std::io::Read::read_to_string(&mut z.by_name("word/document.xml").unwrap(), &mut s)
            .unwrap();
        assert!(s.contains("Hello World") && s.contains("w:br w:type=\"page\""));
        assert!(extract_text(b"", None, &PageSelection::All).is_err());
    }

    #[test]
    fn compare_docs() {
        let a = sample_pdf(&["same line", "only in a"]);
        let b = sample_pdf(&["same line", "only in b", "extra"]);
        let c = compare(&a, &b, None, None, 10).unwrap();
        assert_eq!(c["pages_a"], 2);
        assert_eq!(c["pages_b"], 3);
        assert_eq!(c["identical"], false);
        assert_eq!(c["pages"][0]["identical_text"], true);
        assert_eq!(c["pages"][1]["lines_added"], 1);
        assert_eq!(c["pages"][2]["in_a"], false);
        let same = compare(&a, &a, None, None, 10).unwrap();
        assert_eq!(same["identical"], true);
        assert!(compare(&a, b"bad", None, None, 10).is_err());
    }
}
