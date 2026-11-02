//! # Dev Toolbox JSON & Structured Data Engine (`json-engine`)
//!
//! Serves the `json` catalog engine and the `xml_yaml_toml` engine (the two
//! share parsers, so they are shipped as a single binary).
//!
//! | Action | Tool(s) |
//! |---|---|
//! | `format` / `minify` / `validate` / `repair` | json-formatter, json-beautifier, json-minifier, json-validator |
//! | `escape` / `unescape` | json-escape, json-unescape |
//! | `sort_keys` / `sort_array` / `flatten` / `unflatten` / `to_table` | json-key-sorter, json-array-sorter, json-flattener, json-unflattener, json-to-table |
//! | `diff` / `apply_patch` / `path_query` | json-diff, json-compare, json-path-query |
//! | `schema_validate` / `schema_generate` | json-schema-validator, json-schema-generator |
//! | `to_code` | json-to-typescript, json-to-rust-struct, json-to-go-struct, json-to-code |
//! | `to_yaml` / `from_yaml` / `yaml_format` / `yaml_validate` | json-to-yaml, yaml-to-json, yaml-formatter, yaml-validator |
//! | `to_xml` / `from_xml` / `xml_format` / `xml_minify` / `xml_validate` | json-to-xml, xml-to-json, xml-formatter, xml-minifier, xml-validator |
//! | `to_toml` / `from_toml` / `toml_format` / `toml_validate` | json-to-toml, toml-to-json, toml-formatter, toml-validator |
//! | `from_ini` / `to_ini` / `ini_format` / `from_plist` | ini-to-json, ini-formatter, plist-viewer |
//! | `to_csv` / `from_csv` / `to_jsonl` / `from_jsonl` | json-to-csv, csv-to-json, csv-to-jsonl, jsonl-to-csv |
//!
//! All text inputs are strings; every parse error carries a position.

pub mod convert;
pub mod core;
pub mod tools;

use engine_shared::{limits, EngineError, EngineResult};
use serde::Deserialize;
use serde_json::{json, Value};

fn default_indent() -> usize {
    2
}
fn default_true() -> bool {
    true
}
fn default_dot() -> String {
    ".".into()
}
fn default_dots() -> String {
    "dots".into()
}
fn default_root() -> String {
    "root".into()
}
fn default_at() -> String {
    "@".into()
}
fn default_text_key() -> String {
    "#text".into()
}
fn default_lang() -> String {
    "typescript".into()
}

fn check(text: &str) -> EngineResult<()> {
    limits::check_size(text.len(), limits::MAX_TEXT_CHARS, "input")
}

/// Every action understood by the JSON engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum JsonAction {
    /// Pretty-prints JSON.
    Format {
        /// JSON source.
        text: String,
        /// Indent width in spaces.
        #[serde(default = "default_indent")]
        indent: usize,
        /// Indent with tabs instead of spaces.
        #[serde(default)]
        tabs: bool,
        /// Sort object keys.
        #[serde(default)]
        sort_keys: bool,
    },
    /// Removes insignificant whitespace.
    Minify {
        /// JSON source.
        text: String,
    },
    /// Validates JSON and reports the error position.
    Validate {
        /// JSON source.
        text: String,
    },
    /// Repairs common mistakes (comments, trailing commas, single quotes, unquoted keys).
    Repair {
        /// JSON-ish source.
        text: String,
        /// Indent width for the repaired output.
        #[serde(default = "default_indent")]
        indent: usize,
    },
    /// Escapes text for embedding in a JSON string.
    Escape {
        /// Text to escape.
        text: String,
        /// Wrap the result in quotes.
        #[serde(default)]
        quotes: bool,
    },
    /// Unescapes a JSON string literal.
    Unescape {
        /// Escaped text.
        text: String,
    },
    /// Sorts object keys recursively.
    SortKeys {
        /// JSON source.
        text: String,
        /// Indent width.
        #[serde(default = "default_indent")]
        indent: usize,
    },
    /// Sorts a JSON array (optionally by key).
    SortArray {
        /// JSON source.
        text: String,
        /// Object key to sort by.
        #[serde(default)]
        key: Option<String>,
        /// Descending order.
        #[serde(default)]
        descending: bool,
        /// Numeric comparison.
        #[serde(default)]
        numeric: bool,
        /// Remove duplicates.
        #[serde(default)]
        unique: bool,
    },
    /// Flattens nested objects into dotted keys.
    Flatten {
        /// JSON source.
        text: String,
        /// Key separator.
        #[serde(default = "default_dot")]
        separator: String,
        /// `dots` (`a.0`) or `brackets` (`a[0]`) for array indices.
        #[serde(default = "default_dots")]
        array_style: String,
    },
    /// Rebuilds nested objects from flattened keys.
    Unflatten {
        /// JSON source.
        text: String,
        /// Key separator.
        #[serde(default = "default_dot")]
        separator: String,
    },
    /// Converts arrays of objects to a table (plus Markdown/HTML).
    ToTable {
        /// JSON source.
        text: String,
        /// Maximum rows returned.
        #[serde(default)]
        max_rows: usize,
    },
    /// Structural diff of two documents (JSON Patch style).
    Diff {
        /// Left document.
        a: String,
        /// Right document.
        b: String,
        /// Compare arrays as sets.
        #[serde(default)]
        ignore_order: bool,
    },
    /// Applies an RFC 6902 JSON Patch.
    ApplyPatch {
        /// JSON source.
        text: String,
        /// Patch operations.
        patch: Vec<Value>,
    },
    /// JSONPath query.
    PathQuery {
        /// JSON source.
        text: String,
        /// JSONPath expression (e.g. `$.items[*].name`).
        path: String,
    },
    /// Validates data against a JSON Schema (draft 7 subset).
    SchemaValidate {
        /// Schema JSON.
        schema: String,
        /// Data JSON.
        data: String,
    },
    /// Infers a JSON Schema from sample data.
    SchemaGenerate {
        /// JSON source.
        text: String,
        /// Schema title.
        #[serde(default)]
        title: Option<String>,
        /// Include `examples` in the schema.
        #[serde(default)]
        examples: bool,
    },
    /// Generates typed code (TypeScript, Rust, Go, Python, Kotlin, Swift, C#, Java).
    ToCode {
        /// JSON source.
        text: String,
        /// `typescript`, `rust`, `go`, `python`, `kotlin`, `swift`, `csharp` or `java`.
        #[serde(default = "default_lang")]
        language: String,
        /// Name of the root type.
        #[serde(default = "default_root")]
        root_name: String,
        /// Treat `null` fields as optional.
        #[serde(default = "default_true")]
        optional_nulls: bool,
    },

    /// JSON → YAML.
    ToYaml {
        /// JSON source.
        text: String,
    },
    /// YAML → JSON.
    FromYaml {
        /// YAML source.
        text: String,
        /// Indent width.
        #[serde(default = "default_indent")]
        indent: usize,
    },
    /// Normalises YAML formatting.
    YamlFormat {
        /// YAML source.
        text: String,
    },
    /// Validates YAML.
    YamlValidate {
        /// YAML source.
        text: String,
    },

    /// JSON → XML.
    ToXml {
        /// JSON source.
        text: String,
        /// Root element name.
        #[serde(default = "default_root")]
        root: String,
        /// Prefix marking attributes.
        #[serde(default = "default_at")]
        attribute_prefix: String,
        /// Key used for text content.
        #[serde(default = "default_text_key")]
        text_key: String,
        /// Indent width.
        #[serde(default = "default_indent")]
        indent: usize,
        /// Emit the XML declaration.
        #[serde(default = "default_true")]
        declaration: bool,
    },
    /// XML → JSON.
    FromXml {
        /// XML source.
        text: String,
        /// Prefix marking attributes.
        #[serde(default = "default_at")]
        attribute_prefix: String,
        /// Key used for text content.
        #[serde(default = "default_text_key")]
        text_key: String,
        /// Convert numeric/boolean text to JSON values.
        #[serde(default = "default_true")]
        parse_values: bool,
        /// Indent width.
        #[serde(default = "default_indent")]
        indent: usize,
    },
    /// Pretty-prints XML.
    XmlFormat {
        /// XML source.
        text: String,
        /// Indent width.
        #[serde(default = "default_indent")]
        indent: usize,
        /// Keep comments.
        #[serde(default = "default_true")]
        keep_comments: bool,
    },
    /// Minifies XML.
    XmlMinify {
        /// XML source.
        text: String,
    },
    /// Checks XML well-formedness.
    XmlValidate {
        /// XML source.
        text: String,
    },

    /// JSON → TOML.
    ToToml {
        /// JSON source.
        text: String,
    },
    /// TOML → JSON.
    FromToml {
        /// TOML source.
        text: String,
        /// Indent width.
        #[serde(default = "default_indent")]
        indent: usize,
    },
    /// Normalises TOML formatting.
    TomlFormat {
        /// TOML source.
        text: String,
    },
    /// Validates TOML.
    TomlValidate {
        /// TOML source.
        text: String,
    },

    /// INI → JSON.
    FromIni {
        /// INI source.
        text: String,
        /// Section name for keys outside any section.
        #[serde(default)]
        default_section: String,
        /// Convert numeric/boolean text to JSON values.
        #[serde(default = "default_true")]
        parse_values: bool,
    },
    /// JSON → INI.
    ToIni {
        /// JSON source.
        text: String,
    },
    /// Normalises INI formatting.
    IniFormat {
        /// INI source.
        text: String,
    },
    /// Apple plist (XML) → JSON.
    FromPlist {
        /// Plist source.
        text: String,
        /// Indent width.
        #[serde(default = "default_indent")]
        indent: usize,
    },

    /// JSON array → CSV.
    ToCsv {
        /// JSON source.
        text: String,
        /// Delimiter (default `,`).
        #[serde(default)]
        delimiter: Option<String>,
        /// Emit a header row.
        #[serde(default = "default_true")]
        headers: bool,
    },
    /// CSV → JSON.
    FromCsv {
        /// CSV source.
        text: String,
        /// Delimiter (auto-detected when omitted).
        #[serde(default)]
        delimiter: Option<String>,
        /// First row is a header.
        #[serde(default = "default_true")]
        headers: bool,
        /// Convert numeric/boolean cells.
        #[serde(default = "default_true")]
        infer_types: bool,
        /// Trim cell whitespace.
        #[serde(default = "default_true")]
        trim: bool,
        /// Indent width.
        #[serde(default = "default_indent")]
        indent: usize,
    },
    /// JSON array → JSON Lines.
    ToJsonl {
        /// JSON source.
        text: String,
    },
    /// JSON Lines → JSON array.
    FromJsonl {
        /// JSONL source.
        text: String,
        /// Indent width.
        #[serde(default = "default_indent")]
        indent: usize,
    },
}

fn out(value: &Value, indent: usize) -> EngineResult<Value> {
    Ok(json!({ "result": value, "text": core::format(value, indent, false, false)? }))
}

/// Dispatches a [`JsonAction`] to its implementation.
pub fn handle_action(action: JsonAction) -> EngineResult<Value> {
    match action {
        JsonAction::Format { text, indent, tabs, sort_keys } => {
            check(&text)?;
            let v = core::parse(&text)?;
            let formatted = core::format(&v, indent, tabs, sort_keys)?;
            Ok(json!({
                "result": formatted,
                "bytes_before": text.len(),
                "bytes_after": formatted.len()
            }))
        },
        JsonAction::Minify { text } => {
            check(&text)?;
            let v = core::parse(&text)?;
            let min = serde_json::to_string(&v)?;
            Ok(json!({
                "result": min,
                "bytes_before": text.len(),
                "bytes_after": min.len(),
                "saved_percent": if !text.is_empty() { ((text.len() as f64 - min.len() as f64) / text.len() as f64 * 1000.0).round() / 10.0 } else { 0.0 }
            }))
        },
        JsonAction::Validate { text } => {
            check(&text)?;
            Ok(core::validate(&text))
        },
        JsonAction::Repair { text, indent } => {
            check(&text)?;
            if text.trim().is_empty() {
                return Err(EngineError::empty("text"));
            }
            let v = core::repair(&text)?;
            Ok(json!({
                "result": core::format(&v, indent, false, false)?,
                "value": v,
                "was_valid": serde_json::from_str::<Value>(&text).is_ok()
            }))
        },
        JsonAction::Escape { text, quotes } => Ok(json!({ "result": core::escape(&text, quotes) })),
        JsonAction::Unescape { text } => Ok(json!({ "result": core::unescape(&text)? })),
        JsonAction::SortKeys { text, indent } => {
            let v = core::sort_keys_recursive(&core::parse(&text)?);
            out(&v, indent)
        },
        JsonAction::SortArray { text, key, descending, numeric, unique } => {
            core::sort_array(&core::parse(&text)?, key.as_deref(), descending, numeric, unique)
        },
        JsonAction::Flatten { text, separator, array_style } => {
            core::flatten(&core::parse(&text)?, &separator, &array_style)
        },
        JsonAction::Unflatten { text, separator } => {
            core::unflatten(&core::parse(&text)?, &separator)
        },
        JsonAction::ToTable { text, max_rows } => core::to_table(&core::parse(&text)?, max_rows),
        JsonAction::Diff { a, b, ignore_order } => {
            let va = core::parse(&a)
                .map_err(|e| EngineError::new(e.code, format!("A: {}", e.message)))?;
            let vb = core::parse(&b)
                .map_err(|e| EngineError::new(e.code, format!("B: {}", e.message)))?;
            Ok(tools::diff(&va, &vb, ignore_order))
        },
        JsonAction::ApplyPatch { text, patch } => {
            let v = tools::apply_patch(&core::parse(&text)?, &patch)?;
            out(&v, 2)
        },
        JsonAction::PathQuery { text, path } => tools::path_query(&core::parse(&text)?, &path),
        JsonAction::SchemaValidate { schema, data } => {
            let s = core::parse(&schema)
                .map_err(|e| EngineError::new(e.code, format!("Schema: {}", e.message)))?;
            let d = core::parse(&data)
                .map_err(|e| EngineError::new(e.code, format!("Data: {}", e.message)))?;
            tools::schema_validate(&s, &d)
        },
        JsonAction::SchemaGenerate { text, title, examples } => {
            let s = tools::schema_generate(&core::parse(&text)?, title.as_deref(), examples);
            out(&s, 2)
        },
        JsonAction::ToCode {
            text,
            language,
            root_name,
            optional_nulls,
        } => tools::to_code(&core::parse(&text)?, &language, &root_name, optional_nulls),

        JsonAction::ToYaml { text } => {
            Ok(json!({ "result": convert::to_yaml(&core::parse(&text)?)? }))
        },
        JsonAction::FromYaml { text, indent } => {
            check(&text)?;
            out(&convert::from_yaml(&text)?, indent)
        },
        JsonAction::YamlFormat { text } => {
            check(&text)?;
            Ok(json!({ "result": convert::to_yaml(&convert::from_yaml(&text)?)? }))
        },
        JsonAction::YamlValidate { text } => Ok(match convert::from_yaml(&text) {
            Ok(v) => {
                json!({
                    "valid": true,
                    "root_type": match &v { Value::Object(_) => "mapping", Value::Array(_) => "sequence", _ => "scalar" },
                    "documents": match v.as_array() { Some(a) if text.contains("\n---") => a.len(), _ => 1 }
                })
            },
            Err(e) => json!({ "valid": false, "error": e.message, "error_code": e.code.as_str() }),
        }),

        JsonAction::ToXml {
            text,
            root,
            attribute_prefix,
            text_key,
            indent,
            declaration,
        } => {
            let opts = convert::XmlOptions {
                root,
                attribute_prefix,
                text_key,
                indent,
                declaration,
                ..Default::default()
            };
            Ok(json!({ "result": convert::to_xml(&core::parse(&text)?, &opts)? }))
        },
        JsonAction::FromXml {
            text,
            attribute_prefix,
            text_key,
            parse_values,
            indent,
        } => {
            check(&text)?;
            let opts = convert::XmlOptions {
                attribute_prefix,
                text_key,
                parse_values,
                ..Default::default()
            };
            let r = convert::from_xml(&text, &opts)?;
            Ok(json!({
                "result": r["result"],
                "root": r["root"],
                "text": core::format(&r["result"], indent, false, false)?
            }))
        },
        JsonAction::XmlFormat { text, indent, keep_comments } => {
            check(&text)?;
            Ok(json!({ "result": convert::format_xml(&text, indent.max(1), keep_comments)? }))
        },
        JsonAction::XmlMinify { text } => {
            check(&text)?;
            let r = convert::format_xml(&text, 0, false)?;
            Ok(json!({ "result": r, "bytes_before": text.len(), "bytes_after": r.len() }))
        },
        JsonAction::XmlValidate { text } => {
            check(&text)?;
            Ok(convert::validate_xml(&text))
        },

        JsonAction::ToToml { text } => {
            Ok(json!({ "result": convert::to_toml(&core::parse(&text)?)? }))
        },
        JsonAction::FromToml { text, indent } => {
            check(&text)?;
            out(&convert::from_toml(&text)?, indent)
        },
        JsonAction::TomlFormat { text } => {
            Ok(json!({ "result": convert::to_toml(&convert::from_toml(&text)?)? }))
        },
        JsonAction::TomlValidate { text } => Ok(match convert::from_toml(&text) {
            Ok(v) => {
                json!({
                    "valid": true,
                    "tables": v.as_object().map(|m| m.values().filter(|x| x.is_object()).count()).unwrap_or(0),
                    "keys": v.as_object().map(|m| m.len()).unwrap_or(0)
                })
            },
            Err(e) => json!({ "valid": false, "error": e.message, "error_code": e.code.as_str() }),
        }),

        JsonAction::FromIni { text, default_section, parse_values } => {
            convert::from_ini(&text, &default_section, parse_values)
        },
        JsonAction::ToIni { text } => {
            Ok(json!({ "result": convert::to_ini(&core::parse(&text)?)? }))
        },
        JsonAction::IniFormat { text } => convert::format_ini(&text),
        JsonAction::FromPlist { text, indent } => {
            let r = convert::from_plist(&text)?;
            out(&r["result"], indent)
        },

        JsonAction::ToCsv { text, delimiter, headers } => {
            convert::to_csv(&core::parse(&text)?, delimiter.as_deref(), headers)
        },
        JsonAction::FromCsv {
            text,
            delimiter,
            headers,
            infer_types,
            trim,
            indent,
        } => {
            check(&text)?;
            let r = convert::from_csv(&text, delimiter.as_deref(), headers, infer_types, trim)?;
            Ok(json!({
                "result": r["result"],
                "rows": r["rows"],
                "columns": r["columns"],
                "delimiter": r["delimiter"],
                "warnings": r["warnings"],
                "text": core::format(&r["result"], indent, false, false)?
            }))
        },
        JsonAction::ToJsonl { text } => {
            Ok(json!({ "result": convert::to_jsonl(&core::parse(&text)?)? }))
        },
        JsonAction::FromJsonl { text, indent } => out(&convert::from_jsonl(&text)?, indent),
    }
}

engine_shared::export_engine!(handle_action, JsonAction);
