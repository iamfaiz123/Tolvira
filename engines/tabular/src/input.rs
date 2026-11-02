//! Input handling: text or uploaded file bytes with encoding detection.

use engine_shared::{EngineError, EngineResult};
use serde::Deserialize;
use serde_json::{json, Value};

/// Either inline `text` or a base64 `file`. When both are present `text` wins.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Source {
    /// Inline text (wins over `file`).
    pub text: Option<String>,
    /// File bytes (base64 on the wire).
    #[serde(with = "engine_shared::b64::option")]
    pub file: Option<Vec<u8>>,
    /// Force a source encoding label (`utf-8`, `utf-16le`, `windows-1252`, ...). Default auto.
    pub encoding: Option<String>,
    /// Original file name (used for output names and TSV detection).
    pub name: Option<String>,
}

/// Text decoded from a [`Source`] with encoding facts.
#[derive(Debug, Clone)]
pub struct Decoded {
    /// Decoded UTF-8 text (BOM removed).
    pub text: String,
    /// Encoding label that was used.
    pub encoding: String,
    /// A byte-order mark was present.
    pub had_bom: bool,
    /// Invalid sequences were replaced with U+FFFD.
    pub had_replacements: bool,
}

impl Source {
    /// `true` when neither `text` nor `file` carries data.
    pub fn is_empty(&self) -> bool {
        self.text.as_deref().map(|t| t.trim().is_empty()).unwrap_or(true)
            && self.file.as_deref().map(|f| f.is_empty()).unwrap_or(true)
    }

    /// Decodes the source (BOM/UTF-16 detection, optional forced encoding).
    pub fn decode(&self) -> EngineResult<Decoded> {
        if let Some(t) = &self.text {
            if !t.trim().is_empty() {
                let had_bom = t.starts_with('\u{feff}');
                return Ok(Decoded {
                    text: t.clone(),
                    encoding: "utf-8".into(),
                    had_bom,
                    had_replacements: false,
                });
            }
        }
        match &self.file {
            Some(bytes) if !bytes.is_empty() => {
                engine_shared::limits::check_size(
                    bytes.len(),
                    engine_shared::limits::MAX_FILE_BYTES,
                    "file",
                )?;
                decode_bytes(bytes, self.encoding.as_deref())
            },
            _ => Err(EngineError::empty("csv")),
        }
    }

    /// Convenience: decodes and returns just the text.
    pub fn text(&self) -> EngineResult<String> {
        self.decode().map(|d| d.text)
    }
}

/// Resolves an encoding label to an `encoding_rs` encoding.
pub fn lookup_encoding(label: &str) -> EngineResult<&'static encoding_rs::Encoding> {
    let l = label.trim().to_ascii_lowercase().replace('_', "-");
    let l = match l.as_str() {
        "utf8" | "utf-8-bom" | "utf-8-sig" | "utf8-bom" => "utf-8",
        "utf16" | "utf-16" | "utf16le" => "utf-16le",
        "utf16be" => "utf-16be",
        "latin1" | "latin-1" | "iso8859-1" | "iso-8859-1" | "cp1252" | "windows1252" => {
            "windows-1252"
        },
        "ascii" | "us-ascii" => "windows-1252",
        other => other,
    };
    encoding_rs::Encoding::for_label(l.as_bytes())
        .ok_or_else(|| EngineError::unsupported_format(format!("Unknown encoding `{label}`")))
}

/// Decodes bytes to a string, honouring BOMs and falling back to
/// Windows-1252 when the bytes are not valid UTF-8.
pub fn decode_bytes(bytes: &[u8], forced: Option<&str>) -> EngineResult<Decoded> {
    if let Some(label) = forced.filter(|l| !l.trim().is_empty() && !l.eq_ignore_ascii_case("auto"))
    {
        let enc = lookup_encoding(label)?;
        let (cow, actual, had_replacements) = enc.decode(bytes);
        let had_bom = cow.starts_with('\u{feff}')
            || bytes.starts_with(&[0xEF, 0xBB, 0xBF])
            || bytes.starts_with(&[0xFF, 0xFE])
            || bytes.starts_with(&[0xFE, 0xFF]);
        return Ok(Decoded {
            text: cow.trim_start_matches('\u{feff}').to_string(),
            encoding: actual.name().to_ascii_lowercase(),
            had_bom,
            had_replacements,
        });
    }
    if let Some((enc, bom_len)) = encoding_rs::Encoding::for_bom(bytes) {
        let (cow, had_replacements) = enc.decode_without_bom_handling(&bytes[bom_len..]);
        return Ok(Decoded {
            text: cow.into_owned(),
            encoding: enc.name().to_ascii_lowercase(),
            had_bom: true,
            had_replacements,
        });
    }
    // UTF-16 without BOM heuristic: many NUL bytes in even/odd positions.
    if bytes.len() >= 4 {
        let nul_odd = bytes.iter().skip(1).step_by(2).filter(|b| **b == 0).count();
        let nul_even = bytes.iter().step_by(2).filter(|b| **b == 0).count();
        let half = bytes.len() / 2;
        if nul_odd > half / 2 && nul_even == 0 {
            let (cow, rep) = encoding_rs::UTF_16LE.decode_without_bom_handling(bytes);
            return Ok(Decoded {
                text: cow.into_owned(),
                encoding: "utf-16le".into(),
                had_bom: false,
                had_replacements: rep,
            });
        }
        if nul_even > half / 2 && nul_odd == 0 {
            let (cow, rep) = encoding_rs::UTF_16BE.decode_without_bom_handling(bytes);
            return Ok(Decoded {
                text: cow.into_owned(),
                encoding: "utf-16be".into(),
                had_bom: false,
                had_replacements: rep,
            });
        }
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => Ok(Decoded {
            text: s.to_string(),
            encoding: if bytes.is_ascii() {
                "ascii".into()
            } else {
                "utf-8".into()
            },
            had_bom: false,
            had_replacements: false,
        }),
        Err(_) => {
            let (cow, rep) = encoding_rs::WINDOWS_1252.decode_without_bom_handling(bytes);
            Ok(Decoded {
                text: cow.into_owned(),
                encoding: "windows-1252".into(),
                had_bom: false,
                had_replacements: rep,
            })
        },
    }
}

/// Encodes text into the target encoding, optionally prefixing a BOM.
pub fn encode_text(text: &str, target: &str, bom: Option<bool>) -> EngineResult<(Vec<u8>, String)> {
    let label = target.trim().to_ascii_lowercase();
    let wants_bom = bom.unwrap_or(
        label.contains("bom")
            || label.contains("sig")
            || label.starts_with("utf-16")
            || label.starts_with("utf16"),
    );
    let text = text.trim_start_matches('\u{feff}');
    let out = match label.as_str() {
        "utf-16le" | "utf16le" | "utf-16" | "utf16" => {
            let mut v = Vec::with_capacity(text.len() * 2 + 2);
            if wants_bom {
                v.extend_from_slice(&[0xFF, 0xFE]);
            }
            for u in text.encode_utf16() {
                v.extend_from_slice(&u.to_le_bytes());
            }
            (v, "utf-16le".to_string())
        },
        "utf-16be" | "utf16be" => {
            let mut v = Vec::with_capacity(text.len() * 2 + 2);
            if wants_bom {
                v.extend_from_slice(&[0xFE, 0xFF]);
            }
            for u in text.encode_utf16() {
                v.extend_from_slice(&u.to_be_bytes());
            }
            (v, "utf-16be".to_string())
        },
        _ => {
            let enc = lookup_encoding(&label)?;
            let (cow, _, unmappable) = enc.encode(text);
            if unmappable {
                return Err(EngineError::invalid(format!(
                    "Text contains characters that cannot be represented in {}",
                    enc.name()
                )));
            }
            let mut v = Vec::new();
            if wants_bom && enc == encoding_rs::UTF_8 {
                v.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
            }
            v.extend_from_slice(&cow);
            (v, enc.name().to_ascii_lowercase())
        },
    };
    Ok(out)
}

/// Encoding facts of a [`Decoded`] as JSON.
pub fn decoded_json(d: &Decoded) -> Value {
    json!({
        "encoding": d.encoding,
        "had_bom": d.had_bom,
        "had_replacement_characters": d.had_replacements
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_encodings() {
        let d = decode_bytes(b"\xEF\xBB\xBFa,b\n1,2\n", None).unwrap();
        assert_eq!(d.encoding, "utf-8");
        assert!(d.had_bom);
        assert_eq!(d.text, "a,b\n1,2\n");
        let d = decode_bytes(b"caf\xE9", None).unwrap();
        assert_eq!(d.encoding, "windows-1252");
        assert_eq!(d.text, "café");
        let (bytes, _) = encode_text("a,b", "utf-16le", None).unwrap();
        assert_eq!(bytes[..2], [0xFF, 0xFE]);
        let d = decode_bytes(&bytes, None).unwrap();
        assert_eq!(d.text, "a,b");
        let (bytes, _) = encode_text("a,b", "utf-16le", Some(false)).unwrap();
        assert_eq!(decode_bytes(&bytes, None).unwrap().encoding, "utf-16le");
        assert!(encode_text("日本", "windows-1252", None).is_err());
        assert!(lookup_encoding("klingon").is_err());
    }
}
