//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use colordesign_engine::process_request;
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
fn png() -> String {
    let img = image::RgbaImage::from_pixel(64, 64, image::Rgba([200, 30, 30, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    engine_shared::b64::encode(&out.into_inner())
}

#[test]
fn valid_inputs() {
    assert_eq!(ok("convert", json!({ "color": "rgb(51,102,153)" }))["hex"], "#336699");
    assert_eq!(ok("convert", json!({ "color": "hsl(210,50%,40%)" }))["rgb"]["r"], 51);
    assert_eq!(
        ok("palette", json!({ "color": "#336699", "scheme": "analogous", "count": 5 }))["colors"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
    assert!(ok(
        "gradient",
        json!({ "stops": [{ "color": "#000" }, { "color": "#fff" }], "kind": "radial" })
    )["css_value"]
        .as_str()
        .unwrap()
        .starts_with("radial-gradient"));
    assert_eq!(
        ok("contrast", json!({ "foreground": "#000", "background": "#fff" }))["grade"],
        "AAA"
    );
    assert!(ok(
        "accessible_pairs",
        json!({ "colors": ["#fff", "#000", "#888"], "level": "AAA" })
    )["passing_pairs"]
        .is_array());
    assert!(
        ok("simulate_color_blindness", json!({ "colors": ["#f00", "#0f0"] }))["simulations"]
            .as_array()
            .unwrap()
            .len()
            == 8
    );
    assert_eq!(
        ok("image_palette", json!({ "image": png(), "count": 3 }))["colors"][0]["nearest_name"],
        "firebrick"
    );
    assert!(ok("shadow", json!({ "preset": "soft" }))["layers"] == 5);
    assert_eq!(ok("border_radius", json!({ "all": 9999 }))["tailwind"], "rounded-full");
    assert_eq!(ok("css_units", json!({ "value": 1, "from": "in" }))["px"], 96.0);
    assert_eq!(
        ok(
            "spacing_scale",
            json!({ "base": 8, "ratio": 1.5, "steps": 5, "names": "numeric" })
        )["steps"][0]["name"],
        "0"
    );
    assert_eq!(
        ok("type_scale", json!({ "base": 18, "ratio": 1.25 }))["ratio_name"],
        "major third"
    );
    assert_eq!(
        ok("favicon_preview", json!({ "image": png() }))["previews"]
            .as_array()
            .unwrap()
            .len(),
        7
    );
    assert_eq!(
        ok("social_dimensions", json!({ "width": 1080, "height": 1080 }))["perfect_for"][0],
        "Instagram square"
    );
    assert_eq!(ok("social_dimensions", json!({ "image": png() }))["width"], 64);
}

#[test]
fn invalid_inputs() {
    assert_eq!(call("convert", json!({ "color": "bogus" }))["error_code"], "E_INVALID_INPUT");
    assert_eq!(
        call("palette", json!({ "color": "#fff", "scheme": "zzz" }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("gradient", json!({ "stops": [{ "color": "#fff" }] }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call(
            "gradient",
            json!({ "stops": [{ "color": "#fff" }, { "color": "#000" }], "kind": "spiral" })
        )["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("accessible_pairs", json!({ "colors": ["#fff"] }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("accessible_pairs", json!({ "colors": ["#fff", "#000"], "level": "ZZ" }))
            ["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("simulate_color_blindness", json!({ "colors": ["#fff"], "types": ["nope"] }))
            ["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("image_palette", json!({ "image": engine_shared::b64::encode(b"nope") }))
            ["error_code"],
        "E_UNSUPPORTED_FORMAT"
    );
    assert_eq!(call("shadow", json!({ "preset": "weird" }))["error_code"], "E_INVALID_INPUT");
    assert_eq!(
        call("border_radius", json!({ "top_left": -5 }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("css_units", json!({ "value": 1, "from": "furlong" }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(call("spacing_scale", json!({ "base": 0 }))["error_code"], "E_INVALID_INPUT");
    assert_eq!(call("type_scale", json!({ "ratio": "weird" }))["error_code"], "E_INVALID_INPUT");
    assert_eq!(
        call("social_dimensions", json!({ "width": 0, "height": 10 }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(call("nope", json!({}))["error_code"], "E_BAD_REQUEST");
}

#[test]
fn empty_inputs() {
    assert_eq!(call("convert", json!({ "color": "" }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("palette", json!({ "color": " " }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("simulate_color_blindness", json!({}))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("image_palette", json!({ "image": "" }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("favicon_preview", json!({ "image": "" }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(
        call("contrast", json!({ "foreground": "", "background": "#fff" }))["error_code"],
        "E_EMPTY_INPUT"
    );
    // No dimensions and no image → the full platform table (valid, informational).
    assert!(ok("social_dimensions", json!({}))["platforms"].is_array());
}
