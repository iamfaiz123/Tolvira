//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Protocol level tests: every request goes through the same
//! `{action, payload}` JSON envelope the browser worker uses.

use engine_shared::b64;
use pdf_engine::process_request;
use pdf_engine::testutil::{jpeg_bytes, png_bytes, sample_pdf};
use serde_json::{json, Value};

fn call(action: &str, payload: Value) -> Value {
    let req = json!({ "action": action, "payload": payload }).to_string();
    serde_json::from_slice(&process_request(req.as_bytes())).unwrap()
}

fn ok(action: &str, payload: Value) -> Value {
    let r = call(action, payload);
    assert_eq!(r["success"], true, "{action}: {}", r);
    r["data"].clone()
}

fn err(action: &str, payload: Value) -> String {
    let r = call(action, payload);
    assert_eq!(r["success"], false, "{action} unexpectedly succeeded: {}", r);
    assert!(r["error"].as_str().map(|s| !s.is_empty()).unwrap_or(false));
    r["error_code"].as_str().unwrap().to_string()
}

fn pdf_b64(pages: &[&str]) -> String {
    b64::encode(&sample_pdf(pages))
}

#[test]
fn envelope_errors() {
    let r: Value = serde_json::from_slice(&process_request(b"")).unwrap();
    assert_eq!(r["error_code"], "E_BAD_REQUEST");
    let r: Value =
        serde_json::from_slice(&process_request(b"{\"action\":\"nope\",\"payload\":{}}")).unwrap();
    assert_eq!(r["error_code"], "E_BAD_REQUEST");
    // Missing required field.
    assert_eq!(err("rotate", json!({ "file": pdf_b64(&["a"]) })), "E_BAD_REQUEST");
    // Invalid base64.
    assert_eq!(err("metadata", json!({ "file": "@@@" })), "E_BAD_REQUEST");
}

#[test]
fn empty_and_invalid_inputs() {
    for action in [
        "metadata",
        "page_sizes",
        "extract_text",
        "compress",
        "repair",
        "grayscale",
        "outlines",
        "form_fields",
        "signatures",
        "permissions",
        "linearize",
        "to_html",
        "to_docx",
        "to_markdown",
        "extract_images",
    ] {
        assert_eq!(err(action, json!({ "file": "" })), "E_EMPTY_INPUT", "{action}");
        let code = err(action, json!({ "file": b64::encode(b"this is not a pdf") }));
        assert!(code == "E_UNSUPPORTED_FORMAT", "{action}: {code}");
        let code = err(action, json!({ "file": b64::encode(b"%PDF-1.7\n1 0 obj << /Broken") }));
        assert!(code == "E_MALFORMED" || code == "E_INTERNAL", "{action}: {code}");
    }
    assert_eq!(err("merge", json!({ "files": [] })), "E_EMPTY_INPUT");
    assert_eq!(err("from_images", json!({ "images": [] })), "E_EMPTY_INPUT");
    assert_eq!(err("from_text", json!({ "text": "   " })), "E_EMPTY_INPUT");
    assert_eq!(err("from_markdown", json!({ "markdown": "" })), "E_EMPTY_INPUT");
    assert_eq!(err("from_html", json!({ "html": "<script>x</script>" })), "E_INVALID_INPUT");
    assert_eq!(
        err("from_images", json!({ "images": [b64::encode(b"junk")] })),
        "E_UNSUPPORTED_FORMAT"
    );
    assert_eq!(
        err("rotate", json!({ "file": pdf_b64(&["a"]), "angle": 45 })),
        "E_INVALID_INPUT"
    );
    assert_eq!(
        err("rotate", json!({ "file": pdf_b64(&["a"]), "angle": 90, "pages": [5] })),
        "E_NOT_FOUND"
    );
    assert_eq!(
        err("extract_pages", json!({ "file": pdf_b64(&["a"]), "pages": "2-" })),
        "E_NOT_FOUND"
    );
    assert_eq!(
        err("delete_pages", json!({ "file": pdf_b64(&["a"]), "pages": "1" })),
        "E_INVALID_INPUT"
    );
    assert_eq!(err("to_images", json!({})), "E_UNSUPPORTED");
    assert_eq!(err("to_svg", json!({})), "E_UNSUPPORTED");
}

#[test]
fn page_operations() {
    let a = pdf_b64(&["A1", "A2", "A3"]);
    let b = pdf_b64(&["B1"]);
    let merged = ok("merge", json!({ "files": [a, b] }));
    assert_eq!(merged["output"]["pages"], 4);
    assert_eq!(merged["output"]["mime"], "application/pdf");
    let merged_file = merged["output"]["file"].as_str().unwrap().to_string();

    let split = ok("split", json!({ "file": merged_file, "mode": "every_n", "n": 3 }));
    assert_eq!(split["files"].as_array().unwrap().len(), 2);
    assert_eq!(split["files"][1]["pages"], json!([4]));

    let rotated = ok("rotate", json!({ "file": merged_file, "angle": 90, "pages": "1-2" }));
    let sizes = ok("page_sizes", json!({ "file": rotated["output"]["file"] }));
    assert_eq!(sizes["pages"][0]["rotation"], 90);
    assert_eq!(sizes["pages"][0]["orientation"], "landscape");
    assert_eq!(sizes["pages"][2]["rotation"], 0);

    let cropped = ok(
        "crop",
        json!({ "file": merged_file, "top": 10, "bottom": 10, "left": 5, "right": 5, "unit": "mm" }),
    );
    let sizes = ok("page_sizes", json!({ "file": cropped["output"]["file"] }));
    assert!((sizes["pages"][0]["width_mm"].as_f64().unwrap() - 200.0).abs() < 0.5);

    let reordered = ok("reorder", json!({ "file": merged_file, "order": [4, 1] }));
    assert_eq!(reordered["order"], json!([4, 1, 2, 3]));
    let text = ok("extract_text", json!({ "file": reordered["output"]["file"], "pages": [1] }));
    assert!(text["text"].as_str().unwrap().contains("B1"));

    let deleted = ok("delete_pages", json!({ "file": merged_file, "pages": "2,3" }));
    assert_eq!(deleted["remaining"], 2);
    let extracted = ok("extract_pages", json!({ "file": merged_file, "pages": [3] }));
    assert_eq!(extracted["output"]["pages"], 1);
    let dup = ok("duplicate_pages", json!({ "file": merged_file, "pages": [1], "copies": 1 }));
    assert_eq!(dup["output"]["pages"], 5);
    let resized = ok("resize_pages", json!({ "file": merged_file, "page_size": "Letter" }));
    let sizes = ok("page_sizes", json!({ "file": resized["output"]["file"] }));
    assert_eq!(sizes["pages"][0]["named_size"], "Letter");
}

#[test]
fn decoration_and_editing() {
    let f = pdf_b64(&["one", "two"]);
    let numbered = ok(
        "add_page_numbers",
        json!({ "file": f, "format": "{n}/{total}", "position": "bottom-right" }),
    );
    let t = ok("extract_text", json!({ "file": numbered["output"]["file"] }));
    assert!(t["text"].as_str().unwrap().contains("2/2"));

    let hf = ok(
        "add_header_footer",
        json!({ "file": f, "header": "ACME", "footer": "p{page}", "align": "left", "font_size": 8 }),
    );
    assert_eq!(hf["output"]["pages"], 2);
    let wm = ok(
        "add_watermark",
        json!({ "file": f, "text": "DRAFT", "opacity": 0.3, "rotation": 30, "color": "#336699" }),
    );
    assert!(ok("extract_text", json!({ "file": wm["output"]["file"] }))["text"]
        .as_str()
        .unwrap()
        .contains("DRAFT"));
    let stamp = ok(
        "stamp",
        json!({
            "file": f,
            "image": b64::encode(&jpeg_bytes(40, 20)),
            "width": 80,
            "position": "top-left"
        }),
    );
    assert_eq!(stamp["stamp"]["height_pt"], 40.0);
    let ov = ok(
        "overlay",
        json!({ "file": f, "overlay_file": pdf_b64(&["OVERLAY"]), "opacity": 0.5 }),
    );
    assert_eq!(ov["pages"], json!([1, 2]));
    let ann = ok(
        "annotate",
        json!({
            "file": f,
            "annotations": [
                { "page": 1, "kind": "highlight", "rect": [60, 690, 300, 730] },
                {
                    "page": 2,
                    "kind": "link",
                    "rect": [0, 0, 100, 100],
                    "url": "https://example.com"
                }
            ]
        }),
    );
    assert_eq!(ann["added"], 2);
    let red = ok(
        "redact",
        json!({ "file": pdf_b64(&["Account 4242 closed"]), "terms": ["4242"] }),
    );
    assert!(!ok("extract_text", json!({ "file": red["output"]["file"] }))["text"]
        .as_str()
        .unwrap()
        .contains("4242"));
    let gray = ok("grayscale", json!({ "file": f }));
    assert!(gray["colour_operators_converted"].as_u64().unwrap() >= 2);
}

#[test]
fn security_flow() {
    let f = pdf_b64(&["secret"]);
    let entropy = b64::encode(&[9u8; 32]);
    let prot = ok(
        "protect",
        json!({
            "file": f,
            "user_password": "u",
            "owner_password": "o",
            "permissions": { "print": false },
            "entropy": entropy
        }),
    );
    assert_eq!(prot["secure_entropy"], true);
    assert_eq!(prot["algorithm"], "aes256");
    let enc = prot["output"]["file"].clone();
    assert_eq!(err("extract_text", json!({ "file": enc })), "E_ENCRYPTED");
    assert_eq!(
        err("extract_text", json!({ "file": enc, "password": "wrong" })),
        "E_WRONG_PASSWORD"
    );
    let t = ok("extract_text", json!({ "file": enc, "password": "u" }));
    assert!(t["text"].as_str().unwrap().contains("secret"));
    let perms = ok("permissions", json!({ "file": enc, "password": "o" }));
    assert_eq!(perms["encrypted"], true);
    assert_eq!(perms["method"], "AES-256");
    assert_eq!(perms["permissions"]["print"], false);
    assert_eq!(perms["password_check"]["owner_password_ok"], true);
    let unlocked = ok("unlock", json!({ "file": enc, "password": "u" }));
    assert_eq!(
        ok("permissions", json!({ "file": unlocked["output"]["file"] }))["encrypted"],
        false
    );
    assert_eq!(err("unlock", json!({ "file": enc, "password": "bad" })), "E_WRONG_PASSWORD");
    assert_eq!(err("unlock", json!({ "file": f })), "E_INVALID_INPUT");
    // Empty user password: file opens without password but is encrypted.
    let owner_only =
        ok("protect", json!({ "file": f, "owner_password": "o", "algorithm": "aes128" }));
    let perms = ok("permissions", json!({ "file": owner_only["output"]["file"] }));
    assert_eq!(perms["encrypted"], true);
    assert_eq!(perms["user_password_required"], false);
    assert!(ok("extract_text", json!({ "file": owner_only["output"]["file"] }))["text"]
        .as_str()
        .unwrap()
        .contains("secret"));
}

#[test]
fn metadata_forms_and_structure() {
    let f = pdf_b64(&["x", "y"]);
    let m = ok("metadata", json!({ "file": f }));
    assert_eq!(m["pages"], 2);
    assert_eq!(m["info"]["title"], "Sample");
    let edited = ok(
        "set_metadata",
        json!({
            "file": f,
            "title": "New",
            "author": "Me",
            "custom": { "Dept": "R&D" },
            "remove": ["Producer"]
        }),
    );
    assert_eq!(edited["info"]["title"], "New");
    assert_eq!(edited["custom_info"]["Dept"], "R&D");
    let bm = ok(
        "set_bookmarks",
        json!({
            "file": f,
            "bookmarks": [
                { "title": "Start", "page": 1, "children": [{ "title": "End", "page": 2 }] }
            ]
        }),
    );
    let o = ok("outlines", json!({ "file": bm["output"]["file"] }));
    assert_eq!(o["count"], 2);
    assert_eq!(o["outlines"][0]["children"][0]["title"], "End");
    let ff = ok("form_fields", json!({ "file": b64::encode(&pdf_engine::testutil::form_pdf()) }));
    assert_eq!(ff["count"], 3);
    let filled = ok(
        "fill_form",
        json!({
            "file": b64::encode(&pdf_engine::testutil::form_pdf()),
            "values": { "name": "Bob", "agree": "yes" }
        }),
    );
    assert_eq!(filled["filled"].as_array().unwrap().len(), 2);
    let sig = ok("signatures", json!({ "file": f }));
    assert_eq!(sig["signed"], false);
}

#[test]
fn conversions() {
    let from_text = ok(
        "from_text",
        json!({ "text": "Hello\n\nWorld", "page_size": "A5", "page_numbers": true, "title": "Doc" }),
    );
    assert_eq!(from_text["output"]["pages"], 1);
    let t = ok("extract_text", json!({ "file": from_text["output"]["file"] }));
    assert!(t["text"].as_str().unwrap().contains("Hello"));
    let md = ok(
        "from_markdown",
        json!({ "markdown": "# Title\n\n- a\n- b\n\n```\ncode\n```", "font": "times" }),
    );
    assert!(ok("extract_text", json!({ "file": md["output"]["file"] }))["text"]
        .as_str()
        .unwrap()
        .contains("Title"));
    let html = ok("from_html", json!({ "html": "<h1>Head</h1><p>Body &amp; soul</p>" }));
    assert!(ok("extract_text", json!({ "file": html["output"]["file"] }))["text"]
        .as_str()
        .unwrap()
        .contains("Body & soul"));
    let imgs = ok(
        "from_images",
        json!({
            "images": [b64::encode(&png_bytes(30, 20)), b64::encode(&jpeg_bytes(20, 30))],
            "page_size": "A4",
            "margin": 20
        }),
    );
    assert_eq!(imgs["output"]["pages"], 2);
    let extracted = ok("extract_images", json!({ "file": imgs["output"]["file"] }));
    assert_eq!(extracted["count"], 2);
    assert_eq!(extracted["images"][1]["mime"], "image/jpeg");
    let h = ok("to_html", json!({ "file": pdf_b64(&["para"]) }));
    assert!(h["html"].as_str().unwrap().contains("<p>para</p>"));
    let d = ok("to_docx", json!({ "file": pdf_b64(&["para"]) }));
    assert!(d["output"]["bytes"].as_u64().unwrap() > 100);
    let mdo = ok("to_markdown", json!({ "file": pdf_b64(&["Heading"]) }));
    assert!(mdo["markdown"].as_str().unwrap().contains("Heading"));
    let cmp = ok(
        "compare",
        json!({ "file_a": pdf_b64(&["same", "a"]), "file_b": pdf_b64(&["same", "b"]) }),
    );
    assert_eq!(cmp["identical"], false);
    assert_eq!(cmp["pages"][0]["identical_text"], true);
    let c = ok("compress", json!({ "file": pdf_b64(&["a", "b", "c"]), "strip_metadata": true }));
    assert!(c["bytes_after"].as_u64().unwrap() > 0);
    let r = ok("repair", json!({ "file": pdf_b64(&["fix"]) }));
    assert_eq!(r["pages_recovered"], 1);
    let l = ok("linearize", json!({ "file": pdf_b64(&["fast"]) }));
    assert!(l["output"]["bytes"].as_u64().unwrap() > 0);
}

#[test]
fn legacy_actions_still_work() {
    let legacy = ok("inspect", json!({ "bytes_len": 90000, "header": "%PDF-1.4" }));
    assert_eq!(legacy["valid"], true);
    assert_eq!(legacy["version"], "1.4");
    assert_eq!(legacy["estimated_pages"], 2);
    let modern = ok("inspect", json!({ "file": pdf_b64(&["a", "b"]) }));
    assert_eq!(modern["pages"], 2);
    assert_eq!(ok("estimate_pages", json!({ "page_count_hint": 7 }))["pages"], 7);
}
