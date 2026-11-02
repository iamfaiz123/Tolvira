//! Geometric transforms: resize, crop, rotate, flip, borders, trimming,
//! rounded corners and print-size fitting.

use crate::codec::{self, parse_color, Decoded, EncodeOptions, OutFormat};
use engine_shared::{limits, EngineError, EngineResult};
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use serde::Deserialize;
use serde_json::{json, Value};

/// Common output options accepted by most transform actions.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct OutputOptions {
    /// `png`, `jpeg`, `webp`, `gif`, `bmp`, `ico` or `tiff` (default: the input format).
    pub format: Option<String>,
    /// JPEG/WebP quality 1–100.
    pub quality: Option<u8>,
    /// PNG compression 0–2.
    pub png_level: Option<u8>,
    /// Background used when flattening alpha or padding.
    pub background: Option<String>,
}

impl OutputOptions {
    /// Resolves the output format (falling back to the input format) and validates the options.
    pub fn encode_options(&self, input: image::ImageFormat) -> EngineResult<EncodeOptions> {
        let format = codec::resolve_format(self.format.as_deref(), input)?;
        let bg = self
            .background
            .as_deref()
            .map(parse_color)
            .transpose()?
            .unwrap_or([255, 255, 255, 255]);
        Ok(EncodeOptions {
            format,
            quality: self.quality.unwrap_or(85),
            png_level: self.png_level.unwrap_or(1),
            background: [bg[0], bg[1], bg[2]],
        })
    }
}

/// Parses a resampling filter name.
pub fn filter_from(name: Option<&str>) -> EngineResult<FilterType> {
    Ok(match name.unwrap_or("lanczos3").to_ascii_lowercase().as_str() {
        "nearest" | "pixel" => FilterType::Nearest,
        "triangle" | "bilinear" | "linear" => FilterType::Triangle,
        "catmullrom" | "catmull-rom" | "bicubic" => FilterType::CatmullRom,
        "gaussian" => FilterType::Gaussian,
        "lanczos3" | "lanczos" | "auto" => FilterType::Lanczos3,
        other => return Err(EngineError::invalid(format!("Unknown resampling filter `{other}`"))),
    })
}

/// Computes the target size for `fit` semantics.
/// * `inside`/`contain`: fit within the box keeping aspect ratio (never upscale for `inside`).
/// * `cover`: fill the box keeping aspect ratio, then centre-crop.
/// * `fill`/`stretch`: exact size, aspect ratio ignored.
/// * `scale`: percentage scale.
#[allow(clippy::too_many_arguments)]
pub fn resize(
    d: &Decoded,
    width: Option<u32>,
    height: Option<u32>,
    scale: Option<f64>,
    fit: &str,
    filter: Option<&str>,
    allow_upscale: bool,
    out: &OutputOptions,
) -> EngineResult<Value> {
    let (sw, sh) = d.image.dimensions();
    let filter = filter_from(filter)?;
    let (tw, th) = if let Some(s) = scale {
        engine_shared::error::require_range(s, 0.01, 100.0, "scale")?;
        ((sw as f64 * s).round().max(1.0) as u32, (sh as f64 * s).round().max(1.0) as u32)
    } else {
        match (width, height) {
            (None, None) => {
                return Err(EngineError::invalid("Provide `width`, `height` or `scale`"))
            },
            (Some(w), None) => {
                if w == 0 {
                    return Err(EngineError::invalid("`width` must be > 0"));
                }
                (w, ((sh as f64 * w as f64 / sw as f64).round() as u32).max(1))
            },
            (None, Some(h)) => {
                if h == 0 {
                    return Err(EngineError::invalid("`height` must be > 0"));
                }
                (((sw as f64 * h as f64 / sh as f64).round() as u32).max(1), h)
            },
            (Some(w), Some(h)) => {
                if w == 0 || h == 0 {
                    return Err(EngineError::invalid("`width` and `height` must be > 0"));
                }
                (w, h)
            },
        }
    };
    limits::check_dimensions(tw, th)?;
    let mode = fit.to_ascii_lowercase();
    let result: DynamicImage = match mode.as_str() {
        "fill" | "stretch" | "exact" => d.image.resize_exact(tw, th, filter),
        "cover" => d.image.resize_to_fill(tw, th, filter),
        "inside" | "contain" | "fit" | "" => {
            if !allow_upscale && tw >= sw && th >= sh && mode != "contain" {
                d.image.clone()
            } else {
                d.image.resize(tw, th, filter)
            }
        },
        "pad" | "letterbox" => {
            let inner = d.image.resize(tw, th, filter);
            let bg = out
                .background
                .as_deref()
                .map(parse_color)
                .transpose()?
                .unwrap_or([255, 255, 255, 255]);
            let mut canvas = RgbaImage::from_pixel(tw, th, Rgba(bg));
            let x = (tw - inner.width()) / 2;
            let y = (th - inner.height()) / 2;
            image::imageops::overlay(&mut canvas, &inner.to_rgba8(), x as i64, y as i64);
            DynamicImage::ImageRgba8(canvas)
        },
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown fit mode `{other}` (inside, contain, cover, fill, pad)"
            )))
        },
    };
    let opts = out.encode_options(d.format)?;
    Ok(json!({
        "output": codec::output(&result, &opts)?,
        "source": { "width": sw, "height": sh },
        "scale_x": result.width() as f64 / sw as f64,
        "scale_y": result.height() as f64 / sh as f64,
    }))
}

/// Crops a rectangle (clamped to the image bounds).
pub fn crop(
    d: &Decoded,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    out: &OutputOptions,
) -> EngineResult<Value> {
    let (sw, sh) = d.image.dimensions();
    if x >= sw || y >= sh {
        return Err(EngineError::invalid(format!(
            "Crop origin ({x}, {y}) is outside the {sw}x{sh} image"
        )));
    }
    if width == 0 || height == 0 {
        return Err(EngineError::invalid("Crop width and height must be > 0"));
    }
    let w = width.min(sw - x);
    let h = height.min(sh - y);
    let cropped = d.image.crop_imm(x, y, w, h);
    let opts = out.encode_options(d.format)?;
    Ok(json!({
        "output": codec::output(&cropped, &opts)?,
        "clamped": w != width || h != height,
        "rect": { "x": x, "y": y, "width": w, "height": h }
    }))
}

/// Crops to an aspect ratio (`"1:1"`, `"16:9"`, or a float), centred or
/// anchored (`gravity`: center, north, south, east, west, ...).
pub fn crop_ratio(
    d: &Decoded,
    ratio: &str,
    gravity: &str,
    out: &OutputOptions,
) -> EngineResult<Value> {
    let target = parse_ratio(ratio)?;
    let (sw, sh) = d.image.dimensions();
    let current = sw as f64 / sh as f64;
    let (w, h) = if current > target {
        (((sh as f64) * target).round() as u32, sh)
    } else {
        (sw, ((sw as f64) / target).round() as u32)
    };
    let (w, h) = (w.clamp(1, sw), h.clamp(1, sh));
    let g = gravity.to_ascii_lowercase();
    let x = if g.contains("west") || g.contains("left") {
        0
    } else if g.contains("east") || g.contains("right") {
        sw - w
    } else {
        (sw - w) / 2
    };
    let y = if g.contains("north") || g.contains("top") {
        0
    } else if g.contains("south") || g.contains("bottom") {
        sh - h
    } else {
        (sh - h) / 2
    };
    crop(d, x, y, w, h, out)
}

/// Parses `w:h`, `w/h` or a decimal ratio.
pub fn parse_ratio(s: &str) -> EngineResult<f64> {
    let t = s.trim();
    if let Some((a, b)) = t.split_once([':', '/', 'x']) {
        let a: f64 = a
            .trim()
            .parse()
            .map_err(|_| EngineError::invalid(format!("Invalid ratio `{s}`")))?;
        let b: f64 = b
            .trim()
            .parse()
            .map_err(|_| EngineError::invalid(format!("Invalid ratio `{s}`")))?;
        if a <= 0.0 || b <= 0.0 {
            return Err(EngineError::invalid(format!("Invalid ratio `{s}`")));
        }
        return Ok(a / b);
    }
    let v: f64 = t
        .parse()
        .map_err(|_| EngineError::invalid(format!("Invalid ratio `{s}`")))?;
    engine_shared::error::require_range(v, 0.01, 100.0, "ratio")
}

/// Rotates by any angle (degrees, clockwise). Multiples of 90 are lossless;
/// other angles use bilinear sampling and expand the canvas (`expand`) or keep
/// the original size.
pub fn rotate(
    d: &Decoded,
    angle: f64,
    expand: bool,
    background: Option<&str>,
    out: &OutputOptions,
) -> EngineResult<Value> {
    if !angle.is_finite() {
        return Err(EngineError::invalid("`angle` must be a finite number of degrees"));
    }
    let norm = ((angle % 360.0) + 360.0) % 360.0;
    let bg = background
        .or(out.background.as_deref())
        .map(parse_color)
        .transpose()?
        .unwrap_or([0, 0, 0, 0]);
    let result = if (norm - 0.0).abs() < 1e-9 {
        d.image.clone()
    } else if (norm - 90.0).abs() < 1e-9 {
        d.image.rotate90()
    } else if (norm - 180.0).abs() < 1e-9 {
        d.image.rotate180()
    } else if (norm - 270.0).abs() < 1e-9 {
        d.image.rotate270()
    } else {
        rotate_arbitrary(&d.image.to_rgba8(), norm, expand, bg)?
    };
    let mut opts = out.encode_options(d.format)?;
    // Arbitrary angles introduce transparent corners – prefer an alpha capable format.
    if !opts.format.supports_alpha() && bg[3] < 255 && out.format.is_none() && norm % 90.0 != 0.0 {
        opts.format = OutFormat::Png;
    }
    Ok(json!({ "output": codec::output(&result, &opts)?, "angle": norm }))
}

/// Rotates an RGBA tile by any angle, expanding the canvas (transparent fill).
pub fn rotate_rgba(src: &RgbaImage, degrees: f64) -> EngineResult<RgbaImage> {
    Ok(rotate_arbitrary(src, degrees, true, [0, 0, 0, 0])?.to_rgba8())
}

fn rotate_arbitrary(
    src: &RgbaImage,
    degrees: f64,
    expand: bool,
    bg: [u8; 4],
) -> EngineResult<DynamicImage> {
    let (sw, sh) = src.dimensions();
    let rad = degrees.to_radians();
    let (sin, cos) = rad.sin_cos();
    let (dw, dh) = if expand {
        let w = (sw as f64 * cos.abs() + sh as f64 * sin.abs()).ceil() as u32;
        let h = (sw as f64 * sin.abs() + sh as f64 * cos.abs()).ceil() as u32;
        (w.max(1), h.max(1))
    } else {
        (sw, sh)
    };
    limits::check_dimensions(dw, dh)?;
    let mut out = RgbaImage::from_pixel(dw, dh, Rgba(bg));
    let (cx, cy) = (sw as f64 / 2.0, sh as f64 / 2.0);
    let (dcx, dcy) = (dw as f64 / 2.0, dh as f64 / 2.0);
    for y in 0..dh {
        for x in 0..dw {
            // Inverse mapping: destination -> source (rotate by -angle).
            let dx = x as f64 + 0.5 - dcx;
            let dy = y as f64 + 0.5 - dcy;
            let sx = dx * cos + dy * sin + cx - 0.5;
            let sy = -dx * sin + dy * cos + cy - 0.5;
            if sx < -1.0 || sy < -1.0 || sx > sw as f64 || sy > sh as f64 {
                continue;
            }
            let p = bilinear(src, sx, sy, bg);
            out.put_pixel(x, y, Rgba(p));
        }
    }
    Ok(DynamicImage::ImageRgba8(out))
}

fn bilinear(src: &RgbaImage, x: f64, y: f64, bg: [u8; 4]) -> [u8; 4] {
    let (w, h) = src.dimensions();
    let x0 = x.floor();
    let y0 = y.floor();
    let fx = x - x0;
    let fy = y - y0;
    let sample = |xi: i64, yi: i64| -> [f64; 4] {
        if xi < 0 || yi < 0 || xi >= w as i64 || yi >= h as i64 {
            [bg[0] as f64, bg[1] as f64, bg[2] as f64, bg[3] as f64]
        } else {
            let p = src.get_pixel(xi as u32, yi as u32);
            [p[0] as f64, p[1] as f64, p[2] as f64, p[3] as f64]
        }
    };
    let p00 = sample(x0 as i64, y0 as i64);
    let p10 = sample(x0 as i64 + 1, y0 as i64);
    let p01 = sample(x0 as i64, y0 as i64 + 1);
    let p11 = sample(x0 as i64 + 1, y0 as i64 + 1);
    let mut outp = [0u8; 4];
    for c in 0..4 {
        let top = p00[c] * (1.0 - fx) + p10[c] * fx;
        let bottom = p01[c] * (1.0 - fx) + p11[c] * fx;
        outp[c] = (top * (1.0 - fy) + bottom * fy).round().clamp(0.0, 255.0) as u8;
    }
    outp
}

/// Mirrors horizontally or vertically.
pub fn flip(d: &Decoded, direction: &str, out: &OutputOptions) -> EngineResult<Value> {
    let result = match direction.to_ascii_lowercase().as_str() {
        "horizontal" | "h" | "x" => d.image.fliph(),
        "vertical" | "v" | "y" => d.image.flipv(),
        "both" => d.image.fliph().flipv(),
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown flip direction `{other}` (horizontal, vertical, both)"
            )))
        },
    };
    let opts = out.encode_options(d.format)?;
    Ok(json!({ "output": codec::output(&result, &opts)? }))
}

/// Adds a solid border of `size` px (or per-side sizes).
pub fn add_border(
    d: &Decoded,
    size: u32,
    color: &str,
    top: Option<u32>,
    right: Option<u32>,
    bottom: Option<u32>,
    left: Option<u32>,
    out: &OutputOptions,
) -> EngineResult<Value> {
    let c = parse_color(color)?;
    let (t, r, b, l) = (
        top.unwrap_or(size),
        right.unwrap_or(size),
        bottom.unwrap_or(size),
        left.unwrap_or(size),
    );
    if t + r + b + l == 0 {
        return Err(EngineError::invalid("Border size must be > 0"));
    }
    let (sw, sh) = d.image.dimensions();
    let (nw, nh) = (sw + l + r, sh + t + b);
    limits::check_dimensions(nw, nh)?;
    let mut canvas = RgbaImage::from_pixel(nw, nh, Rgba(c));
    image::imageops::overlay(&mut canvas, &d.image.to_rgba8(), l as i64, t as i64);
    let opts = out.encode_options(d.format)?;
    Ok(json!({ "output": codec::output(&DynamicImage::ImageRgba8(canvas), &opts)? }))
}

/// Removes a uniform border (auto-trim) by comparing edge pixels to the
/// corner colour within `tolerance` (0-255). Transparent borders are trimmed
/// too.
pub fn trim_border(d: &Decoded, tolerance: u8, out: &OutputOptions) -> EngineResult<Value> {
    let img = d.image.to_rgba8();
    let (w, h) = img.dimensions();
    let reference = *img.get_pixel(0, 0);
    let same = |p: &Rgba<u8>| -> bool {
        if reference[3] == 0 && p[3] == 0 {
            return true;
        }
        (0..4).all(|c| (p[c] as i32 - reference[c] as i32).abs() <= tolerance as i32)
    };
    let mut top = 0;
    while top < h && (0..w).all(|x| same(img.get_pixel(x, top))) {
        top += 1;
    }
    if top == h {
        return Err(EngineError::invalid(
            "The whole image is a uniform colour; nothing would remain after trimming",
        ));
    }
    let mut bottom = h - 1;
    while bottom > top && (0..w).all(|x| same(img.get_pixel(x, bottom))) {
        bottom -= 1;
    }
    let mut left = 0;
    while left < w && (top..=bottom).all(|y| same(img.get_pixel(left, y))) {
        left += 1;
    }
    let mut right = w - 1;
    while right > left && (top..=bottom).all(|y| same(img.get_pixel(right, y))) {
        right -= 1;
    }
    let cw = right - left + 1;
    let ch = bottom - top + 1;
    let cropped = d.image.crop_imm(left, top, cw, ch);
    let opts = out.encode_options(d.format)?;
    Ok(json!({
        "output": codec::output(&cropped, &opts)?,
        "removed": { "top": top, "bottom": h - 1 - bottom, "left": left, "right": w - 1 - right },
        "rect": { "x": left, "y": top, "width": cw, "height": ch },
    }))
}

/// Rounds the corners (radius in px, or `"50%"` for a circle/ellipse).
pub fn round_corners(d: &Decoded, radius: &str, out: &OutputOptions) -> EngineResult<Value> {
    let mut img = d.image.to_rgba8();
    let (w, h) = img.dimensions();
    let r: f64 = if let Some(p) = radius.trim().strip_suffix('%') {
        let pct: f64 = p.trim().parse().map_err(|_| EngineError::invalid("Invalid radius"))?;
        engine_shared::error::require_range(pct, 0.0, 50.0, "radius percent")?;
        (w.min(h) as f64) * pct / 100.0
    } else {
        let px: f64 = radius.trim().trim_end_matches("px").parse().map_err(|_| {
            EngineError::invalid("Invalid radius (use pixels or a percentage like 50%)")
        })?;
        engine_shared::error::require_range(px, 0.0, (w.min(h) / 2) as f64, "radius")?
    };
    if r > 0.0 {
        for y in 0..h {
            for x in 0..w {
                let px = x as f64 + 0.5;
                let py = y as f64 + 0.5;
                // Distance to nearest corner centre if inside a corner square.
                let cx = if px < r {
                    r
                } else if px > w as f64 - r {
                    w as f64 - r
                } else {
                    px
                };
                let cy = if py < r {
                    r
                } else if py > h as f64 - r {
                    h as f64 - r
                } else {
                    py
                };
                let dist = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
                if dist > r {
                    let coverage = (r + 0.5 - dist).clamp(0.0, 1.0);
                    let p = img.get_pixel_mut(x, y);
                    p[3] = (p[3] as f64 * coverage).round() as u8;
                }
            }
        }
    }
    let mut opts = out.encode_options(d.format)?;
    if !opts.format.supports_alpha() && out.format.is_none() {
        opts.format = OutFormat::Png;
    }
    Ok(json!({ "output": codec::output(&DynamicImage::ImageRgba8(img), &opts)?, "radius_px": r }))
}

/// Resizes/crops to a physical print size (mm or inches) at a DPI, e.g.
/// passport photos (35x45 mm @ 300 dpi), and embeds the DPI.
pub fn fit_print_size(
    d: &Decoded,
    width: f64,
    height: f64,
    unit: &str,
    dpi: f64,
    mode: &str,
    out: &OutputOptions,
) -> EngineResult<Value> {
    engine_shared::error::require_positive(width, "width")?;
    engine_shared::error::require_positive(height, "height")?;
    engine_shared::error::require_range(dpi, 36.0, 2400.0, "dpi")?;
    let inches = |v: f64| -> EngineResult<f64> {
        Ok(match unit {
            "mm" => v / 25.4,
            "cm" => v / 2.54,
            "in" | "inch" | "inches" => v,
            "px" => v / dpi,
            other => return Err(EngineError::invalid(format!("Unknown unit `{other}`"))),
        })
    };
    let pw = (inches(width)? * dpi).round().max(1.0) as u32;
    let ph = (inches(height)? * dpi).round().max(1.0) as u32;
    limits::check_dimensions(pw, ph)?;
    let resized = match mode {
        "pad" => {
            let inner = d.image.resize(pw, ph, FilterType::Lanczos3);
            let bg = out
                .background
                .as_deref()
                .map(parse_color)
                .transpose()?
                .unwrap_or([255, 255, 255, 255]);
            let mut canvas = RgbaImage::from_pixel(pw, ph, Rgba(bg));
            image::imageops::overlay(
                &mut canvas,
                &inner.to_rgba8(),
                ((pw - inner.width()) / 2) as i64,
                ((ph - inner.height()) / 2) as i64,
            );
            DynamicImage::ImageRgba8(canvas)
        },
        _ => d.image.resize_to_fill(pw, ph, FilterType::Lanczos3),
    };
    let opts = out.encode_options(d.format)?;
    let bytes = codec::encode(&resized, &opts)?;
    let with_dpi = crate::meta::set_dpi(&bytes, dpi)?;
    Ok(json!({
        "output": with_dpi["output"],
        "pixels": { "width": pw, "height": ph },
        "dpi": dpi,
        "physical": { "width": width, "height": height, "unit": unit },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{decode_output, png_bytes};

    fn d() -> Decoded {
        codec::decode(&png_bytes(40, 20)).unwrap()
    }

    #[test]
    fn resize_modes() {
        let o = OutputOptions::default();
        let r = resize(&d(), Some(20), None, None, "inside", None, true, &o).unwrap();
        assert_eq!(
            (r["output"]["width"].clone(), r["output"]["height"].clone()),
            (json!(20), json!(10))
        );
        let r = resize(&d(), Some(10), Some(10), None, "cover", Some("nearest"), true, &o).unwrap();
        assert_eq!(r["output"]["width"], 10);
        let r = resize(&d(), Some(10), Some(10), None, "fill", None, true, &o).unwrap();
        assert_eq!(r["output"]["height"], 10);
        let r = resize(&d(), Some(10), Some(10), None, "pad", None, true, &o).unwrap();
        assert_eq!(r["output"]["height"], 10);
        let r = resize(&d(), None, None, Some(0.5), "inside", None, true, &o).unwrap();
        assert_eq!(r["output"]["width"], 20);
        // no upscale when not allowed
        let r = resize(&d(), Some(400), None, None, "inside", None, false, &o).unwrap();
        assert_eq!(r["output"]["width"], 40);
        assert!(resize(&d(), None, None, None, "inside", None, true, &o).is_err());
        assert!(resize(&d(), Some(0), None, None, "inside", None, true, &o).is_err());
        assert!(resize(&d(), Some(10), None, None, "weird", None, true, &o).is_err());
        assert!(resize(&d(), Some(10), None, None, "inside", Some("magic"), true, &o).is_err());
        assert!(resize(&d(), Some(100_000), Some(100_000), None, "fill", None, true, &o).is_err());
    }

    #[test]
    fn crop_rotate_flip() {
        let o = OutputOptions::default();
        let r = crop(&d(), 5, 5, 100, 100, &o).unwrap();
        assert_eq!(r["clamped"], true);
        assert_eq!(r["rect"]["width"], 35);
        assert!(crop(&d(), 50, 0, 1, 1, &o).is_err());
        let r = crop_ratio(&d(), "1:1", "west", &o).unwrap();
        assert_eq!(r["rect"], json!({ "x": 0, "y": 0, "width": 20, "height": 20 }));
        assert!(parse_ratio("0:1").is_err());
        let r = rotate(&d(), 90.0, true, None, &o).unwrap();
        assert_eq!(
            (r["output"]["width"].clone(), r["output"]["height"].clone()),
            (json!(20), json!(40))
        );
        let r = rotate(&d(), 45.0, true, None, &o).unwrap();
        assert!(r["output"]["width"].as_u64().unwrap() > 40);
        assert_eq!(r["output"]["mime"], "image/png");
        let r = rotate(&d(), -30.0, false, Some("white"), &o).unwrap();
        assert_eq!(r["output"]["width"], 40);
        assert!(rotate(&d(), f64::NAN, true, None, &o).is_err());
        assert!(flip(&d(), "horizontal", &o).is_ok());
        assert!(flip(&d(), "diagonal", &o).is_err());
    }

    #[test]
    fn borders_and_corners() {
        let o = OutputOptions::default();
        let r = add_border(&d(), 3, "red", None, None, None, None, &o).unwrap();
        assert_eq!(r["output"]["width"], 46);
        let bordered = decode_output(&r);
        let t = trim_border(&bordered, 0, &o).unwrap();
        assert_eq!(t["rect"]["width"], 40);
        assert_eq!(t["removed"]["top"], 3);
        let uniform = codec::decode(
            &codec::encode(
                &DynamicImage::ImageRgba8(RgbaImage::from_pixel(5, 5, Rgba([1, 2, 3, 255]))),
                &EncodeOptions::default(),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(trim_border(&uniform, 0, &o).is_err());
        assert!(add_border(&d(), 0, "red", None, None, None, None, &o).is_err());
        let r = round_corners(&d(), "50%", &o).unwrap();
        assert_eq!(r["output"]["mime"], "image/png");
        let img = decode_output(&r).image.to_rgba8();
        assert_eq!(img.get_pixel(0, 0)[3], 0);
        assert_eq!(img.get_pixel(20, 10)[3], 255);
        assert!(round_corners(&d(), "x", &o).is_err());
        let p = fit_print_size(&d(), 35.0, 45.0, "mm", 300.0, "cover", &o).unwrap();
        assert_eq!(p["pixels"]["width"], 413);
        assert!(fit_print_size(&d(), 35.0, 45.0, "furlong", 300.0, "cover", &o).is_err());
    }
}
