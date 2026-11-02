//! Workbook model and readers (XLSX / XLSM / XLSB / XLS / ODS via calamine).

use calamine::{Data, Reader};
use engine_shared::{EngineError, EngineResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Cursor;
use tabular_core::table::Table;

/// A single cell value. Dates are kept as ISO-8601 strings so they survive
/// every conversion path unambiguously.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Cell {
    /// Empty cell.
    #[default]
    Empty,
    /// Numeric value.
    Number(f64),
    /// Text value.
    Text(String),
    /// Boolean value.
    Bool(bool),
    /// ISO date / datetime / time string.
    Date(String),
    /// Error value such as `#REF!`.
    Error(String),
}

impl Cell {
    /// `true` for [`Cell::Empty`].
    pub fn is_empty(&self) -> bool {
        matches!(self, Cell::Empty)
    }

    /// Display text.
    pub fn to_display(&self) -> String {
        match self {
            Cell::Empty => String::new(),
            Cell::Number(n) => fmt_number(*n),
            Cell::Text(s) => s.clone(),
            Cell::Bool(b) => {
                if *b {
                    "TRUE".into()
                } else {
                    "FALSE".into()
                }
            },
            Cell::Date(d) => d.clone(),
            Cell::Error(e) => e.clone(),
        }
    }

    /// JSON value (number/string/bool/null).
    pub fn to_json(&self) -> Value {
        match self {
            Cell::Empty => Value::Null,
            Cell::Number(n) => crate::formula::num_json(*n),
            Cell::Text(s) => Value::String(s.clone()),
            Cell::Bool(b) => Value::Bool(*b),
            Cell::Date(d) => Value::String(d.clone()),
            Cell::Error(e) => json!({ "error": e }),
        }
    }

    /// Builds a cell from a JSON value.
    pub fn from_json(v: &Value) -> Cell {
        match v {
            Value::Null => Cell::Empty,
            Value::Bool(b) => Cell::Bool(*b),
            Value::Number(n) => Cell::Number(n.as_f64().unwrap_or(0.0)),
            Value::String(s) => Cell::from_text(s),
            other => Cell::Text(other.to_string()),
        }
    }

    /// Parses free text the way a spreadsheet would when typing into a cell.
    pub fn from_text(s: &str) -> Cell {
        let t = s.trim();
        if t.is_empty() {
            return Cell::Empty;
        }
        if let Ok(n) = t.parse::<f64>() {
            if n.is_finite()
                && !t.starts_with('+')
                && !(t.len() > 1 && t.starts_with('0') && !t.starts_with("0."))
            {
                return Cell::Number(n);
            }
        }
        match t.to_ascii_lowercase().as_str() {
            "true" => return Cell::Bool(true),
            "false" => return Cell::Bool(false),
            _ => {},
        }
        if tabular_core::table::classify(t) == tabular_core::table::ColumnType::Date
            && t.len() >= 10
            && &t[4..5] == "-"
        {
            return Cell::Date(t.to_string());
        }
        Cell::Text(s.to_string())
    }

    /// Numeric value, parsing numeric text and booleans.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Cell::Number(n) => Some(*n),
            Cell::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Cell::Text(s) => s.trim().parse::<f64>().ok(),
            _ => None,
        }
    }
}

/// Formats a number the way spreadsheets display it (no trailing zeros).
pub fn fmt_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        let s = format!("{:.10}", n);
        let s = s.trim_end_matches('0').trim_end_matches('.');
        s.to_string()
    }
}

/// One worksheet.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Sheet {
    /// Sheet name.
    pub name: String,
    /// Dense grid (rows × cols), trimmed to the used range.
    pub cells: Vec<Vec<Cell>>,
    /// Formulas by (row, col) zero-based.
    #[serde(skip)]
    pub formulas: HashMap<(usize, usize), String>,
    /// Sheet is hidden in the workbook.
    #[serde(skip)]
    pub hidden: bool,
    /// Merged ranges in A1 notation.
    #[serde(skip)]
    pub merged: Vec<String>,
}

impl Sheet {
    /// Creates an empty sheet.
    pub fn new(name: &str) -> Self {
        Sheet {
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// Number of rows in the used range.
    pub fn rows(&self) -> usize {
        self.cells.len()
    }

    /// Number of columns in the used range.
    pub fn cols(&self) -> usize {
        self.cells.iter().map(Vec::len).max().unwrap_or(0)
    }

    /// Cell at (row, col), `Cell::Empty` when outside the grid.
    pub fn get(&self, row: usize, col: usize) -> &Cell {
        static EMPTY: Cell = Cell::Empty;
        self.cells.get(row).and_then(|r| r.get(col)).unwrap_or(&EMPTY)
    }

    /// Sets a cell, growing the grid as needed.
    pub fn set(&mut self, row: usize, col: usize, cell: Cell) {
        while self.cells.len() <= row {
            self.cells.push(Vec::new());
        }
        let r = &mut self.cells[row];
        while r.len() <= col {
            r.push(Cell::Empty);
        }
        r[col] = cell;
    }

    /// Pads every row to the same width.
    pub fn normalize(&mut self) {
        let w = self.cols();
        for r in self.cells.iter_mut() {
            r.resize(w, Cell::Empty);
        }
        // Trim trailing empty rows.
        while self.cells.last().is_some_and(|r| r.iter().all(Cell::is_empty)) {
            self.cells.pop();
        }
    }

    /// Used range in A1 notation (`A1:C10`).
    pub fn used_range(&self) -> Option<String> {
        if self.cells.is_empty() || self.cols() == 0 {
            return None;
        }
        Some(format!("A1:{}{}", column_letter(self.cols()), self.rows()))
    }

    /// Converts to the CSV engine table model (first row as header when `header`).
    pub fn to_table(&self, header: bool) -> Table {
        let mut rows: Vec<Vec<String>> = self
            .cells
            .iter()
            .map(|r| r.iter().map(Cell::to_display).collect())
            .collect();
        let width = self.cols();
        for r in rows.iter_mut() {
            r.resize(width, String::new());
        }
        let headers = if header && !rows.is_empty() {
            let h = rows.remove(0);
            h.into_iter()
                .enumerate()
                .map(|(i, c)| {
                    if c.trim().is_empty() {
                        format!("column_{}", i + 1)
                    } else {
                        c
                    }
                })
                .collect()
        } else {
            (1..=width).map(|i| format!("column_{i}")).collect()
        };
        Table {
            headers,
            rows,
            issues: vec![],
            delimiter: b',',
        }
    }

    /// Builds a sheet from a tabular table.
    pub fn from_table(name: &str, t: &Table, header: bool, typed: bool) -> Sheet {
        let mut sheet = Sheet::new(name);
        let mut r = 0;
        if header {
            for (c, h) in t.headers.iter().enumerate() {
                sheet.set(0, c, Cell::Text(h.clone()));
            }
            r = 1;
        }
        let types = if typed { t.column_types() } else { vec![] };
        for row in &t.rows {
            for (c, v) in row.iter().enumerate() {
                let cell = if typed {
                    match tabular_core::table::cell_to_value(
                        v,
                        types.get(c).copied().unwrap_or(tabular_core::table::ColumnType::Text),
                    ) {
                        Value::Null => Cell::Empty,
                        Value::Number(n) => Cell::Number(n.as_f64().unwrap_or(0.0)),
                        Value::Bool(b) => Cell::Bool(b),
                        Value::String(s) => {
                            if types.get(c) == Some(&tabular_core::table::ColumnType::Date) {
                                Cell::Date(s)
                            } else {
                                Cell::Text(s)
                            }
                        },
                        other => Cell::Text(other.to_string()),
                    }
                } else if v.is_empty() {
                    Cell::Empty
                } else {
                    Cell::Text(v.clone())
                };
                sheet.set(r, c, cell);
            }
            r += 1;
        }
        sheet.normalize();
        sheet
    }

    /// JSON summary (name, dimensions, used range, formula count).
    pub fn summary(&self) -> Value {
        let mut numbers = 0;
        let mut texts = 0;
        let mut empties = 0;
        let mut dates = 0;
        let mut errors = 0;
        for c in self.cells.iter().flatten() {
            match c {
                Cell::Empty => empties += 1,
                Cell::Number(_) | Cell::Bool(_) => numbers += 1,
                Cell::Text(_) => texts += 1,
                Cell::Date(_) => dates += 1,
                Cell::Error(_) => errors += 1,
            }
        }
        json!({
            "name": self.name,
            "rows": self.rows(),
            "columns": self.cols(),
            "used_range": self.used_range(),
            "formulas": self.formulas.len(),
            "hidden": self.hidden,
            "merged_ranges": self.merged,
            "cells": {
                "numbers": numbers,
                "text": texts,
                "dates": dates,
                "errors": errors,
                "empty": empties
            },
        })
    }
}

/// An in-memory workbook.
#[derive(Debug, Clone, Default)]
pub struct Workbook {
    /// Worksheets in order.
    pub sheets: Vec<Sheet>,
    /// Source format label.
    pub format: String,
    /// Defined names (`name`, `reference`).
    pub defined_names: Vec<(String, String)>,
    /// Source file contained VBA macros.
    pub has_macros: bool,
}

/// Detected container format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Office Open XML (`.xlsx` / `.xlsm`).
    Xlsx,
    /// Binary Excel (`.xlsb`).
    Xlsb,
    /// Legacy BIFF Excel (`.xls`).
    Xls,
    /// OpenDocument spreadsheet.
    Ods,
    /// Plain CSV.
    Csv,
}

impl Format {
    /// Lower-case label.
    pub fn label(self) -> &'static str {
        match self {
            Format::Xlsx => "xlsx",
            Format::Xlsb => "xlsb",
            Format::Xls => "xls",
            Format::Ods => "ods",
            Format::Csv => "csv",
        }
    }
}

/// Sniffs the file format from magic bytes and (for zip containers) the
/// member names. Falls back to the `hint` (file extension) when ambiguous.
pub fn detect_format(bytes: &[u8], hint: Option<&str>) -> EngineResult<Format> {
    if bytes.len() < 8 {
        return Err(EngineError::malformed("File is too small to be a spreadsheet"));
    }
    if bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]) {
        // CFB: legacy XLS or an encrypted OOXML package (checked by the reader).
        return Ok(Format::Xls);
    }
    if bytes.starts_with(b"PK\x03\x04") {
        let names = zip_member_names(bytes).ok_or_else(|| {
            EngineError::malformed(
                "File starts like a ZIP/OOXML package but the archive is corrupt or truncated",
            )
        })?;
        if names.iter().any(|n| n == "xl/workbook.bin") {
            return Ok(Format::Xlsb);
        }
        if names.iter().any(|n| n == "xl/workbook.xml") {
            return Ok(Format::Xlsx);
        }
        if names.iter().any(|n| n == "content.xml") || names.iter().any(|n| n == "mimetype") {
            return Ok(Format::Ods);
        }
        return Err(EngineError::unsupported_format(
            "ZIP archive does not contain a workbook (expected xl/workbook.xml, xl/workbook.bin \
             or content.xml)",
        ));
    }
    if let Some(h) = hint {
        let ext = h.rsplit('.').next().unwrap_or(h).to_ascii_lowercase();
        match ext.as_str() {
            "csv" | "tsv" | "txt" => return Ok(Format::Csv),
            _ => {},
        }
    }
    let looks_textual = |b: &[u8]| {
        let sample = &b[..b.len().min(4096)];
        !sample.contains(&0)
            && sample
                .iter()
                .filter(|x| x.is_ascii_control() && !matches!(x, b'\n' | b'\r' | b'\t'))
                .count()
                * 20
                < sample.len().max(1)
    };
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF])
        || bytes.starts_with(&[0xFF, 0xFE])
        || bytes.starts_with(&[0xFE, 0xFF])
        || (std::str::from_utf8(bytes).is_ok() && looks_textual(bytes))
    {
        return Ok(Format::Csv);
    }
    Err(EngineError::unsupported_format(
        "Unrecognised spreadsheet format (supported: xlsx, xlsm, xlsb, xls, ods, csv)",
    ))
}

fn zip_member_names(bytes: &[u8]) -> Option<Vec<String>> {
    zip::ZipArchive::new(Cursor::new(bytes))
        .ok()
        .map(|z| z.file_names().map(str::to_string).collect())
}

fn convert_data(d: &Data) -> Cell {
    match d {
        Data::Empty => Cell::Empty,
        Data::Int(i) => Cell::Number(*i as f64),
        Data::Float(f) => Cell::Number(*f),
        Data::String(s) => Cell::Text(s.clone()),
        Data::Bool(b) => Cell::Bool(*b),
        Data::DateTime(dt) => {
            let (y, mo, d, h, mi, s, ms) = dt.to_ymd_hms_milli();
            if dt.is_duration() {
                let total = dt.as_f64();
                let hours = (total * 24.0).floor();
                let mins = ((total * 24.0 - hours) * 60.0).floor();
                let secs = (((total * 24.0 - hours) * 60.0 - mins) * 60.0).round();
                Cell::Date(format!("{:02}:{:02}:{:02}", hours as i64, mins as i64, secs as i64))
            } else if dt.as_f64() < 1.0 {
                Cell::Date(format!("{h:02}:{mi:02}:{s:02}"))
            } else if h == 0 && mi == 0 && s == 0 && ms == 0 {
                Cell::Date(format!("{y:04}-{mo:02}-{d:02}"))
            } else {
                Cell::Date(format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}"))
            }
        },
        Data::DateTimeIso(s) | Data::DurationIso(s) => Cell::Date(s.clone()),
        Data::Error(e) => Cell::Error(e.to_string()),
    }
}

fn map_err<E: std::fmt::Display>(e: E) -> EngineError {
    let msg = e.to_string();
    if msg.to_ascii_lowercase().contains("password") || msg.contains("Encrypted") {
        EngineError::encrypted(
            "Workbook is password protected; remove the password in your spreadsheet application \
             first",
        )
    } else {
        EngineError::malformed(format!("Cannot read workbook: {msg}"))
    }
}

fn read_with<R: Reader<Cursor<Vec<u8>>>>(
    mut wb: R,
    format: Format,
    want_formulas: bool,
) -> EngineResult<Workbook>
where
    R::Error: std::fmt::Display,
{
    let meta: Vec<(String, bool)> = wb
        .sheets_metadata()
        .iter()
        .map(|s| (s.name.clone(), !matches!(s.visible, calamine::SheetVisible::Visible)))
        .collect();
    let defined_names = wb.defined_names().to_vec();
    let has_macros = matches!(wb.vba_project(), Ok(Some(_)));
    let mut sheets = Vec::new();
    for (name, hidden) in meta {
        let range = wb.worksheet_range(&name).map_err(map_err)?;
        let mut sheet = Sheet::new(&name);
        sheet.hidden = hidden;
        if let Some((r0, c0)) = range.start() {
            let (h, w) = range.get_size();
            engine_shared::limits::check_size(h * w, 50_000_000, "sheet cell count")?;
            sheet.cells = vec![vec![Cell::Empty; (c0 as usize) + w]; (r0 as usize) + h];
            for (pr, pc, d) in range.used_cells() {
                let (r, c) = (pr + r0 as usize, pc + c0 as usize);
                sheet.cells[r][c] = convert_data(d);
            }
        }
        if want_formulas {
            if let Ok(frange) = wb.worksheet_formula(&name) {
                if let Some((r0, c0)) = frange.start() {
                    for (pr, pc, f) in frange.used_cells() {
                        if !f.is_empty() {
                            sheet.formulas.insert((pr + r0 as usize, pc + c0 as usize), f.clone());
                        }
                    }
                }
            }
        }
        sheet.normalize();
        sheets.push(sheet);
    }
    Ok(Workbook {
        sheets,
        format: format.label().into(),
        defined_names,
        has_macros,
    })
}

impl Workbook {
    /// Reads any supported spreadsheet (or CSV text) from bytes.
    pub fn read(bytes: &[u8], hint: Option<&str>, want_formulas: bool) -> EngineResult<Workbook> {
        engine_shared::error::require_bytes(bytes, "file")?;
        engine_shared::limits::check_size(
            bytes.len(),
            engine_shared::limits::MAX_FILE_BYTES,
            "file",
        )?;
        let format = detect_format(bytes, hint)?;
        let cursor = Cursor::new(bytes.to_vec());
        match format {
            Format::Xlsx => {
                let wb = calamine::Xlsx::new(cursor).map_err(map_err)?;
                let mut out = read_with(wb, format, want_formulas)?;
                if let Some(h) = hint {
                    if h.to_ascii_lowercase().ends_with(".xlsm") {
                        out.format = "xlsm".into();
                    }
                }
                // Merged regions.
                if let Ok(mut wb2) = calamine::Xlsx::new(Cursor::new(bytes.to_vec())) {
                    if wb2.load_merged_regions().is_ok() {
                        for (sheet, _, dims) in wb2.merged_regions().clone() {
                            if let Some(s) = out.sheets.iter_mut().find(|s| s.name == sheet) {
                                s.merged.push(format!(
                                    "{}{}:{}{}",
                                    column_letter(dims.start.1 as usize + 1),
                                    dims.start.0 + 1,
                                    column_letter(dims.end.1 as usize + 1),
                                    dims.end.0 + 1
                                ));
                            }
                        }
                    }
                }
                Ok(out)
            },
            Format::Xlsb => {
                read_with(calamine::Xlsb::new(cursor).map_err(map_err)?, format, want_formulas)
            },
            Format::Xls => match calamine::Xls::new(cursor) {
                Ok(wb) => read_with(wb, format, want_formulas),
                Err(e) => {
                    let msg = e.to_string();
                    if msg.to_ascii_lowercase().contains("password") {
                        return Err(EngineError::encrypted(
                            "Workbook is password protected; remove the password in your \
                             spreadsheet application first",
                        ));
                    }
                    // An OOXML package inside CFB means "encrypted xlsx".
                    match calamine::Xlsx::new(Cursor::new(bytes.to_vec())) {
                        Err(calamine::XlsxError::Password) => Err(EngineError::encrypted(
                            "Workbook is password protected (encrypted OOXML package)",
                        )),
                        _ => Err(map_err(e)),
                    }
                },
            },
            Format::Ods => {
                read_with(calamine::Ods::new(cursor).map_err(map_err)?, format, want_formulas)
            },
            Format::Csv => {
                let d = tabular_core::input::decode_bytes(bytes, None)?;
                let t = Table::parse(&d.text, &tabular_core::table::CsvOptions::default())?;
                let name = hint
                    .map(|h| {
                        h.rsplit('/')
                            .next()
                            .unwrap_or(h)
                            .rsplit_once('.')
                            .map(|(n, _)| n.to_string())
                            .unwrap_or_else(|| h.to_string())
                    })
                    .unwrap_or_else(|| "Sheet1".into());
                Ok(Workbook {
                    sheets: vec![Sheet::from_table(&name, &t, true, true)],
                    format: "csv".into(),
                    defined_names: vec![],
                    has_macros: false,
                })
            },
        }
    }

    /// Finds a sheet by name or index (first when `None`).
    pub fn sheet(&self, reference: Option<&str>) -> EngineResult<&Sheet> {
        let Some(r) = reference.filter(|r| !r.trim().is_empty()) else {
            return self
                .sheets
                .first()
                .ok_or_else(|| EngineError::empty("workbook has no sheets"));
        };
        if let Some(s) = self.sheets.iter().find(|s| s.name == r) {
            return Ok(s);
        }
        if let Some(s) = self.sheets.iter().find(|s| s.name.eq_ignore_ascii_case(r.trim())) {
            return Ok(s);
        }
        if let Ok(n) = r.trim().parse::<usize>() {
            if n >= 1 && n <= self.sheets.len() {
                return Ok(&self.sheets[n - 1]);
            }
        }
        Err(EngineError::not_found(format!(
            "Sheet `{r}` not found (available: {})",
            self.sheets
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )))
    }

    /// Finds a sheet index by name or index.
    pub fn sheet_index(&self, reference: Option<&str>) -> EngineResult<usize> {
        let s = self.sheet(reference)?;
        Ok(self.sheets.iter().position(|x| std::ptr::eq(x, s)).unwrap_or(0))
    }

    /// JSON summary of every sheet.
    pub fn summary(&self) -> Value {
        json!({
            "format": self.format,
            "sheet_count": self.sheets.len(),
            "sheets": self.sheets.iter().map(Sheet::summary).collect::<Vec<_>>(),
            "defined_names": self.defined_names.iter().map(|(n, r)| json!({ "name": n, "refers_to": r })).collect::<Vec<_>>(),
            "has_macros": self.has_macros,
            "total_cells": self.sheets.iter().map(|s| s.rows() * s.cols()).sum::<usize>(),
        })
    }
}

/// 1-based column number → letters (`1 → A`, `27 → AA`).
pub fn column_letter(mut n: usize) -> String {
    let mut s = Vec::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        s.push((b'A' + rem as u8) as char);
        n = (n - 1) / 26;
    }
    s.iter().rev().collect()
}

/// Letters → 1-based column number.
pub fn column_number(letters: &str) -> Option<usize> {
    let l = letters.trim();
    if l.is_empty() || l.len() > 3 || !l.bytes().all(|b| b.is_ascii_alphabetic()) {
        return None;
    }
    let n = l
        .bytes()
        .fold(0usize, |acc, b| acc * 26 + (b.to_ascii_uppercase() - b'A' + 1) as usize);
    if n > 16_384 {
        None
    } else {
        Some(n)
    }
}

/// Parses `B3` → (row 2, col 1) zero-based; `$B$3` accepted.
pub fn parse_cell_ref(r: &str) -> Option<(usize, usize)> {
    let r = r.replace('$', "");
    let split = r.find(|c: char| c.is_ascii_digit())?;
    let (letters, digits) = r.split_at(split);
    let col = column_number(letters)?;
    let row: usize = digits.parse().ok()?;
    if row == 0 {
        return None;
    }
    Some((row - 1, col - 1))
}

/// Sanitises a sheet name to Excel's rules (≤31 chars, no `[]:*?/\`).
pub fn safe_sheet_name(name: &str, existing: &[String]) -> String {
    let mut s: String = name
        .chars()
        .filter(|c| !matches!(c, '[' | ']' | ':' | '*' | '?' | '/' | '\\'))
        .take(31)
        .collect();
    if s.trim().is_empty() {
        s = "Sheet".into();
    }
    let base = s.clone();
    let mut n = 2;
    while existing.iter().any(|e| e.eq_ignore_ascii_case(&s)) {
        let suffix = format!(" ({n})");
        s = format!("{}{}", base.chars().take(31 - suffix.len()).collect::<String>(), suffix);
        n += 1;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_and_refs() {
        assert_eq!(column_letter(1), "A");
        assert_eq!(column_letter(26), "Z");
        assert_eq!(column_letter(27), "AA");
        assert_eq!(column_letter(703), "AAA");
        assert_eq!(column_number("AA"), Some(27));
        assert_eq!(column_number("xfd"), Some(16384));
        assert_eq!(column_number("XFE"), None);
        assert_eq!(parse_cell_ref("$B$3"), Some((2, 1)));
        assert_eq!(parse_cell_ref("3B"), None);
        assert_eq!(safe_sheet_name("a/b:c", &["abc".into()]), "abc (2)");
    }

    #[test]
    fn cell_parsing() {
        assert_eq!(Cell::from_text("12.5"), Cell::Number(12.5));
        assert_eq!(Cell::from_text("007"), Cell::Text("007".into()));
        assert_eq!(Cell::from_text("TRUE"), Cell::Bool(true));
        assert_eq!(Cell::from_text("2024-03-01"), Cell::Date("2024-03-01".into()));
        assert_eq!(Cell::from_text(""), Cell::Empty);
        assert_eq!(fmt_number(1.5), "1.5");
        assert_eq!(fmt_number(3.0), "3");
        assert_eq!(fmt_number(0.1 + 0.2), "0.3");
    }

    #[test]
    fn detect_and_csv_read() {
        assert_eq!(detect_format(b"a,b\n1,2\n", Some("x.csv")).unwrap(), Format::Csv);
        assert!(detect_format(b"\x00\x01\x02\x03\x04\x05\x06\x07\x08", None).is_err());
        let wb = Workbook::read(b"a,b\n1,x\n", Some("data.csv"), false).unwrap();
        assert_eq!(wb.sheets[0].name, "data");
        assert_eq!(wb.sheets[0].get(1, 0), &Cell::Number(1.0));
        assert!(wb.sheet(Some("nope")).is_err());
        assert!(wb.sheet(Some("1")).is_ok());
    }
}
