//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use math_engine::process_request;
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
fn valid_inputs() {
    assert_eq!(
        ok("evaluate", json!({ "expression": "sin(90)", "degrees": true }))["result"],
        1.0
    );
    assert_eq!(
        ok("fraction", json!({ "a": "1/2", "op": "+", "b": "1/3" }))["result"]["fraction"],
        "5/6"
    );
    assert_eq!(ok("decimal_to_fraction", json!({ "value": 0.5 }))["fraction"], "1/2");
    assert_eq!(ok("percentage", json!({ "kind": "of", "a": 10, "b": 50 }))["result"], 5.0);
    assert_eq!(ok("ratio", json!({ "values": [4, 6] }))["simplified"], "2:3");
    assert_eq!(ok("statistics", json!({ "text": "1 2 3" }))["mean"], 2.0);
    assert_eq!(ok("frequency", json!({ "values": [1, 1, 2] }))["unique_values"], 2);
    assert_eq!(ok("prime_check", json!({ "number": "7" }))["is_prime"], true);
    assert_eq!(ok("prime_factorization", json!({ "number": "12" }))["expression"], "2^2 × 3");
    assert_eq!(ok("gcd_lcm", json!({ "numbers": ["8", "12"] }))["gcd"], "4");
    assert_eq!(ok("combinatorics", json!({ "n": 4, "r": 2 }))["combinations"], "6");
    assert_eq!(ok("factorial", json!({ "n": 5 }))["factorial"], "120");
    assert_eq!(
        ok("probability", json!({ "kind": "simple", "favorable": 1, "total": 2 }))["probability"],
        0.5
    );
    assert_eq!(ok("quadratic", json!({ "a": 1, "b": 0, "c": -4 }))["roots"], json!([2.0, -2.0]));
    assert_eq!(ok("linear_equation", json!({ "equation": "2x = 8" }))["x"], 4.0);
    assert_eq!(
        ok("linear_system", json!({ "matrix": [[1, 0], [0, 1]], "constants": [3, 4] }))["solution"],
        json!([3.0, 4.0])
    );
    assert!(
        (ok("numeric_root", json!({ "expression": "cos(x) - x", "lo": 0, "hi": 1 }))["root"]
            .as_f64()
            .unwrap()
            - 0.739085)
            .abs()
            < 1e-5
    );
    assert_eq!(
        ok("matrix", json!({ "op": "determinant", "a": [[1, 2], [3, 4]] }))["determinant"],
        -2.0
    );
    assert_eq!(ok("trigonometry", json!({ "angle": 45 }))["exact"]["tan"], "1");
    assert_eq!(ok("triangle", json!({ "sides": [3, 4, 5] }))["area"], 6.0);
    assert_eq!(
        ok("plot", json!({ "expression": "x", "x_min": 0, "x_max": 1, "points": 3 }))["y"][2],
        1.0
    );
    assert!(
        (ok("derivative", json!({ "expression": "x^2", "x": 3 }))["first_derivative"]
            .as_f64()
            .unwrap()
            - 6.0)
            .abs()
            < 1e-5
    );
    assert!(
        (ok("integral", json!({ "expression": "x", "from": 0, "to": 2 }))["integral"]
            .as_f64()
            .unwrap()
            - 2.0)
            .abs()
            < 1e-9
    );
    assert_eq!(ok("limit", json!({ "expression": "(x^2-1)/(x-1)", "x0": 1 }))["limit"], 2.0);
    assert_eq!(
        ok("base_convert", json!({ "value": "255", "from_base": 10, "to_base": 16 }))["result"],
        "FF"
    );
    assert_eq!(
        ok("binary_calc", json!({ "a": "6", "op": "xor", "b": "3", "bits": 8 }))["result"]
            ["decimal"],
        5
    );
    assert_eq!(ok("roman", json!({ "value": "XIV" }))["number"], 14);
    assert_eq!(ok("number_to_words", json!({ "value": 12 }))["words"], "Twelve");
    assert_eq!(
        ok("sequence", json!({ "kind": "squares", "count": 3 }))["terms"],
        json!([1, 4, 9])
    );
    assert_eq!(ok("scientific_notation", json!({ "value": 1500 }))["exponent"], 3);
    assert_eq!(ok("random_numbers", json!({ "min": 5, "max": 5 }))["values"][0], 5);
}

#[test]
fn invalid_and_empty_inputs() {
    assert_eq!(err("evaluate", json!({ "expression": "" })), "E_EMPTY_INPUT");
    assert_eq!(err("evaluate", json!({ "expression": "1/0" })), "E_INVALID_INPUT");
    assert_eq!(err("evaluate", json!({ "expression": "2 +" })), "E_INVALID_INPUT");
    assert_eq!(err("fraction", json!({ "a": "" })), "E_EMPTY_INPUT");
    assert_eq!(err("fraction", json!({ "a": "1/0" })), "E_INVALID_INPUT");
    assert_eq!(err("statistics", json!({})), "E_EMPTY_INPUT");
    assert_eq!(err("prime_check", json!({ "number": "-5" })), "E_INVALID_INPUT");
    assert_eq!(
        err("matrix", json!({ "op": "inverse", "a": [[1, 2], [2, 4]] })),
        "E_INVALID_INPUT"
    );
    assert_eq!(err("matrix", json!({ "op": "determinant", "a": [] })), "E_EMPTY_INPUT");
    assert_eq!(
        err("base_convert", json!({ "value": "", "from_base": 10, "to_base": 2 })),
        "E_EMPTY_INPUT"
    );
    assert_eq!(
        err("base_convert", json!({ "value": "G", "from_base": 16, "to_base": 2 })),
        "E_INVALID_INPUT"
    );
    assert_eq!(err("roman", json!({ "value": "IIII" })), "E_INVALID_INPUT");
    assert_eq!(err("quadratic", json!({ "a": 0, "b": 0, "c": 1 })), "E_INVALID_INPUT");
    assert_eq!(
        err("plot", json!({ "expression": "x", "x_min": 5, "x_max": 1 })),
        "E_INVALID_INPUT"
    );
    assert_eq!(err("sequence", json!({ "kind": "nope" })), "E_INVALID_INPUT");
    assert_eq!(err("evaluate", json!({})), "E_BAD_REQUEST");
}
