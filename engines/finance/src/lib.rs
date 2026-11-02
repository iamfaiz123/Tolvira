//! # Dev Toolbox Finance Engine (`finance-engine`)
//!
//! Money values are rounded to 2 decimals per period with `rust_decimal`
//! (bankers rounding avoided; half-up) so amortisation schedules sum exactly;
//! rate mathematics uses `f64`. Currency and tax rates are **never**
//! hard-coded: callers pass `rates` / `brackets` (with their own source and
//! date) so results are auditable.
//!
//! | Action | Tool(s) |
//! |---|---|
//! | `loan` | loan-emi-calculator, emi-calculator, loan-calculator, loan-payoff-calculator (extra payments) |
//! | `mortgage` | mortgage-calculator (taxes, insurance, PMI, HOA) |
//! | `interest` | interest-calculator, simple-interest-calculator, compound-interest-calculator |
//! | `investment` | sip-calculator, lumpsum-calculator, mutual-fund-return-calculator, savings-goal-calculator |
//! | `fixed_deposit` / `recurring_deposit` / `ppf` | fd-calculator, rd-calculator, ppf-calculator |
//! | `inflation` | inflation-calculator |
//! | `cagr` / `roi` / `irr` / `npv` | growth & project metrics |
//! | `discount` / `markup` / `margin` / `break_even` | pricing tools |
//! | `salary` / `hourly` / `salary_hike` / `overtime` / `freelance_rate` | pay calculators |
//! | `currency` | currency-converter / currency-calculator (rates supplied by caller) |
//! | `tip` | tip-calculator, split-bill-calculator |
//! | `tax_brackets` | tax-bracket-calculator (progressive brackets supplied by caller) |

use engine_shared::{EngineError, EngineResult};
use rust_decimal::prelude::*;
use serde::Deserialize;
use serde_json::{json, Value};

fn money(v: f64) -> f64 {
    Decimal::from_f64_retain(v)
        .map(|d| {
            d.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
                .to_f64()
                .unwrap_or(v)
        })
        .unwrap_or(v)
}

fn pct(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn positive(v: f64, what: &str) -> EngineResult<f64> {
    engine_shared::error::require_positive(v, what)
}

fn non_negative(v: f64, what: &str) -> EngineResult<f64> {
    if !v.is_finite() || v < 0.0 {
        return Err(EngineError::invalid(format!("{what} must be a non-negative number")));
    }
    Ok(v)
}

fn periods_per_year(frequency: &str) -> EngineResult<f64> {
    Ok(match frequency.to_lowercase().as_str() {
        "" | "monthly" | "month" => 12.0,
        "yearly" | "annual" | "annually" | "year" => 1.0,
        "half-yearly" | "semi-annual" | "semiannual" | "half" => 2.0,
        "quarterly" | "quarter" => 4.0,
        "weekly" | "week" => 52.0,
        "biweekly" | "fortnightly" => 26.0,
        "daily" | "day" => 365.0,
        "continuous" => f64::INFINITY,
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown frequency `{other}` (monthly, yearly, half-yearly, quarterly, weekly, \
                 biweekly, daily, continuous)"
            )))
        },
    })
}

fn has_extra_payments(e: &ExtraPayment) -> bool {
    e.monthly > 0.0 || e.lump_sum > 0.0 || e.yearly > 0.0
}

/// Standard annuity payment.
fn payment(principal: f64, rate_per_period: f64, n: f64) -> f64 {
    if rate_per_period == 0.0 {
        principal / n
    } else {
        principal * rate_per_period * (1.0 + rate_per_period).powf(n)
            / ((1.0 + rate_per_period).powf(n) - 1.0)
    }
}

/// Optional extra payments applied to a loan schedule.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct ExtraPayment {
    /// Extra amount added to every payment.
    pub monthly: f64,
    /// One-off lump sum paid at period `lump_sum_period`.
    pub lump_sum: f64,
    /// 1-based period in which the lump sum is paid.
    pub lump_sum_period: u32,
    /// Extra yearly payment (applied every 12th period).
    pub yearly: f64,
}

/// Amortised loan: payment, totals, payoff date and optional schedule with extra payments.
pub fn loan(
    principal: f64,
    annual_rate: f64,
    years: f64,
    months: u32,
    frequency: &str,
    extra: &ExtraPayment,
    schedule: bool,
    start_period_label: Option<&str>,
) -> EngineResult<Value> {
    positive(principal, "principal")?;
    non_negative(annual_rate, "annual_rate")?;
    let ppy = periods_per_year(frequency)?;
    if !ppy.is_finite() {
        return Err(EngineError::invalid("Continuous compounding is not valid for loan payments"));
    }
    let n = (years * ppy).round() as u32 + if ppy == 12.0 { months } else { 0 };
    if n == 0 || n > 1200 {
        return Err(EngineError::invalid("Loan term must be between 1 and 1200 periods"));
    }
    let r = annual_rate / 100.0 / ppy;
    let pmt = payment(principal, r, n as f64);
    let to_decimal = |v: f64, what: &str| {
        Decimal::from_f64_retain(v).ok_or_else(|| {
            EngineError::invalid(format!("{what} is not representable as a decimal"))
        })
    };
    let pmt_d = to_decimal(money(pmt), "payment")?;
    let mut balance = to_decimal(principal, "principal")?.round_dp(2);
    let mut rows = Vec::new();
    let mut total_interest = Decimal::ZERO;
    let mut total_paid = Decimal::ZERO;
    let mut period = 0u32;
    let r_d = Decimal::from_f64_retain(r).unwrap_or(Decimal::ZERO);
    let extra_m = Decimal::from_f64_retain(money(extra.monthly)).unwrap_or(Decimal::ZERO);
    let extra_y = Decimal::from_f64_retain(money(extra.yearly)).unwrap_or(Decimal::ZERO);
    let lump = Decimal::from_f64_retain(money(extra.lump_sum)).unwrap_or(Decimal::ZERO);
    let mut yearly_rows: Vec<(u32, Decimal, Decimal, Decimal)> = Vec::new();
    while balance > Decimal::ZERO && period < 1200 * 2 {
        period += 1;
        let interest =
            (balance * r_d).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
        let mut pay = pmt_d + extra_m;
        if extra.yearly > 0.0 && period.is_multiple_of(12) {
            pay += extra_y;
        }
        if extra.lump_sum > 0.0 && period == extra.lump_sum_period.max(1) {
            pay += lump;
        }
        // Final scheduled period (or overpayment): settle the remaining balance exactly.
        if pay > balance + interest || (period >= n && !has_extra_payments(extra)) {
            pay = balance + interest;
        }
        let principal_part = pay - interest;
        balance -= principal_part;
        total_interest += interest;
        total_paid += pay;
        let yi = (period - 1) / 12;
        match yearly_rows.last_mut() {
            Some(row) if row.0 == yi => {
                row.1 += principal_part;
                row.2 += interest;
                row.3 = balance;
            },
            _ => yearly_rows.push((yi, principal_part, interest, balance)),
        }
        if schedule {
            rows.push(json!({
                "period": period,
                "payment": pay.to_f64(),
                "principal": principal_part.to_f64(),
                "interest": interest.to_f64(),
                "balance": balance.max(Decimal::ZERO).to_f64(),
                "cumulative_interest": total_interest.to_f64()
            }));
        }
        if balance <= Decimal::ZERO {
            break;
        }
    }
    let baseline_interest = money(pmt * n as f64 - principal);
    let has_extra = extra.monthly > 0.0 || extra.lump_sum > 0.0 || extra.yearly > 0.0;
    Ok(json!({
        "payment": money(pmt),
        "periods": n,
        "periods_per_year": ppy,
        "actual_periods": period,
        "total_paid": total_paid.to_f64(),
        "total_interest": total_interest.to_f64(),
        "interest_percent_of_principal": pct(total_interest.to_f64().unwrap_or(0.0) / principal * 100.0),
        "rate_per_period_percent": pct(r * 100.0 * 10000.0) / 10000.0,
        "baseline": { "total_interest": baseline_interest, "total_paid": money(pmt * n as f64) },
        "savings_from_extra": if has_extra { json!({ "interest_saved": money(baseline_interest - total_interest.to_f64().unwrap_or(0.0)), "periods_saved": n.saturating_sub(period) }) } else { Value::Null },
        "yearly": yearly_rows.iter().map(|(y, p, i, b)| json!({ "year": y + 1, "principal": p.to_f64(), "interest": i.to_f64(), "balance": (*b).max(Decimal::ZERO).to_f64() })).collect::<Vec<_>>(),
        "schedule": if schedule { json!(rows) } else { Value::Null },
        "start_label": start_period_label,
    }))
}

/// Mortgage payment including taxes, insurance, HOA and PMI.
pub fn mortgage(
    price: f64,
    down_payment: f64,
    down_payment_percent: Option<f64>,
    annual_rate: f64,
    years: f64,
    property_tax_yearly: f64,
    insurance_yearly: f64,
    hoa_monthly: f64,
    pmi_rate_percent: f64,
) -> EngineResult<Value> {
    positive(price, "price")?;
    let down = match down_payment_percent {
        Some(p) => price * p / 100.0,
        None => down_payment,
    };
    non_negative(down, "down_payment")?;
    if down >= price {
        return Err(EngineError::invalid("Down payment must be less than the price"));
    }
    let principal = price - down;
    let base = loan(
        principal,
        annual_rate,
        years,
        0,
        "monthly",
        &ExtraPayment::default(),
        false,
        None,
    )?;
    let pi = base["payment"].as_f64().unwrap_or(0.0);
    let ltv = principal / price * 100.0;
    let pmi = if ltv > 80.0 && pmi_rate_percent > 0.0 {
        principal * pmi_rate_percent / 100.0 / 12.0
    } else {
        0.0
    };
    let tax = property_tax_yearly / 12.0;
    let ins = insurance_yearly / 12.0;
    let total_monthly = pi + pmi + tax + ins + hoa_monthly;
    Ok(json!({
        "loan_amount": money(principal),
        "down_payment": money(down),
        "down_payment_percent": pct(down / price * 100.0),
        "loan_to_value_percent": pct(ltv),
        "principal_and_interest": money(pi),
        "property_tax": money(tax),
        "insurance": money(ins),
        "pmi": money(pmi),
        "hoa": money(hoa_monthly),
        "total_monthly": money(total_monthly),
        "total_interest": base["total_interest"],
        "total_cost": money(base["total_paid"].as_f64().unwrap_or(0.0) + down + (tax + ins + hoa_monthly) * years * 12.0),
        "yearly": base["yearly"],
        "pmi_note": if ltv > 80.0 { "PMI applies while loan-to-value exceeds 80%" } else { "No PMI (LTV ≤ 80%)" },
    }))
}

/// Simple/compound interest with optional periodic contributions.
pub fn interest(
    principal: f64,
    annual_rate: f64,
    years: f64,
    compounding: &str,
    contribution: f64,
    contribution_frequency: &str,
    contribute_at_start: bool,
) -> EngineResult<Value> {
    positive(principal.max(if contribution > 0.0 { 1e-9 } else { 0.0 }), "principal")?;
    non_negative(annual_rate, "annual_rate")?;
    positive(years, "years")?;
    let simple_interest = principal * annual_rate / 100.0 * years;
    let m = periods_per_year(compounding)?;
    let rate = annual_rate / 100.0;
    let compound_amount = if m.is_infinite() {
        principal * (rate * years).exp()
    } else {
        principal * (1.0 + rate / m).powf(m * years)
    };
    let cf = periods_per_year(contribution_frequency)?;

    let (total_contrib, contrib_value) = if contribution > 0.0 && cf.is_finite() {
        let periods = (years * cf).round();
        let r = if m.is_infinite() {
            (rate / cf).exp() - 1.0
        } else {
            (1.0 + rate / m).powf(m / cf) - 1.0
        };
        let fv = if r == 0.0 {
            contribution * periods
        } else {
            contribution
                * (((1.0 + r).powf(periods) - 1.0) / r)
                * if contribute_at_start { 1.0 + r } else { 1.0 }
        };
        (contribution * periods, fv)
    } else {
        (0.0, 0.0)
    };
    let final_amount = compound_amount + contrib_value;
    let mut yearly = Vec::new();
    for y in 1..=(years.ceil() as u32).min(100) {
        let t = (y as f64).min(years);
        let base = if m.is_infinite() {
            principal * (rate * t).exp()
        } else {
            principal * (1.0 + rate / m).powf(m * t)
        };
        let c = if contribution > 0.0 && cf.is_finite() {
            let p = (t * cf).round();
            let r = if m.is_infinite() {
                (rate / cf).exp() - 1.0
            } else {
                (1.0 + rate / m).powf(m / cf) - 1.0
            };
            if r == 0.0 {
                contribution * p
            } else {
                contribution
                    * (((1.0 + r).powf(p) - 1.0) / r)
                    * if contribute_at_start { 1.0 + r } else { 1.0 }
            }
        } else {
            0.0
        };
        yearly.push(json!({
            "year": y,
            "balance": money(base + c),
            "contributions": money(principal + contribution * (t * cf).round()),
            "interest": money(base + c - principal - contribution * (t * cf).round())
        }));
    }
    let apy = if m.is_infinite() {
        (rate.exp() - 1.0) * 100.0
    } else {
        ((1.0 + rate / m).powf(m) - 1.0) * 100.0
    };
    Ok(json!({
        "simple": {
            "interest": money(simple_interest),
            "total": money(principal + simple_interest)
        },
        "compound": {
            "interest": money(final_amount - principal - total_contrib),
            "total": money(final_amount),
            "compounding": if compounding.is_empty() { "monthly" } else { compounding },
            "effective_annual_rate_percent": pct(apy)
        },
        "contributions_total": money(total_contrib),
        "yearly": yearly,
        "doubling_time_years": if rate > 0.0 { json!((2f64.ln() / (1.0 + rate / m.min(1e9)).ln() / m.min(1e9) * 100.0).round() / 100.0) } else { Value::Null },
        "rule_of_72_years": if annual_rate > 0.0 { json!(pct(72.0 / annual_rate)) } else { Value::Null },
    }))
}

/// SIP / lumpsum / goal-seeking investment projections.
pub fn investment(
    monthly: f64,
    lumpsum: f64,
    annual_return: f64,
    years: f64,
    step_up_percent: f64,
    target: Option<f64>,
    inflation: f64,
) -> EngineResult<Value> {
    non_negative(monthly, "monthly")?;
    non_negative(lumpsum, "lumpsum")?;
    if monthly == 0.0 && lumpsum == 0.0 && target.is_none() {
        return Err(EngineError::invalid("Provide `monthly`, `lumpsum` or `target`"));
    }
    positive(years, "years")?;
    let r = annual_return / 100.0 / 12.0;
    let n = (years * 12.0).round() as u32;
    let mut invested = lumpsum;
    let mut value = lumpsum;
    let mut sip = monthly;
    let mut yearly = Vec::new();
    for month in 1..=n {
        value = value * (1.0 + r) + sip;
        invested += sip;
        if month % 12 == 0 {
            yearly.push(json!({
                "year": month / 12,
                "invested": money(invested),
                "value": money(value),
                "gain": money(value - invested),
                "monthly_sip": money(sip)
            }));
            sip *= 1.0 + step_up_percent / 100.0;
        }
    }
    let lump_only = lumpsum * (1.0 + r).powf(n as f64);
    let required_sip = target.map(|t| {
        let remaining = (t - lump_only).max(0.0);
        if r == 0.0 {
            remaining / n as f64
        } else {
            remaining * r / ((1.0 + r).powf(n as f64) - 1.0) / (1.0 + r)
        }
    });
    let real_value = value / (1.0 + inflation / 100.0).powf(years);
    let cagr = if invested > 0.0 && years > 0.0 {
        ((value / invested).powf(1.0 / years) - 1.0) * 100.0
    } else {
        0.0
    };
    Ok(json!({
        "invested": money(invested),
        "maturity_value": money(value),
        "estimated_gain": money(value - invested),
        "gain_percent": if invested > 0.0 { pct((value - invested) / invested * 100.0) } else { 0.0 },
        "lumpsum_component": money(lump_only),
        "inflation_adjusted_value": money(real_value),
        "effective_cagr_percent": pct(cagr),
        "required_monthly_for_target": required_sip.map(money),
        "yearly": yearly,
    }))
}

/// Fixed deposit maturity (cumulative or payout).
pub fn fixed_deposit(
    principal: f64,
    annual_rate: f64,
    years: f64,
    months: u32,
    compounding: &str,
    payout: bool,
) -> EngineResult<Value> {
    positive(principal, "principal")?;
    non_negative(annual_rate, "annual_rate")?;
    let t = years + months as f64 / 12.0;
    positive(t, "tenure")?;
    let m = periods_per_year(if compounding.is_empty() {
        "quarterly"
    } else {
        compounding
    })?;
    let maturity = if payout {
        principal
    } else {
        principal * (1.0 + annual_rate / 100.0 / m).powf(m * t)
    };
    let interest = if payout {
        principal * annual_rate / 100.0 * t
    } else {
        maturity - principal
    };
    let effective = if m.is_infinite() {
        ((annual_rate / 100.0).exp() - 1.0) * 100.0
    } else {
        ((1.0 + annual_rate / 100.0 / m).powf(m) - 1.0) * 100.0
    };
    Ok(json!({
        "maturity_amount": money(maturity),
        "interest_earned": money(interest),
        "tenure_years": t,
        "effective_annual_yield_percent": pct(effective),
        "monthly_payout": if payout { json!(money(principal * annual_rate / 100.0 / 12.0)) } else { Value::Null },
        "compounding": if compounding.is_empty() { "quarterly" } else { compounding }
    }))
}

/// Recurring deposit maturity (quarterly compounding).
pub fn recurring_deposit(monthly: f64, annual_rate: f64, months: u32) -> EngineResult<Value> {
    positive(monthly, "monthly")?;
    non_negative(annual_rate, "annual_rate")?;
    if months == 0 || months > 600 {
        return Err(EngineError::invalid("months must be between 1 and 600"));
    }
    // Indian bank convention: quarterly compounding on each instalment.
    let r = annual_rate / 100.0 / 4.0;
    let mut maturity = 0.0;
    for i in 0..months {
        let remaining_months = (months - i) as f64;
        maturity += monthly * (1.0 + r).powf(remaining_months / 3.0);
    }
    let invested = monthly * months as f64;
    Ok(json!({
        "maturity_amount": money(maturity),
        "total_deposited": money(invested),
        "interest_earned": money(maturity - invested),
        "months": months,
        "note": "Quarterly compounding per instalment (Indian bank convention)"
    }))
}

/// Public Provident Fund projection with optional extension years.
pub fn ppf(
    yearly_deposit: f64,
    annual_rate: f64,
    years: u32,
    extension_years: u32,
) -> EngineResult<Value> {
    positive(yearly_deposit, "yearly_deposit")?;
    non_negative(annual_rate, "annual_rate")?;
    let total_years = years.max(1) + extension_years;
    if total_years > 60 {
        return Err(EngineError::invalid("Total tenure must be at most 60 years"));
    }
    let mut balance = 0.0;
    let mut invested = 0.0;
    let mut rows = Vec::new();
    for y in 1..=total_years {
        let deposit = if y <= years { yearly_deposit } else { 0.0 };
        balance += deposit;
        invested += deposit;
        let interest = balance * annual_rate / 100.0;
        balance += interest;
        rows.push(json!({
            "year": y,
            "deposit": money(deposit),
            "interest": money(interest),
            "balance": money(balance)
        }));
    }
    Ok(json!({
        "maturity_amount": money(balance),
        "total_invested": money(invested),
        "interest_earned": money(balance - invested),
        "years": total_years,
        "yearly": rows,
        "note": "Annual compounding with deposits made at the start of each year; rate must be \
                 supplied (statutory rates change quarterly)"
    }))
}

/// Inflation-adjusted value forwards or backwards in time.
pub fn inflation(
    amount: f64,
    annual_rate: f64,
    years: f64,
    direction: &str,
) -> EngineResult<Value> {
    positive(amount, "amount")?;
    positive(years, "years")?;
    if !annual_rate.is_finite() || annual_rate <= -100.0 {
        return Err(EngineError::invalid("annual_rate must be > -100"));
    }
    let factor = (1.0 + annual_rate / 100.0).powf(years);
    let (future, past) = (amount * factor, amount / factor);
    let yearly: Vec<Value> = (1..=(years.ceil() as u32).min(100))
        .map(|y| {
            json!({
                "year": y,
                "equivalent": money(amount * (1.0 + annual_rate / 100.0).powf(y as f64)),
                "purchasing_power": money(amount / (1.0 + annual_rate / 100.0).powf(y as f64))
            })
        })
        .collect();
    Ok(json!({
        "result": money(if direction == "backward" || direction == "past" { past } else { future }),
        "future_cost_of_today_amount": money(future),
        "purchasing_power_of_amount_in_future": money(past),
        "cumulative_inflation_percent": pct((factor - 1.0) * 100.0),
        "purchasing_power_lost_percent": pct((1.0 - 1.0 / factor) * 100.0),
        "yearly": yearly,
    }))
}

/// Compound annual growth rate.
pub fn cagr(start: f64, end: f64, years: f64) -> EngineResult<Value> {
    positive(start, "start_value")?;
    positive(end, "end_value")?;
    positive(years, "years")?;
    let c = ((end / start).powf(1.0 / years) - 1.0) * 100.0;
    Ok(json!({
        "cagr_percent": pct(c),
        "total_growth_percent": pct((end / start - 1.0) * 100.0),
        "absolute_gain": money(end - start),
        "multiple": (end / start * 1000.0).round() / 1000.0,
        "doubling_years": if c > 0.0 { json!(pct(2f64.ln() / (1.0 + c / 100.0).ln())) } else { Value::Null }
    }))
}

/// Return on investment (total and annualised).
pub fn roi(cost: f64, gain: f64, years: Option<f64>, extra_costs: f64) -> EngineResult<Value> {
    positive(cost, "cost")?;
    let total_cost = cost + extra_costs;
    let net = gain - total_cost;
    let r = net / total_cost * 100.0;
    let annualized = years
        .filter(|y| *y > 0.0)
        .map(|y| ((gain / total_cost).max(0.0).powf(1.0 / y) - 1.0) * 100.0);
    Ok(json!({
        "roi_percent": pct(r),
        "net_profit": money(net),
        "total_cost": money(total_cost),
        "annualized_roi_percent": annualized.map(pct),
        "payback_multiple": (gain / total_cost * 1000.0).round() / 1000.0
    }))
}

/// Net present value of a series of cash flows.
pub fn npv(rate_percent: f64, cash_flows: &[f64]) -> EngineResult<Value> {
    if cash_flows.is_empty() {
        return Err(EngineError::empty("cash_flows"));
    }
    let r = rate_percent / 100.0;
    if r <= -1.0 {
        return Err(EngineError::invalid("rate must be > -100%"));
    }
    let mut total = 0.0;
    let rows: Vec<Value> = cash_flows
        .iter()
        .enumerate()
        .map(|(t, cf)| {
            let pv = cf / (1.0 + r).powi(t as i32);
            total += pv;
            json!({
                "period": t,
                "cash_flow": cf,
                "discount_factor": (1.0 / (1.0 + r).powi(t as i32) * 10000.0).round() / 10000.0,
                "present_value": money(pv),
                "cumulative": money(total)
            })
        })
        .collect();
    let pi = if cash_flows[0] < 0.0 {
        Some((total - cash_flows[0]) / -cash_flows[0])
    } else {
        None
    };
    Ok(json!({
        "npv": money(total),
        "rate_percent": rate_percent,
        "rows": rows,
        "profitability_index": pi.map(|p| (p * 1000.0).round() / 1000.0),
        "decision": if total > 0.0 { "accept (NPV > 0)" } else if total < 0.0 { "reject (NPV < 0)" } else { "indifferent" }
    }))
}

/// Internal rate of return (Newton with bisection fallback).
pub fn irr(cash_flows: &[f64], guess: f64) -> EngineResult<Value> {
    if cash_flows.len() < 2 {
        return Err(EngineError::invalid(
            "Provide at least two cash flows (initial outlay then inflows)",
        ));
    }
    if !(cash_flows.iter().any(|c| *c < 0.0) && cash_flows.iter().any(|c| *c > 0.0)) {
        return Err(EngineError::invalid(
            "Cash flows need at least one negative and one positive value",
        ));
    }
    let f = |r: f64| {
        cash_flows
            .iter()
            .enumerate()
            .map(|(t, cf)| cf / (1.0 + r).powi(t as i32))
            .sum::<f64>()
    };
    let df = |r: f64| {
        cash_flows
            .iter()
            .enumerate()
            .map(|(t, cf)| -(t as f64) * cf / (1.0 + r).powi(t as i32 + 1))
            .sum::<f64>()
    };
    let mut r = guess / 100.0;
    let mut converged = false;
    for _ in 0..200 {
        let v = f(r);
        let d = df(r);
        if d.abs() < 1e-14 {
            break;
        }
        let next = r - v / d;
        if !next.is_finite() || next <= -0.9999 {
            break;
        }
        if (next - r).abs() < 1e-10 {
            r = next;
            converged = true;
            break;
        }
        r = next;
    }
    if !converged {
        // Bisection fallback on [-0.99, 10].
        let (mut lo, mut hi) = (-0.99, 10.0);
        if f(lo).signum() == f(hi).signum() {
            return Err(EngineError::not_found(
                "IRR could not be found (no sign change between -99% and 1000%)",
            ));
        }
        for _ in 0..300 {
            let mid = (lo + hi) / 2.0;
            if f(mid).signum() == f(lo).signum() {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        r = (lo + hi) / 2.0;
    }
    let mut cumulative = 0.0;
    let mut payback: Option<f64> = None;
    for (t, cf) in cash_flows.iter().enumerate() {
        let prev = cumulative;
        cumulative += cf;
        if payback.is_none() && cumulative >= 0.0 && t > 0 && *cf > 0.0 {
            payback = Some((t - 1) as f64 + (-prev / cf));
        }
    }
    Ok(json!({
        "irr_percent": pct(r * 100.0),
        "npv_at_irr": money(f(r)),
        "payback_period": payback.map(|p| (p * 100.0).round() / 100.0),
        "total_inflows": money(cash_flows.iter().filter(|c| **c > 0.0).sum()),
        "total_outflows": money(cash_flows.iter().filter(|c| **c < 0.0).sum::<f64>().abs())
    }))
}

/// Discount calculator (percent, amount or final price) with tax and quantity.
pub fn discount(
    price: f64,
    discount_percent: Option<f64>,
    discount_amount: Option<f64>,
    final_price: Option<f64>,
    tax_percent: f64,
    quantity: f64,
) -> EngineResult<Value> {
    positive(price, "price")?;
    let q = if quantity <= 0.0 { 1.0 } else { quantity };
    let (pct_off, amount_off) = match (discount_percent, discount_amount, final_price) {
        (Some(p), _, _) => (p, price * p / 100.0),
        (_, Some(a), _) => (a / price * 100.0, a),
        (_, _, Some(f)) => ((price - f) / price * 100.0, price - f),
        _ => {
            return Err(EngineError::invalid(
                "Provide discount_percent, discount_amount or final_price",
            ))
        },
    };
    if !(0.0..=100.0).contains(&pct_off) {
        return Err(EngineError::invalid("Discount must be between 0% and 100%"));
    }
    let discounted = price - amount_off;
    let tax = discounted * tax_percent / 100.0;
    Ok(json!({
        "original_price": money(price),
        "discount_percent": pct(pct_off),
        "discount_amount": money(amount_off),
        "discounted_price": money(discounted),
        "tax": money(tax),
        "final_price": money(discounted + tax),
        "quantity": q,
        "total_for_quantity": money((discounted + tax) * q),
        "total_saved": money(amount_off * q)
    }))
}

/// Markup/margin calculator from any two of cost, price and markup/margin percent.
pub fn markup(
    cost: f64,
    markup_percent: Option<f64>,
    price: Option<f64>,
    margin_percent: Option<f64>,
) -> EngineResult<Value> {
    positive(cost, "cost")?;
    let sell = match (markup_percent, price, margin_percent) {
        (Some(m), _, _) => cost * (1.0 + m / 100.0),
        (_, Some(p), _) => p,
        (_, _, Some(g)) => {
            if g >= 100.0 {
                return Err(EngineError::invalid("Margin must be below 100%"));
            }
            cost / (1.0 - g / 100.0)
        },
        _ => return Err(EngineError::invalid("Provide markup_percent, price or margin_percent")),
    };
    let profit = sell - cost;
    Ok(json!({
        "cost": money(cost),
        "price": money(sell),
        "profit": money(profit),
        "markup_percent": pct(profit / cost * 100.0),
        "margin_percent": if sell != 0.0 { pct(profit / sell * 100.0) } else { 0.0 },
        "formulas": { "markup": "(price − cost) / cost", "margin": "(price − cost) / price" }
    }))
}

/// Gross/net margin from revenue, cost and expenses.
pub fn margin(revenue: f64, cost: f64, expenses: f64) -> EngineResult<Value> {
    positive(revenue, "revenue")?;
    non_negative(cost, "cost")?;
    let gross = revenue - cost;
    let net = gross - expenses;
    Ok(json!({
        "gross_profit": money(gross),
        "gross_margin_percent": pct(gross / revenue * 100.0),
        "net_profit": money(net),
        "net_margin_percent": pct(net / revenue * 100.0),
        "markup_percent": if cost > 0.0 { json!(pct(gross / cost * 100.0)) } else { Value::Null },
        "cost_ratio_percent": pct(cost / revenue * 100.0)
    }))
}

/// Break-even units and revenue.
pub fn break_even(
    fixed_costs: f64,
    price_per_unit: f64,
    variable_cost_per_unit: f64,
    target_profit: f64,
) -> EngineResult<Value> {
    non_negative(fixed_costs, "fixed_costs")?;
    positive(price_per_unit, "price_per_unit")?;
    non_negative(variable_cost_per_unit, "variable_cost_per_unit")?;
    let contribution = price_per_unit - variable_cost_per_unit;
    if contribution <= 0.0 {
        return Err(EngineError::invalid("Price must exceed the variable cost per unit"));
    }
    let units = fixed_costs / contribution;
    let target_units = (fixed_costs + target_profit) / contribution;
    let chart: Vec<Value> = (0..=10)
        .map(|i| {
            let u = (units * 2.0 / 10.0 * i as f64).round();
            json!({
                "units": u,
                "revenue": money(u * price_per_unit),
                "total_cost": money(fixed_costs + u * variable_cost_per_unit),
                "profit": money(u * contribution - fixed_costs)
            })
        })
        .collect();
    Ok(json!({
        "break_even_units": units.ceil(),
        "break_even_units_exact": (units * 100.0).round() / 100.0,
        "break_even_revenue": money(units * price_per_unit),
        "contribution_margin_per_unit": money(contribution),
        "contribution_margin_ratio_percent": pct(contribution / price_per_unit * 100.0),
        "units_for_target_profit": target_units.ceil(),
        "chart": chart
    }))
}

/// A salary deduction (percentage and/or fixed amount).
#[derive(Deserialize, Debug, Clone)]
pub struct Deduction {
    /// Label shown in the breakdown.
    pub name: String,
    /// Percentage of gross.
    #[serde(default)]
    pub percent: f64,
    /// Fixed amount per `period`.
    #[serde(default)]
    pub amount: f64,
    /// `monthly` (default) or `yearly` for the fixed `amount`.
    #[serde(default)]
    pub period: String,
}

/// Salary breakdown across periods with deductions.
pub fn salary(
    gross: f64,
    period: &str,
    deductions: &[Deduction],
    hours_per_week: f64,
    weeks_per_year: f64,
) -> EngineResult<Value> {
    positive(gross, "gross")?;
    let hpw = if hours_per_week <= 0.0 { 40.0 } else { hours_per_week };
    let wpy = if weeks_per_year <= 0.0 { 52.0 } else { weeks_per_year };
    let yearly = match period {
        "" | "yearly" | "annual" => gross,
        "monthly" => gross * 12.0,
        "weekly" => gross * wpy,
        "biweekly" => gross * wpy / 2.0,
        "daily" => gross * wpy * 5.0,
        "hourly" => gross * hpw * wpy,
        other => return Err(EngineError::invalid(format!("Unknown period `{other}`"))),
    };
    let mut total_ded = 0.0;
    let rows: Vec<Value> = deductions
        .iter()
        .map(|d| {
            let fixed = if d.period == "yearly" {
                d.amount
            } else {
                d.amount * 12.0
            };
            let amt = yearly * d.percent / 100.0 + fixed;
            total_ded += amt;
            json!({ "name": d.name, "yearly": money(amt), "monthly": money(amt / 12.0) })
        })
        .collect();
    let net = yearly - total_ded;
    let breakdown = |v: f64| {
        json!({
            "yearly": money(v),
            "monthly": money(v / 12.0),
            "biweekly": money(v / (wpy / 2.0)),
            "weekly": money(v / wpy),
            "daily": money(v / (wpy * 5.0)),
            "hourly": money(v / (wpy * hpw))
        })
    };
    Ok(json!({
        "gross": breakdown(yearly),
        "net": breakdown(net),
        "deductions": rows,
        "total_deductions_yearly": money(total_ded),
        "effective_deduction_rate_percent": pct(total_ded / yearly * 100.0),
        "assumptions": { "hours_per_week": hpw, "weeks_per_year": wpy }
    }))
}

/// Salary hike: new salary or percentage from the other.
pub fn salary_hike(
    current: f64,
    new_salary: Option<f64>,
    hike_percent: Option<f64>,
) -> EngineResult<Value> {
    positive(current, "current")?;
    let (new, p) = match (new_salary, hike_percent) {
        (Some(n), _) => (n, (n - current) / current * 100.0),
        (_, Some(h)) => (current * (1.0 + h / 100.0), h),
        _ => return Err(EngineError::invalid("Provide new_salary or hike_percent")),
    };
    Ok(json!({
        "current": money(current),
        "new": money(new),
        "hike_percent": pct(p),
        "increase": money(new - current),
        "monthly_increase": money((new - current) / 12.0),
        "years_to_double_at_this_rate": if p > 0.0 { json!(pct(2f64.ln() / (1.0 + p / 100.0).ln())) } else { Value::Null }
    }))
}

/// Overtime pay with configurable multipliers.
pub fn overtime(
    hourly_rate: f64,
    regular_hours: f64,
    overtime_hours: f64,
    multiplier: f64,
    double_hours: f64,
) -> EngineResult<Value> {
    positive(hourly_rate, "hourly_rate")?;
    non_negative(regular_hours, "regular_hours")?;
    non_negative(overtime_hours, "overtime_hours")?;
    let m = if multiplier <= 0.0 { 1.5 } else { multiplier };
    let regular = hourly_rate * regular_hours;
    let ot = hourly_rate * m * overtime_hours;
    let dbl = hourly_rate * 2.0 * double_hours.max(0.0);
    Ok(json!({
        "regular_pay": money(regular),
        "overtime_pay": money(ot),
        "double_time_pay": money(dbl),
        "total_pay": money(regular + ot + dbl),
        "overtime_rate": money(hourly_rate * m),
        "total_hours": regular_hours + overtime_hours + double_hours.max(0.0),
        "effective_hourly": money((regular + ot + dbl) / (regular_hours + overtime_hours + double_hours.max(0.0)).max(1.0))
    }))
}

/// Hourly rate a freelancer must charge to reach a target income.
pub fn freelance_rate(
    target_income: f64,
    billable_hours_per_week: f64,
    weeks_off: f64,
    yearly_expenses: f64,
    tax_percent: f64,
    profit_margin_percent: f64,
) -> EngineResult<Value> {
    positive(target_income, "target_income")?;
    positive(billable_hours_per_week, "billable_hours_per_week")?;
    let weeks = (52.0 - weeks_off).max(1.0);
    let hours = billable_hours_per_week * weeks;
    let gross_needed = (target_income + yearly_expenses)
        / (1.0 - tax_percent.clamp(0.0, 99.0) / 100.0)
        * (1.0 + profit_margin_percent.max(0.0) / 100.0);
    let rate = gross_needed / hours;
    Ok(json!({
        "hourly_rate": money(rate),
        "daily_rate_8h": money(rate * 8.0),
        "weekly_rate": money(rate * billable_hours_per_week),
        "monthly_revenue": money(gross_needed / 12.0),
        "gross_revenue_needed": money(gross_needed),
        "billable_hours_per_year": hours,
        "working_weeks": weeks
    }))
}

/// Converts pay between hourly/daily/weekly/monthly/yearly.
pub fn hourly(
    amount: f64,
    from: &str,
    hours_per_week: f64,
    weeks_per_year: f64,
) -> EngineResult<Value> {
    positive(amount, "amount")?;
    let hpw = if hours_per_week <= 0.0 { 40.0 } else { hours_per_week };
    let wpy = if weeks_per_year <= 0.0 { 52.0 } else { weeks_per_year };
    let yearly = match from {
        "hourly" | "" => amount * hpw * wpy,
        "daily" => amount * 5.0 * wpy,
        "weekly" => amount * wpy,
        "monthly" => amount * 12.0,
        "yearly" | "annual" => amount,
        other => return Err(EngineError::invalid(format!("Unknown period `{other}`"))),
    };
    Ok(json!({
        "hourly": money(yearly / (hpw * wpy)),
        "daily": money(yearly / (5.0 * wpy)),
        "weekly": money(yearly / wpy),
        "biweekly": money(yearly / wpy * 2.0),
        "monthly": money(yearly / 12.0),
        "yearly": money(yearly),
        "assumptions": { "hours_per_week": hpw, "weeks_per_year": wpy }
    }))
}

/// Currency conversion using caller-supplied rates (the engine has no network).
pub fn currency(
    amount: f64,
    from: &str,
    to: &str,
    rates: &serde_json::Map<String, Value>,
    base: &str,
    rates_date: Option<&str>,
) -> EngineResult<Value> {
    non_negative(amount, "amount")?;
    if rates.is_empty() {
        return Err(EngineError::invalid(
            "Provide `rates`: { \"USD\": 1, \"EUR\": 0.92, ... } relative to `base` (the engine \
             ships no exchange rates)",
        ));
    }
    let b = if base.is_empty() { "USD" } else { base };
    let rate_of = |code: &str| -> EngineResult<f64> {
        let c = code.trim().to_uppercase();
        if c == b.to_uppercase() {
            return Ok(1.0);
        }
        rates
            .iter()
            .find(|(k, _)| k.to_uppercase() == c)
            .and_then(|(_, v)| v.as_f64())
            .filter(|v| *v > 0.0)
            .ok_or_else(|| {
                EngineError::not_found(format!("No rate for `{code}` in the supplied table"))
            })
    };
    let (rf, rt) = (rate_of(from)?, rate_of(to)?);
    let result = amount / rf * rt;
    Ok(json!({
        "amount": amount,
        "from": from.to_uppercase(),
        "to": to.to_uppercase(),
        "result": money(result),
        "rate": (rt / rf * 1_000_000.0).round() / 1_000_000.0,
        "inverse_rate": (rf / rt * 1_000_000.0).round() / 1_000_000.0,
        "base": b,
        "rates_date": rates_date,
        "note": "Rates are caller supplied; verify their source and timestamp"
    }))
}

/// Tip calculator with split and rounding.
pub fn tip(
    bill: f64,
    tip_percent: f64,
    people: u32,
    round_up: bool,
    tax_percent: f64,
    tip_on_pre_tax: bool,
) -> EngineResult<Value> {
    positive(bill, "bill")?;
    non_negative(tip_percent, "tip_percent")?;
    let n = people.max(1) as f64;
    let tax = bill * tax_percent / 100.0;
    let tip_base = if tip_on_pre_tax { bill } else { bill + tax };
    let mut tip_amount = tip_base * tip_percent / 100.0;
    let mut total = bill + tax + tip_amount;
    if round_up {
        let rounded = total.ceil();
        tip_amount += rounded - total;
        total = rounded;
    }
    let suggestions: Vec<Value> = [10.0, 12.5, 15.0, 18.0, 20.0, 25.0]
        .iter()
        .map(|p| {
            json!({
                "percent": p,
                "tip": money(tip_base * p / 100.0),
                "total": money(bill + tax + tip_base * p / 100.0)
            })
        })
        .collect();
    Ok(json!({
        "bill": money(bill),
        "tax": money(tax),
        "tip": money(tip_amount),
        "total": money(total),
        "per_person": money(total / n),
        "tip_per_person": money(tip_amount / n),
        "people": people.max(1),
        "effective_tip_percent": pct(tip_amount / bill * 100.0),
        "suggestions": suggestions
    }))
}

/// One progressive tax bracket.
#[derive(Deserialize, Debug, Clone)]
pub struct Bracket {
    /// Lower bound of the bracket (inclusive).
    pub from: f64,
    /// Upper bound (exclusive); `null` for the top bracket.
    #[serde(default)]
    pub to: Option<f64>,
    /// Marginal rate in percent.
    pub rate_percent: f64,
}

/// Progressive tax with surcharge, cess and credits.
pub fn tax_brackets(
    income: f64,
    brackets: &[Bracket],
    deductions: f64,
    surcharge_percent: f64,
    cess_percent: f64,
    credits: f64,
) -> EngineResult<Value> {
    non_negative(income, "income")?;
    if brackets.is_empty() {
        return Err(EngineError::invalid(
            "Provide `brackets` [{from, to, rate_percent}] for your jurisdiction and year (no \
             rates are built in)",
        ));
    }
    let taxable = (income - deductions.max(0.0)).max(0.0);
    let mut sorted = brackets.to_vec();
    sorted.sort_by(|a, b| a.from.total_cmp(&b.from));
    let mut tax = 0.0;
    let mut rows = Vec::new();
    let mut marginal = 0.0;
    for b in &sorted {
        let upper = b.to.unwrap_or(f64::INFINITY);
        if taxable <= b.from {
            break;
        }
        let amount_in = taxable.min(upper) - b.from;
        let t = amount_in * b.rate_percent / 100.0;
        tax += t;
        marginal = b.rate_percent;
        rows.push(json!({
            "from": b.from,
            "to": b.to,
            "rate_percent": b.rate_percent,
            "taxable_in_bracket": money(amount_in),
            "tax": money(t)
        }));
    }
    let surcharge = tax * surcharge_percent.max(0.0) / 100.0;
    let cess = (tax + surcharge) * cess_percent.max(0.0) / 100.0;
    let total = (tax + surcharge + cess - credits.max(0.0)).max(0.0);
    Ok(json!({
        "gross_income": money(income),
        "taxable_income": money(taxable),
        "base_tax": money(tax),
        "surcharge": money(surcharge),
        "cess": money(cess),
        "credits": money(credits),
        "total_tax": money(total),
        "net_income": money(income - total),
        "effective_rate_percent": if income > 0.0 { pct(total / income * 100.0) } else { 0.0 },
        "marginal_rate_percent": marginal,
        "brackets": rows,
        "monthly_tax": money(total / 12.0)
    }))
}

fn default_monthly() -> String {
    "monthly".into()
}
fn default_15() -> f64 {
    15.0
}
fn default_one() -> u32 {
    1
}
fn default_true() -> bool {
    true
}
fn default_guess() -> f64 {
    10.0
}
fn default_15y() -> u32 {
    15
}
fn default_1_5() -> f64 {
    1.5
}

/// Every action understood by the finance engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum FinanceAction {
    /// Loan / EMI calculator.
    Loan {
        /// Amount borrowed.
        principal: f64,
        /// Nominal annual interest rate in percent.
        annual_rate: f64,
        /// Term in years (alternative to `months`).
        #[serde(default)]
        years: f64,
        /// Term in months (alternative to `years`).
        #[serde(default)]
        months: u32,
        /// Payment frequency (`monthly` default, `biweekly`, `weekly`, `quarterly`, `yearly`).
        #[serde(default = "default_monthly")]
        frequency: String,
        /// Extra payments.
        #[serde(default)]
        extra: ExtraPayment,
        /// Include the full amortisation schedule.
        #[serde(default)]
        schedule: bool,
    },
    /// Mortgage calculator.
    Mortgage {
        /// Property price.
        price: f64,
        /// Down payment amount.
        #[serde(default)]
        down_payment: f64,
        /// Down payment as a percentage of price (alternative).
        #[serde(default)]
        down_payment_percent: Option<f64>,
        /// Nominal annual interest rate in percent.
        annual_rate: f64,
        /// Term in years.
        years: f64,
        /// Yearly property tax.
        #[serde(default)]
        property_tax_yearly: f64,
        /// Yearly home insurance.
        #[serde(default)]
        insurance_yearly: f64,
        /// Monthly HOA fee.
        #[serde(default)]
        hoa_monthly: f64,
        /// PMI rate in percent of the loan (applied when down payment < 20 %).
        #[serde(default)]
        pmi_rate_percent: f64,
    },
    /// Interest / compound growth calculator.
    Interest {
        /// Starting amount.
        #[serde(default)]
        principal: f64,
        /// Annual rate in percent.
        annual_rate: f64,
        /// Duration in years.
        years: f64,
        /// `simple` or a compounding frequency (`monthly`, `yearly`, `half-yearly`, `quarterly`,
        /// `weekly`, `biweekly`, `daily`, `continuous`).
        #[serde(default)]
        compounding: String,
        /// Periodic contribution.
        #[serde(default)]
        contribution: f64,
        /// Contribution frequency (`monthly` default).
        #[serde(default = "default_monthly")]
        contribution_frequency: String,
        /// Contribute at the start of each period.
        #[serde(default)]
        contribute_at_start: bool,
    },
    /// SIP / lump-sum investment projection.
    Investment {
        /// Monthly investment.
        #[serde(default)]
        monthly: f64,
        /// One-off initial investment.
        #[serde(default)]
        lumpsum: f64,
        /// Expected annual return in percent.
        annual_return: f64,
        /// Duration in years.
        years: f64,
        /// Yearly increase of the monthly amount in percent.
        #[serde(default)]
        step_up_percent: f64,
        /// Target corpus (reports the monthly amount needed).
        #[serde(default)]
        target: Option<f64>,
        /// Inflation rate for the real-value figure.
        #[serde(default)]
        inflation: f64,
    },
    /// Fixed deposit calculator.
    FixedDeposit {
        /// Deposit amount.
        principal: f64,
        /// Annual rate in percent.
        annual_rate: f64,
        /// Term in years.
        #[serde(default)]
        years: f64,
        /// Term in months (alternative).
        #[serde(default)]
        months: u32,
        /// Compounding frequency (`quarterly` default, `monthly`, `half-yearly`, `yearly`).
        #[serde(default)]
        compounding: String,
        /// Interest paid out periodically instead of compounded.
        #[serde(default)]
        payout: bool,
    },
    /// Recurring deposit calculator.
    RecurringDeposit {
        /// Monthly deposit.
        monthly: f64,
        /// Annual rate in percent.
        annual_rate: f64,
        /// Term in months.
        months: u32,
    },
    /// PPF calculator.
    Ppf {
        /// Yearly deposit.
        yearly_deposit: f64,
        /// Annual rate in percent.
        annual_rate: f64,
        /// Initial term in years (15).
        #[serde(default = "default_15y")]
        years: u32,
        /// Extension blocks in years.
        #[serde(default)]
        extension_years: u32,
    },
    /// Inflation calculator.
    Inflation {
        /// Amount of money.
        amount: f64,
        /// Annual inflation in percent.
        annual_rate: f64,
        /// Number of years.
        years: f64,
        /// `forward` (default) or `backward`.
        #[serde(default)]
        direction: String,
    },
    /// CAGR calculator.
    Cagr {
        /// Starting value.
        start_value: f64,
        /// Ending value.
        end_value: f64,
        /// Number of years.
        years: f64,
    },
    /// ROI calculator.
    Roi {
        /// Initial cost.
        cost: f64,
        /// Final value / gain.
        gain: f64,
        /// Holding period for the annualised figure.
        #[serde(default)]
        years: Option<f64>,
        /// Additional costs.
        #[serde(default)]
        extra_costs: f64,
    },
    /// Net present value.
    Npv {
        /// Discount rate in percent.
        rate_percent: f64,
        /// Cash flows (index 0 = today, usually negative).
        cash_flows: Vec<f64>,
    },
    /// Internal rate of return.
    Irr {
        /// Cash flows (index 0 = today, usually negative).
        cash_flows: Vec<f64>,
        /// Initial guess in percent.
        #[serde(default = "default_guess")]
        guess_percent: f64,
    },
    /// Discount calculator.
    Discount {
        /// Original price.
        price: f64,
        /// Discount in percent.
        #[serde(default)]
        discount_percent: Option<f64>,
        /// Discount amount.
        #[serde(default)]
        discount_amount: Option<f64>,
        /// Final price (derives the discount).
        #[serde(default)]
        final_price: Option<f64>,
        /// Tax applied after the discount.
        #[serde(default)]
        tax_percent: f64,
        /// Number of items.
        #[serde(default)]
        quantity: f64,
    },
    /// Markup calculator.
    Markup {
        /// Unit cost.
        cost: f64,
        /// Markup in percent.
        #[serde(default)]
        markup_percent: Option<f64>,
        /// Selling price.
        #[serde(default)]
        price: Option<f64>,
        /// Desired margin in percent.
        #[serde(default)]
        margin_percent: Option<f64>,
    },
    /// Margin calculator.
    Margin {
        /// Revenue.
        revenue: f64,
        /// Cost of goods.
        cost: f64,
        /// Operating expenses.
        #[serde(default)]
        expenses: f64,
    },
    /// Break-even calculator.
    BreakEven {
        /// Fixed costs.
        fixed_costs: f64,
        /// Selling price per unit.
        price_per_unit: f64,
        /// Variable cost per unit.
        variable_cost_per_unit: f64,
        /// Profit target to include.
        #[serde(default)]
        target_profit: f64,
    },
    /// Salary breakdown.
    Salary {
        /// Gross pay.
        gross: f64,
        /// Period of `gross` (`yearly` default, `monthly`, `weekly`, `biweekly`, `daily`,
        /// `hourly`).
        #[serde(default)]
        period: String,
        /// Deductions applied.
        #[serde(default)]
        deductions: Vec<Deduction>,
        /// Working hours per week.
        #[serde(default)]
        hours_per_week: f64,
        /// Working weeks per year.
        #[serde(default)]
        weeks_per_year: f64,
    },
    /// Pay period conversion.
    Hourly {
        /// Pay amount.
        amount: f64,
        /// Period of `amount` (`hourly`, `daily`, `weekly`, `monthly`, `yearly`).
        #[serde(default)]
        from: String,
        /// Working hours per week.
        #[serde(default)]
        hours_per_week: f64,
        /// Working weeks per year.
        #[serde(default)]
        weeks_per_year: f64,
    },
    /// Salary hike calculator.
    SalaryHike {
        /// Current salary.
        current: f64,
        /// New salary (derives the percentage).
        #[serde(default)]
        new_salary: Option<f64>,
        /// Hike in percent (derives the new salary).
        #[serde(default)]
        hike_percent: Option<f64>,
    },
    /// Overtime calculator.
    Overtime {
        /// Base hourly rate.
        hourly_rate: f64,
        /// Regular hours worked.
        regular_hours: f64,
        /// Overtime hours worked.
        #[serde(default)]
        overtime_hours: f64,
        /// Overtime multiplier (1.5 default).
        #[serde(default = "default_1_5")]
        multiplier: f64,
        /// Hours paid at double time.
        #[serde(default)]
        double_hours: f64,
    },
    /// Freelance rate calculator.
    FreelanceRate {
        /// Desired yearly income.
        target_income: f64,
        /// Billable hours per week.
        billable_hours_per_week: f64,
        /// Weeks of holiday per year.
        #[serde(default)]
        weeks_off: f64,
        /// Yearly business expenses.
        #[serde(default)]
        yearly_expenses: f64,
        /// Tax rate in percent.
        #[serde(default)]
        tax_percent: f64,
        /// Profit margin to add in percent.
        #[serde(default)]
        profit_margin_percent: f64,
    },
    /// Currency conversion with supplied rates.
    Currency {
        /// Amount to convert.
        amount: f64,
        /// Source currency code.
        from: String,
        /// Target currency code.
        to: String,
        /// Rate table (`{"USD": 1, "EUR": 0.92, …}`) relative to `base`.
        #[serde(default)]
        rates: serde_json::Map<String, Value>,
        /// Base currency of `rates`.
        #[serde(default)]
        base: String,
        /// Date of the rates (echoed back).
        #[serde(default)]
        rates_date: Option<String>,
    },
    /// Tip calculator.
    Tip {
        /// Bill amount.
        bill: f64,
        /// Tip in percent.
        #[serde(default = "default_15")]
        tip_percent: f64,
        /// Number of people splitting the bill.
        #[serde(default = "default_one")]
        people: u32,
        /// Round each share up to the nearest unit.
        #[serde(default)]
        round_up: bool,
        /// Tax included in the bill in percent.
        #[serde(default)]
        tax_percent: f64,
        /// Compute the tip on the pre-tax amount.
        #[serde(default = "default_true")]
        tip_on_pre_tax: bool,
    },
    /// Progressive tax calculator with custom brackets.
    TaxBrackets {
        /// Taxable income before deductions.
        income: f64,
        /// Tax brackets.
        brackets: Vec<Bracket>,
        /// Deductions subtracted from income.
        #[serde(default)]
        deductions: f64,
        /// Surcharge on the tax in percent.
        #[serde(default)]
        surcharge_percent: f64,
        /// Cess on tax + surcharge in percent.
        #[serde(default)]
        cess_percent: f64,
        /// Tax credits subtracted at the end.
        #[serde(default)]
        credits: f64,
    },
}

/// Dispatches a [`FinanceAction`] to its implementation.
pub fn handle_action(action: FinanceAction) -> EngineResult<Value> {
    match action {
        FinanceAction::Loan {
            principal,
            annual_rate,
            years,
            months,
            frequency,
            extra,
            schedule,
        } => loan(principal, annual_rate, years, months, &frequency, &extra, schedule, None),
        FinanceAction::Mortgage {
            price,
            down_payment,
            down_payment_percent,
            annual_rate,
            years,
            property_tax_yearly,
            insurance_yearly,
            hoa_monthly,
            pmi_rate_percent,
        } => mortgage(
            price,
            down_payment,
            down_payment_percent,
            annual_rate,
            years,
            property_tax_yearly,
            insurance_yearly,
            hoa_monthly,
            pmi_rate_percent,
        ),
        FinanceAction::Interest {
            principal,
            annual_rate,
            years,
            compounding,
            contribution,
            contribution_frequency,
            contribute_at_start,
        } => interest(
            principal,
            annual_rate,
            years,
            &compounding,
            contribution,
            &contribution_frequency,
            contribute_at_start,
        ),
        FinanceAction::Investment {
            monthly,
            lumpsum,
            annual_return,
            years,
            step_up_percent,
            target,
            inflation,
        } => investment(monthly, lumpsum, annual_return, years, step_up_percent, target, inflation),
        FinanceAction::FixedDeposit {
            principal,
            annual_rate,
            years,
            months,
            compounding,
            payout,
        } => fixed_deposit(principal, annual_rate, years, months, &compounding, payout),
        FinanceAction::RecurringDeposit { monthly, annual_rate, months } => {
            recurring_deposit(monthly, annual_rate, months)
        },
        FinanceAction::Ppf {
            yearly_deposit,
            annual_rate,
            years,
            extension_years,
        } => ppf(yearly_deposit, annual_rate, years, extension_years),
        FinanceAction::Inflation { amount, annual_rate, years, direction } => {
            inflation(amount, annual_rate, years, &direction)
        },
        FinanceAction::Cagr { start_value, end_value, years } => {
            cagr(start_value, end_value, years)
        },
        FinanceAction::Roi { cost, gain, years, extra_costs } => {
            roi(cost, gain, years, extra_costs)
        },
        FinanceAction::Npv { rate_percent, cash_flows } => npv(rate_percent, &cash_flows),
        FinanceAction::Irr { cash_flows, guess_percent } => irr(&cash_flows, guess_percent),
        FinanceAction::Discount {
            price,
            discount_percent,
            discount_amount,
            final_price,
            tax_percent,
            quantity,
        } => discount(price, discount_percent, discount_amount, final_price, tax_percent, quantity),
        FinanceAction::Markup {
            cost,
            markup_percent,
            price,
            margin_percent,
        } => markup(cost, markup_percent, price, margin_percent),
        FinanceAction::Margin { revenue, cost, expenses } => margin(revenue, cost, expenses),
        FinanceAction::BreakEven {
            fixed_costs,
            price_per_unit,
            variable_cost_per_unit,
            target_profit,
        } => break_even(fixed_costs, price_per_unit, variable_cost_per_unit, target_profit),
        FinanceAction::Salary {
            gross,
            period,
            deductions,
            hours_per_week,
            weeks_per_year,
        } => salary(gross, &period, &deductions, hours_per_week, weeks_per_year),
        FinanceAction::Hourly {
            amount,
            from,
            hours_per_week,
            weeks_per_year,
        } => hourly(amount, &from, hours_per_week, weeks_per_year),
        FinanceAction::SalaryHike { current, new_salary, hike_percent } => {
            salary_hike(current, new_salary, hike_percent)
        },
        FinanceAction::Overtime {
            hourly_rate,
            regular_hours,
            overtime_hours,
            multiplier,
            double_hours,
        } => overtime(hourly_rate, regular_hours, overtime_hours, multiplier, double_hours),
        FinanceAction::FreelanceRate {
            target_income,
            billable_hours_per_week,
            weeks_off,
            yearly_expenses,
            tax_percent,
            profit_margin_percent,
        } => freelance_rate(
            target_income,
            billable_hours_per_week,
            weeks_off,
            yearly_expenses,
            tax_percent,
            profit_margin_percent,
        ),
        FinanceAction::Currency {
            amount,
            from,
            to,
            rates,
            base,
            rates_date,
        } => currency(amount, &from, &to, &rates, &base, rates_date.as_deref()),
        FinanceAction::Tip {
            bill,
            tip_percent,
            people,
            round_up,
            tax_percent,
            tip_on_pre_tax,
        } => tip(bill, tip_percent, people, round_up, tax_percent, tip_on_pre_tax),
        FinanceAction::TaxBrackets {
            income,
            brackets,
            deductions,
            surcharge_percent,
            cess_percent,
            credits,
        } => tax_brackets(income, &brackets, deductions, surcharge_percent, cess_percent, credits),
    }
}

engine_shared::export_engine!(handle_action, FinanceAction);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loans_and_mortgages() {
        let l =
            loan(100_000.0, 6.0, 30.0, 0, "monthly", &ExtraPayment::default(), true, None).unwrap();
        assert_eq!(l["payment"], 599.55);
        assert_eq!(l["periods"], 360);
        assert_eq!(l["actual_periods"], 360);
        let schedule = l["schedule"].as_array().unwrap();
        assert_eq!(schedule.len(), 360);
        assert_eq!(schedule[359]["balance"], 0.0);
        let paid: f64 = schedule.iter().map(|r| r["payment"].as_f64().unwrap()).sum();
        assert!((paid - l["total_paid"].as_f64().unwrap()).abs() < 0.01);
        let extra = loan(
            100_000.0,
            6.0,
            30.0,
            0,
            "monthly",
            &ExtraPayment { monthly: 100.0, ..Default::default() },
            false,
            None,
        )
        .unwrap();
        assert!(extra["actual_periods"].as_u64().unwrap() < 300);
        assert!(extra["savings_from_extra"]["interest_saved"].as_f64().unwrap() > 39_000.0);
        assert_eq!(
            loan(1200.0, 0.0, 1.0, 0, "monthly", &ExtraPayment::default(), false, None).unwrap()
                ["payment"],
            100.0
        );
        assert!(loan(0.0, 5.0, 1.0, 0, "monthly", &ExtraPayment::default(), false, None).is_err());
        assert!(loan(100.0, 5.0, 0.0, 0, "monthly", &ExtraPayment::default(), false, None).is_err());
        let m =
            mortgage(400_000.0, 0.0, Some(10.0), 6.5, 30.0, 4800.0, 1200.0, 100.0, 0.5).unwrap();
        assert_eq!(m["loan_amount"], 360_000.0);
        assert_eq!(m["pmi"], 150.0);
        assert_eq!(m["property_tax"], 400.0);
        assert!(mortgage(100.0, 200.0, None, 5.0, 30.0, 0.0, 0.0, 0.0, 0.0).is_err());
    }

    #[test]
    fn interest_and_investments() {
        let i = interest(1000.0, 5.0, 10.0, "yearly", 0.0, "monthly", false).unwrap();
        assert_eq!(i["simple"]["interest"], 500.0);
        assert_eq!(i["compound"]["total"], 1628.89);
        let c = interest(1000.0, 12.0, 1.0, "monthly", 100.0, "monthly", false).unwrap();
        assert_eq!(c["compound"]["total"], 2395.08);
        assert_eq!(
            interest(1000.0, 5.0, 1.0, "continuous", 0.0, "monthly", false).unwrap()["compound"]
                ["total"],
            1051.27
        );
        let s = investment(5000.0, 0.0, 12.0, 10.0, 0.0, Some(1_000_000.0), 6.0).unwrap();
        assert_eq!(s["invested"], 600_000.0);
        assert!((s["maturity_value"].as_f64().unwrap() - 1_150_193.0).abs() < 1000.0);
        assert!(s["required_monthly_for_target"].as_f64().unwrap() > 4000.0);
        assert!(investment(0.0, 0.0, 10.0, 1.0, 0.0, None, 0.0).is_err());
        let fd = fixed_deposit(100_000.0, 7.0, 1.0, 0, "", false).unwrap();
        assert_eq!(fd["maturity_amount"], 107_185.9);
        let rd = recurring_deposit(1000.0, 6.0, 12).unwrap();
        assert!(rd["maturity_amount"].as_f64().unwrap() > 12_300.0);
        let p = ppf(150_000.0, 7.1, 15, 0).unwrap();
        assert!(p["maturity_amount"].as_f64().unwrap() > 4_000_000.0);
        assert_eq!(inflation(100.0, 5.0, 2.0, "").unwrap()["result"], 110.25);
        assert_eq!(cagr(1000.0, 2000.0, 5.0).unwrap()["cagr_percent"], 14.87);
        assert_eq!(roi(1000.0, 1500.0, Some(2.0), 0.0).unwrap()["roi_percent"], 50.0);
    }

    #[test]
    fn project_metrics_and_pricing() {
        let n = npv(10.0, &[-1000.0, 500.0, 400.0, 300.0]).unwrap();
        assert_eq!(n["npv"], 10.52);
        let i = irr(&[-1000.0, 500.0, 400.0, 300.0], 10.0).unwrap();
        assert_eq!(i["irr_percent"], 10.65);
        assert!(irr(&[-1.0], 10.0).is_err());
        assert!(irr(&[1.0, 2.0], 10.0).is_err());
        let d = discount(80.0, Some(25.0), None, None, 10.0, 2.0).unwrap();
        assert_eq!(d["discounted_price"], 60.0);
        assert_eq!(d["final_price"], 66.0);
        assert_eq!(d["total_for_quantity"], 132.0);
        assert_eq!(
            discount(80.0, None, None, Some(60.0), 0.0, 1.0).unwrap()["discount_percent"],
            25.0
        );
        assert!(discount(80.0, Some(150.0), None, None, 0.0, 1.0).is_err());
        let m = markup(50.0, Some(40.0), None, None).unwrap();
        assert_eq!(m["price"], 70.0);
        assert_eq!(m["margin_percent"], 28.57);
        assert_eq!(markup(50.0, None, None, Some(50.0)).unwrap()["price"], 100.0);
        assert_eq!(margin(1000.0, 600.0, 100.0).unwrap()["net_margin_percent"], 30.0);
        let b = break_even(10_000.0, 50.0, 30.0, 0.0).unwrap();
        assert_eq!(b["break_even_units"], 500.0);
        assert!(break_even(10.0, 5.0, 5.0, 0.0).is_err());
    }

    #[test]
    fn pay_currency_tip_tax() {
        let s = salary(
            60_000.0,
            "yearly",
            &[
                Deduction {
                    name: "tax".into(),
                    percent: 20.0,
                    amount: 0.0,
                    period: String::new(),
                },
                Deduction {
                    name: "health".into(),
                    percent: 0.0,
                    amount: 100.0,
                    period: "monthly".into(),
                },
            ],
            40.0,
            52.0,
        )
        .unwrap();
        assert_eq!(s["net"]["yearly"], 46_800.0);
        assert_eq!(s["gross"]["monthly"], 5000.0);
        assert_eq!(hourly(20.0, "hourly", 40.0, 52.0).unwrap()["yearly"], 41_600.0);
        assert_eq!(salary_hike(50_000.0, None, Some(10.0)).unwrap()["new"], 55_000.0);
        assert_eq!(overtime(20.0, 40.0, 10.0, 1.5, 0.0).unwrap()["total_pay"], 1100.0);
        assert!(
            freelance_rate(80_000.0, 25.0, 4.0, 5000.0, 25.0, 10.0).unwrap()["hourly_rate"]
                .as_f64()
                .unwrap()
                > 90.0
        );
        let mut rates = serde_json::Map::new();
        rates.insert("EUR".into(), json!(0.9));
        rates.insert("INR".into(), json!(83.0));
        let c = currency(100.0, "eur", "inr", &rates, "USD", Some("2024-01-31")).unwrap();
        assert_eq!(c["result"], 9222.22);
        assert!(currency(1.0, "EUR", "GBP", &rates, "USD", None).is_err());
        assert!(currency(1.0, "EUR", "USD", &serde_json::Map::new(), "USD", None).is_err());
        let t = tip(100.0, 15.0, 4, false, 0.0, true).unwrap();
        assert_eq!(t["per_person"], 28.75);
        assert_eq!(tip(100.0, 15.0, 1, true, 0.0, true).unwrap()["total"], 115.0);
        let brackets = vec![
            Bracket {
                from: 0.0,
                to: Some(10_000.0),
                rate_percent: 0.0,
            },
            Bracket {
                from: 10_000.0,
                to: Some(50_000.0),
                rate_percent: 10.0,
            },
            Bracket {
                from: 50_000.0,
                to: None,
                rate_percent: 20.0,
            },
        ];
        let tx = tax_brackets(80_000.0, &brackets, 0.0, 0.0, 4.0, 0.0).unwrap();
        assert_eq!(tx["base_tax"], 10_000.0);
        assert_eq!(tx["cess"], 400.0);
        assert_eq!(tx["marginal_rate_percent"], 20.0);
        assert!(tax_brackets(1.0, &[], 0.0, 0.0, 0.0, 0.0).is_err());
    }
}
