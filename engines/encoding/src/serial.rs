//! Binary serialisation formats: MessagePack, CBOR and protobuf wire
//! format inspection.

use engine_shared::{b64, limits, EngineError, EngineResult};
use serde_json::{json, Value};

fn parse_json(text: &str) -> EngineResult<Value> {
    if text.trim().is_empty() {
        return Err(EngineError::empty("json"));
    }
    serde_json::from_str(text).map_err(|e| EngineError::malformed(format!("Invalid JSON: {e}")))
}

/// JSON → MessagePack (base64 output).
pub fn msgpack_encode(json_text: &str) -> EngineResult<Value> {
    let v = parse_json(json_text)?;
    let bytes = rmp_serde::to_vec(&v)
        .map_err(|e| EngineError::internal(format!("MessagePack encoding failed: {e}")))?;
    Ok(json!({
        "base64": b64::encode(&bytes),
        "hex": hex::encode(&bytes),
        "bytes": bytes.len(),
        "json_bytes": json_text.len(),
        "saved_percent": ((json_text.len() as f64 - bytes.len() as f64) / json_text.len().max(1) as f64 * 1000.0).round() / 10.0
    }))
}

/// MessagePack → JSON.
pub fn msgpack_decode(data: &[u8]) -> EngineResult<Value> {
    if data.is_empty() {
        return Err(EngineError::empty("data"));
    }
    limits::check_size(data.len(), limits::MAX_FILE_BYTES, "data")?;
    let v: Value = rmp_serde::from_slice(data)
        .map_err(|e| EngineError::malformed(format!("Invalid MessagePack: {e}")))?;
    Ok(json!({ "result": v, "text": serde_json::to_string_pretty(&v)?, "bytes": data.len() }))
}

/// JSON → CBOR (base64 output).
pub fn cbor_encode(json_text: &str) -> EngineResult<Value> {
    let v = parse_json(json_text)?;
    let mut bytes = Vec::new();
    ciborium::into_writer(&v, &mut bytes)
        .map_err(|e| EngineError::internal(format!("CBOR encoding failed: {e}")))?;
    Ok(json!({
        "base64": b64::encode(&bytes),
        "hex": hex::encode(&bytes),
        "bytes": bytes.len(),
        "diagnostic": cbor_diagnostic(&bytes).unwrap_or_default()
    }))
}

/// CBOR → JSON.
pub fn cbor_decode(data: &[u8]) -> EngineResult<Value> {
    if data.is_empty() {
        return Err(EngineError::empty("data"));
    }
    limits::check_size(data.len(), limits::MAX_FILE_BYTES, "data")?;
    let v: ciborium::Value = ciborium::from_reader(data)
        .map_err(|e| EngineError::malformed(format!("Invalid CBOR: {e}")))?;
    let json = cbor_to_json(&v);
    Ok(json!({
        "result": json,
        "text": serde_json::to_string_pretty(&json)?,
        "diagnostic": cbor_diagnostic(data).unwrap_or_default(),
        "bytes": data.len()
    }))
}

fn cbor_to_json(v: &ciborium::Value) -> Value {
    use ciborium::Value as C;
    match v {
        C::Integer(i) => {
            let n: i128 = (*i).into();
            if let Ok(x) = i64::try_from(n) {
                json!(x)
            } else if let Ok(x) = u64::try_from(n) {
                json!(x)
            } else {
                json!(n.to_string())
            }
        },
        C::Bytes(b) => json!({ "$bytes": b64::encode(b), "hex": hex::encode(b) }),
        C::Float(f) => json!(f),
        C::Text(t) => json!(t),
        C::Bool(b) => json!(b),
        C::Null => Value::Null,
        C::Tag(t, inner) => json!({ "$tag": t, "value": cbor_to_json(inner) }),
        C::Array(a) => Value::Array(a.iter().map(cbor_to_json).collect()),
        C::Map(m) => {
            let mut out = serde_json::Map::new();
            for (k, val) in m {
                let key = match k {
                    C::Text(t) => t.clone(),
                    other => serde_json::to_string(&cbor_to_json(other)).unwrap_or_default(),
                };
                out.insert(key, cbor_to_json(val));
            }
            Value::Object(out)
        },
        _ => Value::Null,
    }
}

/// Produces CBOR diagnostic notation (RFC 8949 §8) for display.
fn cbor_diagnostic(data: &[u8]) -> Option<String> {
    let v: ciborium::Value = ciborium::from_reader(data).ok()?;
    fn fmt(v: &ciborium::Value) -> String {
        use ciborium::Value as C;
        match v {
            C::Integer(i) => {
                let n: i128 = (*i).into();
                n.to_string()
            },
            C::Bytes(b) => format!("h'{}'", hex::encode(b)),
            C::Float(f) => format!("{f}"),
            C::Text(t) => format!("{:?}", t),
            C::Bool(b) => b.to_string(),
            C::Null => "null".into(),
            C::Tag(t, inner) => format!("{t}({})", fmt(inner)),
            C::Array(a) => format!("[{}]", a.iter().map(fmt).collect::<Vec<_>>().join(", ")),
            C::Map(m) => format!(
                "{{{}}}",
                m.iter()
                    .map(|(k, v)| format!("{}: {}", fmt(k), fmt(v)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            _ => "undefined".into(),
        }
    }
    Some(fmt(&v))
}

fn read_varint(data: &[u8], pos: &mut usize) -> Option<u64> {
    let mut result = 0u64;
    let mut shift = 0;
    loop {
        let b = *data.get(*pos)?;
        *pos += 1;
        result |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            return Some(result);
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
}

fn decode_fields(data: &[u8], depth: usize) -> Option<Vec<Value>> {
    let mut pos = 0;
    let mut fields = Vec::new();
    while pos < data.len() {
        let start = pos;
        let key = read_varint(data, &mut pos)?;
        let field = key >> 3;
        let wire = key & 7;
        if field == 0 {
            return None;
        }
        let mut entry = json!({ "field": field, "wire_type": wire, "offset": start });
        match wire {
            0 => {
                let v = read_varint(data, &mut pos)?;
                let zigzag = ((v >> 1) as i64) ^ -((v & 1) as i64);
                entry["type"] = json!("varint");
                entry["value"] = json!(v);
                entry["as_signed"] = json!(v as i64);
                entry["as_zigzag"] = json!(zigzag);
                entry["as_bool"] = json!(v != 0);
            },
            1 => {
                let b: [u8; 8] = data.get(pos..pos + 8)?.try_into().ok()?;
                pos += 8;
                entry["type"] = json!("64-bit");
                entry["as_double"] = json!(f64::from_le_bytes(b));
                entry["as_fixed64"] = json!(u64::from_le_bytes(b));
            },
            2 => {
                let len = read_varint(data, &mut pos)? as usize;
                let b = data.get(pos..pos + len)?;
                pos += len;
                entry["type"] = json!("length-delimited");
                entry["length"] = json!(len);
                if let Ok(s) = std::str::from_utf8(b) {
                    if s.chars().all(|c| !c.is_control() || c.is_whitespace()) {
                        entry["as_string"] = json!(s);
                    }
                }
                entry["as_hex"] = json!(hex::encode(&b[..b.len().min(64)]));
                if depth < 6 && !b.is_empty() {
                    if let Some(nested) = decode_fields(b, depth + 1) {
                        if !nested.is_empty() {
                            entry["as_message"] = Value::Array(nested);
                        }
                    }
                }
            },
            5 => {
                let b: [u8; 4] = data.get(pos..pos + 4)?.try_into().ok()?;
                pos += 4;
                entry["type"] = json!("32-bit");
                entry["as_float"] = json!(f32::from_le_bytes(b));
                entry["as_fixed32"] = json!(u32::from_le_bytes(b));
            },
            3 | 4 => {
                entry["type"] = json!(if wire == 3 {
                    "group-start (deprecated)"
                } else {
                    "group-end"
                });
            },
            _ => return None,
        }
        fields.push(entry);
        if fields.len() > 10_000 {
            break;
        }
    }
    Some(fields)
}

/// Schema-less decoding of a protobuf message into its wire fields.
pub fn protobuf_inspect(data: &[u8]) -> EngineResult<Value> {
    if data.is_empty() {
        return Err(EngineError::empty("data"));
    }
    limits::check_size(data.len(), 64 * 1024 * 1024, "data")?;
    let fields = decode_fields(data, 0).ok_or_else(|| {
        EngineError::malformed("Data is not a valid protobuf wire-format message")
    })?;
    Ok(json!({
        "fields": fields,
        "field_count": fields.len(),
        "bytes": data.len(),
        "note": "Decoded without a schema: strings, nested messages and numeric interpretations \
                 are best-effort guesses"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msgpack_cbor_roundtrip() {
        let j = r#"{"a":1,"b":[true,null,"x"],"c":1.5}"#;
        let m = msgpack_encode(j).unwrap();
        let back = msgpack_decode(&b64::decode(m["base64"].as_str().unwrap()).unwrap()).unwrap();
        assert_eq!(back["result"]["b"][2], "x");
        assert!(msgpack_decode(&[0xc1]).is_err());
        assert!(msgpack_encode("{").is_err());
        let c = cbor_encode(j).unwrap();
        assert!(c["diagnostic"].as_str().unwrap().contains("\"a\": 1"));
        let back = cbor_decode(&b64::decode(c["base64"].as_str().unwrap()).unwrap()).unwrap();
        assert_eq!(back["result"]["c"], 1.5);
        // Tagged + bytes.
        let tagged = cbor_decode(&[0xc1, 0x1a, 0x51, 0x4b, 0x67, 0xb0]).unwrap();
        assert_eq!(tagged["result"]["$tag"], 1);
        assert!(cbor_decode(&[0xff]).is_err());
    }

    #[test]
    fn protobuf() {
        // field 1 varint 150, field 2 string "testing", field 3 nested {field 1: 1}
        let data = [
            0x08, 0x96, 0x01, 0x12, 0x07, b't', b'e', b's', b't', b'i', b'n', b'g', 0x1a, 0x02,
            0x08, 0x01,
        ];
        let p = protobuf_inspect(&data).unwrap();
        assert_eq!(p["field_count"], 3);
        assert_eq!(p["fields"][0]["value"], 150);
        assert_eq!(p["fields"][1]["as_string"], "testing");
        assert_eq!(p["fields"][2]["as_message"][0]["value"], 1);
        assert!(protobuf_inspect(&[0x00]).is_err());
        assert!(protobuf_inspect(&[]).is_err());
    }
}
