//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use finance_engine::process_request;
use serde_json::{json, Value};

fn call(action: &str, payload: Value) -> Value {
    serde_json::from_slice(&process_request(
        json!({ "action": action, "payload": payload }).to_string().as_bytes(),
    ))
    .unwrap()
}

#[test]
fn protocol() {
    let l = call("loan", json!({ "principal": 250000, "annual_rate": 7.5, "years": 20 }));
    assert_eq!(l["success"], true);
    assert_eq!(l["data"]["payment"], 2013.98);
    assert_eq!(
        call("loan", json!({ "principal": -5, "annual_rate": 7.5, "years": 20 }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call(
            "mortgage",
            json!({ "price": 300000, "down_payment_percent": 20, "annual_rate": 6, "years": 30 })
        )["data"]["loan_amount"],
        240000.0
    );
    assert_eq!(
        call(
            "interest",
            json!({ "principal": 1000, "annual_rate": 10, "years": 2, "compounding": "yearly" })
        )["data"]["compound"]["total"],
        1210.0
    );
    assert_eq!(
        call("investment", json!({ "monthly": 1000, "annual_return": 12, "years": 1 }))["data"]
            ["invested"],
        12000.0
    );
    assert_eq!(
        call(
            "fixed_deposit",
            json!({ "principal": 1000, "annual_rate": 8, "years": 1, "compounding": "yearly" })
        )["data"]["maturity_amount"],
        1080.0
    );
    assert_eq!(
        call("recurring_deposit", json!({ "monthly": 500, "annual_rate": 5, "months": 6 }))["data"]
            ["total_deposited"],
        3000.0
    );
    assert!(
        call("ppf", json!({ "yearly_deposit": 10000, "annual_rate": 7 }))["data"]
            ["maturity_amount"]
            .as_f64()
            .unwrap()
            > 250000.0
    );
    assert_eq!(
        call("inflation", json!({ "amount": 100, "annual_rate": 10, "years": 1 }))["data"]
            ["result"],
        110.0
    );
    assert_eq!(
        call("cagr", json!({ "start_value": 100, "end_value": 121, "years": 2 }))["data"]
            ["cagr_percent"],
        10.0
    );
    assert_eq!(call("roi", json!({ "cost": 100, "gain": 150 }))["data"]["roi_percent"], 50.0);
    assert_eq!(
        call("npv", json!({ "rate_percent": 0, "cash_flows": [-100, 60, 60] }))["data"]["npv"],
        20.0
    );
    assert!(
        call("irr", json!({ "cash_flows": [-100, 60, 60] }))["data"]["irr_percent"]
            .as_f64()
            .unwrap()
            > 13.0
    );
    assert_eq!(
        call("discount", json!({ "price": 100, "discount_percent": 10 }))["data"]["final_price"],
        90.0
    );
    assert_eq!(
        call("markup", json!({ "cost": 100, "markup_percent": 50 }))["data"]["price"],
        150.0
    );
    assert_eq!(
        call("margin", json!({ "revenue": 200, "cost": 150 }))["data"]["gross_margin_percent"],
        25.0
    );
    assert_eq!(
        call(
            "break_even",
            json!({ "fixed_costs": 1000, "price_per_unit": 20, "variable_cost_per_unit": 10 })
        )["data"]["break_even_units"],
        100.0
    );
    assert_eq!(call("salary", json!({ "gross": 120000 }))["data"]["net"]["monthly"], 10000.0);
    assert_eq!(call("hourly", json!({ "amount": 50 }))["data"]["yearly"], 104000.0);
    assert_eq!(
        call("salary_hike", json!({ "current": 100, "hike_percent": 5 }))["data"]["new"],
        105.0
    );
    assert_eq!(
        call(
            "overtime",
            json!({ "hourly_rate": 10, "regular_hours": 40, "overtime_hours": 2 })
        )["data"]["total_pay"],
        430.0
    );
    assert!(
        call(
            "freelance_rate",
            json!({ "target_income": 50000, "billable_hours_per_week": 20 })
        )["data"]["hourly_rate"]
            .as_f64()
            .unwrap()
            > 40.0
    );
    assert_eq!(
        call(
            "currency",
            json!({ "amount": 10, "from": "USD", "to": "EUR", "rates": { "EUR": 0.5 } })
        )["data"]["result"],
        5.0
    );
    assert_eq!(
        call("currency", json!({ "amount": 10, "from": "USD", "to": "EUR" }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("tip", json!({ "bill": 50, "tip_percent": 20, "people": 2 }))["data"]["per_person"],
        30.0
    );
    assert_eq!(
        call(
            "tax_brackets",
            json!({ "income": 100, "brackets": [{ "from": 0, "rate_percent": 10 }] })
        )["data"]["total_tax"],
        10.0
    );
    assert_eq!(
        call("tax_brackets", json!({ "income": 100, "brackets": [] }))["error_code"],
        "E_INVALID_INPUT"
    );
}
