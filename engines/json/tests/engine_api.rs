//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use json_engine::process_request;
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

const SAMPLE: &str = r#"{"name":"Dev","tags":["a","b"],"nested":{"n":1,"ok":true}}"#;

#[test]
fn json_core_actions() {
    assert!(ok("format", json!({ "text": SAMPLE, "indent": 4 }))["result"]
        .as_str()
        .unwrap()
        .contains("    \"name\""));
    assert_eq!(ok("minify", json!({ "text": "{ \"a\" : 1 }" }))["result"], "{\"a\":1}");
    assert_eq!(ok("validate", json!({ "text": "{" }))["valid"], false);
    assert_eq!(ok("validate", json!({ "text": SAMPLE }))["valid"], true);
    assert_eq!(ok("repair", json!({ "text": "{a: 1, b: 'x',}" }))["value"]["b"], "x");
    assert_eq!(ok("escape", json!({ "text": "a\"b" }))["result"], "a\\\"b");
    assert_eq!(ok("unescape", json!({ "text": "a\\nb" }))["result"], "a\nb");
    assert!(ok("sort_keys", json!({ "text": SAMPLE }))["text"]
        .as_str()
        .unwrap()
        .starts_with("{\n  \"name\""));
    assert_eq!(ok("sort_array", json!({ "text": "[3,1,2]" }))["result"], json!([1, 2, 3]));
    assert_eq!(ok("flatten", json!({ "text": SAMPLE }))["result"]["nested.n"], 1);
    assert_eq!(
        ok("unflatten", json!({ "text": "{\"a.b\":1}" }))["result"],
        json!({ "a": { "b": 1 } })
    );
    assert_eq!(ok("to_table", json!({ "text": "[{\"a\":1}]" }))["columns"], json!(["a"]));
    assert_eq!(ok("diff", json!({ "a": "{\"a\":1}", "b": "{\"a\":2}" }))["changed"], 1);
    assert_eq!(
        ok(
            "apply_patch",
            json!({ "text": "{\"a\":1}", "patch": [{ "op": "replace", "path": "/a", "new": 5 }] })
        )["result"]["a"],
        5
    );
    assert_eq!(ok("path_query", json!({ "text": SAMPLE, "path": "$.tags[*]" }))["count"], 2);
    assert_eq!(
        ok("schema_validate", json!({ "schema": "{\"type\":\"number\"}", "data": "\"x\"" }))
            ["valid"],
        false
    );
    assert_eq!(ok("schema_generate", json!({ "text": SAMPLE }))["result"]["type"], "object");
    assert!(ok("to_code", json!({ "text": SAMPLE, "language": "rust" }))["code"]
        .as_str()
        .unwrap()
        .contains("pub struct Root"));
    // empty / invalid
    assert_eq!(err("format", json!({ "text": "" })), "E_EMPTY_INPUT");
    assert_eq!(err("format", json!({ "text": "{oops" })), "E_MALFORMED");
    assert_eq!(err("path_query", json!({ "text": SAMPLE, "path": "$[" })), "E_INVALID_INPUT");
    assert_eq!(err("sort_array", json!({ "text": "{}" })), "E_INVALID_INPUT");
    assert_eq!(err("diff", json!({ "a": "{", "b": "{}" })), "E_MALFORMED");
}

#[test]
fn conversions() {
    let y = ok("to_yaml", json!({ "text": SAMPLE }));
    assert!(y["result"].as_str().unwrap().contains("name: Dev"));
    assert_eq!(ok("from_yaml", json!({ "text": "a: 1" }))["result"]["a"], 1);
    assert_eq!(ok("yaml_validate", json!({ "text": "a: [" }))["valid"], false);
    assert_eq!(ok("yaml_format", json!({ "text": "a:   1" }))["result"], "a: 1\n");
    let x = ok("to_xml", json!({ "text": SAMPLE, "indent": 0, "declaration": false }));
    assert_eq!(x["result"], "<root><name>Dev</name><tags>a</tags><tags>b</tags><nested><n>1</n><ok>true</ok></nested></root>");
    assert_eq!(
        ok("from_xml", json!({ "text": "<r a=\"1\"><x>t</x></r>" }))["result"]["r"]["@a"],
        1
    );
    assert_eq!(
        ok("xml_format", json!({ "text": "<a><b/></a>" }))["result"],
        "<a>\n  <b/>\n</a>"
    );
    assert_eq!(ok("xml_minify", json!({ "text": "<a>\n <b/>\n</a>" }))["result"], "<a><b/></a>");
    assert_eq!(ok("xml_validate", json!({ "text": "<a>" }))["valid"], false);
    assert_eq!(err("xml_format", json!({ "text": "<a>" })), "E_MALFORMED");
    assert!(ok("to_toml", json!({ "text": "{\"a\":1,\"t\":{\"b\":2}}" }))["result"]
        .as_str()
        .unwrap()
        .contains("[t]"));
    assert_eq!(ok("from_toml", json!({ "text": "a = 1" }))["result"]["a"], 1);
    assert_eq!(ok("toml_validate", json!({ "text": "a =" }))["valid"], false);
    assert_eq!(ok("toml_format", json!({ "text": "a=1" }))["result"], "a = 1\n");
    assert_eq!(ok("from_ini", json!({ "text": "[s]\nk=v" }))["result"]["s"]["k"], "v");
    assert_eq!(
        ok("to_ini", json!({ "text": "{\"s\":{\"k\":\"v\"}}" }))["result"],
        "[s]\nk = v\n"
    );
    assert_eq!(ok("ini_format", json!({ "text": "[s]\nk  =  v" }))["result"], "[s]\nk = v\n");
    assert_eq!(
        ok(
            "from_plist",
            json!({ "text": "<plist><dict><key>a</key><integer>1</integer></dict></plist>" })
        )["result"]["a"],
        1
    );
    assert_eq!(ok("to_csv", json!({ "text": "[{\"a\":1,\"b\":\"x\"}]" }))["csv"], "a,b\n1,x\n");
    assert_eq!(ok("from_csv", json!({ "text": "a,b\n1,x" }))["result"][0]["a"], 1);
    assert_eq!(ok("to_jsonl", json!({ "text": "[1,{\"a\":2}]" }))["result"], "1\n{\"a\":2}");
    assert_eq!(ok("from_jsonl", json!({ "text": "1\n2" }))["result"], json!([1, 2]));
    assert_eq!(err("from_yaml", json!({ "text": "" })), "E_EMPTY_INPUT");
    assert_eq!(err("from_csv", json!({ "text": "   " })), "E_EMPTY_INPUT");
    assert_eq!(err("to_toml", json!({ "text": "[1]" })), "E_INVALID_INPUT");
}
