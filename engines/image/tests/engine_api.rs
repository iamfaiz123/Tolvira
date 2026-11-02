//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use engine_shared::b64;
use image_engine::process_request;
use image_engine::testutil::{gif_bytes, jpeg_bytes, png_bytes};
use serde_json::{json, Value};

fn call(action: &str, payload: Value) -> Value {
    let req = json!({ "action": action, "payload": payload }).to_string();
    serde_json::from_slice(&process_request(req.as_bytes())).unwrap()
}
fn ok(action: &str, payload: Value) -> Value {
    let r = call(action, payload);
    assert_eq!(r["success"], true, "{action}: {r}");
    r["data"].clone()
}
fn err(action: &str, payload: Value) -> String {
    let r = call(action, payload);
    assert_eq!(r["success"], false, "{action} unexpectedly succeeded: {r}");
    r["error_code"].as_str().unwrap().to_string()
}
fn png() -> String {
    b64::encode(&png_bytes(64, 32))
}

#[test]
fn legacy_actions() {
    assert_eq!(
        ok("aspect_ratio", json!({ "width": 1920, "height": 1080 }))["ratio_str"],
        "16:9"
    );
    let p = ok("preset_crop", json!({ "orig_width": 800, "orig_height": 600, "preset": "1:1" }));
    assert_eq!(p["crop_width"], 600);
    assert_eq!(
        ok(
            "calculate_crop",
            json!({
                "orig_width": 100,
                "orig_height": 100,
                "crop_x": 0,
                "crop_y": 0,
                "crop_width": 50,
                "crop_height": 50
            })
        )["area_percent"],
        25.0
    );
    assert_eq!(ok("grayscale_filter", json!({ "width": 2, "height": 2 }))["pixels"], 4);
    assert_eq!(err("aspect_ratio", json!({ "width": 0, "height": 1 })), "E_INVALID_INPUT");
}

#[test]
fn empty_invalid_malformed() {
    for action in [
        "info",
        "resize",
        "flip",
        "palette",
        "dominant_color",
        "strip_metadata",
        "to_data_url",
        "gif_frames",
        "trim_border",
    ] {
        let mut payload = json!({ "file": "" });
        if action == "resize" {
            payload["width"] = json!(10);
        }
        if action == "flip" {
            payload["direction"] = json!("horizontal");
        }
        assert_eq!(err(action, payload.clone()), "E_EMPTY_INPUT", "{action}");
        payload["file"] = json!(b64::encode(b"definitely not an image"));
        assert_eq!(err(action, payload.clone()), "E_UNSUPPORTED_FORMAT", "{action}");
        payload["file"] = json!(b64::encode(&png_bytes(8, 8)[..30]));
        let code = err(action, payload);
        // A truncated PNG is malformed for raster actions and "not a GIF" for GIF actions.
        assert!(
            code == "E_MALFORMED" || (action == "gif_frames" && code == "E_UNSUPPORTED_FORMAT"),
            "{action}: {code}"
        );
    }
    assert_eq!(err("resize", json!({ "file": png() })), "E_INVALID_INPUT");
    assert_eq!(err("convert", json!({ "file": png(), "format": "avif" })), "E_UNSUPPORTED");
    assert_eq!(err("convert", json!({ "file": png(), "format": "xyz" })), "E_INVALID_INPUT");
    assert_eq!(err("svg_to_raster", json!({ "svg": "" })), "E_EMPTY_INPUT");
    assert_eq!(err("svg_to_raster", json!({ "svg": "<svg" })), "E_MALFORMED");
    assert_eq!(err("sprite", json!({ "files": [] })), "E_EMPTY_INPUT");
    assert_eq!(err("adjust", json!({ "file": png(), "adjustments": [] })), "E_EMPTY_INPUT");
    assert_eq!(
        err("adjust", json!({ "file": png(), "adjustments": [{ "op": "nope" }] })),
        "E_BAD_REQUEST"
    );
    assert_eq!(
        err(
            "resize",
            json!({ "file": png(), "width": 100000, "height": 100000, "fit": "fill" })
        ),
        "E_LIMIT_EXCEEDED"
    );
}

#[test]
fn geometry_and_encoding() {
    let r = ok("resize", json!({ "file": png(), "width": 32, "format": "jpeg", "quality": 70 }));
    assert_eq!(r["output"]["mime"], "image/jpeg");
    assert_eq!(r["output"]["height"], 16);
    let r = ok("crop", json!({ "file": png(), "x": 10, "y": 5, "width": 20, "height": 10 }));
    assert_eq!(r["output"]["width"], 20);
    let r = ok("crop_ratio", json!({ "file": png(), "ratio": "1:1" }));
    assert_eq!(r["output"]["width"], 32);
    let r = ok("rotate", json!({ "file": png(), "angle": 90 }));
    assert_eq!(r["output"]["width"], 32);
    let r = ok("flip", json!({ "file": png(), "direction": "vertical" }));
    assert_eq!(r["output"]["width"], 64);
    for f in ["png", "jpeg", "webp", "gif", "bmp", "tiff"] {
        let r = ok("convert", json!({ "file": png(), "format": f }));
        assert_eq!(r["output"]["extension"], if f == "jpeg" { "jpg" } else { f });
        let back = ok("info", json!({ "file": r["output"]["file"] }));
        assert_eq!(back["width"], 64, "{f}");
    }
    let r = ok(
        "compress",
        json!({ "file": b64::encode(&jpeg_bytes(200, 200)), "target_kb": 2, "max_width": 100 }),
    );
    assert_eq!(r["resized"], true);
    assert_eq!(r["target_met"], true);
    assert!(r["quality"].as_u64().unwrap() <= 80);
}

#[test]
fn filters_framing_text() {
    let r = ok(
        "adjust",
        json!({
            "file": png(),
            "adjustments": [
                { "op": "grayscale" },
                { "op": "blur", "sigma": 1.5 },
                { "op": "brightness", "value": 10 }
            ]
        }),
    );
    assert_eq!(r["steps"], 3);
    assert!(
        ok("pixel_art", json!({ "file": png(), "pixel_size": 4, "colors": 4 }))["palette"]
            .is_array()
    );
    let r = ok("add_border", json!({ "file": png(), "size": 2, "color": "#00ff00" }));
    assert_eq!(r["output"]["width"], 68);
    let t = ok("trim_border", json!({ "file": r["output"]["file"], "tolerance": 0 }));
    assert_eq!(t["rect"]["width"], 64);
    assert_eq!(
        ok("round_corners", json!({ "file": png(), "radius": "8" }))["output"]["mime"],
        "image/png"
    );
    assert_eq!(
        ok(
            "fit_print_size",
            json!({ "file": png(), "width": 2, "height": 2, "unit": "in", "dpi": 72 })
        )["pixels"]["width"],
        144
    );
    assert!(
        ok("watermark_text", json!({ "file": png(), "text": "hello", "font_size": 10 }))
            ["text_size"]["width"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(
        ok(
            "watermark_image",
            json!({ "file": png(), "overlay": b64::encode(&png_bytes(8, 8)), "scale": 0.2 })
        )["overlay_size"]["width"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(
        ok("render_text", json!({ "text": "Caption", "font_size": 20 }))["output"]["width"]
            .as_u64()
            .unwrap()
            > 20
    );
    assert_eq!(
        ok("remove_background", json!({ "file": png(), "tolerance": 0 }))["output"]["mime"],
        "image/png"
    );
}

#[test]
fn metadata_analysis_svg_gif_compose() {
    let info = ok("info", json!({ "file": b64::encode(&jpeg_bytes(10, 5)) }));
    assert_eq!(info["format"], "jpeg");
    assert_eq!(
        ok("set_dpi", json!({ "file": b64::encode(&jpeg_bytes(10, 5)), "dpi": 150 }))["dpi"][0],
        150.0
    );
    assert_eq!(ok("strip_metadata", json!({ "file": png() }))["lossless"], true);
    let du = ok("to_data_url", json!({ "file": png() }));
    assert_eq!(ok("from_data_url", json!({ "data": du["data_url"] }))["info"]["width"], 64);
    assert_eq!(ok("palette", json!({ "file": png(), "count": 3 }))["count"], 3);
    assert!(ok("dominant_color", json!({ "file": png() }))["dominant"]["hex"].is_string());
    assert_eq!(ok("color_at", json!({ "file": png(), "x": 0, "y": 0 }))["rgb"]["b"], 128);
    assert_eq!(ok("to_ascii", json!({ "file": png(), "columns": 16 }))["columns"], 16);
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="red"/></svg>"#;
    assert_eq!(ok("svg_to_raster", json!({ "svg": svg, "scale": 3 }))["output"]["width"], 30);
    assert!(ok("svg_optimize", json!({ "svg": svg }))["svg"].is_string());
    assert_eq!(ok("svg_info", json!({ "svg": svg }))["elements"]["rect"], 1);
    let gif = b64::encode(&gif_bytes(4, 10, 10));
    assert_eq!(ok("gif_frames", json!({ "file": gif }))["total_frames"], 4);
    assert_eq!(ok("gif_resize", json!({ "file": gif, "width": 5 }))["output"]["width"], 5);
    let files = vec![png(), b64::encode(&png_bytes(8, 8))];
    assert_eq!(ok("sprite", json!({ "files": files, "columns": 2 }))["rows"], 1);
    assert_eq!(
        ok(
            "contact_sheet",
            json!({
                "files": files,
                "columns": 1,
                "cell_width": 20,
                "cell_height": 20,
                "padding": 2
            })
        )["rows"],
        2
    );
    assert_eq!(ok("difference", json!({ "file_a": png(), "file_b": png() }))["identical"], true);
    assert_eq!(
        ok("favicon", json!({ "file": png(), "sizes": [16, 32] }))["pngs"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        ok(
            "og_image",
            json!({ "width": 200, "height": 100, "title": "Hi", "title_size": 20, "padding": 10 })
        )["output"]["width"],
        200
    );
}
