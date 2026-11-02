//! Base64 helpers for transporting binary data inside the JSON envelope.
//!
//! The existing worker protocol is JSON only, so files are passed as base64
//! strings. Use the serde adaptors on struct fields:
//!
//! ```ignore
//! #[derive(Deserialize)]
//! struct Payload {
//!     #[serde(with = "engine_shared::b64")]
//!     file: Vec<u8>,
//!     #[serde(default, with = "engine_shared::b64::option")]
//!     watermark: Option<Vec<u8>>,
//! }
//! ```
//!
//! Decoding is lenient: standard and URL-safe alphabets, with or without
//! padding, surrounding whitespace and `data:` URL prefixes are all accepted.

use crate::error::{EngineError, EngineResult};
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use serde::{Deserialize, Deserializer, Serializer};

/// Encodes bytes using the standard alphabet with padding.
pub fn encode(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

/// Decodes base64 leniently (see module docs). Returns a structured
/// `E_INVALID_INPUT` error on failure.
pub fn decode(input: &str) -> EngineResult<Vec<u8>> {
    let mut s = input.trim();
    if s.starts_with("data:") {
        if let Some(idx) = s.find(',') {
            s = &s[idx + 1..];
        }
    }
    // Strip embedded whitespace / line breaks (MIME style wrapping).
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.is_empty() {
        return Ok(Vec::new());
    }
    let candidates: [&base64::engine::GeneralPurpose; 4] =
        [&STANDARD, &STANDARD_NO_PAD, &URL_SAFE, &URL_SAFE_NO_PAD];
    for engine in candidates {
        if let Ok(v) = engine.decode(&compact) {
            return Ok(v);
        }
    }
    Err(EngineError::invalid(
        "Value is not valid base64. Provide the file content encoded with base64 (standard or \
         URL-safe alphabet).",
    ))
}

/// Serde `with` adaptor: serialise `Vec<u8>` as base64.
pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&encode(bytes))
}

/// Serde `with` adaptor: deserialise base64 into `Vec<u8>`.
pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
    let s = String::deserialize(d)?;
    decode(&s).map_err(serde::de::Error::custom)
}

/// Adaptor for `Option<Vec<u8>>` fields (`#[serde(default, with = "engine_shared::b64::option")]`).
pub mod option {
    use super::*;

    /// Serialises `Some(bytes)` as a base64 string and `None` as `null`.
    pub fn serialize<S: Serializer>(bytes: &Option<Vec<u8>>, s: S) -> Result<S::Ok, S::Error> {
        match bytes {
            Some(b) => s.serialize_some(&encode(b)),
            None => s.serialize_none(),
        }
    }

    /// Accepts `null`, an empty string or base64 (data-URI prefix allowed).
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<u8>>, D::Error> {
        let s: Option<String> = Option::deserialize(d)?;
        match s {
            None => Ok(None),
            Some(v) if v.trim().is_empty() => Ok(None),
            Some(v) => decode(&v).map(Some).map_err(serde::de::Error::custom),
        }
    }
}

/// Adaptor for `Vec<Vec<u8>>` fields (lists of files).
pub mod list {
    use super::*;

    /// Serialises each item as a base64 string.
    pub fn serialize<S: Serializer>(items: &[Vec<u8>], s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = s.serialize_seq(Some(items.len()))?;
        for b in items {
            seq.serialize_element(&encode(b))?;
        }
        seq.end()
    }

    /// Decodes a JSON array of base64 strings.
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Vec<u8>>, D::Error> {
        let items: Vec<String> = Vec::deserialize(d)?;
        items
            .iter()
            .map(|s| decode(s).map_err(serde::de::Error::custom))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_lenient_decoding() {
        let data = b"hello\x00world";
        let enc = encode(data);
        assert_eq!(decode(&enc).unwrap(), data);
        assert_eq!(decode(enc.trim_end_matches('=')).unwrap(), data);
        assert_eq!(decode(&format!("data:application/octet-stream;base64,{enc}")).unwrap(), data);
        assert_eq!(decode(&format!("  {}\n{}  ", &enc[..4], &enc[4..])).unwrap(), data);
        assert_eq!(decode("").unwrap(), Vec::<u8>::new());
        assert!(decode("@@@").is_err());
        assert_eq!(decode("-_-_").unwrap(), URL_SAFE_NO_PAD.decode("-_-_").unwrap());
    }

    #[test]
    fn serde_adaptors() {
        #[derive(serde::Deserialize, serde::Serialize)]
        struct P {
            #[serde(with = "crate::b64")]
            file: Vec<u8>,
            #[serde(default, with = "crate::b64::option")]
            extra: Option<Vec<u8>>,
            #[serde(default, with = "crate::b64::list")]
            files: Vec<Vec<u8>>,
        }
        let p: P = serde_json::from_str(r#"{"file":"aGk=","files":["YQ==","Yg=="]}"#).unwrap();
        assert_eq!(p.file, b"hi");
        assert!(p.extra.is_none());
        assert_eq!(p.files, vec![b"a".to_vec(), b"b".to_vec()]);
        let out = serde_json::to_string(&p).unwrap();
        assert!(out.contains("\"aGk=\""));
        let bad: Result<P, _> = serde_json::from_str(r#"{"file":"***"}"#);
        assert!(bad.is_err());
    }
}
