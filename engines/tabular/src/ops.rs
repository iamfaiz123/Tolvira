//! Table transformations.

use crate::table::{
    cell_to_value, classify, is_null, natural_cmp, parse_number, ColumnType, Table,
};
use engine_shared::{EngineError, EngineResult};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};

fn r4(v: f64) -> f64 {
    (v * 10_000.0).round() / 10_000.0
}

// ----------------------------------------------------------------- clean --

/// Switches for the `clean` action; all default to off.
#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct CleanOptions {
    /// Trim whitespace around cells.
    pub trim: bool,
    /// Collapse runs of whitespace inside cells.
    pub collapse_whitespace: bool,
    /// Drop rows whose cells are all empty.
    pub remove_empty_rows: bool,
    /// Drop columns whose cells are all empty.
    pub remove_empty_columns: bool,
    /// Drop exact duplicate rows.
    pub remove_duplicate_rows: bool,
    /// `snake_case`, `lower`, `upper`, `title`, `keep` (default `keep`).
    pub header_case: String,
    /// Replace `NULL`, `N/A`, `-`, ... with this (None keeps values as-is).
    pub standardize_nulls: Option<String>,
    /// Value written into empty cells.
    pub fill_empty: Option<String>,
    /// Columns to drop.
    pub remove_columns: Vec<String>,
    /// Strip currency symbols / thousands separators from numeric-looking cells.
    pub normalize_numbers: bool,
    /// Remove control / non-printable characters.
    pub strip_non_printable: bool,
}

impl Default for CleanOptions {
    fn default() -> Self {
        Self {
            trim: true,
            collapse_whitespace: false,
            remove_empty_rows: true,
            remove_empty_columns: false,
            remove_duplicate_rows: false,
            header_case: "keep".into(),
            standardize_nulls: None,
            fill_empty: None,
            remove_columns: vec![],
            normalize_numbers: false,
            strip_non_printable: true,
        }
    }
}

/// Applies [`CleanOptions`] and reports what changed.
pub fn clean(mut t: Table, o: &CleanOptions) -> EngineResult<(Table, Value)> {
    let mut report = Map::new();
    let before_rows = t.rows.len();
    if !o.remove_columns.is_empty() {
        let idx: HashSet<usize> = t.column_indices(&o.remove_columns)?.into_iter().collect();
        t = keep_columns(&t, &(0..t.width()).filter(|i| !idx.contains(i)).collect::<Vec<_>>());
        report.insert("columns_removed".into(), json!(idx.len()));
    }
    let mut cells_changed = 0usize;
    for row in t.rows.iter_mut() {
        for c in row.iter_mut() {
            let orig = c.clone();
            if o.strip_non_printable {
                *c = c
                    .chars()
                    .filter(|ch| !ch.is_control() || *ch == '\n' || *ch == '\t')
                    .collect();
            }
            if o.trim {
                *c = c.trim().to_string();
            }
            if o.collapse_whitespace {
                *c = c.split_whitespace().collect::<Vec<_>>().join(" ");
            }
            if let Some(n) = &o.standardize_nulls {
                if is_null(c) {
                    *c = n.clone();
                }
            }
            if o.normalize_numbers && matches!(classify(c), ColumnType::Number) {
                if let Some(v) = parse_number(c) {
                    *c = fmt_num(v);
                }
            }
            if let Some(f) = &o.fill_empty {
                if c.is_empty() {
                    *c = f.clone();
                }
            }
            if *c != orig {
                cells_changed += 1;
            }
        }
    }
    if o.remove_empty_rows {
        t.rows.retain(|r| r.iter().any(|c| !c.trim().is_empty()));
    }
    if o.remove_empty_columns {
        let keep: Vec<usize> = (0..t.width())
            .filter(|i| t.rows.iter().any(|r| !r[*i].trim().is_empty()))
            .collect();
        let removed = t.width() - keep.len();
        if removed > 0 {
            t = keep_columns(&t, &keep);
            report.insert("empty_columns_removed".into(), json!(removed));
        }
    }
    if o.remove_duplicate_rows {
        let (nt, removed) = dedupe(t, &[], false, "first")?;
        t = nt;
        report.insert("duplicate_rows_removed".into(), json!(removed.len()));
    }
    let mut headers_changed = 0;
    for h in t.headers.iter_mut() {
        let new = match o.header_case.as_str() {
            "snake_case" | "snake" => to_snake(h),
            "lower" => h.to_lowercase(),
            "upper" => h.to_uppercase(),
            "title" => h
                .split_whitespace()
                .map(|w| {
                    let mut c = w.chars();
                    c.next()
                        .map(|f| f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase())
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
                .join(" "),
            _ => h.trim().to_string(),
        };
        if new != *h {
            headers_changed += 1;
            *h = new;
        }
    }
    report.insert("rows_before".into(), json!(before_rows));
    report.insert("rows_after".into(), json!(t.rows.len()));
    report.insert("rows_removed".into(), json!(before_rows - t.rows.len()));
    report.insert("cells_changed".into(), json!(cells_changed));
    report.insert("headers_changed".into(), json!(headers_changed));
    Ok((t, Value::Object(report)))
}

fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v}");
        s
    }
}

/// Converts a header to `snake_case`.
pub fn to_snake(s: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for ch in s.trim().chars() {
        if ch.is_alphanumeric() {
            if ch.is_uppercase() && prev_lower {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
            prev_lower = ch.is_lowercase() || ch.is_ascii_digit();
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
            prev_lower = false;
        }
    }
    out.trim_end_matches('_').to_string()
}

/// Returns a table with only the given column indices.
pub fn keep_columns(t: &Table, idx: &[usize]) -> Table {
    Table {
        headers: idx.iter().map(|i| t.headers[*i].clone()).collect(),
        rows: t
            .rows
            .iter()
            .map(|r| idx.iter().map(|i| r.get(*i).cloned().unwrap_or_default()).collect())
            .collect(),
        issues: t.issues.clone(),
        delimiter: t.delimiter,
    }
}

// -------------------------------------------------------------- validate --

/// Validation rule for one column.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct ColumnRule {
    /// Column name.
    pub name: String,
    /// `integer`, `number`, `boolean`, `date`, `text`, `email`, `url`, `any`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Cell must not be empty.
    pub required: bool,
    /// Values must be unique.
    pub unique: bool,
    /// Regular-expression-like pattern the value must match.
    pub pattern: Option<String>,
    /// Minimum numeric value.
    pub min: Option<f64>,
    /// Maximum numeric value.
    pub max: Option<f64>,
    /// Minimum string length.
    pub min_length: Option<usize>,
    /// Maximum string length.
    pub max_length: Option<usize>,
    /// Allowed values.
    pub allowed: Vec<String>,
}

/// Schema for the `validate` action.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Schema {
    /// Column rules.
    pub columns: Vec<ColumnRule>,
    /// Reject columns that are not declared in the schema.
    pub strict_columns: bool,
    /// Require declared column order.
    pub ordered: bool,
}

/// Validates structure (ragged rows, duplicates headers, types) and schema rules.
pub fn validate(t: &Table, schema: Option<&Schema>, max_errors: usize) -> Value {
    let mut errors: Vec<Value> = Vec::new();
    let mut warnings: Vec<Value> = Vec::new();
    let max_errors = if max_errors == 0 { 500 } else { max_errors };
    for i in &t.issues {
        let target = if i.kind == "field_count" || i.kind == "parse_error" {
            &mut errors
        } else {
            &mut warnings
        };
        target.push(
            json!({ "row": i.row, "column": Value::Null, "code": i.kind, "message": i.message }),
        );
    }
    // Structural checks.
    let types = t.column_types();
    for (ci, h) in t.headers.iter().enumerate() {
        if types[ci] == ColumnType::Empty && !t.rows.is_empty() {
            warnings.push(json!({
                "row": 0,
                "column": h,
                "code": "empty_column",
                "message": format!("Column `{h}` has no values")
            }));
        }
        let empties = t.rows.iter().filter(|r| r[ci].trim().is_empty()).count();
        if empties > 0 && empties < t.rows.len() {
            warnings.push(json!({
                "row": 0,
                "column": h,
                "code": "missing_values",
                "message": format!("Column `{h}` has {empties} empty cell(s)")
            }));
        }
        // Mixed types: majority numeric but some text.
        if types[ci] == ColumnType::Text {
            let numeric = t
                .rows
                .iter()
                .filter(|r| matches!(classify(&r[ci]), ColumnType::Integer | ColumnType::Number))
                .count();
            let filled = t.rows.iter().filter(|r| !is_null(&r[ci])).count();
            if filled > 2 && numeric * 10 >= filled * 8 && numeric < filled {
                warnings.push(json!({
                    "row": 0,
                    "column": h,
                    "code": "mixed_types",
                    "message": format!("Column `{h}` is mostly numeric but contains {} \
                                        non-numeric value(s)", filled - numeric)
                }));
            }
        }
    }
    if let Some(s) = schema {
        let declared: HashSet<&str> = s.columns.iter().map(|c| c.name.as_str()).collect();
        if s.strict_columns {
            for h in &t.headers {
                if !declared.contains(h.as_str()) {
                    errors.push(json!({
                        "row": 0,
                        "column": h,
                        "code": "undeclared_column",
                        "message": format!("Column `{h}` is not declared in the schema")
                    }));
                }
            }
        }
        if s.ordered {
            let actual: Vec<&str> = t
                .headers
                .iter()
                .map(String::as_str)
                .filter(|h| declared.contains(h))
                .collect();
            let expected: Vec<&str> = s
                .columns
                .iter()
                .map(|c| c.name.as_str())
                .filter(|n| t.headers.iter().any(|h| h == n))
                .collect();
            if actual != expected {
                errors.push(json!({
                    "row": 0,
                    "column": Value::Null,
                    "code": "column_order",
                    "message": format!("Expected column order {:?}, found {:?}", expected, actual)
                }));
            }
        }
        for rule in &s.columns {
            let Some(ci) = t.headers.iter().position(|h| h == &rule.name) else {
                if rule.required {
                    errors.push(json!({
                        "row": 0,
                        "column": rule.name,
                        "code": "missing_column",
                        "message": format!("Required column `{}` is missing", rule.name)
                    }));
                }
                continue;
            };
            let re = match rule.pattern.as_deref().filter(|p| !p.is_empty()) {
                Some(p) => match SimplePattern::compile(p) {
                    Ok(r) => Some(r),
                    Err(e) => {
                        errors.push(json!({
                            "row": 0,
                            "column": rule.name,
                            "code": "bad_pattern",
                            "message": e
                        }));
                        None
                    },
                },
                None => None,
            };
            let mut seen: HashMap<&str, usize> = HashMap::new();
            for (ri, row) in t.rows.iter().enumerate() {
                if errors.len() >= max_errors {
                    break;
                }
                let cell = row[ci].as_str();
                let rown = ri + 1;
                let push = |errors: &mut Vec<Value>, code: &str, msg: String| {
                    errors.push(json!({
                        "row": rown,
                        "column": rule.name,
                        "code": code,
                        "message": msg,
                        "value": cell
                    }))
                };
                if is_null(cell) {
                    if rule.required {
                        push(
                            &mut errors,
                            "required",
                            format!("Row {rown}: `{}` is required", rule.name),
                        );
                    }
                    continue;
                }
                let ok_type = match rule.kind.as_str() {
                    "" | "any" | "text" | "string" => true,
                    "integer" | "int" => cell.trim().parse::<i64>().is_ok(),
                    "number" | "float" | "decimal" => parse_number(cell).is_some(),
                    "boolean" | "bool" => classify(cell) == ColumnType::Boolean,
                    "date" => classify(cell) == ColumnType::Date,
                    "email" => is_email(cell),
                    "url" => cell.starts_with("http://") || cell.starts_with("https://"),
                    other => {
                        push(
                            &mut errors,
                            "unknown_type",
                            format!("Unknown type `{other}` in schema"),
                        );
                        true
                    },
                };
                if !ok_type {
                    push(
                        &mut errors,
                        "type",
                        format!("Row {rown}: `{}` should be {}", rule.name, rule.kind),
                    );
                    continue;
                }
                if let (Some(n), true) =
                    (parse_number(cell), rule.min.is_some() || rule.max.is_some())
                {
                    if rule.min.is_some_and(|m| n < m) || rule.max.is_some_and(|m| n > m) {
                        push(
                            &mut errors,
                            "range",
                            format!(
                                "Row {rown}: `{}` = {n} is outside [{}, {}]",
                                rule.name,
                                rule.min.map(|v| v.to_string()).unwrap_or("-∞".into()),
                                rule.max.map(|v| v.to_string()).unwrap_or("∞".into())
                            ),
                        );
                    }
                }
                let len = cell.chars().count();
                if rule.min_length.is_some_and(|m| len < m)
                    || rule.max_length.is_some_and(|m| len > m)
                {
                    push(
                        &mut errors,
                        "length",
                        format!(
                            "Row {rown}: `{}` length {len} is outside the allowed range",
                            rule.name
                        ),
                    );
                }
                if !rule.allowed.is_empty() && !rule.allowed.iter().any(|a| a == cell.trim()) {
                    push(
                        &mut errors,
                        "allowed",
                        format!(
                            "Row {rown}: `{}` value `{cell}` is not one of the allowed values",
                            rule.name
                        ),
                    );
                }
                if let Some(re) = &re {
                    if !re.is_match(cell) {
                        push(
                            &mut errors,
                            "pattern",
                            format!("Row {rown}: `{}` does not match the pattern", rule.name),
                        );
                    }
                }
                if rule.unique {
                    if let Some(first) = seen.get(cell) {
                        push(
                            &mut errors,
                            "unique",
                            format!(
                                "Row {rown}: duplicate `{}` value `{cell}` (first seen in row \
                                 {first})",
                                rule.name
                            ),
                        );
                    } else {
                        seen.insert(cell, rown);
                    }
                }
            }
        }
    }
    let truncated = errors.len() >= max_errors;
    json!({
        "valid": errors.is_empty(),
        "rows": t.rows.len(),
        "columns": t.width(),
        "headers": t.headers,
        "column_types": types.iter().map(|t| t.label()).collect::<Vec<_>>(),
        "error_count": errors.len(),
        "warning_count": warnings.len(),
        "errors": errors,
        "warnings": warnings,
        "truncated": truncated,
    })
}

fn is_email(s: &str) -> bool {
    let s = s.trim();
    let Some((local, domain)) = s.rsplit_once('@') else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !s.contains(char::is_whitespace)
}

/// Minimal glob/regex-lite matcher: supports `^`, `$`, `.`, `*` (any run),
/// `\d`, `\w`, `\s`, character classes `[a-z]` and `+`/`?` on single atoms.
/// Keeps the engine free of a full regex dependency while covering common
/// validation patterns (phone numbers, codes, IDs).
pub struct SimplePattern {
    atoms: Vec<(Atom, Quant)>,
    anchored_start: bool,
    anchored_end: bool,
}

#[derive(Clone)]
enum Atom {
    Lit(char),
    Any,
    Digit,
    Word,
    Space,
    Class(Vec<(char, char)>, bool),
}

#[derive(Clone, Copy, PartialEq)]
enum Quant {
    One,
    Optional,
    Star,
    Plus,
}

impl SimplePattern {
    /// Compiles a simplified pattern (`^`, `$`, `.`, `*`, `+`, `?`, classes, escapes).
    pub fn compile(p: &str) -> Result<Self, String> {
        let mut chars = p.chars().peekable();
        let mut anchored_start = false;
        let mut anchored_end = false;
        if chars.peek() == Some(&'^') {
            anchored_start = true;
            chars.next();
        }
        let mut atoms = Vec::new();
        while let Some(c) = chars.next() {
            let atom = match c {
                '$' if chars.peek().is_none() => {
                    anchored_end = true;
                    break;
                },
                '.' => Atom::Any,
                '\\' => match chars.next() {
                    Some('d') => Atom::Digit,
                    Some('w') => Atom::Word,
                    Some('s') => Atom::Space,
                    Some(other) => Atom::Lit(other),
                    None => return Err("Pattern ends with a backslash".into()),
                },
                '[' => {
                    let mut negate = false;
                    if chars.peek() == Some(&'^') {
                        negate = true;
                        chars.next();
                    }
                    let mut ranges = Vec::new();
                    loop {
                        let Some(a) = chars.next() else {
                            return Err("Unterminated character class".into());
                        };
                        if a == ']' {
                            break;
                        }
                        let a = if a == '\\' {
                            chars.next().ok_or("Bad escape")?
                        } else {
                            a
                        };
                        if chars.peek() == Some(&'-') {
                            chars.next();
                            let b = chars.next().ok_or("Unterminated range")?;
                            ranges.push((a, b));
                        } else {
                            ranges.push((a, a));
                        }
                    }
                    Atom::Class(ranges, negate)
                },
                '(' | ')' | '|' | '{' | '}' => {
                    return Err(format!(
                        "Unsupported pattern construct `{c}`; use ^ $ . * + ? \\d \\w \\s and \
                         [...] classes"
                    ))
                },
                other => Atom::Lit(other),
            };
            let quant = match chars.peek() {
                Some('*') => {
                    chars.next();
                    Quant::Star
                },
                Some('+') => {
                    chars.next();
                    Quant::Plus
                },
                Some('?') => {
                    chars.next();
                    Quant::Optional
                },
                _ => Quant::One,
            };
            atoms.push((atom, quant));
        }
        Ok(Self { atoms, anchored_start, anchored_end })
    }

    fn atom_matches(a: &Atom, c: char) -> bool {
        match a {
            Atom::Lit(l) => *l == c,
            Atom::Any => true,
            Atom::Digit => c.is_ascii_digit(),
            Atom::Word => c.is_alphanumeric() || c == '_',
            Atom::Space => c.is_whitespace(),
            Atom::Class(ranges, neg) => ranges.iter().any(|(lo, hi)| c >= *lo && c <= *hi) != *neg,
        }
    }

    fn match_here(&self, ai: usize, text: &[char], ti: usize) -> bool {
        if ai == self.atoms.len() {
            return !self.anchored_end || ti == text.len();
        }
        let (atom, q) = &self.atoms[ai];
        match q {
            Quant::One => {
                ti < text.len()
                    && Self::atom_matches(atom, text[ti])
                    && self.match_here(ai + 1, text, ti + 1)
            },
            Quant::Optional => {
                (ti < text.len()
                    && Self::atom_matches(atom, text[ti])
                    && self.match_here(ai + 1, text, ti + 1))
                    || self.match_here(ai + 1, text, ti)
            },
            Quant::Star | Quant::Plus => {
                let mut k = ti;
                if *q == Quant::Plus {
                    if k < text.len() && Self::atom_matches(atom, text[k]) {
                        k += 1;
                    } else {
                        return false;
                    }
                }
                let mut end = k;
                while end < text.len() && Self::atom_matches(atom, text[end]) {
                    end += 1;
                }
                // Greedy with backtracking.
                let mut pos = end;
                loop {
                    if self.match_here(ai + 1, text, pos) {
                        return true;
                    }
                    if pos == k {
                        return false;
                    }
                    pos -= 1;
                }
            },
        }
    }

    /// Tests a string against the pattern.
    pub fn is_match(&self, s: &str) -> bool {
        let text: Vec<char> = s.chars().collect();
        if self.anchored_start {
            return self.match_here(0, &text, 0);
        }
        (0..=text.len()).any(|start| self.match_here(0, &text, start))
    }
}

// ------------------------------------------------------------------ merge --

/// Appends several tables (same headers or header union).
pub fn append(
    tables: &[Table],
    union_headers: bool,
    source_column: Option<&str>,
    names: &[String],
) -> EngineResult<Table> {
    let first = tables.first().ok_or_else(|| EngineError::empty("files"))?;
    let mut headers = first.headers.clone();
    if union_headers {
        for t in &tables[1..] {
            for h in &t.headers {
                if !headers.contains(h) {
                    headers.push(h.clone());
                }
            }
        }
    } else {
        for (i, t) in tables.iter().enumerate().skip(1) {
            if t.width() != first.width() {
                return Err(EngineError::invalid(format!(
                    "File {} has {} columns but the first file has {} (enable union_headers to \
                     merge by name)",
                    i + 1,
                    t.width(),
                    first.width()
                )));
            }
        }
    }
    let mut rows = Vec::new();
    for (i, t) in tables.iter().enumerate() {
        let map: Vec<Option<usize>> = if union_headers {
            headers
                .iter()
                .map(|h| t.headers.iter().position(|x| x == h))
                .collect()
        } else {
            (0..headers.len()).map(Some).collect()
        };
        for r in &t.rows {
            let mut out: Vec<String> = map
                .iter()
                .map(|m| m.and_then(|j| r.get(j).cloned()).unwrap_or_default())
                .collect();
            if source_column.is_some() {
                out.push(names.get(i).cloned().unwrap_or_else(|| format!("file_{}", i + 1)));
            }
            rows.push(out);
        }
    }
    if let Some(sc) = source_column {
        headers.push(sc.to_string());
    }
    Ok(Table {
        headers,
        rows,
        issues: vec![],
        delimiter: first.delimiter,
    })
}

/// Joins two tables on key columns.
pub fn join(
    left: &Table,
    right: &Table,
    left_key: &str,
    right_key: &str,
    kind: &str,
) -> EngineResult<Table> {
    let lk = left.column_index(left_key)?;
    let rk = right.column_index(right_key)?;
    let mut index: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, r) in right.rows.iter().enumerate() {
        index.entry(r[rk].as_str()).or_default().push(i);
    }
    let mut headers = left.headers.clone();
    let right_cols: Vec<usize> = (0..right.width()).filter(|c| *c != rk).collect();
    for c in &right_cols {
        let h = &right.headers[*c];
        headers.push(if headers.contains(h) {
            format!("{h}_right")
        } else {
            h.clone()
        });
    }
    let empty_right: Vec<String> = vec![String::new(); right_cols.len()];
    let mut rows = Vec::new();
    let mut matched_right: HashSet<usize> = HashSet::new();
    for l in &left.rows {
        match index.get(l[lk].as_str()) {
            Some(matches) => {
                for ri in matches {
                    matched_right.insert(*ri);
                    let mut row = l.clone();
                    row.extend(right_cols.iter().map(|c| right.rows[*ri][*c].clone()));
                    rows.push(row);
                }
            },
            None => {
                if matches!(kind, "left" | "outer" | "full") {
                    let mut row = l.clone();
                    row.extend(empty_right.iter().cloned());
                    rows.push(row);
                }
            },
        }
    }
    if matches!(kind, "right" | "outer" | "full") {
        for (ri, r) in right.rows.iter().enumerate() {
            if !matched_right.contains(&ri) {
                let mut row = vec![String::new(); left.width()];
                row[lk] = r[rk].clone();
                row.extend(right_cols.iter().map(|c| r[*c].clone()));
                rows.push(row);
            }
        }
    }
    if !matches!(kind, "inner" | "left" | "right" | "outer" | "full") {
        return Err(EngineError::invalid(format!(
            "Unknown join type `{kind}` (inner, left, right, outer)"
        )));
    }
    Ok(Table {
        headers,
        rows,
        issues: vec![],
        delimiter: left.delimiter,
    })
}

// ----------------------------------------------------------------- dedupe --

/// Removes duplicate rows by all or selected columns.
pub fn dedupe(
    mut t: Table,
    columns: &[String],
    case_insensitive: bool,
    keep: &str,
) -> EngineResult<(Table, Vec<Value>)> {
    let idx: Vec<usize> = if columns.is_empty() {
        (0..t.width()).collect()
    } else {
        t.column_indices(columns)?
    };
    let key = |r: &Vec<String>| -> String {
        idx.iter()
            .map(|i| {
                let c = r[*i].trim();
                if case_insensitive {
                    c.to_lowercase()
                } else {
                    c.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\u{1f}")
    };
    let mut first_seen: HashMap<String, usize> = HashMap::new();
    let mut keep_flags = vec![true; t.rows.len()];
    let mut removed = Vec::new();
    if keep == "last" {
        for (i, r) in t.rows.iter().enumerate().rev() {
            let k = key(r);
            if let Some(prev) = first_seen.get(&k) {
                keep_flags[i] = false;
                removed.push(json!({ "row": i + 1, "duplicate_of": prev + 1 }));
            } else {
                first_seen.insert(k, i);
            }
        }
        removed.reverse();
    } else {
        for (i, r) in t.rows.iter().enumerate() {
            let k = key(r);
            if let Some(prev) = first_seen.get(&k) {
                keep_flags[i] = false;
                removed.push(json!({ "row": i + 1, "duplicate_of": prev + 1 }));
            } else {
                first_seen.insert(k, i);
            }
        }
    }
    let mut flags = keep_flags.into_iter();
    t.rows.retain(|_| flags.next().unwrap_or(true));
    Ok((t, removed))
}

// ----------------------------------------------------------------- filter --

/// A filter condition.
#[derive(Deserialize, Debug, Clone)]
pub struct Condition {
    /// Column name or index.
    pub column: String,
    /// `eq`, `neq`, `contains`, `not_contains`, `starts_with`, `ends_with`, `gt`, `gte`, `lt`,
    /// `lte`,
    /// `between`, `empty`, `not_empty`, `in`, `not_in`, `matches`.
    pub op: String,
    /// Comparison value.
    #[serde(default)]
    pub value: Value,
    /// Second value for `between`.
    #[serde(default)]
    pub value2: Value,
    /// Ignore case when comparing text.
    #[serde(default)]
    pub case_insensitive: bool,
}

fn val_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Keeps rows matching the conditions.
pub fn filter(
    t: &Table,
    conditions: &[Condition],
    match_all: bool,
    invert: bool,
) -> EngineResult<Table> {
    if conditions.is_empty() {
        return Err(EngineError::empty("conditions"));
    }
    struct Compiled {
        col: usize,
        op: String,
        value: String,
        value2: String,
        num: Option<f64>,
        num2: Option<f64>,
        list: Vec<String>,
        ci: bool,
        pattern: Option<SimplePattern>,
    }
    let compiled: Vec<Compiled> = conditions
        .iter()
        .map(|c| {
            let col = t.column_index(&c.column)?;
            let value = val_str(&c.value);
            let list = match &c.value {
                Value::Array(a) => a.iter().map(val_str).collect(),
                _ => value.split(',').map(|s| s.trim().to_string()).collect(),
            };
            let pattern = if c.op == "matches" {
                Some(SimplePattern::compile(&value).map_err(EngineError::invalid)?)
            } else {
                None
            };
            Ok(Compiled {
                col,
                op: c.op.clone(),
                num: parse_number(&value),
                num2: parse_number(&val_str(&c.value2)),
                value2: val_str(&c.value2),
                value,
                list,
                ci: c.case_insensitive,
                pattern,
            })
        })
        .collect::<EngineResult<_>>()?;
    let test = |row: &Vec<String>, c: &Compiled| -> EngineResult<bool> {
        let cell = row[c.col].as_str();
        let (a, b) = if c.ci {
            (cell.to_lowercase(), c.value.to_lowercase())
        } else {
            (cell.to_string(), c.value.clone())
        };
        let n = parse_number(cell);
        Ok(match c.op.as_str() {
            "eq" | "=" | "==" => match (n, c.num) {
                (Some(x), Some(y)) => x == y,
                _ => a.trim() == b.trim(),
            },
            "neq" | "!=" | "<>" => match (n, c.num) {
                (Some(x), Some(y)) => x != y,
                _ => a.trim() != b.trim(),
            },
            "contains" => a.contains(&b),
            "not_contains" => !a.contains(&b),
            "starts_with" => a.starts_with(&b),
            "ends_with" => a.ends_with(&b),
            "gt" | ">" => match (n, c.num) {
                (Some(x), Some(y)) => x > y,
                _ => a > b,
            },
            "gte" | ">=" => match (n, c.num) {
                (Some(x), Some(y)) => x >= y,
                _ => a >= b,
            },
            "lt" | "<" => match (n, c.num) {
                (Some(x), Some(y)) => x < y,
                _ => a < b,
            },
            "lte" | "<=" => match (n, c.num) {
                (Some(x), Some(y)) => x <= y,
                _ => a <= b,
            },
            "between" => match (n, c.num, c.num2) {
                (Some(x), Some(lo), Some(hi)) => x >= lo && x <= hi,
                _ => a >= b && a <= c.value2,
            },
            "empty" | "is_empty" => is_null(cell),
            "not_empty" => !is_null(cell),
            "in" => c.list.iter().any(|v| {
                if c.ci {
                    v.eq_ignore_ascii_case(cell.trim())
                } else {
                    v == cell.trim()
                }
            }),
            "not_in" => !c.list.iter().any(|v| {
                if c.ci {
                    v.eq_ignore_ascii_case(cell.trim())
                } else {
                    v == cell.trim()
                }
            }),
            "matches" => c.pattern.as_ref().map(|p| p.is_match(cell)).unwrap_or(false),
            other => {
                return Err(EngineError::invalid(format!("Unknown filter operator `{other}`")))
            },
        })
    };
    let mut rows = Vec::new();
    for r in &t.rows {
        let results = compiled
            .iter()
            .map(|c| test(r, c))
            .collect::<EngineResult<Vec<bool>>>()?;
        let keep = if match_all {
            results.iter().all(|x| *x)
        } else {
            results.iter().any(|x| *x)
        };
        if keep != invert {
            rows.push(r.clone());
        }
    }
    Ok(Table {
        headers: t.headers.clone(),
        rows,
        issues: vec![],
        delimiter: t.delimiter,
    })
}

// ------------------------------------------------------------------- sort --

/// One sort key.
#[derive(Deserialize, Debug, Clone)]
pub struct SortKey {
    /// Column name or index.
    pub column: String,
    /// `asc` (default) or `desc`.
    #[serde(default)]
    pub order: String,
    /// `auto` (default), `text`, `number`, `natural`, `date`, `length`.
    #[serde(default, rename = "type")]
    pub kind: String,
    /// Ignore case when comparing text.
    #[serde(default)]
    pub case_insensitive: bool,
    /// Place empty cells last regardless of order.
    #[serde(default)]
    pub empty_last: bool,
}

/// Stable multi-key sort.
pub fn sort(t: &Table, keys: &[SortKey]) -> EngineResult<Table> {
    if keys.is_empty() {
        return Err(EngineError::empty("keys"));
    }
    let resolved: Vec<(usize, &SortKey, ColumnType)> = keys
        .iter()
        .map(|k| t.column_index(&k.column).map(|i| (i, k, t.column_type(i))))
        .collect::<EngineResult<_>>()?;
    let mut rows = t.rows.clone();
    rows.sort_by(|a, b| {
        for (ci, k, ty) in &resolved {
            let (x, y) = (a[*ci].as_str(), b[*ci].as_str());
            let (xe, ye) = (is_null(x), is_null(y));
            if xe || ye {
                let ord = match (xe, ye) {
                    (true, true) => Ordering::Equal,
                    (true, false) => {
                        if k.empty_last {
                            Ordering::Greater
                        } else {
                            Ordering::Less
                        }
                    },
                    (false, true) => {
                        if k.empty_last {
                            Ordering::Less
                        } else {
                            Ordering::Greater
                        }
                    },
                    (false, false) => Ordering::Equal, // excluded by the `xe || ye` guard
                };
                if ord != Ordering::Equal {
                    return if k.empty_last {
                        ord
                    } else if k.order.eq_ignore_ascii_case("desc") {
                        ord.reverse()
                    } else {
                        ord
                    };
                }
                continue;
            }
            let kind = if k.kind.is_empty() || k.kind == "auto" {
                match ty {
                    ColumnType::Integer | ColumnType::Number => "number",
                    ColumnType::Date => "date",
                    _ => "text",
                }
            } else {
                k.kind.as_str()
            };
            let mut ord = match kind {
                "number" => parse_number(x)
                    .unwrap_or(f64::NAN)
                    .partial_cmp(&parse_number(y).unwrap_or(f64::NAN))
                    .unwrap_or(Ordering::Equal),
                "natural" => natural_cmp(x, y),
                "length" => x.chars().count().cmp(&y.chars().count()),
                "date" => normalize_date(x).cmp(&normalize_date(y)),
                _ => {
                    if k.case_insensitive {
                        x.to_lowercase().cmp(&y.to_lowercase())
                    } else {
                        x.cmp(y)
                    }
                },
            };
            if k.order.eq_ignore_ascii_case("desc") {
                ord = ord.reverse();
            }
            if ord != Ordering::Equal {
                return ord;
            }
        }
        Ordering::Equal
    });
    Ok(Table {
        headers: t.headers.clone(),
        rows,
        issues: vec![],
        delimiter: t.delimiter,
    })
}

/// Converts `dd/mm/yyyy`, `mm/dd/yyyy` (ambiguous → dd/mm) and ISO dates to a sortable
/// `yyyymmdd...` string.
fn normalize_date(s: &str) -> String {
    let t = s.trim();
    let b = t.as_bytes();
    if b.len() >= 10 && b[4] == b'-' && b[7] == b'-' {
        return t.replace(['-', ':', 'T', ' '], "");
    }
    if b.len() == 10 && (b[2] == b'/' || b[2] == b'-' || b[2] == b'.') {
        return format!("{}{}{}", &t[6..10], &t[3..5], &t[0..2]);
    }
    t.to_string()
}

// ------------------------------------------------------------- statistics --

/// Per-column statistics.
pub fn statistics(t: &Table, columns: &[String], top_n: usize) -> EngineResult<Value> {
    let idx: Vec<usize> = if columns.is_empty() {
        (0..t.width()).collect()
    } else {
        t.column_indices(columns)?
    };
    let top_n = if top_n == 0 { 5 } else { top_n };
    let mut cols = Vec::new();
    for ci in idx {
        let ty = t.column_type(ci);
        let cells: Vec<&str> = t.rows.iter().map(|r| r[ci].as_str()).collect();
        let filled: Vec<&str> = cells.iter().copied().filter(|c| !is_null(c)).collect();
        let mut uniq: HashMap<&str, usize> = HashMap::new();
        for c in &filled {
            *uniq.entry(c.trim()).or_insert(0) += 1;
        }
        let mut top: Vec<(&str, usize)> = uniq.iter().map(|(k, v)| (*k, *v)).collect();
        top.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        let mut col = json!({
            "name": t.headers[ci],
            "index": ci,
            "type": ty.label(),
            "count": cells.len(),
            "filled": filled.len(),
            "empty": cells.len() - filled.len(),
            "empty_percent": if cells.is_empty() { 0.0 } else { r4((cells.len() - filled.len()) as f64 / cells.len() as f64 * 100.0) },
            "unique": uniq.len(),
            "top_values": top.iter().take(top_n).map(|(v, n)| json!({ "value": v, "count": n })).collect::<Vec<_>>(),
        });
        let lens: Vec<usize> = filled.iter().map(|c| c.chars().count()).collect();
        if !lens.is_empty() {
            col["min_length"] = json!(lens.iter().min());
            col["max_length"] = json!(lens.iter().max());
        }
        if matches!(ty, ColumnType::Integer | ColumnType::Number) {
            let mut nums: Vec<f64> = filled.iter().filter_map(|c| parse_number(c)).collect();
            if !nums.is_empty() {
                nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
                let n = nums.len() as f64;
                let sum: f64 = nums.iter().sum();
                let mean = sum / n;
                let var = nums.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
                let median = if nums.len().is_multiple_of(2) {
                    (nums[nums.len() / 2 - 1] + nums[nums.len() / 2]) / 2.0
                } else {
                    nums[nums.len() / 2]
                };
                let q = |p: f64| {
                    let pos = p * (nums.len() - 1) as f64;
                    let lo = pos.floor() as usize;
                    let hi = pos.ceil() as usize;
                    nums[lo] + (nums[hi] - nums[lo]) * (pos - lo as f64)
                };
                col["numeric"] = json!({
                    "min": nums[0],
                    "max": nums[nums.len() - 1],
                    "sum": r4(sum),
                    "mean": r4(mean),
                    "median": r4(median),
                    "std_dev": r4(var.sqrt()),
                    "variance": r4(var),
                    "q1": r4(q(0.25)),
                    "q3": r4(q(0.75)),
                    "negatives": nums.iter().filter(|v| **v < 0.0).count(),
                    "zeros": nums.iter().filter(|v| **v == 0.0).count(),
                    "histogram": histogram(&nums, 10),
                });
            }
        }
        if ty == ColumnType::Date {
            let mut dates: Vec<String> = filled.iter().map(|c| normalize_date(c)).collect();
            dates.sort();
            col["date_range"] = json!({
                "earliest": filled.iter().min_by_key(|c| normalize_date(c)),
                "latest": filled.iter().max_by_key(|c| normalize_date(c))
            });
        }
        if ty == ColumnType::Boolean {
            let trues = filled
                .iter()
                .filter(|c| {
                    matches!(c.trim().to_ascii_lowercase().as_str(), "true" | "yes" | "y" | "1")
                })
                .count();
            col["boolean"] = json!({ "true": trues, "false": filled.len() - trues });
        }
        cols.push(col);
    }
    let dup_rows = {
        let mut seen = HashSet::new();
        t.rows.iter().filter(|r| !seen.insert(r.join("\u{1f}"))).count()
    };
    Ok(json!({
        "rows": t.rows.len(),
        "columns": t.width(),
        "duplicate_rows": dup_rows,
        "empty_cells": t.rows.iter().flatten().filter(|c| c.trim().is_empty()).count(),
        "columns_detail": cols,
    }))
}

fn histogram(sorted: &[f64], bins: usize) -> Vec<Value> {
    let (min, max) = (sorted[0], sorted[sorted.len() - 1]);
    if max == min {
        return vec![json!({ "from": min, "to": max, "count": sorted.len() })];
    }
    let width = (max - min) / bins as f64;
    let mut counts = vec![0usize; bins];
    for v in sorted {
        let mut b = ((v - min) / width) as usize;
        if b >= bins {
            b = bins - 1;
        }
        counts[b] += 1;
    }
    counts
        .iter()
        .enumerate()
        .map(|(i, c)| {
            json!({
                "from": r4(min + width * i as f64),
                "to": r4(min + width * (i + 1) as f64),
                "count": c
            })
        })
        .collect()
}

// ------------------------------------------------------------------- diff --

/// Row-level diff of two tables by key columns.
pub fn diff(
    a: &Table,
    b: &Table,
    key_columns: &[String],
    ignore_columns: &[String],
    case_insensitive: bool,
) -> EngineResult<Value> {
    let all_headers: Vec<String> = {
        let mut h = a.headers.clone();
        for x in &b.headers {
            if !h.contains(x) {
                h.push(x.clone());
            }
        }
        h
    };
    let added_columns: Vec<&String> = b.headers.iter().filter(|h| !a.headers.contains(h)).collect();
    let removed_columns: Vec<&String> =
        a.headers.iter().filter(|h| !b.headers.contains(h)).collect();
    let ignore: HashSet<String> = ignore_columns.iter().cloned().collect();
    let compare_cols: Vec<&String> = all_headers
        .iter()
        .filter(|h| !ignore.contains(*h) && a.headers.contains(h) && b.headers.contains(h))
        .collect();
    let norm = |s: &str| {
        if case_insensitive {
            s.trim().to_lowercase()
        } else {
            s.trim().to_string()
        }
    };
    let get = |t: &Table, row: &[String], h: &str| -> String {
        t.headers
            .iter()
            .position(|x| x == h)
            .and_then(|i| row.get(i))
            .cloned()
            .unwrap_or_default()
    };

    let key_of = |t: &Table, row: &[String], idx: usize| -> EngineResult<String> {
        if key_columns.is_empty() {
            Ok(idx.to_string())
        } else {
            Ok(key_columns
                .iter()
                .map(|k| t.column_index(k).map(|i| norm(&row[i])))
                .collect::<EngineResult<Vec<_>>>()?
                .join("\u{1f}"))
        }
    };
    let mut a_map: BTreeMap<String, (usize, &Vec<String>)> = BTreeMap::new();
    for (i, r) in a.rows.iter().enumerate() {
        a_map.insert(key_of(a, r, i)?, (i, r));
    }
    let mut b_map: BTreeMap<String, (usize, &Vec<String>)> = BTreeMap::new();
    for (i, r) in b.rows.iter().enumerate() {
        b_map.insert(key_of(b, r, i)?, (i, r));
    }
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    let mut unchanged = 0usize;
    let mut annotated_rows: Vec<Vec<String>> = Vec::new();
    let row_obj = |t: &Table, r: &[String]| -> Value {
        Value::Object(
            t.headers
                .iter()
                .zip(r.iter())
                .map(|(h, c)| (h.clone(), Value::String(c.clone())))
                .collect(),
        )
    };
    for (k, (ia, ra)) in &a_map {
        match b_map.get(k) {
            None => {
                removed
                    .push(json!({ "row": ia + 1, "key": key_display(k), "data": row_obj(a, ra) }));
                let mut out: Vec<String> = all_headers.iter().map(|h| get(a, ra, h)).collect();
                out.push("removed".into());
                annotated_rows.push(out);
            },
            Some((ib, rb)) => {
                let diffs: Vec<Value> = compare_cols
                    .iter()
                    .filter(|h| norm(&get(a, ra, h)) != norm(&get(b, rb, h)))
                    .map(
                        |h| json!({ "column": h, "before": get(a, ra, h), "after": get(b, rb, h) }),
                    )
                    .collect();
                if diffs.is_empty() {
                    unchanged += 1;
                    let mut out: Vec<String> = all_headers.iter().map(|h| get(b, rb, h)).collect();
                    out.push("unchanged".into());
                    annotated_rows.push(out);
                } else {
                    changed.push(json!({
                        "row_a": ia + 1,
                        "row_b": ib + 1,
                        "key": key_display(k),
                        "changes": diffs
                    }));
                    let mut out: Vec<String> = all_headers
                        .iter()
                        .map(|h| {
                            let (x, y) = (get(a, ra, h), get(b, rb, h));
                            if norm(&x) != norm(&y) {
                                format!("{x} → {y}")
                            } else {
                                y
                            }
                        })
                        .collect();
                    out.push("changed".into());
                    annotated_rows.push(out);
                }
            },
        }
    }
    for (k, (ib, rb)) in &b_map {
        if !a_map.contains_key(k) {
            added.push(json!({ "row": ib + 1, "key": key_display(k), "data": row_obj(b, rb) }));
            let mut out: Vec<String> = all_headers.iter().map(|h| get(b, rb, h)).collect();
            out.push("added".into());
            annotated_rows.push(out);
        }
    }
    let mut headers = all_headers.clone();
    headers.push("_status".into());
    let annotated = Table {
        headers,
        rows: annotated_rows,
        issues: vec![],
        delimiter: a.delimiter,
    };
    Ok(json!({
        "identical": added.is_empty() && removed.is_empty() && changed.is_empty() && added_columns.is_empty() && removed_columns.is_empty(),
        "summary": {
            "rows_a": a.rows.len(),
            "rows_b": b.rows.len(),
            "added": added.len(),
            "removed": removed.len(),
            "changed": changed.len(),
            "unchanged": unchanged,
            "added_columns": added_columns,
            "removed_columns": removed_columns
        },
        "added": added,
        "removed": removed,
        "changed": changed,
        "annotated_csv": annotated.to_csv(&crate::table::CsvOptions::default())?,
        "keyed": !key_columns.is_empty(),
    }))
}

fn key_display(k: &str) -> String {
    k.replace('\u{1f}', " | ")
}

// ----------------------------------------------------------------- to_sql --

/// Generates SQL DDL/DML for a table.
pub fn to_sql(
    t: &Table,
    table: &str,
    dialect: &str,
    create_table: bool,
    batch_size: usize,
    primary_key: Option<&str>,
) -> EngineResult<Value> {
    if t.rows.is_empty() && !create_table {
        return Err(EngineError::empty("rows"));
    }
    let dialect = if dialect.is_empty() { "postgres" } else { dialect };
    let (q_open, q_close) = match dialect {
        "mysql" | "mariadb" => ('`', '`'),
        "mssql" | "sqlserver" => ('[', ']'),
        "postgres" | "postgresql" | "sqlite" => ('"', '"'),
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown SQL dialect `{other}` (postgres, mysql, sqlite, mssql)"
            )))
        },
    };
    let quote_ident = |s: &str| {
        format!("{q_open}{}{q_close}", s.replace(q_close, &format!("{q_close}{q_close}")))
    };
    let table_name = if table.trim().is_empty() {
        "data".to_string()
    } else {
        table.trim().to_string()
    };
    let types = t.column_types();
    let col_names: Vec<String> = t
        .headers
        .iter()
        .map(|h| to_snake(h))
        .map(|h| if h.is_empty() { "col".into() } else { h })
        .collect();
    let mut sql = String::new();
    if create_table {
        let mut defs: Vec<String> = Vec::new();
        for (i, n) in col_names.iter().enumerate() {
            let max_len = t.rows.iter().map(|r| r[i].chars().count()).max().unwrap_or(0);
            let ty = match (types[i], dialect) {
                (ColumnType::Integer, "postgres" | "postgresql") => "BIGINT".to_string(),
                (ColumnType::Integer, "sqlite") => "INTEGER".into(),
                (ColumnType::Integer, _) => "BIGINT".into(),
                (ColumnType::Number, "sqlite") => "REAL".into(),
                (ColumnType::Number, "mysql" | "mariadb") => "DECIMAL(18,4)".into(),
                (ColumnType::Number, _) => "NUMERIC(18,4)".into(),
                (ColumnType::Boolean, "mysql" | "mariadb") => "TINYINT(1)".into(),
                (ColumnType::Boolean, "sqlite") => "INTEGER".into(),
                (ColumnType::Boolean, "mssql" | "sqlserver") => "BIT".into(),
                (ColumnType::Boolean, _) => "BOOLEAN".into(),
                (ColumnType::Date, "sqlite") => "TEXT".into(),
                (ColumnType::Date, "mysql" | "mariadb") => "DATETIME".into(),
                (ColumnType::Date, "mssql" | "sqlserver") => "DATETIME2".into(),
                (ColumnType::Date, _) => "TIMESTAMP".into(),
                (_, "sqlite") => "TEXT".into(),
                (_, "postgres" | "postgresql") => "TEXT".into(),
                (_, "mssql" | "sqlserver") => {
                    if max_len > 4000 {
                        "NVARCHAR(MAX)".into()
                    } else {
                        format!("NVARCHAR({})", (max_len.max(1) * 2).clamp(32, 4000))
                    }
                },
                _ => {
                    if max_len > 16000 {
                        "TEXT".into()
                    } else {
                        format!("VARCHAR({})", (max_len.max(1) * 2).clamp(32, 16000))
                    }
                },
            };
            let pk = primary_key.map(|p| p == t.headers[i] || p == *n).unwrap_or(false);
            defs.push(format!(
                "  {} {}{}",
                quote_ident(n),
                ty,
                if pk { " PRIMARY KEY" } else { "" }
            ));
        }
        sql.push_str(&format!(
            "CREATE TABLE {} (\n{}\n);\n\n",
            quote_ident(&table_name),
            defs.join(",\n")
        ));
    }
    let batch = if batch_size == 0 { 500 } else { batch_size };
    let cols = col_names
        .iter()
        .map(|c| quote_ident(c))
        .collect::<Vec<_>>()
        .join(", ");
    let literal = |cell: &str, ty: ColumnType| -> String {
        if is_null(cell) {
            return "NULL".into();
        }
        match cell_to_value(cell, ty) {
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => match dialect {
                "postgres" | "postgresql" => {
                    if b {
                        "TRUE".into()
                    } else {
                        "FALSE".into()
                    }
                },
                _ => {
                    if b {
                        "1".into()
                    } else {
                        "0".into()
                    }
                },
            },
            Value::String(s) => {
                let esc = s.replace('\'', "''");
                match dialect {
                    "mysql" | "mariadb" => format!("'{}'", esc.replace('\\', "\\\\")),
                    "mssql" | "sqlserver" => format!("N'{esc}'"),
                    _ => format!("'{esc}'"),
                }
            },
            other => format!("'{}'", other.to_string().replace('\'', "''")),
        }
    };
    let mut statements = 0usize;
    for chunk in t.rows.chunks(batch) {
        let values: Vec<String> = chunk
            .iter()
            .map(|r| {
                format!(
                    "  ({})",
                    r.iter()
                        .enumerate()
                        .map(|(i, c)| literal(c, types[i]))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .collect();
        sql.push_str(&format!(
            "INSERT INTO {} ({}) VALUES\n{};\n",
            quote_ident(&table_name),
            cols,
            values.join(",\n")
        ));
        statements += 1;
    }
    Ok(json!({
        "sql": sql,
        "dialect": dialect,
        "table": table_name,
        "columns": col_names,
        "column_types": types.iter().map(|t| t.label()).collect::<Vec<_>>(),
        "rows": t.rows.len(),
        "insert_statements": statements
    }))
}

// -------------------------------------------------------------- transform --

/// One transformation step.
#[derive(Deserialize, Debug, Clone)]
pub struct Transform {
    /// Target column (not required for `add_row_number`, `add_column`).
    #[serde(default)]
    pub column: String,
    /// `uppercase`, `lowercase`, `title_case`, `trim`, `replace`, `prefix`, `suffix`, `fill_empty`,
    /// `rename`, `round`, `multiply`, `add`, `extract_digits`, `pad_left`, `add_column`,
    /// `add_row_number`, `split`, `concat`, `substring`, `date_format_iso`.
    pub op: String,
    /// Primary argument (replacement text, factor, width…).
    #[serde(default)]
    pub value: Value,
    /// Secondary argument.
    #[serde(default)]
    pub value2: Value,
    /// New column name for `rename` / `add_column`.
    #[serde(default)]
    pub name: String,
    /// Source columns for `concat`.
    #[serde(default)]
    pub columns: Vec<String>,
}

/// Applies transformation steps in order.
pub fn transform(mut t: Table, steps: &[Transform]) -> EngineResult<Table> {
    if steps.is_empty() {
        return Err(EngineError::empty("steps"));
    }
    for s in steps {
        let v = val_str(&s.value);
        let v2 = val_str(&s.value2);
        match s.op.as_str() {
            "add_row_number" => {
                let name = if s.name.is_empty() {
                    "row_number".to_string()
                } else {
                    s.name.clone()
                };
                t.headers.insert(0, name);
                for (i, r) in t.rows.iter_mut().enumerate() {
                    r.insert(0, (i + 1).to_string());
                }
            },
            "add_column" => {
                let name = if s.name.is_empty() {
                    format!("column_{}", t.width() + 1)
                } else {
                    s.name.clone()
                };
                t.headers.push(name);
                for r in t.rows.iter_mut() {
                    r.push(v.clone());
                }
            },
            "concat" => {
                let idx = t.column_indices(&s.columns)?;
                let name = if s.name.is_empty() {
                    s.columns.join("_")
                } else {
                    s.name.clone()
                };
                t.headers.push(name);
                for r in t.rows.iter_mut() {
                    let joined = idx.iter().map(|i| r[*i].clone()).collect::<Vec<_>>().join(&v);
                    r.push(joined);
                }
            },
            "split" => {
                let ci = t.column_index(&s.column)?;
                let sep = if v.is_empty() { " ".to_string() } else { v.clone() };
                let parts = t.rows.iter().map(|r| r[ci].split(&sep).count()).max().unwrap_or(1);
                let base = if s.name.is_empty() {
                    t.headers[ci].clone()
                } else {
                    s.name.clone()
                };
                for p in 0..parts {
                    t.headers.push(format!("{base}_{}", p + 1));
                }
                for r in t.rows.iter_mut() {
                    let pieces: Vec<String> =
                        r[ci].split(&sep).map(|p| p.trim().to_string()).collect();
                    for p in 0..parts {
                        r.push(pieces.get(p).cloned().unwrap_or_default());
                    }
                }
            },
            "rename" => {
                let ci = t.column_index(&s.column)?;
                let name = if s.name.is_empty() { v.clone() } else { s.name.clone() };
                if name.is_empty() {
                    return Err(EngineError::invalid("rename requires `name`"));
                }
                t.headers[ci] = name;
            },
            op => {
                let ci = t.column_index(&s.column)?;
                for r in t.rows.iter_mut() {
                    let c = &mut r[ci];
                    *c = match op {
                        "uppercase" => c.to_uppercase(),
                        "lowercase" => c.to_lowercase(),
                        "title_case" => c
                            .split_whitespace()
                            .map(|w| {
                                let mut ch = w.chars();
                                ch.next()
                                    .map(|f| {
                                        f.to_uppercase().collect::<String>()
                                            + &ch.as_str().to_lowercase()
                                    })
                                    .unwrap_or_default()
                            })
                            .collect::<Vec<_>>()
                            .join(" "),
                        "trim" => c.trim().to_string(),
                        "replace" => c.replace(&v, &v2),
                        "prefix" => format!("{v}{c}"),
                        "suffix" => format!("{c}{v}"),
                        "fill_empty" => {
                            if is_null(c) {
                                v.clone()
                            } else {
                                c.clone()
                            }
                        },
                        "round" => match parse_number(c) {
                            Some(n) => {
                                let d = v.parse::<i32>().unwrap_or(0);
                                let f = 10f64.powi(d);
                                fmt_num((n * f).round() / f)
                            },
                            None => c.clone(),
                        },
                        "multiply" => match (parse_number(c), v.parse::<f64>()) {
                            (Some(n), Ok(m)) => fmt_num(n * m),
                            _ => c.clone(),
                        },
                        "add" => match (parse_number(c), v.parse::<f64>()) {
                            (Some(n), Ok(m)) => fmt_num(n + m),
                            _ => c.clone(),
                        },
                        "extract_digits" => c.chars().filter(|ch| ch.is_ascii_digit()).collect(),
                        "pad_left" => {
                            let width = v.parse::<usize>().unwrap_or(0);
                            let pad = v2.chars().next().unwrap_or('0');
                            let len = c.chars().count();
                            if len < width {
                                std::iter::repeat_n(pad, width - len).collect::<String>() + c
                            } else {
                                c.clone()
                            }
                        },
                        "substring" => {
                            let start = v.parse::<usize>().unwrap_or(0);
                            let len = v2.parse::<usize>().unwrap_or(usize::MAX);
                            c.chars().skip(start).take(len).collect()
                        },
                        "date_format_iso" => {
                            let n = normalize_date(c);
                            if n.len() >= 8 && n.bytes().take(8).all(|b| b.is_ascii_digit()) {
                                format!("{}-{}-{}", &n[0..4], &n[4..6], &n[6..8])
                            } else {
                                c.clone()
                            }
                        },
                        other => {
                            return Err(EngineError::invalid(format!(
                                "Unknown transform `{other}`"
                            )))
                        },
                    };
                }
            },
        }
    }
    Ok(t)
}

// -------------------------------------------------------------- transpose --

/// Transposes the table.
pub fn transpose(t: &Table, first_column_as_header: bool) -> Table {
    // Build a full grid including the header row.
    let mut grid: Vec<Vec<String>> = vec![t.headers.clone()];
    grid.extend(t.rows.iter().cloned());
    let w = grid.iter().map(Vec::len).max().unwrap_or(0);
    let h = grid.len();
    let mut out: Vec<Vec<String>> = (0..w)
        .map(|c| (0..h).map(|r| grid[r].get(c).cloned().unwrap_or_default()).collect())
        .collect();
    let headers = if first_column_as_header && !out.is_empty() {
        out.remove(0)
    } else {
        (1..=h).map(|i| format!("column_{i}")).collect()
    };
    Table {
        headers,
        rows: out,
        issues: vec![],
        delimiter: t.delimiter,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::CsvOptions;

    fn t(s: &str) -> Table {
        Table::parse(s, &CsvOptions::default()).unwrap()
    }

    #[test]
    fn clean_and_dedupe() {
        let (c, rep) = clean(
            t("Name , Age\n  Bob ,  30\n\n, \nBob,30\n"),
            &CleanOptions {
                remove_duplicate_rows: true,
                header_case: "snake_case".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(c.headers, vec!["name", "age"]);
        assert_eq!(c.rows.len(), 1);
        assert_eq!(rep["duplicate_rows_removed"], 1);
    }

    #[test]
    fn validate_schema() {
        let schema = Schema {
            columns: vec![
                ColumnRule {
                    name: "age".into(),
                    kind: "integer".into(),
                    required: true,
                    min: Some(0.0),
                    max: Some(120.0),
                    ..Default::default()
                },
                ColumnRule {
                    name: "email".into(),
                    kind: "email".into(),
                    unique: true,
                    ..Default::default()
                },
            ],
            strict_columns: false,
            ordered: false,
        };
        let v = validate(
            &t("age,email\n30,a@b.com\nabc,a@b.com\n200,nope\n,x@y.org\n"),
            Some(&schema),
            0,
        );
        assert_eq!(v["valid"], false);
        let codes: Vec<&str> = v["errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["code"].as_str().unwrap())
            .collect();
        assert!(
            codes.contains(&"type")
                && codes.contains(&"unique")
                && codes.contains(&"range")
                && codes.contains(&"required")
        );
        assert_eq!(validate(&t("a,b\n1,2\n"), None, 0)["valid"], true);
    }

    #[test]
    fn pattern_matcher() {
        let p = SimplePattern::compile("^\\d{3}").err().unwrap();
        assert!(p.contains("Unsupported"));
        let p = SimplePattern::compile("^[A-Z]\\d+-\\w*$").unwrap();
        assert!(p.is_match("A12-abc_9"));
        assert!(!p.is_match("a12-abc"));
        assert!(SimplePattern::compile("b.d").unwrap().is_match("abcde"));
        assert!(SimplePattern::compile("^colou?r$").unwrap().is_match("color"));
    }

    #[test]
    fn merge_join_filter_sort() {
        let a = t("id,name\n1,Ann\n2,Bob\n");
        let b = t("id,city\n2,Paris\n3,Rome\n");
        let j = join(&a, &b, "id", "id", "outer").unwrap();
        assert_eq!(j.headers, vec!["id", "name", "city"]);
        assert_eq!(j.rows.len(), 3);
        assert_eq!(join(&a, &b, "id", "id", "inner").unwrap().rows.len(), 1);
        let ap = append(
            &[a.clone(), b.clone()],
            true,
            Some("source"),
            &["a.csv".into(), "b.csv".into()],
        )
        .unwrap();
        assert_eq!(ap.headers, vec!["id", "name", "city", "source"]);
        assert!(append(&[a.clone(), b.clone()], false, None, &[]).is_ok());
        assert!(append(&[a.clone(), t("x,y,z\n1,2,3\n")], false, None, &[]).is_err());
        let f = filter(
            &t("n,v\na,10\nb,20\nc,30\n"),
            &[Condition {
                column: "v".into(),
                op: "gte".into(),
                value: json!(20),
                value2: Value::Null,
                case_insensitive: false,
            }],
            true,
            false,
        )
        .unwrap();
        assert_eq!(f.rows.len(), 2);
        let s = sort(
            &t("n\nfile10\nfile2\nFile1\n"),
            &[SortKey {
                column: "n".into(),
                order: "asc".into(),
                kind: "natural".into(),
                case_insensitive: true,
                empty_last: true,
            }],
        )
        .unwrap();
        assert_eq!(
            s.rows.iter().map(|r| r[0].as_str()).collect::<Vec<_>>(),
            vec!["File1", "file2", "file10"]
        );
    }

    #[test]
    fn stats_diff_sql() {
        let st = statistics(&t("x,flag\n1,yes\n2,no\n3,yes\n,yes\n"), &[], 0).unwrap();
        assert_eq!(st["columns_detail"][0]["numeric"]["mean"], 2.0);
        assert_eq!(st["columns_detail"][1]["boolean"]["true"], 3);
        let d = diff(&t("id,v\n1,a\n2,b\n"), &t("id,v\n2,c\n3,d\n"), &["id".into()], &[], false)
            .unwrap();
        assert_eq!(d["summary"]["added"], 1);
        assert_eq!(d["summary"]["removed"], 1);
        assert_eq!(d["summary"]["changed"], 1);
        let sql = to_sql(
            &t("Id,Full Name,Score\n1,O'Neil,9.5\n"),
            "people",
            "postgres",
            true,
            0,
            Some("Id"),
        )
        .unwrap();
        let s = sql["sql"].as_str().unwrap();
        assert!(s.contains("\"full_name\" TEXT"));
        assert!(s.contains("'O''Neil'"));
        assert!(s.contains("\"id\" BIGINT PRIMARY KEY"));
        assert!(to_sql(&t("a\n1\n"), "x", "oracle", true, 0, None).is_err());
    }

    #[test]
    fn transforms_and_transpose() {
        let out = transform(
            t("name,amt\nbob,1.234\n"),
            &[
                Transform {
                    column: "name".into(),
                    op: "uppercase".into(),
                    value: Value::Null,
                    value2: Value::Null,
                    name: String::new(),
                    columns: vec![],
                },
                Transform {
                    column: "amt".into(),
                    op: "round".into(),
                    value: json!(1),
                    value2: Value::Null,
                    name: String::new(),
                    columns: vec![],
                },
                Transform {
                    column: String::new(),
                    op: "add_row_number".into(),
                    value: Value::Null,
                    value2: Value::Null,
                    name: "#".into(),
                    columns: vec![],
                },
            ],
        )
        .unwrap();
        assert_eq!(out.headers, vec!["#", "name", "amt"]);
        assert_eq!(out.rows[0], vec!["1", "BOB", "1.2"]);
        let tr = transpose(&t("a,b\n1,2\n3,4\n"), true);
        assert_eq!(tr.headers, vec!["a", "1", "3"]);
        assert_eq!(tr.rows, vec![vec!["b", "2", "4"]]);
    }
}
