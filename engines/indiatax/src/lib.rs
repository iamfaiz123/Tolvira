//! # Dev Toolbox India GST & Tax Engine (`indiatax-engine`)
//!
//! Every rate/slab comes from `rules.rs` presets labelled with a source and
//! financial year, and every action accepts overrides so the UI can ship
//! updated data without a rebuild. Amounts are rounded to paise (2 dp).
//!
//! | Action | Tool(s) |
//! |---|---|
//! | `gst` | gst-calculator, gst-inclusive-exclusive-calculator, gst-rate-calculator |
//! | `itc` | input-tax-credit-calculator |
//! | `validate_gstin` | gstin-format-validator |
//! | `validate_invoice_number` / `validate_invoice` | invoice-number-validator, gst-invoice-validator |
//! | `hsn_lookup` | hsn-sac-lookup (chapter level dataset) |
//! | `tds` | tds-calculator |
//! | `income_tax` | income-tax-estimator, income-tax-calculator |
//! | `salary_in_hand` | salary-in-hand-calculator, take-home-salary-calculator |
//! | `professional_tax` | professional-tax-calculator |
//! | `capital_gains` | capital-gains-estimator |
//! | `bank_statement` | bank-statement-cleaner, bank-statement-to-csv, bank-statement-to-tally |
//! | `validate_gstr` | gstr-data-validator |
//! | `reconcile` | purchase-register-reconciliation, sales-register-reconciliation |
//! | `rules` | returns the bundled rule presets (version, sources) |

pub mod rules;

use engine_shared::{EngineError, EngineResult};
use serde::Deserialize;
use serde_json::{json, Map, Value};

fn rupee(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn positive(v: f64, what: &str) -> EngineResult<f64> {
    engine_shared::error::require_positive(v, what)
}

// ------------------------------------------------------------------- GST --

/// GST calculator: CGST/SGST or IGST split, inclusive/exclusive, cess, quantity.
pub fn gst(
    amount: f64,
    rate: f64,
    inclusive: bool,
    interstate: bool,
    cess_percent: f64,
    quantity: f64,
) -> EngineResult<Value> {
    positive(amount, "amount")?;
    if !rate.is_finite() || !(0.0..=100.0).contains(&rate) {
        return Err(EngineError::invalid("GST rate must be between 0 and 100"));
    }
    let q = if quantity <= 0.0 { 1.0 } else { quantity };
    let total_rate = rate + cess_percent.max(0.0);
    let (base, tax) = if inclusive {
        let base = amount / (1.0 + total_rate / 100.0);
        (base, amount - base)
    } else {
        (amount, amount * total_rate / 100.0)
    };
    let gst_tax = base * rate / 100.0;
    let cess = base * cess_percent.max(0.0) / 100.0;
    let split = if interstate {
        json!({ "igst": rupee(gst_tax), "igst_rate": rate })
    } else {
        json!({
            "cgst": rupee(gst_tax / 2.0),
            "sgst": rupee(gst_tax / 2.0),
            "cgst_rate": rate / 2.0,
            "sgst_rate": rate / 2.0
        })
    };
    let slabs: Vec<Value> = rules::GST_SLABS
        .iter()
        .map(|s| {
            json!({
                "rate": s,
                "tax": rupee(base * s / 100.0),
                "total": rupee(base * (1.0 + s / 100.0))
            })
        })
        .collect();
    Ok(json!({
        "taxable_value": rupee(base),
        "gst": rupee(gst_tax),
        "cess": rupee(cess),
        "total_tax": rupee(tax),
        "total": rupee(base + tax),
        "rate": rate,
        "mode": if inclusive { "inclusive" } else { "exclusive" },
        "supply": if interstate { "inter-state (IGST)" } else { "intra-state (CGST + SGST)" },
        "split": split,
        "quantity": q,
        "line_total": rupee((base + tax) * q),
        "line_tax": rupee(tax * q),
        "all_slabs": slabs,
        "is_standard_slab": rules::GST_SLABS.contains(&rate),
    }))
}

/// Input tax credit set-off across IGST/CGST/SGST heads following the statutory order.
pub fn itc(output_tax: &Map<String, Value>, input_tax: &Map<String, Value>) -> EngineResult<Value> {
    let get =
        |m: &Map<String, Value>, k: &str| m.get(k).and_then(Value::as_f64).unwrap_or(0.0).max(0.0);
    let (out_igst, out_cgst, out_sgst) =
        (get(output_tax, "igst"), get(output_tax, "cgst"), get(output_tax, "sgst"));
    let (mut in_igst, mut in_cgst, mut in_sgst) =
        (get(input_tax, "igst"), get(input_tax, "cgst"), get(input_tax, "sgst"));
    if out_igst + out_cgst + out_sgst + in_igst + in_cgst + in_sgst == 0.0 {
        return Err(EngineError::empty("tax amounts"));
    }
    let mut steps = Vec::new();
    // IGST credit: IGST → CGST → SGST.
    let mut pay_igst = out_igst;
    let use_ = |credit: &mut f64, liability: &mut f64, label: &str, steps: &mut Vec<Value>| {
        let u = credit.min(*liability);
        if u > 0.0 {
            *credit -= u;
            *liability -= u;
            steps.push(json!({ "step": label, "amount": rupee(u) }));
        }
    };
    use_(&mut in_igst, &mut pay_igst, "IGST credit against IGST", &mut steps);
    let mut pay_cgst = out_cgst;
    let mut pay_sgst = out_sgst;
    use_(&mut in_igst, &mut pay_cgst, "IGST credit against CGST", &mut steps);
    use_(&mut in_igst, &mut pay_sgst, "IGST credit against SGST", &mut steps);
    use_(&mut in_cgst, &mut pay_cgst, "CGST credit against CGST", &mut steps);
    use_(&mut in_cgst, &mut pay_igst, "CGST credit against IGST", &mut steps);
    use_(&mut in_sgst, &mut pay_sgst, "SGST credit against SGST", &mut steps);
    use_(&mut in_sgst, &mut pay_igst, "SGST credit against IGST", &mut steps);
    Ok(json!({
        "payable": {
            "igst": rupee(pay_igst),
            "cgst": rupee(pay_cgst),
            "sgst": rupee(pay_sgst),
            "total": rupee(pay_igst + pay_cgst + pay_sgst)
        },
        "credit_carried_forward": {
            "igst": rupee(in_igst),
            "cgst": rupee(in_cgst),
            "sgst": rupee(in_sgst),
            "total": rupee(in_igst + in_cgst + in_sgst)
        },
        "utilisation_steps": steps,
        "rule": "Section 49/49A: IGST credit first (IGST→CGST→SGST), CGST credit cannot offset \
                 SGST and vice versa",
    }))
}

/// Validates a GSTIN: 15 chars, state code, PAN pattern, entity code, 'Z', checksum.
pub fn validate_gstin(gstin: &str) -> EngineResult<Value> {
    let g = gstin.trim().to_uppercase();
    if g.is_empty() {
        return Err(EngineError::empty("gstin"));
    }
    let mut errors = Vec::new();
    let chars: Vec<char> = g.chars().collect();
    if chars.len() != 15 {
        errors.push(format!("GSTIN must be 15 characters (got {})", chars.len()));
    }
    let state = if chars.len() >= 2 {
        g[..2].to_string()
    } else {
        String::new()
    };
    let state_name = rules::STATE_CODES.iter().find(|(c, _)| *c == state).map(|(_, n)| *n);
    if chars.len() >= 2 {
        if !chars[0].is_ascii_digit() || !chars[1].is_ascii_digit() {
            errors.push("Characters 1-2 must be a numeric state code".into());
        } else if state_name.is_none() {
            errors.push(format!("Unknown state code {state}"));
        }
    }
    let pan = if chars.len() >= 12 {
        g[2..12].to_string()
    } else {
        String::new()
    };
    let pan_ok = pan.len() == 10
        && pan[..5].chars().all(|c| c.is_ascii_uppercase())
        && pan[5..9].chars().all(|c| c.is_ascii_digit())
        && pan[9..].chars().all(|c| c.is_ascii_uppercase());
    if chars.len() >= 12 && !pan_ok {
        errors.push("Characters 3-12 must be a valid PAN (AAAAA9999A)".into());
    }
    let pan_type = pan.chars().nth(3).map(|c| match c {
        'P' => "Individual",
        'C' => "Company",
        'H' => "HUF",
        'F' => "Firm / LLP",
        'A' => "AOP",
        'T' => "Trust",
        'B' => "Body of individuals",
        'L' => "Local authority",
        'J' => "Artificial juridical person",
        'G' => "Government",
        _ => "Unknown",
    });
    if chars.len() >= 13 && !chars[12].is_ascii_alphanumeric() {
        errors.push("Character 13 (entity number) must be alphanumeric".into());
    }
    if chars.len() >= 14 && chars[13] != 'Z' {
        errors.push("Character 14 must be 'Z'".into());
    }
    let checksum_ok = if chars.len() == 15 {
        match gstin_checksum(&g[..14]) {
            Some(c) => {
                if c != chars[14] {
                    errors.push(format!("Checksum mismatch: expected '{c}'"));
                    false
                } else {
                    true
                }
            },
            None => {
                errors.push("Invalid characters for checksum".into());
                false
            },
        }
    } else {
        false
    };
    Ok(json!({
        "gstin": g,
        "valid": errors.is_empty(),
        "errors": errors,
        "state_code": state,
        "state": state_name,
        "pan": pan,
        "pan_holder_type": pan_type,
        "entity_number": chars.get(12).map(|c| c.to_string()),
        "checksum_valid": checksum_ok,
        "registration_type": if chars.get(12) == Some(&'Z') || chars.len() < 15 { Value::Null } else { json!("\
            Regular / composition (entity sequence within state)") },
    }))
}

fn gstin_checksum(first14: &str) -> Option<char> {
    const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut sum = 0u32;
    for (i, c) in first14.bytes().enumerate() {
        let v = ALPHABET.iter().position(|&a| a == c)? as u32;
        let factor = if i % 2 == 0 { 1 } else { 2 };
        let product = v * factor;
        sum += product / 36 + product % 36;
    }
    let check = (36 - sum % 36) % 36;
    Some(ALPHABET[check as usize] as char)
}

/// Checks a GST invoice number (≤16 chars, alphanumeric with `/` and `-`).
pub fn validate_invoice_number(number: &str) -> EngineResult<Value> {
    let n = number.trim();
    if n.is_empty() {
        return Err(EngineError::empty("invoice number"));
    }
    let mut errors: Vec<String> = Vec::new();
    if n.chars().count() > 16 {
        errors.push("Invoice number must be at most 16 characters (Rule 46)".into());
    }
    if !n.chars().all(|c| c.is_ascii_alphanumeric() || c == '/' || c == '-') {
        errors.push("Only letters, digits, '/' and '-' are allowed".into());
    }
    if n.starts_with(['/', '-']) || n.ends_with(['/', '-']) {
        errors.push("Must not start or end with a separator".into());
    }
    if !n.chars().any(|c| c.is_ascii_digit()) {
        errors.push("Should contain at least one digit".into());
    }
    Ok(json!({
        "invoice_number": n,
        "valid": errors.is_empty(),
        "errors": errors,
        "length": n.chars().count(),
        "has_series_prefix": n.contains(['/', '-'])
    }))
}

/// Invoice fields checked by [`validate_invoice`].
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Invoice {
    /// Invoice number.
    pub invoice_number: String,
    /// Invoice date (`YYYY-MM-DD` or `DD/MM/YYYY`).
    pub date: String,
    /// Supplier GSTIN.
    pub supplier_gstin: String,
    /// Buyer GSTIN (may be empty for B2C).
    pub buyer_gstin: String,
    /// Place of supply (state code or name).
    pub place_of_supply: String,
    /// Taxable value.
    pub taxable_value: f64,
    /// GST rate in percent.
    pub rate: f64,
    /// CGST amount.
    pub cgst: f64,
    /// SGST amount.
    pub sgst: f64,
    /// IGST amount.
    pub igst: f64,
    /// Invoice total.
    pub total: f64,
    /// Reverse-charge flag.
    pub reverse_charge: bool,
    /// HSN/SAC code.
    pub hsn: String,
}

/// Validates invoice fields and arithmetic (GSTINs, tax split, totals, HSN length).
pub fn validate_invoice(inv: &Invoice) -> EngineResult<Value> {
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let inv_num = validate_invoice_number(&inv.invoice_number);
    match inv_num {
        Ok(v) if v["valid"] == false => {
            errors.push(format!("Invoice number: {}", v["errors"][0].as_str().unwrap_or("")))
        },
        Err(_) => errors.push("Invoice number is required".into()),
        _ => {},
    }
    if inv.date.trim().is_empty() {
        errors.push("Invoice date is required".into());
    }
    let sup = validate_gstin(&inv.supplier_gstin).ok();
    match &sup {
        Some(v) if v["valid"] == false => errors.push("Supplier GSTIN is invalid".into()),
        None => errors.push("Supplier GSTIN is required".into()),
        _ => {},
    }
    let buyer = if inv.buyer_gstin.trim().is_empty() {
        None
    } else {
        validate_gstin(&inv.buyer_gstin).ok()
    };
    if let Some(v) = &buyer {
        if v["valid"] == false {
            errors.push("Buyer GSTIN is invalid".into());
        }
    } else if inv.taxable_value > 250_000.0 {
        warnings.push(
            "B2C invoices above ₹2.5 lakh need the recipient's name, address and state (Rule 46)"
                .into(),
        );
    }
    let pos = inv.place_of_supply.trim();
    let sup_state = sup
        .as_ref()
        .and_then(|v| v["state_code"].as_str().map(str::to_string))
        .unwrap_or_default();
    let pos_code = if pos.len() >= 2 && pos[..2].chars().all(|c| c.is_ascii_digit()) {
        pos[..2].to_string()
    } else {
        rules::STATE_CODES
            .iter()
            .find(|(_, n)| n.eq_ignore_ascii_case(pos))
            .map(|(c, _)| c.to_string())
            .unwrap_or_default()
    };
    if pos_code.is_empty() {
        errors.push("Place of supply (state code or name) is required".into());
    }
    let interstate = !pos_code.is_empty() && !sup_state.is_empty() && pos_code != sup_state;
    let expected_tax = inv.taxable_value * inv.rate / 100.0;
    if inv.taxable_value <= 0.0 {
        errors.push("Taxable value must be positive".into());
    }
    if interstate {
        if (inv.igst - expected_tax).abs() > 1.0 {
            errors.push(format!("Inter-state supply: IGST should be {:.2}", expected_tax));
        }
        if inv.cgst + inv.sgst > 0.0 {
            errors.push("Inter-state supply must not carry CGST/SGST".into());
        }
    } else if !pos_code.is_empty() {
        if (inv.cgst - expected_tax / 2.0).abs() > 1.0
            || (inv.sgst - expected_tax / 2.0).abs() > 1.0
        {
            errors.push(format!(
                "Intra-state supply: CGST and SGST should each be {:.2}",
                expected_tax / 2.0
            ));
        }
        if inv.igst > 0.0 {
            errors.push("Intra-state supply must not carry IGST".into());
        }
    }
    let computed_total = inv.taxable_value + inv.cgst + inv.sgst + inv.igst;
    if inv.total > 0.0 && (inv.total - computed_total).abs() > 1.0 {
        errors.push(format!(
            "Total {:.2} does not match taxable + tax = {:.2}",
            inv.total, computed_total
        ));
    }
    if !rules::GST_SLABS.contains(&inv.rate) {
        warnings.push(format!("{}% is not a standard GST slab", inv.rate));
    }
    if inv.hsn.trim().is_empty() {
        warnings.push("HSN/SAC code is missing (mandatory for most registered suppliers)".into());
    } else if !(4..=8).contains(&inv.hsn.trim().len())
        || !inv.hsn.trim().chars().all(|c| c.is_ascii_digit())
    {
        errors.push("HSN/SAC must be 4 to 8 digits".into());
    }
    if inv.reverse_charge {
        warnings.push("Reverse charge: the recipient pays the tax".into());
    }
    Ok(json!({
        "valid": errors.is_empty(),
        "errors": errors,
        "warnings": warnings,
        "supply_type": if interstate { "inter-state" } else { "intra-state" },
        "expected_tax": rupee(expected_tax),
        "computed_total": rupee(computed_total),
        "supplier_state": sup_state,
        "place_of_supply_code": pos_code
    }))
}

/// Searches the bundled HSN chapter / SAC tables by code or description.
pub fn hsn_lookup(query: &str) -> EngineResult<Value> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Err(EngineError::empty("query"));
    }
    let digits = q.chars().all(|c| c.is_ascii_digit());
    let mut results = Vec::new();
    if digits {
        let chapter = &q[..q.len().min(2)];
        for (c, d) in rules::HSN_CHAPTERS {
            if *c == chapter {
                results.push(json!({
                    "code": c,
                    "description": d,
                    "kind": "HSN chapter",
                    "match": "prefix"
                }));
            }
        }
        for (c, d, r) in rules::SAC_HEADINGS {
            if c.starts_with(&q) || q.starts_with(c) {
                results.push(json!({
                    "code": c,
                    "description": d,
                    "kind": "SAC",
                    "typical_rate": r,
                    "match": "code"
                }));
            }
        }
    } else {
        for (c, d) in rules::HSN_CHAPTERS {
            if d.to_lowercase().contains(&q) {
                results.push(json!({
                    "code": c,
                    "description": d,
                    "kind": "HSN chapter",
                    "match": "description"
                }));
            }
        }
        for (c, d, r) in rules::SAC_HEADINGS {
            if d.to_lowercase().contains(&q) {
                results.push(json!({
                    "code": c,
                    "description": d,
                    "kind": "SAC",
                    "typical_rate": r,
                    "match": "description"
                }));
            }
        }
    }
    Ok(json!({
        "query": query,
        "results": results,
        "count": results.len(),
        "note": "Chapter/heading level dataset for guidance; confirm the 6/8-digit code and rate \
                 on the CBIC portal"
    }))
}

// ---------------------------------------------------------------- income --

/// TDS deduction for a section (rates, thresholds, no-PAN surcharge).
pub fn tds(
    amount: f64,
    section: &str,
    rate_override: Option<f64>,
    has_pan: bool,
    senior_citizen: bool,
) -> EngineResult<Value> {
    positive(amount, "amount")?;
    let table = rules::tds_sections();
    let sec = table["sections"].get(section.trim()).ok_or_else(|| {
        EngineError::invalid(format!(
            "Unknown TDS section `{section}`. Known: {}",
            table["sections"]
                .as_object()
                .map(|o| o.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default()
                .join(", ")
        ))
    })?;
    let base_rate = rate_override.or_else(|| sec["rate"].as_f64()).ok_or_else(|| {
        EngineError::invalid("This section uses slab rates; pass `rate_override`")
    })?;
    let threshold = if senior_citizen {
        sec["threshold_senior"].as_f64().or_else(|| sec["threshold"].as_f64())
    } else {
        sec["threshold"].as_f64()
    }
    .unwrap_or(0.0);
    let rate = if has_pan { base_rate } else { base_rate.max(20.0) };
    let applicable = amount > threshold || threshold == 0.0;
    let tds_amount = if applicable { amount * rate / 100.0 } else { 0.0 };
    Ok(json!({
        "section": section,
        "name": sec["name"],
        "rate_percent": rate,
        "threshold": threshold,
        "applicable": applicable,
        "tds": rupee(tds_amount),
        "net_payment": rupee(amount - tds_amount),
        "no_pan_penalty_rate": !has_pan,
        "source": table["source"],
        "note": sec["note"]
    }))
}

/// Inputs for the income-tax calculator.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct TaxInputs {
    /// Gross total income.
    pub income: f64,
    /// `new` (default) or `old` regime.
    pub regime: String,
    /// Financial year key (e.g. `2024-25`).
    pub financial_year: String,
    /// Salaried (enables the standard deduction).
    pub is_salaried: bool,
    /// Section 80C deductions (old regime).
    pub deductions_80c: f64,
    /// Section 80D deductions (old regime).
    pub deductions_80d: f64,
    /// Other chapter VI-A deductions (old regime).
    pub other_deductions: f64,
    /// HRA exemption (old regime).
    pub hra_exemption: f64,
    /// Home-loan interest (old regime, section 24).
    pub home_loan_interest: f64,
    /// Age (senior citizen thresholds).
    pub age: u32,
    /// Overrides: [[from, to, rate]] – replaces the preset slabs.
    pub slabs: Vec<Vec<Option<f64>>>,
    /// Override the cess percentage.
    pub cess_percent: Option<f64>,
    /// Override the standard deduction.
    pub standard_deduction: Option<f64>,
    /// Override the section 87A rebate limit.
    pub rebate_limit: Option<f64>,
}

fn slab_tax(taxable: f64, slabs: &[(f64, Option<f64>, f64)]) -> (f64, Vec<Value>) {
    let mut tax = 0.0;
    let mut rows = Vec::new();
    for (from, to, rate) in slabs {
        if taxable <= *from {
            break;
        }
        let upper = to.unwrap_or(f64::INFINITY);
        let portion = taxable.min(upper) - from;
        let t = portion * rate / 100.0;
        tax += t;
        rows.push(json!({
            "from": from,
            "to": to,
            "rate": rate,
            "amount": rupee(portion),
            "tax": rupee(t)
        }));
    }
    (tax, rows)
}

fn preset_slabs(preset: &Value) -> Vec<(f64, Option<f64>, f64)> {
    preset["slabs"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|s| {
                    (s[0].as_f64().unwrap_or(0.0), s[1].as_f64(), s[2].as_f64().unwrap_or(0.0))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Income tax for one regime with slab breakdown, surcharge, cess and rebate.
pub fn income_tax(inp: &TaxInputs) -> EngineResult<Value> {
    if !inp.income.is_finite() || inp.income < 0.0 {
        return Err(EngineError::invalid("income must be a non-negative number"));
    }
    let presets = rules::income_tax_presets();
    let fy = if inp.financial_year.is_empty() {
        "FY2024-25"
    } else {
        inp.financial_year.trim()
    };
    let regime = if inp.regime.is_empty() {
        "new"
    } else {
        inp.regime.trim()
    };
    let key = format!("{fy}-{regime}");
    let preset = presets.get(&key).ok_or_else(|| {
        EngineError::invalid(format!(
            "No preset `{key}`; available: {}. Pass `slabs` to use custom rates.",
            presets
                .as_object()
                .map(|o| o.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default()
                .join(", ")
        ))
    })?;
    let mut slabs = preset_slabs(preset);
    if !inp.slabs.is_empty() {
        slabs = inp
            .slabs
            .iter()
            .map(|s| {
                (
                    s.first().copied().flatten().unwrap_or(0.0),
                    s.get(1).copied().flatten(),
                    s.get(2).copied().flatten().unwrap_or(0.0),
                )
            })
            .collect();
    }
    // Senior citizen basic exemption (old regime).
    if regime == "old" && inp.slabs.is_empty() {
        let exemption = if inp.age >= 80 {
            500_000.0
        } else if inp.age >= 60 {
            300_000.0
        } else {
            250_000.0
        };
        if let Some(first) = slabs.first_mut() {
            first.1 = Some(exemption);
        }
        if let Some(second) = slabs.get_mut(1) {
            second.0 = exemption;
        }
    }
    let std_ded = if inp.is_salaried {
        inp.standard_deduction
            .unwrap_or_else(|| preset["standard_deduction_salary"].as_f64().unwrap_or(0.0))
    } else {
        0.0
    };
    let chapter_via =
        if preset["allows_chapter_via"].as_bool().unwrap_or(false) || !inp.slabs.is_empty() {
            inp.deductions_80c.min(150_000.0)
                + inp.deductions_80d
                + inp.other_deductions
                + inp.hra_exemption
                + inp.home_loan_interest.min(200_000.0)
        } else {
            0.0
        };
    let ignored = if chapter_via == 0.0
        && (inp.deductions_80c
            + inp.deductions_80d
            + inp.other_deductions
            + inp.hra_exemption
            + inp.home_loan_interest)
            > 0.0
    {
        Some("Chapter VI-A / HRA / home-loan deductions are not available in the new regime")
    } else {
        None
    };
    let taxable = (inp.income - std_ded - chapter_via).max(0.0);
    let (mut tax, rows) = slab_tax(taxable, &slabs);
    let rebate_limit = inp
        .rebate_limit
        .unwrap_or_else(|| preset["rebate_limit"].as_f64().unwrap_or(0.0));
    let rebate = if taxable <= rebate_limit {
        tax.min(preset["rebate_max"].as_f64().unwrap_or(tax))
    } else {
        0.0
    };
    tax -= rebate;
    // Marginal relief for the new regime just above the rebate limit.
    let mut marginal_relief = 0.0;
    if regime == "new"
        && taxable > rebate_limit
        && rebate_limit > 0.0
        && tax > taxable - rebate_limit
    {
        marginal_relief = tax - (taxable - rebate_limit);
        tax -= marginal_relief;
    }
    let surcharge_rate = preset["surcharge"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|s| taxable > s[0].as_f64().unwrap_or(f64::INFINITY))
                .map(|s| s[1].as_f64().unwrap_or(0.0))
                .fold(0.0, f64::max)
        })
        .unwrap_or(0.0);
    let surcharge = tax * surcharge_rate / 100.0;
    let cess = (tax + surcharge)
        * inp
            .cess_percent
            .unwrap_or_else(|| preset["cess_percent"].as_f64().unwrap_or(4.0))
        / 100.0;
    let total = tax + surcharge + cess;
    Ok(json!({
        "preset": key,
        "label": preset["label"],
        "source": preset["source"],
        "gross_income": rupee(inp.income),
        "standard_deduction": rupee(std_ded),
        "chapter_via_deductions": rupee(chapter_via),
        "taxable_income": rupee(taxable),
        "slab_tax": rupee(tax + rebate + marginal_relief),
        "rebate_87a": rupee(rebate),
        "marginal_relief": rupee(marginal_relief),
        "tax_after_rebate": rupee(tax),
        "surcharge": rupee(surcharge),
        "surcharge_rate": surcharge_rate,
        "cess": rupee(cess),
        "total_tax": rupee(total),
        "monthly_tds": rupee(total / 12.0),
        "net_income": rupee(inp.income - total),
        "effective_rate_percent": if inp.income > 0.0 { rupee(total / inp.income * 100.0) } else { 0.0 },
        "slabs": rows,
        "ignored_deductions_note": ignored,
    }))
}

/// Runs both regimes and recommends the cheaper one.
pub fn compare_regimes(inp: &TaxInputs) -> EngineResult<Value> {
    let mut new_inp = inp.clone();
    new_inp.regime = "new".into();
    new_inp.slabs.clear();
    let mut old_inp = inp.clone();
    old_inp.regime = "old".into();
    old_inp.slabs.clear();
    let n = income_tax(&new_inp)?;
    let o = income_tax(&old_inp)?;
    let (nt, ot) = (n["total_tax"].as_f64().unwrap_or(0.0), o["total_tax"].as_f64().unwrap_or(0.0));
    Ok(json!({
        "new_regime": n,
        "old_regime": o,
        "better": if nt <= ot { "new" } else { "old" },
        "savings": rupee((nt - ot).abs())
    }))
}

/// Inputs for the CTC → in-hand calculator.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct SalaryInputs {
    /// Annual cost to company.
    pub ctc: f64,
    /// Basic salary as a percentage of CTC.
    pub basic_percent: f64,
    /// HRA as a percentage of basic.
    pub hra_percent_of_basic: f64,
    /// Employer PF is part of the CTC figure.
    pub employer_pf_included_in_ctc: bool,
    /// Employee PF contribution in percent of basic.
    pub employee_pf_percent: f64,
    /// Employer PF contribution in percent of basic.
    pub employer_pf_percent: f64,
    /// Wage ceiling for PF (₹15 000 statutory).
    pub pf_wage_cap: f64,
    /// Gratuity provision is part of the CTC figure.
    pub gratuity_included: bool,
    /// Professional tax per month (overrides the state preset).
    pub professional_tax_monthly: f64,
    /// State for the professional-tax preset.
    pub state: String,
    /// Metro city (50 % HRA exemption instead of 40 %).
    pub metro_city: bool,
    /// Monthly rent paid (HRA exemption).
    pub rent_paid_monthly: f64,
    /// Variable pay included in CTC.
    pub variable_pay: f64,
    /// Other monthly allowances.
    pub other_allowances_monthly: f64,
    /// `new` or `old` regime.
    pub regime: String,
    /// Financial year key.
    pub financial_year: String,
    /// Section 80C deductions (old regime).
    pub deductions_80c: f64,
    /// Section 80D deductions (old regime).
    pub deductions_80d: f64,
}

/// CTC → monthly in-hand breakdown (PF, professional tax, income tax, HRA).
pub fn salary_in_hand(s: &SalaryInputs) -> EngineResult<Value> {
    positive(s.ctc, "ctc")?;
    let basic_pct = if s.basic_percent <= 0.0 { 40.0 } else { s.basic_percent };
    let hra_pct = if s.hra_percent_of_basic <= 0.0 {
        if s.metro_city {
            50.0
        } else {
            40.0
        }
    } else {
        s.hra_percent_of_basic
    };
    let emp_pf_pct = if s.employee_pf_percent <= 0.0 {
        12.0
    } else {
        s.employee_pf_percent
    };
    let er_pf_pct = if s.employer_pf_percent <= 0.0 {
        12.0
    } else {
        s.employer_pf_percent
    };
    let fixed_ctc = s.ctc - s.variable_pay.max(0.0);
    let basic = fixed_ctc * basic_pct / 100.0;
    let pf_wage = if s.pf_wage_cap > 0.0 {
        basic.min(s.pf_wage_cap)
    } else {
        basic
    };
    let employer_pf = pf_wage * er_pf_pct / 100.0;
    let gratuity = if s.gratuity_included { basic * 4.81 / 100.0 } else { 0.0 };
    let employee_pf = pf_wage * emp_pf_pct / 100.0;
    let hra = basic * hra_pct / 100.0;
    let gross = fixed_ctc
        - if s.employer_pf_included_in_ctc {
            employer_pf
        } else {
            0.0
        }
        - gratuity;
    let special = (gross - basic - hra - s.other_allowances_monthly * 12.0).max(0.0);
    // HRA exemption (old regime): min(HRA, rent − 10% basic, 50/40% basic).
    let rent = s.rent_paid_monthly * 12.0;
    let hra_exempt = if rent > 0.0 {
        hra.min((rent - 0.1 * basic).max(0.0))
            .min(basic * if s.metro_city { 0.5 } else { 0.4 })
    } else {
        0.0
    };
    let pt = if s.professional_tax_monthly > 0.0 {
        s.professional_tax_monthly * 12.0
    } else if !s.state.is_empty() {
        professional_tax(gross / 12.0, &s.state, None)
            .map(|v| v["yearly"].as_f64().unwrap_or(0.0))
            .unwrap_or(0.0)
    } else {
        0.0
    };
    let tax_inp = TaxInputs {
        income: gross + s.variable_pay.max(0.0),
        regime: s.regime.clone(),
        financial_year: s.financial_year.clone(),
        is_salaried: true,
        deductions_80c: s.deductions_80c.max(employee_pf),
        deductions_80d: s.deductions_80d,
        hra_exemption: hra_exempt,
        other_deductions: pt,
        ..Default::default()
    };
    let tax = income_tax(&tax_inp)?;
    let total_tax = tax["total_tax"].as_f64().unwrap_or(0.0);
    let net_yearly = gross - employee_pf - pt - total_tax;
    Ok(json!({
        "ctc": rupee(s.ctc),
        "fixed_ctc": rupee(fixed_ctc),
        "variable_pay": rupee(s.variable_pay),
        "components_yearly": {
            "basic": rupee(basic),
            "hra": rupee(hra),
            "special_allowance": rupee(special),
            "other_allowances": rupee(s.other_allowances_monthly * 12.0),
            "employer_pf": rupee(employer_pf),
            "gratuity": rupee(gratuity)
        },
        "gross_salary_yearly": rupee(gross),
        "gross_salary_monthly": rupee(gross / 12.0),
        "deductions_yearly": {
            "employee_pf": rupee(employee_pf),
            "professional_tax": rupee(pt),
            "income_tax": rupee(total_tax)
        },
        "hra_exemption_old_regime": rupee(hra_exempt),
        "in_hand_yearly": rupee(net_yearly),
        "in_hand_monthly": rupee(net_yearly / 12.0),
        "tax": tax,
        "assumptions": {
            "basic_percent": basic_pct,
            "hra_percent_of_basic": hra_pct,
            "employee_pf_percent": emp_pf_pct,
            "employer_pf_percent": er_pf_pct,
            "pf_wage_cap": s.pf_wage_cap
        },
    }))
}

/// Professional tax from the state slab presets.
pub fn professional_tax(
    monthly_salary: f64,
    state: &str,
    gender: Option<&str>,
) -> EngineResult<Value> {
    if !monthly_salary.is_finite() || monthly_salary < 0.0 {
        return Err(EngineError::invalid("monthly_salary must be non-negative"));
    }
    let presets = rules::professional_tax_presets();
    let key = state.trim().to_lowercase().replace(' ', "-");
    let st = presets["states"].get(&key).ok_or_else(|| {
        EngineError::invalid(format!(
            "No professional tax preset for `{state}`; available: {}",
            presets["states"]
                .as_object()
                .map(|o| o.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default()
                .join(", ")
        ))
    })?;
    if key == "maharashtra"
        && gender
            .map(|g| g.eq_ignore_ascii_case("female") || g.eq_ignore_ascii_case("f"))
            .unwrap_or(false)
        && monthly_salary <= 25_000.0
    {
        return Ok(json!({
            "state": key,
            "monthly": 0.0,
            "yearly": 0.0,
            "note": "Women earning up to ₹25,000/month are exempt in Maharashtra",
            "source": presets["source"]
        }));
    }
    let monthly = st["slabs"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|s| {
            monthly_salary >= s[0].as_f64().unwrap_or(0.0)
                && s[1].as_f64().map(|to| monthly_salary < to).unwrap_or(true)
        })
        .and_then(|s| s[2].as_f64())
        .unwrap_or(0.0);
    let feb = st["february_amount"].as_f64().unwrap_or(monthly);
    let yearly = if monthly > 0.0 { monthly * 11.0 + feb } else { 0.0 };
    Ok(json!({
        "state": key,
        "monthly": monthly,
        "february": if monthly > 0.0 { feb } else { 0.0 },
        "yearly": yearly,
        "note": st["note"],
        "source": presets["source"]
    }))
}

/// Capital-gains tax by asset class and holding period (LTCG/STCG, indexation where applicable).
pub fn capital_gains(
    asset: &str,
    buy_price: f64,
    sell_price: f64,
    holding_months: u32,
    expenses: f64,
    slab_rate_percent: f64,
    ltcg_rate: Option<f64>,
    stcg_rate: Option<f64>,
    exemption_override: Option<f64>,
) -> EngineResult<Value> {
    positive(buy_price, "buy_price")?;
    positive(sell_price, "sell_price")?;
    let presets = rules::capital_gains_presets();
    let key = asset.trim().to_lowercase();
    let a = presets["assets"].get(&key).ok_or_else(|| {
        EngineError::invalid(format!(
            "Unknown asset `{asset}`; available: {}",
            presets["assets"]
                .as_object()
                .map(|o| o.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default()
                .join(", ")
        ))
    })?;
    let gain = sell_price - buy_price - expenses.max(0.0);
    let lt_months = a["long_term_months"].as_u64().unwrap_or(12) as u32;
    let long_term = lt_months > 0 && holding_months >= lt_months;
    let (rate, kind, exemption) = if long_term {
        (
            ltcg_rate.or_else(|| a["ltcg_rate"].as_f64()),
            "long-term",
            exemption_override.unwrap_or_else(|| a["ltcg_exemption"].as_f64().unwrap_or(0.0)),
        )
    } else {
        (stcg_rate.or_else(|| a["stcg_rate"].as_f64()), "short-term", 0.0)
    };
    let effective_rate = rate.unwrap_or(slab_rate_percent);
    let taxable_gain = (gain - exemption).max(0.0);
    let tax = if gain <= 0.0 {
        0.0
    } else {
        taxable_gain * effective_rate / 100.0
    };
    let cess = tax * 0.04;
    Ok(json!({
        "asset": a["label"],
        "holding_months": holding_months,
        "classification": kind,
        "long_term_threshold_months": lt_months,
        "gain": rupee(gain),
        "exemption_used": rupee(exemption.min(gain.max(0.0))),
        "taxable_gain": rupee(taxable_gain),
        "rate_percent": effective_rate,
        "rate_basis": if rate.is_some() { "special rate" } else { "slab rate (supplied)" },
        "tax": rupee(tax),
        "cess": rupee(cess),
        "total_tax": rupee(tax + cess),
        "net_proceeds": rupee(sell_price - expenses.max(0.0) - tax - cess),
        "is_loss": gain < 0.0,
        "note": a["note"],
        "source": presets["source"],
    }))
}

// ------------------------------------------------------------ statements --

#[derive(Debug, Clone)]
struct Txn {
    date: String,
    description: String,
    debit: f64,
    credit: f64,
    balance: Option<f64>,
    reference: String,
}

fn parse_amount(s: &str) -> Option<f64> {
    let t = s.trim().replace(['₹', ',', ' '], "");
    if t.is_empty() || t == "-" {
        return None;
    }
    let neg = t.ends_with("Dr") || t.ends_with("DR") || t.starts_with('-');
    let cleaned: String = t.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
    cleaned.parse::<f64>().ok().map(|v| if neg { -v } else { v })
}

fn normalise_date(s: &str) -> String {
    let t = s.trim();
    let parts: Vec<&str> = t.split(['/', '-', '.']).collect();
    if parts.len() == 3 {
        let (a, b, c) = (parts[0], parts[1], parts[2]);
        if a.len() == 4 {
            return format!("{a}-{:0>2}-{:0>2}", b, c);
        }
        let year = if c.len() == 2 { format!("20{c}") } else { c.to_string() };
        let month = match b.to_lowercase().as_str() {
            "jan" => "01",
            "feb" => "02",
            "mar" => "03",
            "apr" => "04",
            "may" => "05",
            "jun" => "06",
            "jul" => "07",
            "aug" => "08",
            "sep" => "09",
            "oct" => "10",
            "nov" => "11",
            "dec" => "12",
            _ => b,
        };
        return format!("{year}-{:0>2}-{:0>2}", month, a);
    }
    t.to_string()
}

fn classify(desc: &str) -> &'static str {
    let d = desc.to_uppercase();
    if d.contains("UPI") {
        "UPI"
    } else if d.contains("NEFT") {
        "NEFT"
    } else if d.contains("IMPS") {
        "IMPS"
    } else if d.contains("RTGS") {
        "RTGS"
    } else if d.contains("ATM") || d.contains("CASH") {
        "Cash"
    } else if d.contains("POS") || d.contains("CARD") {
        "Card"
    } else if d.contains("EMI") || d.contains("LOAN") {
        "Loan/EMI"
    } else if d.contains("SALARY") || d.contains("SAL ") {
        "Salary"
    } else if d.contains("INT") && (d.contains("CREDIT") || d.contains("PAID"))
        || d.contains("INTEREST")
    {
        "Interest"
    } else if d.contains("CHARGE") || d.contains("FEE") || d.contains("GST") {
        "Charges"
    } else if d.contains("CHQ") || d.contains("CHEQUE") {
        "Cheque"
    } else if d.contains("SIP") || d.contains("MF ") || d.contains("MUTUAL") {
        "Investment"
    } else {
        "Other"
    }
}

fn parse_statement(text: &str) -> EngineResult<(Vec<Txn>, Vec<String>)> {
    if text.trim().is_empty() {
        return Err(EngineError::empty("statement"));
    }
    let delimiter = if text.lines().take(5).filter(|l| l.contains(',')).count() >= 2 {
        b','
    } else if text.contains('\t') {
        b'\t'
    } else {
        b'|'
    };
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .has_headers(false)
        .trim(csv::Trim::All)
        .from_reader(text.as_bytes());
    let mut rows: Vec<Vec<String>> = Vec::new();
    for rec in rdr.records().flatten() {
        let r: Vec<String> = rec.iter().map(|s| s.to_string()).collect();
        if r.iter().any(|c| !c.is_empty()) {
            rows.push(r);
        }
    }
    if rows.len() < 2 {
        // Whitespace separated fallback: date description amount [balance]
        let mut out = Vec::new();
        for line in text.lines() {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() >= 3 && looks_like_date(cols[0]) {
                let amounts: Vec<f64> =
                    cols.iter().rev().take(3).filter_map(|c| parse_amount(c)).collect();
                if let Some(&amount) = amounts.first() {
                    let desc = cols[1..cols.len() - amounts.len().min(cols.len() - 1)].join(" ");
                    out.push(Txn {
                        date: normalise_date(cols[0]),
                        description: desc,
                        debit: if amount < 0.0 { -amount } else { 0.0 },
                        credit: if amount > 0.0 { amount } else { 0.0 },
                        balance: amounts.get(1).copied(),
                        reference: String::new(),
                    });
                }
            }
        }
        if out.is_empty() {
            return Err(EngineError::malformed(
                "Could not detect transactions. Paste CSV/TSV with columns like date, \
                 description, debit, credit, balance",
            ));
        }
        return Ok((
            out,
            vec!["Parsed whitespace separated lines (date description amount [balance])".into()],
        ));
    }
    // Header detection.
    let header: Vec<String> = rows[0].iter().map(|h| h.to_lowercase()).collect();
    let find = |names: &[&str]| header.iter().position(|h| names.iter().any(|n| h.contains(n)));
    let has_header = find(&["date"]).is_some()
        || find(&["narration", "description", "particulars", "details"]).is_some();
    let (date_i, desc_i, debit_i, credit_i, bal_i, amt_i, ref_i) = if has_header {
        (
            find(&["date"]),
            find(&["narration", "description", "particulars", "details", "remarks"]),
            find(&["debit", "withdrawal", "dr"]),
            find(&["credit", "deposit", "cr"]),
            find(&["balance"]),
            find(&["amount"]),
            find(&["ref", "chq", "cheque", "utr"]),
        )
    } else {
        (Some(0), Some(1), None, None, None, Some(2), None)
    };
    let date_i = date_i.ok_or_else(|| EngineError::malformed("No date column found"))?;
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    for (i, r) in rows.iter().enumerate().skip(if has_header { 1 } else { 0 }) {
        let get = |idx: Option<usize>| idx.and_then(|i| r.get(i)).cloned().unwrap_or_default();
        let date = get(Some(date_i));
        if !looks_like_date(&date) {
            if i > 0 && warnings.len() < 10 {
                warnings.push(format!("row {}: skipped (no date)", i + 1));
            }
            continue;
        }
        let (debit, credit) = match (debit_i, credit_i, amt_i) {
            (Some(d), Some(c), _) => (
                parse_amount(&get(Some(d))).unwrap_or(0.0).abs(),
                parse_amount(&get(Some(c))).unwrap_or(0.0).abs(),
            ),
            (_, _, Some(a)) => {
                let v = parse_amount(&get(Some(a))).unwrap_or(0.0);
                if v < 0.0 {
                    (-v, 0.0)
                } else {
                    (0.0, v)
                }
            },
            _ => (0.0, 0.0),
        };
        out.push(Txn {
            date: normalise_date(&date),
            description: get(desc_i).split_whitespace().collect::<Vec<_>>().join(" "),
            debit,
            credit,
            balance: parse_amount(&get(bal_i)),
            reference: get(ref_i),
        });
    }
    if out.is_empty() {
        return Err(EngineError::malformed("No transaction rows could be parsed"));
    }
    Ok((out, warnings))
}

fn looks_like_date(s: &str) -> bool {
    let t = s.trim();
    t.len() >= 6
        && t.chars().filter(|c| c.is_ascii_digit()).count() >= 4
        && t.contains(['/', '-', '.'])
}

/// Parses bank-statement text/CSV into transactions and exports CSV / Tally XML.
pub fn bank_statement(
    text: &str,
    output: &str,
    ledger_name: &str,
    company: &str,
) -> EngineResult<Value> {
    let (txns, warnings) = parse_statement(text)?;
    let total_debit: f64 = txns.iter().map(|t| t.debit).sum();
    let total_credit: f64 = txns.iter().map(|t| t.credit).sum();
    let mut categories: Map<String, Value> = Map::new();
    for t in &txns {
        let c = classify(&t.description);
        let e = categories
            .entry(c.to_string())
            .or_insert(json!({ "count": 0, "debit": 0.0, "credit": 0.0 }));
        e["count"] = json!(e["count"].as_u64().unwrap_or(0) + 1);
        e["debit"] = json!(rupee(e["debit"].as_f64().unwrap_or(0.0) + t.debit));
        e["credit"] = json!(rupee(e["credit"].as_f64().unwrap_or(0.0) + t.credit));
    }
    let rows: Vec<Value> = txns
        .iter()
        .map(|t| {
            json!({
                "date": t.date,
                "description": t.description,
                "debit": rupee(t.debit),
                "credit": rupee(t.credit),
                "balance": t.balance.map(rupee),
                "reference": t.reference,
                "category": classify(&t.description)
            })
        })
        .collect();
    let mut csv_out = String::from("date,description,reference,category,debit,credit,balance\n");
    for t in &txns {
        let esc = |s: &str| {
            if s.contains([',', '"']) {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.to_string()
            }
        };
        csv_out.push_str(&format!(
            "{},{},{},{},{:.2},{:.2},{}\n",
            t.date,
            esc(&t.description),
            esc(&t.reference),
            classify(&t.description),
            t.debit,
            t.credit,
            t.balance.map(|b| format!("{b:.2}")).unwrap_or_default()
        ));
    }
    let tally = if output == "tally" || output == "all" {
        let bank = if ledger_name.is_empty() { "Bank" } else { ledger_name };
        let mut xml = format!("<ENVELOPE>\n<HEADER><TALLYREQUEST>Import \
                               Data</TALLYREQUEST></HEADER>\n<BODY><IMPORTDATA><REQUESTDESC><REPORTNAME>Vouchers</REPORTNAME><STATICVARIABLES><SVCURRENTCOMPANY>{}</SVCURRENTCOMPANY></STATICVARIABLES></REQUESTDESC>\n<REQUESTDATA>\n", xml_escape(company));
        for t in &txns {
            let (vtype, amount, ledger_first_dr) = if t.debit > 0.0 {
                ("Payment", t.debit, true)
            } else {
                ("Receipt", t.credit, false)
            };
            let d = t.date.replace('-', "");
            let party = xml_escape(&format!(
                "{} ({})",
                classify(&t.description),
                t.description.chars().take(40).collect::<String>()
            ));
            xml.push_str(&format!("<TALLYMESSAGE xmlns:UDF=\"TallyUDF\"><VOUCHER \
                                   VCHTYPE=\"{vtype}\" \
                                   ACTION=\"Create\"><DATE>{d}</DATE><VOUCHERTYPENAME>{vtype}</VOUCHERTYPENAME><NARRATION>{}</NARRATION>", xml_escape(&t.description)));
            xml.push_str(&format!("<ALLLEDGERENTRIES.LIST><LEDGERNAME>{}</LEDGERNAME><ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE><AMOUNT>-{amount:.2}</AMOUNT></ALLLEDGERENTRIES.LIST>", if ledger_first_dr { party.clone() } else { xml_escape(bank) }));
            xml.push_str(&format!("<ALLLEDGERENTRIES.LIST><LEDGERNAME>{}</LEDGERNAME><ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE><AMOUNT>{amount:.2}</AMOUNT></ALLLEDGERENTRIES.LIST></VOUCHER></TALLYMESSAGE>\n", if ledger_first_dr { xml_escape(bank) } else { party }));
        }
        xml.push_str("</REQUESTDATA></IMPORTDATA></BODY></ENVELOPE>\n");
        Some(xml)
    } else {
        None
    };
    Ok(json!({
        "transactions": rows,
        "count": txns.len(),
        "summary": {
            "total_debit": rupee(total_debit),
            "total_credit": rupee(total_credit),
            "net": rupee(total_credit - total_debit),
            "opening_balance": txns.first().and_then(|t| t.balance.map(|b| rupee(b - t.credit + t.debit))),
            "closing_balance": txns.last().and_then(|t| t.balance.map(rupee)),
            "from": txns.iter().map(|t| t.date.clone()).min(),
            "to": txns.iter().map(|t| t.date.clone()).max()
        },
        "categories": categories,
        "csv": csv_out,
        "tally_xml": tally,
        "warnings": warnings,
        "note": "Review Tally ledger names before importing; party ledgers are derived from \
                 narrations",
    }))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// --------------------------------------------------------- reconciliation --

fn parse_register(text: &str) -> EngineResult<Vec<Map<String, Value>>> {
    if text.trim().is_empty() {
        return Err(EngineError::empty("register"));
    }
    if text.trim_start().starts_with('[') {
        let v: Vec<Map<String, Value>> = serde_json::from_str(text)
            .map_err(|e| EngineError::malformed(format!("Invalid JSON register: {e}")))?;
        return Ok(v
            .into_iter()
            .map(|m| {
                m.into_iter()
                    .map(|(k, v)| (k.to_lowercase().replace([' ', '_', '-'], ""), v))
                    .collect()
            })
            .collect());
    }
    let delimiter = if text.lines().next().map(|l| l.contains('\t')).unwrap_or(false) {
        b'\t'
    } else {
        b','
    };
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(text.as_bytes());
    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| EngineError::malformed(e.to_string()))?
        .iter()
        .map(|h| h.to_lowercase().replace([' ', '_', '-'], ""))
        .collect();
    let mut out = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(|e| EngineError::malformed(format!("Invalid CSV: {e}")))?;
        let mut m = Map::new();
        for (i, f) in rec.iter().enumerate() {
            if let Some(h) = headers.get(i) {
                m.insert(h.clone(), json!(f));
            }
        }
        out.push(m);
    }
    Ok(out)
}

fn field<'a>(m: &'a Map<String, Value>, names: &[&str]) -> Option<&'a Value> {
    names.iter().find_map(|n| m.get(*n))
}

fn num(v: Option<&Value>) -> f64 {
    match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(s)) => parse_amount(s).unwrap_or(0.0),
        _ => 0.0,
    }
}

fn text_of(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.trim().to_uppercase(),
        Some(other) if !other.is_null() => other.to_string().to_uppercase(),
        _ => String::new(),
    }
}

fn norm_invoice(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .trim_start_matches('0')
        .to_uppercase()
}

/// Reconciles a purchase/sales register against the GST portal data (GSTR-2A/2B/1).
pub fn reconcile(books: &str, portal: &str, tolerance: f64, kind: &str) -> EngineResult<Value> {
    let a = parse_register(books)?;
    let b = parse_register(portal)?;
    let tol = if tolerance <= 0.0 { 1.0 } else { tolerance };
    let key_of = |m: &Map<String, Value>| -> (String, String) {
        let gstin = text_of(field(
            m,
            &[
                "gstin",
                "suppliergstin",
                "gstinofsupplier",
                "ctin",
                "customergstin",
                "recipientgstin",
                "partygstin",
            ],
        ));
        let inv = norm_invoice(&text_of(field(
            m,
            &["invoicenumber", "invoiceno", "invno", "inum", "invoice", "billno", "number"],
        )));
        (gstin, inv)
    };
    let amount_of = |m: &Map<String, Value>| -> (f64, f64) {
        let taxable =
            num(field(m, &["taxablevalue", "taxable", "txval", "taxableamount", "basicamount"]));
        let tax = num(field(m, &["igst", "iamt"]))
            + num(field(m, &["cgst", "camt"]))
            + num(field(m, &["sgst", "samt"]))
            + num(field(m, &["tax", "totaltax", "gst"]));
        let total = num(field(m, &["total", "invoicevalue", "val", "grandtotal", "amount"]));
        (taxable, if total > 0.0 { total } else { taxable + tax })
    };
    let mut portal_map: std::collections::HashMap<(String, String), Vec<usize>> =
        std::collections::HashMap::new();
    for (i, m) in b.iter().enumerate() {
        portal_map.entry(key_of(m)).or_default().push(i);
    }
    let mut used_portal = vec![false; b.len()];
    let mut matched = Vec::new();
    let mut mismatched = Vec::new();
    let mut missing_in_portal = Vec::new();
    let mut probable = Vec::new();
    for (i, m) in a.iter().enumerate() {
        let key = key_of(m);
        let (ta, tot_a) = amount_of(m);
        let candidates = portal_map.get(&key).cloned().unwrap_or_default();
        let mut hit: Option<usize> = None;
        for c in &candidates {
            if !used_portal[*c] {
                hit = Some(*c);
                break;
            }
        }
        match hit {
            Some(j) => {
                used_portal[j] = true;
                let (tb, tot_b) = amount_of(&b[j]);
                let diff_taxable = (ta - tb).abs();
                let diff_total = (tot_a - tot_b).abs();
                let entry = json!({
                    "gstin": key.0,
                    "invoice": key.1,
                    "books_row": i + 1,
                    "portal_row": j + 1,
                    "taxable_books": rupee(ta),
                    "taxable_portal": rupee(tb),
                    "total_books": rupee(tot_a),
                    "total_portal": rupee(tot_b),
                    "difference": rupee(tot_a - tot_b)
                });
                if diff_taxable <= tol && diff_total <= tol {
                    matched.push(entry);
                } else {
                    mismatched.push(entry);
                }
            },
            None => {
                // Probable match: same GSTIN and same total within tolerance.
                let prob = b.iter().enumerate().find(|(j, pm)| {
                    !used_portal[*j]
                        && key_of(pm).0 == key.0
                        && (amount_of(pm).1 - tot_a).abs() <= tol
                });
                if let Some((j, pm)) = prob {
                    used_portal[j] = true;
                    probable.push(json!({
                        "gstin": key.0,
                        "invoice_books": key.1,
                        "invoice_portal": key_of(pm).1,
                        "total": rupee(tot_a),
                        "books_row": i + 1,
                        "portal_row": j + 1,
                        "reason": "invoice number differs, amount matches"
                    }));
                } else {
                    missing_in_portal.push(json!({
                        "gstin": key.0,
                        "invoice": key.1,
                        "total": rupee(tot_a),
                        "books_row": i + 1
                    }));
                }
            },
        }
    }
    let missing_in_books: Vec<Value> = b
        .iter()
        .enumerate()
        .filter(|(j, _)| !used_portal[*j])
        .map(|(j, m)| {
            let k = key_of(m);
            json!({
                "gstin": k.0,
                "invoice": k.1,
                "total": rupee(amount_of(m).1),
                "portal_row": j + 1
            })
        })
        .collect();
    let sum = |v: &[Value], k: &str| rupee(v.iter().map(|x| x[k].as_f64().unwrap_or(0.0)).sum());
    Ok(json!({
        "kind": if kind.is_empty() { "purchase" } else { kind },
        "summary": {
            "books_rows": a.len(),
            "portal_rows": b.len(),
            "matched": matched.len(),
            "mismatched": mismatched.len(),
            "probable": probable.len(),
            "missing_in_portal": missing_in_portal.len(),
            "missing_in_books": missing_in_books.len(),
            "tolerance": tol,
            "value_matched": sum(&matched, "total_books"),
            "value_missing_in_portal": sum(&missing_in_portal, "total"),
            "value_missing_in_books": sum(&missing_in_books, "total")
        },
        "matched": matched,
        "mismatched": mismatched,
        "probable_matches": probable,
        "missing_in_portal": missing_in_portal,
        "missing_in_books": missing_in_books,
        "itc_at_risk": if kind != "sales" { json!(sum(&missing_in_portal, "total")) } else { Value::Null },
    }))
}

/// Validates GSTR JSON structure (GSTIN, period, invoice sections).
pub fn validate_gstr(text: &str) -> EngineResult<Value> {
    let rows = parse_register(text)?;
    let mut issues = Vec::new();
    let mut totals = (0.0, 0.0);
    let mut seen = std::collections::HashSet::new();
    for (i, m) in rows.iter().enumerate() {
        let gstin =
            text_of(field(m, &["gstin", "ctin", "suppliergstin", "customergstin", "partygstin"]));
        let inv = text_of(field(m, &["invoicenumber", "invoiceno", "inum", "invoice", "invno"]));
        let taxable = num(field(m, &["taxablevalue", "taxable", "txval"]));
        let rate = num(field(m, &["rate", "rt", "gstrate"]));
        let tax = num(field(m, &["igst", "iamt"]))
            + num(field(m, &["cgst", "camt"]))
            + num(field(m, &["sgst", "samt"]));
        let row = i + 2;
        if gstin.is_empty() {
            issues.push(
                json!({ "row": row, "severity": "warning", "message": "Missing GSTIN (B2C?)" }),
            );
        } else if validate_gstin(&gstin).map(|v| v["valid"] == false).unwrap_or(true) {
            issues.push(json!({
                "row": row,
                "severity": "error",
                "message": format!("Invalid GSTIN {gstin}")
            }));
        }
        if inv.is_empty() {
            issues.push(
                json!({ "row": row, "severity": "error", "message": "Missing invoice number" }),
            );
        } else if !seen.insert((gstin.clone(), norm_invoice(&inv))) {
            issues.push(json!({
                "row": row,
                "severity": "error",
                "message": format!("Duplicate invoice {inv} for {gstin}")
            }));
        } else if let Ok(v) = validate_invoice_number(&inv) {
            if v["valid"] == false {
                issues.push(json!({
                    "row": row,
                    "severity": "warning",
                    "message": format!("Invoice number {inv}: {}", v["errors"][0].as_str().unwrap_or(""))
                }));
            }
        }
        if taxable <= 0.0 {
            issues.push(json!({
                "row": row,
                "severity": "error",
                "message": "Taxable value must be positive"
            }));
        }
        if rate > 0.0 && tax > 0.0 && (taxable * rate / 100.0 - tax).abs() > 1.0 {
            issues.push(json!({
                "row": row,
                "severity": "error",
                "message": format!("Tax {:.2} ≠ taxable × rate = {:.2}", tax, taxable * rate / 100.0)
            }));
        }
        if rate > 0.0 && !rules::GST_SLABS.contains(&rate) {
            issues.push(json!({
                "row": row,
                "severity": "warning",
                "message": format!("{rate}% is not a standard slab")
            }));
        }
        totals.0 += taxable;
        totals.1 += tax;
    }
    let errors = issues.iter().filter(|i| i["severity"] == "error").count();
    Ok(json!({
        "rows": rows.len(),
        "valid": errors == 0,
        "errors": errors,
        "warnings": issues.len() - errors,
        "issues": issues,
        "totals": { "taxable": rupee(totals.0), "tax": rupee(totals.1) }
    }))
}

fn default_true() -> bool {
    true
}
fn default_one() -> f64 {
    1.0
}

/// Every action understood by the India tax engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum IndiaTaxAction {
    /// GST calculator.
    Gst {
        /// Amount.
        amount: f64,
        /// GST rate in percent.
        rate: f64,
        /// `amount` already includes GST.
        #[serde(default)]
        inclusive: bool,
        /// Inter-state supply (IGST instead of CGST+SGST).
        #[serde(default)]
        interstate: bool,
        /// Compensation cess in percent.
        #[serde(default)]
        cess_percent: f64,
        /// Quantity.
        #[serde(default = "default_one")]
        quantity: f64,
    },
    /// Input tax credit set-off.
    Itc {
        /// Output tax liability per head (`igst`, `cgst`, `sgst`).
        #[serde(default)]
        output_tax: Map<String, Value>,
        /// Available input credit per head.
        #[serde(default)]
        input_tax: Map<String, Value>,
    },
    /// GSTIN format and checksum validation.
    ValidateGstin {
        /// GSTIN.
        gstin: String,
    },
    /// Invoice number validation.
    ValidateInvoiceNumber {
        /// Invoice number.
        number: String,
    },
    /// Full invoice validation.
    ValidateInvoice {
        /// Invoice fields.
        #[serde(flatten)]
        invoice: Invoice,
    },
    /// HSN/SAC search.
    HsnLookup {
        /// Code prefix or description words.
        query: String,
    },
    /// TDS calculator.
    Tds {
        /// Payment amount.
        amount: f64,
        /// Section (`194C`, `194J`, `192`, …).
        section: String,
        /// Override the statutory rate.
        #[serde(default)]
        rate_override: Option<f64>,
        /// Deductee has a PAN (otherwise 20 % applies).
        #[serde(default = "default_true")]
        has_pan: bool,
        /// Deductee is a senior citizen (194A thresholds).
        #[serde(default)]
        senior_citizen: bool,
    },
    /// Income-tax calculator.
    IncomeTax {
        /// Calculator inputs.
        #[serde(flatten)]
        inputs: TaxInputs,
        /// Compare old vs new regime.
        #[serde(default)]
        compare: bool,
    },
    /// CTC → in-hand calculator.
    SalaryInHand {
        /// Calculator inputs.
        #[serde(flatten)]
        inputs: SalaryInputs,
    },
    /// Professional tax lookup.
    ProfessionalTax {
        /// Monthly salary.
        monthly_salary: f64,
        /// State name.
        state: String,
        /// `male`/`female` (state-specific exemptions).
        #[serde(default)]
        gender: Option<String>,
    },
    /// Capital-gains calculator.
    CapitalGains {
        /// Asset class (`equity`, `debt_fund`, `property`, `gold`, …).
        asset: String,
        /// Purchase price.
        buy_price: f64,
        /// Sale price.
        sell_price: f64,
        /// Holding period in months.
        holding_months: u32,
        /// Transfer expenses.
        #[serde(default)]
        expenses: f64,
        /// Slab rate for STCG on non-equity assets.
        #[serde(default)]
        slab_rate_percent: f64,
        /// Override the LTCG rate.
        #[serde(default)]
        ltcg_rate: Option<f64>,
        /// Override the STCG rate.
        #[serde(default)]
        stcg_rate: Option<f64>,
        /// Override the LTCG exemption.
        #[serde(default)]
        exemption: Option<f64>,
    },
    /// Bank-statement parser / Tally exporter.
    BankStatement {
        /// Statement text (CSV or plain text).
        text: String,
        /// `csv`, `tally` or `all`.
        #[serde(default)]
        output: String,
        /// Tally ledger name for the bank account.
        #[serde(default)]
        ledger_name: String,
        /// Company name for the Tally XML.
        #[serde(default)]
        company: String,
    },
    /// GSTR JSON validation.
    ValidateGstr {
        /// GSTR JSON text.
        text: String,
    },
    /// Register reconciliation.
    Reconcile {
        /// Books register (CSV/JSON).
        books: String,
        /// Portal data (CSV/JSON).
        portal: String,
        /// Amount tolerance for a match.
        #[serde(default)]
        tolerance: f64,
        /// `purchase` (default) or `sales`.
        #[serde(default)]
        kind: String,
    },
    /// Returns the bundled rule tables and their version.
    Rules {},
}

/// Dispatches an [`IndiaTaxAction`] to its implementation.
pub fn handle_action(action: IndiaTaxAction) -> EngineResult<Value> {
    match action {
        IndiaTaxAction::Gst {
            amount,
            rate,
            inclusive,
            interstate,
            cess_percent,
            quantity,
        } => gst(amount, rate, inclusive, interstate, cess_percent, quantity),
        IndiaTaxAction::Itc { output_tax, input_tax } => itc(&output_tax, &input_tax),
        IndiaTaxAction::ValidateGstin { gstin } => validate_gstin(&gstin),
        IndiaTaxAction::ValidateInvoiceNumber { number } => validate_invoice_number(&number),
        IndiaTaxAction::ValidateInvoice { invoice } => validate_invoice(&invoice),
        IndiaTaxAction::HsnLookup { query } => hsn_lookup(&query),
        IndiaTaxAction::Tds {
            amount,
            section,
            rate_override,
            has_pan,
            senior_citizen,
        } => tds(amount, &section, rate_override, has_pan, senior_citizen),
        IndiaTaxAction::IncomeTax { inputs, compare } => {
            if compare {
                compare_regimes(&inputs)
            } else {
                income_tax(&inputs)
            }
        },
        IndiaTaxAction::SalaryInHand { inputs } => salary_in_hand(&inputs),
        IndiaTaxAction::ProfessionalTax { monthly_salary, state, gender } => {
            professional_tax(monthly_salary, &state, gender.as_deref())
        },
        IndiaTaxAction::CapitalGains {
            asset,
            buy_price,
            sell_price,
            holding_months,
            expenses,
            slab_rate_percent,
            ltcg_rate,
            stcg_rate,
            exemption,
        } => capital_gains(
            &asset,
            buy_price,
            sell_price,
            holding_months,
            expenses,
            slab_rate_percent,
            ltcg_rate,
            stcg_rate,
            exemption,
        ),
        IndiaTaxAction::BankStatement { text, output, ledger_name, company } => {
            bank_statement(&text, &output, &ledger_name, &company)
        },
        IndiaTaxAction::ValidateGstr { text } => validate_gstr(&text),
        IndiaTaxAction::Reconcile { books, portal, tolerance, kind } => {
            reconcile(&books, &portal, tolerance, &kind)
        },
        IndiaTaxAction::Rules {} => Ok(json!({
            "version": rules::RULES_VERSION,
            "gst_slabs": rules::GST_SLABS,
            "income_tax": rules::income_tax_presets(),
            "tds": rules::tds_sections(),
            "professional_tax": rules::professional_tax_presets(),
            "capital_gains": rules::capital_gains_presets(),
            "state_codes": rules::STATE_CODES.iter().map(|(c, n)| json!({ "code": c, "state": n })).collect::<Vec<_>>()
        })),
    }
}

engine_shared::export_engine!(handle_action, IndiaTaxAction);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gst_maths() {
        let g = gst(1000.0, 18.0, false, false, 0.0, 1.0).unwrap();
        assert_eq!(g["gst"], 180.0);
        assert_eq!(g["split"]["cgst"], 90.0);
        assert_eq!(g["total"], 1180.0);
        let inc = gst(1180.0, 18.0, true, true, 0.0, 1.0).unwrap();
        assert_eq!(inc["taxable_value"], 1000.0);
        assert_eq!(inc["split"]["igst"], 180.0);
        assert!(gst(0.0, 18.0, false, false, 0.0, 1.0).is_err());
        assert!(gst(100.0, 150.0, false, false, 0.0, 1.0).is_err());
        let mut out = Map::new();
        out.insert("igst".into(), json!(1000.0));
        out.insert("cgst".into(), json!(500.0));
        out.insert("sgst".into(), json!(500.0));
        let mut inp = Map::new();
        inp.insert("igst".into(), json!(1200.0));
        inp.insert("cgst".into(), json!(100.0));
        let i = itc(&out, &inp).unwrap();
        assert_eq!(i["payable"]["igst"], 0.0);
        assert_eq!(i["payable"]["cgst"], 200.0);
        assert_eq!(i["payable"]["sgst"], 500.0);
    }

    #[test]
    fn validators() {
        let v = validate_gstin("27AAPFU0939F1ZV").unwrap();
        assert_eq!(v["valid"], true, "{v}");
        assert_eq!(v["state"], "Maharashtra");
        assert_eq!(v["pan_holder_type"], "Firm / LLP");
        assert_eq!(validate_gstin("27AAPFU0939F1ZX").unwrap()["valid"], false);
        assert_eq!(validate_gstin("short").unwrap()["valid"], false);
        assert!(validate_gstin("").is_err());
        assert_eq!(validate_invoice_number("INV/2024-25/0012").unwrap()["valid"], true);
        assert_eq!(validate_invoice_number("THIS-IS-WAY-TOO-LONG-12345").unwrap()["valid"], false);
        assert_eq!(validate_invoice_number("INV#1").unwrap()["valid"], false);
        let inv = Invoice {
            invoice_number: "INV-1".into(),
            date: "2024-05-01".into(),
            supplier_gstin: "27AAPFU0939F1ZV".into(),
            buyer_gstin: String::new(),
            place_of_supply: "Karnataka".into(),
            taxable_value: 1000.0,
            rate: 18.0,
            igst: 180.0,
            total: 1180.0,
            hsn: "998313".into(),
            ..Default::default()
        };
        let r = validate_invoice(&inv).unwrap();
        assert_eq!(r["valid"], true, "{r}");
        assert_eq!(r["supply_type"], "inter-state");
        let bad = Invoice {
            cgst: 90.0,
            sgst: 90.0,
            igst: 0.0,
            ..inv.clone()
        };
        assert_eq!(validate_invoice(&bad).unwrap()["valid"], false);
        assert_eq!(hsn_lookup("8471").unwrap()["results"][0]["code"], "84");
        assert!(
            hsn_lookup("information technology").unwrap()["count"]
                .as_u64()
                .unwrap()
                > 0
        );
        assert!(hsn_lookup("").is_err());
    }

    #[test]
    fn income_taxes() {
        let t = income_tax(&TaxInputs {
            income: 1_200_000.0,
            is_salaried: true,
            ..Default::default()
        })
        .unwrap();
        // New regime FY24-25: taxable 11.25L → 0 + 20000 + 30000 + 18750 = 68750 + 4% cess
        assert_eq!(t["taxable_income"], 1_125_000.0);
        assert_eq!(t["tax_after_rebate"], 68_750.0);
        assert_eq!(t["total_tax"], 71_500.0);
        let rebate = income_tax(&TaxInputs {
            income: 775_000.0,
            is_salaried: true,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(rebate["total_tax"], 0.0);
        let old = income_tax(&TaxInputs {
            income: 1_200_000.0,
            regime: "old".into(),
            is_salaried: true,
            deductions_80c: 150_000.0,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(old["taxable_income"], 1_000_000.0);
        assert_eq!(old["tax_after_rebate"], 112_500.0);
        let cmp = compare_regimes(&TaxInputs {
            income: 1_200_000.0,
            is_salaried: true,
            deductions_80c: 150_000.0,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(cmp["better"], "new");
        assert!(income_tax(&TaxInputs {
            income: 100.0,
            financial_year: "FY1999-00".into(),
            ..Default::default()
        })
        .is_err());
        let custom = income_tax(&TaxInputs {
            income: 100_000.0,
            slabs: vec![vec![Some(0.0), None, Some(10.0)]],
            rebate_limit: Some(0.0),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(custom["tax_after_rebate"], 10_000.0);
        let td = tds(100_000.0, "194J-professional", None, true, false).unwrap();
        assert_eq!(td["tds"], 10_000.0);
        assert_eq!(
            tds(100_000.0, "194J-professional", None, false, false).unwrap()["rate_percent"],
            20.0
        );
        assert_eq!(
            tds(20_000.0, "194J-professional", None, true, false).unwrap()["applicable"],
            false
        );
        assert!(tds(1.0, "999", None, true, false).is_err());
        let pt = professional_tax(30_000.0, "Maharashtra", Some("male")).unwrap();
        assert_eq!(pt["monthly"], 200.0);
        assert_eq!(pt["yearly"], 2500.0);
        assert_eq!(
            professional_tax(20_000.0, "maharashtra", Some("female")).unwrap()["yearly"],
            0.0
        );
        let cg =
            capital_gains("equity", 100_000.0, 300_000.0, 18, 0.0, 30.0, None, None, None).unwrap();
        assert_eq!(cg["classification"], "long-term");
        assert_eq!(cg["taxable_gain"], 75_000.0);
        assert_eq!(cg["tax"], 9375.0);
        assert_eq!(
            capital_gains("equity", 100.0, 200.0, 3, 0.0, 30.0, None, None, None).unwrap()
                ["rate_percent"],
            20.0
        );
        assert_eq!(
            capital_gains("property", 100.0, 200.0, 3, 0.0, 30.0, None, None, None).unwrap()
                ["rate_basis"],
            "slab rate (supplied)"
        );
        let s = salary_in_hand(&SalaryInputs {
            ctc: 1_200_000.0,
            employer_pf_included_in_ctc: true,
            ..Default::default()
        })
        .unwrap();
        assert!(s["in_hand_monthly"].as_f64().unwrap() > 80_000.0);
    }

    #[test]
    fn statements_and_reconciliation() {
        let stmt = "Date,Narration,Chq/Ref No,Withdrawal Amt,Deposit Amt,Closing \
                    Balance\n01/04/2024,UPI-SWIGGY-123,UPI123,450.00,,10000.00\n02/04/2024,SALARY \
                    APR 2024,NEFT1,,\"50,000.00\",60000.00\n03/04/2024,ATM \
                    WDL,ATM9,2000,,58000.00\n";
        let r = bank_statement(stmt, "all", "HDFC Bank", "Acme").unwrap();
        assert_eq!(r["count"], 3);
        assert_eq!(r["summary"]["total_credit"], 50_000.0);
        assert_eq!(r["transactions"][0]["date"], "2024-04-01");
        assert_eq!(r["transactions"][1]["category"], "Salary");
        assert!(r["csv"].as_str().unwrap().contains("UPI-SWIGGY-123"));
        assert!(r["tally_xml"]
            .as_str()
            .unwrap()
            .contains("<VOUCHER VCHTYPE=\"Receipt\""));
        let ws = bank_statement(
            "01/04/2024 UPI PAYMENT -450.00 9550.00\n02/04/2024 SALARY 50000.00 59550.00",
            "csv",
            "",
            "",
        )
        .unwrap();
        assert_eq!(ws["count"], 2);
        assert!(bank_statement("", "csv", "", "").is_err());
        assert!(bank_statement("no dates here\nat all", "csv", "", "").is_err());
        let books = "gstin,invoice_no,taxable_value,igst,total\n27AAPFU0939F1ZV,INV-001,1000,180,1180\n27AAPFU0939F1ZV,INV-002,2000,360,2360\n29AAAAA0000A1Z5,INV-003,500,90,590\n";
        let portal = "ctin,inum,txval,iamt,val\n27AAPFU0939F1ZV,INV/001,1000,180,1180\n27AAPFU0939F1ZV,INV-002,2100,378,2478\n29AAAAA0000A1Z5,INV-009,700,126,826\n";
        let rec = reconcile(books, portal, 1.0, "purchase").unwrap();
        assert_eq!(rec["summary"]["matched"], 1);
        assert_eq!(rec["summary"]["mismatched"], 1);
        assert_eq!(rec["summary"]["missing_in_portal"], 1);
        assert_eq!(rec["summary"]["missing_in_books"], 1);
        let gstr = validate_gstr(books).unwrap();
        assert_eq!(gstr["valid"], false); // second GSTIN has a bad checksum
        assert!(gstr["errors"].as_u64().unwrap() >= 1);
        assert!(reconcile("", portal, 1.0, "").is_err());
    }
}
