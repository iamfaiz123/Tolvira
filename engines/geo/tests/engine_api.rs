//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use geo_engine::process_request;
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

const FC: &str = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"name":"A"},"geometry":{"type":"Point","coordinates":[77.2090,28.6139]}},{"type":"Feature","properties":{"name":"B"},"geometry":{"type":"Polygon","coordinates":[[[77.1,28.5],[77.3,28.5],[77.3,28.7],[77.1,28.7],[77.1,28.5]]]}}]}"#;

#[test]
fn valid_inputs() {
    let f = ok("format", json!({ "text": FC, "precision": 3, "add_bbox": true }));
    assert!(f["text"].as_str().unwrap().contains("\"bbox\""));
    assert_eq!(f["summary"]["features"], 2);
    assert_eq!(ok("validate", json!({ "text": FC }))["valid"], true);
    assert_eq!(
        ok("validate", json!({ "text": "{\"type\":\"Point\",\"coordinates\":[999,0]}" }))["valid"],
        false
    );
    let csv = ok("to_csv", json!({ "text": FC }));
    assert!(csv["csv"].as_str().unwrap().starts_with("name,geometry_type,wkt\n"));
    let back = ok("from_csv", json!({ "text": "name,lat,lon\nX,28.6,77.2\nY,bad,1\n" }));
    assert_eq!(back["features"], 1);
    assert_eq!(back["skipped"].as_array().unwrap().len(), 1);
    let kml = ok("to_kml", json!({ "text": FC, "name": "Delhi" }));
    assert!(kml["kml"].as_str().unwrap().contains("<Placemark>"));
    assert_eq!(ok("from_kml", json!({ "text": kml["kml"] }))["features"], 2);
    assert_eq!(ok("wkt_parse", json!({ "wkt": "LINESTRING (0 0, 1 1)" }))["type"], "LineString");
    assert!(ok("wkt_from_geojson", json!({ "text": FC }))["wkt"]
        .as_str()
        .unwrap()
        .starts_with("POINT (77.209 28.6139)"));
    let c = ok("convert_coordinate", json!({ "coordinate": "28°36'50\"N 77°12'32\"E" }));
    assert_eq!(c["utm"]["zone"], 43);
    assert!(c["geohash"].as_str().unwrap().starts_with("ttn"));
    let d = ok(
        "distance",
        json!({ "from": "28.6139, 77.2090", "to": "19.0760, 72.8777", "ellipsoid": true }),
    );
    assert!((d["distance"]["kilometers"].as_f64().unwrap() - 1153.0).abs() < 10.0, "{d}");
    assert_eq!(d["compass"], "SSW");
    let a = ok(
        "area",
        json!({ "coordinates": [[28.5, 77.1], [28.5, 77.3], [28.7, 77.3], [28.7, 77.1]] }),
    );
    assert!((a["area"]["square_kilometers"].as_f64().unwrap() - 433.0).abs() < 10.0, "{a}");
    assert_eq!(ok("area", json!({ "text": FC }))["vertices"], 4);
    let s = ok("simplify", json!({ "text": FC, "tolerance_m": 50 }));
    assert!(s["coordinates_after"].as_u64().unwrap() <= s["coordinates_before"].as_u64().unwrap());
    assert_eq!(ok("bbox", json!({ "text": FC }))["north"], 28.7);
    assert_eq!(
        ok("geohash", json!({ "coordinate": "57.64911, 10.40744", "precision": 7 }))["geohash"],
        "u4pruyd"
    );
    assert!(
        (ok("geohash", json!({ "hash": "u4pruyd" }))["latitude"]
            .as_f64()
            .unwrap()
            - 57.649)
            .abs()
            < 0.01
    );
    let u = ok("utm", json!({ "coordinate": "40.7128, -74.0060" }));
    assert_eq!(u["zone"], 18);
    assert_eq!(u["epsg"], 32618);
    let back = ok(
        "utm",
        json!({ "zone": 18, "hemisphere": "N", "easting": 583959.4, "northing": 4507351.0 }),
    );
    assert!((back["latitude"].as_f64().unwrap() - 40.7128).abs() < 1e-4);
    let dest = ok("destination", json!({ "from": "0,0", "bearing": 90, "distance_m": 111320 }));
    assert!((dest["destination"]["longitude"].as_f64().unwrap() - 1.0).abs() < 0.01);
}

#[test]
fn invalid_inputs() {
    assert_eq!(call("format", json!({ "text": "{not json" }))["error_code"], "E_MALFORMED");
    assert_eq!(
        call("format", json!({ "text": "{\"type\":\"Blob\"}" }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("wkt_parse", json!({ "wkt": "TRIANGLE (0 0)" }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(call("wkt_parse", json!({ "wkt": "POINT (1 2" }))["error_code"], "E_MALFORMED");
    assert_eq!(
        call("convert_coordinate", json!({ "coordinate": "hello world" }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("distance", json!({ "from": "0,0", "to": "91,0" }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("area", json!({ "coordinates": [[0, 0], [1, 1]] }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(call("geohash", json!({ "hash": "aaa" }))["error_code"], "E_INVALID_INPUT");
    assert_eq!(
        call("utm", json!({ "zone": 99, "easting": 500000, "northing": 0 }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("from_kml", json!({ "text": "<notkml/>" }))["error_code"],
        "E_UNSUPPORTED_FORMAT"
    );
    assert_eq!(
        call("from_csv", json!({ "text": "a,b\n1,2\n" }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("simplify", json!({ "text": FC, "tolerance": -1 }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(call("nope", json!({}))["error_code"], "E_BAD_REQUEST");
}

#[test]
fn empty_inputs() {
    assert_eq!(call("format", json!({}))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("validate", json!({ "text": "" }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("wkt_parse", json!({ "wkt": " " }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(
        call("convert_coordinate", json!({ "coordinate": "" }))["error_code"],
        "E_EMPTY_INPUT"
    );
    assert_eq!(call("geohash", json!({}))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("utm", json!({}))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("from_kml", json!({ "text": "" }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("area", json!({}))["error_code"], "E_EMPTY_INPUT");
}
