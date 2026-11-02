//! # Dev Toolbox Construction Engine (`construction-engine`)
//!
//! All quantities are computed in SI (metres, kg, litres) from explicit,
//! overridable assumptions (mix ratios, dry-volume factors, wastage, unit
//! weights). Every response echoes the assumptions used so estimators can
//! audit them. Regional defaults follow common Indian/ISO practice; they are
//! estimates, not engineering designs.
//!
//! | Action | Tool(s) |
//! |---|---|
//! | `concrete` | concrete-calculator, cement-calculator, sand-calculator, aggregate-calculator |
//! | `masonry` | brick-calculator, block-calculator, mortar-calculator |
//! | `plaster` | plaster-calculator |
//! | `paint` | paint-calculator |
//! | `tiles` | tile-calculator, flooring-calculator, paver-calculator |
//! | `roofing` | roofing-calculator |
//! | `steel` | steel-weight-calculator, rebar-calculator |
//! | `excavation` | excavation-calculator |
//! | `wall_area` | wall-area-calculator |
//! | `staircase` | staircase-calculator |
//! | `tank` | water-tank-capacity-calculator |
//! | `cost_estimate` | construction-cost-estimator |
//! | `unit` | quick length/area/volume conversions (ft, in, m, cm, sqft, cft) |

use engine_shared::{EngineError, EngineResult};
use serde::Deserialize;
use serde_json::{json, Value};

const CEMENT_BAG_KG: f64 = 50.0;
const CEMENT_DENSITY: f64 = 1440.0; // kg/m³
const SAND_DENSITY: f64 = 1600.0;
const AGGREGATE_DENSITY: f64 = 1500.0;
const STEEL_DENSITY: f64 = 7850.0;

fn r2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
fn r3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn positive(v: f64, what: &str) -> EngineResult<f64> {
    engine_shared::error::require_positive(v, what)
}

fn length_factor(unit: &str) -> EngineResult<f64> {
    Ok(match unit.trim().to_lowercase().as_str() {
        "" | "m" | "meter" | "metre" | "meters" | "metres" => 1.0,
        "cm" => 0.01,
        "mm" => 0.001,
        "ft" | "feet" | "foot" => 0.3048,
        "in" | "inch" | "inches" => 0.0254,
        "yd" | "yard" => 0.9144,
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown length unit `{other}` (m, cm, mm, ft, in, yd)"
            )))
        },
    })
}

fn mix_ratio(mix: &str) -> EngineResult<(f64, f64, f64, &'static str)> {
    Ok(match mix.trim().to_uppercase().replace(' ', "").as_str() {
        "" | "M20" | "1:1.5:3" => (1.0, 1.5, 3.0, "M20"),
        "M25" | "1:1:2" => (1.0, 1.0, 2.0, "M25"),
        "M15" | "1:2:4" => (1.0, 2.0, 4.0, "M15"),
        "M10" | "1:3:6" => (1.0, 3.0, 6.0, "M10"),
        "M7.5" | "1:4:8" => (1.0, 4.0, 8.0, "M7.5"),
        "M5" | "1:5:10" => (1.0, 5.0, 10.0, "M5"),
        other => {
            let parts: Vec<f64> = other.split(':').filter_map(|p| p.parse().ok()).collect();
            if parts.len() == 3 && parts.iter().all(|p| *p > 0.0) {
                (parts[0], parts[1], parts[2], "custom")
            } else {
                return Err(EngineError::invalid(format!(
                    "Unknown mix `{mix}` (M10, M15, M20, M25 or c:s:a like 1:2:4)"
                )));
            }
        },
    })
}

/// Concrete volume and material breakdown.
pub fn concrete(
    length: f64,
    width: f64,
    thickness: f64,
    unit: &str,
    thickness_unit: Option<&str>,
    mix: &str,
    quantity: f64,
    dry_factor: f64,
    wastage_percent: f64,
    cement_price: f64,
    sand_price_m3: f64,
    aggregate_price_m3: f64,
) -> EngineResult<Value> {
    let f = length_factor(unit)?;
    let tf = length_factor(thickness_unit.unwrap_or(unit))?;
    positive(length, "length")?;
    positive(width, "width")?;
    positive(thickness, "thickness")?;
    let q = if quantity <= 0.0 { 1.0 } else { quantity };
    let wet = length * f * width * f * thickness * tf * q;
    let dry_f = if dry_factor <= 0.0 { 1.54 } else { dry_factor };
    let waste = 1.0 + wastage_percent.max(0.0) / 100.0;
    let dry = wet * dry_f * waste;
    let (c, s, a, grade) = mix_ratio(mix)?;
    let total = c + s + a;
    let cement_m3 = dry * c / total;
    let sand_m3 = dry * s / total;
    let agg_m3 = dry * a / total;
    let cement_kg = cement_m3 * CEMENT_DENSITY;
    let bags = cement_kg / CEMENT_BAG_KG;
    let water_l = cement_kg * 0.5;
    let cost = if cement_price > 0.0 || sand_price_m3 > 0.0 || aggregate_price_m3 > 0.0 {
        Some(r2(bags.ceil() * cement_price
            + sand_m3 * sand_price_m3
            + agg_m3 * aggregate_price_m3))
    } else {
        None
    };
    Ok(json!({
        "wet_volume_m3": r3(wet),
        "wet_volume_cft": r2(wet * 35.3147),
        "dry_volume_m3": r3(dry),
        "mix": { "grade": grade, "ratio": format!("{c}:{s}:{a}") },
        "cement": {
            "m3": r3(cement_m3),
            "kg": r2(cement_kg),
            "bags_50kg": r2(bags),
            "bags_to_buy": bags.ceil()
        },
        "sand": {
            "m3": r3(sand_m3),
            "cft": r2(sand_m3 * 35.3147),
            "kg": r2(sand_m3 * SAND_DENSITY),
            "tonnes": r3(sand_m3 * SAND_DENSITY / 1000.0)
        },
        "aggregate": {
            "m3": r3(agg_m3),
            "cft": r2(agg_m3 * 35.3147),
            "kg": r2(agg_m3 * AGGREGATE_DENSITY),
            "tonnes": r3(agg_m3 * AGGREGATE_DENSITY / 1000.0)
        },
        "water_litres_approx": r2(water_l),
        "estimated_cost": cost,
        "assumptions": {
            "dry_volume_factor": dry_f,
            "wastage_percent": wastage_percent.max(0.0),
            "cement_density_kg_m3": CEMENT_DENSITY,
            "sand_density_kg_m3": SAND_DENSITY,
            "aggregate_density_kg_m3": AGGREGATE_DENSITY,
            "water_cement_ratio": 0.5,
            "bag_kg": CEMENT_BAG_KG
        },
    }))
}

/// Brick / block masonry: count, mortar and its cement/sand.
pub fn masonry(
    wall_length: f64,
    wall_height: f64,
    wall_thickness: f64,
    unit: &str,
    thickness_unit: Option<&str>,
    unit_type: &str,
    brick_l: f64,
    brick_w: f64,
    brick_h: f64,
    brick_unit: &str,
    mortar_joint_mm: f64,
    mortar_ratio: &str,
    wastage_percent: f64,
    openings_area: f64,
    price_per_unit: f64,
) -> EngineResult<Value> {
    let f = length_factor(unit)?;
    let tf = length_factor(thickness_unit.unwrap_or(unit))?;
    positive(wall_length, "wall_length")?;
    positive(wall_height, "wall_height")?;
    positive(wall_thickness, "wall_thickness")?;
    let (bl, bw, bh, label) = match unit_type.to_lowercase().as_str() {
        "" | "brick" | "standard-brick" => (0.19, 0.09, 0.09, "standard brick 190×90×90 mm"),
        "modular-brick" => (0.19, 0.09, 0.09, "modular brick 190×90×90 mm"),
        "traditional-brick" => (0.23, 0.114, 0.076, "traditional brick 230×114×76 mm"),
        "us-brick" => (0.194, 0.092, 0.057, "US modular brick 7⅝×3⅝×2¼ in"),
        "aac-block" | "aac" => (0.6, 0.2, 0.2, "AAC block 600×200×200 mm"),
        "concrete-block" | "cmu" | "block" => (0.4, 0.2, 0.2, "concrete block 400×200×200 mm"),
        "cmu-us" => (0.406, 0.203, 0.203, "US CMU 16×8×8 in"),
        "fly-ash-brick" => (0.23, 0.11, 0.075, "fly-ash brick 230×110×75 mm"),
        "custom" => {
            let bf = length_factor(brick_unit)?;
            if brick_l <= 0.0 || brick_w <= 0.0 || brick_h <= 0.0 {
                return Err(EngineError::invalid("custom unit needs brick_l, brick_w, brick_h"));
            }
            (brick_l * bf, brick_w * bf, brick_h * bf, "custom unit")
        },
        other => return Err(EngineError::invalid(format!("Unknown unit_type `{other}`"))),
    };
    let joint = if mortar_joint_mm <= 0.0 { 10.0 } else { mortar_joint_mm } / 1000.0;
    let wall_vol =
        (wall_length * f * wall_height * f - openings_area.max(0.0) * f * f) * wall_thickness * tf;
    if wall_vol <= 0.0 {
        return Err(EngineError::invalid("Openings exceed the wall area"));
    }
    let unit_with_mortar = (bl + joint) * (bw + joint) * (bh + joint);
    let units = wall_vol / unit_with_mortar;
    let waste = 1.0 + wastage_percent.max(0.0) / 100.0;
    let units_to_buy = (units * waste).ceil();
    let mortar_wet = wall_vol - units * bl * bw * bh;
    let mortar_dry = mortar_wet * 1.33 * waste;
    let (c, s) = match mortar_ratio.trim().replace(' ', "").as_str() {
        "" | "1:6" => (1.0, 6.0),
        "1:4" => (1.0, 4.0),
        "1:5" => (1.0, 5.0),
        "1:3" => (1.0, 3.0),
        other => {
            let p: Vec<f64> = other.split(':').filter_map(|x| x.parse().ok()).collect();
            if p.len() == 2 && p[0] > 0.0 && p[1] > 0.0 {
                (p[0], p[1])
            } else {
                return Err(EngineError::invalid("mortar_ratio must look like 1:6"));
            }
        },
    };
    let cement_m3 = mortar_dry * c / (c + s);
    let sand_m3 = mortar_dry * s / (c + s);
    let cement_kg = cement_m3 * CEMENT_DENSITY;
    Ok(json!({
        "unit": label,
        "unit_size_m": { "length": bl, "width": bw, "height": bh },
        "wall_volume_m3": r3(wall_vol),
        "wall_area_m2": r2(wall_length * f * wall_height * f - openings_area.max(0.0) * f * f),
        "units_required": r2(units),
        "units_to_buy": units_to_buy,
        "units_per_m3": r2(1.0 / unit_with_mortar),
        "units_per_m2": r2(1.0 / ((bl + joint) * (bh + joint))),
        "mortar": {
            "wet_m3": r3(mortar_wet),
            "dry_m3": r3(mortar_dry),
            "ratio": format!("{c}:{s}"),
            "cement_kg": r2(cement_kg),
            "cement_bags_50kg": (cement_kg / CEMENT_BAG_KG).ceil(),
            "sand_m3": r3(sand_m3),
            "sand_cft": r2(sand_m3 * 35.3147)
        },
        "estimated_cost": if price_per_unit > 0.0 { json!(r2(units_to_buy * price_per_unit)) } else { Value::Null },
        "assumptions": {
            "mortar_joint_mm": joint * 1000.0,
            "wastage_percent": wastage_percent.max(0.0),
            "dry_mortar_factor": 1.33
        },
    }))
}

/// Plaster: cement/sand quantities for an area, thickness and mix ratio.
pub fn plaster(
    area: f64,
    area_unit: &str,
    thickness_mm: f64,
    ratio: &str,
    sides: u32,
    wastage_percent: f64,
) -> EngineResult<Value> {
    positive(area, "area")?;
    let af = match area_unit.to_lowercase().as_str() {
        "" | "m2" | "sqm" => 1.0,
        "ft2" | "sqft" => 0.09290304,
        other => {
            return Err(EngineError::invalid(format!("Unknown area unit `{other}` (m2, sqft)")))
        },
    };
    let t = if thickness_mm <= 0.0 { 12.0 } else { thickness_mm } / 1000.0;
    let sides = sides.max(1) as f64;
    let wet = area * af * sides * t;
    let dry = wet * 1.27 * (1.0 + wastage_percent.max(0.0) / 100.0) * 1.2; // +20% for uneven surfaces
    let (c, s) = match ratio.trim().replace(' ', "").as_str() {
        "" | "1:4" => (1.0, 4.0),
        "1:6" => (1.0, 6.0),
        "1:5" => (1.0, 5.0),
        "1:3" => (1.0, 3.0),
        other => {
            let p: Vec<f64> = other.split(':').filter_map(|x| x.parse().ok()).collect();
            if p.len() == 2 && p[0] > 0.0 && p[1] > 0.0 {
                (p[0], p[1])
            } else {
                return Err(EngineError::invalid("ratio must look like 1:4"));
            }
        },
    };
    let cement_m3 = dry * c / (c + s);
    let sand_m3 = dry * s / (c + s);
    let cement_kg = cement_m3 * CEMENT_DENSITY;
    Ok(json!({
        "area_m2": r2(area * af * sides),
        "thickness_mm": t * 1000.0,
        "wet_volume_m3": r3(wet),
        "dry_volume_m3": r3(dry),
        "ratio": format!("{c}:{s}"),
        "cement_kg": r2(cement_kg),
        "cement_bags_50kg": (cement_kg / CEMENT_BAG_KG).ceil(),
        "sand_m3": r3(sand_m3),
        "sand_cft": r2(sand_m3 * 35.3147),
        "water_litres_approx": r2(cement_kg * 0.55),
        "assumptions": {
            "dry_factor": 1.27,
            "surface_irregularity_allowance": "20%",
            "wastage_percent": wastage_percent.max(0.0)
        }
    }))
}

/// Paint: litres, cans and cost for an area with coats and primer.
pub fn paint(
    area: f64,
    area_unit: &str,
    coats: u32,
    coverage_m2_per_litre: f64,
    openings_area: f64,
    primer: bool,
    can_size_litres: f64,
    price_per_litre: f64,
) -> EngineResult<Value> {
    positive(area, "area")?;
    let af = match area_unit.to_lowercase().as_str() {
        "" | "m2" | "sqm" => 1.0,
        "ft2" | "sqft" => 0.09290304,
        other => return Err(EngineError::invalid(format!("Unknown area unit `{other}`"))),
    };
    let net = (area - openings_area.max(0.0)) * af;
    if net <= 0.0 {
        return Err(EngineError::invalid("Openings exceed the area"));
    }
    let cov = if coverage_m2_per_litre <= 0.0 {
        10.0
    } else {
        coverage_m2_per_litre
    };
    let coats = coats.max(1) as f64;
    let litres = net * coats / cov;
    let primer_l = if primer { net / 9.0 } else { 0.0 };
    let can = if can_size_litres <= 0.0 { 4.0 } else { can_size_litres };
    Ok(json!({
        "net_area_m2": r2(net),
        "coats": coats,
        "paint_litres": r2(litres),
        "paint_litres_with_10pct_wastage": r2(litres * 1.1),
        "cans_to_buy": (litres * 1.1 / can).ceil(),
        "can_size_litres": can,
        "primer_litres": r2(primer_l),
        "estimated_cost": if price_per_litre > 0.0 { json!(r2(((litres * 1.1 / can).ceil() * can + primer_l) * price_per_litre)) } else { Value::Null },
        "assumptions": {
            "coverage_m2_per_litre": cov,
            "primer_coverage_m2_per_litre": 9.0,
            "wastage_percent": 10.0
        }
    }))
}

/// Tiles: count, boxes, grout and adhesive for a floor/wall with wastage.
pub fn tiles(
    area_length: f64,
    area_width: f64,
    unit: &str,
    tile_length: f64,
    tile_width: f64,
    tile_unit: &str,
    grout_mm: f64,
    wastage_percent: f64,
    tiles_per_box: u32,
    price_per_box: f64,
    pattern: &str,
) -> EngineResult<Value> {
    let f = length_factor(unit)?;
    let tf = length_factor(tile_unit)?;
    positive(area_length, "area_length")?;
    positive(area_width, "area_width")?;
    positive(tile_length, "tile_length")?;
    positive(tile_width, "tile_width")?;
    let area = area_length * f * area_width * f;
    let g = grout_mm.max(0.0) / 1000.0;
    let tile_area = (tile_length * tf + g) * (tile_width * tf + g);
    let waste = if wastage_percent <= 0.0 {
        match pattern {
            "diagonal" | "herringbone" => 15.0,
            _ => 10.0,
        }
    } else {
        wastage_percent
    };
    let count = area / tile_area;
    let to_buy = (count * (1.0 + waste / 100.0)).ceil();
    let boxes = if tiles_per_box > 0 {
        Some((to_buy / tiles_per_box as f64).ceil())
    } else {
        None
    };
    let grout_kg = if g > 0.0 {
        (tile_length * tf + tile_width * tf) / (tile_length * tf * tile_width * tf)
            * g
            * 0.006
            * 1.6
            * 1000.0
            * area
            * 0.001
    } else {
        0.0
    };
    Ok(json!({
        "area_m2": r2(area),
        "area_sqft": r2(area * 10.7639),
        "tile_size_m": { "length": tile_length * tf, "width": tile_width * tf },
        "tiles_required": r2(count),
        "tiles_to_buy": to_buy,
        "boxes_to_buy": boxes,
        "wastage_percent": waste,
        "grout_kg_approx": r2(grout_kg),
        "adhesive_kg_approx": r2(area * 4.0),
        "estimated_cost": match (boxes, price_per_box > 0.0) { (Some(b), true) => json!(r2(b * price_per_box)), _ => Value::Null },
        "assumptions": {
            "grout_mm": g * 1000.0,
            "adhesive_kg_per_m2": 4.0,
            "pattern": if pattern.is_empty() { "straight" } else { pattern }
        }
    }))
}

/// Roofing: sloped area, sheets/shingles/underlayment for a pitch and roof type.
pub fn roofing(
    length: f64,
    width: f64,
    unit: &str,
    pitch_degrees: f64,
    roof_type: &str,
    sheet_length: f64,
    sheet_width: f64,
    overlap_percent: f64,
    wastage_percent: f64,
) -> EngineResult<Value> {
    let f = length_factor(unit)?;
    positive(length, "length")?;
    positive(width, "width")?;
    if !(0.0..=75.0).contains(&pitch_degrees) {
        return Err(EngineError::invalid("pitch must be between 0 and 75 degrees"));
    }
    let plan = length * f * width * f;
    let slope_factor = 1.0 / pitch_degrees.to_radians().cos();
    let roof_area = plan
        * slope_factor
        * match roof_type {
            "hip" => 1.0,
            "flat" => 1.0,
            _ => 1.0,
        };
    let waste = 1.0 + if wastage_percent <= 0.0 { 10.0 } else { wastage_percent } / 100.0;
    let sheets = if sheet_length > 0.0 && sheet_width > 0.0 {
        let effective =
            sheet_length * f * sheet_width * f * (1.0 - overlap_percent.clamp(0.0, 50.0) / 100.0);
        Some((roof_area * waste / effective).ceil())
    } else {
        None
    };
    let squares = roof_area * 10.7639 / 100.0;
    Ok(json!({
        "plan_area_m2": r2(plan),
        "slope_factor": r3(slope_factor),
        "roof_area_m2": r2(roof_area),
        "roof_area_sqft": r2(roof_area * 10.7639),
        "roofing_squares_100sqft": r2(squares),
        "area_to_order_m2": r2(roof_area * waste),
        "sheets_to_buy": sheets,
        "ridge_length_m": r2(length * f),
        "rise_m": r2(width * f / 2.0 * pitch_degrees.to_radians().tan()),
        "asphalt_shingle_bundles": (squares * waste * 3.0).ceil(),
        "underlayment_rolls_37m2": (roof_area * waste / 37.0).ceil(),
        "assumptions": {
            "wastage_percent": (waste - 1.0) * 100.0,
            "overlap_percent": overlap_percent,
            "roof_type": if roof_type.is_empty() { "gable" } else { roof_type }
        }
    }))
}

/// Steel weight and cost for standard sections (round, flat, pipe, angle, square…).
pub fn steel(
    shape: &str,
    length: f64,
    unit: &str,
    diameter_mm: f64,
    width_mm: f64,
    thickness_mm: f64,
    height_mm: f64,
    quantity: f64,
    price_per_kg: f64,
) -> EngineResult<Value> {
    let f = length_factor(unit)?;
    positive(length, "length")?;
    let q = if quantity <= 0.0 { 1.0 } else { quantity };
    let l = length * f;
    let (area_mm2, label) = match shape.to_lowercase().as_str() {
        "" | "round" | "bar" | "rebar" | "rod" => {
            (std::f64::consts::PI * (diameter_mm / 2.0).powi(2), "round bar")
        },
        "square" => (width_mm * width_mm, "square bar"),
        "flat" | "plate" | "strip" => (width_mm * thickness_mm, "flat bar / plate"),
        "pipe" | "tube" | "hollow-round" => {
            let inner = (diameter_mm - 2.0 * thickness_mm).max(0.0);
            (std::f64::consts::PI / 4.0 * (diameter_mm.powi(2) - inner.powi(2)), "round pipe")
        },
        "square-tube" | "shs" => (
            (width_mm * width_mm) - (width_mm - 2.0 * thickness_mm).max(0.0).powi(2),
            "square hollow section",
        ),
        "rect-tube" | "rhs" => (
            (width_mm * height_mm)
                - ((width_mm - 2.0 * thickness_mm).max(0.0)
                    * (height_mm - 2.0 * thickness_mm).max(0.0)),
            "rectangular hollow section",
        ),
        "angle" | "l" => {
            ((width_mm + height_mm - thickness_mm) * thickness_mm, "equal/unequal angle")
        },
        "channel" | "c" => {
            ((height_mm + 2.0 * (width_mm - thickness_mm)) * thickness_mm, "channel")
        },
        "i-beam" | "h-beam" | "beam" => (
            (2.0 * width_mm + (height_mm - 2.0 * thickness_mm)) * thickness_mm,
            "I / H beam (uniform thickness)",
        ),
        "hex" | "hexagonal" => (2.598 * (width_mm / 2.0).powi(2), "hexagonal bar"),
        other => return Err(EngineError::invalid(format!("Unknown shape `{other}`"))),
    };
    if area_mm2 <= 0.0 || !area_mm2.is_finite() {
        return Err(EngineError::invalid(
            "Section dimensions produce a zero area; check diameter/width/thickness",
        ));
    }
    let kg_per_m = area_mm2 / 1e6 * STEEL_DENSITY;
    let weight = kg_per_m * l * q;
    let rebar_formula =
        if matches!(shape.to_lowercase().as_str(), "" | "round" | "bar" | "rebar" | "rod") {
            Some(json!({
                "d2_by_162": r3(diameter_mm * diameter_mm / 162.0),
                "note": "kg/m ≈ d²/162 for steel rebar"
            }))
        } else {
            None
        };
    Ok(json!({
        "shape": label,
        "section_area_mm2": r2(area_mm2),
        "weight_per_metre_kg": r3(kg_per_m),
        "weight_per_foot_kg": r3(kg_per_m * 0.3048),
        "total_length_m": r2(l * q),
        "total_weight_kg": r2(weight),
        "total_weight_tonnes": r3(weight / 1000.0),
        "quantity": q,
        "estimated_cost": if price_per_kg > 0.0 { json!(r2(weight * price_per_kg)) } else { Value::Null },
        "rebar_formula": rebar_formula,
        "assumptions": { "steel_density_kg_m3": STEEL_DENSITY }
    }))
}

/// Rebar: bar count, length, laps and weight for a slab mesh.
pub fn rebar(
    slab_length: f64,
    slab_width: f64,
    unit: &str,
    spacing_mm: f64,
    diameter_mm: f64,
    layers: u32,
    cover_mm: f64,
    lap_percent: f64,
    bar_length_m: f64,
    price_per_kg: f64,
) -> EngineResult<Value> {
    let f = length_factor(unit)?;
    positive(slab_length, "slab_length")?;
    positive(slab_width, "slab_width")?;
    positive(diameter_mm, "diameter_mm")?;
    let sp = if spacing_mm <= 0.0 { 150.0 } else { spacing_mm } / 1000.0;
    let cover = if cover_mm < 0.0 {
        25.0
    } else {
        if cover_mm == 0.0 {
            25.0
        } else {
            cover_mm
        }
    } / 1000.0;
    let (l, w) = (slab_length * f, slab_width * f);
    let layers = layers.max(1) as f64;
    let bars_along_length = ((w - 2.0 * cover) / sp).floor() + 1.0;
    let bars_along_width = ((l - 2.0 * cover) / sp).floor() + 1.0;
    let len_each_l = l - 2.0 * cover;
    let len_each_w = w - 2.0 * cover;
    let total_length = (bars_along_length * len_each_l + bars_along_width * len_each_w)
        * layers
        * (1.0 + lap_percent.max(0.0) / 100.0);
    let kg_per_m = diameter_mm * diameter_mm / 162.0;
    let weight = total_length * kg_per_m;
    let stock = if bar_length_m <= 0.0 { 12.0 } else { bar_length_m };
    Ok(json!({
        "bars_along_length": bars_along_length * layers,
        "bars_along_width": bars_along_width * layers,
        "bar_length_along_length_m": r2(len_each_l),
        "bar_length_along_width_m": r2(len_each_w),
        "total_length_m": r2(total_length),
        "weight_kg": r2(weight),
        "weight_per_m2_kg": r2(weight / (l * w)),
        "stock_bars_to_buy": (total_length / stock).ceil(),
        "stock_bar_length_m": stock,
        "estimated_cost": if price_per_kg > 0.0 { json!(r2(weight * price_per_kg)) } else { Value::Null },
        "assumptions": {
            "spacing_mm": sp * 1000.0,
            "cover_mm": cover * 1000.0,
            "layers": layers,
            "lap_allowance_percent": lap_percent.max(0.0),
            "kg_per_m": r3(kg_per_m)
        }
    }))
}

/// Excavation: volume, swelled volume, truck loads and cost for a soil type.
pub fn excavation(
    length: f64,
    width: f64,
    depth: f64,
    unit: &str,
    depth_unit: Option<&str>,
    swell_percent: f64,
    truck_capacity_m3: f64,
    soil: &str,
    price_per_m3: f64,
) -> EngineResult<Value> {
    let f = length_factor(unit)?;
    let df = length_factor(depth_unit.unwrap_or(unit))?;
    positive(length, "length")?;
    positive(width, "width")?;
    positive(depth, "depth")?;
    let bank = length * f * width * f * depth * df;
    let (swell, density) = match soil.to_lowercase().as_str() {
        "" | "loam" | "soil" => (if swell_percent <= 0.0 { 25.0 } else { swell_percent }, 1500.0),
        "sand" | "gravel" => (if swell_percent <= 0.0 { 12.0 } else { swell_percent }, 1600.0),
        "clay" => (if swell_percent <= 0.0 { 35.0 } else { swell_percent }, 1700.0),
        "rock" => (if swell_percent <= 0.0 { 60.0 } else { swell_percent }, 2500.0),
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown soil `{other}` (loam, sand, gravel, clay, rock)"
            )))
        },
    };
    let loose = bank * (1.0 + swell / 100.0);
    let truck = if truck_capacity_m3 <= 0.0 {
        10.0
    } else {
        truck_capacity_m3
    };
    Ok(json!({
        "bank_volume_m3": r3(bank),
        "bank_volume_cft": r2(bank * 35.3147),
        "bank_volume_yd3": r2(bank * 1.30795),
        "loose_volume_m3": r3(loose),
        "swell_percent": swell,
        "weight_tonnes": r2(bank * density / 1000.0),
        "truck_loads": (loose / truck).ceil(),
        "truck_capacity_m3": truck,
        "estimated_cost": if price_per_m3 > 0.0 { json!(r2(bank * price_per_m3)) } else { Value::Null },
        "assumptions": {
            "soil": if soil.is_empty() { "loam" } else { soil },
            "density_kg_m3": density
        }
    }))
}

/// A wall segment for [`wall_area`].
#[derive(Deserialize, Debug, Clone, Default)]
pub struct Wall {
    /// Wall length.
    pub length: f64,
    /// Wall height.
    pub height: f64,
}

/// A door/window opening subtracted from wall area.
#[derive(Deserialize, Debug, Clone, Default)]
pub struct Opening {
    /// Opening width.
    pub width: f64,
    /// Opening height.
    pub height: f64,
    /// Number of identical openings.
    #[serde(default = "one")]
    pub count: u32,
}
fn one() -> u32 {
    1
}

/// Net wall (and optional ceiling) area from wall segments or room dimensions minus openings.
pub fn wall_area(
    walls: &[Wall],
    openings: &[Opening],
    unit: &str,
    room_length: Option<f64>,
    room_width: Option<f64>,
    room_height: Option<f64>,
    include_ceiling: bool,
) -> EngineResult<Value> {
    let f = length_factor(unit)?;
    let mut gross = 0.0;
    let mut wall_rows = Vec::new();
    if let (Some(l), Some(w), Some(h)) = (room_length, room_width, room_height) {
        positive(l, "room_length")?;
        positive(w, "room_width")?;
        positive(h, "room_height")?;
        gross = 2.0 * (l + w) * h * f * f;
        wall_rows.push(
            json!({ "perimeter": r2(2.0 * (l + w) * f), "height": h * f, "area_m2": r2(gross) }),
        );
    }
    for wl in walls {
        positive(wl.length, "wall length")?;
        positive(wl.height, "wall height")?;
        let a = wl.length * f * wl.height * f;
        gross += a;
        wall_rows
            .push(json!({ "length": wl.length * f, "height": wl.height * f, "area_m2": r2(a) }));
    }
    if gross <= 0.0 {
        return Err(EngineError::invalid("Provide walls [{length, height}] or room dimensions"));
    }
    let open: f64 = openings
        .iter()
        .map(|o| o.width * f * o.height * f * o.count.max(1) as f64)
        .sum();
    let net = gross - open;
    if net <= 0.0 {
        return Err(EngineError::invalid("Openings exceed the wall area"));
    }
    let ceiling = if include_ceiling {
        room_length.zip(room_width).map(|(l, w)| l * w * f * f).unwrap_or(0.0)
    } else {
        0.0
    };
    Ok(json!({
        "walls": wall_rows,
        "gross_wall_area_m2": r2(gross),
        "openings_area_m2": r2(open),
        "net_wall_area_m2": r2(net),
        "ceiling_area_m2": r2(ceiling),
        "total_paintable_m2": r2(net + ceiling),
        "net_wall_area_sqft": r2(net * 10.7639),
        "paint_litres_two_coats": r2((net + ceiling) * 2.0 / 10.0),
        "primer_litres": r2((net + ceiling) / 9.0)
    }))
}

/// Staircase: risers, treads, run and waist-slab concrete for a total rise.
pub fn staircase(
    total_rise: f64,
    unit: &str,
    riser_target: f64,
    tread_target: f64,
    width: f64,
    waist_mm: f64,
    landing: bool,
) -> EngineResult<Value> {
    let f = length_factor(unit)?;
    positive(total_rise, "total_rise")?;
    let rise = total_rise * f;
    let rt = if riser_target <= 0.0 { 0.175 } else { riser_target * f };
    let tt = if tread_target <= 0.0 { 0.275 } else { tread_target * f };
    let risers = (rise / rt).round().max(2.0);
    let riser_h = rise / risers;
    let treads = risers - 1.0;
    let run = treads * tt;
    let slope = (riser_h / tt).atan().to_degrees();
    let comfort = 2.0 * riser_h + tt; // Blondel formula 0.60–0.65 m
    let comfortable = (0.60..=0.65).contains(&comfort);
    let w = if width <= 0.0 { 1.0 } else { width * f };
    let waist = if waist_mm <= 0.0 { 0.15 } else { waist_mm / 1000.0 };
    let stringer = (rise * rise + run * run).sqrt();
    // Concrete volume: waist slab + steps (triangular prisms).
    let conc = stringer * w * waist
        + treads * 0.5 * riser_h * tt * w
        + if landing { w * w * waist } else { 0.0 };
    Ok(json!({
        "total_rise_m": r2(rise),
        "risers": risers,
        "riser_height_m": r3(riser_h),
        "riser_height_mm": (riser_h * 1000.0).round(),
        "treads": treads,
        "tread_depth_m": r3(tt),
        "total_run_m": r2(run),
        "slope_degrees": r2(slope),
        "stringer_length_m": r2(stringer),
        "blondel_2r_plus_t_m": r3(comfort),
        "comfortable": comfortable,
        "code_check": {
            "riser_150_190mm": (0.15..=0.19).contains(&riser_h),
            "tread_min_250mm": tt >= 0.25,
            "slope_under_42deg": slope <= 42.0
        },
        "concrete_m3": r3(conc),
        "width_m": w,
        "assumptions": { "waist_thickness_mm": waist * 1000.0, "landing_included": landing }
    }))
}

/// Water tank capacity for rectangular/cylindrical shapes with wall thickness.
pub fn tank(
    shape: &str,
    length: f64,
    width: f64,
    height: f64,
    diameter: f64,
    unit: &str,
    wall_thickness: f64,
    fill_percent: f64,
) -> EngineResult<Value> {
    let f = length_factor(unit)?;
    positive(height, "height")?;
    let t = wall_thickness.max(0.0) * f;
    let (inner_vol, outer_vol, label) = match shape.to_lowercase().as_str() {
        "" | "rectangular" | "box" => {
            positive(length, "length")?;
            positive(width, "width")?;
            let (l, w, h) = (length * f, width * f, height * f);
            (
                (l - 2.0 * t).max(0.0) * (w - 2.0 * t).max(0.0) * (h - t).max(0.0),
                l * w * h,
                "rectangular",
            )
        },
        "cylindrical" | "cylinder" | "round" => {
            positive(diameter, "diameter")?;
            let (d, h) = (diameter * f, height * f);
            (
                std::f64::consts::PI / 4.0 * (d - 2.0 * t).max(0.0).powi(2) * (h - t).max(0.0),
                std::f64::consts::PI / 4.0 * d * d * h,
                "cylindrical",
            )
        },
        "horizontal-cylinder" => {
            positive(diameter, "diameter")?;
            positive(length, "length")?;
            let (d, l) = (diameter * f, length * f);
            (
                std::f64::consts::PI / 4.0
                    * (d - 2.0 * t).max(0.0).powi(2)
                    * (l - 2.0 * t).max(0.0),
                std::f64::consts::PI / 4.0 * d * d * l,
                "horizontal cylinder",
            )
        },
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown tank shape `{other}` (rectangular, cylindrical, horizontal-cylinder)"
            )))
        },
    };
    let fill = if fill_percent <= 0.0 {
        100.0
    } else {
        fill_percent.min(100.0)
    };
    let litres = inner_vol * 1000.0;
    Ok(json!({
        "shape": label,
        "capacity_litres": r2(litres),
        "capacity_m3": r3(inner_vol),
        "capacity_gallons_us": r2(litres / 3.785411784),
        "capacity_gallons_uk": r2(litres / 4.54609),
        "filled_litres": r2(litres * fill / 100.0),
        "water_weight_kg": r2(litres * fill / 100.0),
        "outer_volume_m3": r3(outer_vol),
        "wall_material_m3": r3(outer_vol - inner_vol),
        "people_served_per_day_135l": (litres / 135.0).floor(),
        "assumptions": {
            "wall_thickness_m": t,
            "fill_percent": fill,
            "litres_per_person_per_day": 135
        }
    }))
}

/// Rough building cost from built-up area, quality tier and breakdown percentages.
pub fn cost_estimate(
    built_area: f64,
    area_unit: &str,
    rate_per_unit: f64,
    floors: u32,
    quality: &str,
    contingency_percent: f64,
    breakdown: Option<serde_json::Map<String, Value>>,
) -> EngineResult<Value> {
    positive(built_area, "built_area")?;
    let af = match area_unit.to_lowercase().as_str() {
        "" | "sqft" | "ft2" => 1.0,
        "m2" | "sqm" => 10.7639,
        other => return Err(EngineError::invalid(format!("Unknown area unit `{other}`"))),
    };
    let sqft = built_area * af * floors.max(1) as f64;
    let rate = if rate_per_unit > 0.0 {
        rate_per_unit / af
    } else {
        match quality {
            "basic" | "economy" => 1500.0,
            "" | "standard" | "medium" => 2000.0,
            "premium" | "luxury" => 3000.0,
            other => {
                return Err(EngineError::invalid(format!(
                    "Unknown quality `{other}` (basic, standard, premium) or pass rate_per_unit"
                )))
            },
        }
    };
    let base = sqft * rate;
    let default_split: Vec<(&str, f64)> = vec![
        ("civil_structure", 0.35),
        ("masonry_plaster", 0.12),
        ("flooring_tiles", 0.10),
        ("doors_windows", 0.08),
        ("electrical", 0.07),
        ("plumbing_sanitary", 0.07),
        ("painting_finishing", 0.06),
        ("labour_other", 0.15),
    ];
    let split: Vec<(String, f64)> = match breakdown {
        Some(m) if !m.is_empty() => {
            let total: f64 = m.values().filter_map(Value::as_f64).sum();
            if total <= 0.0 {
                return Err(EngineError::invalid("breakdown percentages must be positive"));
            }
            m.iter()
                .filter_map(|(k, v)| v.as_f64().map(|p| (k.clone(), p / total)))
                .collect()
        },
        _ => default_split.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
    };
    let items: Vec<Value> = split
        .iter()
        .map(|(k, p)| json!({ "item": k, "share_percent": r2(p * 100.0), "cost": r2(base * p) }))
        .collect();
    let contingency = base * contingency_percent.max(0.0) / 100.0;
    Ok(json!({
        "built_area_sqft": r2(sqft),
        "built_area_m2": r2(sqft / 10.7639),
        "rate_per_sqft": r2(rate),
        "rate_per_m2": r2(rate * 10.7639),
        "base_cost": r2(base),
        "contingency": r2(contingency),
        "total_cost": r2(base + contingency),
        "cost_per_floor": r2((base + contingency) / floors.max(1) as f64),
        "breakdown": items,
        "materials_rule_of_thumb": {
            "cement_bags": (sqft * 0.4).round(),
            "steel_kg": (sqft * 4.0).round(),
            "sand_cft": (sqft * 1.8).round(),
            "aggregate_cft": (sqft * 1.35).round(),
            "bricks": (sqft * 8.0).round(),
            "note": "Per-sqft thumb rules for RCC framed residential construction; verify with a \
                     BOQ"
        },
        "assumptions": {
            "quality": if rate_per_unit > 0.0 { "custom rate" } else if quality.is_empty() { "standard" } else { quality },
            "floors": floors.max(1),
            "contingency_percent": contingency_percent.max(0.0),
            "rate_note": "Default rates are placeholders (currency-agnostic); supply \
                          rate_per_unit for your region"
        }
    }))
}

/// Converts between construction units (m, ft, in, mm, m², sqft, m³, cft, kg, lb, …).
pub fn unit_convert(value: f64, from: &str, to: &str) -> EngineResult<Value> {
    if !value.is_finite() {
        return Err(EngineError::invalid("value must be finite"));
    }
    let table: &[(&str, f64)] = &[
        ("m", 1.0),
        ("cm", 0.01),
        ("mm", 0.001),
        ("ft", 0.3048),
        ("in", 0.0254),
        ("yd", 0.9144),
        ("m2", 1.0),
        ("sqft", 0.09290304),
        ("sqyd", 0.83612736),
        ("acre", 4046.8564224),
        ("hectare", 10000.0),
        ("cent", 40.468564224),
        ("guntha", 101.1714),
        ("m3", 1.0),
        ("cft", 0.028316846592),
        ("litre", 0.001),
        ("yd3", 0.764554857984),
        ("brass", 2.8316846592),
        ("kg", 1.0),
        ("tonne", 1000.0),
        ("lb", 0.45359237),
        ("bag", 50.0),
    ];
    let kind = |u: &str| -> Option<(&'static str, f64)> {
        let n = u.trim().to_lowercase();
        let f = table.iter().find(|(k, _)| *k == n).map(|(_, f)| *f)?;
        let k = match n.as_str() {
            "m" | "cm" | "mm" | "ft" | "in" | "yd" => "length",
            "m2" | "sqft" | "sqyd" | "acre" | "hectare" | "cent" | "guntha" => "area",
            "m3" | "cft" | "litre" | "yd3" | "brass" => "volume",
            _ => "mass",
        };
        Some((k, f))
    };
    let (kf, ff) =
        kind(from).ok_or_else(|| EngineError::invalid(format!("Unknown unit `{from}`")))?;
    let (kt, tf) = kind(to).ok_or_else(|| EngineError::invalid(format!("Unknown unit `{to}`")))?;
    if kf != kt {
        return Err(EngineError::invalid(format!("Cannot convert {kf} to {kt}")));
    }
    Ok(json!({ "result": value * ff / tf, "kind": kf }))
}

fn default_unit() -> String {
    "m".into()
}
fn default_two_u() -> u32 {
    2
}

/// Every action understood by the construction engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum ConstructionAction {
    /// Concrete volume and material breakdown.
    Concrete {
        /// Slab length.
        length: f64,
        /// Slab width.
        width: f64,
        /// Slab thickness.
        thickness: f64,
        /// Length unit (`m` default, `ft`, `cm`, `in`).
        #[serde(default = "default_unit")]
        unit: String,
        /// Unit of `thickness` when different (`mm`, `in`, …).
        #[serde(default)]
        thickness_unit: Option<String>,
        /// Nominal mix (`1:2:4`, `M20`, …).
        #[serde(default)]
        mix: String,
        /// Number of identical slabs.
        #[serde(default)]
        quantity: f64,
        /// Dry-volume factor (1.54 default).
        #[serde(default)]
        dry_factor: f64,
        /// Wastage allowance.
        #[serde(default)]
        wastage_percent: f64,
        /// Price per cement bag.
        #[serde(default)]
        cement_price: f64,
        /// Sand price per m³.
        #[serde(default)]
        sand_price_m3: f64,
        /// Aggregate price per m³.
        #[serde(default)]
        aggregate_price_m3: f64,
    },
    /// Brick/block masonry quantities.
    Masonry {
        /// Wall length.
        wall_length: f64,
        /// Wall height.
        wall_height: f64,
        /// Wall thickness.
        wall_thickness: f64,
        /// `m` (default), `ft`, `cm`, `in` or `mm`.
        #[serde(default = "default_unit")]
        unit: String,
        /// Unit of `wall_thickness` when different.
        #[serde(default)]
        thickness_unit: Option<String>,
        /// `brick` (default), `modular-brick`, `concrete-block`, `aac-block`, `custom`.
        #[serde(default)]
        unit_type: String,
        /// Custom brick length.
        #[serde(default)]
        brick_l: f64,
        /// Custom brick width.
        #[serde(default)]
        brick_w: f64,
        /// Custom brick height.
        #[serde(default)]
        brick_h: f64,
        /// Unit of the custom brick dimensions.
        #[serde(default = "default_mm")]
        brick_unit: String,
        /// Mortar joint thickness.
        #[serde(default)]
        mortar_joint_mm: f64,
        /// Mortar mix ratio (`1:6`).
        #[serde(default)]
        mortar_ratio: String,
        /// Wastage allowance.
        #[serde(default)]
        wastage_percent: f64,
        /// Area of openings to subtract.
        #[serde(default)]
        openings_area: f64,
        /// Price per brick/block.
        #[serde(default)]
        price_per_unit: f64,
    },
    /// Plaster quantities.
    Plaster {
        /// Surface area.
        area: f64,
        /// Area unit (`m2` default, `sqft`).
        #[serde(default)]
        area_unit: String,
        /// Plaster thickness.
        #[serde(default)]
        thickness_mm: f64,
        /// Cement:sand ratio (`1:4`).
        #[serde(default)]
        ratio: String,
        /// Sides plastered (1 or 2).
        #[serde(default = "one")]
        sides: u32,
        /// Wastage allowance.
        #[serde(default)]
        wastage_percent: f64,
    },
    /// Paint quantities.
    Paint {
        /// Surface area.
        area: f64,
        /// Area unit.
        #[serde(default)]
        area_unit: String,
        /// Number of coats.
        #[serde(default = "default_two_u")]
        coats: u32,
        /// Paint coverage per litre.
        #[serde(default)]
        coverage_m2_per_litre: f64,
        /// Area of openings to subtract.
        #[serde(default)]
        openings_area: f64,
        /// Include a primer coat.
        #[serde(default)]
        primer: bool,
        /// Can size for rounding up.
        #[serde(default)]
        can_size_litres: f64,
        /// Price per litre.
        #[serde(default)]
        price_per_litre: f64,
    },
    /// Tile quantities.
    Tiles {
        /// Area length.
        area_length: f64,
        /// Area width.
        area_width: f64,
        /// `m` (default), `ft`, `cm`, `in` or `mm` for the area dimensions.
        #[serde(default = "default_unit")]
        unit: String,
        /// Tile length.
        tile_length: f64,
        /// Tile width.
        tile_width: f64,
        /// Unit of the tile dimensions.
        #[serde(default = "default_mm")]
        tile_unit: String,
        /// Grout gap.
        #[serde(default)]
        grout_mm: f64,
        /// Wastage allowance.
        #[serde(default)]
        wastage_percent: f64,
        /// Tiles per box.
        #[serde(default)]
        tiles_per_box: u32,
        /// Price per box.
        #[serde(default)]
        price_per_box: f64,
        /// `straight` (default), `diagonal` or `herringbone` (extra wastage).
        #[serde(default)]
        pattern: String,
    },
    /// Roofing quantities.
    Roofing {
        /// Building length.
        length: f64,
        /// Building width.
        width: f64,
        /// `m` (default), `ft`, `cm`, `in` or `mm`.
        #[serde(default = "default_unit")]
        unit: String,
        /// Roof pitch in degrees.
        #[serde(default)]
        pitch_degrees: f64,
        /// `gable` (default), `hip`, `flat`, `shed`.
        #[serde(default)]
        roof_type: String,
        /// Sheet length (for sheet counts).
        #[serde(default)]
        sheet_length: f64,
        /// Sheet width.
        #[serde(default)]
        sheet_width: f64,
        /// Sheet overlap.
        #[serde(default)]
        overlap_percent: f64,
        /// Wastage allowance.
        #[serde(default)]
        wastage_percent: f64,
    },
    /// Steel weight.
    Steel {
        /// Section (`round`, `flat`, `pipe`, `square`, `angle`, `channel`, `i-beam`…).
        #[serde(default)]
        shape: String,
        /// Length per piece.
        length: f64,
        /// `m` (default), `ft`, `cm`, `in` or `mm`.
        #[serde(default = "default_unit")]
        unit: String,
        /// Diameter for round sections.
        #[serde(default)]
        diameter_mm: f64,
        /// Width for flat/angle sections.
        #[serde(default)]
        width_mm: f64,
        /// Thickness for flat/pipe/angle sections.
        #[serde(default)]
        thickness_mm: f64,
        /// Height for angle/channel sections.
        #[serde(default)]
        height_mm: f64,
        /// Number of pieces.
        #[serde(default)]
        quantity: f64,
        /// Price per kilogram.
        #[serde(default)]
        price_per_kg: f64,
    },
    /// Rebar mesh for a slab.
    Rebar {
        /// Slab length.
        slab_length: f64,
        /// Slab width.
        slab_width: f64,
        /// `m` (default), `ft`, `cm`, `in` or `mm`.
        #[serde(default = "default_unit")]
        unit: String,
        /// Bar spacing.
        #[serde(default)]
        spacing_mm: f64,
        /// Bar diameter.
        diameter_mm: f64,
        /// Mesh layers (1 or 2).
        #[serde(default = "one")]
        layers: u32,
        /// Concrete cover.
        #[serde(default)]
        cover_mm: f64,
        /// Lap length as a percentage of bar length.
        #[serde(default)]
        lap_percent: f64,
        /// Stock bar length (12 m default).
        #[serde(default)]
        bar_length_m: f64,
        /// Price per kilogram.
        #[serde(default)]
        price_per_kg: f64,
    },
    /// Excavation volume.
    Excavation {
        /// Pit length.
        length: f64,
        /// Pit width.
        width: f64,
        /// Pit depth.
        depth: f64,
        /// `m` (default), `ft`, `cm`, `in` or `mm`.
        #[serde(default = "default_unit")]
        unit: String,
        /// Unit of `depth` when different.
        #[serde(default)]
        depth_unit: Option<String>,
        /// Bulking/swell percentage (overrides the soil preset).
        #[serde(default)]
        swell_percent: f64,
        /// Truck capacity in m³.
        #[serde(default)]
        truck_capacity_m3: f64,
        /// Soil type (`loam` default, `sand`, `clay`, `gravel`, `rock`).
        #[serde(default)]
        soil: String,
        /// Excavation price per m³.
        #[serde(default)]
        price_per_m3: f64,
    },
    /// Wall/ceiling area.
    WallArea {
        /// Wall segments.
        #[serde(default)]
        walls: Vec<Wall>,
        /// Openings to subtract.
        #[serde(default)]
        openings: Vec<Opening>,
        /// `m` (default), `ft`, `cm`, `in` or `mm`.
        #[serde(default = "default_unit")]
        unit: String,
        /// Room length (alternative to `walls`).
        #[serde(default)]
        room_length: Option<f64>,
        /// Room width (alternative to `walls`).
        #[serde(default)]
        room_width: Option<f64>,
        /// Room height (alternative to `walls`).
        #[serde(default)]
        room_height: Option<f64>,
        /// Add the ceiling area.
        #[serde(default)]
        include_ceiling: bool,
    },
    /// Staircase design.
    Staircase {
        /// Floor-to-floor height.
        total_rise: f64,
        /// `m` (default), `ft`, `cm`, `in` or `mm`.
        #[serde(default = "default_unit")]
        unit: String,
        /// Target riser height (mm).
        #[serde(default)]
        riser_target: f64,
        /// Target tread depth (mm).
        #[serde(default)]
        tread_target: f64,
        /// Stair width.
        #[serde(default)]
        width: f64,
        /// Waist slab thickness.
        #[serde(default)]
        waist_mm: f64,
        /// Include a mid landing.
        #[serde(default)]
        landing: bool,
    },
    /// Tank capacity.
    Tank {
        /// `rectangular` (default) or `cylindrical`.
        #[serde(default)]
        shape: String,
        /// Inner length.
        #[serde(default)]
        length: f64,
        /// Inner width.
        #[serde(default)]
        width: f64,
        /// Inner height.
        height: f64,
        /// Inner diameter (cylindrical).
        #[serde(default)]
        diameter: f64,
        /// `m` (default), `ft`, `cm`, `in` or `mm`.
        #[serde(default = "default_unit")]
        unit: String,
        /// Wall thickness (for outer dimensions).
        #[serde(default)]
        wall_thickness: f64,
        /// Fill level in percent.
        #[serde(default)]
        fill_percent: f64,
    },
    /// Construction cost estimate.
    CostEstimate {
        /// Built-up area.
        built_area: f64,
        /// Area unit (`sqft` default, `m2`).
        #[serde(default)]
        area_unit: String,
        /// Rate per area unit (overrides the quality preset).
        #[serde(default)]
        rate_per_unit: f64,
        /// Number of floors.
        #[serde(default = "one")]
        floors: u32,
        /// `basic`, `standard` (default) or `premium`.
        #[serde(default)]
        quality: String,
        /// Contingency allowance.
        #[serde(default)]
        contingency_percent: f64,
        /// Custom breakdown percentages (`{"structure": 40, …}`).
        #[serde(default)]
        breakdown: Option<serde_json::Map<String, Value>>,
    },
    /// Unit conversion.
    Unit {
        /// Value to convert.
        value: f64,
        /// Source unit.
        from: String,
        /// Target unit.
        to: String,
    },
}

fn default_mm() -> String {
    "mm".into()
}

/// Dispatches a [`ConstructionAction`] to its implementation.
pub fn handle_action(action: ConstructionAction) -> EngineResult<Value> {
    match action {
        ConstructionAction::Concrete {
            length,
            width,
            thickness,
            unit,
            thickness_unit,
            mix,
            quantity,
            dry_factor,
            wastage_percent,
            cement_price,
            sand_price_m3,
            aggregate_price_m3,
        } => concrete(
            length,
            width,
            thickness,
            &unit,
            thickness_unit.as_deref(),
            &mix,
            quantity,
            dry_factor,
            wastage_percent,
            cement_price,
            sand_price_m3,
            aggregate_price_m3,
        ),
        ConstructionAction::Masonry {
            wall_length,
            wall_height,
            wall_thickness,
            unit,
            thickness_unit,
            unit_type,
            brick_l,
            brick_w,
            brick_h,
            brick_unit,
            mortar_joint_mm,
            mortar_ratio,
            wastage_percent,
            openings_area,
            price_per_unit,
        } => masonry(
            wall_length,
            wall_height,
            wall_thickness,
            &unit,
            thickness_unit.as_deref(),
            &unit_type,
            brick_l,
            brick_w,
            brick_h,
            &brick_unit,
            mortar_joint_mm,
            &mortar_ratio,
            wastage_percent,
            openings_area,
            price_per_unit,
        ),
        ConstructionAction::Plaster {
            area,
            area_unit,
            thickness_mm,
            ratio,
            sides,
            wastage_percent,
        } => plaster(area, &area_unit, thickness_mm, &ratio, sides, wastage_percent),
        ConstructionAction::Paint {
            area,
            area_unit,
            coats,
            coverage_m2_per_litre,
            openings_area,
            primer,
            can_size_litres,
            price_per_litre,
        } => paint(
            area,
            &area_unit,
            coats,
            coverage_m2_per_litre,
            openings_area,
            primer,
            can_size_litres,
            price_per_litre,
        ),
        ConstructionAction::Tiles {
            area_length,
            area_width,
            unit,
            tile_length,
            tile_width,
            tile_unit,
            grout_mm,
            wastage_percent,
            tiles_per_box,
            price_per_box,
            pattern,
        } => tiles(
            area_length,
            area_width,
            &unit,
            tile_length,
            tile_width,
            &tile_unit,
            grout_mm,
            wastage_percent,
            tiles_per_box,
            price_per_box,
            &pattern,
        ),
        ConstructionAction::Roofing {
            length,
            width,
            unit,
            pitch_degrees,
            roof_type,
            sheet_length,
            sheet_width,
            overlap_percent,
            wastage_percent,
        } => roofing(
            length,
            width,
            &unit,
            pitch_degrees,
            &roof_type,
            sheet_length,
            sheet_width,
            overlap_percent,
            wastage_percent,
        ),
        ConstructionAction::Steel {
            shape,
            length,
            unit,
            diameter_mm,
            width_mm,
            thickness_mm,
            height_mm,
            quantity,
            price_per_kg,
        } => steel(
            &shape,
            length,
            &unit,
            diameter_mm,
            width_mm,
            thickness_mm,
            height_mm,
            quantity,
            price_per_kg,
        ),
        ConstructionAction::Rebar {
            slab_length,
            slab_width,
            unit,
            spacing_mm,
            diameter_mm,
            layers,
            cover_mm,
            lap_percent,
            bar_length_m,
            price_per_kg,
        } => rebar(
            slab_length,
            slab_width,
            &unit,
            spacing_mm,
            diameter_mm,
            layers,
            cover_mm,
            lap_percent,
            bar_length_m,
            price_per_kg,
        ),
        ConstructionAction::Excavation {
            length,
            width,
            depth,
            unit,
            depth_unit,
            swell_percent,
            truck_capacity_m3,
            soil,
            price_per_m3,
        } => excavation(
            length,
            width,
            depth,
            &unit,
            depth_unit.as_deref(),
            swell_percent,
            truck_capacity_m3,
            &soil,
            price_per_m3,
        ),
        ConstructionAction::WallArea {
            walls,
            openings,
            unit,
            room_length,
            room_width,
            room_height,
            include_ceiling,
        } => wall_area(
            &walls,
            &openings,
            &unit,
            room_length,
            room_width,
            room_height,
            include_ceiling,
        ),
        ConstructionAction::Staircase {
            total_rise,
            unit,
            riser_target,
            tread_target,
            width,
            waist_mm,
            landing,
        } => staircase(total_rise, &unit, riser_target, tread_target, width, waist_mm, landing),
        ConstructionAction::Tank {
            shape,
            length,
            width,
            height,
            diameter,
            unit,
            wall_thickness,
            fill_percent,
        } => tank(&shape, length, width, height, diameter, &unit, wall_thickness, fill_percent),
        ConstructionAction::CostEstimate {
            built_area,
            area_unit,
            rate_per_unit,
            floors,
            quality,
            contingency_percent,
            breakdown,
        } => cost_estimate(
            built_area,
            &area_unit,
            rate_per_unit,
            floors,
            &quality,
            contingency_percent,
            breakdown,
        ),
        ConstructionAction::Unit { value, from, to } => unit_convert(value, &from, &to),
    }
}

engine_shared::export_engine!(handle_action, ConstructionAction);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concrete_and_masonry() {
        let c = concrete(10.0, 10.0, 150.0, "ft", Some("mm"), "M20", 1.0, 0.0, 0.0, 0.0, 0.0, 0.0)
            .unwrap();
        assert!((c["wet_volume_m3"].as_f64().unwrap() - 1.394).abs() < 0.01);
        assert_eq!(c["mix"]["grade"], "M20");
        // 1 m³ M20 ≈ 8 bags cement
        let one = concrete(1.0, 1.0, 1.0, "m", None, "M20", 1.0, 0.0, 0.0, 0.0, 0.0, 0.0).unwrap();
        assert!((one["cement"]["bags_50kg"].as_f64().unwrap() - 8.06).abs() < 0.1);
        assert!(concrete(1.0, 1.0, 1.0, "m", None, "M99", 1.0, 0.0, 0.0, 0.0, 0.0, 0.0).is_err());
        assert!(concrete(0.0, 1.0, 1.0, "m", None, "", 1.0, 0.0, 0.0, 0.0, 0.0, 0.0).is_err());
        let m = masonry(
            10.0,
            3.0,
            230.0,
            "m",
            Some("mm"),
            "brick",
            0.0,
            0.0,
            0.0,
            "mm",
            10.0,
            "1:6",
            5.0,
            2.0,
            8.0,
        )
        .unwrap();
        // ~500 bricks per m³ → 6.44 m³ ≈ 3220
        assert!((m["units_per_m3"].as_f64().unwrap() - 500.0).abs() < 5.0);
        assert!(m["units_to_buy"].as_f64().unwrap() > 3200.0);
        assert!(m["mortar"]["cement_bags_50kg"].as_f64().unwrap() > 5.0);
        assert!(masonry(
            1.0, 1.0, 0.1, "m", None, "lego", 0.0, 0.0, 0.0, "mm", 0.0, "", 0.0, 0.0, 0.0
        )
        .is_err());
        assert!(masonry(
            1.0, 1.0, 0.1, "m", None, "brick", 0.0, 0.0, 0.0, "mm", 0.0, "", 0.0, 5.0, 0.0
        )
        .is_err());
        let p = plaster(100.0, "m2", 12.0, "1:4", 1, 0.0).unwrap();
        assert!(p["cement_bags_50kg"].as_f64().unwrap() >= 10.0);
    }

    #[test]
    fn finishes_and_steel() {
        let pt = paint(120.0, "m2", 2, 10.0, 20.0, true, 4.0, 500.0).unwrap();
        assert_eq!(pt["paint_litres"], 20.0);
        assert_eq!(pt["cans_to_buy"], 6.0);
        assert!(paint(10.0, "m2", 1, 10.0, 20.0, false, 0.0, 0.0).is_err());
        let t = tiles(5.0, 4.0, "m", 600.0, 600.0, "mm", 3.0, 0.0, 4, 1200.0, "").unwrap();
        assert!((t["tiles_required"].as_f64().unwrap() - 55.0).abs() < 1.0);
        assert_eq!(t["boxes_to_buy"], 16.0);
        let r = roofing(10.0, 8.0, "m", 30.0, "gable", 3.0, 1.0, 10.0, 10.0).unwrap();
        assert!((r["slope_factor"].as_f64().unwrap() - 1.1547).abs() < 0.001);
        assert!(r["sheets_to_buy"].as_f64().unwrap() > 30.0);
        assert!(roofing(10.0, 8.0, "m", 80.0, "", 0.0, 0.0, 0.0, 0.0).is_err());
        let s = steel("round", 12.0, "m", 12.0, 0.0, 0.0, 0.0, 10.0, 60.0).unwrap();
        assert!((s["weight_per_metre_kg"].as_f64().unwrap() - 0.888).abs() < 0.01);
        assert!((s["total_weight_kg"].as_f64().unwrap() - 106.5).abs() < 0.2);
        assert!(
            (steel("pipe", 1.0, "m", 50.0, 0.0, 3.0, 0.0, 1.0, 0.0).unwrap()
                ["weight_per_metre_kg"]
                .as_f64()
                .unwrap()
                - 3.48)
                .abs()
                < 0.05
        );
        assert!(steel("round", 1.0, "m", 0.0, 0.0, 0.0, 0.0, 1.0, 0.0).is_err());
        let rb = rebar(6.0, 4.0, "m", 150.0, 12.0, 2, 25.0, 5.0, 12.0, 0.0).unwrap();
        assert!(rb["weight_kg"].as_f64().unwrap() > 140.0);
        assert!(rb["stock_bars_to_buy"].as_f64().unwrap() > 10.0);
    }

    #[test]
    fn earthworks_walls_stairs_tanks_costs() {
        let e = excavation(10.0, 5.0, 1.5, "m", None, 0.0, 10.0, "clay", 300.0).unwrap();
        assert_eq!(e["bank_volume_m3"], 75.0);
        assert_eq!(e["truck_loads"], 11.0);
        assert!(excavation(1.0, 1.0, 1.0, "m", None, 0.0, 0.0, "lava", 0.0).is_err());
        let w = wall_area(
            &[],
            &[
                Opening { width: 1.0, height: 2.0, count: 1 },
                Opening { width: 1.2, height: 1.2, count: 2 },
            ],
            "m",
            Some(5.0),
            Some(4.0),
            Some(3.0),
            true,
        )
        .unwrap();
        assert_eq!(w["gross_wall_area_m2"], 54.0);
        assert_eq!(w["openings_area_m2"], 4.88);
        assert_eq!(w["ceiling_area_m2"], 20.0);
        assert!(wall_area(&[], &[], "m", None, None, None, false).is_err());
        let st = staircase(3.0, "m", 0.0, 0.0, 1.0, 150.0, false).unwrap();
        assert_eq!(st["risers"], 17.0);
        assert_eq!(st["treads"], 16.0);
        assert_eq!(st["comfortable"], true);
        let tk = tank("cylindrical", 0.0, 0.0, 2.0, 1.0, "m", 0.0, 100.0).unwrap();
        assert!((tk["capacity_litres"].as_f64().unwrap() - 1570.8).abs() < 0.1);
        assert_eq!(
            tank("rectangular", 2.0, 1.0, 1.0, 0.0, "m", 0.0, 50.0).unwrap()["filled_litres"],
            1000.0
        );
        assert!(tank("sphere", 1.0, 1.0, 1.0, 1.0, "m", 0.0, 0.0).is_err());
        let c = cost_estimate(1200.0, "sqft", 2500.0, 2, "", 5.0, None).unwrap();
        assert_eq!(c["base_cost"], 6_000_000.0);
        assert_eq!(c["total_cost"], 6_300_000.0);
        assert_eq!(c["breakdown"].as_array().unwrap().len(), 8);
        assert_eq!(unit_convert(1.0, "brass", "cft").unwrap()["result"], 100.0);
        assert!(unit_convert(1.0, "kg", "m").is_err());
    }
}
