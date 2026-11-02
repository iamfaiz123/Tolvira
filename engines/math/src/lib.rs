//! # Dev Toolbox Math Engine (`math-engine`)
//!
//! Precision: `f64` (IEEE-754 double) for evaluation and statistics with
//! results rounded to 12 significant decimals where noted; exact big
//! integers for factorials, combinatorics, gcd/lcm and base conversion;
//! exact rational arithmetic (i128) for fractions. Trigonometry honours the
//! `degrees` flag. Overflow yields `∞`/errors rather than wrapping.
//!
//! | Action | Tool(s) |
//! |---|---|
//! | `evaluate` | scientific-calculator, basic-calculator, algebra-calculator |
//! | `fraction` / `decimal_to_fraction` | fraction-calculator |
//! | `percentage` / `ratio` | percentage-calculator, percentage-change-calculator, ratio-calculator |
//! | `statistics` / `frequency` | average, mean-median-mode, standard-deviation, variance, statistics-calculator |
//! | `prime_check` / `prime_factorization` / `gcd_lcm` | prime tools, gcd/lcm calculators |
//! | `combinatorics` / `factorial` / `probability` | permutation/combination/probability calculators |
//! | `quadratic` / `linear_equation` / `linear_system` / `numeric_root` | equation solvers |
//! | `matrix` | matrix-calculator, matrix-multiplication, determinant-calculator, matrix-inverse |
//! | `trigonometry` / `triangle` | unit-circle, trigonometry, triangle, pythagorean calculators |
//! | `plot` / `derivative` / `integral` / `limit` | equation-graph-data, graph-plotter, derivative/integral/limit calculators (numeric) |
//! | `base_convert` / `binary_calc` | base-converter, binary-calculator, binary/hex/octal converters |
//! | `roman` / `number_to_words` / `sequence` / `scientific_notation` | roman-numeral-converter, number-to-words, sequence-generator, scientific-notation-converter |
//! | `random_numbers` | random-number-generator (seeded by caller entropy when provided) |

pub mod algebra;
pub mod expr;
pub mod numbers;

use engine_shared::{rng, EngineError, EngineResult};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

fn default_points() -> usize {
    200
}
fn default_bins() -> usize {
    10
}
fn default_ten() -> u32 {
    10
}
fn default_two() -> u32 {
    2
}
fn default_bits() -> u32 {
    32
}
fn default_count() -> usize {
    10
}
fn default_sig() -> usize {
    4
}
fn default_max_den() -> i128 {
    10_000
}
fn default_one() -> usize {
    1
}

/// Every action understood by the math engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum MathAction {
    /// Evaluates an arithmetic expression with functions, constants and variables.
    Evaluate {
        /// Expression source.
        expression: String,
        /// Variable values.
        #[serde(default)]
        variables: HashMap<String, f64>,
        /// Trigonometric functions take degrees.
        #[serde(default)]
        degrees: bool,
        /// Decimal places in the formatted result.
        #[serde(default)]
        precision: Option<usize>,
    },
    /// Fraction arithmetic (`a op b`) or simplification.
    Fraction {
        /// First fraction (`3/4`, `1 1/2`, `0.75`).
        a: String,
        /// `+`, `-`, `*`, `/` or `simplify`.
        #[serde(default)]
        op: String,
        /// Second fraction.
        #[serde(default)]
        b: Option<String>,
    },
    /// Converts a decimal to the nearest fraction.
    DecimalToFraction {
        /// Decimal value.
        value: f64,
        /// Largest denominator allowed.
        #[serde(default = "default_max_den")]
        max_denominator: i128,
    },
    /// Percentage calculators.
    Percentage {
        /// `of`, `is_what_percent`, `change`, `add`, `subtract`, `reverse` or `difference`.
        #[serde(default)]
        kind: String,
        /// First operand.
        a: f64,
        /// Second operand.
        b: f64,
    },
    /// Simplifies a ratio and optionally scales it.
    Ratio {
        /// Ratio terms.
        values: Vec<f64>,
        /// Value to scale the ratio to.
        #[serde(default)]
        scale_to: Option<f64>,
        /// Index of the term that should equal `scale_to`.
        #[serde(default)]
        scale_index: usize,
    },
    /// Descriptive statistics.
    Statistics {
        /// Sample values.
        #[serde(default)]
        values: Vec<f64>,
        /// Alternative: whitespace/comma separated numbers.
        #[serde(default)]
        text: Option<String>,
        /// Population (instead of sample) variance.
        #[serde(default)]
        population: bool,
    },
    /// Frequency table and histogram.
    Frequency {
        /// Sample values.
        #[serde(default)]
        values: Vec<f64>,
        /// Alternative: whitespace/comma separated numbers.
        #[serde(default)]
        text: Option<String>,
        /// Histogram bins.
        #[serde(default = "default_bins")]
        bins: usize,
    },
    /// Primality test (up to 128-bit, deterministic Miller-Rabin).
    PrimeCheck {
        /// Integer as a string.
        number: String,
    },
    /// Prime factorisation.
    PrimeFactorization {
        /// Integer as a string.
        number: String,
    },
    /// GCD and LCM of several integers.
    GcdLcm {
        /// Integers as strings.
        numbers: Vec<String>,
    },
    /// Combinations and permutations.
    Combinatorics {
        /// Set size.
        n: u64,
        /// Selection size.
        r: u64,
        /// Allow repetition.
        #[serde(default)]
        repetition: bool,
    },
    /// Exact factorial (big integer).
    Factorial {
        /// Non-negative integer.
        n: u64,
    },
    /// Probability calculators.
    Probability {
        /// `simple`, `binomial` or `normal`.
        kind: String,
        /// Success probability (binomial).
        #[serde(default)]
        p: f64,
        /// Number of trials (binomial).
        #[serde(default)]
        n: u64,
        /// Number of successes (binomial).
        #[serde(default)]
        k: u64,
        /// Favourable outcomes (simple).
        #[serde(default)]
        favorable: f64,
        /// Total outcomes (simple).
        #[serde(default)]
        total: f64,
        /// Mean (normal).
        #[serde(default)]
        mean: f64,
        /// Standard deviation (normal).
        #[serde(default = "default_one_f")]
        std_dev: f64,
        /// Value to evaluate (normal).
        #[serde(default)]
        x: f64,
    },
    /// Quadratic equation solver.
    Quadratic {
        /// Coefficient of x².
        a: f64,
        /// Coefficient of x.
        b: f64,
        /// Constant term.
        c: f64,
    },
    /// Solves a linear equation in one variable.
    LinearEquation {
        /// Equation such as `3x + 2 = 11`.
        equation: String,
    },
    /// Solves a system of linear equations.
    LinearSystem {
        /// Coefficient matrix.
        matrix: Vec<Vec<f64>>,
        /// Right-hand side.
        constants: Vec<f64>,
    },
    /// Finds a root of `f(x) = 0` in `[lo, hi]` (bisection / Newton).
    NumericRoot {
        /// Expression in `x`.
        expression: String,
        /// Lower bracket.
        #[serde(default = "neg_ten")]
        lo: f64,
        /// Upper bracket.
        #[serde(default = "ten")]
        hi: f64,
    },
    /// Matrix operations.
    Matrix {
        /// `add`, `subtract`, `multiply`, `scalar`, `transpose`, `determinant`, `inverse`, `rank`,
        /// `trace` or `power`.
        op: String,
        /// First matrix.
        a: Vec<Vec<f64>>,
        /// Second matrix (binary operations).
        #[serde(default)]
        b: Option<Vec<Vec<f64>>>,
        /// Scalar for `scalar`/`power`.
        #[serde(default)]
        scalar: Option<f64>,
    },
    /// Trigonometric values for an angle.
    Trigonometry {
        /// Angle value.
        angle: f64,
        /// `degrees` (default), `radians` or `gradians`.
        #[serde(default)]
        unit: String,
    },
    /// Triangle solver.
    Triangle {
        /// Known sides (`a`, `b`, `c`; 0 = unknown).
        #[serde(default)]
        sides: Vec<f64>,
        /// Known angles in degrees (0 = unknown).
        #[serde(default)]
        angles: Vec<f64>,
    },
    /// Samples an expression for plotting (plus optional derivative).
    Plot {
        /// Expression in `x`.
        expression: String,
        /// Start of the x range.
        #[serde(default = "neg_ten")]
        x_min: f64,
        /// End of the x range.
        #[serde(default = "ten")]
        x_max: f64,
        /// Number of samples.
        #[serde(default = "default_points")]
        points: usize,
        /// Trigonometric functions take degrees.
        #[serde(default)]
        degrees: bool,
        /// Also sample the numerical derivative.
        #[serde(default)]
        derivative: bool,
    },
    /// Numerical derivative at a point.
    Derivative {
        /// Expression in `x`.
        expression: String,
        /// Evaluation point.
        x: f64,
        /// Trigonometric functions take degrees.
        #[serde(default)]
        degrees: bool,
    },
    /// Numerical definite integral.
    Integral {
        /// Expression in `x`.
        expression: String,
        /// Lower bound.
        from: f64,
        /// Upper bound.
        to: f64,
        /// Trigonometric functions take degrees.
        #[serde(default)]
        degrees: bool,
    },
    /// Numerical limit.
    Limit {
        /// Expression in `x`.
        expression: String,
        /// Point to approach.
        x0: f64,
        /// Trigonometric functions take degrees.
        #[serde(default)]
        degrees: bool,
    },
    /// Converts between number bases 2–36.
    BaseConvert {
        /// Digits in the source base.
        value: String,
        /// Source base.
        #[serde(default = "default_ten")]
        from_base: u32,
        /// Target base.
        #[serde(default = "default_two")]
        to_base: u32,
    },
    /// Bitwise / integer arithmetic on binary, hex or decimal literals.
    BinaryCalc {
        /// First operand.
        a: String,
        /// `and`, `or`, `xor`, `not`, `shl`, `shr`, `add`, `sub`, `mul`, `div` or `mod`.
        op: String,
        /// Second operand.
        #[serde(default)]
        b: Option<String>,
        /// Word size in bits (8–128).
        #[serde(default = "default_bits")]
        bits: u32,
    },
    /// Roman numeral ↔ integer.
    Roman {
        /// Roman numeral or integer.
        value: String,
    },
    /// Number → English words.
    NumberToWords {
        /// Numeric value.
        value: f64,
        /// British style (`and`, long scale names).
        #[serde(default)]
        british: bool,
        /// Currency code for money wording (e.g. `USD`).
        #[serde(default)]
        currency: Option<String>,
    },
    /// Generates numeric sequences.
    Sequence {
        /// `arithmetic` (default), `geometric`, `fibonacci`, `primes`, `triangular`, `squares`,
        /// `cubes`.
        #[serde(default)]
        kind: String,
        /// Number of terms.
        #[serde(default = "default_count")]
        count: usize,
        /// First term.
        #[serde(default)]
        start: f64,
        /// Common difference / ratio.
        #[serde(default = "default_one_f")]
        step: f64,
    },
    /// Scientific / engineering notation.
    ScientificNotation {
        /// Numeric value.
        value: f64,
        /// Significant figures.
        #[serde(default = "default_sig")]
        significant: usize,
    },
    /// Random numbers (needs caller entropy).
    RandomNumbers {
        /// Lower bound.
        min: i64,
        /// Upper bound.
        max: i64,
        /// Number of values.
        #[serde(default = "default_one")]
        count: usize,
        /// Disallow repeats.
        #[serde(default)]
        unique: bool,
        /// Decimal places for non-integer values.
        #[serde(default)]
        decimals: Option<u8>,
    },
}

fn neg_ten() -> f64 {
    -10.0
}
fn ten() -> f64 {
    10.0
}
fn default_one_f() -> f64 {
    1.0
}

fn values_from(values: Vec<f64>, text: Option<&str>) -> EngineResult<Vec<f64>> {
    if !values.is_empty() {
        return Ok(values);
    }
    match text {
        Some(t) if !t.trim().is_empty() => algebra::parse_numbers(t),
        _ => Err(EngineError::empty("values")),
    }
}

fn normal_cdf(z: f64) -> f64 {
    0.5 * (1.0 + erf(z / std::f64::consts::SQRT_2))
}

fn erf(x: f64) -> f64 {
    // Abramowitz & Stegun 7.1.26
    let t = 1.0 / (1.0 + 0.3275911 * x.abs());
    let y = 1.0
        - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t
            + 0.254829592)
            * t
            * (-x * x).exp();
    if x >= 0.0 {
        y
    } else {
        -y
    }
}

fn probability(
    kind: &str,
    p: f64,
    n: u64,
    k: u64,
    favorable: f64,
    total: f64,
    mean: f64,
    std_dev: f64,
    x: f64,
) -> EngineResult<Value> {
    Ok(match kind {
        "simple" | "basic" => {
            if total <= 0.0 || favorable < 0.0 || favorable > total {
                return Err(EngineError::invalid("0 <= favorable <= total and total > 0"));
            }
            let pr = favorable / total;
            json!({
                "probability": pr,
                "percent": pr * 100.0,
                "odds_for": format!("{}:{}", expr::fmt_num(favorable), expr::fmt_num(total - favorable)),
                "complement": 1.0 - pr
            })
        },
        "binomial" => {
            if !(0.0..=1.0).contains(&p) || k > n || n > 10_000 {
                return Err(EngineError::invalid("Need 0 <= p <= 1, k <= n <= 10000"));
            }
            let pmf = |j: u64| {
                numbers::combinations_f(n as f64, j as f64).unwrap_or(0.0)
                    * p.powi(j as i32)
                    * (1.0 - p).powi((n - j) as i32)
            };
            let exactly = pmf(k);
            let at_most: f64 = (0..=k).map(pmf).sum();
            json!({
                "exactly_k": exactly,
                "at_most_k": at_most,
                "at_least_k": 1.0 - at_most + exactly,
                "mean": n as f64 * p,
                "variance": n as f64 * p * (1.0 - p),
                "std_dev": (n as f64 * p * (1.0 - p)).sqrt()
            })
        },
        "normal" | "z" => {
            if std_dev <= 0.0 {
                return Err(EngineError::invalid("std_dev must be > 0"));
            }
            let z = (x - mean) / std_dev;
            let cdf = normal_cdf(z);
            json!({
                "z_score": z,
                "cdf": cdf,
                "p_less_than_x": cdf,
                "p_greater_than_x": 1.0 - cdf,
                "pdf": (-(z * z) / 2.0).exp() / (std_dev * (2.0 * std::f64::consts::PI).sqrt()),
                "percentile": cdf * 100.0
            })
        },
        "poisson" => {
            if mean <= 0.0 || k > 1000 {
                return Err(EngineError::invalid("mean (λ) must be > 0 and k <= 1000"));
            }
            let pmf = |j: u64| {
                (-mean).exp() * mean.powi(j as i32)
                    / expr::factorial(j as f64).unwrap_or(f64::INFINITY)
            };
            let at_most: f64 = (0..=k).map(pmf).sum();
            json!({
                "exactly_k": pmf(k),
                "at_most_k": at_most,
                "at_least_k": 1.0 - at_most + pmf(k),
                "mean": mean,
                "variance": mean
            })
        },
        "independent_and" => json!({
            "probability": p * favorable,
            "description": "P(A and B) = P(A)·P(B)"
        }),
        "either_or" => json!({
            "probability": p + favorable - p * favorable,
            "description": "P(A or B) = P(A) + P(B) − P(A)·P(B) for independent events"
        }),
        "at_least_once" => {
            if !(0.0..=1.0).contains(&p) {
                return Err(EngineError::invalid("0 <= p <= 1"));
            }
            json!({
                "probability": 1.0 - (1.0 - p).powi(n as i32),
                "description": format!("1 − (1 − p)^{n}")
            })
        },
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown probability kind `{other}` (simple, binomial, normal, poisson, \
                 independent_and, either_or, at_least_once)"
            )))
        },
    })
}

/// Dispatches a [`MathAction`] to its implementation.
pub fn handle_action(action: MathAction) -> EngineResult<Value> {
    match action {
        MathAction::Evaluate {
            expression,
            variables,
            degrees,
            precision,
        } => {
            let ctx = expr::Context { vars: &variables, degrees };
            let v = expr::eval(&expression, &ctx)?;
            let rounded = match precision {
                Some(p) => {
                    let f = 10f64.powi(p.min(15) as i32);
                    (v * f).round() / f
                },
                None => v,
            };
            Ok(json!({
                "result": if rounded.is_finite() { json!(rounded) } else { Value::Null },
                "formatted": expr::fmt_num(rounded),
                "scientific": if rounded.is_finite() && rounded != 0.0 { json!(format!("{:e}", rounded)) } else { Value::Null },
                "is_integer": rounded.is_finite() && rounded.fract() == 0.0,
                "expression": expression
            }))
        },
        MathAction::Fraction { a, op, b } => numbers::fraction_calc(&a, &op, b.as_deref()),
        MathAction::DecimalToFraction { value, max_denominator } => {
            numbers::decimal_to_fraction(value, max_denominator)
        },
        MathAction::Percentage { kind, a, b } => numbers::percentage(&kind, a, b),
        MathAction::Ratio { values, scale_to, scale_index } => {
            numbers::ratio(&values, scale_to, scale_index)
        },
        MathAction::Statistics { values, text, population } => {
            algebra::statistics(&values_from(values, text.as_deref())?, population)
        },
        MathAction::Frequency { values, text, bins } => {
            algebra::frequency(&values_from(values, text.as_deref())?, bins)
        },
        MathAction::PrimeCheck { number } => numbers::prime_check(&number),
        MathAction::PrimeFactorization { number } => numbers::prime_factorization(&number),
        MathAction::GcdLcm { numbers } => numbers::gcd_lcm(&numbers),
        MathAction::Combinatorics { n, r, repetition } => numbers::combinatorics(n, r, repetition),
        MathAction::Factorial { n } => numbers::factorial_big(n),
        MathAction::Probability {
            kind,
            p,
            n,
            k,
            favorable,
            total,
            mean,
            std_dev,
            x,
        } => probability(&kind, p, n, k, favorable, total, mean, std_dev, x),
        MathAction::Quadratic { a, b, c } => algebra::quadratic(a, b, c),
        MathAction::LinearEquation { equation } => algebra::linear_equation(&equation),
        MathAction::LinearSystem { matrix, constants } => {
            algebra::linear_system(&matrix, &constants)
        },
        MathAction::NumericRoot { expression, lo, hi } => {
            algebra::numeric_root(&expression, lo, hi)
        },
        MathAction::Matrix { op, a, b, scalar } => algebra::matrix_op(&op, &a, b.as_ref(), scalar),
        MathAction::Trigonometry { angle, unit } => algebra::trigonometry(angle, &unit),
        MathAction::Triangle { sides, angles } => algebra::triangle(&sides, &angles),
        MathAction::Plot {
            expression,
            x_min,
            x_max,
            points,
            degrees,
            derivative,
        } => algebra::plot(&expression, x_min, x_max, points, degrees, derivative),
        MathAction::Derivative { expression, x, degrees } => {
            algebra::derivative_at(&expression, x, degrees)
        },
        MathAction::Integral { expression, from, to, degrees } => {
            algebra::integral(&expression, from, to, degrees)
        },
        MathAction::Limit { expression, x0, degrees } => algebra::limit(&expression, x0, degrees),
        MathAction::BaseConvert { value, from_base, to_base } => {
            numbers::base_convert(&value, from_base, to_base)
        },
        MathAction::BinaryCalc { a, op, b, bits } => {
            numbers::binary_calc(&a, &op, b.as_deref(), bits)
        },
        MathAction::Roman { value } => numbers::roman(&value),
        MathAction::NumberToWords { value, british, currency } => {
            numbers::number_to_words(value, british, currency.as_deref())
        },
        MathAction::Sequence { kind, count, start, step } => {
            numbers::sequence(&kind, count, start, step)
        },
        MathAction::ScientificNotation { value, significant } => {
            numbers::scientific_notation(value, significant)
        },
        MathAction::RandomNumbers { min, max, count, unique, decimals } => {
            if min > max {
                return Err(EngineError::invalid("`min` must be <= `max`"));
            }
            if !(1..=100_000).contains(&count) {
                return Err(EngineError::invalid("`count` must be between 1 and 100000"));
            }
            let values: Vec<Value> = match decimals {
                Some(d) if d > 0 => (0..count)
                    .map(|_| {
                        let f = 10f64.powi(d.min(10) as i32);
                        json!(((min as f64 + rng::next_f64() * (max - min) as f64) * f).round() / f)
                    })
                    .collect(),
                _ => {
                    if unique && (count as i128) > (max as i128 - min as i128 + 1) {
                        return Err(EngineError::invalid(
                            "Range too small for that many unique values",
                        ));
                    }
                    let mut seen = std::collections::HashSet::new();
                    let mut out = Vec::with_capacity(count);
                    while out.len() < count {
                        let v = rng::range_inclusive(min, max);
                        if unique && !seen.insert(v) {
                            continue;
                        }
                        out.push(json!(v));
                    }
                    out
                },
            };
            let sum: f64 = values.iter().filter_map(|v| v.as_f64()).sum();
            Ok(json!({
                "values": values,
                "count": count,
                "sum": sum,
                "secure": rng::has_caller_entropy()
            }))
        },
    }
}

engine_shared::export_engine!(handle_action, MathAction);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probability_kinds() {
        assert!(
            (probability("binomial", 0.5, 10, 5, 0.0, 0.0, 0.0, 1.0, 0.0).unwrap()["exactly_k"]
                .as_f64()
                .unwrap()
                - 0.24609375)
                .abs()
                < 1e-9
        );
        assert!(
            (probability("normal", 0.0, 0, 0, 0.0, 0.0, 0.0, 1.0, 1.96).unwrap()["cdf"]
                .as_f64()
                .unwrap()
                - 0.975)
                .abs()
                < 1e-3
        );
        assert_eq!(
            probability("simple", 0.0, 0, 0, 1.0, 4.0, 0.0, 1.0, 0.0).unwrap()["percent"],
            25.0
        );
        assert!(probability("simple", 0.0, 0, 0, 5.0, 4.0, 0.0, 1.0, 0.0).is_err());
        assert!(
            probability("poisson", 0.0, 0, 2, 0.0, 0.0, 3.0, 1.0, 0.0).unwrap()["exactly_k"]
                .as_f64()
                .unwrap()
                > 0.22
        );
        assert!(probability("what", 0.0, 0, 0, 0.0, 0.0, 0.0, 1.0, 0.0).is_err());
    }

    #[test]
    fn evaluate_and_random() {
        let r = handle_action(MathAction::Evaluate {
            expression: "2^10 / 4".into(),
            variables: HashMap::new(),
            degrees: false,
            precision: None,
        })
        .unwrap();
        assert_eq!(r["result"], 256.0);
        assert_eq!(r["formatted"], "256");
        let r = handle_action(MathAction::Evaluate {
            expression: "1/3".into(),
            variables: HashMap::new(),
            degrees: false,
            precision: Some(3),
        })
        .unwrap();
        assert_eq!(r["result"], 0.333);
        let rn = handle_action(MathAction::RandomNumbers {
            min: 1,
            max: 3,
            count: 3,
            unique: true,
            decimals: None,
        })
        .unwrap();
        assert_eq!(rn["sum"], 6.0);
        assert!(handle_action(MathAction::RandomNumbers {
            min: 1,
            max: 2,
            count: 3,
            unique: true,
            decimals: None
        })
        .is_err());
    }
}
