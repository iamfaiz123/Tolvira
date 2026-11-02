//! # Dev Toolbox Data Visualisation Engine (`dataviz-engine`)
//!
//! Accepts tabular data as CSV text (`csv`) or a JSON array of objects /
//! arrays (`json`) and produces SVG charts plus statistical summaries.
//!
//! | Action | Tool(s) |
//! |---|---|
//! | `chart` | csv-chart-generator, json-chart-generator |
//! | `histogram` | histogram-generator |
//! | `frequency` | frequency-distribution |
//! | `pivot` | pivot-table-lite |
//! | `profile` | data-profiler |
//! | `describe` | descriptive-statistics |
//! | `outliers` | outlier-detector (IQR, z-score, modified z-score) |
//! | `sample` | data-sampler (random with seed, systematic, first/last, stratified) |

pub mod chart;

use chart::{ChartData, ChartOptions, Series};
use engine_shared::{EngineError, EngineResult};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use tabular_core::table::{parse_number, CsvOptions, Table};

/// Tabular input as CSV text or a JSON array of objects/arrays.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct DataInput {
    /// CSV text.
    pub csv: Option<String>,
    /// JSON rows (alternative to `csv`).
    pub json: Option<Value>,
    /// CSV parser options.
    pub options: CsvOptions,
}

fn load(input: &DataInput) -> EngineResult<Table> {
    if let Some(j) = &input.json {
        if !j.is_null() {
            return match j {
                Value::Array(items) if !items.is_empty() && items.iter().all(Value::is_object) => {
                    Table::from_objects(items)
                },
                Value::Array(items) if !items.is_empty() && items.iter().all(Value::is_array) => {
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
                Value::Object(m)
                    if !m.is_empty() && m.values().all(|v| v.is_number() || v.is_string()) =>
                {
                    // {label: value} map → two columns.
                    Ok(Table {
                        headers: vec!["label".into(), "value".into()],
                        rows: m
                            .iter()
                            .map(|(k, v)| {
                                vec![k.clone(), tabular_core::table::value_to_cell(v.clone())]
                            })
                            .collect(),
                        issues: vec![],
                        delimiter: b',',
                    })
                },
                Value::Array(a) if a.is_empty() => Err(EngineError::empty("json")),
                _ => Err(EngineError::invalid(
                    "json must be an array of objects, an array of rows (first row = headers) or \
                     a {label: value} object",
                )),
            };
        }
    }
    let text = input.csv.as_deref().unwrap_or("");
    if text.trim().is_empty() {
        return Err(EngineError::empty("csv or json"));
    }
    Table::parse(text, &input.options)
}

fn numeric_column(t: &Table, col: &str) -> EngineResult<(usize, Vec<f64>)> {
    let ci = t.column_index(col)?;
    let vals: Vec<f64> = t.rows.iter().filter_map(|r| parse_number(&r[ci])).collect();
    if vals.is_empty() {
        return Err(EngineError::invalid(format!(
            "Column `{}` has no numeric values",
            t.headers[ci]
        )));
    }
    Ok((ci, vals))
}

fn numeric_columns(t: &Table) -> Vec<usize> {
    (0..t.width())
        .filter(|c| {
            matches!(
                t.column_type(*c),
                tabular_core::table::ColumnType::Integer | tabular_core::table::ColumnType::Number
            )
        })
        .collect()
}

fn r4(v: f64) -> f64 {
    (v * 10_000.0).round() / 10_000.0
}

/// Number → cell text without a trailing `.0` for whole numbers.
fn num_cell(v: f64) -> String {
    let r = r4(v);
    if r.fract() == 0.0 && r.abs() < 1e15 {
        format!("{}", r as i64)
    } else {
        r.to_string()
    }
}

/// Descriptive statistics (count, mean, median, mode, quartiles, std dev, MAD, skewness…).
pub fn describe_values(vals: &[f64]) -> Value {
    let mut s = vals.to_vec();
    s.sort_by(f64::total_cmp);
    let n = s.len() as f64;
    let sum: f64 = s.iter().sum();
    let mean = sum / n;
    let var_p = s.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    let var_s = if s.len() > 1 {
        s.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0)
    } else {
        0.0
    };
    let q = |p: f64| {
        let pos = p * (s.len() - 1) as f64;
        let (lo, hi) = (pos.floor() as usize, pos.ceil() as usize);
        s[lo] + (s[hi] - s[lo]) * (pos - lo as f64)
    };
    let median = q(0.5);
    let sd = var_s.sqrt();
    let skew = if sd > 0.0 && s.len() > 2 {
        s.iter().map(|v| ((v - mean) / sd).powi(3)).sum::<f64>() * n / ((n - 1.0) * (n - 2.0))
    } else {
        0.0
    };
    let kurt = if sd > 0.0 && s.len() > 3 {
        s.iter().map(|v| ((v - mean) / sd).powi(4)).sum::<f64>() / n - 3.0
    } else {
        0.0
    };
    let mut mode: Option<f64> = None;
    let mut best = 1;
    let mut i = 0;
    while i < s.len() {
        let mut j = i;
        while j < s.len() && s[j] == s[i] {
            j += 1;
        }
        if j - i > best {
            best = j - i;
            mode = Some(s[i]);
        }
        i = j;
    }
    let geo_mean = if s.iter().all(|v| *v > 0.0) {
        Some(r4((s.iter().map(|v| v.ln()).sum::<f64>() / n).exp()))
    } else {
        None
    };
    let mad = {
        let mut dev: Vec<f64> = s.iter().map(|v| (v - median).abs()).collect();
        dev.sort_by(f64::total_cmp);
        let m = dev.len() / 2;
        if dev.len().is_multiple_of(2) {
            (dev[m - 1] + dev[m]) / 2.0
        } else {
            dev[m]
        }
    };
    json!({
        "count": s.len(),
        "sum": r4(sum),
        "mean": r4(mean),
        "median": r4(median),
        "mode": mode,
        "geometric_mean": geo_mean,
        "min": s[0],
        "max": s[s.len() - 1],
        "range": r4(s[s.len() - 1] - s[0]),
        "variance_sample": r4(var_s),
        "variance_population": r4(var_p),
        "std_dev_sample": r4(sd),
        "std_dev_population": r4(var_p.sqrt()),
        "standard_error": r4(sd / n.sqrt()),
        "coefficient_of_variation": if mean != 0.0 { json!(r4(sd / mean.abs())) } else { Value::Null },
        "q1": r4(q(0.25)),
        "q3": r4(q(0.75)),
        "iqr": r4(q(0.75) - q(0.25)),
        "p5": r4(q(0.05)),
        "p10": r4(q(0.10)),
        "p90": r4(q(0.90)),
        "p95": r4(q(0.95)),
        "p99": r4(q(0.99)),
        "mad": r4(mad),
        "skewness": r4(skew),
        "excess_kurtosis": r4(kurt),
        "confidence_interval_95": if s.len() > 1 { json!([r4(mean - 1.96 * sd / n.sqrt()), r4(mean + 1.96 * sd / n.sqrt())]) } else { Value::Null },
        "zeros": s.iter().filter(|v| **v == 0.0).count(),
        "negatives": s.iter().filter(|v| **v < 0.0).count(),
    })
}

/// Histogram bins for numeric values (fixed count or width).
pub fn histogram_bins(vals: &[f64], bins: usize, bin_width: Option<f64>) -> Value {
    let mut s = vals.to_vec();
    s.sort_by(f64::total_cmp);
    let (min, max) = (s[0], s[s.len() - 1]);
    let n = s.len() as f64;
    let bins = if bins > 0 {
        bins.min(500)
    } else if let Some(w) = bin_width.filter(|w| *w > 0.0) {
        (((max - min) / w).ceil() as usize).clamp(1, 500)
    } else {
        // Freedman–Diaconis, falling back to Sturges.
        let q1 = s[(0.25 * (s.len() - 1) as f64) as usize];
        let q3 = s[(0.75 * (s.len() - 1) as f64) as usize];
        let fd = 2.0 * (q3 - q1) / n.cbrt();
        if fd > 0.0 {
            (((max - min) / fd).ceil() as usize).clamp(1, 100)
        } else {
            (n.log2().ceil() as usize + 1).clamp(1, 100)
        }
    };
    let width = if let Some(w) = bin_width.filter(|w| *w > 0.0) {
        w
    } else if max > min {
        (max - min) / bins as f64
    } else {
        1.0
    };
    let start = if bin_width.is_some() {
        (min / width).floor() * width
    } else {
        min
    };
    let mut counts = vec![0usize; bins];
    for v in &s {
        let mut b = ((v - start) / width).floor() as usize;
        if b >= bins {
            b = bins - 1;
        }
        counts[b] += 1;
    }
    let mut cumulative = 0;
    let rows: Vec<Value> = counts
        .iter()
        .enumerate()
        .map(|(i, c)| {
            cumulative += c;
            json!({
                "bin": i,
                "from": r4(start + width * i as f64),
                "to": r4(start + width * (i + 1) as f64),
                "count": c,
                "percent": r4(*c as f64 / n * 100.0),
                "cumulative": cumulative,
                "cumulative_percent": r4(cumulative as f64 / n * 100.0),
                "label": format!("{}–{}", r4(start + width * i as f64), r4(start + width * (i + 1) as f64))
            })
        })
        .collect();
    json!({
        "bins": rows,
        "bin_count": bins,
        "bin_width": r4(width),
        "min": min,
        "max": max,
        "n": s.len()
    })
}

/// What to plot.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct ChartSpec {
    /// `bar`, `hbar`, `line`, `area`, `scatter`, `pie`, `donut`.
    pub kind: String,
    /// Category / x column (default: first non-numeric column or first column).
    pub x: String,
    /// Value columns (default: all numeric columns).
    pub y: Vec<String>,
    /// Aggregate duplicate categories: `sum` (default), `mean`, `count`, `min`, `max`, `none`.
    pub aggregate: String,
    /// Sort categories: `none` (default), `asc`, `desc` (by first series), `label`.
    pub sort: String,
    /// Keep only the top N categories (others grouped as "Other" for pies).
    pub top: usize,
}

/// Aggregates the data per the spec and renders an SVG chart.
pub fn chart(input: &DataInput, spec: &ChartSpec, opts: &ChartOptions) -> EngineResult<Value> {
    let t = load(input)?;
    if t.rows.is_empty() {
        return Err(EngineError::empty("rows"));
    }
    let numeric = numeric_columns(&t);
    let x_idx = if spec.x.is_empty() {
        (0..t.width()).find(|c| !numeric.contains(c)).unwrap_or(0)
    } else {
        t.column_index(&spec.x)?
    };
    let y_idx: Vec<usize> = if spec.y.is_empty() {
        numeric.iter().copied().filter(|c| *c != x_idx).collect()
    } else {
        t.column_indices(&spec.y)?
    };
    if y_idx.is_empty() {
        return Err(EngineError::invalid("No numeric value columns found; pass `y`"));
    }
    let kind = if spec.kind.is_empty() {
        "bar"
    } else {
        spec.kind.as_str()
    };
    let agg = if spec.aggregate.is_empty() {
        if kind == "scatter" || kind == "line" {
            "none"
        } else {
            "sum"
        }
    } else {
        spec.aggregate.as_str()
    };
    // Group by x.
    let mut order: Vec<String> = Vec::new();
    let mut groups: Map<String, Value> = Map::new();
    let mut acc: std::collections::HashMap<String, Vec<Vec<f64>>> =
        std::collections::HashMap::new();
    for r in &t.rows {
        let key = if agg == "none" {
            format!("{}\u{1f}{}", r[x_idx], order.len())
        } else {
            r[x_idx].trim().to_string()
        };
        if !acc.contains_key(&key) {
            order.push(key.clone());
        }
        let entry = acc
            .entry(key.clone())
            .or_insert_with(|| vec![Vec::new(); y_idx.len()]);
        for (k, yi) in y_idx.iter().enumerate() {
            if let Some(v) = parse_number(&r[*yi]) {
                entry[k].push(v);
            }
        }
    }
    let reduce = |vals: &[f64]| -> Option<f64> {
        if vals.is_empty() {
            return None;
        }
        Some(match agg {
            "sum" | "none" => vals.iter().sum(),
            "mean" | "avg" | "average" => vals.iter().sum::<f64>() / vals.len() as f64,
            "count" => vals.len() as f64,
            "min" => vals.iter().cloned().fold(f64::MAX, f64::min),
            "max" => vals.iter().cloned().fold(f64::MIN, f64::max),
            _ => vals.iter().sum(),
        })
    };
    if !matches!(agg, "sum" | "none" | "mean" | "avg" | "average" | "count" | "min" | "max") {
        return Err(EngineError::invalid(format!("Unknown aggregate `{agg}`")));
    }
    let mut labels: Vec<String> = order
        .iter()
        .map(|k| k.split('\u{1f}').next().unwrap_or("").to_string())
        .collect();
    let mut series: Vec<Series> = y_idx
        .iter()
        .enumerate()
        .map(|(k, yi)| Series {
            name: t.headers[*yi].clone(),
            values: order.iter().map(|key| reduce(&acc[key][k])).collect(),
        })
        .collect();
    // Sorting.
    let mut idx: Vec<usize> = (0..labels.len()).collect();
    match spec.sort.as_str() {
        "asc" => idx.sort_by(|a, b| {
            series[0].values[*a]
                .unwrap_or(f64::MIN)
                .total_cmp(&series[0].values[*b].unwrap_or(f64::MIN))
        }),
        "desc" => idx.sort_by(|a, b| {
            series[0].values[*b]
                .unwrap_or(f64::MIN)
                .total_cmp(&series[0].values[*a].unwrap_or(f64::MIN))
        }),
        "label" => idx.sort_by(|a, b| tabular_core::table::natural_cmp(&labels[*a], &labels[*b])),
        _ => {},
    }
    labels = idx.iter().map(|i| labels[*i].clone()).collect();
    for s in series.iter_mut() {
        s.values = idx.iter().map(|i| s.values[*i]).collect();
    }
    if spec.top > 0 && labels.len() > spec.top {
        let rest_sum: Vec<Option<f64>> = series
            .iter()
            .map(|s| {
                let r: Vec<f64> = s.values[spec.top..].iter().flatten().cloned().collect();
                if r.is_empty() {
                    None
                } else {
                    Some(r.iter().sum())
                }
            })
            .collect();
        labels.truncate(spec.top);
        for (k, s) in series.iter_mut().enumerate() {
            s.values.truncate(spec.top);
            if matches!(kind, "pie" | "donut") {
                s.values.push(rest_sum[k]);
            }
        }
        if matches!(kind, "pie" | "donut") {
            labels.push("Other".into());
        }
    }
    groups.insert("categories".into(), json!(labels.len()));
    let data = ChartData { labels: labels.clone(), series };
    let mut out = chart::render(kind, &data, opts)?;
    out["data"] = json!({
        "labels": labels,
        "series": data.series.iter().map(|s| json!({ "name": s.name, "values": s.values })).collect::<Vec<_>>(),
        "aggregate": agg,
        "x_column": t.headers[x_idx],
        "y_columns": y_idx.iter().map(|i| t.headers[*i].clone()).collect::<Vec<_>>()
    });
    Ok(out)
}

/// Frequency table for a column (binned for numeric columns).
pub fn frequency(
    input: &DataInput,
    column: &str,
    top: usize,
    bins_for_numeric: usize,
) -> EngineResult<Value> {
    let t = load(input)?;
    let ci = t.column_index(column)?;
    let ty = t.column_type(ci);
    let total = t.rows.len() as f64;
    if matches!(ty, tabular_core::table::ColumnType::Number) && bins_for_numeric > 0 {
        let vals: Vec<f64> = t.rows.iter().filter_map(|r| parse_number(&r[ci])).collect();
        let h = histogram_bins(&vals, bins_for_numeric, None);
        return Ok(json!({
            "column": t.headers[ci],
            "type": "numeric_binned",
            "table": h["bins"],
            "bin_width": h["bin_width"],
            "total": vals.len()
        }));
    }
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for r in &t.rows {
        let key = r[ci].trim().to_string();
        let key = if key.is_empty() { "(empty)".to_string() } else { key };
        if !counts.contains_key(&key) {
            order.push(key.clone());
        }
        *counts.entry(key).or_insert(0) += 1;
    }
    let mut rows: Vec<(String, usize)> = order
        .into_iter()
        .map(|k| {
            let c = counts[&k];
            (k, c)
        })
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let unique = rows.len();
    let limit = if top == 0 { 100 } else { top };
    let shown: Vec<&(String, usize)> = rows.iter().take(limit).collect();
    let other: usize = rows.iter().skip(limit).map(|r| r.1).sum();
    let mut cumulative = 0usize;
    let mut table: Vec<Value> = shown
        .iter()
        .enumerate()
        .map(|(i, (k, c))| {
            cumulative += c;
            json!({
                "rank": i + 1,
                "value": k,
                "count": c,
                "percent": r4(*c as f64 / total * 100.0),
                "cumulative_percent": r4(cumulative as f64 / total * 100.0)
            })
        })
        .collect();
    if other > 0 {
        table.push(json!({
            "rank": Value::Null,
            "value": format!("(other: {} values)", unique - limit),
            "count": other,
            "percent": r4(other as f64 / total * 100.0),
            "cumulative_percent": 100.0
        }));
    }
    // Entropy / diversity.
    let entropy: f64 = rows
        .iter()
        .map(|(_, c)| {
            let p = *c as f64 / total;
            -p * p.log2()
        })
        .sum();
    let svg = chart::render(
        "bar",
        &ChartData {
            labels: shown.iter().take(30).map(|r| r.0.clone()).collect(),
            series: vec![Series {
                name: "count".into(),
                values: shown.iter().take(30).map(|r| Some(r.1 as f64)).collect(),
            }],
        },
        &ChartOptions {
            title: format!("Frequency of {}", t.headers[ci]),
            show_legend: false,
            show_values: shown.len() <= 20,
            ..Default::default()
        },
    )?;
    Ok(json!({
        "column": t.headers[ci],
        "type": ty.label(),
        "total": t.rows.len(),
        "unique": unique,
        "mode": rows.first().map(|r| r.0.clone()),
        "mode_count": rows.first().map(|r| r.1),
        "entropy_bits": r4(entropy),
        "normalized_entropy": if unique > 1 { r4(entropy / (unique as f64).log2()) } else { 0.0 },
        "table": table,
        "svg": svg["svg"]
    }))
}

/// Pivot table with an aggregate (`sum`, `count`, `mean`, `min`, `max`, `median`) and optional
/// totals.
pub fn pivot(
    input: &DataInput,
    rows_col: &str,
    cols_col: &str,
    value_col: &str,
    agg: &str,
    totals: bool,
) -> EngineResult<Value> {
    let t = load(input)?;
    let ri = t.column_index(rows_col)?;
    let ci = if cols_col.is_empty() {
        None
    } else {
        Some(t.column_index(cols_col)?)
    };
    let vi = if value_col.is_empty() {
        None
    } else {
        Some(t.column_index(value_col)?)
    };
    let agg = if agg.is_empty() {
        if vi.is_some() {
            "sum"
        } else {
            "count"
        }
    } else {
        agg
    };
    if vi.is_none() && agg != "count" {
        return Err(EngineError::invalid("A value column is required unless aggregate is `count`"));
    }
    let mut row_keys: Vec<String> = Vec::new();
    let mut col_keys: Vec<String> = Vec::new();
    let mut cells: std::collections::HashMap<(String, String), Vec<f64>> =
        std::collections::HashMap::new();
    for r in &t.rows {
        let rk = r[ri].trim().to_string();
        let ck = ci.map(|c| r[c].trim().to_string()).unwrap_or_else(|| agg.to_string());
        if !row_keys.contains(&rk) {
            row_keys.push(rk.clone());
        }
        if !col_keys.contains(&ck) {
            col_keys.push(ck.clone());
        }
        let v = match vi {
            Some(v) => parse_number(&r[v]),
            None => Some(1.0),
        };
        if let Some(v) = v {
            cells.entry((rk, ck)).or_default().push(v);
        }
    }
    if row_keys.len() * col_keys.len() > 250_000 {
        return Err(EngineError::limit("Pivot would exceed 250,000 cells"));
    }
    row_keys.sort_by(|a, b| tabular_core::table::natural_cmp(a, b));
    col_keys.sort_by(|a, b| tabular_core::table::natural_cmp(a, b));
    let reduce = |vals: &[f64]| -> Option<f64> {
        if vals.is_empty() {
            return None;
        }
        Some(match agg {
            "sum" => vals.iter().sum(),
            "count" => vals.len() as f64,
            "mean" | "avg" | "average" => vals.iter().sum::<f64>() / vals.len() as f64,
            "min" => vals.iter().cloned().fold(f64::MAX, f64::min),
            "max" => vals.iter().cloned().fold(f64::MIN, f64::max),
            "median" => {
                let mut s = vals.to_vec();
                s.sort_by(f64::total_cmp);
                let m = s.len() / 2;
                if s.len().is_multiple_of(2) {
                    (s[m - 1] + s[m]) / 2.0
                } else {
                    s[m]
                }
            },
            _ => vals.iter().sum(),
        })
    };
    if !matches!(agg, "sum" | "count" | "mean" | "avg" | "average" | "min" | "max" | "median") {
        return Err(EngineError::invalid(format!(
            "Unknown aggregate `{agg}` (sum, count, mean, min, max, median)"
        )));
    }
    let mut headers = vec![t.headers[ri].clone()];
    headers.extend(col_keys.iter().cloned());
    if totals {
        headers.push("Total".into());
    }
    let mut out_rows: Vec<Vec<String>> = Vec::new();
    let mut col_totals: Vec<Vec<f64>> = vec![Vec::new(); col_keys.len()];
    let mut matrix: Vec<Vec<Option<f64>>> = Vec::new();
    for rk in &row_keys {
        let mut row = vec![rk.clone()];
        let mut all: Vec<f64> = Vec::new();
        let mut mrow = Vec::new();
        for (k, ck) in col_keys.iter().enumerate() {
            let vals = cells.get(&(rk.clone(), ck.clone())).cloned().unwrap_or_default();
            let v = reduce(&vals);
            all.extend(&vals);
            col_totals[k].extend(&vals);
            row.push(v.map(num_cell).unwrap_or_default());
            mrow.push(v.map(r4));
        }
        if totals {
            row.push(reduce(&all).map(num_cell).unwrap_or_default());
        }
        out_rows.push(row);
        matrix.push(mrow);
    }
    if totals {
        let mut row = vec!["Total".to_string()];
        let mut grand: Vec<f64> = Vec::new();
        for ct in &col_totals {
            grand.extend(ct);
            row.push(reduce(ct).map(num_cell).unwrap_or_default());
        }
        row.push(reduce(&grand).map(num_cell).unwrap_or_default());
        out_rows.push(row);
    }
    let table = Table {
        headers: headers.clone(),
        rows: out_rows,
        issues: vec![],
        delimiter: b',',
    };
    let mut md = format!(
        "| {} |\n| {} |\n",
        headers.join(" | "),
        headers.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")
    );
    for r in &table.rows {
        md.push_str(&format!("| {} |\n", r.join(" | ")));
    }
    Ok(json!({
        "headers": headers,
        "rows": table.rows,
        "csv": table.to_csv(&CsvOptions::default())?,
        "markdown": md,
        "matrix": matrix,
        "row_keys": row_keys,
        "column_keys": col_keys,
        "aggregate": agg,
        "value_column": vi.map(|v| t.headers[v].clone()),
        "cells": row_keys.len() * col_keys.len()
    }))
}

/// Column-by-column profile: types, nulls, uniques, stats, top values, correlations.
pub fn profile(input: &DataInput) -> EngineResult<Value> {
    let t = load(input)?;
    let mut stats = tabular_core::ops::statistics(&t, &[], 5)?;
    // Enrich numeric columns with full descriptive stats and a histogram.
    if let Some(cols) = stats["columns_detail"].as_array_mut() {
        for c in cols.iter_mut() {
            if c["numeric"].is_object() {
                let ci = c["index"].as_u64().unwrap_or(0) as usize;
                let vals: Vec<f64> = t.rows.iter().filter_map(|r| parse_number(&r[ci])).collect();
                if !vals.is_empty() {
                    c["describe"] = describe_values(&vals);
                    c["outliers_iqr"] = json!(iqr_outliers(&vals, 1.5).len());
                }
            }
            let ci = c["index"].as_u64().unwrap_or(0) as usize;
            let filled = c["filled"].as_u64().unwrap_or(0);
            let unique = c["unique"].as_u64().unwrap_or(0);
            c["is_unique_key"] =
                json!(filled > 0 && unique == filled && c["empty"].as_u64() == Some(0));
            c["is_constant"] = json!(unique <= 1);
            c["cardinality"] = json!(if filled == 0 {
                "empty"
            } else if unique == filled {
                "unique"
            } else if unique as f64 / filled as f64 > 0.5 {
                "high"
            } else if unique <= 10 {
                "low"
            } else {
                "medium"
            });
            let _ = ci;
        }
    }
    // Correlations between numeric columns.
    let num = numeric_columns(&t);
    let mut correlations = Vec::new();
    for (a, &ca) in num.iter().enumerate() {
        for &cb in num.iter().skip(a + 1) {
            let pairs: Vec<(f64, f64)> = t
                .rows
                .iter()
                .filter_map(|r| Some((parse_number(&r[ca])?, parse_number(&r[cb])?)))
                .collect();
            if pairs.len() >= 3 {
                let r = pearson(&pairs);
                correlations.push(json!({
                    "a": t.headers[ca],
                    "b": t.headers[cb],
                    "pearson": r4(r),
                    "strength": match r.abs() { x if x >= 0.8 => "very strong", x if x >= 0.6 => "strong", x if x >= 0.4 => "moderate", x if x >= 0.2 => "weak", _ => "none" },
                    "n": pairs.len()
                }));
            }
        }
    }
    correlations.sort_by(|a, b| {
        b["pearson"]
            .as_f64()
            .unwrap_or(0.0)
            .abs()
            .total_cmp(&a["pearson"].as_f64().unwrap_or(0.0).abs())
    });
    stats["correlations"] = json!(correlations.into_iter().take(50).collect::<Vec<_>>());
    stats["quality"] = json!({
        "completeness_percent": r4((1.0 - stats["empty_cells"].as_f64().unwrap_or(0.0) / (t.rows.len().max(1) * t.width().max(1)) as f64) * 100.0),
        "duplicate_rows": stats["duplicate_rows"],
        "issues": t.issues,
    });
    stats["headers"] = json!(t.headers);
    stats["sample"] = json!(t.to_json_rows(0, 5));
    Ok(stats)
}

fn pearson(p: &[(f64, f64)]) -> f64 {
    let n = p.len() as f64;
    let (mx, my) = (p.iter().map(|x| x.0).sum::<f64>() / n, p.iter().map(|x| x.1).sum::<f64>() / n);
    let cov: f64 = p.iter().map(|(x, y)| (x - mx) * (y - my)).sum();
    let sx: f64 = p.iter().map(|(x, _)| (x - mx).powi(2)).sum::<f64>().sqrt();
    let sy: f64 = p.iter().map(|(_, y)| (y - my).powi(2)).sum::<f64>().sqrt();
    if sx == 0.0 || sy == 0.0 {
        0.0
    } else {
        cov / (sx * sy)
    }
}

fn iqr_outliers(vals: &[f64], k: f64) -> Vec<usize> {
    let mut s = vals.to_vec();
    s.sort_by(f64::total_cmp);
    let q = |p: f64| {
        let pos = p * (s.len() - 1) as f64;
        let (lo, hi) = (pos.floor() as usize, pos.ceil() as usize);
        s[lo] + (s[hi] - s[lo]) * (pos - lo as f64)
    };
    let (q1, q3) = (q(0.25), q(0.75));
    let iqr = q3 - q1;
    vals.iter()
        .enumerate()
        .filter(|(_, v)| **v < q1 - k * iqr || **v > q3 + k * iqr)
        .map(|(i, _)| i)
        .collect()
}

/// Flags outliers by `iqr`, `zscore` or `modified_zscore` (MAD).
pub fn outliers(
    input: &DataInput,
    column: &str,
    method: &str,
    threshold: Option<f64>,
) -> EngineResult<Value> {
    let t = load(input)?;
    let ci = t.column_index(column)?;
    let indexed: Vec<(usize, f64)> = t
        .rows
        .iter()
        .enumerate()
        .filter_map(|(i, r)| parse_number(&r[ci]).map(|v| (i, v)))
        .collect();
    if indexed.len() < 4 {
        return Err(EngineError::invalid("Need at least 4 numeric values to detect outliers"));
    }
    let vals: Vec<f64> = indexed.iter().map(|x| x.1).collect();
    let d = describe_values(&vals);
    let method = if method.is_empty() { "iqr" } else { method };
    let (flagged, bounds): (Vec<usize>, Value) = match method {
        "iqr" => {
            let k = threshold.unwrap_or(1.5);
            let (q1, q3) = (d["q1"].as_f64().unwrap_or(0.0), d["q3"].as_f64().unwrap_or(0.0));
            let iqr = q3 - q1;
            (
                iqr_outliers(&vals, k),
                json!({ "lower": r4(q1 - k * iqr), "upper": r4(q3 + k * iqr), "k": k }),
            )
        },
        "zscore" | "z" => {
            let z = threshold.unwrap_or(3.0);
            let (mean, sd) =
                (d["mean"].as_f64().unwrap_or(0.0), d["std_dev_sample"].as_f64().unwrap_or(0.0));
            if sd == 0.0 {
                (vec![], json!({ "note": "zero variance" }))
            } else {
                (
                    vals.iter()
                        .enumerate()
                        .filter(|(_, v)| ((*v - mean) / sd).abs() > z)
                        .map(|(i, _)| i)
                        .collect(),
                    json!({ "lower": r4(mean - z * sd), "upper": r4(mean + z * sd), "z": z }),
                )
            }
        },
        "modified_zscore" | "mad" => {
            let z = threshold.unwrap_or(3.5);
            let (median, mad) =
                (d["median"].as_f64().unwrap_or(0.0), d["mad"].as_f64().unwrap_or(0.0));
            if mad == 0.0 {
                (vec![], json!({ "note": "zero MAD" }))
            } else {
                (
                    vals.iter()
                        .enumerate()
                        .filter(|(_, v)| (0.6745 * (*v - median) / mad).abs() > z)
                        .map(|(i, _)| i)
                        .collect(),
                    json!({
                        "lower": r4(median - z * mad / 0.6745),
                        "upper": r4(median + z * mad / 0.6745),
                        "threshold": z
                    }),
                )
            }
        },
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown method `{other}` (iqr, zscore, modified_zscore)"
            )))
        },
    };
    let rows: Vec<Value> = flagged
        .iter()
        .take(1000)
        .map(|&k| {
            let (row_idx, v) = indexed[k];
            let mut obj = Map::new();
            for (h, c) in t.headers.iter().zip(&t.rows[row_idx]) {
                obj.insert(h.clone(), Value::String(c.clone()));
            }
            json!({
                "row": row_idx + 1,
                "value": v,
                "z_score": if d["std_dev_sample"].as_f64().unwrap_or(0.0) > 0.0 { json!(r4((v - d["mean"].as_f64().unwrap_or(0.0)) / d["std_dev_sample"].as_f64().unwrap_or(0.0))) } else { Value::Null },
                "data": obj
            })
        })
        .collect();
    let clean: Vec<f64> = vals
        .iter()
        .enumerate()
        .filter(|(i, _)| !flagged.contains(i))
        .map(|x| *x.1)
        .collect();
    let kept_rows: Vec<usize> = (0..t.rows.len())
        .filter(|i| !flagged.iter().any(|&k| indexed[k].0 == *i))
        .collect();
    let cleaned = Table {
        headers: t.headers.clone(),
        rows: kept_rows.iter().map(|i| t.rows[*i].clone()).collect(),
        issues: vec![],
        delimiter: t.delimiter,
    };
    Ok(json!({
        "column": t.headers[ci],
        "method": method,
        "bounds": bounds,
        "outlier_count": flagged.len(),
        "outlier_percent": r4(flagged.len() as f64 / vals.len() as f64 * 100.0),
        "outliers": rows,
        "stats": d,
        "stats_without_outliers": if clean.is_empty() { Value::Null } else { describe_values(&clean) },
        "cleaned_csv": cleaned.to_csv(&CsvOptions::default())?
    }))
}

/// Samples rows (`random`, `first`, `last`, `systematic`, `stratified`).
pub fn sample(
    input: &DataInput,
    method: &str,
    n: usize,
    percent: Option<f64>,
    seed: Option<u64>,
    stratify: &str,
) -> EngineResult<Value> {
    let t = load(input)?;
    let total = t.rows.len();
    if total == 0 {
        return Err(EngineError::empty("rows"));
    }
    let want = if let Some(p) = percent.filter(|p| *p > 0.0) {
        ((p / 100.0) * total as f64).round().max(1.0) as usize
    } else if n > 0 {
        n
    } else {
        10
    };
    let want = want.min(total);
    let mut state = seed.unwrap_or(0x2545_F491_4F6C_DD1D) | 1;
    let mut rnd = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let method = if method.is_empty() { "random" } else { method };
    let mut idx: Vec<usize> = match method {
        "random" => {
            let mut all: Vec<usize> = (0..total).collect();
            for i in (1..all.len()).rev() {
                let j = (rnd() % (i as u64 + 1)) as usize;
                all.swap(i, j);
            }
            all.truncate(want);
            all.sort_unstable();
            all
        },
        "first" | "head" => (0..want).collect(),
        "last" | "tail" => (total - want..total).collect(),
        "systematic" | "every_nth" => {
            let step = (total as f64 / want as f64).max(1.0);
            let start = (rnd() % step.floor().max(1.0) as u64) as f64;
            (0..want)
                .map(|i| ((start + i as f64 * step) as usize).min(total - 1))
                .collect()
        },
        "stratified" => {
            if stratify.is_empty() {
                return Err(EngineError::invalid("stratified sampling needs `stratify` column"));
            }
            let ci = t.column_index(stratify)?;
            let mut groups: std::collections::BTreeMap<String, Vec<usize>> =
                std::collections::BTreeMap::new();
            for (i, r) in t.rows.iter().enumerate() {
                groups.entry(r[ci].trim().to_string()).or_default().push(i);
            }
            let mut out = Vec::new();
            for members in groups.values_mut() {
                let share =
                    ((members.len() as f64 / total as f64) * want as f64).round().max(1.0) as usize;
                for i in (1..members.len()).rev() {
                    let j = (rnd() % (i as u64 + 1)) as usize;
                    members.swap(i, j);
                }
                out.extend(members.iter().take(share));
            }
            out.sort_unstable();
            out.dedup();
            out
        },
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown method `{other}` (random, first, last, systematic, stratified)"
            )))
        },
    };
    idx.dedup();
    let sampled = Table {
        headers: t.headers.clone(),
        rows: idx.iter().map(|i| t.rows[*i].clone()).collect(),
        issues: vec![],
        delimiter: t.delimiter,
    };
    Ok(json!({
        "method": method,
        "requested": want,
        "sampled": sampled.rows.len(),
        "total": total,
        "percent": r4(sampled.rows.len() as f64 / total as f64 * 100.0),
        "seed": seed,
        "row_indices": idx.iter().map(|i| i + 1).collect::<Vec<_>>(),
        "csv": sampled.to_csv(&CsvOptions::default())?,
        "rows": sampled.to_objects(true)
    }))
}

/// Every action understood by the data-visualisation engine (`{"action": "...", "payload":
/// {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum VizAction {
    /// Renders an SVG chart.
    Chart {
        /// Tabular input.
        #[serde(flatten)]
        input: DataInput,
        /// Columns, aggregation and sorting.
        #[serde(default)]
        spec: ChartSpec,
        /// Rendering options.
        #[serde(default)]
        chart: ChartOptions,
    },
    /// Histogram of a numeric column.
    Histogram {
        /// Tabular input.
        #[serde(flatten)]
        input: DataInput,
        /// Column name.
        column: String,
        /// Number of bins.
        #[serde(default)]
        bins: usize,
        /// Fixed bin width (overrides `bins`).
        #[serde(default)]
        bin_width: Option<f64>,
        /// Rendering options.
        #[serde(default)]
        chart: ChartOptions,
    },
    /// Frequency table.
    Frequency {
        /// Tabular input.
        #[serde(flatten)]
        input: DataInput,
        /// Column name.
        column: String,
        /// Number of top values.
        #[serde(default)]
        top: usize,
        /// Bins used for numeric columns.
        #[serde(default)]
        bins_for_numeric: usize,
    },
    /// Pivot table.
    Pivot {
        /// Tabular input.
        #[serde(flatten)]
        input: DataInput,
        /// Row field.
        rows: String,
        /// Column field.
        #[serde(default)]
        columns: String,
        /// Value field.
        #[serde(default)]
        value: String,
        /// Aggregate function.
        #[serde(default)]
        aggregate: String,
        /// Add row/column totals.
        #[serde(default = "default_true")]
        totals: bool,
    },
    /// Data profile.
    Profile {
        /// Tabular input.
        #[serde(flatten)]
        input: DataInput,
    },
    /// Descriptive statistics for a column or a raw list.
    Describe {
        /// Tabular input.
        #[serde(flatten)]
        input: DataInput,
        /// Column name.
        #[serde(default)]
        column: Option<String>,
        /// Raw values (alternative to `input`).
        #[serde(default)]
        values: Vec<f64>,
    },
    /// Outlier detection.
    Outliers {
        /// Tabular input.
        #[serde(flatten)]
        input: DataInput,
        /// Column name.
        column: String,
        /// `iqr` (default), `zscore` or `modified_zscore`.
        #[serde(default)]
        method: String,
        /// Method threshold (1.5 / 3 / 3.5 by default).
        #[serde(default)]
        threshold: Option<f64>,
    },
    /// Row sampling.
    Sample {
        /// Tabular input.
        #[serde(flatten)]
        input: DataInput,
        /// `random` (default), `first`, `last`, `systematic`, `stratified`.
        #[serde(default)]
        method: String,
        /// Number of rows.
        #[serde(default)]
        n: usize,
        /// Percentage of rows (alternative to `n`).
        #[serde(default)]
        percent: Option<f64>,
        /// Seed for reproducible sampling.
        #[serde(default)]
        seed: Option<u64>,
        /// Column to stratify by.
        #[serde(default)]
        stratify: String,
    },
}

fn default_true() -> bool {
    true
}

/// Dispatches a [`VizAction`] to its implementation.
pub fn handle_action(action: VizAction) -> EngineResult<Value> {
    match action {
        VizAction::Chart { input, spec, chart: o } => chart(&input, &spec, &o),
        VizAction::Histogram { input, column, bins, bin_width, chart: o } => {
            let t = load(&input)?;
            let (ci, vals) = numeric_column(&t, &column)?;
            let mut h = histogram_bins(&vals, bins, bin_width);
            let bins_arr = h["bins"].as_array().cloned().unwrap_or_default();
            let labels: Vec<String> = bins_arr
                .iter()
                .map(|b| b["label"].as_str().unwrap_or("").to_string())
                .collect();
            let counts: Vec<Option<f64>> = bins_arr.iter().map(|b| b["count"].as_f64()).collect();
            let mut opts = o.clone();
            if opts.title.is_empty() {
                opts.title = format!("Distribution of {}", t.headers[ci]);
            }
            opts.show_legend = false;
            opts.max_categories = 500;
            let svg = chart::render(
                "bar",
                &ChartData {
                    labels,
                    series: vec![Series { name: "count".into(), values: counts }],
                },
                &opts,
            )?;
            h["svg"] = svg["svg"].clone();
            h["column"] = json!(t.headers[ci]);
            h["stats"] = describe_values(&vals);
            Ok(h)
        },
        VizAction::Frequency { input, column, top, bins_for_numeric } => {
            frequency(&input, &column, top, bins_for_numeric)
        },
        VizAction::Pivot {
            input,
            rows,
            columns,
            value,
            aggregate,
            totals,
        } => pivot(&input, &rows, &columns, &value, &aggregate, totals),
        VizAction::Profile { input } => profile(&input),
        VizAction::Describe { input, column, values } => {
            if !values.is_empty() {
                return Ok(json!({
                    "column": Value::Null,
                    "stats": describe_values(&values),
                    "histogram": histogram_bins(&values, 0, None)
                }));
            }
            let t = load(&input)?;
            match column {
                Some(c) if !c.is_empty() => {
                    let (ci, vals) = numeric_column(&t, &c)?;
                    Ok(json!({
                        "column": t.headers[ci],
                        "stats": describe_values(&vals),
                        "histogram": histogram_bins(&vals, 0, None)
                    }))
                },
                _ => {
                    let cols: Vec<Value> = numeric_columns(&t).into_iter().map(|ci| { let vals: Vec<f64> = t.rows.iter().filter_map(|r| parse_number(&r[ci])).collect(); json!({
                        "column": t.headers[ci],
                        "stats": if vals.is_empty() { Value::Null } else { describe_values(&vals) }
                    }) }).collect();
                    if cols.is_empty() {
                        return Err(EngineError::invalid("No numeric columns found"));
                    }
                    Ok(json!({ "columns": cols }))
                },
            }
        },
        VizAction::Outliers { input, column, method, threshold } => {
            outliers(&input, &column, &method, threshold)
        },
        VizAction::Sample {
            input,
            method,
            n,
            percent,
            seed,
            stratify,
        } => sample(&input, &method, n, percent, seed, &stratify),
    }
}

engine_shared::export_engine!(handle_action, VizAction);

#[cfg(test)]
mod tests {
    use super::*;

    const CSV: &str = "region,product,sales,units\nNorth,A,100,5\nNorth,B,150,7\nSouth,A,80,4\nSouth,B,120,6\nEast,A,300,12\nEast,B,90,3\nWest,A,110,5\nWest,B,1000,40\n";

    fn input() -> DataInput {
        DataInput {
            csv: Some(CSV.into()),
            ..Default::default()
        }
    }

    #[test]
    fn charts_and_tables() {
        let c = chart(
            &input(),
            &ChartSpec {
                kind: "bar".into(),
                x: "region".into(),
                y: vec!["sales".into()],
                aggregate: "sum".into(),
                sort: "desc".into(),
                top: 0,
            },
            &ChartOptions::default(),
        )
        .unwrap();
        assert_eq!(c["data"]["labels"][0], "West");
        assert_eq!(c["data"]["series"][0]["values"][0], 1110.0);
        let p = chart(
            &input(),
            &ChartSpec {
                kind: "pie".into(),
                x: "region".into(),
                y: vec!["units".into()],
                top: 2,
                ..Default::default()
            },
            &ChartOptions::default(),
        )
        .unwrap();
        assert_eq!(p["data"]["labels"].as_array().unwrap().len(), 3);
        let j = chart(
            &DataInput {
                json: Some(json!({ "a": 1, "b": 2 })),
                ..Default::default()
            },
            &ChartSpec::default(),
            &ChartOptions::default(),
        )
        .unwrap();
        assert_eq!(j["points"], 2);
        let f = frequency(&input(), "product", 0, 0).unwrap();
        assert_eq!(f["unique"], 2);
        assert_eq!(f["table"][0]["count"], 4);
        let pv = pivot(&input(), "region", "product", "sales", "sum", true).unwrap();
        assert_eq!(pv["headers"], json!(["region", "A", "B", "Total"]));
        assert_eq!(pv["rows"][0], json!(["East", "300", "90", "390"]));
        assert_eq!(pv["rows"][4][3], "1950");
        assert!(pivot(&input(), "region", "", "", "mean", false).is_err());
        let pr = profile(&input()).unwrap();
        assert_eq!(pr["columns_detail"][2]["cardinality"], "unique");
        assert!(pr["correlations"][0]["pearson"].as_f64().unwrap() > 0.9);
    }

    #[test]
    fn stats_outliers_sampling() {
        let d = describe_values(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]);
        assert_eq!(d["mean"], 5.0);
        assert_eq!(d["std_dev_population"], 2.0);
        assert_eq!(d["mode"], 4.0);
        let o = outliers(&input(), "sales", "iqr", None).unwrap();
        assert_eq!(o["outlier_count"], 1);
        assert_eq!(o["outliers"][0]["value"], 1000.0);
        assert!(outliers(&input(), "sales", "weird", None).is_err());
        let s = sample(&input(), "random", 3, None, Some(42), "").unwrap();
        assert_eq!(s["sampled"], 3);
        let s2 = sample(&input(), "random", 3, None, Some(42), "").unwrap();
        assert_eq!(s["row_indices"], s2["row_indices"]);
        assert_eq!(
            sample(&input(), "stratified", 4, None, Some(1), "product").unwrap()["sampled"],
            4
        );
        assert_eq!(sample(&input(), "first", 0, Some(25.0), None, "").unwrap()["sampled"], 2);
        let h = histogram_bins(&[1.0, 2.0, 2.0, 3.0, 10.0], 3, None);
        assert_eq!(h["bin_count"], 3);
        assert_eq!(h["bins"][0]["count"], 4);
        assert!(load(&DataInput::default()).is_err());
        assert!(load(&DataInput {
            json: Some(json!([1, 2])),
            ..Default::default()
        })
        .is_err());
    }
}
