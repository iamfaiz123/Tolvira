//! Decoding, encoding and inspection shared by every image action.

use engine_shared::{b64, limits, EngineError, EngineResult};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::{ColorType, DynamicImage, ExtendedColorType, ImageEncoder, ImageFormat};
use serde::Serialize;
use serde_json::{json, Value};
use std::io::Cursor;

/// Output formats the engine can encode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutFormat {
    /// PNG (lossless, alpha).
    Png,
    /// JPEG (lossy, no alpha).
    Jpeg,
    /// WebP (lossless encoder).
    Webp,
    /// GIF (palette, single frame).
    Gif,
    /// BMP (uncompressed).
    Bmp,
    /// ICO (embedded PNG frames).
    Ico,
    /// TIFF (uncompressed).
    Tiff,
}

impl OutFormat {
    /// Parses a user-supplied format name or extension (`jpg`, `image/png`, …).
    pub fn parse(name: &str) -> EngineResult<OutFormat> {
        Ok(
            match name
                .trim()
                .trim_start_matches("image/")
                .trim_start_matches('.')
                .to_ascii_lowercase()
                .as_str()
            {
                "png" => OutFormat::Png,
                "jpg" | "jpeg" => OutFormat::Jpeg,
                "webp" => OutFormat::Webp,
                "gif" => OutFormat::Gif,
                "bmp" => OutFormat::Bmp,
                "ico" | "x-icon" | "vnd.microsoft.icon" => OutFormat::Ico,
                "tif" | "tiff" => OutFormat::Tiff,
                "avif" | "heic" | "heif" => {
                    return Err(EngineError::unsupported(format!(
                        "Encoding `{name}` is not available in the WASM engine (no pure-Rust \
                         encoder). Convert to PNG/JPEG/WebP here, or use the browser Canvas \
                         `toBlob('image/avif')` where supported."
                    )))
                },
                other => {
                    return Err(EngineError::invalid(format!(
                        "Unknown output format `{other}` (use png, jpeg, webp, gif, bmp, ico or \
                         tiff)"
                    )))
                },
            },
        )
    }

    /// Best output format for a given decoded input format.
    pub fn from_input(fmt: ImageFormat) -> OutFormat {
        match fmt {
            ImageFormat::Jpeg => OutFormat::Jpeg,
            ImageFormat::WebP => OutFormat::Webp,
            ImageFormat::Gif => OutFormat::Gif,
            ImageFormat::Bmp => OutFormat::Bmp,
            ImageFormat::Ico => OutFormat::Ico,
            ImageFormat::Tiff => OutFormat::Tiff,
            _ => OutFormat::Png,
        }
    }

    /// IANA media type.
    pub fn mime(self) -> &'static str {
        match self {
            OutFormat::Png => "image/png",
            OutFormat::Jpeg => "image/jpeg",
            OutFormat::Webp => "image/webp",
            OutFormat::Gif => "image/gif",
            OutFormat::Bmp => "image/bmp",
            OutFormat::Ico => "image/x-icon",
            OutFormat::Tiff => "image/tiff",
        }
    }

    /// Default file extension.
    pub fn extension(self) -> &'static str {
        match self {
            OutFormat::Png => "png",
            OutFormat::Jpeg => "jpg",
            OutFormat::Webp => "webp",
            OutFormat::Gif => "gif",
            OutFormat::Bmp => "bmp",
            OutFormat::Ico => "ico",
            OutFormat::Tiff => "tiff",
        }
    }

    /// Whether the format can store an alpha channel.
    pub fn supports_alpha(self) -> bool {
        !matches!(self, OutFormat::Jpeg | OutFormat::Bmp)
    }
}

/// A decoded image with its source format.
pub struct Decoded {
    /// Decoded pixels.
    pub image: DynamicImage,
    /// Format the bytes were decoded from.
    pub format: ImageFormat,
}

/// Human-readable name of an input format.
pub fn format_name(f: ImageFormat) -> &'static str {
    match f {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpeg",
        ImageFormat::Gif => "gif",
        ImageFormat::WebP => "webp",
        ImageFormat::Bmp => "bmp",
        ImageFormat::Ico => "ico",
        ImageFormat::Tiff => "tiff",
        ImageFormat::Avif => "avif",
        _ => "unknown",
    }
}

/// Detects the container format from magic bytes.
pub fn detect_format(bytes: &[u8]) -> EngineResult<ImageFormat> {
    if bytes.is_empty() {
        return Err(EngineError::empty("image file"));
    }
    image::guess_format(bytes).map_err(|_| {
        let hint = if bytes.starts_with(b"<") || bytes.starts_with(b"\xEF\xBB\xBF<") {
            " The data looks like SVG/XML – use the `svg_to_raster` action."
        } else if bytes.starts_with(b"%PDF") {
            " The data is a PDF – use the PDF engine."
        } else if bytes.len() > 12 && &bytes[4..12] == b"ftypheic"
            || bytes.len() > 12 && &bytes[4..12] == b"ftypavif"
        {
            " HEIC/AVIF decoding is not available in the WASM engine; decode it with the browser \
             (createImageBitmap) first."
        } else {
            ""
        };
        EngineError::unsupported_format(format!(
            "Unrecognised image format (expected PNG, JPEG, GIF, WebP, BMP, ICO or TIFF).{hint}"
        ))
    })
}

/// Decodes an image enforcing the shared size limits.
pub fn decode(bytes: &[u8]) -> EngineResult<Decoded> {
    limits::check_size(bytes.len(), limits::MAX_FILE_BYTES, "image file")?;
    let format = detect_format(bytes)?;
    if matches!(format, ImageFormat::Avif) {
        return Err(EngineError::unsupported(
            "AVIF decoding is not available in the WASM engine; decode it with the browser \
             (createImageBitmap / Canvas) and send PNG.",
        ));
    }
    // Check the header dimensions before allocating the full image.
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    reader.no_limits();
    let (w, h) = reader
        .into_dimensions()
        .map_err(|e| EngineError::malformed(format!("Image header could not be read: {e}")))?;
    limits::check_dimensions(w, h)?;
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    reader.no_limits();
    let image = reader.decode().map_err(|e| {
        EngineError::malformed(format!("Image could not be decoded ({}): {e}", format_name(format)))
    })?;
    Ok(Decoded { image, format })
}

/// Encoding options.
#[derive(Debug, Clone)]
pub struct EncodeOptions {
    /// Target format.
    pub format: OutFormat,
    /// JPEG quality 1-100 (default 85). Also used for WebP (lossless only; ignored).
    pub quality: u8,
    /// PNG compression: 0 = fast, 1 = default, 2 = best.
    pub png_level: u8,
    /// Background colour used when alpha has to be dropped (JPEG/BMP).
    pub background: [u8; 3],
}

impl Default for EncodeOptions {
    fn default() -> Self {
        EncodeOptions {
            format: OutFormat::Png,
            quality: 85,
            png_level: 1,
            background: [255, 255, 255],
        }
    }
}

impl EncodeOptions {
    /// Options for `format` with default quality/compression.
    pub fn with_format(format: OutFormat) -> Self {
        EncodeOptions { format, ..Default::default() }
    }
    /// Checks that quality/compression values are within range.
    pub fn validate(&self) -> EngineResult<()> {
        if !(1..=100).contains(&self.quality) {
            return Err(EngineError::invalid("`quality` must be between 1 and 100"));
        }
        if self.png_level > 2 {
            return Err(EngineError::invalid(
                "`png_level` must be 0 (fast), 1 (default) or 2 (best)",
            ));
        }
        Ok(())
    }
}

/// Composites an RGBA image onto a solid background, returning RGB.
pub fn flatten(img: &DynamicImage, bg: [u8; 3]) -> image::RgbImage {
    let rgba = img.to_rgba8();
    let mut out = image::RgbImage::new(rgba.width(), rgba.height());
    for (x, y, p) in rgba.enumerate_pixels() {
        let a = p[3] as u32;
        let mut o = [0u8; 3];
        for c in 0..3 {
            o[c] = ((p[c] as u32 * a + bg[c] as u32 * (255 - a)) / 255) as u8;
        }
        out.put_pixel(x, y, image::Rgb(o));
    }
    out
}

/// Encodes an image to bytes.
pub fn encode(img: &DynamicImage, opts: &EncodeOptions) -> EngineResult<Vec<u8>> {
    opts.validate()?;
    let mut out = Vec::new();
    let has_alpha = img.color().has_alpha();
    let res: Result<(), image::ImageError> = match opts.format {
        OutFormat::Png => {
            let level = match opts.png_level {
                0 => CompressionType::Fast,
                2 => CompressionType::Best,
                _ => CompressionType::Default,
            };
            let enc =
                PngEncoder::new_with_quality(Cursor::new(&mut out), level, PngFilter::Adaptive);
            match img.color() {
                ColorType::L8
                | ColorType::La8
                | ColorType::Rgb8
                | ColorType::Rgba8
                | ColorType::L16
                | ColorType::La16
                | ColorType::Rgb16
                | ColorType::Rgba16 => {
                    enc.write_image(img.as_bytes(), img.width(), img.height(), img.color().into())
                },
                _ => {
                    let rgba = img.to_rgba8();
                    enc.write_image(
                        rgba.as_raw(),
                        rgba.width(),
                        rgba.height(),
                        ExtendedColorType::Rgba8,
                    )
                },
            }
        },
        OutFormat::Jpeg => {
            let mut enc = JpegEncoder::new_with_quality(Cursor::new(&mut out), opts.quality);
            if matches!(img.color(), ColorType::L8 | ColorType::L16) {
                let g = img.to_luma8();
                enc.encode(g.as_raw(), g.width(), g.height(), ExtendedColorType::L8)
            } else if has_alpha {
                let rgb = flatten(img, opts.background);
                enc.encode(rgb.as_raw(), rgb.width(), rgb.height(), ExtendedColorType::Rgb8)
            } else {
                let rgb = img.to_rgb8();
                enc.encode(rgb.as_raw(), rgb.width(), rgb.height(), ExtendedColorType::Rgb8)
            }
        },
        OutFormat::Webp => {
            let enc = image::codecs::webp::WebPEncoder::new_lossless(Cursor::new(&mut out));
            if has_alpha {
                let rgba = img.to_rgba8();
                enc.encode(rgba.as_raw(), rgba.width(), rgba.height(), ExtendedColorType::Rgba8)
            } else {
                let rgb = img.to_rgb8();
                enc.encode(rgb.as_raw(), rgb.width(), rgb.height(), ExtendedColorType::Rgb8)
            }
        },
        OutFormat::Gif => {
            let mut enc = image::codecs::gif::GifEncoder::new(Cursor::new(&mut out));
            let rgba = img.to_rgba8();
            enc.encode(rgba.as_raw(), rgba.width(), rgba.height(), ExtendedColorType::Rgba8)
        },
        OutFormat::Bmp => {
            let enc = image::codecs::bmp::BmpEncoder::new(&mut out);
            let rgb = if has_alpha {
                flatten(img, opts.background)
            } else {
                img.to_rgb8()
            };
            enc.write_image(rgb.as_raw(), rgb.width(), rgb.height(), ExtendedColorType::Rgb8)
        },
        OutFormat::Ico => {
            if img.width() > 256 || img.height() > 256 {
                return Err(EngineError::invalid(
                    "ICO images must be at most 256x256 pixels (use the `favicon` action to \
                     generate resized variants)",
                ));
            }
            let enc = image::codecs::ico::IcoEncoder::new(Cursor::new(&mut out));
            let rgba = img.to_rgba8();
            enc.write_image(rgba.as_raw(), rgba.width(), rgba.height(), ExtendedColorType::Rgba8)
        },
        OutFormat::Tiff => {
            let enc = image::codecs::tiff::TiffEncoder::new(Cursor::new(&mut out));
            let rgba = img.to_rgba8();
            enc.write_image(rgba.as_raw(), rgba.width(), rgba.height(), ExtendedColorType::Rgba8)
        },
    };
    res.map_err(|e| {
        EngineError::internal(format!("Encoding to {} failed: {e}", opts.format.extension()))
    })?;
    Ok(out)
}

/// Standard output envelope for generated images.
#[derive(Serialize)]
pub struct ImageOutput {
    /// Base64 image bytes.
    pub file: String,
    /// IANA media type.
    pub mime: &'static str,
    /// File extension without a dot.
    pub extension: &'static str,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Encoded size in bytes.
    pub bytes: usize,
}

impl ImageOutput {
    /// Encodes `bytes` as base64 and fills in the format metadata.
    pub fn new(bytes: Vec<u8>, format: OutFormat, width: u32, height: u32) -> Self {
        ImageOutput {
            bytes: bytes.len(),
            file: b64::encode(&bytes),
            mime: format.mime(),
            extension: format.extension(),
            width,
            height,
        }
    }
}

/// Encodes `img` and wraps it into the standard output.
pub fn output(img: &DynamicImage, opts: &EncodeOptions) -> EngineResult<ImageOutput> {
    let bytes = encode(img, opts)?;
    Ok(ImageOutput::new(bytes, opts.format, img.width(), img.height()))
}

/// Resolves the output format: explicit `format` or the input format
/// (falling back to PNG for formats we cannot encode).
pub fn resolve_format(requested: Option<&str>, input: ImageFormat) -> EngineResult<OutFormat> {
    match requested.filter(|s| !s.trim().is_empty() && *s != "auto" && *s != "same") {
        Some(f) => OutFormat::parse(f),
        None => Ok(OutFormat::from_input(input)),
    }
}

/// Parses `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`, `transparent` or names.
pub fn parse_color(s: &str) -> EngineResult<[u8; 4]> {
    let t = s.trim().to_ascii_lowercase();
    let named: Option<[u8; 4]> = match t.as_str() {
        "transparent" | "none" => Some([0, 0, 0, 0]),
        "black" => Some([0, 0, 0, 255]),
        "white" => Some([255, 255, 255, 255]),
        "red" => Some([255, 0, 0, 255]),
        "green" => Some([0, 128, 0, 255]),
        "lime" => Some([0, 255, 0, 255]),
        "blue" => Some([0, 0, 255, 255]),
        "yellow" => Some([255, 255, 0, 255]),
        "cyan" | "aqua" => Some([0, 255, 255, 255]),
        "magenta" | "fuchsia" => Some([255, 0, 255, 255]),
        "gray" | "grey" => Some([128, 128, 128, 255]),
        "silver" => Some([192, 192, 192, 255]),
        "orange" => Some([255, 165, 0, 255]),
        "purple" => Some([128, 0, 128, 255]),
        "pink" => Some([255, 192, 203, 255]),
        "brown" => Some([165, 42, 42, 255]),
        "navy" => Some([0, 0, 128, 255]),
        "teal" => Some([0, 128, 128, 255]),
        _ => None,
    };
    if let Some(c) = named {
        return Ok(c);
    }
    if let Some(hex) = t.strip_prefix('#') {
        let expanded: String = if hex.len() == 3 || hex.len() == 4 {
            hex.chars().flat_map(|c| [c, c]).collect()
        } else {
            hex.to_string()
        };
        if expanded.len() == 6 || expanded.len() == 8 {
            if let Ok(v) = u32::from_str_radix(&expanded, 16) {
                return Ok(if expanded.len() == 6 {
                    [(v >> 16) as u8, (v >> 8) as u8, v as u8, 255]
                } else {
                    [(v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, v as u8]
                });
            }
        }
    }
    if let Some(inner) = t
        .strip_prefix("rgba(")
        .or_else(|| t.strip_prefix("rgb("))
        .and_then(|r| r.strip_suffix(')'))
    {
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        if parts.len() == 3 || parts.len() == 4 {
            let mut out = [0u8, 0, 0, 255];
            for (i, p) in parts.iter().enumerate().take(3) {
                let v: f64 = p
                    .parse()
                    .map_err(|_| EngineError::invalid(format!("Invalid colour `{s}`")))?;
                out[i] = v.clamp(0.0, 255.0).round() as u8;
            }
            if parts.len() == 4 {
                let a: f64 = parts[3]
                    .parse()
                    .map_err(|_| EngineError::invalid(format!("Invalid colour `{s}`")))?;
                out[3] = if a <= 1.0 {
                    (a * 255.0).round() as u8
                } else {
                    a.clamp(0.0, 255.0) as u8
                };
            }
            return Ok(out);
        }
    }
    Err(EngineError::invalid(format!(
        "Unrecognised colour `{s}` (use #rrggbb, #rrggbbaa, rgb(r,g,b), rgba(r,g,b,a), a colour \
         name or transparent)"
    )))
}

/// Formats an RGB triple as `#rrggbb`.
pub fn hex_color(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// Full inspection report: format, dimensions, colour model, EXIF, DPI.
pub fn inspect(bytes: &[u8]) -> EngineResult<Value> {
    let format = detect_format(bytes)?;
    limits::check_size(bytes.len(), limits::MAX_FILE_BYTES, "image file")?;
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    reader.no_limits();
    let (w, h) = reader
        .into_dimensions()
        .map_err(|e| EngineError::malformed(format!("Image header could not be read: {e}")))?;
    let color = if (w as u64) * (h as u64) <= limits::MAX_IMAGE_PIXELS {
        let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
        reader.no_limits();
        reader.decode().ok().map(|i| i.color())
    } else {
        None
    };
    let (color_type, bits, channels, has_alpha) = match color {
        Some(c) => (format!("{c:?}"), c.bits_per_pixel(), c.channel_count(), c.has_alpha()),
        None => ("unknown".into(), 0, 0, false),
    };
    let dpi = crate::meta::read_dpi(bytes, format);
    let exif = crate::meta::read_exif(bytes);
    let frames = if format == ImageFormat::Gif {
        crate::gif::frame_count(bytes).ok()
    } else {
        None
    };
    let g = gcd(w, h);
    Ok(json!({
        "format": format_name(format),
        "mime": format!("image/{}", format_name(format)),
        "width": w,
        "height": h,
        "megapixels": ((w as f64 * h as f64) / 1e6 * 100.0).round() / 100.0,
        "aspect_ratio": match (w.checked_div(g), h.checked_div(g)) { (Some(a), Some(b)) => format!("{a}:{b}"), _ => "?".into() },
        "orientation": if w > h { "landscape" } else if h > w { "portrait" } else { "square" },
        "color_type": color_type,
        "bits_per_pixel": bits,
        "channels": channels,
        "has_alpha": has_alpha,
        "file_bytes": bytes.len(),
        "dpi": dpi,
        "print_size_in": dpi.map(|d| json!({ "width": (w as f64 / d.0 * 100.0).round() / 100.0, "height": (h as f64 / d.1 * 100.0).round() / 100.0 })),
        "frames": frames,
        "exif": exif,
    }))
}

/// Greatest common divisor (used for aspect ratios).
pub fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{jpeg_bytes, png_bytes};
    use image::GenericImageView;

    #[test]
    fn formats_and_colours() {
        assert_eq!(OutFormat::parse("image/jpeg").unwrap(), OutFormat::Jpeg);
        assert_eq!(OutFormat::parse(".PNG").unwrap(), OutFormat::Png);
        assert!(OutFormat::parse("avif").is_err());
        assert!(OutFormat::parse("xyz").is_err());
        assert_eq!(parse_color("#f00").unwrap(), [255, 0, 0, 255]);
        assert_eq!(parse_color("#11223344").unwrap(), [0x11, 0x22, 0x33, 0x44]);
        assert_eq!(parse_color("rgba(1,2,3,0.5)").unwrap(), [1, 2, 3, 128]);
        assert_eq!(parse_color("transparent").unwrap(), [0, 0, 0, 0]);
        assert!(parse_color("#12").is_err());
        assert!(parse_color("rgb(a,b,c)").is_err());
    }

    #[test]
    fn decode_encode_roundtrip() {
        let png = png_bytes(10, 6);
        let d = decode(&png).unwrap();
        assert_eq!(d.format, ImageFormat::Png);
        assert_eq!(d.image.dimensions(), (10, 6));
        for f in [
            OutFormat::Png,
            OutFormat::Jpeg,
            OutFormat::Webp,
            OutFormat::Gif,
            OutFormat::Bmp,
            OutFormat::Ico,
            OutFormat::Tiff,
        ] {
            let bytes = encode(&d.image, &EncodeOptions::with_format(f)).unwrap();
            let back = decode(&bytes).unwrap();
            assert_eq!(back.image.dimensions(), (10, 6), "{f:?}");
        }
        assert!(encode(&d.image, &EncodeOptions { quality: 0, ..Default::default() }).is_err());
        assert!(decode(b"").is_err());
        assert!(decode(b"<svg></svg>").is_err());
        assert!(decode(&png[..20]).is_err());
        let big = image::DynamicImage::new_rgb8(300, 1);
        assert!(encode(&big, &EncodeOptions::with_format(OutFormat::Ico)).is_err());
    }

    #[test]
    fn inspection() {
        let info = inspect(&jpeg_bytes(16, 9)).unwrap();
        assert_eq!(info["format"], "jpeg");
        assert_eq!(info["aspect_ratio"], "16:9");
        assert_eq!(info["has_alpha"], false);
        let info = inspect(&png_bytes(4, 4)).unwrap();
        assert_eq!(info["orientation"], "square");
        assert!(inspect(b"nope").is_err());
    }
}
