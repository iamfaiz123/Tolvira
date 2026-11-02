//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use developer_engine::process_request;
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

fn err(action: &str, payload: Value, code: &str) {
    let v = call(action, payload);
    assert_eq!(v["success"], false, "{action} should fail: {v}");
    assert_eq!(v["error_code"], code, "{action}: {v}");
}

#[test]
fn valid_inputs() {
    assert_eq!(
        ok("regex_test", json!({ "pattern": "\\d+", "flags": "g", "text": "a1 b22" }))
            ["match_count"],
        2
    );
    assert_eq!(ok("regex_explain", json!({ "pattern": "^a+$" }))["valid"], true);
    assert_eq!(
        ok(
            "cron_parse",
            json!({ "expression": "0 9 * * 1-5", "from": "2024-01-06T00:00:00Z", "count": 1 })
        )["next_runs"][0]["utc"],
        "2024-01-08T09:00:00Z"
    );
    assert_eq!(
        ok("cron_generate", json!({ "preset": "daily", "hour": 3 }))["normalized"],
        "0 3 * * *"
    );
    assert_eq!(
        ok("semver_check", json!({ "version": "1.2.3", "range": "^1.0" }))["range"]["satisfied"],
        true
    );
    assert_eq!(ok("semver_sort", json!({ "versions": ["1.0.0", "0.9.0"] }))["latest"], "1.0.0");
    assert_eq!(
        ok(
            "semver_max_satisfying",
            json!({ "versions": ["1.0.0", "1.5.0", "2.0.0"], "range": "^1" })
        )["max_satisfying"],
        "1.5.0"
    );
    assert_eq!(ok("color_convert", json!({ "color": "#ff0000" }))["hsl"]["h"], 0.0);
    assert_eq!(
        ok("color_contrast", json!({ "foreground": "black", "background": "white" }))["ratio"],
        21.0
    );
    assert_eq!(ok("color_mix", json!({ "a": "#000", "b": "#fff" }))["result"]["hex"], "#808080");
    assert!(ok("sql_format", json!({ "sql": "select 1" }))["formatted"]
        .as_str()
        .unwrap()
        .starts_with("SELECT"));
    assert_eq!(ok("sql_minify", json!({ "sql": "select  1 ;" }))["minified"], "select 1;");
    assert_eq!(ok("sql_validate", json!({ "sql": "SELECT a FROM t" }))["valid"], true);
    assert_eq!(
        ok("sql_erd", json!({ "sql": "CREATE TABLE t (id INT PRIMARY KEY)" }))["entity_count"],
        1
    );
    assert_eq!(ok("graphql_format", json!({ "text": "{a}" }))["formatted"], "{\n  a\n}\n");
    assert_eq!(
        ok("graphql_validate", json!({ "text": "type Query { a: Int }" }))["valid"],
        true
    );
    assert_eq!(
        ok("graphql_schema", json!({ "text": "type Query { a: Int }" }))["type_count"],
        1
    );
    assert_eq!(
        ok("html_format", json!({ "html": "<div><p>x</p></div>" }))["formatted"],
        "<div>\n  <p>x</p>\n</div>\n"
    );
    assert_eq!(ok("html_minify", json!({ "html": "<p>  a  </p>" }))["minified"], "<p>a</p>");
    assert!(ok("html_to_jsx", json!({ "html": "<div class=\"x\"></div>" }))["jsx"]
        .as_str()
        .unwrap()
        .contains("className"));
    assert_eq!(ok("css_format", json!({ "css": "a{b:c}" }))["formatted"], "a {\n  b: c;\n}\n");
    assert_eq!(ok("css_minify", json!({ "css": "a { b : c ; }" }))["minified"], "a{b:c}");
    assert_eq!(ok("js_format", json!({ "source": "let a=1" }))["formatted"], "let a = 1\n");
    assert_eq!(
        ok("js_minify", json!({ "source": "let a = 1; // c\nb()" }))["minified"],
        "let a=1;b()"
    );
    assert_eq!(
        ok("js_inspect", json!({ "source": "let a: number = 1" }))["looks_like_typescript"],
        true
    );
    assert_eq!(ok("lint_markdown", json!({ "text": "# T\n\nx\n" }))["valid"], true);
    assert_eq!(ok("lint_dockerfile", json!({ "text": "FROM a:1\nUSER x\n" }))["valid"], true);
    assert_eq!(ok("lint_yaml", json!({ "text": "a: 1\n" }))["valid"], true);
    assert_eq!(
        ok(
            "validate_compose",
            json!({ "text": "services:\n  a:\n    image: x:1\n    restart: always\n" })
        )["valid"],
        true
    );
    assert!(ok("gitignore", json!({ "stacks": ["rust"] }))["gitignore"]
        .as_str()
        .unwrap()
        .contains("/target/"));
    assert!(ok("gitignore_templates", json!({}))["templates"].is_array());
    assert_eq!(ok("env_parse", json!({ "text": "A=1\nB=${A}2" }))["variables"]["B"], "12");
    assert_eq!(ok("diff", json!({ "old": "a\n", "new": "b\n" }))["added"], 1);
    assert!(ok("diff_words", json!({ "old": "a b", "new": "a c" }))["segments"].is_array());
    let patch = ok("diff", json!({ "old": "a\nb\n", "new": "a\nc\n" }))["unified"].clone();
    assert_eq!(ok("patch_parse", json!({ "patch": patch }))["file_count"], 1);
    assert_eq!(
        ok("patch_apply", json!({ "text": "a\nb\n", "patch": patch }))["result"],
        "a\nc\n"
    );
    assert_eq!(ok("chmod", json!({ "mode": "644" }))["symbolic"], "rw-r--r--");
    assert_eq!(
        ok("mime", json!({ "query": "json" }))["by_extension"][0]["mime"],
        "application/json"
    );
    assert_eq!(
        ok("mime", json!({ "sample": engine_shared::b64::encode(b"%PDF-1.7 rest") }))["sniffed"]
            ["mime"],
        "application/pdf"
    );
    assert_eq!(ok("http_status", json!({ "query": "418" }))["match"]["code"], 418);
    assert!(ok("http_status_all", json!({}))["codes"].as_array().unwrap().len() > 50);
    assert_eq!(ok("user_agent", json!({ "user_agent": "curl/8.0" }))["bot"]["name"], "curl");
    assert_eq!(ok("url_parse", json!({ "url": "https://a.b/c?d=1" }))["query_object"]["d"], "1");
    assert_eq!(ok("query_parse", json!({ "query": "a=1&b=2" }))["count"], 2);
    assert_eq!(ok("query_build", json!({ "object": { "a": 1 } }))["query_string"], "a=1");
}

#[test]
fn invalid_inputs() {
    err("regex_test", json!({ "pattern": "(", "text": "x" }), "E_INVALID_INPUT");
    err(
        "regex_test",
        json!({ "pattern": "a", "flags": "z", "text": "x" }),
        "E_INVALID_INPUT",
    );
    err("cron_parse", json!({ "expression": "* *" }), "E_INVALID_INPUT");
    assert_eq!(ok("cron_parse", json!({ "expression": "99 * * * *" }))["valid"], false);
    err("cron_generate", json!({ "preset": "nope" }), "E_INVALID_INPUT");
    err("semver_check", json!({ "version": "x.y" }), "E_INVALID_INPUT");
    err(
        "semver_check",
        json!({ "version": "1.0.0", "range": "^1 || ^2" }),
        "E_UNSUPPORTED",
    );
    err("color_convert", json!({ "color": "blurple" }), "E_INVALID_INPUT");
    err("color_convert", json!({ "color": "hwb(1 2 3)" }), "E_UNSUPPORTED_FORMAT");
    err(
        "sql_validate",
        json!({ "sql": "SELECT 1", "dialect": "oracle" }),
        "E_INVALID_INPUT",
    );
    assert_eq!(ok("sql_validate", json!({ "sql": "SELEC 1" }))["valid"], false);
    err("sql_erd", json!({ "sql": "SELECT 1" }), "E_INVALID_INPUT");
    err("graphql_format", json!({ "text": "{" }), "E_MALFORMED");
    err("graphql_schema", json!({ "text": "type" }), "E_MALFORMED");
    err("ts_to_js", json!({ "source": "let a: number = 1" }), "E_UNSUPPORTED");
    err("validate_compose", json!({ "text": "[1,2" }), "E_MALFORMED");
    err("validate_compose", json!({ "text": "- a" }), "E_INVALID_INPUT");
    err("gitignore", json!({ "stacks": ["zzz"] }), "E_NOT_FOUND");
    err("env_parse", json!({ "text": "A=1", "output": "weird" }), "E_INVALID_INPUT");
    err("patch_parse", json!({ "patch": "hello" }), "E_MALFORMED");
    err("chmod", json!({ "mode": "999" }), "E_INVALID_INPUT");
    err("mime", json!({ "query": "zzzzzzz" }), "E_NOT_FOUND");
    err("http_status", json!({ "query": "999" }), "E_INVALID_INPUT");
    err("url_parse", json!({ "url": "http://[::1" }), "E_INVALID_INPUT");
    err("query_build", json!({ "object": [1] }), "E_INVALID_INPUT");
    err("nope", json!({}), "E_BAD_REQUEST");
}

#[test]
fn empty_inputs() {
    err("regex_test", json!({ "pattern": "", "text": "x" }), "E_EMPTY_INPUT");
    err("regex_explain", json!({ "pattern": " " }), "E_EMPTY_INPUT");
    err("cron_parse", json!({ "expression": "" }), "E_EMPTY_INPUT");
    err("semver_check", json!({ "version": "" }), "E_EMPTY_INPUT");
    err("semver_sort", json!({ "versions": [] }), "E_EMPTY_INPUT");
    err("color_convert", json!({ "color": "" }), "E_EMPTY_INPUT");
    err("sql_format", json!({ "sql": "" }), "E_EMPTY_INPUT");
    err("sql_minify", json!({ "sql": "  " }), "E_EMPTY_INPUT");
    err("graphql_format", json!({ "text": "" }), "E_EMPTY_INPUT");
    err("html_format", json!({ "html": "" }), "E_EMPTY_INPUT");
    err("html_minify", json!({ "html": "" }), "E_EMPTY_INPUT");
    err("css_format", json!({ "css": "" }), "E_EMPTY_INPUT");
    err("js_format", json!({ "source": "" }), "E_EMPTY_INPUT");
    err("js_minify", json!({ "source": "" }), "E_EMPTY_INPUT");
    err("lint_markdown", json!({ "text": "" }), "E_EMPTY_INPUT");
    err("lint_dockerfile", json!({ "text": "" }), "E_EMPTY_INPUT");
    err("lint_yaml", json!({ "text": "" }), "E_EMPTY_INPUT");
    err("validate_compose", json!({ "text": "" }), "E_EMPTY_INPUT");
    err("gitignore", json!({}), "E_EMPTY_INPUT");
    err("env_parse", json!({ "text": "" }), "E_EMPTY_INPUT");
    err("diff", json!({ "old": "", "new": "" }), "E_EMPTY_INPUT");
    err("patch_parse", json!({ "patch": "" }), "E_EMPTY_INPUT");
    err("chmod", json!({ "mode": "" }), "E_EMPTY_INPUT");
    err("mime", json!({}), "E_EMPTY_INPUT");
    err("http_status", json!({ "query": "" }), "E_EMPTY_INPUT");
    err("user_agent", json!({ "user_agent": "" }), "E_EMPTY_INPUT");
    err("url_parse", json!({ "url": "" }), "E_EMPTY_INPUT");
    err("query_parse", json!({ "query": "?" }), "E_EMPTY_INPUT");
    err("query_build", json!({ "object": {} }), "E_EMPTY_INPUT");
    // Missing required field → bad request from the deserializer.
    err("regex_test", json!({}), "E_BAD_REQUEST");
}

#[test]
fn code_analysis_actions() {
    let files = json!([
        {
            "name": "a.js",
            "content": "// c\nfunction f(a) {\n  if (a) { return 1; }\n  return 0;\n}\n"
        }
    ]);
    assert_eq!(ok("code_line_count", json!({ "files": files }))["total"]["comment"], 1);
    assert_eq!(
        ok("code_complexity", json!({ "files": files }))["files"][0]["functions"][0]["cyclomatic"],
        2
    );
    assert_eq!(
        ok("code_duplicates", json!({ "files": files, "min_lines": 3 }))["duplicate_blocks"],
        0
    );
    assert!(ok(
        "import_sort",
        json!({ "name": "x.py", "content": "import sys\nimport os\nprint(os, sys)\n" })
    )["sorted"]
        .as_str()
        .unwrap()
        .starts_with("import os\nimport sys"));
    assert_eq!(
        ok("unused_imports", json!({ "name": "x.py", "content": "import os\nprint(1)\n" }))
            ["unused"][0]["name"],
        "os"
    );
    assert_eq!(
        ok(
            "dependency_scan",
            json!({
                "files": [
                    {
                        "name": "package.json",
                        "content": "{\"dependencies\":{\"left-pad\":\"1.0.0\"}}"
                    }
                ],
                "licenses": { "left-pad": "WTFPL" }
            })
        )["count"],
        1
    );
    assert_eq!(
        ok("sql_analyze", json!({ "sql": "SELECT * FROM t" }))["analysis"]["findings"][0]["code"],
        "select-star"
    );
    assert_eq!(
        ok("code_format", json!({ "language": "css", "source": "a{b:c}" }))["formatted"],
        "a {\n  b: c;\n}\n"
    );
    assert_eq!(
        ok("code_minify", json!({ "language": "json", "source": "{ \"a\" : 1 }" }))["minified"],
        "{\"a\":1}"
    );
    err("sandbox", json!({ "tool": "javascript-sandbox" }), "E_UNSUPPORTED");
    err("code_line_count", json!({ "files": [] }), "E_EMPTY_INPUT");
    err("import_sort", json!({ "name": "x.js", "content": "let a = 1;" }), "E_NOT_FOUND");
    err("code_format", json!({ "language": "python", "source": "x=1" }), "E_UNSUPPORTED");
    err(
        "dependency_scan",
        json!({ "files": [{ "name": "README.md", "content": "hi" }] }),
        "E_NOT_FOUND",
    );
}
