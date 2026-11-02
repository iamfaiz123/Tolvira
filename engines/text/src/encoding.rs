//! Character encodings and ciphers: numeric codes (binary/octal/decimal/hex),
//! Unicode escapes, ROT/Caesar, Morse, Unicode inspection/normalisation and
//! pattern extraction.

use engine_shared::{EngineError, EngineResult};
use serde_json::{json, Value};
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

fn radix_name(base: u32) -> &'static str {
    match base {
        2 => "binary",
        8 => "octal",
        10 => "decimal",
        16 => "hexadecimal",
        _ => "custom",
    }
}

/// Encodes text as numeric codes. `unit`: `utf8` (bytes), `codepoint`
/// (Unicode scalar values) or `utf16`.
pub fn encode_codes(
    text: &str,
    base: u32,
    unit: &str,
    separator: &str,
    pad: bool,
    prefix: &str,
) -> EngineResult<Value> {
    if !matches!(base, 2 | 8 | 10 | 16) {
        return Err(EngineError::invalid("`base` must be 2, 8, 10 or 16"));
    }
    if text.is_empty() {
        return Err(EngineError::empty("text"));
    }
    let values: Vec<u32> = match unit {
        "" | "utf8" | "bytes" => text.bytes().map(|b| b as u32).collect(),
        "codepoint" | "unicode" => text.chars().map(|c| c as u32).collect(),
        "utf16" => text.encode_utf16().map(|u| u as u32).collect(),
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown unit `{other}` (utf8, codepoint, utf16)"
            )))
        },
    };
    let width = match (base, unit) {
        (2, "" | "utf8" | "bytes") => 8,
        (16, "" | "utf8" | "bytes") => 2,
        (8, "" | "utf8" | "bytes") => 3,
        (2, _) => 16,
        (16, _) => 4,
        _ => 0,
    };
    let fmt = |v: u32| -> String {
        let s = match base {
            2 => format!("{v:b}"),
            8 => format!("{v:o}"),
            16 => {
                if prefix.contains("U+") || prefix.contains("\\u") {
                    format!("{v:X}")
                } else {
                    format!("{v:x}")
                }
            },
            _ => v.to_string(),
        };
        let padded = if pad && width > 0 && s.len() < width {
            format!("{}{}", "0".repeat(width - s.len()), s)
        } else {
            s
        };
        format!("{prefix}{padded}")
    };
    let encoded: Vec<String> = values.iter().map(|&v| fmt(v)).collect();
    Ok(json!({
        "result": encoded.join(separator),
        "count": values.len(),
        "base": base,
        "radix": radix_name(base),
        "unit": if unit.is_empty() { "utf8" } else { unit }
    }))
}

/// Decodes numeric codes back to text. Accepts separators (space, comma,
/// newline), `0b`/`0x`/`U+`/`\u` prefixes and unpadded groups (binary groups
/// of 8 are inferred when the input has no separators).
pub fn decode_codes(codes: &str, base: u32, unit: &str) -> EngineResult<Value> {
    if !matches!(base, 2 | 8 | 10 | 16) {
        return Err(EngineError::invalid("`base` must be 2, 8, 10 or 16"));
    }
    let cleaned = codes.trim();
    if cleaned.is_empty() {
        return Err(EngineError::empty("codes"));
    }
    let mut tokens: Vec<String> = cleaned
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .filter(|t| !t.is_empty())
        .map(|t| {
            let t = t
                .trim_start_matches("\\u")
                .trim_start_matches("\\x")
                .trim_start_matches("U+")
                .trim_start_matches("u+")
                .trim_start_matches("&#x")
                .trim_start_matches("&#")
                .trim_end_matches(';');
            let t = match base {
                2 => t.trim_start_matches("0b").trim_start_matches("0B"),
                8 => t.trim_start_matches("0o"),
                16 => t.trim_start_matches("0x").trim_start_matches("0X"),
                _ => t,
            };
            t.to_string()
        })
        .collect();
    // Unseparated binary / hex streams.
    if tokens.len() == 1 {
        let t = tokens[0].clone();
        let group = match (base, unit) {
            (2, "" | "utf8" | "bytes") => 8,
            (16, "" | "utf8" | "bytes") => 2,
            (2, _) => 16,
            (16, _) => 4,
            _ => 0,
        };
        if group > 0 && t.len() > group && t.len().is_multiple_of(group) {
            tokens = t
                .as_bytes()
                .chunks(group)
                .map(|c| String::from_utf8_lossy(c).into_owned())
                .collect();
        }
    }
    let mut values = Vec::with_capacity(tokens.len());
    for t in &tokens {
        let v = u32::from_str_radix(t, base).map_err(|_| {
            EngineError::invalid(format!("`{t}` is not a valid {} number", radix_name(base)))
        })?;
        values.push(v);
    }
    let text = match unit {
        "" | "utf8" | "bytes" => {
            let bytes: Vec<u8> = values
                .iter()
                .map(|&v| if v > 255 { None } else { Some(v as u8) })
                .collect::<Option<_>>()
                .ok_or_else(|| {
                    EngineError::invalid(
                        "Byte values must be 0-255; use unit `codepoint` for larger values",
                    )
                })?;
            String::from_utf8(bytes)
                .map_err(|_| EngineError::invalid("Decoded bytes are not valid UTF-8 text"))?
        },
        "codepoint" | "unicode" => values
            .iter()
            .map(|&v| {
                char::from_u32(v).ok_or_else(|| {
                    EngineError::invalid(format!("U+{v:04X} is not a valid Unicode scalar value"))
                })
            })
            .collect::<EngineResult<String>>()?,
        "utf16" => {
            let units: Vec<u16> = values
                .iter()
                .map(|&v| if v > 0xFFFF { None } else { Some(v as u16) })
                .collect::<Option<_>>()
                .ok_or_else(|| EngineError::invalid("UTF-16 units must be 0-65535"))?;
            String::from_utf16(&units)
                .map_err(|_| EngineError::invalid("Invalid UTF-16 sequence"))?
        },
        other => return Err(EngineError::invalid(format!("Unknown unit `{other}`"))),
    };
    Ok(json!({ "result": text, "count": values.len() }))
}

/// Caesar / ROT cipher over ASCII letters (and digits with `rotate_digits`).
/// ROT47 rotates all printable ASCII.
pub fn rot(text: &str, shift: i32, rotate_digits: bool, rot47: bool) -> EngineResult<Value> {
    if text.is_empty() {
        return Err(EngineError::empty("text"));
    }
    let out: String = if rot47 {
        text.chars()
            .map(|c| {
                if (33..=126).contains(&(c as u32)) {
                    char::from_u32(33 + ((c as u32 - 33 + 47) % 94)).unwrap_or(c)
                } else {
                    c
                }
            })
            .collect()
    } else {
        let s = shift.rem_euclid(26) as u8;
        text.chars()
            .map(|c| match c {
                'a'..='z' => ((c as u8 - b'a' + s) % 26 + b'a') as char,
                'A'..='Z' => ((c as u8 - b'A' + s) % 26 + b'A') as char,
                '0'..='9' if rotate_digits => {
                    ((c as u8 - b'0' + (shift.rem_euclid(10) as u8)) % 10 + b'0') as char
                },
                _ => c,
            })
            .collect()
    };
    Ok(json!({ "result": out, "shift": if rot47 { 47 } else { shift.rem_euclid(26) } }))
}

const MORSE: &[(char, &str)] = &[
    ('a', ".-"),
    ('b', "-..."),
    ('c', "-.-."),
    ('d', "-.."),
    ('e', "."),
    ('f', "..-."),
    ('g', "--."),
    ('h', "...."),
    ('i', ".."),
    ('j', ".---"),
    ('k', "-.-"),
    ('l', ".-.."),
    ('m', "--"),
    ('n', "-."),
    ('o', "---"),
    ('p', ".--."),
    ('q', "--.-"),
    ('r', ".-."),
    ('s', "..."),
    ('t', "-"),
    ('u', "..-"),
    ('v', "...-"),
    ('w', ".--"),
    ('x', "-..-"),
    ('y', "-.--"),
    ('z', "--.."),
    ('0', "-----"),
    ('1', ".----"),
    ('2', "..---"),
    ('3', "...--"),
    ('4', "....-"),
    ('5', "....."),
    ('6', "-...."),
    ('7', "--..."),
    ('8', "---.."),
    ('9', "----."),
    ('.', ".-.-.-"),
    (',', "--..--"),
    ('?', "..--.."),
    ('\'', ".----."),
    ('!', "-.-.--"),
    ('/', "-..-."),
    ('(', "-.--."),
    (')', "-.--.-"),
    ('&', ".-..."),
    (':', "---..."),
    (';', "-.-.-."),
    ('=', "-...-"),
    ('+', ".-.-."),
    ('-', "-....-"),
    ('_', "..--.-"),
    ('"', ".-..-."),
    ('$', "...-..-"),
    ('@', ".--.-."),
    ('é', "..-.."),
    ('ä', ".-.-"),
    ('ö', "---."),
    ('ü', "..--"),
    ('ñ', "--.--"),
    ('ç', "-.-.."),
];

/// Encodes text as Morse code with configurable symbols and separators.
pub fn morse_encode(
    text: &str,
    dot: &str,
    dash: &str,
    letter_sep: &str,
    word_sep: &str,
) -> EngineResult<Value> {
    if text.trim().is_empty() {
        return Err(EngineError::empty("text"));
    }
    let mut unsupported = Vec::new();
    let words: Vec<String> = text
        .split_whitespace()
        .map(|w| {
            w.chars()
                .filter_map(|c| {
                    let lc = c.to_lowercase().next().unwrap_or(c);
                    match MORSE.iter().find(|(k, _)| *k == lc) {
                        Some((_, code)) => Some(code.replace('.', dot).replace('-', dash)),
                        None => {
                            if !unsupported.contains(&c) {
                                unsupported.push(c);
                            }
                            None
                        },
                    }
                })
                .collect::<Vec<_>>()
                .join(letter_sep)
        })
        .collect();
    Ok(json!({
        "result": words.join(word_sep),
        "unsupported": unsupported.iter().collect::<String>()
    }))
}

/// Decodes Morse code back to text; unknown groups become `?`.
pub fn morse_decode(
    code: &str,
    dot: &str,
    dash: &str,
    letter_sep: &str,
    word_sep: &str,
) -> EngineResult<Value> {
    if code.trim().is_empty() {
        return Err(EngineError::empty("morse code"));
    }
    let normalised = code
        .replace(dash, "-")
        .replace(dot, ".")
        .replace(['·', '•'], ".")
        .replace(['–', '—', '_'], "-");
    let ws = if word_sep.trim().is_empty() {
        " / ".to_string()
    } else {
        word_sep.to_string()
    };
    let ls = if letter_sep.is_empty() { " " } else { letter_sep };
    let mut unknown = Vec::new();
    let words: Vec<String> = normalised
        .split(ws.trim())
        .map(|w| {
            w.split(ls)
                .filter(|l| !l.is_empty())
                .map(|l| match MORSE.iter().find(|(_, c)| *c == l) {
                    Some((ch, _)) => *ch,
                    None => {
                        unknown.push(l.to_string());
                        '?'
                    },
                })
                .collect()
        })
        .collect();
    Ok(json!({ "result": words.join(" "), "unknown": unknown }))
}

fn char_category(c: char) -> &'static str {
    if c.is_control() {
        "control"
    } else if unicode_normalization::char::is_combining_mark(c) {
        "combining mark"
    } else if c.is_whitespace() {
        "whitespace"
    } else if c.is_ascii_digit() || c.is_numeric() {
        "number"
    } else if c.is_alphabetic() {
        if c.is_uppercase() {
            "uppercase letter"
        } else if c.is_lowercase() {
            "lowercase letter"
        } else {
            "letter"
        }
    } else if c.is_ascii_punctuation()
        || matches!(c, '\u{2000}'..='\u{206F}' | '\u{3000}'..='\u{303F}')
    {
        "punctuation"
    } else if crate::transform::is_emoji(c) {
        "emoji / pictograph"
    } else {
        "symbol"
    }
}

fn block_name(cp: u32) -> &'static str {
    match cp {
        0x0000..=0x007F => "Basic Latin",
        0x0080..=0x00FF => "Latin-1 Supplement",
        0x0100..=0x017F => "Latin Extended-A",
        0x0180..=0x024F => "Latin Extended-B",
        0x0250..=0x02FF => "IPA / Spacing Modifiers",
        0x0300..=0x036F => "Combining Diacritical Marks",
        0x0370..=0x03FF => "Greek and Coptic",
        0x0400..=0x052F => "Cyrillic",
        0x0530..=0x058F => "Armenian",
        0x0590..=0x05FF => "Hebrew",
        0x0600..=0x06FF => "Arabic",
        0x0900..=0x097F => "Devanagari",
        0x0980..=0x09FF => "Bengali",
        0x0B80..=0x0BFF => "Tamil",
        0x0E00..=0x0E7F => "Thai",
        0x1100..=0x11FF => "Hangul Jamo",
        0x1E00..=0x1EFF => "Latin Extended Additional",
        0x2000..=0x206F => "General Punctuation",
        0x2070..=0x20CF => "Super/Subscripts & Currency",
        0x2100..=0x214F => "Letterlike Symbols",
        0x2190..=0x21FF => "Arrows",
        0x2200..=0x22FF => "Mathematical Operators",
        0x2500..=0x257F => "Box Drawing",
        0x2600..=0x26FF => "Miscellaneous Symbols",
        0x2700..=0x27BF => "Dingbats",
        0x3000..=0x303F => "CJK Symbols and Punctuation",
        0x3040..=0x30FF => "Hiragana / Katakana",
        0x4E00..=0x9FFF => "CJK Unified Ideographs",
        0xAC00..=0xD7AF => "Hangul Syllables",
        0xE000..=0xF8FF => "Private Use Area",
        0xFB00..=0xFB4F => "Alphabetic Presentation Forms",
        0xFE00..=0xFE0F => "Variation Selectors",
        0xFF00..=0xFFEF => "Halfwidth and Fullwidth Forms",
        0x1F300..=0x1F5FF => "Misc Symbols and Pictographs",
        0x1F600..=0x1F64F => "Emoticons",
        0x1F680..=0x1F6FF => "Transport and Map Symbols",
        0x1F900..=0x1F9FF => "Supplemental Symbols and Pictographs",
        _ => "Other",
    }
}

/// Per-character Unicode report.
pub fn unicode_inspect(text: &str, limit: usize) -> EngineResult<Value> {
    if text.is_empty() {
        return Err(EngineError::empty("text"));
    }
    let limit = if limit == 0 { 500 } else { limit.min(5000) };
    let chars: Vec<Value> = text
        .chars()
        .take(limit)
        .map(|c| {
            let cp = c as u32;
            let mut utf8 = [0u8; 4];
            let utf8_bytes = c.encode_utf8(&mut utf8).as_bytes().iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ");
            let mut utf16 = [0u16; 2];
            let utf16_units = c.encode_utf16(&mut utf16).iter().map(|u| format!("{u:04X}")).collect::<Vec<_>>().join(" ");
            json!({
                "char": if c.is_control() || c.is_whitespace() { String::new() } else { c.to_string() },
                "code_point": format!("U+{cp:04X}"),
                "decimal": cp,
                "utf8": utf8_bytes,
                "utf16": utf16_units,
                "html_entity": format!("&#{cp};"),
                "css_escape": format!("\\{cp:X}"),
                "js_escape": if cp > 0xFFFF { format!("\\u{{{cp:X}}}") } else { format!("\\u{cp:04X}") },
                "category": char_category(c),
                "block": block_name(cp),
                "is_ascii": c.is_ascii(),
            })
        })
        .collect();
    let non_ascii = text.chars().filter(|c| !c.is_ascii()).count();
    let invisible = text.chars().filter(|c| matches!(*c, '\u{200B}'..='\u{200F}' | '\u{FEFF}' | '\u{2060}' | '\u{00AD}' | '\u{00A0}' | '\u{202A}'..='\u{202E}')).count();
    Ok(json!({
        "characters": chars,
        "code_points": text.chars().count(),
        "graphemes": text.graphemes(true).count(),
        "utf8_bytes": text.len(),
        "utf16_units": text.encode_utf16().count(),
        "non_ascii": non_ascii,
        "invisible_characters": invisible,
        "truncated": text.chars().count() > limit,
        "is_nfc": unicode_normalization::is_nfc(text),
    }))
}

/// Applies a Unicode normalization form (NFC/NFD/NFKC/NFKD) and optionally strips combining
/// accents.
pub fn unicode_normalize(text: &str, form: &str, strip_accents: bool) -> EngineResult<Value> {
    let f = form.trim().to_uppercase();
    let mut out: String = match f.as_str() {
        "" | "NFC" => text.nfc().collect(),
        "NFD" => text.nfd().collect(),
        "NFKC" => text.nfkc().collect(),
        "NFKD" => text.nfkd().collect(),
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown normalisation form `{other}` (NFC, NFD, NFKC, NFKD)"
            )))
        },
    };
    if strip_accents {
        out = crate::transform::strip_accents(&out);
    }
    Ok(json!({
        "result": out,
        "changed": out != text,
        "code_points_before": text.chars().count(),
        "code_points_after": out.chars().count()
    }))
}

/// Escapes text as `\uXXXX` (JS/JSON), `&#x...;` (HTML) or `%XX` (URL-style UTF-8).
pub fn unicode_escape(text: &str, style: &str, ascii_only: bool) -> EngineResult<Value> {
    if text.is_empty() {
        return Err(EngineError::empty("text"));
    }
    let out: String = text
        .chars()
        .map(|c| {
            if ascii_only && c.is_ascii() && !c.is_control() {
                return c.to_string();
            }
            let cp = c as u32;
            match style {
                "" | "js" | "json" => {
                    if cp > 0xFFFF {
                        let mut b = [0u16; 2];
                        c.encode_utf16(&mut b).iter().map(|u| format!("\\u{u:04X}")).collect()
                    } else {
                        format!("\\u{cp:04X}")
                    }
                },
                "es6" => format!("\\u{{{cp:X}}}"),
                "html" | "html-hex" => format!("&#x{cp:X};"),
                "html-decimal" => format!("&#{cp};"),
                "css" => format!("\\{cp:X} "),
                "python" | "rust" => {
                    if cp > 0xFFFF {
                        format!("\\U{cp:08X}")
                    } else {
                        format!("\\u{cp:04X}")
                    }
                },
                "uplus" | "u+" => format!("U+{cp:04X} "),
                _ => format!("\\u{cp:04X}"),
            }
        })
        .collect();
    Ok(json!({ "result": out.trim_end().to_string() }))
}

/// Unescapes `\uXXXX`, `\u{...}`, `\xHH`, `&#...;`, `U+XXXX` sequences.
pub fn unicode_unescape(text: &str) -> EngineResult<Value> {
    if text.is_empty() {
        return Err(EngineError::empty("text"));
    }
    let re = crate::re(
        r"\\u\{([0-9A-Fa-f]{1,6})\}|\\u([0-9A-Fa-f]{4})|\\U([0-9A-Fa-f]{8})|\\x([0-9A-Fa-f]{2})|&#x([0-9A-Fa-f]{1,6});|&#([0-9]{1,7});|U\+([0-9A-Fa-f]{4,6})",
    )?;
    // First pass: collect UTF-16 surrogate pairs from \uXXXX sequences.
    let mut out = String::new();
    let mut last = 0;
    let mut pending_high: Option<u16> = None;
    for m in re.captures_iter(text) {
        let Some(whole) = m.get(0) else { continue };
        out.push_str(&text[last..whole.start()]);
        last = whole.end();
        let (val, is_u16) = if let Some(g) = m.get(1) {
            (u32::from_str_radix(g.as_str(), 16).ok(), false)
        } else if let Some(g) = m.get(2) {
            (u32::from_str_radix(g.as_str(), 16).ok(), true)
        } else if let Some(g) = m.get(3) {
            (u32::from_str_radix(g.as_str(), 16).ok(), false)
        } else if let Some(g) = m.get(4) {
            (u32::from_str_radix(g.as_str(), 16).ok(), false)
        } else if let Some(g) = m.get(5) {
            (u32::from_str_radix(g.as_str(), 16).ok(), false)
        } else if let Some(g) = m.get(6) {
            (g.as_str().parse().ok(), false)
        } else {
            (m.get(7).and_then(|g| u32::from_str_radix(g.as_str(), 16).ok()), false)
        };
        let Some(v) = val else {
            out.push_str(whole.as_str());
            continue;
        };
        if is_u16 && (0xD800..=0xDBFF).contains(&v) {
            pending_high = Some(v as u16);
            continue;
        }
        if is_u16 && (0xDC00..=0xDFFF).contains(&v) {
            if let Some(h) = pending_high.take() {
                if let Ok(s) = String::from_utf16(&[h, v as u16]) {
                    out.push_str(&s);
                    continue;
                }
            }
        }
        pending_high = None;
        match char::from_u32(v) {
            Some(c) => out.push(c),
            None => out.push('\u{FFFD}'),
        }
    }
    out.push_str(&text[last..]);
    Ok(json!({ "result": out }))
}

/// Extracts patterns: emails, urls, numbers, hashtags, mentions, phones,
/// ipv4, dates, words, or a custom regex (with optional capture group).
pub fn extract(text: &str, kind: &str, pattern: Option<&str>, unique: bool) -> EngineResult<Value> {
    if text.is_empty() {
        return Err(EngineError::empty("text"));
    }
    let re = match kind {
        "emails" | "email" => crate::re(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}")?,
        "urls" | "url" => crate::re(r#"(?i)\b(?:https?://|ftp://|www\.)[^\s<>"')\]]+"#)?,
        "numbers" | "number" => crate::re(r"-?\d+(?:[.,]\d+)*")?,
        "hashtags" | "hashtag" => crate::re(r"#[\p{L}\p{N}_]+")?,
        "mentions" | "mention" => crate::re(r"(?:^|[^\w@.])(@[A-Za-z0-9_]+)")?,
        "phones" | "phone" => crate::re(r"\+?\d[\d\s().-]{6,}\d")?,
        "ipv4" | "ips" => crate::re(r"\b(?:\d{1,3}\.){3}\d{1,3}\b")?,
        "dates" | "date" => crate::re(
            r"\b\d{4}-\d{2}-\d{2}\b|\b\d{1,2}/\d{1,2}/\d{2,4}\b|\b\d{1,2}\.\d{1,2}\.\d{2,4}\b",
        )?,
        "words" => crate::re(r"\p{L}[\p{L}\p{N}'-]*")?,
        "capitalized" => crate::re(r"\b\p{Lu}\p{Ll}+(?:\s+\p{Lu}\p{Ll}+)*")?,
        "custom" | "regex" => {
            let p = pattern.filter(|p| !p.is_empty()).ok_or_else(|| {
                EngineError::invalid("`pattern` is required for custom extraction")
            })?;
            crate::transform::build_regex(p, true, true, false, true)?
        },
        other => return Err(EngineError::invalid(format!("Unknown extraction kind `{other}`"))),
    };
    let mut items: Vec<String> = re
        .captures_iter(text)
        .take(100_000)
        .map(|c| {
            c.get(1)
                .or_else(|| c.get(0))
                .map(|m| m.as_str().to_string())
                .unwrap_or_default()
        })
        .collect();
    let total = items.len();
    if unique {
        let mut seen = std::collections::HashSet::new();
        items.retain(|i| seen.insert(i.clone()));
    }
    Ok(json!({
        "items": items,
        "count": items.len(),
        "total_matches": total,
        "result": items.join("\n")
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_roundtrip() {
        let b = encode_codes("Hi", 2, "utf8", " ", true, "").unwrap();
        assert_eq!(b["result"], "01001000 01101001");
        assert_eq!(decode_codes("01001000 01101001", 2, "utf8").unwrap()["result"], "Hi");
        assert_eq!(decode_codes("0100100001101001", 2, "utf8").unwrap()["result"], "Hi");
        assert_eq!(encode_codes("é", 16, "utf8", " ", true, "").unwrap()["result"], "c3 a9");
        assert_eq!(decode_codes("c3a9", 16, "utf8").unwrap()["result"], "é");
        assert_eq!(
            encode_codes("A€", 16, "codepoint", " ", true, "U+").unwrap()["result"],
            "U+0041 U+20AC"
        );
        assert_eq!(decode_codes("U+0041 U+20AC", 16, "codepoint").unwrap()["result"], "A€");
        assert_eq!(encode_codes("AB", 10, "utf8", ",", false, "").unwrap()["result"], "65,66");
        assert!(decode_codes("zz", 16, "utf8").is_err());
        assert!(decode_codes("300", 10, "utf8").is_err());
        assert!(encode_codes("", 2, "utf8", " ", true, "").is_err());
        assert!(encode_codes("x", 3, "utf8", " ", true, "").is_err());
    }

    #[test]
    fn ciphers() {
        assert_eq!(rot("Hello, World!", 13, false, false).unwrap()["result"], "Uryyb, Jbeyq!");
        assert_eq!(rot("Uryyb", 13, false, false).unwrap()["result"], "Hello");
        assert_eq!(rot("abc1", 1, true, false).unwrap()["result"], "bcd2");
        assert_eq!(rot("Hello", 0, false, true).unwrap()["result"], "w6==@");
        assert!(rot("", 1, false, false).is_err());
        let m = morse_encode("SOS hi", ".", "-", " ", " / ").unwrap();
        assert_eq!(m["result"], "... --- ... / .... ..");
        assert_eq!(
            morse_decode("... --- ... / .... ..", ".", "-", " ", " / ").unwrap()["result"],
            "sos hi"
        );
        assert_eq!(morse_encode("a€", ".", "-", " ", " / ").unwrap()["unsupported"], "€");
    }

    #[test]
    fn unicode_tools() {
        let u = unicode_inspect("Aé\u{200B}😀", 0).unwrap();
        assert_eq!(u["code_points"], 4);
        assert_eq!(u["graphemes"], 4);
        assert_eq!(u["invisible_characters"], 1);
        assert_eq!(u["characters"][1]["code_point"], "U+00E9");
        assert_eq!(u["characters"][3]["block"], "Emoticons");
        assert!(unicode_inspect("", 0).is_err());
        let n = unicode_normalize("e\u{0301}", "NFC", false).unwrap();
        assert_eq!(n["result"], "é");
        assert_eq!(unicode_normalize("é", "NFD", true).unwrap()["result"], "e");
        assert!(unicode_normalize("x", "NFX", false).is_err());
        assert_eq!(unicode_escape("a€", "js", true).unwrap()["result"], "a\\u20AC");
        assert_eq!(unicode_escape("😀", "js", true).unwrap()["result"], "\\uD83D\\uDE00");
        assert_eq!(
            unicode_unescape("\\uD83D\\uDE00 \\u{1F600} &#x20AC; U+0041").unwrap()["result"],
            "😀 😀 € A"
        );
    }

    #[test]
    fn extraction() {
        let t = "Mail a@b.com or c@d.org, visit https://x.io/path #rust #wasm @dev call +1 \
                 555-123-4567 on 2024-01-31";
        assert_eq!(extract(t, "emails", None, true).unwrap()["count"], 2);
        assert_eq!(extract(t, "urls", None, true).unwrap()["items"][0], "https://x.io/path");
        assert_eq!(extract(t, "hashtags", None, true).unwrap()["count"], 2);
        assert_eq!(extract(t, "mentions", None, true).unwrap()["items"][0], "@dev");
        assert_eq!(extract(t, "dates", None, true).unwrap()["items"][0], "2024-01-31");
        assert_eq!(extract(t, "custom", Some(r"#(\w+)"), true).unwrap()["items"][1], "wasm");
        assert!(extract(t, "custom", None, true).is_err());
        assert!(extract(t, "aliens", None, true).is_err());
    }
}
