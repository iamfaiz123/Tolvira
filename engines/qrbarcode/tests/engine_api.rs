//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use qrbarcode_engine::process_request;
use serde_json::{json, Value};

fn call(action: &str, payload: Value) -> Value {
    serde_json::from_slice(&process_request(
        json!({ "action": action, "payload": payload }).to_string().as_bytes(),
    ))
    .unwrap()
}

#[test]
fn valid_inputs() {
    let g = call(
        "qr_generate",
        json!({
            "data": "hello",
            "formats": ["svg", "png", "ascii"],
            "style": { "module_shape": "rounded", "eye_shape": "circle", "foreground": "#112233" }
        }),
    );
    assert_eq!(g["success"], true, "{g}");
    assert!(g["data"]["svg"].as_str().unwrap().contains("<circle"));
    let png = g["data"]["png"].as_str().unwrap().to_string();
    let d = call("qr_decode", json!({ "image": png }));
    assert_eq!(d["data"]["codes"][0]["content"], "hello");
    let w = call(
        "qr_wifi",
        json!({ "ssid": "Home", "password": "supersecret", "security": "WPA2" }),
    );
    assert_eq!(w["data"]["payload"], "WIFI:T:WPA;S:Home;P:supersecret;;");
    assert_eq!(w["data"]["payload_type"], "wifi");
    let v = call("qr_vcard", json!({ "first_name": "A", "last_name": "B", "email": "a@b.c" }));
    assert!(v["data"]["payload"].as_str().unwrap().starts_with("BEGIN:VCARD"));
    assert_eq!(
        call("qr_url", json!({ "url": "example.org" }))["data"]["payload"],
        "https://example.org"
    );
    assert_eq!(call("qr_email", json!({ "to": "a@b.c" }))["data"]["payload"], "mailto:a@b.c");
    assert_eq!(
        call("qr_sms", json!({ "number": "+1555", "message": "hi" }))["data"]["payload"],
        "SMSTO:+1555:hi"
    );
    assert_eq!(
        call("qr_geo", json!({ "latitude": 1, "longitude": 2 }))["data"]["payload"],
        "geo:1,2"
    );
    assert!(
        call("qr_event", json!({ "title": "Meet", "start": "2024-01-01T10:00:00" }))["data"]
            ["payload"]
            .as_str()
            .unwrap()
            .contains("DTSTART:20240101T100000")
    );
    let b = call("qr_bulk", json!({ "items": ["a", { "data": "b", "label": "second" }, ""] }));
    assert_eq!(b["data"]["count"], 3);
    assert_eq!(b["data"]["failures"], 1);
    assert_eq!(b["data"]["items"][1]["filename"], "qr_002_second.svg");
    assert_eq!(
        call("payload_inspect", json!({ "text": "mailto:x@y.z" }))["data"]["type"],
        "email"
    );
    let bc = call(
        "barcode_generate",
        json!({ "symbology": "code128", "data": "ABC-12345678", "formats": ["svg", "png"] }),
    );
    assert_eq!(bc["data"]["symbology"], "Code 128");
    assert!(bc["data"]["png"].is_string());
    let ean = call("barcode_generate", json!({ "symbology": "EAN-13", "data": "590123412345" }));
    assert_eq!(ean["data"]["text"], "5901234123457");
    let r = call("barcode_read", json!({ "image": ean["data"]["png"] }));
    assert_eq!(r["data"]["barcodes"][0]["value"], "5901234123457");
    assert_eq!(
        call("barcode_validate", json!({ "code": "9780306406157" }))["data"]["valid"],
        true
    );
}

#[test]
fn invalid_inputs() {
    assert_eq!(
        call("qr_generate", json!({ "data": "x", "style": { "error_correction": "Z" } }))
            ["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("qr_generate", json!({ "data": "x", "style": { "foreground": "#zz" } }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("qr_generate", json!({ "data": "x".repeat(4000) }))["error_code"],
        "E_LIMIT_EXCEEDED"
    );
    assert_eq!(
        call("qr_generate", json!({ "data": "x".repeat(200), "style": { "version": 1 } }))
            ["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("qr_wifi", json!({ "ssid": "x", "security": "WPA" }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(call("qr_url", json!({ "url": "has space" }))["error_code"], "E_INVALID_INPUT");
    assert_eq!(
        call("qr_geo", json!({ "latitude": 91, "longitude": 0 }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("qr_decode", json!({ "image": engine_shared::b64::encode(b"garbage bytes") }))
            ["error_code"],
        "E_UNSUPPORTED_FORMAT"
    );
    assert_eq!(
        call("barcode_generate", json!({ "symbology": "ean13", "data": "12AB" }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("barcode_generate", json!({ "symbology": "pdf417", "data": "x" }))["error_code"],
        "E_UNSUPPORTED"
    );
    assert_eq!(
        call("barcode_validate", json!({ "code": "4006381333932" }))["data"]["valid"],
        false
    );
    assert_eq!(
        call("barcode_validate", json!({ "code": "abc" }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(call("nope", json!({}))["error_code"], "E_BAD_REQUEST");
}

#[test]
fn empty_inputs() {
    assert_eq!(call("qr_generate", json!({ "data": "" }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("qr_wifi", json!({ "ssid": "" }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("qr_url", json!({ "url": "" }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("qr_bulk", json!({ "items": [] }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("qr_decode", json!({ "image": "" }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("payload_inspect", json!({ "text": "" }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(
        call("barcode_generate", json!({ "symbology": "code128", "data": "" }))["error_code"],
        "E_EMPTY_INPUT"
    );
    assert_eq!(call("barcode_validate", json!({ "code": " " }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("barcode_read", json!({ "image": "" }))["error_code"], "E_EMPTY_INPUT");
}
