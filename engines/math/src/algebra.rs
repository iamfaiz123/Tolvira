//! Statistics, equations, matrices, trigonometry, calculus (numeric) and
//! plotting data.

use crate::expr::{self, Context};
use engine_shared::{EngineError, EngineResult};
use serde_json::{json, Value};
use std::collections::HashMap;

fn finite(values: &[f64], what: &str) -> EngineResult<()> {
    if values.is_empty() {
        return Err(EngineError::empty(what));
    }
    if values.iter().any(|v| !v.is_finite()) {
        return Err(EngineError::invalid(format!("{what} must contain only finite numbers")));
    }
    Ok(())
}

/// Parses a free-form list of numbers ("1, 2 3\n4").
pub fn parse_numbers(text: &str) -> EngineResult<Vec<f64>> {
    let mut out = Vec::new();
    for tok in text.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
        let t = tok.trim();
        if t.is_empty() {
            continue;
        }
        out.push(
            t.parse::<f64>()
                .map_err(|_| EngineError::invalid(format!("`{t}` is not a number")))?,
        );
    }
    Ok(out)
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let rank = p / 100.0 * (sorted.len() - 1) as f64;
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    sorted[lo] + (sorted[hi] - sorted[lo]) * (rank - lo as f64)
}

/// Descriptive statistics.
pub fn statistics(values: &[f64], population: bool) -> EngineResult<Value> {
    finite(values, "values")?;
    let n = values.len() as f64;
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let sum: f64 = values.iter().sum();
    let mean = sum / n;
    let median = if sorted.len().is_multiple_of(2) {
        (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
    } else {
        sorted[sorted.len() / 2]
    };
    let mut counts: Vec<(f64, usize)> = Vec::new();
    for &v in &sorted {
        match counts.last_mut() {
            Some((x, c)) if *x == v => *c += 1,
            _ => counts.push((v, 1)),
        }
    }
    let max_count = counts.iter().map(|(_, c)| *c).max().unwrap_or(0);
    let modes: Vec<f64> = if max_count <= 1 {
        vec![]
    } else {
        counts
            .iter()
            .filter(|(_, c)| *c == max_count)
            .map(|(v, _)| *v)
            .collect()
    };
    let ss: f64 = values.iter().map(|v| (v - mean).powi(2)).sum();
    let variance = if population || values.len() == 1 {
        ss / n
    } else {
        ss / (n - 1.0)
    };
    let std_dev = variance.sqrt();
    let q1 = percentile(&sorted, 25.0);
    let q3 = percentile(&sorted, 75.0);
    let iqr = q3 - q1;
    let outliers: Vec<f64> = sorted
        .iter()
        .copied()
        .filter(|v| *v < q1 - 1.5 * iqr || *v > q3 + 1.5 * iqr)
        .collect();
    let skew = if std_dev > 0.0 && n > 2.0 {
        values.iter().map(|v| ((v - mean) / std_dev).powi(3)).sum::<f64>() * n
            / ((n - 1.0) * (n - 2.0))
    } else {
        0.0
    };
    let kurt = if std_dev > 0.0 && n > 3.0 {
        values.iter().map(|v| ((v - mean) / std_dev).powi(4)).sum::<f64>() / n - 3.0
    } else {
        0.0
    };
    let geo_mean = if sorted[0] > 0.0 {
        (values.iter().map(|v| v.ln()).sum::<f64>() / n).exp()
    } else {
        f64::NAN
    };
    let harm_mean = if sorted[0] > 0.0 {
        n / values.iter().map(|v| 1.0 / v).sum::<f64>()
    } else {
        f64::NAN
    };
    let rms = (values.iter().map(|v| v * v).sum::<f64>() / n).sqrt();
    let mad = values.iter().map(|v| (v - mean).abs()).sum::<f64>() / n;
    let opt = |v: f64| if v.is_finite() { json!(v) } else { Value::Null };
    Ok(json!({
        "count": values.len(),
        "sum": sum,
        "mean": mean,
        "median": median,
        "mode": modes,
        "min": sorted[0],
        "max": sorted[sorted.len() - 1],
        "range": sorted[sorted.len() - 1] - sorted[0],
        "variance": variance,
        "standard_deviation": std_dev,
        "variance_kind": if population { "population" } else { "sample" },
        "standard_error": std_dev / n.sqrt(),
        "coefficient_of_variation": if mean != 0.0 { json!(std_dev / mean.abs()) } else { Value::Null },
        "q1": q1,
        "q3": q3,
        "iqr": iqr,
        "percentiles": {
            "p5": percentile(&sorted, 5.0),
            "p10": percentile(&sorted, 10.0),
            "p90": percentile(&sorted, 90.0),
            "p95": percentile(&sorted, 95.0)
        },
        "outliers": outliers,
        "skewness": skew,
        "kurtosis_excess": kurt,
        "geometric_mean": opt(geo_mean),
        "harmonic_mean": opt(harm_mean),
        "root_mean_square": rms,
        "mean_absolute_deviation": mad,
        "sorted": sorted,
        "sum_of_squares": ss,
    }))
}

/// Frequency table + histogram bins.
pub fn frequency(values: &[f64], bins: usize) -> EngineResult<Value> {
    finite(values, "values")?;
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mut table: Vec<(f64, usize)> = Vec::new();
    for &v in &sorted {
        match table.last_mut() {
            Some((x, c)) if *x == v => *c += 1,
            _ => table.push((v, 1)),
        }
    }
    let n = values.len() as f64;
    let mut cumulative = 0usize;
    let freq: Vec<Value> = table
        .iter()
        .map(|(v, c)| {
            cumulative += c;
            json!({ "value": v, "count": c, "relative": *c as f64 / n, "cumulative": cumulative })
        })
        .collect();
    let (min, max) = (sorted[0], sorted[sorted.len() - 1]);
    let bins = bins.clamp(1, 200);
    let width = if max > min { (max - min) / bins as f64 } else { 1.0 };
    let mut hist = vec![0usize; bins];
    for &v in &sorted {
        let idx = (((v - min) / width).floor() as usize).min(bins - 1);
        hist[idx] += 1;
    }
    let histogram: Vec<Value> = hist
        .iter()
        .enumerate()
        .map(|(i, c)| {
            json!({
                "from": min + i as f64 * width,
                "to": min + (i + 1) as f64 * width,
                "count": c
            })
        })
        .collect();
    Ok(json!({
        "frequency": freq,
        "histogram": histogram,
        "bin_width": width,
        "unique_values": table.len()
    }))
}

/// Solves `ax² + bx + c = 0` (real or complex roots, discriminant, vertex).
pub fn quadratic(a: f64, b: f64, c: f64) -> EngineResult<Value> {
    finite(&[a, b, c], "coefficients")?;
    if a == 0.0 {
        if b == 0.0 {
            return Err(EngineError::invalid("a and b cannot both be zero"));
        }
        return Ok(
            json!({ "type": "linear", "roots": [-c / b], "equation": format!("{b}x + {c} = 0") }),
        );
    }
    let disc = b * b - 4.0 * a * c;
    let vertex_x = -b / (2.0 * a);
    let vertex_y = a * vertex_x * vertex_x + b * vertex_x + c;
    let steps = vec![
        format!("Discriminant Δ = b² − 4ac = ({b})² − 4·({a})·({c}) = {}", expr::fmt_num(disc)),
        "x = (−b ± √Δ) / 2a".to_string(),
    ];
    let (kind, roots): (&str, Vec<Value>) = if disc > 0.0 {
        let s = disc.sqrt();
        ("two real roots", vec![json!((-b + s) / (2.0 * a)), json!((-b - s) / (2.0 * a))])
    } else if disc == 0.0 {
        ("one repeated real root", vec![json!(-b / (2.0 * a))])
    } else {
        let re = -b / (2.0 * a);
        let im = (-disc).sqrt() / (2.0 * a);
        (
            "two complex roots",
            vec![
                json!({
                    "re": re,
                    "im": im,
                    "text": format!("{} + {}i", expr::fmt_num(re), expr::fmt_num(im.abs()))
                }),
                json!({
                    "re": re,
                    "im": -im,
                    "text": format!("{} − {}i", expr::fmt_num(re), expr::fmt_num(im.abs()))
                }),
            ],
        )
    };
    let factored = if disc >= 0.0 {
        let r1 = (-b + disc.sqrt()) / (2.0 * a);
        let r2 = (-b - disc.sqrt()) / (2.0 * a);
        Some(format!(
            "{}(x − {})(x − {})",
            expr::fmt_num(a),
            expr::fmt_num(r1),
            expr::fmt_num(r2)
        ))
    } else {
        None
    };
    Ok(json!({
        "type": kind,
        "discriminant": disc,
        "roots": roots,
        "vertex": { "x": vertex_x, "y": vertex_y },
        "axis_of_symmetry": vertex_x,
        "opens": if a > 0.0 { "upward" } else { "downward" },
        "factored": factored,
        "steps": steps,
        "equation": format!("{a}x² + {b}x + {c} = 0")
    }))
}

/// Solves a linear system given as a coefficient matrix and constants
/// (Gaussian elimination with partial pivoting).
pub fn linear_system(matrix: &[Vec<f64>], constants: &[f64]) -> EngineResult<Value> {
    let n = matrix.len();
    if n == 0 || constants.len() != n || matrix.iter().any(|r| r.len() != n) {
        return Err(EngineError::invalid("Provide an n×n coefficient matrix and n constants"));
    }
    if n > 50 {
        return Err(EngineError::limit("At most 50 equations"));
    }
    let mut a: Vec<Vec<f64>> = matrix
        .iter()
        .zip(constants)
        .map(|(r, c)| {
            let mut row = r.clone();
            row.push(*c);
            row
        })
        .collect();
    for r in &a {
        finite(r, "matrix")?;
    }
    let mut rank = 0;
    let mut pivot_cols = Vec::new();
    let mut row = 0;
    for col in 0..n {
        let pivot = (row..n).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()));
        let Some(p) = pivot else { break };
        if a[p][col].abs() < 1e-12 {
            continue;
        }
        a.swap(row, p);
        let pv = a[row][col];
        for j in col..=n {
            a[row][j] /= pv;
        }
        for i in 0..n {
            if i != row && a[i][col].abs() > 0.0 {
                let f = a[i][col];
                for j in col..=n {
                    a[i][j] -= f * a[row][j];
                }
            }
        }
        pivot_cols.push(col);
        rank += 1;
        row += 1;
        if row == n {
            break;
        }
    }
    // Inconsistent: a zero row with non-zero constant.
    let inconsistent = a
        .iter()
        .any(|r| r[..n].iter().all(|v| v.abs() < 1e-10) && r[n].abs() > 1e-9);
    if inconsistent {
        return Ok(json!({
            "status": "inconsistent",
            "rank": rank,
            "solution": Value::Null,
            "message": "The system has no solution"
        }));
    }
    if rank < n {
        return Ok(json!({
            "status": "infinite",
            "rank": rank,
            "free_variables": (0..n).filter(|c| !pivot_cols.contains(c)).map(|c| format!("x{}", c + 1)).collect::<Vec<_>>(),
            "message": "The system has infinitely many solutions"
        }));
    }
    let solution: Vec<f64> = (0..n)
        .map(|i| {
            let v = a[i][n];
            if v.abs() < 1e-12 {
                0.0
            } else {
                (v * 1e10).round() / 1e10
            }
        })
        .collect();
    Ok(json!({
        "status": "unique",
        "rank": n,
        "solution": solution,
        "variables": (0..n).map(|i| format!("x{} = {}", i + 1, expr::fmt_num(solution[i]))).collect::<Vec<_>>()
    }))
}

/// Solves `a·x + b = c·x + d` style single-variable linear equations given as text.
pub fn linear_equation(equation: &str) -> EngineResult<Value> {
    let (lhs, rhs) = equation
        .split_once('=')
        .ok_or_else(|| EngineError::invalid("Equation must contain `=`"))?;
    // Evaluate both sides at x = 0 and x = 1 to get the linear coefficients.
    let eval_at = |side: &str, x: f64| -> EngineResult<f64> {
        let vars: HashMap<String, f64> = [("x".to_string(), x)].into();
        expr::eval(side, &Context { vars: &vars, degrees: false })
    };
    let (l0, l1, r0, r1) =
        (eval_at(lhs, 0.0)?, eval_at(lhs, 1.0)?, eval_at(rhs, 0.0)?, eval_at(rhs, 1.0)?);
    let l2 = eval_at(lhs, 2.0)?;
    let r2 = eval_at(rhs, 2.0)?;
    // Verify linearity.
    if ((l2 - l1) - (l1 - l0)).abs() > 1e-9 || ((r2 - r1) - (r1 - r0)).abs() > 1e-9 {
        return Err(EngineError::invalid(
            "The equation is not linear in x (use the quadratic or numeric solver)",
        ));
    }
    let a = (l1 - l0) - (r1 - r0);
    let b = l0 - r0;
    if a.abs() < 1e-12 {
        return Ok(json!({
            "status": if b.abs() < 1e-12 { "identity" } else { "no_solution" },
            "message": if b.abs() < 1e-12 { "True for every x" } else { "No solution" }
        }));
    }
    let x = -b / a;
    Ok(json!({
        "status": "unique",
        "x": (x * 1e12).round() / 1e12,
        "check": { "lhs": eval_at(lhs, x)?, "rhs": eval_at(rhs, x)? },
        "normal_form": format!("{}x + {} = 0", expr::fmt_num(a), expr::fmt_num(b))
    }))
}

/// Bisection / Newton hybrid root finder for f(x) = 0 on [lo, hi].
pub fn numeric_root(expression: &str, lo: f64, hi: f64) -> EngineResult<Value> {
    let compiled = expr::compile_expr(expression)?;
    let f = |x: f64| -> EngineResult<f64> {
        let vars: HashMap<String, f64> = [("x".to_string(), x)].into();
        expr::eval_compiled(&compiled, &Context { vars: &vars, degrees: false })
    };
    let (mut a, mut b) = (lo.min(hi), lo.max(hi));
    let (mut fa, fb) = (f(a)?, f(b)?);
    if fa == 0.0 {
        return Ok(json!({ "root": a, "iterations": 0 }));
    }
    if fb == 0.0 {
        return Ok(json!({ "root": b, "iterations": 0 }));
    }
    if fa.signum() == fb.signum() {
        // Scan for a sign change.
        let steps = 200;
        let mut found = None;
        let mut prev = (a, fa);
        for i in 1..=steps {
            let x = a + (b - a) * i as f64 / steps as f64;
            let fx = f(x)?;
            if fx == 0.0 || fx.signum() != prev.1.signum() {
                found = Some((prev.0, x));
                break;
            }
            prev = (x, fx);
        }
        let (na, nb) = found.ok_or_else(|| {
            EngineError::not_found("No sign change found in the interval; try a different range")
        })?;
        a = na;
        b = nb;
        fa = f(a)?;
    }
    let mut iterations = 0;
    for _ in 0..200 {
        iterations += 1;
        let m = (a + b) / 2.0;
        let fm = f(m)?;
        if fm == 0.0 || (b - a).abs() < 1e-13 {
            return Ok(json!({ "root": m, "iterations": iterations, "f_root": fm }));
        }
        if fm.signum() == fa.signum() {
            a = m;
            fa = fm;
        } else {
            b = m;
        }
    }
    Ok(json!({ "root": (a + b) / 2.0, "iterations": iterations, "f_root": f((a + b) / 2.0)? }))
}

// ---------------------------------------------------------------- matrices

/// Dense row-major matrix.
pub type Matrix = Vec<Vec<f64>>;

fn check_matrix(m: &Matrix, name: &str) -> EngineResult<(usize, usize)> {
    if m.is_empty() || m[0].is_empty() {
        return Err(EngineError::empty(&format!("matrix {name}")));
    }
    let cols = m[0].len();
    if m.iter().any(|r| r.len() != cols) {
        return Err(EngineError::invalid(format!(
            "Matrix {name} rows must all have the same length"
        )));
    }
    if m.len() > 100 || cols > 100 {
        return Err(EngineError::limit("Matrices are limited to 100×100"));
    }
    for r in m {
        finite(r, &format!("matrix {name}"))?;
    }
    Ok((m.len(), cols))
}

/// Determinant by Gaussian elimination with partial pivoting.
pub fn determinant(m: &Matrix) -> EngineResult<f64> {
    let (r, c) = check_matrix(m, "A")?;
    if r != c {
        return Err(EngineError::invalid("Determinant needs a square matrix"));
    }
    let mut a = m.clone();
    let n = r;
    let mut det = 1.0;
    for col in 0..n {
        let pivot = (col..n)
            .max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))
            .unwrap_or(col);
        if a[pivot][col].abs() < 1e-14 {
            return Ok(0.0);
        }
        if pivot != col {
            a.swap(pivot, col);
            det = -det;
        }
        det *= a[col][col];
        for i in col + 1..n {
            let f = a[i][col] / a[col][col];
            for j in col..n {
                a[i][j] -= f * a[col][j];
            }
        }
    }
    Ok(det)
}

/// Inverse by Gauss-Jordan elimination (`E_INVALID_INPUT` when singular).
pub fn inverse(m: &Matrix) -> EngineResult<Matrix> {
    let (r, c) = check_matrix(m, "A")?;
    if r != c {
        return Err(EngineError::invalid("Inverse needs a square matrix"));
    }
    let n = r;
    let mut a: Matrix = m
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mut x = row.clone();
            x.extend((0..n).map(|j| if i == j { 1.0 } else { 0.0 }));
            x
        })
        .collect();
    for col in 0..n {
        let pivot = (col..n)
            .max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))
            .unwrap_or(col);
        if a[pivot][col].abs() < 1e-12 {
            return Err(EngineError::invalid(
                "Matrix is singular (determinant is zero); no inverse exists",
            ));
        }
        a.swap(pivot, col);
        let pv = a[col][col];
        for j in 0..2 * n {
            a[col][j] /= pv;
        }
        for i in 0..n {
            if i != col {
                let f = a[i][col];
                for j in 0..2 * n {
                    a[i][j] -= f * a[col][j];
                }
            }
        }
    }
    Ok(a.into_iter()
        .map(|row| {
            row[n..]
                .iter()
                .map(|v| {
                    if v.abs() < 1e-12 {
                        0.0
                    } else {
                        (v * 1e12).round() / 1e12
                    }
                })
                .collect()
        })
        .collect())
}

/// Transpose.
pub fn transpose(m: &Matrix) -> Matrix {
    if m.is_empty() {
        return vec![];
    }
    (0..m[0].len()).map(|j| m.iter().map(|r| r[j]).collect()).collect()
}

/// Matrix product (`E_INVALID_INPUT` on dimension mismatch).
pub fn multiply(a: &Matrix, b: &Matrix) -> EngineResult<Matrix> {
    let (ar, ac) = check_matrix(a, "A")?;
    let (br, bc) = check_matrix(b, "B")?;
    if ac != br {
        return Err(EngineError::invalid(format!(
            "Cannot multiply {ar}×{ac} by {br}×{bc}: inner dimensions must match"
        )));
    }
    Ok((0..ar)
        .map(|i| (0..bc).map(|j| (0..ac).map(|k| a[i][k] * b[k][j]).sum()).collect())
        .collect())
}

/// Rank by row reduction.
pub fn rank(m: &Matrix) -> EngineResult<usize> {
    let (r, c) = check_matrix(m, "A")?;
    let mut a = m.clone();
    let mut rank = 0;
    let mut row = 0;
    for col in 0..c {
        if row >= r {
            break;
        }
        let pivot = (row..r)
            .max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))
            .unwrap_or(row);
        if a[pivot][col].abs() < 1e-10 {
            continue;
        }
        a.swap(pivot, row);
        for i in row + 1..r {
            let f = a[i][col] / a[row][col];
            for j in col..c {
                a[i][j] -= f * a[row][j];
            }
        }
        rank += 1;
        row += 1;
    }
    Ok(rank)
}

/// Matrix operation dispatcher (`add`, `subtract`, `multiply`, `scalar`, `transpose`,
/// `determinant`, `inverse`, `rank`, `trace`, `power`).
pub fn matrix_op(
    op: &str,
    a: &Matrix,
    b: Option<&Matrix>,
    scalar: Option<f64>,
) -> EngineResult<Value> {
    let (ar, ac) = check_matrix(a, "A")?;
    let need_b = || b.ok_or_else(|| EngineError::invalid("Second matrix `b` is required"));
    let result: Value = match op {
        "add" | "+" | "subtract" | "-" => {
            let bm = need_b()?;
            let (br, bc) = check_matrix(bm, "B")?;
            if (ar, ac) != (br, bc) {
                return Err(EngineError::invalid("Matrices must have the same dimensions"));
            }
            let sign = if op == "add" || op == "+" { 1.0 } else { -1.0 };
            json!({
                "matrix": a.iter().zip(bm).map(|(x, y)| x.iter().zip(y).map(|(p, q)| p + sign * q).collect::<Vec<_>>()).collect::<Vec<_>>()
            })
        },
        "multiply" | "*" => json!({ "matrix": multiply(a, need_b()?)? }),
        "scalar" => {
            let s = scalar.ok_or_else(|| EngineError::invalid("`scalar` is required"))?;
            json!({
                "matrix": a.iter().map(|r| r.iter().map(|v| v * s).collect::<Vec<_>>()).collect::<Vec<_>>()
            })
        },
        "transpose" => json!({ "matrix": transpose(a) }),
        "determinant" | "det" => json!({ "determinant": determinant(a)? }),
        "inverse" => {
            let inv = inverse(a)?;
            json!({ "matrix": inv, "determinant": determinant(a)? })
        },
        "rank" => json!({ "rank": rank(a)? }),
        "trace" => {
            if ar != ac {
                return Err(EngineError::invalid("Trace needs a square matrix"));
            }
            json!({ "trace": (0..ar).map(|i| a[i][i]).sum::<f64>() })
        },
        "power" => {
            let p = scalar
                .ok_or_else(|| EngineError::invalid("`scalar` (the exponent) is required"))?;
            if ar != ac || p < 0.0 || p.fract() != 0.0 || p > 64.0 {
                return Err(EngineError::invalid(
                    "Power needs a square matrix and an integer exponent 0..64",
                ));
            }
            let mut result: Matrix = (0..ar)
                .map(|i| (0..ar).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
                .collect();
            for _ in 0..p as usize {
                result = multiply(&result, a)?;
            }
            json!({ "matrix": result })
        },
        "info" => {
            json!({
                "rows": ar,
                "columns": ac,
                "square": ar == ac,
                "rank": rank(a)?,
                "determinant": if ar == ac { json!(determinant(a)?) } else { Value::Null },
                "symmetric": ar == ac && (0..ar).all(|i| (0..ac).all(|j| (a[i][j] - a[j][i]).abs() < 1e-12)),
                "identity": ar == ac && (0..ar).all(|i| (0..ac).all(|j| (a[i][j] - if i == j { 1.0 } else { 0.0 }).abs() < 1e-12))
            })
        },
        other => return Err(EngineError::invalid(format!("Unknown matrix operation `{other}`"))),
    };
    Ok(result)
}

// ------------------------------------------------------------ trigonometry

/// All trigonometric/hyperbolic values for an angle in degrees, radians or gradians.
pub fn trigonometry(angle: f64, unit: &str) -> EngineResult<Value> {
    if !angle.is_finite() {
        return Err(EngineError::invalid("Angle must be finite"));
    }
    let rad = match unit {
        "" | "deg" | "degrees" => angle.to_radians(),
        "rad" | "radians" => angle,
        "grad" | "gradians" => angle * std::f64::consts::PI / 200.0,
        "turn" | "turns" => angle * std::f64::consts::TAU,
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown unit `{other}` (deg, rad, grad, turn)"
            )))
        },
    };
    let deg = rad.to_degrees();
    let norm_deg = deg.rem_euclid(360.0);
    let clean = |v: f64| {
        if v.abs() < 1e-12 {
            0.0
        } else if v.abs() > 1e15 {
            f64::INFINITY
        } else {
            (v * 1e12).round() / 1e12
        }
    };
    let (s, c, t) = (
        clean(rad.sin()),
        clean(rad.cos()),
        if clean(rad.cos()) == 0.0 {
            f64::INFINITY
        } else {
            clean(rad.tan())
        },
    );
    let opt = |v: f64| {
        if v.is_finite() {
            json!(v)
        } else {
            json!("undefined")
        }
    };
    let quadrant = match norm_deg {
        d if d == 0.0 || d == 90.0 || d == 180.0 || d == 270.0 => "axis",
        d if d < 90.0 => "I",
        d if d < 180.0 => "II",
        d if d < 270.0 => "III",
        _ => "IV",
    };
    let reference = if norm_deg <= 90.0 {
        norm_deg
    } else if norm_deg <= 180.0 {
        180.0 - norm_deg
    } else if norm_deg <= 270.0 {
        norm_deg - 180.0
    } else {
        360.0 - norm_deg
    };
    let exact = exact_trig(norm_deg);
    Ok(json!({
        "degrees": deg,
        "radians": rad,
        "gradians": rad * 200.0 / std::f64::consts::PI,
        "turns": rad / std::f64::consts::TAU,
        "normalized_degrees": norm_deg,
        "radians_as_pi": format!("{}π", expr::fmt_num((rad / std::f64::consts::PI * 1e6).round() / 1e6)),
        "sin": s,
        "cos": c,
        "tan": opt(t),
        "csc": if s == 0.0 { json!("undefined") } else { json!(clean(1.0 / s)) },
        "sec": if c == 0.0 { json!("undefined") } else { json!(clean(1.0 / c)) },
        "cot": if s == 0.0 { json!("undefined") } else { json!(clean(c / s)) },
        "quadrant": quadrant,
        "reference_angle_degrees": reference,
        "unit_circle_point": { "x": c, "y": s },
        "exact": exact,
    }))
}

fn exact_trig(deg: f64) -> Value {
    let table: &[(f64, &str, &str, &str)] = &[
        (0.0, "0", "1", "0"),
        (30.0, "1/2", "√3/2", "√3/3"),
        (45.0, "√2/2", "√2/2", "1"),
        (60.0, "√3/2", "1/2", "√3"),
        (90.0, "1", "0", "undefined"),
        (120.0, "√3/2", "-1/2", "-√3"),
        (135.0, "√2/2", "-√2/2", "-1"),
        (150.0, "1/2", "-√3/2", "-√3/3"),
        (180.0, "0", "-1", "0"),
        (210.0, "-1/2", "-√3/2", "√3/3"),
        (225.0, "-√2/2", "-√2/2", "1"),
        (240.0, "-√3/2", "-1/2", "√3"),
        (270.0, "-1", "0", "undefined"),
        (300.0, "-√3/2", "1/2", "-√3"),
        (315.0, "-√2/2", "√2/2", "-1"),
        (330.0, "-1/2", "√3/2", "-√3/3"),
    ];
    table
        .iter()
        .find(|(d, _, _, _)| (d - deg).abs() < 1e-9)
        .map(|(_, s, c, t)| json!({ "sin": s, "cos": c, "tan": t }))
        .unwrap_or(Value::Null)
}

/// Solves a triangle from any valid combination of sides/angles (SSS, SAS, ASA, AAS, SSA).
pub fn triangle(sides: &[f64], angles: &[f64]) -> EngineResult<Value> {
    let (a, b, c) = match sides.len() {
        3 => (sides[0], sides[1], sides[2]),
        2 if angles.len() == 1 => {
            // SAS: included angle between the two sides.
            let gamma = angles[0].to_radians();
            let c = (sides[0].powi(2) + sides[1].powi(2) - 2.0 * sides[0] * sides[1] * gamma.cos())
                .sqrt();
            (sides[0], sides[1], c)
        },
        1 if angles.len() >= 2 => {
            // ASA / AAS: use law of sines.
            let (ang_a, ang_b) = (angles[0], angles[1]);
            let ang_c = 180.0 - ang_a - ang_b;
            if ang_c <= 0.0 {
                return Err(EngineError::invalid("Angles must sum to less than 180°"));
            }
            let a = sides[0];
            let k = a / ang_a.to_radians().sin();
            (a, k * ang_b.to_radians().sin(), k * ang_c.to_radians().sin())
        },
        _ => {
            return Err(EngineError::invalid(
                "Provide 3 sides, 2 sides + included angle, or 1 side + 2 angles",
            ))
        },
    };
    if !(a > 0.0 && b > 0.0 && c > 0.0) || a + b <= c || a + c <= b || b + c <= a {
        return Err(EngineError::invalid("These lengths do not form a valid triangle"));
    }
    let cos_a = (b * b + c * c - a * a) / (2.0 * b * c);
    let cos_b = (a * a + c * c - b * b) / (2.0 * a * c);
    let ang_a = cos_a.clamp(-1.0, 1.0).acos().to_degrees();
    let ang_b = cos_b.clamp(-1.0, 1.0).acos().to_degrees();
    let ang_c = 180.0 - ang_a - ang_b;
    let s = (a + b + c) / 2.0;
    let area = (s * (s - a) * (s - b) * (s - c)).sqrt();
    let kind = if (a - b).abs() < 1e-9 && (b - c).abs() < 1e-9 {
        "equilateral"
    } else if (a - b).abs() < 1e-9 || (b - c).abs() < 1e-9 || (a - c).abs() < 1e-9 {
        "isosceles"
    } else {
        "scalene"
    };
    let max_ang = ang_a.max(ang_b).max(ang_c);
    let angle_kind = if (max_ang - 90.0).abs() < 1e-7 {
        "right"
    } else if max_ang > 90.0 {
        "obtuse"
    } else {
        "acute"
    };
    let heights = [2.0 * area / a, 2.0 * area / b, 2.0 * area / c];
    Ok(json!({
        "sides": { "a": a, "b": b, "c": c },
        "angles_degrees": { "A": ang_a, "B": ang_b, "C": ang_c },
        "perimeter": a + b + c,
        "area": area,
        "type": kind,
        "angle_type": angle_kind,
        "heights": { "to_a": heights[0], "to_b": heights[1], "to_c": heights[2] },
        "inradius": area / s,
        "circumradius": a * b * c / (4.0 * area),
    }))
}

// ----------------------------------------------------------------- calculus

/// Evaluates an expression of x over a range (plot data) with optional
/// numeric derivative and integral.
pub fn plot(
    expression: &str,
    x_min: f64,
    x_max: f64,
    points: usize,
    degrees: bool,
    derivative: bool,
) -> EngineResult<Value> {
    if !(2..=10_000).contains(&points) {
        return Err(EngineError::invalid("`points` must be between 2 and 10000"));
    }
    if !(x_min < x_max) || !x_min.is_finite() || !x_max.is_finite() {
        return Err(EngineError::invalid("x_min must be less than x_max"));
    }
    let compiled = expr::compile_expr(expression)?;
    let f = |x: f64| -> Option<f64> {
        let vars: HashMap<String, f64> = [("x".to_string(), x)].into();
        expr::eval_compiled(&compiled, &Context { vars: &vars, degrees })
            .ok()
            .filter(|v| v.is_finite())
    };
    let step = (x_max - x_min) / (points - 1) as f64;
    let mut xs = Vec::with_capacity(points);
    let mut ys = Vec::with_capacity(points);
    let mut dys = Vec::with_capacity(points);
    let (mut ymin, mut ymax) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut undefined = 0;
    for i in 0..points {
        let x = x_min + step * i as f64;
        xs.push(x);
        match f(x) {
            Some(y) => {
                ymin = ymin.min(y);
                ymax = ymax.max(y);
                ys.push(json!(y));
            },
            None => {
                undefined += 1;
                ys.push(Value::Null);
            },
        }
        if derivative {
            let h = step / 100.0;
            dys.push(match (f(x + h), f(x - h)) {
                (Some(a), Some(b)) => json!((a - b) / (2.0 * h)),
                _ => Value::Null,
            });
        }
    }
    // Simpson integral over defined samples.
    let integral = if undefined == 0 && points >= 3 {
        let n = if points % 2 == 1 { points - 1 } else { points - 2 };
        let h = step;
        let y: Vec<f64> = ys.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect();
        let mut s = y[0] + y[n];
        for i in 1..n {
            s += if i % 2 == 1 { 4.0 * y[i] } else { 2.0 * y[i] };
        }
        Some(s * h / 3.0)
    } else {
        None
    };
    // Approximate roots by sign change.
    let mut roots = Vec::new();
    for i in 1..points {
        if let (Some(a), Some(b)) = (ys[i - 1].as_f64(), ys[i].as_f64()) {
            if a == 0.0 {
                roots.push(xs[i - 1]);
            } else if a.signum() != b.signum() && roots.len() < 50 {
                let t = a / (a - b);
                roots.push(xs[i - 1] + t * step);
            }
        }
    }
    Ok(json!({
        "x": xs,
        "y": ys,
        "dy": if derivative { json!(dys) } else { Value::Null },
        "y_min": if ymin.is_finite() { json!(ymin) } else { Value::Null },
        "y_max": if ymax.is_finite() { json!(ymax) } else { Value::Null },
        "undefined_points": undefined,
        "integral_simpson": integral,
        "approximate_roots": roots,
        "expression": expression
    }))
}

/// Numerical derivative (central difference) at `x`.
pub fn derivative_at(expression: &str, x: f64, degrees: bool) -> EngineResult<Value> {
    let compiled = expr::compile_expr(expression)?;
    let f = |x: f64| -> EngineResult<f64> {
        let vars: HashMap<String, f64> = [("x".to_string(), x)].into();
        expr::eval_compiled(&compiled, &Context { vars: &vars, degrees })
    };
    let h = 1e-5 * x.abs().max(1.0);
    let d1 = (f(x + h)? - f(x - h)?) / (2.0 * h);
    let d2 = (f(x + h)? - 2.0 * f(x)? + f(x - h)?) / (h * h);
    Ok(json!({
        "x": x,
        "f": f(x)?,
        "first_derivative": (d1 * 1e8).round() / 1e8,
        "second_derivative": (d2 * 1e5).round() / 1e5,
        "tangent_line": format!("y = {}x + {}", expr::fmt_num((d1 * 1e6).round() / 1e6), expr::fmt_num(((f(x)? - d1 * x) * 1e6).round() / 1e6)),
        "method": "central difference (h = 1e-5)"
    }))
}

/// Numerical integral (adaptive Simpson) from `a` to `b`.
pub fn integral(expression: &str, a: f64, b: f64, degrees: bool) -> EngineResult<Value> {
    if !a.is_finite() || !b.is_finite() {
        return Err(EngineError::invalid("Bounds must be finite"));
    }
    let compiled = expr::compile_expr(expression)?;
    let f = |x: f64| -> EngineResult<f64> {
        let vars: HashMap<String, f64> = [("x".to_string(), x)].into();
        let v = expr::eval_compiled(&compiled, &Context { vars: &vars, degrees })?;
        if !v.is_finite() {
            return Err(EngineError::invalid(format!("Function is undefined at x = {x}")));
        }
        Ok(v)
    };
    let n = 2000;
    let h = (b - a) / n as f64;
    let mut s = f(a)? + f(b)?;
    for i in 1..n {
        let x = a + h * i as f64;
        s += if i % 2 == 1 { 4.0 * f(x)? } else { 2.0 * f(x)? };
    }
    let simpson = s * h / 3.0;
    let mut trap = (f(a)? + f(b)?) / 2.0;
    for i in 1..n {
        trap += f(a + h * i as f64)?;
    }
    let trapezoid = trap * h;
    Ok(json!({
        "integral": (simpson * 1e10).round() / 1e10,
        "trapezoid": trapezoid,
        "simpson": simpson,
        "from": a,
        "to": b,
        "subintervals": n,
        "error_estimate": (simpson - trapezoid).abs()
    }))
}

/// Numerical limit at `x0` from both sides.
pub fn limit(expression: &str, x0: f64, degrees: bool) -> EngineResult<Value> {
    let compiled = expr::compile_expr(expression)?;
    let f = |x: f64| -> Option<f64> {
        let vars: HashMap<String, f64> = [("x".to_string(), x)].into();
        expr::eval_compiled(&compiled, &Context { vars: &vars, degrees }).ok()
    };
    let sample = |sign: f64| -> Vec<Option<f64>> {
        [1e-2, 1e-4, 1e-6, 1e-8].iter().map(|h| f(x0 + sign * h)).collect()
    };
    let left = sample(-1.0);
    let right = sample(1.0);
    let est = |s: &[Option<f64>]| -> Option<f64> { s.last().copied().flatten() };
    let (l, r) = (est(&left), est(&right));
    let diverging = |s: &[Option<f64>]| {
        s.iter()
            .flatten()
            .map(|v| v.abs())
            .collect::<Vec<_>>()
            .windows(2)
            .all(|w| w[1] > w[0] * 10.0)
            && s.iter().flatten().last().map(|v| v.abs() > 1e6).unwrap_or(false)
    };
    let status = match (l, r) {
        (Some(l), Some(r)) if diverging(&left) || diverging(&right) => {
            if l.signum() == r.signum() {
                if l > 0.0 {
                    "+∞"
                } else {
                    "-∞"
                }
            } else {
                "diverges (±∞)"
            }
        },
        (Some(l), Some(r)) if (l - r).abs() <= 1e-6 * l.abs().max(1.0) => "exists",
        (Some(_), Some(_)) => "left and right limits differ",
        _ => "undefined",
    };
    let value = if status == "exists" {
        l.map(|v| (v * 1e6).round() / 1e6)
    } else {
        None
    };
    Ok(json!({
        "x0": x0,
        "limit": value,
        "status": status,
        "left_limit": l,
        "right_limit": r,
        "f_at_x0": f(x0).filter(|v| v.is_finite()),
        "samples": { "left": left, "right": right }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats() {
        let s = statistics(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0], true).unwrap();
        assert_eq!(s["mean"], 5.0);
        assert_eq!(s["standard_deviation"], 2.0);
        assert_eq!(s["median"], 4.5);
        assert_eq!(s["mode"], json!([4.0]));
        let sample = statistics(&[1.0, 2.0, 3.0, 4.0], false).unwrap();
        assert!((sample["variance"].as_f64().unwrap() - 1.6667).abs() < 1e-3);
        assert!(statistics(&[], true).is_err());
        assert!(statistics(&[f64::NAN], true).is_err());
        assert_eq!(parse_numbers("1, 2 3\n4;5").unwrap(), vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        assert!(parse_numbers("1 x").is_err());
        let f = frequency(&[1.0, 1.0, 2.0, 5.0], 2).unwrap();
        assert_eq!(f["frequency"][0]["count"], 2);
        assert_eq!(f["histogram"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn equations() {
        let q = quadratic(1.0, -3.0, 2.0).unwrap();
        assert_eq!(q["roots"], json!([2.0, 1.0]));
        assert_eq!(quadratic(1.0, 0.0, 1.0).unwrap()["roots"][0]["im"], 1.0);
        assert_eq!(quadratic(0.0, 2.0, -4.0).unwrap()["roots"][0], 2.0);
        assert!(quadratic(0.0, 0.0, 1.0).is_err());
        let s = linear_system(&[vec![2.0, 1.0], vec![1.0, 3.0]], &[5.0, 10.0]).unwrap();
        assert_eq!(s["solution"], json!([1.0, 3.0]));
        assert_eq!(
            linear_system(&[vec![1.0, 1.0], vec![2.0, 2.0]], &[1.0, 2.0]).unwrap()["status"],
            "infinite"
        );
        assert_eq!(
            linear_system(&[vec![1.0, 1.0], vec![1.0, 1.0]], &[1.0, 2.0]).unwrap()["status"],
            "inconsistent"
        );
        assert_eq!(linear_equation("3x + 2 = 11").unwrap()["x"], 3.0);
        assert_eq!(linear_equation("2(x - 1) = x + 4").unwrap()["x"], 6.0);
        assert!(linear_equation("x^2 = 4").is_err());
        assert!(linear_equation("x + 1").is_err());
        let r = numeric_root("x^2 - 2", 0.0, 2.0).unwrap();
        assert!((r["root"].as_f64().unwrap() - 2f64.sqrt()).abs() < 1e-9);
        assert!(numeric_root("x^2 + 1", -1.0, 1.0).is_err());
    }

    #[test]
    fn matrices() {
        let a = vec![vec![4.0, 7.0], vec![2.0, 6.0]];
        assert_eq!(determinant(&a).unwrap(), 10.0);
        let inv = inverse(&a).unwrap();
        assert!((inv[0][0] - 0.6).abs() < 1e-12 && (inv[0][1] + 0.7).abs() < 1e-12);
        let id = multiply(&a, &inv).unwrap();
        assert!((id[0][0] - 1.0).abs() < 1e-12 && id[0][1].abs() < 1e-12);
        assert_eq!(transpose(&vec![vec![1.0, 2.0, 3.0]]), vec![vec![1.0], vec![2.0], vec![3.0]]);
        assert_eq!(rank(&vec![vec![1.0, 2.0], vec![2.0, 4.0]]).unwrap(), 1);
        assert!(inverse(&vec![vec![1.0, 2.0], vec![2.0, 4.0]]).is_err());
        assert!(multiply(&a, &vec![vec![1.0, 2.0, 3.0]]).is_err());
        assert_eq!(matrix_op("trace", &a, None, None).unwrap()["trace"], 10.0);
        assert_eq!(matrix_op("power", &a, None, Some(2.0)).unwrap()["matrix"][0][0], 30.0);
        assert_eq!(matrix_op("info", &a, None, None).unwrap()["square"], true);
        assert!(matrix_op("add", &a, None, None).is_err());
        assert!(check_matrix(&vec![vec![1.0], vec![1.0, 2.0]], "A").is_err());
    }

    #[test]
    fn trig_and_calculus() {
        let t = trigonometry(30.0, "deg").unwrap();
        assert_eq!(t["sin"], 0.5);
        assert_eq!(t["exact"]["tan"], "√3/3");
        assert_eq!(trigonometry(90.0, "deg").unwrap()["tan"], "undefined");
        assert_eq!(trigonometry(std::f64::consts::PI, "rad").unwrap()["normalized_degrees"], 180.0);
        assert!(trigonometry(1.0, "furlongs").is_err());
        let tr = triangle(&[3.0, 4.0, 5.0], &[]).unwrap();
        assert_eq!(tr["area"], 6.0);
        assert_eq!(tr["angle_type"], "right");
        assert!(triangle(&[1.0, 1.0, 5.0], &[]).is_err());
        let sas = triangle(&[3.0, 4.0], &[90.0]).unwrap();
        assert!((sas["sides"]["c"].as_f64().unwrap() - 5.0).abs() < 1e-9);
        let p = plot("x^2", -1.0, 1.0, 21, false, true).unwrap();
        assert_eq!(p["y_min"], 0.0);
        assert!((p["integral_simpson"].as_f64().unwrap() - 2.0 / 3.0).abs() < 1e-6);
        assert_eq!(plot("1/x", -1.0, 1.0, 3, false, false).unwrap()["undefined_points"], 1);
        assert!(plot("x", 1.0, 0.0, 10, false, false).is_err());
        let d = derivative_at("x^3", 2.0, false).unwrap();
        assert!((d["first_derivative"].as_f64().unwrap() - 12.0).abs() < 1e-4);
        let i = integral("sin(x)", 0.0, std::f64::consts::PI, false).unwrap();
        assert!((i["integral"].as_f64().unwrap() - 2.0).abs() < 1e-8);
        assert!(integral("1/x", -1.0, 1.0, false).is_err());
        let l = limit("sin(x)/x", 0.0, false).unwrap();
        assert_eq!(l["status"], "exists");
        assert_eq!(l["limit"], 1.0);
        assert_eq!(limit("1/x", 0.0, false).unwrap()["status"], "diverges (±∞)");
    }
}
