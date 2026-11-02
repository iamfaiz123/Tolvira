//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{json, Value};
use spreadsheet_engine::process_request;

fn call(action: &str, payload: Value) -> Value {
    serde_json::from_slice(&process_request(
        json!({ "action": action, "payload": payload }).to_string().as_bytes(),
    ))
    .unwrap()
}

fn b64(v: &[u8]) -> String {
    engine_shared::b64::encode(v)
}

fn sample() -> String {
    let v = call(
        "from_csv",
        json!({ "text": "name,qty,price\nA,2,1.5\nB,3,2\nA,2,1.5\n", "name": "sample.csv" }),
    );
    assert_eq!(v["success"], true, "{v}");
    v["data"]["file"].as_str().unwrap().to_string()
}

#[test]
fn valid_inputs() {
    let file = sample();
    let i = call("inspect", json!({ "file": file, "name": "sample.xlsx" }));
    assert_eq!(i["data"]["format"], "xlsx");
    assert_eq!(i["data"]["sheets"][0]["name"], "sample");
    let r = call("read", json!({ "file": file, "limit": 2 }));
    assert_eq!(r["data"]["total_rows"], 3);
    assert_eq!(r["data"]["has_more"], true);
    assert_eq!(r["data"]["rows"][0][1], 2);
    assert_eq!(
        call("to_csv", json!({ "file": file }))["data"]["text"],
        "name,qty,price\nA,2,1.5\nB,3,2\nA,2,1.5\n"
    );
    assert_eq!(call("to_json", json!({ "file": file }))["data"]["data"][0]["price"], 1.5);
    let x = call("from_json", json!({ "data": [{ "a": 1 }, { "a": 2 }] }));
    assert_eq!(x["data"]["sheets"][0]["rows"], 3);
    assert_eq!(
        call("convert", json!({ "file": file, "to": "ods" }))["data"]["mime"],
        "application/vnd.oasis.opendocument.spreadsheet"
    );
    assert_eq!(call("convert", json!({ "file": file, "to": "csv" }))["data"]["rows"], 3);
    assert_eq!(
        call(
            "merge",
            json!({
                "files": [{ "file": file, "name": "a.xlsx" }, { "file": file, "name": "b.xlsx" }]
            })
        )["data"]["sheets"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(call("split_sheets", json!({ "file": file }))["data"]["count"], 1);
    assert_eq!(
        call("dedupe", json!({ "file": file }))["data"]["report"]["duplicates_removed"],
        1
    );
    let c = call("clean", json!({ "file": file, "clean": { "header_case": "upper" } }));
    assert_eq!(c["success"], true);
    let f = call("formulas", json!({ "file": file }));
    assert_eq!(f["data"]["total_formulas"], 0);
    let e = call(
        "evaluate",
        json!({ "formula": "=IF(SUM(A1:A2)>2,\"big\",\"small\")", "grid": [[1], [2]] }),
    );
    assert_eq!(e["data"]["value"], "big");
    let e = call("evaluate", json!({ "formula": "=TODAY()+1", "today_serial": 45292 }));
    assert_eq!(e["data"]["value"], 45293);
    assert_eq!(
        call("diff", json!({ "a": { "file": file }, "b": { "file": file } }))["data"]["identical"],
        true
    );
    assert_eq!(
        call("column_letter", json!({ "input": 703 }))["data"]["results"][0]["letter"],
        "AAA"
    );
    assert!(call("to_html", json!({ "file": file }))["data"]["html"]
        .as_str()
        .unwrap()
        .contains("<table>"));
}

#[test]
fn invalid_inputs() {
    let file = sample();
    assert_eq!(
        call("read", json!({ "file": file, "sheet": "nope" }))["error_code"],
        "E_NOT_FOUND"
    );
    assert_eq!(
        call(
            "inspect",
            json!({ "file": b64(b"PK\x03\x04 this is not a real zip archive at all") })
        )["error_code"],
        "E_MALFORMED"
    );
    // CFB header with no stream → malformed (not encrypted).
    let cfb = call(
        "inspect",
        json!({
            "file": b64(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1, 0, 0, 0, 0, 0, 0, 0, 0])
        }),
    );
    assert_eq!(cfb["error_code"], "E_MALFORMED", "{cfb}");
    assert_eq!(
        call(
            "inspect",
            json!({ "file": b64(&[0x89, b'P', b'N', b'G', 0, 0, 0, 0, 0, 0, 0, 0]) })
        )["error_code"],
        "E_UNSUPPORTED_FORMAT"
    );
    assert_eq!(
        call("convert", json!({ "file": file, "to": "pdf" }))["error_code"],
        "E_UNSUPPORTED_FORMAT"
    );
    assert_eq!(
        call("merge", json!({ "files": [{ "file": file }], "mode": "weird" }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("column_letter", json!({ "input": "A1:B" }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(call("from_json", json!({ "text": "[1,2,3]" }))["error_code"], "E_INVALID_INPUT");
    let e = call("evaluate", json!({ "formula": "=1/0" }));
    assert_eq!(e["data"]["is_error"], true);
    assert_eq!(e["data"]["display"], "#DIV/0!");
    assert_eq!(call("nope", json!({}))["error_code"], "E_BAD_REQUEST");
}

#[test]
fn empty_inputs() {
    assert_eq!(call("inspect", json!({ "file": "" }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("inspect", json!({}))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("from_csv", json!({}))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("from_csv", json!({ "text": "" }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("from_json", json!({}))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("merge", json!({ "files": [] }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("evaluate", json!({ "formula": "   " }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("column_letter", json!({ "input": [] }))["error_code"], "E_EMPTY_INPUT");
}
