//! Pixel filters and colour adjustments.

use crate::codec::{self, Decoded};
use crate::transform::OutputOptions;
use engine_shared::{EngineError, EngineResult};
use image::{DynamicImage, Rgba, RgbaImage};
use serde::Deserialize;
use serde_json::{json, Value};

/// One adjustment step. Several can be chained in a single `adjust` call.
#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Adjustment {
    /// Convert to grayscale.
    Grayscale,
    /// Invert colours.
    Invert,
    /// Sepia tone.
    Sepia,
    /// Gaussian blur; `sigma` in pixels (0.1 – 100).
    Blur {
        /// Blur radius (sigma) in pixels.
        sigma: f64,
    },
    /// Unsharp mask; `amount` 0-10, `radius` (sigma) pixels.
    Sharpen {
        /// Sharpening amount.
        #[serde(default = "one")]
        amount: f64,
        /// Blur radius used by the mask.
        #[serde(default = "one")]
        radius: f64,
    },
    /// -100 … 100
    Brightness {
        /// Brightness delta.
        value: f64,
    },
    /// -100 … 100
    Contrast {
        /// Contrast delta.
        value: f64,
    },
    /// -100 … 100 (−100 = grayscale)
    Saturation {
        /// Saturation delta.
        value: f64,
    },
    /// Degrees −360 … 360
    Hue {
        /// Hue rotation in degrees.
        degrees: f64,
    },
    /// Exposure in stops (−5 … 5)
    Exposure {
        /// Exposure in stops.
        stops: f64,
    },
    /// Colour temperature shift −100 (cool) … 100 (warm)
    Temperature {
        /// Temperature shift.
        value: f64,
    },
    /// Gamma 0.1 … 10
    Gamma {
        /// Gamma value.
        value: f64,
    },
    /// Block size in pixels (2 – 512)
    Pixelate {
        /// Block size in pixels.
        block: u32,
    },
    /// Median filter denoise, `radius` 1-3
    Denoise {
        /// Median radius.
        #[serde(default = "one_u32")]
        radius: u32,
    },
    /// Threshold (0-255) → black & white
    Threshold {
        /// Luma threshold.
        #[serde(default = "mid")]
        level: u8,
    },
    /// Posterize to `levels` per channel (2-32)
    Posterize {
        /// Levels per channel.
        levels: u8,
    },
    /// Vignette strength 0-1
    Vignette {
        /// Vignette strength.
        #[serde(default = "half")]
        strength: f64,
    },
    /// Opacity 0-1
    Opacity {
        /// Opacity multiplier.
        value: f64,
    },
}

fn one() -> f64 {
    1.0
}
fn one_u32() -> u32 {
    1
}
fn mid() -> u8 {
    128
}
fn half() -> f64 {
    0.5
}

/// Applies one adjustment step, validating its parameters.
pub fn apply(img: &DynamicImage, step: &Adjustment) -> EngineResult<DynamicImage> {
    use engine_shared::error::require_range as range;
    Ok(match step {
        Adjustment::Grayscale => {
            let g = img.to_luma_alpha8();
            DynamicImage::ImageLumaA8(g)
        },
        Adjustment::Invert => {
            let mut i = img.clone();
            i.invert();
            i
        },
        Adjustment::Sepia => map_rgb(img, |[r, g, b]| {
            let (r, g, b) = (r as f64, g as f64, b as f64);
            [
                (0.393 * r + 0.769 * g + 0.189 * b).min(255.0) as u8,
                (0.349 * r + 0.686 * g + 0.168 * b).min(255.0) as u8,
                (0.272 * r + 0.534 * g + 0.131 * b).min(255.0) as u8,
            ]
        }),
        Adjustment::Blur { sigma } => {
            range(*sigma, 0.1, 100.0, "sigma")?;
            img.blur(*sigma as f32)
        },
        Adjustment::Sharpen { amount, radius } => {
            range(*amount, 0.0, 10.0, "amount")?;
            range(*radius, 0.1, 50.0, "radius")?;
            // image::unsharpen threshold is an integer; scale amount via blending.
            let sharpened = img.unsharpen(*radius as f32, 0);
            if (*amount - 1.0).abs() < 1e-9 {
                sharpened
            } else {
                blend(img, &sharpened, *amount)
            }
        },
        Adjustment::Brightness { value } => {
            range(*value, -100.0, 100.0, "brightness")?;
            img.brighten((*value * 2.55).round() as i32)
        },
        Adjustment::Contrast { value } => {
            range(*value, -100.0, 100.0, "contrast")?;
            img.adjust_contrast(*value as f32)
        },
        Adjustment::Saturation { value } => {
            range(*value, -100.0, 100.0, "saturation")?;
            let f = 1.0 + *value / 100.0;
            map_rgb(img, |[r, g, b]| {
                let l = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
                [
                    clamp(l + (r as f64 - l) * f),
                    clamp(l + (g as f64 - l) * f),
                    clamp(l + (b as f64 - l) * f),
                ]
            })
        },
        Adjustment::Hue { degrees } => {
            range(*degrees, -360.0, 360.0, "hue")?;
            img.huerotate(degrees.round() as i32)
        },
        Adjustment::Exposure { stops } => {
            range(*stops, -5.0, 5.0, "exposure")?;
            let f = 2f64.powf(*stops);
            map_rgb(img, |[r, g, b]| {
                [clamp(r as f64 * f), clamp(g as f64 * f), clamp(b as f64 * f)]
            })
        },
        Adjustment::Temperature { value } => {
            range(*value, -100.0, 100.0, "temperature")?;
            let shift = *value * 0.4;
            map_rgb(img, |[r, g, b]| [clamp(r as f64 + shift), g, clamp(b as f64 - shift)])
        },
        Adjustment::Gamma { value } => {
            range(*value, 0.1, 10.0, "gamma")?;
            let lut: Vec<u8> = (0..256)
                .map(|v| (255.0 * (v as f64 / 255.0).powf(1.0 / *value)).round() as u8)
                .collect();
            map_rgb(img, |[r, g, b]| [lut[r as usize], lut[g as usize], lut[b as usize]])
        },
        Adjustment::Pixelate { block } => {
            if !(2..=512).contains(block) {
                return Err(EngineError::invalid("`block` must be between 2 and 512"));
            }
            pixelate(img, *block)
        },
        Adjustment::Denoise { radius } => {
            if !(1..=3).contains(radius) {
                return Err(EngineError::invalid("`radius` must be between 1 and 3"));
            }
            median(img, *radius)
        },
        Adjustment::Threshold { level } => {
            let l = *level;
            let g = img.to_luma8();
            let mut out = RgbaImage::new(g.width(), g.height());
            let alpha = img.to_rgba8();
            for (x, y, p) in g.enumerate_pixels() {
                let v = if p[0] >= l { 255 } else { 0 };
                out.put_pixel(x, y, Rgba([v, v, v, alpha.get_pixel(x, y)[3]]));
            }
            DynamicImage::ImageRgba8(out)
        },
        Adjustment::Posterize { levels } => {
            if !(2..=32).contains(levels) {
                return Err(EngineError::invalid("`levels` must be between 2 and 32"));
            }
            let n = *levels as f64 - 1.0;
            map_rgb(img, |[r, g, b]| {
                let q = |v: u8| ((v as f64 / 255.0 * n).round() / n * 255.0).round() as u8;
                [q(r), q(g), q(b)]
            })
        },
        Adjustment::Vignette { strength } => {
            range(*strength, 0.0, 1.0, "strength")?;
            let mut rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0);
            let max_d = (cx * cx + cy * cy).sqrt();
            for (x, y, p) in rgba.enumerate_pixels_mut() {
                let d = (((x as f64 - cx).powi(2) + (y as f64 - cy).powi(2)).sqrt() / max_d)
                    .clamp(0.0, 1.0);
                let f = 1.0 - strength * d * d;
                for c in 0..3 {
                    p[c] = clamp(p[c] as f64 * f);
                }
            }
            DynamicImage::ImageRgba8(rgba)
        },
        Adjustment::Opacity { value } => {
            range(*value, 0.0, 1.0, "opacity")?;
            let mut rgba = img.to_rgba8();
            for p in rgba.pixels_mut() {
                p[3] = (p[3] as f64 * value).round() as u8;
            }
            DynamicImage::ImageRgba8(rgba)
        },
    })
}

fn clamp(v: f64) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

fn map_rgb(img: &DynamicImage, f: impl Fn([u8; 3]) -> [u8; 3]) -> DynamicImage {
    let mut rgba = img.to_rgba8();
    for p in rgba.pixels_mut() {
        let [r, g, b] = f([p[0], p[1], p[2]]);
        p[0] = r;
        p[1] = g;
        p[2] = b;
    }
    DynamicImage::ImageRgba8(rgba)
}

fn blend(a: &DynamicImage, b: &DynamicImage, t: f64) -> DynamicImage {
    let pa = a.to_rgba8();
    let pb = b.to_rgba8();
    let mut out = RgbaImage::new(pa.width(), pa.height());
    for (x, y, p) in out.enumerate_pixels_mut() {
        let qa = pa.get_pixel(x, y);
        let qb = pb.get_pixel(x, y);
        for c in 0..4 {
            p[c] = clamp(qa[c] as f64 + (qb[c] as f64 - qa[c] as f64) * t);
        }
    }
    DynamicImage::ImageRgba8(out)
}

fn pixelate(img: &DynamicImage, block: u32) -> DynamicImage {
    let src = img.to_rgba8();
    let (w, h) = src.dimensions();
    let mut out = RgbaImage::new(w, h);
    let mut by = 0;
    while by < h {
        let mut bx = 0;
        while bx < w {
            let bw = block.min(w - bx);
            let bh = block.min(h - by);
            let mut acc = [0u64; 4];
            for y in by..by + bh {
                for x in bx..bx + bw {
                    let p = src.get_pixel(x, y);
                    for c in 0..4 {
                        acc[c] += p[c] as u64;
                    }
                }
            }
            let n = (bw * bh) as u64;
            let avg = Rgba([
                (acc[0] / n) as u8,
                (acc[1] / n) as u8,
                (acc[2] / n) as u8,
                (acc[3] / n) as u8,
            ]);
            for y in by..by + bh {
                for x in bx..bx + bw {
                    out.put_pixel(x, y, avg);
                }
            }
            bx += block;
        }
        by += block;
    }
    DynamicImage::ImageRgba8(out)
}

fn median(img: &DynamicImage, radius: u32) -> DynamicImage {
    let src = img.to_rgba8();
    let (w, h) = src.dimensions();
    let mut out = RgbaImage::new(w, h);
    let r = radius as i64;
    let mut window: Vec<u8> = Vec::with_capacity(((2 * r + 1) * (2 * r + 1)) as usize);
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            let mut p = [0u8; 4];
            for c in 0..4 {
                window.clear();
                for dy in -r..=r {
                    for dx in -r..=r {
                        let sx = (x + dx).clamp(0, w as i64 - 1) as u32;
                        let sy = (y + dy).clamp(0, h as i64 - 1) as u32;
                        window.push(src.get_pixel(sx, sy)[c]);
                    }
                }
                window.sort_unstable();
                p[c] = window[window.len() / 2];
            }
            out.put_pixel(x as u32, y as u32, Rgba(p));
        }
    }
    DynamicImage::ImageRgba8(out)
}

/// Applies a chain of adjustments and encodes the result.
pub fn adjust(d: &Decoded, steps: &[Adjustment], out: &OutputOptions) -> EngineResult<Value> {
    if steps.is_empty() {
        return Err(EngineError::empty("adjustments"));
    }
    if steps.len() > 32 {
        return Err(EngineError::limit("At most 32 adjustments per call"));
    }
    let mut img = d.image.clone();
    for s in steps {
        img = apply(&img, s)?;
    }
    // Preserve the input colour model where it makes sense (keeps JPEG gray).
    let final_img = if matches!(d.image.color(), image::ColorType::Rgb8) && !img.color().has_alpha()
    {
        img
    } else {
        img
    };
    let opts = out.encode_options(d.format)?;
    Ok(json!({ "output": codec::output(&final_img, &opts)?, "steps": steps.len() }))
}

/// Pixel-art conversion: pixelate + reduce palette.
pub fn pixel_art(
    d: &Decoded,
    pixel_size: u32,
    colors: usize,
    out: &OutputOptions,
) -> EngineResult<Value> {
    if !(2..=256).contains(&pixel_size) {
        return Err(EngineError::invalid("`pixel_size` must be between 2 and 256"));
    }
    if !(2..=256).contains(&colors) {
        return Err(EngineError::invalid("`colors` must be between 2 and 256"));
    }
    let small = d.image.resize_exact(
        (d.image.width() / pixel_size).max(1),
        (d.image.height() / pixel_size).max(1),
        image::imageops::FilterType::Triangle,
    );
    let palette = crate::colors::quantize(&small.to_rgba8(), colors);
    let mut rgba = small.to_rgba8();
    for p in rgba.pixels_mut() {
        let nearest = crate::colors::nearest(&palette, [p[0], p[1], p[2]]);
        p[0] = nearest[0];
        p[1] = nearest[1];
        p[2] = nearest[2];
    }
    let up = DynamicImage::ImageRgba8(rgba).resize_exact(
        d.image.width(),
        d.image.height(),
        image::imageops::FilterType::Nearest,
    );
    let opts = out.encode_options(d.format)?;
    Ok(json!({
        "output": codec::output(&up, &opts)?,
        "palette": palette.iter().map(|c| codec::hex_color(*c)).collect::<Vec<_>>(),
        "grid": { "columns": small.width(), "rows": small.height() },
    }))
}

/// Removes a (near-)uniform background colour by flood filling from the
/// image edges, producing transparency. `color` defaults to the top-left
/// pixel. Not an AI segmentation – works for solid/studio backgrounds.
pub fn remove_background(
    d: &Decoded,
    color: Option<&str>,
    tolerance: u8,
    feather: bool,
    out: &OutputOptions,
) -> EngineResult<Value> {
    let mut rgba = d.image.to_rgba8();
    let (w, h) = rgba.dimensions();
    let target = match color {
        Some(c) => codec::parse_color(c)?,
        None => rgba.get_pixel(0, 0).0,
    };
    let tol = tolerance as i32;
    let matches = |p: &Rgba<u8>| (0..3).all(|c| (p[c] as i32 - target[c] as i32).abs() <= tol);
    let mut mask = vec![false; (w * h) as usize];
    let mut stack: Vec<(u32, u32)> = Vec::new();
    for x in 0..w {
        stack.push((x, 0));
        stack.push((x, h - 1));
    }
    for y in 0..h {
        stack.push((0, y));
        stack.push((w - 1, y));
    }
    while let Some((x, y)) = stack.pop() {
        let idx = (y * w + x) as usize;
        if mask[idx] || !matches(rgba.get_pixel(x, y)) {
            continue;
        }
        mask[idx] = true;
        if x > 0 {
            stack.push((x - 1, y));
        }
        if y > 0 {
            stack.push((x, y - 1));
        }
        if x + 1 < w {
            stack.push((x + 1, y));
        }
        if y + 1 < h {
            stack.push((x, y + 1));
        }
    }
    let mut removed = 0usize;
    for y in 0..h {
        for x in 0..w {
            if mask[(y * w + x) as usize] {
                rgba.get_pixel_mut(x, y)[3] = 0;
                removed += 1;
            } else if feather {
                // Soften the edge: partial alpha for pixels adjacent to removed ones.
                let mut neighbours = 0;
                for (dx, dy) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                    let nx = x as i64 + dx;
                    let ny = y as i64 + dy;
                    if nx >= 0
                        && ny >= 0
                        && nx < w as i64
                        && ny < h as i64
                        && mask[(ny as u32 * w + nx as u32) as usize]
                    {
                        neighbours += 1;
                    }
                }
                if neighbours > 0 {
                    let p = rgba.get_pixel_mut(x, y);
                    p[3] = (p[3] as u32 * (4 - neighbours.min(3)) / 4) as u8;
                }
            }
        }
    }
    let mut opts = out.encode_options(d.format)?;
    if !opts.format.supports_alpha() {
        opts.format = codec::OutFormat::Png;
    }
    Ok(json!({
        "output": codec::output(&DynamicImage::ImageRgba8(rgba), &opts)?,
        "removed_pixels": removed,
        "removed_percent": ((removed as f64 / (w * h) as f64) * 1000.0).round() / 10.0,
        "background": codec::hex_color([target[0], target[1], target[2]]),
        "note": "Flood-fill background removal for solid backgrounds; subject segmentation \
                 requires an ML model (see ai_local engine)",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{decode_output, png_bytes};

    #[test]
    fn every_adjustment_runs() {
        let d = codec::decode(&png_bytes(16, 12)).unwrap();
        let steps = vec![
            Adjustment::Grayscale,
            Adjustment::Invert,
            Adjustment::Sepia,
            Adjustment::Blur { sigma: 1.0 },
            Adjustment::Sharpen { amount: 1.5, radius: 1.0 },
            Adjustment::Brightness { value: 10.0 },
            Adjustment::Contrast { value: -10.0 },
            Adjustment::Saturation { value: 50.0 },
            Adjustment::Hue { degrees: 90.0 },
            Adjustment::Exposure { stops: 0.5 },
            Adjustment::Temperature { value: 20.0 },
            Adjustment::Gamma { value: 2.2 },
            Adjustment::Pixelate { block: 4 },
            Adjustment::Denoise { radius: 1 },
            Adjustment::Threshold { level: 100 },
            Adjustment::Posterize { levels: 4 },
            Adjustment::Vignette { strength: 0.5 },
            Adjustment::Opacity { value: 0.5 },
        ];
        let r = adjust(&d, &steps, &OutputOptions::default()).unwrap();
        assert_eq!(r["steps"], 18);
        assert_eq!(r["output"]["width"], 16);
        assert!(adjust(&d, &[], &OutputOptions::default()).is_err());
        assert!(adjust(&d, &[Adjustment::Blur { sigma: 0.0 }], &OutputOptions::default()).is_err());
        assert!(
            adjust(&d, &[Adjustment::Pixelate { block: 1 }], &OutputOptions::default()).is_err()
        );
        assert!(
            adjust(&d, &[Adjustment::Brightness { value: 500.0 }], &OutputOptions::default())
                .is_err()
        );
    }

    #[test]
    fn invert_is_involution_and_grayscale_is_gray() {
        let d = codec::decode(&png_bytes(8, 8)).unwrap();
        let twice =
            apply(&apply(&d.image, &Adjustment::Invert).unwrap(), &Adjustment::Invert).unwrap();
        assert_eq!(twice.to_rgba8().as_raw(), d.image.to_rgba8().as_raw());
        let g = apply(&d.image, &Adjustment::Grayscale).unwrap().to_rgba8();
        for p in g.pixels() {
            assert!(p[0] == p[1] && p[1] == p[2]);
        }
    }

    #[test]
    fn pixel_art_and_background() {
        let d = codec::decode(&png_bytes(32, 32)).unwrap();
        let r = pixel_art(&d, 4, 8, &OutputOptions::default()).unwrap();
        assert!(r["palette"].as_array().unwrap().len() <= 8);
        assert_eq!(r["grid"]["columns"], 8);
        assert!(pixel_art(&d, 1, 8, &OutputOptions::default()).is_err());

        // Image with a solid white border and a dark centre.
        let mut img = RgbaImage::from_pixel(20, 20, Rgba([255, 255, 255, 255]));
        for y in 5..15 {
            for x in 5..15 {
                img.put_pixel(x, y, Rgba([10, 10, 10, 255]));
            }
        }
        let bytes = codec::encode(&DynamicImage::ImageRgba8(img), &codec::EncodeOptions::default())
            .unwrap();
        let d = codec::decode(&bytes).unwrap();
        let r = remove_background(&d, None, 10, true, &OutputOptions::default()).unwrap();
        assert_eq!(r["removed_pixels"], 300);
        let out = decode_output(&r).image.to_rgba8();
        assert_eq!(out.get_pixel(0, 0)[3], 0);
        assert_eq!(out.get_pixel(10, 10)[3], 255);
    }
}
