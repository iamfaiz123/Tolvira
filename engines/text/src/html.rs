//! A small HTML tokenizer used for HTML → Markdown / text conversion.
//! It is intentionally forgiving (no full DOM): unknown tags are ignored and
//! their content kept.

use engine_shared::{EngineError, EngineResult};
use serde_json::{json, Value};

/// Token produced by the lightweight HTML tokenizer.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// Text run (entities already decoded).
    Text(String),
    /// Opening tag.
    Open {
        /// Lower-cased tag name.
        name: String,
        /// Attribute name/value pairs in source order.
        attrs: Vec<(String, String)>,
        /// `true` for `<br/>`-style self-closing syntax.
        self_closing: bool,
    },
    /// Closing tag (lower-cased name).
    Close(String),
}

/// Decodes named, decimal and hexadecimal HTML entities.
pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        match after.find(';').filter(|&n| n <= 12) {
            Some(semi) => {
                let ent = &after[..semi];
                let decoded = match ent {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
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
                    "middot" => Some('·'),
                    "euro" => Some('€'),
                    "pound" => Some('£'),
                    "yen" => Some('¥'),
                    "cent" => Some('¢'),
                    "sect" => Some('§'),
                    "deg" => Some('°'),
                    "times" => Some('×'),
                    "divide" => Some('÷'),
                    "plusmn" => Some('±'),
                    "para" => Some('¶'),
                    "iexcl" => Some('¡'),
                    "iquest" => Some('¿'),
                    "eacute" => Some('é'),
                    "egrave" => Some('è'),
                    "agrave" => Some('à'),
                    "aacute" => Some('á'),
                    "uuml" => Some('ü'),
                    "ouml" => Some('ö'),
                    "auml" => Some('ä'),
                    "szlig" => Some('ß'),
                    "ccedil" => Some('ç'),
                    "ntilde" => Some('ñ'),
                    "larr" => Some('←'),
                    "rarr" => Some('→'),
                    "uarr" => Some('↑'),
                    "darr" => Some('↓'),
                    "hearts" => Some('♥'),
                    "check" => Some('✓'),
                    _ => {
                        if let Some(h) = ent.strip_prefix("#x").or_else(|| ent.strip_prefix("#X")) {
                            u32::from_str_radix(h, 16).ok().and_then(char::from_u32)
                        } else if let Some(d) = ent.strip_prefix('#') {
                            d.parse::<u32>().ok().and_then(char::from_u32)
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
            },
            None => {
                out.push('&');
                rest = after;
            },
        }
    }
    out.push_str(rest);
    out
}

fn parse_attrs(raw: &str) -> Vec<(String, String)> {
    let mut attrs = Vec::new();
    let chars: Vec<char> = raw.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        while i < chars.len() && (chars[i].is_whitespace() || chars[i] == '/') {
            i += 1;
        }
        let start = i;
        while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '=' && chars[i] != '/' {
            i += 1;
        }
        if start == i {
            break;
        }
        let name: String = chars[start..i].iter().collect::<String>().to_ascii_lowercase();
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if i < chars.len() && chars[i] == '=' {
            i += 1;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            if i < chars.len() && (chars[i] == '"' || chars[i] == '\'') {
                let q = chars[i];
                i += 1;
                let vs = i;
                while i < chars.len() && chars[i] != q {
                    i += 1;
                }
                value = chars[vs..i.min(chars.len())].iter().collect();
                i += 1;
            } else {
                let vs = i;
                while i < chars.len() && !chars[i].is_whitespace() {
                    i += 1;
                }
                value = chars[vs..i].iter().collect();
            }
        }
        attrs.push((name, decode_entities(&value)));
    }
    attrs
}

/// Tokenises HTML. Script/style bodies are skipped, comments dropped.
pub fn tokenize(html: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut rest = html;
    let mut skip_until: Option<&str> = None;
    while !rest.is_empty() {
        if let Some(end_tag) = skip_until {
            let lower = rest.to_ascii_lowercase();
            match lower.find(end_tag) {
                Some(i) => {
                    rest = &rest[i..];
                    skip_until = None;
                },
                None => break,
            }
            continue;
        }
        match rest.find('<') {
            None => {
                tokens.push(Token::Text(decode_entities(rest)));
                break;
            },
            Some(i) => {
                if i > 0 {
                    tokens.push(Token::Text(decode_entities(&rest[..i])));
                }
                let after = &rest[i + 1..];
                if after.starts_with("!--") {
                    match after.find("-->") {
                        Some(e) => rest = &after[e + 3..],
                        None => break,
                    }
                    continue;
                }
                if after.starts_with('!') || after.starts_with('?') {
                    match after.find('>') {
                        Some(e) => rest = &after[e + 1..],
                        None => break,
                    }
                    continue;
                }
                let Some(end) = after.find('>') else {
                    tokens.push(Token::Text(decode_entities(rest)));
                    break;
                };
                let raw = &after[..end];
                rest = &after[end + 1..];
                if let Some(name) = raw.strip_prefix('/') {
                    tokens.push(Token::Close(name.trim().to_ascii_lowercase()));
                } else {
                    let name_end =
                        raw.find(|c: char| c.is_whitespace() || c == '/').unwrap_or(raw.len());
                    let name = raw[..name_end].to_ascii_lowercase();
                    let self_closing = raw.ends_with('/')
                        || matches!(
                            name.as_str(),
                            "br" | "hr"
                                | "img"
                                | "input"
                                | "meta"
                                | "link"
                                | "area"
                                | "base"
                                | "col"
                                | "embed"
                                | "source"
                                | "track"
                                | "wbr"
                        );
                    let attrs = parse_attrs(&raw[name_end..]);
                    if matches!(name.as_str(), "script" | "style" | "noscript" | "template") {
                        skip_until = Some(if name == "script" {
                            "</script"
                        } else if name == "style" {
                            "</style"
                        } else if name == "noscript" {
                            "</noscript"
                        } else {
                            "</template"
                        });
                    }
                    tokens.push(Token::Open { name, attrs, self_closing });
                }
            },
        }
    }
    tokens
}

fn attr<'a>(attrs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// Converts HTML to Markdown.
pub fn to_markdown(html: &str) -> EngineResult<Value> {
    if html.trim().is_empty() {
        return Err(EngineError::empty("html"));
    }
    let tokens = tokenize(html);
    let mut out = String::new();
    let mut list_stack: Vec<Option<usize>> = Vec::new();
    let mut pre = 0usize;
    let mut link: Option<String> = None;
    let mut link_text = String::new();
    let mut table: Option<Vec<Vec<String>>> = None;
    let mut cell: Option<String> = None;
    let mut in_head = 0usize;
    let mut quote = 0usize;
    let mut skipped_tags = 0usize;
    let mut warnings: Vec<String> = Vec::new();

    let push_block = |out: &mut String| {
        while out.ends_with(' ') {
            out.pop();
        }
        if !out.is_empty() && !out.ends_with("\n\n") {
            if out.ends_with('\n') {
                out.push('\n');
            } else {
                out.push_str("\n\n");
            }
        }
    };
    let emit = |out: &mut String,
                link_text: &mut String,
                cell: &mut Option<String>,
                link: &Option<String>,
                s: &str| {
        if let Some(c) = cell.as_mut() {
            c.push_str(s);
        } else if link.is_some() {
            link_text.push_str(s);
        } else {
            out.push_str(s);
        }
    };

    for t in tokens {
        match t {
            Token::Text(text) => {
                if in_head > 0 {
                    continue;
                }
                if pre > 0 {
                    out.push_str(&text);
                } else {
                    let collapsed: String = {
                        let mut s = String::new();
                        let mut prev_ws = false;
                        for c in text.chars() {
                            if c.is_whitespace() {
                                if !prev_ws {
                                    s.push(' ');
                                }
                                prev_ws = true;
                            } else {
                                s.push(c);
                                prev_ws = false;
                            }
                        }
                        s
                    };
                    let trimmed = if out.ends_with('\n') || out.is_empty() {
                        collapsed.trim_start().to_string()
                    } else {
                        collapsed
                    };
                    if !trimmed.is_empty() {
                        let prefixed = if quote > 0 && (out.ends_with('\n') || out.is_empty()) {
                            format!("{}{}", "> ".repeat(quote), trimmed)
                        } else {
                            trimmed
                        };
                        emit(&mut out, &mut link_text, &mut cell, &link, &prefixed);
                    }
                }
            },
            Token::Open { name, attrs, .. } => match name.as_str() {
                "head" => in_head += 1,
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    push_block(&mut out);
                    let n = name[1..].parse::<usize>().unwrap_or(1);
                    out.push_str(&"#".repeat(n));
                    out.push(' ');
                },
                "p" | "div" | "section" | "article" | "header" | "footer" | "main" | "nav"
                | "aside" | "figure" | "address" | "dl" | "dt" | "dd" | "details" | "summary" => {
                    push_block(&mut out)
                },
                "br" => emit(&mut out, &mut link_text, &mut cell, &link, "  \n"),
                "hr" => {
                    push_block(&mut out);
                    out.push_str("---\n\n");
                },
                "strong" | "b" => emit(&mut out, &mut link_text, &mut cell, &link, "**"),
                "em" | "i" | "cite" | "var" => {
                    emit(&mut out, &mut link_text, &mut cell, &link, "*")
                },
                "del" | "s" | "strike" => emit(&mut out, &mut link_text, &mut cell, &link, "~~"),
                "code" | "kbd" | "samp" | "tt" if pre == 0 => {
                    emit(&mut out, &mut link_text, &mut cell, &link, "`")
                },
                "pre" => {
                    push_block(&mut out);
                    out.push_str("```\n");
                    pre += 1;
                },
                "blockquote" => {
                    push_block(&mut out);
                    quote += 1;
                },
                "ul" => {
                    if list_stack.is_empty() {
                        push_block(&mut out);
                    }
                    list_stack.push(None);
                },
                "ol" => {
                    if list_stack.is_empty() {
                        push_block(&mut out);
                    }
                    let start = attr(&attrs, "start").and_then(|s| s.parse().ok()).unwrap_or(1);
                    list_stack.push(Some(start));
                },
                "li" => {
                    if !out.ends_with('\n') && !out.is_empty() {
                        out.push('\n');
                    }
                    let depth = list_stack.len().saturating_sub(1);
                    out.push_str(&"  ".repeat(depth));
                    match list_stack.last_mut() {
                        Some(Some(n)) => {
                            out.push_str(&format!("{n}. "));
                            *n += 1;
                        },
                        _ => out.push_str("- "),
                    }
                },
                "a" => {
                    link = attr(&attrs, "href").map(|h| h.to_string());
                    link_text.clear();
                },
                "img" => {
                    let src = attr(&attrs, "src").unwrap_or("");
                    let alt = attr(&attrs, "alt").unwrap_or("");
                    let title =
                        attr(&attrs, "title").map(|t| format!(" \"{t}\"")).unwrap_or_default();
                    emit(
                        &mut out,
                        &mut link_text,
                        &mut cell,
                        &link,
                        &format!("![{alt}]({src}{title})"),
                    );
                },
                "table" => {
                    push_block(&mut out);
                    table = Some(Vec::new());
                },
                "tr" => {
                    if let Some(t) = table.as_mut() {
                        t.push(Vec::new());
                    }
                },
                "td" | "th" => {
                    if table.is_some() {
                        cell = Some(String::new());
                    }
                },
                "input" => {
                    if attr(&attrs, "type") == Some("checkbox") {
                        out.push_str(if attrs.iter().any(|(k, _)| k == "checked") {
                            "[x] "
                        } else {
                            "[ ] "
                        });
                    }
                },
                "sup" => emit(&mut out, &mut link_text, &mut cell, &link, "^"),
                "sub" => emit(&mut out, &mut link_text, &mut cell, &link, "~"),
                "iframe" | "video" | "audio" | "canvas" | "svg" | "object" => {
                    skipped_tags += 1;
                },
                _ => {},
            },
            Token::Close(name) => match name.as_str() {
                "head" => in_head = in_head.saturating_sub(1),
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "div" | "section" | "article"
                | "header" | "footer" | "main" | "nav" | "aside" | "figure" | "dd" | "dt"
                | "details" | "summary" => push_block(&mut out),
                "strong" | "b" => emit(&mut out, &mut link_text, &mut cell, &link, "**"),
                "em" | "i" | "cite" | "var" => {
                    emit(&mut out, &mut link_text, &mut cell, &link, "*")
                },
                "del" | "s" | "strike" => emit(&mut out, &mut link_text, &mut cell, &link, "~~"),
                "code" | "kbd" | "samp" | "tt" if pre == 0 => {
                    emit(&mut out, &mut link_text, &mut cell, &link, "`")
                },
                "pre" => {
                    pre = pre.saturating_sub(1);
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                    out.push_str("```\n\n");
                },
                "blockquote" => {
                    quote = quote.saturating_sub(1);
                    push_block(&mut out);
                },
                "ul" | "ol" => {
                    list_stack.pop();
                    if list_stack.is_empty() {
                        push_block(&mut out);
                    }
                },
                "li" => {
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                },
                "a" => {
                    if let Some(href) = link.take() {
                        let text = if link_text.trim().is_empty() {
                            href.clone()
                        } else {
                            link_text.trim().to_string()
                        };
                        let md = format!("[{text}]({href})");
                        if let Some(c) = cell.as_mut() {
                            c.push_str(&md);
                        } else {
                            out.push_str(&md);
                        }
                        link_text.clear();
                    }
                },
                "td" | "th" => {
                    if let (Some(t), Some(c)) = (table.as_mut(), cell.take()) {
                        if let Some(row) = t.last_mut() {
                            row.push(c.trim().replace('|', "\\|"));
                        }
                    }
                },
                "table" => {
                    if let Some(rows) = table.take() {
                        if let Some(first) = rows.first() {
                            let cols =
                                rows.iter().map(|r| r.len()).max().unwrap_or(first.len()).max(1);
                            let fmt_row = |r: &Vec<String>| {
                                format!(
                                    "| {} |",
                                    (0..cols)
                                        .map(|i| r.get(i).cloned().unwrap_or_default())
                                        .collect::<Vec<_>>()
                                        .join(" | ")
                                )
                            };
                            out.push_str(&fmt_row(first));
                            out.push('\n');
                            out.push_str(&format!("|{}\n", " --- |".repeat(cols)));
                            for r in rows.iter().skip(1) {
                                out.push_str(&fmt_row(r));
                                out.push('\n');
                            }
                            out.push('\n');
                        }
                    }
                },
                "sup" => emit(&mut out, &mut link_text, &mut cell, &link, "^"),
                "sub" => emit(&mut out, &mut link_text, &mut cell, &link, "~"),
                _ => {},
            },
        }
    }
    if skipped_tags > 0 {
        warnings.push(format!(
            "{skipped_tags} embedded media element(s) (iframe/video/audio/svg) were dropped"
        ));
    }
    let cleaned = out.lines().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n");
    let re_blank = crate::re(r"\n{3,}")?;
    let result = re_blank.replace_all(cleaned.trim(), "\n\n").into_owned();
    Ok(json!({ "markdown": result, "warnings": warnings }))
}

/// Strips tags and returns plain text (block elements become newlines).
pub fn to_text(html: &str) -> EngineResult<Value> {
    if html.trim().is_empty() {
        return Err(EngineError::empty("html"));
    }
    let mut out = String::new();
    let mut in_head = 0usize;
    for t in tokenize(html) {
        match t {
            Token::Text(s) => {
                if in_head == 0 {
                    out.push_str(&s);
                }
            },
            Token::Open { name, .. } => match name.as_str() {
                "head" => in_head += 1,
                "br" => out.push('\n'),
                "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "li" | "tr" | "section"
                | "article" | "blockquote" | "pre" | "hr" => {
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                },
                "td" | "th" => out.push('\t'),
                _ => {},
            },
            Token::Close(name) => match name.as_str() {
                "head" => in_head = in_head.saturating_sub(1),
                "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "li" | "tr" | "section"
                | "article" | "blockquote" | "pre"
                    if !out.ends_with('\n') =>
                {
                    out.push('\n');
                },
                _ => {},
            },
        }
    }
    let re = crate::re(r"[ \t]+\n")?;
    let text = re.replace_all(&out, "\n").into_owned();
    let re2 = crate::re(r"\n{3,}")?;
    let text = re2.replace_all(text.trim(), "\n\n").into_owned();
    Ok(json!({ "text": text }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizer() {
        let t = tokenize(
            "<p class=\"x\">Hi &amp; <b>bold</b><br/></p><!-- c --><script>var x = \
             '<p>';</script>tail",
        );
        assert_eq!(
            t[0],
            Token::Open {
                name: "p".into(),
                attrs: vec![("class".into(), "x".into())],
                self_closing: false
            }
        );
        assert_eq!(t[1], Token::Text("Hi & ".into()));
        assert!(matches!(&t[5], Token::Open { name, self_closing: true, .. } if name == "br"));
        assert_eq!(t.last(), Some(&Token::Text("tail".into())));
        assert_eq!(decode_entities("&lt;a&gt; &#65;&#x42; &unknown; &"), "<a> AB &unknown; &");
    }

    #[test]
    fn html_to_markdown() {
        let html = r#"<html><head><title>T</title></head><body>
<h1>Title</h1>
<p>Some <strong>bold</strong> and <em>em</em> with a <a href="https://x.io">link</a> and <code>code</code>.</p>
<ul><li>one</li><li>two<ul><li>nested</li></ul></li></ul>
<ol start="3"><li>three</li><li>four</li></ol>
<pre><code>let x = 1;
let y = 2;</code></pre>
<blockquote><p>quoted</p></blockquote>
<table><tr><th>A</th><th>B</th></tr><tr><td>1</td><td>2</td></tr></table>
<img src="i.png" alt="pic"><hr>
</body></html>"#;
        let md = to_markdown(html).unwrap();
        let m = md["markdown"].as_str().unwrap();
        assert!(m.starts_with("# Title"));
        assert!(
            m.contains("**bold**")
                && m.contains("*em*")
                && m.contains("[link](https://x.io)")
                && m.contains("`code`")
        );
        assert!(m.contains("- one\n- two\n  - nested"));
        assert!(m.contains("3. three\n4. four"));
        assert!(m.contains("```\nlet x = 1;\nlet y = 2;\n```"));
        assert!(m.contains("> quoted"));
        assert!(m.contains("| A | B |\n| --- | --- |\n| 1 | 2 |"));
        assert!(m.contains("![pic](i.png)"));
        assert!(m.contains("---"));
        assert!(to_markdown("   ").is_err());
        let t = to_text("<h1>Hi</h1><p>there<br>you</p>").unwrap();
        assert_eq!(t["text"], "Hi\nthere\nyou");
    }
}
