//! # Dev Toolbox Colour & Design Engine (`colordesign-engine`)
//!
//! | Action | Tool(s) |
//! |---|---|
//! | `convert` | hex-to-rgb, rgb-to-hex, rgb-to-hsl, hsl-to-rgb, color-picker |
//! | `palette` | color-palette-generator (harmonies, monochrome, seeded random, tints/shades) |
//! | `gradient` | gradient-generator, gradient-css-generator |
//! | `contrast` | contrast-checker |
//! | `accessible_pairs` | accessible-color-pairing |
//! | `simulate_color_blindness` | color-blindness-simulator (colours and images) |
//! | `image_palette` | image-palette-extractor |
//! | `shadow` | shadow-generator |
//! | `border_radius` | border-radius-generator |
//! | `css_units` | css-unit-converter |
//! | `spacing_scale` | spacing-scale-generator |
//! | `type_scale` | typography-scale-generator |
//! | `favicon_preview` | favicon-preview |
//! | `social_dimensions` | social-image-dimension-checker |

use color_core::{contrast_json, contrast_ratio, parse, Rgba};
use engine_shared::{EngineError, EngineResult};
use image::GenericImageView;
use serde::Deserialize;
use serde_json::{json, Map, Value};

fn r2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

// ---------------------------------------------------------------- palette --

/// Generates a colour scheme (`monochromatic`, `analogous`, `complementary`, `split`, `triadic`,
/// `tetradic`, `shades`, `tints`, `tones`, `material`, `random`) with CSS/Tailwind output.
pub fn palette(base: &str, scheme: &str, count: usize, seed: Option<u64>) -> EngineResult<Value> {
    let c = parse(base)?;
    let (h, s, l) = c.hsl();
    let n = count.clamp(2, 24);
    let colors: Vec<Rgba> = match scheme {
        "" | "monochromatic" | "mono" => (0..n)
            .map(|i| Rgba::from_hsl(h, s, 10.0 + 80.0 * i as f64 / (n - 1) as f64, 1.0))
            .collect(),
        "analogous" => (0..n)
            .map(|i| Rgba::from_hsl(h - 30.0 + 60.0 * i as f64 / (n - 1) as f64, s, l, 1.0))
            .collect(),
        "complementary" => (0..n)
            .map(|i| {
                if i % 2 == 0 {
                    Rgba::from_hsl(h, s, (l - 10.0 * (i / 2) as f64).clamp(5.0, 95.0), 1.0)
                } else {
                    Rgba::from_hsl(h + 180.0, s, (l - 10.0 * (i / 2) as f64).clamp(5.0, 95.0), 1.0)
                }
            })
            .collect(),
        "split" | "split_complementary" => [0.0, 150.0, 210.0]
            .iter()
            .cycle()
            .take(n)
            .enumerate()
            .map(|(i, d)| {
                Rgba::from_hsl(h + d, s, (l + 5.0 * (i / 3) as f64).clamp(5.0, 95.0), 1.0)
            })
            .collect(),
        "triadic" => [0.0, 120.0, 240.0]
            .iter()
            .cycle()
            .take(n)
            .enumerate()
            .map(|(i, d)| {
                Rgba::from_hsl(h + d, s, (l + 8.0 * (i / 3) as f64).clamp(5.0, 95.0), 1.0)
            })
            .collect(),
        "tetradic" | "square" => [0.0, 90.0, 180.0, 270.0]
            .iter()
            .cycle()
            .take(n)
            .enumerate()
            .map(|(i, d)| {
                Rgba::from_hsl(h + d, s, (l + 8.0 * (i / 4) as f64).clamp(5.0, 95.0), 1.0)
            })
            .collect(),
        "shades" => (0..n)
            .map(|i| Rgba::from_hsl(h, s, l * (1.0 - i as f64 / n as f64), 1.0))
            .collect(),
        "tints" => (0..n)
            .map(|i| Rgba::from_hsl(h, s, l + (100.0 - l) * i as f64 / n as f64, 1.0))
            .collect(),
        "tones" => (0..n)
            .map(|i| Rgba::from_hsl(h, s * (1.0 - i as f64 / n as f64), l, 1.0))
            .collect(),
        "material" | "tailwind" => {
            // 50..950 style ramp keyed by lightness.
            let steps = [97.0, 93.0, 86.0, 76.0, 64.0, 52.0, 44.0, 36.0, 28.0, 20.0, 12.0];
            steps
                .iter()
                .map(|light| {
                    Rgba::from_hsl(
                        h,
                        (s * if *light > 80.0 { 0.9 } else { 1.0 }).min(100.0),
                        *light,
                        1.0,
                    )
                })
                .collect()
        },
        "random" => {
            let mut state = seed.unwrap_or(0x9E37_79B9_7F4A_7C15) | 1;
            (0..n)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    let hue = (state % 360) as f64;
                    let sat = 45 + (state >> 8) % 45;
                    let light = 35 + (state >> 16) % 35;
                    Rgba::from_hsl(hue, sat as f64, light as f64, 1.0)
                })
                .collect()
        },
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown scheme `{other}` (monochromatic, analogous, complementary, split, \
                 triadic, tetradic, shades, tints, tones, material, random)"
            )))
        },
    };
    let white = Rgba::new(255.0, 255.0, 255.0, 1.0);
    let black = Rgba::new(0.0, 0.0, 0.0, 1.0);
    let names = if scheme == "material" || scheme == "tailwind" {
        vec!["50", "100", "200", "300", "400", "500", "600", "700", "800", "900", "950"]
    } else {
        vec![]
    };
    let items: Vec<Value> = colors
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let (hh, ss, ll) = c.hsl();
            json!({
                "hex": c.hex(),
                "rgb": format!("rgb({}, {}, {})", c.r.round() as u8, c.g.round() as u8, c.b.round() as u8),
                "hsl": format!("hsl({}, {}%, {}%)", hh.round(), ss.round(), ll.round()),
                "name": names.get(i),
                "text_color": if contrast_ratio(c, &white) >= contrast_ratio(c, &black) { "#ffffff" } else { "#000000" },
                "contrast_on_white": r2(contrast_ratio(c, &white)),
                "is_dark": c.luminance() < 0.179
            })
        })
        .collect();
    let css_vars = items
        .iter()
        .enumerate()
        .map(|(i, c)| {
            format!(
                "  --color-{}: {};",
                names
                    .get(i)
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| (i + 1).to_string()),
                c["hex"].as_str().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(json!({
        "base": c.hex(),
        "scheme": if scheme.is_empty() { "monochromatic" } else { scheme },
        "colors": items,
        "css": format!(":root {{\n{css_vars}\n}}"),
        "tailwind": {
            "extend": {
                "colors": {
                    "brand": items.iter().enumerate().map(|(i, c)| (names.get(i).map(|n| n.to_string()).unwrap_or_else(|| (i + 1).to_string()), c["hex"].clone())).collect::<Map<_,
                    _>>()
                }
            }
        },
        "svg": swatch_svg(&colors)
    }))
}

fn swatch_svg(colors: &[Rgba]) -> String {
    let w = 60;
    let mut s = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} 80" width="{}" height="80">"#,
        colors.len() * w,
        colors.len() * w
    );
    for (i, c) in colors.iter().enumerate() {
        s.push_str(&format!(r#"<rect x="{}" y="0" width="{w}" height="60" fill="{}"/><text x="{}" y="74" font-size="9" font-family="monospace" text-anchor="middle">{}</text>"#, i * w, c.hex(), i * w + w / 2, c.hex()));
    }
    s.push_str("</svg>");
    s
}

// --------------------------------------------------------------- gradient --

/// A gradient colour stop.
#[derive(Deserialize, Debug, Clone)]
pub struct Stop {
    /// Colour in any CSS syntax.
    pub color: String,
    /// Position 0–100 (evenly spaced when omitted).
    #[serde(default)]
    pub position: Option<f64>,
}

fn lerp(a: &Rgba, b: &Rgba, t: f64, space: &str) -> Rgba {
    match space {
        "hsl" => {
            let (h1, s1, l1) = a.hsl();
            let (h2, s2, l2) = b.hsl();
            let mut dh = h2 - h1;
            if dh > 180.0 {
                dh -= 360.0;
            }
            if dh < -180.0 {
                dh += 360.0;
            }
            Rgba::from_hsl(
                h1 + dh * t,
                s1 + (s2 - s1) * t,
                l1 + (l2 - l1) * t,
                a.a + (b.a - a.a) * t,
            )
        },
        "lab" | "oklab" => {
            let (l1, a1, b1) = a.lab();
            let (l2, a2, b2) = b.lab();
            Rgba::from_lab(
                l1 + (l2 - l1) * t,
                a1 + (a2 - a1) * t,
                b1 + (b2 - b1) * t,
                a.a + (b.a - a.a) * t,
            )
        },
        _ => Rgba::new(
            a.r + (b.r - a.r) * t,
            a.g + (b.g - a.g) * t,
            a.b + (b.b - a.b) * t,
            a.a + (b.a - a.a) * t,
        ),
    }
}

/// Builds linear/radial/conic CSS gradients with interpolated steps (RGB, HSL, LAB, OKLCH).
pub fn gradient(
    stops: &[Stop],
    kind: &str,
    angle: f64,
    steps: usize,
    space: &str,
    shape: &str,
) -> EngineResult<Value> {
    if stops.len() < 2 {
        return Err(EngineError::invalid("A gradient needs at least two colour stops"));
    }
    if stops.len() > 32 {
        return Err(EngineError::limit("At most 32 stops"));
    }
    let colors: Vec<Rgba> = stops.iter().map(|s| parse(&s.color)).collect::<EngineResult<_>>()?;
    // Assign positions: explicit or evenly distributed.
    let mut positions: Vec<f64> = stops
        .iter()
        .enumerate()
        .map(|(i, s)| s.position.unwrap_or(i as f64 * 100.0 / (stops.len() - 1) as f64))
        .collect();
    for i in 1..positions.len() {
        if positions[i] < positions[i - 1] {
            positions[i] = positions[i - 1];
        }
    }
    let css_stops: Vec<String> = colors
        .iter()
        .zip(&positions)
        .map(|(c, p)| format!("{} {}%", if c.a < 1.0 { c.hex8() } else { c.hex() }, r2(*p)))
        .collect();
    let kind = if kind.is_empty() { "linear" } else { kind };
    let css = match kind {
        "linear" => format!("linear-gradient({}deg, {})", angle, css_stops.join(", ")),
        "radial" => format!(
            "radial-gradient({} at center, {})",
            if shape.is_empty() { "circle" } else { shape },
            css_stops.join(", ")
        ),
        "conic" => format!("conic-gradient(from {}deg, {})", angle, css_stops.join(", ")),
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown gradient type `{other}` (linear, radial, conic)"
            )))
        },
    };
    let n = steps.clamp(2, 256);
    let samples: Vec<Value> = (0..n)
        .map(|i| {
            let p = i as f64 * 100.0 / (n - 1) as f64;
            let idx = positions
                .iter()
                .position(|x| *x >= p)
                .unwrap_or(positions.len() - 1)
                .max(1);
            let (p0, p1) = (positions[idx - 1], positions[idx]);
            let t = if p1 > p0 {
                ((p - p0) / (p1 - p0)).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let c = lerp(&colors[idx - 1], &colors[idx], t, space);
            json!({ "position": r2(p), "hex": c.hex() })
        })
        .collect();
    let svg_stops: String = colors
        .iter()
        .zip(&positions)
        .map(|(c, p)| {
            format!(
                r#"<stop offset="{}%" stop-color="{}" stop-opacity="{}"/>"#,
                r2(*p),
                c.hex(),
                c.a
            )
        })
        .collect();
    let (x2, y2) = (
        (angle.to_radians().sin() * 50.0 + 50.0),
        (-angle.to_radians().cos() * 50.0 + 50.0),
    );
    let svg = match kind {
        "radial" => format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100"><defs><radialGradient id="g">{svg_stops}</radialGradient></defs><rect width="200" height="100" fill="url(#g)"/></svg>"#
        ),
        _ => format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100"><defs><linearGradient id="g" x1="{}%" y1="{}%" x2="{x2}%" y2="{y2}%">{svg_stops}</linearGradient></defs><rect width="200" height="100" fill="url(#g)"/></svg>"#,
            100.0 - x2,
            100.0 - y2
        ),
    };
    Ok(json!({
        "css": format!("background: {css};"),
        "css_value": css,
        "with_fallback": format!("background: {};\nbackground: {css};", colors[0].hex()),
        "tailwind_hint": if kind == "linear" { json!(format!("bg-gradient-to-r from-[{}] to-[{}]", colors[0].hex(), colors[colors.len() - 1].hex())) } else { Value::Null },
        "svg": svg,
        "stops": colors.iter().zip(&positions).map(|(c, p)| json!({ "hex": c.hex(), "position": r2(*p) })).collect::<Vec<_>>(),
        "samples": samples,
        "interpolation": if space.is_empty() { "rgb" } else { space },
        "contrast_extremes": contrast_json(&colors[0], &colors[colors.len() - 1]),
    }))
}

// ---------------------------------------------------------- accessibility --

/// Checks every pair of colours for WCAG contrast and optionally suggests fixes.
pub fn accessible_pairs(colors: &[String], level: &str, fix: bool) -> EngineResult<Value> {
    if colors.len() < 2 {
        return Err(EngineError::invalid("Provide at least two colours"));
    }
    if colors.len() > 40 {
        return Err(EngineError::limit("At most 40 colours"));
    }
    let parsed: Vec<Rgba> = colors.iter().map(|c| parse(c)).collect::<EngineResult<_>>()?;
    let threshold = match level.to_ascii_uppercase().as_str() {
        "" | "AA" => 4.5,
        "AAA" => 7.0,
        "AA-LARGE" | "AA_LARGE" | "LARGE" => 3.0,
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown level `{other}` (AA, AAA, AA-large)"
            )))
        },
    };
    let mut pairs = Vec::new();
    let mut failing = Vec::new();
    for i in 0..parsed.len() {
        for j in 0..parsed.len() {
            if i == j {
                continue;
            }
            let ratio = contrast_ratio(&parsed[i], &parsed[j]);
            let entry = json!({
                "foreground": parsed[i].hex(),
                "background": parsed[j].hex(),
                "ratio": r2(ratio),
                "passes": ratio >= threshold
            });
            if ratio >= threshold {
                pairs.push(entry);
            } else if i < j {
                let mut e = entry;
                if fix {
                    e["suggested_foreground"] = json!(adjust_for_contrast(
                        &parsed[i], &parsed[j], threshold
                    )
                    .map(|c| c.hex()));
                }
                failing.push(e);
            }
        }
    }
    pairs.sort_by(|a, b| {
        b["ratio"]
            .as_f64()
            .unwrap_or(0.0)
            .total_cmp(&a["ratio"].as_f64().unwrap_or(0.0))
    });
    let white = Rgba::new(255.0, 255.0, 255.0, 1.0);
    let black = Rgba::new(0.0, 0.0, 0.0, 1.0);
    Ok(json!({
        "level": if level.is_empty() { "AA" } else { level },
        "threshold": threshold,
        "passing_pairs": pairs,
        "failing_pairs": failing,
        "per_color": parsed.iter().map(|c| json!({ "hex": c.hex(), "on_white": r2(contrast_ratio(c, &white)), "on_black": r2(contrast_ratio(c, &black)), "best_text": if contrast_ratio(c, &white) >= contrast_ratio(c, &black) { "#ffffff" } else { "#000000" } })).collect::<Vec<_>>(),
    }))
}

fn adjust_for_contrast(fg: &Rgba, bg: &Rgba, threshold: f64) -> Option<Rgba> {
    let (h, s, l) = fg.hsl();
    let dir = if bg.luminance() > 0.5 { -1.0 } else { 1.0 };
    let search = |dir: f64| {
        (0..=40)
            .map(|i| Rgba::from_hsl(h, s, (l + dir * i as f64 * 2.5).clamp(0.0, 100.0), 1.0))
            .find(|c| contrast_ratio(c, bg) >= threshold)
    };
    search(dir).or_else(|| search(-dir))
}

// ------------------------------------------------------- colour blindness --

const CVD: &[(&str, &str, [[f64; 3]; 3])] = &[
    (
        "protanopia",
        "red-blind (~1% of men)",
        [[0.567, 0.433, 0.0], [0.558, 0.442, 0.0], [0.0, 0.242, 0.758]],
    ),
    (
        "protanomaly",
        "red-weak",
        [[0.817, 0.183, 0.0], [0.333, 0.667, 0.0], [0.0, 0.125, 0.875]],
    ),
    (
        "deuteranopia",
        "green-blind (~1% of men)",
        [[0.625, 0.375, 0.0], [0.7, 0.3, 0.0], [0.0, 0.3, 0.7]],
    ),
    (
        "deuteranomaly",
        "green-weak (~5% of men, most common)",
        [[0.8, 0.2, 0.0], [0.258, 0.742, 0.0], [0.0, 0.142, 0.858]],
    ),
    (
        "tritanopia",
        "blue-blind (rare)",
        [[0.95, 0.05, 0.0], [0.0, 0.433, 0.567], [0.0, 0.475, 0.525]],
    ),
    (
        "tritanomaly",
        "blue-weak",
        [[0.967, 0.033, 0.0], [0.0, 0.733, 0.267], [0.0, 0.183, 0.817]],
    ),
    (
        "achromatopsia",
        "total colour blindness",
        [[0.299, 0.587, 0.114], [0.299, 0.587, 0.114], [0.299, 0.587, 0.114]],
    ),
    (
        "achromatomaly",
        "partial colour blindness",
        [[0.618, 0.320, 0.062], [0.163, 0.775, 0.062], [0.163, 0.320, 0.516]],
    ),
];

fn apply_matrix(c: &Rgba, m: &[[f64; 3]; 3]) -> Rgba {
    let lin = |v: f64| {
        let v = v / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let gam = |v: f64| {
        let v = v.clamp(0.0, 1.0);
        (if v <= 0.0031308 {
            v * 12.92
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        }) * 255.0
    };
    let (r, g, b) = (lin(c.r), lin(c.g), lin(c.b));
    Rgba::new(
        gam(m[0][0] * r + m[0][1] * g + m[0][2] * b),
        gam(m[1][0] * r + m[1][1] * g + m[1][2] * b),
        gam(m[2][0] * r + m[2][1] * g + m[2][2] * b),
        c.a,
    )
}

/// Simulates colour-vision deficiencies on swatches or an image.
pub fn simulate(colors: &[String], image: Option<&[u8]>, kinds: &[String]) -> EngineResult<Value> {
    let selected: Vec<&(&str, &str, [[f64; 3]; 3])> = if kinds.is_empty() {
        CVD.iter().collect()
    } else {
        kinds
            .iter()
            .map(|k| {
                CVD.iter().find(|c| c.0 == k.to_ascii_lowercase()).ok_or_else(|| {
                    EngineError::invalid(format!(
                        "Unknown deficiency `{k}` ({})",
                        CVD.iter().map(|c| c.0).collect::<Vec<_>>().join(", ")
                    ))
                })
            })
            .collect::<EngineResult<_>>()?
    };
    let mut out = Map::new();
    if !colors.is_empty() {
        let parsed: Vec<Rgba> = colors.iter().map(|c| parse(c)).collect::<EngineResult<_>>()?;
        let mut per_kind = Vec::new();
        for (name, desc, m) in &selected {
            let sim: Vec<Rgba> = parsed.iter().map(|c| apply_matrix(c, m)).collect();
            // Distinguishability: smallest pairwise Lab distance after simulation.
            let mut min_d = f64::MAX;
            let mut confusable = Vec::new();
            for i in 0..sim.len() {
                for j in i + 1..sim.len() {
                    let (l1, a1, b1) = sim[i].lab();
                    let (l2, a2, b2) = sim[j].lab();
                    let d = ((l1 - l2).powi(2) + (a1 - a2).powi(2) + (b1 - b2).powi(2)).sqrt();
                    min_d = min_d.min(d);
                    if d < 10.0 {
                        confusable.push(json!([parsed[i].hex(), parsed[j].hex()]));
                    }
                }
            }
            per_kind.push(json!({
                "type": name,
                "description": desc,
                "colors": sim.iter().map(|c| c.hex()).collect::<Vec<_>>(),
                "min_distance": if min_d == f64::MAX { Value::Null } else { json!(r2(min_d)) },
                "confusable_pairs": confusable
            }));
        }
        out.insert("original".into(), json!(parsed.iter().map(|c| c.hex()).collect::<Vec<_>>()));
        out.insert("simulations".into(), json!(per_kind));
    }
    if let Some(bytes) = image.filter(|b| !b.is_empty()) {
        let img = image::load_from_memory(bytes)
            .map_err(|e| EngineError::unsupported_format(format!("Cannot decode image: {e}")))?;
        engine_shared::limits::check_dimensions(img.width(), img.height())?;
        let img = if img.width() > 1024 || img.height() > 1024 {
            img.thumbnail(1024, 1024)
        } else {
            img
        };
        let rgba = img.to_rgba8();
        let mut images = Vec::new();
        for (name, desc, m) in &selected {
            let mut buf = rgba.clone();
            for p in buf.pixels_mut() {
                let c = apply_matrix(&Rgba::new(p[0] as f64, p[1] as f64, p[2] as f64, 1.0), m);
                p[0] = c.r.round() as u8;
                p[1] = c.g.round() as u8;
                p[2] = c.b.round() as u8;
            }
            let mut cursor = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(buf)
                .write_to(&mut cursor, image::ImageFormat::Png)
                .map_err(|e| EngineError::internal(e.to_string()))?;
            images.push(json!({
                "type": name,
                "description": desc,
                "png": engine_shared::b64::encode(&cursor.into_inner()),
                "width": img.width(),
                "height": img.height()
            }));
        }
        out.insert("images".into(), json!(images));
    }
    if out.is_empty() {
        return Err(EngineError::empty("colors or image"));
    }
    out.insert(
        "types".into(),
        json!(CVD
            .iter()
            .map(|c| json!({ "type": c.0, "description": c.1 }))
            .collect::<Vec<_>>()),
    );
    Ok(Value::Object(out))
}

// ---------------------------------------------------------- image palette --

/// Extracts a k-means palette from an image with dominant/vibrant/muted picks.
pub fn image_palette(bytes: &[u8], count: usize) -> EngineResult<Value> {
    engine_shared::error::require_bytes(bytes, "image")?;
    let img = image::load_from_memory(bytes)
        .map_err(|e| EngineError::unsupported_format(format!("Cannot decode image: {e}")))?;
    engine_shared::limits::check_dimensions(img.width(), img.height())?;
    let small = img.thumbnail(160, 160).to_rgba8();
    let pixels: Vec<[f64; 3]> = small
        .pixels()
        .filter(|p| p[3] > 64)
        .map(|p| [p[0] as f64, p[1] as f64, p[2] as f64])
        .collect();
    if pixels.is_empty() {
        return Err(EngineError::invalid("Image is fully transparent"));
    }
    let k = count.clamp(2, 16).min(pixels.len());
    // k-means++ init (deterministic) + 12 iterations.
    let mut centers: Vec<[f64; 3]> = vec![pixels[0]];
    while centers.len() < k {
        let (idx, _) = pixels
            .iter()
            .enumerate()
            .map(|(i, p)| (i, centers.iter().map(|c| dist2(p, c)).fold(f64::MAX, f64::min)))
            .fold((0, -1.0), |m, x| if x.1 > m.1 { x } else { m });
        centers.push(pixels[idx]);
    }
    let mut assign = vec![0usize; pixels.len()];
    for _ in 0..12 {
        for (i, p) in pixels.iter().enumerate() {
            assign[i] = centers
                .iter()
                .enumerate()
                .map(|(ci, c)| (ci, dist2(p, c)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(ci, _)| ci)
                .unwrap_or(0);
        }
        let mut sums = vec![[0.0f64; 4]; k];
        for (i, p) in pixels.iter().enumerate() {
            let s = &mut sums[assign[i]];
            s[0] += p[0];
            s[1] += p[1];
            s[2] += p[2];
            s[3] += 1.0;
        }
        for (ci, s) in sums.iter().enumerate() {
            if s[3] > 0.0 {
                centers[ci] = [s[0] / s[3], s[1] / s[3], s[2] / s[3]];
            }
        }
    }
    let mut counts = vec![0usize; k];
    for a in &assign {
        counts[*a] += 1;
    }
    let total = pixels.len() as f64;
    let mut items: Vec<(Rgba, f64)> = centers
        .iter()
        .zip(&counts)
        .filter(|(_, n)| **n > 0)
        .map(|(c, n)| (Rgba::new(c[0], c[1], c[2], 1.0), *n as f64 / total * 100.0))
        .collect();
    items.sort_by(|a, b| b.1.total_cmp(&a.1));
    let white = Rgba::new(255.0, 255.0, 255.0, 1.0);
    let black = Rgba::new(0.0, 0.0, 0.0, 1.0);
    let colors: Vec<Value> = items
        .iter()
        .map(|(c, pct)| {
            let (h, s, l) = c.hsl();
            json!({
                "hex": c.hex(),
                "rgb": [c.r.round() as u8, c.g.round() as u8, c.b.round() as u8],
                "hsl": [h.round(), s.round(), l.round()],
                "percent": r2(*pct),
                "nearest_name": c.nearest_name().0,
                "text_color": if contrast_ratio(c, &white) >= contrast_ratio(c, &black) { "#ffffff" } else { "#000000" }
            })
        })
        .collect();
    let vibrant = items
        .iter()
        .max_by(|a, b| {
            let sa = a.0.hsl().1 * (1.0 - (a.0.hsl().2 - 50.0).abs() / 50.0);
            let sb = b.0.hsl().1 * (1.0 - (b.0.hsl().2 - 50.0).abs() / 50.0);
            sa.total_cmp(&sb)
        })
        .map(|x| x.0.hex());
    let muted = items
        .iter()
        .min_by(|a, b| a.0.hsl().1.total_cmp(&b.0.hsl().1))
        .map(|x| x.0.hex());
    let swatches: Vec<Rgba> = items.iter().map(|x| x.0).collect();
    Ok(json!({
        "colors": colors,
        "dominant": colors.first().map(|c| c["hex"].clone()),
        "vibrant": vibrant,
        "muted": muted,
        "lightest": items.iter().max_by(|a, b| a.0.luminance().total_cmp(&b.0.luminance())).map(|x| x.0.hex()),
        "darkest": items.iter().min_by(|a, b| a.0.luminance().total_cmp(&b.0.luminance())).map(|x| x.0.hex()),
        "css": format!(":root {{\n{}\n}}", colors.iter().enumerate().map(|(i, c)| format!("  --palette-{}: {};", i + 1, c["hex"].as_str().unwrap_or(""))).collect::<Vec<_>>().join("\n")),
        "svg": swatch_svg(&swatches),
        "image": { "width": img.width(), "height": img.height(), "sampled_pixels": pixels.len() }
    }))
}

fn dist2(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

// ------------------------------------------------------------- CSS helpers --

/// One `box-shadow` layer.
#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct ShadowLayer {
    /// Horizontal offset (px).
    pub x: f64,
    /// Vertical offset (px).
    pub y: f64,
    /// Blur radius (px).
    pub blur: f64,
    /// Spread radius (px).
    pub spread: f64,
    /// Shadow colour.
    pub color: String,
    /// Inset shadow.
    pub inset: bool,
}

impl Default for ShadowLayer {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 4.0,
            blur: 12.0,
            spread: 0.0,
            color: "rgba(0,0,0,0.15)".into(),
            inset: false,
        }
    }
}

/// Builds `box-shadow` CSS from layers or a preset (`sm`, `md`, `lg`, `xl`, `soft`, `hard`…).
pub fn shadow(layers: &[ShadowLayer], preset: &str, color: &str) -> EngineResult<Value> {
    let base = if color.is_empty() {
        Rgba::new(0.0, 0.0, 0.0, 1.0)
    } else {
        parse(color)?
    };
    let layers: Vec<ShadowLayer> = if !layers.is_empty() {
        layers.to_vec()
    } else {
        let c = |a: f64| {
            format!(
                "rgba({}, {}, {}, {a})",
                base.r.round() as u8,
                base.g.round() as u8,
                base.b.round() as u8
            )
        };
        match preset {
            "" | "medium" | "md" => vec![
                ShadowLayer {
                    x: 0.0,
                    y: 4.0,
                    blur: 6.0,
                    spread: -1.0,
                    color: c(0.1),
                    inset: false,
                },
                ShadowLayer {
                    x: 0.0,
                    y: 2.0,
                    blur: 4.0,
                    spread: -2.0,
                    color: c(0.1),
                    inset: false,
                },
            ],
            "small" | "sm" => vec![ShadowLayer {
                x: 0.0,
                y: 1.0,
                blur: 2.0,
                spread: 0.0,
                color: c(0.05),
                inset: false,
            }],
            "large" | "lg" => vec![
                ShadowLayer {
                    x: 0.0,
                    y: 10.0,
                    blur: 15.0,
                    spread: -3.0,
                    color: c(0.1),
                    inset: false,
                },
                ShadowLayer {
                    x: 0.0,
                    y: 4.0,
                    blur: 6.0,
                    spread: -4.0,
                    color: c(0.1),
                    inset: false,
                },
            ],
            "xl" => vec![
                ShadowLayer {
                    x: 0.0,
                    y: 20.0,
                    blur: 25.0,
                    spread: -5.0,
                    color: c(0.1),
                    inset: false,
                },
                ShadowLayer {
                    x: 0.0,
                    y: 8.0,
                    blur: 10.0,
                    spread: -6.0,
                    color: c(0.1),
                    inset: false,
                },
            ],
            "2xl" => vec![ShadowLayer {
                x: 0.0,
                y: 25.0,
                blur: 50.0,
                spread: -12.0,
                color: c(0.25),
                inset: false,
            }],
            "inner" => vec![ShadowLayer {
                x: 0.0,
                y: 2.0,
                blur: 4.0,
                spread: 0.0,
                color: c(0.05),
                inset: true,
            }],
            "soft" | "layered" => (1..=5)
                .map(|i| {
                    let f = 2f64.powi(i);
                    ShadowLayer {
                        x: 0.0,
                        y: f,
                        blur: f * 2.0,
                        spread: 0.0,
                        color: c(0.04),
                        inset: false,
                    }
                })
                .collect(),
            "neumorphic" => vec![
                ShadowLayer {
                    x: 8.0,
                    y: 8.0,
                    blur: 16.0,
                    spread: 0.0,
                    color: c(0.2),
                    inset: false,
                },
                ShadowLayer {
                    x: -8.0,
                    y: -8.0,
                    blur: 16.0,
                    spread: 0.0,
                    color: "rgba(255,255,255,0.7)".into(),
                    inset: false,
                },
            ],
            "hard" => vec![ShadowLayer {
                x: 6.0,
                y: 6.0,
                blur: 0.0,
                spread: 0.0,
                color: c(1.0),
                inset: false,
            }],
            other => {
                return Err(EngineError::invalid(format!(
                    "Unknown preset `{other}` (sm, md, lg, xl, 2xl, inner, soft, neumorphic, hard)"
                )))
            },
        }
    };
    if layers.len() > 16 {
        return Err(EngineError::limit("At most 16 shadow layers"));
    }
    let parts: Vec<String> = layers
        .iter()
        .map(|l| {
            format!(
                "{}{}px {}px {}px {}px {}",
                if l.inset { "inset " } else { "" },
                l.x,
                l.y,
                l.blur,
                l.spread,
                l.color
            )
        })
        .collect();
    let css = format!("box-shadow: {};", parts.join(", "));
    let drop: Vec<String> = layers
        .iter()
        .filter(|l| !l.inset)
        .map(|l| format!("drop-shadow({}px {}px {}px {})", l.x, l.y, l.blur / 2.0, l.color))
        .collect();
    Ok(json!({
        "css": css,
        "value": parts.join(", "),
        "filter_equivalent": if drop.is_empty() { Value::Null } else { json!(format!("filter: {};", drop.join(" "))) },
        "layers": layers.len(),
        "tailwind_arbitrary": format!("shadow-[{}]", parts.join(",").replace(' ', "_")),
        "svg_preview": format!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 140"><rect x="50" y="30" width="100" height="80" rx="12" fill="#fff" style="{css}"/></svg>"##)
    }))
}

/// Builds `border-radius` CSS (per corner, elliptical) with a preview SVG.
pub fn border_radius(
    tl: f64,
    tr: f64,
    br: f64,
    bl: f64,
    unit: &str,
    elliptical: Option<[f64; 4]>,
) -> EngineResult<Value> {
    for v in [tl, tr, br, bl] {
        if v < 0.0 || !v.is_finite() {
            return Err(EngineError::invalid("Radii must be non-negative numbers"));
        }
    }
    let u = if unit.is_empty() { "px" } else { unit };
    let all_same = tl == tr && tr == br && br == bl;
    let value = if all_same {
        format!("{tl}{u}")
    } else if tl == br && tr == bl {
        format!("{tl}{u} {tr}{u}")
    } else {
        format!("{tl}{u} {tr}{u} {br}{u} {bl}{u}")
    };
    let value = match elliptical {
        Some(v) => format!("{value} / {}{u} {}{u} {}{u} {}{u}", v[0], v[1], v[2], v[3]),
        None => value,
    };
    Ok(json!({
        "css": format!("border-radius: {value};"),
        "value": value,
        "longhand": {
            "border-top-left-radius": format!("{tl}{u}"),
            "border-top-right-radius": format!("{tr}{u}"),
            "border-bottom-right-radius": format!("{br}{u}"),
            "border-bottom-left-radius": format!("{bl}{u}")
        },
        "tailwind": if all_same { json!(match tl as i64 { 0 => "rounded-none".to_string(), 2 => "rounded-sm".into(), 4 => "rounded".into(), 6 => "rounded-md".into(), 8 => "rounded-lg".into(), 12 => "rounded-xl".into(), 16 => "rounded-2xl".into(), 24 => "rounded-3xl".into(), 9999 => "rounded-full".into(), _ => format!("rounded-[{tl}{u}]") }) } else { json!(format!("\
            rounded-tl-[{tl}{u}] rounded-tr-[{tr}{u}] rounded-br-[{br}{u}] rounded-bl-[{bl}{u}]")) },
        "svg_preview": format!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 140"><rect x="20" y="20" width="160" height="100" fill="#3b82f6" style="border-radius:{value}" rx="{}"/></svg>"##, if all_same { tl } else { 0.0 })
    }))
}

/// Converts CSS lengths (`px`, `rem`, `em`, `%`, `vw`, `vh`, `pt`, `cm`, `mm`, `in`).
pub fn css_units(
    value: f64,
    from: &str,
    to: &[String],
    base_font_px: f64,
    viewport_w: f64,
    viewport_h: f64,
    parent_px: f64,
) -> EngineResult<Value> {
    if !value.is_finite() {
        return Err(EngineError::invalid("value must be a number"));
    }
    let base = if base_font_px <= 0.0 { 16.0 } else { base_font_px };
    let (vw, vh) = (
        if viewport_w <= 0.0 { 1920.0 } else { viewport_w },
        if viewport_h <= 0.0 { 1080.0 } else { viewport_h },
    );
    let parent = if parent_px <= 0.0 { base } else { parent_px };
    let to_px = |v: f64, u: &str| -> EngineResult<f64> {
        Ok(match u.to_ascii_lowercase().as_str() {
            "px" => v,
            "rem" => v * base,
            "em" => v * parent,
            "pt" => v * 96.0 / 72.0,
            "pc" => v * 16.0,
            "in" => v * 96.0,
            "cm" => v * 96.0 / 2.54,
            "mm" => v * 96.0 / 25.4,
            "q" => v * 96.0 / 25.4 / 4.0,
            "vw" => v * vw / 100.0,
            "vh" => v * vh / 100.0,
            "vmin" => v * vw.min(vh) / 100.0,
            "vmax" => v * vw.max(vh) / 100.0,
            "%" | "percent" => v * parent / 100.0,
            other => return Err(EngineError::invalid(format!("Unknown CSS unit `{other}`"))),
        })
    };
    let px = to_px(value, from)?;
    let units = if to.is_empty() {
        vec!["px", "rem", "em", "pt", "%", "vw", "vh", "in", "cm", "mm"]
            .into_iter()
            .map(String::from)
            .collect()
    } else {
        to.to_vec()
    };
    let mut out = Map::new();
    for u in &units {
        let one = to_px(1.0, u)?;
        out.insert(u.clone(), json!(((px / one) * 10000.0).round() / 10000.0));
    }
    Ok(json!({
        "input": format!("{value}{from}"),
        "px": (px * 10000.0).round() / 10000.0,
        "conversions": out,
        "assumptions": {
            "root_font_size_px": base,
            "parent_font_size_px": parent,
            "viewport": [vw, vh],
            "dpi": 96
        }
    }))
}

/// Geometric spacing scale with CSS variables and Tailwind config.
pub fn spacing_scale(
    base: f64,
    ratio: f64,
    steps: usize,
    unit: &str,
    names: &str,
) -> EngineResult<Value> {
    if base <= 0.0 || ratio <= 1.0 {
        return Err(EngineError::invalid("base must be > 0 and ratio > 1"));
    }
    let n = steps.clamp(2, 24);
    let u = if unit.is_empty() { "px" } else { unit };
    let labels: Vec<String> = match names {
        "tshirt" | "" => [
            "3xs", "2xs", "xs", "sm", "md", "lg", "xl", "2xl", "3xl", "4xl", "5xl", "6xl", "7xl",
            "8xl",
        ]
        .iter()
        .map(|s| s.to_string())
        .chain((9..).map(|i| format!("{i}xl")))
        .take(n)
        .collect(),
        "numeric" => (0..n).map(|i| i.to_string()).collect(),
        "tailwind" => (0..n).map(|i| (i + 1).to_string()).collect(),
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown naming `{other}` (tshirt, numeric, tailwind)"
            )))
        },
    };
    let vals: Vec<f64> = (0..n)
        .map(|i| base * ratio.powi(i as i32 - (n as i32 / 2).min(4)))
        .collect();
    let items: Vec<Value> = labels
        .iter()
        .zip(&vals)
        .map(|(l, v)| {
            json!({
                "name": l,
                "value": r2(*v),
                "css": format!("{}{u}", r2(*v)),
                "rem": r2(*v / 16.0)
            })
        })
        .collect();
    Ok(json!({
        "base": base,
        "ratio": ratio,
        "steps": items,
        "css": format!(":root {{\n{}\n}}", items.iter().map(|i| format!("  --space-{}: {};", i["name"].as_str().unwrap_or(""), i["css"].as_str().unwrap_or(""))).collect::<Vec<_>>().join("\n")),
        "tailwind_spacing": items.iter().map(|i| (i["name"].as_str().unwrap_or("").to_string(), i["css"].clone())).collect::<Map<_,
        _>>()
    }))
}

/// Modular typographic scale with CSS variables and Tailwind config.
pub fn type_scale(
    base: f64,
    ratio_or_name: &Value,
    steps_up: usize,
    steps_down: usize,
    line_height: f64,
    unit: &str,
) -> EngineResult<Value> {
    let named = [
        ("minor second", 1.067),
        ("major second", 1.125),
        ("minor third", 1.2),
        ("major third", 1.25),
        ("perfect fourth", 1.333),
        ("augmented fourth", 1.414),
        ("perfect fifth", 1.5),
        ("golden ratio", 1.618),
    ];
    let ratio = match ratio_or_name {
        Value::Number(n) => n.as_f64().unwrap_or(1.25),
        Value::String(s) => named
            .iter()
            .find(|(n, _)| n.replace(' ', "_") == s.to_lowercase().replace([' ', '-'], "_"))
            .map(|x| x.1)
            .or_else(|| s.parse::<f64>().ok())
            .ok_or_else(|| {
                EngineError::invalid(format!(
                    "Unknown ratio `{s}` ({})",
                    named.iter().map(|n| n.0).collect::<Vec<_>>().join(", ")
                ))
            })?,
        Value::Null => 1.25,
        _ => return Err(EngineError::invalid("ratio must be a number or a named interval")),
    };
    if base <= 0.0 || ratio <= 1.0 {
        return Err(EngineError::invalid("base must be > 0 and ratio > 1"));
    }
    let up = steps_up.min(12) as i32;
    let down = steps_down.min(6) as i32;
    let lh = if line_height <= 0.0 { 1.5 } else { line_height };
    let u = if unit.is_empty() { "px" } else { unit };
    let names_up = [
        "base", "lg", "xl", "2xl", "3xl", "4xl", "5xl", "6xl", "7xl", "8xl", "9xl", "10xl", "11xl",
    ];
    let names_down = ["sm", "xs", "2xs", "3xs", "4xs", "5xs"];
    let mut items = Vec::new();
    for i in (-down..=up).rev() {
        let size = base * ratio.powi(i);
        let name = if i >= 0 {
            names_up
                .get(i as usize)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("step{i}"))
        } else {
            names_down
                .get((-i - 1) as usize)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("step{i}"))
        };
        // Tighter line height for large display sizes.
        let step_lh = if size > base * 2.0 {
            (lh - 0.3).max(1.1)
        } else if size > base * 1.4 {
            (lh - 0.15).max(1.2)
        } else {
            lh
        };
        let heading = match i {
            6 | 7 => Some("h1 (display)"),
            5 => Some("h1"),
            4 => Some("h2"),
            3 => Some("h3"),
            2 => Some("h4"),
            1 => Some("h5"),
            0 => Some("body"),
            -1 => Some("small / caption"),
            _ => None,
        };
        items.push(json!({
            "step": i,
            "name": name,
            "size": r2(size),
            "css": format!("{}{u}", r2(size)),
            "rem": r2(size / 16.0),
            "line_height": step_lh,
            "line_height_px": r2(size * step_lh),
            "suggested_use": heading
        }));
    }
    Ok(json!({
        "base": base,
        "ratio": ratio,
        "ratio_name": named.iter().find(|(_, r)| (r - ratio).abs() < 0.002).map(|n| n.0),
        "steps": items,
        "css": format!(":root {{\n{}\n}}", items.iter().map(|i| format!("  --text-{}: {};", i["name"].as_str().unwrap_or(""), i["css"].as_str().unwrap_or(""))).collect::<Vec<_>>().join("\n")),
        "tailwind_font_size": items.iter().map(|i| (i["name"].as_str().unwrap_or("").to_string(), json!([i["css"], { "lineHeight": i["line_height"].to_string() }]))).collect::<Map<_,
        _>>()
    }))
}

/// Renders favicon previews at standard sizes (16/32/48/180/192/512) on a background.
pub fn favicon_preview(bytes: &[u8], background: &str) -> EngineResult<Value> {
    engine_shared::error::require_bytes(bytes, "image")?;
    let img = image::load_from_memory(bytes)
        .map_err(|e| EngineError::unsupported_format(format!("Cannot decode image: {e}")))?;
    engine_shared::limits::check_dimensions(img.width(), img.height())?;
    let (w, h) = img.dimensions();
    let mut issues = Vec::new();
    if w != h {
        issues.push(format!("Image is {w}×{h}; favicons should be square"));
    }
    if w < 512 {
        issues.push(format!(
            "Source is only {w}px wide; provide at least 512×512 for crisp large icons"
        ));
    }
    let rgba = img.to_rgba8();
    let transparent = rgba.pixels().any(|p| p[3] < 250);
    let sizes: [(u32, &str, &str); 7] = [
        (16, "favicon-16x16.png", "browser tab"),
        (32, "favicon-32x32.png", "taskbar / high-DPI tab"),
        (48, "favicon-48x48.png", "Windows site icon"),
        (180, "apple-touch-icon.png", "iOS home screen"),
        (192, "android-chrome-192x192.png", "Android home screen"),
        (512, "android-chrome-512x512.png", "PWA splash"),
        (64, "favicon-64x64.png", "legacy / Safari pinned"),
    ];
    let previews: Vec<Value> = sizes
        .iter()
        .map(|(s, name, usage)| {
            let resized = img
                .resize_exact(*s, *s, image::imageops::FilterType::Lanczos3)
                .to_rgba8();
            let mut cursor = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(resized)
                .write_to(&mut cursor, image::ImageFormat::Png)
                .map_err(|e| EngineError::internal(e.to_string()))?;
            Ok(json!({
                "size": s,
                "filename": name,
                "usage": usage,
                "png": engine_shared::b64::encode(&cursor.into_inner())
            }))
        })
        .collect::<EngineResult<_>>()?;
    let small = img
        .resize_exact(16, 16, image::imageops::FilterType::Lanczos3)
        .to_luma8();
    let contrast_16: f64 = {
        let vals: Vec<f64> = small.pixels().map(|p| p[0] as f64).collect();
        let mean = vals.iter().sum::<f64>() / vals.len() as f64;
        (vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / vals.len() as f64).sqrt()
    };
    if contrast_16 < 25.0 {
        issues.push("Very low contrast at 16×16 – simplify the design for small sizes".into());
    }
    let bg = if background.is_empty() {
        "#ffffff".to_string()
    } else {
        parse(background)?.hex()
    };
    Ok(json!({
        "source": { "width": w, "height": h, "transparent": transparent },
        "previews": previews,
        "issues": issues,
        "html": "<link rel=\"icon\" type=\"image/png\" sizes=\"32x32\" \
                 href=\"/favicon-32x32.png\">\n<link rel=\"icon\" type=\"image/png\" \
                 sizes=\"16x16\" href=\"/favicon-16x16.png\">\n<link rel=\"apple-touch-icon\" \
                 sizes=\"180x180\" href=\"/apple-touch-icon.png\">\n<link rel=\"manifest\" \
                 href=\"/site.webmanifest\">",
        "manifest": {
            "icons": [
                { "src": "/android-chrome-192x192.png", "sizes": "192x192", "type": "image/png" },
                { "src": "/android-chrome-512x512.png", "sizes": "512x512", "type": "image/png" }
            ]
        },
        "preview_background": bg,
        "tip": "Use the image engine `favicon` action to produce the multi-size .ico file",
    }))
}

/// `(platform, kind, width, height, notes)` table of social image sizes.
pub const SOCIAL_SIZES: &[(&str, &str, u32, u32, &str)] = &[
    ("Open Graph (Facebook/LinkedIn link preview)", "og:image", 1200, 630, "1.91:1"),
    ("Twitter / X summary large image", "twitter:image", 1200, 628, "1.91:1"),
    ("Twitter / X post image", "post", 1600, 900, "16:9"),
    ("Instagram square", "post", 1080, 1080, "1:1"),
    ("Instagram portrait", "post", 1080, 1350, "4:5"),
    ("Instagram story / Reels cover", "story", 1080, 1920, "9:16"),
    ("Facebook cover", "cover", 820, 312, "2.63:1"),
    ("Facebook post", "post", 1200, 630, "1.91:1"),
    ("LinkedIn post", "post", 1200, 627, "1.91:1"),
    ("LinkedIn company cover", "cover", 1128, 191, "5.9:1"),
    ("YouTube thumbnail", "thumbnail", 1280, 720, "16:9"),
    ("YouTube channel art", "cover", 2560, 1440, "16:9"),
    ("Pinterest pin", "post", 1000, 1500, "2:3"),
    ("TikTok video cover", "story", 1080, 1920, "9:16"),
    ("WhatsApp profile", "avatar", 500, 500, "1:1"),
    ("Google Business post", "post", 1200, 900, "4:3"),
    ("Email header", "banner", 600, 200, "3:1"),
];

/// Lists social image sizes and checks which ones an image (or size) fits.
pub fn social_dimensions(
    width: Option<u32>,
    height: Option<u32>,
    image: Option<&[u8]>,
) -> EngineResult<Value> {
    let (w, h) = match (width, height, image) {
        (Some(w), Some(h), _) => (w, h),
        (_, _, Some(bytes)) if !bytes.is_empty() => {
            let img = image::load_from_memory(bytes).map_err(|e| {
                EngineError::unsupported_format(format!("Cannot decode image: {e}"))
            })?;
            img.dimensions()
        },
        _ => {
            return Ok(json!({
                "platforms": SOCIAL_SIZES.iter().map(|(n, k, w, h, r)| json!({ "platform": n, "kind": k, "width": w, "height": h, "ratio": r })).collect::<Vec<_>>()
            }))
        },
    };
    if w == 0 || h == 0 {
        return Err(EngineError::invalid("width and height must be > 0"));
    }
    let ratio = w as f64 / h as f64;
    let results: Vec<Value> = SOCIAL_SIZES
        .iter()
        .map(|(n, k, tw, th, r)| {
            let tr = *tw as f64 / *th as f64;
            let ratio_ok = (ratio - tr).abs() / tr < 0.02;
            let size_ok = w >= *tw && h >= *th;
            let status = if ratio_ok && size_ok {
                "perfect"
            } else if ratio_ok {
                "correct ratio, too small"
            } else if size_ok {
                "large enough, needs cropping"
            } else {
                "needs resize and crop"
            };
            let (cw, ch) = if ratio > tr {
                ((h as f64 * tr).round() as u32, h)
            } else {
                (w, (w as f64 / tr).round() as u32)
            };
            json!({
                "platform": n,
                "kind": k,
                "width": tw,
                "height": th,
                "ratio": r,
                "status": status,
                "fits": ratio_ok && size_ok,
                "crop_to": { "width": cw, "height": ch, "x": (w - cw) / 2, "y": (h - ch) / 2 },
                "scale_factor": r2((*tw as f64 / cw as f64).min(*th as f64 / ch as f64))
            })
        })
        .collect();
    let perfect: Vec<&str> = results
        .iter()
        .filter(|r| r["fits"] == true)
        .filter_map(|r| r["platform"].as_str())
        .collect();
    Ok(json!({
        "width": w,
        "height": h,
        "ratio": r2(ratio),
        "megapixels": r2(w as f64 * h as f64 / 1e6),
        "perfect_for": perfect,
        "platforms": results
    }))
}

// --------------------------------------------------------------- dispatch --

/// Every action understood by the colour/design engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum ColorAction {
    /// Parses a colour and returns every representation.
    Convert {
        /// Colour in any CSS syntax.
        color: String,
    },
    /// Colour scheme generation.
    Palette {
        /// Base colour.
        color: String,
        /// `monochromatic` (default), `analogous`, `complementary`, `split`, `triadic`, `tetradic`,
        /// `shades`, `tints`, `tones`, `material` or `random`.
        #[serde(default)]
        scheme: String,
        /// Number of colours.
        #[serde(default)]
        count: usize,
        /// Seed for the `random` scheme.
        #[serde(default)]
        seed: Option<u64>,
    },
    /// CSS gradient generation.
    Gradient {
        /// Colour stops.
        stops: Vec<Stop>,
        /// `linear` (default), `radial` or `conic`.
        #[serde(default)]
        kind: String,
        /// Angle in degrees (linear/conic).
        #[serde(default)]
        angle: Option<f64>,
        /// Number of interpolated steps returned.
        #[serde(default)]
        steps: usize,
        /// `rgb` (default), `hsl`, `lab` or `oklch` interpolation.
        #[serde(default)]
        interpolation: String,
        /// Radial shape (`circle`, `ellipse`).
        #[serde(default)]
        shape: String,
    },
    /// WCAG contrast check.
    Contrast {
        /// Foreground colour.
        foreground: String,
        /// Background colour.
        background: String,
    },
    /// Contrast matrix for a set of colours.
    AccessiblePairs {
        /// Colours.
        colors: Vec<String>,
        /// `AA` (default) or `AAA`.
        #[serde(default)]
        level: String,
        /// Suggest lightened/darkened alternatives for failing pairs.
        #[serde(default = "default_true")]
        suggest_fixes: bool,
    },
    /// Colour-blindness simulation.
    SimulateColorBlindness {
        /// Swatch colours.
        #[serde(default)]
        colors: Vec<String>,
        /// Image (base64) to simulate.
        #[serde(default, with = "engine_shared::b64::option")]
        image: Option<Vec<u8>>,
        /// Deficiency types (`protanopia`, `deuteranopia`, `tritanopia`, `achromatopsia`…; all when
        /// empty).
        #[serde(default)]
        types: Vec<String>,
    },
    /// Image palette extraction.
    ImagePalette {
        /// Image bytes (base64).
        #[serde(with = "engine_shared::b64")]
        image: Vec<u8>,
        /// Palette size.
        #[serde(default)]
        count: usize,
    },
    /// `box-shadow` builder.
    Shadow {
        /// Explicit layers.
        #[serde(default)]
        layers: Vec<ShadowLayer>,
        /// `medium` (default), `small`, `large`, `xl`, `inner`, `soft`, `neumorphic` or `hard`
        /// (used
        /// when `layers` is empty).
        #[serde(default)]
        preset: String,
        /// Shadow colour for presets.
        #[serde(default)]
        color: String,
    },
    /// `border-radius` builder.
    BorderRadius {
        /// Top-left radius.
        #[serde(default)]
        top_left: f64,
        /// Top-right radius.
        #[serde(default)]
        top_right: f64,
        /// Bottom-right radius.
        #[serde(default)]
        bottom_right: f64,
        /// Bottom-left radius.
        #[serde(default)]
        bottom_left: f64,
        /// Same radius for every corner.
        #[serde(default)]
        all: Option<f64>,
        /// CSS unit (`px` default, `%`, `rem`).
        #[serde(default)]
        unit: String,
        /// Vertical radii for elliptical corners.
        #[serde(default)]
        elliptical: Option<[f64; 4]>,
    },
    /// CSS unit conversion.
    CssUnits {
        /// Value to convert.
        value: f64,
        /// `px`, `rem`, `em`, `pt`, `pc`, `in`, `cm`, `mm`, `q`, `vw` or `vh`.
        from: String,
        /// Target units (all when empty).
        #[serde(default)]
        to: Vec<String>,
        /// Root font size in px.
        #[serde(default)]
        base_font_px: f64,
        /// Viewport width in px.
        #[serde(default)]
        viewport_width: f64,
        /// Viewport height in px.
        #[serde(default)]
        viewport_height: f64,
        /// Parent font size in px (for `em`).
        #[serde(default)]
        parent_font_px: f64,
    },
    /// Spacing scale.
    SpacingScale {
        /// Base size (default 4).
        #[serde(default)]
        base: Option<f64>,
        /// Ratio between steps (default 2 for a 4/8/16 scale).
        #[serde(default)]
        ratio: Option<f64>,
        /// Number of steps.
        #[serde(default)]
        steps: usize,
        /// `px` (default) or `rem`.
        #[serde(default)]
        unit: String,
        /// Naming (`numeric` default, `tshirt`).
        #[serde(default)]
        names: String,
    },
    /// Type scale.
    TypeScale {
        /// Base font size (default 16).
        #[serde(default)]
        base: Option<f64>,
        /// Ratio as a number or a name (`minor-third`, `perfect-fourth`, `golden`…).
        #[serde(default)]
        ratio: Value,
        /// Steps above the base.
        #[serde(default)]
        steps_up: Option<usize>,
        /// Steps below the base.
        #[serde(default)]
        steps_down: Option<usize>,
        /// Line-height multiplier.
        #[serde(default)]
        line_height: f64,
        /// `px` (default) or `rem`.
        #[serde(default)]
        unit: String,
    },
    /// Favicon preview.
    FaviconPreview {
        /// Image bytes (base64).
        #[serde(with = "engine_shared::b64")]
        image: Vec<u8>,
        /// Background colour.
        #[serde(default)]
        background: String,
    },
    /// Social image dimensions.
    SocialDimensions {
        /// Image width to check.
        #[serde(default)]
        width: Option<u32>,
        /// Image height to check.
        #[serde(default)]
        height: Option<u32>,
        /// Image (base64) whose size is checked.
        #[serde(default, with = "engine_shared::b64::option")]
        image: Option<Vec<u8>>,
    },
}

fn default_true() -> bool {
    true
}

/// Dispatches a [`ColorAction`] to its implementation.
pub fn handle_action(action: ColorAction) -> EngineResult<Value> {
    match action {
        ColorAction::Convert { color } => color_core::convert(&color),
        ColorAction::Palette { color, scheme, count, seed } => {
            palette(&color, &scheme, if count == 0 { 6 } else { count }, seed)
        },
        ColorAction::Gradient {
            stops,
            kind,
            angle,
            steps,
            interpolation,
            shape,
        } => gradient(
            &stops,
            &kind,
            angle.unwrap_or(90.0),
            if steps == 0 { 10 } else { steps },
            &interpolation,
            &shape,
        ),
        ColorAction::Contrast { foreground, background } => {
            color_core::contrast(&foreground, &background)
        },
        ColorAction::AccessiblePairs { colors, level, suggest_fixes } => {
            accessible_pairs(&colors, &level, suggest_fixes)
        },
        ColorAction::SimulateColorBlindness { colors, image, types } => {
            simulate(&colors, image.as_deref(), &types)
        },
        ColorAction::ImagePalette { image, count } => {
            image_palette(&image, if count == 0 { 6 } else { count })
        },
        ColorAction::Shadow { layers, preset, color } => shadow(&layers, &preset, &color),
        ColorAction::BorderRadius {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
            all,
            unit,
            elliptical,
        } => match all {
            Some(a) => border_radius(a, a, a, a, &unit, elliptical),
            None => {
                border_radius(top_left, top_right, bottom_right, bottom_left, &unit, elliptical)
            },
        },
        ColorAction::CssUnits {
            value,
            from,
            to,
            base_font_px,
            viewport_width,
            viewport_height,
            parent_font_px,
        } => css_units(
            value,
            &from,
            &to,
            base_font_px,
            viewport_width,
            viewport_height,
            parent_font_px,
        ),
        ColorAction::SpacingScale { base, ratio, steps, unit, names } => spacing_scale(
            base.unwrap_or(16.0),
            ratio.unwrap_or(1.5),
            if steps == 0 { 10 } else { steps },
            &unit,
            &names,
        ),
        ColorAction::TypeScale {
            base,
            ratio,
            steps_up,
            steps_down,
            line_height,
            unit,
        } => type_scale(
            base.unwrap_or(16.0),
            &ratio,
            steps_up.unwrap_or(6),
            steps_down.unwrap_or(2),
            line_height,
            &unit,
        ),
        ColorAction::FaviconPreview { image, background } => favicon_preview(&image, &background),
        ColorAction::SocialDimensions { width, height, image } => {
            social_dimensions(width, height, image.as_deref())
        },
    }
}

engine_shared::export_engine!(handle_action, ColorAction);

#[cfg(test)]
mod tests {
    use super::*;

    fn png(color: [u8; 4], w: u32, h: u32) -> Vec<u8> {
        let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba(color));
        for y in 0..h / 2 {
            for x in 0..w {
                img.put_pixel(x, y, image::Rgba([0, 0, 255, 255]));
            }
        }
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn palettes_and_gradients() {
        let p = palette("#3366cc", "triadic", 6, None).unwrap();
        assert_eq!(p["colors"].as_array().unwrap().len(), 6);
        assert!(p["css"].as_str().unwrap().contains("--color-1"));
        assert_eq!(palette("#3366cc", "material", 0, None).unwrap()["colors"][5]["name"], "500");
        assert!(palette("#3366cc", "weird", 5, None).is_err());
        let r1 = palette("#3366cc", "random", 4, Some(7)).unwrap();
        let r2 = palette("#3366cc", "random", 4, Some(7)).unwrap();
        assert_eq!(r1["colors"], r2["colors"]);
        let g = gradient(
            &[
                Stop { color: "#ff0000".into(), position: None },
                Stop { color: "#0000ff".into(), position: None },
            ],
            "linear",
            90.0,
            3,
            "rgb",
            "",
        )
        .unwrap();
        assert_eq!(g["css_value"], "linear-gradient(90deg, #ff0000 0%, #0000ff 100%)");
        assert_eq!(g["samples"][1]["hex"], "#800080");
        assert!(gradient(
            &[Stop { color: "#fff".into(), position: None }],
            "linear",
            0.0,
            3,
            "",
            ""
        )
        .is_err());
        let hsl = gradient(
            &[
                Stop { color: "#ff0000".into(), position: None },
                Stop { color: "#0000ff".into(), position: None },
            ],
            "linear",
            90.0,
            3,
            "hsl",
            "",
        )
        .unwrap();
        assert_ne!(hsl["samples"][1]["hex"], "#800080");
    }

    #[test]
    fn accessibility_and_cvd() {
        let a =
            accessible_pairs(&["#ffffff".into(), "#777777".into(), "#000000".into()], "AA", true)
                .unwrap();
        assert!(a["passing_pairs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["foreground"] == "#000000" && p["background"] == "#ffffff"));
        assert!(a["failing_pairs"][0]["suggested_foreground"].is_string());
        let s = simulate(&["#ff0000".into(), "#00aa00".into()], None, &["deuteranopia".into()])
            .unwrap();
        assert_eq!(s["simulations"][0]["type"], "deuteranopia");
        assert_ne!(s["simulations"][0]["colors"][0], "#ff0000");
        let s2 = simulate(
            &["#d95f02".into(), "#1b9e77".into(), "#7570b3".into()],
            None,
            &["achromatopsia".into()],
        )
        .unwrap();
        assert!(s2["simulations"][0]["min_distance"].as_f64().unwrap() < 30.0);
        let img = png([255, 0, 0, 255], 20, 20);
        let s = simulate(&[], Some(&img), &["protanopia".into()]).unwrap();
        assert!(s["images"][0]["png"].is_string());
        assert!(simulate(&[], None, &[]).is_err());
        assert!(simulate(&["#fff".into()], None, &["nope".into()]).is_err());
        let pal = image_palette(&img, 2).unwrap();
        let names: Vec<&str> = pal["colors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["nearest_name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"red") && names.contains(&"blue"), "{names:?}");
    }

    #[test]
    fn css_generators() {
        let s = shadow(&[], "lg", "#112233").unwrap();
        assert!(s["css"]
            .as_str()
            .unwrap()
            .starts_with("box-shadow: 0px 10px 15px -3px rgba(17, 34, 51, 0.1)"));
        assert!(shadow(&[], "nope", "").is_err());
        assert_eq!(border_radius(8.0, 8.0, 8.0, 8.0, "", None).unwrap()["tailwind"], "rounded-lg");
        assert_eq!(border_radius(4.0, 8.0, 4.0, 8.0, "px", None).unwrap()["value"], "4px 8px");
        assert!(border_radius(-1.0, 0.0, 0.0, 0.0, "", None).is_err());
        let u = css_units(24.0, "px", &[], 16.0, 0.0, 0.0, 0.0).unwrap();
        assert_eq!(u["conversions"]["rem"], 1.5);
        assert_eq!(u["conversions"]["pt"], 18.0);
        assert!(css_units(1.0, "parsecs", &[], 16.0, 0.0, 0.0, 0.0).is_err());
        let sp = spacing_scale(4.0, 2.0, 6, "", "").unwrap();
        assert_eq!(sp["steps"].as_array().unwrap().len(), 6);
        let ts = type_scale(16.0, &json!("perfect fourth"), 4, 1, 1.5, "").unwrap();
        assert_eq!(ts["ratio"], 1.333);
        assert_eq!(ts["steps"].as_array().unwrap().len(), 6);
        assert!(type_scale(16.0, &json!("weird"), 4, 1, 1.5, "").is_err());
        let fav = favicon_preview(&png([0, 0, 0, 255], 64, 64), "").unwrap();
        assert_eq!(fav["previews"].as_array().unwrap().len(), 7);
        assert!(fav["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i.as_str().unwrap().contains("512")));
        let sd = social_dimensions(Some(1200), Some(630), None).unwrap();
        assert!(sd["perfect_for"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p.as_str().unwrap().contains("Open Graph")));
        assert!(social_dimensions(None, None, None).unwrap()["platforms"].is_array());
    }
}
