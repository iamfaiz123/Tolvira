//! # Dev Toolbox Units Engine (`units-engine`)
//!
//! | Action | Tool(s) |
//! |---|---|
//! | `convert` | every *-converter tool (length, weight, area, volume, speed, time, pressure, energy, power, data-size, data-rate, fuel-economy, angle, force, density, frequency, torque) plus the convenience tools (feet-to-meters, kg-to-pounds, mb-to-gb, …) |
//! | `temperature` | temperature-converter, celsius-to-fahrenheit |
//! | `cooking` | cooking-unit-converter, recipe-measurement-converter (volume ⇄ weight by ingredient) |
//! | `shoe_size` | shoe-size-converter |
//! | `paper_size` | paper-size-converter |
//! | `pixels` | pixels-to-inches, dpi-calculator |
//! | `categories` / `units` | discovery of the factor tables (versioned) |
//!
//! All factors are exact NIST/ISO definitions (see `tables.rs`,
//! `TABLE_VERSION`). Results are `f64`; rounding is left to the caller
//! (`precision` rounds to significant digits when supplied).

pub mod tables;

use engine_shared::{EngineError, EngineResult};
use serde::Deserialize;
use serde_json::{json, Value};
use tables::{Category, Unit, CATEGORIES};

fn norm(s: &str) -> String {
    s.trim().to_lowercase().replace("sq ", "sq").replace(['_', ' '], "")
}

fn find_unit<'a>(cat: &'a Category, name: &str) -> Option<&'a Unit> {
    let n = norm(name);
    cat.units
        .iter()
        .find(|u| norm(u.id) == n || norm(u.name) == n || u.aliases.iter().any(|a| norm(a) == n))
}

fn find_category(name: &str) -> Option<&'static Category> {
    let n = norm(name);
    CATEGORIES.iter().copied().find(|c| {
        norm(c.id) == n
            || norm(c.name) == n
            || (n == "weight" && c.id == "mass")
            || (n == "datasize" && c.id == "data")
            || (n == "storage" && c.id == "data")
            || (n == "bandwidth" && c.id == "data_rate")
            || (n == "fuel" && c.id == "fuel_economy")
            || (n == "rotation" && c.id == "frequency")
    })
}

/// Finds a category containing both units when none is given.
fn resolve<'a>(
    category: Option<&str>,
    from: &str,
    to: &str,
) -> EngineResult<(&'a Category, &'a Unit, &'a Unit)> {
    if let Some(c) = category.filter(|c| !c.trim().is_empty()) {
        let cat = find_category(c).ok_or_else(|| {
            EngineError::invalid(format!(
                "Unknown category `{c}`. Available: {}",
                CATEGORIES.iter().map(|c| c.id).collect::<Vec<_>>().join(", ")
            ))
        })?;
        let f = find_unit(cat, from)
            .ok_or_else(|| EngineError::invalid(format!("Unknown {} unit `{from}`", cat.name)))?;
        let t = find_unit(cat, to)
            .ok_or_else(|| EngineError::invalid(format!("Unknown {} unit `{to}`", cat.name)))?;
        return Ok((cat, f, t));
    }
    let mut matches: Vec<(&Category, &Unit, &Unit)> = Vec::new();
    for cat in CATEGORIES {
        if let (Some(f), Some(t)) = (find_unit(cat, from), find_unit(cat, to)) {
            matches.push((cat, f, t));
        }
    }
    match matches.len() {
        0 => {
            let from_cats: Vec<&str> = CATEGORIES
                .iter()
                .filter(|c| find_unit(c, from).is_some())
                .map(|c| c.id)
                .collect();
            let to_cats: Vec<&str> = CATEGORIES
                .iter()
                .filter(|c| find_unit(c, to).is_some())
                .map(|c| c.id)
                .collect();
            if from_cats.is_empty() {
                Err(EngineError::invalid(format!("Unknown unit `{from}`")))
            } else if to_cats.is_empty() {
                Err(EngineError::invalid(format!("Unknown unit `{to}`")))
            } else {
                Err(EngineError::invalid(format!(
                    "Cannot convert {} ({}) to {} ({})",
                    from,
                    from_cats.join("/"),
                    to,
                    to_cats.join("/")
                )))
            }
        },
        1 => Ok(matches[0]),
        _ => Err(EngineError::invalid(format!(
            "Ambiguous units; specify `category` (one of {})",
            matches.iter().map(|m| m.0.id).collect::<Vec<_>>().join(", ")
        ))),
    }
}

fn to_base(cat: &Category, unit: &Unit, v: f64) -> f64 {
    match (cat.id, unit.id) {
        ("fuel_economy", "l/100km") => {
            if v == 0.0 {
                f64::INFINITY
            } else {
                100.0 / v
            }
        },
        ("speed", "min/km") => {
            if v == 0.0 {
                f64::INFINITY
            } else {
                1000.0 / (v * 60.0)
            }
        },
        ("speed", "min/mi") => {
            if v == 0.0 {
                f64::INFINITY
            } else {
                1609.344 / (v * 60.0)
            }
        },
        ("power", "dbm") => 10f64.powf(v / 10.0) / 1000.0,
        _ => v * unit.factor,
    }
}

fn from_base(cat: &Category, unit: &Unit, b: f64) -> f64 {
    match (cat.id, unit.id) {
        ("fuel_economy", "l/100km") => {
            if b == 0.0 {
                f64::INFINITY
            } else {
                100.0 / b
            }
        },
        ("speed", "min/km") => {
            if b == 0.0 {
                f64::INFINITY
            } else {
                1000.0 / b / 60.0
            }
        },
        ("speed", "min/mi") => {
            if b == 0.0 {
                f64::INFINITY
            } else {
                1609.344 / b / 60.0
            }
        },
        ("power", "dbm") => 10.0 * (b * 1000.0).log10(),
        _ => b / unit.factor,
    }
}

fn round_sig(v: f64, sig: Option<usize>) -> f64 {
    match sig {
        Some(s) if v.is_finite() && v != 0.0 => {
            let s = s.clamp(1, 15) as i32;
            let mag = v.abs().log10().floor() as i32;
            let f = 10f64.powi(s - 1 - mag);
            (v * f).round() / f
        },
        _ => v,
    }
}

/// Converts a value between two units of the same category (auto-detects the category from the unit
/// names).
pub fn convert(
    value: f64,
    from: &str,
    to: &str,
    category: Option<&str>,
    precision: Option<usize>,
) -> EngineResult<Value> {
    if !value.is_finite() {
        return Err(EngineError::invalid("Value must be a finite number"));
    }
    if from.trim().is_empty() || to.trim().is_empty() {
        return Err(EngineError::empty("unit"));
    }
    let (cat, f, t) = resolve(category, from, to)?;
    let base = to_base(cat, f, value);
    let result = from_base(cat, t, base);
    let factor = if f.factor.is_finite() && t.factor.is_finite() {
        Some(f.factor / t.factor)
    } else {
        None
    };
    let all: Vec<Value> = cat
        .units
        .iter()
        .map(|u| {
            json!({
                "unit": u.id,
                "name": u.name,
                "value": round_sig(from_base(cat, u, base), precision.or(Some(10)))
            })
        })
        .collect();
    Ok(json!({
        "value": value,
        "from": { "unit": f.id, "name": f.name },
        "to": { "unit": t.id, "name": t.name },
        "result": round_sig(result, precision),
        "category": cat.id,
        "factor": factor,
        "formula": factor.map(|k| format!("{} {} × {} = {} {}", value, f.id, round_sig(k, Some(10)), round_sig(result, Some(10)), t.id)),
        "all_units": all,
        "table_version": tables::TABLE_VERSION,
    }))
}

/// Converts temperatures between Celsius, Fahrenheit, Kelvin and Rankine (all scales when `to` is
/// omitted).
pub fn temperature(
    value: f64,
    from: &str,
    to: Option<&str>,
    precision: Option<usize>,
) -> EngineResult<Value> {
    if !value.is_finite() {
        return Err(EngineError::invalid("Value must be a finite number"));
    }
    let unit = |s: &str| -> EngineResult<char> {
        match s
            .trim()
            .to_lowercase()
            .trim_start_matches('°')
            .trim_start_matches("deg")
            .trim()
        {
            "c" | "celsius" | "centigrade" => Ok('C'),
            "f" | "fahrenheit" => Ok('F'),
            "k" | "kelvin" => Ok('K'),
            "r" | "ra" | "rankine" => Ok('R'),
            "re" | "reaumur" | "réaumur" => Ok('E'),
            other => Err(EngineError::invalid(format!(
                "Unknown temperature unit `{other}` (C, F, K, R, Re)"
            ))),
        }
    };
    let f = unit(from)?;
    let kelvin = match f {
        'C' => value + 273.15,
        'F' => (value + 459.67) * 5.0 / 9.0,
        'K' => value,
        'R' => value * 5.0 / 9.0,
        _ => value * 1.25 + 273.15,
    };
    if kelvin < -1e-9 {
        return Err(EngineError::invalid(format!("{value} °{f} is below absolute zero")));
    }
    let c = kelvin - 273.15;
    let all = json!({
        "celsius": round_sig(c, precision),
        "fahrenheit": round_sig(c * 9.0 / 5.0 + 32.0, precision),
        "kelvin": round_sig(kelvin, precision),
        "rankine": round_sig(kelvin * 9.0 / 5.0, precision),
        "reaumur": round_sig(c * 0.8, precision),
    });
    let result = match to.map(unit).transpose()? {
        Some('C') | None => all["celsius"].clone(),
        Some('F') => all["fahrenheit"].clone(),
        Some('K') => all["kelvin"].clone(),
        Some('R') => all["rankine"].clone(),
        _ => all["reaumur"].clone(),
    };
    let note = if c <= 0.0 {
        "at or below the freezing point of water"
    } else if c >= 100.0 {
        "at or above the boiling point of water (1 atm)"
    } else if (36.0..=38.0).contains(&c) {
        "around human body temperature"
    } else {
        ""
    };
    Ok(json!({
        "value": value,
        "from": f.to_string(),
        "result": result,
        "all": all,
        "note": note,
        "formulas": {
            "c_to_f": "°F = °C × 9/5 + 32",
            "f_to_c": "°C = (°F − 32) × 5/9",
            "c_to_k": "K = °C + 273.15"
        }
    }))
}

/// Converts cooking measures, using ingredient densities for volume ↔ weight.
pub fn cooking(
    value: f64,
    from: &str,
    to: &str,
    ingredient: Option<&str>,
    precision: Option<usize>,
) -> EngineResult<Value> {
    if !value.is_finite() || value < 0.0 {
        return Err(EngineError::invalid("Value must be a non-negative number"));
    }
    let vol = &tables::VOLUME;
    let mass = &tables::MASS;
    let (fv, fm) = (find_unit(vol, from), find_unit(mass, from));
    let (tv, tm) = (find_unit(vol, to), find_unit(mass, to));
    let density = ingredient
        .map(|i| {
            let n = i.trim().to_lowercase();
            tables::INGREDIENTS
                .iter()
                .find(|(name, _)| *name == n || name.starts_with(&n))
                .map(|(_, d)| *d)
                .ok_or_else(|| {
                    EngineError::invalid(format!(
                        "Unknown ingredient `{i}`. Known: {}",
                        tables::INGREDIENTS
                            .iter()
                            .map(|(n, _)| *n)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                })
        })
        .transpose()?;
    let (result, kind) = match (fv, fm, tv, tm) {
        (Some(f), _, Some(t), _) => (value * f.factor / t.factor, "volume"),
        (_, Some(f), _, Some(t)) => (value * f.factor / t.factor, "weight"),
        (Some(f), _, _, Some(t)) => {
            let d = density.ok_or_else(|| {
                EngineError::invalid("Converting volume to weight needs an `ingredient`")
            })?;
            (value * f.factor * 1000.0 * d / 1000.0 / t.factor, "volume→weight")
        },
        (_, Some(f), Some(t), _) => {
            let d = density.ok_or_else(|| {
                EngineError::invalid("Converting weight to volume needs an `ingredient`")
            })?;
            (value * f.factor * 1000.0 / d / 1000.0 / t.factor, "weight→volume")
        },
        _ => {
            return Err(EngineError::invalid(format!(
                "Unknown cooking unit `{}`",
                if fv.is_none() && fm.is_none() { from } else { to }
            )))
        },
    };
    Ok(json!({
        "result": round_sig(result, precision),
        "kind": kind,
        "ingredient": ingredient,
        "density_g_per_ml": density,
        "from": from,
        "to": to,
        "ingredients": tables::INGREDIENTS.iter().map(|(n, _)| *n).collect::<Vec<_>>()
    }))
}

/// Converts a shoe size between US men/women, UK, EU and centimetre systems.
pub fn shoe_size(value: f64, system: &str) -> EngineResult<Value> {
    let idx = match system.trim().to_lowercase().as_str() {
        "us" | "us_men" | "us men" | "us-men" => 0,
        "us_women" | "us women" | "us-women" | "usw" => 1,
        "uk" => 2,
        "eu" | "euro" | "europe" => 3,
        "cm" | "foot" | "foot_length" => 4,
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown shoe system `{other}` (us_men, us_women, uk, eu, cm)"
            )))
        },
    };
    let pick = |row: &(f64, f64, f64, f64, f64), i: usize| match i {
        0 => row.0,
        1 => row.1,
        2 => row.2,
        3 => row.3,
        _ => row.4,
    };
    let row = tables::SHOES
        .iter()
        .min_by(|a, b| (pick(a, idx) - value).abs().total_cmp(&(pick(b, idx) - value).abs()))
        .ok_or_else(|| EngineError::internal("shoe size table is empty"))?;
    if (pick(row, idx) - value).abs()
        > if idx == 4 {
            0.5
        } else if idx == 3 {
            1.0
        } else {
            0.75
        }
    {
        return Err(EngineError::invalid(format!(
            "{value} is outside the supported {system} range"
        )));
    }
    Ok(json!({
        "input": { "system": system, "size": value },
        "us_men": row.0,
        "us_women": row.1,
        "uk": row.2,
        "eu": row.3,
        "foot_length_cm": row.4,
        "foot_length_in": (row.4 / 2.54 * 100.0).round() / 100.0,
        "note": "Sizes vary by brand; treat as approximate"
    }))
}

/// Looks up a paper size by name or finds the closest standard size for given dimensions.
pub fn paper_size(
    name: Option<&str>,
    width_mm: Option<f64>,
    height_mm: Option<f64>,
) -> EngineResult<Value> {
    let fmt = |n: &str, w: f64, h: f64, std: &str| {
        json!({
            "name": n,
            "standard": std,
            "mm": { "width": w, "height": h },
            "cm": { "width": w / 10.0, "height": h / 10.0 },
            "inches": {
                "width": (w / 25.4 * 1000.0).round() / 1000.0,
                "height": (h / 25.4 * 1000.0).round() / 1000.0
            },
            "points": {
                "width": (w / 25.4 * 72.0 * 100.0).round() / 100.0,
                "height": (h / 25.4 * 72.0 * 100.0).round() / 100.0
            },
            "pixels_300dpi": {
                "width": (w / 25.4 * 300.0).round(),
                "height": (h / 25.4 * 300.0).round()
            },
            "pixels_72dpi": {
                "width": (w / 25.4 * 72.0).round(),
                "height": (h / 25.4 * 72.0).round()
            },
            "area_m2": w * h / 1e6,
            "aspect_ratio": (h / w * 1000.0).round() / 1000.0
        })
    };
    if let Some(n) = name.filter(|n| !n.trim().is_empty()) {
        let key = n.trim().to_lowercase().replace(' ', "");
        let p = tables::PAPER
            .iter()
            .find(|(pn, _, _, _)| pn.to_lowercase().replace(' ', "") == key)
            .ok_or_else(|| EngineError::invalid(format!("Unknown paper size `{n}`")))?;
        return Ok(json!({
            "size": fmt(p.0, p.1, p.2, p.3),
            "all": tables::PAPER.iter().map(|(n, w, h, s)| json!({ "name": n, "width_mm": w, "height_mm": h, "standard": s })).collect::<Vec<_>>()
        }));
    }
    match (width_mm, height_mm) {
        (Some(w), Some(h)) if w > 0.0 && h > 0.0 => {
            let (pw, ph) = (w.min(h), w.max(h));
            let closest = tables::PAPER
                .iter()
                .min_by(|a, b| {
                    ((a.1 - pw).abs() + (a.2 - ph).abs())
                        .total_cmp(&((b.1 - pw).abs() + (b.2 - ph).abs()))
                })
                .ok_or_else(|| EngineError::internal("paper size table is empty"))?;
            let exact = (closest.1 - pw).abs() < 1.5 && (closest.2 - ph).abs() < 1.5;
            Ok(json!({
                "size": fmt(if exact { closest.0 } else { "custom" }, w, h, if exact { closest.3 } else { "" }),
                "closest": closest.0,
                "exact_match": exact
            }))
        },
        _ => Ok(json!({
            "all": tables::PAPER.iter().map(|(n, w, h, s)| json!({ "name": n, "width_mm": w, "height_mm": h, "standard": s })).collect::<Vec<_>>()
        })),
    }
}

/// Pixel/inch/DPI and PPI calculations.
pub fn pixels(
    pixels: Option<f64>,
    inches: Option<f64>,
    dpi: Option<f64>,
    width_px: Option<f64>,
    height_px: Option<f64>,
    diagonal_in: Option<f64>,
) -> EngineResult<Value> {
    // Screen DPI from resolution + diagonal.
    if let (Some(w), Some(h), Some(d)) = (width_px, height_px, diagonal_in) {
        if w <= 0.0 || h <= 0.0 || d <= 0.0 {
            return Err(EngineError::invalid("Resolution and diagonal must be positive"));
        }
        let ppi = (w * w + h * h).sqrt() / d;
        let g = {
            let (mut a, mut b) = (w as u64, h as u64);
            while b != 0 {
                let t = b;
                b = a % b;
                a = t;
            }
            a.max(1)
        };
        return Ok(json!({
            "ppi": (ppi * 100.0).round() / 100.0,
            "dot_pitch_mm": (25.4 / ppi * 10000.0).round() / 10000.0,
            "aspect_ratio": format!("{}:{}", w as u64 / g, h as u64 / g),
            "width_in": (w / ppi * 100.0).round() / 100.0,
            "height_in": (h / ppi * 100.0).round() / 100.0,
            "megapixels": (w * h / 1e6 * 100.0).round() / 100.0
        }));
    }
    let dpi = dpi.unwrap_or(96.0);
    if dpi <= 0.0 {
        return Err(EngineError::invalid("dpi must be positive"));
    }
    match (pixels, inches) {
        (Some(px), _) => Ok(json!({
            "pixels": px,
            "inches": (px / dpi * 10000.0).round() / 10000.0,
            "cm": (px / dpi * 2.54 * 10000.0).round() / 10000.0,
            "mm": (px / dpi * 25.4 * 1000.0).round() / 1000.0,
            "points": (px / dpi * 72.0 * 100.0).round() / 100.0,
            "dpi": dpi
        })),
        (None, Some(inch)) => Ok(json!({
            "inches": inch,
            "pixels": (inch * dpi).round(),
            "cm": inch * 2.54,
            "dpi": dpi
        })),
        _ => Err(EngineError::invalid(
            "Provide `pixels` or `inches` (with optional `dpi`), or `width_px` + `height_px` + \
             `diagonal_in`",
        )),
    }
}

/// Lists all unit categories.
pub fn categories() -> Value {
    json!({
        "version": tables::TABLE_VERSION,
        "categories": CATEGORIES.iter().map(|c| json!({ "id": c.id, "name": c.name, "base": c.base, "units": c.units.len() })).collect::<Vec<_>>(),
        "special": ["temperature", "cooking", "shoe_size", "paper_size", "pixels"]
    })
}

/// Lists the units of one category.
pub fn units(category: &str) -> EngineResult<Value> {
    let cat = find_category(category)
        .ok_or_else(|| EngineError::invalid(format!("Unknown category `{category}`")))?;
    Ok(json!({
        "category": cat.id,
        "base": cat.base,
        "units": cat.units.iter().map(|u| json!({ "id": u.id, "name": u.name, "aliases": u.aliases, "factor": if u.factor.is_finite() { json!(u.factor) } else { json!("non-linear") } })).collect::<Vec<_>>()
    }))
}

/// Every action understood by the units engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum UnitsAction {
    /// General unit conversion.
    Convert {
        /// Value to convert.
        value: f64,
        /// Source unit id or alias.
        from: String,
        /// Target unit id or alias.
        to: String,
        /// Category hint when unit names are ambiguous.
        #[serde(default)]
        category: Option<String>,
        /// Decimal places in the result.
        #[serde(default)]
        precision: Option<usize>,
    },
    /// Temperature conversion.
    Temperature {
        /// Value to convert.
        value: f64,
        /// `c` (Celsius), `f` (Fahrenheit), `k` (Kelvin) or `r` (Rankine).
        from: String,
        /// `c`, `f`, `k` or `r` (all scales when omitted).
        #[serde(default)]
        to: Option<String>,
        /// Decimal places in the result.
        #[serde(default)]
        precision: Option<usize>,
    },
    /// Cooking measure conversion.
    Cooking {
        /// Value to convert.
        value: f64,
        /// Source unit (`cup`, `tbsp`, `g`, …).
        from: String,
        /// Target unit.
        to: String,
        /// Ingredient name for density-based conversions (`flour`, `sugar`, `butter`, …).
        #[serde(default)]
        ingredient: Option<String>,
        /// Decimal places in the result.
        #[serde(default)]
        precision: Option<usize>,
    },
    /// Shoe size conversion.
    ShoeSize {
        /// Size value.
        size: f64,
        /// Source system (`us`, `us_women`, `uk`, `eu`, `cm`).
        system: String,
    },
    /// Paper size lookup.
    PaperSize {
        /// Named size (`A4`, `Letter`, …).
        #[serde(default)]
        name: Option<String>,
        /// Width in millimetres (for closest-match lookup).
        #[serde(default)]
        width_mm: Option<f64>,
        /// Height in millimetres (for closest-match lookup).
        #[serde(default)]
        height_mm: Option<f64>,
    },
    /// Pixel / DPI calculator.
    Pixels {
        /// Pixel count.
        #[serde(default)]
        pixels: Option<f64>,
        /// Physical inches.
        #[serde(default)]
        inches: Option<f64>,
        /// Dots per inch.
        #[serde(default)]
        dpi: Option<f64>,
        /// Screen width in pixels (PPI calculation).
        #[serde(default)]
        width_px: Option<f64>,
        /// Screen height in pixels (PPI calculation).
        #[serde(default)]
        height_px: Option<f64>,
        /// Screen diagonal in inches (PPI calculation).
        #[serde(default)]
        diagonal_in: Option<f64>,
    },
    /// Lists categories.
    Categories {},
    /// Lists the units of a category.
    Units {
        /// Category id.
        category: String,
    },
}

/// Dispatches a [`UnitsAction`] to its implementation.
pub fn handle_action(action: UnitsAction) -> EngineResult<Value> {
    match action {
        UnitsAction::Convert { value, from, to, category, precision } => {
            convert(value, &from, &to, category.as_deref(), precision)
        },
        UnitsAction::Temperature { value, from, to, precision } => {
            temperature(value, &from, to.as_deref(), precision)
        },
        UnitsAction::Cooking { value, from, to, ingredient, precision } => {
            cooking(value, &from, &to, ingredient.as_deref(), precision)
        },
        UnitsAction::ShoeSize { size, system } => shoe_size(size, &system),
        UnitsAction::PaperSize { name, width_mm, height_mm } => {
            paper_size(name.as_deref(), width_mm, height_mm)
        },
        UnitsAction::Pixels {
            pixels: px,
            inches,
            dpi,
            width_px,
            height_px,
            diagonal_in,
        } => pixels(px, inches, dpi, width_px, height_px, diagonal_in),
        UnitsAction::Categories {} => Ok(categories()),
        UnitsAction::Units { category } => units(&category),
    }
}

engine_shared::export_engine!(handle_action, UnitsAction);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions_are_exact() {
        assert_eq!(convert(1.0, "mi", "km", None, None).unwrap()["result"], 1.609344);
        assert_eq!(convert(10.0, "feet", "meters", None, None).unwrap()["result"], 3.048);
        assert_eq!(convert(1.0, "kg", "lb", None, Some(8)).unwrap()["result"], 2.2046226);
        assert_eq!(convert(1.0, "acre", "ft2", None, Some(6)).unwrap()["result"], 43560.0);
        assert_eq!(convert(1.0, "gallon", "l", None, None).unwrap()["result"], 3.785411784);
        assert_eq!(convert(100.0, "km/h", "m/s", None, Some(6)).unwrap()["result"], 27.7778);
        assert_eq!(convert(1.0, "gib", "mb", None, None).unwrap()["result"], 1073.741824);
        assert_eq!(convert(100.0, "mbps", "MB/s", None, None).unwrap()["result"], 12.5);
        assert_eq!(convert(1.0, "atm", "psi", None, Some(5)).unwrap()["result"], 14.696);
        assert_eq!(convert(1.0, "kwh", "kj", None, None).unwrap()["result"], 3600.0);
        assert_eq!(
            convert(180.0, "deg", "rad", None, Some(11)).unwrap()["result"],
            (std::f64::consts::PI * 1e10).round() / 1e10
        );
        assert_eq!(convert(1.0, "hp", "w", None, Some(6)).unwrap()["result"], 745.7);
        assert_eq!(convert(1.0, "lbft", "nm", None, Some(6)).unwrap()["result"], 1.35582);
        // Non-linear units.
        assert_eq!(convert(8.0, "l/100km", "km/l", None, None).unwrap()["result"], 12.5);
        assert_eq!(convert(30.0, "mpg", "l/100km", None, Some(3)).unwrap()["result"], 7.84);
        assert_eq!(convert(5.0, "min/km", "km/h", None, Some(4)).unwrap()["result"], 12.0);
        assert_eq!(convert(30.0, "dbm", "w", None, Some(6)).unwrap()["result"], 1.0);
        // Errors.
        assert!(convert(1.0, "kg", "m", None, None).is_err());
        assert!(convert(1.0, "foo", "m", None, None).is_err());
        assert!(convert(f64::NAN, "m", "km", None, None).is_err());
        assert!(convert(1.0, "", "m", None, None).is_err());
        assert!(convert(1.0, "ton", "kg", Some("bogus"), None).is_err());
    }

    #[test]
    fn temperature_and_cooking() {
        let t = temperature(100.0, "C", Some("F"), None).unwrap();
        assert_eq!(t["result"], 212.0);
        assert_eq!(temperature(32.0, "F", Some("C"), None).unwrap()["result"], 0.0);
        assert_eq!(temperature(0.0, "K", Some("C"), Some(6)).unwrap()["result"], -273.15);
        assert!(temperature(-300.0, "C", None, None).is_err());
        assert!(temperature(1.0, "X", None, None).is_err());
        let c = cooking(1.0, "cup", "g", Some("flour"), Some(4)).unwrap();
        assert_eq!(c["result"], 125.4);
        assert_eq!(cooking(3.0, "tsp", "tbsp", None, Some(6)).unwrap()["result"], 1.0);
        assert!(cooking(1.0, "cup", "g", None, None).is_err());
        assert!(cooking(1.0, "cup", "g", Some("unobtainium"), None).is_err());
    }

    #[test]
    fn shoes_paper_pixels() {
        let s = shoe_size(9.0, "us_men").unwrap();
        assert_eq!(s["eu"], 42.5);
        assert_eq!(shoe_size(42.0, "eu").unwrap()["us_men"], 8.5);
        assert!(shoe_size(30.0, "us_men").is_err());
        let p = paper_size(Some("a4"), None, None).unwrap();
        assert_eq!(p["size"]["pixels_300dpi"]["width"], 2480.0);
        assert_eq!(paper_size(None, Some(210.0), Some(297.0)).unwrap()["closest"], "A4");
        assert!(paper_size(Some("Z9"), None, None).is_err());
        assert_eq!(pixels(Some(96.0), None, None, None, None, None).unwrap()["inches"], 1.0);
        let screen = pixels(None, None, None, Some(1920.0), Some(1080.0), Some(24.0)).unwrap();
        assert!((screen["ppi"].as_f64().unwrap() - 91.79).abs() < 0.01);
        assert_eq!(screen["aspect_ratio"], "16:9");
        assert!(pixels(None, None, None, None, None, None).is_err());
        assert!(units("length").unwrap()["units"].as_array().unwrap().len() > 10);
        assert!(units("nope").is_err());
        assert_eq!(categories()["categories"].as_array().unwrap().len(), CATEGORIES.len());
    }
}
