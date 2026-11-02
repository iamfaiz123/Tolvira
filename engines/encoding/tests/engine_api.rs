//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use encoding_engine::process_request;
use engine_shared::b64;
use serde_json::{json, Value};

fn call(action: &str, payload: Value) -> Value {
    serde_json::from_slice(&process_request(
        json!({ "action": action, "payload": payload }).to_string().as_bytes(),
    ))
    .unwrap()
}
fn ok(action: &str, payload: Value) -> Value {
    let r = call(action, payload);
    assert_eq!(r["success"], true, "{action}: {r}");
    r["data"].clone()
}
fn err(action: &str, payload: Value) -> String {
    let r = call(action, payload);
    assert_eq!(r["success"], false, "{action}: {r}");
    r["error_code"].as_str().unwrap().to_string()
}

#[test]
fn all_actions() {
    assert_eq!(ok("base64_encode", json!({ "text": "hi" }))["result"], "aGk=");
    assert_eq!(
        ok(
            "base64_encode",
            json!({ "file": b64::encode(b"hi"), "url_safe": true, "padding": false })
        )["result"],
        "aGk"
    );
    assert_eq!(ok("base64_decode", json!({ "text": "aGk=" }))["text"], "hi");
    assert_eq!(err("base64_decode", json!({ "text": "" })), "E_EMPTY_INPUT");
    assert_eq!(err("base64_decode", json!({ "text": "!!" })), "E_INVALID_INPUT");
    assert_eq!(ok("base32_encode", json!({ "text": "a" }))["result"], "ME======");
    assert_eq!(ok("base32_decode", json!({ "text": "ME======" }))["text"], "a");
    assert_eq!(ok("hex_encode", json!({ "text": "A" }))["result"], "41");
    assert_eq!(ok("hex_decode", json!({ "text": "41" }))["text"], "A");
    assert_eq!(ok("url_encode", json!({ "text": "a b" }))["result"], "a%20b");
    assert_eq!(ok("url_decode", json!({ "text": "a%20b" }))["result"], "a b");
    assert_eq!(ok("html_encode", json!({ "text": "<x>" }))["result"], "&lt;x&gt;");
    assert_eq!(ok("html_decode", json!({ "text": "&lt;x&gt;" }))["result"], "<x>");
    assert_eq!(
        ok("punycode_encode", json!({ "text": "münchen.de" }))["result"],
        "xn--mnchen-3ya.de"
    );
    assert_eq!(
        ok("punycode_decode", json!({ "text": "xn--mnchen-3ya.de" }))["result"],
        "münchen.de"
    );
    assert_eq!(ok("quoted_printable_encode", json!({ "text": "é" }))["result"], "=C3=A9");
    assert_eq!(ok("quoted_printable_decode", json!({ "text": "=C3=A9" }))["result"], "é");
    assert_eq!(ok("mime_word_encode", json!({ "text": "é" }))["result"], "=?UTF-8?B?w6k=?=");
    assert_eq!(ok("mime_word_decode", json!({ "text": "=?UTF-8?B?w6k=?=" }))["result"], "é");
    let mp = ok("msgpack_encode", json!({ "json": "[1,2]" }));
    assert_eq!(ok("msgpack_decode", json!({ "data": mp["base64"] }))["result"], json!([1, 2]));
    let cb = ok("cbor_encode", json!({ "json": "{\"a\":true}" }));
    assert_eq!(ok("cbor_decode", json!({ "data": cb["base64"] }))["result"]["a"], true);
    assert_eq!(
        ok("protobuf_inspect", json!({ "data": b64::encode(&[0x08, 0x01]) }))["fields"][0]["value"],
        1
    );
    assert_eq!(err("protobuf_inspect", json!({ "data": "" })), "E_EMPTY_INPUT");
    assert_eq!(ok("detect", json!({ "text": "aGVsbG8=" }))["best"], "base64");
    assert_eq!(err("url_encode", json!({ "text": "" })), "E_EMPTY_INPUT");
}
