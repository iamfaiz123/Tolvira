//! Token-level JavaScript / TypeScript formatter and minifier.
//!
//! This is deliberately *not* a full ECMAScript parser: it tokenizes (strings,
//! template literals with nested `${}`, regex literals, comments, numbers,
//! identifiers, punctuators) and applies layout rules. It is safe for
//! minification (string/regex aware, ASI-hazard aware) and produces readable
//! output for formatting, but it does not validate syntax.

use engine_shared::{EngineError, EngineResult};
use serde_json::{json, Value};

const MAX: usize = 5 * 1024 * 1024;

/// Token classes produced by [`tokenize`].
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// Identifier.
    Ident,
    /// Reserved word.
    Keyword,
    /// Numeric literal.
    Number,
    /// String literal (quotes included).
    String,
    /// Template literal.
    Template,
    /// Regular-expression literal.
    Regex,
    /// Punctuation / operator.
    Punct,
    /// `//` comment.
    LineComment,
    /// `/* */` comment.
    BlockComment,
    /// Line break (significant for ASI heuristics).
    Newline,
}

/// A JavaScript token.
#[derive(Debug, Clone, PartialEq)]
pub struct Tok {
    /// Token class.
    pub kind: Kind,
    /// Source text of the token.
    pub text: String,
}

const KEYWORDS: &[&str] = &[
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "let",
    "new",
    "null",
    "of",
    "return",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
    "async",
    "static",
    "get",
    "set",
    "interface",
    "type",
    "implements",
    "public",
    "private",
    "protected",
    "readonly",
    "declare",
    "namespace",
    "abstract",
    "as",
    "from",
    "keyof",
    "satisfies",
];

fn regex_allowed(prev: Option<&Tok>) -> bool {
    match prev {
        None => true,
        Some(t) => match t.kind {
            Kind::Number | Kind::String | Kind::Template | Kind::Regex | Kind::Ident => false,
            Kind::Keyword => {
                !matches!(t.text.as_str(), "this" | "super" | "null" | "true" | "false")
            },
            Kind::Punct => !matches!(t.text.as_str(), ")" | "]" | "}" | "++" | "--"),
            _ => true,
        },
    }
}

/// Tokenises JavaScript/TypeScript source (regex vs. division disambiguated heuristically).
pub fn tokenize(src: &str) -> Vec<Tok> {
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out: Vec<Tok> = Vec::new();
    let puncts3 = ["===", "!==", "**=", "<<=", ">>=", ">>>", "...", "&&=", "||=", "??="];
    let puncts2 = [
        "=>", "==", "!=", "<=", ">=", "&&", "||", "??", "?.", "++", "--", "+=", "-=", "*=", "/=",
        "%=", "&=", "|=", "^=", "<<", ">>", "**",
    ];
    while i < chars.len() {
        let c = chars[i];
        if c == '\n' {
            out.push(Tok { kind: Kind::Newline, text: "\n".into() });
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            let start = i;
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            out.push(Tok {
                kind: Kind::LineComment,
                text: chars[start..i].iter().collect(),
            });
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'*') {
            let start = i;
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(chars.len());
            out.push(Tok {
                kind: Kind::BlockComment,
                text: chars[start..i].iter().collect(),
            });
            continue;
        }
        if c == '"' || c == '\'' {
            let start = i;
            i += 1;
            while i < chars.len() && chars[i] != c && chars[i] != '\n' {
                if chars[i] == '\\' {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(chars.len());
            out.push(Tok {
                kind: Kind::String,
                text: chars[start..i].iter().collect(),
            });
            continue;
        }
        if c == '`' {
            let start = i;
            i += 1;
            let mut depth = 0;
            while i < chars.len() {
                if chars[i] == '\\' {
                    i += 2;
                    continue;
                }
                if depth == 0 && chars[i] == '`' {
                    i += 1;
                    break;
                }
                if chars[i] == '$' && chars.get(i + 1) == Some(&'{') {
                    depth += 1;
                    i += 2;
                    continue;
                }
                if depth > 0 && chars[i] == '}' {
                    depth -= 1;
                }
                if depth > 0 && chars[i] == '{' {
                    depth += 1;
                }
                i += 1;
            }
            out.push(Tok {
                kind: Kind::Template,
                text: chars[start..i.min(chars.len())].iter().collect(),
            });
            continue;
        }
        if c == '/'
            && regex_allowed(out.iter().rev().find(|t| {
                t.kind != Kind::Newline
                    && t.kind != Kind::LineComment
                    && t.kind != Kind::BlockComment
            }))
        {
            let start = i;
            i += 1;
            let mut in_class = false;
            let mut ok = false;
            while i < chars.len() && chars[i] != '\n' {
                if chars[i] == '\\' {
                    i += 2;
                    continue;
                }
                if chars[i] == '[' {
                    in_class = true;
                } else if chars[i] == ']' {
                    in_class = false;
                } else if chars[i] == '/' && !in_class {
                    i += 1;
                    ok = true;
                    break;
                }
                i += 1;
            }
            if ok {
                while i < chars.len() && chars[i].is_ascii_alphabetic() {
                    i += 1;
                }
                out.push(Tok {
                    kind: Kind::Regex,
                    text: chars[start..i].iter().collect(),
                });
                continue;
            }
            i = start;
        }
        if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(char::is_ascii_digit)) {
            let start = i;
            i += 1;
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric()
                    || chars[i] == '.'
                    || chars[i] == '_'
                    || ((chars[i] == '+' || chars[i] == '-')
                        && matches!(chars[i - 1], 'e' | 'E')
                        && !chars[start..i].iter().collect::<String>().starts_with("0x")))
            {
                i += 1;
            }
            out.push(Tok {
                kind: Kind::Number,
                text: chars[start..i].iter().collect(),
            });
            continue;
        }
        if c.is_alphabetic() || c == '_' || c == '$' || c == '#' || c == '@' {
            let start = i;
            i += 1;
            while i < chars.len()
                && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '$')
            {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            let kind = if KEYWORDS.contains(&text.as_str()) {
                Kind::Keyword
            } else {
                Kind::Ident
            };
            out.push(Tok { kind, text });
            continue;
        }
        let rest: String = chars[i..(i + 3).min(chars.len())].iter().collect();
        let p = puncts3
            .iter()
            .find(|p| rest.starts_with(*p))
            .map(|p| p.len())
            .or_else(|| puncts2.iter().find(|p| rest.starts_with(*p)).map(|p| p.len()))
            .unwrap_or(1);
        out.push(Tok {
            kind: Kind::Punct,
            text: chars[i..i + p].iter().collect(),
        });
        i += p;
    }
    out
}

fn check(src: &str) -> EngineResult<()> {
    engine_shared::error::require_text(src, "source")?;
    engine_shared::limits::check_size(src.len(), MAX, "source")
}

fn is_word(t: &Tok) -> bool {
    matches!(t.kind, Kind::Ident | Kind::Keyword | Kind::Number)
}

/// Minifies by removing comments and whitespace while preserving semantics
/// (keeps a newline where automatic semicolon insertion might be relied on).
pub fn minify(src: &str, keep_license: bool) -> EngineResult<Value> {
    check(src)?;
    let toks = tokenize(src);
    let mut out = String::with_capacity(src.len() / 2);
    let mut prev: Option<&Tok> = None;
    let mut pending_newline = false;
    for t in &toks {
        match t.kind {
            Kind::Newline => {
                pending_newline = true;
                continue;
            },
            Kind::LineComment => continue,
            Kind::BlockComment => {
                if keep_license
                    && (t.text.starts_with("/*!")
                        || t.text.contains("@license")
                        || t.text.contains("@preserve"))
                {
                    out.push_str(&t.text);
                    out.push('\n');
                }
                continue;
            },
            _ => {},
        }
        if let Some(p) = prev {
            let hazard = pending_newline && {
                let p_ends_expr = is_word(p)
                    || matches!(p.kind, Kind::String | Kind::Template | Kind::Regex)
                    || matches!(p.text.as_str(), ")" | "]" | "}" | "++" | "--");
                let n_starts_expr = is_word(t)
                    || matches!(t.kind, Kind::String | Kind::Template | Kind::Regex)
                    || matches!(
                        t.text.as_str(),
                        "(" | "[" | "+" | "-" | "/" | "`" | "++" | "--" | "!" | "~"
                    );
                p_ends_expr && n_starts_expr && !matches!(p.text.as_str(), ";" | "{" | "," | "(")
            };
            if hazard {
                out.push('\n');
            } else if (is_word(p) && is_word(t))
                || (p.text == "+" && (t.text == "+" || t.text == "++"))
                || (p.text == "-" && (t.text == "-" || t.text == "--"))
                || (is_word(p)
                    && (t.kind == Kind::Regex || t.kind == Kind::String && p.kind == Kind::Keyword))
                || (p.kind == Kind::Keyword && t.kind == Kind::Template)
                || (matches!(p.kind, Kind::Keyword)
                    && matches!(t.text.as_str(), "(" | "[" | "{")
                    && matches!(
                        p.text.as_str(),
                        "return"
                            | "typeof"
                            | "case"
                            | "in"
                            | "of"
                            | "instanceof"
                            | "await"
                            | "yield"
                            | "delete"
                            | "void"
                            | "throw"
                            | "new"
                            | "else"
                            | "do"
                    )
                    && false)
                || (p.kind == Kind::Keyword
                    && matches!(
                        p.text.as_str(),
                        "return"
                            | "typeof"
                            | "case"
                            | "in"
                            | "of"
                            | "instanceof"
                            | "await"
                            | "yield"
                            | "delete"
                            | "void"
                            | "throw"
                            | "else"
                            | "do"
                            | "new"
                            | "export"
                            | "import"
                            | "extends"
                            | "as"
                            | "from"
                            | "default"
                    )
                    && !matches!(t.text.as_str(), ";" | "," | ")" | "]" | "}" | "." | ":"))
            {
                out.push(' ');
            }
        }
        // `;` directly before `}` is redundant.
        if t.text == ";" {
            let next = toks.iter().skip_while(|x| !std::ptr::eq(*x, t)).skip(1).find(|x| {
                !matches!(x.kind, Kind::Newline | Kind::LineComment | Kind::BlockComment)
            });
            if next.map(|n| n.text == "}").unwrap_or(true) {
                prev = Some(t);
                pending_newline = false;
                continue;
            }
        }
        out.push_str(&t.text);
        prev = Some(t);
        pending_newline = false;
    }
    let out = out.trim().to_string();
    Ok(json!({
        "minified": out,
        "input_length": src.len(),
        "output_length": out.len(),
        "saved_percent": if src.is_empty() { 0.0 } else { ((1.0 - out.len() as f64 / src.len() as f64) * 1000.0).round() / 10.0 },
        "note": "Token-level minifier: removes comments/whitespace; does not rename identifiers \
                 or fold constants"
    }))
}

/// Formats JavaScript / TypeScript source with consistent indentation and spacing.
pub fn format(
    src: &str,
    indent_size: usize,
    semicolons: bool,
    single_quotes: bool,
) -> EngineResult<Value> {
    check(src)?;
    let toks: Vec<Tok> = tokenize(src);
    let unit = if indent_size == 0 {
        "\t".to_string()
    } else {
        " ".repeat(indent_size.min(8))
    };
    let mut out = String::new();
    let mut depth: usize = 0;
    let mut paren_depth: usize = 0;
    let mut at_line_start = true;
    let mut prev: Option<&Tok> = None;
    let mut blank_pending = false;
    let mut i = 0;
    let newline = |out: &mut String, depth: usize, unit: &str, at_line_start: &mut bool| {
        while out.ends_with(' ') {
            out.pop();
        }
        out.push('\n');
        out.push_str(&unit.repeat(depth));
        *at_line_start = true;
    };
    let mut case_depth: Vec<usize> = Vec::new();
    let mut brackets: Vec<char> = Vec::new();
    while i < toks.len() {
        let t = &toks[i];
        let next = toks.iter().skip(i + 1).find(|x| x.kind != Kind::Newline);
        match t.kind {
            Kind::Newline => {
                // Preserve at most one blank line.
                if matches!(toks.get(i + 1), Some(Tok { kind: Kind::Newline, .. }))
                    && !at_line_start
                {
                    blank_pending = true;
                }
                i += 1;
                continue;
            },
            Kind::LineComment => {
                if !at_line_start {
                    out.push(' ');
                }
                out.push_str(t.text.trim_end());
                newline(&mut out, depth, &unit, &mut at_line_start);
                prev = Some(t);
                i += 1;
                continue;
            },
            Kind::BlockComment => {
                if !at_line_start {
                    out.push(' ');
                }
                let lines: Vec<&str> = t.text.lines().collect();
                for (k, l) in lines.iter().enumerate() {
                    if k > 0 {
                        newline(&mut out, depth, &unit, &mut at_line_start);
                        out.push(' ');
                    }
                    out.push_str(l.trim());
                }
                if t.text.contains('\n') {
                    newline(&mut out, depth, &unit, &mut at_line_start);
                } else {
                    out.push(' ');
                }
                prev = Some(t);
                i += 1;
                continue;
            },
            _ => {},
        }
        if blank_pending && !at_line_start {
            blank_pending = false;
        } else if blank_pending && at_line_start && t.text != "}" {
            // insert blank line
            while out.ends_with(' ') {
                out.pop();
            }
            out.push('\n');
            out.push_str(&unit.repeat(depth));
            blank_pending = false;
        } else {
            blank_pending = false;
        }
        let mut text = t.text.clone();
        if t.kind == Kind::String && t.text.len() >= 2 {
            let q = t.text.chars().next().unwrap_or('"');
            let inner = &t.text[1..t.text.len() - 1];
            let want = if single_quotes { '\'' } else { '"' };
            if q != want && !inner.contains(want) {
                text = format!("{want}{inner}{want}");
            }
        }
        match text.as_str() {
            "{" => {
                if !at_line_start
                    && !out.ends_with(' ')
                    && !out.ends_with('(')
                    && !out.ends_with('[')
                {
                    out.push(' ');
                }
                out.push('{');
                brackets.push('{');
                // Empty block stays inline.
                if next.map(|n| n.text == "}").unwrap_or(false) {
                    brackets.pop();
                    out.push('}');
                    i += 2;
                    while matches!(toks.get(i), Some(Tok { kind: Kind::Newline, .. })) {
                        i += 1;
                    }
                    prev = Some(&toks[i - 1]);
                    at_line_start = false;
                    continue;
                }
                depth += 1;
                newline(&mut out, depth, &unit, &mut at_line_start);
            },
            "}" => {
                brackets.pop();
                depth = depth.saturating_sub(1);
                if case_depth.last() == Some(&depth) {
                    case_depth.pop();
                }
                if !at_line_start {
                    newline(&mut out, depth, &unit, &mut at_line_start);
                } else {
                    // Re-indent current (empty) line.
                    while out.ends_with(' ') || out.ends_with('\t') {
                        out.pop();
                    }
                    out.push_str(&unit.repeat(depth));
                }
                out.push('}');
                at_line_start = false;
                let continues = next
                    .map(|n| {
                        matches!(
                            n.text.as_str(),
                            "else"
                                | "catch"
                                | "finally"
                                | "while"
                                | ","
                                | ")"
                                | ";"
                                | "."
                                | "]"
                                | ":"
                                | "?"
                                | "="
                                | "||"
                                | "&&"
                                | "("
                        )
                    })
                    .unwrap_or(false)
                    || (paren_depth > 0);
                if !continues {
                    newline(&mut out, depth, &unit, &mut at_line_start);
                }
            },
            ";" => {
                if paren_depth > 0 {
                    out.push_str("; ");
                } else {
                    if semicolons {
                        out.push(';');
                    }
                    newline(&mut out, depth, &unit, &mut at_line_start);
                }
            },
            "(" | "[" => {
                if !at_line_start {
                    let need_space = prev
                        .map(|p| {
                            p.kind == Kind::Keyword
                                && matches!(
                                    p.text.as_str(),
                                    "if" | "for"
                                        | "while"
                                        | "switch"
                                        | "catch"
                                        | "return"
                                        | "typeof"
                                        | "await"
                                        | "in"
                                        | "of"
                                        | "instanceof"
                                        | "case"
                                        | "yield"
                                        | "delete"
                                        | "void"
                                        | "throw"
                                        | "new"
                                        | "do"
                                        | "else"
                                )
                                || matches!(
                                    p.text.as_str(),
                                    "=" | ","
                                        | ":"
                                        | "=>"
                                        | "?"
                                        | "+"
                                        | "-"
                                        | "*"
                                        | "/"
                                        | "%"
                                        | "&&"
                                        | "||"
                                        | "??"
                                        | "=="
                                        | "==="
                                        | "!="
                                        | "!=="
                                        | "<"
                                        | ">"
                                        | "<="
                                        | ">="
                                        | "&"
                                        | "|"
                                        | "^"
                                        | "<<"
                                        | ">>"
                                        | "return"
                                )
                        })
                        .unwrap_or(false);
                    if need_space && !out.ends_with(' ') {
                        out.push(' ');
                    }
                }
                out.push_str(&text);
                brackets.push(text.chars().next().unwrap_or('('));
                paren_depth += 1;
                at_line_start = false;
            },
            ")" | "]" => {
                brackets.pop();
                paren_depth = paren_depth.saturating_sub(1);
                while out.ends_with(' ') {
                    out.pop();
                }
                out.push_str(&text);
                at_line_start = false;
            },
            "," => {
                while out.ends_with(' ') {
                    out.pop();
                }
                if brackets.last() == Some(&'{') {
                    out.push(',');
                    newline(&mut out, depth, &unit, &mut at_line_start);
                } else {
                    out.push_str(", ");
                    at_line_start = false;
                }
            },
            "." | "?." => {
                while out.ends_with(' ') {
                    out.pop();
                }
                out.push_str(&text);
                at_line_start = false;
            },
            "++" | "--" | "!" | "~" | "..." => {
                if !at_line_start
                    && prev
                        .map(|p| is_word(p) || matches!(p.text.as_str(), ")" | "]"))
                        .unwrap_or(false)
                    && matches!(text.as_str(), "++" | "--")
                {
                    // postfix
                } else if !at_line_start
                    && !out.ends_with(' ')
                    && !out.ends_with('(')
                    && !out.ends_with('[')
                {
                    out.push(' ');
                }
                out.push_str(&text);
                at_line_start = false;
            },
            ":" => {
                let in_case = case_depth.last() == Some(&depth.saturating_sub(0))
                    && prev
                        .map(|p| p.kind != Kind::Punct || p.text == ")" || p.text == "]")
                        .unwrap_or(false)
                    && toks[..i]
                        .iter()
                        .rev()
                        .take_while(|x| x.kind != Kind::Newline)
                        .any(|x| x.text == "case" || x.text == "default");
                while out.ends_with(' ') {
                    out.pop();
                }
                // Ternary `a ? b : c` keeps spaces; object/type colon does not lead with space.
                let ternary = toks[..i]
                    .iter()
                    .rev()
                    .take_while(|x| {
                        x.kind != Kind::Newline && x.text != "{" && x.text != "," && x.text != ";"
                    })
                    .any(|x| x.text == "?");
                if ternary {
                    out.push_str(" : ");
                } else {
                    out.push_str(": ");
                }
                at_line_start = false;
                if in_case {
                    while out.ends_with(' ') {
                        out.pop();
                    }
                    depth += 1;
                    newline(&mut out, depth, &unit, &mut at_line_start);
                }
            },
            "?" => {
                out.push_str(" ? ");
                at_line_start = false;
            },
            _ if t.kind == Kind::Punct => {
                // binary operators with spaces; unary minus/plus after operator or `(` without.
                let unary = matches!(text.as_str(), "+" | "-")
                    && prev
                        .map(|p| {
                            p.kind == Kind::Punct && !matches!(p.text.as_str(), ")" | "]" | "}")
                                || p.kind == Kind::Keyword
                                    && matches!(
                                        p.text.as_str(),
                                        "return" | "case" | "typeof" | "in" | "of"
                                    )
                        })
                        .unwrap_or(true);
                if unary {
                    if !at_line_start
                        && !out.ends_with(' ')
                        && !out.ends_with('(')
                        && !out.ends_with('[')
                    {
                        out.push(' ');
                    }
                    out.push_str(&text);
                } else {
                    while out.ends_with(' ') {
                        out.pop();
                    }
                    if !at_line_start {
                        out.push(' ');
                    }
                    out.push_str(&text);
                    out.push(' ');
                }
                at_line_start = false;
            },
            _ => {
                // words, strings, templates, regex
                if t.kind == Kind::Keyword
                    && (text == "case" || text == "default")
                    && next.map(|n| n.text != "(" && n.text != ":").unwrap_or(true)
                    || (text == "default" && next.map(|n| n.text == ":").unwrap_or(false))
                    || text == "case"
                {
                    // dedent if previous case body indented
                    if case_depth.last() == Some(&depth.saturating_sub(1)) {
                        depth -= 1;
                        while out.ends_with(' ') || out.ends_with('\t') {
                            out.pop();
                        }
                        if at_line_start {
                            out.push_str(&unit.repeat(depth));
                        }
                    }
                    if !case_depth.contains(&depth) {
                        case_depth.push(depth);
                    }
                }
                let after_sign = out.ends_with('-') || out.ends_with('+');
                let plain_gap = !out.ends_with(' ')
                    && !out.ends_with('(')
                    && !out.ends_with('[')
                    && !out.ends_with('.')
                    && !out.ends_with('!')
                    && !out.ends_with('~')
                    && !out.ends_with("...")
                    && !after_sign;
                let binary_sign_gap = after_sign
                    && prev
                        .map(|p| !(p.kind == Kind::Punct && (p.text == "-" || p.text == "+")))
                        .unwrap_or(false);
                if !at_line_start && (plain_gap || binary_sign_gap) {
                    out.push(' ');
                }
                if (text == "else" || text == "catch" || text == "finally") && out.ends_with('}') {
                    out.push(' ');
                }
                out.push_str(&text);
                at_line_start = false;
                // Keywords that introduce a statement and are followed by `(`: handled by `(` rule.
            },
        }
        prev = Some(t);
        i += 1;
    }
    let mut result = out.lines().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n");
    while result.contains("\n\n\n") {
        result = result.replace("\n\n\n", "\n\n");
    }
    let result = result.trim().to_string() + "\n";
    Ok(json!({
        "formatted": result,
        "lines": result.lines().count(),
        "note": "Token-level formatter (no AST); review output for complex constructs"
    }))
}

/// Shallow inspection used by both tools: token stats and brace balance.
pub fn inspect(src: &str) -> EngineResult<Value> {
    check(src)?;
    let toks = tokenize(src);
    let (mut braces, mut parens, mut brackets) = (0i64, 0i64, 0i64);
    for t in &toks {
        match t.text.as_str() {
            "{" => braces += 1,
            "}" => braces -= 1,
            "(" => parens += 1,
            ")" => parens -= 1,
            "[" => brackets += 1,
            "]" => brackets -= 1,
            _ => {},
        }
    }
    let balance = json!({ "braces": braces, "parens": parens, "brackets": brackets });
    let ts_hints = toks.iter().any(|t| {
        t.kind == Kind::Keyword
            && matches!(
                t.text.as_str(),
                "interface" | "implements" | "declare" | "namespace" | "readonly" | "satisfies"
            )
    }) || toks.windows(2).any(|w| {
        w[0].text == ":"
            && w[1].kind == Kind::Ident
            && matches!(
                w[1].text.as_str(),
                "string" | "number" | "boolean" | "any" | "unknown" | "void" | "never"
            )
    });
    Ok(json!({
        "tokens": toks.len(),
        "comments": toks.iter().filter(|t| matches!(t.kind, Kind::LineComment | Kind::BlockComment)).count(),
        "strings": toks.iter().filter(|t| matches!(t.kind, Kind::String | Kind::Template)).count(),
        "regex_literals": toks.iter().filter(|t| t.kind == Kind::Regex).count(),
        "balance": balance,
        "balanced": balance["braces"] == 0 && balance["parens"] == 0 && balance["brackets"] == 0,
        "looks_like_typescript": ts_hints,
        "uses_modules": toks.iter().any(|t| t.kind == Kind::Keyword && (t.text == "import" || t.text == "export"))
    }))
}

/// TypeScript → JavaScript needs a real parser to strip types safely.
pub fn ts_to_js_unsupported() -> EngineError {
    EngineError::unsupported(
        "TypeScript → JavaScript transpilation requires a full TypeScript parser (e.g. the \
         TypeScript compiler or swc running in the browser JavaScript runtime); the WASM \
         developer engine only provides token-level formatting/minification",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens() {
        let t = tokenize(
            "const re = /ab+c/gi; let s = `x ${a + `${b}`} y`; a = b / c / d; // hi\n/* block */ \
             x++",
        );
        assert!(t.iter().any(|x| x.kind == Kind::Regex && x.text == "/ab+c/gi"));
        assert!(t
            .iter()
            .any(|x| x.kind == Kind::Template && x.text == "`x ${a + `${b}`} y`"));
        assert_eq!(t.iter().filter(|x| x.kind == Kind::Regex).count(), 1);
        assert!(t.iter().any(|x| x.kind == Kind::LineComment));
        assert!(t.iter().any(|x| x.kind == Kind::BlockComment));
    }

    #[test]
    fn minify_safe() {
        let m = minify(
            "function add(a, b) {\n  // sum\n  return a + b;\n}\nlet x = add(1, \
             2)\n(function(){})()\nvar y = x\n++y\nif (a) b\nelse c",
            true,
        )
        .unwrap();
        let s = m["minified"].as_str().unwrap();
        assert!(
            s.starts_with(
                "function add(a,b){return a+b}\nlet x=add(1,2)\n(function(){})()\nvar y=x\n++y"
            ),
            "{s}"
        );
        assert!(s.contains("if(a)b\nelse c") || s.contains("if(a)b else c"), "{s}");
        let m = minify("/*! license */\nlet a = 1;", true).unwrap();
        assert!(m["minified"].as_str().unwrap().starts_with("/*! license */"));
        let m = minify("return typeof x === 'a' ? a : b", false).unwrap();
        assert_eq!(m["minified"], "return typeof x==='a'?a:b");
    }

    #[test]
    fn format_basic() {
        let f = format(
            "function f(a,b){if(a){return a+b}else{for(let i=0;i<3;i++){console.log(i)}}}\nconst \
             o={a:1,b:[1,2]};",
            2,
            true,
            false,
        )
        .unwrap();
        let s = f["formatted"].as_str().unwrap();
        assert_eq!(
            s,
            "function f(a, b) {\n  if (a) {\n    return a + b\n  } else {\n    for (let i = 0; i \
             < 3; i++) {\n      console.log(i)\n    }\n  }\n}\nconst o = {\n  a: 1,\n  b: [1, \
             2]\n};\n",
            "{s}"
        );
        let f = format(
            "const x = a ? -1 : b;\nswitch (k) { case 1: run(); break; default: stop() }",
            2,
            true,
            true,
        )
        .unwrap();
        let s = f["formatted"].as_str().unwrap();
        assert!(s.contains("const x = a ? -1 : b;"), "{s}");
        assert!(s.contains("  case 1:\n    run();\n    break;\n  default:\n    stop()"), "{s}");
        assert_eq!(
            inspect("let a: string = 'x'; interface I {}").unwrap()["looks_like_typescript"],
            true
        );
        assert!(format("", 2, true, false).is_err());
    }
}
