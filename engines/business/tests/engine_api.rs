//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use business_engine::process_request;
use serde_json::{json, Value};

fn call(action: &str, payload: Value) -> Value {
    serde_json::from_slice(&process_request(
        json!({ "action": action, "payload": payload }).to_string().as_bytes(),
    ))
    .unwrap()
}

#[test]
fn protocol() {
    let inv = call(
        "document",
        json!({
            "kind": "invoice",
            "number": "INV-1",
            "from": { "name": "A" },
            "to": { "name": "B" },
            "items": [{ "description": "Widget", "quantity": 2, "unit_price": 100 }],
            "tax_percent": 18
        }),
    );
    assert_eq!(inv["success"], true, "{inv}");
    assert_eq!(inv["data"]["totals"]["subtotal"], 200.0);
    assert!(inv["data"]["html"].as_str().unwrap().contains("Widget"));
    assert_eq!(call("document", json!({ "items": [] }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(
        call("profit_loss", json!({ "revenue": 1000, "cogs": 400 }))["data"]["gross_profit"],
        600.0
    );
    assert_eq!(
        call("markup_margin", json!({ "cost": 100, "price": 150 }))["data"]["markup_percent"],
        50.0
    );
    assert!(call(
        "inventory_turnover",
        json!({ "cogs": 1200, "opening_inventory": 200, "closing_inventory": 400 })
    )["data"]["turnover_ratio"]
        .is_number());
    assert!(call(
        "reorder",
        json!({
            "annual_demand": 1200,
            "ordering_cost": 50,
            "holding_cost_per_unit": 2,
            "lead_time_days": 7
        })
    )["data"]["eoq"]
        .is_number());
    assert!(call(
        "discount_pricing",
        json!({ "list_price": 100, "quantity": 10, "coupon_percent": 10 })
    )["data"]["total"]
        .is_number());
    assert!(
        call("sales_tax", json!({ "amount": 100, "rates": [5, 2.5] }))["data"]["total"]
            .as_f64()
            .unwrap()
            > 100.0
    );
    assert!(
        call("commission", json!({ "sales": 10000, "rate_percent": 5 }))["data"]["commission"]
            .is_number()
    );
    assert!(call(
        "payroll",
        json!({ "employees": [{ "name": "x", "hours": 160, "hourly_rate": 20 }] })
    )["data"]["totals"]["gross"]
        .is_number());
    assert!(call(
        "cash_flow",
        json!({
            "opening_balance": 1000,
            "months": 6,
            "recurring_inflow": 500,
            "recurring_outflow": 700
        })
    )["data"]["closing_balance"]
        .is_number());
    assert_eq!(
        call(
            "break_even",
            json!({ "fixed_costs": 1000, "price_per_unit": 10, "variable_cost_per_unit": 5 })
        )["data"]["break_even_units"],
        200.0
    );
    assert_eq!(
        call(
            "break_even",
            json!({ "fixed_costs": 1000, "price_per_unit": 5, "variable_cost_per_unit": 5 })
        )["error_code"],
        "E_INVALID_INPUT"
    );
    assert!(
        call("valuation", json!({ "revenue": 1000000, "revenue_multiple": 3 }))["data"].is_object()
    );
    assert!(call(
        "unit_economics",
        json!({
            "price": 50,
            "variable_cost": 20,
            "cac": 100,
            "purchases_per_year": 4,
            "retention_years": 3
        })
    )["data"]["ltv"]
        .is_number());
    assert_eq!(call("payroll", json!({ "employees": [] }))["success"], false);
    assert_eq!(call("nope", json!({}))["error_code"], "E_BAD_REQUEST");
}
