//! JSON parsing, formatting, validation, inspection, escaping, sorting,
//! flattening and table conversion.

use engine_shared::{EngineError, EngineResult};
use serde_json::{json, Map, Value};

/// Parses JSON with a structured error (line/column) on failure.
pub fn parse(text: &str) -> EngineResult<Value> {
    if text.trim().is_empty() {
        return Err(EngineError::empty("json"));
    }
    serde_json::from_str(text).map_err(|e| {
        EngineError::new(
            engine_shared::error::ErrorCode::Malformed,
            format!("Invalid JSON at line {}, column {}: {}", e.line(), e.column(), classify(&e)),
        )
    })
}

fn classify(e: &serde_json::Error) -> String {
    let msg = e.to_string();
    // Strip serde's "at line x column y" suffix since we report it separately.
    match msg.find(" at line ") {
        Some(i) => msg[..i].to_string(),
        None => msg,
    }
}

/// Lenient parse: also accepts JSON5-ish input (comments, trailing commas,
/// single quotes) by pre-processing. Used by `repair`.
pub fn repair(text: &str) -> EngineResult<Value> {
    if let Ok(v) = serde_json::from_str::<Value>(text) {
        return Ok(v);
    }
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut in_str: Option<char> = None;
    while i < chars.len() {
        let c = chars[i];
        match in_str {
            Some(q) => {
                if c == '\\' && i + 1 < chars.len() {
                    out.push(c);
                    out.push(chars[i + 1]);
                    i += 2;
                    continue;
                }
                if c == q {
                    out.push('"');
                    in_str = None;
                } else if c == '"' {
                    out.push_str("\\\"");
                } else {
                    out.push(c);
                }
            },
            None => {
                if c == '/' && chars.get(i + 1) == Some(&'/') {
                    while i < chars.len() && chars[i] != '\n' {
                        i += 1;
                    }
                    continue;
                }
                if c == '/' && chars.get(i + 1) == Some(&'*') {
                    i += 2;
                    while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                        i += 1;
                    }
                    i += 2;
                    continue;
                }
                if c == '"' || c == '\'' {
                    in_str = Some(c);
                    out.push('"');
                } else if c == ',' {
                    // Drop trailing commas.
                    let mut j = i + 1;
                    while j < chars.len() && chars[j].is_whitespace() {
                        j += 1;
                    }
                    if j < chars.len() && (chars[j] == '}' || chars[j] == ']') {
                        i += 1;
                        continue;
                    }
                    out.push(c);
                } else {
                    out.push(c);
                }
            },
        }
        i += 1;
    }
    // Quote unquoted keys: { key: 1 } → { "key": 1 }
    let re = regex_lite_keys(&out);
    serde_json::from_str(&re)
        .map_err(|e| EngineError::malformed(format!("JSON could not be repaired: {e}")))
}

fn regex_lite_keys(s: &str) -> String {
    // Simple state machine: after `{` or `,` (outside strings) an identifier followed by `:` is a key.
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut in_str = false;
    let mut expect_key = false;
    while i < chars.len() {
        let c = chars[i];
        if in_str {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_str = true;
            out.push(c);
            i += 1;
            continue;
        }
        if c == '{' || c == ',' {
            expect_key = true;
            out.push(c);
            i += 1;
            continue;
        }
        if expect_key && (c.is_alphabetic() || c == '_' || c == '$') {
            let start = i;
            while i < chars.len()
                && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '$')
            {
                i += 1;
            }
            let mut j = i;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && chars[j] == ':' {
                out.push('"');
                out.extend(&chars[start..i]);
                out.push('"');
                expect_key = false;
                continue;
            }
            out.extend(&chars[start..i]);
            continue;
        }
        if !c.is_whitespace() {
            expect_key = false;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Pretty prints with a custom indent (spaces or tab).
pub fn format(
    value: &Value,
    indent: usize,
    use_tabs: bool,
    sort_keys: bool,
) -> EngineResult<String> {
    let v = if sort_keys {
        sort_keys_recursive(value)
    } else {
        value.clone()
    };
    if indent == 0 && !use_tabs {
        return Ok(serde_json::to_string(&v)?);
    }
    let indent_str = if use_tabs {
        "\t".to_string()
    } else {
        " ".repeat(indent.min(16))
    };
    let mut buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(indent_str.as_bytes());
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    serde::Serialize::serialize(&v, &mut ser)?;
    String::from_utf8(buf).map_err(|e| EngineError::internal(e.to_string()))
}

/// Returns a copy with object keys sorted recursively.
pub fn sort_keys_recursive(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            let mut entries: Vec<(&String, &Value)> = m.iter().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(k, v)| (k.clone(), sort_keys_recursive(v)))
                    .collect(),
            )
        },
        Value::Array(a) => Value::Array(a.iter().map(sort_keys_recursive).collect()),
        other => other.clone(),
    }
}

fn depth(v: &Value) -> usize {
    match v {
        Value::Object(m) => 1 + m.values().map(depth).max().unwrap_or(0),
        Value::Array(a) => 1 + a.iter().map(depth).max().unwrap_or(0),
        _ => 0,
    }
}

fn count_nodes(v: &Value, counts: &mut Map<String, Value>) {
    let key = match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    };
    let n = counts.get(key).and_then(Value::as_u64).unwrap_or(0) + 1;
    counts.insert(key.into(), json!(n));
    match v {
        Value::Object(m) => m.values().for_each(|x| count_nodes(x, counts)),
        Value::Array(a) => a.iter().for_each(|x| count_nodes(x, counts)),
        _ => {},
    }
}

/// Validation + structural report.
pub fn validate(text: &str) -> Value {
    match parse(text) {
        Ok(v) => {
            let mut counts = Map::new();
            count_nodes(&v, &mut counts);
            json!({
                "valid": true,
                "root_type": match &v { Value::Object(_) => "object", Value::Array(_) => "array", Value::String(_) => "string", Value::Number(_) => "number", Value::Bool(_) => "boolean", Value::Null => "null" },
                "depth": depth(&v),
                "top_level_keys": v.as_object().map(|m| m.keys().cloned().collect::<Vec<_>>()),
                "top_level_length": v.as_array().map(|a| a.len()).or_else(|| v.as_object().map(|m| m.len())),
                "node_counts": counts,
                "bytes": text.len(),
                "minified_bytes": serde_json::to_string(&v).map(|s| s.len()).unwrap_or(0),
            })
        },
        Err(e) => {
            let (line, col) = extract_line_col(&e.message);
            json!({
                "valid": false,
                "error": e.message,
                "line": line,
                "column": col,
                "error_code": e.code.as_str()
            })
        },
    }
}

fn extract_line_col(msg: &str) -> (Option<u64>, Option<u64>) {
    let line = msg
        .split("line ")
        .nth(1)
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.trim().parse().ok());
    let col = msg
        .split("column ")
        .nth(1)
        .and_then(|s| s.split(':').next())
        .and_then(|s| s.trim().parse().ok());
    (line, col)
}

/// Escapes text for embedding inside a JSON string literal.
pub fn escape(text: &str, with_quotes: bool) -> String {
    let s = serde_json::to_string(text).unwrap_or_default();
    if with_quotes {
        s
    } else {
        s[1..s.len() - 1].to_string()
    }
}

/// Unescapes a JSON string literal (quotes optional).
pub fn unescape(text: &str) -> EngineResult<String> {
    let t = text.trim();
    if t.is_empty() {
        return Err(EngineError::empty("text"));
    }
    let quoted = if t.starts_with('"') && t.ends_with('"') && t.len() >= 2 {
        t.to_string()
    } else {
        format!("\"{t}\"")
    };
    serde_json::from_str::<String>(&quoted)
        .map_err(|e| EngineError::invalid(format!("Invalid JSON string escape sequence: {e}")))
}

fn cmp_values(a: &Value, b: &Value, numeric: bool) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            x.as_f64().partial_cmp(&y.as_f64()).unwrap_or(Ordering::Equal)
        },
        (Value::String(x), Value::String(y)) if numeric => {
            match (x.trim().parse::<f64>(), y.trim().parse::<f64>()) {
                (Ok(p), Ok(q)) => p.partial_cmp(&q).unwrap_or(Ordering::Equal),
                _ => x.cmp(y),
            }
        },
        (Value::String(x), Value::String(y)) => x.cmp(y),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Less,
        (_, Value::Null) => Ordering::Greater,
        (Value::Number(_), _) => Ordering::Less,
        (_, Value::Number(_)) => Ordering::Greater,
        (Value::String(_), _) => Ordering::Less,
        (_, Value::String(_)) => Ordering::Greater,
        _ => serde_json::to_string(a)
            .unwrap_or_default()
            .cmp(&serde_json::to_string(b).unwrap_or_default()),
    }
}

fn get_path<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    if key.is_empty() {
        return Some(v);
    }
    let mut cur = v;
    for part in key.split('.') {
        cur = match cur {
            Value::Object(m) => m.get(part)?,
            Value::Array(a) => a.get(part.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur)
}

/// Sorts a JSON array (optionally by a dotted key of each element).
pub fn sort_array(
    value: &Value,
    key: Option<&str>,
    descending: bool,
    numeric: bool,
    unique: bool,
) -> EngineResult<Value> {
    let arr = value
        .as_array()
        .ok_or_else(|| EngineError::invalid("The JSON root must be an array"))?;
    let mut items: Vec<Value> = arr.clone();
    let key = key.unwrap_or("");
    if !key.is_empty() {
        let missing = items.iter().filter(|i| get_path(i, key).is_none()).count();
        if missing == items.len() && !items.is_empty() {
            return Err(EngineError::not_found(format!("No element has the key `{key}`")));
        }
    }
    items.sort_by(|a, b| {
        cmp_values(
            get_path(a, key).unwrap_or(&Value::Null),
            get_path(b, key).unwrap_or(&Value::Null),
            numeric,
        )
    });
    if descending {
        items.reverse();
    }
    let before = items.len();
    if unique {
        let mut seen = std::collections::HashSet::new();
        items.retain(|i| seen.insert(serde_json::to_string(i).unwrap_or_default()));
    }
    Ok(json!({ "result": items, "count": items.len(), "removed_duplicates": before - items.len() }))
}

/// Flattens nested objects/arrays into a single-level object.
pub fn flatten(value: &Value, separator: &str, array_style: &str) -> EngineResult<Value> {
    if !value.is_object() && !value.is_array() {
        return Err(EngineError::invalid("The JSON root must be an object or array"));
    }
    let mut out = Map::new();
    fn walk(v: &Value, prefix: String, sep: &str, style: &str, out: &mut Map<String, Value>) {
        match v {
            Value::Object(m) if !m.is_empty() => {
                for (k, x) in m {
                    let key = if prefix.is_empty() {
                        k.clone()
                    } else {
                        format!("{prefix}{sep}{k}")
                    };
                    walk(x, key, sep, style, out);
                }
            },
            Value::Array(a) if !a.is_empty() => {
                for (i, x) in a.iter().enumerate() {
                    let key = match style {
                        "brackets" => format!("{prefix}[{i}]"),
                        _ => {
                            if prefix.is_empty() {
                                i.to_string()
                            } else {
                                format!("{prefix}{sep}{i}")
                            }
                        },
                    };
                    walk(x, key, sep, style, out);
                }
            },
            other => {
                out.insert(prefix, other.clone());
            },
        }
    }
    walk(value, String::new(), separator, array_style, &mut out);
    Ok(json!({ "result": out, "keys": out.len() }))
}

/// Reverses `flatten`.
pub fn unflatten(value: &Value, separator: &str) -> EngineResult<Value> {
    let m = value
        .as_object()
        .ok_or_else(|| EngineError::invalid("The JSON root must be a flat object"))?;
    let mut root = Value::Object(Map::new());
    for (k, v) in m {
        // Support both `a.b.0` and `a.b[0]` styles.
        let normalised = k.replace('[', separator).replace(']', "");
        let parts: Vec<&str> = normalised.split(separator).filter(|p| !p.is_empty()).collect();
        if parts.is_empty() {
            continue;
        }
        let mut cur = &mut root;
        for (i, part) in parts.iter().enumerate() {
            let last = i == parts.len() - 1;
            let next_is_index =
                parts.get(i + 1).map(|p| p.parse::<usize>().is_ok()).unwrap_or(false);
            let index = part.parse::<usize>().ok();
            // Promote empty containers to arrays when an index is used.
            if index.is_some()
                && (cur.is_null() || cur.as_object().map(|o| o.is_empty()).unwrap_or(false))
            {
                *cur = Value::Array(vec![]);
            }
            cur = match (index, cur) {
                (Some(idx), Value::Array(a)) => {
                    while a.len() <= idx {
                        a.push(Value::Null);
                    }
                    if last {
                        a[idx] = v.clone();
                        break;
                    }
                    if a[idx].is_null() {
                        a[idx] = if next_is_index {
                            Value::Array(vec![])
                        } else {
                            Value::Object(Map::new())
                        };
                    }
                    &mut a[idx]
                },
                (_, slot) => {
                    if !slot.is_object() {
                        *slot = Value::Object(Map::new());
                    }
                    let Value::Object(obj) = slot else { break };
                    if last {
                        obj.insert(part.to_string(), v.clone());
                        break;
                    }
                    obj.entry(part.to_string()).or_insert_with(|| {
                        if next_is_index {
                            Value::Array(vec![])
                        } else {
                            Value::Object(Map::new())
                        }
                    })
                },
            };
        }
    }
    Ok(json!({ "result": root }))
}

/// Converts an array of objects (or an object) to a table (columns + rows),
/// plus Markdown and HTML renderings.
pub fn to_table(value: &Value, max_rows: usize) -> EngineResult<Value> {
    let rows: Vec<Value> = match value {
        Value::Array(a) => a.clone(),
        Value::Object(m) => m.iter().map(|(k, v)| json!({ "key": k, "value": v })).collect(),
        _ => {
            return Err(EngineError::invalid(
                "The JSON root must be an array of objects or an object",
            ))
        },
    };
    if rows.is_empty() {
        return Err(EngineError::empty("table data"));
    }
    let mut columns: Vec<String> = Vec::new();
    for r in &rows {
        match r {
            Value::Object(m) => {
                for k in m.keys() {
                    if !columns.contains(k) {
                        columns.push(k.clone());
                    }
                }
            },
            _ => {
                if !columns.contains(&"value".to_string()) {
                    columns.push("value".into());
                }
            },
        }
    }
    let cell = |v: Option<&Value>| -> String {
        match v {
            None | Some(Value::Null) => String::new(),
            Some(Value::String(s)) => s.clone(),
            Some(other) => serde_json::to_string(other).unwrap_or_default(),
        }
    };
    let limit = if max_rows == 0 { 10_000 } else { max_rows };
    let table_rows: Vec<Vec<String>> = rows
        .iter()
        .take(limit)
        .map(|r| match r {
            Value::Object(m) => columns.iter().map(|c| cell(m.get(c))).collect(),
            other => columns
                .iter()
                .map(|c| if c == "value" { cell(Some(other)) } else { String::new() })
                .collect(),
        })
        .collect();
    let esc = |s: &str| s.replace('|', "\\|").replace('\n', " ");
    let mut md = format!(
        "| {} |\n|{}\n",
        columns.iter().map(|c| esc(c)).collect::<Vec<_>>().join(" | "),
        " --- |".repeat(columns.len())
    );
    for r in &table_rows {
        md.push_str(&format!("| {} |\n", r.iter().map(|c| esc(c)).collect::<Vec<_>>().join(" | ")));
    }
    let h = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let mut html = String::from("<table>\n<thead><tr>");
    for c in &columns {
        html.push_str(&format!("<th>{}</th>", h(c)));
    }
    html.push_str("</tr></thead>\n<tbody>\n");
    for r in &table_rows {
        html.push_str("<tr>");
        for c in r {
            html.push_str(&format!("<td>{}</td>", h(c)));
        }
        html.push_str("</tr>\n");
    }
    html.push_str("</tbody>\n</table>");
    Ok(json!({
        "columns": columns,
        "rows": table_rows,
        "row_count": rows.len(),
        "truncated": rows.len() > limit,
        "markdown": md,
        "html": html
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_format_validate() {
        let v = parse(r#"{"b":1,"a":[1,2,{"c":null}]}"#).unwrap();
        assert_eq!(
            format(&v, 2, false, true).unwrap(),
            "{\n  \"a\": [\n    1,\n    2,\n    {\n      \"c\": null\n    }\n  ],\n  \"b\": 1\n}"
        );
        assert!(format(&v, 0, true, false).unwrap().contains('\t'));
        assert_eq!(format(&v, 0, false, false).unwrap(), r#"{"b":1,"a":[1,2,{"c":null}]}"#);
        let bad = validate("{\"a\": 1,\n \"b\": }");
        assert_eq!(bad["valid"], false);
        assert_eq!(bad["line"], 2);
        let good = validate(r#"{"a":{"b":[1]}}"#);
        assert_eq!(good["depth"], 3);
        assert_eq!(good["node_counts"]["number"], 1);
        assert!(parse("").is_err());
        assert_eq!(parse("nope").unwrap_err().code, engine_shared::error::ErrorCode::Malformed);
    }

    #[test]
    fn repair_json5() {
        let v =
            repair("{ // comment\n  name: 'x', list: [1, 2,], /* c */ \"ok\": true, }").unwrap();
        assert_eq!(v["name"], "x");
        assert_eq!(v["list"].as_array().unwrap().len(), 2);
        assert_eq!(v["ok"], true);
        assert!(repair("{{{").is_err());
    }

    #[test]
    fn escape_sort_flatten_table() {
        assert_eq!(escape("a\"b\n", false), "a\\\"b\\n");
        assert_eq!(unescape("\"a\\\"b\\n\"").unwrap(), "a\"b\n");
        assert_eq!(unescape("a\\tb").unwrap(), "a\tb");
        assert!(unescape("\\x").is_err());
        let arr = json!([{ "n": "b", "v": 2 }, { "n": "a", "v": 10 }]);
        assert_eq!(
            sort_array(&arr, Some("n"), false, false, false).unwrap()["result"][0]["n"],
            "a"
        );
        assert_eq!(sort_array(&arr, Some("v"), true, false, false).unwrap()["result"][0]["v"], 10);
        assert!(sort_array(&arr, Some("zzz"), false, false, false).is_err());
        assert!(sort_array(&json!({}), None, false, false, false).is_err());
        let flat =
            flatten(&json!({ "a": { "b": [1, { "c": 2 }] }, "d": {} }), ".", "dots").unwrap();
        assert_eq!(flat["result"], json!({ "a.b.0": 1, "a.b.1.c": 2, "d": {} }));
        let back = unflatten(&flat["result"], ".").unwrap();
        assert_eq!(back["result"], json!({ "a": { "b": [1, { "c": 2 }] }, "d": {} }));
        let br = flatten(&json!({ "x": [1, 2] }), ".", "brackets").unwrap();
        assert_eq!(br["result"], json!({ "x[0]": 1, "x[1]": 2 }));
        assert_eq!(unflatten(&br["result"], ".").unwrap()["result"], json!({ "x": [1, 2] }));
        let t = to_table(&json!([{ "a": 1, "b": "x" }, { "a": 2, "c": true }]), 0).unwrap();
        assert_eq!(t["columns"], json!(["a", "b", "c"]));
        assert_eq!(t["rows"][1], json!(["2", "", "true"]));
        assert!(t["markdown"].as_str().unwrap().starts_with("| a | b | c |"));
        assert!(to_table(&json!(5), 0).is_err());
    }
}
