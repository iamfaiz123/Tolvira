//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{json, Value};
use units_engine::process_request;

fn call(action: &str, payload: Value) -> Value {
    serde_json::from_slice(&process_request(
        json!({ "action": action, "payload": payload }).to_string().as_bytes(),
    ))
    .unwrap()
}

#[test]
fn protocol() {
    let r = call("convert", json!({ "value": 5, "from": "km", "to": "mi", "precision": 6 }));
    assert_eq!(r["success"], true);
    assert_eq!(r["data"]["result"], 3.10686);
    assert_eq!(
        call("convert", json!({ "value": 1, "from": "kg", "to": "km" }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("convert", json!({ "value": 1, "from": "", "to": "km" }))["error_code"],
        "E_EMPTY_INPUT"
    );
    assert_eq!(
        call("temperature", json!({ "value": 37, "from": "c", "to": "f" }))["data"]["result"],
        98.6
    );
    assert_eq!(
        call(
            "cooking",
            json!({ "value": 200, "from": "g", "to": "cup", "ingredient": "sugar", "precision": 3 })
        )["data"]["result"],
        0.995
    );
    assert_eq!(call("shoe_size", json!({ "size": 40, "system": "eu" }))["data"]["uk"], 6.5);
    assert_eq!(
        call("paper_size", json!({ "name": "Letter" }))["data"]["size"]["inches"]["width"],
        8.5
    );
    assert_eq!(call("pixels", json!({ "inches": 2, "dpi": 300 }))["data"]["pixels"], 600.0);
    assert_eq!(call("categories", json!({}))["success"], true);
    assert_eq!(call("units", json!({ "category": "data" }))["data"]["base"], "b");
    assert_eq!(call("convert", json!({}))["error_code"], "E_BAD_REQUEST");
}
