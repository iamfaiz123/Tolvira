//! # Dev Toolbox Business Operations Engine (`business-engine`)
//!
//! Document generators (`invoice`, `quotation`, `purchase_order`) compute all
//! totals, tax splits and produce a printable HTML plus a structured JSON
//! model; rendering to PDF is delegated to the PDF engine (`from_html`).
//!
//! | Action | Tool(s) |
//! |---|---|
//! | `document` | invoice-generator, quotation-generator, purchase-order-generator |
//! | `profit_loss` | profit-loss-calculator |
//! | `markup_margin` | markup-margin-calculator |
//! | `inventory_turnover` | inventory-turnover-calculator |
//! | `reorder` | stock-reorder-calculator (EOQ, reorder point, safety stock) |
//! | `discount_pricing` | discount-pricing-calculator (tiers / bundles) |
//! | `sales_tax` | sales-tax-calculator |
//! | `commission` | commission-calculator (flat / tiered) |
//! | `payroll` | payroll-estimator |
//! | `cash_flow` | cash-flow-forecast |
//! | `break_even` | break-even-analysis |
//! | `valuation` | business-valuation-estimator (multiples, DCF) |
//! | `unit_economics` | unit-economics-calculator (CAC, LTV, payback) |

use engine_shared::{EngineError, EngineResult};
use serde::Deserialize;
use serde_json::{json, Value};

fn r2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
fn positive(v: f64, what: &str) -> EngineResult<f64> {
    engine_shared::error::require_positive(v, what)
}
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// One line of an invoice/quotation.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct LineItem {
    /// Item description.
    pub description: String,
    /// Quantity.
    pub quantity: f64,
    /// Price per unit.
    pub unit_price: f64,
    /// Tax rate override for this line.
    pub tax_percent: Option<f64>,
    /// Line discount in percent.
    pub discount_percent: f64,
    /// Unit label (`pcs`, `hrs`, …).
    pub unit: String,
    /// Stock keeping unit / item code.
    pub sku: String,
}

/// A party on a document (issuer or customer).
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Party {
    /// Name.
    pub name: String,
    /// Postal address.
    pub address: String,
    /// E-mail.
    pub email: String,
    /// Phone.
    pub phone: String,
    /// Tax identifier (VAT/GSTIN/EIN).
    pub tax_id: String,
}

/// Invoice, quotation or receipt to be calculated and rendered.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Document {
    /// `invoice` (default), `quotation`, `purchase_order`, `receipt`, `proforma`.
    pub kind: String,
    /// Document number.
    pub number: String,
    /// Issue date.
    pub date: String,
    /// Payment due date.
    pub due_date: String,
    /// ISO currency code.
    pub currency: String,
    /// Issuing party.
    pub from: Party,
    /// Receiving party.
    pub to: Party,
    /// Line items.
    pub items: Vec<LineItem>,
    /// Default tax rate in percent.
    pub tax_percent: f64,
    /// `exclusive` (default) or `inclusive`.
    pub tax_mode: String,
    /// Label for the tax line (`VAT`, `GST`, …).
    pub tax_label: String,
    /// Split tax equally into two components (e.g. CGST/SGST).
    pub split_tax: bool,
    /// Document-level discount in percent.
    pub discount_percent: f64,
    /// Document-level discount amount.
    pub discount_amount: f64,
    /// Shipping charge.
    pub shipping: f64,
    /// Free-text notes.
    pub notes: String,
    /// Payment terms.
    pub terms: String,
    /// Amount already paid.
    pub paid_amount: f64,
    /// Round the grand total to the nearest unit.
    pub rounding: bool,
}

/// Calculates totals and renders the document as HTML/text/Markdown.
pub fn document(d: &Document) -> EngineResult<Value> {
    if d.items.is_empty() {
        return Err(EngineError::empty("items"));
    }
    let kind = if d.kind.is_empty() { "invoice" } else { d.kind.as_str() };
    let title = match kind {
        "invoice" => "INVOICE",
        "quotation" | "quote" | "estimate" => "QUOTATION",
        "purchase_order" | "po" => "PURCHASE ORDER",
        "receipt" => "RECEIPT",
        "proforma" => "PROFORMA INVOICE",
        "credit_note" => "CREDIT NOTE",
        other => return Err(EngineError::invalid(format!("Unknown document kind `{other}`"))),
    };
    let cur = if d.currency.is_empty() { "" } else { d.currency.as_str() };
    let inclusive = d.tax_mode == "inclusive";
    let mut subtotal = 0.0;
    let mut total_tax = 0.0;
    let mut line_discounts = 0.0;
    let mut lines = Vec::new();
    for (i, it) in d.items.iter().enumerate() {
        if it.quantity <= 0.0
            || it.unit_price < 0.0
            || !it.quantity.is_finite()
            || !it.unit_price.is_finite()
        {
            return Err(EngineError::invalid(format!(
                "Item {}: quantity must be > 0 and unit_price >= 0",
                i + 1
            )));
        }
        let gross = it.quantity * it.unit_price;
        let disc = gross * it.discount_percent.clamp(0.0, 100.0) / 100.0;
        let rate = it.tax_percent.unwrap_or(d.tax_percent).max(0.0);
        let (net, tax) = if inclusive {
            let net = (gross - disc) / (1.0 + rate / 100.0);
            (net, gross - disc - net)
        } else {
            (gross - disc, (gross - disc) * rate / 100.0)
        };
        subtotal += net;
        total_tax += tax;
        line_discounts += disc;
        lines.push(json!({
            "n": i + 1,
            "description": it.description,
            "sku": it.sku,
            "quantity": it.quantity,
            "unit": it.unit,
            "unit_price": r2(it.unit_price),
            "discount_percent": it.discount_percent,
            "discount": r2(disc),
            "net": r2(net),
            "tax_percent": rate,
            "tax": r2(tax),
            "total": r2(net + tax)
        }));
    }
    // Document level discount applies proportionally before tax.
    let doc_disc = if d.discount_percent > 0.0 {
        subtotal * d.discount_percent.clamp(0.0, 100.0) / 100.0
    } else {
        d.discount_amount.max(0.0).min(subtotal)
    };
    let disc_ratio = if subtotal > 0.0 { 1.0 - doc_disc / subtotal } else { 1.0 };
    let taxable = subtotal - doc_disc;
    let tax_after = total_tax * disc_ratio;
    let mut total = taxable + tax_after + d.shipping.max(0.0);
    let rounding = if d.rounding { total.round() - total } else { 0.0 };
    total += rounding;
    let balance = total - d.paid_amount.max(0.0);
    let tax_label = if d.tax_label.is_empty() {
        "Tax".to_string()
    } else {
        d.tax_label.clone()
    };
    let tax_rows: Vec<(String, f64)> = if d.split_tax {
        vec![
            (format!("{tax_label} (1/2)"), tax_after / 2.0),
            (format!("{tax_label} (2/2)"), tax_after / 2.0),
        ]
    } else {
        vec![(tax_label.clone(), tax_after)]
    };
    let money = |v: f64| {
        if cur.is_empty() {
            format!("{:.2}", v)
        } else {
            format!("{cur} {:.2}", v)
        }
    };
    let party = |p: &Party| {
        format!(
            "<strong>{}</strong><br>{}{}{}{}",
            esc(&p.name),
            if p.address.is_empty() {
                String::new()
            } else {
                format!("{}<br>", esc(&p.address).replace('\n', "<br>"))
            },
            if p.email.is_empty() {
                String::new()
            } else {
                format!("{}<br>", esc(&p.email))
            },
            if p.phone.is_empty() {
                String::new()
            } else {
                format!("{}<br>", esc(&p.phone))
            },
            if p.tax_id.is_empty() {
                String::new()
            } else {
                format!("Tax ID: {}", esc(&p.tax_id))
            }
        )
    };
    let mut html = format!("<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>{title} \
                            {}</title><style>body{{font-family:Helvetica,Arial,sans-serif;font-size:13px;color:#222;margin:32px}}h1{{font-size:26px;letter-spacing:2px;margin:0 0 4px}}table{{width:100%;border-collapse:collapse;margin-top:16px}}th,td{{padding:6px 8px;border-bottom:1px solid #ddd;text-align:left}}th{{background:#f3f4f6;font-size:11px;text-transform:uppercase}}.num{{text-align:right}}.totals td{{border:none}}.grand{{font-size:16px;font-weight:bold}}.meta{{display:flex;justify-content:space-between;margin-top:12px}}.box{{width:48%}}.muted{{color:#666}}</style></head><body>", esc(&d.number));
    html.push_str(&format!(
        "<h1>{title}</h1><div class=\"muted\">No. {} &nbsp;|&nbsp; Date: {}{}</div>",
        esc(&d.number),
        esc(&d.date),
        if d.due_date.is_empty() {
            String::new()
        } else {
            format!(" &nbsp;|&nbsp; Due: {}", esc(&d.due_date))
        }
    ));
    html.push_str(&format!(
        "<div class=\"meta\"><div class=\"box\"><div class=\"muted\">From</div>{}</div><div \
         class=\"box\"><div class=\"muted\">{}</div>{}</div></div>",
        party(&d.from),
        if kind == "purchase_order" { "Vendor" } else { "Bill to" },
        party(&d.to)
    ));
    html.push_str(
        "<table><thead><tr><th>#</th><th>Description</th><th class=\"num\">Qty</th><th \
         class=\"num\">Unit price</th><th class=\"num\">Tax %</th><th \
         class=\"num\">Amount</th></tr></thead><tbody>",
    );
    for l in &lines {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}{}</td><td class=\"num\">{} {}</td><td class=\"num\">{}</td><td \
             class=\"num\">{}</td><td class=\"num\">{}</td></tr>",
            l["n"],
            esc(l["description"].as_str().unwrap_or("")),
            if l["discount"].as_f64().unwrap_or(0.0) > 0.0 {
                format!(" <span class=\"muted\">(−{}%)</span>", l["discount_percent"])
            } else {
                String::new()
            },
            l["quantity"],
            esc(l["unit"].as_str().unwrap_or("")),
            money(l["unit_price"].as_f64().unwrap_or(0.0)),
            l["tax_percent"],
            money(l["net"].as_f64().unwrap_or(0.0))
        ));
    }
    html.push_str("</tbody></table><table class=\"totals\" style=\"width:40%;margin-left:auto\">");
    html.push_str(&format!("<tr><td>Subtotal</td><td class=\"num\">{}</td></tr>", money(subtotal)));
    if doc_disc > 0.0 {
        html.push_str(&format!(
            "<tr><td>Discount</td><td class=\"num\">−{}</td></tr>",
            money(doc_disc)
        ));
    }
    for (label, amount) in &tax_rows {
        html.push_str(&format!(
            "<tr><td>{}</td><td class=\"num\">{}</td></tr>",
            esc(label),
            money(*amount)
        ));
    }
    if d.shipping > 0.0 {
        html.push_str(&format!(
            "<tr><td>Shipping</td><td class=\"num\">{}</td></tr>",
            money(d.shipping)
        ));
    }
    if rounding.abs() > 0.0001 {
        html.push_str(&format!(
            "<tr><td>Rounding</td><td class=\"num\">{}</td></tr>",
            money(rounding)
        ));
    }
    html.push_str(&format!(
        "<tr class=\"grand\"><td>Total</td><td class=\"num\">{}</td></tr>",
        money(total)
    ));
    if d.paid_amount > 0.0 {
        html.push_str(&format!(
            "<tr><td>Paid</td><td class=\"num\">{}</td></tr><tr class=\"grand\"><td>Balance \
             due</td><td class=\"num\">{}</td></tr>",
            money(d.paid_amount),
            money(balance)
        ));
    }
    html.push_str("</table>");
    if !d.notes.is_empty() {
        html.push_str(&format!(
            "<p><strong>Notes</strong><br>{}</p>",
            esc(&d.notes).replace('\n', "<br>")
        ));
    }
    if !d.terms.is_empty() {
        html.push_str(&format!(
            "<p class=\"muted\"><strong>Terms</strong><br>{}</p>",
            esc(&d.terms).replace('\n', "<br>")
        ));
    }
    html.push_str("</body></html>");
    Ok(json!({
        "kind": kind,
        "title": title,
        "number": d.number,
        "currency": cur,
        "lines": lines,
        "totals": {
            "subtotal": r2(subtotal),
            "line_discounts": r2(line_discounts),
            "document_discount": r2(doc_disc),
            "taxable": r2(taxable),
            "tax": r2(tax_after),
            "tax_components": tax_rows.iter().map(|(l, a)| json!({ "label": l, "amount": r2(*a) })).collect::<Vec<_>>(),
            "shipping": r2(d.shipping.max(0.0)),
            "rounding": r2(rounding),
            "total": r2(total),
            "paid": r2(d.paid_amount.max(0.0)),
            "balance_due": r2(balance)
        },
        "amount_in_words_hint": "use math-engine number_to_words with currency for the words line",
        "html": html,
    }))
}

/// Profit & loss statement with gross/operating/net margins.
pub fn profit_loss(
    revenue: f64,
    cogs: f64,
    operating_expenses: &serde_json::Map<String, Value>,
    other_income: f64,
    interest: f64,
    depreciation: f64,
    tax_percent: f64,
) -> EngineResult<Value> {
    positive(revenue, "revenue")?;
    let opex_total: f64 = operating_expenses.values().filter_map(Value::as_f64).sum();
    let gross = revenue - cogs.max(0.0);
    let ebitda = gross - opex_total + other_income;
    let ebit = ebitda - depreciation.max(0.0);
    let ebt = ebit - interest.max(0.0);
    let tax = if ebt > 0.0 {
        ebt * tax_percent.max(0.0) / 100.0
    } else {
        0.0
    };
    let net = ebt - tax;
    let pct = |v: f64| r2(v / revenue * 100.0);
    Ok(json!({
        "revenue": r2(revenue),
        "cogs": r2(cogs.max(0.0)),
        "gross_profit": r2(gross),
        "gross_margin_percent": pct(gross),
        "operating_expenses": operating_expenses.iter().map(|(k, v)| json!({ "name": k, "amount": v.as_f64().map(r2), "percent_of_revenue": v.as_f64().map(pct) })).collect::<Vec<_>>(),
        "operating_expenses_total": r2(opex_total),
        "other_income": r2(other_income),
        "ebitda": r2(ebitda),
        "ebitda_margin_percent": pct(ebitda),
        "depreciation": r2(depreciation.max(0.0)),
        "ebit": r2(ebit),
        "operating_margin_percent": pct(ebit),
        "interest": r2(interest.max(0.0)),
        "profit_before_tax": r2(ebt),
        "tax": r2(tax),
        "net_profit": r2(net),
        "net_margin_percent": pct(net),
        "status": if net > 0.0 { "profit" } else if net < 0.0 { "loss" } else { "break-even" },
    }))
}

/// Markup ↔ margin from any two of cost, price, markup %, margin %.
pub fn markup_margin(
    cost: Option<f64>,
    price: Option<f64>,
    markup_percent: Option<f64>,
    margin_percent: Option<f64>,
) -> EngineResult<Value> {
    let (c, p) = match (cost, price, markup_percent, margin_percent) {
        (Some(c), Some(p), _, _) => (c, p),
        (Some(c), None, Some(m), _) => (c, c * (1.0 + m / 100.0)),
        (Some(c), None, None, Some(g)) => {
            if g >= 100.0 {
                return Err(EngineError::invalid("margin must be below 100%"));
            }
            (c, c / (1.0 - g / 100.0))
        },
        (None, Some(p), Some(m), _) => (p / (1.0 + m / 100.0), p),
        (None, Some(p), None, Some(g)) => (p * (1.0 - g / 100.0), p),
        _ => {
            return Err(EngineError::invalid(
                "Provide two of: cost, price, markup_percent, margin_percent",
            ))
        },
    };
    positive(c, "cost")?;
    positive(p, "price")?;
    let profit = p - c;
    let table: Vec<Value> = [10.0, 20.0, 25.0, 30.0, 40.0, 50.0, 75.0, 100.0]
        .iter()
        .map(|m| {
            json!({
                "markup_percent": m,
                "margin_percent": r2(m / (100.0 + m) * 100.0),
                "price": r2(c * (1.0 + m / 100.0))
            })
        })
        .collect();
    Ok(json!({
        "cost": r2(c),
        "price": r2(p),
        "profit": r2(profit),
        "markup_percent": r2(profit / c * 100.0),
        "margin_percent": r2(profit / p * 100.0),
        "conversion_table": table,
        "formulas": {
            "markup_to_margin": "margin = markup / (1 + markup)",
            "margin_to_markup": "markup = margin / (1 − margin)"
        }
    }))
}

/// Inventory turnover, days of inventory and GMROI.
pub fn inventory_turnover(
    cogs: f64,
    opening_inventory: f64,
    closing_inventory: f64,
    period_days: u32,
    revenue: Option<f64>,
) -> EngineResult<Value> {
    positive(cogs, "cogs")?;
    let avg = (opening_inventory.max(0.0) + closing_inventory.max(0.0)) / 2.0;
    if avg <= 0.0 {
        return Err(EngineError::invalid("Average inventory must be positive"));
    }
    let days = if period_days == 0 { 365 } else { period_days } as f64;
    let turnover = cogs / avg;
    let dio = days / turnover;
    Ok(json!({
        "average_inventory": r2(avg),
        "turnover_ratio": r2(turnover),
        "days_inventory_outstanding": r2(dio),
        "weeks_of_stock": r2(dio / 7.0),
        "annualized_turnover": r2(turnover * 365.0 / days),
        "inventory_to_sales_percent": revenue.filter(|r| *r > 0.0).map(|r| r2(avg / r * 100.0)),
        "gmroi": revenue.filter(|r| *r > 0.0).map(|r| r2((r - cogs) / avg)),
        "benchmark": if turnover >= 8.0 { "high (fast moving / lean stock)" } else if turnover >= 4.0 { "healthy" } else if turnover >= 2.0 { "slow" } else { "\
            very slow – check overstock" }
    }))
}

/// Economic order quantity, reorder point and safety stock.
pub fn reorder(
    annual_demand: f64,
    ordering_cost: f64,
    holding_cost_per_unit: f64,
    lead_time_days: f64,
    daily_demand: Option<f64>,
    max_daily_demand: Option<f64>,
    max_lead_time_days: Option<f64>,
    service_level_z: f64,
    demand_std_dev_daily: f64,
    working_days: f64,
) -> EngineResult<Value> {
    positive(annual_demand, "annual_demand")?;
    let wd = if working_days <= 0.0 { 365.0 } else { working_days };
    let daily = daily_demand.unwrap_or(annual_demand / wd);
    let eoq = if ordering_cost > 0.0 && holding_cost_per_unit > 0.0 {
        Some((2.0 * annual_demand * ordering_cost / holding_cost_per_unit).sqrt())
    } else {
        None
    };
    let safety = if demand_std_dev_daily > 0.0 && lead_time_days > 0.0 {
        service_level_z.max(0.0) * demand_std_dev_daily * lead_time_days.sqrt()
    } else {
        let max_d = max_daily_demand.unwrap_or(daily);
        let max_lt = max_lead_time_days.unwrap_or(lead_time_days);
        (max_d * max_lt - daily * lead_time_days).max(0.0)
    };
    let rop = daily * lead_time_days + safety;
    let orders_per_year = eoq.map(|q| annual_demand / q);
    Ok(json!({
        "daily_demand": r2(daily),
        "lead_time_demand": r2(daily * lead_time_days),
        "safety_stock": r2(safety),
        "reorder_point": r2(rop),
        "eoq": eoq.map(|q| q.ceil()),
        "orders_per_year": orders_per_year.map(r2),
        "days_between_orders": orders_per_year.map(|o| r2(wd / o)),
        "annual_ordering_cost": eoq.map(|q| r2(annual_demand / q * ordering_cost)),
        "annual_holding_cost": eoq.map(|q| r2(q / 2.0 * holding_cost_per_unit)),
        "max_stock_level": eoq.map(|q| r2(rop + q)),
        "assumptions": {
            "working_days": wd,
            "service_level_z": service_level_z,
            "note": "z = 1.65 ≈ 95% service level, 2.05 ≈ 98%, 2.33 ≈ 99%"
        }
    }))
}

/// A quantity-based discount tier.
#[derive(Deserialize, Debug, Clone)]
pub struct Tier {
    /// Minimum quantity for the tier.
    pub min_quantity: f64,
    /// Discount in percent.
    pub discount_percent: f64,
}

/// Tiered/coupon pricing with margin guard.
pub fn discount_pricing(
    list_price: f64,
    quantity: f64,
    tiers: &[Tier],
    coupon_percent: f64,
    coupon_amount: f64,
    tax_percent: f64,
    cost: Option<f64>,
    min_margin_percent: Option<f64>,
) -> EngineResult<Value> {
    positive(list_price, "list_price")?;
    positive(quantity, "quantity")?;
    let mut sorted = tiers.to_vec();
    sorted.sort_by(|a, b| a.min_quantity.total_cmp(&b.min_quantity));
    let tier = sorted.iter().rev().find(|t| quantity >= t.min_quantity);
    let tier_pct = tier.map(|t| t.discount_percent).unwrap_or(0.0).clamp(0.0, 100.0);
    let unit_after_tier = list_price * (1.0 - tier_pct / 100.0);
    let gross = unit_after_tier * quantity;
    let coupon = if coupon_percent > 0.0 {
        gross * coupon_percent.min(100.0) / 100.0
    } else {
        coupon_amount.max(0.0).min(gross)
    };
    let net = gross - coupon;
    let tax = net * tax_percent.max(0.0) / 100.0;
    let effective_unit = net / quantity;
    let margin = cost
        .filter(|c| *c > 0.0)
        .map(|c| (effective_unit - c) / effective_unit * 100.0);
    let margin_ok = match (margin, min_margin_percent) {
        (Some(m), Some(min)) => Some(m >= min),
        _ => None,
    };
    let floor_price = cost
        .zip(min_margin_percent)
        .filter(|(_, m)| *m < 100.0)
        .map(|(c, m)| c / (1.0 - m / 100.0));
    Ok(json!({
        "list_price": r2(list_price),
        "quantity": quantity,
        "tier_discount_percent": tier_pct,
        "unit_price_after_tier": r2(unit_after_tier),
        "gross": r2(gross),
        "coupon": r2(coupon),
        "net": r2(net),
        "tax": r2(tax),
        "total": r2(net + tax),
        "effective_unit_price": r2(effective_unit),
        "total_discount_percent": r2((1.0 - effective_unit / list_price) * 100.0),
        "savings": r2(list_price * quantity - net),
        "margin_percent": margin.map(r2),
        "margin_ok": margin_ok,
        "minimum_unit_price_for_margin": floor_price.map(r2),
        "tiers": sorted.iter().map(|t| json!({ "min_quantity": t.min_quantity, "discount_percent": t.discount_percent, "unit_price": r2(list_price * (1.0 - t.discount_percent / 100.0)) })).collect::<Vec<_>>()
    }))
}

/// Sales tax with several stacked rates (state + county + city).
pub fn sales_tax(
    amount: f64,
    rates: &[f64],
    labels: &[String],
    inclusive: bool,
    exempt_amount: f64,
) -> EngineResult<Value> {
    positive(amount, "amount")?;
    if rates.is_empty() {
        return Err(EngineError::invalid(
            "Provide at least one tax rate (e.g. [state, county, city])",
        ));
    }
    let total_rate: f64 = rates.iter().map(|r| r.max(0.0)).sum();
    let taxable_gross = (amount - exempt_amount.max(0.0)).max(0.0);
    let (base, tax) = if inclusive {
        let b = taxable_gross / (1.0 + total_rate / 100.0);
        (b, taxable_gross - b)
    } else {
        (taxable_gross, taxable_gross * total_rate / 100.0)
    };
    let components: Vec<Value> = rates
        .iter()
        .enumerate()
        .map(|(i, r)| {
            json!({
                "label": labels.get(i).cloned().unwrap_or_else(|| format!("rate {}", i + 1)),
                "rate_percent": r,
                "tax": r2(base * r.max(0.0) / 100.0)
            })
        })
        .collect();
    let total = if inclusive { amount } else { amount + tax };
    Ok(json!({
        "pre_tax": r2(if inclusive { base + exempt_amount.max(0.0) } else { amount }),
        "taxable": r2(base),
        "exempt": r2(exempt_amount.max(0.0)),
        "combined_rate_percent": r2(total_rate),
        "tax": r2(tax),
        "total": r2(total),
        "components": components,
        "mode": if inclusive { "inclusive" } else { "exclusive" }
    }))
}

/// A commission tier.
#[derive(Deserialize, Debug, Clone)]
pub struct CommissionTier {
    /// Lower bound of sales (inclusive).
    pub from: f64,
    /// Upper bound (`null` = no limit).
    #[serde(default)]
    pub to: Option<f64>,
    /// Commission rate in percent.
    pub rate_percent: f64,
}

/// Commission from a flat rate or tiers (progressive or whole-amount), with quota accelerators and
/// splits.
pub fn commission(
    sales: f64,
    rate_percent: Option<f64>,
    tiers: &[CommissionTier],
    base_salary: f64,
    progressive: bool,
    quota: Option<f64>,
    accelerator_percent: f64,
    split_percent: f64,
) -> EngineResult<Value> {
    positive(sales, "sales")?;
    let mut rows = Vec::new();
    let mut comm = 0.0;
    if let Some(r) = rate_percent {
        comm = sales * r.max(0.0) / 100.0;
        rows.push(json!({ "from": 0, "to": sales, "rate_percent": r, "commission": r2(comm) }));
    } else if !tiers.is_empty() {
        let mut sorted = tiers.to_vec();
        sorted.sort_by(|a, b| a.from.total_cmp(&b.from));
        if progressive {
            for t in &sorted {
                if sales <= t.from {
                    break;
                }
                let upper = t.to.unwrap_or(f64::INFINITY);
                let portion = sales.min(upper) - t.from;
                let c = portion * t.rate_percent / 100.0;
                comm += c;
                rows.push(json!({
                    "from": t.from,
                    "to": t.to,
                    "rate_percent": t.rate_percent,
                    "portion": r2(portion),
                    "commission": r2(c)
                }));
            }
        } else {
            let t = sorted
                .iter()
                .rev()
                .find(|t| sales >= t.from)
                .ok_or_else(|| EngineError::invalid("Sales are below the first tier"))?;
            comm = sales * t.rate_percent / 100.0;
            rows.push(json!({
                "from": t.from,
                "to": t.to,
                "rate_percent": t.rate_percent,
                "commission": r2(comm),
                "mode": "whole amount at the reached tier"
            }));
        }
    } else {
        return Err(EngineError::invalid("Provide rate_percent or tiers"));
    }
    let accelerator = match quota {
        Some(q) if q > 0.0 && sales > q && accelerator_percent > 0.0 => {
            (sales - q) * accelerator_percent / 100.0
        },
        _ => 0.0,
    };
    let split = if split_percent > 0.0 && split_percent < 100.0 {
        split_percent / 100.0
    } else {
        1.0
    };
    let total_comm = (comm + accelerator) * split;
    Ok(json!({
        "sales": r2(sales),
        "commission": r2(comm * split),
        "accelerator": r2(accelerator * split),
        "total_commission": r2(total_comm),
        "base_salary": r2(base_salary.max(0.0)),
        "total_earnings": r2(base_salary.max(0.0) + total_comm),
        "effective_rate_percent": r2(total_comm / sales * 100.0),
        "quota_attainment_percent": quota.filter(|q| *q > 0.0).map(|q| r2(sales / q * 100.0)),
        "split_share_percent": split * 100.0,
        "tiers": rows
    }))
}

/// One employee for the payroll run.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Employee {
    /// Name.
    pub name: String,
    /// Monthly salary (salaried staff).
    pub gross_monthly: f64,
    /// Hourly rate (hourly staff).
    pub hourly_rate: f64,
    /// Regular hours worked.
    pub hours: f64,
    /// Overtime hours worked.
    pub overtime_hours: f64,
    /// Bonus for the period.
    pub bonus: f64,
    /// Employee-side deductions in percent (PF, insurance…).
    pub employee_deductions_percent: f64,
    /// Employer-side contributions in percent.
    pub employer_contributions_percent: f64,
    /// Fixed deductions.
    pub fixed_deductions: f64,
    /// Income tax withheld in percent.
    pub tax_percent: f64,
}

/// Payroll run: gross, deductions, net and employer cost per employee.
pub fn payroll(
    employees: &[Employee],
    overtime_multiplier: f64,
    default_employer_percent: f64,
    default_employee_percent: f64,
) -> EngineResult<Value> {
    if employees.is_empty() {
        return Err(EngineError::empty("employees"));
    }
    let otm = if overtime_multiplier <= 0.0 {
        1.5
    } else {
        overtime_multiplier
    };
    let mut totals = (0.0, 0.0, 0.0, 0.0, 0.0);
    let rows: Vec<Value> = employees
        .iter()
        .map(|e| {
            let gross = if e.gross_monthly > 0.0 {
                e.gross_monthly
            } else {
                e.hourly_rate * e.hours.max(0.0)
            } + e.hourly_rate * e.overtime_hours.max(0.0) * otm
                + e.bonus.max(0.0);
            let emp_pct = if e.employee_deductions_percent > 0.0 {
                e.employee_deductions_percent
            } else {
                default_employee_percent.max(0.0)
            };
            let er_pct = if e.employer_contributions_percent > 0.0 {
                e.employer_contributions_percent
            } else {
                default_employer_percent.max(0.0)
            };
            let deductions = gross * emp_pct / 100.0 + e.fixed_deductions.max(0.0);
            let tax = (gross - deductions).max(0.0) * e.tax_percent.max(0.0) / 100.0;
            let net = gross - deductions - tax;
            let employer = gross * er_pct / 100.0;
            totals.0 += gross;
            totals.1 += deductions;
            totals.2 += tax;
            totals.3 += net;
            totals.4 += employer;
            json!({
                "name": e.name,
                "gross": r2(gross),
                "deductions": r2(deductions),
                "tax": r2(tax),
                "net": r2(net),
                "employer_contributions": r2(employer),
                "cost_to_company": r2(gross + employer)
            })
        })
        .collect();
    Ok(json!({
        "employees": rows,
        "headcount": employees.len(),
        "totals": {
            "gross": r2(totals.0),
            "deductions": r2(totals.1),
            "tax": r2(totals.2),
            "net_payable": r2(totals.3),
            "employer_contributions": r2(totals.4),
            "total_payroll_cost": r2(totals.0 + totals.4)
        },
        "assumptions": {
            "overtime_multiplier": otm,
            "note": "Statutory rates vary by jurisdiction; pass percentages explicitly"
        }
    }))
}

/// One cash-flow period.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Period {
    /// Period label.
    pub label: String,
    /// Cash in.
    pub inflows: f64,
    /// Cash out.
    pub outflows: f64,
}

/// Cash-flow projection from explicit periods or recurring amounts with growth.
pub fn cash_flow(
    opening_balance: f64,
    periods: &[Period],
    months: u32,
    recurring_inflow: f64,
    recurring_outflow: f64,
    growth_percent: f64,
    minimum_balance: f64,
) -> EngineResult<Value> {
    let mut balance = opening_balance;
    let mut rows = Vec::new();
    let mut lowest = (opening_balance, 0usize);
    let sequence: Vec<Period> = if !periods.is_empty() {
        periods.to_vec()
    } else {
        if months == 0 || months > 120 {
            return Err(EngineError::invalid("Provide `periods` or `months` (1-120)"));
        }
        (0..months)
            .map(|i| Period {
                label: format!("Month {}", i + 1),
                inflows: recurring_inflow * (1.0 + growth_percent / 100.0).powi(i as i32),
                outflows: recurring_outflow,
            })
            .collect()
    };
    let mut first_negative: Option<String> = None;
    for (i, p) in sequence.iter().enumerate() {
        let net = p.inflows - p.outflows;
        balance += net;
        if balance < lowest.0 {
            lowest = (balance, i + 1);
        }
        if balance < minimum_balance && first_negative.is_none() {
            first_negative = Some(p.label.clone());
        }
        rows.push(json!({
            "period": p.label,
            "inflows": r2(p.inflows),
            "outflows": r2(p.outflows),
            "net": r2(net),
            "closing_balance": r2(balance),
            "below_minimum": balance < minimum_balance
        }));
    }
    let total_in: f64 = sequence.iter().map(|p| p.inflows).sum();
    let total_out: f64 = sequence.iter().map(|p| p.outflows).sum();
    let avg_burn = if total_out > total_in {
        (total_out - total_in) / sequence.len() as f64
    } else {
        0.0
    };
    Ok(json!({
        "opening_balance": r2(opening_balance),
        "closing_balance": r2(balance),
        "total_inflows": r2(total_in),
        "total_outflows": r2(total_out),
        "net_change": r2(total_in - total_out),
        "lowest_balance": { "amount": r2(lowest.0), "period_index": lowest.1 },
        "first_period_below_minimum": first_negative,
        "average_monthly_burn": r2(avg_burn),
        "runway_months": if avg_burn > 0.0 { json!(r2(opening_balance / avg_burn)) } else { json!("∞ (cash-flow positive)") },
        "periods": rows
    }))
}

/// Break-even units/revenue, margin of safety and operating leverage.
pub fn break_even(
    fixed_costs: f64,
    price_per_unit: f64,
    variable_cost_per_unit: f64,
    target_profit: f64,
    current_units: Option<f64>,
) -> EngineResult<Value> {
    positive(price_per_unit, "price_per_unit")?;
    let cm = price_per_unit - variable_cost_per_unit.max(0.0);
    if cm <= 0.0 {
        return Err(EngineError::invalid("Price must exceed the variable cost per unit"));
    }
    let units = fixed_costs.max(0.0) / cm;
    let revenue = units * price_per_unit;
    let margin_of_safety = current_units.filter(|u| *u > 0.0).map(|u| {
        json!({
            "units": r2(u - units),
            "percent": r2((u - units) / u * 100.0),
            "profit_at_current": r2(u * cm - fixed_costs)
        })
    });
    let sensitivity: Vec<Value> = [-10.0, -5.0, 5.0, 10.0]
        .iter()
        .map(|p| {
            let np = price_per_unit * (1.0 + p / 100.0);
            let ncm = np - variable_cost_per_unit;
            json!({
                "price_change_percent": p,
                "price": r2(np),
                "break_even_units": if ncm > 0.0 { json!((fixed_costs / ncm).ceil()) } else { Value::Null }
            })
        })
        .collect();
    Ok(json!({
        "break_even_units": units.ceil(),
        "break_even_revenue": r2(revenue),
        "contribution_margin": r2(cm),
        "contribution_margin_ratio_percent": r2(cm / price_per_unit * 100.0),
        "units_for_target_profit": ((fixed_costs.max(0.0) + target_profit.max(0.0)) / cm).ceil(),
        "operating_leverage_at_target": if target_profit > 0.0 { json!(r2((fixed_costs + target_profit) / target_profit)) } else { Value::Null },
        "margin_of_safety": margin_of_safety,
        "sensitivity": sensitivity
    }))
}

/// Business valuation by multiples, DCF and book value.
pub fn valuation(
    revenue: f64,
    ebitda: f64,
    net_profit: f64,
    revenue_multiple: f64,
    ebitda_multiple: f64,
    pe_multiple: f64,
    cash: f64,
    debt: f64,
    free_cash_flows: &[f64],
    discount_rate_percent: f64,
    terminal_growth_percent: f64,
    book_value: f64,
) -> EngineResult<Value> {
    if revenue <= 0.0
        && ebitda <= 0.0
        && net_profit <= 0.0
        && free_cash_flows.is_empty()
        && book_value <= 0.0
    {
        return Err(EngineError::invalid(
            "Provide revenue/ebitda/net_profit, free_cash_flows or book_value",
        ));
    }
    let mut methods = Vec::new();
    if revenue > 0.0 && revenue_multiple > 0.0 {
        methods.push(json!({
            "method": "revenue multiple",
            "enterprise_value": r2(revenue * revenue_multiple),
            "basis": format!("{revenue_multiple}× revenue")
        }));
    }
    if ebitda > 0.0 && ebitda_multiple > 0.0 {
        methods.push(json!({
            "method": "EBITDA multiple",
            "enterprise_value": r2(ebitda * ebitda_multiple),
            "basis": format!("{ebitda_multiple}× EBITDA")
        }));
    }
    if net_profit > 0.0 && pe_multiple > 0.0 {
        methods.push(json!({
            "method": "P/E multiple",
            "enterprise_value": r2(net_profit * pe_multiple + debt - cash),
            "basis": format!("{pe_multiple}× earnings (equity value adjusted to EV)")
        }));
    }
    if !free_cash_flows.is_empty() {
        let r = discount_rate_percent / 100.0;
        let g = terminal_growth_percent / 100.0;
        if r <= g {
            return Err(EngineError::invalid("discount rate must exceed terminal growth"));
        }
        let mut pv = 0.0;
        for (i, cf) in free_cash_flows.iter().enumerate() {
            pv += cf / (1.0 + r).powi(i as i32 + 1);
        }
        let last = free_cash_flows.last().copied().unwrap_or(0.0);
        let terminal = last * (1.0 + g) / (r - g);
        let tv_pv = terminal / (1.0 + r).powi(free_cash_flows.len() as i32);
        methods.push(json!({
            "method": "DCF",
            "enterprise_value": r2(pv + tv_pv),
            "pv_of_cash_flows": r2(pv),
            "terminal_value": r2(terminal),
            "pv_of_terminal": r2(tv_pv),
            "basis": format!("{discount_rate_percent}% discount, {terminal_growth_percent}% \
                              terminal growth")
        }));
    }
    if book_value > 0.0 {
        methods.push(json!({
            "method": "book value",
            "enterprise_value": r2(book_value + debt - cash),
            "basis": "net assets"
        }));
    }
    if methods.is_empty() {
        return Err(EngineError::invalid(
            "No valuation method could be applied; supply multiples or cash flows",
        ));
    }
    let evs: Vec<f64> = methods
        .iter()
        .filter_map(|m| m["enterprise_value"].as_f64())
        .collect();
    let avg = evs.iter().sum::<f64>() / evs.len() as f64;
    let min = evs.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = evs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    Ok(json!({
        "methods": methods,
        "enterprise_value": { "low": r2(min), "average": r2(avg), "high": r2(max) },
        "equity_value": {
            "low": r2(min - debt + cash),
            "average": r2(avg - debt + cash),
            "high": r2(max - debt + cash)
        },
        "net_debt": r2(debt - cash),
        "note": "Indicative only – multiples depend on sector, growth and market conditions"
    }))
}

/// Customer lifetime value, CAC payback and LTV:CAC ratio.
pub fn unit_economics(
    price: f64,
    variable_cost: f64,
    cac: f64,
    purchases_per_year: f64,
    retention_years: f64,
    churn_percent_monthly: Option<f64>,
    gross_margin_percent: Option<f64>,
    discount_rate_percent: f64,
) -> EngineResult<Value> {
    positive(price, "price")?;
    let margin = gross_margin_percent
        .map(|g| g / 100.0)
        .unwrap_or((price - variable_cost.max(0.0)) / price);
    if margin <= 0.0 {
        return Err(EngineError::invalid("Gross margin must be positive"));
    }
    let lifetime_years = match churn_percent_monthly {
        Some(c) if c > 0.0 => 1.0 / (c / 100.0) / 12.0,
        _ => {
            if retention_years > 0.0 {
                retention_years
            } else {
                3.0
            }
        },
    };
    let freq = if purchases_per_year <= 0.0 {
        12.0
    } else {
        purchases_per_year
    };
    let annual_margin = price * margin * freq;
    let ltv = annual_margin * lifetime_years;
    let ltv_discounted = if discount_rate_percent > 0.0 {
        (1..=lifetime_years.ceil() as i32)
            .map(|y| {
                annual_margin.min(annual_margin * (lifetime_years - (y - 1) as f64).min(1.0))
                    / (1.0 + discount_rate_percent / 100.0).powi(y)
            })
            .sum()
    } else {
        ltv
    };
    let ratio = if cac > 0.0 { ltv / cac } else { f64::INFINITY };
    let payback_months = if cac > 0.0 && annual_margin > 0.0 {
        cac / (annual_margin / 12.0)
    } else {
        0.0
    };
    Ok(json!({
        "contribution_per_order": r2(price * margin),
        "gross_margin_percent": r2(margin * 100.0),
        "orders_per_year": freq,
        "annual_contribution_per_customer": r2(annual_margin),
        "customer_lifetime_years": r2(lifetime_years),
        "ltv": r2(ltv),
        "ltv_discounted": r2(ltv_discounted),
        "cac": r2(cac),
        "ltv_to_cac": if ratio.is_finite() { json!(r2(ratio)) } else { json!("∞") },
        "cac_payback_months": r2(payback_months),
        "verdict": if !ratio.is_finite() || ratio >= 3.0 { "healthy (LTV:CAC ≥ 3)" } else if ratio >= 1.0 { "\
            marginal – reduce CAC or raise retention" } else { "\
            losing money on each customer" },
        "break_even_customers_for_cac": if cac > 0.0 { json!(r2(cac / (price * margin))) } else { Value::Null }
    }))
}

fn default_true() -> bool {
    true
}

/// Every action understood by the business engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum BusinessAction {
    /// Invoice / quotation / receipt calculator and renderer.
    Document {
        /// Document definition.
        #[serde(flatten)]
        doc: Document,
    },
    /// Profit & loss statement.
    ProfitLoss {
        /// Revenue.
        revenue: f64,
        /// Cost of goods sold.
        #[serde(default)]
        cogs: f64,
        /// Operating expenses by name.
        #[serde(default)]
        operating_expenses: serde_json::Map<String, Value>,
        /// Other income.
        #[serde(default)]
        other_income: f64,
        /// Interest expense.
        #[serde(default)]
        interest: f64,
        /// Depreciation & amortisation.
        #[serde(default)]
        depreciation: f64,
        /// Income tax rate in percent.
        #[serde(default)]
        tax_percent: f64,
    },
    /// Markup/margin calculator.
    MarkupMargin {
        /// Unit cost.
        #[serde(default)]
        cost: Option<f64>,
        /// Selling price.
        #[serde(default)]
        price: Option<f64>,
        /// Markup in percent.
        #[serde(default)]
        markup_percent: Option<f64>,
        /// Margin in percent.
        #[serde(default)]
        margin_percent: Option<f64>,
    },
    /// Inventory turnover.
    InventoryTurnover {
        /// Cost of goods sold for the period.
        cogs: f64,
        /// Opening inventory value.
        opening_inventory: f64,
        /// Closing inventory value.
        closing_inventory: f64,
        /// Period length in days.
        #[serde(default)]
        period_days: u32,
        /// Revenue (for GMROI).
        #[serde(default)]
        revenue: Option<f64>,
    },
    /// EOQ / reorder point.
    Reorder {
        /// Annual demand in units.
        annual_demand: f64,
        /// Cost per order.
        #[serde(default)]
        ordering_cost: f64,
        /// Annual holding cost per unit.
        #[serde(default)]
        holding_cost_per_unit: f64,
        /// Average lead time in days.
        #[serde(default)]
        lead_time_days: f64,
        /// Average daily demand (derived from annual demand when omitted).
        #[serde(default)]
        daily_demand: Option<f64>,
        /// Maximum daily demand (for safety stock).
        #[serde(default)]
        max_daily_demand: Option<f64>,
        /// Maximum lead time in days (for safety stock).
        #[serde(default)]
        max_lead_time_days: Option<f64>,
        /// Service-level z-score (1.65 ≈ 95 %).
        #[serde(default)]
        service_level_z: f64,
        /// Standard deviation of daily demand.
        #[serde(default)]
        demand_std_dev_daily: f64,
        /// Working days per year.
        #[serde(default)]
        working_days: f64,
    },
    /// Tiered / coupon pricing.
    DiscountPricing {
        /// List price per unit.
        list_price: f64,
        /// Quantity ordered.
        quantity: f64,
        /// Quantity discount tiers.
        #[serde(default)]
        tiers: Vec<Tier>,
        /// Coupon discount in percent.
        #[serde(default)]
        coupon_percent: f64,
        /// Coupon discount amount.
        #[serde(default)]
        coupon_amount: f64,
        /// Tax rate in percent.
        #[serde(default)]
        tax_percent: f64,
        /// Unit cost (for margin checks).
        #[serde(default)]
        cost: Option<f64>,
        /// Minimum acceptable margin in percent.
        #[serde(default)]
        min_margin_percent: Option<f64>,
    },
    /// Stacked sales tax.
    SalesTax {
        /// Taxable amount.
        amount: f64,
        /// Tax rates in percent (state, county, city…).
        rates: Vec<f64>,
        /// Labels matching `rates`.
        #[serde(default)]
        labels: Vec<String>,
        /// `amount` already includes tax.
        #[serde(default)]
        inclusive: bool,
        /// Tax-exempt portion of the amount.
        #[serde(default)]
        exempt_amount: f64,
    },
    /// Sales commission.
    Commission {
        /// Sales amount.
        sales: f64,
        /// Flat commission rate in percent.
        #[serde(default)]
        rate_percent: Option<f64>,
        /// Commission tiers (alternative to `rate_percent`).
        #[serde(default)]
        tiers: Vec<CommissionTier>,
        /// Base salary added to the payout.
        #[serde(default)]
        base_salary: f64,
        /// Apply tiers progressively (like tax brackets) instead of to the whole amount.
        #[serde(default = "default_true")]
        progressive: bool,
        /// Sales quota for the accelerator.
        #[serde(default)]
        quota: Option<f64>,
        /// Extra rate on sales above quota.
        #[serde(default)]
        accelerator_percent: f64,
        /// Share of the commission paid to this rep (team splits).
        #[serde(default)]
        split_percent: f64,
    },
    /// Payroll run.
    Payroll {
        /// Employees.
        employees: Vec<Employee>,
        /// Overtime multiplier (1.5 default).
        #[serde(default)]
        overtime_multiplier: f64,
        /// Employer contribution default in percent.
        #[serde(default)]
        default_employer_percent: f64,
        /// Employee deduction default in percent.
        #[serde(default)]
        default_employee_percent: f64,
    },
    /// Cash-flow projection.
    CashFlow {
        /// Starting cash balance.
        #[serde(default)]
        opening_balance: f64,
        /// Explicit periods (alternative to recurring amounts).
        #[serde(default)]
        periods: Vec<Period>,
        /// Number of months to project.
        #[serde(default)]
        months: u32,
        /// Recurring monthly inflow.
        #[serde(default)]
        recurring_inflow: f64,
        /// Recurring monthly outflow.
        #[serde(default)]
        recurring_outflow: f64,
        /// Monthly growth of inflows in percent.
        #[serde(default)]
        growth_percent: f64,
        /// Minimum balance to flag shortfalls.
        #[serde(default)]
        minimum_balance: f64,
    },
    /// Break-even analysis.
    BreakEven {
        /// Fixed costs.
        fixed_costs: f64,
        /// Selling price per unit.
        price_per_unit: f64,
        /// Variable cost per unit.
        variable_cost_per_unit: f64,
        /// Target profit.
        #[serde(default)]
        target_profit: f64,
        /// Current sales volume (margin of safety).
        #[serde(default)]
        current_units: Option<f64>,
    },
    /// Business valuation.
    Valuation {
        /// Annual revenue.
        #[serde(default)]
        revenue: f64,
        /// EBITDA.
        #[serde(default)]
        ebitda: f64,
        /// Net profit.
        #[serde(default)]
        net_profit: f64,
        /// Revenue multiple.
        #[serde(default)]
        revenue_multiple: f64,
        /// EBITDA multiple.
        #[serde(default)]
        ebitda_multiple: f64,
        /// Price/earnings multiple.
        #[serde(default)]
        pe_multiple: f64,
        /// Cash on hand.
        #[serde(default)]
        cash: f64,
        /// Debt.
        #[serde(default)]
        debt: f64,
        /// Projected free cash flows for the DCF.
        #[serde(default)]
        free_cash_flows: Vec<f64>,
        /// Discount rate in percent.
        #[serde(default)]
        discount_rate_percent: f64,
        /// Terminal growth rate in percent.
        #[serde(default)]
        terminal_growth_percent: f64,
        /// Book value of equity.
        #[serde(default)]
        book_value: f64,
    },
    /// Unit economics / LTV.
    UnitEconomics {
        /// Price per purchase.
        price: f64,
        /// Variable cost per purchase.
        #[serde(default)]
        variable_cost: f64,
        /// Customer acquisition cost.
        #[serde(default)]
        cac: f64,
        /// Purchases per customer per year.
        #[serde(default)]
        purchases_per_year: f64,
        /// Average customer lifetime in years.
        #[serde(default)]
        retention_years: f64,
        /// Monthly churn in percent (derives lifetime).
        #[serde(default)]
        churn_percent_monthly: Option<f64>,
        /// Gross margin in percent (derives variable cost).
        #[serde(default)]
        gross_margin_percent: Option<f64>,
        /// Discount rate for the discounted LTV.
        #[serde(default)]
        discount_rate_percent: f64,
    },
}

/// Dispatches a [`BusinessAction`] to its implementation.
pub fn handle_action(action: BusinessAction) -> EngineResult<Value> {
    match action {
        BusinessAction::Document { doc } => document(&doc),
        BusinessAction::ProfitLoss {
            revenue,
            cogs,
            operating_expenses,
            other_income,
            interest,
            depreciation,
            tax_percent,
        } => profit_loss(
            revenue,
            cogs,
            &operating_expenses,
            other_income,
            interest,
            depreciation,
            tax_percent,
        ),
        BusinessAction::MarkupMargin {
            cost,
            price,
            markup_percent,
            margin_percent,
        } => markup_margin(cost, price, markup_percent, margin_percent),
        BusinessAction::InventoryTurnover {
            cogs,
            opening_inventory,
            closing_inventory,
            period_days,
            revenue,
        } => inventory_turnover(cogs, opening_inventory, closing_inventory, period_days, revenue),
        BusinessAction::Reorder {
            annual_demand,
            ordering_cost,
            holding_cost_per_unit,
            lead_time_days,
            daily_demand,
            max_daily_demand,
            max_lead_time_days,
            service_level_z,
            demand_std_dev_daily,
            working_days,
        } => reorder(
            annual_demand,
            ordering_cost,
            holding_cost_per_unit,
            lead_time_days,
            daily_demand,
            max_daily_demand,
            max_lead_time_days,
            service_level_z,
            demand_std_dev_daily,
            working_days,
        ),
        BusinessAction::DiscountPricing {
            list_price,
            quantity,
            tiers,
            coupon_percent,
            coupon_amount,
            tax_percent,
            cost,
            min_margin_percent,
        } => discount_pricing(
            list_price,
            quantity,
            &tiers,
            coupon_percent,
            coupon_amount,
            tax_percent,
            cost,
            min_margin_percent,
        ),
        BusinessAction::SalesTax {
            amount,
            rates,
            labels,
            inclusive,
            exempt_amount,
        } => sales_tax(amount, &rates, &labels, inclusive, exempt_amount),
        BusinessAction::Commission {
            sales,
            rate_percent,
            tiers,
            base_salary,
            progressive,
            quota,
            accelerator_percent,
            split_percent,
        } => commission(
            sales,
            rate_percent,
            &tiers,
            base_salary,
            progressive,
            quota,
            accelerator_percent,
            split_percent,
        ),
        BusinessAction::Payroll {
            employees,
            overtime_multiplier,
            default_employer_percent,
            default_employee_percent,
        } => payroll(
            &employees,
            overtime_multiplier,
            default_employer_percent,
            default_employee_percent,
        ),
        BusinessAction::CashFlow {
            opening_balance,
            periods,
            months,
            recurring_inflow,
            recurring_outflow,
            growth_percent,
            minimum_balance,
        } => cash_flow(
            opening_balance,
            &periods,
            months,
            recurring_inflow,
            recurring_outflow,
            growth_percent,
            minimum_balance,
        ),
        BusinessAction::BreakEven {
            fixed_costs,
            price_per_unit,
            variable_cost_per_unit,
            target_profit,
            current_units,
        } => break_even(
            fixed_costs,
            price_per_unit,
            variable_cost_per_unit,
            target_profit,
            current_units,
        ),
        BusinessAction::Valuation {
            revenue,
            ebitda,
            net_profit,
            revenue_multiple,
            ebitda_multiple,
            pe_multiple,
            cash,
            debt,
            free_cash_flows,
            discount_rate_percent,
            terminal_growth_percent,
            book_value,
        } => valuation(
            revenue,
            ebitda,
            net_profit,
            revenue_multiple,
            ebitda_multiple,
            pe_multiple,
            cash,
            debt,
            &free_cash_flows,
            discount_rate_percent,
            terminal_growth_percent,
            book_value,
        ),
        BusinessAction::UnitEconomics {
            price,
            variable_cost,
            cac,
            purchases_per_year,
            retention_years,
            churn_percent_monthly,
            gross_margin_percent,
            discount_rate_percent,
        } => unit_economics(
            price,
            variable_cost,
            cac,
            purchases_per_year,
            retention_years,
            churn_percent_monthly,
            gross_margin_percent,
            discount_rate_percent,
        ),
    }
}

engine_shared::export_engine!(handle_action, BusinessAction);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents() {
        let d = Document {
            number: "INV-7".into(),
            date: "2024-01-31".into(),
            currency: "₹".into(),
            from: Party {
                name: "Acme".into(),
                ..Default::default()
            },
            to: Party {
                name: "Bob <b>".into(),
                ..Default::default()
            },
            items: vec![
                LineItem {
                    description: "Widget".into(),
                    quantity: 2.0,
                    unit_price: 100.0,
                    discount_percent: 10.0,
                    ..Default::default()
                },
                LineItem {
                    description: "Service".into(),
                    quantity: 1.0,
                    unit_price: 50.0,
                    tax_percent: Some(5.0),
                    ..Default::default()
                },
            ],
            tax_percent: 18.0,
            split_tax: true,
            shipping: 20.0,
            rounding: true,
            paid_amount: 100.0,
            ..Default::default()
        };
        let r = document(&d).unwrap();
        assert_eq!(r["totals"]["subtotal"], 230.0);
        assert_eq!(r["totals"]["tax"], 34.9); // 180*18% + 50*5%
        assert_eq!(r["totals"]["total"], 285.0); // 230+34.9+20 = 284.9 → rounded
        assert_eq!(r["totals"]["balance_due"], 185.0);
        assert_eq!(r["totals"]["tax_components"].as_array().unwrap().len(), 2);
        assert!(r["html"].as_str().unwrap().contains("Bob &lt;b&gt;"));
        let inc = document(&Document {
            items: vec![LineItem {
                description: "x".into(),
                quantity: 1.0,
                unit_price: 118.0,
                ..Default::default()
            }],
            tax_percent: 18.0,
            tax_mode: "inclusive".into(),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(inc["totals"]["subtotal"], 100.0);
        assert_eq!(inc["totals"]["total"], 118.0);
        assert!(document(&Document::default()).is_err());
        assert!(document(&Document {
            kind: "memo".into(),
            items: vec![LineItem { quantity: 1.0, ..Default::default() }],
            ..Default::default()
        })
        .is_err());
    }

    #[test]
    fn calculators() {
        let mut opex = serde_json::Map::new();
        opex.insert("rent".into(), json!(1000.0));
        opex.insert("salaries".into(), json!(5000.0));
        let pl = profit_loss(20_000.0, 8000.0, &opex, 500.0, 200.0, 300.0, 25.0).unwrap();
        assert_eq!(pl["gross_profit"], 12_000.0);
        assert_eq!(pl["ebitda"], 6500.0);
        assert_eq!(pl["net_profit"], 4500.0);
        assert_eq!(
            markup_margin(Some(80.0), None, Some(25.0), None).unwrap()["margin_percent"],
            20.0
        );
        assert_eq!(markup_margin(None, Some(100.0), None, Some(20.0)).unwrap()["cost"], 80.0);
        assert!(markup_margin(None, None, Some(1.0), None).is_err());
        let it = inventory_turnover(120_000.0, 20_000.0, 40_000.0, 365, Some(200_000.0)).unwrap();
        assert_eq!(it["turnover_ratio"], 4.0);
        assert_eq!(it["days_inventory_outstanding"], 91.25);
        let ro = reorder(12_000.0, 50.0, 2.0, 7.0, None, Some(40.0), Some(10.0), 0.0, 0.0, 365.0)
            .unwrap();
        assert_eq!(ro["eoq"], 775.0);
        assert!((ro["reorder_point"].as_f64().unwrap() - 400.0).abs() < 0.01);
        let dp = discount_pricing(
            100.0,
            50.0,
            &[
                Tier {
                    min_quantity: 10.0,
                    discount_percent: 5.0,
                },
                Tier {
                    min_quantity: 50.0,
                    discount_percent: 12.0,
                },
            ],
            0.0,
            100.0,
            10.0,
            Some(60.0),
            Some(25.0),
        )
        .unwrap();
        assert_eq!(dp["tier_discount_percent"], 12.0);
        assert_eq!(dp["net"], 4300.0);
        assert_eq!(dp["margin_ok"], true);
        let st =
            sales_tax(100.0, &[6.25, 1.0, 0.5], &["state".into(), "county".into()], false, 0.0)
                .unwrap();
        assert_eq!(st["total"], 107.75);
        assert_eq!(st["components"][2]["label"], "rate 3");
        let cm = commission(
            120_000.0,
            None,
            &[
                CommissionTier {
                    from: 0.0,
                    to: Some(50_000.0),
                    rate_percent: 5.0,
                },
                CommissionTier {
                    from: 50_000.0,
                    to: None,
                    rate_percent: 8.0,
                },
            ],
            3000.0,
            true,
            Some(100_000.0),
            2.0,
            0.0,
        )
        .unwrap();
        assert_eq!(cm["commission"], 8100.0);
        assert_eq!(cm["accelerator"], 400.0);
        assert_eq!(cm["total_earnings"], 11_500.0);
        assert!(commission(100.0, None, &[], 0.0, true, None, 0.0, 0.0).is_err());
    }

    #[test]
    fn payroll_cash_valuation() {
        let p = payroll(
            &[
                Employee {
                    name: "A".into(),
                    gross_monthly: 5000.0,
                    employee_deductions_percent: 12.0,
                    employer_contributions_percent: 13.0,
                    tax_percent: 10.0,
                    ..Default::default()
                },
                Employee {
                    name: "B".into(),
                    hourly_rate: 20.0,
                    hours: 160.0,
                    overtime_hours: 10.0,
                    ..Default::default()
                },
            ],
            1.5,
            0.0,
            0.0,
        )
        .unwrap();
        assert_eq!(p["employees"][0]["net"], 3960.0);
        assert_eq!(p["employees"][1]["gross"], 3500.0);
        assert_eq!(p["totals"]["total_payroll_cost"], 9150.0);
        let cf = cash_flow(10_000.0, &[], 6, 5000.0, 7000.0, 0.0, 0.0).unwrap();
        assert_eq!(cf["closing_balance"], -2000.0);
        assert_eq!(cf["first_period_below_minimum"], "Month 6");
        assert_eq!(cf["runway_months"], 5.0);
        assert!(cash_flow(0.0, &[], 0, 0.0, 0.0, 0.0, 0.0).is_err());
        let be = break_even(50_000.0, 25.0, 15.0, 10_000.0, Some(8000.0)).unwrap();
        assert_eq!(be["break_even_units"], 5000.0);
        assert_eq!(be["margin_of_safety"]["units"], 3000.0);
        let v = valuation(
            1_000_000.0,
            200_000.0,
            120_000.0,
            2.0,
            8.0,
            15.0,
            100_000.0,
            300_000.0,
            &[150_000.0, 170_000.0, 190_000.0],
            12.0,
            3.0,
            0.0,
        )
        .unwrap();
        assert_eq!(v["methods"].as_array().unwrap().len(), 4);
        assert!(v["enterprise_value"]["average"].as_f64().unwrap() > 1_000_000.0);
        assert!(valuation(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, &[], 0.0, 0.0, 0.0).is_err());
        let ue = unit_economics(50.0, 20.0, 100.0, 12.0, 0.0, Some(5.0), None, 0.0).unwrap();
        assert_eq!(ue["ltv"], 600.0);
        assert_eq!(ue["ltv_to_cac"], 6.0);
        assert!(ue["verdict"].as_str().unwrap().starts_with("healthy"));
    }
}
