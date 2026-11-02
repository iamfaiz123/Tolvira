//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use mesh3d_engine::{cube, process_request, write_stl_ascii, write_stl_binary};
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
fn err(action: &str, payload: Value) -> String {
    let v = call(action, payload);
    assert_eq!(v["success"], false, "{action}: {v}");
    v["error_code"].as_str().unwrap_or_default().to_string()
}
fn stl() -> String {
    engine_shared::b64::encode(&write_stl_binary(&cube(10.0), "cube"))
}

#[test]
fn valid_inputs() {
    let i = ok("inspect", json!({ "file": stl(), "name": "cube.stl" }));
    assert_eq!(i["triangles"], 12);
    assert_eq!(i["volume"], 1000.0);
    assert_eq!(i["watertight"], true);
    let ascii = engine_shared::b64::encode(write_stl_ascii(&cube(2.0), "c").as_bytes());
    assert_eq!(ok("inspect", json!({ "file": ascii }))["format"], "stl-ascii");
    let o = ok("convert", json!({ "file": stl(), "name": "cube.stl", "to": "obj" }));
    assert_eq!(o["filename"], "cube.obj");
    let back = ok("convert", json!({ "file": o["file"], "name": "cube.obj", "to": "stl" }));
    assert_eq!(back["triangles"], 12);
    let t = ok("transform", json!({ "file": stl(), "scale": [0.5, 0.5, 0.5] }));
    assert_eq!(t["volume"], 125.0);
    let r = ok("repair", json!({ "file": stl() }));
    assert_eq!(r["watertight"], true);
    let th = ok("thumbnail", json!({ "file": stl(), "width": 48, "height": 48 }));
    assert_eq!(th["mime"], "image/png");
    let gltf = engine_shared::b64::encode(
        json!({ "asset": { "version": "2.0" }, "meshes": [] })
            .to_string()
            .as_bytes(),
    );
    assert_eq!(ok("gltf_inspect", json!({ "file": gltf }))["format"], "gltf");
    assert_eq!(ok("gltf_optimize", json!({ "file": gltf }))["mime"], "model/gltf-binary");
}

#[test]
fn invalid_inputs() {
    let junk = engine_shared::b64::encode(b"definitely not a mesh");
    assert_eq!(err("inspect", json!({ "file": junk })), "E_UNSUPPORTED_FORMAT");
    let mut trunc = write_stl_binary(&cube(1.0), "c");
    trunc.truncate(300);
    assert_eq!(
        err(
            "inspect",
            json!({ "file": engine_shared::b64::encode(&trunc), "name": "c.stl" })
        ),
        "E_MALFORMED"
    );
    let bad_obj = engine_shared::b64::encode(b"v 0 0 0\nv 1 0 0\nf 1 2 7\n");
    assert_eq!(err("convert", json!({ "file": bad_obj, "name": "x.obj" })), "E_MALFORMED");
    assert_eq!(err("convert", json!({ "file": stl(), "to": "fbx" })), "E_INVALID_INPUT");
    assert_eq!(
        err("transform", json!({ "file": stl(), "scale": [0, 1, 1] })),
        "E_INVALID_INPUT"
    );
    assert_eq!(
        err("thumbnail", json!({ "file": stl(), "background": "red" })),
        "E_INVALID_INPUT"
    );
    assert_eq!(err("glb_to_gltf", json!({ "file": stl() })), "E_UNSUPPORTED_FORMAT");
    assert_eq!(err("nope", json!({})), "E_BAD_REQUEST");
}

#[test]
fn empty_inputs() {
    for action in [
        "inspect",
        "convert",
        "transform",
        "repair",
        "thumbnail",
        "gltf_inspect",
        "glb_to_gltf",
        "gltf_optimize",
    ] {
        assert_eq!(err(action, json!({})), "E_EMPTY_INPUT", "{action}");
        assert_eq!(err(action, json!({ "file": "" })), "E_EMPTY_INPUT", "{action}");
    }
}
