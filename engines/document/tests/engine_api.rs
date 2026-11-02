//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use document_engine::process_request;
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

#[test]
fn valid_inputs() {
    let docx = engine_shared::b64::encode(include_bytes!("fixtures/sample.docx"));
    let i = ok("inspect", json!({ "file": docx, "name": "sample.docx" }));
    assert_eq!(i["format"], "docx");
    assert!(i["stats"]["words"].as_u64().unwrap() > 10, "{i}");
    let t = ok("to_text", json!({ "file": docx }));
    assert!(t["text"].as_str().unwrap().len() > 50);
    assert!(ok("to_html", json!({ "file": docx }))["html"]
        .as_str()
        .unwrap()
        .starts_with("<!DOCTYPE html>"));
    assert!(ok("to_markdown", json!({ "file": docx }))["markdown"].is_string());
    assert!(ok("to_json", json!({ "file": docx }))["blocks"].is_array());
    let rtf = String::from_utf8_lossy(include_bytes!("fixtures/sample.rtf")).to_string();
    let r = ok("to_text", json!({ "text": rtf }));
    assert_eq!(r["format"], "rtf");
    assert!(r["stats"]["words"].as_u64().unwrap() > 50, "{r}");
    let made = ok(
        "from_markdown",
        json!({ "markdown": "# Offer\n\nDear {{name}},\n\nTotal: **{{total}}**\n", "name": "offer" }),
    );
    assert_eq!(made["filename"], "offer.docx");
    let filled = ok(
        "fill_template",
        json!({ "file": made["file"], "values": { "name": "Ada", "total": "€42" } }),
    );
    assert_eq!(filled["report"]["replaced"]["name"], 1);
    assert!(ok("to_text", json!({ "file": filled["file"] }))["text"]
        .as_str()
        .unwrap()
        .contains("Dear Ada,"));
    let conv = ok("convert", json!({ "file": docx, "to": "markdown" }));
    assert_eq!(conv["from_format"], "docx");
    assert!(ok("convert", json!({ "text": "# Hi\n\nthere" }))["filename"]
        .as_str()
        .unwrap()
        .ends_with(".docx"));
}

#[test]
fn invalid_inputs() {
    assert_eq!(
        call(
            "inspect",
            json!({ "file": engine_shared::b64::encode(b"plain text, not a package") })
        )["error_code"],
        "E_UNSUPPORTED_FORMAT"
    );
    assert_eq!(
        call(
            "inspect",
            json!({ "file": engine_shared::b64::encode(b"PK\x03\x04broken zip data here") })
        )["error_code"],
        "E_MALFORMED"
    );
    assert_eq!(
        call(
            "inspect",
            json!({ "file": engine_shared::b64::encode(&[0xD0, 0xCF, 0x11, 0xE0, 0, 0, 0, 0]) })
        )["error_code"],
        "E_UNSUPPORTED_FORMAT"
    );
    let made = ok("from_markdown", json!({ "markdown": "x" }));
    assert_eq!(
        call("convert", json!({ "file": made["file"], "to": "pdf" }))["error_code"],
        "E_UNSUPPORTED"
    );
    assert_eq!(
        call("convert", json!({ "file": made["file"], "to": "wpd" }))["error_code"],
        "E_UNSUPPORTED_FORMAT"
    );
    assert_eq!(
        call("pptx_slides", json!({ "file": made["file"] }))["error_code"],
        "E_UNSUPPORTED_FORMAT"
    );
    assert_eq!(call("render", json!({ "tool": "docx-to-pdf" }))["error_code"], "E_UNSUPPORTED");
    assert_eq!(call("nope", json!({}))["error_code"], "E_BAD_REQUEST");
}

#[test]
fn empty_inputs() {
    assert_eq!(call("inspect", json!({}))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(
        call("to_text", json!({ "file": "", "text": "  " }))["error_code"],
        "E_EMPTY_INPUT"
    );
    assert_eq!(call("from_markdown", json!({ "markdown": "" }))["error_code"], "E_EMPTY_INPUT");
    let made = ok("from_markdown", json!({ "markdown": "x" }));
    assert_eq!(
        call("fill_template", json!({ "file": made["file"], "values": {} }))["error_code"],
        "E_EMPTY_INPUT"
    );
    assert_eq!(
        call("fill_template", json!({ "values": { "a": 1 } }))["error_code"],
        "E_EMPTY_INPUT"
    );
    assert_eq!(call("pptx_slides", json!({}))["error_code"], "E_EMPTY_INPUT");
}

#[test]
fn ebook_actions() {
    // Build a minimal EPUB through the same helper the unit tests use (re-implemented here).
    use std::io::Write;
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    w.start_file("mimetype", stored).unwrap();
    w.write_all(b"application/epub+zip").unwrap();
    for (n, c) in [
        (
            "META-INF/container.xml",
            r#"<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#,
        ),
        (
            "content.opf",
            r#"<package xmlns="http://www.idpf.org/2007/opf" version="2.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>API Book</dc:title><dc:identifier>x</dc:identifier><dc:language>en</dc:language></metadata><manifest><item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/><item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/></manifest><spine toc="ncx"><itemref idref="c1"/></spine></package>"#,
        ),
        (
            "toc.ncx",
            r#"<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/"><navMap><navPoint id="n1"><navLabel><text>Start</text></navLabel><content src="c1.xhtml"/></navPoint></navMap></ncx>"#,
        ),
        ("c1.xhtml", "<html><body><h1>Start</h1><p>Body text.</p></body></html>"),
    ] {
        w.start_file(n, deflated).unwrap();
        w.write_all(c.as_bytes()).unwrap();
    }
    let epub = engine_shared::b64::encode(&w.finish().unwrap().into_inner());
    let i = ok("epub_inspect", json!({ "file": epub }));
    assert_eq!(i["metadata"]["title"], "API Book");
    assert_eq!(i["toc"][0]["label"], "Start");
    assert!(ok("epub_chapter", json!({ "file": epub, "index": 0 }))["text"]
        .as_str()
        .unwrap()
        .contains("Body text."));
    assert!(ok("epub_to_html", json!({ "file": epub }))["html"]
        .as_str()
        .unwrap()
        .contains("<h1>Start</h1>"));
    assert!(ok("epub_convert", json!({ "file": epub, "to": "docx" }))["filename"]
        .as_str()
        .unwrap()
        .ends_with(".docx"));
    assert_eq!(ok("epub_validate", json!({ "file": epub }))["valid"], true);
    let edited = ok("epub_set_metadata", json!({ "file": epub, "values": { "author": "Grace" } }));
    assert_eq!(
        ok("epub_inspect", json!({ "file": edited["file"] }))["metadata"]["author"],
        "Grace"
    );
    assert_eq!(call("epub_cover", json!({ "file": epub }))["error_code"], "E_NOT_FOUND");
    assert_eq!(
        call("epub_convert", json!({ "file": epub, "to": "mobi" }))["error_code"],
        "E_UNSUPPORTED"
    );
    assert_eq!(
        call(
            "epub_inspect",
            json!({ "file": engine_shared::b64::encode(b"not an epub at all") })
        )["error_code"],
        "E_UNSUPPORTED_FORMAT"
    );
    assert_eq!(
        call("mobi_metadata", json!({ "file": engine_shared::b64::encode(&[0u8; 120]) }))
            ["error_code"],
        "E_UNSUPPORTED_FORMAT"
    );
    assert_eq!(call("epub_inspect", json!({}))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(
        call("epub_set_metadata", json!({ "file": epub, "values": {} }))["error_code"],
        "E_EMPTY_INPUT"
    );
}
