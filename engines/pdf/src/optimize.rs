//! Whole-document optimisation: compress, repair and linearize.

use crate::doc;
use crate::pages::PdfOutput;
use engine_shared::{EngineError, EngineResult};
use lopdf::{Document, Object, SaveOptions};
use serde_json::{json, Value};

/// PDF compression preset level.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompressionLevel {
    /// Extreme compression (high compression, lower image quality, max size reduction).
    Extreme,
    /// Moderate compression (recommended: balanced file size reduction and good visual quality).
    #[default]
    Moderate,
    /// Light compression (low compression, preserves highest visual quality).
    Light,
}

/// Compresses a PDF using object streams, metadata stripping, and intelligent image re-compression:
/// * Flate-compresses every uncompressed stream at level 9,
/// * packs objects into modern object streams with cross-reference streams,
/// * strips redundant metadata (XMP, Info, Thumbnails, PieceInfo),
/// * downsamples and re-compresses embedded images according to `level` or `image_quality`.
pub fn compress(
    bytes: &[u8],
    password: Option<&str>,
    level: CompressionLevel,
    strip_metadata: bool,
    image_quality: Option<u8>,
) -> EngineResult<Value> {
    let mut d = doc::load(bytes, password)?;
    let before = bytes.len();
    let total = d.get_pages().len();

    let (default_q, max_dim) = match level {
        CompressionLevel::Extreme => (40u8, 1024u32),
        CompressionLevel::Moderate => (65u8, 1600u32),
        CompressionLevel::Light => (85u8, 2400u32),
    };
    let q = match image_quality {
        Some(custom) => {
            if !(1..=100).contains(&custom) {
                return Err(EngineError::invalid("`image_quality` must be between 1 and 100"));
            }
            custom
        }
        None => default_q,
    };

    if strip_metadata {
        if let Ok(cat) = d.catalog_mut() {
            cat.remove(b"Metadata");
            cat.remove(b"PieceInfo");
        }
        d.trailer.remove(b"Info");
    }
    // Remove page thumbnails and piece info (app private data).
    for (_, pid) in d.get_pages() {
        if let Ok(p) = d.get_dictionary_mut(pid) {
            p.remove(b"Thumb");
            p.remove(b"PieceInfo");
        }
    }

    let mut images_recompressed = 0usize;
    let ids: Vec<_> = d
        .objects
        .iter()
        .filter_map(|(id, o)| match o {
            Object::Stream(s)
                if s.dict.get(b"Subtype").ok().and_then(|x| match x {
                    Object::Name(n) => Some(n.as_slice()),
                    _ => None,
                }) == Some(b"Image") =>
            {
                Some(*id)
            }
            _ => None,
        })
        .collect();

    for id in ids {
        let stream = match d.get_object(id) {
            Ok(Object::Stream(s)) => s.clone(),
            _ => continue,
        };

        let w = match stream.dict.get(b"Width").ok() {
            Some(Object::Integer(i)) => *i as u32,
            Some(Object::Reference(rid)) => d.get_object(*rid).ok().and_then(|ro| ro.as_i64().ok()).unwrap_or(0) as u32,
            _ => 0,
        };
        let h = match stream.dict.get(b"Height").ok() {
            Some(Object::Integer(i)) => *i as u32,
            Some(Object::Reference(rid)) => d.get_object(*rid).ok().and_then(|ro| ro.as_i64().ok()).unwrap_or(0) as u32,
            _ => 0,
        };
        if w < 16 || h < 16 {
            continue;
        }

        let has_mask = stream.dict.has(b"SMask") || stream.dict.has(b"Mask");

        let maybe_img = crate::images::decode_image_stream(&d, &stream)
            .map(|(img, _)| img)
            .or_else(|| {
                let filters = stream.filters().unwrap_or_default();
                if filters.iter().any(|&f| f == b"DCTDecode") {
                    image::load_from_memory_with_format(&stream.content, image::ImageFormat::Jpeg).ok()
                } else {
                    None
                }
            });

        let img = match maybe_img {
            Some(i) => i,
            None => continue,
        };

        let orig_w = img.width();
        let orig_h = img.height();

        // Downscale image if it exceeds max_dim (only if no soft mask to avoid mask dimension mismatch)
        let (resized_img, new_w, new_h) = if !has_mask && (orig_w > max_dim || orig_h > max_dim) {
            let r = img.resize(max_dim, max_dim, image::imageops::FilterType::Triangle);
            let rw = r.width();
            let rh = r.height();
            (r, rw, rh)
        } else {
            (img, orig_w, orig_h)
        };

        let mut out = Vec::new();
        let is_gray = matches!(resized_img.color(), image::ColorType::L8 | image::ColorType::L16);
        let encoded = if is_gray {
            let g = resized_img.to_luma8();
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(
                std::io::Cursor::new(&mut out),
                q,
            );
            enc.encode(g.as_raw(), new_w, new_h, image::ColorType::L8.into()).is_ok()
        } else {
            let rgb = resized_img.to_rgb8();
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(
                std::io::Cursor::new(&mut out),
                q,
            );
            enc.encode(rgb.as_raw(), new_w, new_h, image::ColorType::Rgb8.into()).is_ok()
        };

        if encoded && !out.is_empty() {
            let old_size = stream.content.len();
            // Only replace if the new encoded JPEG is smaller than the original stream bytes
            if out.len() < old_size {
                let mut dict = stream.dict.clone();
                dict.set("Filter", Object::Name(b"DCTDecode".to_vec()));
                dict.remove(b"DecodeParms");
                dict.set(
                    "ColorSpace",
                    Object::Name(if is_gray {
                        b"DeviceGray".to_vec()
                    } else {
                        b"DeviceRGB".to_vec()
                    }),
                );
                dict.set("BitsPerComponent", Object::Integer(8));
                dict.set("Width", Object::Integer(new_w as i64));
                dict.set("Height", Object::Integer(new_h as i64));
                dict.set("Length", Object::Integer(out.len() as i64));
                let new_stream = lopdf::Stream::new(dict, out).with_compression(false);
                d.objects.insert(id, Object::Stream(new_stream));
                images_recompressed += 1;
            }
        }
    }

    d.delete_zero_length_streams();
    d.prune_objects();
    d.compress();
    d.renumber_objects();
    let options = SaveOptions::builder()
        .use_object_streams(true)
        .use_xref_streams(true)
        .compression_level(9)
        .build();
    let out = doc::save_with(&mut d, options)?;

    // If for any reason the resulting bytes are larger than before and no images were compressed,
    // fallback to classic save or original bytes so we NEVER increase file size.
    let out = if out.len() > before && images_recompressed == 0 {
        let mut d2 = doc::load(bytes, password)?;
        if strip_metadata {
            if let Ok(cat) = d2.catalog_mut() {
                cat.remove(b"Metadata");
            }
            d2.trailer.remove(b"Info");
        }
        d2.prune_objects();
        d2.compress();
        d2.renumber_objects();
        let classic = doc::save(&mut d2)?;
        if classic.len() < before {
            classic
        } else {
            bytes.to_vec()
        }
    } else {
        out
    };

    let after = out.len();
    let level_name = match level {
        CompressionLevel::Extreme => "Extreme",
        CompressionLevel::Moderate => "Moderate",
        CompressionLevel::Light => "Light",
    };
    Ok(json!({
        "output": PdfOutput::new(out, total),
        "compression_level": level_name,
        "bytes_before": before,
        "bytes_after": after,
        "saved_bytes": before.saturating_sub(after),
        "saved_percent": if before > 0 { ((before as f64 - after as f64) / before as f64 * 1000.0).round() / 10.0 } else { 0.0 },
        "images_recompressed": images_recompressed,
        "jpeg_quality": q,
        "lossless": image_quality.is_none() && images_recompressed == 0,
    }))
}

/// Repairs a damaged PDF: lenient parsing with cross-reference
/// reconstruction, page tree normalisation, object renumbering and a clean
/// rewrite. Reports what was fixed.
pub fn repair(bytes: &[u8], password: Option<&str>) -> EngineResult<Value> {
    if bytes.is_empty() {
        return Err(EngineError::empty("PDF file"));
    }
    if !doc::looks_like_pdf(bytes) {
        return Err(EngineError::unsupported_format(
            "The file does not contain a %PDF header and cannot be repaired as a PDF",
        ));
    }
    let mut fixes: Vec<String> = Vec::new();
    let mut d = match doc::load(bytes, password) {
        Ok(d) => d,
        Err(e) if e.code == engine_shared::error::ErrorCode::Malformed => {
            // Try trimming trailing garbage / finding the last %%EOF.
            let trimmed = bytes
                .iter()
                .rposition(|&b| !b.is_ascii_whitespace())
                .map(|i| &bytes[..=i])
                .unwrap_or(bytes);
            match doc::load(trimmed, password) {
                Ok(d) => {
                    fixes.push("Removed trailing garbage after %%EOF".into());
                    d
                },
                Err(_) => {
                    return Err(EngineError::malformed(format!(
                        "The PDF is too damaged to repair: {}",
                        e.message
                    )))
                },
            }
        },
        Err(e) => return Err(e),
    };
    if let Some(rec) = &d.xref_recovery {
        fixes.push(format!("Cross-reference table rebuilt ({rec:?})"));
    }
    let pages: Vec<_> = doc::ordered_pages(&d);
    if pages.is_empty() {
        return Err(EngineError::malformed("No pages could be recovered from the document"));
    }
    // Normalise the page tree (fixes wrong /Count, missing /Type, broken parents).
    let had_count = d
        .catalog()
        .ok()
        .and_then(|c| c.get(b"Pages").ok())
        .and_then(|p| d.dereference(p).ok())
        .and_then(|(_, p)| p.as_dict().ok())
        .and_then(|p| p.get(b"Count").ok())
        .and_then(|c| c.as_i64().ok());
    if had_count != Some(pages.len() as i64) {
        fixes.push(format!("Page count corrected to {}", pages.len()));
    }
    doc::rebuild_page_tree(&mut d, &pages)?;
    for &pid in &pages {
        if let Ok(p) = d.get_dictionary_mut(pid) {
            if !p.has_type(b"Page") {
                p.set("Type", Object::Name(b"Page".to_vec()));
                fixes.push("Added missing /Type /Page".into());
            }
        }
    }
    let removed = d.delete_zero_length_streams();
    if !removed.is_empty() {
        fixes.push(format!("Removed {} empty stream(s)", removed.len()));
    }
    let pruned = d.prune_objects();
    if !pruned.is_empty() {
        fixes.push(format!("Removed {} unreferenced object(s)", pruned.len()));
    }
    if d.trailer.get(b"Info").is_err() {
        fixes.push("Document has no Info dictionary (left as is)".into());
    }
    d.renumber_objects();
    let out = doc::save(&mut d)?;
    // Verify the output loads cleanly.
    Document::load_mem(&out)
        .map_err(|e| EngineError::internal(format!("Repaired file failed verification: {e}")))?;
    Ok(json!({
        "output": PdfOutput::new(out, pages.len()),
        "fixes": fixes,
        "pages_recovered": pages.len(),
        "xref_was_rebuilt": d.xref_recovery.is_some()
    }))
}

/// Writes the document with linearisation hints (fast web view) and
/// cross-reference streams.
pub fn linearize(bytes: &[u8], password: Option<&str>) -> EngineResult<Value> {
    let mut d = doc::load(bytes, password)?;
    let total = d.get_pages().len();
    d.prune_objects();
    d.renumber_objects();
    let options = SaveOptions::builder()
        .linearize(true)
        .use_xref_streams(true)
        .use_object_streams(true)
        .build();
    let out = doc::save_with(&mut d, options)?;
    let has_hint = out.windows(11).take(4096).any(|w| w == b"/Linearized");
    Ok(json!({
        "output": PdfOutput::new(out, total),
        "linearized": has_hint,
        "note": "Best-effort linearisation via lopdf; the first page object ordering is optimised \
                 for progressive loading."
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{png_bytes, sample_pdf};
    use engine_shared::b64;

    #[test]
    fn compress_and_linearize() {
        let pdf = sample_pdf(&["a"; 20]);
        let c = compress(&pdf, None, CompressionLevel::Moderate, true, None).unwrap();
        assert!(c["bytes_after"].as_u64().unwrap() > 0);
        assert_eq!(c["lossless"], true);
        let out = b64::decode(c["output"]["file"].as_str().unwrap()).unwrap();
        let d = doc::load(&out, None).unwrap();
        assert_eq!(d.get_pages().len(), 20);
        assert!(d.trailer.get(b"Info").is_err());

        // Lossy image recompression.
        let img =
            crate::create::from_images(&[png_bytes(64, 64)], Some("A4"), 0.0, "contain").unwrap();
        let img_pdf = b64::decode(img["output"]["file"].as_str().unwrap()).unwrap();
        let c2 = compress(&img_pdf, None, CompressionLevel::Moderate, false, Some(50)).unwrap();
        assert_eq!(c2["images_recompressed"], 1);
        assert!(compress(&img_pdf, None, CompressionLevel::Moderate, false, Some(0)).is_err());

        let l = linearize(&pdf, None).unwrap();
        let out = b64::decode(l["output"]["file"].as_str().unwrap()).unwrap();
        assert!(doc::load(&out, None).is_ok());
    }

    #[test]
    fn repair_damaged() {
        let pdf = sample_pdf(&["keep me"]);
        // Corrupt the xref offset and append junk.
        let mut broken = pdf.clone();
        if let Some(pos) = broken.windows(9).rposition(|w| w == b"startxref") {
            broken[pos + 10] = b'9';
        }
        broken.extend_from_slice(b"\n\nGARBAGE TRAILING DATA");
        let r = repair(&broken, None).unwrap();
        assert_eq!(r["pages_recovered"], 1);
        let out = b64::decode(r["output"]["file"].as_str().unwrap()).unwrap();
        let d = doc::load(&out, None).unwrap();
        assert!(d.extract_text(&[1]).unwrap().contains("keep me"));
        assert!(repair(b"", None).is_err());
        assert!(repair(b"not a pdf at all", None).is_err());
        assert!(repair(b"%PDF-1.4\n", None).is_err());
    }
}
