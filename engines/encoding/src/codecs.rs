//! Text / binary codecs.

use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use engine_shared::{b64, limits, EngineError, EngineResult};
use percent_encoding::{
    percent_decode_str, utf8_percent_encode, AsciiSet, CONTROLS, NON_ALPHANUMERIC,
};
use serde_json::{json, Value};

/// Resolves the input bytes from `file` (preferred) or `text`; empty input is an error.
pub fn input_bytes(text: Option<&str>, file: Option<&[u8]>) -> EngineResult<Vec<u8>> {
    match (file, text) {
        (Some(f), _) if !f.is_empty() => {
            limits::check_size(f.len(), limits::MAX_FILE_BYTES, "file")?;
            Ok(f.to_vec())
        },
        (_, Some(t)) if !t.is_empty() => Ok(t.as_bytes().to_vec()),
        _ => Err(EngineError::empty("input (`text` or `file`)")),
    }
}

fn text_or_null(bytes: &[u8]) -> Value {
    match std::str::from_utf8(bytes) {
        Ok(s) => json!(s),
        Err(_) => Value::Null,
    }
}

/// Base64 encoding (standard/URL-safe, optional padding, line wrapping, data-URI).
pub fn base64_encode(
    data: &[u8],
    url_safe: bool,
    padding: bool,
    line_length: usize,
    mime: Option<&str>,
) -> EngineResult<Value> {
    let engine: &base64::engine::GeneralPurpose = match (url_safe, padding) {
        (false, true) => &STANDARD,
        (false, false) => &STANDARD_NO_PAD,
        (true, true) => &URL_SAFE,
        (true, false) => &URL_SAFE_NO_PAD,
    };
    let mut encoded = engine.encode(data);
    if line_length > 0 {
        let ll = line_length.clamp(4, 1000);
        encoded = encoded
            .as_bytes()
            .chunks(ll)
            .filter_map(|c| std::str::from_utf8(c).ok()) // ASCII: chunks never split a char
            .collect::<Vec<_>>()
            .join("\n");
    }
    let data_url = mime
        .filter(|m| !m.is_empty())
        .map(|m| format!("data:{m};base64,{}", engine.encode(data)));
    Ok(json!({
        "result": encoded,
        "bytes_in": data.len(),
        "length": encoded.len(),
        "data_url": data_url,
        "url_safe": url_safe
    }))
}

/// Decodes base64 (any alphabet, padding optional) and reports text/MIME when possible.
pub fn base64_decode(text: &str, as_text: Option<bool>) -> EngineResult<Value> {
    if text.trim().is_empty() {
        return Err(EngineError::empty("text"));
    }
    let bytes = b64::decode(text)?;
    let utf8 = std::str::from_utf8(&bytes).ok();
    let want_text = as_text.unwrap_or(utf8.is_some());
    Ok(json!({
        "text": if want_text { utf8.map(|s| json!(s)).unwrap_or(Value::Null) } else { Value::Null },
        "bytes": bytes.len(),
        "file": b64::encode(&bytes),
        "is_text": utf8.is_some(),
        "hex": if bytes.len() <= 4096 { json!(hex::encode(&bytes)) } else { Value::Null },
        "detected_mime": sniff_mime(&bytes),
    }))
}

/// Guesses a MIME type from magic bytes.
pub fn sniff_mime(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(b"\x89PNG") {
        Some("image/png")
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if b.starts_with(b"GIF8") {
        Some("image/gif")
    } else if b.len() > 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("image/webp")
    } else if b.starts_with(b"%PDF") {
        Some("application/pdf")
    } else if b.starts_with(b"PK\x03\x04") {
        Some("application/zip")
    } else if b.starts_with(b"\x1F\x8B") {
        Some("application/gzip")
    } else if b.starts_with(b"<svg") || b.starts_with(b"<?xml") {
        Some("image/svg+xml")
    } else if b.starts_with(b"{") || b.starts_with(b"[") {
        Some("application/json")
    } else if b.starts_with(b"OggS") {
        Some("audio/ogg")
    } else if b.starts_with(b"ID3") || b.starts_with(&[0xFF, 0xFB]) {
        Some("audio/mpeg")
    } else if b.len() > 12 && &b[4..8] == b"ftyp" {
        Some("video/mp4")
    } else if b.starts_with(b"RIFF") {
        Some("audio/wav")
    } else if std::str::from_utf8(b).is_ok() {
        Some("text/plain")
    } else {
        None
    }
}

const B32: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// RFC 4648 base32 encoding.
pub fn base32_encode(data: &[u8], padding: bool) -> EngineResult<Value> {
    let mut out = String::new();
    let mut buffer: u64 = 0;
    let mut bits = 0;
    for &b in data {
        buffer = (buffer << 8) | b as u64;
        bits += 8;
        while bits >= 5 {
            out.push(B32[((buffer >> (bits - 5)) & 31) as usize] as char);
            bits -= 5;
        }
    }
    if bits > 0 {
        out.push(B32[((buffer << (5 - bits)) & 31) as usize] as char);
    }
    if padding {
        while !out.len().is_multiple_of(8) {
            out.push('=');
        }
    }
    Ok(json!({ "result": out, "bytes_in": data.len() }))
}

/// RFC 4648 base32 decoding (case-insensitive).
pub fn base32_decode(text: &str) -> EngineResult<Value> {
    let clean: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '=')
        .collect::<String>()
        .to_uppercase();
    if clean.is_empty() {
        return Err(EngineError::empty("text"));
    }
    let mut out = Vec::new();
    let mut buffer: u64 = 0;
    let mut bits = 0;
    for c in clean.bytes() {
        let v = B32.iter().position(|&a| a == c).ok_or_else(|| {
            EngineError::invalid(format!("Invalid base32 character `{}`", c as char))
        })?;
        buffer = (buffer << 5) | v as u64;
        bits += 5;
        if bits >= 8 {
            out.push((buffer >> (bits - 8)) as u8);
            bits -= 8;
        }
    }
    Ok(json!({
        "text": text_or_null(&out),
        "bytes": out.len(),
        "file": b64::encode(&out),
        "hex": hex::encode(&out)
    }))
}

/// Hex encoding with optional case, separator and per-byte prefix.
pub fn hex_encode(
    data: &[u8],
    uppercase: bool,
    separator: &str,
    prefix: &str,
) -> EngineResult<Value> {
    let parts: Vec<String> = data
        .iter()
        .map(|b| {
            format!(
                "{prefix}{}",
                if uppercase {
                    format!("{b:02X}")
                } else {
                    format!("{b:02x}")
                }
            )
        })
        .collect();
    Ok(json!({ "result": parts.join(separator), "bytes_in": data.len() }))
}

/// Hex decoding tolerant of separators, `0x` prefixes and whitespace.
pub fn hex_decode(text: &str) -> EngineResult<Value> {
    let clean: String = text
        .split(|c: char| c.is_whitespace() || c == ',' || c == ':')
        .map(|t| {
            t.trim_start_matches("0x")
                .trim_start_matches("0X")
                .trim_start_matches("\\x")
        })
        .collect::<Vec<_>>()
        .join("");
    if clean.is_empty() {
        return Err(EngineError::empty("text"));
    }
    let bytes =
        hex::decode(&clean).map_err(|e| EngineError::invalid(format!("Invalid hex: {e}")))?;
    Ok(json!({
        "text": text_or_null(&bytes),
        "bytes": bytes.len(),
        "file": b64::encode(&bytes),
        "is_text": std::str::from_utf8(&bytes).is_ok()
    }))
}

const COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');
const QUERY: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'<')
    .add(b'>')
    .add(b'&')
    .add(b'=')
    .add(b'+')
    .add(b'%');
const PATH: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}')
    .add(b'%');
const FULL_URL: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'<')
    .add(b'>')
    .add(b'`')
    .add(b'{')
    .add(b'}')
    .add(b'|')
    .add(b'\\')
    .add(b'^');

/// URL encoding. `mode`: `component` (encodeURIComponent, default), `uri`
/// (encodeURI – keeps `/?:@&=+$#`), `query` (form field value), `path`, `all`
/// (every byte).
pub fn url_encode(text: &str, mode: &str, space_as_plus: bool) -> EngineResult<Value> {
    if text.is_empty() {
        return Err(EngineError::empty("text"));
    }
    let mut out = match mode {
        "" | "component" => utf8_percent_encode(text, COMPONENT).to_string(),
        "uri" | "full" | "url" => utf8_percent_encode(text, FULL_URL).to_string(),
        "query" | "form" => utf8_percent_encode(text, QUERY).to_string(),
        "path" => utf8_percent_encode(text, PATH).to_string(),
        "all" => text.bytes().map(|b| format!("%{b:02X}")).collect(),
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown mode `{other}` (component, uri, query, path, all)"
            )))
        },
    };
    if space_as_plus {
        out = out.replace("%20", "+");
    }
    Ok(json!({ "result": out, "length": out.len() }))
}

/// Percent-decoding (optionally treating `+` as space).
pub fn url_decode(text: &str, plus_as_space: bool) -> EngineResult<Value> {
    if text.is_empty() {
        return Err(EngineError::empty("text"));
    }
    let pre = if plus_as_space {
        text.replace('+', " ")
    } else {
        text.to_string()
    };
    let bytes = percent_decode_str(&pre).collect::<Vec<u8>>();
    let s = String::from_utf8_lossy(&bytes).into_owned();
    let lossy = String::from_utf8(bytes.clone()).is_err();
    let mut warnings = Vec::new();
    if lossy {
        warnings
            .push("Decoded bytes are not valid UTF-8; invalid sequences were replaced".to_string());
    }
    // Detect double encoding.
    let double =
        s.contains('%') && percent_decode_str(&s).decode_utf8().map(|d| d != s).unwrap_or(false);
    Ok(json!({ "result": s, "double_encoded": double, "warnings": warnings }))
}

const NAMED: &[(char, &str)] = &[
    ('&', "amp"),
    ('<', "lt"),
    ('>', "gt"),
    ('"', "quot"),
    ('\'', "apos"),
    ('\u{a0}', "nbsp"),
    ('©', "copy"),
    ('®', "reg"),
    ('™', "trade"),
    ('€', "euro"),
    ('£', "pound"),
    ('¥', "yen"),
    ('¢', "cent"),
    ('§', "sect"),
    ('°', "deg"),
    ('±', "plusmn"),
    ('×', "times"),
    ('÷', "divide"),
    ('—', "mdash"),
    ('–', "ndash"),
    ('…', "hellip"),
    ('«', "laquo"),
    ('»', "raquo"),
    ('“', "ldquo"),
    ('”', "rdquo"),
    ('‘', "lsquo"),
    ('’', "rsquo"),
    ('•', "bull"),
    ('·', "middot"),
    ('¶', "para"),
    ('←', "larr"),
    ('→', "rarr"),
    ('↑', "uarr"),
    ('↓', "darr"),
    ('♥', "hearts"),
    ('✓', "check"),
    ('é', "eacute"),
    ('è', "egrave"),
    ('à', "agrave"),
    ('á', "aacute"),
    ('ü', "uuml"),
    ('ö', "ouml"),
    ('ä', "auml"),
    ('ß', "szlig"),
    ('ç', "ccedil"),
    ('ñ', "ntilde"),
    ('¿', "iquest"),
    ('¡', "iexcl"),
];

/// HTML entity encoding. `mode`: `minimal` (& < > " '), `non-ascii`
/// (also every non-ASCII char), `all` (every character).
pub fn html_encode(text: &str, mode: &str, named: bool) -> EngineResult<Value> {
    if text.is_empty() {
        return Err(EngineError::empty("text"));
    }
    let out: String = text
        .chars()
        .map(|c| {
            let special = matches!(c, '&' | '<' | '>' | '"' | '\'');
            let encode = match mode {
                "" | "minimal" => special,
                "non-ascii" | "nonascii" => special || !c.is_ascii(),
                "all" => true,
                _ => special,
            };
            if !encode {
                return c.to_string();
            }
            if named {
                if let Some((_, n)) = NAMED.iter().find(|(ch, _)| *ch == c) {
                    return format!("&{n};");
                }
            }
            match c {
                '&' => "&amp;".into(),
                '<' => "&lt;".into(),
                '>' => "&gt;".into(),
                '"' => "&quot;".into(),
                '\'' => "&#39;".into(),
                _ => format!("&#{};", c as u32),
            }
        })
        .collect();
    Ok(json!({ "result": out, "length": out.len() }))
}

/// Decodes named, decimal and hexadecimal HTML entities.
pub fn html_decode(text: &str) -> EngineResult<Value> {
    if text.is_empty() {
        return Err(EngineError::empty("text"));
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut unknown = Vec::new();
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        match after.find(';').filter(|&n| n <= 12) {
            Some(semi) => {
                let ent = &after[..semi];
                let decoded =
                    if let Some(h) = ent.strip_prefix("#x").or_else(|| ent.strip_prefix("#X")) {
                        u32::from_str_radix(h, 16).ok().and_then(char::from_u32)
                    } else if let Some(d) = ent.strip_prefix('#') {
                        d.parse::<u32>().ok().and_then(char::from_u32)
                    } else {
                        NAMED.iter().find(|(_, n)| *n == ent).map(|(c, _)| *c)
                    };
                match decoded {
                    Some(c) => {
                        out.push(c);
                        rest = &after[semi + 1..];
                    },
                    None => {
                        if !ent.is_empty()
                            && ent.chars().all(|c| c.is_ascii_alphanumeric() || c == '#')
                        {
                            unknown.push(format!("&{ent};"));
                        }
                        out.push('&');
                        rest = after;
                    },
                }
            },
            None => {
                out.push('&');
                rest = after;
            },
        }
    }
    out.push_str(rest);
    Ok(json!({ "result": out, "unknown_entities": unknown }))
}

/// IDNA/Punycode encoding of a domain (`xn--…`).
pub fn punycode_encode(text: &str) -> EngineResult<Value> {
    let t = text.trim();
    if t.is_empty() {
        return Err(EngineError::empty("text"));
    }
    // Accept full URLs: only convert the host.
    let (prefix, host, suffix) = split_host(t);
    let ascii = idna::domain_to_ascii(host)
        .map_err(|e| EngineError::invalid(format!("Invalid domain: {e:?}")))?;
    Ok(json!({
        "result": format!("{prefix}{ascii}{suffix}"),
        "host": ascii,
        "changed": ascii != host,
        "labels": ascii.split('.').map(|l| json!({ "label": l, "punycode": l.starts_with("xn--") })).collect::<Vec<_>>()
    }))
}

/// IDNA/Punycode decoding of a domain.
pub fn punycode_decode(text: &str) -> EngineResult<Value> {
    let t = text.trim();
    if t.is_empty() {
        return Err(EngineError::empty("text"));
    }
    let (prefix, host, suffix) = split_host(t);
    let (unicode, result) = idna::domain_to_unicode(host);
    if let Err(e) = result {
        return Err(EngineError::invalid(format!("Invalid IDNA domain: {e:?}")));
    }
    Ok(json!({
        "result": format!("{prefix}{unicode}{suffix}"),
        "host": unicode,
        "changed": unicode != host
    }))
}

fn split_host(t: &str) -> (&str, &str, &str) {
    if let Some(idx) = t.find("://") {
        let after = &t[idx + 3..];
        let end = after.find(['/', '?', '#']).unwrap_or(after.len());
        let host = &after[..end];
        let host_start = host.rfind('@').map(|i| i + 1).unwrap_or(0);
        let host_end = host[host_start..]
            .find(':')
            .map(|i| host_start + i)
            .unwrap_or(host.len());
        return (
            &t[..idx + 3 + host_start],
            &host[host_start..host_end],
            &t[idx + 3 + host_end..],
        );
    }
    (&t[..0], t, &t[t.len()..])
}

/// Quoted-printable encoding (RFC 2045).
pub fn qp_encode(text: &str) -> EngineResult<Value> {
    if text.is_empty() {
        return Err(EngineError::empty("text"));
    }
    let mut out = String::new();
    let mut line_len = 0;
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let token: String = if b == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
            i += 2;
            out.push_str("\r\n");
            line_len = 0;
            continue;
        } else if b == b'\n' {
            i += 1;
            out.push_str("\r\n");
            line_len = 0;
            continue;
        } else if (33..=126).contains(&b) && b != b'=' || b == b' ' || b == b'\t' {
            // Trailing whitespace before a line break must be encoded.
            let next_is_break = matches!(bytes.get(i + 1), Some(b'\n') | Some(b'\r') | None);
            if (b == b' ' || b == b'\t') && next_is_break {
                format!("={b:02X}")
            } else {
                (b as char).to_string()
            }
        } else {
            format!("={b:02X}")
        };
        if line_len + token.len() > 75 {
            out.push_str("=\r\n");
            line_len = 0;
        }
        out.push_str(&token);
        line_len += token.len();
        i += 1;
    }
    Ok(json!({ "result": out }))
}

/// Quoted-printable decoding.
pub fn qp_decode(text: &str) -> EngineResult<Value> {
    if text.is_empty() {
        return Err(EngineError::empty("text"));
    }
    let out = qp_decode_bytes(text);
    Ok(json!({ "result": String::from_utf8_lossy(&out).into_owned(), "bytes": out.len() }))
}

fn qp_decode_bytes(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'=' {
            if bytes.get(i + 1) == Some(&b'\r') && bytes.get(i + 2) == Some(&b'\n') {
                i += 3;
                continue;
            }
            if bytes.get(i + 1) == Some(&b'\n') {
                i += 2;
                continue;
            }
            if i + 2 < bytes.len() {
                if let Ok(v) = u8::from_str_radix(
                    std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("zz"),
                    16,
                ) {
                    out.push(v);
                    i += 3;
                    continue;
                }
            }
            out.push(b'=');
            i += 1;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    out
}

/// RFC 2047 encoded-word encoding (`=?charset?B?...?=` / `?Q?`).
pub fn mime_word_encode(text: &str, encoding: &str, charset: &str) -> EngineResult<Value> {
    if text.is_empty() {
        return Err(EngineError::empty("text"));
    }
    let cs = if charset.is_empty() { "UTF-8" } else { charset };
    if !cs.eq_ignore_ascii_case("utf-8") && !cs.eq_ignore_ascii_case("us-ascii") {
        return Err(EngineError::unsupported(
            "Only UTF-8 / US-ASCII encoding is supported in the browser engine",
        ));
    }
    let result = match encoding.to_ascii_uppercase().as_str() {
        "" | "B" | "BASE64" => format!("=?{cs}?B?{}?=", STANDARD.encode(text.as_bytes())),
        "Q" | "QUOTED-PRINTABLE" => {
            let body: String = text
                .bytes()
                .map(|b| match b {
                    b' ' => "_".to_string(),
                    b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'!' | b'*' | b'+' | b'-' | b'/' => {
                        (b as char).to_string()
                    },
                    _ => format!("={b:02X}"),
                })
                .collect();
            format!("=?{cs}?Q?{body}?=")
        },
        other => return Err(EngineError::invalid(format!("Unknown encoding `{other}` (B or Q)"))),
    };
    Ok(json!({ "result": result }))
}

/// Decodes RFC 2047 encoded words (`=?UTF-8?B?…?=`).
pub fn mime_word_decode(text: &str) -> EngineResult<Value> {
    if text.trim().is_empty() {
        return Err(EngineError::empty("text"));
    }
    let mut out = String::new();
    let mut rest = text;
    let mut words = 0;
    let mut charsets = Vec::new();
    while let Some(start) = rest.find("=?") {
        let before = &rest[..start];
        // Whitespace between adjacent encoded words is ignored.
        if !(words > 0 && before.trim().is_empty()) {
            out.push_str(before);
        }
        let body = &rest[start + 2..];
        let Some(end) = body.find("?=") else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let parts: Vec<&str> = body[..end].splitn(3, '?').collect();
        if parts.len() != 3 {
            out.push_str(&rest[start..start + 2]);
            rest = &rest[start + 2..];
            continue;
        }
        let (charset, enc, data) = (parts[0], parts[1], parts[2]);
        let bytes = match enc.to_ascii_uppercase().as_str() {
            "B" => STANDARD
                .decode(data)
                .or_else(|_| STANDARD_NO_PAD.decode(data))
                .map_err(|_| EngineError::invalid("Invalid base64 in encoded word"))?,
            "Q" => qp_decode_bytes(&data.replace('_', " ")),
            other => {
                return Err(EngineError::invalid(format!(
                    "Unknown encoded-word encoding `{other}`"
                )))
            },
        };
        let decoded = match charset.to_ascii_lowercase().split('*').next().unwrap_or("") {
            "utf-8" | "utf8" | "us-ascii" | "ascii" => String::from_utf8_lossy(&bytes).into_owned(),
            "iso-8859-1" | "latin1" | "windows-1252" | "cp1252" => {
                bytes.iter().map(|&b| b as char).collect()
            },
            other => {
                charsets.push(format!("{other} (decoded as Latin-1)"));
                bytes.iter().map(|&b| b as char).collect()
            },
        };
        out.push_str(&decoded);
        words += 1;
        if !charsets.contains(&charset.to_string()) {
            charsets.push(charset.to_string());
        }
        rest = &body[end + 2..];
    }
    out.push_str(rest);
    Ok(json!({ "result": out, "encoded_words": words, "charsets": charsets }))
}

/// Guesses what encoding a string is in.
pub fn detect(text: &str) -> EngineResult<Value> {
    let t = text.trim();
    if t.is_empty() {
        return Err(EngineError::empty("text"));
    }
    let mut guesses: Vec<Value> = Vec::new();
    let push = |g: &mut Vec<Value>, kind: &str, confidence: f64, decoded: Option<String>| {
        g.push(json!({
            "encoding": kind,
            "confidence": confidence,
            "decoded_preview": decoded.map(|d| d.chars().take(80).collect::<String>())
        }))
    };
    if t.starts_with("data:") && t.contains(";base64,") {
        push(&mut guesses, "data-url", 0.99, None);
    }
    if t.len() >= 4
        && t.len().is_multiple_of(4)
        && t.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
        && !t.chars().all(|c| c.is_ascii_digit())
    {
        if let Ok(d) = STANDARD.decode(t) {
            push(
                &mut guesses,
                "base64",
                if std::str::from_utf8(&d).is_ok() { 0.85 } else { 0.6 },
                String::from_utf8(d).ok(),
            );
        }
    }
    if t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && t.len() >= 8
        && t.contains(['-', '_'])
    {
        if let Ok(d) = URL_SAFE_NO_PAD.decode(t) {
            push(&mut guesses, "base64url", 0.6, String::from_utf8(d).ok());
        }
    }
    if t.len().is_multiple_of(2) && t.len() >= 4 && t.chars().all(|c| c.is_ascii_hexdigit()) {
        let d = hex::decode(t).unwrap_or_default();
        push(
            &mut guesses,
            "hex",
            if std::str::from_utf8(&d)
                .map(|s| s.chars().all(|c| !c.is_control()))
                .unwrap_or(false)
            {
                0.8
            } else {
                0.5
            },
            String::from_utf8(d).ok(),
        );
    }
    if t.contains('%') && percent_decode_str(t).decode_utf8().map(|d| d != t).unwrap_or(false) {
        push(
            &mut guesses,
            "url-encoded",
            0.8,
            percent_decode_str(t).decode_utf8().ok().map(|d| d.into_owned()),
        );
    }
    if t.contains('&')
        && t.contains(';')
        && (t.contains("&#") || t.contains("&amp;") || t.contains("&lt;"))
    {
        push(
            &mut guesses,
            "html-entities",
            0.8,
            html_decode(t)
                .ok()
                .and_then(|v| v["result"].as_str().map(str::to_string)),
        );
    }
    if t.starts_with("=?") && t.ends_with("?=") {
        push(
            &mut guesses,
            "mime-encoded-word",
            0.95,
            mime_word_decode(t)
                .ok()
                .and_then(|v| v["result"].as_str().map(str::to_string)),
        );
    }
    if t.contains("xn--") {
        push(
            &mut guesses,
            "punycode",
            0.9,
            punycode_decode(t)
                .ok()
                .and_then(|v| v["result"].as_str().map(str::to_string)),
        );
    }
    if t.split('.').count() == 3
        && t.split('.').all(|p| {
            !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
    {
        push(&mut guesses, "jwt", 0.85, None);
    }
    if t.chars().all(|c| c == '0' || c == '1' || c.is_whitespace()) && t.len() >= 8 {
        push(&mut guesses, "binary", 0.7, None);
    }
    if t.contains("=") && t.contains('\n') && t.lines().all(|l| l.len() <= 76) && t.contains("=\n")
    {
        push(&mut guesses, "quoted-printable", 0.5, None);
    }
    if t.len() >= 8
        && t.chars().all(|c| {
            (c.is_ascii_uppercase() && c != '0' && c != '1' && c != '8' && c != '9')
                || ('2'..='7').contains(&c)
                || c == '='
        })
    {
        push(&mut guesses, "base32", 0.5, None);
    }
    if guesses.is_empty() {
        push(&mut guesses, "plain-text", 0.5, None);
    }
    guesses.sort_by(|a, b| {
        b["confidence"]
            .as_f64()
            .unwrap_or(0.0)
            .total_cmp(&a["confidence"].as_f64().unwrap_or(0.0))
    });
    let best = guesses.first().map(|g| g["encoding"].clone()).unwrap_or(Value::Null);
    Ok(json!({ "guesses": guesses, "best": best }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_codecs() {
        assert_eq!(base64_encode(b"hello", false, true, 0, None).unwrap()["result"], "aGVsbG8=");
        assert_eq!(base64_encode(b"\xfb\xff", true, false, 0, None).unwrap()["result"], "-_8");
        let wrapped = base64_encode(&[0u8; 60], false, true, 76, Some("image/png")).unwrap();
        assert!(wrapped["result"].as_str().unwrap().contains('\n'));
        assert!(wrapped["data_url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,"));
        let d = base64_decode("aGVsbG8=", None).unwrap();
        assert_eq!(d["text"], "hello");
        assert_eq!(d["detected_mime"], "text/plain");
        assert!(base64_decode("@@", None).is_err());
        assert!(base64_decode("", None).is_err());
        assert_eq!(base32_encode(b"foobar", true).unwrap()["result"], "MZXW6YTBOI======");
        assert_eq!(base32_decode("MZXW6YTBOI======").unwrap()["text"], "foobar");
        assert!(base32_decode("1").is_err());
        assert_eq!(hex_encode(b"\x01\xff", true, " ", "0x").unwrap()["result"], "0x01 0xFF");
        assert_eq!(hex_decode("0x48 0x69").unwrap()["text"], "Hi");
        assert!(hex_decode("zz").is_err());
    }

    #[test]
    fn url_and_html() {
        assert_eq!(
            url_encode("a b&c=d/é", "component", false).unwrap()["result"],
            "a%20b%26c%3Dd%2F%C3%A9"
        );
        assert_eq!(
            url_encode("https://x.io/a b?q=1&r=é", "uri", false).unwrap()["result"],
            "https://x.io/a%20b?q=1&r=%C3%A9"
        );
        assert_eq!(url_encode("a b", "query", true).unwrap()["result"], "a+b");
        assert_eq!(url_decode("a%20b+c", true).unwrap()["result"], "a b c");
        assert_eq!(url_decode("%2520", false).unwrap()["double_encoded"], true);
        assert!(url_encode("x", "weird", false).is_err());
        assert_eq!(
            html_encode("<a href=\"x\">Tom & Jerry's</a>", "minimal", false).unwrap()["result"],
            "&lt;a href=&quot;x&quot;&gt;Tom &amp; Jerry&#39;s&lt;/a&gt;"
        );
        assert_eq!(
            html_encode("café ©", "non-ascii", true).unwrap()["result"],
            "caf&eacute; &copy;"
        );
        assert_eq!(html_encode("é", "non-ascii", false).unwrap()["result"], "&#233;");
        let d = html_decode("&lt;b&gt; &amp;&copy;&#169;&#xA9;&bogus; &").unwrap();
        assert_eq!(d["result"], "<b> &©©©&bogus; &");
        assert_eq!(d["unknown_entities"], json!(["&bogus;"]));
    }

    #[test]
    fn punycode_qp_mime() {
        let p = punycode_encode("https://bücher.example/päth?x=1").unwrap();
        assert_eq!(p["result"], "https://xn--bcher-kva.example/päth?x=1");
        assert_eq!(punycode_decode("xn--bcher-kva.example").unwrap()["result"], "bücher.example");
        assert_eq!(punycode_encode("plain.com").unwrap()["changed"], false);
        assert!(punycode_encode("").is_err());
        let q = qp_encode("Héllo = world \nline2").unwrap();
        assert_eq!(q["result"], "H=C3=A9llo =3D world=20\r\nline2");
        assert_eq!(
            qp_decode("H=C3=A9llo =3D world=20\r\nline2 soft=\r\nwrap").unwrap()["result"],
            "Héllo = world \r\nline2 softwrap"
        );
        let b = mime_word_encode("Héllo", "B", "").unwrap();
        assert_eq!(b["result"], "=?UTF-8?B?SMOpbGxv?=");
        let qw = mime_word_encode("Hé lo", "Q", "").unwrap();
        assert_eq!(qw["result"], "=?UTF-8?Q?H=C3=A9_lo?=");
        assert_eq!(
            mime_word_decode("Subject: =?UTF-8?B?SMOpbGxv?= =?UTF-8?Q?_w=C3=B6rld?=").unwrap()
                ["result"],
            "Subject: Héllo wörld"
        );
        assert_eq!(mime_word_decode("=?ISO-8859-1?Q?caf=E9?=").unwrap()["result"], "café");
        assert!(mime_word_decode("=?UTF-8?X?abc?=").is_err());
    }

    #[test]
    fn detection() {
        assert_eq!(detect("aGVsbG8gd29ybGQ=").unwrap()["best"], "base64");
        assert_eq!(detect("48656c6c6f").unwrap()["best"], "hex");
        assert_eq!(detect("a%20b%3Dc").unwrap()["best"], "url-encoded");
        assert_eq!(detect("=?UTF-8?B?SMOpbGxv?=").unwrap()["best"], "mime-encoded-word");
        assert_eq!(detect("eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.abc").unwrap()["best"], "jwt");
        assert_eq!(detect("just words").unwrap()["best"], "plain-text");
        assert!(detect("  ").is_err());
    }
}
