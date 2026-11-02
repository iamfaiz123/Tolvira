//! # Tabular core (`tabular-core`)
//!
//! Library half of the CSV engine, shared with the spreadsheet engine.
//! The `csv-engine` crate wraps [`handle_action`] with the WASM FFI exports.
//!
//! Tabular data utilities on top of the `csv` crate. Input is accepted as
//! inline `text` or an uploaded `file` (base64) in any common encoding
//! (UTF-8/16 with or without BOM, Windows-1252 fallback). Parsing is lenient:
//! ragged rows are padded and reported as `issues` instead of failing, blank
//! and duplicate headers are normalised, and the delimiter is sniffed.
//!
//! | Action | Tool(s) |
//! |---|---|
//! | `parse` | csv-viewer, csv-editor (load) |
//! | `detect` | csv-viewer, csv-encoding-converter (inspection) |
//! | `write` | csv-editor (save grid / apply cell edits) |
//! | `clean` | csv-cleaner |
//! | `validate` | csv-validator |
//! | `convert` | csv-to-tsv, tsv-to-csv (any delimiter / line ending / quoting) |
//! | `merge` | csv-merge (append or key join) |
//! | `split` | csv-split |
//! | `dedupe` | csv-deduplicator |
//! | `select_columns` | csv-column-extractor |
//! | `reorder_columns` | csv-column-reorder |
//! | `filter` | csv-filter |
//! | `sort` | csv-sort |
//! | `to_sql` | csv-to-sql |
//! | `to_json` / `to_jsonl` / `from_json` / `from_jsonl` | csv-to-jsonl, jsonl-to-csv |
//! | `statistics` | csv-statistics |
//! | `transpose` | csv-transpose |
//! | `diff` | csv-compare, csv-diff |
//! | `encoding_convert` | csv-encoding-converter |
//! | `transform` | csv-editor / csv-cleaner column operations |
//! | `to_markdown` / `to_html` | csv-viewer export |
//!
//! `csv-to-excel` / `excel-to-csv` are served by the spreadsheet engine
//! (`from_csv` / `to_csv`), which depends on this crate's table model.

pub mod input;
pub mod ops;
pub mod table;

use engine_shared::{EngineError, EngineResult};
use input::Source;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use table::{CsvOptions, Table};

const MAX_SPLIT_PARTS: usize = 1000;

fn load(src: &Source, opts: &CsvOptions) -> EngineResult<(Table, input::Decoded)> {
    let decoded = src.decode()?;
    let table = Table::parse(&decoded.text, opts)?;
    Ok((table, decoded))
}

fn output(t: &Table, opts: &CsvOptions, extra: Map<String, Value>) -> EngineResult<Value> {
    let mut m = Map::new();
    m.insert("csv".into(), json!(t.to_csv(opts)?));
    m.insert("headers".into(), json!(t.headers));
    m.insert("rows".into(), json!(t.rows.len()));
    m.insert("columns".into(), json!(t.width()));
    if !t.issues.is_empty() {
        m.insert("issues".into(), t.issues_json());
    }
    m.extend(extra);
    Ok(Value::Object(m))
}

// ------------------------------------------------------------------ parse --

/// Parses a source into a paginated JSON table with inferred column types.
pub fn parse(
    src: &Source,
    opts: &CsvOptions,
    offset: usize,
    limit: usize,
    typed: bool,
) -> EngineResult<Value> {
    let (t, d) = load(src, opts)?;
    let types = t.column_types();
    let limit = if limit == 0 { 1000 } else { limit.min(100_000) };
    let rows = if typed {
        t.to_objects(true)
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect::<Vec<_>>()
    } else {
        t.to_json_rows(offset, limit)
    };
    Ok(json!({
        "headers": t.headers,
        "columns": t.headers.iter().enumerate().map(|(i, h)| json!({ "index": i, "name": h, "type": types[i].label() })).collect::<Vec<_>>(),
        "rows": rows,
        "offset": offset,
        "limit": limit,
        "total_rows": t.rows.len(),
        "total_columns": t.width(),
        "has_more": offset + limit < t.rows.len(),
        "delimiter": table::delimiter_label(t.delimiter),
        "source": input::decoded_json(&d),
        "issues": t.issues,
        "issue_count": t.issues.len(),
    }))
}

/// Reports delimiter, line endings, encoding, header guess and row count.
pub fn detect(src: &Source) -> EngineResult<Value> {
    let d = src.decode()?;
    let s = table::sniff(&d.text);
    let preview = d.text.lines().take(5).collect::<Vec<_>>();
    let t = Table::parse(&d.text, &CsvOptions::default()).ok();
    Ok(json!({
        "encoding": d.encoding,
        "had_bom": d.had_bom,
        "had_replacement_characters": d.had_replacements,
        "delimiter": s.delimiter,
        "line_ending": s.line_ending,
        "quote_char": s.quote_char,
        "line_count": s.line_count,
        "bytes": src.file.as_ref().map(Vec::len).unwrap_or_else(|| d.text.len()),
        "columns": t.as_ref().map(|t| t.width()),
        "rows": t.as_ref().map(|t| t.rows.len()),
        "headers": t.as_ref().map(|t| t.headers.clone()),
        "issue_count": t.as_ref().map(|t| t.issues.len()),
        "preview": preview,
    }))
}

// ------------------------------------------------------------------ write --

/// One edit applied by the `write` action.
#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Edit {
    /// Sets a single cell.
    SetCell {
        /// 0-based row index.
        row: usize,
        /// Column name or index.
        column: String,
        /// New value.
        value: Value,
    },
    /// Appends or inserts a row.
    AddRow {
        /// Cell values in column order.
        #[serde(default)]
        values: Vec<Value>,
        /// Insert position (append when omitted).
        #[serde(default)]
        at: Option<usize>,
    },
    /// Deletes one row.
    DeleteRow {
        /// 0-based row index.
        row: usize,
    },
    /// Deletes several rows.
    DeleteRows {
        /// 0-based row indices.
        rows: Vec<usize>,
    },
    /// Adds a column.
    AddColumn {
        /// Column name.
        name: String,
        /// Default value for existing rows.
        #[serde(default)]
        default: Value,
        /// Insert position (append when omitted).
        #[serde(default)]
        at: Option<usize>,
    },
    /// Deletes a column.
    DeleteColumn {
        /// Column name or index.
        column: String,
    },
    /// Renames a column.
    RenameColumn {
        /// Column name or index.
        column: String,
        /// New name.
        name: String,
    },
    /// Moves a column.
    MoveColumn {
        /// Column name or index.
        column: String,
        /// New 0-based position.
        to: usize,
    },
    /// Moves a row.
    MoveRow {
        /// 0-based row index.
        row: usize,
        /// New 0-based position.
        to: usize,
    },
}

fn apply_edits(t: &mut Table, edits: &[Edit]) -> EngineResult<()> {
    for e in edits {
        match e {
            Edit::SetCell { row, column, value } => {
                let ci = t.column_index(column)?;
                let r = t
                    .rows
                    .get_mut(
                        row.checked_sub(1)
                            .ok_or_else(|| EngineError::invalid("Rows are 1-based"))?,
                    )
                    .ok_or_else(|| EngineError::not_found(format!("Row {row} does not exist")))?;
                r[ci] = table::value_to_cell(value.clone());
            },
            Edit::AddRow { values, at } => {
                let mut row: Vec<String> =
                    values.iter().cloned().map(table::value_to_cell).collect();
                row.resize(t.width(), String::new());
                let pos = at
                    .map(|a| a.saturating_sub(1))
                    .unwrap_or(t.rows.len())
                    .min(t.rows.len());
                t.rows.insert(pos, row);
            },
            Edit::DeleteRow { row } => {
                if *row == 0 || *row > t.rows.len() {
                    return Err(EngineError::not_found(format!("Row {row} does not exist")));
                }
                t.rows.remove(row - 1);
            },
            Edit::DeleteRows { rows } => {
                let mut sorted = rows.clone();
                sorted.sort_unstable();
                sorted.dedup();
                for r in sorted.iter().rev() {
                    if *r >= 1 && *r <= t.rows.len() {
                        t.rows.remove(r - 1);
                    }
                }
            },
            Edit::AddColumn { name, default, at } => {
                let pos = at.map(|a| a.saturating_sub(1)).unwrap_or(t.width()).min(t.width());
                t.headers.insert(pos, name.clone());
                let d = table::value_to_cell(default.clone());
                for r in t.rows.iter_mut() {
                    r.insert(pos, d.clone());
                }
            },
            Edit::DeleteColumn { column } => {
                let ci = t.column_index(column)?;
                t.headers.remove(ci);
                for r in t.rows.iter_mut() {
                    r.remove(ci);
                }
            },
            Edit::RenameColumn { column, name } => {
                let ci = t.column_index(column)?;
                t.headers[ci] = name.clone();
            },
            Edit::MoveColumn { column, to } => {
                let ci = t.column_index(column)?;
                let to = to.saturating_sub(1).min(t.width() - 1);
                let h = t.headers.remove(ci);
                t.headers.insert(to, h);
                for r in t.rows.iter_mut() {
                    let c = r.remove(ci);
                    r.insert(to, c);
                }
            },
            Edit::MoveRow { row, to } => {
                if *row == 0 || *row > t.rows.len() {
                    return Err(EngineError::not_found(format!("Row {row} does not exist")));
                }
                let r = t.rows.remove(row - 1);
                let to = to.saturating_sub(1).min(t.rows.len());
                t.rows.insert(to, r);
            },
        }
    }
    Ok(())
}

/// Builds a table from either an existing CSV source or JSON `headers`+`rows`
/// (rows may be arrays or objects), applies edits and writes CSV.
pub fn write(
    src: &Source,
    headers: Vec<String>,
    rows: Vec<Value>,
    edits: &[Edit],
    opts: &CsvOptions,
) -> EngineResult<Value> {
    let mut t = if !src.is_empty() {
        load(src, opts)?.0
    } else if !rows.is_empty() && rows.iter().all(Value::is_object) {
        Table::from_objects(&rows)?
    } else if !headers.is_empty() {
        let arrays: Vec<Vec<Value>> = rows
            .into_iter()
            .map(|r| match r {
                Value::Array(a) => Ok(a),
                _ => Err(EngineError::invalid("rows must be arrays or objects")),
            })
            .collect::<EngineResult<_>>()?;
        let mut t = Table::from_json_rows(headers, arrays);
        let w = t.width();
        for r in t.rows.iter_mut() {
            r.resize(w, String::new());
        }
        t
    } else {
        return Err(EngineError::empty("csv"));
    };
    apply_edits(&mut t, edits)?;
    let mut extra = Map::new();
    extra.insert("edits_applied".into(), json!(edits.len()));
    output(&t, opts, extra)
}

// ---------------------------------------------------------------- convert --

/// Rewrites with a different delimiter, line ending, quote style or BOM.
pub fn convert(
    src: &Source,
    opts: &CsvOptions,
    to_delimiter: &str,
    line_ending: &str,
    quote_style: &str,
    bom: bool,
) -> EngineResult<Value> {
    let (t, d) = load(src, opts)?;
    let target = if to_delimiter.is_empty() {
        if t.delimiter == b',' {
            "\\t".to_string()
        } else {
            ",".to_string()
        }
    } else {
        to_delimiter.to_string()
    };
    let out_opts = CsvOptions {
        delimiter: target.clone(),
        line_ending: if line_ending.is_empty() {
            opts.line_ending.clone()
        } else {
            line_ending.into()
        },
        quote_style: if quote_style.is_empty() {
            opts.quote_style.clone()
        } else {
            quote_style.into()
        },
        has_header: opts.has_header,
        ..Default::default()
    };
    let mut csv = t.to_csv(&out_opts)?;
    if bom {
        csv.insert(0, '\u{feff}');
    }
    Ok(json!({
        "text": csv,
        "from_delimiter": table::delimiter_label(t.delimiter),
        "to_delimiter": target,
        "rows": t.rows.len(),
        "columns": t.width(),
        "source": input::decoded_json(&d),
        "issues": t.issues,
        "suggested_extension": if target == "\\t" { "tsv" } else { "csv" },
    }))
}

// ------------------------------------------------------------- merge/split --

/// Appends several files or joins two on a key column.
#[allow(clippy::too_many_arguments)]
pub fn merge(
    files: &[Source],
    opts: &CsvOptions,
    mode: &str,
    union_headers: bool,
    source_column: Option<String>,
    key: &str,
    right_key: &str,
    join_type: &str,
) -> EngineResult<Value> {
    engine_shared::limits::check_count(
        files.len(),
        engine_shared::limits::MAX_BATCH_FILES,
        "files",
    )?;
    let tables: Vec<Table> = files
        .iter()
        .map(|f| load(f, opts).map(|x| x.0))
        .collect::<EngineResult<_>>()?;
    let names: Vec<String> = files
        .iter()
        .enumerate()
        .map(|(i, f)| f.name.clone().unwrap_or_else(|| format!("file_{}", i + 1)))
        .collect();
    let merged = match mode {
        "" | "append" | "stack" => {
            ops::append(&tables, union_headers, source_column.as_deref(), &names)?
        },
        "join" => {
            if tables.len() != 2 {
                return Err(EngineError::invalid("join mode requires exactly two files"));
            }
            if key.is_empty() {
                return Err(EngineError::invalid(
                    "join mode requires `key` (column name present in both files)",
                ));
            }
            let rk = if right_key.is_empty() { key } else { right_key };
            ops::join(
                &tables[0],
                &tables[1],
                key,
                rk,
                if join_type.is_empty() { "inner" } else { join_type },
            )?
        },
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown merge mode `{other}` (append, join)"
            )))
        },
    };
    let mut extra = Map::new();
    extra.insert("files".into(), json!(tables.len()));
    extra.insert(
        "input_rows".into(),
        json!(tables.iter().map(|t| t.rows.len()).collect::<Vec<_>>()),
    );
    output(&merged, opts, extra)
}

/// Splits into several files by row count, part count or column value.
pub fn split(
    src: &Source,
    opts: &CsvOptions,
    mode: &str,
    rows_per_file: usize,
    parts: usize,
    column: &str,
    name_prefix: &str,
) -> EngineResult<Value> {
    let (t, _) = load(src, opts)?;
    if t.rows.is_empty() {
        return Err(EngineError::empty("rows"));
    }
    let prefix = if name_prefix.is_empty() {
        src.name
            .as_deref()
            .map(|n| n.trim_end_matches(".csv").trim_end_matches(".tsv"))
            .unwrap_or("part")
    } else {
        name_prefix
    };
    let mut groups: Vec<(String, Vec<Vec<String>>)> = Vec::new();
    match mode {
        "" | "rows" => {
            let n = if rows_per_file == 0 { 1000 } else { rows_per_file };
            if t.rows.len().div_ceil(n) > MAX_SPLIT_PARTS {
                return Err(EngineError::limit(format!(
                    "Splitting would produce more than {MAX_SPLIT_PARTS} files; increase \
                     rows_per_file"
                )));
            }
            for (i, chunk) in t.rows.chunks(n).enumerate() {
                groups.push((format!("{prefix}_{:03}", i + 1), chunk.to_vec()));
            }
        },
        "parts" => {
            let p = parts.clamp(1, MAX_SPLIT_PARTS).min(t.rows.len());
            let n = t.rows.len().div_ceil(p);
            for (i, chunk) in t.rows.chunks(n).enumerate() {
                groups.push((format!("{prefix}_{:03}", i + 1), chunk.to_vec()));
            }
        },
        "column" | "value" => {
            let ci = t.column_index(column)?;
            let mut order: Vec<String> = Vec::new();
            let mut map: std::collections::HashMap<String, Vec<Vec<String>>> =
                std::collections::HashMap::new();
            for r in &t.rows {
                let k = r[ci].trim().to_string();
                if !map.contains_key(&k) {
                    order.push(k.clone());
                    if order.len() > MAX_SPLIT_PARTS {
                        return Err(EngineError::limit(format!(
                            "Column `{column}` has more than {MAX_SPLIT_PARTS} distinct values"
                        )));
                    }
                }
                map.entry(k).or_default().push(r.clone());
            }
            for k in order {
                let safe: String = k
                    .chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c == '-' || c == '_' {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect();
                let rows = map.remove(&k).unwrap_or_default();
                groups.push((
                    format!(
                        "{prefix}_{}",
                        if safe.is_empty() { "empty".to_string() } else { safe }
                    ),
                    rows,
                ));
            }
        },
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown split mode `{other}` (rows, parts, column)"
            )))
        },
    }
    let files: Vec<Value> = groups
        .iter()
        .map(|(name, rows)| {
            let part = Table {
                headers: t.headers.clone(),
                rows: rows.clone(),
                issues: vec![],
                delimiter: t.delimiter,
            };
            Ok(json!({
                "name": format!("{name}.{}", if t.delimiter == b'\t' { "tsv" } else { "csv" }),
                "rows": rows.len(),
                "text": part.to_csv(opts)?
            }))
        })
        .collect::<EngineResult<_>>()?;
    Ok(json!({
        "files": files,
        "count": files.len(),
        "total_rows": t.rows.len(),
        "headers": t.headers
    }))
}

// ---------------------------------------------------------------- columns --

/// Keeps (or excludes) the named columns.
pub fn select_columns(
    src: &Source,
    opts: &CsvOptions,
    columns: &[String],
    exclude: bool,
) -> EngineResult<Value> {
    let (t, _) = load(src, opts)?;
    if columns.is_empty() {
        return Err(EngineError::empty("columns"));
    }
    let chosen = t.column_indices(columns)?;
    let idx: Vec<usize> = if exclude {
        (0..t.width()).filter(|i| !chosen.contains(i)).collect()
    } else {
        chosen
    };
    if idx.is_empty() {
        return Err(EngineError::invalid("No columns left after exclusion"));
    }
    let out = ops::keep_columns(&t, &idx);
    output(&out, opts, Map::new())
}

/// Re-orders columns, optionally dropping the rest.
pub fn reorder_columns(
    src: &Source,
    opts: &CsvOptions,
    order: &[String],
    drop_rest: bool,
) -> EngineResult<Value> {
    let (t, _) = load(src, opts)?;
    if order.is_empty() {
        return Err(EngineError::empty("order"));
    }
    let mut idx = t.column_indices(order)?;
    if !drop_rest {
        for i in 0..t.width() {
            if !idx.contains(&i) {
                idx.push(i);
            }
        }
    }
    let out = ops::keep_columns(&t, &idx);
    output(&out, opts, Map::new())
}

// ------------------------------------------------------------ json family --

/// Converts to a JSON array (or JSON Lines) of objects.
pub fn to_json(
    src: &Source,
    opts: &CsvOptions,
    typed: bool,
    pretty: bool,
    jsonl: bool,
) -> EngineResult<Value> {
    let (t, _) = load(src, opts)?;
    let objects = t.to_objects(typed);
    let text = if jsonl {
        objects
            .iter()
            .map(|o| serde_json::to_string(o).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n")
    } else if pretty {
        serde_json::to_string_pretty(&objects).map_err(|e| EngineError::internal(e.to_string()))?
    } else {
        serde_json::to_string(&objects).map_err(|e| EngineError::internal(e.to_string()))?
    };
    Ok(json!({
        "text": text,
        "rows": t.rows.len(),
        "headers": t.headers,
        "format": if jsonl { "jsonl" } else { "json" },
        "issues": t.issues
    }))
}

/// Builds a table from a JSON array / JSON Lines of objects (optionally flattening nested keys).
pub fn from_json(text: &str, opts: &CsvOptions, jsonl: bool, flatten: bool) -> EngineResult<Value> {
    engine_shared::error::require_text(text, if jsonl { "jsonl" } else { "json" })?;
    let mut objects: Vec<Value> = Vec::new();
    if jsonl {
        for (i, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let v: Value = serde_json::from_str(line)
                .map_err(|e| EngineError::malformed(format!("Line {}: {e}", i + 1)))?;
            objects.push(v);
        }
    } else {
        let v: Value = serde_json::from_str(text)
            .map_err(|e| EngineError::malformed(format!("Invalid JSON: {e}")))?;
        match v {
            Value::Array(a) => objects = a,
            Value::Object(ref o) => {
                // Accept {"data": [...]} style wrappers with a single array member.
                if let Some(arr) = o.values().find_map(|x| x.as_array()) {
                    if o.values().filter(|x| x.is_array()).count() == 1 {
                        objects = arr.clone();
                    } else {
                        objects = vec![v.clone()];
                    }
                } else {
                    objects = vec![v.clone()];
                }
            },
            _ => return Err(EngineError::invalid("JSON must be an array of objects")),
        }
    }
    if objects.is_empty() {
        return Err(EngineError::empty("rows"));
    }
    let objects: Vec<Value> = objects
        .into_iter()
        .map(|o| match o {
            Value::Object(m) => Ok(if flatten {
                Value::Object(flatten_object(&m, ""))
            } else {
                Value::Object(m)
            }),
            Value::Array(a) => Ok(Value::Object(
                a.into_iter()
                    .enumerate()
                    .map(|(i, v)| (format!("column_{}", i + 1), v))
                    .collect(),
            )),
            other => Ok(Value::Object([("value".to_string(), other)].into_iter().collect())),
        })
        .collect::<EngineResult<_>>()?;
    let t = Table::from_objects(&objects)?;
    output(&t, opts, Map::new())
}

fn flatten_object(m: &Map<String, Value>, prefix: &str) -> Map<String, Value> {
    let mut out = Map::new();
    for (k, v) in m {
        let key = if prefix.is_empty() {
            k.clone()
        } else {
            format!("{prefix}.{k}")
        };
        match v {
            Value::Object(inner) => out.extend(flatten_object(inner, &key)),
            Value::Array(a) if a.iter().all(|x| !x.is_object() && !x.is_array()) => {
                out.insert(
                    key,
                    Value::String(
                        a.iter()
                            .map(|x| table::value_to_cell(x.clone()))
                            .collect::<Vec<_>>()
                            .join("; "),
                    ),
                );
            },
            Value::Array(a) => {
                for (i, item) in a.iter().enumerate() {
                    match item {
                        Value::Object(inner) => {
                            out.extend(flatten_object(inner, &format!("{key}[{i}]")))
                        },
                        other => {
                            out.insert(format!("{key}[{i}]"), other.clone());
                        },
                    }
                }
            },
            other => {
                out.insert(key, other.clone());
            },
        }
    }
    out
}

// --------------------------------------------------------- markdown / html --

/// Renders as a Markdown table.
pub fn to_markdown(src: &Source, opts: &CsvOptions, max_rows: usize) -> EngineResult<Value> {
    let (t, _) = load(src, opts)?;
    let types = t.column_types();
    let esc = |s: &str| s.replace('|', "\\|").replace('\n', " ");
    let mut md =
        format!("| {} |\n", t.headers.iter().map(|h| esc(h)).collect::<Vec<_>>().join(" | "));
    md.push_str(&format!(
        "| {} |\n",
        types
            .iter()
            .map(|ty| if matches!(ty, table::ColumnType::Integer | table::ColumnType::Number) {
                "---:"
            } else {
                "---"
            })
            .collect::<Vec<_>>()
            .join(" | ")
    ));
    let limit = if max_rows == 0 { usize::MAX } else { max_rows };
    for r in t.rows.iter().take(limit) {
        md.push_str(&format!("| {} |\n", r.iter().map(|c| esc(c)).collect::<Vec<_>>().join(" | ")));
    }
    if t.rows.len() > limit {
        md.push_str(&format!("\n_{} more row(s) not shown_\n", t.rows.len() - limit));
    }
    Ok(json!({ "markdown": md, "rows": t.rows.len(), "truncated": t.rows.len() > limit }))
}

/// Renders as an HTML table.
pub fn to_html(
    src: &Source,
    opts: &CsvOptions,
    max_rows: usize,
    class: &str,
) -> EngineResult<Value> {
    let (t, _) = load(src, opts)?;
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    };
    let limit = if max_rows == 0 { usize::MAX } else { max_rows };
    let mut html = String::new();
    html.push_str(&format!(
        "<table{}>\n  <thead>\n    <tr>{}</tr>\n  </thead>\n  <tbody>\n",
        if class.is_empty() {
            String::new()
        } else {
            format!(" class=\"{}\"", esc(class))
        },
        t.headers
            .iter()
            .map(|h| format!("<th>{}</th>", esc(h)))
            .collect::<String>()
    ));
    for r in t.rows.iter().take(limit) {
        html.push_str(&format!(
            "    <tr>{}</tr>\n",
            r.iter().map(|c| format!("<td>{}</td>", esc(c))).collect::<String>()
        ));
    }
    html.push_str("  </tbody>\n</table>\n");
    Ok(json!({ "html": html, "rows": t.rows.len(), "truncated": t.rows.len() > limit }))
}

// --------------------------------------------------------------- encoding --

/// Re-encodes the raw text (UTF-8/UTF-16/Latin-1/Windows-1252) with BOM and line-ending control.
pub fn encoding_convert(
    src: &Source,
    to: &str,
    bom: Option<bool>,
    line_ending: &str,
) -> EngineResult<Value> {
    let d = src.decode()?;
    let mut text = d.text.clone();
    match line_ending {
        "crlf" => text = text.replace("\r\n", "\n").replace('\n', "\r\n"),
        "lf" => text = text.replace("\r\n", "\n").replace('\r', "\n"),
        _ => {},
    }
    let target = if to.is_empty() { "utf-8" } else { to };
    let (bytes, label) = input::encode_text(&text, target, bom)?;
    Ok(json!({
        "file": engine_shared::b64::encode(&bytes),
        "bytes": bytes.len(),
        "from": input::decoded_json(&d),
        "to": {
            "encoding": label,
            "bom": bytes.starts_with(&[0xEF, 0xBB, 0xBF]) || bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF])
        },
        "characters": text.chars().count(),
        "preview": text.lines().take(3).collect::<Vec<_>>(),
    }))
}

// ---------------------------------------------------------------- actions --

/// Every action understood by the CSV engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum CsvAction {
    /// Parses and pages through a CSV.
    Parse {
        /// Input text/file.
        #[serde(flatten)]
        source: Source,
        /// Parser options.
        #[serde(default)]
        options: CsvOptions,
        /// First row to return.
        #[serde(default)]
        offset: usize,
        /// Maximum rows to return (0 = all).
        #[serde(default)]
        limit: usize,
        /// Return typed JSON values instead of strings.
        #[serde(default)]
        typed: bool,
    },
    /// Detects dialect and encoding.
    Detect {
        /// Input text/file.
        #[serde(flatten)]
        source: Source,
    },
    /// Builds or edits a CSV from headers/rows and edit operations.
    Write {
        /// Existing CSV to edit (optional).
        #[serde(flatten)]
        source: Source,
        /// Header names when building from scratch.
        #[serde(default)]
        headers: Vec<String>,
        /// Rows (arrays or objects) when building from scratch.
        #[serde(default)]
        rows: Vec<Value>,
        /// Edits applied in order.
        #[serde(default)]
        edits: Vec<Edit>,
        /// Parser/writer options.
        #[serde(default)]
        options: CsvOptions,
    },
    /// Cleans cells, headers and rows.
    Clean {
        /// Input text/file.
        #[serde(flatten)]
        source: Source,
        /// Parser options.
        #[serde(default)]
        options: CsvOptions,
        /// Cleaning switches.
        #[serde(default)]
        clean: ops::CleanOptions,
    },
    /// Validates structure and optional schema rules.
    Validate {
        /// Input text/file.
        #[serde(flatten)]
        source: Source,
        /// Parser options.
        #[serde(default)]
        options: CsvOptions,
        /// Column rules.
        #[serde(default)]
        schema: Option<ops::Schema>,
        /// Stop after this many errors.
        #[serde(default)]
        max_errors: usize,
    },
    /// Rewrites in another dialect.
    Convert {
        /// Input text/file.
        #[serde(flatten)]
        source: Source,
        /// Parser options.
        #[serde(default)]
        options: CsvOptions,
        /// Output delimiter (`,`, `;`, `tab`, `|`).
        #[serde(default)]
        to_delimiter: String,
        /// `lf` or `crlf`.
        #[serde(default)]
        line_ending: String,
        /// `necessary`, `always`, `never` or `non_numeric`.
        #[serde(default)]
        quote_style: String,
        /// Prepend a UTF-8 BOM.
        #[serde(default)]
        bom: bool,
    },
    /// Appends or joins files.
    Merge {
        /// Inputs.
        files: Vec<Source>,
        /// Parser options.
        #[serde(default)]
        options: CsvOptions,
        /// `append` (default) or `join`.
        #[serde(default)]
        mode: String,
        /// Union of all headers when appending (otherwise the first file's headers).
        #[serde(default = "default_true")]
        union_headers: bool,
        /// Add a column with the source file name.
        #[serde(default)]
        source_column: Option<String>,
        /// Join key in the left file.
        #[serde(default)]
        key: String,
        /// Join key in the right file (default: same as `key`).
        #[serde(default)]
        right_key: String,
        /// `inner` (default), `left`, `right` or `outer`.
        #[serde(default)]
        join_type: String,
    },
    /// Splits into several files.
    Split {
        /// Input text/file.
        #[serde(flatten)]
        source: Source,
        /// Parser options.
        #[serde(default)]
        options: CsvOptions,
        /// `rows` (default), `parts` or `column`.
        #[serde(default)]
        mode: String,
        /// Rows per output file.
        #[serde(default)]
        rows_per_file: usize,
        /// Number of equal parts.
        #[serde(default)]
        parts: usize,
        /// Column whose values define the groups.
        #[serde(default)]
        column: String,
        /// Prefix for output names.
        #[serde(default)]
        name_prefix: String,
    },
    /// Removes duplicate rows.
    Dedupe {
        /// Input text/file.
        #[serde(flatten)]
        source: Source,
        /// Parser options.
        #[serde(default)]
        options: CsvOptions,
        /// Columns that define a duplicate (all when empty).
        #[serde(default)]
        columns: Vec<String>,
        /// Ignore case when comparing.
        #[serde(default)]
        case_insensitive: bool,
        /// `first` (default) or `last` occurrence to keep.
        #[serde(default)]
        keep: String,
    },
    /// Selects columns.
    SelectColumns {
        /// Input text/file.
        #[serde(flatten)]
        source: Source,
        /// Parser options.
        #[serde(default)]
        options: CsvOptions,
        /// Column names or indices.
        columns: Vec<String>,
        /// Drop the listed columns instead of keeping them.
        #[serde(default)]
        exclude: bool,
    },
    /// Re-orders columns.
    ReorderColumns {
        /// Input text/file.
        #[serde(flatten)]
        source: Source,
        /// Parser options.
        #[serde(default)]
        options: CsvOptions,
        /// New column order.
        order: Vec<String>,
        /// Drop columns not in `order`.
        #[serde(default)]
        drop_rest: bool,
    },
    /// Filters rows by conditions.
    Filter {
        /// Input text/file.
        #[serde(flatten)]
        source: Source,
        /// Parser options.
        #[serde(default)]
        options: CsvOptions,
        /// Conditions.
        conditions: Vec<ops::Condition>,
        /// All conditions must match (otherwise any).
        #[serde(default = "default_true")]
        match_all: bool,
        /// Keep non-matching rows instead.
        #[serde(default)]
        invert: bool,
    },
    /// Sorts rows by one or more keys.
    Sort {
        /// Input text/file.
        #[serde(flatten)]
        source: Source,
        /// Parser options.
        #[serde(default)]
        options: CsvOptions,
        /// Sort keys in priority order.
        keys: Vec<ops::SortKey>,
    },
    /// Generates SQL `CREATE TABLE` / `INSERT` statements.
    ToSql {
        /// Input text/file.
        #[serde(flatten)]
        source: Source,
        /// Parser options.
        #[serde(default)]
        options: CsvOptions,
        /// Table name.
        #[serde(default)]
        table: String,
        /// `postgres` (default), `mysql`, `sqlite` or `mssql`.
        #[serde(default)]
        dialect: String,
        /// Emit a `CREATE TABLE` statement.
        #[serde(default = "default_true")]
        create_table: bool,
        /// Rows per `INSERT` statement.
        #[serde(default)]
        batch_size: usize,
        /// Column to declare as primary key.
        #[serde(default)]
        primary_key: Option<String>,
    },
    /// CSV → JSON array.
    ToJson {
        /// Input text/file.
        #[serde(flatten)]
        source: Source,
        /// Parser options.
        #[serde(default)]
        options: CsvOptions,
        /// Convert numbers/booleans.
        #[serde(default = "default_true")]
        typed: bool,
        /// Pretty-print the JSON.
        #[serde(default = "default_true")]
        pretty: bool,
    },
    /// CSV → JSON Lines.
    ToJsonl {
        /// Input text/file.
        #[serde(flatten)]
        source: Source,
        /// Parser options.
        #[serde(default)]
        options: CsvOptions,
        /// Convert numbers/booleans.
        #[serde(default = "default_true")]
        typed: bool,
    },
    /// JSON array → CSV.
    FromJson {
        /// JSON text.
        text: String,
        /// Writer options.
        #[serde(default)]
        options: CsvOptions,
        /// Flatten nested objects into dotted columns.
        #[serde(default = "default_true")]
        flatten: bool,
    },
    /// JSON Lines → CSV.
    FromJsonl {
        /// JSONL text.
        text: String,
        /// Writer options.
        #[serde(default)]
        options: CsvOptions,
        /// Flatten nested objects into dotted columns.
        #[serde(default = "default_true")]
        flatten: bool,
    },
    /// Per-column statistics (types, nulls, min/max/mean, top values).
    Statistics {
        /// Input text/file.
        #[serde(flatten)]
        source: Source,
        /// Parser options.
        #[serde(default)]
        options: CsvOptions,
        /// Columns to analyse (all when empty).
        #[serde(default)]
        columns: Vec<String>,
        /// Number of most frequent values to list.
        #[serde(default)]
        top_n: usize,
    },
    /// Swaps rows and columns.
    Transpose {
        /// Input text/file.
        #[serde(flatten)]
        source: Source,
        /// Parser options.
        #[serde(default)]
        options: CsvOptions,
        /// Use the first column as the new header row.
        #[serde(default = "default_true")]
        first_column_as_header: bool,
    },
    /// Compares two CSVs by key columns.
    Diff {
        /// Left CSV.
        a: Source,
        /// Right CSV.
        b: Source,
        /// Parser options.
        #[serde(default)]
        options: CsvOptions,
        /// Columns that identify a row (row order when empty).
        #[serde(default)]
        key_columns: Vec<String>,
        /// Columns ignored when comparing.
        #[serde(default)]
        ignore_columns: Vec<String>,
        /// Ignore case when comparing.
        #[serde(default)]
        case_insensitive: bool,
    },
    /// Re-encodes the raw text.
    EncodingConvert {
        /// Input text/file.
        #[serde(flatten)]
        source: Source,
        /// Target encoding (`utf-8`, `utf-16le`, `utf-16be`, `latin1`, `windows-1252`).
        #[serde(default)]
        to: String,
        /// Write a BOM (default: keep as-is).
        #[serde(default)]
        bom: Option<bool>,
        /// `lf`, `crlf` or empty to keep.
        #[serde(default)]
        line_ending: String,
    },
    /// Applies column transformations.
    Transform {
        /// Input text/file.
        #[serde(flatten)]
        source: Source,
        /// Parser options.
        #[serde(default)]
        options: CsvOptions,
        /// Transformation steps in order.
        steps: Vec<ops::Transform>,
    },
    /// CSV → Markdown table.
    ToMarkdown {
        /// Input text/file.
        #[serde(flatten)]
        source: Source,
        /// Parser options.
        #[serde(default)]
        options: CsvOptions,
        /// Maximum rows rendered (0 = all).
        #[serde(default)]
        max_rows: usize,
    },
    /// CSV → HTML table.
    ToHtml {
        /// Input text/file.
        #[serde(flatten)]
        source: Source,
        /// Parser options.
        #[serde(default)]
        options: CsvOptions,
        /// Maximum rows rendered (0 = all).
        #[serde(default)]
        max_rows: usize,
        /// CSS class for the `<table>`.
        #[serde(default)]
        class: String,
    },
}

fn default_true() -> bool {
    true
}

/// Dispatches a [`CsvAction`] to its implementation.
pub fn handle_action(action: CsvAction) -> EngineResult<Value> {
    match action {
        CsvAction::Parse { source, options, offset, limit, typed } => {
            parse(&source, &options, offset, limit, typed)
        },
        CsvAction::Detect { source } => detect(&source),
        CsvAction::Write { source, headers, rows, edits, options } => {
            write(&source, headers, rows, &edits, &options)
        },
        CsvAction::Clean { source, options, clean: c } => {
            let (t, _) = load(&source, &options)?;
            let (out, report) = ops::clean(t, &c)?;
            let mut extra = Map::new();
            extra.insert("report".into(), report);
            output(&out, &options, extra)
        },
        CsvAction::Validate { source, options, schema, max_errors } => {
            let (t, d) = load(&source, &options)?;
            let mut v = ops::validate(&t, schema.as_ref(), max_errors);
            v["source"] = input::decoded_json(&d);
            v["delimiter"] = json!(table::delimiter_label(t.delimiter));
            Ok(v)
        },
        CsvAction::Convert {
            source,
            options,
            to_delimiter,
            line_ending,
            quote_style,
            bom,
        } => convert(&source, &options, &to_delimiter, &line_ending, &quote_style, bom),
        CsvAction::Merge {
            files,
            options,
            mode,
            union_headers,
            source_column,
            key,
            right_key,
            join_type,
        } => merge(
            &files,
            &options,
            &mode,
            union_headers,
            source_column,
            &key,
            &right_key,
            &join_type,
        ),
        CsvAction::Split {
            source,
            options,
            mode,
            rows_per_file,
            parts,
            column,
            name_prefix,
        } => split(&source, &options, &mode, rows_per_file, parts, &column, &name_prefix),
        CsvAction::Dedupe {
            source,
            options,
            columns,
            case_insensitive,
            keep,
        } => {
            let (t, _) = load(&source, &options)?;
            let before = t.rows.len();
            let (out, removed) = ops::dedupe(t, &columns, case_insensitive, &keep)?;
            let mut extra = Map::new();
            extra.insert("rows_before".into(), json!(before));
            extra.insert("duplicates_removed".into(), json!(removed.len()));
            extra.insert(
                "duplicates".into(),
                json!(removed.into_iter().take(1000).collect::<Vec<_>>()),
            );
            output(&out, &options, extra)
        },
        CsvAction::SelectColumns { source, options, columns, exclude } => {
            select_columns(&source, &options, &columns, exclude)
        },
        CsvAction::ReorderColumns { source, options, order, drop_rest } => {
            reorder_columns(&source, &options, &order, drop_rest)
        },
        CsvAction::Filter {
            source,
            options,
            conditions,
            match_all,
            invert,
        } => {
            let (t, _) = load(&source, &options)?;
            let before = t.rows.len();
            let out = ops::filter(&t, &conditions, match_all, invert)?;
            let mut extra = Map::new();
            extra.insert("rows_before".into(), json!(before));
            extra.insert("rows_removed".into(), json!(before - out.rows.len()));
            output(&out, &options, extra)
        },
        CsvAction::Sort { source, options, keys } => {
            let (t, _) = load(&source, &options)?;
            output(&ops::sort(&t, &keys)?, &options, Map::new())
        },
        CsvAction::ToSql {
            source,
            options,
            table,
            dialect,
            create_table,
            batch_size,
            primary_key,
        } => {
            let (t, _) = load(&source, &options)?;
            ops::to_sql(&t, &table, &dialect, create_table, batch_size, primary_key.as_deref())
        },
        CsvAction::ToJson { source, options, typed, pretty } => {
            to_json(&source, &options, typed, pretty, false)
        },
        CsvAction::ToJsonl { source, options, typed } => {
            to_json(&source, &options, typed, false, true)
        },
        CsvAction::FromJson { text, options, flatten } => {
            from_json(&text, &options, false, flatten)
        },
        CsvAction::FromJsonl { text, options, flatten } => {
            from_json(&text, &options, true, flatten)
        },
        CsvAction::Statistics { source, options, columns, top_n } => {
            let (t, _) = load(&source, &options)?;
            ops::statistics(&t, &columns, top_n)
        },
        CsvAction::Transpose { source, options, first_column_as_header } => {
            let (t, _) = load(&source, &options)?;
            output(&ops::transpose(&t, first_column_as_header), &options, Map::new())
        },
        CsvAction::Diff {
            a,
            b,
            options,
            key_columns,
            ignore_columns,
            case_insensitive,
        } => {
            let (ta, _) = load(&a, &options)?;
            let (tb, _) = load(&b, &options)?;
            ops::diff(&ta, &tb, &key_columns, &ignore_columns, case_insensitive)
        },
        CsvAction::EncodingConvert { source, to, bom, line_ending } => {
            encoding_convert(&source, &to, bom, &line_ending)
        },
        CsvAction::Transform { source, options, steps } => {
            let (t, _) = load(&source, &options)?;
            output(&ops::transform(t, &steps)?, &options, Map::new())
        },
        CsvAction::ToMarkdown { source, options, max_rows } => {
            to_markdown(&source, &options, max_rows)
        },
        CsvAction::ToHtml { source, options, max_rows, class } => {
            to_html(&source, &options, max_rows, &class)
        },
    }
}

/// Pure entry point used by tests and by other engines.
pub fn process_request(input: &[u8]) -> Vec<u8> {
    engine_shared::run_request::<CsvAction, _>(input, handle_action)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(text: &str) -> Source {
        Source {
            text: Some(text.into()),
            ..Default::default()
        }
    }

    #[test]
    fn parse_and_write() {
        let p = parse(&src("a,b\n1,x\n2,y\n"), &CsvOptions::default(), 0, 1, true).unwrap();
        assert_eq!(p["total_rows"], 2);
        assert_eq!(p["has_more"], true);
        assert_eq!(p["rows"][0]["a"], 1);
        let w = write(
            &src("a,b\n1,x\n"),
            vec![],
            vec![],
            &[
                Edit::SetCell {
                    row: 1,
                    column: "b".into(),
                    value: json!("z"),
                },
                Edit::AddColumn {
                    name: "c".into(),
                    default: json!(0),
                    at: None,
                },
            ],
            &CsvOptions::default(),
        )
        .unwrap();
        assert_eq!(w["csv"], "a,b,c\n1,z,0\n");
        let w = write(
            &Source::default(),
            vec!["x".into(), "y".into()],
            vec![json!([1, 2]), json!([3])],
            &[],
            &CsvOptions::default(),
        )
        .unwrap();
        assert_eq!(w["csv"], "x,y\n1,2\n3,\n");
        assert!(write(&Source::default(), vec![], vec![], &[], &CsvOptions::default()).is_err());
    }

    #[test]
    fn conversions() {
        let c =
            convert(&src("a,b\n1,\"x,y\"\n"), &CsvOptions::default(), "", "", "", false).unwrap();
        assert_eq!(c["text"], "a\tb\n1\tx,y\n");
        assert_eq!(c["suggested_extension"], "tsv");
        let j = to_json(&src("a,b\n1,true\n"), &CsvOptions::default(), true, false, true).unwrap();
        assert_eq!(j["text"], "{\"a\":1,\"b\":true}");
        let f =
            from_json("{\"a\":1,\"n\":{\"x\":2}}\n{\"a\":3}\n", &CsvOptions::default(), true, true)
                .unwrap();
        assert_eq!(f["csv"], "a,n.x\n1,2\n3,\n");
        assert!(from_json("{bad", &CsvOptions::default(), false, true).is_err());
        let e = encoding_convert(&src("a,b"), "utf-16le", None, "").unwrap();
        assert_eq!(e["to"]["encoding"], "utf-16le");
        assert_eq!(e["bytes"], 8);
        let md = to_markdown(&src("a,b\n1,x\n"), &CsvOptions::default(), 0).unwrap();
        assert!(md["markdown"]
            .as_str()
            .unwrap()
            .starts_with("| a | b |\n| ---: | --- |"));
    }

    #[test]
    fn split_and_select() {
        let s =
            split(&src("k,v\na,1\nb,2\na,3\n"), &CsvOptions::default(), "column", 0, 0, "k", "out")
                .unwrap();
        assert_eq!(s["count"], 2);
        assert_eq!(s["files"][0]["name"], "out_a.csv");
        let s = split(&src("k,v\na,1\nb,2\na,3\n"), &CsvOptions::default(), "rows", 2, 0, "", "")
            .unwrap();
        assert_eq!(s["count"], 2);
        let sel = select_columns(
            &src("a,b,c\n1,2,3\n"),
            &CsvOptions::default(),
            &["c".into(), "a".into()],
            false,
        )
        .unwrap();
        assert_eq!(sel["csv"], "c,a\n3,1\n");
        let re =
            reorder_columns(&src("a,b,c\n1,2,3\n"), &CsvOptions::default(), &["b".into()], false)
                .unwrap();
        assert_eq!(re["csv"], "b,a,c\n2,1,3\n");
    }
}
