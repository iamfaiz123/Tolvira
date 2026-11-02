//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{json, Value};
use solar_engine::process_request;

fn call(action: &str, payload: Value) -> Value {
    serde_json::from_slice(&process_request(
        json!({ "action": action, "payload": payload }).to_string().as_bytes(),
    ))
    .unwrap()
}

#[test]
fn protocol() {
    // valid
    let p = call("panel_size", json!({ "monthly_kwh": 300 }));
    assert_eq!(p["success"], true, "{p}");
    assert!(p["data"]["panels"].as_f64().unwrap() >= 1.0);
    let r = call("roi", json!({ "system_kw": 3, "cost": 150000, "tariff_per_kwh": 8 }));
    assert!(r["data"]["payback_years"].as_f64().unwrap() > 0.0);
    assert!(call("battery", json!({ "daily_kwh": 10, "backup_hours": 8 }))["data"]
        ["required_capacity_kwh"]
        .is_number());
    assert!(call(
        "inverter",
        json!({
            "appliances": [{ "name": "fridge", "watts": 200, "quantity": 1, "hours_per_day": 24 }]
        })
    )["data"]["recommended_va"]
        .is_number());
    assert!(
        call(
            "bill",
            json!({
                "units_kwh": 250,
                "slabs": [{ "upto": 100, "rate": 3 }, { "upto": 200, "rate": 5 }, { "rate": 7 }]
            })
        )["data"]["total"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert!(
        call(
            "appliances",
            json!({
                "appliances": [{ "name": "tv", "watts": 100, "hours_per_day": 4 }],
                "tariff_per_kwh": 8
            })
        )["data"]["monthly_kwh"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert!(
        call("carbon", json!({ "electricity_kwh_month": 300 }))["data"]["yearly_kg_co2e"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    // invalid
    assert_eq!(
        call("roi", json!({ "system_kw": 0, "cost": 1, "tariff_per_kwh": 1 }))["error_code"],
        "E_INVALID_INPUT"
    );
    // empty
    assert_eq!(call("inverter", json!({ "appliances": [] }))["success"], false);
    assert_eq!(call("panel_size", json!({}))["success"], false);
    // unknown action
    assert_eq!(call("nope", json!({}))["error_code"], "E_BAD_REQUEST");
}
