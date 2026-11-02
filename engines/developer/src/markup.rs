//! HTML and CSS formatting / minification and HTML → JSX conversion, built on
//! a tolerant hand-written tokenizer (no HTML5 tree construction needed).

use engine_shared::EngineResult;
use serde_json::{json, Value};

const MAX: usize = 5 * 1024 * 1024;

const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];
const INLINE: &[&str] = &[
    "a", "abbr", "b", "bdi", "bdo", "cite", "code", "data", "dfn", "em", "i", "kbd", "mark", "q",
    "s", "samp", "small", "span", "strong", "sub", "sup", "time", "u", "var", "wbr", "br", "img",
    "label", "button", "input", "select", "textarea",
];
const RAW: &[&str] = &["script", "style", "pre", "textarea"];

/// An HTML attribute.
#[derive(Debug, Clone, PartialEq)]
pub struct Attr {
    /// Attribute name (lower-cased).
    pub name: String,
    /// Attribute value (`None` for boolean attributes).
    pub value: Option<String>,
}

/// Token produced by the HTML tokenizer.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// `<!DOCTYPE …>` declaration.
    Doctype(String),
    /// `<!-- … -->` comment.
    Comment(String),
    /// Text run.
    Text(String),
    /// Start tag.
    Start {
        /// Tag name (lower-cased).
        name: String,
        /// Attributes in source order.
        attrs: Vec<Attr>,
        /// `true` for `<br/>`-style syntax.
        self_closing: bool,
    },
    /// End tag (lower-cased name).
    End(String),
    /// Raw content of script/style/pre/textarea.
    Raw {
        /// Tag name.
        tag: String,
        /// Raw content.
        content: String,
    },
}

/// Tokenises HTML, keeping script/style/pre/textarea content raw.
pub fn tokenize(html: &str) -> Vec<Token> {
    let chars: Vec<char> = html.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    let mut text = String::new();
    let flush = |text: &mut String, out: &mut Vec<Token>| {
        if !text.is_empty() {
            out.push(Token::Text(std::mem::take(text)));
        }
    };
    while i < chars.len() {
        let c = chars[i];
        if c == '<' {
            let rest: String = chars[i..(i + 12).min(chars.len())].iter().collect();
            if rest.starts_with("<!--") {
                flush(&mut text, &mut out);
                let start = i + 4;
                let mut j = start;
                while j + 2 < chars.len()
                    && !(chars[j] == '-' && chars[j + 1] == '-' && chars[j + 2] == '>')
                {
                    j += 1;
                }
                out.push(Token::Comment(chars[start..j.min(chars.len())].iter().collect()));
                i = (j + 3).min(chars.len());
                continue;
            }
            if rest.to_ascii_lowercase().starts_with("<!doctype") || rest.starts_with("<!") {
                flush(&mut text, &mut out);
                let mut j = i;
                while j < chars.len() && chars[j] != '>' {
                    j += 1;
                }
                out.push(Token::Doctype(chars[i + 2..j.min(chars.len())].iter().collect()));
                i = (j + 1).min(chars.len());
                continue;
            }
            if i + 1 < chars.len()
                && (chars[i + 1].is_ascii_alphabetic()
                    || chars[i + 1] == '/'
                        && i + 2 < chars.len()
                        && chars[i + 2].is_ascii_alphabetic())
            {
                flush(&mut text, &mut out);
                let closing = chars[i + 1] == '/';
                let mut j = i + if closing { 2 } else { 1 };
                let mut name = String::new();
                while j < chars.len()
                    && !chars[j].is_whitespace()
                    && chars[j] != '>'
                    && chars[j] != '/'
                {
                    name.push(chars[j]);
                    j += 1;
                }
                let name = name.to_ascii_lowercase();
                let mut attrs = Vec::new();
                let mut self_closing = false;
                loop {
                    while j < chars.len() && chars[j].is_whitespace() {
                        j += 1;
                    }
                    if j >= chars.len() {
                        break;
                    }
                    if chars[j] == '>' {
                        j += 1;
                        break;
                    }
                    if chars[j] == '/' {
                        self_closing = true;
                        j += 1;
                        continue;
                    }
                    let mut an = String::new();
                    while j < chars.len()
                        && !chars[j].is_whitespace()
                        && chars[j] != '='
                        && chars[j] != '>'
                        && !(chars[j] == '/' && chars.get(j + 1) == Some(&'>'))
                    {
                        an.push(chars[j]);
                        j += 1;
                    }
                    while j < chars.len() && chars[j].is_whitespace() {
                        j += 1;
                    }
                    let mut value = None;
                    if j < chars.len() && chars[j] == '=' {
                        j += 1;
                        while j < chars.len() && chars[j].is_whitespace() {
                            j += 1;
                        }
                        if j < chars.len() && (chars[j] == '"' || chars[j] == '\'') {
                            let q = chars[j];
                            j += 1;
                            let mut v = String::new();
                            while j < chars.len() && chars[j] != q {
                                v.push(chars[j]);
                                j += 1;
                            }
                            j += 1;
                            value = Some(v);
                        } else {
                            let mut v = String::new();
                            while j < chars.len() && !chars[j].is_whitespace() && chars[j] != '>' {
                                v.push(chars[j]);
                                j += 1;
                            }
                            value = Some(v);
                        }
                    }
                    if !an.is_empty() {
                        attrs.push(Attr { name: an, value });
                    } else if j < chars.len() && chars[j] != '>' && chars[j] != '/' {
                        j += 1;
                    }
                }
                i = j;
                if closing {
                    out.push(Token::End(name));
                } else {
                    let is_raw = RAW.contains(&name.as_str());
                    out.push(Token::Start { name: name.clone(), attrs, self_closing });
                    if is_raw && !self_closing {
                        // Capture until the matching close tag.
                        let end = find_close_tag(&chars, i, &name).unwrap_or(chars.len() - i);
                        let content: String = chars[i..i + end].iter().collect();
                        out.push(Token::Raw { tag: name.clone(), content });
                        i += end;
                    }
                }
                continue;
            }
        }
        text.push(c);
        i += 1;
    }
    flush(&mut text, &mut out);
    out
}

/// Finds the offset (relative to `from`) of `</name` (case-insensitive).
fn find_close_tag(chars: &[char], from: usize, name: &str) -> Option<usize> {
    let needle: Vec<char> = format!("</{name}").chars().collect();
    let hay = &chars[from..];
    (0..hay.len().saturating_sub(needle.len()) + 1).find(|&k| {
        hay[k..k + needle.len()]
            .iter()
            .zip(&needle)
            .all(|(a, b)| a.to_ascii_lowercase() == *b)
    })
}

fn check(html: &str) -> EngineResult<()> {
    engine_shared::error::require_text(html, "html")?;
    engine_shared::limits::check_size(html.len(), MAX, "html")
}

fn attr_string(attrs: &[Attr]) -> String {
    attrs
        .iter()
        .map(|a| match &a.value {
            Some(v) => format!(" {}=\"{}\"", a.name, v.replace('"', "&quot;")),
            None => format!(" {}", a.name),
        })
        .collect()
}

/// Indents HTML: block elements on their own lines, inline runs kept together,
/// raw elements preserved.
pub fn format_html(html: &str, indent_size: usize, max_inline: usize) -> EngineResult<Value> {
    check(html)?;
    let tokens = tokenize(html);
    let indent_unit = if indent_size == 0 {
        "\t".to_string()
    } else {
        " ".repeat(indent_size.min(8))
    };
    let max_inline = if max_inline == 0 { 80 } else { max_inline };
    let mut out = String::new();
    let mut depth: usize = 0;
    let mut stack: Vec<String> = Vec::new();
    let is_inline = |n: &str| INLINE.contains(&n);
    let mut i = 0;
    let mut line_open = false; // currently writing inline content on a line
    let newline = |out: &mut String, depth: usize, unit: &str| {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&unit.repeat(depth));
    };
    while i < tokens.len() {
        match &tokens[i] {
            Token::Doctype(d) => {
                newline(&mut out, depth, &indent_unit);
                out.push_str(&format!("<!{}>", d.trim()));
                line_open = false;
            },
            Token::Comment(c) => {
                newline(&mut out, depth, &indent_unit);
                out.push_str(&format!("<!--{c}-->"));
                line_open = false;
            },
            Token::Text(t) => {
                let collapsed = t.split_whitespace().collect::<Vec<_>>().join(" ");
                if collapsed.is_empty() {
                    i += 1;
                    continue;
                }
                if !line_open {
                    newline(&mut out, depth, &indent_unit);
                } else if !t.starts_with(|c: char| c.is_whitespace()) {
                } else {
                    out.push(' ');
                }
                out.push_str(collapsed.trim_start_matches(' '));
                if t.ends_with(|c: char| c.is_whitespace()) && i + 1 < tokens.len() {
                    out.push(' ');
                }
                line_open = true;
            },
            Token::Start { name, attrs, self_closing } => {
                let tag = format!(
                    "<{}{}{}>",
                    name,
                    attr_string(attrs),
                    if *self_closing && !VOID.contains(&name.as_str()) {
                        " /"
                    } else {
                        ""
                    }
                );
                let void = VOID.contains(&name.as_str()) || *self_closing;
                if is_inline(name) && line_open {
                    out.push_str(&tag);
                } else if is_inline(name) {
                    newline(&mut out, depth, &indent_unit);
                    out.push_str(&tag);
                    line_open = true;
                } else {
                    // Block: does its whole content fit on one line?
                    let (end_idx, inline_len) = block_inline_extent(&tokens, i, name);
                    newline(&mut out, depth, &indent_unit);
                    out.push_str(&tag);
                    if let Some(end) = end_idx.filter(|_| inline_len <= max_inline) {
                        for (k, t) in tokens[i + 1..end].iter().enumerate() {
                            match t {
                                Token::Text(s) => {
                                    let collapsed =
                                        s.split_whitespace().collect::<Vec<_>>().join(" ");
                                    if k > 0
                                        && s.starts_with(|c: char| c.is_whitespace())
                                        && !collapsed.is_empty()
                                    {
                                        out.push(' ');
                                    }
                                    out.push_str(&collapsed);
                                    if k + 2 < end - i && s.ends_with(|c: char| c.is_whitespace()) {
                                        out.push(' ');
                                    }
                                },
                                Token::Start { name, attrs, self_closing } => {
                                    out.push_str(&format!(
                                        "<{}{}{}>",
                                        name,
                                        attr_string(attrs),
                                        if *self_closing && !VOID.contains(&name.as_str()) {
                                            " /"
                                        } else {
                                            ""
                                        }
                                    ))
                                },
                                Token::End(n) => out.push_str(&format!("</{n}>")),
                                _ => {},
                            }
                        }
                        out.push_str(&format!("</{name}>"));
                        i = end + 1;
                        line_open = false;
                        continue;
                    }
                    line_open = false;
                    if !void {
                        depth += 1;
                        stack.push(name.clone());
                    }
                }
                if void && !is_inline(name) {
                    line_open = false;
                }
            },
            Token::Raw { tag, content } => {
                if content.trim().is_empty() {
                    i += 1;
                    continue;
                }
                if tag == "pre" || tag == "textarea" {
                    out.push_str(content);
                    line_open = true;
                } else {
                    // Re-indent script/style relative to the tag.
                    let lines: Vec<&str> = content.lines().collect();
                    let min_indent = lines
                        .iter()
                        .filter(|l| !l.trim().is_empty())
                        .map(|l| l.len() - l.trim_start().len())
                        .min()
                        .unwrap_or(0);
                    for l in lines.iter().skip_while(|l| l.trim().is_empty()) {
                        out.push('\n');
                        if !l.trim().is_empty() {
                            out.push_str(&indent_unit.repeat(depth));
                            out.push_str(l[min_indent.min(l.len())..].trim_end());
                        }
                    }
                    line_open = false;
                }
            },
            Token::End(name) => {
                if is_inline(name) && line_open {
                    out.push_str(&format!("</{name}>"));
                } else if RAW.contains(&name.as_str()) && (name == "pre" || name == "textarea") {
                    out.push_str(&format!("</{name}>"));
                    depth = depth.saturating_sub(1);
                    stack.pop();
                    line_open = false;
                } else {
                    if stack.last() == Some(name) {
                        depth = depth.saturating_sub(1);
                        stack.pop();
                    } else if stack.contains(name) {
                        while stack.last() != Some(name) {
                            stack.pop();
                            depth = depth.saturating_sub(1);
                        }
                        stack.pop();
                        depth = depth.saturating_sub(1);
                    }
                    newline(&mut out, depth, &indent_unit);
                    out.push_str(&format!("</{name}>"));
                    line_open = false;
                }
            },
        }
        i += 1;
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    let unclosed = stack.clone();
    Ok(json!({
        "formatted": out,
        "unclosed_tags": unclosed,
        "element_count": tokens.iter().filter(|t| matches!(t, Token::Start { .. })).count()
    }))
}

/// If the block element starting at `start` contains only text/inline
/// children, returns the index of its end tag and the inline length.
fn block_inline_extent(tokens: &[Token], start: usize, name: &str) -> (Option<usize>, usize) {
    let mut len = 0;
    let mut depth = 0;
    for (j, t) in tokens.iter().enumerate().skip(start + 1) {
        match t {
            Token::Text(s) => len += s.split_whitespace().map(|w| w.len() + 1).sum::<usize>(),
            Token::Start { name: n, self_closing, .. } => {
                if !INLINE.contains(&n.as_str()) {
                    return (None, 0);
                }
                len += n.len() + 2;
                if !VOID.contains(&n.as_str()) && !self_closing {
                    depth += 1;
                }
            },
            Token::End(n) => {
                if n == name && depth == 0 {
                    return (Some(j), len);
                }
                if INLINE.contains(&n.as_str()) {
                    depth -= 1;
                    len += n.len() + 3;
                } else {
                    return (None, 0);
                }
            },
            Token::Doctype(_) | Token::Comment(_) | Token::Raw { .. } => return (None, 0),
        }
        if len > 10_000 {
            return (None, 0);
        }
    }
    (None, 0)
}

/// Minifies HTML: strips comments (keeps conditional comments), collapses
/// whitespace, drops whitespace between block tags, preserves raw elements.
pub fn minify_html(
    html: &str,
    remove_comments: bool,
    collapse_boolean: bool,
) -> EngineResult<Value> {
    check(html)?;
    let tokens = tokenize(html);
    let mut out = String::with_capacity(html.len());
    let mut prev_block = true;
    for (i, t) in tokens.iter().enumerate() {
        match t {
            Token::Doctype(d) => {
                out.push_str(&format!("<!{}>", d.split_whitespace().collect::<Vec<_>>().join(" ")))
            },
            Token::Comment(c) => {
                if !remove_comments || c.starts_with('[') || c.trim_start().starts_with("<![endif]")
                {
                    out.push_str(&format!("<!--{c}-->"));
                }
            },
            Token::Text(s) => {
                let next_block = matches!(tokens.get(i + 1), Some(Token::Start { name, .. }) | Some(Token::End(name)) if !INLINE.contains(&name.as_str()));
                let collapsed = s.split_whitespace().collect::<Vec<_>>().join(" ");
                if collapsed.is_empty() {
                    if !(prev_block || next_block) {
                        out.push(' ');
                        prev_block = false;
                    }
                    continue;
                } else {
                    if s.starts_with(|c: char| c.is_whitespace()) && !prev_block {
                        out.push(' ');
                    }
                    out.push_str(&collapsed);
                    if s.ends_with(|c: char| c.is_whitespace()) && !next_block {
                        out.push(' ');
                    }
                }
                prev_block = false;
                continue;
            },
            Token::Start { name, attrs, self_closing } => {
                out.push('<');
                out.push_str(name);
                for a in attrs {
                    out.push(' ');
                    out.push_str(&a.name);
                    if let Some(v) = &a.value {
                        let is_bool = collapse_boolean
                            && matches!(
                                a.name.as_str(),
                                "disabled"
                                    | "checked"
                                    | "selected"
                                    | "readonly"
                                    | "required"
                                    | "hidden"
                                    | "autoplay"
                                    | "controls"
                                    | "loop"
                                    | "muted"
                                    | "multiple"
                                    | "async"
                                    | "defer"
                                    | "novalidate"
                                    | "open"
                            );
                        if is_bool
                            && (v.is_empty() || v.eq_ignore_ascii_case(&a.name) || v == "true")
                        {
                            continue;
                        }
                        let v = if a.name == "class" {
                            v.split_whitespace().collect::<Vec<_>>().join(" ")
                        } else {
                            v.clone()
                        };
                        let needs_quotes = v.is_empty()
                            || v.contains(|c: char| {
                                c.is_whitespace() || matches!(c, '"' | '\'' | '=' | '<' | '>' | '`')
                            });
                        if needs_quotes {
                            out.push_str(&format!("=\"{}\"", v.replace('"', "&quot;")));
                        } else {
                            out.push('=');
                            out.push_str(&v);
                        }
                    }
                }
                if *self_closing && !VOID.contains(&name.as_str()) {
                    out.push('/');
                }
                out.push('>');
                prev_block = !INLINE.contains(&name.as_str());
            },
            Token::Raw { tag, content } => {
                if tag == "script" || tag == "style" {
                    // Light minification: trim lines and drop blank ones (keeps strings intact).
                    let lines: Vec<&str> =
                        content.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
                    out.push_str(&lines.join("\n"));
                } else {
                    out.push_str(content);
                }
            },
            Token::End(name) => {
                out.push_str(&format!("</{name}>"));
                prev_block = !INLINE.contains(&name.as_str());
            },
        }
    }
    let out = out.trim().to_string();
    Ok(json!({
        "minified": out,
        "input_length": html.len(),
        "output_length": out.len(),
        "saved_percent": if html.is_empty() { 0.0 } else { ((1.0 - out.len() as f64 / html.len() as f64) * 1000.0).round() / 10.0 }
    }))
}

// ------------------------------------------------------------------- JSX --

fn camel(s: &str) -> String {
    let mut out = String::new();
    let mut up = false;
    for c in s.chars() {
        if c == '-' || c == ':' || c == '_' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn jsx_attr_name(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    let mapped = match lower.as_str() {
        "class" => "className",
        "for" => "htmlFor",
        "tabindex" => "tabIndex",
        "readonly" => "readOnly",
        "maxlength" => "maxLength",
        "minlength" => "minLength",
        "colspan" => "colSpan",
        "rowspan" => "rowSpan",
        "cellpadding" => "cellPadding",
        "cellspacing" => "cellSpacing",
        "autocomplete" => "autoComplete",
        "autofocus" => "autoFocus",
        "autoplay" => "autoPlay",
        "contenteditable" => "contentEditable",
        "crossorigin" => "crossOrigin",
        "datetime" => "dateTime",
        "enctype" => "encType",
        "formaction" => "formAction",
        "frameborder" => "frameBorder",
        "hreflang" => "hrefLang",
        "inputmode" => "inputMode",
        "novalidate" => "noValidate",
        "srcset" => "srcSet",
        "srcdoc" => "srcDoc",
        "usemap" => "useMap",
        "accesskey" => "accessKey",
        "allowfullscreen" => "allowFullScreen",
        "spellcheck" => "spellCheck",
        "accept-charset" => "acceptCharset",
        "http-equiv" => "httpEquiv",
        "viewbox" => "viewBox",
        "xlink:href" => "xlinkHref",
        "xml:lang" => "xmlLang",
        "xmlns:xlink" => "xmlnsXlink",
        "charset" => "charSet",
        "itemprop" => "itemProp",
        "itemscope" => "itemScope",
        "itemtype" => "itemType",
        "playsinline" => "playsInline",
        "referrerpolicy" => "referrerPolicy",
        _ => "",
    };
    if !mapped.is_empty() {
        return mapped.to_string();
    }
    if lower.starts_with("data-") || lower.starts_with("aria-") {
        return lower;
    }
    if let Some(ev) = lower.strip_prefix("on") {
        let mut c = ev.chars();
        return match c.next() {
            Some(f) => format!("on{}{}", f.to_uppercase(), c.as_str()),
            None => lower,
        };
    }
    if lower.contains('-') || lower.contains(':') {
        return camel(&lower);
    }
    name.to_string()
}

fn style_to_object(style: &str) -> String {
    let decls: Vec<String> = style
        .split(';')
        .filter_map(|d| {
            let (k, v) = d.split_once(':')?;
            let k = k.trim();
            let v = v.trim();
            if k.is_empty() || v.is_empty() {
                return None;
            }
            let key = if k.starts_with("--") {
                format!("'{k}'")
            } else if let Some(rest) = k.strip_prefix('-') {
                camel(rest.trim_start_matches('-')).pipe(|s| {
                    let mut c = s.chars();
                    c.next()
                        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                        .unwrap_or_default()
                })
            } else {
                camel(k)
            };
            let num = v.parse::<f64>().is_ok();
            Some(format!(
                "{key}: {}",
                if num {
                    v.to_string()
                } else {
                    format!("'{}'", v.replace('\'', "\\'"))
                }
            ))
        })
        .collect();
    format!("{{{{ {} }}}}", decls.join(", "))
}

trait Pipe: Sized {
    fn pipe<R>(self, f: impl FnOnce(Self) -> R) -> R {
        f(self)
    }
}
impl Pipe for String {}

fn jsx_text(t: &str) -> String {
    let mut out = String::new();
    for c in t.chars() {
        match c {
            '{' => out.push_str("{'{'}"),
            '}' => out.push_str("{'}'}"),
            _ => out.push(c),
        }
    }
    out
}

/// Indents generated JSX: one element/text run per line, nested by depth.
/// Works on the JSX string itself because JSX attribute expressions
/// (`{...}`) are not valid HTML for the tokenizer.
fn pretty_jsx(jsx: &str, fragment: bool) -> String {
    let chars: Vec<char> = jsx.chars().collect();
    let mut pieces: Vec<String> = Vec::new();
    let mut i = 0;
    let mut text = String::new();
    while i < chars.len() {
        if chars[i] == '<' {
            if !text.trim().is_empty() {
                pieces.push(text.trim().to_string());
            }
            text.clear();
            // Read a tag, honouring quotes and {} expressions.
            let start = i;
            let mut brace = 0;
            let mut quote: Option<char> = None;
            while i < chars.len() {
                let c = chars[i];
                if let Some(q) = quote {
                    if c == q {
                        quote = None;
                    }
                } else if c == '"' || c == '\'' {
                    quote = Some(c);
                } else if c == '{' {
                    brace += 1;
                } else if c == '}' {
                    brace -= 1;
                } else if c == '>' && brace == 0 {
                    i += 1;
                    break;
                }
                i += 1;
            }
            pieces.push(chars[start..i].iter().collect());
            continue;
        }
        if chars[i] == '{' && text.trim().is_empty() {
            // JSX expression container at text level: {/* */} or {`...`}
            let start = i;
            let mut brace = 0;
            while i < chars.len() {
                if chars[i] == '{' {
                    brace += 1;
                } else if chars[i] == '}' {
                    brace -= 1;
                    if brace == 0 {
                        i += 1;
                        break;
                    }
                }
                i += 1;
            }
            pieces.push(chars[start..i].iter().collect());
            text.clear();
            continue;
        }
        text.push(chars[i]);
        i += 1;
    }
    if !text.trim().is_empty() {
        pieces.push(text.trim().to_string());
    }
    let mut out = String::new();
    let mut depth = if fragment { 1 } else { 0 };
    if fragment {
        out.push_str("<>\n");
    }
    let mut k = 0;
    while k < pieces.len() {
        let p = &pieces[k];
        let is_close = p.starts_with("</");
        let is_open = p.starts_with('<') && !is_close && !p.ends_with("/>");
        // Element with a single short text child and matching close → one line.
        if is_open
            && k + 2 < pieces.len()
            && !pieces[k + 1].starts_with('<')
            && pieces[k + 2].starts_with("</")
            && p.len() + pieces[k + 1].len() < 80
        {
            out.push_str(&"  ".repeat(depth));
            out.push_str(&format!("{}{}{}\n", p, pieces[k + 1], pieces[k + 2]));
            k += 3;
            continue;
        }
        if is_close {
            depth = depth.saturating_sub(1);
        }
        out.push_str(&"  ".repeat(depth));
        out.push_str(p);
        out.push('\n');
        if is_open {
            depth += 1;
        }
        k += 1;
    }
    if fragment {
        out.push_str("</>");
    }
    out.trim_end().to_string()
}

/// Converts HTML to JSX. Optionally wraps it in a function component.
pub fn html_to_jsx(html: &str, component_name: &str, typescript: bool) -> EngineResult<Value> {
    check(html)?;
    let tokens = tokenize(html);
    let mut body = String::new();
    let mut roots = 0usize;
    let mut depth = 0usize;
    let mut notes: Vec<String> = Vec::new();
    for t in &tokens {
        match t {
            Token::Doctype(_) => notes.push("Removed <!DOCTYPE> (not valid in JSX)".into()),
            Token::Comment(c) => body.push_str(&format!("{{/*{c}*/}}")),
            Token::Text(s) => {
                if depth == 0 && !s.trim().is_empty() {
                    roots += 1;
                }
                body.push_str(&jsx_text(s));
            },
            Token::Start { name, attrs, self_closing } => {
                if depth == 0 {
                    roots += 1;
                }
                let tag = if name == "html" || name == "head" || name == "body" {
                    notes.push(format!(
                        "<{name}> kept as-is; usually remove document-level tags in components"
                    ));
                    name.clone()
                } else {
                    name.clone()
                };
                body.push('<');
                body.push_str(&tag);
                for a in attrs {
                    let jn = jsx_attr_name(&a.name);
                    match &a.value {
                        None => body.push_str(&format!(" {jn}")),
                        Some(v) if jn == "style" => {
                            body.push_str(&format!(" style={}", style_to_object(v)))
                        },
                        Some(v)
                            if jn.starts_with("on")
                                && jn.len() > 2
                                && jn.chars().nth(2).is_some_and(|c| c.is_uppercase()) =>
                        {
                            notes.push(format!(
                                "Inline handler `{}` converted to an arrow function; review the \
                                 body",
                                a.name
                            ));
                            body.push_str(&format!(" {jn}={{() => {{ {v} }}}}"));
                        },
                        Some(v) if v.contains('"') && !v.contains('\'') => {
                            body.push_str(&format!(" {jn}='{v}'"))
                        },
                        Some(v) if v.contains('"') => {
                            body.push_str(&format!(" {jn}={{\"{}\"}}", v.replace('"', "\\\"")))
                        },
                        Some(v) => body.push_str(&format!(" {jn}=\"{v}\"")),
                    }
                }
                if VOID.contains(&name.as_str()) || *self_closing {
                    body.push_str(" />");
                } else {
                    body.push('>');
                    depth += 1;
                }
            },
            Token::Raw { tag, content } => {
                if tag == "script" {
                    notes.push(
                        "<script> content kept verbatim inside a template literal; move logic \
                         into the component"
                            .into(),
                    );
                    body.push_str(&format!("{{`{}`}}", content.replace('`', "\\`")));
                } else if tag == "style" {
                    body.push_str(&format!("{{`{}`}}", content.replace('`', "\\`")));
                } else {
                    body.push_str(&jsx_text(content));
                }
            },
            Token::End(name) => {
                depth = depth.saturating_sub(1);
                body.push_str(&format!("</{name}>"));
            },
        }
    }
    let needs_fragment = roots > 1;
    let pretty = pretty_jsx(&body, needs_fragment);
    let name = if component_name.is_empty() {
        "Component".to_string()
    } else {
        component_name.to_string()
    };
    let indented: String = pretty
        .lines()
        .map(|l| format!("    {l}"))
        .collect::<Vec<_>>()
        .join("\n");
    let component = if typescript {
        format!(
            "import React from 'react';\n\nexport default function {name}(): JSX.Element \
             {{\n  return (\n{indented}\n  );\n}}\n"
        )
    } else {
        format!("export default function {name}() {{\n  return (\n{indented}\n  );\n}}\n")
    };
    notes.sort();
    notes.dedup();
    Ok(json!({
        "jsx": pretty,
        "component": component,
        "component_name": name,
        "wrapped_in_fragment": needs_fragment,
        "notes": notes
    }))
}

// ------------------------------------------------------------------- CSS --

#[derive(Debug, Clone, PartialEq)]
enum CssTok {
    Comment(String),
    Text(String), // selector / at-rule prelude / declaration text
    Open,
    Close,
    Semi,
}

fn css_tokenize(css: &str) -> Vec<CssTok> {
    let chars: Vec<char> = css.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    let mut buf = String::new();
    let flush = |buf: &mut String, out: &mut Vec<CssTok>| {
        let t = buf.trim();
        if !t.is_empty() {
            out.push(CssTok::Text(t.split_whitespace().collect::<Vec<_>>().join(" ")));
        }
        buf.clear();
    };
    while i < chars.len() {
        let c = chars[i];
        if c == '/' && chars.get(i + 1) == Some(&'*') {
            flush(&mut buf, &mut out);
            let start = i + 2;
            let mut j = start;
            while j + 1 < chars.len() && !(chars[j] == '*' && chars[j + 1] == '/') {
                j += 1;
            }
            out.push(CssTok::Comment(chars[start..j.min(chars.len())].iter().collect()));
            i = (j + 2).min(chars.len());
            continue;
        }
        if c == '"' || c == '\'' {
            let q = c;
            buf.push(c);
            i += 1;
            while i < chars.len() && chars[i] != q {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    buf.push(chars[i]);
                    i += 1;
                }
                buf.push(chars[i]);
                i += 1;
            }
            if i < chars.len() {
                buf.push(chars[i]);
                i += 1;
            }
            continue;
        }
        if c == '(' {
            // url(...) and friends: copy verbatim until matching paren.
            let mut depth = 0;
            while i < chars.len() {
                buf.push(chars[i]);
                if chars[i] == '(' {
                    depth += 1;
                } else if chars[i] == ')' {
                    depth -= 1;
                    if depth == 0 {
                        i += 1;
                        break;
                    }
                }
                i += 1;
            }
            continue;
        }
        match c {
            '{' => {
                flush(&mut buf, &mut out);
                out.push(CssTok::Open);
            },
            '}' => {
                flush(&mut buf, &mut out);
                out.push(CssTok::Close);
            },
            ';' => {
                flush(&mut buf, &mut out);
                out.push(CssTok::Semi);
            },
            _ => buf.push(c),
        }
        i += 1;
    }
    flush(&mut buf, &mut out);
    out
}

fn format_declaration(d: &str) -> String {
    match d.split_once(':') {
        Some((k, v)) if !k.trim().is_empty() && !k.trim_start().starts_with('@') => {
            let v = v
                .trim()
                .replace(" !important", "!important")
                .replace("!important", " !important");
            let v = split_top_level(&v, ',')
                .iter()
                .map(|p| p.trim())
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}: {};", k.trim(), v.trim())
        },
        _ => format!("{};", d.trim()),
    }
}

/// Splits on `sep` outside parentheses and quotes.
fn split_top_level(s: &str, sep: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut quote: Option<char> = None;
    let mut cur = String::new();
    for c in s.chars() {
        match quote {
            Some(q) => {
                cur.push(c);
                if c == q {
                    quote = None;
                }
                continue;
            },
            None => {
                if c == '"' || c == '\'' {
                    quote = Some(c);
                } else if c == '(' {
                    depth += 1;
                } else if c == ')' {
                    depth -= 1;
                } else if c == sep && depth == 0 {
                    out.push(std::mem::take(&mut cur));
                    continue;
                }
                cur.push(c);
            },
        }
    }
    out.push(cur);
    out
}

fn format_selector(s: &str) -> String {
    if s.starts_with('@') {
        return s.to_string();
    }
    s.split(',').map(str::trim).collect::<Vec<_>>().join(",\n")
}

/// Formats CSS with one declaration per line and nested indentation.
pub fn format_css(css: &str, indent_size: usize) -> EngineResult<Value> {
    engine_shared::error::require_text(css, "css")?;
    engine_shared::limits::check_size(css.len(), MAX, "css")?;
    let toks = css_tokenize(css);
    let unit = if indent_size == 0 {
        "\t".to_string()
    } else {
        " ".repeat(indent_size.min(8))
    };
    let mut out = String::new();
    let mut depth = 0usize;
    let mut i = 0;
    let mut rules = 0;
    let mut declarations = 0;
    let mut unbalanced = false;
    while i < toks.len() {
        match &toks[i] {
            CssTok::Comment(c) => {
                out.push_str(&unit.repeat(depth));
                out.push_str(&format!("/*{c}*/\n"));
            },
            CssTok::Text(t) => match toks.get(i + 1) {
                Some(CssTok::Open) => {
                    if !out.is_empty() && !out.ends_with("{\n") && depth == 0 {
                        out.push('\n');
                    }
                    for (k, line) in format_selector(t).lines().enumerate() {
                        out.push_str(&unit.repeat(depth));
                        out.push_str(line);
                        if k + 1 < format_selector(t).lines().count() {
                            out.push('\n');
                        }
                    }
                    out.push_str(" {\n");
                    depth += 1;
                    rules += 1;
                    i += 2;
                    continue;
                },
                _ => {
                    out.push_str(&unit.repeat(depth));
                    out.push_str(&format_declaration(t));
                    out.push('\n');
                    declarations += 1;
                    if matches!(toks.get(i + 1), Some(CssTok::Semi)) {
                        i += 1;
                    }
                },
            },
            CssTok::Open => {
                out.push_str(&unit.repeat(depth));
                out.push_str("{\n");
                depth += 1;
            },
            CssTok::Close => {
                if depth == 0 {
                    unbalanced = true;
                } else {
                    depth -= 1;
                }
                out.push_str(&unit.repeat(depth));
                out.push_str("}\n");
            },
            CssTok::Semi => {},
        }
        i += 1;
    }
    if depth > 0 {
        unbalanced = true;
    }
    Ok(json!({
        "formatted": out.trim_end().to_string() + "\n",
        "rules": rules,
        "declarations": declarations,
        "balanced": !unbalanced
    }))
}

/// Minifies CSS (comments, whitespace, trailing semicolons, zero units, short hex).
pub fn minify_css(css: &str) -> EngineResult<Value> {
    engine_shared::error::require_text(css, "css")?;
    engine_shared::limits::check_size(css.len(), MAX, "css")?;
    let toks = css_tokenize(css);
    let mut out = String::with_capacity(css.len());
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            CssTok::Comment(c) => {
                if c.starts_with('!') {
                    out.push_str(&format!("/*{c}*/"));
                }
            },
            CssTok::Text(t) => {
                let next_open = matches!(toks.get(i + 1), Some(CssTok::Open));
                if next_open {
                    let sel = if t.starts_with('@') {
                        t.to_string()
                    } else {
                        t.split(',').map(str::trim).collect::<Vec<_>>().join(",")
                    };
                    out.push_str(&tighten_selector(&sel));
                } else {
                    out.push_str(&minify_declaration(t));
                    let next_close = matches!(toks.get(i + 1), Some(CssTok::Close))
                        || matches!(
                            (toks.get(i + 1), toks.get(i + 2)),
                            (Some(CssTok::Semi), Some(CssTok::Close))
                        )
                        || i + 1 >= toks.len();
                    if !next_close {
                        out.push(';');
                    }
                    if matches!(toks.get(i + 1), Some(CssTok::Semi)) {
                        i += 1;
                    }
                }
            },
            CssTok::Open => out.push('{'),
            CssTok::Close => out.push('}'),
            CssTok::Semi => {},
        }
        i += 1;
    }
    let out = out.trim().to_string();
    // Drop empty rules.
    let mut cleaned = out;
    loop {
        let before = cleaned.len();
        cleaned = remove_empty_rules(&cleaned);
        if cleaned.len() == before {
            break;
        }
    }
    Ok(json!({
        "minified": cleaned,
        "input_length": css.len(),
        "output_length": cleaned.len(),
        "saved_percent": if css.is_empty() { 0.0 } else { ((1.0 - cleaned.len() as f64 / css.len() as f64) * 1000.0).round() / 10.0 }
    }))
}

fn remove_empty_rules(css: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = css.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '{' && chars.get(i + 1) == Some(&'}') {
            // remove the selector before it
            while let Some(last) = out.chars().last() {
                if last == '}' || last == ';' || last == '{' {
                    break;
                }
                out.pop();
            }
            i += 2;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn tighten_selector(sel: &str) -> String {
    let mut out = String::new();
    let mut prev_space = false;
    for c in sel.chars() {
        if c == ' ' {
            prev_space = true;
            continue;
        }
        if prev_space
            && !matches!(c, '>' | '+' | '~' | ',')
            && !out.ends_with(['>', '+', '~', ','])
            && !out.is_empty()
        {
            out.push(' ');
        }
        prev_space = false;
        out.push(c);
    }
    out
}

fn minify_declaration(d: &str) -> String {
    let Some((k, v)) = d.split_once(':') else {
        return d.replace(' ', "");
    };
    let k = k.trim();
    let mut v = v.trim().replace(" !important", "!important").replace(", ", ",");
    // Collapse spaces around operators inside values.
    v = v.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut parts: Vec<String> = Vec::new();
    for tok in v.split(' ') {
        let mut t = tok.to_string();
        let lower = t.to_ascii_lowercase();
        if matches!(lower.as_str(), "0px" | "0em" | "0rem" | "0pt" | "0%" | "0vh" | "0vw")
            && !k.contains("flex")
        {
            t = "0".into();
        }
        if let Some(num) = t.strip_prefix("0.") {
            t = format!(".{num}");
        }
        if let Some(num) = t.strip_prefix("-0.") {
            t = format!("-.{num}");
        }
        if t.len() == 7 && t.starts_with('#') {
            let b: Vec<char> = t.chars().collect();
            if b[1] == b[2] && b[3] == b[4] && b[5] == b[6] {
                t = format!("#{}{}{}", b[1], b[3], b[5]);
            }
        }
        parts.push(t);
    }
    format!("{k}:{}", parts.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_format_and_minify() {
        let f = format_html(
            "<!DOCTYPE html><html><body><div class=\"a\"><p>Hello \
             <b>world</b></p><ul><li>one</li><li>two</li></ul><script>\n  var x = \
             1;\n</script></div><br></body></html>",
            2,
            80,
        )
        .unwrap();
        let s = f["formatted"].as_str().unwrap();
        assert!(s.contains("      <p>Hello <b>world</b></p>\n"), "{s}");
        assert!(s.contains("        <li>one</li>\n"), "{s}");
        assert!(s.contains("      <script>\n        var x = 1;\n      </script>"), "{s}");
        assert_eq!(f["unclosed_tags"].as_array().unwrap().len(), 0);
        let m = minify_html(
            "<div   class=\"a  b\">\n   <p> Hello   <b>world</b> </p>\n  <!-- c -->\n <input \
             disabled=\"disabled\" value=\"x y\"></div>",
            true,
            true,
        )
        .unwrap();
        assert_eq!(
            m["minified"],
            "<div class=\"a b\"><p>Hello <b>world</b></p><input disabled value=\"x y\"></div>"
        );
        let pre = minify_html("<pre>\n  keep   this\n</pre>", true, true).unwrap();
        assert_eq!(pre["minified"], "<pre>\n  keep   this\n</pre>");
        assert!(format_html("", 2, 80).is_err());
    }

    #[test]
    fn jsx() {
        let j = html_to_jsx(
            "<div class=\"box\" style=\"background-color: red; z-index: 2\" \
             onclick=\"go()\"><label for=\"x\">A {b}</label><input type=\"text\" \
             tabindex=\"1\"><!-- hi --><svg viewBox=\"0 0 1 1\" stroke-width=\"2\"></svg></div>",
            "Card",
            true,
        )
        .unwrap();
        let s = j["jsx"].as_str().unwrap();
        assert!(s.contains("className=\"box\""), "{s}");
        assert!(s.contains("style={{ backgroundColor: 'red', zIndex: 2 }}"), "{s}");
        assert!(s.contains("onClick={() => { go() }}"), "{s}");
        assert!(s.contains("htmlFor=\"x\""), "{s}");
        assert!(s.contains("{'{'}b{'}'}"), "{s}");
        assert!(s.contains("tabIndex=\"1\" />"), "{s}");
        assert!(s.contains("{/* hi */}"), "{s}");
        assert!(s.contains("strokeWidth=\"2\""), "{s}");
        assert!(j["component"]
            .as_str()
            .unwrap()
            .contains("export default function Card(): JSX.Element"));
        assert_eq!(html_to_jsx("<a></a><b></b>", "", false).unwrap()["wrapped_in_fragment"], true);
    }

    #[test]
    fn css() {
        let f = format_css(
            "a,b{color:red;background:url(data:image/png;base64,AAA) no-repeat}@media \
             (max-width:600px){.x{margin:0 auto!important}}",
            2,
        )
        .unwrap();
        assert_eq!(
            f["formatted"],
            "a,\nb {\n  color: red;\n  background: url(data:image/png;base64,AAA) \
             no-repeat;\n}\n\n@media (max-width:600px) {\n  .x {\n    margin: 0 auto \
             !important;\n  }\n}\n"
        );
        assert_eq!(f["balanced"], true);
        let m = minify_css(
            "/* c */ a , b { color : #aabbcc ; margin: 0px 0.5em; }\n.empty { }\n.k > .j { \
             padding: 0 ; }",
        )
        .unwrap();
        assert_eq!(m["minified"], "a,b{color:#abc;margin:0 .5em}.k>.j{padding:0}");
        assert_eq!(format_css("a{", 2).unwrap()["balanced"], false);
    }
}
