//! Fixtures shared by unit and integration tests (compiled only for tests).
//!
//! Only compiled for tests (`cfg(test)` / `testutil` feature), so test-style
//! `unwrap` is acceptable here.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::doc;
use lopdf::{Dictionary, Object, Stream};

/// Builds a simple A4 document with one page per entry of `page_texts`,
/// each page drawing the text with Helvetica 24pt at (72, 700).
pub fn sample_pdf(page_texts: &[&str]) -> Vec<u8> {
    let (mut d, pages_id) = doc::new_document();
    let font = crate::fonts::StdFont::resolve("helvetica", false, false);
    let font_id = font.add_to_document(&mut d);
    let mut fonts = Dictionary::new();
    fonts.set("F1", Object::Reference(font_id));
    let mut res = Dictionary::new();
    res.set("Font", Object::Dictionary(fonts));
    let res_id = d.add_object(Object::Dictionary(res));
    for text in page_texts {
        let (enc, _) = crate::fonts::encode_winansi(text);
        let content = format!(
            "0.2 0.4 0.8 rg BT /F1 24 Tf 72 700 Td ({}) Tj ET\n",
            String::from_utf8_lossy(&doc::escape_pdf_string(&enc))
        );
        let cid = d.add_object(Stream::new(Dictionary::new(), content.into_bytes()));
        let mut page = Dictionary::new();
        page.set("MediaBox", doc::Rect { x0: 0.0, y0: 0.0, x1: 595.0, y1: 842.0 }.to_object());
        page.set("Contents", Object::Reference(cid));
        page.set("Resources", Object::Reference(res_id));
        doc::append_page(&mut d, pages_id, page);
    }
    let mut info = Dictionary::new();
    info.set("Title", Object::string_literal("Sample"));
    let info_id = d.add_object(Object::Dictionary(info));
    d.trailer.set("Info", Object::Reference(info_id));
    doc::save(&mut d).unwrap()
}

/// A document with an AcroForm containing a text field, a checkbox and a
/// combo box (each with a widget on page 1).
pub fn form_pdf() -> Vec<u8> {
    let (mut d, pages_id) = doc::new_document();
    let page_id = d.new_object_id();

    let mut text_field = Dictionary::new();
    text_field.set("Type", Object::Name(b"Annot".to_vec()));
    text_field.set("Subtype", Object::Name(b"Widget".to_vec()));
    text_field.set("FT", Object::Name(b"Tx".to_vec()));
    text_field.set("T", Object::string_literal("name"));
    text_field.set(
        "Rect",
        doc::Rect {
            x0: 50.0,
            y0: 700.0,
            x1: 300.0,
            y1: 720.0,
        }
        .to_object(),
    );
    text_field.set("P", Object::Reference(page_id));
    text_field.set("MaxLen", Object::Integer(20));
    let tf_id = d.add_object(Object::Dictionary(text_field));

    // Checkbox with an appearance dictionary (/Yes and /Off streams).
    let mut on_dict = Dictionary::new();
    on_dict.set("Type", Object::Name(b"XObject".to_vec()));
    on_dict.set("Subtype", Object::Name(b"Form".to_vec()));
    on_dict.set("BBox", doc::Rect { x0: 0.0, y0: 0.0, x1: 12.0, y1: 12.0 }.to_object());
    let on_id = d.add_object(Stream::new(on_dict.clone(), b"0 g 2 2 8 8 re f".to_vec()));
    let off_id = d.add_object(Stream::new(on_dict, b"".to_vec()));
    let mut n = Dictionary::new();
    n.set("Yes", Object::Reference(on_id));
    n.set("Off", Object::Reference(off_id));
    let mut ap = Dictionary::new();
    ap.set("N", Object::Dictionary(n));
    let mut checkbox = Dictionary::new();
    checkbox.set("Type", Object::Name(b"Annot".to_vec()));
    checkbox.set("Subtype", Object::Name(b"Widget".to_vec()));
    checkbox.set("FT", Object::Name(b"Btn".to_vec()));
    checkbox.set("T", Object::string_literal("agree"));
    checkbox.set("Rect", doc::Rect { x0: 50.0, y0: 650.0, x1: 62.0, y1: 662.0 }.to_object());
    checkbox.set("V", Object::Name(b"Off".to_vec()));
    checkbox.set("AS", Object::Name(b"Off".to_vec()));
    checkbox.set("AP", Object::Dictionary(ap));
    checkbox.set("P", Object::Reference(page_id));
    let cb_id = d.add_object(Object::Dictionary(checkbox));

    let mut combo = Dictionary::new();
    combo.set("Type", Object::Name(b"Annot".to_vec()));
    combo.set("Subtype", Object::Name(b"Widget".to_vec()));
    combo.set("FT", Object::Name(b"Ch".to_vec()));
    combo.set("Ff", Object::Integer(1 << 17));
    combo.set("T", Object::string_literal("colour"));
    combo.set(
        "Opt",
        Object::Array(vec![Object::string_literal("Red"), Object::string_literal("Green")]),
    );
    combo.set(
        "Rect",
        doc::Rect {
            x0: 50.0,
            y0: 600.0,
            x1: 200.0,
            y1: 620.0,
        }
        .to_object(),
    );
    combo.set("P", Object::Reference(page_id));
    let combo_id = d.add_object(Object::Dictionary(combo));

    let mut page = Dictionary::new();
    page.set("Type", Object::Name(b"Page".to_vec()));
    page.set("Parent", Object::Reference(pages_id));
    page.set("MediaBox", doc::Rect { x0: 0.0, y0: 0.0, x1: 595.0, y1: 842.0 }.to_object());
    page.set(
        "Annots",
        Object::Array(vec![
            Object::Reference(tf_id),
            Object::Reference(cb_id),
            Object::Reference(combo_id),
        ]),
    );
    d.objects.insert(page_id, Object::Dictionary(page));
    if let Ok(pages) = d.get_dictionary_mut(pages_id) {
        pages.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
        pages.set("Count", Object::Integer(1));
    }

    let mut acro = Dictionary::new();
    acro.set(
        "Fields",
        Object::Array(vec![
            Object::Reference(tf_id),
            Object::Reference(cb_id),
            Object::Reference(combo_id),
        ]),
    );
    let acro_id = d.add_object(Object::Dictionary(acro));
    d.catalog_mut().unwrap().set("AcroForm", Object::Reference(acro_id));
    doc::save(&mut d).unwrap()
}

/// A small RGB PNG with a gradient.
pub fn png_bytes(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbImage::from_fn(w, h, |x, y| {
        image::Rgb([(x * 255 / w.max(1)) as u8, (y * 255 / h.max(1)) as u8, 128])
    });
    let mut out = Vec::new();
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

/// A small JPEG.
pub fn jpeg_bytes(w: u32, h: u32) -> Vec<u8> {
    let img =
        image::RgbImage::from_fn(w, h, |x, _| image::Rgb([(x * 255 / w.max(1)) as u8, 50, 200]));
    let mut out = Vec::new();
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Jpeg)
        .unwrap();
    out
}
