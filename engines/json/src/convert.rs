//! Format conversions: YAML, XML, TOML, INI, plist, CSV and JSON Lines.

use engine_shared::{EngineError, EngineResult};
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use serde_json::{json, Map, Value};

// ---------------------------------------------------------------- YAML ----

/// Serialises a JSON value as YAML.
pub fn to_yaml(value: &Value) -> EngineResult<String> {
    serde_yaml::to_string(value)
        .map_err(|e| EngineError::internal(format!("YAML serialisation failed: {e}")))
}

/// Parses YAML (multi-document streams become an array).
pub fn from_yaml(text: &str) -> EngineResult<Value> {
    if text.trim().is_empty() {
        return Err(EngineError::empty("yaml"));
    }
    // Multi-document support: return an array when several documents exist.
    let mut docs: Vec<Value> = Vec::new();
    for doc in serde_yaml::Deserializer::from_str(text) {
        let v: serde_yaml::Value = serde::Deserialize::deserialize(doc).map_err(|e| {
            let loc = e
                .location()
                .map(|l| format!(" at line {}, column {}", l.line(), l.column()))
                .unwrap_or_default();
            EngineError::malformed(format!("Invalid YAML{loc}: {e}"))
        })?;
        docs.push(yaml_to_json(v)?);
    }
    Ok(match docs.len() {
        0 => Value::Null,
        1 => docs.pop().unwrap_or(Value::Null),
        _ => Value::Array(docs),
    })
}

fn yaml_to_json(v: serde_yaml::Value) -> EngineResult<Value> {
    Ok(match v {
        serde_yaml::Value::Null => Value::Null,
        serde_yaml::Value::Bool(b) => Value::Bool(b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                json!(i)
            } else if let Some(u) = n.as_u64() {
                json!(u)
            } else {
                json!(n.as_f64().unwrap_or(0.0))
            }
        },
        serde_yaml::Value::String(s) => Value::String(s),
        serde_yaml::Value::Sequence(s) => {
            Value::Array(s.into_iter().map(yaml_to_json).collect::<EngineResult<_>>()?)
        },
        serde_yaml::Value::Mapping(m) => {
            let mut out = Map::new();
            for (k, v) in m {
                let key = match k {
                    serde_yaml::Value::String(s) => s,
                    serde_yaml::Value::Number(n) => n.to_string(),
                    serde_yaml::Value::Bool(b) => b.to_string(),
                    serde_yaml::Value::Null => "null".into(),
                    other => serde_yaml::to_string(&other).unwrap_or_default().trim().to_string(),
                };
                out.insert(key, yaml_to_json(v)?);
            }
            Value::Object(out)
        },
        serde_yaml::Value::Tagged(t) => yaml_to_json(t.value)?,
    })
}

// ---------------------------------------------------------------- TOML ----

/// Serialises a JSON object as TOML.
pub fn to_toml(value: &Value) -> EngineResult<String> {
    let table = value
        .as_object()
        .ok_or_else(|| EngineError::invalid("TOML documents must be objects at the root"))?;
    // toml requires non-table values before tables; serialise via toml::Value.
    let tv: toml::Value = serde_json::from_value(Value::Object(table.clone()))
        .map_err(|e| EngineError::invalid(format!("Value cannot be represented in TOML: {e}")))?;
    toml::to_string_pretty(&tv)
        .map_err(|e| EngineError::invalid(format!("TOML serialisation failed: {e}")))
}

/// Parses TOML into JSON.
pub fn from_toml(text: &str) -> EngineResult<Value> {
    if text.trim().is_empty() {
        return Err(EngineError::empty("toml"));
    }
    let tv: toml::Value = toml::from_str(text).map_err(|e| {
        let span = e
            .span()
            .map(|s| format!(" (bytes {}..{})", s.start, s.end))
            .unwrap_or_default();
        EngineError::malformed(format!("Invalid TOML{span}: {}", e.message()))
    })?;
    serde_json::to_value(tv).map_err(|e| EngineError::internal(e.to_string()))
}

// ----------------------------------------------------------------- INI ----

/// Parses INI / .properties style text into `{section: {key: value}}`.
/// Keys before any section go under `""` (or `default_section`).
pub fn from_ini(text: &str, default_section: &str, parse_values: bool) -> EngineResult<Value> {
    if text.trim().is_empty() {
        return Err(EngineError::empty("ini"));
    }
    let mut out: Map<String, Value> = Map::new();
    let mut current = default_section.to_string();
    let mut errors = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(sec) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            current = sec.trim().to_string();
            out.entry(current.clone())
                .or_insert_with(|| Value::Object(Map::new()));
            continue;
        }
        let Some((k, v)) = line.split_once(['=', ':']) else {
            errors.push(format!("line {}: expected key=value", i + 1));
            continue;
        };
        let key = k.trim().to_string();
        let mut val = v.trim().to_string();
        // Strip inline comments and quotes.
        if let Some(idx) = val.find(" ;").or_else(|| val.find(" #")) {
            val = val[..idx].trim().to_string();
        }
        if (val.starts_with('"') && val.ends_with('"')
            || val.starts_with('\'') && val.ends_with('\''))
            && val.len() >= 2
        {
            val = val[1..val.len() - 1].to_string();
        }
        let parsed = if parse_values {
            parse_scalar(&val)
        } else {
            Value::String(val)
        };
        let section = out
            .entry(current.clone())
            .or_insert_with(|| Value::Object(Map::new()));
        if let Value::Object(m) = section {
            m.insert(key, parsed);
        }
    }
    Ok(json!({ "result": out, "sections": out.len(), "errors": errors }))
}

/// Parses a scalar string into a JSON number/bool/null when it looks like one.
pub fn parse_scalar(s: &str) -> Value {
    match s.to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" => return Value::Bool(true),
        "false" | "no" | "off" => return Value::Bool(false),
        "null" | "none" | "~" => return Value::Null,
        _ => {},
    }
    if let Ok(i) = s.parse::<i64>() {
        return json!(i);
    }
    if let Ok(f) = s.parse::<f64>() {
        if f.is_finite() && s.contains(['.', 'e', 'E']) {
            return json!(f);
        }
    }
    Value::String(s.to_string())
}

/// Serialises `{section: {key: value}}` to INI.
pub fn to_ini(value: &Value) -> EngineResult<String> {
    let m = value
        .as_object()
        .ok_or_else(|| EngineError::invalid("INI root must be an object of sections"))?;
    let mut out = String::new();
    let fmt = |v: &Value| -> String {
        match v {
            Value::String(s) => s.clone(),
            Value::Null => String::new(),
            other => serde_json::to_string(other).unwrap_or_default(),
        }
    };
    // Top-level scalars first (global section).
    for (k, v) in m {
        if !v.is_object() {
            out.push_str(&format!("{k} = {}\n", fmt(v)));
        }
    }
    for (section, v) in m {
        if let Value::Object(entries) = v {
            if !out.is_empty() {
                out.push('\n');
            }
            if !section.is_empty() {
                out.push_str(&format!("[{section}]\n"));
            }
            for (k, val) in entries {
                out.push_str(&format!("{k} = {}\n", fmt(val)));
            }
        }
    }
    Ok(out)
}

/// Re-formats INI text (normalised spacing, sections, preserved order).
pub fn format_ini(text: &str) -> EngineResult<Value> {
    let parsed = from_ini(text, "", false)?;
    Ok(json!({
        "result": to_ini(&parsed["result"])?,
        "sections": parsed["sections"],
        "errors": parsed["errors"]
    }))
}

// ----------------------------------------------------------------- XML ----

/// Options shared by the XML conversions.
#[derive(Debug, Clone)]
pub struct XmlOptions {
    /// Prefix marking attributes in JSON keys (default `@`).
    pub attribute_prefix: String,
    /// Key used for element text content (default `#text`).
    pub text_key: String,
    /// Root element name when the JSON root is anonymous.
    pub root: String,
    /// Element name for array items.
    pub item_name: String,
    /// Indent width for pretty printing.
    pub indent: usize,
    /// Emit the `<?xml …?>` declaration.
    pub declaration: bool,
    /// Convert numeric/boolean text into JSON numbers/booleans.
    pub parse_values: bool,
}

impl Default for XmlOptions {
    fn default() -> Self {
        XmlOptions {
            attribute_prefix: "@".into(),
            text_key: "#text".into(),
            root: "root".into(),
            item_name: "item".into(),
            indent: 2,
            declaration: true,
            parse_values: true,
        }
    }
}

fn xml_err(e: quick_xml::Error, pos: u64) -> EngineError {
    EngineError::malformed(format!("Invalid XML at byte {pos}: {e}"))
}

/// XML → JSON. Attributes become `@attr`, text becomes `#text` (or the
/// value itself for leaf elements), repeated children become arrays.
pub fn from_xml(text: &str, opts: &XmlOptions) -> EngineResult<Value> {
    if text.trim().is_empty() {
        return Err(EngineError::empty("xml"));
    }
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut stack: Vec<(String, Map<String, Value>, String)> = Vec::new();
    let mut root: Option<Value> = None;
    let mut root_name = String::new();

    fn insert_child(parent: &mut Map<String, Value>, name: String, child: Value) {
        match parent.get_mut(&name) {
            Some(Value::Array(a)) => a.push(child),
            Some(existing) => {
                let prev = existing.take();
                *existing = Value::Array(vec![prev, child]);
            },
            None => {
                parent.insert(name, child);
            },
        }
    }

    let finish = |name: String,
                  mut obj: Map<String, Value>,
                  txt: String,
                  opts: &XmlOptions|
     -> (String, Value) {
        let t = txt.trim();
        let text_val = if t.is_empty() {
            None
        } else {
            Some(if opts.parse_values {
                parse_scalar(t)
            } else {
                Value::String(t.to_string())
            })
        };
        let v = if obj.is_empty() {
            text_val.unwrap_or(Value::Null)
        } else {
            if let Some(tv) = text_val {
                obj.insert(opts.text_key.clone(), tv);
            }
            Value::Object(obj)
        };
        (name, v)
    };

    let mut buf = Vec::new();
    loop {
        let pos = reader.buffer_position();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let mut obj = Map::new();
                for a in e.attributes().flatten() {
                    let k = String::from_utf8_lossy(a.key.as_ref()).into_owned();
                    let v = a.unescape_value().map(|v| v.into_owned()).unwrap_or_default();
                    obj.insert(
                        format!("{}{}", opts.attribute_prefix, k),
                        if opts.parse_values {
                            parse_scalar(&v)
                        } else {
                            Value::String(v)
                        },
                    );
                }
                stack.push((name, obj, String::new()));
            },
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let mut obj = Map::new();
                for a in e.attributes().flatten() {
                    let k = String::from_utf8_lossy(a.key.as_ref()).into_owned();
                    let v = a.unescape_value().map(|v| v.into_owned()).unwrap_or_default();
                    obj.insert(
                        format!("{}{}", opts.attribute_prefix, k),
                        if opts.parse_values {
                            parse_scalar(&v)
                        } else {
                            Value::String(v)
                        },
                    );
                }
                let v = if obj.is_empty() {
                    Value::Null
                } else {
                    Value::Object(obj)
                };
                match stack.last_mut() {
                    Some((_, parent, _)) => insert_child(parent, name, v),
                    None => {
                        root_name = name;
                        root = Some(v);
                    },
                }
            },
            Ok(Event::Text(t)) => {
                if let Some((_, _, txt)) = stack.last_mut() {
                    txt.push_str(&t.unescape().map(|s| s.into_owned()).unwrap_or_default());
                }
            },
            Ok(Event::CData(c)) => {
                if let Some((_, _, txt)) = stack.last_mut() {
                    txt.push_str(&String::from_utf8_lossy(&c));
                }
            },
            Ok(Event::End(_)) => {
                let (name, obj, txt) = stack
                    .pop()
                    .ok_or_else(|| EngineError::malformed("Unexpected closing tag"))?;
                let (name, v) = finish(name, obj, txt, opts);
                match stack.last_mut() {
                    Some((_, parent, _)) => insert_child(parent, name, v),
                    None => {
                        root_name = name;
                        root = Some(v);
                    },
                }
            },
            Ok(Event::Eof) => break,
            Ok(_) => {},
            Err(e) => return Err(xml_err(e, pos)),
        }
        buf.clear();
    }
    if !stack.is_empty() {
        return Err(EngineError::malformed(format!(
            "Unclosed element <{}>",
            stack.last().map(|s| s.0.as_str()).unwrap_or("")
        )));
    }
    let root = root.ok_or_else(|| EngineError::malformed("No root element found"))?;
    let mut wrapper = Map::new();
    wrapper.insert(root_name.clone(), root);
    Ok(json!({ "result": wrapper, "root": root_name }))
}

fn is_valid_xml_name(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_alphabetic() || c == '_' => {},
        _ => return false,
    }
    chars.all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ':'))
}

fn sanitise_name(s: &str) -> String {
    if is_valid_xml_name(s) {
        return s.to_string();
    }
    let mut out: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '_' | '-' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if !(out.starts_with(|c: char| c.is_alphabetic()) || out.starts_with('_')) {
        out = format!("_{out}");
    }
    out
}

/// JSON → XML.
pub fn to_xml(value: &Value, opts: &XmlOptions) -> EngineResult<String> {
    let mut writer = if opts.indent > 0 {
        Writer::new_with_indent(Vec::new(), b' ', opts.indent)
    } else {
        Writer::new(Vec::new())
    };
    if opts.declaration {
        writer
            .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
            .map_err(|e| EngineError::internal(e.to_string()))?;
    }
    // A single-key object uses that key as the root element.
    let (root_name, body): (String, &Value) = match value {
        Value::Object(m) if m.len() == 1 && m.values().next().is_some_and(|v| !v.is_array()) => {
            let (k, v) = m
                .iter()
                .next()
                .map(|(k, v)| (k.as_str(), v))
                .unwrap_or(("root", value));
            (sanitise_name(k), v)
        },
        _ => (sanitise_name(&opts.root), value),
    };
    write_element(&mut writer, &root_name, body, opts)?;
    String::from_utf8(writer.into_inner()).map_err(|e| EngineError::internal(e.to_string()))
}

fn scalar_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn write_element(
    w: &mut Writer<Vec<u8>>,
    name: &str,
    v: &Value,
    opts: &XmlOptions,
) -> EngineResult<()> {
    let ie = |e: std::io::Error| EngineError::internal(e.to_string());
    match v {
        Value::Array(items) => {
            for item in items {
                write_element(w, name, item, opts)?;
            }
            Ok(())
        },
        Value::Object(m) => {
            let mut start = BytesStart::new(name);
            let mut children: Vec<(&String, &Value)> = Vec::new();
            let mut text: Option<&Value> = None;
            for (k, val) in m {
                if let Some(attr) = k.strip_prefix(opts.attribute_prefix.as_str()) {
                    if !opts.attribute_prefix.is_empty() && !val.is_object() && !val.is_array() {
                        start.push_attribute((
                            sanitise_name(attr).as_str(),
                            scalar_text(val).as_str(),
                        ));
                        continue;
                    }
                }
                if k == &opts.text_key {
                    text = Some(val);
                } else {
                    children.push((k, val));
                }
            }
            if children.is_empty() && text.is_none() {
                w.write_event(Event::Empty(start)).map_err(ie)?;
                return Ok(());
            }
            w.write_event(Event::Start(start)).map_err(ie)?;
            if let Some(t) = text {
                w.write_event(Event::Text(BytesText::new(&scalar_text(t))))
                    .map_err(ie)?;
            }
            for (k, val) in children {
                write_element(w, &sanitise_name(k), val, opts)?;
            }
            w.write_event(Event::End(BytesEnd::new(name))).map_err(ie)?;
            Ok(())
        },
        Value::Null => {
            w.write_event(Event::Empty(BytesStart::new(name))).map_err(ie)?;
            Ok(())
        },
        scalar => {
            w.write_event(Event::Start(BytesStart::new(name))).map_err(ie)?;
            w.write_event(Event::Text(BytesText::new(&scalar_text(scalar))))
                .map_err(ie)?;
            w.write_event(Event::End(BytesEnd::new(name))).map_err(ie)?;
            Ok(())
        },
    }
}

/// Validates XML well-formedness with element statistics.
pub fn validate_xml(text: &str) -> Value {
    if text.trim().is_empty() {
        return json!({ "valid": false, "error": "xml must not be empty" });
    }
    let mut reader = Reader::from_str(text);
    let mut buf = Vec::new();
    let mut depth = 0usize;
    let mut max_depth = 0usize;
    let mut elements = 0usize;
    let mut attributes = 0usize;
    let mut stack: Vec<String> = Vec::new();
    let mut root: Option<String> = None;
    let mut has_decl = false;
    loop {
        let pos = reader.buffer_position();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Decl(_)) => has_decl = true,
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                if root.is_none() {
                    root = Some(name.clone());
                } else if depth == 0 {
                    return json!({
                        "valid": false,
                        "error": format!("Multiple root elements (<{}> after </{}>)", name, root.unwrap_or_default()),
                        "position": pos
                    });
                }
                elements += 1;
                attributes += e.attributes().count();
                stack.push(name);
                depth += 1;
                max_depth = max_depth.max(depth);
            },
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                if root.is_none() {
                    root = Some(name);
                } else if depth == 0 {
                    return json!({
                        "valid": false,
                        "error": format!("Multiple root elements (<{}> after </{}>)", name, root.unwrap_or_default()),
                        "position": pos
                    });
                }
                elements += 1;
                attributes += e.attributes().count();
            },
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                match stack.pop() {
                    Some(open) if open == name => depth -= 1,
                    Some(open) => {
                        return json!({
                            "valid": false,
                            "error": format!("Mismatched closing tag </{}> (expected </{}>)", name, open),
                            "position": pos
                        })
                    },
                    None => {
                        return json!({
                            "valid": false,
                            "error": format!("Unexpected closing tag </{}>", name),
                            "position": pos
                        })
                    },
                }
            },
            Ok(Event::Eof) => break,
            Ok(_) => {},
            Err(e) => return json!({ "valid": false, "error": e.to_string(), "position": pos }),
        }
        buf.clear();
    }
    if let Some(open) = stack.last() {
        return json!({
            "valid": false,
            "error": format!("Unclosed element <{open}>"),
            "position": text.len()
        });
    }
    if root.is_none() {
        return json!({ "valid": false, "error": "No root element" });
    }
    json!({
        "valid": true,
        "root": root,
        "elements": elements,
        "attributes": attributes,
        "max_depth": max_depth,
        "has_declaration": has_decl,
        "bytes": text.len()
    })
}

/// Re-indents (or minifies with `indent = 0`) XML.
pub fn format_xml(text: &str, indent: usize, keep_comments: bool) -> EngineResult<String> {
    let v = validate_xml(text);
    if v["valid"] != true {
        return Err(EngineError::malformed(
            v["error"].as_str().unwrap_or("Invalid XML").to_string(),
        ));
    }
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut writer = if indent > 0 {
        Writer::new_with_indent(Vec::new(), b' ', indent)
    } else {
        Writer::new(Vec::new())
    };
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Comment(_)) if !keep_comments => {},
            Ok(Event::Text(t)) => {
                writer
                    .write_event(Event::Text(t.into_owned()))
                    .map_err(|e| EngineError::internal(e.to_string()))?;
            },
            Ok(ev) => writer
                .write_event(ev.into_owned())
                .map_err(|e| EngineError::internal(e.to_string()))?,
            Err(e) => return Err(xml_err(e, reader.buffer_position())),
        }
        buf.clear();
    }
    String::from_utf8(writer.into_inner()).map_err(|e| EngineError::internal(e.to_string()))
}

// --------------------------------------------------------------- plist ----

/// Apple XML property list → JSON.
pub fn from_plist(text: &str) -> EngineResult<Value> {
    if text.trim().is_empty() {
        return Err(EngineError::empty("plist"));
    }
    if text.starts_with("bplist") || text.as_bytes().starts_with(b"bplist") {
        return Err(EngineError::unsupported(
            "Binary plists are not supported; convert with `plutil -convert xml1` first",
        ));
    }
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    #[derive(Debug)]
    enum Node {
        Dict(Map<String, Value>, Option<String>),
        Array(Vec<Value>),
    }
    let mut stack: Vec<Node> = Vec::new();
    let mut result: Option<Value> = None;
    let mut current_tag: Option<String> = None;
    let mut text_buf = String::new();
    let mut buf = Vec::new();

    #[allow(clippy::ptr_arg)]
    fn push_value(stack: &mut Vec<Node>, result: &mut Option<Value>, v: Value) -> EngineResult<()> {
        match stack.last_mut() {
            Some(Node::Dict(m, key)) => {
                let k = key
                    .take()
                    .ok_or_else(|| EngineError::malformed("plist dict value without <key>"))?;
                m.insert(k, v);
            },
            Some(Node::Array(a)) => a.push(v),
            None => *result = Some(v),
        }
        Ok(())
    }

    loop {
        let pos = reader.buffer_position();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                match name.as_str() {
                    "dict" => stack.push(Node::Dict(Map::new(), None)),
                    "array" => stack.push(Node::Array(Vec::new())),
                    "plist" => {},
                    _ => {
                        current_tag = Some(name);
                        text_buf.clear();
                    },
                }
            },
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let v = match name.as_str() {
                    "true" => Value::Bool(true),
                    "false" => Value::Bool(false),
                    "dict" => Value::Object(Map::new()),
                    "array" => Value::Array(vec![]),
                    "string" | "data" | "date" => Value::String(String::new()),
                    _ => continue,
                };
                push_value(&mut stack, &mut result, v)?;
            },
            Ok(Event::Text(t)) => {
                text_buf.push_str(&t.unescape().map(|s| s.into_owned()).unwrap_or_default())
            },
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                match name.as_str() {
                    "dict" | "array" => {
                        let node = stack
                            .pop()
                            .ok_or_else(|| EngineError::malformed("Unbalanced plist container"))?;
                        let v = match node {
                            Node::Dict(m, _) => Value::Object(m),
                            Node::Array(a) => Value::Array(a),
                        };
                        push_value(&mut stack, &mut result, v)?;
                    },
                    "key" => {
                        if let Some(Node::Dict(_, key)) = stack.last_mut() {
                            *key = Some(text_buf.clone());
                        }
                    },
                    "string" | "date" | "data" => push_value(
                        &mut stack,
                        &mut result,
                        Value::String(text_buf.trim().to_string()),
                    )?,
                    "integer" => push_value(
                        &mut stack,
                        &mut result,
                        text_buf
                            .trim()
                            .parse::<i64>()
                            .map(|i| json!(i))
                            .unwrap_or_else(|_| Value::String(text_buf.clone())),
                    )?,
                    "real" => push_value(
                        &mut stack,
                        &mut result,
                        text_buf
                            .trim()
                            .parse::<f64>()
                            .map(|f| json!(f))
                            .unwrap_or_else(|_| Value::String(text_buf.clone())),
                    )?,
                    "true" => push_value(&mut stack, &mut result, Value::Bool(true))?,
                    "false" => push_value(&mut stack, &mut result, Value::Bool(false))?,
                    _ => {},
                }
                current_tag = None;
                text_buf.clear();
            },
            Ok(Event::Eof) => break,
            Ok(_) => {},
            Err(e) => return Err(xml_err(e, pos)),
        }
        buf.clear();
    }
    let _ = current_tag;
    let value = result.ok_or_else(|| EngineError::malformed("plist contains no value"))?;
    Ok(json!({ "result": value }))
}

// ----------------------------------------------------------------- CSV ----

fn csv_delimiter(d: Option<&str>) -> EngineResult<u8> {
    match d.unwrap_or(",") {
        "," => Ok(b','),
        ";" => Ok(b';'),
        "\t" | "tab" => Ok(b'\t'),
        "|" => Ok(b'|'),
        other if other.len() == 1 => Ok(other.as_bytes()[0]),
        other => Err(EngineError::invalid(format!(
            "Delimiter must be a single character (got `{other}`)"
        ))),
    }
}

/// JSON (array of objects / arrays) → CSV. Nested values are flattened
/// with dotted keys.
pub fn to_csv(value: &Value, delimiter: Option<&str>, headers: bool) -> EngineResult<Value> {
    let rows = match value {
        Value::Array(a) => a.clone(),
        Value::Object(_) => vec![value.clone()],
        _ => return Err(EngineError::invalid("JSON root must be an array of objects (or arrays)")),
    };
    if rows.is_empty() {
        return Err(EngineError::empty("rows"));
    }
    let delim = csv_delimiter(delimiter)?;
    let mut wtr = csv::WriterBuilder::new().delimiter(delim).from_writer(Vec::new());
    let all_arrays = rows.iter().all(|r| r.is_array());
    let mut columns: Vec<String> = Vec::new();
    let flat_rows: Vec<Map<String, Value>> = if all_arrays {
        rows.iter()
            .map(|r| {
                r.as_array()
                    .into_iter()
                    .flatten()
                    .enumerate()
                    .map(|(i, v)| (i.to_string(), v.clone()))
                    .collect()
            })
            .collect()
    } else {
        rows.iter()
            .map(|r| match r {
                Value::Object(_) => crate::core::flatten(r, ".", "dots")
                    .map(|f| f["result"].as_object().cloned().unwrap_or_default())
                    .unwrap_or_default(),
                other => {
                    let mut m = Map::new();
                    m.insert("value".into(), other.clone());
                    m
                },
            })
            .collect()
    };
    for r in &flat_rows {
        for k in r.keys() {
            if !columns.contains(k) {
                columns.push(k.clone());
            }
        }
    }
    let ie = |e: csv::Error| EngineError::internal(format!("CSV write failed: {e}"));
    if headers && !all_arrays {
        wtr.write_record(&columns).map_err(ie)?;
    }
    for r in &flat_rows {
        let rec: Vec<String> = columns
            .iter()
            .map(|c| match r.get(c) {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Null) | None => String::new(),
                Some(other) => other.to_string(),
            })
            .collect();
        wtr.write_record(&rec).map_err(ie)?;
    }
    let bytes = wtr.into_inner().map_err(|e| EngineError::internal(e.to_string()))?;
    let csv_text = String::from_utf8(bytes).map_err(|e| EngineError::internal(e.to_string()))?;
    Ok(json!({ "csv": csv_text, "rows": flat_rows.len(), "columns": columns }))
}

/// CSV → JSON array of objects (or arrays when `headers = false`).
pub fn from_csv(
    text: &str,
    delimiter: Option<&str>,
    headers: bool,
    infer_types: bool,
    trim: bool,
) -> EngineResult<Value> {
    if text.trim().is_empty() {
        return Err(EngineError::empty("csv"));
    }
    let delim = match delimiter {
        Some(d) => csv_delimiter(Some(d))?,
        None => sniff_delimiter(text),
    };
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(headers)
        .flexible(true)
        .trim(if trim { csv::Trim::All } else { csv::Trim::None })
        .from_reader(text.as_bytes());
    let header_row: Vec<String> = if headers {
        rdr.headers()
            .map_err(|e| EngineError::malformed(format!("Invalid CSV header: {e}")))?
            .iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        vec![]
    };
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    for (i, rec) in rdr.records().enumerate() {
        let rec = rec
            .map_err(|e| EngineError::malformed(format!("Invalid CSV at record {}: {e}", i + 1)))?;
        let conv = |s: &str| {
            if infer_types {
                parse_scalar(s)
            } else {
                Value::String(s.to_string())
            }
        };
        if headers {
            if rec.len() != header_row.len() && warnings.len() < 20 {
                warnings.push(format!(
                    "row {} has {} fields (header has {})",
                    i + 1,
                    rec.len(),
                    header_row.len()
                ));
            }
            let mut m = Map::new();
            for (j, field) in rec.iter().enumerate() {
                let key = header_row
                    .get(j)
                    .cloned()
                    .unwrap_or_else(|| format!("column_{}", j + 1));
                m.insert(key, conv(field));
            }
            out.push(Value::Object(m));
        } else {
            out.push(Value::Array(rec.iter().map(conv).collect()));
        }
        if out.len() > 1_000_000 {
            return Err(EngineError::limit("CSV has more than 1,000,000 rows"));
        }
    }
    Ok(json!({
        "result": out,
        "rows": out.len(),
        "columns": header_row,
        "delimiter": (delim as char).to_string(),
        "warnings": warnings
    }))
}

/// Guesses the CSV delimiter (`,`, `;`, tab, `|`) from the header line.
pub fn sniff_delimiter(text: &str) -> u8 {
    let first: String = text.lines().take(5).collect::<Vec<_>>().join("\n");
    let candidates = *b",;\t|";
    candidates
        .iter()
        .copied()
        .max_by_key(|&c| first.bytes().filter(|&b| b == c).count())
        .unwrap_or(b',')
}

/// JSON array → JSON Lines and back.
pub fn to_jsonl(value: &Value) -> EngineResult<String> {
    let arr = value
        .as_array()
        .ok_or_else(|| EngineError::invalid("JSON root must be an array"))?;
    Ok(arr
        .iter()
        .map(|v| serde_json::to_string(v).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Parses JSON Lines into an array.
pub fn from_jsonl(text: &str) -> EngineResult<Value> {
    if text.trim().is_empty() {
        return Err(EngineError::empty("jsonl"));
    }
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        out.push(serde_json::from_str::<Value>(line).map_err(|e| {
            EngineError::malformed(format!("Invalid JSON on line {}: {}", i + 1, e))
        })?);
    }
    Ok(Value::Array(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_toml_ini() {
        let v = from_yaml("a: 1\nb:\n  - x\n  - y\nc: true\n").unwrap();
        assert_eq!(v, json!({ "a": 1, "b": ["x", "y"], "c": true }));
        assert!(to_yaml(&v).unwrap().contains("- x"));
        assert!(from_yaml("a: [1, 2").is_err());
        assert!(from_yaml("").is_err());
        let multi = from_yaml("a: 1\n---\nb: 2\n").unwrap();
        assert_eq!(multi.as_array().unwrap().len(), 2);
        let t = to_toml(&json!({ "title": "x", "owner": { "name": "n" } })).unwrap();
        assert!(t.contains("[owner]"));
        assert_eq!(from_toml(&t).unwrap()["owner"]["name"], "n");
        assert!(from_toml("a = ").is_err());
        assert!(to_toml(&json!([1])).is_err());
        let ini = from_ini("; c\nglobal = 1\n[s]\nk = v ; note\nflag = yes\nn = 2.5\n", "", true)
            .unwrap();
        assert_eq!(ini["result"][""]["global"], 1);
        assert_eq!(ini["result"]["s"]["k"], "v");
        assert_eq!(ini["result"]["s"]["flag"], true);
        assert_eq!(ini["result"]["s"]["n"], 2.5);
        let f = format_ini("[a]\nx=1\n").unwrap();
        assert_eq!(f["result"], "[a]\nx = 1\n");
    }

    #[test]
    fn xml_roundtrip() {
        let xml = r#"<?xml version="1.0"?><root id="1"><item>a</item><item>b</item><n>5</n><e/><t>x<![CDATA[<y>]]></t></root>"#;
        let v = from_xml(xml, &XmlOptions::default()).unwrap();
        assert_eq!(v["root"], "root");
        assert_eq!(v["result"]["root"]["@id"], 1);
        assert_eq!(v["result"]["root"]["item"], json!(["a", "b"]));
        assert_eq!(v["result"]["root"]["n"], 5);
        assert_eq!(v["result"]["root"]["e"], Value::Null);
        assert_eq!(v["result"]["root"]["t"], "x<y>");
        let back = to_xml(&v["result"], &XmlOptions { indent: 0, ..Default::default() }).unwrap();
        assert!(back.contains(
            "<root id=\"1\"><item>a</item><item>b</item><n>5</n><e/><t>x&lt;y&gt;</t></root>"
        ));
        assert!(from_xml("<a><b></a>", &XmlOptions::default()).is_err());
        assert!(from_xml("", &XmlOptions::default()).is_err());
        let val = validate_xml("<a><b/></a>");
        assert_eq!(val["valid"], true);
        assert_eq!(val["elements"], 2);
        assert_eq!(validate_xml("<a><b></a>")["valid"], false);
        assert_eq!(validate_xml("<a/><b/>")["valid"], false);
        let pretty = format_xml("<a><b>x</b></a>", 2, true).unwrap();
        assert_eq!(pretty, "<a>\n  <b>x</b>\n</a>");
        assert_eq!(format_xml("<a>\n  <b>x</b>\n</a>", 0, true).unwrap(), "<a><b>x</b></a>");
        let weird = to_xml(
            &json!({ "1 bad": 1 }),
            &XmlOptions {
                indent: 0,
                declaration: false,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(weird, "<_1_bad>1</_1_bad>");
    }

    #[test]
    fn plist_csv_jsonl() {
        let p = r#"<?xml version="1.0"?><plist version="1.0"><dict><key>Name</key><string>App</string><key>Ver</key><integer>3</integer><key>On</key><true/><key>List</key><array><real>1.5</real><string>x</string></array></dict></plist>"#;
        let v = from_plist(p).unwrap();
        assert_eq!(v["result"]["Name"], "App");
        assert_eq!(v["result"]["Ver"], 3);
        assert_eq!(v["result"]["On"], true);
        assert_eq!(v["result"]["List"][0], 1.5);
        assert!(from_plist("bplist00").is_err());
        let c = to_csv(&json!([{ "a": 1, "b": { "c": "x,y" } }, { "a": 2 }]), None, true).unwrap();
        assert_eq!(c["csv"], "a,b.c\n1,\"x,y\"\n2,\n");
        let back = from_csv(c["csv"].as_str().unwrap(), None, true, true, true).unwrap();
        assert_eq!(back["result"][0]["a"], 1);
        assert_eq!(back["result"][0]["b.c"], "x,y");
        let semi = from_csv("a;b\n1;2", None, true, true, true).unwrap();
        assert_eq!(semi["delimiter"], ";");
        let raw = from_csv("1,2\n3,4", None, false, true, true).unwrap();
        assert_eq!(raw["result"][1], json!([3, 4]));
        assert!(from_csv("", None, true, true, true).is_err());
        let jl = to_jsonl(&json!([{ "a": 1 }, 2])).unwrap();
        assert_eq!(jl, "{\"a\":1}\n2");
        assert_eq!(from_jsonl(&jl).unwrap(), json!([{ "a": 1 }, 2]));
        assert!(from_jsonl("{bad").is_err());
    }
}
