//! # color-core
//!
//! Colour parsing and conversion (hex / rgb / hsl / hsv / cmyk / lab / lch /
//! CSS names), WCAG contrast and harmony palettes.

use engine_shared::{EngineError, EngineResult};
use serde_json::{json, Value};

/// CSS named colours (the 148 keywords, `rebeccapurple` included).
pub const NAMED: &[(&str, u32)] = &[
    ("aliceblue", 0xF0F8FF),
    ("antiquewhite", 0xFAEBD7),
    ("aqua", 0x00FFFF),
    ("aquamarine", 0x7FFFD4),
    ("azure", 0xF0FFFF),
    ("beige", 0xF5F5DC),
    ("bisque", 0xFFE4C4),
    ("black", 0x000000),
    ("blanchedalmond", 0xFFEBCD),
    ("blue", 0x0000FF),
    ("blueviolet", 0x8A2BE2),
    ("brown", 0xA52A2A),
    ("burlywood", 0xDEB887),
    ("cadetblue", 0x5F9EA0),
    ("chartreuse", 0x7FFF00),
    ("chocolate", 0xD2691E),
    ("coral", 0xFF7F50),
    ("cornflowerblue", 0x6495ED),
    ("cornsilk", 0xFFF8DC),
    ("crimson", 0xDC143C),
    ("cyan", 0x00FFFF),
    ("darkblue", 0x00008B),
    ("darkcyan", 0x008B8B),
    ("darkgoldenrod", 0xB8860B),
    ("darkgray", 0xA9A9A9),
    ("darkgreen", 0x006400),
    ("darkgrey", 0xA9A9A9),
    ("darkkhaki", 0xBDB76B),
    ("darkmagenta", 0x8B008B),
    ("darkolivegreen", 0x556B2F),
    ("darkorange", 0xFF8C00),
    ("darkorchid", 0x9932CC),
    ("darkred", 0x8B0000),
    ("darksalmon", 0xE9967A),
    ("darkseagreen", 0x8FBC8F),
    ("darkslateblue", 0x483D8B),
    ("darkslategray", 0x2F4F4F),
    ("darkslategrey", 0x2F4F4F),
    ("darkturquoise", 0x00CED1),
    ("darkviolet", 0x9400D3),
    ("deeppink", 0xFF1493),
    ("deepskyblue", 0x00BFFF),
    ("dimgray", 0x696969),
    ("dimgrey", 0x696969),
    ("dodgerblue", 0x1E90FF),
    ("firebrick", 0xB22222),
    ("floralwhite", 0xFFFAF0),
    ("forestgreen", 0x228B22),
    ("fuchsia", 0xFF00FF),
    ("gainsboro", 0xDCDCDC),
    ("ghostwhite", 0xF8F8FF),
    ("gold", 0xFFD700),
    ("goldenrod", 0xDAA520),
    ("gray", 0x808080),
    ("green", 0x008000),
    ("greenyellow", 0xADFF2F),
    ("grey", 0x808080),
    ("honeydew", 0xF0FFF0),
    ("hotpink", 0xFF69B4),
    ("indianred", 0xCD5C5C),
    ("indigo", 0x4B0082),
    ("ivory", 0xFFFFF0),
    ("khaki", 0xF0E68C),
    ("lavender", 0xE6E6FA),
    ("lavenderblush", 0xFFF0F5),
    ("lawngreen", 0x7CFC00),
    ("lemonchiffon", 0xFFFACD),
    ("lightblue", 0xADD8E6),
    ("lightcoral", 0xF08080),
    ("lightcyan", 0xE0FFFF),
    ("lightgoldenrodyellow", 0xFAFAD2),
    ("lightgray", 0xD3D3D3),
    ("lightgreen", 0x90EE90),
    ("lightgrey", 0xD3D3D3),
    ("lightpink", 0xFFB6C1),
    ("lightsalmon", 0xFFA07A),
    ("lightseagreen", 0x20B2AA),
    ("lightskyblue", 0x87CEFA),
    ("lightslategray", 0x778899),
    ("lightslategrey", 0x778899),
    ("lightsteelblue", 0xB0C4DE),
    ("lightyellow", 0xFFFFE0),
    ("lime", 0x00FF00),
    ("limegreen", 0x32CD32),
    ("linen", 0xFAF0E6),
    ("magenta", 0xFF00FF),
    ("maroon", 0x800000),
    ("mediumaquamarine", 0x66CDAA),
    ("mediumblue", 0x0000CD),
    ("mediumorchid", 0xBA55D3),
    ("mediumpurple", 0x9370DB),
    ("mediumseagreen", 0x3CB371),
    ("mediumslateblue", 0x7B68EE),
    ("mediumspringgreen", 0x00FA9A),
    ("mediumturquoise", 0x48D1CC),
    ("mediumvioletred", 0xC71585),
    ("midnightblue", 0x191970),
    ("mintcream", 0xF5FFFA),
    ("mistyrose", 0xFFE4E1),
    ("moccasin", 0xFFE4B5),
    ("navajowhite", 0xFFDEAD),
    ("navy", 0x000080),
    ("oldlace", 0xFDF5E6),
    ("olive", 0x808000),
    ("olivedrab", 0x6B8E23),
    ("orange", 0xFFA500),
    ("orangered", 0xFF4500),
    ("orchid", 0xDA70D6),
    ("palegoldenrod", 0xEEE8AA),
    ("palegreen", 0x98FB98),
    ("paleturquoise", 0xAFEEEE),
    ("palevioletred", 0xDB7093),
    ("papayawhip", 0xFFEFD5),
    ("peachpuff", 0xFFDAB9),
    ("peru", 0xCD853F),
    ("pink", 0xFFC0CB),
    ("plum", 0xDDA0DD),
    ("powderblue", 0xB0E0E6),
    ("purple", 0x800080),
    ("rebeccapurple", 0x663399),
    ("red", 0xFF0000),
    ("rosybrown", 0xBC8F8F),
    ("royalblue", 0x4169E1),
    ("saddlebrown", 0x8B4513),
    ("salmon", 0xFA8072),
    ("sandybrown", 0xF4A460),
    ("seagreen", 0x2E8B57),
    ("seashell", 0xFFF5EE),
    ("sienna", 0xA0522D),
    ("silver", 0xC0C0C0),
    ("skyblue", 0x87CEEB),
    ("slateblue", 0x6A5ACD),
    ("slategray", 0x708090),
    ("slategrey", 0x708090),
    ("snow", 0xFFFAFA),
    ("springgreen", 0x00FF7F),
    ("steelblue", 0x4682B4),
    ("tan", 0xD2B48C),
    ("teal", 0x008080),
    ("thistle", 0xD8BFD8),
    ("tomato", 0xFF6347),
    ("turquoise", 0x40E0D0),
    ("violet", 0xEE82EE),
    ("wheat", 0xF5DEB3),
    ("white", 0xFFFFFF),
    ("whitesmoke", 0xF5F5F5),
    ("yellow", 0xFFFF00),
    ("yellowgreen", 0x9ACD32),
];

/// An sRGB colour with alpha (`r`, `g`, `b` in 0–255, `a` in 0–1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    /// Red 0–255.
    pub r: f64,
    /// Green 0–255.
    pub g: f64,
    /// Blue 0–255.
    pub b: f64,
    /// Alpha 0–1.
    pub a: f64,
}

fn r1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}
fn r3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

impl Rgba {
    /// Builds a colour, clamping channels to their ranges.
    pub fn new(r: f64, g: f64, b: f64, a: f64) -> Self {
        Rgba {
            r: r.clamp(0.0, 255.0),
            g: g.clamp(0.0, 255.0),
            b: b.clamp(0.0, 255.0),
            a: a.clamp(0.0, 1.0),
        }
    }

    /// Builds an opaque colour from a `0xRRGGBB` integer.
    pub fn from_hex_u32(v: u32) -> Self {
        Rgba::new(((v >> 16) & 0xFF) as f64, ((v >> 8) & 0xFF) as f64, (v & 0xFF) as f64, 1.0)
    }

    /// `#rrggbb` (alpha ignored).
    pub fn hex(&self) -> String {
        format!(
            "#{:02x}{:02x}{:02x}",
            self.r.round() as u8,
            self.g.round() as u8,
            self.b.round() as u8
        )
    }

    /// `#rrggbbaa`.
    pub fn hex8(&self) -> String {
        format!("{}{:02x}", self.hex(), (self.a * 255.0).round() as u8)
    }

    /// Hue (0–360), saturation (0–100) and lightness (0–100).
    pub fn hsl(&self) -> (f64, f64, f64) {
        let (r, g, b) = (self.r / 255.0, self.g / 255.0, self.b / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        if (max - min).abs() < 1e-9 {
            return (0.0, 0.0, l * 100.0);
        }
        let d = max - min;
        let s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };
        let h = hue_of(r, g, b, max, d);
        (h, s * 100.0, l * 100.0)
    }

    /// Hue (0–360), saturation (0–100) and value (0–100).
    pub fn hsv(&self) -> (f64, f64, f64) {
        let (r, g, b) = (self.r / 255.0, self.g / 255.0, self.b / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let d = max - min;
        let s = if max == 0.0 { 0.0 } else { d / max };
        let h = if d.abs() < 1e-9 { 0.0 } else { hue_of(r, g, b, max, d) };
        (h, s * 100.0, max * 100.0)
    }

    /// CMYK components in 0–100.
    pub fn cmyk(&self) -> (f64, f64, f64, f64) {
        let (r, g, b) = (self.r / 255.0, self.g / 255.0, self.b / 255.0);
        let k = 1.0 - r.max(g).max(b);
        if (1.0 - k).abs() < 1e-9 {
            return (0.0, 0.0, 0.0, 100.0);
        }
        (
            (1.0 - r - k) / (1.0 - k) * 100.0,
            (1.0 - g - k) / (1.0 - k) * 100.0,
            (1.0 - b - k) / (1.0 - k) * 100.0,
            k * 100.0,
        )
    }

    fn linear(c: f64) -> f64 {
        let c = c / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    /// Relative luminance (WCAG 2.x).
    pub fn luminance(&self) -> f64 {
        0.2126 * Self::linear(self.r)
            + 0.7152 * Self::linear(self.g)
            + 0.0722 * Self::linear(self.b)
    }

    /// CIE XYZ (D65).
    pub fn xyz(&self) -> (f64, f64, f64) {
        let (r, g, b) = (Self::linear(self.r), Self::linear(self.g), Self::linear(self.b));
        (
            r * 0.4124564 + g * 0.3575761 + b * 0.1804375,
            r * 0.2126729 + g * 0.7151522 + b * 0.0721750,
            r * 0.0193339 + g * 0.1191920 + b * 0.9503041,
        )
    }

    /// CIE L*a*b* (D65).
    pub fn lab(&self) -> (f64, f64, f64) {
        let (x, y, z) = self.xyz();
        let f = |t: f64| {
            if t > 0.008856 {
                t.cbrt()
            } else {
                7.787 * t + 16.0 / 116.0
            }
        };
        let (fx, fy, fz) = (f(x / 0.95047), f(y / 1.0), f(z / 1.08883));
        (116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz))
    }

    /// CIE LCh (lightness, chroma, hue).
    pub fn lch(&self) -> (f64, f64, f64) {
        let (l, a, b) = self.lab();
        let c = (a * a + b * b).sqrt();
        let h = b.atan2(a).to_degrees().rem_euclid(360.0);
        (l, c, h)
    }

    /// Builds from HSL (`h` 0–360, `s`/`l` 0–100).
    pub fn from_hsl(h: f64, s: f64, l: f64, a: f64) -> Self {
        let (s, l) = (s / 100.0, l / 100.0);
        let h = h.rem_euclid(360.0) / 360.0;
        if s == 0.0 {
            return Rgba::new(l * 255.0, l * 255.0, l * 255.0, a);
        }
        let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
        let p = 2.0 * l - q;
        let hue = |mut t: f64| {
            if t < 0.0 {
                t += 1.0;
            }
            if t > 1.0 {
                t -= 1.0;
            }
            if t < 1.0 / 6.0 {
                p + (q - p) * 6.0 * t
            } else if t < 0.5 {
                q
            } else if t < 2.0 / 3.0 {
                p + (q - p) * (2.0 / 3.0 - t) * 6.0
            } else {
                p
            }
        };
        Rgba::new(hue(h + 1.0 / 3.0) * 255.0, hue(h) * 255.0, hue(h - 1.0 / 3.0) * 255.0, a)
    }

    /// Builds from HSV (`h` 0–360, `s`/`v` 0–100).
    pub fn from_hsv(h: f64, s: f64, v: f64, a: f64) -> Self {
        let (s, v) = (s / 100.0, v / 100.0);
        let h = h.rem_euclid(360.0) / 60.0;
        let i = h.floor();
        let f = h - i;
        let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
        let (r, g, b) = match i as i32 % 6 {
            0 => (v, t, p),
            1 => (q, v, p),
            2 => (p, v, t),
            3 => (p, q, v),
            4 => (t, p, v),
            _ => (v, p, q),
        };
        Rgba::new(r * 255.0, g * 255.0, b * 255.0, a)
    }

    /// Builds from CMYK (0–100 each).
    pub fn from_cmyk(c: f64, m: f64, y: f64, k: f64, a: f64) -> Self {
        let (c, m, y, k) = (c / 100.0, m / 100.0, y / 100.0, k / 100.0);
        Rgba::new(
            255.0 * (1.0 - c) * (1.0 - k),
            255.0 * (1.0 - m) * (1.0 - k),
            255.0 * (1.0 - y) * (1.0 - k),
            a,
        )
    }

    /// Builds from CIE Lab (D65).
    pub fn from_lab(l: f64, a: f64, b: f64, alpha: f64) -> Self {
        let fy = (l + 16.0) / 116.0;
        let fx = a / 500.0 + fy;
        let fz = fy - b / 200.0;
        let inv = |t: f64| {
            if t.powi(3) > 0.008856 {
                t.powi(3)
            } else {
                (t - 16.0 / 116.0) / 7.787
            }
        };
        let (x, y, z) = (inv(fx) * 0.95047, inv(fy), inv(fz) * 1.08883);
        let rl = x * 3.2404542 + y * -1.5371385 + z * -0.4985314;
        let gl = x * -0.9692660 + y * 1.8760108 + z * 0.0415560;
        let bl = x * 0.0556434 + y * -0.2040259 + z * 1.0572252;
        let gamma = |c: f64| {
            let c = c.clamp(0.0, 1.0);
            if c <= 0.0031308 {
                12.92 * c
            } else {
                1.055 * c.powf(1.0 / 2.4) - 0.055
            }
        };
        Rgba::new(gamma(rl) * 255.0, gamma(gl) * 255.0, gamma(bl) * 255.0, alpha)
    }

    /// Exact CSS colour name, if any.
    pub fn name(&self) -> Option<&'static str> {
        let v = ((self.r.round() as u32) << 16)
            | ((self.g.round() as u32) << 8)
            | self.b.round() as u32;
        NAMED.iter().find(|(_, c)| *c == v).map(|(n, _)| *n)
    }

    /// Nearest CSS colour name and its ΔE distance.
    pub fn nearest_name(&self) -> (&'static str, f64) {
        let (l1, a1, b1) = self.lab();
        let mut best = ("black", f64::MAX);
        for (name, hex) in NAMED {
            let c = Rgba::from_hex_u32(*hex);
            let (l2, a2, b2) = c.lab();
            let d = ((l1 - l2).powi(2) + (a1 - a2).powi(2) + (b1 - b2).powi(2)).sqrt();
            if d < best.1 {
                best = (name, d);
            }
        }
        best
    }

    /// All representations (hex, rgb, hsl, hsv, cmyk, lab, lch, name) as JSON.
    pub fn to_json(&self) -> Value {
        let (h, s, l) = self.hsl();
        let (hv, sv, v) = self.hsv();
        let (c, m, y, k) = self.cmyk();
        let (ll, la, lb) = self.lab();
        let (lc_l, lc_c, lc_h) = self.lch();
        let has_alpha = self.a < 1.0;
        let (r, g, b) = (self.r.round() as u8, self.g.round() as u8, self.b.round() as u8);
        let (near, dist) = self.nearest_name();
        json!({
            "hex": self.hex(),
            "hex8": self.hex8(),
            "rgb": { "r": r, "g": g, "b": b, "a": r3(self.a) },
            "hsl": { "h": r1(h), "s": r1(s), "l": r1(l) },
            "hsv": { "h": r1(hv), "s": r1(sv), "v": r1(v) },
            "cmyk": { "c": r1(c), "m": r1(m), "y": r1(y), "k": r1(k) },
            "lab": { "l": r1(ll), "a": r1(la), "b": r1(lb) },
            "lch": { "l": r1(lc_l), "c": r1(lc_c), "h": r1(lc_h) },
            "css": {
                "hex": if has_alpha { self.hex8() } else { self.hex() },
                "rgb": if has_alpha { format!("rgba({r}, {g}, {b}, {})", r3(self.a)) } else { format!("rgb({r}, {g}, {b})") },
                "rgb_modern": format!("rgb({r} {g} {b}{})", if has_alpha { format!(" / {}", r3(self.a)) } else { String::new() }),
                "hsl": if has_alpha { format!("hsla({}, {}%, {}%, {})", r1(h), r1(s), r1(l), r3(self.a)) } else { format!("hsl({}, {}%, {}%)", r1(h), r1(s), r1(l)) },
                "lab": format!("lab({}% {} {})", r1(ll), r1(la), r1(lb)),
                "lch": format!("lch({}% {} {})", r1(lc_l), r1(lc_c), r1(lc_h)),
            },
            "name": self.name(),
            "nearest_name": { "name": near, "distance": r1(dist), "exact": dist < 0.01 },
            "luminance": r3(self.luminance()),
            "is_dark": self.luminance() < 0.179,
            "decimal": ((r as u32) << 16) | ((g as u32) << 8) | b as u32,
            "android": format!("0x{:02X}{:02X}{:02X}{:02X}", (self.a * 255.0).round() as u8, r, g, b),
            "swift": format!("UIColor(red: {:.3}, green: {:.3}, blue: {:.3}, alpha: {:.3})", self.r / 255.0, self.g / 255.0, self.b / 255.0, self.a),
            "flutter": format!("Color(0x{:02X}{:02X}{:02X}{:02X})", (self.a * 255.0).round() as u8, r, g, b),
        })
    }
}

fn hue_of(r: f64, g: f64, b: f64, max: f64, d: f64) -> f64 {
    let h = if (max - r).abs() < 1e-9 {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() < 1e-9 {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0).rem_euclid(360.0)
}

fn num(s: &str, what: &str) -> EngineResult<f64> {
    let t = s.trim();
    let t = t.strip_suffix('%').unwrap_or(t);
    let t = t.trim_end_matches("deg");
    t.trim()
        .parse()
        .map_err(|_| EngineError::invalid(format!("`{s}` is not a number in {what}")))
}

fn alpha_of(s: &str) -> EngineResult<f64> {
    let t = s.trim();
    if let Some(p) = t.strip_suffix('%') {
        return Ok(p
            .trim()
            .parse::<f64>()
            .map_err(|_| EngineError::invalid("Invalid alpha"))?
            / 100.0);
    }
    t.parse::<f64>()
        .map_err(|_| EngineError::invalid(format!("`{s}` is not a valid alpha value")))
}

fn split_args(inner: &str) -> Vec<String> {
    inner
        .replace('/', " ")
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Parses any supported colour notation.
pub fn parse(input: &str) -> EngineResult<Rgba> {
    let t = input.trim();
    engine_shared::error::require_text(t, "color")?;
    let lower = t.to_ascii_lowercase();
    if let Some((_, v)) = NAMED.iter().find(|(n, _)| *n == lower) {
        return Ok(Rgba::from_hex_u32(*v));
    }
    if lower == "transparent" {
        return Ok(Rgba::new(0.0, 0.0, 0.0, 0.0));
    }
    let hex = lower
        .strip_prefix('#')
        .or_else(|| lower.strip_prefix("0x"))
        .unwrap_or(&lower);
    if hex.chars().all(|c| c.is_ascii_hexdigit())
        && matches!(hex.len(), 3 | 4 | 6 | 8)
        && (lower.starts_with('#') || lower.starts_with("0x") || hex.len() == 6 || hex.len() == 3)
    {
        // All characters were verified to be hex digits above, so parsing cannot fail;
        // `unwrap_or(0)` merely keeps the closure total.
        let expand = |s: &str| -> Vec<u8> {
            if s.len() <= 4 {
                s.chars()
                    .map(|c| u8::from_str_radix(&format!("{c}{c}"), 16).unwrap_or(0))
                    .collect()
            } else {
                (0..s.len() / 2)
                    .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap_or(0))
                    .collect()
            }
        };
        let v = expand(hex);
        return Ok(Rgba::new(
            v[0] as f64,
            v[1] as f64,
            v[2] as f64,
            v.get(3).map(|a| *a as f64 / 255.0).unwrap_or(1.0),
        ));
    }
    if let Some(open) = lower.find('(') {
        let func = lower[..open].trim();
        let inner = lower[open + 1..].trim_end_matches(')');
        let args = split_args(inner);
        let a =
            |i: usize| -> EngineResult<f64> { args.get(i).map(|s| alpha_of(s)).unwrap_or(Ok(1.0)) };
        let n = |i: usize| -> EngineResult<f64> {
            args.get(i).map(|s| num(s, func)).unwrap_or_else(|| {
                Err(EngineError::invalid(format!("{func}() needs more components")))
            })
        };
        return match func {
            "rgb" | "rgba" => {
                let pct = args.first().map(|s| s.ends_with('%')).unwrap_or(false);
                let scale = if pct { 2.55 } else { 1.0 };
                Ok(Rgba::new(n(0)? * scale, n(1)? * scale, n(2)? * scale, a(3)?))
            },
            "hsl" | "hsla" => Ok(Rgba::from_hsl(n(0)?, n(1)?, n(2)?, a(3)?)),
            "hsv" | "hsva" | "hsb" | "hsba" => Ok(Rgba::from_hsv(n(0)?, n(1)?, n(2)?, a(3)?)),
            "cmyk" => Ok(Rgba::from_cmyk(n(0)?, n(1)?, n(2)?, n(3)?, a(4)?)),
            "lab" => Ok(Rgba::from_lab(n(0)?, n(1)?, n(2)?, a(3)?)),
            "lch" => {
                let (l, c, h) = (n(0)?, n(1)?, n(2)?);
                Ok(Rgba::from_lab(l, c * h.to_radians().cos(), c * h.to_radians().sin(), a(3)?))
            },
            other => Err(EngineError::unsupported_format(format!(
                "Unknown colour function `{other}()` (rgb, hsl, hsv, cmyk, lab, lch)"
            ))),
        };
    }
    // Bare "r, g, b" or "r g b"
    let parts = split_args(&lower);
    if parts.len() == 3 || parts.len() == 4 {
        if let Ok(vals) = parts.iter().map(|p| num(p, "rgb")).collect::<EngineResult<Vec<_>>>() {
            return Ok(Rgba::new(vals[0], vals[1], vals[2], vals.get(3).copied().unwrap_or(1.0)));
        }
    }
    Err(EngineError::invalid(format!(
        "Cannot parse colour `{input}` (try #rrggbb, rgb(), hsl(), hsv(), cmyk(), lab() or a CSS \
         name)"
    )))
}

/// WCAG 2.x contrast ratio (1–21).
pub fn contrast_ratio(a: &Rgba, b: &Rgba) -> f64 {
    let (l1, l2) = (a.luminance(), b.luminance());
    let (hi, lo) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (hi + 0.05) / (lo + 0.05)
}

/// Contrast ratio plus AA/AAA pass flags for normal and large text.
pub fn contrast_json(fg: &Rgba, bg: &Rgba) -> Value {
    let ratio = contrast_ratio(fg, bg);
    json!({
        "ratio": (ratio * 100.0).round() / 100.0,
        "display": format!("{:.2}:1", ratio),
        "wcag": {
            "aa_normal_text": ratio >= 4.5,
            "aa_large_text": ratio >= 3.0,
            "aaa_normal_text": ratio >= 7.0,
            "aaa_large_text": ratio >= 4.5,
            "ui_components": ratio >= 3.0,
        },
        "grade": if ratio >= 7.0 { "AAA" } else if ratio >= 4.5 { "AA" } else if ratio >= 3.0 { "AA Large" } else { "Fail" },
    })
}

/// Full conversion report for one colour.
pub fn convert(input: &str) -> EngineResult<Value> {
    let c = parse(input)?;
    let mut v = c.to_json();
    v["input"] = json!(input);
    let white = Rgba::new(255.0, 255.0, 255.0, 1.0);
    let black = Rgba::new(0.0, 0.0, 0.0, 1.0);
    v["contrast"] = json!({
        "on_white": contrast_json(&c, &white),
        "on_black": contrast_json(&c, &black),
        "best_text_color": if contrast_ratio(&c, &white) >= contrast_ratio(&c, &black) { "#ffffff" } else { "#000000" }
    });
    let (h, s, l) = c.hsl();
    let shade = |dl: f64| Rgba::from_hsl(h, s, (l + dl).clamp(0.0, 100.0), c.a).hex();
    let gray = 0.299 * c.r + 0.587 * c.g + 0.114 * c.b;
    let grayscale = Rgba::new(gray, gray, gray, c.a);
    v["harmony"] = json!({
        "complementary": [Rgba::from_hsl(h + 180.0, s, l, c.a).hex()],
        "analogous": [
            Rgba::from_hsl(h - 30.0, s, l, c.a).hex(),
            Rgba::from_hsl(h + 30.0, s, l, c.a).hex()
        ],
        "triadic": [
            Rgba::from_hsl(h + 120.0, s, l, c.a).hex(),
            Rgba::from_hsl(h + 240.0, s, l, c.a).hex()
        ],
        "split_complementary": [
            Rgba::from_hsl(h + 150.0, s, l, c.a).hex(),
            Rgba::from_hsl(h + 210.0, s, l, c.a).hex()
        ],
        "tetradic": [
            Rgba::from_hsl(h + 90.0, s, l, c.a).hex(),
            Rgba::from_hsl(h + 180.0, s, l, c.a).hex(),
            Rgba::from_hsl(h + 270.0, s, l, c.a).hex()
        ],
        "tints": (1..=5).map(|i| shade(i as f64 * (100.0 - l) / 6.0)).collect::<Vec<_>>(),
        "shades": (1..=5).map(|i| shade(-(i as f64) * l / 6.0)).collect::<Vec<_>>(),
        "grayscale": grayscale.hex(),
        "inverted": Rgba::new(255.0 - c.r, 255.0 - c.g, 255.0 - c.b, c.a).hex(),
    });
    Ok(v)
}

/// Contrast between two colours, with suggestions to reach AA when failing.
pub fn contrast(foreground: &str, background: &str) -> EngineResult<Value> {
    let fg = parse(foreground)?;
    let bg = parse(background)?;
    let mut v = contrast_json(&fg, &bg);
    v["foreground"] = json!(fg.hex());
    v["background"] = json!(bg.hex());
    if contrast_ratio(&fg, &bg) < 4.5 {
        // Walk lightness of the foreground until AA is reached.
        let (h, s, l) = fg.hsl();
        let darker = (0..=20)
            .map(|i| Rgba::from_hsl(h, s, (l - i as f64 * 5.0).max(0.0), 1.0))
            .find(|c| contrast_ratio(c, &bg) >= 4.5);
        let lighter = (0..=20)
            .map(|i| Rgba::from_hsl(h, s, (l + i as f64 * 5.0).min(100.0), 1.0))
            .find(|c| contrast_ratio(c, &bg) >= 4.5);
        v["suggestions"] = json!({
            "darker_foreground": darker.map(|c| c.hex()),
            "lighter_foreground": lighter.map(|c| c.hex())
        });
    }
    Ok(v)
}

/// Mixes two colours in RGB space.
pub fn mix(a: &str, b: &str, weight: f64) -> EngineResult<Value> {
    let (ca, cb) = (parse(a)?, parse(b)?);
    let w = weight.clamp(0.0, 1.0);
    let m = Rgba::new(
        ca.r * (1.0 - w) + cb.r * w,
        ca.g * (1.0 - w) + cb.g * w,
        ca.b * (1.0 - w) + cb.b * w,
        ca.a * (1.0 - w) + cb.a * w,
    );
    Ok(json!({
        "result": m.to_json(),
        "weight": w,
        "steps": (0..=10).map(|i| { let w = i as f64 / 10.0; Rgba::new(ca.r * (1.0 - w) + cb.r * w, ca.g * (1.0 - w) + cb.g * w, ca.b * (1.0 - w) + cb.b * w, 1.0).hex() }).collect::<Vec<_>>()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing_and_conversion() {
        let c = convert("#336699").unwrap();
        assert_eq!(c["rgb"]["r"], 51);
        assert_eq!(c["hsl"]["h"], 210.0);
        assert_eq!(c["hsl"]["s"], 50.0);
        assert_eq!(c["hsl"]["l"], 40.0);
        assert_eq!(c["cmyk"]["k"], 40.0);
        assert_eq!(c["css"]["rgb"], "rgb(51, 102, 153)");
        assert_eq!(parse("rgb(51 102 153 / 50%)").unwrap().a, 0.5);
        assert_eq!(parse("hsl(210, 50%, 40%)").unwrap().hex(), "#336699");
        assert_eq!(parse("rebeccapurple").unwrap().hex(), "#663399");
        assert_eq!(parse("#abc").unwrap().hex(), "#aabbcc");
        assert_eq!(parse("cmyk(0%, 100%, 100%, 0%)").unwrap().hex(), "#ff0000");
        let lab = parse("#ff0000").unwrap().lab();
        assert!((lab.0 - 53.24).abs() < 0.1);
        assert_eq!(Rgba::from_lab(lab.0, lab.1, lab.2, 1.0).hex(), "#ff0000");
        assert!(parse("notacolor").is_err());
        assert!(parse("").is_err());
        let k = contrast("#777777", "#ffffff").unwrap();
        assert_eq!(k["ratio"], 4.48);
        assert_eq!(k["wcag"]["aa_normal_text"], false);
        assert!(k["suggestions"]["darker_foreground"].is_string());
        assert_eq!(convert("white").unwrap()["contrast"]["on_black"]["ratio"], 21.0);
        assert_eq!(mix("#000000", "#ffffff", 0.5).unwrap()["result"]["hex"], "#808080");
    }
}
