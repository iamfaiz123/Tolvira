//! A small, safe expression evaluator (shunting-yard → RPN) with functions,
//! constants, variables, implicit multiplication and degree/radian modes.

use engine_shared::{EngineError, EngineResult};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Op(char),
    LParen,
    RParen,
    Comma,
}

fn tokenize(src: &str) -> EngineResult<Vec<Tok>> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit()
            || (c == '.' && chars.get(i + 1).map(|n| n.is_ascii_digit()).unwrap_or(false))
        {
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == '_')
            {
                i += 1;
            }
            if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                let save = i;
                i += 1;
                if i < chars.len() && (chars[i] == '+' || chars[i] == '-') {
                    i += 1;
                }
                if i < chars.len() && chars[i].is_ascii_digit() {
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                } else {
                    i = save;
                }
            }
            let s: String = chars[start..i].iter().filter(|c| **c != '_').collect();
            let v: f64 = s
                .parse()
                .map_err(|_| EngineError::invalid(format!("Invalid number `{s}`")))?;
            out.push(Tok::Num(v));
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push(Tok::Ident(chars[start..i].iter().collect()));
            continue;
        }
        match c {
            '+' | '-' | '*' | '/' | '^' | '%' | '!' => out.push(Tok::Op(c)),
            '×' => out.push(Tok::Op('*')),
            '÷' => out.push(Tok::Op('/')),
            '−' => out.push(Tok::Op('-')),
            '(' | '[' | '{' => out.push(Tok::LParen),
            ')' | ']' | '}' => out.push(Tok::RParen),
            ',' | ';' => out.push(Tok::Comma),
            'π' => out.push(Tok::Ident("pi".into())),
            '√' => out.push(Tok::Ident("sqrt".into())),
            other => {
                return Err(EngineError::invalid(format!(
                    "Unexpected character `{other}` at position {i}"
                )))
            },
        }
        i += 1;
    }
    Ok(out)
}

#[derive(Debug, Clone)]
enum Rpn {
    Num(f64),
    Var(String),
    Op(char),
    Neg,
    Fact,
    Func(String, usize),
}

fn precedence(op: char) -> u8 {
    match op {
        '+' | '-' => 1,
        '*' | '/' | '%' => 2,
        '^' => 4,
        _ => 0,
    }
}

/// Compiles an expression into RPN (with implicit multiplication inserted).
fn compile(src: &str) -> EngineResult<Vec<Rpn>> {
    let raw = tokenize(src)?;
    if raw.is_empty() {
        return Err(EngineError::empty("expression"));
    }
    // Insert implicit multiplication: `2x`, `2(3)`, `)(`, `x y`, `)x`.
    let mut toks: Vec<Tok> = Vec::with_capacity(raw.len());
    for t in raw {
        if let Some(prev) = toks.last() {
            let prev_value = matches!(prev, Tok::Num(_) | Tok::RParen)
                || matches!(prev, Tok::Ident(n) if !is_function(n));
            let next_value = matches!(t, Tok::Num(_) | Tok::LParen | Tok::Ident(_));
            if prev_value
                && next_value
                && !(matches!(prev, Tok::Ident(_)) && matches!(t, Tok::LParen))
            {
                toks.push(Tok::Op('*'));
            }
        }
        toks.push(t);
    }
    let mut output: Vec<Rpn> = Vec::new();
    #[derive(Debug)]
    enum Stack {
        Op(char),
        Neg,
        LParen,
        Func(String),
    }
    let mut stack: Vec<Stack> = Vec::new();
    let mut arg_counts: Vec<usize> = Vec::new();
    let mut prev_is_value = false;
    let mut iter = toks.into_iter().peekable();
    while let Some(t) = iter.next() {
        match t {
            Tok::Num(v) => {
                output.push(Rpn::Num(v));
                prev_is_value = true;
            },
            Tok::Ident(name) => {
                if is_function(&name) && matches!(iter.peek(), Some(Tok::LParen)) {
                    stack.push(Stack::Func(name));
                    prev_is_value = false;
                } else {
                    output.push(Rpn::Var(name));
                    prev_is_value = true;
                }
            },
            Tok::Op('!') => {
                output.push(Rpn::Fact);
                prev_is_value = true;
            },
            Tok::Op(op) => {
                if (op == '-' || op == '+') && !prev_is_value {
                    if op == '-' {
                        stack.push(Stack::Neg);
                    }
                    continue;
                }
                while let Some(top) = stack.last() {
                    let pop = match top {
                        Stack::Neg => op != '^',
                        Stack::Op(o) => {
                            precedence(*o) > precedence(op)
                                || (precedence(*o) == precedence(op) && op != '^')
                        },
                        _ => false,
                    };
                    if !pop {
                        break;
                    }
                    match stack.pop() {
                        Some(Stack::Op(o)) => output.push(Rpn::Op(o)),
                        Some(Stack::Neg) => output.push(Rpn::Neg),
                        // `pop` is only true for operators / unary minus.
                        _ => break,
                    }
                }
                stack.push(Stack::Op(op));
                prev_is_value = false;
            },
            Tok::LParen => {
                let is_func = matches!(stack.last(), Some(Stack::Func(_)));
                stack.push(Stack::LParen);
                if is_func {
                    arg_counts.push(if matches!(iter.peek(), Some(Tok::RParen)) { 0 } else { 1 });
                }
                prev_is_value = false;
            },
            Tok::Comma => {
                loop {
                    match stack.last() {
                        Some(Stack::LParen) => break,
                        Some(_) => match stack.pop() {
                            Some(Stack::Op(o)) => output.push(Rpn::Op(o)),
                            Some(Stack::Neg) => output.push(Rpn::Neg),
                            _ => return Err(EngineError::invalid("Misplaced comma")),
                        },
                        None => return Err(EngineError::invalid("Misplaced comma")),
                    }
                }
                if let Some(c) = arg_counts.last_mut() {
                    *c += 1;
                }
                prev_is_value = false;
            },
            Tok::RParen => {
                loop {
                    match stack.pop() {
                        Some(Stack::LParen) => break,
                        Some(Stack::Op(o)) => output.push(Rpn::Op(o)),
                        Some(Stack::Neg) => output.push(Rpn::Neg),
                        Some(Stack::Func(_)) | None => {
                            return Err(EngineError::invalid("Mismatched parentheses"))
                        },
                    }
                }
                if let Some(Stack::Func(_)) = stack.last() {
                    if let Some(Stack::Func(name)) = stack.pop() {
                        let n = arg_counts.pop().unwrap_or(1);
                        output.push(Rpn::Func(name, n));
                    }
                }
                prev_is_value = true;
            },
        }
    }
    while let Some(s) = stack.pop() {
        match s {
            Stack::Op(o) => output.push(Rpn::Op(o)),
            Stack::Neg => output.push(Rpn::Neg),
            Stack::LParen | Stack::Func(_) => {
                return Err(EngineError::invalid("Mismatched parentheses"))
            },
        }
    }
    Ok(output)
}

fn is_function(name: &str) -> bool {
    matches!(
        name,
        "sin"
            | "cos"
            | "tan"
            | "asin"
            | "acos"
            | "atan"
            | "atan2"
            | "sinh"
            | "cosh"
            | "tanh"
            | "asinh"
            | "acosh"
            | "atanh"
            | "sqrt"
            | "cbrt"
            | "abs"
            | "ln"
            | "log"
            | "log2"
            | "log10"
            | "exp"
            | "floor"
            | "ceil"
            | "round"
            | "trunc"
            | "sign"
            | "min"
            | "max"
            | "pow"
            | "hypot"
            | "fact"
            | "factorial"
            | "deg"
            | "rad"
            | "sec"
            | "csc"
            | "cot"
            | "avg"
            | "mean"
            | "sum"
            | "gcd"
            | "lcm"
            | "mod"
            | "root"
            | "nCr"
            | "nPr"
            | "ncr"
            | "npr"
            | "gamma"
            | "clamp"
    )
}

/// Factorial of a non-negative integer (gamma for non-integers), `E_INVALID_INPUT` on overflow.
pub fn factorial(n: f64) -> EngineResult<f64> {
    if n < 0.0 || n.fract() != 0.0 {
        return Err(EngineError::invalid("Factorial needs a non-negative integer"));
    }
    if n > 170.0 {
        return Ok(f64::INFINITY);
    }
    Ok((1..=n as u64).map(|x| x as f64).product())
}

fn gamma(x: f64) -> f64 {
    // Lanczos approximation.
    if x < 0.5 {
        return std::f64::consts::PI / ((std::f64::consts::PI * x).sin() * gamma(1.0 - x));
    }
    let g = 7.0;
    let coef = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    let x = x - 1.0;
    let mut a = coef[0];
    let t = x + g + 0.5;
    for (i, c) in coef.iter().enumerate().skip(1) {
        a += c / (x + i as f64);
    }
    (2.0 * std::f64::consts::PI).sqrt() * t.powf(x + 0.5) * (-t).exp() * a
}

fn gcd_f(a: f64, b: f64) -> f64 {
    let (mut a, mut b) = (a.abs().round(), b.abs().round());
    while b != 0.0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

/// Evaluation context.
pub struct Context<'a> {
    /// Variable bindings.
    pub vars: &'a HashMap<String, f64>,
    /// Interpret trigonometric arguments as degrees.
    pub degrees: bool,
}

/// Evaluates an expression string.
pub fn eval(src: &str, ctx: &Context) -> EngineResult<f64> {
    let rpn = compile(src)?;
    eval_rpn(&rpn, ctx)
}

/// Pre-compiled expression for repeated evaluation (plots).
pub struct Compiled(Vec<Rpn>);

/// Parses an expression once so it can be evaluated many times.
pub fn compile_expr(src: &str) -> EngineResult<Compiled> {
    Ok(Compiled(compile(src)?))
}

/// Evaluates a pre-compiled expression.
pub fn eval_compiled(c: &Compiled, ctx: &Context) -> EngineResult<f64> {
    eval_rpn(&c.0, ctx)
}

fn eval_rpn(rpn: &[Rpn], ctx: &Context) -> EngineResult<f64> {
    let mut st: Vec<f64> = Vec::with_capacity(16);
    let to_rad = |v: f64| if ctx.degrees { v.to_radians() } else { v };
    let from_rad = |v: f64| if ctx.degrees { v.to_degrees() } else { v };
    for item in rpn {
        match item {
            Rpn::Num(v) => st.push(*v),
            Rpn::Var(name) => {
                let v = match name.as_str() {
                    "pi" | "PI" | "Pi" => std::f64::consts::PI,
                    "e" | "E" => std::f64::consts::E,
                    "tau" | "TAU" => std::f64::consts::TAU,
                    "phi" | "PHI" => (1.0 + 5f64.sqrt()) / 2.0,
                    "inf" | "Infinity" => f64::INFINITY,
                    other => *ctx.vars.get(other).ok_or_else(|| {
                        EngineError::invalid(format!("Unknown variable or function `{other}`"))
                    })?,
                };
                st.push(v);
            },
            Rpn::Neg => {
                let v = st.pop().ok_or_else(|| EngineError::invalid("Syntax error"))?;
                st.push(-v);
            },
            Rpn::Fact => {
                let v = st.pop().ok_or_else(|| EngineError::invalid("Syntax error"))?;
                st.push(factorial(v)?);
            },
            Rpn::Op(op) => {
                let b = st
                    .pop()
                    .ok_or_else(|| EngineError::invalid("Syntax error: missing operand"))?;
                let a = st
                    .pop()
                    .ok_or_else(|| EngineError::invalid("Syntax error: missing operand"))?;
                st.push(match op {
                    '+' => a + b,
                    '-' => a - b,
                    '*' => a * b,
                    '/' => {
                        if b == 0.0 {
                            return Err(EngineError::invalid("Division by zero"));
                        }
                        a / b
                    },
                    '%' => {
                        if b == 0.0 {
                            return Err(EngineError::invalid("Modulo by zero"));
                        }
                        a % b
                    },
                    '^' => a.powf(b),
                    other => {
                        return Err(EngineError::invalid(format!("Unknown operator `{other}`")))
                    },
                });
            },
            Rpn::Func(name, argc) => {
                if st.len() < *argc {
                    return Err(EngineError::invalid(format!("Not enough arguments for {name}()")));
                }
                let args: Vec<f64> = st.split_off(st.len() - argc);
                let one = |args: &[f64]| -> EngineResult<f64> {
                    args.first()
                        .copied()
                        .ok_or_else(|| EngineError::invalid(format!("{name}() needs an argument")))
                };
                let two = |args: &[f64]| -> EngineResult<(f64, f64)> {
                    if args.len() == 2 {
                        Ok((args[0], args[1]))
                    } else {
                        Err(EngineError::invalid(format!("{name}() needs two arguments")))
                    }
                };
                let v = match name.as_str() {
                    "sin" => to_rad(one(&args)?).sin(),
                    "cos" => to_rad(one(&args)?).cos(),
                    "tan" => to_rad(one(&args)?).tan(),
                    "sec" => 1.0 / to_rad(one(&args)?).cos(),
                    "csc" => 1.0 / to_rad(one(&args)?).sin(),
                    "cot" => 1.0 / to_rad(one(&args)?).tan(),
                    "asin" => from_rad(one(&args)?.asin()),
                    "acos" => from_rad(one(&args)?.acos()),
                    "atan" => from_rad(one(&args)?.atan()),
                    "atan2" => {
                        let (y, x) = two(&args)?;
                        from_rad(y.atan2(x))
                    },
                    "sinh" => one(&args)?.sinh(),
                    "cosh" => one(&args)?.cosh(),
                    "tanh" => one(&args)?.tanh(),
                    "asinh" => one(&args)?.asinh(),
                    "acosh" => one(&args)?.acosh(),
                    "atanh" => one(&args)?.atanh(),
                    "sqrt" => one(&args)?.sqrt(),
                    "cbrt" => one(&args)?.cbrt(),
                    "root" => {
                        let (x, n) = two(&args)?;
                        if x < 0.0 && n % 2.0 == 1.0 {
                            -(-x).powf(1.0 / n)
                        } else {
                            x.powf(1.0 / n)
                        }
                    },
                    "abs" => one(&args)?.abs(),
                    "ln" => one(&args)?.ln(),
                    "log" => match args.len() {
                        1 => args[0].log10(),
                        2 => args[1].ln() / args[0].ln(),
                        _ => {
                            return Err(EngineError::invalid(
                                "log() takes 1 or 2 arguments (log(x) = log10, log(base, x))",
                            ))
                        },
                    },
                    "log2" => one(&args)?.log2(),
                    "log10" => one(&args)?.log10(),
                    "exp" => one(&args)?.exp(),
                    "floor" => one(&args)?.floor(),
                    "ceil" => one(&args)?.ceil(),
                    "round" => match args.len() {
                        1 => args[0].round(),
                        2 => {
                            let f = 10f64.powi(args[1] as i32);
                            (args[0] * f).round() / f
                        },
                        _ => return Err(EngineError::invalid("round() takes 1 or 2 arguments")),
                    },
                    "trunc" => one(&args)?.trunc(),
                    "sign" => one(&args)?.signum(),
                    "min" => args.iter().cloned().fold(f64::INFINITY, f64::min),
                    "max" => args.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                    "sum" => args.iter().sum(),
                    "avg" | "mean" => {
                        if args.is_empty() {
                            return Err(EngineError::invalid("avg() needs arguments"));
                        }
                        args.iter().sum::<f64>() / args.len() as f64
                    },
                    "pow" => {
                        let (a, b) = two(&args)?;
                        a.powf(b)
                    },
                    "hypot" => {
                        let (a, b) = two(&args)?;
                        a.hypot(b)
                    },
                    "mod" => {
                        let (a, b) = two(&args)?;
                        a.rem_euclid(b)
                    },
                    "gcd" => {
                        let (a, b) = two(&args)?;
                        gcd_f(a, b)
                    },
                    "lcm" => {
                        let (a, b) = two(&args)?;
                        let g = gcd_f(a, b);
                        if g == 0.0 {
                            0.0
                        } else {
                            (a * b).abs() / g
                        }
                    },
                    "fact" | "factorial" => factorial(one(&args)?)?,
                    "gamma" => gamma(one(&args)?),
                    "ncr" | "nCr" => {
                        let (n, r) = two(&args)?;
                        crate::numbers::combinations_f(n, r)?
                    },
                    "npr" | "nPr" => {
                        let (n, r) = two(&args)?;
                        crate::numbers::permutations_f(n, r)?
                    },
                    "deg" => one(&args)?.to_degrees(),
                    "rad" => one(&args)?.to_radians(),
                    "clamp" => {
                        if args.len() != 3 {
                            return Err(EngineError::invalid(
                                "clamp(x, lo, hi) needs three arguments",
                            ));
                        }
                        args[0].max(args[1]).min(args[2])
                    },
                    other => {
                        return Err(EngineError::invalid(format!("Unknown function `{other}`")))
                    },
                };
                st.push(v);
            },
        }
    }
    if st.len() != 1 {
        return Err(EngineError::invalid(
            "Syntax error: expression does not reduce to a single value",
        ));
    }
    Ok(st[0])
}

/// Formats a float for display (avoids `-0`, trims noise).
pub fn fmt_num(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "∞".into() } else { "-∞".into() };
    }
    let rounded = (v * 1e12).round() / 1e12;
    if rounded == rounded.trunc() && rounded.abs() < 1e15 {
        format!("{}", rounded as i64)
    } else {
        let s = format!("{rounded}");
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(s: &str) -> f64 {
        let vars = HashMap::new();
        eval(s, &Context { vars: &vars, degrees: false }).unwrap()
    }

    #[test]
    fn arithmetic_and_precedence() {
        assert_eq!(ev("1 + 2 * 3"), 7.0);
        assert_eq!(ev("(1 + 2) * 3"), 9.0);
        assert_eq!(ev("2 ^ 3 ^ 2"), 512.0);
        assert_eq!(ev("-2 ^ 2"), -4.0);
        assert_eq!(ev("(-2) ^ 2"), 4.0);
        assert_eq!(ev("10 % 3"), 1.0);
        assert_eq!(ev("5!"), 120.0);
        assert_eq!(ev("2(3+4)"), 14.0);
        assert_eq!(ev("2pi"), 2.0 * std::f64::consts::PI);
        assert_eq!(ev("1e3 + 1_000"), 2000.0);
        assert_eq!(
            ev("sqrt(16) + abs(-2) + max(1, 5, 3) + log(100) + log(2, 8)"),
            4.0 + 2.0 + 5.0 + 2.0 + 3.0
        );
        assert_eq!(ev("round(2.71828, 2)"), 2.72);
        assert_eq!(ev("nCr(5, 2) + nPr(5, 2)"), 30.0);
        assert_eq!(ev("gcd(12, 18) + lcm(4, 6)"), 18.0);
        assert!((ev("sin(pi/2)") - 1.0).abs() < 1e-12);
        assert!((ev("gamma(5)") - 24.0).abs() < 1e-9);
        let vars: HashMap<String, f64> = [("x".to_string(), 3.0)].into();
        assert_eq!(eval("2x + 1", &Context { vars: &vars, degrees: false }).unwrap(), 7.0);
        assert_eq!(eval("sin(90)", &Context { vars: &vars, degrees: true }).unwrap(), 1.0);
    }

    #[test]
    fn errors() {
        let vars = HashMap::new();
        let ctx = Context { vars: &vars, degrees: false };
        assert!(eval("", &ctx).is_err());
        assert!(eval("1 / 0", &ctx).is_err());
        assert!(eval("(1 + 2", &ctx).is_err());
        assert!(eval("1 + 2)", &ctx).is_err());
        assert!(eval("foo(1)", &ctx).is_err());
        assert!(eval("y + 1", &ctx).is_err());
        assert!(eval("1 +", &ctx).is_err());
        assert!(eval("1 $ 2", &ctx).is_err());
        assert!(eval("(-1)!", &ctx).is_err());
        assert_eq!(fmt_num(3.0), "3");
        assert_eq!(fmt_num(-0.0), "0");
        assert_eq!(fmt_num(0.1 + 0.2), "0.3");
    }
}
