//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use construction_engine::process_request;
use serde_json::{json, Value};

fn call(action: &str, payload: Value) -> Value {
    serde_json::from_slice(&process_request(
        json!({ "action": action, "payload": payload }).to_string().as_bytes(),
    ))
    .unwrap()
}

#[test]
fn protocol() {
    assert_eq!(
        call("concrete", json!({ "length": 1, "width": 1, "thickness": 1 }))["data"]
            ["wet_volume_m3"],
        1.0
    );
    assert_eq!(
        call("concrete", json!({ "length": 0, "width": 1, "thickness": 1 }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert!(
        call("masonry", json!({ "wall_length": 5, "wall_height": 3, "wall_thickness": 0.23 }))
            ["data"]["units_to_buy"]
            .as_f64()
            .unwrap()
            > 1000.0
    );
    assert!(
        call("plaster", json!({ "area": 50 }))["data"]["cement_bags_50kg"]
            .as_f64()
            .unwrap()
            >= 5.0
    );
    assert_eq!(call("paint", json!({ "area": 100 }))["data"]["paint_litres"], 20.0);
    assert!(
        call(
            "tiles",
            json!({ "area_length": 4, "area_width": 3, "tile_length": 300, "tile_width": 300 })
        )["data"]["tiles_to_buy"]
            .as_f64()
            .unwrap()
            > 130.0
    );
    assert!(
        call("roofing", json!({ "length": 10, "width": 6, "pitch_degrees": 20 }))["data"]
            ["roof_area_m2"]
            .as_f64()
            .unwrap()
            > 60.0
    );
    assert!(
        call("steel", json!({ "shape": "rebar", "length": 1, "diameter_mm": 16 }))["data"]
            ["weight_per_metre_kg"]
            .as_f64()
            .unwrap()
            > 1.5
    );
    assert!(
        call("rebar", json!({ "slab_length": 5, "slab_width": 5, "diameter_mm": 10 }))["data"]
            ["weight_kg"]
            .as_f64()
            .unwrap()
            > 50.0
    );
    assert_eq!(
        call("excavation", json!({ "length": 2, "width": 2, "depth": 1 }))["data"]
            ["bank_volume_m3"],
        4.0
    );
    assert_eq!(
        call("wall_area", json!({ "walls": [{ "length": 4, "height": 3 }] }))["data"]
            ["net_wall_area_m2"],
        12.0
    );
    assert_eq!(call("staircase", json!({ "total_rise": 3 }))["data"]["risers"], 17.0);
    assert_eq!(
        call("tank", json!({ "shape": "rectangular", "length": 1, "width": 1, "height": 1 }))
            ["data"]["capacity_litres"],
        1000.0
    );
    assert!(
        call("cost_estimate", json!({ "built_area": 1000 }))["data"]["total_cost"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert_eq!(
        call("unit", json!({ "value": 1, "from": "sqft", "to": "m2" }))["data"]["result"],
        0.09290304
    );
    assert_eq!(
        call("tank", json!({ "shape": "rectangular", "length": 1, "width": 1, "height": 0 }))
            ["error_code"],
        "E_INVALID_INPUT"
    );
}
