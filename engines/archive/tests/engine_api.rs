//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use archive_engine::process_request;
use archive_engine::testutil::{sample_files, zip_of};
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
fn full_cycle_through_protocol() {
    let files = json!([
        { "name": "hello.txt", "data": b64::encode(b"hello"), "mtime_unix": 1_700_000_000 },
        { "name": "nested/data.bin", "data": b64::encode(&[0u8; 2048]) }
    ]);
    let c = ok("create", json!({ "files": files, "format": "zip", "password": "pw" }));
    assert_eq!(c["encrypted"], true);
    let archive = c["output"]["file"].clone();
    assert_eq!(ok("detect", json!({ "file": archive }))["kind"], "zip");
    assert_eq!(ok("list", json!({ "file": archive }))["files"], 2);
    assert_eq!(err("extract", json!({ "file": archive })), "E_ENCRYPTED");
    assert_eq!(
        err("extract", json!({ "file": archive, "password": "nope" })),
        "E_WRONG_PASSWORD"
    );
    let e = ok(
        "extract",
        json!({ "file": archive, "password": "pw", "entries": ["hello.txt"] }),
    );
    assert_eq!(e["count"], 1);
    assert_eq!(b64::decode(e["entries"][0]["data"].as_str().unwrap()).unwrap(), b"hello");
    assert_eq!(ok("test", json!({ "file": archive, "password": "pw" }))["ok"], true);
    let conv = ok("convert", json!({ "file": archive, "password": "pw", "to": "tar.gz" }));
    assert_eq!(conv["kind"], "tar.gz");
    let tgz = conv["output"]["file"].clone();
    assert_eq!(ok("list", json!({ "file": tgz }))["files"], 2);
    let gz = ok(
        "create",
        json!({ "files": [{ "name": "x.txt", "data": b64::encode(b"abc") }], "format": "gz" }),
    );
    assert_eq!(
        ok("extract", json!({ "file": gz["output"]["file"] }))["entries"][0]["name"],
        "x.txt"
    );
}

#[test]
fn repair_and_errors() {
    let z = zip_of(&sample_files());
    let broken = b64::encode(&z[..z.len() - 40]);
    assert_eq!(err("list", json!({ "file": broken })), "E_MALFORMED");
    let r = ok("zip_repair", json!({ "file": broken }));
    assert_eq!(r["recovered_entries"], 3);
    assert_eq!(err("list", json!({ "file": "" })), "E_EMPTY_INPUT");
    assert_eq!(
        err("list", json!({ "file": b64::encode(b"plain text") })),
        "E_UNSUPPORTED_FORMAT"
    );
    assert_eq!(err("create", json!({ "files": [] })), "E_EMPTY_INPUT");
    assert_eq!(
        err("create", json!({ "files": [{ "name": "a", "data": "AA==" }], "format": "rar" })),
        "E_INVALID_INPUT"
    );
    assert_eq!(err("seven_zip_extract", json!({})), "E_UNSUPPORTED");
    assert_eq!(err("rar_extract", json!({})), "E_UNSUPPORTED");
    assert_eq!(
        err("extract", json!({ "file": b64::encode(&z), "max_total_bytes": 1 })),
        "E_LIMIT_EXCEEDED"
    );
}
