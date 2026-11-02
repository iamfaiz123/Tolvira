//! Raster image helpers: building image XObjects from PNG/JPEG/... bytes and
//! extracting embedded images back out of a document.

use crate::doc::{self, PageSelection};
use engine_shared::{b64, limits, EngineError, EngineResult};
use image::{ColorType, DynamicImage, GenericImageView, ImageEncoder, ImageFormat};
use lopdf::{Dictionary, Document, Object, Stream};
use serde_json::{json, Value};
use std::io::Cursor;

/// Decodes an image and returns an XObject stream plus its pixel size.
/// JPEG bytes are embedded unchanged (DCTDecode); other formats become Flate
/// compressed raw samples. Alpha channels are dropped (composited on white).
pub fn image_xobject(bytes: &[u8]) -> EngineResult<(Object, u32, u32)> {
    let format = image::guess_format(bytes).map_err(|_| {
        EngineError::unsupported_format(
            "Unrecognised image format (expected PNG, JPEG, GIF, WebP or BMP)",
        )
    })?;
    let img = image::load_from_memory_with_format(bytes, format)
        .map_err(|e| EngineError::malformed(format!("Image could not be decoded: {e}")))?;
    let (w, h) = img.dimensions();
    limits::check_dimensions(w, h)?;

    let mut dict = Dictionary::new();
    dict.set("Type", Object::Name(b"XObject".to_vec()));
    dict.set("Subtype", Object::Name(b"Image".to_vec()));
    dict.set("Width", Object::Integer(w as i64));
    dict.set("Height", Object::Integer(h as i64));
    dict.set("BitsPerComponent", Object::Integer(8));

    if format == ImageFormat::Jpeg && !matches!(img.color(), ColorType::Rgba8 | ColorType::La8) {
        let cs: &[u8] = match img.color() {
            ColorType::L8 | ColorType::L16 => b"DeviceGray",
            _ => b"DeviceRGB",
        };
        dict.set("ColorSpace", Object::Name(cs.to_vec()));
        dict.set("Filter", Object::Name(b"DCTDecode".to_vec()));
        let stream = Stream::new(dict, bytes.to_vec()).with_compression(false);
        return Ok((Object::Stream(stream), w, h));
    }

    let (samples, cs): (Vec<u8>, &[u8]) = match img.color() {
        ColorType::L8 => (img.to_luma8().into_raw(), b"DeviceGray"),
        _ => {
            // Composite alpha onto white so transparent PNGs look right.
            let rgba = img.to_rgba8();
            let mut out = Vec::with_capacity((w * h * 3) as usize);
            for p in rgba.pixels() {
                let a = p[3] as u32;
                for c in 0..3 {
                    let v = (p[c] as u32 * a + 255 * (255 - a)) / 255;
                    out.push(v as u8);
                }
            }
            (out, b"DeviceRGB")
        },
    };
    dict.set("ColorSpace", Object::Name(cs.to_vec()));
    let mut stream = Stream::new(dict, samples);
    stream
        .compress()
        .map_err(|e| EngineError::internal(format!("Failed to compress image samples: {e}")))?;
    Ok((Object::Stream(stream), w, h))
}

/// Extracts embedded raster images from the selected pages.
///
/// * `DCTDecode` (JPEG) streams are returned verbatim.
/// * Uncompressed / Flate 8-bit Gray, RGB and CMYK streams are converted to PNG.
/// * Other encodings (JPX, CCITT, JBIG2, indexed, 1/16-bit) are listed but
///   not decoded (`decoded: false`).
pub fn extract_images(
    bytes: &[u8],
    password: Option<&str>,
    pages: &PageSelection,
    min_size: u32,
) -> EngineResult<Value> {
    let d = doc::load(bytes, password)?;
    let total = d.get_pages().len();
    let selected = pages.resolve(total)?;
    let page_map = d.get_pages();
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for &n in &selected {
        let pid = page_map[&n];
        let (resources, res_ids) = match d.get_page_resources(pid) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let mut xobject_dicts: Vec<Dictionary> = Vec::new();
        if let Some(r) = resources {
            xobject_dicts.push(r.clone());
        }
        for rid in res_ids {
            if let Ok(r) = d.get_dictionary(rid) {
                xobject_dicts.push(r.clone());
            }
        }
        for res in xobject_dicts {
            let xobjs = match res
                .get(b"XObject")
                .ok()
                .and_then(|x| d.dereference(x).ok())
                .and_then(|(_, x)| x.as_dict().ok())
            {
                Some(x) => x.clone(),
                None => continue,
            };
            for (name, obj) in xobjs.iter() {
                let (id, stream) = match d.dereference(obj) {
                    Ok((Some(id), Object::Stream(s))) => (id, s),
                    _ => continue,
                };
                if !seen.insert(id) {
                    continue;
                }
                if stream.dict.get(b"Subtype").ok().and_then(|s| s.as_name().ok()) != Some(b"Image")
                {
                    continue;
                }
                let w = stream
                    .dict
                    .get(b"Width")
                    .ok()
                    .and_then(|v| v.as_i64().ok())
                    .unwrap_or(0) as u32;
                let h = stream
                    .dict
                    .get(b"Height")
                    .ok()
                    .and_then(|v| v.as_i64().ok())
                    .unwrap_or(0) as u32;
                if w < min_size || h < min_size {
                    continue;
                }
                let bpc = stream
                    .dict
                    .get(b"BitsPerComponent")
                    .ok()
                    .and_then(|v| v.as_i64().ok())
                    .unwrap_or(8);
                let filters = stream.filters().unwrap_or_default();
                let filter_names: Vec<String> = filters
                    .iter()
                    .map(|f| String::from_utf8_lossy(f).into_owned())
                    .collect();
                let cs = stream
                    .dict
                    .get(b"ColorSpace")
                    .ok()
                    .and_then(|c| d.dereference(c).ok())
                    .map(|(_, c)| match c {
                        Object::Name(n) => String::from_utf8_lossy(n).into_owned(),
                        Object::Array(a) => a
                            .first()
                            .and_then(|f| f.as_name().ok())
                            .map(|n| String::from_utf8_lossy(n).into_owned())
                            .unwrap_or_else(|| "Array".into()),
                        _ => "Unknown".into(),
                    })
                    .unwrap_or_else(|| "Unknown".into());
                let name_str = String::from_utf8_lossy(name).into_owned();
                let mut entry = json!({
                    "page": n,
                    "name": name_str,
                    "width": w,
                    "height": h,
                    "bits_per_component": bpc,
                    "color_space": cs,
                    "filters": filter_names,
                    "decoded": false,
                });
                if filters.last().map(|f| *f == b"DCTDecode").unwrap_or(false) && filters.len() == 1
                {
                    entry["decoded"] = json!(true);
                    entry["mime"] = json!("image/jpeg");
                    entry["file"] = json!(b64::encode(&stream.content));
                    entry["bytes"] = json!(stream.content.len());
                } else if bpc == 8 && (filters.is_empty() || filters == [b"FlateDecode".as_slice()])
                {
                    if let Ok(raw) =
                        stream.decompressed_content_with_limit(doc::MAX_DECOMPRESSED_STREAM)
                    {
                        let channels = match cs.as_str() {
                            "DeviceGray" | "CalGray" => 1,
                            "DeviceRGB" | "CalRGB" => 3,
                            "DeviceCMYK" => 4,
                            _ => 0,
                        };
                        if channels > 0
                            && raw.len() >= (w as usize * h as usize * channels)
                            && limits::check_dimensions(w, h).is_ok()
                        {
                            let rgb: Vec<u8> = match channels {
                                1 => raw[..(w * h) as usize].to_vec(),
                                3 => raw[..(w * h * 3) as usize].to_vec(),
                                _ => raw[..(w * h * 4) as usize]
                                    .chunks(4)
                                    .flat_map(|c| {
                                        let k = 255 - c[3] as u32;
                                        [
                                            ((255 - c[0] as u32) * k / 255) as u8,
                                            ((255 - c[1] as u32) * k / 255) as u8,
                                            ((255 - c[2] as u32) * k / 255) as u8,
                                        ]
                                    })
                                    .collect(),
                            };
                            let color = if channels == 1 { ColorType::L8 } else { ColorType::Rgb8 };
                            let mut png = Vec::new();
                            if image::codecs::png::PngEncoder::new(&mut png)
                                .write_image(&rgb, w, h, color.into())
                                .is_ok()
                            {
                                entry["decoded"] = json!(true);
                                entry["mime"] = json!("image/png");
                                entry["bytes"] = json!(png.len());
                                entry["file"] = json!(b64::encode(&png));
                            }
                        }
                    }
                }
                out.push(entry);
                if out.len() > limits::MAX_BATCH_FILES {
                    return Err(EngineError::limit(
                        "Too many images to extract in one call; narrow the page selection",
                    ));
                }
            }
        }
    }
    let decoded = out.iter().filter(|e| e["decoded"] == true).count();
    Ok(json!({ "images": out, "count": out.len(), "decoded": decoded }))
}

/// Decodes a `DCTDecode` or Flate RGB image stream into a `DynamicImage`
/// (used by the grayscale conversion).
pub fn decode_image_stream(
    doc: &Document,
    stream: &Stream,
) -> Option<(DynamicImage, &'static str)> {
    let filters = stream.filters().unwrap_or_default();
    if filters.iter().any(|&f| f == b"DCTDecode") {
        if let Ok(i) = image::load_from_memory_with_format(&stream.content, ImageFormat::Jpeg) {
            return Some((i, "jpeg"));
        }
    }
    if !(filters.is_empty() || filters.iter().any(|&f| f == b"FlateDecode")) {
        return None;
    }
    let w = match stream.dict.get(b"Width").ok()? {
        Object::Integer(i) => *i as u32,
        Object::Reference(rid) => doc.get_object(*rid).ok()?.as_i64().ok()? as u32,
        _ => return None,
    };
    let h = match stream.dict.get(b"Height").ok()? {
        Object::Integer(i) => *i as u32,
        Object::Reference(rid) => doc.get_object(*rid).ok()?.as_i64().ok()? as u32,
        _ => return None,
    };
    let bpc = stream
        .dict
        .get(b"BitsPerComponent")
        .ok()
        .and_then(|v| match v {
            Object::Integer(i) => Some(*i),
            Object::Reference(rid) => doc.get_object(*rid).ok()?.as_i64().ok(),
            _ => None,
        })
        .unwrap_or(8);
    if bpc != 8 {
        return None;
    }
    let raw = stream
        .decompressed_content_with_limit(doc::MAX_DECOMPRESSED_STREAM)
        .ok()?;
    let cs = stream
        .dict
        .get(b"ColorSpace")
        .ok()
        .and_then(|c| doc.dereference(c).ok())
        .and_then(|(_, c)| match c {
            Object::Name(n) => Some(n.clone()),
            Object::Array(arr) => arr.first().and_then(|x| x.as_name().ok()).map(|n| n.to_vec()),
            _ => None,
        });

    let img = match cs.as_deref() {
        Some(b"DeviceRGB") | Some(b"ICCBased") if raw.len() >= (w * h * 3) as usize => {
            image::RgbImage::from_raw(w, h, raw[..(w * h * 3) as usize].to_vec())
                .map(DynamicImage::ImageRgb8)
        }
        Some(b"DeviceGray") if raw.len() >= (w * h) as usize => {
            image::GrayImage::from_raw(w, h, raw[..(w * h) as usize].to_vec())
                .map(DynamicImage::ImageLuma8)
        }
        _ => {
            if raw.len() >= (w * h * 3) as usize {
                image::RgbImage::from_raw(w, h, raw[..(w * h * 3) as usize].to_vec())
                    .map(DynamicImage::ImageRgb8)
            } else if raw.len() >= (w * h) as usize {
                image::GrayImage::from_raw(w, h, raw[..(w * h) as usize].to_vec())
                    .map(DynamicImage::ImageLuma8)
            } else {
                None
            }
        }
    }?;
    Some((img, "flate"))
}

/// Encodes a grayscale image back into a stream of the given kind.
pub fn gray_stream(img: &DynamicImage, kind: &str, template: &Stream) -> Option<Stream> {
    let gray = img.to_luma8();
    let mut dict = template.dict.clone();
    dict.set("ColorSpace", Object::Name(b"DeviceGray".to_vec()));
    dict.remove(b"Decode");
    dict.remove(b"SMask");
    dict.remove(b"Mask");
    if kind == "jpeg" {
        let mut out = Vec::new();
        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(Cursor::new(&mut out), 85);
        enc.encode(gray.as_raw(), gray.width(), gray.height(), ColorType::L8.into())
            .ok()?;
        dict.set("Filter", Object::Name(b"DCTDecode".to_vec()));
        dict.remove(b"DecodeParms");
        dict.set("Length", Object::Integer(out.len() as i64));
        Some(Stream::new(dict, out).with_compression(false))
    } else {
        dict.remove(b"Filter");
        dict.remove(b"DecodeParms");
        let mut s = Stream::new(dict, gray.into_raw());
        s.compress().ok()?;
        Some(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{png_bytes, sample_pdf};

    #[test]
    fn xobject_from_png_and_invalid() {
        let png = png_bytes(4, 3);
        let (obj, w, h) = image_xobject(&png).unwrap();
        assert_eq!((w, h), (4, 3));
        assert!(matches!(obj, Object::Stream(_)));
        assert!(image_xobject(b"not an image").is_err());
        assert!(image_xobject(&[0x89, b'P', b'N', b'G', 1, 2, 3]).is_err());
    }

    #[test]
    fn extract_roundtrip() {
        let png = png_bytes(8, 8);
        let pdf = crate::create::from_images(&[png], Some("A4"), 10.0, "contain").unwrap();
        let bytes = b64::decode(pdf["output"]["file"].as_str().unwrap()).unwrap();
        let res = extract_images(&bytes, None, &PageSelection::All, 1).unwrap();
        assert_eq!(res["count"], 1);
        assert_eq!(res["images"][0]["mime"], "image/png");
        assert_eq!(res["images"][0]["width"], 8);
        let plain = sample_pdf(&["x"]);
        assert_eq!(extract_images(&plain, None, &PageSelection::All, 1).unwrap()["count"], 0);
    }
}
