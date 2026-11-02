//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use datetime_engine::process_request;
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
fn protocol() {
    assert_eq!(ok("parse", json!({ "date": "2024-02-29" }))["leap_year"], true);
    assert_eq!(
        ok("difference", json!({ "from": "2024-01-01", "to": "2024-01-31" }))["total"]["days"],
        30
    );
    assert_eq!(
        ok("add", json!({ "date": "2024-01-01", "days": 10 }))["result"]["date"],
        "2024-01-11"
    );
    assert_eq!(
        ok("add", json!({ "date": "2024-01-11", "days": 10, "subtract": true }))["result"]["date"],
        "2024-01-01"
    );
    assert_eq!(
        ok("age", json!({ "birth_date": "2000-01-01", "now": "2024-01-01" }))["years"],
        24
    );
    assert_eq!(
        ok(
            "convert_zone",
            json!({ "datetime": "2024-01-01T12:00:00Z", "to": ["Asia/Tokyo"] })
        )["results"][0]["time"],
        "21:00"
    );
    assert!(ok("zones", json!({ "filter": "europe" }))["count"].as_u64().unwrap() > 10);
    assert_eq!(ok("timestamp", json!({ "input": "0" }))["iso_utc"], "1970-01-01T00:00:00Z");
    assert_eq!(
        ok("business_days", json!({ "start": "2024-06-03", "end": "2024-06-10" }))["business_days"],
        5
    );
    assert_eq!(ok("week_number", json!({ "date": "2024-06-05" }))["iso_week"], 23);
    assert_eq!(
        ok("countdown", json!({ "target": "2025-01-01", "now": "2024-12-31T00:00:00Z" }))
            ["remaining"]["days"],
        1
    );
    assert_eq!(ok("calendar", json!({ "year": 2024, "month": 1 }))["months"][0]["days"], 31);
    assert_eq!(
        ok(
            "meeting_planner",
            json!({ "date": "2024-06-10", "zones": ["UTC", "Europe/Berlin"] })
        )["has_common_slot"],
        true
    );
    assert_eq!(
        ok(
            "cron_next",
            json!({ "expression": "30 8 * * *", "now": "2024-06-07T12:00:00Z" })
        )["next_runs"][0]["utc"],
        "2024-06-08T08:30:00Z"
    );
    assert_eq!(ok("pomodoro", json!({ "start": "2024-01-01T09:00:00" }))["focus_minutes"], 100);
    assert_eq!(
        ok("format", json!({ "date": "2024-01-31", "pattern": "%d/%m/%Y" }))["formatted"],
        "31/01/2024"
    );
    assert_eq!(err("parse", json!({ "date": "" })), "E_EMPTY_INPUT");
    assert_eq!(err("parse", json!({ "date": "tomorrow-ish" })), "E_INVALID_INPUT");
    assert_eq!(
        err("cron_next", json!({ "expression": "", "now": "2024-06-07T12:00:00Z" })),
        "E_EMPTY_INPUT"
    );
    assert_eq!(
        err("age", json!({ "birth_date": "2030-01-01", "now": "2024-01-01" })),
        "E_INVALID_INPUT"
    );
    assert_eq!(err("calendar", json!({ "year": 0 })), "E_INVALID_INPUT");
}
