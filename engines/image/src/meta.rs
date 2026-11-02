//! Container level metadata: EXIF reading, lossless metadata stripping and
//! DPI editing for JPEG and PNG.

use engine_shared::{b64, EngineError, EngineResult};
use image::ImageFormat;
use serde_json::{json, Map, Value};

/// Reads EXIF tags (JPEG, TIFF, PNG, WebP containers). Returns `null` when
/// the image carries no EXIF block.
pub fn read_exif(bytes: &[u8]) -> Value {
    let reader = exif::Reader::new();
    let exif = match reader.read_from_container(&mut std::io::Cursor::new(bytes)) {
        Ok(e) => e,
        Err(_) => return Value::Null,
    };
    let mut map = Map::new();
    let mut gps = Map::new();
    for f in exif.fields() {
        let name = f.tag.to_string();
        let value = f.display_value().with_unit(&exif).to_string();
        if f.ifd_num == exif::In::PRIMARY || f.ifd_num == exif::In::THUMBNAIL {
            if name.starts_with("GPS") {
                gps.insert(name, json!(value));
            } else if map.len() < 200 {
                map.insert(name, json!(value));
            }
        }
    }
    // Decimal GPS coordinates when available.
    let coord = |tag: exif::Tag, ref_tag: exif::Tag| -> Option<f64> {
        let f = exif.get_field(tag, exif::In::PRIMARY)?;
        let r = exif
            .get_field(ref_tag, exif::In::PRIMARY)
            .and_then(|r| r.display_value().to_string().chars().next())
            .unwrap_or('N');
        if let exif::Value::Rational(v) = &f.value {
            if v.len() >= 3 {
                let d = v[0].to_f64() + v[1].to_f64() / 60.0 + v[2].to_f64() / 3600.0;
                return Some(if r == 'S' || r == 'W' { -d } else { d });
            }
        }
        None
    };
    let lat = coord(exif::Tag::GPSLatitude, exif::Tag::GPSLatitudeRef);
    let lon = coord(exif::Tag::GPSLongitude, exif::Tag::GPSLongitudeRef);
    let orientation = exif
        .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        .and_then(|f| f.value.get_uint(0));
    json!({
        "tags": map,
        "gps": if gps.is_empty() { Value::Null } else { Value::Object(gps) },
        "latitude": lat,
        "longitude": lon,
        "orientation": orientation,
        "tag_count": exif.fields().count(),
    })
}

/// Reads the pixel density declared in the container (JFIF APP0 or PNG pHYs)
/// falling back to EXIF XResolution/YResolution.
pub fn read_dpi(bytes: &[u8], format: ImageFormat) -> Option<(f64, f64)> {
    match format {
        ImageFormat::Jpeg => {
            for seg in jpeg_segments(bytes) {
                if seg.marker == 0xE0 && seg.data.starts_with(b"JFIF\0") && seg.data.len() >= 12 {
                    let units = seg.data[7];
                    let xd = u16::from_be_bytes([seg.data[8], seg.data[9]]) as f64;
                    let yd = u16::from_be_bytes([seg.data[10], seg.data[11]]) as f64;
                    match units {
                        1 if xd > 0.0 && yd > 0.0 => return Some((xd, yd)),
                        2 if xd > 0.0 && yd > 0.0 => return Some((xd * 2.54, yd * 2.54)),
                        _ => {},
                    }
                }
            }
        },
        ImageFormat::Png => {
            for chunk in png_chunks(bytes) {
                if chunk.kind == *b"pHYs" && chunk.data.len() >= 9 && chunk.data[8] == 1 {
                    let x = u32::from_be_bytes([
                        chunk.data[0],
                        chunk.data[1],
                        chunk.data[2],
                        chunk.data[3],
                    ]) as f64;
                    let y = u32::from_be_bytes([
                        chunk.data[4],
                        chunk.data[5],
                        chunk.data[6],
                        chunk.data[7],
                    ]) as f64;
                    if x > 0.0 && y > 0.0 {
                        return Some((
                            (x * 0.0254 * 100.0).round() / 100.0,
                            (y * 0.0254 * 100.0).round() / 100.0,
                        ));
                    }
                }
            }
        },
        _ => {},
    }
    // EXIF fallback.
    let exif = exif::Reader::new()
        .read_from_container(&mut std::io::Cursor::new(bytes))
        .ok()?;
    let x = exif.get_field(exif::Tag::XResolution, exif::In::PRIMARY)?;
    let y = exif.get_field(exif::Tag::YResolution, exif::In::PRIMARY)?;
    let unit = exif
        .get_field(exif::Tag::ResolutionUnit, exif::In::PRIMARY)
        .and_then(|u| u.value.get_uint(0))
        .unwrap_or(2);
    if let (exif::Value::Rational(xr), exif::Value::Rational(yr)) = (&x.value, &y.value) {
        let (mut xv, mut yv) = (xr.first()?.to_f64(), yr.first()?.to_f64());
        if unit == 3 {
            xv *= 2.54;
            yv *= 2.54;
        }
        if xv > 0.0 && yv > 0.0 {
            return Some((xv, yv));
        }
    }
    None
}

/// A JPEG marker segment.
pub struct JpegSegment<'a> {
    /// Marker byte (e.g. `0xE1` for APP1).
    pub marker: u8,
    /// Segment payload (after the length field).
    pub data: &'a [u8],
    /// Byte range of the whole segment (marker included).
    pub range: std::ops::Range<usize>,
}

/// Iterates JPEG marker segments up to the start of scan.
pub fn jpeg_segments(bytes: &[u8]) -> Vec<JpegSegment<'_>> {
    let mut out = Vec::new();
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return out;
    }
    let mut i = 2;
    while i + 4 <= bytes.len() {
        if bytes[i] != 0xFF {
            break;
        }
        let marker = bytes[i + 1];
        if marker == 0xD8 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            i += 2;
            continue;
        }
        if marker == 0xDA || marker == 0xD9 {
            break;
        }
        let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        if len < 2 || i + 2 + len > bytes.len() {
            break;
        }
        out.push(JpegSegment {
            marker,
            data: &bytes[i + 4..i + 2 + len],
            range: i..i + 2 + len,
        });
        i += 2 + len;
    }
    out
}

/// A PNG chunk.
pub struct PngChunk<'a> {
    /// Four-letter chunk type.
    pub kind: [u8; 4],
    /// Chunk payload.
    pub data: &'a [u8],
    /// Byte range of the whole chunk (length, type, data, CRC) in the file.
    pub range: std::ops::Range<usize>,
}

/// Iterates PNG chunks.
pub fn png_chunks(bytes: &[u8]) -> Vec<PngChunk<'_>> {
    let mut out = Vec::new();
    if bytes.len() < 8 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        return out;
    }
    let mut i = 8;
    while i + 12 <= bytes.len() {
        let len = u32::from_be_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]) as usize;
        let kind = [bytes[i + 4], bytes[i + 5], bytes[i + 6], bytes[i + 7]];
        let end = i + 12 + len;
        if end > bytes.len() {
            break;
        }
        out.push(PngChunk {
            kind,
            data: &bytes[i + 8..i + 8 + len],
            range: i..end,
        });
        i = end;
        if kind == *b"IEND" {
            break;
        }
    }
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (n, entry) in table.iter_mut().enumerate() {
        let mut c = n as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *entry = c;
    }
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

fn png_chunk_bytes(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 12);
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_input = kind.to_vec();
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
    out
}

/// Removes metadata. JPEG and PNG are edited losslessly at the container
/// level (APP segments / ancillary chunks); other formats are re-encoded.
pub fn strip_metadata(bytes: &[u8], keep_icc: bool) -> EngineResult<Value> {
    let format = crate::codec::detect_format(bytes)?;
    let mut removed: Vec<String> = Vec::new();
    let (out, lossless): (Vec<u8>, bool) = match format {
        ImageFormat::Jpeg => {
            let segs = jpeg_segments(bytes);
            let mut out = vec![0xFF, 0xD8];
            let mut last_end = 2;
            for s in &segs {
                let is_app = (0xE0..=0xEF).contains(&s.marker) || s.marker == 0xFE;
                let is_icc = s.marker == 0xE2 && s.data.starts_with(b"ICC_PROFILE\0");
                let is_jfif = s.marker == 0xE0 && s.data.starts_with(b"JFIF\0");
                let drop = is_app && !is_jfif && !(keep_icc && is_icc);
                if drop {
                    let label = match s.marker {
                        0xE1 if s.data.starts_with(b"Exif") => "EXIF (APP1)".to_string(),
                        0xE1 if s.data.starts_with(b"http://ns.adobe.com/xap/") => {
                            "XMP (APP1)".to_string()
                        },
                        0xE2 if is_icc => "ICC profile (APP2)".to_string(),
                        0xED => "Photoshop IRB (APP13)".to_string(),
                        0xFE => "Comment".to_string(),
                        m => format!("APP{}", m - 0xE0),
                    };
                    removed.push(label);
                } else {
                    out.extend_from_slice(&bytes[s.range.clone()]);
                }
                last_end = s.range.end;
            }
            out.extend_from_slice(&bytes[last_end..]);
            (out, true)
        },
        ImageFormat::Png => {
            let chunks = png_chunks(bytes);
            if chunks.is_empty() {
                return Err(EngineError::malformed("PNG chunk structure is invalid"));
            }
            let mut out = bytes[..8].to_vec();
            for c in &chunks {
                let kind = std::str::from_utf8(&c.kind).unwrap_or("????");
                let drop = matches!(kind, "tEXt" | "zTXt" | "iTXt" | "eXIf" | "tIME" | "dSIG")
                    || (kind == "iCCP" && !keep_icc);
                if drop {
                    removed.push(kind.to_string());
                } else {
                    out.extend_from_slice(&bytes[c.range.clone()]);
                }
            }
            (out, true)
        },
        _ => {
            let d = crate::codec::decode(bytes)?;
            let fmt = crate::codec::OutFormat::from_input(format);
            removed.push("all container metadata (re-encoded)".into());
            (
                crate::codec::encode(&d.image, &crate::codec::EncodeOptions::with_format(fmt))?,
                false,
            )
        },
    };
    let fmt = crate::codec::OutFormat::from_input(format);
    let (w, h) = image::ImageReader::with_format(std::io::Cursor::new(&out), format)
        .into_dimensions()
        .unwrap_or((0, 0));
    Ok(json!({
        "output": crate::codec::ImageOutput::new(out.clone(), fmt, w, h),
        "removed": removed,
        "lossless": lossless,
        "bytes_before": bytes.len(),
        "bytes_after": out.len(),
        "has_exif_after": !read_exif(&out).is_null(),
    }))
}

/// Sets the pixel density (DPI) losslessly for JPEG (JFIF APP0) and PNG
/// (pHYs). Other formats are re-encoded as PNG with a pHYs chunk.
pub fn set_dpi(bytes: &[u8], dpi: f64) -> EngineResult<Value> {
    engine_shared::error::require_range(dpi, 1.0, 10_000.0, "dpi")?;
    let format = crate::codec::detect_format(bytes)?;
    let out = match format {
        ImageFormat::Jpeg => {
            let d = dpi.round().clamp(1.0, 65535.0) as u16;
            let mut jfif = b"JFIF\0\x01\x02\x01".to_vec();
            jfif.extend_from_slice(&d.to_be_bytes());
            jfif.extend_from_slice(&d.to_be_bytes());
            jfif.extend_from_slice(&[0, 0]);
            let mut seg = vec![0xFF, 0xE0];
            seg.extend_from_slice(&((jfif.len() + 2) as u16).to_be_bytes());
            seg.extend_from_slice(&jfif);
            let segs = jpeg_segments(bytes);
            let mut out = vec![0xFF, 0xD8];
            out.extend_from_slice(&seg);
            let mut last_end = 2;
            for s in &segs {
                if !(s.marker == 0xE0 && s.data.starts_with(b"JFIF\0")) {
                    out.extend_from_slice(&bytes[s.range.clone()]);
                }
                last_end = s.range.end;
            }
            out.extend_from_slice(&bytes[last_end..]);
            out
        },
        ImageFormat::Png => {
            let chunks = png_chunks(bytes);
            if chunks.is_empty() {
                return Err(EngineError::malformed("PNG chunk structure is invalid"));
            }
            let ppm = (dpi / 0.0254).round() as u32;
            let mut phys = Vec::new();
            phys.extend_from_slice(&ppm.to_be_bytes());
            phys.extend_from_slice(&ppm.to_be_bytes());
            phys.push(1);
            let phys_chunk = png_chunk_bytes(b"pHYs", &phys);
            let mut out = bytes[..8].to_vec();
            let mut inserted = false;
            for c in &chunks {
                if c.kind == *b"pHYs" {
                    continue;
                }
                if !inserted && (c.kind == *b"IDAT" || c.kind == *b"PLTE") {
                    out.extend_from_slice(&phys_chunk);
                    inserted = true;
                }
                out.extend_from_slice(&bytes[c.range.clone()]);
            }
            out
        },
        _ => {
            let d = crate::codec::decode(bytes)?;
            let png = crate::codec::encode(&d.image, &crate::codec::EncodeOptions::default())?;
            return set_dpi(&png, dpi);
        },
    };
    let fmt = crate::codec::OutFormat::from_input(format);
    let (w, h) = image::ImageReader::with_format(std::io::Cursor::new(&out), format)
        .into_dimensions()
        .unwrap_or((0, 0));
    let new_dpi = read_dpi(&out, format);
    Ok(json!({
        "output": crate::codec::ImageOutput::new(out, fmt, w, h),
        "dpi": new_dpi,
        "print_size_in": new_dpi.map(|d| json!({ "width": (w as f64 / d.0 * 100.0).round() / 100.0, "height": (h as f64 / d.1 * 100.0).round() / 100.0 })),
        "lossless": true,
    }))
}

/// Wraps image bytes in a data URL.
pub fn to_data_url(bytes: &[u8]) -> EngineResult<Value> {
    let format = crate::codec::detect_format(bytes)?;
    let mime = format!("image/{}", crate::codec::format_name(format));
    let (w, h) = image::ImageReader::with_format(std::io::Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(|e| EngineError::malformed(e.to_string()))?;
    let encoded = b64::encode(bytes);
    Ok(json!({
        "data_url": format!("data:{mime};base64,{encoded}"),
        "base64": encoded,
        "mime": mime,
        "width": w,
        "height": h,
        "bytes": bytes.len(),
        "base64_length": encoded.len(),
    }))
}

/// Decodes a base64 / data URL back into an image and validates it.
pub fn from_data_url(data: &str) -> EngineResult<Value> {
    let trimmed = data.trim();
    if trimmed.is_empty() {
        return Err(EngineError::empty("base64 data"));
    }
    let declared_mime = trimmed
        .strip_prefix("data:")
        .and_then(|r| r.split(';').next())
        .map(str::to_string);
    let bytes = b64::decode(trimmed)?;
    if bytes.is_empty() {
        return Err(EngineError::empty("decoded data"));
    }
    let info = crate::codec::inspect(&bytes)?;
    let fmt = crate::codec::OutFormat::from_input(crate::codec::detect_format(&bytes)?);
    Ok(json!({
        "output": crate::codec::ImageOutput::new(bytes, fmt, info["width"].as_u64().unwrap_or(0) as u32, info["height"].as_u64().unwrap_or(0) as u32),
        "info": info,
        "declared_mime": declared_mime,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{jpeg_bytes, png_bytes};

    #[test]
    fn dpi_roundtrip() {
        let jpg = jpeg_bytes(8, 8);
        let r = set_dpi(&jpg, 300.0).unwrap();
        assert_eq!(r["dpi"], json!([300.0, 300.0]));
        let out = b64::decode(r["output"]["file"].as_str().unwrap()).unwrap();
        assert_eq!(read_dpi(&out, ImageFormat::Jpeg), Some((300.0, 300.0)));
        let png = png_bytes(8, 8);
        let r = set_dpi(&png, 72.0).unwrap();
        let out = b64::decode(r["output"]["file"].as_str().unwrap()).unwrap();
        let d = read_dpi(&out, ImageFormat::Png).unwrap();
        assert!((d.0 - 72.0).abs() < 0.1);
        assert!(crate::codec::decode(&out).is_ok());
        assert!(set_dpi(&png, 0.0).is_err());
    }

    #[test]
    fn strip() {
        let png = png_bytes(8, 8);
        // Add a tEXt chunk.
        let chunks = png_chunks(&png);
        let mut with_text = png[..8].to_vec();
        for c in &chunks {
            if c.kind == *b"IEND" {
                with_text.extend_from_slice(&png_chunk_bytes(b"tEXt", b"Comment\0hello"));
            }
            with_text.extend_from_slice(&png[c.range.clone()]);
        }
        let r = strip_metadata(&with_text, false).unwrap();
        assert_eq!(r["removed"], json!(["tEXt"]));
        assert_eq!(r["lossless"], true);
        let out = b64::decode(r["output"]["file"].as_str().unwrap()).unwrap();
        assert!(crate::codec::decode(&out).is_ok());
        let jpg = jpeg_bytes(8, 8);
        let r = strip_metadata(&jpg, true).unwrap();
        assert_eq!(r["has_exif_after"], false);
        assert!(strip_metadata(b"", false).is_err());
    }

    #[test]
    fn data_urls() {
        let png = png_bytes(3, 2);
        let r = to_data_url(&png).unwrap();
        assert!(r["data_url"].as_str().unwrap().starts_with("data:image/png;base64,"));
        let back = from_data_url(r["data_url"].as_str().unwrap()).unwrap();
        assert_eq!(back["info"]["width"], 3);
        assert_eq!(back["declared_mime"], "image/png");
        assert!(from_data_url("").is_err());
        assert!(from_data_url("aGVsbG8=").is_err());
    }
}
