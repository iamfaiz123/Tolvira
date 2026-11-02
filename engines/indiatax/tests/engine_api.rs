//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use indiatax_engine::process_request;
use serde_json::{json, Value};

fn call(action: &str, payload: Value) -> Value {
    serde_json::from_slice(&process_request(
        json!({ "action": action, "payload": payload }).to_string().as_bytes(),
    ))
    .unwrap()
}

#[test]
fn protocol() {
    assert_eq!(call("gst", json!({ "amount": 100, "rate": 5 }))["data"]["total"], 105.0);
    assert_eq!(call("gst", json!({ "amount": 0, "rate": 5 }))["error_code"], "E_INVALID_INPUT");
    assert_eq!(
        call("validate_gstin", json!({ "gstin": "27AAPFU0939F1ZV" }))["data"]["valid"],
        true
    );
    assert_eq!(call("validate_gstin", json!({ "gstin": "" }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(
        call("validate_invoice_number", json!({ "number": "A/1" }))["data"]["valid"],
        true
    );
    assert_eq!(
        call(
            "validate_invoice",
            json!({
                "invoice_number": "A/1",
                "date": "2024-01-01",
                "supplier_gstin": "27AAPFU0939F1ZV",
                "place_of_supply": "27",
                "taxable_value": 100,
                "rate": 18,
                "cgst": 9,
                "sgst": 9,
                "hsn": "9983"
            })
        )["data"]["valid"],
        true
    );
    assert_eq!(
        call("hsn_lookup", json!({ "query": "cement" }))["data"]["results"][0]["code"],
        "25"
    );
    assert_eq!(
        call("tds", json!({ "amount": 100000, "section": "194C" }))["data"]["tds"],
        1000.0
    );
    assert_eq!(call("income_tax", json!({ "income": 500000 }))["data"]["total_tax"], 0.0);
    assert_eq!(
        call("income_tax", json!({ "income": 1500000, "compare": true }))["data"]["better"],
        "new"
    );
    assert!(
        call("salary_in_hand", json!({ "ctc": 600000 }))["data"]["in_hand_monthly"]
            .as_f64()
            .unwrap()
            > 40000.0
    );
    assert_eq!(
        call("professional_tax", json!({ "monthly_salary": 50000, "state": "karnataka" }))["data"]
            ["monthly"],
        200.0
    );
    assert_eq!(
        call(
            "capital_gains",
            json!({ "asset": "equity", "buy_price": 100, "sell_price": 150, "holding_months": 24 })
        )["data"]["tax"],
        0.0
    );
    assert_eq!(
        call(
            "bank_statement",
            json!({ "text": "date,description,debit,credit\n01/01/2024,UPI X,10,\n" })
        )["data"]["count"],
        1
    );
    assert_eq!(
        call(
            "validate_gstr",
            json!({ "text": "gstin,invoice_no,taxable_value\n27AAPFU0939F1ZV,INV1,100\n" })
        )["data"]["valid"],
        true
    );
    assert_eq!(
        call(
            "reconcile",
            json!({
                "books": "gstin,invoice_no,total\n27AAPFU0939F1ZV,1,100",
                "portal": "ctin,inum,val\n27AAPFU0939F1ZV,1,100"
            })
        )["data"]["summary"]["matched"],
        1
    );
    assert_eq!(call("rules", json!({}))["data"]["version"], "2024-25.1");
}
