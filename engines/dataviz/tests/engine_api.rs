//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dataviz_engine::process_request;
use serde_json::{json, Value};

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
const CSV: &str = "city,year,temp,rain\nOslo,2020,6.1,760\nOslo,2021,5.8,800\nRome,2020,16.2,700\nRome,2021,16.8,650\nCairo,2020,22.1,25\nCairo,2021,22.9,900\n";

#[test]
fn valid_inputs() {
    let c = ok(
        "chart",
        json!({
            "csv": CSV,
            "spec": { "kind": "bar", "x": "city", "y": ["temp"], "aggregate": "mean" },
            "chart": { "title": "Temp" }
        }),
    );
    assert!(c["svg"].as_str().unwrap().contains("Temp"));
    assert_eq!(c["data"]["series"][0]["values"][1], 16.5);
    let j = ok(
        "chart",
        json!({ "json": [{ "k": "a", "v": 3 }, { "k": "b", "v": 5 }], "spec": { "kind": "donut" } }),
    );
    assert_eq!(j["meta"]["total"], 8.0);
    let line = ok(
        "chart",
        json!({
            "csv": CSV,
            "spec": { "kind": "line", "x": "year", "y": ["rain"], "aggregate": "sum" },
            "chart": { "smooth": true }
        }),
    );
    assert_eq!(line["points"], 2);
    let h = ok("histogram", json!({ "csv": CSV, "column": "rain", "bins": 4 }));
    assert_eq!(h["bin_count"], 4);
    assert!(h["svg"].is_string());
    assert_eq!(ok("frequency", json!({ "csv": CSV, "column": "city" }))["unique"], 3);
    let p = ok(
        "pivot",
        json!({ "csv": CSV, "rows": "city", "columns": "year", "value": "rain", "aggregate": "sum" }),
    );
    assert_eq!(p["headers"], json!(["city", "2020", "2021", "Total"]));
    assert!(ok("profile", json!({ "csv": CSV }))["correlations"].is_array());
    assert_eq!(ok("describe", json!({ "values": [1, 2, 3, 4] }))["stats"]["mean"], 2.5);
    assert_eq!(ok("describe", json!({ "csv": CSV, "column": "temp" }))["stats"]["count"], 6);
    assert!(
        ok("describe", json!({ "csv": CSV }))["columns"]
            .as_array()
            .unwrap()
            .len()
            >= 2
    );
    assert_eq!(
        ok(
            "outliers",
            json!({ "csv": CSV, "column": "rain", "method": "zscore", "threshold": 1.5 })
        )["method"],
        "zscore"
    );
    assert_eq!(ok("sample", json!({ "csv": CSV, "n": 2, "seed": 3 }))["sampled"], 2);
}

#[test]
fn invalid_inputs() {
    assert_eq!(
        call("chart", json!({ "csv": CSV, "spec": { "kind": "radar" } }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("chart", json!({ "csv": CSV, "spec": { "x": "nope" } }))["error_code"],
        "E_NOT_FOUND"
    );
    assert_eq!(call("chart", json!({ "csv": "a\nx\ny\n" }))["error_code"], "E_INVALID_INPUT");
    assert_eq!(
        call("histogram", json!({ "csv": CSV, "column": "city" }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call(
            "pivot",
            json!({ "csv": CSV, "rows": "city", "value": "rain", "aggregate": "weird" })
        )["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("outliers", json!({ "csv": CSV, "column": "temp", "method": "nope" }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("sample", json!({ "csv": CSV, "method": "stratified" }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(call("chart", json!({ "json": [1, 2, 3] }))["error_code"], "E_INVALID_INPUT");
    assert_eq!(call("nope", json!({}))["error_code"], "E_BAD_REQUEST");
}

#[test]
fn empty_inputs() {
    assert_eq!(call("chart", json!({}))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("chart", json!({ "csv": "   " }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("chart", json!({ "json": [] }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("profile", json!({ "csv": "" }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("describe", json!({}))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("chart", json!({ "csv": "a,b\n" }))["error_code"], "E_EMPTY_INPUT");
}
