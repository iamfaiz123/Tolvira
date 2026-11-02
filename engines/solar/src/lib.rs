//! # Dev Toolbox Solar & Energy Engine (`solar-engine`)
//!
//! Location data (peak sun hours, tariffs, incentives, grid emission
//! factors) is supplied by the caller; the engine ships conservative defaults
//! and reports every assumption.
//!
//! | Action | Tool(s) |
//! |---|---|
//! | `panel_size` | solar-panel-size-calculator, solar-roof-area-calculator |
//! | `roi` | solar-roi-calculator, solar-savings-calculator |
//! | `battery` | solar-battery-sizing, backup-time-calculator |
//! | `inverter` | inverter-sizing-calculator |
//! | `bill` | electricity-bill-calculator, electricity-unit-cost-calculator (slab tariffs) |
//! | `appliances` | appliance-energy-calculator |
//! | `carbon` | carbon-footprint-estimator |

use engine_shared::{EngineError, EngineResult};
use serde::Deserialize;
use serde_json::{json, Value};

fn r2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
fn positive(v: f64, what: &str) -> EngineResult<f64> {
    engine_shared::error::require_positive(v, what)
}

/// Sizes a PV system from consumption.
pub fn panel_size(
    daily_kwh: Option<f64>,
    monthly_kwh: Option<f64>,
    peak_sun_hours: f64,
    system_efficiency: f64,
    panel_watts: f64,
    panel_area_m2: f64,
    roof_area_m2: Option<f64>,
    coverage_percent: f64,
    cost_per_kw: f64,
) -> EngineResult<Value> {
    let daily = match (daily_kwh, monthly_kwh) {
        (Some(d), _) => d,
        (None, Some(m)) => m / 30.4375,
        _ => return Err(EngineError::invalid("Provide daily_kwh or monthly_kwh")),
    };
    positive(daily, "consumption")?;
    let psh = if peak_sun_hours <= 0.0 { 4.5 } else { peak_sun_hours };
    let eff = if system_efficiency <= 0.0 {
        0.78
    } else if system_efficiency > 1.0 {
        system_efficiency / 100.0
    } else {
        system_efficiency
    };
    let pw = if panel_watts <= 0.0 { 550.0 } else { panel_watts };
    let pa = if panel_area_m2 <= 0.0 { 2.6 } else { panel_area_m2 };
    let cov = if coverage_percent <= 0.0 {
        100.0
    } else {
        coverage_percent.min(100.0)
    };
    let target_daily = daily * cov / 100.0;
    let kw_needed = target_daily / (psh * eff);
    let panels = (kw_needed * 1000.0 / pw).ceil();
    let installed_kw = panels * pw / 1000.0;
    let area = panels * pa * 1.15; // spacing / walkways
    let yearly_generation = installed_kw * psh * eff * 365.0;
    let roof_fit = roof_area_m2.map(|r| {
        json!({
            "roof_area_m2": r,
            "fits": r >= area,
            "max_panels": (r / (pa * 1.15)).floor(),
            "max_kw": r2((r / (pa * 1.15)).floor() * pw / 1000.0)
        })
    });
    Ok(json!({
        "daily_consumption_kwh": r2(daily),
        "target_daily_kwh": r2(target_daily),
        "system_size_kw": r2(kw_needed),
        "recommended_kw": r2(installed_kw),
        "panels": panels,
        "panel_watts": pw,
        "roof_area_needed_m2": r2(area),
        "roof_area_needed_sqft": r2(area * 10.7639),
        "estimated_daily_generation_kwh": r2(installed_kw * psh * eff),
        "estimated_yearly_generation_kwh": r2(yearly_generation),
        "estimated_cost": if cost_per_kw > 0.0 { json!(r2(installed_kw * cost_per_kw)) } else { Value::Null },
        "roof": roof_fit,
        "assumptions": {
            "peak_sun_hours": psh,
            "system_efficiency": eff,
            "panel_area_m2": pa,
            "spacing_factor": 1.15,
            "coverage_percent": cov
        },
    }))
}

/// Payback, IRR-style savings and cumulative cash flow for a PV system.
pub fn roi(
    system_kw: f64,
    cost: f64,
    tariff_per_kwh: f64,
    peak_sun_hours: f64,
    system_efficiency: f64,
    incentives: f64,
    degradation_percent: f64,
    tariff_escalation_percent: f64,
    years: u32,
    maintenance_yearly: f64,
    export_share_percent: f64,
    export_tariff: f64,
) -> EngineResult<Value> {
    positive(system_kw, "system_kw")?;
    positive(cost, "cost")?;
    positive(tariff_per_kwh, "tariff_per_kwh")?;
    let psh = if peak_sun_hours <= 0.0 { 4.5 } else { peak_sun_hours };
    let eff = if system_efficiency <= 0.0 {
        0.78
    } else if system_efficiency > 1.0 {
        system_efficiency / 100.0
    } else {
        system_efficiency
    };
    let years = years.clamp(1, 40);
    let degr = if degradation_percent <= 0.0 {
        0.5
    } else {
        degradation_percent
    } / 100.0;
    let esc = tariff_escalation_percent.max(0.0) / 100.0;
    let net_cost = (cost - incentives.max(0.0)).max(0.0);
    let mut cumulative = -net_cost;
    let mut payback: Option<f64> = None;
    let mut rows = Vec::new();
    let mut total_gen = 0.0;
    let mut total_savings = 0.0;
    let export_share = export_share_percent.clamp(0.0, 100.0) / 100.0;
    for y in 1..=years {
        let gen = system_kw * psh * eff * 365.0 * (1.0 - degr).powi(y as i32 - 1);
        let tariff = tariff_per_kwh * (1.0 + esc).powi(y as i32 - 1);
        let self_use = gen * (1.0 - export_share) * tariff;
        let exported = gen
            * export_share
            * if export_tariff > 0.0 {
                export_tariff * (1.0 + esc).powi(y as i32 - 1)
            } else {
                tariff
            };
        let savings = self_use + exported - maintenance_yearly.max(0.0);
        let prev = cumulative;
        cumulative += savings;
        if payback.is_none() && cumulative >= 0.0 && savings > 0.0 {
            payback = Some((y as f64 - 1.0) + (-prev / savings));
        }
        total_gen += gen;
        total_savings += savings;
        rows.push(json!({
            "year": y,
            "generation_kwh": r2(gen),
            "tariff": r2(tariff * 100.0) / 100.0,
            "savings": r2(savings),
            "cumulative": r2(cumulative)
        }));
    }
    let lcoe = net_cost / total_gen;
    let roi_pct = (total_savings - net_cost) / net_cost * 100.0;
    Ok(json!({
        "net_cost": r2(net_cost),
        "payback_years": payback.map(r2),
        "total_savings": r2(total_savings),
        "net_profit": r2(total_savings - net_cost),
        "roi_percent": r2(roi_pct),
        "annualized_roi_percent": r2(roi_pct / years as f64),
        "lifetime_generation_kwh": r2(total_gen),
        "lcoe_per_kwh": r2(lcoe * 100.0) / 100.0,
        "first_year_savings": rows.first().map(|r| r["savings"].clone()),
        "yearly": rows,
        "assumptions": {
            "peak_sun_hours": psh,
            "system_efficiency": eff,
            "degradation_percent_per_year": degr * 100.0,
            "tariff_escalation_percent": esc * 100.0,
            "years": years,
            "export_share_percent": export_share * 100.0
        },
    }))
}

/// Battery bank sizing from daily load, backup hours/autonomy and depth of discharge.
pub fn battery(
    daily_kwh: f64,
    backup_hours: f64,
    autonomy_days: f64,
    depth_of_discharge: f64,
    system_voltage: f64,
    inverter_efficiency: f64,
    battery_ah: f64,
    battery_voltage: f64,
    load_watts: f64,
    capacity_kwh: Option<f64>,
) -> EngineResult<Value> {
    let dod = if depth_of_discharge <= 0.0 {
        0.8
    } else if depth_of_discharge > 1.0 {
        depth_of_discharge / 100.0
    } else {
        depth_of_discharge
    };
    let inv_eff = if inverter_efficiency <= 0.0 {
        0.9
    } else if inverter_efficiency > 1.0 {
        inverter_efficiency / 100.0
    } else {
        inverter_efficiency
    };
    let v = if system_voltage <= 0.0 { 48.0 } else { system_voltage };
    // Backup-time mode.
    if let Some(cap) = capacity_kwh {
        positive(cap, "capacity_kwh")?;
        positive(load_watts, "load_watts")?;
        let usable = cap * dod * inv_eff;
        let hours = usable / (load_watts / 1000.0);
        return Ok(json!({
            "mode": "backup_time",
            "capacity_kwh": cap,
            "usable_kwh": r2(usable),
            "load_watts": load_watts,
            "backup_hours": r2(hours),
            "backup_human": format!("{}h {}m", hours.floor(), ((hours - hours.floor()) * 60.0).round()),
            "assumptions": { "depth_of_discharge": dod, "inverter_efficiency": inv_eff }
        }));
    }
    let energy = if backup_hours > 0.0 && load_watts > 0.0 {
        load_watts / 1000.0 * backup_hours
    } else {
        positive(daily_kwh, "daily_kwh")?;
        daily_kwh * if autonomy_days <= 0.0 { 1.0 } else { autonomy_days }
    };
    let required_kwh = energy / (dod * inv_eff);
    let ah_total = required_kwh * 1000.0 / v;
    let units = if battery_ah > 0.0 && battery_voltage > 0.0 {
        let series = (v / battery_voltage).ceil().max(1.0);
        let parallel = (ah_total / battery_ah).ceil().max(1.0);
        Some(json!({
            "battery_ah": battery_ah,
            "battery_voltage": battery_voltage,
            "in_series": series,
            "in_parallel": parallel,
            "total_batteries": series * parallel,
            "bank_kwh": r2(series * parallel * battery_ah * battery_voltage / 1000.0)
        }))
    } else {
        None
    };
    Ok(json!({
        "mode": "sizing",
        "energy_to_store_kwh": r2(energy),
        "required_capacity_kwh": r2(required_kwh),
        "required_ah_at_system_voltage": r2(ah_total),
        "system_voltage": v,
        "batteries": units,
        "assumptions": {
            "depth_of_discharge": dod,
            "inverter_efficiency": inv_eff,
            "autonomy_days": if autonomy_days <= 0.0 { 1.0 } else { autonomy_days }
        }
    }))
}

/// One appliance for load calculations.
#[derive(Deserialize, Debug, Clone)]
pub struct Appliance {
    /// Label.
    pub name: String,
    /// Rated power in watts.
    pub watts: f64,
    /// Number of units.
    #[serde(default = "one")]
    pub quantity: f64,
    /// Daily usage.
    #[serde(default)]
    pub hours_per_day: f64,
    /// Start-up surge multiplier (motors, compressors).
    #[serde(default)]
    pub surge_factor: f64,
}
fn one() -> f64 {
    1.0
}

/// Inverter sizing from appliance loads, surge and power factor.
pub fn inverter(
    appliances: &[Appliance],
    safety_margin_percent: f64,
    power_factor: f64,
    system_voltage: f64,
) -> EngineResult<Value> {
    if appliances.is_empty() {
        return Err(EngineError::empty("appliances"));
    }
    let margin = 1.0
        + if safety_margin_percent <= 0.0 {
            25.0
        } else {
            safety_margin_percent
        } / 100.0;
    let pf = if power_factor <= 0.0 {
        0.8
    } else {
        power_factor.min(1.0)
    };
    let mut running = 0.0;
    let mut surge = 0.0;
    let rows: Vec<Value> = appliances
        .iter()
        .map(|a| {
            let w = a.watts * a.quantity.max(1.0);
            let sf = if a.surge_factor <= 0.0 { 1.0 } else { a.surge_factor };
            running += w;
            surge = f64::max(surge, running - w + w * sf);
            json!({ "name": a.name, "running_watts": r2(w), "surge_watts": r2(w * sf) })
        })
        .collect();
    let max_surge: f64 = appliances
        .iter()
        .map(|a| {
            a.watts * a.quantity.max(1.0) * if a.surge_factor <= 0.0 { 1.0 } else { a.surge_factor }
        })
        .fold(0.0, f64::max)
        + running
        - appliances
            .iter()
            .map(|a| a.watts * a.quantity.max(1.0))
            .fold(0.0, f64::max);
    let size_w = running * margin;
    let va = size_w / pf;
    let standard = [
        500.0, 800.0, 1000.0, 1500.0, 2000.0, 3000.0, 3500.0, 5000.0, 7500.0, 10000.0, 15000.0,
        20000.0,
    ];
    let recommended = standard.iter().copied().find(|s| *s >= va).unwrap_or(va);
    let v = if system_voltage <= 0.0 { 48.0 } else { system_voltage };
    Ok(json!({
        "running_watts": r2(running),
        "peak_surge_watts": r2(max_surge),
        "size_watts": r2(size_w),
        "size_va": r2(va),
        "recommended_va": recommended,
        "recommended_kva": recommended / 1000.0,
        "dc_current_amps": r2(size_w / v),
        "appliances": rows,
        "assumptions": {
            "safety_margin_percent": (margin - 1.0) * 100.0,
            "power_factor": pf,
            "system_voltage": v
        }
    }))
}

/// One slab of a tiered electricity tariff.
#[derive(Deserialize, Debug, Clone)]
pub struct Slab {
    /// Upper bound of the slab in kWh (`null` for the last slab).
    pub upto: Option<f64>,
    /// Price per kWh within the slab.
    pub rate: f64,
}

/// Electricity bill from slab or flat tariffs with fixed charges, surcharge and tax.
pub fn bill(
    units_kwh: f64,
    slabs: &[Slab],
    flat_rate: Option<f64>,
    fixed_charge: f64,
    tax_percent: f64,
    fuel_surcharge_percent: f64,
    days: u32,
    solar_offset_kwh: f64,
) -> EngineResult<Value> {
    if !units_kwh.is_finite() || units_kwh < 0.0 {
        return Err(EngineError::invalid("units_kwh must be non-negative"));
    }
    let net_units = (units_kwh - solar_offset_kwh.max(0.0)).max(0.0);
    let mut energy = 0.0;
    let mut rows = Vec::new();
    if let Some(rate) = flat_rate {
        energy = net_units * rate;
        rows.push(json!({
            "from": 0,
            "to": net_units,
            "units": net_units,
            "rate": rate,
            "amount": r2(energy)
        }));
    } else {
        if slabs.is_empty() {
            return Err(EngineError::invalid(
                "Provide `slabs` [{upto, rate}] (telescopic tariff) or `flat_rate`",
            ));
        }
        let mut prev = 0.0;
        for s in slabs {
            let upper = s.upto.unwrap_or(f64::INFINITY);
            if net_units <= prev {
                break;
            }
            let u = net_units.min(upper) - prev;
            let amt = u * s.rate;
            energy += amt;
            rows.push(json!({
                "from": prev,
                "to": s.upto,
                "units": r2(u),
                "rate": s.rate,
                "amount": r2(amt)
            }));
            prev = upper;
        }
    }
    let surcharge = energy * fuel_surcharge_percent.max(0.0) / 100.0;
    let subtotal = energy + surcharge + fixed_charge.max(0.0);
    let tax = subtotal * tax_percent.max(0.0) / 100.0;
    let total = subtotal + tax;
    let d = if days == 0 { 30 } else { days };
    Ok(json!({
        "units_billed": r2(net_units),
        "solar_offset_kwh": solar_offset_kwh.max(0.0),
        "energy_charge": r2(energy),
        "fuel_surcharge": r2(surcharge),
        "fixed_charge": r2(fixed_charge.max(0.0)),
        "tax": r2(tax),
        "total": r2(total),
        "average_cost_per_unit": if net_units > 0.0 { r2(total / net_units * 100.0) / 100.0 } else { 0.0 },
        "marginal_rate": rows.last().and_then(|r| r["rate"].as_f64()),
        "daily_average_units": r2(net_units / d as f64),
        "yearly_projection": r2(total * 365.0 / d as f64),
        "slabs": rows
    }))
}

/// Daily/monthly consumption and cost per appliance.
pub fn appliances(items: &[Appliance], tariff_per_kwh: f64, days: u32) -> EngineResult<Value> {
    if items.is_empty() {
        return Err(EngineError::empty("appliances"));
    }
    let d = if days == 0 { 30 } else { days } as f64;
    let mut total_daily = 0.0;
    let mut rows: Vec<Value> = items
        .iter()
        .map(|a| {
            let daily = a.watts * a.quantity.max(1.0) * a.hours_per_day.max(0.0) / 1000.0;
            total_daily += daily;
            json!({
                "name": a.name,
                "watts": a.watts,
                "quantity": a.quantity.max(1.0),
                "hours_per_day": a.hours_per_day,
                "daily_kwh": r2(daily),
                "monthly_kwh": r2(daily * d),
                "yearly_kwh": r2(daily * 365.0),
                "monthly_cost": r2(daily * d * tariff_per_kwh.max(0.0))
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        b["daily_kwh"]
            .as_f64()
            .unwrap_or(0.0)
            .total_cmp(&a["daily_kwh"].as_f64().unwrap_or(0.0))
    });
    for r in rows.iter_mut() {
        let share = if total_daily > 0.0 {
            r["daily_kwh"].as_f64().unwrap_or(0.0) / total_daily * 100.0
        } else {
            0.0
        };
        r["share_percent"] = json!(r2(share));
    }
    Ok(json!({
        "appliances": rows,
        "daily_kwh": r2(total_daily),
        "monthly_kwh": r2(total_daily * d),
        "yearly_kwh": r2(total_daily * 365.0),
        "monthly_cost": r2(total_daily * d * tariff_per_kwh.max(0.0)),
        "yearly_cost": r2(total_daily * 365.0 * tariff_per_kwh.max(0.0)),
        "tariff_per_kwh": tariff_per_kwh,
        "biggest_consumer": rows.first().and_then(|r| r["name"].as_str())
    }))
}

/// Household activity data for the carbon-footprint estimate.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct CarbonInputs {
    /// Grid electricity per month.
    pub electricity_kwh_month: f64,
    /// Grid emission factor (kg CO₂e per kWh).
    pub grid_factor_kg_per_kwh: f64,
    /// LPG consumed per month.
    pub lpg_kg_month: f64,
    /// Piped natural gas per month.
    pub natural_gas_m3_month: f64,
    /// Petrol per month.
    pub petrol_litres_month: f64,
    /// Diesel per month.
    pub diesel_litres_month: f64,
    /// Car distance per month (used when fuel litres are unknown).
    pub car_km_month: f64,
    /// Car emission factor per km.
    pub car_kg_per_km: f64,
    /// Short-haul flights per year.
    pub flights_short_haul_year: f64,
    /// Long-haul flights per year.
    pub flights_long_haul_year: f64,
    /// Public-transport distance per month.
    pub public_transport_km_month: f64,
    /// `vegan`, `vegetarian`, `mixed` (default) or `meat-heavy`.
    pub diet: String,
    /// People in the household.
    pub people: f64,
    /// Rooftop solar generation per month (offsets grid electricity).
    pub solar_kwh_month: f64,
}

/// Annual CO₂e footprint by category with comparisons and offsets.
pub fn carbon(c: &CarbonInputs) -> EngineResult<Value> {
    let grid = if c.grid_factor_kg_per_kwh <= 0.0 {
        0.71
    } else {
        c.grid_factor_kg_per_kwh
    };
    let electricity = (c.electricity_kwh_month - c.solar_kwh_month.max(0.0)).max(0.0) * grid * 12.0;
    let lpg = c.lpg_kg_month.max(0.0) * 2.98 * 12.0;
    let gas = c.natural_gas_m3_month.max(0.0) * 2.0 * 12.0;
    let petrol = c.petrol_litres_month.max(0.0) * 2.31 * 12.0;
    let diesel = c.diesel_litres_month.max(0.0) * 2.68 * 12.0;
    let car = c.car_km_month.max(0.0)
        * if c.car_kg_per_km <= 0.0 { 0.17 } else { c.car_kg_per_km }
        * 12.0;
    let flights =
        c.flights_short_haul_year.max(0.0) * 250.0 + c.flights_long_haul_year.max(0.0) * 1800.0;
    let transit = c.public_transport_km_month.max(0.0) * 0.05 * 12.0;
    let diet = match c.diet.to_lowercase().as_str() {
        "vegan" => 1000.0,
        "vegetarian" | "veg" => 1300.0,
        "" | "mixed" | "average" => 2000.0,
        "meat-heavy" | "non-veg" | "high-meat" => 2800.0,
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown diet `{other}` (vegan, vegetarian, mixed, meat-heavy)"
            )))
        },
    };
    let people = if c.people <= 0.0 { 1.0 } else { c.people };
    let household =
        electricity + lpg + gas + petrol + diesel + car + flights + transit + diet * people;
    let per_person = household / people;
    let trees = household / 22.0;
    Ok(json!({
        "yearly_kg_co2e": r2(household),
        "yearly_tonnes": r2(household / 1000.0),
        "per_person_tonnes": r2(per_person / 1000.0),
        "breakdown_kg": {
            "electricity": r2(electricity),
            "lpg": r2(lpg),
            "natural_gas": r2(gas),
            "petrol": r2(petrol),
            "diesel": r2(diesel),
            "car_km": r2(car),
            "flights": r2(flights),
            "public_transport": r2(transit),
            "diet": r2(diet * people)
        },
        "trees_to_offset": trees.ceil(),
        "comparison": {
            "world_average_tonnes_per_person": 4.7,
            "india_average": 1.9,
            "usa_average": 14.7,
            "paris_2030_target": 2.3,
            "vs_world_percent": r2((per_person / 1000.0 / 4.7 - 1.0) * 100.0)
        },
        "assumptions": {
            "grid_kg_per_kwh": grid,
            "lpg_kg_per_kg": 2.98,
            "petrol_kg_per_l": 2.31,
            "diesel_kg_per_l": 2.68,
            "car_kg_per_km": if c.car_kg_per_km <= 0.0 { 0.17 } else { c.car_kg_per_km },
            "short_haul_flight_kg": 250,
            "long_haul_flight_kg": 1800,
            "tree_kg_per_year": 22,
            "note": "Emission factors are global averages; pass grid_factor_kg_per_kwh for your \
                     grid"
        },
    }))
}

fn default_30() -> u32 {
    30
}
fn default_25y() -> u32 {
    25
}

/// Every action understood by the solar engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum SolarAction {
    /// PV array sizing from consumption.
    PanelSize {
        /// Daily consumption in kWh.
        #[serde(default)]
        daily_kwh: Option<f64>,
        /// Monthly consumption in kWh (alternative).
        #[serde(default)]
        monthly_kwh: Option<f64>,
        /// Peak sun hours per day for the location.
        #[serde(default)]
        peak_sun_hours: f64,
        /// Overall system efficiency (0–1).
        #[serde(default)]
        system_efficiency: f64,
        /// Panel rating in watts.
        #[serde(default)]
        panel_watts: f64,
        /// Area of one panel.
        #[serde(default)]
        panel_area_m2: f64,
        /// Available roof area.
        #[serde(default)]
        roof_area_m2: Option<f64>,
        /// Share of consumption to cover.
        #[serde(default)]
        coverage_percent: f64,
        /// Installed cost per kW.
        #[serde(default)]
        cost_per_kw: f64,
    },
    /// Return on investment for a PV system.
    Roi {
        /// System size in kW.
        system_kw: f64,
        /// Installed cost.
        cost: f64,
        /// Grid tariff per kWh.
        tariff_per_kwh: f64,
        /// Peak sun hours per day.
        #[serde(default)]
        peak_sun_hours: f64,
        /// Overall system efficiency (0–1).
        #[serde(default)]
        system_efficiency: f64,
        /// Subsidies/incentives deducted from cost.
        #[serde(default)]
        incentives: f64,
        /// Annual panel degradation.
        #[serde(default)]
        degradation_percent: f64,
        /// Annual tariff increase.
        #[serde(default)]
        tariff_escalation_percent: f64,
        /// Analysis horizon in years.
        #[serde(default = "default_25y")]
        years: u32,
        /// Yearly maintenance cost.
        #[serde(default)]
        maintenance_yearly: f64,
        /// Share of generation exported to the grid.
        #[serde(default)]
        export_share_percent: f64,
        /// Feed-in tariff per kWh.
        #[serde(default)]
        export_tariff: f64,
    },
    /// Battery sizing.
    Battery {
        /// Daily consumption in kWh.
        #[serde(default)]
        daily_kwh: f64,
        /// Hours of backup required.
        #[serde(default)]
        backup_hours: f64,
        /// Days of autonomy.
        #[serde(default)]
        autonomy_days: f64,
        /// Usable depth of discharge (0–1).
        #[serde(default)]
        depth_of_discharge: f64,
        /// DC bus voltage.
        #[serde(default)]
        system_voltage: f64,
        /// Inverter efficiency (0–1).
        #[serde(default)]
        inverter_efficiency: f64,
        /// Capacity of one battery in Ah.
        #[serde(default)]
        battery_ah: f64,
        /// Voltage of one battery.
        #[serde(default)]
        battery_voltage: f64,
        /// Backup load in watts (alternative to `daily_kwh`).
        #[serde(default)]
        load_watts: f64,
        /// Explicit capacity to evaluate.
        #[serde(default)]
        capacity_kwh: Option<f64>,
    },
    /// Inverter sizing.
    Inverter {
        /// Appliance list.
        appliances: Vec<Appliance>,
        /// Safety margin over the peak load.
        #[serde(default)]
        safety_margin_percent: f64,
        /// Load power factor.
        #[serde(default)]
        power_factor: f64,
        /// DC bus voltage (for current estimates).
        #[serde(default)]
        system_voltage: f64,
    },
    /// Electricity bill calculator.
    Bill {
        /// Units consumed.
        units_kwh: f64,
        /// Tariff slabs.
        #[serde(default)]
        slabs: Vec<Slab>,
        /// Flat tariff per kWh (alternative to `slabs`).
        #[serde(default)]
        flat_rate: Option<f64>,
        /// Fixed monthly charge.
        #[serde(default)]
        fixed_charge: f64,
        /// Tax on the bill.
        #[serde(default)]
        tax_percent: f64,
        /// Fuel surcharge.
        #[serde(default)]
        fuel_surcharge_percent: f64,
        /// Billing days.
        #[serde(default = "default_30")]
        days: u32,
        /// Solar units credited against consumption.
        #[serde(default)]
        solar_offset_kwh: f64,
    },
    /// Appliance consumption calculator.
    Appliances {
        /// Appliance list.
        appliances: Vec<Appliance>,
        /// Tariff per kWh.
        #[serde(default)]
        tariff_per_kwh: f64,
        /// Days in the period.
        #[serde(default = "default_30")]
        days: u32,
    },
    /// Carbon footprint estimate.
    Carbon {
        /// Activity data.
        #[serde(flatten)]
        inputs: CarbonInputs,
    },
}

/// Dispatches a [`SolarAction`] to its implementation.
pub fn handle_action(action: SolarAction) -> EngineResult<Value> {
    match action {
        SolarAction::PanelSize {
            daily_kwh,
            monthly_kwh,
            peak_sun_hours,
            system_efficiency,
            panel_watts,
            panel_area_m2,
            roof_area_m2,
            coverage_percent,
            cost_per_kw,
        } => panel_size(
            daily_kwh,
            monthly_kwh,
            peak_sun_hours,
            system_efficiency,
            panel_watts,
            panel_area_m2,
            roof_area_m2,
            coverage_percent,
            cost_per_kw,
        ),
        SolarAction::Roi {
            system_kw,
            cost,
            tariff_per_kwh,
            peak_sun_hours,
            system_efficiency,
            incentives,
            degradation_percent,
            tariff_escalation_percent,
            years,
            maintenance_yearly,
            export_share_percent,
            export_tariff,
        } => roi(
            system_kw,
            cost,
            tariff_per_kwh,
            peak_sun_hours,
            system_efficiency,
            incentives,
            degradation_percent,
            tariff_escalation_percent,
            years,
            maintenance_yearly,
            export_share_percent,
            export_tariff,
        ),
        SolarAction::Battery {
            daily_kwh,
            backup_hours,
            autonomy_days,
            depth_of_discharge,
            system_voltage,
            inverter_efficiency,
            battery_ah,
            battery_voltage,
            load_watts,
            capacity_kwh,
        } => battery(
            daily_kwh,
            backup_hours,
            autonomy_days,
            depth_of_discharge,
            system_voltage,
            inverter_efficiency,
            battery_ah,
            battery_voltage,
            load_watts,
            capacity_kwh,
        ),
        SolarAction::Inverter {
            appliances,
            safety_margin_percent,
            power_factor,
            system_voltage,
        } => inverter(&appliances, safety_margin_percent, power_factor, system_voltage),
        SolarAction::Bill {
            units_kwh,
            slabs,
            flat_rate,
            fixed_charge,
            tax_percent,
            fuel_surcharge_percent,
            days,
            solar_offset_kwh,
        } => bill(
            units_kwh,
            &slabs,
            flat_rate,
            fixed_charge,
            tax_percent,
            fuel_surcharge_percent,
            days,
            solar_offset_kwh,
        ),
        SolarAction::Appliances { appliances: items, tariff_per_kwh, days } => {
            appliances(&items, tariff_per_kwh, days)
        },
        SolarAction::Carbon { inputs } => carbon(&inputs),
    }
}

engine_shared::export_engine!(handle_action, SolarAction);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizing_and_roi() {
        let p = panel_size(None, Some(300.0), 5.0, 0.8, 500.0, 2.5, Some(30.0), 100.0, 60000.0)
            .unwrap();
        assert_eq!(p["panels"], 5.0);
        assert_eq!(p["recommended_kw"], 2.5);
        assert_eq!(p["roof"]["fits"], true);
        assert!(panel_size(None, None, 5.0, 0.8, 500.0, 2.5, None, 100.0, 0.0).is_err());
        let r = roi(3.0, 150_000.0, 8.0, 5.0, 0.8, 0.0, 0.5, 3.0, 25, 1000.0, 0.0, 0.0).unwrap();
        assert!(r["payback_years"].as_f64().unwrap() < 6.0);
        assert!(r["roi_percent"].as_f64().unwrap() > 200.0);
        assert_eq!(r["yearly"].as_array().unwrap().len(), 25);
        assert!(roi(0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 10, 0.0, 0.0, 0.0).is_err());
    }

    #[test]
    fn storage_and_loads() {
        let b = battery(10.0, 0.0, 1.0, 0.8, 48.0, 0.9, 200.0, 12.0, 0.0, None).unwrap();
        assert_eq!(b["required_capacity_kwh"], 13.89);
        assert_eq!(b["batteries"]["in_series"], 4.0);
        assert_eq!(b["batteries"]["in_parallel"], 2.0);
        let bt = battery(0.0, 0.0, 0.0, 0.8, 0.0, 0.9, 0.0, 0.0, 500.0, Some(5.0)).unwrap();
        assert_eq!(bt["backup_hours"], 7.2);
        assert!(battery(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, None).is_err());
        let apps = vec![
            Appliance {
                name: "fridge".into(),
                watts: 150.0,
                quantity: 1.0,
                hours_per_day: 24.0,
                surge_factor: 3.0,
            },
            Appliance {
                name: "tv".into(),
                watts: 100.0,
                quantity: 2.0,
                hours_per_day: 5.0,
                surge_factor: 0.0,
            },
        ];
        let inv = inverter(&apps, 25.0, 0.8, 48.0).unwrap();
        assert_eq!(inv["running_watts"], 350.0);
        assert_eq!(inv["recommended_va"], 800.0);
        assert!(inverter(&[], 0.0, 0.0, 0.0).is_err());
        let a = appliances(&apps, 8.0, 30).unwrap();
        assert_eq!(a["daily_kwh"], 4.6);
        assert_eq!(a["biggest_consumer"], "fridge");
        let slabs = vec![
            Slab { upto: Some(100.0), rate: 3.0 },
            Slab { upto: Some(200.0), rate: 5.0 },
            Slab { upto: None, rate: 8.0 },
        ];
        let bl = bill(250.0, &slabs, None, 50.0, 10.0, 0.0, 30, 0.0).unwrap();
        assert_eq!(bl["energy_charge"], 1200.0);
        assert_eq!(bl["total"], 1375.0);
        assert_eq!(bl["marginal_rate"], 8.0);
        assert!(bill(10.0, &[], None, 0.0, 0.0, 0.0, 30, 0.0).is_err());
        let c = carbon(&CarbonInputs {
            electricity_kwh_month: 300.0,
            car_km_month: 500.0,
            diet: "vegetarian".into(),
            people: 2.0,
            ..Default::default()
        })
        .unwrap();
        assert!(c["yearly_tonnes"].as_f64().unwrap() > 5.0);
        assert!(carbon(&CarbonInputs {
            diet: "rocks".into(),
            ..Default::default()
        })
        .is_err());
    }
}
