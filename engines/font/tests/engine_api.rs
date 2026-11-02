//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use font_engine::process_request;
use serde_json::{json, Value};

const FONT: &[u8] = include_bytes!("../../image/assets/DejaVuSans-subset.ttf");

fn call(action: &str, payload: Value) -> Value {
    serde_json::from_slice(&process_request(
        json!({ "action": action, "payload": payload }).to_string().as_bytes(),
    ))
    .unwrap()
}
fn ok(action: &str, payload: Value) -> Value {
    let v = call(action, payload);
    assert_eq!(v["success"], true, "{action}: {v}");
    v["data"].clone()
}
fn err(action: &str, payload: Value) -> String {
    let v = call(action, payload);
    assert_eq!(v["success"], false, "{action}: {v}");
    v["error_code"].as_str().unwrap_or_default().to_string()
}
fn font() -> String {
    engine_shared::b64::encode(FONT)
}

#[test]
fn valid_inputs() {
    let i = ok("inspect", json!({ "file": font(), "name": "DejaVuSans.ttf" }));
    assert_eq!(i["container"], "ttf");
    assert!(i["glyphs"]["count"].as_u64().unwrap() > 10, "{i}");
    assert!(i["names"]["family"].as_str().unwrap().contains("DejaVu"), "{i}");
    let m = ok("char_map", json!({ "file": font(), "limit": 20 }));
    assert!(m["glyphs"].as_array().unwrap().len() <= 20, "{m}");
    assert!(m["total"].as_u64().unwrap() > 20);
    let p = ok(
        "preview",
        json!({ "file": font(), "text": "Hello", "size_px": 32, "color": "#333333" }),
    );
    assert_eq!(p["mime"], "image/png");
    assert!(p["width"].as_u64().unwrap() > 20);
    let s = ok("size_check", json!({ "file": font(), "sizes": [12, 16, 24] }));
    assert_eq!(s["sizes"].as_array().unwrap().len(), 3, "{s}");
    let pr = ok("pairing", json!({ "file": font(), "role": "heading" }));
    assert!(pr["suggestions"].as_array().unwrap().len() >= 2, "{pr}");
    let c = ok("convert", json!({ "file": font(), "to": "woff", "name": "a.ttf" }));
    assert_eq!(c["filename"], "a.woff");
    let back = ok("convert", json!({ "file": c["file"], "to": "ttf" }));
    assert_eq!(back["mime"], "font/ttf");
    let sub = ok("subset", json!({ "file": font(), "text": "Hello World" }));
    assert!(sub["bytes"].as_u64().unwrap() < FONT.len() as u64, "{sub}");
    assert_eq!(sub["characters"], 8);
}

#[test]
fn invalid_inputs() {
    let junk = engine_shared::b64::encode(b"this is definitely not a font file at all");
    assert_eq!(err("inspect", json!({ "file": junk })), "E_UNSUPPORTED_FORMAT");
    let mut truncated = FONT[..200].to_vec();
    truncated[0] = 0;
    assert!(!err("inspect", json!({ "file": engine_shared::b64::encode(&truncated) })).is_empty());
    assert_eq!(err("convert", json!({ "file": font(), "to": "woff2" })), "E_UNSUPPORTED");
    assert_eq!(err("convert", json!({ "file": font(), "to": "svg" })), "E_INVALID_INPUT");
    let bad_b64 = err("inspect", json!({ "file": "@@not base64@@" }));
    assert!(bad_b64 == "E_INVALID_INPUT" || bad_b64 == "E_BAD_REQUEST", "{bad_b64}");
    assert_eq!(
        err("preview", json!({ "file": font(), "text": "x", "color": "nope" })),
        "E_INVALID_INPUT"
    );
    assert_eq!(err("no_such_action", json!({})), "E_BAD_REQUEST");
    let v: Value = serde_json::from_slice(&process_request(b"{not json")).unwrap();
    assert_eq!(v["error_code"], "E_BAD_REQUEST");
}

#[test]
fn empty_inputs() {
    for action in ["inspect", "char_map", "preview", "size_check", "pairing", "convert", "subset"] {
        assert_eq!(err(action, json!({})), "E_EMPTY_INPUT", "{action}");
        assert_eq!(err(action, json!({ "file": "" })), "E_EMPTY_INPUT", "{action}");
    }
    assert_eq!(err("subset", json!({ "file": font(), "text": "" })), "E_EMPTY_INPUT");
}
