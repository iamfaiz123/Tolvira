//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{json, Value};
use text_engine::process_request;

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
fn legacy_protocol_compatible_with_v1_ui() {
    assert_eq!(
        ok(
            "convert_case",
            json!({ "text": "Universal Utility Platform", "target_case": "kebab" })
        )["result"],
        "universal-utility-platform"
    );
    assert_eq!(
        ok(
            "analyze",
            json!({ "text": "Hello world. This is a fast Rust WebAssembly engine." })
        )["words"],
        9
    );
    assert!(
        ok("format_json", json!({ "text": "{\"foo\":\"bar\",\"num\":42}", "indent": 2 }))["result"]
            .as_str()
            .unwrap()
            .contains("\n")
    );
    assert_eq!(ok("reverse", json!({ "text": "abc" }))["result"], "cba");
    assert_eq!(
        ok("clean", json!({ "text": "b\na\nb", "dedup_lines": true, "sort_lines": true }))
            ["result"],
        "a\nb"
    );
    assert_eq!(
        err("convert_case", json!({ "text": "x", "target_case": "nope" })),
        "E_INVALID_INPUT"
    );
}

#[test]
fn valid_invalid_empty() {
    assert_eq!(ok("stats", json!({ "text": "One two three." }))["words"], 3);
    assert_eq!(ok("stats", json!({ "text": "" }))["words"], 0);
    assert_eq!(err("stats", json!({ "text": "x", "reading_wpm": 0 })), "E_INVALID_INPUT");
    assert_eq!(
        ok("case", json!({ "text": "hello world", "target": "pascal" }))["result"],
        "HelloWorld"
    );
    assert_eq!(ok("slug", json!({ "text": "Crème Brûlée!" }))["slug"], "creme-brulee");
    assert_eq!(err("slug", json!({ "text": "" })), "E_EMPTY_INPUT");
    assert_eq!(ok("reverse_text", json!({ "text": "a b", "mode": "words" }))["result"], "b a");
    assert_eq!(err("reverse_text", json!({ "text": "" })), "E_EMPTY_INPUT");
    assert_eq!(
        ok("truncate", json!({ "text": "hello world", "max_chars": 8, "suffix": "..." }))["result"],
        "hello..."
    );
    assert_eq!(
        ok("cleanup", json!({ "text": "a   b", "collapse_spaces": true }))["result"],
        "a b"
    );
    assert_eq!(ok("wrap", json!({ "text": "aa bb cc", "width": 5 }))["lines"], 2);
    assert_eq!(
        ok("find_replace", json!({ "text": "cat cat", "find": "cat", "replace": "dog" }))["result"],
        "dog dog"
    );
    assert_eq!(
        err("find_replace", json!({ "text": "x", "find": "[", "regex": true })),
        "E_INVALID_INPUT"
    );
    assert_eq!(
        ok(
            "lines",
            json!({ "text": "b\na\na", "operations": [{ "op": "dedupe" }, { "op": "sort" }] })
        )["result"],
        "a\nb"
    );
    assert_eq!(err("lines", json!({ "text": "x", "operations": [] })), "E_EMPTY_INPUT");
    assert_eq!(
        err("lines", json!({ "text": "x", "operations": [{ "op": "explode" }] })),
        "E_BAD_REQUEST"
    );
    assert_eq!(ok("join", json!({ "text": "a\nb", "separator": "|" }))["result"], "a|b");
    assert_eq!(ok("split", json!({ "text": "a;b", "delimiter": ";" }))["count"], 2);
    assert_eq!(ok("diff", json!({ "a": "x\ny", "b": "x\nz" }))["added"], 1);
    assert_eq!(err("diff", json!({ "a": "", "b": "" })), "E_EMPTY_INPUT");
    assert_eq!(ok("encode_codes", json!({ "text": "A", "base": 2 }))["result"], "01000001");
    assert_eq!(ok("decode_codes", json!({ "codes": "01000001", "base": 2 }))["result"], "A");
    assert_eq!(err("decode_codes", json!({ "codes": "0102", "base": 2 })), "E_INVALID_INPUT");
    assert_eq!(ok("rot", json!({ "text": "abc" }))["result"], "nop");
    assert_eq!(ok("morse_encode", json!({ "text": "e" }))["result"], ".");
    assert_eq!(ok("morse_decode", json!({ "code": ".-" }))["result"], "a");
    assert_eq!(
        ok("unicode_inspect", json!({ "text": "é" }))["characters"][0]["code_point"],
        "U+00E9"
    );
    assert_eq!(
        ok("unicode_normalize", json!({ "text": "é", "form": "NFD" }))["code_points_after"],
        2
    );
    assert_eq!(ok("unicode_escape", json!({ "text": "€" }))["result"], "\\u20AC");
    assert_eq!(ok("unicode_unescape", json!({ "text": "\\u20AC" }))["result"], "€");
    assert_eq!(ok("extract", json!({ "text": "a@b.co", "kind": "emails" }))["count"], 1);
    assert!(ok("markdown_to_html", json!({ "markdown": "# Hi" }))["html"]
        .as_str()
        .unwrap()
        .contains("<h1"));
    assert_eq!(err("markdown_to_html", json!({ "markdown": "  " })), "E_EMPTY_INPUT");
    assert_eq!(
        ok("markdown_parse", json!({ "markdown": "# A\n## B" }))["headings"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(ok("html_to_markdown", json!({ "html": "<h2>Hi</h2>" }))["markdown"], "## Hi");
    assert_eq!(ok("html_to_text", json!({ "html": "<p>Hi</p>" }))["text"], "Hi");
    assert_eq!(ok("lorem", json!({ "count": 2, "unit": "sentences" }))["paragraphs"], 1);
    assert_eq!(
        ok("random_text", json!({ "kind": "digits", "count": 1, "length": 6 }))["result"]
            .as_str()
            .unwrap()
            .len(),
        6
    );
    assert!(
        ok("tts_prep", json!({ "text": "Hello Dr. Who. Second." }))["chunk_count"]
            .as_u64()
            .unwrap()
            >= 1
    );
    assert_eq!(ok("hashtags", json!({ "text": "rust rust wasm" }))["hashtags"][0], "#rust");
    assert_eq!(
        ok("keyword_density", json!({ "text": "rust rust wasm" }))["one_word"][0]["count"],
        2
    );
}
