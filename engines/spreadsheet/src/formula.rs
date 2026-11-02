//! Spreadsheet formula parser and evaluator.
//!
//! Supports the arithmetic/comparison/concatenation operators, cell and
//! range references (`A1`, `$B$2:C9`, `Sheet2!A1` when a workbook is supplied),
//! percent literals and ~100 common worksheet functions (maths, statistics,
//! text, logical, lookup, date serials, basic finance). Volatile functions
//! that need a clock (`NOW`, `TODAY`, `RAND`) are rejected with `#VOLATILE!`
//! so results are deterministic; callers can pass `today_serial` instead.

use crate::model::{column_letter, fmt_number, parse_cell_ref, Cell, Sheet, Workbook};
use crate::writer::iso_to_serial;
use serde_json::{json, Value as Json};
use std::collections::HashSet;

/// Value produced by the formula evaluator.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Empty cell / no value.
    Empty,
    /// Numeric value (dates are serial numbers).
    Number(f64),
    /// Text value.
    Text(String),
    /// Boolean value.
    Bool(bool),
    /// Spreadsheet error such as `#DIV/0!`.
    Error(String),
    /// 2-D array (rows × cols) produced by a range reference or array literal.
    Array(Vec<Vec<Value>>),
}

impl Value {
    fn err(code: &str) -> Value {
        Value::Error(code.into())
    }

    /// JSON representation (`{type, value}`).
    pub fn to_json(&self) -> Json {
        match self {
            Value::Empty => Json::Null,
            Value::Number(n) => num_json(*n),
            Value::Text(s) => Json::String(s.clone()),
            Value::Bool(b) => Json::Bool(*b),
            Value::Error(e) => json!({ "error": e }),
            Value::Array(rows) => Json::Array(
                rows.iter()
                    .map(|r| Json::Array(r.iter().map(Value::to_json).collect()))
                    .collect(),
            ),
        }
    }

    /// Display text as a spreadsheet would show it.
    pub fn display(&self) -> String {
        match self {
            Value::Empty => String::new(),
            Value::Number(n) => fmt_number(*n),
            Value::Text(s) => s.clone(),
            Value::Bool(b) => {
                if *b {
                    "TRUE".into()
                } else {
                    "FALSE".into()
                }
            },
            Value::Error(e) => e.clone(),
            Value::Array(rows) => rows
                .iter()
                .map(|r| r.iter().map(Value::display).collect::<Vec<_>>().join(", "))
                .collect::<Vec<_>>()
                .join("; "),
        }
    }

    /// `empty`, `number`, `text`, `bool` or `error`.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Empty => "empty",
            Value::Number(_) => "number",
            Value::Text(_) => "text",
            Value::Bool(_) => "boolean",
            Value::Error(_) => "error",
            Value::Array(_) => "array",
        }
    }

    fn from_cell(c: &Cell) -> Value {
        match c {
            Cell::Empty => Value::Empty,
            Cell::Number(n) => Value::Number(*n),
            Cell::Text(s) => Value::Text(s.clone()),
            Cell::Bool(b) => Value::Bool(*b),
            Cell::Date(d) => iso_to_serial(d)
                .map(|(s, _)| Value::Number(s))
                .unwrap_or_else(|| Value::Text(d.clone())),
            Cell::Error(e) => Value::Error(e.clone()),
        }
    }

    /// Coerces to a number the way Excel does in arithmetic.
    fn num(&self) -> Result<f64, Value> {
        match self {
            Value::Empty => Ok(0.0),
            Value::Number(n) => Ok(*n),
            Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
            Value::Text(s) => s
                .trim()
                .trim_end_matches('%')
                .parse::<f64>()
                .map(|v| if s.trim().ends_with('%') { v / 100.0 } else { v })
                .map_err(|_| Value::err("#VALUE!")),
            Value::Error(_) => Err(self.clone()),
            Value::Array(rows) => rows
                .first()
                .and_then(|r| r.first())
                .map(Value::num)
                .unwrap_or(Err(Value::err("#VALUE!"))),
        }
    }

    fn text(&self) -> Result<String, Value> {
        match self {
            Value::Error(_) => Err(self.clone()),
            Value::Array(rows) => rows
                .first()
                .and_then(|r| r.first())
                .map(Value::text)
                .unwrap_or(Ok(String::new())),
            other => Ok(other.display()),
        }
    }

    fn boolean(&self) -> Result<bool, Value> {
        match self {
            Value::Empty => Ok(false),
            Value::Bool(b) => Ok(*b),
            Value::Number(n) => Ok(*n != 0.0),
            Value::Text(s) => match s.trim().to_ascii_uppercase().as_str() {
                "TRUE" => Ok(true),
                "FALSE" => Ok(false),
                _ => Err(Value::err("#VALUE!")),
            },
            Value::Error(_) => Err(self.clone()),
            Value::Array(rows) => rows
                .first()
                .and_then(|r| r.first())
                .map(Value::boolean)
                .unwrap_or(Err(Value::err("#VALUE!"))),
        }
    }

    fn flatten(&self) -> Vec<Value> {
        match self {
            Value::Array(rows) => rows.iter().flatten().cloned().collect(),
            other => vec![other.clone()],
        }
    }
}

/// Whole numbers serialise as integers so JSON consumers see `2` not `2.0`.
pub fn num_json(n: f64) -> Json {
    if n.fract() == 0.0 && n.abs() < 9.007_199_254_740_992e15 {
        json!(n as i64)
    } else {
        json!(n)
    }
}

// ------------------------------------------------------------------ lexer --

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Ident(String),
    /// `A1`, `$A$1`, `Sheet!A1`, `Sheet!A1:B2`, `A1:B2`, `A:A`, `1:1`
    Ref(String),
    Op(String),
    LParen,
    RParen,
    Comma,
    Semicolon,
    LBrace,
    RBrace,
}

fn lex(src: &str) -> Result<Vec<Tok>, Value> {
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit())
        {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            if i < chars.len()
                && (chars[i] == 'e' || chars[i] == 'E')
                && i + 1 < chars.len()
                && (chars[i + 1].is_ascii_digit() || chars[i + 1] == '-' || chars[i + 1] == '+')
            {
                i += 2;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let s: String = chars[start..i].iter().collect();
            // Row range like 1:1 – treat as reference.
            if i < chars.len() && chars[i] == ':' && !s.contains('.') {
                let mut j = i + 1;
                while j < chars.len() && chars[j].is_ascii_digit() {
                    j += 1;
                }
                if j > i + 1 {
                    out.push(Tok::Ref(chars[start..j].iter().collect()));
                    i = j;
                    continue;
                }
            }
            out.push(Tok::Num(s.parse().map_err(|_| Value::err("#ERROR!"))?));
            continue;
        }
        if c == '"' {
            let mut s = String::new();
            i += 1;
            loop {
                if i >= chars.len() {
                    return Err(Value::Error("#ERROR! unterminated string".into()));
                }
                if chars[i] == '"' {
                    if i + 1 < chars.len() && chars[i + 1] == '"' {
                        s.push('"');
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                s.push(chars[i]);
                i += 1;
            }
            out.push(Tok::Str(s));
            continue;
        }
        if c == '\'' {
            // Quoted sheet name: 'My Sheet'!A1
            let mut s = String::new();
            i += 1;
            while i < chars.len() && chars[i] != '\'' {
                s.push(chars[i]);
                i += 1;
            }
            i += 1;
            if i < chars.len() && chars[i] == '!' {
                i += 1;
                let start = i;
                while i < chars.len()
                    && (chars[i].is_ascii_alphanumeric() || chars[i] == '$' || chars[i] == ':')
                {
                    i += 1;
                }
                out.push(Tok::Ref(format!("{s}!{}", chars[start..i].iter().collect::<String>())));
                continue;
            }
            return Err(Value::Error("#ERROR! bad sheet reference".into()));
        }
        if c.is_alphabetic() || c == '_' || c == '$' {
            let start = i;
            while i < chars.len()
                && (chars[i].is_alphanumeric()
                    || chars[i] == '_'
                    || chars[i] == '$'
                    || chars[i] == '.')
            {
                i += 1;
            }
            let mut word: String = chars[start..i].iter().collect();
            if i < chars.len() && chars[i] == '!' {
                i += 1;
                let s2 = i;
                while i < chars.len()
                    && (chars[i].is_ascii_alphanumeric() || chars[i] == '$' || chars[i] == ':')
                {
                    i += 1;
                }
                out.push(Tok::Ref(format!("{word}!{}", chars[s2..i].iter().collect::<String>())));
                continue;
            }
            if i < chars.len() && chars[i] == ':' && (is_ref_like(&word) || is_col_like(&word)) {
                let mut j = i + 1;
                while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '$') {
                    j += 1;
                }
                let second: String = chars[i + 1..j].iter().collect();
                if is_ref_like(&second) || second.bytes().all(|b| b.is_ascii_alphabetic()) {
                    word.push(':');
                    word.push_str(&second);
                    out.push(Tok::Ref(word));
                    i = j;
                    continue;
                }
            }
            if i < chars.len() && chars[i] == '(' {
                out.push(Tok::Ident(word.to_ascii_uppercase()));
            } else if is_ref_like(&word) {
                out.push(Tok::Ref(word));
            } else {
                out.push(Tok::Ident(word.to_ascii_uppercase()));
            }
            continue;
        }
        let two: String = chars[i..(i + 2).min(chars.len())].iter().collect();
        if matches!(two.as_str(), "<=" | ">=" | "<>") {
            out.push(Tok::Op(two));
            i += 2;
            continue;
        }
        match c {
            '+' | '-' | '*' | '/' | '^' | '&' | '=' | '<' | '>' | '%' => {
                out.push(Tok::Op(c.to_string()))
            },
            '(' => out.push(Tok::LParen),
            ')' => out.push(Tok::RParen),
            ',' => out.push(Tok::Comma),
            ';' => out.push(Tok::Semicolon),
            '{' => out.push(Tok::LBrace),
            '}' => out.push(Tok::RBrace),
            other => return Err(Value::Error(format!("#ERROR! unexpected character `{other}`"))),
        }
        i += 1;
    }
    Ok(out)
}

fn is_col_like(s: &str) -> bool {
    let s = s.replace('$', "");
    !s.is_empty() && s.len() <= 3 && s.bytes().all(|b| b.is_ascii_alphabetic())
}

fn is_ref_like(s: &str) -> bool {
    let s = s.replace('$', "");
    let letters: String = s.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let digits = &s[letters.len()..];
    !letters.is_empty()
        && letters.len() <= 3
        && !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
}

// ----------------------------------------------------------------- parser --

#[derive(Debug, Clone)]
enum Expr {
    Lit(Value),
    Ref(String),
    Unary(String, Box<Expr>),
    Binary(String, Box<Expr>, Box<Expr>),
    Percent(Box<Expr>),
    Call(String, Vec<Expr>),
    ArrayLit(Vec<Vec<Expr>>),
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }
    fn expect(&mut self, t: Tok) -> Result<(), Value> {
        if self.peek() == Some(&t) {
            self.pos += 1;
            Ok(())
        } else {
            Err(Value::Error(format!("#ERROR! expected {t:?}")))
        }
    }
    fn parse(&mut self) -> Result<Expr, Value> {
        let e = self.comparison()?;
        if self.pos < self.toks.len() {
            return Err(Value::Error(format!(
                "#ERROR! unexpected token {:?}",
                self.toks[self.pos]
            )));
        }
        Ok(e)
    }
    fn comparison(&mut self) -> Result<Expr, Value> {
        let mut left = self.concat()?;
        while let Some(Tok::Op(op)) = self.peek() {
            if matches!(op.as_str(), "=" | "<>" | "<" | ">" | "<=" | ">=") {
                let op = op.clone();
                self.pos += 1;
                let right = self.concat()?;
                left = Expr::Binary(op, Box::new(left), Box::new(right));
            } else {
                break;
            }
        }
        Ok(left)
    }
    fn concat(&mut self) -> Result<Expr, Value> {
        let mut left = self.additive()?;
        while self.peek() == Some(&Tok::Op("&".into())) {
            self.pos += 1;
            let right = self.additive()?;
            left = Expr::Binary("&".into(), Box::new(left), Box::new(right));
        }
        Ok(left)
    }
    fn additive(&mut self) -> Result<Expr, Value> {
        let mut left = self.term()?;
        while let Some(Tok::Op(op)) = self.peek() {
            if op == "+" || op == "-" {
                let op = op.clone();
                self.pos += 1;
                let right = self.term()?;
                left = Expr::Binary(op, Box::new(left), Box::new(right));
            } else {
                break;
            }
        }
        Ok(left)
    }
    fn term(&mut self) -> Result<Expr, Value> {
        let mut left = self.power()?;
        while let Some(Tok::Op(op)) = self.peek() {
            if op == "*" || op == "/" {
                let op = op.clone();
                self.pos += 1;
                let right = self.power()?;
                left = Expr::Binary(op, Box::new(left), Box::new(right));
            } else {
                break;
            }
        }
        Ok(left)
    }
    fn power(&mut self) -> Result<Expr, Value> {
        let base = self.unary()?;
        if self.peek() == Some(&Tok::Op("^".into())) {
            self.pos += 1;
            let exp = self.power()?;
            return Ok(Expr::Binary("^".into(), Box::new(base), Box::new(exp)));
        }
        Ok(base)
    }
    fn unary(&mut self) -> Result<Expr, Value> {
        if let Some(Tok::Op(op)) = self.peek() {
            if op == "-" || op == "+" {
                let op = op.clone();
                self.pos += 1;
                let inner = self.unary()?;
                return Ok(Expr::Unary(op, Box::new(inner)));
            }
        }
        self.postfix()
    }
    fn postfix(&mut self) -> Result<Expr, Value> {
        let mut e = self.primary()?;
        while self.peek() == Some(&Tok::Op("%".into())) {
            self.pos += 1;
            e = Expr::Percent(Box::new(e));
        }
        Ok(e)
    }
    fn primary(&mut self) -> Result<Expr, Value> {
        match self.next() {
            Some(Tok::Num(n)) => Ok(Expr::Lit(Value::Number(n))),
            Some(Tok::Str(s)) => Ok(Expr::Lit(Value::Text(s))),
            Some(Tok::Ref(r)) => Ok(Expr::Ref(r)),
            Some(Tok::Ident(name)) => {
                if self.peek() == Some(&Tok::LParen) {
                    self.pos += 1;
                    let mut args = Vec::new();
                    if self.peek() != Some(&Tok::RParen) {
                        loop {
                            if matches!(self.peek(), Some(Tok::Comma)) {
                                args.push(Expr::Lit(Value::Empty));
                            } else {
                                args.push(self.comparison()?);
                            }
                            match self.peek() {
                                Some(Tok::Comma) | Some(Tok::Semicolon) => {
                                    self.pos += 1;
                                    if self.peek() == Some(&Tok::RParen) {
                                        args.push(Expr::Lit(Value::Empty));
                                    }
                                },
                                _ => break,
                            }
                        }
                    }
                    self.expect(Tok::RParen)?;
                    Ok(Expr::Call(name, args))
                } else {
                    match name.as_str() {
                        "TRUE" => Ok(Expr::Lit(Value::Bool(true))),
                        "FALSE" => Ok(Expr::Lit(Value::Bool(false))),
                        "PI" => Ok(Expr::Lit(Value::Number(std::f64::consts::PI))),
                        _ => Ok(Expr::Ref(name)),
                    }
                }
            },
            Some(Tok::LParen) => {
                let e = self.comparison()?;
                self.expect(Tok::RParen)?;
                Ok(e)
            },
            Some(Tok::LBrace) => {
                let mut rows = Vec::new();
                let mut row = Vec::new();
                loop {
                    let e = self.comparison()?;
                    row.push(e);
                    match self.next() {
                        Some(Tok::Comma) => {},
                        Some(Tok::Semicolon) => rows.push(std::mem::take(&mut row)),
                        Some(Tok::RBrace) => {
                            rows.push(row);
                            break;
                        },
                        _ => return Err(Value::Error("#ERROR! bad array literal".into())),
                    }
                }
                Ok(Expr::ArrayLit(rows))
            },
            Some(t) => Err(Value::Error(format!("#ERROR! unexpected {t:?}"))),
            None => Err(Value::Error("#ERROR! unexpected end of formula".into())),
        }
    }
}

// -------------------------------------------------------------- evaluator --

/// Evaluation context: a workbook (for sheet references) and the active sheet.
pub struct Context<'a> {
    /// Workbook that sheet references resolve against.
    pub workbook: &'a Workbook,
    /// Index of the active sheet.
    pub sheet: usize,
    /// Serial date used by `TODAY()` / `NOW()` (the engine has no clock).
    pub today_serial: Option<f64>,
    /// Defined names (`name`, `reference`).
    pub names: Vec<(String, String)>,
    /// Cells currently being evaluated (cycle detection).
    stack: HashSet<(usize, usize, usize)>,
    /// Current recursion depth (guards runaway evaluation).
    pub depth: usize,
}

impl<'a> Context<'a> {
    /// Creates a context for `sheet` of `workbook`.
    pub fn new(workbook: &'a Workbook, sheet: usize) -> Self {
        Context {
            workbook,
            sheet,
            today_serial: None,
            names: workbook.defined_names.clone(),
            stack: HashSet::new(),
            depth: 0,
        }
    }

    fn sheet_by_name(&self, name: &str) -> Result<usize, Value> {
        let n = name.trim_matches('\'');
        self.workbook
            .sheets
            .iter()
            .position(|s| s.name.eq_ignore_ascii_case(n))
            .ok_or_else(|| Value::err("#REF!"))
    }

    /// Reads a cell, evaluating its formula if it has one.
    fn cell_value(&mut self, sheet: usize, row: usize, col: usize) -> Value {
        let Some(s) = self.workbook.sheets.get(sheet) else {
            return Value::err("#REF!");
        };
        if let Some(f) = s.formulas.get(&(row, col)) {
            let key = (sheet, row, col);
            if self.stack.contains(&key) {
                return Value::err("#CIRCULAR!");
            }
            if self.depth > 200 {
                return Value::err("#DEPTH!");
            }
            self.stack.insert(key);
            self.depth += 1;
            let saved = self.sheet;
            self.sheet = sheet;
            let v = evaluate_in(f, self);
            self.sheet = saved;
            self.depth -= 1;
            self.stack.remove(&key);
            return v;
        }
        Value::from_cell(s.get(row, col))
    }

    fn resolve_ref(&mut self, r: &str) -> Value {
        let (sheet, local) = match r.split_once('!') {
            Some((s, rest)) => match self.sheet_by_name(s) {
                Ok(i) => (i, rest.to_string()),
                Err(e) => return e,
            },
            None => (self.sheet, r.to_string()),
        };
        // Defined names.
        if parse_cell_ref(&local).is_none() && !local.contains(':') {
            if let Some((_, target)) = self
                .names
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(&local))
                .cloned()
            {
                return self.resolve_ref(target.trim_start_matches('='));
            }
            return Value::Error(format!("#NAME? ({local})"));
        }
        let Some(s) = self.workbook.sheets.get(sheet) else {
            return Value::err("#REF!");
        };
        let (rows, cols) = (s.rows(), s.cols());
        if let Some((a, b)) = local.split_once(':') {
            let (a, b) = (a.replace('$', ""), b.replace('$', ""));
            let (r1, c1, r2, c2) = if let (Some((r1, c1)), Some((r2, c2))) =
                (parse_cell_ref(&a), parse_cell_ref(&b))
            {
                (r1.min(r2), c1.min(c2), r1.max(r2), c1.max(c2))
            } else if let (Some(c1), Some(c2)) =
                (crate::model::column_number(&a), crate::model::column_number(&b))
            {
                (0, c1.min(c2) - 1, rows.saturating_sub(1), c1.max(c2) - 1)
            } else if let (Ok(r1), Ok(r2)) = (a.parse::<usize>(), b.parse::<usize>()) {
                (r1.min(r2) - 1, 0, r1.max(r2) - 1, cols.saturating_sub(1))
            } else {
                return Value::err("#REF!");
            };
            if (r2 - r1 + 1) * (c2 - c1 + 1) > 1_000_000 {
                return Value::err("#LIMIT!");
            }
            let mut out = Vec::with_capacity(r2 - r1 + 1);
            for rr in r1..=r2 {
                let mut row = Vec::with_capacity(c2 - c1 + 1);
                for cc in c1..=c2 {
                    row.push(self.cell_value(sheet, rr, cc));
                }
                out.push(row);
            }
            return Value::Array(out);
        }
        match parse_cell_ref(&local) {
            Some((r, c)) => self.cell_value(sheet, r, c),
            None => Value::err("#REF!"),
        }
    }
}

/// Evaluates a formula string (with or without leading `=`) in a context.
pub fn evaluate_in(formula: &str, ctx: &mut Context) -> Value {
    let src = formula.trim().trim_start_matches('=');
    if src.is_empty() {
        return Value::Empty;
    }
    let toks = match lex(src) {
        Ok(t) => t,
        Err(e) => return e,
    };
    let mut p = Parser { toks, pos: 0 };
    match p.parse() {
        Ok(expr) => eval(&expr, ctx),
        Err(e) => e,
    }
}

fn eval(e: &Expr, ctx: &mut Context) -> Value {
    match e {
        Expr::Lit(v) => v.clone(),
        Expr::Ref(r) => ctx.resolve_ref(r),
        Expr::Unary(op, inner) => {
            let v = eval(inner, ctx);
            match v.num() {
                Ok(n) => Value::Number(if op == "-" { -n } else { n }),
                Err(e) => e,
            }
        },
        Expr::Percent(inner) => match eval(inner, ctx).num() {
            Ok(n) => Value::Number(n / 100.0),
            Err(e) => e,
        },
        Expr::Binary(op, l, r) => {
            let a = eval(l, ctx);
            let b = eval(r, ctx);
            binary(op, &a, &b)
        },
        Expr::ArrayLit(rows) => Value::Array(
            rows.iter()
                .map(|r| r.iter().map(|x| eval(x, ctx)).collect())
                .collect(),
        ),
        Expr::Call(name, args) => call(name, args, ctx),
    }
}

fn binary(op: &str, a: &Value, b: &Value) -> Value {
    if let Value::Error(_) = a {
        return a.clone();
    }
    if let Value::Error(_) = b {
        return b.clone();
    }
    match op {
        "&" => match (a.text(), b.text()) {
            (Ok(x), Ok(y)) => Value::Text(x + &y),
            (Err(e), _) | (_, Err(e)) => e,
        },
        "=" | "<>" | "<" | ">" | "<=" | ">=" => {
            let ord = compare(a, b);
            let res = match op {
                "=" => ord == std::cmp::Ordering::Equal,
                "<>" => ord != std::cmp::Ordering::Equal,
                "<" => ord == std::cmp::Ordering::Less,
                ">" => ord == std::cmp::Ordering::Greater,
                "<=" => ord != std::cmp::Ordering::Greater,
                _ => ord != std::cmp::Ordering::Less,
            };
            Value::Bool(res)
        },
        _ => {
            let (x, y) = match (a.num(), b.num()) {
                (Ok(x), Ok(y)) => (x, y),
                (Err(e), _) | (_, Err(e)) => return e,
            };
            let r = match op {
                "+" => x + y,
                "-" => x - y,
                "*" => x * y,
                "/" => {
                    if y == 0.0 {
                        return Value::err("#DIV/0!");
                    }
                    x / y
                },
                "^" => x.powf(y),
                _ => return Value::err("#ERROR!"),
            };
            if r.is_finite() {
                Value::Number(r)
            } else {
                Value::err("#NUM!")
            }
        },
    }
}

fn compare(a: &Value, b: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;
    let rank = |v: &Value| match v {
        Value::Number(_) | Value::Empty => 0,
        Value::Text(_) => 1,
        Value::Bool(_) => 2,
        _ => 3,
    };
    let (a, b) = (
        if let Value::Array(_) = a {
            a.flatten().into_iter().next().unwrap_or(Value::Empty)
        } else {
            a.clone()
        },
        if let Value::Array(_) = b {
            b.flatten().into_iter().next().unwrap_or(Value::Empty)
        } else {
            b.clone()
        },
    );
    match (&a, &b) {
        (Value::Number(x), Value::Number(y)) => x.partial_cmp(y).unwrap_or(Equal),
        (Value::Empty, Value::Number(y)) => 0.0f64.partial_cmp(y).unwrap_or(Equal),
        (Value::Number(x), Value::Empty) => x.partial_cmp(&0.0).unwrap_or(Equal),
        (Value::Empty, Value::Empty) => Equal,
        (Value::Text(x), Value::Text(y)) => x.to_lowercase().cmp(&y.to_lowercase()),
        (Value::Empty, Value::Text(y)) => "".cmp(y.as_str()),
        (Value::Text(x), Value::Empty) => x.as_str().cmp(""),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        _ => rank(&a).cmp(&rank(&b)),
    }
}

fn numbers_of(args: &[Value], include_text_numbers: bool) -> Result<Vec<f64>, Value> {
    let mut out = Vec::new();
    for a in args {
        match a {
            Value::Array(rows) => {
                for v in rows.iter().flatten() {
                    match v {
                        Value::Number(n) => out.push(*n),
                        Value::Error(_) => return Err(v.clone()),
                        _ => {},
                    }
                }
            },
            Value::Number(n) => out.push(*n),
            Value::Bool(b) => out.push(if *b { 1.0 } else { 0.0 }),
            Value::Text(s) if include_text_numbers => {
                if let Ok(n) = s.trim().parse::<f64>() {
                    out.push(n);
                } else {
                    return Err(Value::err("#VALUE!"));
                }
            },
            Value::Error(_) => return Err(a.clone()),
            _ => {},
        }
    }
    Ok(out)
}

/// Excel criteria matching (`">10"`, `"<>x"`, `"a*"`, `5`).
fn criteria_match(v: &Value, criteria: &Value) -> bool {
    let c = criteria.display();
    let (op, rest) = if let Some(r) = c.strip_prefix("<>") {
        ("<>", r)
    } else if let Some(r) = c.strip_prefix(">=") {
        (">=", r)
    } else if let Some(r) = c.strip_prefix("<=") {
        ("<=", r)
    } else if let Some(r) = c.strip_prefix('>') {
        (">", r)
    } else if let Some(r) = c.strip_prefix('<') {
        ("<", r)
    } else if let Some(r) = c.strip_prefix('=') {
        ("=", r)
    } else {
        ("=", c.as_str())
    };
    let target = if let Ok(n) = rest.trim().parse::<f64>() {
        Value::Number(n)
    } else if rest.eq_ignore_ascii_case("TRUE") {
        Value::Bool(true)
    } else if rest.eq_ignore_ascii_case("FALSE") {
        Value::Bool(false)
    } else {
        Value::Text(rest.to_string())
    };
    if let (Value::Text(pat), "=" | "<>") = (&target, op) {
        if pat.contains('*') || pat.contains('?') {
            let m = wildcard(&pat.to_lowercase(), &v.display().to_lowercase());
            return if op == "=" { m } else { !m };
        }
    }
    // Numbers compared to text criteria never match for = (Excel semantics).
    if let (Value::Text(_), Value::Number(_)) = (&target, v) {
        return op == "<>";
    }
    if let (Value::Number(_), Value::Text(_)) = (&target, v) {
        return op == "<>";
    }
    let ord = compare(v, &target);
    match op {
        "=" => ord == std::cmp::Ordering::Equal,
        "<>" => ord != std::cmp::Ordering::Equal,
        ">" => ord == std::cmp::Ordering::Greater,
        "<" => ord == std::cmp::Ordering::Less,
        ">=" => ord != std::cmp::Ordering::Less,
        _ => ord != std::cmp::Ordering::Greater,
    }
}

fn wildcard(pat: &str, text: &str) -> bool {
    let p: Vec<char> = pat.chars().collect();
    let t: Vec<char> = text.chars().collect();
    fn go(p: &[char], t: &[char]) -> bool {
        match p.first() {
            None => t.is_empty(),
            Some('*') => (0..=t.len()).any(|i| go(&p[1..], &t[i..])),
            Some('?') => !t.is_empty() && go(&p[1..], &t[1..]),
            Some(c) => !t.is_empty() && t[0] == *c && go(&p[1..], &t[1..]),
        }
    }
    go(&p, &t)
}

fn serial_to_ymd(serial: f64) -> (i64, i64, i64) {
    // Inverse of `iso_to_serial` (1900 system with the phantom leap day).
    let mut days = serial.floor() as i64;
    if days < 61 {
        days += 1;
    }
    // Days since 1899-12-30 → days since 1970-01-01 → civil date.
    let z = days - 25_569 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn date_serial(y: f64, m: f64, d: f64) -> Value {
    // Normalise month overflow like Excel.
    let mut y = y as i64;
    let mut m = m as i64;
    y += (m - 1).div_euclid(12);
    m = (m - 1).rem_euclid(12) + 1;
    match iso_to_serial(&format!("{y:04}-{m:02}-01")) {
        Some((s, _)) => Value::Number(s + d - 1.0),
        None => Value::err("#NUM!"),
    }
}

fn round_to(n: f64, digits: f64, mode: &str) -> f64 {
    let f = 10f64.powi(digits as i32);
    let x = n * f;
    let r = match mode {
        "up" => {
            if x >= 0.0 {
                x.ceil()
            } else {
                x.floor()
            }
        },
        "down" => x.trunc(),
        _ => x.round(),
    };
    r / f
}

fn call(name: &str, args: &[Expr], ctx: &mut Context) -> Value {
    // Lazy evaluation for IF / IFERROR / IFS / CHOOSE / SWITCH / AND / OR.
    match name {
        "IF" => {
            if args.is_empty() {
                return Value::err("#N/A");
            }
            let cond = eval(&args[0], ctx);
            return match cond.boolean() {
                Ok(true) => args.get(1).map(|a| eval(a, ctx)).unwrap_or(Value::Bool(true)),
                Ok(false) => args.get(2).map(|a| eval(a, ctx)).unwrap_or(Value::Bool(false)),
                Err(e) => e,
            };
        },
        "IFERROR" | "IFNA" => {
            let v = args.first().map(|a| eval(a, ctx)).unwrap_or(Value::Empty);
            return if matches!(v, Value::Error(_)) {
                args.get(1).map(|a| eval(a, ctx)).unwrap_or(Value::Empty)
            } else {
                v
            };
        },
        "IFS" => {
            for pair in args.chunks(2) {
                if pair.len() < 2 {
                    return Value::err("#N/A");
                }
                match eval(&pair[0], ctx).boolean() {
                    Ok(true) => return eval(&pair[1], ctx),
                    Ok(false) => {},
                    Err(e) => return e,
                }
            }
            return Value::err("#N/A");
        },
        "SWITCH" => {
            if args.len() < 3 {
                return Value::err("#N/A");
            }
            let v = eval(&args[0], ctx);
            let rest = &args[1..];
            for pair in rest.chunks(2) {
                if pair.len() == 2 {
                    if compare(&v, &eval(&pair[0], ctx)) == std::cmp::Ordering::Equal {
                        return eval(&pair[1], ctx);
                    }
                } else {
                    return eval(&pair[0], ctx);
                }
            }
            return Value::err("#N/A");
        },
        "CHOOSE" => {
            let idx = match args.first().map(|a| eval(a, ctx)).unwrap_or(Value::Empty).num() {
                Ok(n) => n as usize,
                Err(e) => return e,
            };
            return args.get(idx).map(|a| eval(a, ctx)).unwrap_or(Value::err("#VALUE!"));
        },
        "NOW" | "RAND" | "RANDBETWEEN" => {
            return Value::Error(format!(
                "#VOLATILE! {name} is not available in deterministic evaluation"
            ))
        },
        "TODAY" => {
            return ctx
                .today_serial
                .map(Value::Number)
                .unwrap_or(Value::Error("#VOLATILE! pass today_serial to evaluate TODAY()".into()))
        },
        _ => {},
    }
    let vals: Vec<Value> = args.iter().map(|a| eval(a, ctx)).collect();
    // Propagate errors for strict functions (not for COUNT-like / IS* functions).
    if !matches!(
        name,
        "COUNT"
            | "COUNTA"
            | "COUNTBLANK"
            | "ISERROR"
            | "ISERR"
            | "ISNA"
            | "ISNUMBER"
            | "ISTEXT"
            | "ISBLANK"
            | "ISLOGICAL"
            | "ERROR.TYPE"
            | "N"
            | "T"
            | "TYPE"
    ) {
        if let Some(e) = vals.iter().find(|v| matches!(v, Value::Error(_))) {
            return e.clone();
        }
    }
    let nums = |strict: bool| numbers_of(&vals, strict);
    let arg_num = |i: usize| -> Result<f64, Value> {
        vals.get(i).map(Value::num).unwrap_or(Err(Value::err("#N/A")))
    };
    let arg_text = |i: usize| -> Result<String, Value> {
        vals.get(i).map(Value::text).unwrap_or(Err(Value::err("#N/A")))
    };
    let opt_num = |i: usize, default: f64| -> Result<f64, Value> {
        match vals.get(i) {
            None | Some(Value::Empty) => Ok(default),
            Some(v) => v.num(),
        }
    };
    macro_rules! try_v {
        ($e:expr) => {
            match $e {
                Ok(v) => v,
                Err(e) => return e,
            }
        };
    }
    match name {
        // ---- maths
        "SUM" => Value::Number(try_v!(nums(true)).iter().sum()),
        "PRODUCT" => Value::Number(try_v!(nums(true)).iter().product()),
        "AVERAGE" => {
            let n = try_v!(nums(true));
            if n.is_empty() {
                Value::err("#DIV/0!")
            } else {
                Value::Number(n.iter().sum::<f64>() / n.len() as f64)
            }
        },
        "MIN" => {
            Value::Number(try_v!(nums(true)).into_iter().fold(f64::INFINITY, f64::min)).pipe(|v| {
                if let Value::Number(n) = v {
                    if n.is_infinite() {
                        Value::Number(0.0)
                    } else {
                        Value::Number(n)
                    }
                } else {
                    v
                }
            })
        },
        "MAX" => Value::Number(try_v!(nums(true)).into_iter().fold(f64::NEG_INFINITY, f64::max))
            .pipe(|v| {
                if let Value::Number(n) = v {
                    if n.is_infinite() {
                        Value::Number(0.0)
                    } else {
                        Value::Number(n)
                    }
                } else {
                    v
                }
            }),
        "COUNT" => Value::Number(
            vals.iter()
                .flat_map(Value::flatten)
                .filter(|v| matches!(v, Value::Number(_)))
                .count() as f64,
        ),
        "COUNTA" => Value::Number(
            vals.iter()
                .flat_map(Value::flatten)
                .filter(|v| !matches!(v, Value::Empty))
                .count() as f64,
        ),
        "COUNTBLANK" => Value::Number(
            vals.iter()
                .flat_map(Value::flatten)
                .filter(|v| {
                    matches!(v, Value::Empty) || matches!(v, Value::Text(s) if s.is_empty())
                })
                .count() as f64,
        ),
        "COUNTIF" => {
            if vals.len() < 2 {
                return Value::err("#N/A");
            }
            Value::Number(
                vals[0]
                    .flatten()
                    .iter()
                    .filter(|v| criteria_match(v, &vals[1]))
                    .count() as f64,
            )
        },
        "COUNTIFS" => {
            if vals.len() < 2 || !vals.len().is_multiple_of(2) {
                return Value::err("#N/A");
            }
            let ranges: Vec<Vec<Value>> = vals.chunks(2).map(|c| c[0].flatten()).collect();
            let n = ranges[0].len();
            Value::Number(
                (0..n)
                    .filter(|i| {
                        vals.chunks(2).enumerate().all(|(k, c)| {
                            ranges[k].get(*i).map(|v| criteria_match(v, &c[1])).unwrap_or(false)
                        })
                    })
                    .count() as f64,
            )
        },
        "SUMIF" | "AVERAGEIF" => {
            if vals.len() < 2 {
                return Value::err("#N/A");
            }
            let range = vals[0].flatten();
            let sum_range = vals.get(2).map(Value::flatten).unwrap_or_else(|| range.clone());
            let picked: Vec<f64> = range
                .iter()
                .enumerate()
                .filter(|(_, v)| criteria_match(v, &vals[1]))
                .filter_map(|(i, _)| {
                    sum_range.get(i).and_then(|s| {
                        if let Value::Number(n) = s {
                            Some(*n)
                        } else {
                            None
                        }
                    })
                })
                .collect();
            if name == "SUMIF" {
                Value::Number(picked.iter().sum())
            } else if picked.is_empty() {
                Value::err("#DIV/0!")
            } else {
                Value::Number(picked.iter().sum::<f64>() / picked.len() as f64)
            }
        },
        "SUMIFS" | "AVERAGEIFS" | "MINIFS" | "MAXIFS" => {
            if vals.len() < 3 {
                return Value::err("#N/A");
            }
            let target = vals[0].flatten();
            let pairs: Vec<(Vec<Value>, &Value)> = vals[1..]
                .chunks(2)
                .filter(|c| c.len() == 2)
                .map(|c| (c[0].flatten(), &c[1]))
                .collect();
            let picked: Vec<f64> = (0..target.len())
                .filter(|i| {
                    pairs
                        .iter()
                        .all(|(r, c)| r.get(*i).map(|v| criteria_match(v, c)).unwrap_or(false))
                })
                .filter_map(|i| {
                    if let Value::Number(n) = &target[i] {
                        Some(*n)
                    } else {
                        None
                    }
                })
                .collect();
            match name {
                "SUMIFS" => Value::Number(picked.iter().sum()),
                "AVERAGEIFS" => {
                    if picked.is_empty() {
                        Value::err("#DIV/0!")
                    } else {
                        Value::Number(picked.iter().sum::<f64>() / picked.len() as f64)
                    }
                },
                "MINIFS" => Value::Number(picked.iter().cloned().fold(f64::INFINITY, f64::min))
                    .pipe(|v| {
                        if matches!(v, Value::Number(n) if n.is_infinite()) {
                            Value::Number(0.0)
                        } else {
                            v
                        }
                    }),
                _ => Value::Number(picked.iter().cloned().fold(f64::NEG_INFINITY, f64::max)).pipe(
                    |v| {
                        if matches!(v, Value::Number(n) if n.is_infinite()) {
                            Value::Number(0.0)
                        } else {
                            v
                        }
                    },
                ),
            }
        },
        "SUMPRODUCT" => {
            let arrays: Vec<Vec<f64>> = vals
                .iter()
                .map(|v| v.flatten().iter().map(|x| x.num().unwrap_or(0.0)).collect())
                .collect();
            let n = arrays.iter().map(Vec::len).min().unwrap_or(0);
            Value::Number((0..n).map(|i| arrays.iter().map(|a| a[i]).product::<f64>()).sum())
        },
        "SUMSQ" => Value::Number(try_v!(nums(true)).iter().map(|x| x * x).sum()),
        "ABS" => Value::Number(try_v!(arg_num(0)).abs()),
        "SIGN" => Value::Number(try_v!(arg_num(0)).signum()).pipe(|v| {
            if let Value::Number(n) = v {
                Value::Number(if n.is_nan() { 0.0 } else { n })
            } else {
                v
            }
        }),
        "SQRT" => {
            let n = try_v!(arg_num(0));
            if n < 0.0 {
                Value::err("#NUM!")
            } else {
                Value::Number(n.sqrt())
            }
        },
        "POWER" => Value::Number(try_v!(arg_num(0)).powf(try_v!(arg_num(1)))),
        "EXP" => Value::Number(try_v!(arg_num(0)).exp()),
        "LN" => {
            let n = try_v!(arg_num(0));
            if n <= 0.0 {
                Value::err("#NUM!")
            } else {
                Value::Number(n.ln())
            }
        },
        "LOG" => {
            let n = try_v!(arg_num(0));
            let b = try_v!(opt_num(1, 10.0));
            if n <= 0.0 || b <= 0.0 || b == 1.0 {
                Value::err("#NUM!")
            } else {
                Value::Number(n.ln() / b.ln())
            }
        },
        "LOG10" => {
            let n = try_v!(arg_num(0));
            if n <= 0.0 {
                Value::err("#NUM!")
            } else {
                Value::Number(n.log10())
            }
        },
        "MOD" => {
            let (a, b) = (try_v!(arg_num(0)), try_v!(arg_num(1)));
            if b == 0.0 {
                Value::err("#DIV/0!")
            } else {
                Value::Number(a - b * (a / b).floor())
            }
        },
        "QUOTIENT" => {
            let (a, b) = (try_v!(arg_num(0)), try_v!(arg_num(1)));
            if b == 0.0 {
                Value::err("#DIV/0!")
            } else {
                Value::Number((a / b).trunc())
            }
        },
        "INT" => Value::Number(try_v!(arg_num(0)).floor()),
        "TRUNC" => Value::Number(round_to(try_v!(arg_num(0)), try_v!(opt_num(1, 0.0)), "down")),
        "ROUND" => Value::Number(round_to(try_v!(arg_num(0)), try_v!(opt_num(1, 0.0)), "nearest")),
        "ROUNDUP" => Value::Number(round_to(try_v!(arg_num(0)), try_v!(opt_num(1, 0.0)), "up")),
        "ROUNDDOWN" => Value::Number(round_to(try_v!(arg_num(0)), try_v!(opt_num(1, 0.0)), "down")),
        "CEILING" | "CEILING.MATH" => {
            let n = try_v!(arg_num(0));
            let s = try_v!(opt_num(1, 1.0));
            if s == 0.0 {
                Value::Number(0.0)
            } else {
                Value::Number((n / s).ceil() * s)
            }
        },
        "FLOOR" | "FLOOR.MATH" => {
            let n = try_v!(arg_num(0));
            let s = try_v!(opt_num(1, 1.0));
            if s == 0.0 {
                Value::Number(0.0)
            } else {
                Value::Number((n / s).floor() * s)
            }
        },
        "MROUND" => {
            let n = try_v!(arg_num(0));
            let s = try_v!(arg_num(1));
            if s == 0.0 {
                Value::Number(0.0)
            } else {
                Value::Number((n / s).round() * s)
            }
        },
        "EVEN" => {
            let n = try_v!(arg_num(0));
            let r = (n.abs() / 2.0).ceil() * 2.0;
            Value::Number(r.copysign(n))
        },
        "ODD" => {
            let n = try_v!(arg_num(0));
            let mut r = n.abs().ceil();
            if r % 2.0 == 0.0 {
                r += 1.0;
            }
            Value::Number(r.copysign(n))
        },
        "PI" => Value::Number(std::f64::consts::PI),
        "RADIANS" => Value::Number(try_v!(arg_num(0)).to_radians()),
        "DEGREES" => Value::Number(try_v!(arg_num(0)).to_degrees()),
        "SIN" => Value::Number(try_v!(arg_num(0)).sin()),
        "COS" => Value::Number(try_v!(arg_num(0)).cos()),
        "TAN" => Value::Number(try_v!(arg_num(0)).tan()),
        "ASIN" => Value::Number(try_v!(arg_num(0)).asin()),
        "ACOS" => Value::Number(try_v!(arg_num(0)).acos()),
        "ATAN" => Value::Number(try_v!(arg_num(0)).atan()),
        "ATAN2" => Value::Number(try_v!(arg_num(1)).atan2(try_v!(arg_num(0)))),
        "FACT" => {
            let n = try_v!(arg_num(0));
            if n < 0.0 || n > 170.0 {
                Value::err("#NUM!")
            } else {
                Value::Number((1..=(n as u64)).map(|x| x as f64).product())
            }
        },
        "COMBIN" => {
            let (n, k) = (try_v!(arg_num(0)).floor(), try_v!(arg_num(1)).floor());
            if k < 0.0 || n < k {
                Value::err("#NUM!")
            } else {
                let mut r = 1.0;
                for i in 0..(k as u64) {
                    r = r * (n - i as f64) / (i as f64 + 1.0);
                }
                Value::Number(r.round())
            }
        },
        "GCD" => {
            let n = try_v!(nums(true));
            Value::Number(n.iter().fold(0u64, |a, b| gcd(a, b.abs() as u64)) as f64)
        },
        "LCM" => {
            let n = try_v!(nums(true));
            Value::Number(n.iter().fold(1u64, |a, b| {
                let b = b.abs() as u64;
                if a == 0 || b == 0 {
                    0
                } else {
                    a / gcd(a, b) * b
                }
            }) as f64)
        },
        // ---- statistics
        "MEDIAN" => {
            let mut n = try_v!(nums(true));
            if n.is_empty() {
                return Value::err("#NUM!");
            }
            n.sort_by(f64::total_cmp);
            let m = n.len() / 2;
            Value::Number(if n.len().is_multiple_of(2) {
                (n[m - 1] + n[m]) / 2.0
            } else {
                n[m]
            })
        },
        "MODE" | "MODE.SNGL" => {
            let n = try_v!(nums(true));
            let mut best: Option<(f64, usize)> = None;
            for x in &n {
                let c = n.iter().filter(|y| *y == x).count();
                if c > 1 && best.map(|b| c > b.1).unwrap_or(true) {
                    best = Some((*x, c));
                }
            }
            best.map(|b| Value::Number(b.0)).unwrap_or(Value::err("#N/A"))
        },
        "STDEV" | "STDEV.S" | "VAR" | "VAR.S" | "STDEV.P" | "STDEVP" | "VAR.P" | "VARP" => {
            let n = try_v!(nums(true));
            let sample = !name.ends_with('P') && !name.ends_with(".P");
            if n.len() < if sample { 2 } else { 1 } {
                return Value::err("#DIV/0!");
            }
            let mean = n.iter().sum::<f64>() / n.len() as f64;
            let var = n.iter().map(|x| (x - mean).powi(2)).sum::<f64>()
                / (n.len() as f64 - if sample { 1.0 } else { 0.0 });
            Value::Number(if name.starts_with("VAR") { var } else { var.sqrt() })
        },
        "LARGE" | "SMALL" => {
            let mut n: Vec<f64> = vals
                .first()
                .map(|v| {
                    v.flatten()
                        .iter()
                        .filter_map(|x| if let Value::Number(n) = x { Some(*n) } else { None })
                        .collect()
                })
                .unwrap_or_default();
            let k = try_v!(arg_num(1)) as usize;
            if k == 0 || k > n.len() {
                return Value::err("#NUM!");
            }
            n.sort_by(f64::total_cmp);
            Value::Number(if name == "SMALL" { n[k - 1] } else { n[n.len() - k] })
        },
        "RANK" | "RANK.EQ" => {
            let x = try_v!(arg_num(0));
            let n: Vec<f64> = vals
                .get(1)
                .map(|v| {
                    v.flatten()
                        .iter()
                        .filter_map(|x| if let Value::Number(n) = x { Some(*n) } else { None })
                        .collect()
                })
                .unwrap_or_default();
            let asc = try_v!(opt_num(2, 0.0)) != 0.0;
            if !n.contains(&x) {
                return Value::err("#N/A");
            }
            Value::Number(
                1.0 + n.iter().filter(|y| if asc { **y < x } else { **y > x }).count() as f64,
            )
        },
        "PERCENTILE" | "PERCENTILE.INC" | "QUARTILE" | "QUARTILE.INC" => {
            let mut n: Vec<f64> = vals
                .first()
                .map(|v| {
                    v.flatten()
                        .iter()
                        .filter_map(|x| if let Value::Number(n) = x { Some(*n) } else { None })
                        .collect()
                })
                .unwrap_or_default();
            if n.is_empty() {
                return Value::err("#NUM!");
            }
            let p = try_v!(arg_num(1));
            let p = if name.starts_with("QUARTILE") { p / 4.0 } else { p };
            if !(0.0..=1.0).contains(&p) {
                return Value::err("#NUM!");
            }
            n.sort_by(f64::total_cmp);
            let pos = p * (n.len() - 1) as f64;
            let (lo, hi) = (pos.floor() as usize, pos.ceil() as usize);
            Value::Number(n[lo] + (n[hi] - n[lo]) * (pos - lo as f64))
        },
        // ---- logical
        "AND" => {
            let mut r = true;
            for v in vals.iter().flat_map(Value::flatten) {
                if matches!(v, Value::Empty | Value::Text(_)) {
                    continue;
                }
                r &= try_v!(v.boolean());
            }
            Value::Bool(r)
        },
        "OR" => {
            let mut r = false;
            for v in vals.iter().flat_map(Value::flatten) {
                if matches!(v, Value::Empty | Value::Text(_)) {
                    continue;
                }
                r |= try_v!(v.boolean());
            }
            Value::Bool(r)
        },
        "XOR" => {
            let mut r = false;
            for v in vals.iter().flat_map(Value::flatten) {
                if matches!(v, Value::Empty | Value::Text(_)) {
                    continue;
                }
                r ^= try_v!(v.boolean());
            }
            Value::Bool(r)
        },
        "NOT" => Value::Bool(!try_v!(vals
            .first()
            .map(Value::boolean)
            .unwrap_or(Err(Value::err("#N/A"))))),
        "TRUE" => Value::Bool(true),
        "FALSE" => Value::Bool(false),
        "ISNUMBER" => Value::Bool(matches!(vals.first(), Some(Value::Number(_)))),
        "ISTEXT" => Value::Bool(matches!(vals.first(), Some(Value::Text(_)))),
        "ISNONTEXT" => Value::Bool(!matches!(vals.first(), Some(Value::Text(_)))),
        "ISBLANK" => Value::Bool(matches!(vals.first(), Some(Value::Empty) | None)),
        "ISLOGICAL" => Value::Bool(matches!(vals.first(), Some(Value::Bool(_)))),
        "ISERROR" | "ISERR" => Value::Bool(matches!(vals.first(), Some(Value::Error(_)))),
        "ISNA" => Value::Bool(matches!(vals.first(), Some(Value::Error(e)) if e == "#N/A")),
        "ISEVEN" => Value::Bool((try_v!(arg_num(0)).trunc() as i64) % 2 == 0),
        "ISODD" => Value::Bool((try_v!(arg_num(0)).trunc() as i64) % 2 != 0),
        "N" => Value::Number(vals.first().map(|v| v.num().unwrap_or(0.0)).unwrap_or(0.0)),
        "T" => match vals.first() {
            Some(Value::Text(s)) => Value::Text(s.clone()),
            _ => Value::Text(String::new()),
        },
        "TYPE" => Value::Number(match vals.first() {
            Some(Value::Number(_)) | Some(Value::Empty) | None => 1.0,
            Some(Value::Text(_)) => 2.0,
            Some(Value::Bool(_)) => 4.0,
            Some(Value::Error(_)) => 16.0,
            Some(Value::Array(_)) => 64.0,
        }),
        "NA" => Value::err("#N/A"),
        // ---- text
        "LEN" => Value::Number(try_v!(arg_text(0)).chars().count() as f64),
        "UPPER" => Value::Text(try_v!(arg_text(0)).to_uppercase()),
        "LOWER" => Value::Text(try_v!(arg_text(0)).to_lowercase()),
        "PROPER" => Value::Text({
            let s = try_v!(arg_text(0));
            let mut out = String::new();
            let mut new_word = true;
            for c in s.chars() {
                if c.is_alphanumeric() {
                    out.extend(if new_word {
                        c.to_uppercase().collect::<Vec<_>>()
                    } else {
                        c.to_lowercase().collect()
                    });
                    new_word = false;
                } else {
                    out.push(c);
                    new_word = true;
                }
            }
            out
        }),
        "TRIM" => Value::Text(try_v!(arg_text(0)).split_whitespace().collect::<Vec<_>>().join(" ")),
        "CLEAN" => Value::Text(try_v!(arg_text(0)).chars().filter(|c| !c.is_control()).collect()),
        "LEFT" => {
            let s = try_v!(arg_text(0));
            let n = try_v!(opt_num(1, 1.0)).max(0.0) as usize;
            Value::Text(s.chars().take(n).collect())
        },
        "RIGHT" => {
            let s = try_v!(arg_text(0));
            let n = try_v!(opt_num(1, 1.0)).max(0.0) as usize;
            let len = s.chars().count();
            Value::Text(s.chars().skip(len.saturating_sub(n)).collect())
        },
        "MID" => {
            let s = try_v!(arg_text(0));
            let start = try_v!(arg_num(1));
            let n = try_v!(arg_num(2));
            if start < 1.0 || n < 0.0 {
                return Value::err("#VALUE!");
            }
            Value::Text(s.chars().skip(start as usize - 1).take(n as usize).collect())
        },
        "CONCAT" | "CONCATENATE" => {
            let mut s = String::new();
            for v in vals.iter().flat_map(Value::flatten) {
                s.push_str(&try_v!(v.text()));
            }
            Value::Text(s)
        },
        "TEXTJOIN" => {
            let sep = try_v!(arg_text(0));
            let skip_empty = try_v!(vals.get(1).map(Value::boolean).unwrap_or(Ok(true)));
            let parts: Vec<String> = vals
                .iter()
                .skip(2)
                .flat_map(Value::flatten)
                .filter(|v| !(skip_empty && matches!(v, Value::Empty)))
                .map(|v| v.display())
                .collect();
            Value::Text(parts.join(&sep))
        },
        "REPT" => {
            let s = try_v!(arg_text(0));
            let n = try_v!(arg_num(1));
            if n < 0.0 || s.len() as f64 * n > 32_767.0 {
                return Value::err("#VALUE!");
            }
            Value::Text(s.repeat(n as usize))
        },
        "SUBSTITUTE" => {
            let s = try_v!(arg_text(0));
            let old = try_v!(arg_text(1));
            let new = try_v!(arg_text(2));
            if old.is_empty() {
                return Value::Text(s);
            }
            match vals.get(3) {
                Some(Value::Empty) | None => Value::Text(s.replace(&old, &new)),
                Some(n) => {
                    let k = try_v!(n.num()) as usize;
                    let mut count = 0;
                    let mut out = String::new();
                    let mut rest = s.as_str();
                    while let Some(i) = rest.find(&old) {
                        count += 1;
                        out.push_str(&rest[..i]);
                        out.push_str(if count == k { &new } else { &old });
                        rest = &rest[i + old.len()..];
                    }
                    out.push_str(rest);
                    Value::Text(out)
                },
            }
        },
        "REPLACE" => {
            let s: Vec<char> = try_v!(arg_text(0)).chars().collect();
            let start = try_v!(arg_num(1)) as usize;
            let n = try_v!(arg_num(2)) as usize;
            let new = try_v!(arg_text(3));
            if start < 1 {
                return Value::err("#VALUE!");
            }
            let mut out: String = s.iter().take(start - 1).collect();
            out.push_str(&new);
            out.extend(s.iter().skip(start - 1 + n));
            Value::Text(out)
        },
        "FIND" | "SEARCH" => {
            let needle = try_v!(arg_text(0));
            let hay = try_v!(arg_text(1));
            let start = try_v!(opt_num(2, 1.0)) as usize;
            if start < 1 {
                return Value::err("#VALUE!");
            }
            let (h, n) = if name == "SEARCH" {
                (hay.to_lowercase(), needle.to_lowercase())
            } else {
                (hay.clone(), needle.clone())
            };
            let offset: usize = h.chars().take(start - 1).map(char::len_utf8).sum();
            match h.get(offset..).and_then(|rest| rest.find(&n)) {
                Some(i) => Value::Number(h[..offset + i].chars().count() as f64 + 1.0),
                None => Value::err("#VALUE!"),
            }
        },
        "EXACT" => Value::Bool(try_v!(arg_text(0)) == try_v!(arg_text(1))),
        "VALUE" | "NUMBERVALUE" => {
            let s = try_v!(arg_text(0));
            match Value::Text(s.replace(',', "")).num() {
                Ok(n) => Value::Number(n),
                Err(_) => Value::err("#VALUE!"),
            }
        },
        "CHAR" => {
            let n = try_v!(arg_num(0)) as u32;
            char::from_u32(n)
                .map(|c| Value::Text(c.to_string()))
                .unwrap_or(Value::err("#VALUE!"))
        },
        "CODE" | "UNICODE" => try_v!(arg_text(0))
            .chars()
            .next()
            .map(|c| Value::Number(c as u32 as f64))
            .unwrap_or(Value::err("#VALUE!")),
        "TEXT" => {
            let v = vals.first().cloned().unwrap_or(Value::Empty);
            let fmt = try_v!(arg_text(1));
            Value::Text(format_text(&v, &fmt))
        },
        "FIXED" => {
            let n = try_v!(arg_num(0));
            let d = try_v!(opt_num(1, 2.0)) as usize;
            Value::Text(format!("{:.*}", d, n))
        },
        "DOLLAR" => {
            let n = try_v!(arg_num(0));
            let d = try_v!(opt_num(1, 2.0)) as usize;
            Value::Text(format!("${}", thousands(&format!("{:.*}", d, n))))
        },
        // ---- lookup
        "VLOOKUP" | "HLOOKUP" => {
            if vals.len() < 3 {
                return Value::err("#N/A");
            }
            let key = &vals[0];
            let Value::Array(table) = &vals[1] else {
                return Value::err("#N/A");
            };
            let idx = try_v!(vals[2].num()) as usize;
            let approx = try_v!(vals.get(3).map(Value::boolean).unwrap_or(Ok(true)));
            if idx == 0 {
                return Value::err("#VALUE!");
            }
            type Picker<'a> = Box<dyn Fn(usize) -> Option<Value> + 'a>;
            let (keys, pick): (Vec<Value>, Picker) = if name == "VLOOKUP" {
                (
                    table
                        .iter()
                        .map(|r| r.first().cloned().unwrap_or(Value::Empty))
                        .collect(),
                    Box::new(|i| table.get(i).and_then(|r| r.get(idx - 1)).cloned()),
                )
            } else {
                (
                    table.first().cloned().unwrap_or_default(),
                    Box::new(|i| table.get(idx - 1).and_then(|r| r.get(i)).cloned()),
                )
            };
            let found = if approx {
                let mut best = None;
                for (i, k) in keys.iter().enumerate() {
                    if compare(k, key) != std::cmp::Ordering::Greater {
                        best = Some(i);
                    } else {
                        break;
                    }
                }
                best
            } else {
                keys.iter().position(|k| match (k, key) {
                    (Value::Text(a), Value::Text(b)) if b.contains('*') || b.contains('?') => {
                        wildcard(&b.to_lowercase(), &a.to_lowercase())
                    },
                    _ => compare(k, key) == std::cmp::Ordering::Equal,
                })
            };
            match found {
                Some(i) => pick(i).unwrap_or(Value::err("#REF!")),
                None => Value::err("#N/A"),
            }
        },
        "XLOOKUP" => {
            if vals.len() < 3 {
                return Value::err("#N/A");
            }
            let keys = vals[1].flatten();
            let ret = vals[2].flatten();
            match keys
                .iter()
                .position(|k| compare(k, &vals[0]) == std::cmp::Ordering::Equal)
            {
                Some(i) => ret.get(i).cloned().unwrap_or(Value::err("#REF!")),
                None => vals
                    .get(3)
                    .cloned()
                    .filter(|v| !matches!(v, Value::Empty))
                    .unwrap_or(Value::err("#N/A")),
            }
        },
        "MATCH" => {
            if vals.len() < 2 {
                return Value::err("#N/A");
            }
            let keys = vals[1].flatten();
            let mode = try_v!(opt_num(2, 1.0));
            let pos = if mode == 0.0 {
                keys.iter()
                    .position(|k| compare(k, &vals[0]) == std::cmp::Ordering::Equal)
            } else if mode > 0.0 {
                let mut best = None;
                for (i, k) in keys.iter().enumerate() {
                    if compare(k, &vals[0]) != std::cmp::Ordering::Greater {
                        best = Some(i);
                    } else {
                        break;
                    }
                }
                best
            } else {
                let mut best = None;
                for (i, k) in keys.iter().enumerate() {
                    if compare(k, &vals[0]) != std::cmp::Ordering::Less {
                        best = Some(i);
                    } else {
                        break;
                    }
                }
                best
            };
            pos.map(|p| Value::Number(p as f64 + 1.0))
                .unwrap_or(Value::err("#N/A"))
        },
        "INDEX" => {
            let Some(Value::Array(table)) = vals.first() else {
                return vals.first().cloned().unwrap_or(Value::err("#N/A"));
            };
            let r = try_v!(opt_num(1, 0.0)) as usize;
            let c = try_v!(opt_num(2, 0.0)) as usize;
            match (r, c) {
                (0, 0) => Value::Array(table.clone()),
                (0, c) => Value::Array(
                    table
                        .iter()
                        .map(|row| vec![row.get(c - 1).cloned().unwrap_or(Value::Empty)])
                        .collect(),
                ),
                (r, 0) => {
                    if table.len() == 1 {
                        table[0].get(r - 1).cloned().unwrap_or(Value::err("#REF!"))
                    } else if table.first().map(|x| x.len() == 1).unwrap_or(false) {
                        table
                            .get(r - 1)
                            .and_then(|row| row.first())
                            .cloned()
                            .unwrap_or(Value::err("#REF!"))
                    } else {
                        table
                            .get(r - 1)
                            .map(|row| Value::Array(vec![row.clone()]))
                            .unwrap_or(Value::err("#REF!"))
                    }
                },
                (r, c) => table
                    .get(r - 1)
                    .and_then(|row| row.get(c - 1))
                    .cloned()
                    .unwrap_or(Value::err("#REF!")),
            }
        },
        "ROWS" => Value::Number(match vals.first() {
            Some(Value::Array(t)) => t.len() as f64,
            _ => 1.0,
        }),
        "COLUMNS" => Value::Number(match vals.first() {
            Some(Value::Array(t)) => t.first().map(Vec::len).unwrap_or(0) as f64,
            _ => 1.0,
        }),
        "TRANSPOSE" => match vals.first() {
            Some(Value::Array(t)) => {
                let w = t.first().map(Vec::len).unwrap_or(0);
                Value::Array(
                    (0..w)
                        .map(|c| {
                            t.iter().map(|r| r.get(c).cloned().unwrap_or(Value::Empty)).collect()
                        })
                        .collect(),
                )
            },
            Some(v) => v.clone(),
            None => Value::err("#N/A"),
        },
        "UNIQUE" => {
            let items = vals.first().map(Value::flatten).unwrap_or_default();
            let mut out: Vec<Value> = Vec::new();
            for v in items {
                if !out.iter().any(|x| compare(x, &v) == std::cmp::Ordering::Equal) {
                    out.push(v);
                }
            }
            Value::Array(out.into_iter().map(|v| vec![v]).collect())
        },
        "SORT" => {
            let mut items = vals.first().map(Value::flatten).unwrap_or_default();
            let desc = try_v!(opt_num(2, 1.0)) < 0.0;
            items.sort_by(compare);
            if desc {
                items.reverse();
            }
            Value::Array(items.into_iter().map(|v| vec![v]).collect())
        },
        "FILTER" => {
            let items = vals.first().map(Value::flatten).unwrap_or_default();
            let flags = vals.get(1).map(Value::flatten).unwrap_or_default();
            let out: Vec<Vec<Value>> = items
                .into_iter()
                .zip(flags)
                .filter(|(_, f)| f.boolean().unwrap_or(false))
                .map(|(v, _)| vec![v])
                .collect();
            if out.is_empty() {
                vals.get(2).cloned().unwrap_or(Value::err("#CALC!"))
            } else {
                Value::Array(out)
            }
        },
        // ---- dates (serial based)
        "DATE" => date_serial(try_v!(arg_num(0)), try_v!(arg_num(1)), try_v!(arg_num(2))),
        "DATEVALUE" => {
            let s = try_v!(arg_text(0));
            iso_to_serial(&s)
                .map(|(v, _)| Value::Number(v.floor()))
                .unwrap_or(Value::err("#VALUE!"))
        },
        "TIME" => Value::Number(
            ((try_v!(arg_num(0)) * 3600.0 + try_v!(arg_num(1)) * 60.0 + try_v!(arg_num(2)))
                / 86400.0)
                .rem_euclid(1.0),
        ),
        "YEAR" | "MONTH" | "DAY" => {
            let s = try_v!(arg_num(0));
            if s < 1.0 {
                return Value::err("#NUM!");
            }
            let (y, m, d) = serial_to_ymd(s);
            Value::Number(match name {
                "YEAR" => y,
                "MONTH" => m,
                _ => d,
            } as f64)
        },
        "HOUR" => {
            Value::Number(((try_v!(arg_num(0)).fract() * 86400.0).round() / 3600.0).floor() % 24.0)
        },
        "MINUTE" => {
            Value::Number((((try_v!(arg_num(0)).fract() * 86400.0).round() / 60.0).floor()) % 60.0)
        },
        "SECOND" => Value::Number((try_v!(arg_num(0)).fract() * 86400.0).round() % 60.0),
        "WEEKDAY" => {
            let s = try_v!(arg_num(0)).floor();
            let t = try_v!(opt_num(1, 1.0));
            let sunday0 = ((s as i64 - 1).rem_euclid(7)) as f64;
            Value::Number(match t as i64 {
                2 => (sunday0 + 6.0) % 7.0 + 1.0,
                3 => (sunday0 + 6.0) % 7.0,
                _ => sunday0 + 1.0,
            })
        },
        "DAYS" => Value::Number(try_v!(arg_num(0)).floor() - try_v!(arg_num(1)).floor()),
        "DAYS360" => {
            let (a, b) = (serial_to_ymd(try_v!(arg_num(0))), serial_to_ymd(try_v!(arg_num(1))));
            Value::Number(
                ((b.0 - a.0) * 360 + (b.1 - a.1) * 30 + (b.2.min(30) - a.2.min(30))) as f64,
            )
        },
        "EDATE" | "EOMONTH" => {
            let (y, m, d) = serial_to_ymd(try_v!(arg_num(0)));
            let months = try_v!(arg_num(1)) as i64;
            let total = y * 12 + (m - 1) + months;
            let (ny, nm) = (total.div_euclid(12), total.rem_euclid(12) + 1);
            let dim = days_in_month(ny, nm);
            if name == "EOMONTH" {
                date_serial(ny as f64, nm as f64, dim as f64)
            } else {
                date_serial(ny as f64, nm as f64, d.min(dim) as f64)
            }
        },
        "NETWORKDAYS" => {
            let (a, b) = (try_v!(arg_num(0)).floor() as i64, try_v!(arg_num(1)).floor() as i64);
            let (lo, hi) = (a.min(b), a.max(b));
            let holidays: Vec<i64> = vals
                .get(2)
                .map(|v| {
                    v.flatten()
                        .iter()
                        .filter_map(|x| x.num().ok())
                        .map(|x| x.floor() as i64)
                        .collect()
                })
                .unwrap_or_default();
            let mut n = 0;
            for s in lo..=hi {
                let wd = (s - 1).rem_euclid(7);
                if wd != 0 && wd != 6 && !holidays.contains(&s) {
                    n += 1;
                }
            }
            Value::Number(if a <= b { n as f64 } else { -(n as f64) })
        },
        "WORKDAY" => {
            let mut s = try_v!(arg_num(0)).floor() as i64;
            let mut left = try_v!(arg_num(1)) as i64;
            let holidays: Vec<i64> = vals
                .get(2)
                .map(|v| {
                    v.flatten()
                        .iter()
                        .filter_map(|x| x.num().ok())
                        .map(|x| x.floor() as i64)
                        .collect()
                })
                .unwrap_or_default();
            let step = if left < 0 { -1 } else { 1 };
            while left != 0 {
                s += step;
                let wd = (s - 1).rem_euclid(7);
                if wd != 0 && wd != 6 && !holidays.contains(&s) {
                    left -= step;
                }
            }
            Value::Number(s as f64)
        },
        "YEARFRAC" => {
            let (a, b) = (try_v!(arg_num(0)), try_v!(arg_num(1)));
            Value::Number((b - a).abs() / 365.25)
        },
        // ---- finance
        "PMT" => {
            let r = try_v!(arg_num(0));
            let n = try_v!(arg_num(1));
            let pv = try_v!(arg_num(2));
            let fv = try_v!(opt_num(3, 0.0));
            let t = try_v!(opt_num(4, 0.0));
            if r == 0.0 {
                Value::Number(-(pv + fv) / n)
            } else {
                let f = (1.0 + r).powf(n);
                Value::Number(-(r * (pv * f + fv)) / ((1.0 + r * t) * (f - 1.0)))
            }
        },
        "FV" => {
            let r = try_v!(arg_num(0));
            let n = try_v!(arg_num(1));
            let pmt = try_v!(arg_num(2));
            let pv = try_v!(opt_num(3, 0.0));
            let t = try_v!(opt_num(4, 0.0));
            if r == 0.0 {
                Value::Number(-(pv + pmt * n))
            } else {
                let f = (1.0 + r).powf(n);
                Value::Number(-(pv * f + pmt * (1.0 + r * t) * (f - 1.0) / r))
            }
        },
        "PV" => {
            let r = try_v!(arg_num(0));
            let n = try_v!(arg_num(1));
            let pmt = try_v!(arg_num(2));
            let fv = try_v!(opt_num(3, 0.0));
            let t = try_v!(opt_num(4, 0.0));
            if r == 0.0 {
                Value::Number(-(fv + pmt * n))
            } else {
                let f = (1.0 + r).powf(n);
                Value::Number(-(fv + pmt * (1.0 + r * t) * (f - 1.0) / r) / f)
            }
        },
        "NPER" => {
            let r = try_v!(arg_num(0));
            let pmt = try_v!(arg_num(1));
            let pv = try_v!(arg_num(2));
            let fv = try_v!(opt_num(3, 0.0));
            if r == 0.0 {
                if pmt == 0.0 {
                    return Value::err("#NUM!");
                }
                Value::Number(-(pv + fv) / pmt)
            } else {
                let v = ((pmt - fv * r) / (pmt + pv * r)).ln() / (1.0 + r).ln();
                if v.is_finite() {
                    Value::Number(v)
                } else {
                    Value::err("#NUM!")
                }
            }
        },
        "RATE" => {
            let n = try_v!(arg_num(0));
            let pmt = try_v!(arg_num(1));
            let pv = try_v!(arg_num(2));
            let fv = try_v!(opt_num(3, 0.0));
            let mut r = try_v!(opt_num(5, 0.1));
            for _ in 0..100 {
                let f = (1.0 + r).powf(n);
                let y = pv * f + pmt * (f - 1.0) / r + fv;
                let dy = n * pv * (1.0 + r).powf(n - 1.0)
                    + pmt * (n * (1.0 + r).powf(n - 1.0) * r - (f - 1.0)) / (r * r);
                if dy == 0.0 {
                    break;
                }
                let nr = r - y / dy;
                if (nr - r).abs() < 1e-10 {
                    r = nr;
                    break;
                }
                r = nr;
            }
            if r.is_finite() {
                Value::Number(r)
            } else {
                Value::err("#NUM!")
            }
        },
        "NPV" => {
            let r = try_v!(arg_num(0));
            let flows: Vec<f64> = vals
                .iter()
                .skip(1)
                .flat_map(Value::flatten)
                .filter_map(|v| if let Value::Number(n) = v { Some(n) } else { None })
                .collect();
            Value::Number(
                flows
                    .iter()
                    .enumerate()
                    .map(|(i, cf)| cf / (1.0 + r).powi(i as i32 + 1))
                    .sum(),
            )
        },
        "IRR" => {
            let flows: Vec<f64> = vals
                .first()
                .map(|v| {
                    v.flatten()
                        .iter()
                        .filter_map(|x| if let Value::Number(n) = x { Some(*n) } else { None })
                        .collect()
                })
                .unwrap_or_default();
            let mut r = try_v!(opt_num(1, 0.1));
            for _ in 0..200 {
                let (mut f, mut df) = (0.0, 0.0);
                for (i, cf) in flows.iter().enumerate() {
                    f += cf / (1.0 + r).powi(i as i32);
                    df -= i as f64 * cf / (1.0 + r).powi(i as i32 + 1);
                }
                if df == 0.0 {
                    break;
                }
                let nr = r - f / df;
                if (nr - r).abs() < 1e-10 {
                    r = nr;
                    break;
                }
                r = nr;
            }
            if r.is_finite() && r > -1.0 {
                Value::Number(r)
            } else {
                Value::err("#NUM!")
            }
        },
        "SLN" => {
            let c = try_v!(arg_num(0));
            let s = try_v!(arg_num(1));
            let l = try_v!(arg_num(2));
            if l == 0.0 {
                Value::err("#DIV/0!")
            } else {
                Value::Number((c - s) / l)
            }
        },
        // ---- conversion / misc
        "ROMAN" => {
            let n = try_v!(arg_num(0)) as i64;
            if !(0..4000).contains(&n) {
                return Value::err("#VALUE!");
            }
            Value::Text(roman(n))
        },
        "ARABIC" => {
            let s = try_v!(arg_text(0)).to_uppercase();
            let mut total = 0i64;
            let vals_r = |c: char| match c {
                'I' => 1,
                'V' => 5,
                'X' => 10,
                'L' => 50,
                'C' => 100,
                'D' => 500,
                'M' => 1000,
                _ => 0,
            };
            let chars: Vec<char> = s.chars().collect();
            for i in 0..chars.len() {
                let v = vals_r(chars[i]);
                if v == 0 {
                    return Value::err("#VALUE!");
                }
                if i + 1 < chars.len() && v < vals_r(chars[i + 1]) {
                    total -= v;
                } else {
                    total += v;
                }
            }
            Value::Number(total as f64)
        },
        "BASE" => {
            let n = try_v!(arg_num(0)) as i64;
            let radix = try_v!(arg_num(1)) as u32;
            if !(2..=36).contains(&radix) || n < 0 {
                return Value::err("#NUM!");
            }
            let min_len = try_v!(opt_num(2, 0.0)) as usize;
            let s = to_radix(n as u64, radix);
            Value::Text(format!("{}{}", "0".repeat(min_len.saturating_sub(s.len())), s))
        },
        "DECIMAL" => {
            let s = try_v!(arg_text(0));
            let radix = try_v!(arg_num(1)) as u32;
            if !(2..=36).contains(&radix) {
                return Value::err("#NUM!");
            }
            i64::from_str_radix(s.trim(), radix)
                .map(|n| Value::Number(n as f64))
                .unwrap_or(Value::err("#NUM!"))
        },
        "DEC2BIN" => Value::Text(to_radix(try_v!(arg_num(0)) as u64, 2)),
        "DEC2HEX" => Value::Text(to_radix(try_v!(arg_num(0)) as u64, 16)),
        "DEC2OCT" => Value::Text(to_radix(try_v!(arg_num(0)) as u64, 8)),
        "BIN2DEC" => i64::from_str_radix(&try_v!(arg_text(0)), 2)
            .map(|n| Value::Number(n as f64))
            .unwrap_or(Value::err("#NUM!")),
        "HEX2DEC" => i64::from_str_radix(&try_v!(arg_text(0)), 16)
            .map(|n| Value::Number(n as f64))
            .unwrap_or(Value::err("#NUM!")),
        "OCT2DEC" => i64::from_str_radix(&try_v!(arg_text(0)), 8)
            .map(|n| Value::Number(n as f64))
            .unwrap_or(Value::err("#NUM!")),
        "CONVERT" => {
            let n = try_v!(arg_num(0));
            let from = try_v!(arg_text(1));
            let to = try_v!(arg_text(2));
            convert_units(n, &from, &to)
        },
        "CELL" | "INFO" | "INDIRECT" | "OFFSET" => {
            Value::Error(format!("#UNSUPPORTED! {name} needs a live spreadsheet environment"))
        },
        _ => Value::Error(format!("#NAME? unknown function {name}")),
    }
}

trait Pipe: Sized {
    fn pipe<R>(self, f: impl FnOnce(Self) -> R) -> R {
        f(self)
    }
}
impl Pipe for Value {}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        },
    }
}

fn to_radix(mut n: u64, radix: u32) -> String {
    if n == 0 {
        return "0".into();
    }
    let digits = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut out = Vec::new();
    while n > 0 {
        out.push(digits[(n % radix as u64) as usize] as char);
        n /= radix as u64;
    }
    out.iter().rev().collect()
}

fn roman(mut n: i64) -> String {
    let table = [
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
    let mut s = String::new();
    for (v, r) in table {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    s
}

fn thousands(s: &str) -> String {
    let (int, frac) = s.split_once('.').map(|(a, b)| (a, Some(b))).unwrap_or((s, None));
    let neg = int.starts_with('-');
    let digits: Vec<char> = int.trim_start_matches('-').chars().collect();
    let mut out = String::new();
    for (i, c) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*c);
    }
    format!(
        "{}{}{}",
        if neg { "-" } else { "" },
        out,
        frac.map(|f| format!(".{f}")).unwrap_or_default()
    )
}

/// A pragmatic subset of the TEXT() format language: `0`, `#`, `,`, `.`, `%`,
/// and the date tokens `yyyy`, `mm`, `dd`, `hh`, `ss`.
fn format_text(v: &Value, fmt: &str) -> String {
    let Ok(n) = v.num() else { return v.display() };
    let lower = fmt.to_lowercase();
    if lower.contains("yyyy")
        || lower.contains("dd")
        || (lower.contains("mm") && !lower.contains('#'))
    {
        let (y, m, d) = serial_to_ymd(n);
        let frac = n.fract() * 86400.0;
        let (h, mi, s) = (
            (frac / 3600.0).floor() as i64,
            ((frac % 3600.0) / 60.0).floor() as i64,
            (frac % 60.0).round() as i64,
        );
        let months = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        let mut out = fmt.to_string();
        for (tok, val) in [
            ("yyyy", format!("{y:04}")),
            ("yy", format!("{:02}", y % 100)),
            ("mmmm", months[(m - 1) as usize].to_string()),
            ("mmm", months[(m - 1) as usize][..3].to_string()),
            ("dd", format!("{d:02}")),
            ("hh", format!("{h:02}")),
            ("ss", format!("{s:02}")),
        ] {
            out = replace_ci(&out, tok, &val);
        }
        // `mm` after `hh:` means minutes.
        if let Some(pos) = out.to_lowercase().find("hh").or_else(|| out.find(':')) {
            let _ = pos;
        }
        out = if out.to_lowercase().contains(':') {
            replace_ci(&out, "mm", &format!("{mi:02}"))
        } else {
            replace_ci(&out, "mm", &format!("{m:02}"))
        };
        out = replace_ci(&out, "d", &d.to_string());
        out = replace_ci(&out, "m", &m.to_string());
        return out;
    }
    let percent = fmt.contains('%');
    let n = if percent { n * 100.0 } else { n };
    let decimals = fmt
        .split('.')
        .nth(1)
        .map(|d| d.chars().filter(|c| *c == '0' || *c == '#').count())
        .unwrap_or(0);
    let mut s = format!("{:.*}", decimals, n);
    if fmt.contains(',') {
        s = thousands(&s);
    }
    let min_int = fmt
        .split('.')
        .next()
        .unwrap_or("")
        .chars()
        .filter(|c| *c == '0')
        .count();
    let int_len = s.split('.').next().unwrap_or("").trim_start_matches('-').len();
    if int_len < min_int {
        let pad = "0".repeat(min_int - int_len);
        s = if let Some(rest) = s.strip_prefix('-') {
            format!("-{pad}{rest}")
        } else {
            format!("{pad}{s}")
        };
    }
    if percent {
        s.push('%');
    }
    s
}

fn replace_ci(s: &str, tok: &str, val: &str) -> String {
    let lower = s.to_lowercase();
    let mut out = String::new();
    let mut i = 0;
    while i < s.len() {
        if lower[i..].starts_with(tok) {
            out.push_str(val);
            i += tok.len();
        } else {
            let Some(ch) = s[i..].chars().next() else {
                break;
            };
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

fn convert_units(n: f64, from: &str, to: &str) -> Value {
    let table: &[(&str, &str, f64)] = &[
        ("m", "length", 1.0),
        ("km", "length", 1000.0),
        ("cm", "length", 0.01),
        ("mm", "length", 0.001),
        ("mi", "length", 1609.344),
        ("yd", "length", 0.9144),
        ("ft", "length", 0.3048),
        ("in", "length", 0.0254),
        ("Nmi", "length", 1852.0),
        ("g", "mass", 1.0),
        ("kg", "mass", 1000.0),
        ("lbm", "mass", 453.59237),
        ("ozm", "mass", 28.349523125),
        ("stone", "mass", 6350.29318),
        ("sec", "time", 1.0),
        ("min", "time", 60.0),
        ("hr", "time", 3600.0),
        ("day", "time", 86400.0),
        ("yr", "time", 31_557_600.0),
        ("l", "volume", 1.0),
        ("ml", "volume", 0.001),
        ("gal", "volume", 3.785411784),
        ("qt", "volume", 0.946352946),
        ("pt", "volume", 0.473176473),
        ("cup", "volume", 0.2365882365),
        ("oz", "volume", 0.0295735295625),
        ("tsp", "volume", 0.00492892159375),
        ("tbs", "volume", 0.01478676478125),
        ("m3", "volume", 1000.0),
        ("m2", "area", 1.0),
        ("ha", "area", 10_000.0),
        ("ft2", "area", 0.09290304),
        ("in2", "area", 0.00064516),
        ("mi2", "area", 2_589_988.110336),
        ("ar", "area", 100.0),
        ("uk_acre", "area", 4046.8564224),
        ("us_acre", "area", 4046.87261),
        ("J", "energy", 1.0),
        ("kJ", "energy", 1000.0),
        ("cal", "energy", 4.184),
        ("kcal", "energy", 4184.0),
        ("Wh", "energy", 3600.0),
        ("kWh", "energy", 3_600_000.0),
        ("BTU", "energy", 1055.05585262),
        ("eV", "energy", 1.602176634e-19),
        ("W", "power", 1.0),
        ("kW", "power", 1000.0),
        ("HP", "power", 745.69987158227),
        ("Pa", "pressure", 1.0),
        ("atm", "pressure", 101_325.0),
        ("mmHg", "pressure", 133.322387415),
        ("psi", "pressure", 6894.757293168),
        ("bar", "pressure", 100_000.0),
        ("m/s", "speed", 1.0),
        ("km/h", "speed", 0.277_777_777_8),
        ("mph", "speed", 0.44704),
        ("kn", "speed", 0.514_444_444_4),
        ("byte", "info", 1.0),
        ("bit", "info", 0.125),
        ("kbyte", "info", 1000.0),
        ("Mbyte", "info", 1e6),
        ("Gbyte", "info", 1e9),
        ("kibyte", "info", 1024.0),
        ("Mibyte", "info", 1_048_576.0),
        ("Gibyte", "info", 1_073_741_824.0),
    ];
    let temp = |v: f64, from: &str, to: &str| -> Option<f64> {
        let c = match from {
            "C" | "cel" => v,
            "F" | "fah" => (v - 32.0) * 5.0 / 9.0,
            "K" | "kel" => v - 273.15,
            _ => return None,
        };
        Some(match to {
            "C" | "cel" => c,
            "F" | "fah" => c * 9.0 / 5.0 + 32.0,
            "K" | "kel" => c + 273.15,
            _ => return None,
        })
    };
    if let Some(r) = temp(n, from, to) {
        return Value::Number(r);
    }
    let find = |u: &str| table.iter().find(|(name, _, _)| *name == u);
    match (find(from), find(to)) {
        (Some((_, ca, fa)), Some((_, cb, fb))) if ca == cb => Value::Number(n * fa / fb),
        _ => Value::err("#N/A"),
    }
}

/// Evaluates a formula against an optional workbook/sheet and returns a JSON
/// report (value, type, display, references used).
pub fn evaluate_report(
    formula: &str,
    wb: &Workbook,
    sheet: usize,
    today_serial: Option<f64>,
) -> Json {
    let mut ctx = Context::new(wb, sheet);
    ctx.today_serial = today_serial;
    let value = evaluate_in(formula, &mut ctx);
    let refs = references(formula);
    json!({
        "formula": formula,
        "value": value.to_json(),
        "display": value.display(),
        "type": value.type_name(),
        "is_error": matches!(value, Value::Error(_)),
        "references": refs,
    })
}

/// Extracts cell/range references and function names from a formula.
pub fn references(formula: &str) -> Json {
    let toks = lex(formula.trim().trim_start_matches('=')).unwrap_or_default();
    let mut refs: Vec<String> = Vec::new();
    let mut funcs: Vec<String> = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        match t {
            Tok::Ref(r) => {
                if !refs.contains(r) {
                    refs.push(r.clone());
                }
            },
            Tok::Ident(n) if toks.get(i + 1) == Some(&Tok::LParen) && !funcs.contains(n) => {
                funcs.push(n.clone())
            },
            _ => {},
        }
    }
    json!({ "cells": refs, "functions": funcs })
}

/// Recalculates every formula in a sheet and compares with the cached value.
pub fn recalculate(wb: &Workbook, sheet: usize, today_serial: Option<f64>) -> Json {
    let Some(s) = wb.sheets.get(sheet) else {
        return json!([]);
    };
    let mut keys: Vec<&(usize, usize)> = s.formulas.keys().collect();
    keys.sort();
    let mut out = Vec::new();
    let mut mismatches = 0;
    let mut errors = 0;
    for (r, c) in keys {
        let f = &s.formulas[&(*r, *c)];
        let mut ctx = Context::new(wb, sheet);
        ctx.today_serial = today_serial;
        let v = evaluate_in(f, &mut ctx);
        let cached = Value::from_cell(s.get(*r, *c));
        let matches = match (&v, &cached) {
            (Value::Number(a), Value::Number(b)) => (a - b).abs() < 1e-6 * a.abs().max(1.0),
            (Value::Error(_), _) => false,
            (a, b) => a.display() == b.display(),
        };
        if matches!(v, Value::Error(_)) {
            errors += 1;
        } else if !matches {
            mismatches += 1;
        }
        out.push(json!({
            "cell": format!("{}{}", column_letter(c + 1), r + 1),
            "formula": f,
            "cached": cached.to_json(),
            "computed": v.to_json(),
            "matches_cached": matches,
            "unsupported": matches!(&v, Value::Error(e) if e.starts_with("#NAME?") || e.starts_with("#UNSUPPORTED") || e.starts_with("#VOLATILE"))
        }));
    }
    json!({
        "sheet": s.name,
        "formulas": out.len(),
        "mismatches": mismatches,
        "errors": errors,
        "cells": out
    })
}

/// Builds a throw-away workbook from a JSON grid (array of rows, or `{"A1": v}` map).
pub fn workbook_from_grid(grid: &Json) -> Option<Workbook> {
    let mut sheet = Sheet::new("Sheet1");
    match grid {
        Json::Array(rows) => {
            for (r, row) in rows.iter().enumerate() {
                let Json::Array(cells) = row else { return None };
                for (c, v) in cells.iter().enumerate() {
                    set_grid_cell(&mut sheet, r, c, v);
                }
            }
        },
        Json::Object(map) => {
            for (k, v) in map {
                let (r, c) = parse_cell_ref(k)?;
                set_grid_cell(&mut sheet, r, c, v);
            }
        },
        _ => return None,
    }
    sheet.normalize();
    Some(Workbook {
        sheets: vec![sheet],
        format: "grid".into(),
        defined_names: vec![],
        has_macros: false,
    })
}

fn set_grid_cell(sheet: &mut Sheet, r: usize, c: usize, v: &Json) {
    if let Json::String(s) = v {
        if let Some(f) = s.strip_prefix('=') {
            sheet.formulas.insert((r, c), f.to_string());
            sheet.set(r, c, Cell::Empty);
            return;
        }
    }
    sheet.set(r, c, Cell::from_json(v));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(f: &str) -> Value {
        let wb =
            workbook_from_grid(&json!([[1, 2, 3, "x"], [4, 5, 6, "y"], [7, 8, "=A1+B2", "z"]]))
                .unwrap();
        let mut ctx = Context::new(&wb, 0);
        evaluate_in(f, &mut ctx)
    }

    #[test]
    fn arithmetic_and_precedence() {
        assert_eq!(ev("=1+2*3"), Value::Number(7.0));
        assert_eq!(ev("=(1+2)*3"), Value::Number(9.0));
        assert_eq!(ev("=2^3^2"), Value::Number(512.0));
        // Excel semantics: unary minus binds tighter than `^`.
        assert_eq!(ev("=-2^2"), Value::Number(4.0));
        assert_eq!(ev("=50%"), Value::Number(0.5));
        assert_eq!(ev("=1/0"), Value::Error("#DIV/0!".into()));
        assert_eq!(ev("=\"a\"&\"b\"&1"), Value::Text("ab1".into()));
        assert_eq!(ev("=1<2"), Value::Bool(true));
        assert_eq!(ev("=\"abc\"=\"ABC\""), Value::Bool(true));
    }

    #[test]
    fn references_and_functions() {
        assert_eq!(ev("=SUM(A1:C2)"), Value::Number(21.0));
        assert_eq!(ev("=C3"), Value::Number(6.0));
        assert_eq!(ev("=AVERAGE(A:A)"), Value::Number(4.0));
        assert_eq!(ev("=COUNTA(D1:D3)"), Value::Number(3.0));
        assert_eq!(ev("=COUNTIF(A1:C2,\">2\")"), Value::Number(4.0));
        assert_eq!(ev("=SUMIF(A1:A3,\">1\",B1:B3)"), Value::Number(13.0));
        assert_eq!(ev("=IF(A1>0,\"pos\",\"neg\")"), Value::Text("pos".into()));
        assert_eq!(ev("=VLOOKUP(4,A1:D3,4,FALSE)"), Value::Text("y".into()));
        assert_eq!(ev("=INDEX(A1:C3,2,3)"), Value::Number(6.0));
        assert_eq!(ev("=MATCH(7,A1:A3,0)"), Value::Number(3.0));
        assert_eq!(ev("=ROUND(2.345,2)"), Value::Number(2.35));
        assert_eq!(ev("=LEN(\"héllo\")"), Value::Number(5.0));
        assert_eq!(ev("=MID(\"abcdef\",2,3)"), Value::Text("bcd".into()));
        assert_eq!(ev("=SUBSTITUTE(\"aaa\",\"a\",\"b\",2)"), Value::Text("aba".into()));
        assert_eq!(ev("=TEXTJOIN(\"-\",TRUE,A1:C1)"), Value::Text("1-2-3".into()));
        assert_eq!(ev("=DATE(2024,1,1)"), Value::Number(45292.0));
        assert_eq!(ev("=YEAR(45292)"), Value::Number(2024.0));
        assert_eq!(ev("=MONTH(DATE(2024,2,29))"), Value::Number(2.0));
        assert_eq!(ev("=DAY(DATE(2024,2,29))"), Value::Number(29.0));
        assert_eq!(ev("=EOMONTH(DATE(2024,2,10),0)"), ev("=DATE(2024,2,29)"));
        assert_eq!(ev("=WEEKDAY(DATE(2024,1,1))"), Value::Number(2.0)); // Monday
        assert_eq!(ev("=NETWORKDAYS(DATE(2024,1,1),DATE(2024,1,7))"), Value::Number(5.0));
        assert!(
            matches!(ev("=PMT(0.05/12,360,200000)"), Value::Number(n) if (n + 1073.64).abs() < 0.01)
        );
        assert_eq!(ev("=IFERROR(1/0,\"x\")"), Value::Text("x".into()));
        assert_eq!(ev("=TEXT(1234.5,\"#,##0.00\")"), Value::Text("1,234.50".into()));
        assert_eq!(ev("=TEXT(DATE(2024,3,5),\"yyyy-mm-dd\")"), Value::Text("2024-03-05".into()));
        assert_eq!(ev("=CONVERT(1,\"mi\",\"km\")"), Value::Number(1.609344));
        assert_eq!(ev("=ROMAN(1994)"), Value::Text("MCMXCIV".into()));
        assert_eq!(
            ev("={1,2;3,4}"),
            Value::Array(vec![
                vec![Value::Number(1.0), Value::Number(2.0)],
                vec![Value::Number(3.0), Value::Number(4.0)]
            ])
        );
        assert!(matches!(ev("=FOO(1)"), Value::Error(e) if e.starts_with("#NAME?")));
        assert!(matches!(ev("=NOW()"), Value::Error(e) if e.starts_with("#VOLATILE")));
        assert!(matches!(ev("=1+"), Value::Error(_)));
        assert!(matches!(ev("=ZZ99999"), Value::Empty));
    }

    #[test]
    fn cycles_and_recalc() {
        let wb = workbook_from_grid(&json!({ "A1": "=B1+1", "B1": "=A1+1", "C1": "=SUM(1,2)" }))
            .unwrap();
        let mut ctx = Context::new(&wb, 0);
        assert_eq!(evaluate_in("=A1", &mut ctx), Value::Error("#CIRCULAR!".into()));
        let r = recalculate(&wb, 0, None);
        assert_eq!(r["formulas"], 3);
        assert_eq!(r["errors"], 2);
        assert_eq!(
            references("=SUM(A1:B2)+Sheet2!C3*LEN(\"x\")")["cells"],
            json!(["A1:B2", "Sheet2!C3"])
        );
    }

    #[test]
    fn date_roundtrip() {
        for (y, m, d) in [
            (1900, 1, 1),
            (1900, 2, 28),
            (1900, 3, 1),
            (1999, 12, 31),
            (2000, 2, 29),
            (2024, 7, 15),
            (2100, 1, 1),
        ] {
            let (s, _) = iso_to_serial(&format!("{y:04}-{m:02}-{d:02}")).unwrap();
            assert_eq!(serial_to_ymd(s), (y, m, d), "serial {s}");
        }
    }
}
