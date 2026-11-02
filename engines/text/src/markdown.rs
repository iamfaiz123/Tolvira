//! Markdown ⇄ HTML conversion and Markdown structure parsing.

use engine_shared::{EngineError, EngineResult};
use pulldown_cmark::{html, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde_json::{json, Value};

fn options(
    tables: bool,
    footnotes: bool,
    strikethrough: bool,
    tasklists: bool,
    smart: bool,
) -> Options {
    let mut o = Options::empty();
    if tables {
        o.insert(Options::ENABLE_TABLES);
    }
    if footnotes {
        o.insert(Options::ENABLE_FOOTNOTES);
    }
    if strikethrough {
        o.insert(Options::ENABLE_STRIKETHROUGH);
    }
    if tasklists {
        o.insert(Options::ENABLE_TASKLISTS);
    }
    if smart {
        o.insert(Options::ENABLE_SMART_PUNCTUATION);
    }
    o.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    o
}

/// GitHub-style heading slug (lower-case, punctuation removed, spaces → `-`).
pub fn heading_slug(text: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in text.to_lowercase().chars() {
        if c.is_alphanumeric() {
            if dash && !out.is_empty() {
                out.push('-');
            }
            dash = false;
            out.push(c);
        } else {
            dash = true;
        }
    }
    out
}

/// Minimal HTML escaping.
pub fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Decodes the common named entities and numeric references.
pub fn decode_entities(s: &str) -> String {
    crate::html::decode_entities(s)
}

/// Markdown → HTML (GFM extensions, optional sanitising of raw HTML and
/// heading ids).
pub fn to_html(
    markdown: &str,
    sanitize: bool,
    heading_ids: bool,
    tables: bool,
    footnotes: bool,
    strikethrough: bool,
    tasklists: bool,
    smart_punctuation: bool,
    wrap_document: bool,
) -> EngineResult<Value> {
    if markdown.trim().is_empty() {
        return Err(EngineError::empty("markdown"));
    }
    let parser = Parser::new_ext(
        markdown,
        options(tables, footnotes, strikethrough, tasklists, smart_punctuation),
    );
    let mut headings: Vec<Value> = Vec::new();
    let mut current_heading: Option<(u8, String)> = None;
    let mut used_ids: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut events: Vec<Event> = Vec::new();
    let mut raw_html_removed = 0usize;
    for ev in parser {
        match &ev {
            Event::Html(h) | Event::InlineHtml(h) if sanitize => {
                raw_html_removed += 1;
                // Keep the text content escaped so nothing is silently lost.
                events.push(Event::Text(h.clone()));
                continue;
            },
            Event::Start(Tag::Heading { level, .. }) => {
                current_heading = Some((heading_level(*level), String::new()))
            },
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, s)) = current_heading.as_mut() {
                    s.push_str(t);
                }
            },
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, text)) = current_heading.take() {
                    let base = heading_slug(&text);
                    let n = used_ids.entry(base.clone()).or_insert(0);
                    let id = if *n == 0 { base.clone() } else { format!("{base}-{n}") };
                    *n += 1;
                    headings.push(json!({ "level": level, "text": text, "id": id }));
                }
            },
            _ => {},
        }
        events.push(ev);
    }
    // Re-walk to inject ids.
    let mut heading_idx = 0usize;
    let events: Vec<Event> = events
        .into_iter()
        .map(|ev| match ev {
            Event::Start(Tag::Heading { level, id, classes, attrs }) if heading_ids => {
                let new_id = headings
                    .get(heading_idx)
                    .and_then(|h| h["id"].as_str())
                    .map(|s| s.to_string().into());
                heading_idx += 1;
                Event::Start(Tag::Heading { level, id: id.or(new_id), classes, attrs })
            },
            other => other,
        })
        .collect();
    let mut out = String::with_capacity(markdown.len() * 2);
    html::push_html(&mut out, events.into_iter());
    let body = out;
    let html_doc = if wrap_document {
        format!(
            "<!DOCTYPE html>\n<html>\n<head>\n<meta \
             charset=\"utf-8\">\n<title>{}</title>\n</head>\n<body>\n{}</body>\n</html>\n",
            headings
                .first()
                .and_then(|h| h["text"].as_str())
                .map(escape_html)
                .unwrap_or_else(|| "Document".into()),
            body
        )
    } else {
        body.clone()
    };
    Ok(json!({
        "html": html_doc,
        "headings": headings,
        "raw_html_removed": raw_html_removed,
        "words": crate::analysis::words(markdown).len()
    }))
}

fn heading_level(l: HeadingLevel) -> u8 {
    match l {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Structural summary of a Markdown document (outline, links, images,
/// code blocks, tasks) plus rendered HTML.
pub fn parse(markdown: &str) -> EngineResult<Value> {
    if markdown.trim().is_empty() {
        return Err(EngineError::empty("markdown"));
    }
    let parser = Parser::new_ext(markdown, options(true, true, true, true, false));
    let mut links = Vec::new();
    let mut images = Vec::new();
    let mut code_blocks = Vec::new();
    let mut headings = Vec::new();
    let mut tasks = (0usize, 0usize);
    let mut tables = 0usize;
    let mut lists = 0usize;
    let mut blockquotes = 0usize;
    let mut footnotes = 0usize;
    let mut current_code: Option<(String, String)> = None;
    let mut current_heading: Option<(u8, String)> = None;
    let mut link_text = String::new();
    let mut in_link: Option<(String, String)> = None;
    for ev in parser {
        match ev {
            Event::Start(Tag::Link { dest_url, title, .. }) => {
                in_link = Some((dest_url.to_string(), title.to_string()))
            },
            Event::End(TagEnd::Link) => {
                if let Some((url, title)) = in_link.take() {
                    links.push(json!({
                        "url": url,
                        "text": link_text.trim(),
                        "title": title,
                        "external": url.starts_with("http")
                    }));
                    link_text.clear();
                }
            },
            Event::Start(Tag::Image { dest_url, title, .. }) => {
                images.push(json!({ "url": dest_url.to_string(), "title": title.to_string() }))
            },
            Event::Start(Tag::CodeBlock(kind)) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(l) => l.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                current_code = Some((lang, String::new()));
            },
            Event::End(TagEnd::CodeBlock) => {
                if let Some((lang, code)) = current_code.take() {
                    code_blocks.push(
                        json!({ "language": lang, "lines": code.lines().count(), "code": code }),
                    );
                }
            },
            Event::Start(Tag::Heading { level, .. }) => {
                current_heading = Some((heading_level(level), String::new()))
            },
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, text)) = current_heading.take() {
                    headings.push(
                        json!({ "level": level, "text": text.trim(), "id": heading_slug(&text) }),
                    );
                }
            },
            Event::Start(Tag::Table(_)) => tables += 1,
            Event::Start(Tag::List(_)) => lists += 1,
            Event::Start(Tag::BlockQuote(_)) => blockquotes += 1,
            Event::Start(Tag::FootnoteDefinition(_)) => footnotes += 1,
            Event::TaskListMarker(done) => {
                tasks.0 += 1;
                if done {
                    tasks.1 += 1;
                }
            },
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, c)) = current_code.as_mut() {
                    c.push_str(&t);
                } else {
                    if in_link.is_some() {
                        link_text.push_str(&t);
                    }
                    if let Some((_, h)) = current_heading.as_mut() {
                        h.push_str(&t);
                    }
                }
            },
            _ => {},
        }
    }
    let rendered = to_html(markdown, false, true, true, true, true, true, false, false)?;
    let toc = headings
        .iter()
        .map(|h| {
            format!(
                "{}- [{}](#{})",
                "  ".repeat((h["level"].as_u64().unwrap_or(1) as usize).saturating_sub(1)),
                h["text"].as_str().unwrap_or(""),
                h["id"].as_str().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(json!({
        "headings": headings,
        "toc_markdown": toc,
        "links": links,
        "images": images,
        "code_blocks": code_blocks,
        "tables": tables,
        "lists": lists,
        "blockquotes": blockquotes,
        "footnotes": footnotes,
        "tasks": { "total": tasks.0, "done": tasks.1 },
        "words": crate::analysis::words(markdown).len(),
        "html": rendered["html"],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_to_html() {
        let md = "# Title\n\nSome **bold** and <script>x</script> text.\n\n| a | b \
                  |\n|---|---|\n| 1 | 2 |\n\n- [x] done\n- [ ] todo\n\n## Title\n";
        let r = to_html(md, true, true, true, true, true, true, true, false).unwrap();
        let h = r["html"].as_str().unwrap();
        assert!(h.contains("<h1 id=\"title\">Title</h1>"));
        assert!(h.contains("<h2 id=\"title-1\">"));
        assert!(h.contains("<strong>bold</strong>"));
        assert!(!h.contains("<script>"));
        assert!(h.contains("<table>"));
        assert_eq!(r["raw_html_removed"], 2);
        assert_eq!(r["headings"].as_array().unwrap().len(), 2);
        let doc = to_html("hi", false, false, true, false, false, false, false, true).unwrap();
        assert!(doc["html"].as_str().unwrap().starts_with("<!DOCTYPE html>"));
        assert!(to_html("  ", true, true, true, true, true, true, true, false).is_err());
    }

    #[test]
    fn parse_structure() {
        let md = "# A\n\ntext [link](https://x.io \"t\") ![img](i.png)\n\n```rust\nfn main() \
                  {}\n```\n\n## B\n\n- [x] done\n- [ ] todo\n";
        let p = parse(md).unwrap();
        assert_eq!(p["headings"].as_array().unwrap().len(), 2);
        assert_eq!(p["links"][0]["url"], "https://x.io");
        assert_eq!(p["links"][0]["text"], "link");
        assert_eq!(p["images"][0]["url"], "i.png");
        assert_eq!(p["code_blocks"][0]["language"], "rust");
        assert_eq!(p["tasks"]["done"], 1);
        assert!(p["toc_markdown"].as_str().unwrap().contains("  - [B](#b)"));
        assert!(parse("").is_err());
    }
}
