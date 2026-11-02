//! Diff, JSONPath queries, JSON Schema validation/generation and code
//! generation (TypeScript, Rust, Go, Python, Kotlin, Swift).

use engine_shared::{EngineError, EngineResult};
use indexmap::IndexMap;
use serde_json::{json, Map, Value};

// ---------------------------------------------------------------- diff ----

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn diff_walk(a: &Value, b: &Value, path: &str, out: &mut Vec<Value>, ignore_order: bool) {
    match (a, b) {
        (Value::Object(ma), Value::Object(mb)) => {
            for (k, va) in ma {
                let p = if path.is_empty() {
                    format!("/{k}")
                } else {
                    format!("{path}/{k}")
                };
                match mb.get(k) {
                    Some(vb) => diff_walk(va, vb, &p, out, ignore_order),
                    None => out.push(json!({ "op": "remove", "path": p, "old": va })),
                }
            }
            for (k, vb) in mb {
                if !ma.contains_key(k) {
                    let p = if path.is_empty() {
                        format!("/{k}")
                    } else {
                        format!("{path}/{k}")
                    };
                    out.push(json!({ "op": "add", "path": p, "new": vb }));
                }
            }
        },
        (Value::Array(aa), Value::Array(ab)) => {
            if ignore_order {
                let mut sa: Vec<String> = aa
                    .iter()
                    .map(|v| {
                        serde_json::to_string(&crate::core::sort_keys_recursive(v))
                            .unwrap_or_default()
                    })
                    .collect();
                let mut sb: Vec<String> = ab
                    .iter()
                    .map(|v| {
                        serde_json::to_string(&crate::core::sort_keys_recursive(v))
                            .unwrap_or_default()
                    })
                    .collect();
                sa.sort();
                sb.sort();
                if sa != sb {
                    out.push(json!({
                        "op": "replace",
                        "path": path,
                        "old": a,
                        "new": b,
                        "note": "array contents differ (order ignored)"
                    }));
                }
                return;
            }
            let n = aa.len().max(ab.len());
            for i in 0..n {
                let p = format!("{path}/{i}");
                match (aa.get(i), ab.get(i)) {
                    (Some(x), Some(y)) => diff_walk(x, y, &p, out, ignore_order),
                    (Some(x), None) => out.push(json!({ "op": "remove", "path": p, "old": x })),
                    (None, Some(y)) => out.push(json!({ "op": "add", "path": p, "new": y })),
                    _ => {},
                }
            }
        },
        _ => {
            if a != b {
                let type_changed = type_name(a) != type_name(b);
                out.push(json!({
                    "op": "replace",
                    "path": if path.is_empty() { "/" } else { path },
                    "old": a,
                    "new": b,
                    "type_changed": type_changed
                }));
            }
        },
    }
}

/// Structural diff as a list of JSON-Pointer addressed operations.
pub fn diff(a: &Value, b: &Value, ignore_order: bool) -> Value {
    let mut ops = Vec::new();
    diff_walk(a, b, "", &mut ops, ignore_order);
    let count = |op: &str| ops.iter().filter(|o| o["op"] == op).count();
    json!({
        "identical": ops.is_empty(),
        "changes": ops,
        "added": count("add"),
        "removed": count("remove"),
        "changed": count("replace"),
    })
}

/// Applies a minimal patch list produced by [`diff`] (add/replace/remove).
pub fn apply_patch(value: &Value, ops: &[Value]) -> EngineResult<Value> {
    let mut v = value.clone();
    for op in ops {
        let path = op["path"]
            .as_str()
            .ok_or_else(|| EngineError::invalid("patch op missing `path`"))?;
        let kind = op["op"].as_str().unwrap_or("");
        let parts: Vec<String> = path
            .trim_start_matches('/')
            .split('/')
            .filter(|p| !p.is_empty())
            .map(|p| p.replace("~1", "/").replace("~0", "~"))
            .collect();
        if parts.is_empty() {
            if kind == "remove" {
                v = Value::Null;
            } else {
                v = op["new"].clone();
            }
            continue;
        }
        let Some((last, parents)) = parts.split_last() else {
            continue;
        };
        let mut cur = &mut v;
        for p in parents {
            cur = match cur {
                Value::Object(m) => m.entry(p.clone()).or_insert(Value::Null),
                Value::Array(a) => {
                    let i: usize = p
                        .parse()
                        .map_err(|_| EngineError::invalid(format!("Invalid array index `{p}`")))?;
                    a.get_mut(i)
                        .ok_or_else(|| EngineError::not_found(format!("Path {path} not found")))?
                },
                _ => return Err(EngineError::not_found(format!("Path {path} not found"))),
            };
        }
        match cur {
            Value::Object(m) => {
                if kind == "remove" {
                    m.shift_remove(last);
                } else {
                    m.insert(last.clone(), op["new"].clone());
                }
            },
            Value::Array(a) => {
                let i: usize = last
                    .parse()
                    .map_err(|_| EngineError::invalid(format!("Invalid array index `{last}`")))?;
                match kind {
                    "remove" => {
                        if i < a.len() {
                            a.remove(i);
                        }
                    },
                    "add" if i >= a.len() => a.push(op["new"].clone()),
                    "add" => a.insert(i, op["new"].clone()),
                    _ => {
                        if i < a.len() {
                            a[i] = op["new"].clone();
                        } else {
                            a.push(op["new"].clone());
                        }
                    },
                }
            },
            Value::Null if kind != "remove" => {
                let mut m = Map::new();
                m.insert(last.clone(), op["new"].clone());
                *cur = Value::Object(m);
            },
            _ => return Err(EngineError::not_found(format!("Path {path} not found"))),
        }
    }
    Ok(v)
}

// ------------------------------------------------------------ jsonpath ----

/// Evaluates a JSONPath (RFC 9535) expression.
pub fn path_query(value: &Value, path: &str) -> EngineResult<Value> {
    if path.trim().is_empty() {
        return Err(EngineError::empty("path"));
    }
    let p = serde_json_path::JsonPath::parse(path)
        .map_err(|e| EngineError::invalid(format!("Invalid JSONPath: {e}")))?;
    let located = p.query_located(value);
    let results: Vec<Value> = located.iter().map(|n| n.node().clone()).collect();
    let locations: Vec<String> = located.iter().map(|n| n.location().to_string()).collect();
    Ok(json!({
        "results": results,
        "count": results.len(),
        "locations": locations,
        "first": results.first().cloned()
    }))
}

// -------------------------------------------------------------- schema ----

/// Validates `data` against `schema` (type, required, enum, ranges, patterns, items, properties).
pub fn schema_validate(schema: &Value, data: &Value) -> EngineResult<Value> {
    let validator = jsonschema::validator_for(schema)
        .map_err(|e| EngineError::invalid(format!("Invalid JSON Schema: {e}")))?;
    let errors: Vec<Value> = validator
        .iter_errors(data)
        .take(500)
        .map(|e| {
            json!({
                "path": e.instance_path.to_string(),
                "schema_path": e.schema_path.to_string(),
                "message": e.to_string(),
                "keyword": e.kind_name()
            })
        })
        .collect();
    Ok(json!({ "valid": errors.is_empty(), "errors": errors, "error_count": errors.len() }))
}

trait KindName {
    fn kind_name(&self) -> String;
}
impl KindName for jsonschema::ValidationError<'_> {
    fn kind_name(&self) -> String {
        format!("{:?}", self.kind)
            .split(['(', ' ', '{'])
            .next()
            .unwrap_or("")
            .to_string()
    }
}

fn merge_schema(a: Value, b: Value) -> Value {
    if a == b {
        return a;
    }
    let (ta, tb) = (a["type"].clone(), b["type"].clone());
    if ta == tb {
        match ta.as_str() {
            Some("object") => {
                let mut props = a["properties"].as_object().cloned().unwrap_or_default();
                let pb = b["properties"].as_object().cloned().unwrap_or_default();
                let ra: Vec<String> = a["required"]
                    .as_array()
                    .map(|r| r.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                    .unwrap_or_default();
                let rb: Vec<String> = b["required"]
                    .as_array()
                    .map(|r| r.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                    .unwrap_or_default();
                for (k, v) in pb {
                    let merged = match props.get(&k) {
                        Some(existing) => merge_schema(existing.clone(), v),
                        None => v,
                    };
                    props.insert(k, merged);
                }
                let required: Vec<String> = ra.into_iter().filter(|k| rb.contains(k)).collect();
                return json!({ "type": "object", "properties": props, "required": required });
            },
            Some("array") => {
                let items = match (a.get("items"), b.get("items")) {
                    (Some(x), Some(y)) => merge_schema(x.clone(), y.clone()),
                    (Some(x), None) | (None, Some(x)) => x.clone(),
                    _ => json!({}),
                };
                return json!({ "type": "array", "items": items });
            },
            _ => {},
        }
    }
    // Different types → anyOf (number/integer collapse to number).
    if (ta == "integer" && tb == "number") || (ta == "number" && tb == "integer") {
        return json!({ "type": "number" });
    }
    let mut variants: Vec<Value> = Vec::new();
    for v in [a, b] {
        if let Some(arr) = v.get("anyOf").and_then(|x| x.as_array()) {
            variants.extend(arr.iter().cloned());
        } else {
            variants.push(v);
        }
    }
    variants.dedup();
    json!({ "anyOf": variants })
}

fn infer_schema(v: &Value, examples: bool) -> Value {
    match v {
        Value::Null => json!({ "type": "null" }),
        Value::Bool(_) => json!({ "type": "boolean" }),
        Value::Number(n) => {
            let mut s =
                json!({ "type": if n.is_i64() || n.is_u64() { "integer" } else { "number" } });
            if examples {
                s["examples"] = json!([n]);
            }
            s
        },
        Value::String(s) => {
            let mut out = json!({ "type": "string" });
            let fmt = if s.len() == 10
                && s.as_bytes()[4] == b'-'
                && s.chars().all(|c| c.is_ascii_digit() || c == '-')
            {
                Some("date")
            } else if s.len() >= 19 && s.contains('T') && s.as_bytes()[4] == b'-' {
                Some("date-time")
            } else if s.contains('@') && s.contains('.') && !s.contains(' ') {
                Some("email")
            } else if s.starts_with("http://") || s.starts_with("https://") {
                Some("uri")
            } else if s.len() == 36 && s.matches('-').count() == 4 {
                Some("uuid")
            } else {
                None
            };
            if let Some(f) = fmt {
                out["format"] = json!(f);
            }
            if examples {
                out["examples"] = json!([s]);
            }
            out
        },
        Value::Array(a) => {
            let items = a
                .iter()
                .map(|x| infer_schema(x, examples))
                .reduce(merge_schema)
                .unwrap_or(json!({}));
            json!({ "type": "array", "items": items })
        },
        Value::Object(m) => {
            let props: Map<String, Value> = m
                .iter()
                .map(|(k, v)| (k.clone(), infer_schema(v, examples)))
                .collect();
            let required: Vec<&String> = m.keys().collect();
            json!({
                "type": "object",
                "properties": props,
                "required": required,
                "additionalProperties": false
            })
        },
    }
}

/// Infers a JSON Schema (draft 2020-12) from sample data.
pub fn schema_generate(data: &Value, title: Option<&str>, examples: bool) -> Value {
    let mut s = infer_schema(data, examples);
    if let Value::Object(m) = &mut s {
        let mut prefixed = Map::new();
        prefixed.insert("$schema".into(), json!("https://json-schema.org/draft/2020-12/schema"));
        if let Some(t) = title {
            prefixed.insert("title".into(), json!(t));
        }
        for (k, v) in m.iter() {
            prefixed.insert(k.clone(), v.clone());
        }
        *m = prefixed;
    }
    s
}

// ------------------------------------------------------------- codegen ----

/// Code-generation target language.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    /// TypeScript interfaces.
    TypeScript,
    /// Rust structs with serde derives.
    Rust,
    /// Go structs with JSON tags.
    Go,
    /// Python dataclasses.
    Python,
    /// Kotlin data classes.
    Kotlin,
    /// Swift Codable structs.
    Swift,
    /// C# classes.
    CSharp,
    /// Java classes.
    Java,
}

impl Lang {
    /// Parses a language name (`ts`, `typescript`, `rust`, `go`, …).
    pub fn parse(s: &str) -> EngineResult<Lang> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "typescript" | "ts" => Lang::TypeScript,
            "rust" | "rs" => Lang::Rust,
            "go" | "golang" => Lang::Go,
            "python" | "py" | "dataclass" | "pydantic" => Lang::Python,
            "kotlin" | "kt" => Lang::Kotlin,
            "swift" => Lang::Swift,
            "csharp" | "c#" | "cs" => Lang::CSharp,
            "java" => Lang::Java,
            other => Err(EngineError::invalid(format!(
                "Unsupported language `{other}` (typescript, rust, go, python, kotlin, swift, \
                 csharp, java)"
            )))?,
        })
    }
}

fn pascal(s: &str) -> String {
    let mut out = String::new();
    let mut up = true;
    for c in s.chars() {
        if c.is_alphanumeric() {
            if up {
                out.extend(c.to_uppercase());
            } else {
                out.push(c);
            }
            up = false;
        } else {
            up = true;
        }
    }
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out = format!("Item{out}");
    }
    out
}

fn snake(s: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if c.is_alphanumeric() {
            if c.is_uppercase()
                && i > 0
                && (chars[i - 1].is_lowercase()
                    || chars.get(i + 1).map(|n| n.is_lowercase()).unwrap_or(false))
                && !out.ends_with('_')
            {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        format!("field_{out}")
    } else {
        out
    }
}

fn camel(s: &str) -> String {
    let p = pascal(s);
    let mut c = p.chars();
    match c.next() {
        Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
        None => p,
    }
}

struct Gen {
    lang: Lang,
    types: IndexMap<String, Vec<(String, String, bool)>>, // type name -> fields (original key, type expr, optional)
    used: std::collections::HashSet<String>,
    optional_nulls: bool,
}

impl Gen {
    fn unique_name(&mut self, base: &str) -> String {
        let mut name = pascal(base);
        let mut n = 2;
        while self.used.contains(&name) {
            name = format!("{}{}", pascal(base), n);
            n += 1;
        }
        self.used.insert(name.clone());
        name
    }

    /// Returns the type expression for `v`, registering nested object types.
    fn type_of(&mut self, v: &Value, name_hint: &str) -> String {
        match v {
            Value::Null => self.null_type(),
            Value::Bool(_) => self.prim("bool"),
            Value::Number(n) => self.prim(if n.is_i64() || n.is_u64() { "int" } else { "float" }),
            Value::String(_) => self.prim("string"),
            Value::Array(a) => {
                let inner = if a.is_empty() {
                    self.any_type()
                } else {
                    // Merge element shapes: objects merge keys; otherwise first element wins.
                    if a.iter().all(|x| x.is_object()) {
                        let mut merged = Map::new();
                        let mut counts: Map<String, Value> = Map::new();
                        for x in a {
                            for (k, val) in x.as_object().into_iter().flatten() {
                                merged.entry(k.clone()).or_insert(val.clone());
                                let c = counts.get(k).and_then(Value::as_u64).unwrap_or(0) + 1;
                                counts.insert(k.clone(), json!(c));
                            }
                        }
                        let optional: Vec<String> = counts
                            .iter()
                            .filter(|(_, c)| c.as_u64().unwrap_or(0) < a.len() as u64)
                            .map(|(k, _)| k.clone())
                            .collect();
                        self.object_type(&merged, &singular(name_hint), &optional)
                    } else {
                        let types: Vec<String> =
                            a.iter().map(|x| self.type_of(x, name_hint)).collect();
                        let first = types[0].clone();
                        if types.iter().all(|t| *t == first) {
                            first
                        } else {
                            self.any_type()
                        }
                    }
                };
                self.array_of(&inner)
            },
            Value::Object(m) => self.object_type(m, name_hint, &[]),
        }
    }

    fn object_type(
        &mut self,
        m: &Map<String, Value>,
        name_hint: &str,
        optional: &[String],
    ) -> String {
        let name = self.unique_name(name_hint);
        let mut fields = Vec::new();
        for (k, v) in m {
            let t = self.type_of(v, k);
            let opt = optional.contains(k) || (self.optional_nulls && v.is_null());
            fields.push((k.clone(), t, opt));
        }
        self.types.insert(name.clone(), fields);
        name
    }

    fn prim(&self, p: &str) -> String {
        match (self.lang, p) {
            (Lang::TypeScript, "bool") => "boolean".into(),
            (Lang::TypeScript, "int" | "float") => "number".into(),
            (Lang::TypeScript, "string") => "string".into(),
            (Lang::Rust, "bool") => "bool".into(),
            (Lang::Rust, "int") => "i64".into(),
            (Lang::Rust, "float") => "f64".into(),
            (Lang::Rust, "string") => "String".into(),
            (Lang::Go, "bool") => "bool".into(),
            (Lang::Go, "int") => "int64".into(),
            (Lang::Go, "float") => "float64".into(),
            (Lang::Go, "string") => "string".into(),
            (Lang::Python, "bool") => "bool".into(),
            (Lang::Python, "int") => "int".into(),
            (Lang::Python, "float") => "float".into(),
            (Lang::Python, "string") => "str".into(),
            (Lang::Kotlin, "bool") => "Boolean".into(),
            (Lang::Kotlin, "int") => "Long".into(),
            (Lang::Kotlin, "float") => "Double".into(),
            (Lang::Kotlin, "string") => "String".into(),
            (Lang::Swift, "bool") => "Bool".into(),
            (Lang::Swift, "int") => "Int".into(),
            (Lang::Swift, "float") => "Double".into(),
            (Lang::Swift, "string") => "String".into(),
            (Lang::CSharp, "bool") => "bool".into(),
            (Lang::CSharp, "int") => "long".into(),
            (Lang::CSharp, "float") => "double".into(),
            (Lang::CSharp, "string") => "string".into(),
            (Lang::Java, "bool") => "Boolean".into(),
            (Lang::Java, "int") => "Long".into(),
            (Lang::Java, "float") => "Double".into(),
            (Lang::Java, "string") => "String".into(),
            _ => "any".into(),
        }
    }

    fn null_type(&self) -> String {
        match self.lang {
            Lang::TypeScript => "null".into(),
            Lang::Rust => "Option<serde_json::Value>".into(),
            Lang::Go => "interface{}".into(),
            Lang::Python => "None".into(),
            Lang::Kotlin => "Any?".into(),
            Lang::Swift => "AnyCodable?".into(),
            Lang::CSharp => "object?".into(),
            Lang::Java => "Object".into(),
        }
    }

    fn any_type(&self) -> String {
        match self.lang {
            Lang::TypeScript => "unknown".into(),
            Lang::Rust => "serde_json::Value".into(),
            Lang::Go => "interface{}".into(),
            Lang::Python => "Any".into(),
            Lang::Kotlin => "Any".into(),
            Lang::Swift => "AnyCodable".into(),
            Lang::CSharp => "object".into(),
            Lang::Java => "Object".into(),
        }
    }

    fn array_of(&self, inner: &str) -> String {
        match self.lang {
            Lang::TypeScript => format!("{inner}[]"),
            Lang::Rust => format!("Vec<{inner}>"),
            Lang::Go => format!("[]{inner}"),
            Lang::Python => format!("list[{inner}]"),
            Lang::Kotlin => format!("List<{inner}>"),
            Lang::Swift => format!("[{inner}]"),
            Lang::CSharp => format!("List<{inner}>"),
            Lang::Java => format!("List<{inner}>"),
        }
    }

    fn render(&self) -> String {
        let mut out = String::new();
        match self.lang {
            Lang::Python => out.push_str(
                "from dataclasses import dataclass, field\nfrom typing import Any, Optional\n\n",
            ),
            Lang::Rust => out.push_str("use serde::{Deserialize, Serialize};\n\n"),
            Lang::Kotlin => out.push_str(
                "import kotlinx.serialization.Serializable\nimport \
                 kotlinx.serialization.SerialName\n\n",
            ),
            Lang::CSharp => out.push_str(
                "using System.Collections.Generic;\nusing System.Text.Json.Serialization;\n\n",
            ),
            Lang::Java => out.push_str(
                "import java.util.List;\nimport com.fasterxml.jackson.annotation.JsonProperty;\n\n",
            ),
            _ => {},
        }
        // Nested types first for languages that care; order doesn't matter in most, so reverse insertion.
        let entries: Vec<(&String, &Vec<(String, String, bool)>)> = self.types.iter().collect();
        for (i, (name, fields)) in entries.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            match self.lang {
                Lang::TypeScript => {
                    out.push_str(&format!("export interface {name} {{\n"));
                    for (k, t, opt) in fields.iter() {
                        let key = if is_ident(k) { k.clone() } else { format!("\"{k}\"") };
                        out.push_str(&format!("  {key}{}: {t};\n", if *opt { "?" } else { "" }));
                    }
                    out.push_str("}\n");
                },
                Lang::Rust => {
                    out.push_str("#[derive(Debug, Clone, Serialize, Deserialize)]\n");
                    out.push_str(&format!("pub struct {name} {{\n"));
                    for (k, t, opt) in fields.iter() {
                        let field = snake(k);
                        if field != *k {
                            out.push_str(&format!("    #[serde(rename = \"{k}\")]\n"));
                        }
                        let ty = if *opt && !t.starts_with("Option<") {
                            format!("Option<{t}>")
                        } else {
                            t.clone()
                        };
                        let field = if is_rust_keyword(&field) {
                            format!("r#{field}")
                        } else {
                            field
                        };
                        out.push_str(&format!("    pub {field}: {ty},\n"));
                    }
                    out.push_str("}\n");
                },
                Lang::Go => {
                    out.push_str(&format!("type {name} struct {{\n"));
                    for (k, t, opt) in fields.iter() {
                        let ty = if *opt
                            && !t.starts_with('*')
                            && !t.starts_with("[]")
                            && !t.starts_with("interface")
                        {
                            format!("*{t}")
                        } else {
                            t.clone()
                        };
                        out.push_str(&format!(
                            "\t{} {} `json:\"{k}{}\"`\n",
                            pascal(k),
                            ty,
                            if *opt { ",omitempty" } else { "" }
                        ));
                    }
                    out.push_str("}\n");
                },
                Lang::Python => {
                    out.push_str(&format!("@dataclass\nclass {name}:\n"));
                    if fields.is_empty() {
                        out.push_str("    pass\n");
                    }
                    // Required first (dataclass ordering rule).
                    let mut ordered: Vec<&(String, String, bool)> = fields.iter().collect();
                    ordered.sort_by_key(|(_, _, opt)| *opt);
                    for (k, t, opt) in ordered {
                        let ty = if t == "None" {
                            "Optional[Any]".to_string()
                        } else if *opt {
                            format!("Optional[{t}]")
                        } else {
                            t.clone()
                        };
                        out.push_str(&format!(
                            "    {}: {ty}{}\n",
                            snake(k),
                            if *opt || t == "None" { " = None" } else { "" }
                        ));
                    }
                },
                Lang::Kotlin => {
                    out.push_str(&format!("@Serializable\ndata class {name}(\n"));
                    for (k, t, opt) in fields.iter() {
                        let prop = camel(k);
                        if prop != *k {
                            out.push_str(&format!("    @SerialName(\"{k}\")\n"));
                        }
                        let ty = if *opt && !t.ends_with('?') {
                            format!("{t}?")
                        } else {
                            t.clone()
                        };
                        out.push_str(&format!(
                            "    val {prop}: {ty}{},\n",
                            if *opt { " = null" } else { "" }
                        ));
                    }
                    out.push_str(")\n");
                },
                Lang::Swift => {
                    out.push_str(&format!("struct {name}: Codable {{\n"));
                    let mut needs_keys = false;
                    for (k, t, opt) in fields.iter() {
                        let prop = camel(k);
                        if prop != *k {
                            needs_keys = true;
                        }
                        let ty = if *opt && !t.ends_with('?') {
                            format!("{t}?")
                        } else {
                            t.clone()
                        };
                        out.push_str(&format!("    let {prop}: {ty}\n"));
                    }
                    if needs_keys {
                        out.push_str("\n    enum CodingKeys: String, CodingKey {\n");
                        for (k, _, _) in fields.iter() {
                            let prop = camel(k);
                            if prop != *k {
                                out.push_str(&format!("        case {prop} = \"{k}\"\n"));
                            } else {
                                out.push_str(&format!("        case {prop}\n"));
                            }
                        }
                        out.push_str("    }\n");
                    }
                    out.push_str("}\n");
                },
                Lang::CSharp => {
                    out.push_str(&format!("public class {name}\n{{\n"));
                    for (k, t, opt) in fields.iter() {
                        let ty = if *opt && !t.ends_with('?') {
                            format!("{t}?")
                        } else {
                            t.clone()
                        };
                        out.push_str(&format!(
                            "    [JsonPropertyName(\"{k}\")]\n    public {ty} {} {{ get; set; }}\n",
                            pascal(k)
                        ));
                    }
                    out.push_str("}\n");
                },
                Lang::Java => {
                    out.push_str(&format!("public class {name} {{\n"));
                    for (k, t, _) in fields.iter() {
                        out.push_str(&format!(
                            "    @JsonProperty(\"{k}\")\n    public {t} {};\n",
                            camel(k)
                        ));
                    }
                    out.push_str("}\n");
                },
            }
        }
        out
    }
}

fn singular(s: &str) -> String {
    if s.ends_with("ies") && s.len() > 3 {
        format!("{}y", &s[..s.len() - 3])
    } else if s.ends_with('s') && !s.ends_with("ss") && s.len() > 1 {
        s[..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn is_ident(s: &str) -> bool {
    let mut c = s.chars();
    matches!(c.next(), Some(f) if f.is_alphabetic() || f == '_' || f == '$')
        && c.all(|x| x.is_alphanumeric() || x == '_' || x == '$')
}

fn is_rust_keyword(s: &str) -> bool {
    matches!(
        s,
        "type"
            | "ref"
            | "match"
            | "struct"
            | "enum"
            | "impl"
            | "fn"
            | "mod"
            | "use"
            | "pub"
            | "self"
            | "super"
            | "crate"
            | "move"
            | "loop"
            | "where"
            | "async"
            | "await"
            | "dyn"
            | "abstract"
            | "box"
            | "const"
            | "static"
            | "trait"
            | "in"
            | "as"
            | "if"
            | "else"
            | "for"
            | "while"
            | "let"
            | "mut"
            | "return"
            | "true"
            | "false"
            | "yield"
    )
}

/// Generates typed model code for the given JSON sample.
pub fn to_code(
    value: &Value,
    lang: &str,
    root_name: &str,
    optional_nulls: bool,
) -> EngineResult<Value> {
    let lang = Lang::parse(lang)?;
    if !value.is_object() && !value.is_array() {
        return Err(EngineError::invalid("JSON root must be an object or an array of objects"));
    }
    let mut g = Gen {
        lang,
        types: IndexMap::new(),
        used: std::collections::HashSet::new(),
        optional_nulls,
    };
    let name = if root_name.trim().is_empty() {
        "Root"
    } else {
        root_name.trim()
    };
    let root_type = g.type_of(value, name);
    let mut code = g.render();
    if value.is_array() {
        let alias = match lang {
            Lang::TypeScript => format!("\nexport type {} = {root_type};\n", pascal(name)),
            Lang::Rust => format!("\npub type {} = {root_type};\n", pascal(name)),
            Lang::Go => format!("\ntype {} {root_type}\n", pascal(name)),
            Lang::Python => format!("\n{} = {root_type}\n", pascal(name)),
            Lang::Kotlin => format!("\ntypealias {} = {root_type}\n", pascal(name)),
            Lang::Swift => format!("\ntypealias {} = {root_type}\n", pascal(name)),
            _ => String::new(),
        };
        code.push_str(&alias);
    }
    Ok(json!({
        "code": code,
        "language": format!("{:?}", lang).to_lowercase(),
        "types": g.types.keys().cloned().collect::<Vec<_>>()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diffs_and_patches() {
        let a = json!({ "a": 1, "b": [1, 2], "c": { "d": "x" }, "gone": true });
        let b = json!({ "a": "1", "b": [1, 2, 3], "c": { "d": "y" }, "new": null });
        let d = diff(&a, &b, false);
        assert_eq!(d["identical"], false);
        assert_eq!(d["added"], 2);
        assert_eq!(d["removed"], 1);
        assert_eq!(d["changed"], 2);
        let patched = apply_patch(&a, d["changes"].as_array().unwrap()).unwrap();
        assert_eq!(patched, b);
        assert_eq!(diff(&json!([1, 2]), &json!([2, 1]), true)["identical"], true);
        assert_eq!(diff(&json!([1, 2]), &json!([2, 1]), false)["changed"], 2);
    }

    #[test]
    fn jsonpath_and_schema() {
        let v = json!({
            "store": { "book": [{ "price": 8.95, "title": "A" }, { "price": 12.99, "title": "B" }] }
        });
        let q = path_query(&v, "$.store.book[?@.price > 10].title").unwrap();
        assert_eq!(q["results"], json!(["B"]));
        assert_eq!(q["count"], 1);
        assert!(path_query(&v, "$[").is_err());
        assert!(path_query(&v, "").is_err());
        let schema = json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "integer", "minimum": 0 }
            }
        });
        assert_eq!(
            schema_validate(&schema, &json!({ "name": "x", "age": 3 })).unwrap()["valid"],
            true
        );
        let bad = schema_validate(&schema, &json!({ "age": -1 })).unwrap();
        assert_eq!(bad["valid"], false);
        assert_eq!(bad["error_count"], 2);
        assert!(schema_validate(&json!({ "type": "nope" }), &json!(1)).is_err());
        let gen = schema_generate(
            &json!({
                "id": 1,
                "tags": ["a"],
                "meta": { "ok": true, "when": "2024-01-31" },
                "items": [{ "x": 1 }, { "x": 2, "y": "z" }]
            }),
            Some("T"),
            false,
        );
        assert_eq!(gen["title"], "T");
        assert_eq!(gen["properties"]["id"]["type"], "integer");
        assert_eq!(gen["properties"]["tags"]["items"]["type"], "string");
        assert_eq!(gen["properties"]["meta"]["properties"]["when"]["format"], "date");
        assert_eq!(gen["properties"]["items"]["items"]["required"], json!(["x"]));
        assert_eq!(
            schema_validate(
                &gen,
                &json!({ "id": 1, "tags": [], "meta": { "ok": false, "when": "x" }, "items": [] })
            )
            .unwrap()["valid"],
            true
        );
    }

    #[test]
    fn codegen() {
        let v = json!({
            "user_id": 1,
            "name": "x",
            "tags": ["a"],
            "address": { "zip": "1", "geo": { "lat": 1.5 } },
            "orders": [{ "id": 1 }, { "id": 2, "note": "n" }],
            "type": null
        });
        let ts = to_code(&v, "typescript", "Person", true).unwrap();
        let code = ts["code"].as_str().unwrap();
        assert!(code.contains("export interface Person {"));
        assert!(code.contains("user_id: number;"));
        assert!(code.contains("orders: Order[];"));
        assert!(code.contains("note?: string;"));
        assert!(code.contains("type?: null;"));
        let rs = to_code(&v, "rust", "Person", true).unwrap();
        let code = rs["code"].as_str().unwrap();
        assert!(code.contains("pub struct Person {"));
        assert!(code.contains("pub user_id: i64,"));
        assert!(code.contains("pub r#type: Option<serde_json::Value>,"));
        assert!(code.contains("pub note: Option<String>,"));
        let go = to_code(&v, "go", "Person", false).unwrap();
        assert!(go["code"]
            .as_str()
            .unwrap()
            .contains("UserId int64 `json:\"user_id\"`"));
        let py = to_code(&v, "python", "Person", false).unwrap();
        assert!(py["code"].as_str().unwrap().contains("class Person:"));
        for l in ["kotlin", "swift", "csharp", "java"] {
            assert!(to_code(&v, l, "P", false).is_ok(), "{l}");
        }
        let arr = to_code(&json!([{ "a": 1 }]), "typescript", "Items", false).unwrap();
        assert!(arr["code"].as_str().unwrap().contains("export type Items = Item[];"));
        assert!(to_code(&json!(1), "typescript", "X", false).is_err());
        assert!(to_code(&v, "cobol", "X", false).is_err());
    }
}
