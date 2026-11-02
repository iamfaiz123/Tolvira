//! # Dev Toolbox Spreadsheet Engine (`spreadsheet-engine`)
//!
//! Reads XLSX / XLSM / XLSB / XLS / ODS (via `calamine`) and CSV, writes XLSX
//! and ODS with a built-in writer, and reuses the CSV engine's table
//! operations for cleaning and de-duplication. Encrypted workbooks are
//! reported as `E_ENCRYPTED`; corrupt containers as `E_MALFORMED`.
//!
//! | Action | Tool(s) |
//! |---|---|
//! | `inspect` | xlsx-viewer (sheet list, dimensions, formulas, macros) |
//! | `read` | xlsx-viewer (paged grid) |
//! | `to_csv` | xlsx-to-csv, excel-to-csv |
//! | `from_csv` | csv-to-xlsx, csv-to-excel |
//! | `to_json` / `from_json` | xlsx-to-json, json-to-xlsx |
//! | `convert` | ods-to-xlsx, xlsx-to-ods, xls → xlsx |
//! | `merge` | xlsx-merge |
//! | `split_sheets` | xlsx-split-sheets |
//! | `dedupe` | xlsx-remove-duplicates |
//! | `clean` | xlsx-cleaner |
//! | `formulas` | xlsx-formula-inspector (lists formulas, recalculates, flags mismatches) |
//! | `evaluate` | spreadsheet-formula-evaluator |
//! | `diff` | xlsx-diff |
//! | `column_letter` | spreadsheet-column-letter |
//! | `to_html` | xlsx-to-pdf (render, then `pdf.from_html`) |
//!
//! Limitations: charts, pivot tables, macros (kept only as a flag), cell
//! styling beyond bold headers/date formats and comments are not preserved.

pub mod formula;
pub mod model;
pub mod writer;

use engine_shared::{EngineError, EngineResult};
use model::{Cell, Sheet, Workbook};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use tabular_core::table::{CsvOptions, Table};
use writer::XlsxOptions;

/// A base64 file plus optional original file name.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct FileInput {
    /// File bytes (base64 on the wire).
    #[serde(with = "engine_shared::b64")]
    pub file: Vec<u8>,
    /// Original file name (used to derive output names and formats).
    pub name: Option<String>,
}

/// Styling applied to generated XLSX/ODS files.
#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct OutputOptions {
    /// Render the first row bold.
    pub bold_header: bool,
    /// Freeze the first row.
    pub freeze_header: bool,
    /// Approximate column widths from content.
    pub autofit: bool,
    /// Add an auto-filter to the header row.
    pub auto_filter: bool,
    /// `xlsx` (default) or `ods`.
    pub format: String,
}

impl Default for OutputOptions {
    fn default() -> Self {
        Self {
            bold_header: true,
            freeze_header: true,
            autofit: true,
            auto_filter: false,
            format: "xlsx".into(),
        }
    }
}

impl OutputOptions {
    fn xlsx(&self) -> XlsxOptions {
        XlsxOptions {
            bold_header: self.bold_header,
            freeze_header: self.freeze_header,
            autofit: self.autofit,
            auto_filter: self.auto_filter,
        }
    }
}

fn read(input: &FileInput, formulas: bool) -> EngineResult<Workbook> {
    Workbook::read(&input.file, input.name.as_deref(), formulas)
}

fn base_name(input: &FileInput, fallback: &str) -> String {
    input
        .name
        .as_deref()
        .map(|n| n.rsplit('/').next().unwrap_or(n))
        .and_then(|n| {
            n.rsplit_once('.')
                .map(|(b, _)| b.to_string())
                .or_else(|| Some(n.to_string()))
        })
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn file_json(bytes: Vec<u8>, name: &str, format: &str) -> Value {
    let mime = match format {
        "ods" => "application/vnd.oasis.opendocument.spreadsheet",
        "csv" => "text/csv",
        _ => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    };
    json!({
        "file": engine_shared::b64::encode(&bytes),
        "filename": format!("{name}.{format}"),
        "mime": mime,
        "bytes": bytes.len()
    })
}

fn write_workbook(wb: &Workbook, name: &str, out: &OutputOptions) -> EngineResult<Value> {
    let (bytes, fmt) = match out.format.to_ascii_lowercase().as_str() {
        "" | "xlsx" => (writer::write_xlsx(wb, &out.xlsx())?, "xlsx"),
        "ods" => (writer::write_ods(wb)?, "ods"),
        other => {
            return Err(EngineError::unsupported_format(format!(
                "Cannot write `{other}` (supported: xlsx, ods)"
            )))
        },
    };
    let mut v = file_json(bytes, name, fmt);
    v["sheets"] = json!(wb
        .sheets
        .iter()
        .map(|s| json!({ "name": s.name, "rows": s.rows(), "columns": s.cols() }))
        .collect::<Vec<_>>());
    Ok(v)
}

// ---------------------------------------------------------------- actions --

/// Workbook summary: format, sheets, dimensions, defined names, macros.
pub fn inspect(input: &FileInput) -> EngineResult<Value> {
    let wb = read(input, true)?;
    let mut v = wb.summary();
    v["filename"] = json!(input.name);
    v["bytes"] = json!(input.file.len());
    Ok(v)
}

/// Reads one sheet as a paginated JSON table.
pub fn read_sheet(
    input: &FileInput,
    sheet: Option<&str>,
    header: bool,
    offset: usize,
    limit: usize,
    typed: bool,
) -> EngineResult<Value> {
    let wb = read(input, true)?;
    let s = wb.sheet(sheet)?;
    let limit = if limit == 0 { 1000 } else { limit.min(100_000) };
    let t = s.to_table(header);
    let types = t.column_types();
    let data_offset = if header { 1 } else { 0 };
    let rows: Vec<Value> = s
        .cells
        .iter()
        .enumerate()
        .skip(data_offset + offset)
        .take(limit)
        .map(|(ri, row)| {
            let cells: Vec<Value> = (0..s.cols())
                .map(|ci| {
                    let c = row.get(ci).unwrap_or(&Cell::Empty);
                    let mut v = if typed {
                        c.to_json()
                    } else {
                        Value::String(c.to_display())
                    };
                    if let Some(f) = s.formulas.get(&(ri, ci)) {
                        v = json!({ "value": v, "formula": f });
                    }
                    v
                })
                .collect();
            Value::Array(cells)
        })
        .collect();
    Ok(json!({
        "sheet": s.name,
        "sheets": wb.sheets.iter().map(|x| x.name.clone()).collect::<Vec<_>>(),
        "headers": if header { t.headers.clone() } else { (1..=s.cols()).map(model::column_letter).collect() },
        "columns": t.headers.iter().enumerate().map(|(i, h)| json!({ "index": i, "letter": model::column_letter(i + 1), "name": h, "type": types[i].label() })).collect::<Vec<_>>(),
        "rows": rows,
        "offset": offset,
        "limit": limit,
        "total_rows": s.rows().saturating_sub(data_offset),
        "total_columns": s.cols(),
        "has_more": data_offset + offset + limit < s.rows(),
        "used_range": s.used_range(),
        "formula_count": s.formulas.len(),
        "merged_ranges": s.merged,
        "format": wb.format,
    }))
}

/// Converts a sheet (or every sheet) to CSV text.
pub fn to_csv(
    input: &FileInput,
    sheet: Option<&str>,
    all_sheets: bool,
    options: &CsvOptions,
    header: bool,
) -> EngineResult<Value> {
    let wb = read(input, false)?;
    let base = base_name(input, "sheet");
    let make = |s: &Sheet| -> EngineResult<Value> {
        let t = s.to_table(header);
        let mut o = options.clone();
        if o.delimiter.is_empty() {
            o.delimiter = ",".into();
        }
        Ok(json!({
            "name": format!("{}_{}.csv", base, s.name.replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "_")),
            "sheet": s.name,
            "rows": t.rows.len(),
            "columns": t.width(),
            "text": t.to_csv(&o)?
        }))
    };
    if all_sheets {
        let files: Vec<Value> = wb.sheets.iter().map(make).collect::<EngineResult<_>>()?;
        return Ok(json!({ "files": files, "count": files.len(), "format": wb.format }));
    }
    let s = wb.sheet(sheet)?;
    let mut v = make(s)?;
    v["sheets"] = json!(wb.sheets.iter().map(|x| x.name.clone()).collect::<Vec<_>>());
    v["format"] = json!(wb.format);
    Ok(v)
}

/// A CSV input that becomes one sheet in `from_csv`.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct CsvSource {
    /// Inline CSV text.
    pub text: Option<String>,
    /// CSV file bytes (base64).
    #[serde(with = "engine_shared::b64::option")]
    pub file: Option<Vec<u8>>,
    /// Original file name.
    pub name: Option<String>,
    /// Sheet name to use (default: file stem).
    pub sheet_name: Option<String>,
}

/// Builds a workbook from one or more CSV sources.
pub fn from_csv(
    sources: &[CsvSource],
    options: &CsvOptions,
    typed: bool,
    out: &OutputOptions,
    name: &str,
) -> EngineResult<Value> {
    engine_shared::limits::check_count(sources.len(), 255, "files")?;
    let mut wb = Workbook {
        format: "xlsx".into(),
        ..Default::default()
    };
    let mut used: Vec<String> = Vec::new();
    for (i, src) in sources.iter().enumerate() {
        let s = tabular_core::input::Source {
            text: src.text.clone(),
            file: src.file.clone(),
            encoding: None,
            name: src.name.clone(),
        };
        let text = s.text()?;
        let t = Table::parse(&text, options)?;
        let sheet_name = src
            .sheet_name
            .clone()
            .or_else(|| {
                src.name.as_deref().map(|n| {
                    n.rsplit('/')
                        .next()
                        .unwrap_or(n)
                        .rsplit_once('.')
                        .map(|(b, _)| b.to_string())
                        .unwrap_or_else(|| n.to_string())
                })
            })
            .unwrap_or_else(|| format!("Sheet{}", i + 1));
        let safe = model::safe_sheet_name(&sheet_name, &used);
        used.push(safe.clone());
        wb.sheets
            .push(Sheet::from_table(&safe, &t, options.has_header, typed));
    }
    let strip_ext = |n: &str| {
        n.rsplit('/')
            .next()
            .unwrap_or(n)
            .rsplit_once('.')
            .map(|(b, _)| b.to_string())
            .filter(|b| !b.is_empty())
            .unwrap_or_else(|| n.to_string())
    };
    let name = if name.is_empty() {
        sources
            .first()
            .and_then(|s| s.name.as_deref())
            .map(strip_ext)
            .unwrap_or_else(|| "workbook".into())
    } else {
        strip_ext(name)
    };
    write_workbook(&wb, &name, out)
}

/// Converts a sheet (or every sheet) to JSON objects/arrays.
pub fn to_json(
    input: &FileInput,
    sheet: Option<&str>,
    all_sheets: bool,
    header: bool,
    typed: bool,
    pretty: bool,
) -> EngineResult<Value> {
    let wb = read(input, false)?;
    let sheet_json = |s: &Sheet| -> Value {
        let t = s.to_table(header);
        if header {
            Value::Array(t.to_objects(typed))
        } else {
            Value::Array(
                s.cells
                    .iter()
                    .map(|r| {
                        Value::Array(
                            r.iter()
                                .map(|c| {
                                    if typed {
                                        c.to_json()
                                    } else {
                                        Value::String(c.to_display())
                                    }
                                })
                                .collect(),
                        )
                    })
                    .collect(),
            )
        }
    };
    let data = if all_sheets {
        Value::Object(wb.sheets.iter().map(|s| (s.name.clone(), sheet_json(s))).collect())
    } else {
        sheet_json(wb.sheet(sheet)?)
    };
    let text = if pretty {
        serde_json::to_string_pretty(&data)
    } else {
        serde_json::to_string(&data)
    }
    .map_err(|e| EngineError::internal(e.to_string()))?;
    Ok(json!({
        "data": data,
        "text": text,
        "sheets": wb.sheets.iter().map(|x| x.name.clone()).collect::<Vec<_>>(),
        "format": wb.format
    }))
}

/// Builds a workbook from a JSON array of objects/arrays.
pub fn from_json(
    text: Option<&str>,
    data: Option<&Value>,
    sheet_name: &str,
    out: &OutputOptions,
    name: &str,
) -> EngineResult<Value> {
    let parsed: Value = match (data, text) {
        (Some(d), _) if !d.is_null() => d.clone(),
        (_, Some(t)) if !t.trim().is_empty() => serde_json::from_str(t)
            .map_err(|e| EngineError::malformed(format!("Invalid JSON: {e}")))?,
        _ => return Err(EngineError::empty("json")),
    };
    let mut wb = Workbook {
        format: "xlsx".into(),
        ..Default::default()
    };
    let table_from = |v: &Value| -> EngineResult<Table> {
        match v {
            Value::Array(items) if items.iter().all(Value::is_object) && !items.is_empty() => {
                Table::from_objects(items)
            },
            Value::Array(items) if items.iter().all(Value::is_array) && !items.is_empty() => {
                let mut rows: Vec<Vec<Value>> = items
                    .iter()
                    .map(|r| r.as_array().cloned().unwrap_or_default())
                    .collect();
                let headers: Vec<String> = rows
                    .remove(0)
                    .into_iter()
                    .map(tabular_core::table::value_to_cell)
                    .collect();
                Ok(Table::from_json_rows(headers, rows))
            },
            Value::Array(_) => Err(EngineError::invalid(
                "JSON array must contain objects (records) or arrays (rows)",
            )),
            _ => Err(EngineError::invalid(
                "JSON must be an array of records, an array of rows, or an object of sheet-name → \
                 array",
            )),
        }
    };
    match &parsed {
        Value::Array(_) => {
            let t = table_from(&parsed)?;
            wb.sheets.push(Sheet::from_table(
                if sheet_name.is_empty() { "Sheet1" } else { sheet_name },
                &t,
                true,
                true,
            ));
        },
        Value::Object(map) if map.values().all(Value::is_array) && !map.is_empty() => {
            let mut used = Vec::new();
            for (k, v) in map {
                let t = table_from(v)?;
                let safe = model::safe_sheet_name(k, &used);
                used.push(safe.clone());
                wb.sheets.push(Sheet::from_table(&safe, &t, true, true));
            }
        },
        Value::Object(map) => {
            // Single object → two-column key/value sheet.
            let t = Table {
                headers: vec!["key".into(), "value".into()],
                rows: map
                    .iter()
                    .map(|(k, v)| vec![k.clone(), tabular_core::table::value_to_cell(v.clone())])
                    .collect(),
                issues: vec![],
                delimiter: b',',
            };
            wb.sheets.push(Sheet::from_table(
                if sheet_name.is_empty() { "Sheet1" } else { sheet_name },
                &t,
                true,
                true,
            ));
        },
        _ => return Err(EngineError::invalid("JSON must be an array or object")),
    }
    write_workbook(&wb, if name.is_empty() { "data" } else { name }, out)
}

/// Converts between XLSX, ODS, CSV and JSON.
pub fn convert(
    input: &FileInput,
    to: &str,
    options: &CsvOptions,
    out: &OutputOptions,
) -> EngineResult<Value> {
    let wb = read(input, true)?;
    let name = base_name(input, "workbook");
    match to.to_ascii_lowercase().as_str() {
        "xlsx" => {
            write_workbook(&wb, &name, &OutputOptions { format: "xlsx".into(), ..out.clone() })
        },
        "ods" => write_workbook(&wb, &name, &OutputOptions { format: "ods".into(), ..out.clone() }),
        "csv" => to_csv(input, None, wb.sheets.len() > 1, options, true),
        "json" => to_json(input, None, wb.sheets.len() > 1, true, true, true),
        other => Err(EngineError::unsupported_format(format!(
            "Cannot convert to `{other}` (xlsx, ods, csv, json)"
        ))),
    }
    .map(|mut v| {
        v["from_format"] = json!(wb.format);
        v
    })
}

/// Merges workbooks by collecting sheets or appending rows.
pub fn merge(
    files: &[FileInput],
    mode: &str,
    sheet: Option<&str>,
    header: bool,
    out: &OutputOptions,
    name: &str,
) -> EngineResult<Value> {
    engine_shared::limits::check_count(files.len(), 255, "files")?;
    let books: Vec<Workbook> = files.iter().map(|f| read(f, true)).collect::<EngineResult<_>>()?;
    let mut wb = Workbook {
        format: "xlsx".into(),
        ..Default::default()
    };
    match mode {
        "" | "sheets" | "workbook" => {
            let mut used: Vec<String> = Vec::new();
            for (i, b) in books.iter().enumerate() {
                let prefix = base_name(&files[i], &format!("file{}", i + 1));
                for s in &b.sheets {
                    let wanted = if books.len() > 1
                        && used.iter().any(|u| u.eq_ignore_ascii_case(&s.name))
                    {
                        format!("{prefix}_{}", s.name)
                    } else {
                        s.name.clone()
                    };
                    let safe = model::safe_sheet_name(&wanted, &used);
                    used.push(safe.clone());
                    let mut ns = s.clone();
                    ns.name = safe;
                    wb.sheets.push(ns);
                }
            }
        },
        "append" | "rows" => {
            let tables: Vec<Table> = books
                .iter()
                .map(|b| b.sheet(sheet).map(|s| s.to_table(header)))
                .collect::<EngineResult<_>>()?;
            let names: Vec<String> = files
                .iter()
                .enumerate()
                .map(|(i, f)| base_name(f, &format!("file{}", i + 1)))
                .collect();
            let merged = tabular_core::ops::append(&tables, true, Some("source_file"), &names)?;
            wb.sheets.push(Sheet::from_table("Merged", &merged, header, true));
        },
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown merge mode `{other}` (sheets, append)"
            )))
        },
    }
    write_workbook(&wb, if name.is_empty() { "merged" } else { name }, out)
}

/// Writes every sheet to its own workbook.
pub fn split_sheets(input: &FileInput, out: &OutputOptions) -> EngineResult<Value> {
    let wb = read(input, true)?;
    let base = base_name(input, "workbook");
    let files: Vec<Value> = wb
        .sheets
        .iter()
        .map(|s| {
            let single = Workbook {
                sheets: vec![s.clone()],
                format: "xlsx".into(),
                defined_names: vec![],
                has_macros: false,
            };
            write_workbook(
                &single,
                &format!(
                    "{base}_{}",
                    s.name
                        .replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "_")
                ),
                out,
            )
        })
        .collect::<EngineResult<_>>()?;
    Ok(json!({ "files": files, "count": files.len(), "source_format": wb.format }))
}

fn table_roundtrip(
    input: &FileInput,
    sheet: Option<&str>,
    header: bool,
    f: impl FnOnce(Table) -> EngineResult<(Table, Value)>,
) -> EngineResult<(Workbook, Value, String)> {
    let wb = read(input, false)?;
    let idx = wb.sheet_index(sheet)?;
    let t = wb.sheets[idx].to_table(header);
    let (nt, report) = f(t)?;
    let mut out = wb.clone();
    out.sheets[idx] = Sheet::from_table(&wb.sheets[idx].name, &nt, header, true);
    Ok((out, report, base_name(input, "workbook")))
}

/// Removes duplicate rows from a sheet.
pub fn dedupe(
    input: &FileInput,
    sheet: Option<&str>,
    columns: &[String],
    case_insensitive: bool,
    keep: &str,
    header: bool,
    out: &OutputOptions,
) -> EngineResult<Value> {
    let (wb, report, name) = table_roundtrip(input, sheet, header, |t| {
        let before = t.rows.len();
        let (nt, removed) = tabular_core::ops::dedupe(t, columns, case_insensitive, keep)?;
        Ok((
            nt,
            json!({
                "rows_before": before,
                "rows_after": 0,
                "duplicates_removed": removed.len(),
                "duplicates": removed.into_iter().take(1000).collect::<Vec<_>>()
            }),
        ))
    })?;
    let mut v = write_workbook(&wb, &format!("{name}_deduped"), out)?;
    let mut report = report;
    report["rows_after"] = json!(
        report["rows_before"].as_u64().unwrap_or(0)
            - report["duplicates_removed"].as_u64().unwrap_or(0)
    );
    v["report"] = report;
    Ok(v)
}

/// Applies tabular cleaning to one or all sheets.
pub fn clean(
    input: &FileInput,
    sheet: Option<&str>,
    opts: &tabular_core::ops::CleanOptions,
    header: bool,
    all_sheets: bool,
    out: &OutputOptions,
) -> EngineResult<Value> {
    if all_sheets {
        let wb = read(input, false)?;
        let mut reports = Map::new();
        let mut nwb = wb.clone();
        for (i, s) in wb.sheets.iter().enumerate() {
            let (nt, report) = tabular_core::ops::clean(s.to_table(header), opts)?;
            nwb.sheets[i] = Sheet::from_table(&s.name, &nt, header, true);
            reports.insert(s.name.clone(), report);
        }
        let mut v = write_workbook(&nwb, &format!("{}_clean", base_name(input, "workbook")), out)?;
        v["report"] = Value::Object(reports);
        return Ok(v);
    }
    let (wb, report, name) =
        table_roundtrip(input, sheet, header, |t| tabular_core::ops::clean(t, opts))?;
    let mut v = write_workbook(&wb, &format!("{name}_clean"), out)?;
    v["report"] = report;
    Ok(v)
}

/// Lists formulas (optionally recalculated) with their cell references.
pub fn formulas(
    input: &FileInput,
    sheet: Option<&str>,
    recalc: bool,
    today_serial: Option<f64>,
) -> EngineResult<Value> {
    let wb = read(input, true)?;
    let targets: Vec<usize> = match sheet {
        Some(s) if !s.is_empty() => vec![wb.sheet_index(Some(s))?],
        _ => (0..wb.sheets.len()).collect(),
    };
    let mut sheets = Vec::new();
    let mut total = 0usize;
    let mut functions: Map<String, Value> = Map::new();
    for i in targets {
        let s = &wb.sheets[i];
        let mut keys: Vec<&(usize, usize)> = s.formulas.keys().collect();
        keys.sort();
        let list: Vec<Value> = keys
            .iter()
            .map(|(r, c)| {
                let f = &s.formulas[&(*r, *c)];
                let refs = formula::references(f);
                for fun in refs["functions"].as_array().cloned().unwrap_or_default() {
                    let k = fun.as_str().unwrap_or("").to_string();
                    let n = functions.get(&k).and_then(Value::as_u64).unwrap_or(0) + 1;
                    functions.insert(k, json!(n));
                }
                json!({
                    "cell": format!("{}{}", model::column_letter(c + 1), r + 1),
                    "row": r + 1,
                    "column": c + 1,
                    "formula": format!("={f}"),
                    "cached_value": s.get(*r, *c).to_json(),
                    "references": refs["cells"],
                    "functions": refs["functions"],
                    "cross_sheet": f.contains('!')
                })
            })
            .collect();
        total += list.len();
        let mut entry = json!({ "sheet": s.name, "count": list.len(), "formulas": list });
        if recalc {
            entry["recalculation"] = formula::recalculate(&wb, i, today_serial);
        }
        sheets.push(entry);
    }
    Ok(json!({
        "format": wb.format,
        "total_formulas": total,
        "functions_used": functions,
        "defined_names": wb.defined_names.iter().map(|(n, r)| json!({ "name": n, "refers_to": r })).collect::<Vec<_>>(),
        "has_macros": wb.has_macros,
        "sheets": sheets
    }))
}

/// Evaluates a formula against an inline grid or a workbook sheet.
pub fn evaluate(
    formula_text: &str,
    grid: Option<&Value>,
    file: Option<&FileInput>,
    sheet: Option<&str>,
    today_serial: Option<f64>,
) -> EngineResult<Value> {
    engine_shared::error::require_text(formula_text, "formula")?;
    if formula_text.len() > 10_000 {
        return Err(EngineError::limit("Formula is too long (10,000 character limit)"));
    }
    let (wb, idx) = if let Some(f) = file.filter(|f| !f.file.is_empty()) {
        let wb = read(f, true)?;
        let idx = wb.sheet_index(sheet)?;
        (wb, idx)
    } else if let Some(g) = grid.filter(|g| !g.is_null()) {
        (
            formula::workbook_from_grid(g).ok_or_else(|| {
                EngineError::invalid("grid must be an array of rows or an object of cell → value")
            })?,
            0,
        )
    } else {
        (
            Workbook {
                sheets: vec![Sheet::new("Sheet1")],
                format: "grid".into(),
                defined_names: vec![],
                has_macros: false,
            },
            0,
        )
    };
    let mut report = formula::evaluate_report(formula_text, &wb, idx, today_serial);
    if let Some(g) = grid.filter(|g| g.is_array()) {
        // Also recalculate any formula cells in the grid for convenience.
        report["grid_recalculation"] = formula::recalculate(&wb, 0, today_serial)["cells"].clone();
        let _ = g;
    }
    Ok(report)
}

/// Cell-by-cell diff of the same sheet in two workbooks.
pub fn diff(
    a: &FileInput,
    b: &FileInput,
    sheet: Option<&str>,
    case_insensitive: bool,
    max_changes: usize,
) -> EngineResult<Value> {
    let wa = read(a, true)?;
    let wb = read(b, true)?;
    let max_changes = if max_changes == 0 { 5000 } else { max_changes };
    let pairs: Vec<(&Sheet, Option<&Sheet>)> = match sheet {
        Some(s) if !s.is_empty() => vec![(wa.sheet(Some(s))?, wb.sheet(Some(s)).ok())],
        _ => wa
            .sheets
            .iter()
            .map(|s| (s, wb.sheets.iter().find(|x| x.name.eq_ignore_ascii_case(&s.name))))
            .collect(),
    };
    let added_sheets: Vec<&str> = wb
        .sheets
        .iter()
        .filter(|s| !wa.sheets.iter().any(|x| x.name.eq_ignore_ascii_case(&s.name)))
        .map(|s| s.name.as_str())
        .collect();
    let mut sheets = Vec::new();
    let mut total_changes = 0usize;
    let norm = |c: &Cell| {
        if case_insensitive {
            c.to_display().to_lowercase()
        } else {
            c.to_display()
        }
    };
    for (sa, sb) in pairs {
        let Some(sb) = sb else {
            sheets.push(json!({
                "sheet": sa.name,
                "status": "removed",
                "rows": sa.rows(),
                "columns": sa.cols()
            }));
            total_changes += 1;
            continue;
        };
        let rows = sa.rows().max(sb.rows());
        let cols = sa.cols().max(sb.cols());
        let mut changes = Vec::new();
        let (mut added, mut removed, mut modified, mut formula_changes) =
            (0usize, 0usize, 0usize, 0usize);
        for r in 0..rows {
            for c in 0..cols {
                let (ca, cb) = (sa.get(r, c), sb.get(r, c));
                let (fa, fb) = (sa.formulas.get(&(r, c)), sb.formulas.get(&(r, c)));
                let value_changed = norm(ca) != norm(cb);
                let formula_changed = fa != fb;
                if !value_changed && !formula_changed {
                    continue;
                }
                let kind = if ca.is_empty() && fa.is_none() {
                    added += 1;
                    "added"
                } else if cb.is_empty() && fb.is_none() {
                    removed += 1;
                    "removed"
                } else {
                    modified += 1;
                    "modified"
                };
                if formula_changed {
                    formula_changes += 1;
                }
                if changes.len() < max_changes {
                    changes.push(json!({
                        "cell": format!("{}{}", model::column_letter(c + 1), r + 1),
                        "kind": kind,
                        "before": ca.to_json(),
                        "after": cb.to_json(),
                        "formula_before": fa,
                        "formula_after": fb
                    }));
                }
            }
        }
        let n = added + removed + modified;
        total_changes += n;
        sheets.push(json!({
            "sheet": sa.name,
            "status": if n == 0 { "identical" } else { "changed" },
            "dimensions_a": [sa.rows(), sa.cols()],
            "dimensions_b": [sb.rows(), sb.cols()],
            "added": added,
            "removed": removed,
            "modified": modified,
            "formula_changes": formula_changes,
            "changes": changes,
            "truncated": n > max_changes
        }));
    }
    for name in &added_sheets {
        sheets.push(json!({ "sheet": name, "status": "added" }));
        total_changes += 1;
    }
    Ok(json!({
        "identical": total_changes == 0,
        "total_changes": total_changes,
        "format_a": wa.format,
        "format_b": wb.format,
        "sheets": sheets
    }))
}

/// Converts between column letters and numbers (`AA` ↔ 27).
pub fn column_letter(input: &Value) -> EngineResult<Value> {
    let items: Vec<Value> = match input {
        Value::Array(a) => a.clone(),
        Value::Null => return Err(EngineError::empty("input")),
        other => vec![other.clone()],
    };
    if items.is_empty() {
        return Err(EngineError::empty("input"));
    }
    let results: Vec<Value> = items
        .iter()
        .map(|item| {
            let s = match item {
                Value::Number(n) => n.to_string(),
                Value::String(s) => s.trim().to_string(),
                _ => String::new(),
            };
            if let Ok(n) = s.parse::<usize>() {
                if n == 0 || n > 16_384 {
                    return Err(EngineError::invalid(format!("Column number {n} is out of range \
                                                             (1..=16384)")));
                }
                return Ok(json!({
                    "input": item,
                    "number": n,
                    "letter": model::column_letter(n),
                    "zero_based": n - 1
                }));
            }
            if let Some((r, c)) = model::parse_cell_ref(&s) {
                return Ok(json!({
                    "input": item,
                    "cell": s,
                    "row": r + 1,
                    "column": c + 1,
                    "letter": model::column_letter(c + 1),
                    "r1c1": format!("R{}C{}", r + 1, c + 1)
                }));
            }
            if let Some((a, b)) = s.split_once(':') {
                if let (Some((r1, c1)), Some((r2, c2))) = (model::parse_cell_ref(a), model::parse_cell_ref(b)) {
                    return Ok(json!({
                        "input": item,
                        "range": s,
                        "rows": r2.abs_diff(r1) + 1,
                        "columns": c2.abs_diff(c1) + 1,
                        "cells": (r2.abs_diff(r1) + 1) * (c2.abs_diff(c1) + 1),
                        "top_left": format!("{}{}", model::column_letter(c1.min(c2) + 1), r1.min(r2) + 1),
                        "bottom_right": format!("{}{}", model::column_letter(c1.max(c2) + 1), r1.max(r2) + 1)
                    }));
                }
            }
            match model::column_number(&s) {
                Some(n) => Ok(json!({
                    "input": item,
                    "letter": s.to_ascii_uppercase(),
                    "number": n,
                    "zero_based": n - 1
                })),
                None => Err(EngineError::invalid(format!("`{s}` is not a column letter (A..XFD), \
                                                          column number or cell reference"))),
            }
        })
        .collect::<EngineResult<_>>()?;
    Ok(json!({ "results": results, "max_column": { "letter": "XFD", "number": 16384 } }))
}

/// Renders a sheet (or every sheet) as an HTML table.
pub fn to_html(
    input: &FileInput,
    sheet: Option<&str>,
    all_sheets: bool,
    header: bool,
    max_rows: usize,
) -> EngineResult<Value> {
    let wb = read(input, false)?;
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let limit = if max_rows == 0 { 10_000 } else { max_rows };
    let render = |s: &Sheet| -> String {
        let mut h = format!("<h2>{}</h2>\n<table>\n", esc(&s.name));
        for (ri, row) in s.cells.iter().take(limit).enumerate() {
            let tag = if header && ri == 0 { "th" } else { "td" };
            h.push_str("<tr>");
            for ci in 0..s.cols() {
                let c = row.get(ci).unwrap_or(&Cell::Empty);
                let align = if matches!(c, Cell::Number(_)) {
                    " style=\"text-align:right\""
                } else {
                    ""
                };
                h.push_str(&format!("<{tag}{align}>{}</{tag}>", esc(&c.to_display())));
            }
            h.push_str("</tr>\n");
        }
        h.push_str("</table>\n");
        h
    };
    let body = if all_sheets {
        wb.sheets.iter().map(render).collect::<String>()
    } else {
        render(wb.sheet(sheet)?)
    };
    let title = esc(&base_name(input, "Spreadsheet"));
    let html = format!("<!DOCTYPE html>\n<html><head><meta \
                        charset=\"utf-8\"><title>{title}</title><style>body{{font-family:Helvetica,Arial,sans-serif;font-size:10pt}}table{{border-collapse:collapse;margin-bottom:16px}}th,td{{border:1px solid #999;padding:3px 6px}}th{{background:#eee}}</style></head><body>\n{body}</body></html>\n");
    Ok(json!({
        "html": html,
        "sheets": wb.sheets.iter().map(|x| x.name.clone()).collect::<Vec<_>>(),
        "next_step": "Send `html` to pdf-engine action `from_html` to produce a PDF"
    }))
}

// --------------------------------------------------------------- dispatch --

/// Every action understood by the spreadsheet engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum SpreadsheetAction {
    /// Workbook summary.
    Inspect {
        /// Input workbook.
        #[serde(flatten)]
        input: FileInput,
    },
    /// Reads a sheet.
    Read {
        /// Input workbook.
        #[serde(flatten)]
        input: FileInput,
        /// Sheet name or index (default: first).
        #[serde(default)]
        sheet: Option<String>,
        /// First row is a header.
        #[serde(default = "default_true")]
        header: bool,
        /// First row to return.
        #[serde(default)]
        offset: usize,
        /// Maximum rows to return (0 = all).
        #[serde(default)]
        limit: usize,
        /// Return typed values instead of display strings.
        #[serde(default = "default_true")]
        typed: bool,
    },
    /// Sheet → CSV.
    ToCsv {
        /// Input workbook.
        #[serde(flatten)]
        input: FileInput,
        /// Sheet name or index (default: first).
        #[serde(default)]
        sheet: Option<String>,
        /// Export every sheet.
        #[serde(default)]
        all_sheets: bool,
        /// CSV writer options.
        #[serde(default)]
        options: CsvOptions,
        /// First row is a header.
        #[serde(default = "default_true")]
        header: bool,
    },
    /// CSV → workbook.
    FromCsv {
        /// CSV sources (one sheet each).
        #[serde(default)]
        files: Vec<CsvSource>,
        /// Inline CSV (single sheet shortcut).
        #[serde(default)]
        text: Option<String>,
        /// Output file name.
        #[serde(default)]
        name: String,
        /// Sheet name for the single-sheet shortcut.
        #[serde(default)]
        sheet_name: Option<String>,
        /// CSV parser options.
        #[serde(default)]
        options: CsvOptions,
        /// Convert numbers/booleans/dates.
        #[serde(default = "default_true")]
        typed: bool,
        /// Styling of the output.
        #[serde(default)]
        output: OutputOptions,
    },
    /// Sheet → JSON.
    ToJson {
        /// Input workbook.
        #[serde(flatten)]
        input: FileInput,
        /// Sheet name or index (default: first).
        #[serde(default)]
        sheet: Option<String>,
        /// Export every sheet.
        #[serde(default)]
        all_sheets: bool,
        /// First row is a header.
        #[serde(default = "default_true")]
        header: bool,
        /// Return typed values.
        #[serde(default = "default_true")]
        typed: bool,
        /// Pretty-print the JSON text.
        #[serde(default = "default_true")]
        pretty: bool,
    },
    /// JSON → workbook.
    FromJson {
        /// JSON text.
        #[serde(default)]
        text: Option<String>,
        /// Parsed JSON (alternative to `text`).
        #[serde(default)]
        data: Option<Value>,
        /// Sheet name.
        #[serde(default)]
        sheet_name: String,
        /// Output file name.
        #[serde(default)]
        name: String,
        /// Styling of the output.
        #[serde(default)]
        output: OutputOptions,
    },
    /// Format conversion.
    Convert {
        /// Input workbook.
        #[serde(flatten)]
        input: FileInput,
        /// Target format (`xlsx`, `ods`, `csv`, `json`).
        to: String,
        /// CSV writer options.
        #[serde(default)]
        options: CsvOptions,
        /// Styling of the output.
        #[serde(default)]
        output: OutputOptions,
    },
    /// Merges workbooks.
    Merge {
        /// Input workbooks.
        files: Vec<FileInput>,
        /// `sheets` (collect all sheets, default) or `rows` (append rows of one sheet).
        #[serde(default)]
        mode: String,
        /// Sheet to append in `rows` mode.
        #[serde(default)]
        sheet: Option<String>,
        /// First row is a header (not repeated when appending).
        #[serde(default = "default_true")]
        header: bool,
        /// Output file name.
        #[serde(default)]
        name: String,
        /// Styling of the output.
        #[serde(default)]
        output: OutputOptions,
    },
    /// Splits a workbook into one file per sheet.
    SplitSheets {
        /// Input workbook.
        #[serde(flatten)]
        input: FileInput,
        /// Styling of the output.
        #[serde(default)]
        output: OutputOptions,
    },
    /// Removes duplicate rows.
    Dedupe {
        /// Input workbook.
        #[serde(flatten)]
        input: FileInput,
        /// Sheet name or index (default: first).
        #[serde(default)]
        sheet: Option<String>,
        /// Columns that define a duplicate (all when empty).
        #[serde(default)]
        columns: Vec<String>,
        /// Ignore case when comparing.
        #[serde(default)]
        case_insensitive: bool,
        /// `first` (default) or `last` occurrence to keep.
        #[serde(default)]
        keep: String,
        /// First row is a header.
        #[serde(default = "default_true")]
        header: bool,
        /// Styling of the output.
        #[serde(default)]
        output: OutputOptions,
    },
    /// Cleans cells and headers.
    Clean {
        /// Input workbook.
        #[serde(flatten)]
        input: FileInput,
        /// Sheet name or index (default: first).
        #[serde(default)]
        sheet: Option<String>,
        /// Cleaning switches.
        #[serde(default)]
        clean: tabular_core::ops::CleanOptions,
        /// First row is a header.
        #[serde(default = "default_true")]
        header: bool,
        /// Clean every sheet.
        #[serde(default)]
        all_sheets: bool,
        /// Styling of the output.
        #[serde(default)]
        output: OutputOptions,
    },
    /// Lists formulas.
    Formulas {
        /// Input workbook.
        #[serde(flatten)]
        input: FileInput,
        /// Sheet name or index (default: first).
        #[serde(default)]
        sheet: Option<String>,
        /// Re-evaluate formulas with the built-in engine.
        #[serde(default = "default_true")]
        recalculate: bool,
        /// Serial date for `TODAY()`/`NOW()`.
        #[serde(default)]
        today_serial: Option<f64>,
    },
    /// Evaluates a single formula.
    Evaluate {
        /// Formula text (with or without `=`).
        formula: String,
        /// Inline grid of rows (`[[1, 2], [3, 4]]`).
        #[serde(default)]
        grid: Option<Value>,
        /// Workbook to evaluate against (alternative to `grid`).
        #[serde(default)]
        file: Option<FileInput>,
        /// Sheet name or index in `file`.
        #[serde(default)]
        sheet: Option<String>,
        /// Serial date for `TODAY()`/`NOW()`.
        #[serde(default)]
        today_serial: Option<f64>,
    },
    /// Diffs two workbooks.
    Diff {
        /// First workbook.
        a: FileInput,
        /// Second workbook.
        b: FileInput,
        /// Sheet name or index (default: first).
        #[serde(default)]
        sheet: Option<String>,
        /// Ignore case when comparing text.
        #[serde(default)]
        case_insensitive: bool,
        /// Maximum changes reported.
        #[serde(default)]
        max_changes: usize,
    },
    /// Column letter ↔ number.
    ColumnLetter {
        /// Letter, number or an array of them.
        input: Value,
    },
    /// Sheet → HTML table.
    ToHtml {
        /// Input workbook.
        #[serde(flatten)]
        input: FileInput,
        /// Sheet name or index (default: first).
        #[serde(default)]
        sheet: Option<String>,
        /// Render every sheet.
        #[serde(default)]
        all_sheets: bool,
        /// First row is a header.
        #[serde(default = "default_true")]
        header: bool,
        /// Maximum rows rendered (0 = all).
        #[serde(default)]
        max_rows: usize,
    },
}

fn default_true() -> bool {
    true
}

/// Dispatches a [`SpreadsheetAction`] to its implementation.
pub fn handle_action(action: SpreadsheetAction) -> EngineResult<Value> {
    match action {
        SpreadsheetAction::Inspect { input } => inspect(&input),
        SpreadsheetAction::Read {
            input,
            sheet,
            header,
            offset,
            limit,
            typed,
        } => read_sheet(&input, sheet.as_deref(), header, offset, limit, typed),
        SpreadsheetAction::ToCsv {
            input,
            sheet,
            all_sheets,
            options,
            header,
        } => to_csv(&input, sheet.as_deref(), all_sheets, &options, header),
        SpreadsheetAction::FromCsv {
            mut files,
            text,
            name,
            sheet_name,
            options,
            typed,
            output,
        } => {
            if let Some(t) = text {
                files.insert(
                    0,
                    CsvSource {
                        text: Some(t),
                        file: None,
                        name: if name.is_empty() { None } else { Some(name.clone()) },
                        sheet_name,
                    },
                );
            }
            if files.is_empty() {
                return Err(EngineError::empty("csv"));
            }
            from_csv(&files, &options, typed, &output, &name)
        },
        SpreadsheetAction::ToJson {
            input,
            sheet,
            all_sheets,
            header,
            typed,
            pretty,
        } => to_json(&input, sheet.as_deref(), all_sheets, header, typed, pretty),
        SpreadsheetAction::FromJson { text, data, sheet_name, name, output } => {
            from_json(text.as_deref(), data.as_ref(), &sheet_name, &output, &name)
        },
        SpreadsheetAction::Convert { input, to, options, output } => {
            convert(&input, &to, &options, &output)
        },
        SpreadsheetAction::Merge { files, mode, sheet, header, name, output } => {
            merge(&files, &mode, sheet.as_deref(), header, &output, &name)
        },
        SpreadsheetAction::SplitSheets { input, output } => split_sheets(&input, &output),
        SpreadsheetAction::Dedupe {
            input,
            sheet,
            columns,
            case_insensitive,
            keep,
            header,
            output,
        } => dedupe(&input, sheet.as_deref(), &columns, case_insensitive, &keep, header, &output),
        SpreadsheetAction::Clean {
            input,
            sheet,
            clean: c,
            header,
            all_sheets,
            output,
        } => clean(&input, sheet.as_deref(), &c, header, all_sheets, &output),
        SpreadsheetAction::Formulas { input, sheet, recalculate, today_serial } => {
            formulas(&input, sheet.as_deref(), recalculate, today_serial)
        },
        SpreadsheetAction::Evaluate { formula, grid, file, sheet, today_serial } => {
            evaluate(&formula, grid.as_ref(), file.as_ref(), sheet.as_deref(), today_serial)
        },
        SpreadsheetAction::Diff {
            a,
            b,
            sheet,
            case_insensitive,
            max_changes,
        } => diff(&a, &b, sheet.as_deref(), case_insensitive, max_changes),
        SpreadsheetAction::ColumnLetter { input } => column_letter(&input),
        SpreadsheetAction::ToHtml {
            input,
            sheet,
            all_sheets,
            header,
            max_rows,
        } => to_html(&input, sheet.as_deref(), all_sheets, header, max_rows),
    }
}

engine_shared::export_engine!(handle_action, SpreadsheetAction);

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_xlsx() -> Vec<u8> {
        let src = CsvSource {
            text: Some(
                "name,qty,price,when\nA,2,1.5,2024-01-02\nB,3,2,2024-01-03\nA,2,1.5,2024-01-02\n"
                    .into(),
            ),
            ..Default::default()
        };
        let v = from_csv(&[src], &CsvOptions::default(), true, &OutputOptions::default(), "sample")
            .unwrap();
        engine_shared::b64::decode(v["file"].as_str().unwrap()).unwrap()
    }

    #[test]
    fn csv_xlsx_roundtrip() {
        let bytes = sample_xlsx();
        let input = FileInput {
            file: bytes.clone(),
            name: Some("sample.xlsx".into()),
        };
        let info = inspect(&input).unwrap();
        assert_eq!(info["format"], "xlsx");
        assert_eq!(info["sheets"][0]["rows"], 4);
        let r = read_sheet(&input, None, true, 0, 10, true).unwrap();
        assert_eq!(r["rows"][0][1], 2);
        assert_eq!(r["rows"][0][3], "2024-01-02");
        assert_eq!(r["columns"][2]["type"], "number");
        let csv = to_csv(&input, None, false, &CsvOptions::default(), true).unwrap();
        assert_eq!(
            csv["text"],
            "name,qty,price,when\nA,2,1.5,2024-01-02\nB,3,2,2024-01-03\nA,2,1.5,2024-01-02\n"
        );
        let j = to_json(&input, None, false, true, true, false).unwrap();
        assert_eq!(j["data"][1]["qty"], 3);
        let d = dedupe(&input, None, &[], false, "first", true, &OutputOptions::default()).unwrap();
        assert_eq!(d["report"]["duplicates_removed"], 1);
        assert_eq!(d["report"]["rows_after"], 2);
        let ods =
            convert(&input, "ods", &CsvOptions::default(), &OutputOptions::default()).unwrap();
        assert_eq!(ods["filename"], "sample.ods");
        let ods_bytes = engine_shared::b64::decode(ods["file"].as_str().unwrap()).unwrap();
        let back = convert(
            &FileInput {
                file: ods_bytes,
                name: Some("x.ods".into()),
            },
            "xlsx",
            &CsvOptions::default(),
            &OutputOptions::default(),
        )
        .unwrap();
        assert_eq!(back["from_format"], "ods");
        let split = split_sheets(&input, &OutputOptions::default()).unwrap();
        assert_eq!(split["count"], 1);
        let merged = merge(
            &[input.clone(), input.clone()],
            "sheets",
            None,
            true,
            &OutputOptions::default(),
            "",
        )
        .unwrap();
        assert_eq!(merged["sheets"].as_array().unwrap().len(), 2);
        let appended = merge(
            &[input.clone(), input.clone()],
            "append",
            None,
            true,
            &OutputOptions::default(),
            "",
        )
        .unwrap();
        assert_eq!(appended["sheets"][0]["rows"], 7);
        let html = to_html(&input, None, false, true, 0).unwrap();
        assert!(html["html"].as_str().unwrap().contains("<th>name</th>"));
    }

    #[test]
    fn formulas_and_evaluation() {
        let wb =
            formula::workbook_from_grid(&json!([["a", "b"], [1, "=A2*2"], [3, "=SUM(A2:A3)"]]))
                .unwrap();
        let bytes = writer::write_xlsx(&wb, &XlsxOptions::default()).unwrap();
        let input = FileInput { file: bytes, name: Some("f.xlsx".into()) };
        let f = formulas(&input, None, true, None).unwrap();
        assert_eq!(f["total_formulas"], 2);
        assert_eq!(f["sheets"][0]["recalculation"]["cells"][1]["computed"], 4.0);
        assert_eq!(f["functions_used"]["SUM"], 1);
        let e = evaluate("=SUM(A1:A3)*2", Some(&json!([[1], [2], [3]])), None, None, None).unwrap();
        assert_eq!(e["value"], 12.0);
        let e = evaluate("=B2+1", None, Some(&input), None, None).unwrap();
        assert_eq!(e["value"], 3.0);
        assert!(evaluate("", None, None, None, None).is_err());
        let c = column_letter(&json!(["AA", 28, "B3", "A1:C3"])).unwrap();
        assert_eq!(c["results"][0]["number"], 27);
        assert_eq!(c["results"][1]["letter"], "AB");
        assert_eq!(c["results"][2]["row"], 3);
        assert_eq!(c["results"][3]["cells"], 9);
        assert!(column_letter(&json!("XFE")).is_err());
    }

    #[test]
    fn json_and_diff() {
        let v = from_json(
            Some("[{\"a\":1,\"b\":\"x\"},{\"a\":2,\"b\":\"y\"}]"),
            None,
            "",
            &OutputOptions::default(),
            "",
        )
        .unwrap();
        let a = FileInput {
            file: engine_shared::b64::decode(v["file"].as_str().unwrap()).unwrap(),
            name: Some("a.xlsx".into()),
        };
        let v = from_json(
            Some("[{\"a\":1,\"b\":\"x\"},{\"a\":2,\"b\":\"z\"},{\"a\":3,\"b\":\"w\"}]"),
            None,
            "",
            &OutputOptions::default(),
            "",
        )
        .unwrap();
        let b = FileInput {
            file: engine_shared::b64::decode(v["file"].as_str().unwrap()).unwrap(),
            name: Some("b.xlsx".into()),
        };
        let d = diff(&a, &b, None, false, 0).unwrap();
        assert_eq!(d["identical"], false);
        assert_eq!(d["sheets"][0]["modified"], 1);
        assert_eq!(d["sheets"][0]["added"], 2);
        assert_eq!(diff(&a, &a, None, false, 0).unwrap()["identical"], true);
        assert!(from_json(Some("{bad"), None, "", &OutputOptions::default(), "").is_err());
        let multi = from_json(
            None,
            Some(&json!({ "S1": [[1, 2]], "S2": [{ "k": 1 }] })),
            "",
            &OutputOptions::default(),
            "",
        )
        .unwrap();
        assert_eq!(multi["sheets"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn bad_inputs() {
        let err = Workbook::read(b"PK\x03\x04garbage garbage garbage", None, false).unwrap_err();
        assert!(
            matches!(
                err.code,
                engine_shared::error::ErrorCode::Malformed
                    | engine_shared::error::ErrorCode::UnsupportedFormat
            ),
            "{err:?}"
        );
        let err = Workbook::read(
            &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1, 0, 0, 0, 0],
            None,
            false,
        )
        .unwrap_err();
        assert_eq!(err.code, engine_shared::error::ErrorCode::Malformed);
        assert!(Workbook::read(b"", None, false).is_err());
    }
}
