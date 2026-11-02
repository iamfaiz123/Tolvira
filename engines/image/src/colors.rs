//! Colour analysis: palettes, dominant / average colour, colour picking and
//! ASCII art.

use crate::codec::{self, hex_color, Decoded};
use engine_shared::{EngineError, EngineResult};
use image::{GenericImageView, RgbaImage};
use serde_json::{json, Value};

/// Median-cut palette quantisation over opaque pixels.
pub fn quantize(img: &RgbaImage, colors: usize) -> Vec<[u8; 3]> {
    let mut pixels: Vec<[u8; 3]> = img
        .pixels()
        .filter(|p| p[3] > 16)
        .map(|p| [p[0], p[1], p[2]])
        .collect();
    if pixels.is_empty() {
        return vec![[0, 0, 0]];
    }
    // Subsample very large images for speed.
    if pixels.len() > 200_000 {
        let step = pixels.len() / 200_000 + 1;
        pixels = pixels.into_iter().step_by(step).collect();
    }
    let mut buckets: Vec<Vec<[u8; 3]>> = vec![pixels];
    while buckets.len() < colors {
        // Split the bucket with the largest range.
        let (idx, channel) = buckets
            .iter()
            .enumerate()
            .filter(|(_, b)| b.len() > 1)
            .map(|(i, b)| {
                let mut min = [255u8; 3];
                let mut max = [0u8; 3];
                for p in b {
                    for c in 0..3 {
                        min[c] = min[c].min(p[c]);
                        max[c] = max[c].max(p[c]);
                    }
                }
                let ranges = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
                let ch = (0..3).max_by_key(|&c| ranges[c]).unwrap_or(0);
                (i, ch, ranges[ch])
            })
            .max_by_key(|&(_, _, r)| r)
            .map(|(i, c, _)| (i, c))
            .unwrap_or((usize::MAX, 0));
        if idx == usize::MAX {
            break;
        }
        let mut bucket = buckets.swap_remove(idx);
        bucket.sort_unstable_by_key(|p| p[channel]);
        let mid = bucket.len() / 2;
        let right = bucket.split_off(mid);
        buckets.push(bucket);
        buckets.push(right);
    }
    let mut palette: Vec<([u8; 3], usize)> = buckets
        .iter()
        .filter(|b| !b.is_empty())
        .map(|b| {
            let n = b.len() as u64;
            let sum = b.iter().fold([0u64; 3], |acc, p| {
                [acc[0] + p[0] as u64, acc[1] + p[1] as u64, acc[2] + p[2] as u64]
            });
            ([(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8], b.len())
        })
        .collect();
    palette.sort_by_key(|b| std::cmp::Reverse(b.1));
    palette.into_iter().map(|(c, _)| c).collect()
}

/// Nearest palette entry to `c` by Euclidean RGB distance.
pub fn nearest(palette: &[[u8; 3]], c: [u8; 3]) -> [u8; 3] {
    *palette
        .iter()
        .min_by_key(|p| (0..3).map(|i| (p[i] as i32 - c[i] as i32).pow(2)).sum::<i32>())
        .unwrap_or(&c)
}

fn rgb_to_hsl(c: [u8; 3]) -> (f64, f64, f64) {
    let r = c[0] as f64 / 255.0;
    let g = c[1] as f64 / 255.0;
    let b = c[2] as f64 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < 1e-9 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

fn color_json(c: [u8; 3], share: Option<f64>) -> Value {
    let (h, s, l) = rgb_to_hsl(c);
    let lum = 0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64;
    json!({
        "hex": hex_color(c),
        "rgb": { "r": c[0], "g": c[1], "b": c[2] },
        "hsl": { "h": h.round(), "s": (s * 100.0).round(), "l": (l * 100.0).round() },
        "luminance": lum.round(),
        "is_dark": lum < 128.0,
        "share": share,
    })
}

/// Extracts a palette of `count` colours with their pixel share.
pub fn palette(d: &Decoded, count: usize) -> EngineResult<Value> {
    if !(1..=64).contains(&count) {
        return Err(EngineError::invalid("`count` must be between 1 and 64"));
    }
    let rgba = d.image.to_rgba8();
    let pal = quantize(&rgba, count);
    // Assign each pixel to its nearest palette entry to compute shares.
    let mut counts = vec![0usize; pal.len()];
    let mut total = 0usize;
    let step = ((rgba.width() as usize * rgba.height() as usize) / 100_000).max(1);
    for (i, p) in rgba.pixels().enumerate() {
        if i % step != 0 || p[3] <= 16 {
            continue;
        }
        let idx = pal
            .iter()
            .enumerate()
            .min_by_key(|(_, q)| (0..3).map(|c| (q[c] as i32 - p[c] as i32).pow(2)).sum::<i32>())
            .map(|(i, _)| i)
            .unwrap_or(0);
        counts[idx] += 1;
        total += 1;
    }
    let mut entries: Vec<(usize, [u8; 3])> = counts.into_iter().zip(pal).collect();
    entries.sort_by_key(|e| std::cmp::Reverse(e.0));
    let colors: Vec<Value> = entries
        .iter()
        .map(|(n, c)| {
            color_json(
                *c,
                Some(if total > 0 {
                    ((*n as f64 / total as f64) * 1000.0).round() / 10.0
                } else {
                    0.0
                }),
            )
        })
        .collect();
    Ok(json!({
        "colors": colors,
        "count": entries.len(),
        "css_variables": entries.iter().enumerate().map(|(i, (_, c))| format!("--color-{}: {};", i + 1, hex_color(*c))).collect::<Vec<_>>().join("\n"),
    }))
}

/// Dominant colour (largest palette bucket) and average colour.
pub fn dominant_and_average(d: &Decoded) -> EngineResult<Value> {
    let rgba = d.image.to_rgba8();
    let pal = quantize(&rgba, 5);
    let mut sum = [0u64; 3];
    let mut n = 0u64;
    for p in rgba.pixels() {
        if p[3] > 16 {
            for c in 0..3 {
                sum[c] += p[c] as u64;
            }
            n += 1;
        }
    }
    if n == 0 {
        return Err(EngineError::invalid("The image is fully transparent"));
    }
    let avg = [(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8];
    Ok(json!({
        "dominant": color_json(pal[0], None),
        "average": color_json(avg, None),
        "palette": pal.iter().map(|c| hex_color(*c)).collect::<Vec<_>>(),
    }))
}

/// Colour of the pixel at (x, y), optionally averaged over a square sample.
pub fn color_at(d: &Decoded, x: u32, y: u32, sample: u32) -> EngineResult<Value> {
    let (w, h) = d.image.dimensions();
    if x >= w || y >= h {
        return Err(EngineError::invalid(format!("Point ({x}, {y}) is outside the {w}x{h} image")));
    }
    let r = sample.min(64) as i64 / 2;
    let rgba = d.image.to_rgba8();
    let mut sum = [0u64; 4];
    let mut n = 0u64;
    for dy in -r..=r {
        for dx in -r..=r {
            let sx = x as i64 + dx;
            let sy = y as i64 + dy;
            if sx >= 0 && sy >= 0 && sx < w as i64 && sy < h as i64 {
                let p = rgba.get_pixel(sx as u32, sy as u32);
                for c in 0..4 {
                    sum[c] += p[c] as u64;
                }
                n += 1;
            }
        }
    }
    let c = [(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8];
    let mut v = color_json(c, None);
    v["alpha"] = json!((sum[3] / n) as u8);
    v["x"] = json!(x);
    v["y"] = json!(y);
    Ok(v)
}

/// Converts the image to ASCII art.
pub fn to_ascii(d: &Decoded, columns: u32, charset: &str, invert: bool) -> EngineResult<Value> {
    if !(4..=400).contains(&columns) {
        return Err(EngineError::invalid("`columns` must be between 4 and 400"));
    }
    let chars: Vec<char> = match charset {
        "" | "standard" => "@%#*+=-:. ".chars().collect(),
        "detailed" => "$@B%8&WM#*oahkbdpqwmZO0QLCJUYXzcvunxrjft/\\|()1{}[]?-_+~<>i!lI;:,\"^`'. "
            .chars()
            .collect(),
        "blocks" => "█▓▒░ ".chars().collect(),
        "binary" => "10".chars().collect(),
        custom => custom.chars().collect(),
    };
    if chars.len() < 2 {
        return Err(EngineError::invalid("`charset` needs at least two characters"));
    }
    let (w, h) = d.image.dimensions();
    // Character cells are roughly twice as tall as wide.
    let rows = ((h as f64 / w as f64) * columns as f64 * 0.5).round().max(1.0) as u32;
    let small = d
        .image
        .resize_exact(columns, rows, image::imageops::FilterType::Triangle)
        .to_luma_alpha8();
    let mut lines = Vec::with_capacity(rows as usize);
    for y in 0..rows {
        let mut line = String::with_capacity(columns as usize);
        for x in 0..columns {
            let p = small.get_pixel(x, y);
            let lum = (p[0] as f64 * p[1] as f64 / 255.0) / 255.0;
            let v = if invert { 1.0 - lum } else { lum };
            let idx = ((1.0 - v) * (chars.len() - 1) as f64).round() as usize;
            line.push(chars[idx.min(chars.len() - 1)]);
        }
        lines.push(line);
    }
    Ok(json!({ "ascii": lines.join("\n"), "columns": columns, "rows": rows }))
}

/// Legacy helpers kept for the v1 image cropper UI.
pub mod legacy {
    use super::*;

    /// Reduced aspect ratio (`16:9`) and decimal ratio for a size.
    pub fn aspect_ratio(width: u32, height: u32) -> EngineResult<Value> {
        if width == 0 || height == 0 {
            return Err(EngineError::invalid("Width and height must be greater than zero"));
        }
        let g = codec::gcd(width, height);
        Ok(json!({
            "ratio_str": format!("{}:{}", width / g, height / g),
            "ratio_float": width as f64 / height as f64,
            "orientation": if width > height { "landscape" } else if height > width { "portrait" } else { "square" },
        }))
    }

    fn crop_result(orig_width: u32, orig_height: u32, x: u32, y: u32, w: u32, h: u32) -> Value {
        let g = codec::gcd(w, h);
        let area = ((w as f64 * h as f64) / (orig_width as f64 * orig_height as f64) * 1000.0)
            .round()
            / 10.0;
        json!({
            "valid": true,
            "crop_x": x, "crop_y": y, "crop_width": w, "crop_height": h,
            "orig_width": orig_width, "orig_height": orig_height,
            "ratio_str": format!("{}:{}", w / g, h / g),
            "ratio_float": w as f64 / h as f64,
            "area_percent": area,
        })
    }

    /// Clamps a crop rectangle to the image and reports the resulting geometry.
    pub fn calculate_crop(
        orig_width: u32,
        orig_height: u32,
        crop_x: u32,
        crop_y: u32,
        crop_width: u32,
        crop_height: u32,
    ) -> EngineResult<Value> {
        if orig_width == 0 || orig_height == 0 {
            return Err(EngineError::invalid("Original dimensions must be positive"));
        }
        let x = crop_x.min(orig_width - 1);
        let y = crop_y.min(orig_height - 1);
        let w = crop_width.clamp(1, orig_width - x);
        let h = crop_height.clamp(1, orig_height - y);
        Ok(crop_result(orig_width, orig_height, x, y, w, h))
    }

    /// Largest centred crop matching a named preset (`square`, `16:9`, `instagram`, …).
    pub fn preset_crop(orig_width: u32, orig_height: u32, preset: &str) -> EngineResult<Value> {
        if orig_width == 0 || orig_height == 0 {
            return Err(EngineError::invalid("Original dimensions must be positive"));
        }
        let target = match preset {
            "1:1" => 1.0,
            "16:9" => 16.0 / 9.0,
            "4:3" => 4.0 / 3.0,
            "9:16" => 9.0 / 16.0,
            "3:2" => 1.5,
            "2:3" => 2.0 / 3.0,
            other => crate::transform::parse_ratio(other).map_err(|_| {
                EngineError::invalid(format!("Unsupported aspect ratio preset: {other}"))
            })?,
        };
        let orig = orig_width as f64 / orig_height as f64;
        let (w, h) = if orig > target {
            ((orig_height as f64 * target).round() as u32, orig_height)
        } else {
            (orig_width, (orig_width as f64 / target).round() as u32)
        };
        let (w, h) = (w.clamp(1, orig_width), h.clamp(1, orig_height));
        Ok(crop_result(
            orig_width,
            orig_height,
            (orig_width - w) / 2,
            (orig_height - h) / 2,
            w,
            h,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::png_bytes;

    #[test]
    fn palette_and_dominant() {
        let d = codec::decode(&png_bytes(32, 32)).unwrap();
        let p = palette(&d, 4).unwrap();
        assert_eq!(p["count"], 4);
        let shares: f64 = p["colors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["share"].as_f64().unwrap())
            .sum();
        assert!((shares - 100.0).abs() < 1.0);
        assert!(palette(&d, 0).is_err());
        let da = dominant_and_average(&d).unwrap();
        assert!(da["dominant"]["hex"].as_str().unwrap().starts_with('#'));
        let c = color_at(&d, 0, 0, 1).unwrap();
        assert_eq!(c["rgb"]["b"], 128);
        assert!(color_at(&d, 99, 0, 1).is_err());
    }

    #[test]
    fn ascii() {
        let d = codec::decode(&png_bytes(40, 20)).unwrap();
        let a = to_ascii(&d, 20, "standard", false).unwrap();
        assert_eq!(a["rows"], 5);
        assert_eq!(a["ascii"].as_str().unwrap().lines().count(), 5);
        assert!(to_ascii(&d, 20, "x", false).is_err());
        assert!(to_ascii(&d, 1, "", false).is_err());
    }

    #[test]
    fn legacy_helpers() {
        assert_eq!(legacy::aspect_ratio(1920, 1080).unwrap()["ratio_str"], "16:9");
        assert!(legacy::aspect_ratio(0, 1).is_err());
        let p = legacy::preset_crop(800, 600, "1:1").unwrap();
        assert_eq!((p["crop_width"].clone(), p["crop_height"].clone()), (json!(600), json!(600)));
        assert_eq!(p["crop_x"], 100);
        assert!(legacy::preset_crop(800, 600, "bogus").is_err());
        let c = legacy::calculate_crop(100, 100, 90, 90, 50, 50).unwrap();
        assert_eq!(c["crop_width"], 10);
    }
}
