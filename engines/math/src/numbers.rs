//! Number theory, combinatorics, fractions, percentages, ratios, bases,
//! roman numerals, number words and sequences.

use engine_shared::{EngineError, EngineResult};
use num_bigint::BigUint;
use num_integer::Integer;
use num_traits::{One, ToPrimitive, Zero};
use serde_json::{json, Value};

/// Greatest common divisor (Euclid).
pub fn gcd_u(a: u128, b: u128) -> u128 {
    let (mut a, mut b) = (a, b);
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

/// Deterministic Miller-Rabin primality test for `u128`.
pub fn is_prime(n: u128) -> bool {
    if n < 2 {
        return false;
    }
    for p in [2u128, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        if n.is_multiple_of(p) {
            return n == p;
        }
    }
    // Deterministic Miller-Rabin for n < 3.3e24 with these bases.
    let d = (n - 1) >> (n - 1).trailing_zeros();
    let s = (n - 1).trailing_zeros();
    'outer: for a in [2u128, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        let mut x = mod_pow(a, d, n);
        if x == 1 || x == n - 1 {
            continue;
        }
        for _ in 1..s {
            x = mul_mod(x, x, n);
            if x == n - 1 {
                continue 'outer;
            }
        }
        return false;
    }
    true
}

fn mul_mod(a: u128, b: u128, m: u128) -> u128 {
    if a < (1 << 63) && b < (1 << 63) {
        return (a * b) % m;
    }
    // Schoolbook to avoid overflow.
    let mut result = 0u128;
    let mut a = a % m;
    let mut b = b;
    while b > 0 {
        if b & 1 == 1 {
            result = (result + a) % m;
        }
        a = (a << 1) % m;
        b >>= 1;
    }
    result
}

fn mod_pow(mut base: u128, mut exp: u128, m: u128) -> u128 {
    let mut result = 1u128;
    base %= m;
    while exp > 0 {
        if exp & 1 == 1 {
            result = mul_mod(result, base, m);
        }
        base = mul_mod(base, base, m);
        exp >>= 1;
    }
    result
}

fn parse_u128(s: &str) -> EngineResult<u128> {
    s.trim().replace([',', '_', ' '], "").parse::<u128>().map_err(|_| {
        EngineError::invalid(format!("`{s}` is not a non-negative integer (max 38 digits)"))
    })
}

/// Primality report for a decimal string (next/previous primes included).
pub fn prime_check(n: &str) -> EngineResult<Value> {
    let v = parse_u128(n)?;
    let prime = is_prime(v);
    let next = {
        let mut c = v + 1;
        let mut guard = 0;
        while !is_prime(c) && guard < 10_000 {
            c += 1;
            guard += 1;
        }
        c
    };
    let prev = if v > 2 {
        let mut c = v - 1;
        while c > 1 && !is_prime(c) {
            c -= 1;
        }
        Some(c)
    } else {
        None
    };
    let smallest_factor = if prime || v < 2 {
        None
    } else {
        (2..=100_000u128)
            .find(|d| v % d == 0)
            .or_else(|| factorize(v).first().map(|(p, _)| *p))
    };
    Ok(json!({
        "number": v.to_string(),
        "is_prime": prime,
        "next_prime": next.to_string(),
        "previous_prime": prev.map(|p| p.to_string()),
        "smallest_factor": smallest_factor.map(|f| f.to_string())
    }))
}

/// Trial division + Pollard-rho factorisation into `(prime, exponent)` pairs.
pub fn factorize(mut n: u128) -> Vec<(u128, u32)> {
    let mut out = Vec::new();
    if n < 2 {
        return out;
    }
    let mut d = 2u128;
    while d * d <= n && d < 10_000_000 {
        let mut c = 0;
        while n.is_multiple_of(d) {
            n /= d;
            c += 1;
        }
        if c > 0 {
            out.push((d, c));
        }
        d += if d == 2 { 1 } else { 2 };
    }
    if n > 1 {
        if is_prime(n) {
            out.push((n, 1));
        } else {
            // Pollard rho for the remaining composite.
            let f = pollard_rho(n);
            let mut rest = vec![f, n / f];
            rest.sort();
            for r in rest {
                for (p, c) in factorize(r) {
                    match out.iter_mut().find(|(q, _)| *q == p) {
                        Some(e) => e.1 += c,
                        None => out.push((p, c)),
                    }
                }
            }
            out.sort();
        }
    }
    out
}

fn pollard_rho(n: u128) -> u128 {
    if n.is_multiple_of(2) {
        return 2;
    }
    let mut c = 1u128;
    loop {
        let (mut x, mut y, mut d) = (2u128, 2u128, 1u128);
        let f = |v: u128| (mul_mod(v, v, n) + c) % n;
        let mut iter = 0;
        while d == 1 && iter < 1_000_000 {
            x = f(x);
            y = f(f(y));
            d = gcd_u(x.abs_diff(y), n);
            iter += 1;
        }
        if d != n && d != 1 {
            return d;
        }
        c += 1;
        if c > 50 {
            return n;
        }
    }
}

/// Factorisation report for a decimal string.
pub fn prime_factorization(n: &str) -> EngineResult<Value> {
    let v = parse_u128(n)?;
    if v < 2 {
        return Err(EngineError::invalid("Enter an integer >= 2"));
    }
    let factors = factorize(v);
    let expr = factors
        .iter()
        .map(|(p, c)| if *c == 1 { p.to_string() } else { format!("{p}^{c}") })
        .collect::<Vec<_>>()
        .join(" × ");
    let divisor_count: u128 = factors.iter().map(|(_, c)| (*c as u128) + 1).product();
    let divisors: Option<Vec<String>> = if divisor_count <= 512 {
        Some(all_divisors(&factors).into_iter().map(|d| d.to_string()).collect())
    } else {
        None
    };
    let sum_divisors: Option<String> = divisors.as_ref().map(|d| {
        d.iter()
            .map(|x| x.parse::<u128>().unwrap_or(0))
            .fold(0u128, |a, b| a.saturating_add(b))
            .to_string()
    });
    let perfect = sum_divisors
        .as_ref()
        .and_then(|s| s.parse::<u128>().ok())
        .map(|s| s == 2 * v);
    Ok(json!({
        "number": v.to_string(),
        "factors": factors.iter().map(|(p, c)| json!({ "prime": p.to_string(), "exponent": c })).collect::<Vec<_>>(),
        "expression": expr,
        "is_prime": factors.len() == 1 && factors[0].1 == 1,
        "divisor_count": divisor_count.to_string(),
        "divisors": divisors,
        "sum_of_divisors": sum_divisors,
        "is_perfect": perfect,
    }))
}

fn all_divisors(factors: &[(u128, u32)]) -> Vec<u128> {
    let mut divs = vec![1u128];
    for (p, c) in factors {
        let mut next = Vec::new();
        for d in &divs {
            let mut pp = 1u128;
            for _ in 0..=*c {
                next.push(d.saturating_mul(pp));
                pp = pp.saturating_mul(*p);
            }
        }
        divs = next;
    }
    divs.sort();
    divs.dedup();
    divs
}

/// GCD and LCM of several decimal strings (big integers).
pub fn gcd_lcm(numbers: &[String]) -> EngineResult<Value> {
    if numbers.len() < 2 {
        return Err(EngineError::invalid("Provide at least two integers"));
    }
    let vals: Vec<BigUint> = numbers
        .iter()
        .map(|s| {
            s.trim()
                .replace([',', '_'], "")
                .parse::<BigUint>()
                .map_err(|_| EngineError::invalid(format!("`{s}` is not a non-negative integer")))
        })
        .collect::<EngineResult<_>>()?;
    let gcd = vals.iter().fold(BigUint::zero(), |a, b| a.gcd(b));
    let lcm = if gcd.is_zero() {
        BigUint::zero()
    } else {
        vals.iter().fold(BigUint::one(), |a, b| a.lcm(b))
    };
    // Extended Euclid for two numbers (Bezout coefficients) when small.
    let bezout = if vals.len() == 2 {
        match (vals[0].to_i128(), vals[1].to_i128()) {
            (Some(a), Some(b)) => {
                let (mut old_r, mut r) = (a, b);
                let (mut old_s, mut s) = (1i128, 0i128);
                let (mut old_t, mut t) = (0i128, 1i128);
                while r != 0 {
                    let q = old_r / r;
                    (old_r, r) = (r, old_r - q * r);
                    (old_s, s) = (s, old_s - q * s);
                    (old_t, t) = (t, old_t - q * t);
                }
                Some(json!({
                    "x": old_s.to_string(),
                    "y": old_t.to_string(),
                    "equation": format!("{a}·({old_s}) + {b}·({old_t}) = {old_r}")
                }))
            },
            _ => None,
        }
    } else {
        None
    };
    Ok(json!({
        "gcd": gcd.to_string(),
        "lcm": lcm.to_string(),
        "coprime": gcd == BigUint::one(),
        "bezout": bezout,
        "count": vals.len()
    }))
}

/// `C(n, r)` as a float (exact for small values, logarithmic for large).
pub fn combinations_f(n: f64, r: f64) -> EngineResult<f64> {
    if n < 0.0 || r < 0.0 || n.fract() != 0.0 || r.fract() != 0.0 {
        return Err(EngineError::invalid("n and r must be non-negative integers"));
    }
    if r > n {
        return Ok(0.0);
    }
    let r = r.min(n - r);
    let mut result = 1.0;
    for i in 0..r as u64 {
        result = result * (n - i as f64) / (i as f64 + 1.0);
    }
    Ok(result.round())
}

/// `P(n, r)` as a float.
pub fn permutations_f(n: f64, r: f64) -> EngineResult<f64> {
    if n < 0.0 || r < 0.0 || n.fract() != 0.0 || r.fract() != 0.0 {
        return Err(EngineError::invalid("n and r must be non-negative integers"));
    }
    if r > n {
        return Ok(0.0);
    }
    let mut result = 1.0;
    for i in 0..r as u64 {
        result *= n - i as f64;
    }
    Ok(result)
}

fn big_factorial(n: u64) -> BigUint {
    (1..=n).fold(BigUint::one(), |a, b| a * BigUint::from(b))
}

/// Exact permutations / combinations with big integers.
pub fn combinatorics(n: u64, r: u64, repetition: bool) -> EngineResult<Value> {
    if n > 5000 || r > 5000 {
        return Err(EngineError::invalid("n and r must be <= 5000"));
    }
    let (perm, comb) = if repetition {
        (
            BigUint::from(n).pow(r as u32),
            if n == 0 && r > 0 {
                BigUint::zero()
            } else {
                big_factorial(n + r.saturating_sub(1))
                    / (big_factorial(r) * big_factorial(n.saturating_sub(1)))
            },
        )
    } else if r > n {
        (BigUint::zero(), BigUint::zero())
    } else {
        (
            big_factorial(n) / big_factorial(n - r),
            big_factorial(n) / (big_factorial(r) * big_factorial(n - r)),
        )
    };
    Ok(json!({
        "n": n,
        "r": r,
        "repetition": repetition,
        "permutations": perm.to_string(),
        "combinations": comb.to_string(),
        "n_factorial": if n <= 500 { big_factorial(n).to_string() } else { format!("{} digits", big_factorial(n).to_string().len()) },
        "formula": if repetition { "P = n^r, C = (n+r-1)! / (r!(n-1)!)" } else { "\
            P = n!/(n-r)!, C = n!/(r!(n-r)!)" },
    }))
}

/// Exact factorial as a decimal string (big integer).
pub fn factorial_big(n: u64) -> EngineResult<Value> {
    if n > 5000 {
        return Err(EngineError::invalid("n must be <= 5000"));
    }
    let f = big_factorial(n);
    let s = f.to_string();
    Ok(json!({
        "n": n,
        "factorial": s,
        "digits": s.len(),
        "scientific": if s.len() > 15 { format!("{}.{}e{}", &s[..1], &s[1..7], s.len() - 1) } else { s.clone() }
    }))
}

// --------------------------------------------------------------- fractions

/// Exact rational number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frac {
    /// Numerator (sign carrier).
    pub num: i128,
    /// Denominator (always positive).
    pub den: i128,
}

impl Frac {
    /// Builds a reduced fraction (`E_INVALID_INPUT` for a zero denominator).
    pub fn new(num: i128, den: i128) -> EngineResult<Frac> {
        if den == 0 {
            return Err(EngineError::invalid("Denominator cannot be zero"));
        }
        let g = gcd_u(num.unsigned_abs(), den.unsigned_abs()) as i128;
        let g = if g == 0 { 1 } else { g };
        let sign = if den < 0 { -1 } else { 1 };
        Ok(Frac { num: sign * num / g, den: sign * den / g })
    }
    /// Parses `a/b`, `w a/b`, decimals and integers.
    pub fn parse(s: &str) -> EngineResult<Frac> {
        let t = s.trim();
        if t.is_empty() {
            return Err(EngineError::empty("fraction"));
        }
        // Mixed number "1 2/3" or "-1 2/3".
        if let Some((whole, frac)) = t.split_once(' ') {
            if frac.contains('/') {
                let w: i128 = whole
                    .trim()
                    .parse()
                    .map_err(|_| EngineError::invalid(format!("Invalid mixed number `{s}`")))?;
                let f = Frac::parse(frac)?;
                let sign = if w < 0 || whole.trim().starts_with('-') { -1 } else { 1 };
                return Frac::new(w * f.den + sign * f.num, f.den);
            }
        }
        if let Some((n, d)) = t.split_once('/') {
            let n: i128 = n
                .trim()
                .parse()
                .map_err(|_| EngineError::invalid(format!("Invalid numerator in `{s}`")))?;
            let d: i128 = d
                .trim()
                .parse()
                .map_err(|_| EngineError::invalid(format!("Invalid denominator in `{s}`")))?;
            return Frac::new(n, d);
        }
        if let Some(p) = t.strip_suffix('%') {
            let v: f64 = p
                .trim()
                .parse()
                .map_err(|_| EngineError::invalid(format!("Invalid percentage `{s}`")))?;
            return Frac::from_f64(v / 100.0);
        }
        let v: f64 = t
            .parse()
            .map_err(|_| EngineError::invalid(format!("`{s}` is not a fraction or number")))?;
        Frac::from_f64(v)
    }
    /// Converts a float to an exact fraction (continued fractions).
    pub fn from_f64(v: f64) -> EngineResult<Frac> {
        if !v.is_finite() {
            return Err(EngineError::invalid("Value must be finite"));
        }
        // Continued fraction approximation with a bounded denominator.
        let sign = if v < 0.0 { -1 } else { 1 };
        let mut x = v.abs();
        let (mut h1, mut h0, mut k1, mut k0) = (1i128, 0i128, 0i128, 1i128);
        for _ in 0..64 {
            let a = x.floor();
            let (h2, k2) = (a as i128 * h1 + h0, a as i128 * k1 + k0);
            if k2 > 1_000_000_000 {
                break;
            }
            h0 = h1;
            h1 = h2;
            k0 = k1;
            k1 = k2;
            let frac = x - a;
            if frac.abs() < 1e-12 || (h1 as f64 / k1 as f64 - v.abs()).abs() < 1e-12 {
                break;
            }
            x = 1.0 / frac;
        }
        Frac::new(sign * h1, k1)
    }
    /// Float value.
    pub fn to_f64(self) -> f64 {
        self.num as f64 / self.den as f64
    }
    /// Sum.
    pub fn add(self, o: Frac) -> EngineResult<Frac> {
        Frac::new(self.num * o.den + o.num * self.den, self.den * o.den)
    }
    /// Difference.
    pub fn sub(self, o: Frac) -> EngineResult<Frac> {
        Frac::new(self.num * o.den - o.num * self.den, self.den * o.den)
    }
    /// Product.
    pub fn mul(self, o: Frac) -> EngineResult<Frac> {
        Frac::new(self.num * o.num, self.den * o.den)
    }
    /// Quotient (`E_INVALID_INPUT` when dividing by zero).
    pub fn div(self, o: Frac) -> EngineResult<Frac> {
        if o.num == 0 {
            return Err(EngineError::invalid("Division by zero"));
        }
        Frac::new(self.num * o.den, self.den * o.num)
    }
    /// JSON form with mixed-number and decimal representations.
    pub fn to_json(self) -> Value {
        let whole = self.num / self.den;
        let rem = (self.num % self.den).abs();
        let mixed = if rem == 0 {
            whole.to_string()
        } else if whole == 0 {
            format!("{}{}/{}", if self.num < 0 { "-" } else { "" }, rem, self.den)
        } else {
            format!("{whole} {rem}/{}", self.den)
        };
        json!({
            "numerator": self.num.to_string(),
            "denominator": self.den.to_string(),
            "fraction": if self.den == 1 { self.num.to_string() } else { format!("{}/{}", self.num, self.den) },
            "mixed": mixed,
            "decimal": self.to_f64(),
            "percent": self.to_f64() * 100.0
        })
    }
}

/// Fraction calculator (`+ - * /` or `simplify`).
pub fn fraction_calc(a: &str, op: &str, b: Option<&str>) -> EngineResult<Value> {
    let x = Frac::parse(a)?;
    let result = match (op, b) {
        ("simplify" | "", _) => x,
        ("+" | "add", Some(b)) => x.add(Frac::parse(b)?)?,
        ("-" | "sub" | "subtract", Some(b)) => x.sub(Frac::parse(b)?)?,
        ("*" | "x" | "×" | "mul" | "multiply", Some(b)) => x.mul(Frac::parse(b)?)?,
        ("/" | "÷" | "div" | "divide", Some(b)) => x.div(Frac::parse(b)?)?,
        ("compare", Some(b)) => {
            let y = Frac::parse(b)?;
            let ord = (x.num * y.den).cmp(&(y.num * x.den));
            return Ok(json!({
                "a": x.to_json(),
                "b": y.to_json(),
                "comparison": match ord { std::cmp::Ordering::Less => "<", std::cmp::Ordering::Equal => "=", std::cmp::Ordering::Greater => ">" }
            }));
        },
        (o, None) if o != "simplify" => {
            return Err(EngineError::invalid("Second operand `b` is required"))
        },
        (o, _) => {
            return Err(EngineError::invalid(format!(
                "Unknown operation `{o}` (+, -, *, /, simplify, compare)"
            )))
        },
    };
    Ok(json!({
        "result": result.to_json(),
        "a": x.to_json(),
        "b": b.map(Frac::parse).transpose()?.map(|f| f.to_json()),
        "operation": op
    }))
}

/// Nearest fraction with a bounded denominator.
pub fn decimal_to_fraction(value: f64, max_denominator: i128) -> EngineResult<Value> {
    let f = Frac::from_f64(value)?;
    let f = if f.den > max_denominator.max(1) {
        // Rescale to the nearest fraction with bounded denominator.
        let best = (1..=max_denominator.max(1))
            .map(|d| (d, (value * d as f64).round() as i128))
            .min_by(|a, b| {
                ((a.1 as f64 / a.0 as f64) - value)
                    .abs()
                    .total_cmp(&((b.1 as f64 / b.0 as f64) - value).abs())
            })
            .unwrap_or((1, value.round() as i128));
        Frac::new(best.1, best.0)?
    } else {
        f
    };
    let mut v = f.to_json();
    v["error"] = json!((f.to_f64() - value).abs());
    Ok(v)
}

// ------------------------------------------------------------- percentages

/// Percentage calculators (`of`, `is_what_percent`, `change`, `add`, `subtract`, `reverse`,
/// `difference`).
pub fn percentage(kind: &str, a: f64, b: f64) -> EngineResult<Value> {
    for v in [a, b] {
        if !v.is_finite() {
            return Err(EngineError::invalid("Inputs must be finite numbers"));
        }
    }
    Ok(match kind {
        "of" | "" => json!({ "result": a / 100.0 * b, "description": format!("{a}% of {b}") }),
        "is_what_percent" | "ratio" => {
            if b == 0.0 {
                return Err(EngineError::invalid("Cannot divide by zero"));
            }
            json!({ "result": a / b * 100.0, "description": format!("{a} is what % of {b}") })
        },
        "change" | "increase" | "decrease" => {
            if a == 0.0 {
                return Err(EngineError::invalid("Original value cannot be zero"));
            }
            let change = (b - a) / a.abs() * 100.0;
            json!({
                "result": change,
                "difference": b - a,
                "direction": if change > 0.0 { "increase" } else if change < 0.0 { "decrease" } else { "no change" },
                "description": format!("change from {a} to {b}")
            })
        },
        "add" | "increase_by" => {
            json!({
                "result": a * (1.0 + b / 100.0),
                "description": format!("{a} increased by {b}%")
            })
        },
        "subtract" | "decrease_by" => {
            json!({
                "result": a * (1.0 - b / 100.0),
                "description": format!("{a} decreased by {b}%")
            })
        },
        "reverse" | "original" => {
            if b <= -100.0 {
                return Err(EngineError::invalid("Percentage must be greater than -100"));
            }
            json!({
                "result": a / (1.0 + b / 100.0),
                "description": format!("{a} is the result after {b}%: original value")
            })
        },
        "difference" => {
            let avg = (a.abs() + b.abs()) / 2.0;
            if avg == 0.0 {
                return Err(EngineError::invalid("Values cannot both be zero"));
            }
            json!({
                "result": (a - b).abs() / avg * 100.0,
                "description": "percentage difference (relative to the average)"
            })
        },
        other => return Err(EngineError::invalid(format!("Unknown percentage kind `{other}`"))),
    })
}

/// Simplifies a ratio and optionally scales it so one term equals `scale_to`.
pub fn ratio(values: &[f64], scale_to: Option<f64>, scale_index: usize) -> EngineResult<Value> {
    if values.len() < 2 {
        return Err(EngineError::invalid("Provide at least two values"));
    }
    if values.iter().any(|v| !v.is_finite() || *v < 0.0) {
        return Err(EngineError::invalid("Values must be non-negative finite numbers"));
    }
    // Scale to integers for the simplified ratio.
    let scale = 10f64.powi(
        values
            .iter()
            .map(|v| {
                let s = format!("{v}");
                s.split('.').nth(1).map(|d| d.len()).unwrap_or(0)
            })
            .max()
            .unwrap_or(0)
            .min(9) as i32,
    );
    let ints: Vec<u128> = values.iter().map(|v| (v * scale).round() as u128).collect();
    let g = ints.iter().copied().fold(0u128, gcd_u);
    let simplified: Vec<u128> = if g == 0 {
        ints.clone()
    } else {
        ints.iter().map(|v| v / g).collect()
    };
    let total: f64 = values.iter().sum();
    let scaled = scale_to.map(|target| {
        let idx = scale_index.min(values.len() - 1);
        let factor = if values[idx] == 0.0 { 0.0 } else { target / values[idx] };
        values.iter().map(|v| v * factor).collect::<Vec<_>>()
    });
    Ok(json!({
        "simplified": simplified.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(":"),
        "values": simplified,
        "decimal": if values[1] != 0.0 { json!(values[0] / values[1]) } else { Value::Null },
        "percentages": values.iter().map(|v| if total > 0.0 { v / total * 100.0 } else { 0.0 }).collect::<Vec<_>>(),
        "scaled": scaled,
    }))
}

// ------------------------------------------------------------------ bases

/// Converts digits between bases 2–36 (fractions supported).
pub fn base_convert(value: &str, from: u32, to: u32) -> EngineResult<Value> {
    if !(2..=36).contains(&from) || !(2..=36).contains(&to) {
        return Err(EngineError::invalid("Bases must be between 2 and 36"));
    }
    let t = value.trim().replace([' ', '_', ','], "");
    if t.is_empty() {
        return Err(EngineError::empty("value"));
    }
    let (neg, digits) = match t.strip_prefix('-') {
        Some(d) => (true, d.to_string()),
        None => (false, t.clone()),
    };
    let digits = match from {
        16 => digits.trim_start_matches("0x").trim_start_matches("0X").to_string(),
        2 => digits.trim_start_matches("0b").trim_start_matches("0B").to_string(),
        8 => digits.trim_start_matches("0o").trim_start_matches("0O").to_string(),
        _ => digits,
    };
    let (int_part, frac_part) = match digits.split_once('.') {
        Some((i, f)) => (i.to_string(), Some(f.to_string())),
        None => (digits.clone(), None),
    };
    let int_val = if int_part.is_empty() {
        BigUint::zero()
    } else {
        BigUint::parse_bytes(int_part.to_lowercase().as_bytes(), from).ok_or_else(|| {
            EngineError::invalid(format!("`{value}` is not a valid base-{from} number"))
        })?
    };
    let mut out = int_val.to_str_radix(to);
    // Fractional part (approximate, up to 20 digits).
    if let Some(f) = frac_part {
        let mut frac = 0.0f64;
        for (i, c) in f.chars().enumerate() {
            let d = c.to_digit(from).ok_or_else(|| {
                EngineError::invalid(format!("Invalid digit `{c}` for base {from}"))
            })? as f64;
            frac += d / (from as f64).powi(i as i32 + 1);
        }
        let mut fd = String::new();
        let mut x = frac;
        for _ in 0..20 {
            if x == 0.0 {
                break;
            }
            x *= to as f64;
            let d = x.floor() as u32;
            fd.push(std::char::from_digit(d, to).unwrap_or('0'));
            x -= d as f64;
        }
        if !fd.is_empty() {
            out.push('.');
            out.push_str(&fd);
        }
    }
    let decimal = int_val.to_string();
    let common = |b: u32| -> String {
        let s = int_val.to_str_radix(b);
        if neg {
            format!("-{s}")
        } else {
            s
        }
    };
    Ok(json!({
        "result": format!("{}{}", if neg { "-" } else { "" }, out.to_uppercase()),
        "from_base": from,
        "to_base": to,
        "decimal": if neg { format!("-{decimal}") } else { decimal },
        "binary": common(2),
        "octal": common(8),
        "hexadecimal": common(16).to_uppercase(),
        "bits": int_val.bits(),
    }))
}

/// Bitwise/integer arithmetic on binary, hex or decimal literals with a fixed word size.
pub fn binary_calc(a: &str, op: &str, b: Option<&str>, bits: u32) -> EngineResult<Value> {
    if ![8, 16, 32, 64].contains(&bits) {
        return Err(EngineError::invalid("`bits` must be 8, 16, 32 or 64"));
    }
    let parse = |s: &str| -> EngineResult<u64> {
        let t = s.trim().to_lowercase();
        let (digits, radix) = if let Some(h) = t.strip_prefix("0x") {
            (h.to_string(), 16)
        } else if let Some(b) = t.strip_prefix("0b") {
            (b.to_string(), 2)
        } else if let Some(o) = t.strip_prefix("0o") {
            (o.to_string(), 8)
        } else if t.chars().all(|c| c == '0' || c == '1' || c == ' ') && t.len() >= 4 {
            (t.replace(' ', ""), 2)
        } else {
            (t.clone(), 10)
        };
        u64::from_str_radix(&digits, radix).map_err(|_| {
            EngineError::invalid(format!(
                "`{s}` is not a valid number (use decimal, 0x.., 0b.., 0o..)"
            ))
        })
    };
    let mask: u64 = if bits == 64 { u64::MAX } else { (1u64 << bits) - 1 };
    let x = parse(a)? & mask;
    let y = b.map(parse).transpose()?.map(|v| v & mask);
    let need_y = || y.ok_or_else(|| EngineError::invalid("Second operand `b` is required"));
    let (result, symbol): (u64, String) = match op.to_lowercase().as_str() {
        "and" | "&" => (x & need_y()?, "AND".into()),
        "or" | "|" => (x | need_y()?, "OR".into()),
        "xor" | "^" => (x ^ need_y()?, "XOR".into()),
        "not" | "~" => (!x & mask, "NOT".into()),
        "shl" | "<<" => (x.checked_shl(need_y()? as u32).unwrap_or(0) & mask, "<<".into()),
        "shr" | ">>" => (x.checked_shr(need_y()? as u32).unwrap_or(0), ">>".into()),
        "add" | "+" => (x.wrapping_add(need_y()?) & mask, "+".into()),
        "sub" | "-" => (x.wrapping_sub(need_y()?) & mask, "-".into()),
        "mul" | "*" => (x.wrapping_mul(need_y()?) & mask, "*".into()),
        "div" | "/" => {
            let d = need_y()?;
            if d == 0 {
                return Err(EngineError::invalid("Division by zero"));
            }
            (x / d, "/".into())
        },
        "mod" | "%" => {
            let d = need_y()?;
            if d == 0 {
                return Err(EngineError::invalid("Modulo by zero"));
            }
            (x % d, "%".into())
        },
        "rol" => (x.rotate_left(need_y()? as u32 % bits) & mask, "ROL".into()),
        "ror" => (x.rotate_right(need_y()? as u32 % bits) & mask, "ROR".into()),
        other => return Err(EngineError::invalid(format!("Unknown operation `{other}`"))),
    };
    let signed = if bits == 64 {
        result as i64
    } else if result & (1u64 << (bits - 1)) != 0 {
        result as i64 - (1i64 << bits)
    } else {
        result as i64
    };
    let width = bits as usize;
    Ok(json!({
        "result": {
            "decimal": result,
            "signed": signed,
            "binary": format!("{:0width$b}", result),
            "hex": format!("0x{:0w$X}", result, w = width / 4),
            "octal": format!("0o{:o}", result),
            "bits_set": result.count_ones()
        },
        "a": { "decimal": x, "binary": format!("{:0width$b}", x) },
        "b": y.map(|v| json!({ "decimal": v, "binary": format!("{:0width$b}", v) })),
        "operation": symbol,
        "bits": bits,
    }))
}

// ------------------------------------------------------------------ roman

const ROMAN: &[(u32, &str)] = &[
    (1000, "M"),
    (900, "CM"),
    (500, "D"),
    (400, "CD"),
    (100, "C"),
    (90, "XC"),
    (50, "L"),
    (40, "XL"),
    (10, "X"),
    (9, "IX"),
    (5, "V"),
    (4, "IV"),
    (1, "I"),
];

/// Integer → Roman numeral (1–3999).
pub fn to_roman(mut n: u32) -> EngineResult<String> {
    if n == 0 || n > 3999 {
        return Err(EngineError::invalid("Roman numerals cover 1 to 3999"));
    }
    let mut out = String::new();
    for (v, s) in ROMAN {
        while n >= *v {
            out.push_str(s);
            n -= v;
        }
    }
    Ok(out)
}

/// Roman numeral → integer.
pub fn from_roman(s: &str) -> EngineResult<u32> {
    let t = s.trim().to_uppercase();
    if t.is_empty() {
        return Err(EngineError::empty("roman numeral"));
    }
    let val = |c: char| -> EngineResult<u32> {
        Ok(match c {
            'I' => 1,
            'V' => 5,
            'X' => 10,
            'L' => 50,
            'C' => 100,
            'D' => 500,
            'M' => 1000,
            other => {
                return Err(EngineError::invalid(format!(
                    "`{other}` is not a roman numeral character"
                )))
            },
        })
    };
    let chars: Vec<char> = t.chars().collect();
    let mut total = 0u32;
    for i in 0..chars.len() {
        let v = val(chars[i])?;
        let next = chars.get(i + 1).map(|c| val(*c)).transpose()?.unwrap_or(0);
        if v < next {
            total = total
                .checked_sub(v)
                .ok_or_else(|| EngineError::invalid("Invalid roman numeral"))?;
        } else {
            total += v;
        }
    }
    // Validate canonical form.
    if to_roman(total)? != t {
        return Err(EngineError::invalid(format!(
            "`{s}` is not a valid (canonical) roman numeral"
        )));
    }
    Ok(total)
}

/// Converts in either direction depending on the input.
pub fn roman(input: &str) -> EngineResult<Value> {
    let t = input.trim();
    if let Ok(n) = t.parse::<u32>() {
        return Ok(json!({ "input": t, "roman": to_roman(n)?, "number": n }));
    }
    let n = from_roman(t)?;
    Ok(json!({ "input": t, "number": n, "roman": to_roman(n)? }))
}

// ------------------------------------------------------------------ words

const ONES: &[&str] = &[
    "",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];
const TENS: &[&str] = &[
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];
const SCALES: &[&str] =
    &["", "thousand", "million", "billion", "trillion", "quadrillion", "quintillion"];

fn words_below_1000(n: u64, british: bool) -> String {
    let mut parts = Vec::new();
    let h = n / 100;
    let rest = n % 100;
    if h > 0 {
        parts.push(format!("{} hundred", ONES[h as usize]));
    }
    if rest > 0 {
        if h > 0 && british {
            parts.push("and".into());
        }
        if rest < 20 {
            parts.push(ONES[rest as usize].into());
        } else {
            let t = TENS[(rest / 10) as usize];
            let o = ONES[(rest % 10) as usize];
            parts.push(if o.is_empty() { t.into() } else { format!("{t}-{o}") });
        }
    }
    parts.join(" ")
}

/// Spells a number in English (optionally British, optionally as currency).
pub fn number_to_words(value: f64, british: bool, currency: Option<&str>) -> EngineResult<Value> {
    if !value.is_finite() || value.abs() >= 1e18 {
        return Err(EngineError::invalid("Value must be finite and below 10^18"));
    }
    let neg = value < 0.0;
    let int = value.abs().trunc() as u64;
    let frac = ((value.abs() - value.abs().trunc()) * 100.0).round() as u64;
    let int_words = if int == 0 {
        "zero".to_string()
    } else {
        let mut groups = Vec::new();
        let mut n = int;
        let mut scale = 0;
        while n > 0 {
            let g = n % 1000;
            if g > 0 {
                let w = words_below_1000(g, british);
                groups.push(if SCALES[scale].is_empty() {
                    w
                } else {
                    format!("{w} {}", SCALES[scale])
                });
            }
            n /= 1000;
            scale += 1;
        }
        groups.reverse();
        groups.join(" ")
    };
    let words = match currency {
        Some(cur) if !cur.is_empty() => {
            let cur_lower = cur.to_lowercase();
            let (major, minor) = match cur_lower.as_str() {
                "usd" | "$" | "dollar" => ("dollars", "cents"),
                "eur" | "€" | "euro" => ("euros", "cents"),
                "gbp" | "£" | "pound" => ("pounds", "pence"),
                "inr" | "₹" | "rupee" => ("rupees", "paise"),
                "jpy" | "yen" => ("yen", "sen"),
                other => (other, "cents"),
            };
            let mut s = format!("{int_words} {major}");
            if frac > 0 {
                s.push_str(&format!(" and {} {minor}", words_below_1000(frac, false)));
            }
            s
        },
        _ => {
            let mut s = int_words.clone();
            if frac > 0 {
                s.push_str(&format!(
                    " point {}",
                    format!("{frac:02}")
                        .chars()
                        .map(|c| ONES[c.to_digit(10).unwrap_or(0) as usize].to_string())
                        .map(|w| if w.is_empty() { "zero".to_string() } else { w })
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
            s
        },
    };
    let words = if neg { format!("minus {words}") } else { words };
    let mut c = words.chars();
    let capitalized = c
        .next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default();
    let ordinal = if frac == 0 && int > 0 && int < 1_000_000 {
        Some(ordinal_words(&int_words))
    } else {
        None
    };
    Ok(json!({
        "words": capitalized,
        "lowercase": words,
        "ordinal": ordinal,
        "indian_format": if int >= 100_000 { Some(indian_words(int)) } else { None }
    }))
}

fn ordinal_words(w: &str) -> String {
    let mut parts: Vec<String> = w.split(' ').map(str::to_string).collect();
    if let Some(last) = parts.last_mut() {
        let (head, tail) = match last.rsplit_once('-') {
            Some((h, t)) => (format!("{h}-"), t.to_string()),
            None => (String::new(), last.clone()),
        };
        let ord = match tail.as_str() {
            "one" => "first".into(),
            "two" => "second".into(),
            "three" => "third".into(),
            "five" => "fifth".into(),
            "eight" => "eighth".into(),
            "nine" => "ninth".into(),
            "twelve" => "twelfth".into(),
            t if t.ends_with('y') => format!("{}ieth", &t[..t.len() - 1]),
            t => format!("{t}th"),
        };
        *last = format!("{head}{ord}");
    }
    parts.join(" ")
}

fn indian_words(n: u64) -> String {
    let mut parts = Vec::new();
    let crore = n / 10_000_000;
    let lakh = (n / 100_000) % 100;
    let thousand = (n / 1000) % 100;
    let rest = n % 1000;
    if crore > 0 {
        parts.push(format!(
            "{} crore",
            if crore >= 1000 {
                indian_words(crore)
            } else {
                words_below_1000(crore, false)
            }
        ));
    }
    if lakh > 0 {
        parts.push(format!("{} lakh", words_below_1000(lakh, false)));
    }
    if thousand > 0 {
        parts.push(format!("{} thousand", words_below_1000(thousand, false)));
    }
    if rest > 0 {
        parts.push(words_below_1000(rest, false));
    }
    parts.join(" ")
}

// -------------------------------------------------------------- sequences

/// Generates sequences (`arithmetic`, `geometric`, `fibonacci`, `primes`, `triangular`, …).
pub fn sequence(kind: &str, count: usize, start: f64, step: f64) -> EngineResult<Value> {
    if count == 0 || count > 10_000 {
        return Err(EngineError::invalid("`count` must be between 1 and 10000"));
    }
    let terms: Vec<Value> = match kind {
        "arithmetic" | "" => (0..count).map(|i| json!(start + step * i as f64)).collect(),
        "geometric" => (0..count).map(|i| json!(start * step.powi(i as i32))).collect(),
        "fibonacci" => {
            let mut a = BigUint::zero();
            let mut b = BigUint::one();
            let mut out = Vec::new();
            for _ in 0..count {
                out.push(json!(a.to_string()));
                let next = &a + &b;
                a = b;
                b = next;
            }
            out
        },
        "lucas" => {
            let (mut a, mut b) = (2i128, 1i128);
            (0..count)
                .map(|_| {
                    let v = a;
                    let n = a + b;
                    a = b;
                    b = n;
                    json!(v.to_string())
                })
                .collect()
        },
        "primes" => {
            let mut out = Vec::new();
            let mut c = 2u128;
            while out.len() < count {
                if is_prime(c) {
                    out.push(json!(c.to_string()));
                }
                c += 1;
            }
            out
        },
        "squares" => (1..=count).map(|i| json!((i * i) as u64)).collect(),
        "cubes" => (1..=count).map(|i| json!((i * i * i) as u64)).collect(),
        "triangular" => (1..=count).map(|i| json!((i * (i + 1) / 2) as u64)).collect(),
        "factorials" => (0..count)
            .map(|i| json!(big_factorial(i as u64).to_string()))
            .collect(),
        "powers_of_two" => (0..count)
            .map(|i| json!(BigUint::from(2u32).pow(i as u32).to_string()))
            .collect(),
        "collatz" => {
            let mut n = start.max(1.0) as u64;
            let mut out = vec![json!(n)];
            while n != 1 && out.len() < count {
                n = if n.is_multiple_of(2) { n / 2 } else { 3 * n + 1 };
                out.push(json!(n));
            }
            out
        },
        "catalan" => (0..count)
            .map(|i| {
                json!((big_factorial(2 * i as u64)
                    / (big_factorial(i as u64 + 1) * big_factorial(i as u64)))
                .to_string())
            })
            .collect(),
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown sequence `{other}` (arithmetic, geometric, fibonacci, lucas, primes, \
                 squares, cubes, triangular, factorials, powers_of_two, collatz, catalan)"
            )))
        },
    };
    let numeric_sum: Option<f64> = terms
        .iter()
        .map(|t| t.as_f64().or_else(|| t.as_str().and_then(|s| s.parse().ok())))
        .sum::<Option<f64>>();
    Ok(json!({
        "terms": terms,
        "count": terms.len(),
        "sum": numeric_sum.filter(|s| s.is_finite()),
        "kind": if kind.is_empty() { "arithmetic" } else { kind }
    }))
}

/// Scientific and engineering notation with significant figures.
pub fn scientific_notation(value: f64, significant: usize) -> EngineResult<Value> {
    if !value.is_finite() {
        return Err(EngineError::invalid("Value must be finite"));
    }
    let sig = significant.clamp(1, 17);
    if value == 0.0 {
        return Ok(json!({
            "scientific": "0",
            "mantissa": 0.0,
            "exponent": 0,
            "engineering": "0",
            "e_notation": "0e0"
        }));
    }
    let exp = value.abs().log10().floor() as i32;
    let mantissa = value / 10f64.powi(exp);
    let mant_str = format!("{:.*}", sig - 1, mantissa);
    let eng_exp = (exp as f64 / 3.0).floor() as i32 * 3;
    let eng_mant = value / 10f64.powi(eng_exp);
    Ok(json!({
        "scientific": format!("{mant_str} × 10^{exp}"),
        "e_notation": format!("{mant_str}e{exp}"),
        "mantissa": mant_str.parse::<f64>().unwrap_or(mantissa),
        "exponent": exp,
        "engineering": format!("{} × 10^{eng_exp}", crate::expr::fmt_num((eng_mant * 1e6).round() / 1e6)),
        "decimal": crate::expr::fmt_num(value),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primes_and_factors() {
        assert!(
            is_prime(2)
                && is_prime(97)
                && is_prime(1_000_000_007)
                && !is_prime(1)
                && !is_prime(561)
        );
        assert!(is_prime(18446744073709551557)); // largest 64-bit prime
        let p = prime_check("100").unwrap();
        assert_eq!(p["is_prime"], false);
        assert_eq!(p["next_prime"], "101");
        assert_eq!(p["previous_prime"], "97");
        let f = prime_factorization("360").unwrap();
        assert_eq!(f["expression"], "2^3 × 3^2 × 5");
        assert_eq!(f["divisor_count"], "24");
        assert_eq!(prime_factorization("28").unwrap()["is_perfect"], true);
        assert_eq!(factorize(600851475143), vec![(71, 1), (839, 1), (1471, 1), (6857, 1)]);
        assert!(prime_factorization("1").is_err());
        assert!(prime_check("abc").is_err());
        let g = gcd_lcm(&["12".into(), "18".into()]).unwrap();
        assert_eq!(g["gcd"], "6");
        assert_eq!(g["lcm"], "36");
        assert_eq!(g["bezout"]["x"], "-1");
        assert!(gcd_lcm(&["1".into()]).is_err());
    }

    #[test]
    fn combinatorics_and_factorial() {
        let c = combinatorics(5, 2, false).unwrap();
        assert_eq!(c["permutations"], "20");
        assert_eq!(c["combinations"], "10");
        let r = combinatorics(3, 2, true).unwrap();
        assert_eq!(r["permutations"], "9");
        assert_eq!(r["combinations"], "6");
        assert_eq!(combinatorics(2, 5, false).unwrap()["combinations"], "0");
        assert_eq!(factorial_big(20).unwrap()["factorial"], "2432902008176640000");
        assert_eq!(combinations_f(52.0, 5.0).unwrap(), 2598960.0);
    }

    #[test]
    fn fractions() {
        let r = fraction_calc("1/2", "+", Some("1 1/3")).unwrap();
        assert_eq!(r["result"]["fraction"], "11/6");
        assert_eq!(r["result"]["mixed"], "1 5/6");
        assert_eq!(fraction_calc("6/8", "simplify", None).unwrap()["result"]["fraction"], "3/4");
        assert_eq!(fraction_calc("0.75", "", None).unwrap()["result"]["fraction"], "3/4");
        assert_eq!(fraction_calc("25%", "", None).unwrap()["result"]["fraction"], "1/4");
        assert_eq!(fraction_calc("-1/2", "*", Some("2/3")).unwrap()["result"]["fraction"], "-1/3");
        assert_eq!(fraction_calc("1/2", "compare", Some("2/3")).unwrap()["comparison"], "<");
        assert!(fraction_calc("1/0", "", None).is_err());
        assert!(fraction_calc("1/2", "/", Some("0")).is_err());
        assert!(fraction_calc("1/2", "+", None).is_err());
        assert_eq!(decimal_to_fraction(0.333, 10).unwrap()["fraction"], "1/3");
        assert_eq!(decimal_to_fraction(22.0 / 7.0 - 0.00126, 1000).unwrap()["fraction"], "355/113");
    }

    #[test]
    fn percentages_ratios_bases() {
        assert_eq!(percentage("of", 15.0, 200.0).unwrap()["result"], 30.0);
        assert_eq!(percentage("change", 50.0, 75.0).unwrap()["result"], 50.0);
        assert_eq!(percentage("is_what_percent", 30.0, 120.0).unwrap()["result"], 25.0);
        assert!(percentage("change", 0.0, 1.0).is_err());
        let r = ratio(&[16.0, 9.0], Some(1920.0), 0).unwrap();
        assert_eq!(r["simplified"], "16:9");
        assert_eq!(r["scaled"][1], 1080.0);
        assert_eq!(ratio(&[1.5, 0.5], None, 0).unwrap()["simplified"], "3:1");
        assert!(ratio(&[1.0], None, 0).is_err());
        let b = base_convert("FF", 16, 2).unwrap();
        assert_eq!(b["result"], "11111111");
        assert_eq!(b["decimal"], "255");
        assert_eq!(base_convert("-0b1010", 2, 10).unwrap()["result"], "-10");
        assert_eq!(base_convert("10.5", 10, 2).unwrap()["result"], "1010.1");
        assert_eq!(base_convert("zz", 36, 10).unwrap()["result"], "1295");
        assert!(base_convert("12", 2, 10).is_err());
        assert!(base_convert("1", 1, 10).is_err());
        let bc = binary_calc("0b1100", "and", Some("0b1010"), 8).unwrap();
        assert_eq!(bc["result"]["binary"], "00001000");
        assert_eq!(binary_calc("255", "not", None, 8).unwrap()["result"]["decimal"], 0);
        assert_eq!(binary_calc("1", "shl", Some("7"), 8).unwrap()["result"]["signed"], -128);
        assert!(binary_calc("1", "div", Some("0"), 8).is_err());
    }

    #[test]
    fn roman_words_sequences() {
        assert_eq!(to_roman(1994).unwrap(), "MCMXCIV");
        assert_eq!(from_roman("mcmxciv").unwrap(), 1994);
        assert!(from_roman("IIII").is_err());
        assert!(from_roman("ABC").is_err());
        assert!(to_roman(0).is_err());
        assert_eq!(roman("2024").unwrap()["roman"], "MMXXIV");
        let w = number_to_words(1234567.89, false, None).unwrap();
        assert_eq!(
            w["words"],
            "One million two hundred thirty-four thousand five hundred sixty-seven point eight nine"
        );
        assert_eq!(number_to_words(101.0, true, None).unwrap()["lowercase"], "one hundred and one");
        assert_eq!(
            number_to_words(42.5, false, Some("usd")).unwrap()["lowercase"],
            "forty-two dollars and fifty cents"
        );
        assert_eq!(number_to_words(21.0, false, None).unwrap()["ordinal"], "twenty-first");
        assert_eq!(number_to_words(-5.0, false, None).unwrap()["lowercase"], "minus five");
        assert_eq!(
            number_to_words(1500000.0, false, None).unwrap()["indian_format"],
            "fifteen lakh"
        );
        assert_eq!(sequence("fibonacci", 10, 0.0, 0.0).unwrap()["terms"][9], "34");
        assert_eq!(sequence("arithmetic", 4, 1.0, 2.5).unwrap()["sum"], 19.0);
        assert_eq!(sequence("primes", 5, 0.0, 0.0).unwrap()["terms"][4], "11");
        assert_eq!(sequence("collatz", 100, 6.0, 0.0).unwrap()["count"], 9);
        assert!(sequence("nope", 5, 0.0, 0.0).is_err());
        let s = scientific_notation(123456.789, 4).unwrap();
        assert_eq!(s["e_notation"], "1.235e5");
        assert_eq!(s["exponent"], 5);
        assert!(s["engineering"].as_str().unwrap().contains("10^3"));
    }
}
