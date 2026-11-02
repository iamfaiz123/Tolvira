//! In-memory tabular model shared by every CSV action.
//!
//! CSV files are parsed with the `csv` crate (RFC 4180 with lenient quoting)
//! into a [`Table`] of string cells. Parsing is tolerant by default: rows with a
//! different number of fields are padded/truncated and reported as issues
//! rather than aborting, so a user can still *see* a slightly broken file.

use engine_shared::{EngineError, EngineResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Parsing/serialisation options accepted by every action under `options`.
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(default)]
pub struct CsvOptions {
    /// Field delimiter. Empty or missing means auto-detect (`,`, `;`, `\t`, `|`).
    pub delimiter: String,
    /// Whether the first row contains column names (default `true`).
    pub has_header: bool,
    /// Quote character (default `"`).
    pub quote: String,
    /// Trim whitespace around unquoted fields (default `true`).
    pub trim: bool,
    /// Treat lines starting with this character as comments (e.g. `#`).
    pub comment: String,
    /// Line terminator for output: `lf` (default) or `crlf`.
    pub line_ending: String,
    /// Quote style for output: `necessary` (default), `always`, `never`, `non_numeric`.
    pub quote_style: String,
    /// Maximum rows to parse (0 = no limit).
    pub max_rows: usize,
}

impl Default for CsvOptions {
    fn default() -> Self {
        Self {
            delimiter: String::new(),
            has_header: true,
            quote: "\"".into(),
            trim: true,
            comment: String::new(),
            line_ending: "lf".into(),
            quote_style: "necessary".into(),
            max_rows: 0,
        }
    }
}

impl CsvOptions {
    /// Resolves the delimiter byte, sniffing from `sample` when not configured.
    pub fn delimiter_byte(&self, sample: &str) -> u8 {
        match self.delimiter.as_str() {
            "" | "auto" => detect_delimiter(sample),
            "\\t" | "tab" | "TAB" => b'\t',
            s => s.as_bytes()[0],
        }
    }
    fn quote_byte(&self) -> u8 {
        self.quote.as_bytes().first().copied().unwrap_or(b'"')
    }
}

/// Sniffs the delimiter from the first 20 lines, preferring the candidate with
/// the most consistent per-line count.
pub fn detect_delimiter(sample: &str) -> u8 {
    let lines: Vec<&str> = sample.lines().filter(|l| !l.trim().is_empty()).take(20).collect();
    if lines.is_empty() {
        return b',';
    }
    let mut best = (b',', 0usize, usize::MAX);
    for cand in *b",;\t|:" {
        let counts: Vec<usize> = lines.iter().map(|l| count_outside_quotes(l, cand)).collect();
        let total: usize = counts.iter().sum();
        if total == 0 {
            continue;
        }
        let min = counts.iter().copied().min().unwrap_or(0);
        let max = counts.iter().copied().max().unwrap_or(0);
        let variance = max - min;
        // Prefer consistent delimiters; tie-break on total count.
        if variance < best.2 || (variance == best.2 && total > best.1) {
            best = (cand, total, variance);
        }
    }
    best.0
}

fn count_outside_quotes(line: &str, delim: u8) -> usize {
    let mut in_q = false;
    let mut n = 0;
    for b in line.bytes() {
        if b == b'"' {
            in_q = !in_q;
        } else if b == delim && !in_q {
            n += 1;
        }
    }
    n
}

/// Detected structural facts about the raw text.
#[derive(Serialize, Debug, Clone)]
pub struct Sniff {
    /// Delimiter label (`comma`, `tab`, …).
    pub delimiter: String,
    /// `lf`, `crlf` or `cr`.
    pub line_ending: String,
    /// A UTF-8 BOM was present.
    pub has_bom: bool,
    /// Quote character in use.
    pub quote_char: String,
    /// Number of lines.
    pub line_count: usize,
}

/// Sniffs delimiter, line ending, BOM and quoting from raw text.
pub fn sniff(text: &str) -> Sniff {
    let delim = detect_delimiter(text);
    let crlf = text.matches("\r\n").count();
    let lf = text.matches('\n').count() - crlf;
    let cr = text.matches('\r').count() - crlf;
    let line_ending = if crlf > lf && crlf > cr {
        "crlf"
    } else if cr > lf {
        "cr"
    } else if lf == 0 && crlf == 0 && cr == 0 {
        "none"
    } else if crlf > 0 && lf > 0 {
        "mixed"
    } else {
        "lf"
    };
    Sniff {
        delimiter: delimiter_label(delim),
        line_ending: line_ending.into(),
        has_bom: text.starts_with('\u{feff}'),
        quote_char: if text.contains('"') {
            "\"".into()
        } else if text.contains('\'') {
            "'".into()
        } else {
            String::new()
        },
        line_count: text.lines().filter(|l| !l.trim().is_empty()).count(),
    }
}

/// Human-readable name of a delimiter byte.
pub fn delimiter_label(d: u8) -> String {
    match d {
        b'\t' => "\\t".into(),
        other => (other as char).to_string(),
    }
}

/// A problem found while parsing (the row is still kept).
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Issue {
    /// 1-based row number.
    pub row: usize,
    /// Issue kind (`parse_error`, `blank_header`, `duplicate_header`, `field_count`).
    pub kind: String,
    /// Description.
    pub message: String,
}

/// Parsed table: headers plus string cells.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Table {
    /// Column names.
    pub headers: Vec<String>,
    /// Row cells (one string per column).
    pub rows: Vec<Vec<String>>,
    /// Problems found while parsing.
    #[serde(skip)]
    pub issues: Vec<Issue>,
    /// Delimiter that was used.
    #[serde(skip)]
    pub delimiter: u8,
}

/// Inferred type of a column.
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ColumnType {
    /// No non-empty values.
    Empty,
    /// Whole numbers.
    Integer,
    /// Decimal numbers.
    Number,
    /// `true`/`false`, `yes`/`no`.
    Boolean,
    /// ISO-like dates.
    Date,
    /// Anything else.
    Text,
}

impl ColumnType {
    /// Lower-case label.
    pub fn label(self) -> &'static str {
        match self {
            ColumnType::Empty => "empty",
            ColumnType::Integer => "integer",
            ColumnType::Number => "number",
            ColumnType::Boolean => "boolean",
            ColumnType::Date => "date",
            ColumnType::Text => "text",
        }
    }
}

/// Whether a cell represents a missing value (empty, `NULL`, `N/A`, …).
pub fn is_null(cell: &str) -> bool {
    let t = cell.trim();
    t.is_empty()
        || matches!(t.to_ascii_lowercase().as_str(), "null" | "na" | "n/a" | "nan" | "none" | "-")
}

/// Parses a number, tolerating thousands separators and currency symbols.
pub fn parse_number(cell: &str) -> Option<f64> {
    let t = cell.trim().replace(['$', '€', '£', '₹', ','], "");
    let t = t.strip_suffix('%').unwrap_or(&t);
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn is_bool(cell: &str) -> bool {
    matches!(
        cell.trim().to_ascii_lowercase().as_str(),
        "true" | "false" | "yes" | "no" | "y" | "n" | "0" | "1"
    )
}

fn looks_like_date(cell: &str) -> bool {
    let t = cell.trim();
    let b = t.as_bytes();
    if b.len() < 8 || b.len() > 35 {
        return false;
    }
    // ISO 8601 yyyy-mm-dd[Thh:mm...]
    let iso = b.len() >= 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[8..10].iter().all(u8::is_ascii_digit);
    // dd/mm/yyyy or mm/dd/yyyy or dd-mm-yyyy
    let slash = b.len() == 10
        && (b[2] == b'/' || b[2] == b'-' || b[2] == b'.')
        && b[5] == b[2]
        && b[..2].iter().all(u8::is_ascii_digit)
        && b[3..5].iter().all(u8::is_ascii_digit)
        && b[6..].iter().all(u8::is_ascii_digit);
    iso || slash
}

/// Classifies a single cell.
pub fn classify(cell: &str) -> ColumnType {
    if is_null(cell) {
        ColumnType::Empty
    } else if cell.trim().parse::<i64>().is_ok() {
        ColumnType::Integer
    } else if parse_number(cell).is_some() && !cell.contains(|c: char| c.is_alphabetic()) {
        ColumnType::Number
    } else if is_bool(cell) {
        ColumnType::Boolean
    } else if looks_like_date(cell) {
        ColumnType::Date
    } else {
        ColumnType::Text
    }
}

impl Table {
    /// Parses CSV text with the given options.
    pub fn parse(text: &str, opts: &CsvOptions) -> EngineResult<Table> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        engine_shared::error::require_text(text, "csv")?;
        let delim = opts.delimiter_byte(text);
        let mut builder = csv::ReaderBuilder::new();
        builder
            .delimiter(delim)
            .has_headers(false)
            .flexible(true)
            .quote(opts.quote_byte())
            .trim(if opts.trim { csv::Trim::All } else { csv::Trim::None });
        if let Some(c) = opts.comment.as_bytes().first() {
            builder.comment(Some(*c));
        }
        let mut reader = builder.from_reader(text.as_bytes());
        let mut records: Vec<Vec<String>> = Vec::new();
        let mut issues = Vec::new();
        for (i, rec) in reader.records().enumerate() {
            match rec {
                Ok(r) => records.push(r.iter().map(|s| s.to_string()).collect()),
                Err(e) => {
                    issues.push(Issue {
                        row: i + 1,
                        kind: "parse_error".into(),
                        message: e.to_string(),
                    });
                    if issues.len() > 1000 {
                        return Err(EngineError::malformed(
                            "Too many malformed rows; the file does not look like CSV",
                        ));
                    }
                },
            }
            if opts.max_rows > 0 && records.len() > opts.max_rows {
                break;
            }
        }
        if records.is_empty() {
            return Err(EngineError::empty("csv"));
        }
        let (headers, mut rows) = if opts.has_header {
            let h = records.remove(0);
            (h, records)
        } else {
            let width = records.iter().map(Vec::len).max().unwrap_or(0);
            ((1..=width).map(|i| format!("column_{i}")).collect(), records)
        };
        let mut headers = headers;
        // Normalise blank / duplicate headers so that column addressing is unambiguous.
        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        for (i, h) in headers.iter_mut().enumerate() {
            if h.trim().is_empty() {
                *h = format!("column_{}", i + 1);
                issues.push(Issue {
                    row: 0,
                    kind: "blank_header".into(),
                    message: format!("Column {} has no name; using `{h}`", i + 1),
                });
            }
            let n = seen.entry(h.clone()).or_insert(0);
            *n += 1;
            if *n > 1 {
                let new = format!("{h}_{n}");
                issues.push(Issue {
                    row: 0,
                    kind: "duplicate_header".into(),
                    message: format!("Duplicate column name `{h}` renamed to `{new}`"),
                });
                *h = new;
            }
        }
        let width = headers.len();
        let mut keep = Vec::with_capacity(rows.len());
        for (i, r) in rows.iter_mut().enumerate() {
            if r.len() == width {
                keep.push(true);
                continue;
            }
            // Blank lines (a single empty field) are dropped silently.
            if r.len() <= 1 && r.iter().all(|c| c.trim().is_empty()) {
                keep.push(false);
                continue;
            }
            issues.push(Issue {
                row: i + 1,
                kind: "field_count".into(),
                message: format!("Row {} has {} fields, expected {}", i + 1, r.len(), width),
            });
            r.resize(width, String::new());
            keep.push(true);
        }
        let mut k = keep.into_iter();
        rows.retain(|_| k.next().unwrap_or(true));
        Ok(Table { headers, rows, issues, delimiter: delim })
    }

    /// Builds a table from JSON row arrays.
    pub fn from_json_rows(headers: Vec<String>, rows: Vec<Vec<Value>>) -> Table {
        let rows = rows
            .into_iter()
            .map(|r| r.into_iter().map(value_to_cell).collect())
            .collect();
        Table {
            headers,
            rows,
            issues: vec![],
            delimiter: b',',
        }
    }

    /// Accepts an array of objects (keys become columns, union in first-seen order).
    pub fn from_objects(objects: &[Value]) -> EngineResult<Table> {
        if objects.is_empty() {
            return Err(EngineError::empty("rows"));
        }
        let mut headers: Vec<String> = Vec::new();
        for o in objects {
            if let Some(m) = o.as_object() {
                for k in m.keys() {
                    if !headers.contains(k) {
                        headers.push(k.clone());
                    }
                }
            } else {
                return Err(EngineError::invalid("Every row must be a JSON object"));
            }
        }
        let rows = objects
            .iter()
            .map(|o| {
                headers
                    .iter()
                    .map(|h| value_to_cell(o.get(h).cloned().unwrap_or(Value::Null)))
                    .collect()
            })
            .collect();
        Ok(Table {
            headers,
            rows,
            issues: vec![],
            delimiter: b',',
        })
    }

    /// Number of columns.
    pub fn width(&self) -> usize {
        self.headers.len()
    }

    /// Resolves a column reference: name (case-insensitive fallback), 1-based
    /// index string, or spreadsheet letter (`A`, `AB`).
    pub fn column_index(&self, reference: &str) -> EngineResult<usize> {
        let r = reference.trim();
        if let Some(i) = self.headers.iter().position(|h| h == r) {
            return Ok(i);
        }
        if let Some(i) = self.headers.iter().position(|h| h.eq_ignore_ascii_case(r)) {
            return Ok(i);
        }
        if let Ok(n) = r.parse::<usize>() {
            if n >= 1 && n <= self.width() {
                return Ok(n - 1);
            }
            return Err(EngineError::not_found(format!(
                "Column index {n} is out of range (1..={})",
                self.width()
            )));
        }
        if !r.is_empty() && r.len() <= 3 && r.bytes().all(|b| b.is_ascii_alphabetic()) {
            let idx = r
                .bytes()
                .fold(0usize, |acc, b| acc * 26 + (b.to_ascii_uppercase() - b'A' + 1) as usize);
            if idx >= 1 && idx <= self.width() {
                return Ok(idx - 1);
            }
        }
        Err(EngineError::not_found(format!(
            "Column `{reference}` not found (available: {})",
            self.headers.join(", ")
        )))
    }

    /// Resolves column names/indices to indices.
    pub fn column_indices(&self, refs: &[String]) -> EngineResult<Vec<usize>> {
        refs.iter().map(|r| self.column_index(r)).collect()
    }

    /// Infers the type of a column.
    pub fn column_type(&self, col: usize) -> ColumnType {
        let mut seen_int = false;
        let mut seen_num = false;
        let mut seen_bool = false;
        let mut seen_date = false;
        let mut non_empty = 0usize;
        for r in &self.rows {
            let c = r.get(col).map(String::as_str).unwrap_or("");
            match classify(c) {
                ColumnType::Empty => continue,
                ColumnType::Integer => seen_int = true,
                ColumnType::Number => seen_num = true,
                ColumnType::Boolean => seen_bool = true,
                ColumnType::Date => seen_date = true,
                ColumnType::Text => return ColumnType::Text,
            }
            non_empty += 1;
        }
        if non_empty == 0 {
            ColumnType::Empty
        } else if seen_date && !seen_int && !seen_num && !seen_bool {
            ColumnType::Date
        } else if seen_date {
            ColumnType::Text
        } else if seen_bool && !seen_int && !seen_num {
            ColumnType::Boolean
        } else if seen_num {
            ColumnType::Number
        } else if seen_int {
            ColumnType::Integer
        } else {
            ColumnType::Text
        }
    }

    /// Infers every column type.
    pub fn column_types(&self) -> Vec<ColumnType> {
        (0..self.width()).map(|c| self.column_type(c)).collect()
    }

    /// Serialises the table as CSV text.
    pub fn to_csv(&self, opts: &CsvOptions) -> EngineResult<String> {
        let delim = match opts.delimiter.as_str() {
            "" | "auto" => self.delimiter,
            _ => opts.delimiter_byte(""),
        };
        let style = match opts.quote_style.as_str() {
            "always" => csv::QuoteStyle::Always,
            "never" => csv::QuoteStyle::Never,
            "non_numeric" => csv::QuoteStyle::NonNumeric,
            _ => csv::QuoteStyle::Necessary,
        };
        let term = if opts.line_ending.eq_ignore_ascii_case("crlf") {
            csv::Terminator::CRLF
        } else {
            csv::Terminator::Any(b'\n')
        };
        let mut w = csv::WriterBuilder::new()
            .delimiter(delim)
            .quote(opts.quote_byte())
            .quote_style(style)
            .terminator(term)
            .has_headers(false)
            .from_writer(Vec::new());
        if opts.has_header {
            w.write_record(&self.headers)
                .map_err(|e| EngineError::internal(e.to_string()))?;
        }
        for r in &self.rows {
            w.write_record(r).map_err(|e| EngineError::internal(e.to_string()))?;
        }
        let bytes = w.into_inner().map_err(|e| EngineError::internal(e.to_string()))?;
        String::from_utf8(bytes).map_err(|e| EngineError::internal(e.to_string()))
    }

    /// Rows as JSON arrays (paginated).
    pub fn to_json_rows(&self, offset: usize, limit: usize) -> Vec<Value> {
        self.rows
            .iter()
            .skip(offset)
            .take(if limit == 0 { usize::MAX } else { limit })
            .map(|r| Value::Array(r.iter().map(|c| Value::String(c.clone())).collect()))
            .collect()
    }

    /// Rows as JSON objects keyed by header.
    pub fn to_objects(&self, typed: bool) -> Vec<Value> {
        let types = if typed { self.column_types() } else { vec![] };
        self.rows
            .iter()
            .map(|r| {
                let mut m = serde_json::Map::new();
                for (i, h) in self.headers.iter().enumerate() {
                    let c = r.get(i).map(String::as_str).unwrap_or("");
                    m.insert(
                        h.clone(),
                        if typed {
                            cell_to_value(c, types[i])
                        } else {
                            Value::String(c.to_string())
                        },
                    );
                }
                Value::Object(m)
            })
            .collect()
    }

    /// Parsing issues as JSON.
    pub fn issues_json(&self) -> Value {
        json!(self.issues)
    }
}

/// Converts a JSON value to a cell string.
pub fn value_to_cell(v: Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s,
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

/// Converts a cell to a typed JSON value according to the inferred column type.
pub fn cell_to_value(cell: &str, ty: ColumnType) -> Value {
    let t = cell.trim();
    if t.is_empty() {
        return Value::Null;
    }
    match ty {
        ColumnType::Integer => t
            .parse::<i64>()
            .map(Value::from)
            .unwrap_or_else(|_| Value::String(cell.into())),
        ColumnType::Number => parse_number(t)
            .map(|n| json!(n))
            .unwrap_or_else(|| Value::String(cell.into())),
        ColumnType::Boolean => match t.to_ascii_lowercase().as_str() {
            "true" | "yes" | "y" | "1" => Value::Bool(true),
            "false" | "no" | "n" | "0" => Value::Bool(false),
            _ => Value::String(cell.into()),
        },
        _ => Value::String(cell.into()),
    }
}

/// Natural ("human") ordering: digit runs compare numerically.
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let (ab, bb) = (a.as_bytes(), b.as_bytes());
    let (mut i, mut j) = (0, 0);
    while i < ab.len() && j < bb.len() {
        if ab[i].is_ascii_digit() && bb[j].is_ascii_digit() {
            let si = i;
            while i < ab.len() && ab[i].is_ascii_digit() {
                i += 1;
            }
            let sj = j;
            while j < bb.len() && bb[j].is_ascii_digit() {
                j += 1;
            }
            let na = a[si..i].trim_start_matches('0');
            let nb = b[sj..j].trim_start_matches('0');
            let ord = na.len().cmp(&nb.len()).then_with(|| na.cmp(nb));
            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
        } else {
            let ca = ab[i].to_ascii_lowercase();
            let cb = bb[j].to_ascii_lowercase();
            if ca != cb {
                return ca.cmp(&cb);
            }
            i += 1;
            j += 1;
        }
    }
    (ab.len() - i).cmp(&(bb.len() - j))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_detects() {
        let t = Table::parse("a;b;c\n1;2;3\n4;5\n", &CsvOptions::default()).unwrap();
        assert_eq!(t.delimiter, b';');
        assert_eq!(t.headers, vec!["a", "b", "c"]);
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[1], vec!["4", "5", ""]);
        assert_eq!(t.issues.len(), 1);
        assert_eq!(t.column_type(0), ColumnType::Integer);
        assert!(Table::parse("", &CsvOptions::default()).is_err());
        assert!(Table::parse("\u{feff}a,b\n", &CsvOptions::default())
            .unwrap()
            .rows
            .is_empty());
    }

    #[test]
    fn header_normalisation_and_columns() {
        let t = Table::parse("id,,id\n1,2,3\n", &CsvOptions::default()).unwrap();
        assert_eq!(t.headers, vec!["id", "column_2", "id_2"]);
        assert_eq!(t.column_index("B").unwrap(), 1);
        assert_eq!(t.column_index("3").unwrap(), 2);
        assert!(t.column_index("nope").is_err());
    }

    #[test]
    fn types_and_natural_order() {
        assert_eq!(classify("2024-01-05"), ColumnType::Date);
        assert_eq!(classify("$1,200.50"), ColumnType::Number);
        assert_eq!(classify("yes"), ColumnType::Boolean);
        assert_eq!(classify("N/A"), ColumnType::Empty);
        assert_eq!(natural_cmp("file2", "file10"), std::cmp::Ordering::Less);
        assert_eq!(natural_cmp("a", "B"), std::cmp::Ordering::Less);
    }

    #[test]
    fn roundtrip() {
        let t = Table::parse("a,b\n\"x, y\",2\n", &CsvOptions::default()).unwrap();
        let out = t
            .to_csv(&CsvOptions {
                delimiter: "\\t".into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(out, "a\tb\nx, y\t2\n");
        let s = sniff("a,b\r\n1,2\r\n");
        assert_eq!(s.line_ending, "crlf");
        assert_eq!(s.delimiter, ",");
    }
}
