//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use csv_engine::process_request;
use serde_json::{json, Value};

fn call(action: &str, payload: Value) -> Value {
    serde_json::from_slice(&process_request(
        json!({ "action": action, "payload": payload }).to_string().as_bytes(),
    ))
    .unwrap()
}

const SAMPLE: &str =
    "id,name,amount,date\n1,Ann,10.5,2024-01-02\n2,Bob,20,2024-01-03\n3,Ann,30,2024-02-01\n";

#[test]
fn valid_inputs() {
    let p = call("parse", json!({ "text": SAMPLE }));
    assert_eq!(p["success"], true, "{p}");
    assert_eq!(p["data"]["total_rows"], 3);
    assert_eq!(p["data"]["columns"][2]["type"], "number");
    assert_eq!(p["data"]["columns"][3]["type"], "date");

    let d = call(
        "detect",
        json!({ "file": engine_shared::b64::encode("\u{feff}a;b\r\n1;2\r\n".as_bytes()) }),
    );
    assert_eq!(d["data"]["delimiter"], ";");
    assert_eq!(d["data"]["line_ending"], "crlf");
    assert_eq!(d["data"]["had_bom"], true);

    assert_eq!(
        call("convert", json!({ "text": SAMPLE, "to_delimiter": "\\t" }))["data"]
            ["suggested_extension"],
        "tsv"
    );
    assert_eq!(
        call("convert", json!({ "text": "a\tb\n1\t2\n", "to_delimiter": "," }))["data"]["text"],
        "a,b\n1,2\n"
    );

    let c = call(
        "clean",
        json!({ "text": "A,B\n 1 , x \n\n", "clean": { "header_case": "lower" } }),
    );
    assert_eq!(c["data"]["csv"], "a,b\n1,x\n");

    let v = call(
        "validate",
        json!({
            "text": SAMPLE,
            "schema": {
                "columns": [
                    { "name": "id", "type": "integer", "unique": true },
                    { "name": "amount", "type": "number", "min": 0 }
                ]
            }
        }),
    );
    assert_eq!(v["data"]["valid"], true);
    let v = call("validate", json!({ "text": "a,b\n1,2\n3\n" }));
    assert_eq!(v["data"]["valid"], false);

    let m = call(
        "merge",
        json!({ "files": [{ "text": "a,b\n1,2\n" }, { "text": "b,c\n3,4\n" }] }),
    );
    assert_eq!(m["data"]["csv"], "a,b,c\n1,2,\n,3,4\n");
    let j = call(
        "merge",
        json!({
            "files": [{ "text": "id,n\n1,x\n2,y\n" }, { "text": "id,c\n1,p\n" }],
            "mode": "join",
            "key": "id",
            "join_type": "left"
        }),
    );
    assert_eq!(j["data"]["csv"], "id,n,c\n1,x,p\n2,y,\n");

    assert_eq!(
        call("split", json!({ "text": SAMPLE, "mode": "column", "column": "name" }))["data"]
            ["count"],
        2
    );
    let dd = call("dedupe", json!({ "text": SAMPLE, "columns": ["name"] }));
    assert_eq!(dd["data"]["duplicates_removed"], 1);
    assert_eq!(
        call("select_columns", json!({ "text": SAMPLE, "columns": ["name"] }))["data"]["headers"],
        json!(["name"])
    );
    assert_eq!(
        call(
            "reorder_columns",
            json!({ "text": SAMPLE, "order": ["date", "id"], "drop_rest": true })
        )["data"]["headers"],
        json!(["date", "id"])
    );
    assert_eq!(
        call(
            "filter",
            json!({
                "text": SAMPLE,
                "conditions": [{ "column": "amount", "op": "gt", "value": 15 }]
            })
        )["data"]["rows"],
        2
    );
    let s = call(
        "sort",
        json!({ "text": SAMPLE, "keys": [{ "column": "amount", "order": "desc" }] }),
    );
    assert!(s["data"]["csv"]
        .as_str()
        .unwrap()
        .starts_with("id,name,amount,date\n3,Ann,30"));
    let sql = call("to_sql", json!({ "text": SAMPLE, "table": "t", "dialect": "mysql" }));
    assert!(sql["data"]["sql"].as_str().unwrap().contains("CREATE TABLE `t`"));
    assert_eq!(call("to_jsonl", json!({ "text": "a\n1\n" }))["data"]["text"], "{\"a\":1}");
    assert_eq!(
        call("from_jsonl", json!({ "text": "{\"a\":1}\n{\"a\":2}" }))["data"]["csv"],
        "a\n1\n2\n"
    );
    assert_eq!(
        call("to_json", json!({ "text": "a\n1\n", "pretty": false }))["data"]["text"],
        "[{\"a\":1}]"
    );
    assert_eq!(call("from_json", json!({ "text": "[{\"a\":1}]" }))["data"]["csv"], "a\n1\n");
    assert_eq!(
        call("statistics", json!({ "text": SAMPLE }))["data"]["columns_detail"][2]["numeric"]
            ["sum"],
        60.5
    );
    assert_eq!(call("transpose", json!({ "text": "a,b\n1,2\n" }))["data"]["csv"], "a,1\nb,2\n");
    let df = call(
        "diff",
        json!({
            "a": { "text": "id,v\n1,a\n" },
            "b": { "text": "id,v\n1,b\n" },
            "key_columns": ["id"]
        }),
    );
    assert_eq!(df["data"]["summary"]["changed"], 1);
    let enc = call("encoding_convert", json!({ "text": "a,é", "to": "windows-1252" }));
    assert_eq!(enc["data"]["bytes"], 3);
    assert_eq!(
        call(
            "transform",
            json!({ "text": "n\nab\n", "steps": [{ "column": "n", "op": "uppercase" }] })
        )["data"]["csv"],
        "n\nAB\n"
    );
    assert!(call("to_markdown", json!({ "text": SAMPLE }))["data"]["markdown"]
        .as_str()
        .unwrap()
        .contains("| Ann |"));
    assert!(call("to_html", json!({ "text": SAMPLE }))["data"]["html"]
        .as_str()
        .unwrap()
        .contains("<td>Ann</td>"));
    let w = call(
        "write",
        json!({ "headers": ["a"], "rows": [[1]], "edits": [{ "op": "add_row", "values": [2] }] }),
    );
    assert_eq!(w["data"]["csv"], "a\n1\n2\n");
}

#[test]
fn invalid_inputs() {
    assert_eq!(
        call("select_columns", json!({ "text": SAMPLE, "columns": ["nope"] }))["error_code"],
        "E_NOT_FOUND"
    );
    assert_eq!(
        call(
            "filter",
            json!({ "text": SAMPLE, "conditions": [{ "column": "id", "op": "zzz" }] })
        )["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("merge", json!({ "files": [{ "text": "a\n1\n" }], "mode": "join" }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(call("from_json", json!({ "text": "not json" }))["error_code"], "E_MALFORMED");
    assert_eq!(
        call("encoding_convert", json!({ "text": "a", "to": "klingon" }))["error_code"],
        "E_UNSUPPORTED_FORMAT"
    );
    assert_eq!(
        call("to_sql", json!({ "text": SAMPLE, "dialect": "oracle" }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call(
            "write",
            json!({ "text": "a\n1\n", "edits": [{ "op": "delete_row", "row": 9 }] })
        )["error_code"],
        "E_NOT_FOUND"
    );
    // Garbage bytes still decode (windows-1252 fallback) and parse as a single column.
    let g = call(
        "parse",
        json!({ "file": engine_shared::b64::encode(&[0xff, 0xfe, 0x00, 0x00]) }),
    );
    assert!(g["success"].is_boolean());
    assert_eq!(call("nope", json!({}))["error_code"], "E_BAD_REQUEST");
}

#[test]
fn empty_inputs() {
    assert_eq!(call("parse", json!({ "text": "" }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("parse", json!({}))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("detect", json!({ "text": "   " }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("merge", json!({ "files": [] }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(
        call("filter", json!({ "text": SAMPLE, "conditions": [] }))["error_code"],
        "E_EMPTY_INPUT"
    );
    assert_eq!(
        call("sort", json!({ "text": SAMPLE, "keys": [] }))["error_code"],
        "E_EMPTY_INPUT"
    );
    assert_eq!(call("from_jsonl", json!({ "text": "\n\n" }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(
        call("select_columns", json!({ "text": SAMPLE, "columns": [] }))["error_code"],
        "E_EMPTY_INPUT"
    );
    // Header-only file is valid with zero rows.
    assert_eq!(call("parse", json!({ "text": "a,b\n" }))["data"]["total_rows"], 0);
}
