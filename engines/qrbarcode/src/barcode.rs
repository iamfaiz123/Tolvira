//! 1-D barcodes: EAN-13/8, UPC-A/E, Code 128 (auto A/B/C), Code 39, ITF-14 /
//! Interleaved 2 of 5, Codabar — encoding to bar patterns, SVG/PNG rendering,
//! check-digit validation and a scanline reader for EAN/UPC symbols.

use crate::qr::parse_color;
use engine_shared::{EngineError, EngineResult};
use image::{ImageBuffer, Rgba};
use serde::Deserialize;
use serde_json::{json, Value};

/// Rendering options for 1-D barcodes.
#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct BarStyle {
    /// Pixel width of the narrowest bar (module) for PNG (default 3).
    pub module_width: u32,
    /// Bar height in pixels (default 80).
    pub height: u32,
    /// Quiet zone in modules (default 10).
    pub margin: u32,
    /// Bar colour (`#rrggbb`).
    pub foreground: String,
    /// Background colour (`#rrggbb`).
    pub background: String,
    /// Print the human readable text below the bars (default true).
    pub show_text: bool,
    /// Font size of the human-readable text.
    pub font_size: u32,
}

impl Default for BarStyle {
    fn default() -> Self {
        Self {
            module_width: 3,
            height: 80,
            margin: 10,
            foreground: "#000000".into(),
            background: "#ffffff".into(),
            show_text: true,
            font_size: 14,
        }
    }
}

/// Encoded symbol: sequence of modules (true = bar) plus display text.
pub struct Symbol {
    /// Module sequence (`true` = bar).
    pub modules: Vec<bool>,
    /// Human-readable text printed below the bars.
    pub text: String,
    /// Symbology name.
    pub symbology: &'static str,
    /// Value actually encoded (check digits added, normalised).
    pub encoded_value: String,
}

fn push_pattern(out: &mut Vec<bool>, pattern: &str) {
    for c in pattern.chars() {
        out.push(c == '1');
    }
}

// ------------------------------------------------------------------- EAN --

const EAN_L: [&str; 10] = [
    "0001101", "0011001", "0010011", "0111101", "0100011", "0110001", "0101111", "0111011",
    "0110111", "0001011",
];
const EAN_G: [&str; 10] = [
    "0100111", "0110011", "0011011", "0100001", "0011101", "0111001", "0000101", "0010001",
    "0001001", "0010111",
];
const EAN_R: [&str; 10] = [
    "1110010", "1100110", "1101100", "1000010", "1011100", "1001110", "1010000", "1000100",
    "1001000", "1110100",
];
/// First-digit parity patterns for EAN-13 (L = 0, G = 1).
const EAN13_PARITY: [&str; 10] = [
    "LLLLLL", "LLGLGG", "LLGGLG", "LLGGGL", "LGLLGG", "LGGLLG", "LGGGLL", "LGLGLG", "LGLGGL",
    "LGGLGL",
];

/// GTIN check digit (mod 10, weights 3/1 from the right).
pub fn gtin_check_digit(digits: &str) -> u8 {
    let sum: u32 = digits
        .chars()
        .rev()
        .enumerate()
        .map(|(i, c)| c.to_digit(10).unwrap_or(0) * if i % 2 == 0 { 3 } else { 1 })
        .sum();
    ((10 - sum % 10) % 10) as u8
}

fn digits_only(s: &str, what: &str) -> EngineResult<String> {
    let d: String = s.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
    if d.is_empty() {
        return Err(EngineError::empty(what));
    }
    if !d.chars().all(|c| c.is_ascii_digit()) {
        return Err(EngineError::invalid(format!("{what} must contain digits only")));
    }
    Ok(d)
}

/// Accepts 12 digits (check digit appended) or 13 (verified).
pub fn ean13(input: &str) -> EngineResult<Symbol> {
    let d = digits_only(input, "EAN-13")?;
    let d = match d.len() {
        12 => format!("{d}{}", gtin_check_digit(&d)),
        13 => {
            if gtin_check_digit(&d[..12]) != d[12..].parse::<u8>().unwrap_or(0) {
                return Err(EngineError::invalid(format!(
                    "EAN-13 check digit is wrong (expected {})",
                    gtin_check_digit(&d[..12])
                )));
            }
            d
        },
        n => return Err(EngineError::invalid(format!("EAN-13 needs 12 or 13 digits, got {n}"))),
    };
    let digits: Vec<usize> = d.chars().map(|c| c.to_digit(10).unwrap_or(0) as usize).collect();
    let parity = EAN13_PARITY[digits[0]];
    let mut m = Vec::with_capacity(95);
    push_pattern(&mut m, "101");
    for (i, p) in parity.chars().enumerate() {
        push_pattern(
            &mut m,
            if p == 'L' {
                EAN_L[digits[i + 1]]
            } else {
                EAN_G[digits[i + 1]]
            },
        );
    }
    push_pattern(&mut m, "01010");
    for &dg in &digits[7..13] {
        push_pattern(&mut m, EAN_R[dg]);
    }
    push_pattern(&mut m, "101");
    Ok(Symbol {
        modules: m,
        text: d.clone(),
        symbology: "EAN-13",
        encoded_value: d,
    })
}

/// EAN-8 (7 digits + check digit, or 8 digits validated).
pub fn ean8(input: &str) -> EngineResult<Symbol> {
    let d = digits_only(input, "EAN-8")?;
    let d = match d.len() {
        7 => format!("{d}{}", gtin_check_digit(&d)),
        8 => {
            if gtin_check_digit(&d[..7]) != d[7..].parse::<u8>().unwrap_or(0) {
                return Err(EngineError::invalid(format!(
                    "EAN-8 check digit is wrong (expected {})",
                    gtin_check_digit(&d[..7])
                )));
            }
            d
        },
        n => return Err(EngineError::invalid(format!("EAN-8 needs 7 or 8 digits, got {n}"))),
    };
    let digits: Vec<usize> = d.chars().map(|c| c.to_digit(10).unwrap_or(0) as usize).collect();
    let mut m = Vec::with_capacity(67);
    push_pattern(&mut m, "101");
    for &dg in &digits[..4] {
        push_pattern(&mut m, EAN_L[dg]);
    }
    push_pattern(&mut m, "01010");
    for &dg in &digits[4..] {
        push_pattern(&mut m, EAN_R[dg]);
    }
    push_pattern(&mut m, "101");
    Ok(Symbol {
        modules: m,
        text: d.clone(),
        symbology: "EAN-8",
        encoded_value: d,
    })
}

/// UPC-A is EAN-13 with a leading zero.
pub fn upca(input: &str) -> EngineResult<Symbol> {
    let d = digits_only(input, "UPC-A")?;
    if d.len() != 11 && d.len() != 12 {
        return Err(EngineError::invalid(format!("UPC-A needs 11 or 12 digits, got {}", d.len())));
    }
    let mut s = ean13(&format!("0{d}"))
        .map_err(|e| EngineError::invalid(e.message.replace("EAN-13", "UPC-A")))?;
    s.symbology = "UPC-A";
    s.text = s.encoded_value[1..].to_string();
    s.encoded_value = s.text.clone();
    Ok(s)
}

/// UPC-E: 6 data digits + number system 0/1 + check digit, zero-suppressed.
pub fn upce(input: &str) -> EngineResult<Symbol> {
    let d = digits_only(input, "UPC-E")?;
    let (ns, data, check) = match d.len() {
        6 => (0usize, d.clone(), None),
        7 => (d[..1].parse::<usize>().unwrap_or(0), d[1..].to_string(), None),
        8 => (
            d[..1].parse::<usize>().unwrap_or(0),
            d[1..7].to_string(),
            Some(d[7..].parse::<u8>().unwrap_or(0)),
        ),
        n => return Err(EngineError::invalid(format!("UPC-E needs 6-8 digits, got {n}"))),
    };
    if ns > 1 {
        return Err(EngineError::invalid("UPC-E number system must be 0 or 1"));
    }
    // Expand to UPC-A to compute the check digit.
    let x: Vec<char> = data.chars().collect();
    let expanded = match x[5] {
        '0' | '1' | '2' => format!("{ns}{}{}{}0000{}{}{}", x[0], x[1], x[5], x[2], x[3], x[4]),
        '3' => format!("{ns}{}{}{}00000{}{}", x[0], x[1], x[2], x[3], x[4]),
        '4' => format!("{ns}{}{}{}{}00000{}", x[0], x[1], x[2], x[3], x[4]),
        _ => format!("{ns}{}{}{}{}{}0000{}", x[0], x[1], x[2], x[3], x[4], x[5]),
    };
    let cd = gtin_check_digit(&expanded);
    if let Some(c) = check {
        if c != cd {
            return Err(EngineError::invalid(format!(
                "UPC-E check digit is wrong (expected {cd})"
            )));
        }
    }
    const UPCE_PARITY_0: [&str; 10] = [
        "EEEOOO", "EEOEOO", "EEOOEO", "EEOOOE", "EOEEOO", "EOOEEO", "EOOOEE", "EOEOEO", "EOEOOE",
        "EOOEOE",
    ];
    let parity: String = UPCE_PARITY_0[cd as usize]
        .chars()
        .map(|c| {
            if ns == 1 {
                if c == 'E' {
                    'O'
                } else {
                    'E'
                }
            } else {
                c
            }
        })
        .collect();
    let mut m = Vec::new();
    push_pattern(&mut m, "101");
    for (i, p) in parity.chars().enumerate() {
        let dg = x[i].to_digit(10).unwrap_or(0) as usize;
        push_pattern(&mut m, if p == 'O' { EAN_L[dg] } else { EAN_G[dg] });
    }
    push_pattern(&mut m, "010101");
    let text = format!("{ns}{data}{cd}");
    Ok(Symbol {
        modules: m,
        text: text.clone(),
        symbology: "UPC-E",
        encoded_value: format!("{expanded}{cd}"),
    })
}

// --------------------------------------------------------------- Code 128 --

const CODE128: [&str; 107] = [
    "11011001100",
    "11001101100",
    "11001100110",
    "10010011000",
    "10010001100",
    "10001001100",
    "10011001000",
    "10011000100",
    "10001100100",
    "11001001000",
    "11001000100",
    "11000100100",
    "10110011100",
    "10011011100",
    "10011001110",
    "10111001100",
    "10011101100",
    "10011100110",
    "11001110010",
    "11001011100",
    "11001001110",
    "11011100100",
    "11001110100",
    "11101101110",
    "11101001100",
    "11100101100",
    "11100100110",
    "11101100100",
    "11100110100",
    "11100110010",
    "11011011000",
    "11011000110",
    "11000110110",
    "10100011000",
    "10001011000",
    "10001000110",
    "10110001000",
    "10001101000",
    "10001100010",
    "11010001000",
    "11000101000",
    "11000100010",
    "10110111000",
    "10110001110",
    "10001101110",
    "10111011000",
    "10111000110",
    "10001110110",
    "11101110110",
    "11010001110",
    "11000101110",
    "11011101000",
    "11011100010",
    "11011101110",
    "11101011000",
    "11101000110",
    "11100010110",
    "11101101000",
    "11101100010",
    "11100011010",
    "11101111010",
    "11001000010",
    "11110001010",
    "10100110000",
    "10100001100",
    "10010110000",
    "10010000110",
    "10000101100",
    "10000100110",
    "10110010000",
    "10110000100",
    "10011010000",
    "10011000010",
    "10000110100",
    "10000110010",
    "11000010010",
    "11001010000",
    "11110111010",
    "11000010100",
    "10001111010",
    "10100111100",
    "10010111100",
    "10010011110",
    "10111100100",
    "10011110100",
    "10011110010",
    "11110100100",
    "11110010100",
    "11110010010",
    "11011011110",
    "11011110110",
    "11110110110",
    "10101111000",
    "10100011110",
    "10001011110",
    "10111101000",
    "10111100010",
    "11110101000",
    "11110100010",
    "10111011110",
    "10111101110",
    "11101011110",
    "11110101110",
    "11010000100",
    "11010010000",
    "11010011100",
    "1100011101011",
];
const C128_START_A: usize = 103;
const C128_START_B: usize = 104;
const C128_START_C: usize = 105;
const C128_STOP: usize = 106;
const C128_CODE_A: usize = 101;
const C128_CODE_B: usize = 100;
const C128_CODE_C: usize = 99;
const C128_FNC1: usize = 102;

fn c128_b_value(c: char) -> Option<usize> {
    let b = c as u32;
    if (32..=127).contains(&b) {
        Some((b - 32) as usize)
    } else {
        None
    }
}

fn c128_a_value(c: char) -> Option<usize> {
    let b = c as u32;
    if (32..=95).contains(&b) {
        Some((b - 32) as usize)
    } else if b < 32 {
        Some((b + 64) as usize)
    } else {
        None
    }
}

/// Code 128 with automatic code-set selection (C for digit runs, B otherwise,
/// A for control characters). `gs1` prefixes FNC1 for GS1-128.
pub fn code128(input: &str, gs1: bool) -> EngineResult<Symbol> {
    if input.is_empty() {
        return Err(EngineError::empty("data"));
    }
    if input.len() > 80 {
        return Err(EngineError::limit(
            "Code 128 data is limited to 80 characters here (symbol would be too wide to scan)",
        ));
    }
    if let Some(bad) = input.chars().find(|c| (*c as u32) > 127) {
        return Err(EngineError::invalid(format!(
            "Code 128 only encodes ASCII; `{bad}` is not allowed"
        )));
    }
    let chars: Vec<char> = input.chars().collect();
    let mut codes: Vec<usize> = Vec::new();
    let digit_run = |from: usize| chars[from..].iter().take_while(|c| c.is_ascii_digit()).count();
    let mut i = 0;
    // Choose start set.
    let mut set = if digit_run(0) >= 4
        || (digit_run(0) == chars.len() && chars.len() >= 2 && chars.len().is_multiple_of(2))
    {
        'C'
    } else if chars.iter().any(|c| (*c as u32) < 32)
        && !chars.iter().any(|c| c.is_ascii_lowercase())
    {
        'A'
    } else {
        'B'
    };
    codes.push(match set {
        'A' => C128_START_A,
        'B' => C128_START_B,
        _ => C128_START_C,
    });
    if gs1 {
        codes.push(C128_FNC1);
    }
    while i < chars.len() {
        let run = digit_run(i);
        match set {
            'C' => {
                if run >= 2 {
                    let v = chars[i..i + 2]
                        .iter()
                        .collect::<String>()
                        .parse::<usize>()
                        .unwrap_or(0);
                    codes.push(v);
                    i += 2;
                } else {
                    set = if chars[i..].iter().any(|c| (*c as u32) < 32)
                        && !chars[i..].iter().any(|c| c.is_ascii_lowercase())
                    {
                        'A'
                    } else {
                        'B'
                    };
                    codes.push(if set == 'A' { C128_CODE_A } else { C128_CODE_B });
                }
            },
            _ => {
                if run >= 4 && (run % 2 == 0 || run >= 6) {
                    codes.push(C128_CODE_C);
                    set = 'C';
                    if run % 2 == 1 {
                        // Encode one digit in B first.
                        codes.pop();
                        let v = c128_b_value(chars[i]).unwrap_or(C128_CODE_B);
                        codes.push(v);
                        i += 1;
                        codes.push(C128_CODE_C);
                    }
                    continue;
                }
                let c = chars[i];
                let v = if set == 'B' { c128_b_value(c) } else { c128_a_value(c) };
                match v {
                    Some(v) => {
                        codes.push(v);
                        i += 1;
                    },
                    None => {
                        // Switch set.
                        set = if set == 'B' { 'A' } else { 'B' };
                        codes.push(if set == 'A' { C128_CODE_A } else { C128_CODE_B });
                    },
                }
            },
        }
    }
    let checksum = codes
        .iter()
        .enumerate()
        .fold(0usize, |acc, (i, c)| acc + c * i.max(1))
        % 103;
    codes.push(checksum);
    codes.push(C128_STOP);
    let mut m = Vec::new();
    for c in &codes {
        push_pattern(&mut m, CODE128[*c]);
    }
    Ok(Symbol {
        modules: m,
        text: input.to_string(),
        symbology: if gs1 { "GS1-128" } else { "Code 128" },
        encoded_value: input.to_string(),
    })
}

// ---------------------------------------------------------------- Code 39 --

const CODE39_CHARS: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-. $/+%";
const CODE39_PATTERNS: [&str; 44] = [
    "nnnwwnwnn",
    "wnnwnnnnw",
    "nnwwnnnnw",
    "wnwwnnnnn",
    "nnnwwnnnw",
    "wnnwwnnnn",
    "nnwwwnnnn",
    "nnnwnnwnw",
    "wnnwnnwnn",
    "nnwwnnwnn",
    "wnnnnwnnw",
    "nnwnnwnnw",
    "wnwnnwnnn",
    "nnnnwwnnw",
    "wnnnwwnnn",
    "nnwnwwnnn",
    "nnnnnwwnw",
    "wnnnnwwnn",
    "nnwnnwwnn",
    "nnnnwwwnn",
    "wnnnnnnww",
    "nnwnnnnww",
    "wnwnnnnwn",
    "nnnnwnnww",
    "wnnnwnnwn",
    "nnwnwnnwn",
    "nnnnnnwww",
    "wnnnnnwwn",
    "nnwnnnwwn",
    "nnnnwnwwn",
    "wwnnnnnnw",
    "nwwnnnnnw",
    "wwwnnnnnn",
    "nwnnwnnnw",
    "wwnnwnnnn",
    "nwwnwnnnn",
    "nwnnnnwnw",
    "wwnnnnwnn",
    "nwwnnnwnn",
    "nwnwnwnnn",
    "nwnwnnnwn",
    "nwnnnwnwn",
    "nnnwnwnwn",
    "nwnwnwnnn",
];

/// Code 39 (uppercase alphanumerics and `-. $/+%`), optional mod-43 check digit.
pub fn code39(input: &str, check_digit: bool) -> EngineResult<Symbol> {
    let text = input.to_ascii_uppercase();
    if text.is_empty() {
        return Err(EngineError::empty("data"));
    }
    if text.len() > 60 {
        return Err(EngineError::limit("Code 39 data is limited to 60 characters"));
    }
    if let Some(bad) = text.chars().find(|c| !CODE39_CHARS.contains(*c)) {
        return Err(EngineError::invalid(format!(
            "Code 39 cannot encode `{bad}` (allowed: A-Z, 0-9, space, - . $ / + %)"
        )));
    }
    let mut body = text.clone();
    if check_digit {
        let sum: usize = text.chars().filter_map(|c| CODE39_CHARS.find(c)).sum();
        if let Some(check) = CODE39_CHARS.chars().nth(sum % 43) {
            body.push(check);
        }
    }
    let mut m = Vec::new();
    let emit = |m: &mut Vec<bool>, pattern: &str| {
        for (i, c) in pattern.chars().enumerate() {
            let bar = i % 2 == 0;
            let width = if c == 'w' { 3 } else { 1 };
            for _ in 0..width {
                m.push(bar);
            }
        }
        m.push(false); // inter-character gap
    };
    emit(&mut m, CODE39_PATTERNS[43]); // '*' start
    for idx in body.chars().filter_map(|c| CODE39_CHARS.find(c)) {
        emit(&mut m, CODE39_PATTERNS[idx]);
    }
    emit(&mut m, CODE39_PATTERNS[43]);
    m.pop();
    Ok(Symbol {
        modules: m,
        text,
        symbology: "Code 39",
        encoded_value: body,
    })
}

// --------------------------------------------------------------- ITF / I2of5 --

const ITF_PATTERNS: [&str; 10] = [
    "nnwwn", "wnnnw", "nwnnw", "wwnnn", "nnwnw", "wnwnn", "nwwnn", "nnnww", "wnnwn", "nwnwn",
];

/// Interleaved 2 of 5 (even digit count), optionally as ITF-14 with bearer bars.
pub fn itf(input: &str, itf14: bool) -> EngineResult<Symbol> {
    let mut d = digits_only(input, "ITF")?;
    if itf14 {
        d = match d.len() {
            13 => format!("{d}{}", gtin_check_digit(&d)),
            14 => {
                if gtin_check_digit(&d[..13]) != d[13..].parse::<u8>().unwrap_or(0) {
                    return Err(EngineError::invalid(format!(
                        "ITF-14 check digit is wrong (expected {})",
                        gtin_check_digit(&d[..13])
                    )));
                }
                d
            },
            n => {
                return Err(EngineError::invalid(format!("ITF-14 needs 13 or 14 digits, got {n}")))
            },
        };
    } else if d.len() % 2 == 1 {
        d = format!("0{d}");
    }
    if d.len() > 40 {
        return Err(EngineError::limit("Interleaved 2 of 5 is limited to 40 digits"));
    }
    let mut m = Vec::new();
    push_pattern(&mut m, "1010");
    let digits: Vec<usize> = d.chars().map(|c| c.to_digit(10).unwrap_or(0) as usize).collect();
    for pair in digits.chunks(2) {
        let (a, b) = (ITF_PATTERNS[pair[0]], ITF_PATTERNS[pair[1]]);
        for (ca, cb) in a.chars().zip(b.chars()) {
            m.extend(std::iter::repeat_n(true, if ca == 'w' { 3 } else { 1 }));
            m.extend(std::iter::repeat_n(false, if cb == 'w' { 3 } else { 1 }));
        }
    }
    push_pattern(&mut m, "11101");
    Ok(Symbol {
        modules: m,
        text: d.clone(),
        symbology: if itf14 { "ITF-14" } else { "Interleaved 2 of 5" },
        encoded_value: d,
    })
}

// ---------------------------------------------------------------- Codabar --

const CODABAR_CHARS: &str = "0123456789-$:/.+ABCD";
const CODABAR_PATTERNS: [&str; 20] = [
    "nnnnnww", "nnnnwwn", "nnnwnnw", "wwnnnnn", "nnwnnwn", "wnnnnwn", "nwnnnnw", "nwnnwnn",
    "nwwnnnn", "wnnwnnn", "nnnwwnn", "nnwwnnn", "wnnnwnw", "wnwnnnw", "wnwnwnn", "nnwnwnw",
    "nnwwnwn", "nwnwnnw", "nnnwnww", "nnnwwwn",
];

/// Codabar (digits and `-$:/.+` with A–D start/stop guards).
pub fn codabar(input: &str) -> EngineResult<Symbol> {
    let mut text = input.trim().to_ascii_uppercase();
    if text.is_empty() {
        return Err(EngineError::empty("data"));
    }
    let has_guards = text.len() >= 2
        && text.starts_with(['A', 'B', 'C', 'D'])
        && text.ends_with(['A', 'B', 'C', 'D']);
    if !has_guards {
        text = format!("A{text}B");
    }
    if let Some(bad) = text.chars().find(|c| !CODABAR_CHARS.contains(*c)) {
        return Err(EngineError::invalid(format!(
            "Codabar cannot encode `{bad}` (digits, - $ : / . + and A-D guards)"
        )));
    }
    if text[1..text.len() - 1].chars().any(|c| "ABCD".contains(c)) {
        return Err(EngineError::invalid(
            "Codabar start/stop letters A-D may only appear at the ends",
        ));
    }
    let mut m = Vec::new();
    for (ci, c) in text.chars().enumerate() {
        let Some(p) = CODABAR_CHARS.find(c).map(|i| CODABAR_PATTERNS[i]) else {
            continue;
        };
        for (i, w) in p.chars().enumerate() {
            for _ in 0..(if w == 'w' { 3 } else { 1 }) {
                m.push(i % 2 == 0);
            }
        }
        if ci + 1 < text.len() {
            m.push(false);
        }
    }
    Ok(Symbol {
        modules: m,
        text: text.clone(),
        symbology: "Codabar",
        encoded_value: text,
    })
}

// -------------------------------------------------------------- rendering --

fn runs(modules: &[bool]) -> Vec<(usize, usize, bool)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < modules.len() {
        let start = i;
        while i < modules.len() && modules[i] == modules[start] {
            i += 1;
        }
        out.push((start, i - start, modules[start]));
    }
    out
}

/// Renders a symbol as SVG.
pub fn to_svg(sym: &Symbol, style: &BarStyle) -> EngineResult<String> {
    let fg = parse_color(&style.foreground)?;
    let bg = parse_color(&style.background)?;
    let total_w = sym.modules.len() as u32 + style.margin * 2;
    let text_h = if style.show_text { style.font_size + 6 } else { 0 };
    let h = style.height;
    let mw = style.module_width.max(1);
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {total_w} {}" width="{}" height="{}" shape-rendering="crispEdges">"#,
        h + text_h,
        total_w * mw,
        (h + text_h) * mw / mw.max(1)
    );
    let css = |c: [u8; 4]| {
        if c[3] == 0 {
            "none".to_string()
        } else {
            format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
        }
    };
    if bg[3] > 0 {
        svg.push_str(&format!(r#"<rect width="100%" height="100%" fill="{}"/>"#, css(bg)));
    }
    let mut path = String::new();
    for (start, len, bar) in runs(&sym.modules) {
        if bar {
            path.push_str(&format!("M{} 0h{len}v{h}h-{len}z", start as u32 + style.margin));
        }
    }
    svg.push_str(&format!(r#"<path d="{path}" fill="{}"/>"#, css(fg)));
    if style.show_text {
        svg.push_str(&format!(r#"<text x="{}" y="{}" font-family="monospace" font-size="{}" text-anchor="middle" fill="{}">{}</text>"#, total_w / 2, h + style.font_size, style.font_size, css(fg), sym.text.replace('&', "&amp;").replace('<', "&lt;")));
    }
    svg.push_str("</svg>");
    Ok(svg)
}

/// Renders a symbol as PNG.
pub fn to_png(sym: &Symbol, style: &BarStyle) -> EngineResult<Vec<u8>> {
    let fg = parse_color(&style.foreground)?;
    let bg = parse_color(&style.background)?;
    let mw = style.module_width.max(1);
    let w = (sym.modules.len() as u32 + style.margin * 2) * mw;
    let h = style.height.max(10);
    engine_shared::limits::check_dimensions(w, h)?;
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(w, h, Rgba(bg));
    for (i, bar) in sym.modules.iter().enumerate() {
        if *bar {
            let x0 = (i as u32 + style.margin) * mw;
            for x in x0..x0 + mw {
                for y in 0..h {
                    img.put_pixel(x, y, Rgba(fg));
                }
            }
        }
    }
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| EngineError::internal(e.to_string()))?;
    Ok(out.into_inner())
}

/// Encodes `data` with the named symbology (`ean13`, `ean8`, `upca`, `upce`, `code128`, `code39`,
/// `itf`, `itf14`, `codabar`).
pub fn encode(symbology: &str, data: &str, options: &Value) -> EngineResult<Symbol> {
    let flag = |k: &str| options.get(k).and_then(Value::as_bool).unwrap_or(false);
    match symbology
        .trim()
        .to_ascii_lowercase()
        .replace(['-', '_', ' '], "")
        .as_str()
    {
        "ean13" | "ean" | "gtin13" | "isbn" | "jan" => ean13(data),
        "ean8" | "gtin8" => ean8(data),
        "upca" | "upc" | "gtin12" => upca(data),
        "upce" => upce(data),
        "code128" | "gs1128" | "ean128" => code128(
            data,
            flag("gs1")
                || symbology.to_ascii_lowercase().contains("gs1")
                || symbology.to_ascii_lowercase().contains("ean128"),
        ),
        "code39" | "code3of9" => code39(data, flag("check_digit")),
        "itf14" | "gtin14" => itf(data, true),
        "itf" | "interleaved2of5" | "i2of5" | "itf25" => itf(data, false),
        "codabar" | "nw7" => codabar(data),
        "code93" | "pdf417" | "datamatrix" | "aztec" | "maxicode" => {
            Err(EngineError::unsupported(format!(
                "{symbology} is not supported by this engine version (supported: EAN-13, EAN-8, \
                 UPC-A, UPC-E, Code 128/GS1-128, Code 39, ITF-14, Interleaved 2 of 5, Codabar, QR)"
            )))
        },
        other => Err(EngineError::invalid(format!("Unknown symbology `{other}`"))),
    }
}

/// Encodes and renders in the requested formats (`svg`, `png`) with metadata.
pub fn generate(
    symbology: &str,
    data: &str,
    style: &BarStyle,
    options: &Value,
    formats: &[String],
) -> EngineResult<Value> {
    let sym = encode(symbology, data, options)?;
    let want = |f: &str| formats.is_empty() || formats.iter().any(|x| x.eq_ignore_ascii_case(f));
    let mut out = json!({
        "symbology": sym.symbology,
        "text": sym.text,
        "encoded_value": sym.encoded_value,
        "modules": sym.modules.len(),
        "width_px": (sym.modules.len() as u32 + style.margin * 2) * style.module_width.max(1),
        "height_px": style.height,
        "pattern": sym.modules.iter().map(|b| if *b { '1' } else { '0' }).collect::<String>(),
    });
    if want("svg") {
        out["svg"] = json!(to_svg(&sym, style)?);
    }
    if want("png") {
        let png = to_png(&sym, style)?;
        out["png"] = json!(engine_shared::b64::encode(&png));
        out["png_data_url"] =
            json!(format!("data:image/png;base64,{}", engine_shared::b64::encode(&png)));
    }
    Ok(out)
}

// ------------------------------------------------------------- validation --

/// Validates GTIN-8/12/13/14, ISBN-10/13 and ISSN numbers and reports the
/// expected check digit.
pub fn validate(code: &str) -> EngineResult<Value> {
    let raw = code.trim();
    engine_shared::error::require_text(raw, "code")?;
    let d: String = raw.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
    let upper = d.to_ascii_uppercase();
    if upper.len() == 10
        && upper[..9].chars().all(|c| c.is_ascii_digit())
        && (upper.ends_with('X') || upper.ends_with(|c: char| c.is_ascii_digit()))
    {
        // ISBN-10
        let sum: u32 = upper
            .chars()
            .take(9)
            .enumerate()
            .map(|(i, c)| c.to_digit(10).unwrap_or(0) * (10 - i as u32))
            .sum();
        let expected = (11 - sum % 11) % 11;
        let exp_char = if expected == 10 {
            'X'
        } else {
            char::from_digit(expected, 10).unwrap_or('0')
        };
        let actual = upper.chars().last().unwrap_or('0');
        let isbn13 = format!("978{}", &upper[..9]);
        return Ok(json!({
            "input": raw,
            "type": "ISBN-10",
            "valid": actual == exp_char,
            "expected_check_digit": exp_char.to_string(),
            "actual_check_digit": actual.to_string(),
            "isbn13": format!("{isbn13}{}", gtin_check_digit(&isbn13)),
            "formatted": format!("{}-{}-{}-{}", &upper[..1], &upper[1..4], &upper[4..9], &upper[9..])
        }));
    }
    if upper.len() == 8 && upper[..7].chars().all(|c| c.is_ascii_digit()) && upper.ends_with('X') {
        let sum: u32 = upper
            .chars()
            .take(7)
            .enumerate()
            .map(|(i, c)| c.to_digit(10).unwrap_or(0) * (8 - i as u32))
            .sum();
        let expected = (11 - sum % 11) % 11;
        return Ok(json!({
            "input": raw,
            "type": "ISSN",
            "valid": expected == 10,
            "expected_check_digit": if expected == 10 { "X".to_string() } else { expected.to_string() },
            "actual_check_digit": "X"
        }));
    }
    if !d.chars().all(|c| c.is_ascii_digit()) {
        return Err(EngineError::invalid("Code must be numeric (or an ISBN-10/ISSN with X)"));
    }
    let (kind, gtin14) = match d.len() {
        8 => {
            // Could be EAN-8 or ISSN (digits only); report EAN-8/GTIN-8.
            ("EAN-8 / GTIN-8", format!("000000{d}"))
        },
        12 => ("UPC-A / GTIN-12", format!("00{d}")),
        13 => (
            if d.starts_with("978") || d.starts_with("979") {
                "ISBN-13 / EAN-13"
            } else {
                "EAN-13 / GTIN-13"
            },
            format!("0{d}"),
        ),
        14 => ("ITF-14 / GTIN-14", d.clone()),
        n => {
            return Err(EngineError::invalid(format!(
                "Expected 8, 12, 13 or 14 digits (GTIN) or a 10-digit ISBN, got {n}"
            )))
        },
    };
    let body = &d[..d.len() - 1];
    let expected = gtin_check_digit(body);
    let actual = d[d.len() - 1..].parse::<u8>().unwrap_or(0);
    let mut out = json!({
        "input": raw,
        "type": kind,
        "valid": expected == actual,
        "expected_check_digit": expected,
        "actual_check_digit": actual,
        "gtin14": gtin14,
        "digits": d.len()
    });
    if d.len() == 13 {
        let prefix: u32 = d[..3].parse().unwrap_or(0);
        out["gs1_prefix"] = json!(&d[..3]);
        out["prefix_region"] = json!(gs1_region(prefix));
        if d.starts_with("978") || d.starts_with("979") {
            out["isbn_formatted"] = json!(format!("{}-{}", &d[..3], &d[3..]));
        }
    }
    if d.len() == 8 {
        let issn_sum: u32 = d
            .chars()
            .take(7)
            .enumerate()
            .map(|(i, c)| c.to_digit(10).unwrap_or(0) * (8 - i as u32))
            .sum();
        out["valid_as_issn"] = json!((11 - issn_sum % 11) % 11 == actual as u32);
    }
    Ok(out)
}

fn gs1_region(p: u32) -> &'static str {
    match p {
        0..=19 | 30..=39 | 60..=139 => "USA & Canada (UPC compatible)",
        20..=29 | 40..=49 | 200..=299 => "Restricted distribution (in-store)",
        300..=379 => "France & Monaco",
        380 => "Bulgaria",
        383 => "Slovenia",
        385 => "Croatia",
        387 => "Bosnia and Herzegovina",
        389 => "Montenegro",
        400..=440 => "Germany",
        450..=459 | 490..=499 => "Japan",
        460..=469 => "Russia",
        470 => "Kyrgyzstan",
        471 => "Taiwan",
        474 => "Estonia",
        475 => "Latvia",
        476 => "Azerbaijan",
        477 => "Lithuania",
        478 => "Uzbekistan",
        479 => "Sri Lanka",
        480 => "Philippines",
        481 => "Belarus",
        482 => "Ukraine",
        484 => "Moldova",
        485 => "Armenia",
        486 => "Georgia",
        487 => "Kazakhstan",
        489 => "Hong Kong",
        500..=509 => "United Kingdom",
        520..=521 => "Greece",
        528 => "Lebanon",
        529 => "Cyprus",
        530 => "Albania",
        531 => "North Macedonia",
        535 => "Malta",
        539 => "Ireland",
        540..=549 => "Belgium & Luxembourg",
        560 => "Portugal",
        569 => "Iceland",
        570..=579 => "Denmark",
        590 => "Poland",
        594 => "Romania",
        599 => "Hungary",
        600..=601 => "South Africa",
        603 => "Ghana",
        608 => "Bahrain",
        611 => "Morocco",
        613 => "Algeria",
        616 => "Kenya",
        618 => "Ivory Coast",
        619 => "Tunisia",
        621 => "Syria",
        622 => "Egypt",
        624 => "Libya",
        625 => "Jordan",
        626 => "Iran",
        627 => "Kuwait",
        628 => "Saudi Arabia",
        629 => "United Arab Emirates",
        640..=649 => "Finland",
        690..=699 => "China",
        700..=709 => "Norway",
        729 => "Israel",
        730..=739 => "Sweden",
        740 => "Guatemala",
        741 => "El Salvador",
        742 => "Honduras",
        743 => "Nicaragua",
        744 => "Costa Rica",
        745 => "Panama",
        746 => "Dominican Republic",
        750 => "Mexico",
        754..=755 => "Canada",
        759 => "Venezuela",
        760..=769 => "Switzerland & Liechtenstein",
        770..=771 => "Colombia",
        773 => "Uruguay",
        775 => "Peru",
        777 => "Bolivia",
        778..=779 => "Argentina",
        780 => "Chile",
        784 => "Paraguay",
        786 => "Ecuador",
        789..=790 => "Brazil",
        800..=839 => "Italy, San Marino & Vatican",
        840..=849 => "Spain & Andorra",
        850 => "Cuba",
        858 => "Slovakia",
        859 => "Czech Republic",
        860 => "Serbia",
        865 => "Mongolia",
        867 => "North Korea",
        868..=869 => "Turkey",
        870..=879 => "Netherlands",
        880 => "South Korea",
        884 => "Cambodia",
        885 => "Thailand",
        888 => "Singapore",
        890 => "India",
        893 => "Vietnam",
        896 => "Pakistan",
        899 => "Indonesia",
        900..=919 => "Austria",
        930..=939 => "Australia",
        940..=949 => "New Zealand",
        950 => "GS1 Global Office",
        955 => "Malaysia",
        958 => "Macau",
        960..=969 => "GS1 Global Office (GTIN-8)",
        977 => "Serial publications (ISSN)",
        978..=979 => "Bookland (ISBN)",
        980 => "Refund receipts",
        981..=984 => "Coupon identification",
        990..=999 => "Coupons",
        _ => "Unassigned / reserved",
    }
}

// ----------------------------------------------------------------- reader --

/// Scanline decoder for EAN-13 / UPC-A / EAN-8 symbols in raster images.
pub fn read(bytes: &[u8]) -> EngineResult<Value> {
    engine_shared::error::require_bytes(bytes, "image")?;
    let img = image::load_from_memory(bytes)
        .map_err(|e| EngineError::unsupported_format(format!("Cannot decode image: {e}")))?;
    engine_shared::limits::check_dimensions(img.width(), img.height())?;
    let gray = img.to_luma8();
    let (w, h) = (gray.width() as usize, gray.height() as usize);
    let mut found: Vec<Value> = Vec::new();
    // Sample many rows across the image, both orientations (rotated 90° handled by scanning columns too).
    let rows: Vec<usize> = (0..40).map(|i| (h * (i * 2 + 1)) / 80).collect();
    let mut tried_rows = 0;
    for &y in rows.iter() {
        let line: Vec<u8> = (0..w).map(|x| gray.get_pixel(x as u32, y as u32).0[0]).collect();
        tried_rows += 1;
        if let Some(v) = decode_line(&line) {
            if !found.iter().any(|f| f["value"] == v["value"]) {
                let mut v = v;
                v["row"] = json!(y);
                found.push(v);
            }
        }
    }
    if found.is_empty() {
        let cols: Vec<usize> = (0..40).map(|i| (w * (i * 2 + 1)) / 80).collect();
        for &x in cols.iter() {
            let line: Vec<u8> = (0..h).map(|y| gray.get_pixel(x as u32, y as u32).0[0]).collect();
            if let Some(v) = decode_line(&line) {
                if !found.iter().any(|f| f["value"] == v["value"]) {
                    let mut v = v;
                    v["column"] = json!(x);
                    v["rotated"] = json!(true);
                    found.push(v);
                }
            }
        }
    }
    if found.is_empty() {
        return Err(EngineError::not_found(format!(
            "No EAN/UPC barcode found in {tried_rows} scanlines. Supported for reading: EAN-13, \
             UPC-A, EAN-8 (Code 128/39 reading is not implemented). Use a straight, well-lit, \
             in-focus photo."
        )));
    }
    Ok(json!({ "count": found.len(), "barcodes": found, "image": { "width": w, "height": h } }))
}

/// Binarises a scanline with a local threshold and returns run lengths
/// starting with a bar (dark) run.
fn run_lengths(line: &[u8]) -> (Vec<usize>, bool) {
    let min = *line.iter().min().unwrap_or(&0) as f64;
    let max = *line.iter().max().unwrap_or(&255) as f64;
    if max - min < 40.0 {
        return (vec![], false);
    }
    let thr = ((min + max) / 2.0) as u8;
    let bits: Vec<bool> = line.iter().map(|p| *p < thr).collect();
    let mut runs = Vec::new();
    let mut i = 0;
    let first_dark = bits.first().copied().unwrap_or(false);
    while i < bits.len() {
        let s = i;
        while i < bits.len() && bits[i] == bits[s] {
            i += 1;
        }
        runs.push(i - s);
    }
    (runs, first_dark)
}

fn decode_line(line: &[u8]) -> Option<Value> {
    let (runs, first_dark) = run_lengths(line);
    if runs.len() < 30 {
        return None;
    }
    // Runs alternate; index parity tells colour. We need runs starting with a dark bar.
    let start_offset = if first_dark { 0 } else { 1 };
    let decode_from = |runs: &[usize]| -> Option<Value> {
        // Try every dark run as a potential start guard (1,1,1 pattern preceded by a wide quiet zone).
        let mut i = start_offset;
        while i + 2 < runs.len() {
            let (a, b, c) = (runs[i], runs[i + 1], runs[i + 2]);
            let module = (a + b + c) as f64 / 3.0;
            let similar = |x: usize| (x as f64 - module).abs() <= module * 0.45 + 0.5;
            let quiet_ok = i == 0 || runs[i - 1] as f64 >= module * 5.0;
            if similar(a) && similar(b) && similar(c) && quiet_ok && module >= 1.0 {
                if let Some(v) = try_ean(runs, i, module) {
                    return Some(v);
                }
            }
            i += 2;
        }
        None
    };
    if let Some(v) = decode_from(&runs) {
        return Some(v);
    }
    // Reverse scan direction.
    let mut rev = runs.clone();
    rev.reverse();
    let rev_first_dark = if runs.len() % 2 == 1 { first_dark } else { !first_dark };
    let off = if rev_first_dark { 0 } else { 1 };
    let rev_runs: Vec<usize> = rev[off..].to_vec();
    let mut i = 0;
    while i + 2 < rev_runs.len() {
        let (a, b, c) = (rev_runs[i], rev_runs[i + 1], rev_runs[i + 2]);
        let module = (a + b + c) as f64 / 3.0;
        let similar = |x: usize| (x as f64 - module).abs() <= module * 0.45 + 0.5;
        if similar(a)
            && similar(b)
            && similar(c)
            && module >= 1.0
            && (i == 0 || rev_runs[i - 1] as f64 >= module * 5.0)
        {
            if let Some(mut v) = try_ean(&rev_runs, i, module) {
                v["scan_direction"] = json!("reversed");
                return Some(v);
            }
        }
        i += 2;
    }
    None
}

/// Decodes a 4-run digit group using normalised widths (each digit = 7
/// modules). Left-hand digits use the L/G tables, right-hand digits R.
fn digit_from_runs(r: &[usize], right_side: bool) -> Option<(usize, char)> {
    let total: usize = r.iter().sum();
    if total == 0 {
        return None;
    }
    let scale = 7.0 / total as f64;
    let raw: Vec<f64> = r.iter().map(|x| *x as f64 * scale).collect();
    let tables: Vec<(&[&str; 10], char)> = if right_side {
        vec![(&EAN_R, 'R')]
    } else {
        vec![(&EAN_L, 'L'), (&EAN_G, 'G')]
    };
    let mut best: Option<(f64, usize, char)> = None;
    for (table, parity) in tables {
        for (d, pat) in table.iter().enumerate() {
            let pr = run_lengths_of_pattern(pat);
            if pr.len() != raw.len() {
                continue;
            }
            let dist: f64 = pr.iter().zip(&raw).map(|(a, b)| (*a as f64 - b).abs()).sum();
            if dist < 1.6 && best.map(|b| dist < b.0).unwrap_or(true) {
                best = Some((dist, d, parity));
            }
        }
    }
    best.map(|(_, d, p)| (d, p))
}

fn run_lengths_of_pattern(p: &str) -> Vec<usize> {
    let chars: Vec<char> = p.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let s = i;
        while i < chars.len() && chars[i] == chars[s] {
            i += 1;
        }
        out.push(i - s);
    }
    out
}

fn try_ean(runs: &[usize], start: usize, module: f64) -> Option<Value> {
    // After the start guard (3 runs), left digits: 4 runs each (space,bar,space,bar).
    let pos = start + 3;
    let read_digits = |pos: &mut usize,
                       count: usize,
                       module: f64,
                       right_side: bool|
     -> Option<Vec<(usize, char)>> {
        let mut out = Vec::new();
        for _ in 0..count {
            if *pos + 4 > runs.len() {
                return None;
            }
            let total: usize = runs[*pos..*pos + 4].iter().sum();
            if (total as f64 - module * 7.0).abs() > module * 3.0 {
                return None;
            }
            out.push(digit_from_runs(&runs[*pos..*pos + 4], right_side)?);
            *pos += 4;
        }
        Some(out)
    };
    for digits_per_side in [6usize, 4] {
        let mut p = pos;
        let left = match read_digits(&mut p, digits_per_side, module, false) {
            Some(l) => l,
            None => continue,
        };
        // Centre guard: 5 runs (space bar space bar space).
        if p + 5 > runs.len() {
            continue;
        }
        let centre: usize = runs[p..p + 5].iter().sum();
        if (centre as f64 - module * 5.0).abs() > module * 2.5 {
            continue;
        }
        p += 5;
        let right = match read_digits(&mut p, digits_per_side, module, true) {
            Some(r) => r,
            None => continue,
        };
        // End guard 3 runs.
        if p + 3 > runs.len() {
            continue;
        }
        let end: usize = runs[p..p + 3].iter().sum();
        if (end as f64 - module * 3.0).abs() > module * 2.0 {
            continue;
        }
        // Left digits must be L/G, right digits R-coded (they decode as L/G patterns mirrored; our R table handles it).
        if !left.iter().all(|(_, p)| *p == 'L' || *p == 'G')
            || !right.iter().all(|(_, p)| *p == 'R')
        {
            continue;
        }
        let mut value = String::new();
        if digits_per_side == 6 {
            let parity: String = left.iter().map(|(_, p)| *p).collect();
            let first = EAN13_PARITY.iter().position(|p| *p == parity)?;
            value.push(char::from_digit(first as u32, 10)?);
        } else if !left.iter().all(|(_, p)| *p == 'L') {
            continue;
        }
        for (d, _) in left.iter().chain(right.iter()) {
            value.push(char::from_digit(*d as u32, 10)?);
        }
        let body = &value[..value.len() - 1];
        if gtin_check_digit(body) != value[value.len() - 1..].parse::<u8>().unwrap_or(0) {
            continue;
        }
        let symbology = if digits_per_side == 4 {
            "EAN-8"
        } else if value.starts_with('0') {
            "UPC-A / EAN-13"
        } else {
            "EAN-13"
        };
        return Some(json!({
            "value": value,
            "symbology": symbology,
            "module_px": (module * 10.0).round() / 10.0,
            "check_digit_valid": true
        }));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoders() {
        let e = ean13("590123412345").unwrap();
        assert_eq!(e.text, "5901234123457");
        assert_eq!(e.modules.len(), 95);
        assert!(ean13("5901234123450").is_err());
        assert_eq!(ean8("9638507").unwrap().text, "96385074");
        assert_eq!(upca("03600029145").unwrap().text, "036000291452");
        assert_eq!(upce("0123456").unwrap().text, "01234565");
        let c = code128("Hello123456", false).unwrap();
        assert!(c.modules.len() > 100);
        assert!(code128("é", false).is_err());
        let c39 = code39("abc-1", true).unwrap();
        assert_eq!(c39.text, "ABC-1");
        assert!(code39("a*b", false).is_err());
        assert_eq!(itf("1540014128876", true).unwrap().text, "15400141288763");
        assert_eq!(itf("123", false).unwrap().text, "0123");
        assert_eq!(codabar("40156",).unwrap().text, "A40156B");
        assert!(encode("pdf417", "x", &json!({})).is_err());
        assert!(encode("nope", "x", &json!({})).is_err());
        let g = generate("ean13", "4006381333931", &BarStyle::default(), &json!({}), &[]).unwrap();
        assert!(g["svg"].as_str().unwrap().contains("<path"));
        assert!(g["png"].is_string());
    }

    #[test]
    fn validation() {
        let v = validate("4006381333931").unwrap();
        assert_eq!(v["valid"], true);
        assert_eq!(v["prefix_region"], "Germany");
        assert_eq!(validate("4006381333932").unwrap()["valid"], false);
        let isbn = validate("0-306-40615-2").unwrap();
        assert_eq!(isbn["type"], "ISBN-10");
        assert_eq!(isbn["valid"], true);
        assert_eq!(isbn["isbn13"], "9780306406157");
        assert!(validate("12").is_err());
        assert!(validate("").is_err());
    }

    #[test]
    fn read_generated() {
        for (sym, data) in
            [("ean13", "5901234123457"), ("upca", "036000291452"), ("ean8", "96385074")]
        {
            let style = BarStyle {
                module_width: 3,
                height: 60,
                ..Default::default()
            };
            let s = encode(sym, data, &json!({})).unwrap();
            let png = to_png(&s, &style).unwrap();
            let r = read(&png).unwrap();
            let expected = if sym == "upca" {
                format!("0{data}")
            } else {
                data.to_string()
            };
            assert_eq!(r["barcodes"][0]["value"], expected, "{sym}");
        }
        // Rotated image.
        let s = encode("ean13", "4006381333931", &json!({})).unwrap();
        let png = to_png(&s, &BarStyle::default()).unwrap();
        let img = image::load_from_memory(&png).unwrap().rotate90();
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        assert_eq!(read(&buf.into_inner()).unwrap()["barcodes"][0]["rotated"], true);
        let blank = image::RgbaImage::from_pixel(200, 100, Rgba([255, 255, 255, 255]));
        let mut buf = std::io::Cursor::new(Vec::new());
        blank.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        assert!(read(&buf.into_inner()).is_err());
    }
}
