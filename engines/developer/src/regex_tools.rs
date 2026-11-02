//! Regex tester (fancy-regex: lookaround + backreferences) and explainer.

use engine_shared::{EngineError, EngineResult};
use fancy_regex::{Regex, RegexBuilder};
use serde_json::{json, Value};

const MAX_MATCHES: usize = 10_000;

/// Builds a regex from a pattern plus JS-style flag letters (`gimsxu`).
pub fn build(pattern: &str, flags: &str) -> EngineResult<Regex> {
    engine_shared::error::require_text(pattern, "pattern")?;
    if pattern.len() > 10_000 {
        return Err(EngineError::limit("Pattern is too long (10,000 character limit)"));
    }
    let mut prefix = String::new();
    for f in flags.chars() {
        match f {
            'i' => prefix.push('i'),
            'm' => prefix.push('m'),
            's' => prefix.push('s'),
            'x' => prefix.push('x'),
            'U' => prefix.push('U'),
            'g' | 'u' | 'y' | 'd' | 'v' => {},
            other => {
                return Err(EngineError::invalid(format!(
                    "Unknown flag `{other}` (supported: g i m s x u y)"
                )))
            },
        }
    }
    let full = if prefix.is_empty() {
        pattern.to_string()
    } else {
        format!("(?{prefix}){pattern}")
    };
    RegexBuilder::new(&full)
        .backtrack_limit(1_000_000)
        .build()
        .map_err(|e| {
            EngineError::invalid(format!("Invalid regex: {}", clean_error(&e.to_string())))
        })
}

fn clean_error(msg: &str) -> String {
    msg.lines()
        .filter(|l| !l.trim().is_empty())
        .map(str::trim)
        .collect::<Vec<_>>()
        .join(" ")
}

fn char_index(text: &str, byte: usize) -> usize {
    text[..byte].chars().count()
}

fn line_col(text: &str, byte: usize) -> (usize, usize) {
    let before = &text[..byte];
    let line = before.matches('\n').count() + 1;
    let col = before.rsplit('\n').next().map(|l| l.chars().count()).unwrap_or(0) + 1;
    (line, col)
}

/// Runs the pattern against the text: matches with groups, optional
/// replacement and split results.
pub fn test(
    pattern: &str,
    flags: &str,
    text: &str,
    replacement: Option<&str>,
    max_matches: usize,
) -> EngineResult<Value> {
    let re = build(pattern, flags)?;
    let global = flags.contains('g') || replacement.is_some();
    let limit = if max_matches == 0 {
        MAX_MATCHES
    } else {
        max_matches.min(MAX_MATCHES)
    };
    let names: Vec<Option<String>> = re.capture_names().map(|n| n.map(str::to_string)).collect();
    let mut matches = Vec::new();
    let mut truncated = false;
    for (i, caps) in re.captures_iter(text).enumerate() {
        let caps = caps.map_err(|e| {
            EngineError::limit(format!(
                "Regex evaluation aborted: {} (pattern is probably catastrophic backtracking)",
                clean_error(&e.to_string())
            ))
        })?;
        if i >= limit {
            truncated = true;
            break;
        }
        let Some(whole) = caps.get(0) else { continue };
        let (line, col) = line_col(text, whole.start());
        let groups: Vec<Value> = (1..caps.len())
            .map(|g| {
                let name = names.get(g).cloned().flatten();
                match caps.get(g) {
                    Some(m) => json!({
                        "index": g,
                        "name": name,
                        "value": m.as_str(),
                        "start": char_index(text, m.start()),
                        "end": char_index(text, m.end())
                    }),
                    None => json!({
                        "index": g,
                        "name": name,
                        "value": Value::Null,
                        "start": Value::Null,
                        "end": Value::Null
                    }),
                }
            })
            .collect();
        let named: serde_json::Map<String, Value> = names
            .iter()
            .enumerate()
            .filter_map(|(g, n)| {
                n.as_ref().map(|n| {
                    (
                        n.clone(),
                        caps.get(g)
                            .map(|m| Value::String(m.as_str().into()))
                            .unwrap_or(Value::Null),
                    )
                })
            })
            .collect();
        matches.push(json!({
            "index": i,
            "value": whole.as_str(),
            "start": char_index(text, whole.start()),
            "end": char_index(text, whole.end()),
            "byte_start": whole.start(),
            "byte_end": whole.end(),
            "line": line,
            "column": col,
            "groups": groups,
            "named": named
        }));
        if !global {
            break;
        }
    }
    let mut out = json!({
        "pattern": pattern,
        "flags": flags,
        "is_match": !matches.is_empty(),
        "match_count": matches.len(),
        "truncated": truncated,
        "matches": matches,
        "group_count": names.len().saturating_sub(1),
        "group_names": names.iter().skip(1).collect::<Vec<_>>(),
        "engine": "fancy-regex (Rust syntax + lookaround/backreferences)",
    });
    if let Some(rep) = replacement {
        let rust_rep = js_replacement_to_rust(rep);
        let replaced = if global {
            re.replace_all(text, rust_rep.as_str())
        } else {
            re.replace(text, rust_rep.as_str())
        };
        out["replaced"] = json!(replaced);
    }
    if text.len() <= 1_000_000 {
        let parts: Vec<&str> = {
            let mut parts = Vec::new();
            let mut last = 0;
            for m in re.find_iter(text).take(limit).flatten() {
                parts.push(&text[last..m.start()]);
                last = m.end();
            }
            parts.push(&text[last..]);
            parts
        };
        if parts.len() <= 1001 {
            out["split"] = json!(parts);
        }
    }
    // Highlight ranges for the UI (char offsets, non-overlapping).
    Ok(out)
}

/// Converts `$1`/`$<name>`/`$&` JS replacement syntax into Rust `${1}`/`${name}`/`${0}`.
pub fn js_replacement_to_rust(rep: &str) -> String {
    let chars: Vec<char> = rep.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '$' && i + 1 < chars.len() {
            let n = chars[i + 1];
            if n == '$' {
                out.push_str("$$");
                i += 2;
                continue;
            }
            if n == '&' {
                out.push_str("${0}");
                i += 2;
                continue;
            }
            if n == '<' {
                if let Some(end) = chars[i + 2..].iter().position(|c| *c == '>') {
                    let name: String = chars[i + 2..i + 2 + end].iter().collect();
                    out.push_str(&format!("${{{name}}}"));
                    i += 3 + end;
                    continue;
                }
            }
            if n.is_ascii_digit() {
                let mut j = i + 1;
                while j < chars.len() && chars[j].is_ascii_digit() && j - i <= 2 {
                    j += 1;
                }
                let num: String = chars[i + 1..j].iter().collect();
                out.push_str(&format!("${{{num}}}"));
                i = j;
                continue;
            }
            if n == '{' {
                out.push('$');
                i += 1;
                continue;
            }
        }
        if c == '$' {
            out.push_str("$$");
            i += 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

// -------------------------------------------------------------- explainer --

#[derive(Debug)]
struct Node {
    token: String,
    description: String,
    kind: &'static str,
    children: Vec<Node>,
}

impl Node {
    fn leaf(token: impl Into<String>, kind: &'static str, description: impl Into<String>) -> Node {
        Node {
            token: token.into(),
            description: description.into(),
            kind,
            children: vec![],
        }
    }
    fn to_json(&self) -> Value {
        let mut v =
            json!({ "token": self.token, "type": self.kind, "description": self.description });
        if !self.children.is_empty() {
            v["children"] = Value::Array(self.children.iter().map(Node::to_json).collect());
        }
        v
    }
    fn to_text(&self, depth: usize, out: &mut String) {
        out.push_str(&"  ".repeat(depth));
        out.push_str(&format!("{}  {}\n", self.token, self.description));
        for c in &self.children {
            c.to_text(depth + 1, out);
        }
    }
}

struct Explainer {
    chars: Vec<char>,
    pos: usize,
    group_index: usize,
}

impl Explainer {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }
    fn peek_at(&self, off: usize) -> Option<char> {
        self.chars.get(self.pos + off).copied()
    }
    fn take(&mut self) -> Option<char> {
        let c = self.peek();
        self.pos += 1;
        c
    }
    fn slice(&self, from: usize) -> String {
        self.chars[from..self.pos].iter().collect()
    }

    fn parse_alternation(&mut self, depth: usize) -> Vec<Node> {
        let mut branches: Vec<Vec<Node>> = Vec::new();
        let mut current: Vec<Node> = Vec::new();
        while let Some(c) = self.peek() {
            if c == ')' && depth > 0 {
                break;
            }
            if c == '|' {
                self.pos += 1;
                branches.push(std::mem::take(&mut current));
                continue;
            }
            let atom = self.parse_atom(depth);
            let atom = self.parse_quantifier(atom);
            current.push(atom);
        }
        if branches.is_empty() {
            return current;
        }
        branches.push(current);
        let n = branches.len();
        vec![Node {
            token: "|".into(),
            kind: "alternation",
            description: format!("Match one of {n} alternatives (tried left to right)"),
            children: branches
                .into_iter()
                .enumerate()
                .map(|(i, b)| Node {
                    token: format!("alternative {}", i + 1),
                    kind: "alternative",
                    description: String::new(),
                    children: b,
                })
                .collect(),
        }]
    }

    fn parse_quantifier(&mut self, atom: Node) -> Node {
        let start = self.pos;
        let desc = match self.peek() {
            Some('*') => {
                self.pos += 1;
                Some("zero or more times".to_string())
            },
            Some('+') => {
                self.pos += 1;
                Some("one or more times".to_string())
            },
            Some('?') => {
                self.pos += 1;
                Some("zero or one time (optional)".to_string())
            },
            Some('{') => {
                let save = self.pos;
                self.pos += 1;
                let mut body = String::new();
                while let Some(c) = self.peek() {
                    if c == '}' {
                        break;
                    }
                    body.push(c);
                    self.pos += 1;
                }
                if self.peek() == Some('}')
                    && !body.is_empty()
                    && body.chars().all(|c| c.is_ascii_digit() || c == ',')
                {
                    self.pos += 1;
                    let parts: Vec<&str> = body.split(',').collect();
                    Some(match parts.as_slice() {
                        [n] => format!("exactly {n} times"),
                        [n, ""] => format!("at least {n} times"),
                        [m, n] => format!("between {m} and {n} times"),
                        _ => "a repeated number of times".into(),
                    })
                } else {
                    self.pos = save;
                    None
                }
            },
            _ => None,
        };
        let Some(mut desc) = desc else { return atom };
        let mut kind = "quantifier";
        match self.peek() {
            Some('?') => {
                self.pos += 1;
                desc.push_str(", as few as possible (lazy)");
                kind = "quantifier-lazy";
            },
            Some('+') => {
                self.pos += 1;
                desc.push_str(", possessively (no backtracking)");
                kind = "quantifier-possessive";
            },
            _ => desc.push_str(", as many as possible (greedy)"),
        }
        let q = self.slice(start);
        Node {
            token: format!("{}{}", atom.token, q),
            kind,
            description: format!("Repeat the preceding element {desc}"),
            children: vec![atom],
        }
    }

    fn parse_atom(&mut self, depth: usize) -> Node {
        let start = self.pos;
        // Callers only invoke `parse_atom` after `peek()` returned a character.
        let Some(c) = self.take() else {
            return Node::leaf("", "literal", "end of pattern");
        };
        match c {
            '^' => Node::leaf(
                "^",
                "anchor",
                "Assert position at the start of the string (or line with the m flag)",
            ),
            '$' => Node::leaf(
                "$",
                "anchor",
                "Assert position at the end of the string (or line with the m flag)",
            ),
            '.' => Node::leaf(
                ".",
                "wildcard",
                "Match any character except a line break (any character with the s flag)",
            ),
            '(' => self.parse_group(depth),
            '[' => self.parse_class(),
            '\\' => self.parse_escape(start, false),
            ')' => Node::leaf(")", "error", "Unbalanced closing parenthesis"),
            other => Node::leaf(
                other.to_string(),
                "literal",
                format!("Match the character `{other}` literally"),
            ),
        }
    }

    fn parse_group(&mut self, depth: usize) -> Node {
        let start = self.pos - 1;
        let (kind, desc, capture): (&'static str, String, bool) = if self.peek() == Some('?') {
            self.pos += 1;
            match (self.peek(), self.peek_at(1)) {
                (Some(':'), _) => {
                    self.pos += 1;
                    ("group-noncapturing", "Non-capturing group".into(), false)
                },
                (Some('='), _) => {
                    self.pos += 1;
                    (
                        "lookahead",
                        "Positive lookahead: assert that what follows matches, without consuming it"
                            .into(),
                        false,
                    )
                },
                (Some('!'), _) => {
                    self.pos += 1;
                    (
                        "lookahead-negative",
                        "Negative lookahead: assert that what follows does NOT match".into(),
                        false,
                    )
                },
                (Some('<'), Some('=')) => {
                    self.pos += 2;
                    (
                        "lookbehind",
                        "Positive lookbehind: assert that what precedes matches".into(),
                        false,
                    )
                },
                (Some('<'), Some('!')) => {
                    self.pos += 2;
                    (
                        "lookbehind-negative",
                        "Negative lookbehind: assert that what precedes does NOT match".into(),
                        false,
                    )
                },
                (Some('<'), _) | (Some('P'), Some('<')) => {
                    if self.peek() == Some('P') {
                        self.pos += 1;
                    }
                    self.pos += 1;
                    let mut name = String::new();
                    while let Some(c) = self.peek() {
                        self.pos += 1;
                        if c == '>' {
                            break;
                        }
                        name.push(c);
                    }
                    self.group_index += 1;
                    (
                        "group-named",
                        format!("Named capturing group `{name}` (group {})", self.group_index),
                        true,
                    )
                },
                (Some('>'), _) => {
                    self.pos += 1;
                    (
                        "group-atomic",
                        "Atomic group: once matched, never backtracks into it".into(),
                        false,
                    )
                },
                _ => {
                    // Inline flags (?i) or (?i:...)
                    let mut flags = String::new();
                    while let Some(c) = self.peek() {
                        if c == ')' || c == ':' {
                            break;
                        }
                        flags.push(c);
                        self.pos += 1;
                    }
                    let fdesc = flags
                        .chars()
                        .map(|f| match f {
                            'i' => "case-insensitive",
                            'm' => "multi-line anchors",
                            's' => "dot matches newline",
                            'x' => "ignore whitespace",
                            'U' => "swap greediness",
                            '-' => "turn off:",
                            _ => "unknown flag",
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    if self.peek() == Some(')') {
                        self.pos += 1;
                        return Node::leaf(
                            self.slice(start),
                            "flags",
                            format!("Set inline flags for the rest of the pattern ({fdesc})"),
                        );
                    }
                    self.pos += 1; // ':'
                    ("group-flags", format!("Non-capturing group with flags ({fdesc})"), false)
                },
            }
        } else {
            self.group_index += 1;
            ("group", format!("Capturing group {}", self.group_index), true)
        };
        let children = self.parse_alternation(depth + 1);
        let closed = self.peek() == Some(')');
        if closed {
            self.pos += 1;
        }
        let _ = capture;
        Node {
            token: self.slice(start),
            kind,
            description: if closed {
                desc
            } else {
                format!("{desc} (missing closing parenthesis)")
            },
            children,
        }
    }

    fn parse_class(&mut self) -> Node {
        let start = self.pos - 1;
        let negated = self.peek() == Some('^');
        if negated {
            self.pos += 1;
        }
        let mut items: Vec<Node> = Vec::new();
        let mut first = true;
        while let Some(c) = self.peek() {
            if c == ']' && !first {
                break;
            }
            first = false;
            let item_start = self.pos;
            self.pos += 1;
            let node = if c == '\\' {
                self.parse_escape(item_start, true)
            } else if c == '[' && self.peek() == Some(':') {
                let mut name = String::new();
                self.pos += 1;
                while let Some(ch) = self.peek() {
                    self.pos += 1;
                    if ch == ':' {
                        break;
                    }
                    name.push(ch);
                }
                if self.peek() == Some(']') {
                    self.pos += 1;
                }
                Node::leaf(self.slice(item_start), "posix-class", format!("POSIX class `{name}`"))
            } else {
                Node::leaf(c.to_string(), "literal", format!("`{c}`"))
            };
            // Range a-z
            if self.peek() == Some('-')
                && self.peek_at(1).is_some_and(|n| n != ']')
                && node.kind == "literal"
            {
                self.pos += 1;
                let hi_start = self.pos;
                let hi = self.take().unwrap_or(']');
                let hi_node = if hi == '\\' {
                    self.parse_escape(hi_start, true)
                } else {
                    Node::leaf(hi.to_string(), "literal", String::new())
                };
                items.push(Node::leaf(
                    format!("{}-{}", node.token, hi_node.token),
                    "range",
                    format!("Any character in the range `{}` to `{}`", node.token, hi_node.token),
                ));
                continue;
            }
            items.push(node);
        }
        let closed = self.peek() == Some(']');
        if closed {
            self.pos += 1;
        }
        let summary = items.iter().map(|i| i.token.as_str()).collect::<Vec<_>>().join(", ");
        Node {
            token: self.slice(start),
            kind: if negated { "class-negated" } else { "class" },
            description: format!(
                "Match one character {} the set: {}{}",
                if negated { "NOT in" } else { "in" },
                summary,
                if closed { "" } else { " (missing closing bracket)" }
            ),
            children: items,
        }
    }

    fn parse_escape(&mut self, start: usize, in_class: bool) -> Node {
        let Some(c) = self.take() else {
            return Node::leaf("\\", "error", "Trailing backslash");
        };
        let (kind, desc): (&'static str, String) = match c {
            'd' => ("class-shorthand", "Any digit (0-9)".into()),
            'D' => ("class-shorthand", "Any non-digit".into()),
            'w' => ("class-shorthand", "Any word character (letters, digits, underscore)".into()),
            'W' => ("class-shorthand", "Any non-word character".into()),
            's' => ("class-shorthand", "Any whitespace character (space, tab, line break)".into()),
            'S' => ("class-shorthand", "Any non-whitespace character".into()),
            'b' if !in_class => {
                ("anchor", "Word boundary (between a word and a non-word character)".into())
            },
            'b' => ("literal", "Backspace character".into()),
            'B' => ("anchor", "Not a word boundary".into()),
            'A' => ("anchor", "Start of the string".into()),
            'z' | 'Z' => ("anchor", "End of the string".into()),
            'n' => ("literal", "Line feed (newline)".into()),
            'r' => ("literal", "Carriage return".into()),
            't' => ("literal", "Tab".into()),
            'f' => ("literal", "Form feed".into()),
            'v' => ("literal", "Vertical tab".into()),
            '0' => ("literal", "NUL character".into()),
            'x' => {
                let mut hex = String::new();
                if self.peek() == Some('{') {
                    self.pos += 1;
                    while let Some(h) = self.peek() {
                        self.pos += 1;
                        if h == '}' {
                            break;
                        }
                        hex.push(h);
                    }
                } else {
                    for _ in 0..2 {
                        if let Some(h) = self.peek().filter(char::is_ascii_hexdigit) {
                            hex.push(h);
                            self.pos += 1;
                        }
                    }
                }
                (
                    "literal",
                    format!(
                        "Character with hex code {hex}{}",
                        u32::from_str_radix(&hex, 16)
                            .ok()
                            .and_then(char::from_u32)
                            .map(|ch| format!(" (`{ch}`)"))
                            .unwrap_or_default()
                    ),
                )
            },
            'u' => {
                let mut hex = String::new();
                if self.peek() == Some('{') {
                    self.pos += 1;
                    while let Some(h) = self.peek() {
                        self.pos += 1;
                        if h == '}' {
                            break;
                        }
                        hex.push(h);
                    }
                } else {
                    for _ in 0..4 {
                        if let Some(h) = self.peek().filter(char::is_ascii_hexdigit) {
                            hex.push(h);
                            self.pos += 1;
                        }
                    }
                }
                (
                    "literal",
                    format!(
                        "Unicode character U+{}{}",
                        hex.to_uppercase(),
                        u32::from_str_radix(&hex, 16)
                            .ok()
                            .and_then(char::from_u32)
                            .map(|ch| format!(" (`{ch}`)"))
                            .unwrap_or_default()
                    ),
                )
            },
            'p' | 'P' => {
                let mut name = String::new();
                if self.peek() == Some('{') {
                    self.pos += 1;
                    while let Some(h) = self.peek() {
                        self.pos += 1;
                        if h == '}' {
                            break;
                        }
                        name.push(h);
                    }
                } else if let Some(n) = self.take() {
                    name.push(n);
                }
                (
                    "unicode-class",
                    format!(
                        "Any character {} the Unicode property/category `{name}`",
                        if c == 'p' { "with" } else { "WITHOUT" }
                    ),
                )
            },
            'k' => {
                let mut name = String::new();
                if self.peek() == Some('<') {
                    self.pos += 1;
                    while let Some(h) = self.peek() {
                        self.pos += 1;
                        if h == '>' {
                            break;
                        }
                        name.push(h);
                    }
                }
                ("backreference", format!("Match the same text as the named group `{name}`"))
            },
            d if d.is_ascii_digit() && !in_class => {
                let mut num = d.to_string();
                while let Some(n) = self.peek().filter(char::is_ascii_digit) {
                    num.push(n);
                    self.pos += 1;
                }
                ("backreference", format!("Match the same text as capturing group {num}"))
            },
            other => ("literal", format!("Match the character `{other}` literally (escaped)")),
        };
        Node::leaf(self.slice(start), kind, desc)
    }
}

/// Produces a structured, human-readable explanation of a pattern.
pub fn explain(pattern: &str, flags: &str) -> EngineResult<Value> {
    engine_shared::error::require_text(pattern, "pattern")?;
    if pattern.len() > 5_000 {
        return Err(EngineError::limit("Pattern is too long (5,000 character limit)"));
    }
    let compile_error = build(pattern, flags).err().map(|e| e.message);
    let mut ex = Explainer {
        chars: pattern.chars().collect(),
        pos: 0,
        group_index: 0,
    };
    let nodes = ex.parse_alternation(0);
    let mut nodes = nodes;
    while ex.pos < ex.chars.len() {
        // Stray closing parens at top level.
        ex.pos += 1;
        nodes.push(Node::leaf(")", "error", "Unbalanced closing parenthesis"));
    }
    let mut text = String::new();
    for n in &nodes {
        n.to_text(0, &mut text);
    }
    let flag_desc: Vec<Value> = flags.chars().map(|f| json!({
        "flag": f,
        "meaning": match f { 'g' => "global – find all matches", 'i' => "case-insensitive", 'm' => "\
            multi-line – ^ and $ match at line breaks", 's' => "\
            dot-all – . matches newlines", 'x' => "\
            extended – ignore whitespace and allow # comments", 'u' => "unicode", 'y' => "\
            sticky (JavaScript only)", _ => "unknown" }
    })).collect();
    let features = detect_features(pattern);
    Ok(json!({
        "pattern": pattern,
        "flags": flag_desc,
        "valid": compile_error.is_none(),
        "error": compile_error,
        "capture_groups": ex.group_index,
        "tree": nodes.iter().map(Node::to_json).collect::<Vec<_>>(),
        "text": text,
        "features": features,
        "portability": portability_notes(pattern),
    }))
}

fn detect_features(p: &str) -> Vec<&'static str> {
    let mut f = Vec::new();
    if p.contains("(?=") || p.contains("(?!") {
        f.push("lookahead");
    }
    if p.contains("(?<=") || p.contains("(?<!") {
        f.push("lookbehind");
    }
    if p.contains("(?<") && !p.contains("(?<=") && !p.contains("(?<!") || p.contains("(?P<") {
        f.push("named groups");
    }
    if fancy_regex::Regex::new(r"\\[1-9]|\\k<")
        .map(|r| r.is_match(p).unwrap_or(false))
        .unwrap_or(false)
    {
        f.push("backreferences");
    }
    if p.contains("(?>") {
        f.push("atomic groups");
    }
    if p.contains("\\p{") || p.contains("\\P{") {
        f.push("unicode properties");
    }
    if p.contains("*+") || p.contains("++") || p.contains("?+") {
        f.push("possessive quantifiers");
    }
    f
}

fn portability_notes(p: &str) -> Vec<&'static str> {
    let mut notes = Vec::new();
    if p.contains("(?<=") || p.contains("(?<!") {
        notes.push("Lookbehind is unsupported in Safari < 16.4 and in Go/RE2/Rust-regex.");
    }
    if p.contains("\\1") || p.contains("\\k<") {
        notes.push("Backreferences are unsupported in Go/RE2 and the default Rust regex crate.");
    }
    if p.contains("(?P<") {
        notes.push("`(?P<name>` is Python/Go/Rust syntax; JavaScript uses `(?<name>`.");
    }
    if p.contains("(?>") {
        notes.push("Atomic groups are unavailable in JavaScript.");
    }
    if p.contains("[[:") {
        notes.push("POSIX classes like `[[:alpha:]]` are not supported by JavaScript.");
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching() {
        let r = test(r"(?<year>\d{4})-(\d{2})", "g", "2024-01 and 1999-12", Some("$2/$<year>"), 0)
            .unwrap();
        assert_eq!(r["match_count"], 2);
        assert_eq!(r["matches"][0]["named"]["year"], "2024");
        assert_eq!(r["replaced"], "01/2024 and 12/1999");
        assert_eq!(r["split"].as_array().unwrap().len(), 3);
        let r = test(r"(?<=\$)\d+", "", "cost $42", None, 0).unwrap();
        assert_eq!(r["matches"][0]["value"], "42");
        assert!(test("(", "", "x", None, 0).is_err());
        assert!(test("a", "q", "x", None, 0).is_err());
        assert_eq!(js_replacement_to_rust("$1-$&-$$-$<n>"), "${1}-${0}-$$-${n}");
    }

    #[test]
    fn explaining() {
        let e = explain(r"^(?<user>[a-z0-9._]+)@(\w+\.)+[a-z]{2,}$", "i").unwrap();
        assert_eq!(e["valid"], true);
        assert_eq!(e["capture_groups"], 2);
        let text = e["text"].as_str().unwrap();
        assert!(text.contains("Named capturing group `user`"));
        assert!(text.contains("between 2 and  times") || text.contains("at least 2 times"));
        let e = explain(r"(?<=a)b\1(", "").unwrap();
        assert_eq!(e["valid"], false);
        assert!(e["features"].as_array().unwrap().iter().any(|f| f == "lookbehind"));
    }
}
