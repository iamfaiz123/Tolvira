//! Dependency-free SVG chart renderer.

use engine_shared::{EngineError, EngineResult};
use serde::Deserialize;
use serde_json::{json, Value};

/// Default categorical colour palette.
pub const PALETTE: [&str; 10] = [
    "#3b82f6", "#ef4444", "#10b981", "#f59e0b", "#8b5cf6", "#ec4899", "#14b8a6", "#f97316",
    "#6366f1", "#84cc16",
];

/// Rendering options shared by every chart.
#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct ChartOptions {
    /// SVG width in px.
    pub width: u32,
    /// SVG height in px.
    pub height: u32,
    /// Chart title.
    pub title: String,
    /// X-axis label.
    pub x_label: String,
    /// Y-axis label.
    pub y_label: String,
    /// Series colours (palette when empty).
    pub colors: Vec<String>,
    /// Draw the legend.
    pub show_legend: bool,
    /// Print values on bars/points.
    pub show_values: bool,
    /// Draw grid lines.
    pub show_grid: bool,
    /// Stack series (bar/area).
    pub stacked: bool,
    /// Smooth lines (Catmull-Rom).
    pub smooth: bool,
    /// Render pies as donuts.
    pub donut: bool,
    /// Background colour.
    pub background: String,
    /// Font family.
    pub font_family: String,
    /// Fixed y-axis minimum.
    pub y_min: Option<f64>,
    /// Fixed y-axis maximum.
    pub y_max: Option<f64>,
    /// Maximum categories drawn (rest grouped).
    pub max_categories: usize,
}

impl Default for ChartOptions {
    fn default() -> Self {
        Self {
            width: 800,
            height: 450,
            title: String::new(),
            x_label: String::new(),
            y_label: String::new(),
            colors: vec![],
            show_legend: true,
            show_values: false,
            show_grid: true,
            stacked: false,
            smooth: false,
            donut: false,
            background: "#ffffff".into(),
            font_family: "Inter, Helvetica, Arial, sans-serif".into(),
            y_min: None,
            y_max: None,
            max_categories: 60,
        }
    }
}

/// One series of numeric values aligned with `labels`.
pub struct Series {
    /// Series name (legend).
    pub name: String,
    /// Values aligned with `labels` (`None` = missing).
    pub values: Vec<Option<f64>>,
}

/// Prepared chart data.
pub struct ChartData {
    /// Category labels.
    pub labels: Vec<String>,
    /// Series.
    pub series: Vec<Series>,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn fmt(v: f64) -> String {
    if v.abs() >= 1e6 {
        format!("{:.1}M", v / 1e6)
    } else if v.abs() >= 1e4 {
        format!("{:.1}k", v / 1e3)
    } else if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else if v.abs() < 1.0 {
        format!("{v:.3}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    } else {
        format!("{v:.2}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

/// "Nice" axis ticks (1/2/5 × 10^n).
fn nice_ticks(min: f64, max: f64, target: usize) -> (f64, f64, Vec<f64>) {
    let range = (max - min).abs().max(1e-9);
    let raw = range / target.max(1) as f64;
    let mag = 10f64.powf(raw.log10().floor());
    let norm = raw / mag;
    let step = if norm < 1.5 {
        1.0
    } else if norm < 3.0 {
        2.0
    } else if norm < 7.0 {
        5.0
    } else {
        10.0
    } * mag;
    let lo = (min / step).floor() * step;
    let hi = (max / step).ceil() * step;
    let mut ticks = Vec::new();
    let mut t = lo;
    while t <= hi + step * 0.5 && ticks.len() < 50 {
        ticks.push((t * 1e9).round() / 1e9);
        t += step;
    }
    (lo, hi, ticks)
}

fn color(o: &ChartOptions, i: usize) -> String {
    o.colors
        .get(i)
        .cloned()
        .unwrap_or_else(|| PALETTE[i % PALETTE.len()].to_string())
}

/// Renders a chart of `kind` (`bar`, `hbar`, `line`, `area`, `scatter`, `pie`, `donut`) as SVG.
pub fn render(kind: &str, data: &ChartData, o: &ChartOptions) -> EngineResult<Value> {
    if data.labels.is_empty() || data.series.is_empty() {
        return Err(EngineError::empty("data"));
    }
    if data.labels.len() > o.max_categories.max(1) && !matches!(kind, "line" | "area" | "scatter") {
        return Err(EngineError::limit(format!(
            "{} categories exceed max_categories ({}); aggregate first or raise the limit",
            data.labels.len(),
            o.max_categories
        )));
    }
    if data.labels.len() > 5000 {
        return Err(EngineError::limit("At most 5000 points per chart; sample or aggregate first"));
    }
    let w = o.width.clamp(200, 4000) as f64;
    let h = o.height.clamp(150, 4000) as f64;
    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" font-family="{}" font-size="12">"##,
        esc(&o.font_family)
    );
    if !o.background.is_empty() && o.background != "transparent" {
        svg.push_str(&format!(
            r##"<rect width="100%" height="100%" fill="{}"/>"##,
            esc(&o.background)
        ));
    }
    if !o.title.is_empty() {
        svg.push_str(&format!(r##"<text x="{}" y="24" text-anchor="middle" font-size="16" font-weight="600" fill="#111">{}</text>"##, w / 2.0, esc(&o.title)));
    }
    let body = match kind {
        "bar" | "column" => cartesian(data, o, w, h, Kind::Bar, &mut svg),
        "hbar" | "horizontal_bar" => hbar(data, o, w, h, &mut svg),
        "line" => cartesian(data, o, w, h, Kind::Line, &mut svg),
        "area" => cartesian(data, o, w, h, Kind::Area, &mut svg),
        "scatter" => cartesian(data, o, w, h, Kind::Scatter, &mut svg),
        "pie" | "donut" => pie(data, o, w, h, kind == "donut" || o.donut, &mut svg),
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown chart type `{other}` (bar, hbar, line, area, scatter, pie, donut)"
            )))
        },
    }?;
    svg.push_str("</svg>");
    Ok(json!({
        "svg": svg,
        "type": kind,
        "width": w,
        "height": h,
        "series": data.series.len(),
        "points": data.labels.len(),
        "meta": body
    }))
}

#[derive(PartialEq, Clone, Copy)]
enum Kind {
    Bar,
    Line,
    Area,
    Scatter,
}

fn cartesian(
    data: &ChartData,
    o: &ChartOptions,
    w: f64,
    h: f64,
    kind: Kind,
    svg: &mut String,
) -> EngineResult<Value> {
    let top = if o.title.is_empty() { 20.0 } else { 40.0 };
    let legend_h = if o.show_legend && data.series.len() > 1 {
        24.0
    } else {
        0.0
    };
    let bottom = 50.0 + if o.x_label.is_empty() { 0.0 } else { 18.0 } + legend_h;
    let left = 60.0 + if o.y_label.is_empty() { 0.0 } else { 18.0 };
    let right = 20.0;
    let pw = (w - left - right).max(10.0);
    let ph = (h - top - bottom).max(10.0);
    let n = data.labels.len();
    // Value range.
    let mut vmin = f64::MAX;
    let mut vmax = f64::MIN;
    if o.stacked && kind != Kind::Scatter {
        for i in 0..n {
            let (mut pos, mut neg) = (0.0, 0.0);
            for s in &data.series {
                if let Some(Some(v)) = s.values.get(i) {
                    if *v >= 0.0 {
                        pos += v
                    } else {
                        neg += v
                    }
                }
            }
            vmax = vmax.max(pos);
            vmin = vmin.min(neg);
        }
    } else {
        for s in &data.series {
            for v in s.values.iter().flatten() {
                vmin = vmin.min(*v);
                vmax = vmax.max(*v);
            }
        }
    }
    if vmin == f64::MAX {
        return Err(EngineError::invalid("No numeric values to plot"));
    }
    if kind == Kind::Bar || kind == Kind::Area {
        vmin = vmin.min(0.0);
        vmax = vmax.max(0.0);
    }
    if let Some(m) = o.y_min {
        vmin = m;
    }
    if let Some(m) = o.y_max {
        vmax = m;
    }
    if (vmax - vmin).abs() < 1e-12 {
        vmax = vmin + 1.0;
    }
    let (lo, hi, ticks) = nice_ticks(vmin, vmax, 6);
    let y_of = |v: f64| top + ph - (v - lo) / (hi - lo) * ph;
    // Grid and axes.
    for t in &ticks {
        let y = y_of(*t);
        if o.show_grid {
            svg.push_str(&format!(r##"<line x1="{left}" y1="{y:.1}" x2="{:.1}" y2="{y:.1}" stroke="#e5e7eb" stroke-width="1"/>"##, left + pw));
        }
        svg.push_str(&format!(
            r##"<text x="{:.1}" y="{:.1}" text-anchor="end" fill="#6b7280">{}</text>"##,
            left - 8.0,
            y + 4.0,
            fmt(*t)
        ));
    }
    svg.push_str(&format!(
        r##"<line x1="{left}" y1="{top}" x2="{left}" y2="{:.1}" stroke="#9ca3af"/>"##,
        top + ph
    ));
    let zero_y = y_of(0.0).max(top).min(top + ph.max(0.0));
    svg.push_str(&format!(
        r##"<line x1="{left}" y1="{zero_y:.1}" x2="{:.1}" y2="{zero_y:.1}" stroke="#9ca3af"/>"##,
        left + pw
    ));
    // X labels (thin out when crowded).
    let step = (n as f64 / (pw / 60.0)).ceil().max(1.0) as usize;
    let slot = pw / n as f64;
    let x_center = |i: usize| left + slot * (i as f64 + 0.5);
    for (i, l) in data.labels.iter().enumerate() {
        if i % step == 0 || i == n - 1 && n <= 12 {
            let label: String = l.chars().take(14).collect();
            let rotate = n > 12 || label.len() > 8;
            if rotate {
                svg.push_str(&format!(r##"<text x="{:.1}" y="{:.1}" text-anchor="end" fill="#6b7280" transform="rotate(-35 {:.1} {:.1})">{}</text>"##, x_center(i), top + ph + 14.0, x_center(i), top + ph + 14.0, esc(&label)));
            } else {
                svg.push_str(&format!(
                    r##"<text x="{:.1}" y="{:.1}" text-anchor="middle" fill="#6b7280">{}</text>"##,
                    x_center(i),
                    top + ph + 16.0,
                    esc(&label)
                ));
            }
        }
    }
    if !o.x_label.is_empty() {
        svg.push_str(&format!(r##"<text x="{:.1}" y="{:.1}" text-anchor="middle" fill="#374151" font-weight="600">{}</text>"##, left + pw / 2.0, h - legend_h - 8.0, esc(&o.x_label)));
    }
    if !o.y_label.is_empty() {
        svg.push_str(&format!(r##"<text x="14" y="{:.1}" text-anchor="middle" fill="#374151" font-weight="600" transform="rotate(-90 14 {:.1})">{}</text>"##, top + ph / 2.0, top + ph / 2.0, esc(&o.y_label)));
    }
    // Series.
    let sn = data.series.len();
    let mut stack_pos = vec![0.0f64; n];
    let mut stack_neg = vec![0.0f64; n];
    for (si, s) in data.series.iter().enumerate() {
        let c = color(o, si);
        match kind {
            Kind::Bar => {
                let group_w = slot * 0.8;
                let bar_w = if o.stacked { group_w } else { group_w / sn as f64 };
                for (i, v) in s.values.iter().enumerate().take(n) {
                    let Some(v) = v else { continue };
                    let (base, topv) = if o.stacked {
                        if *v >= 0.0 {
                            let b = stack_pos[i];
                            stack_pos[i] += v;
                            (b, b + v)
                        } else {
                            let b = stack_neg[i];
                            stack_neg[i] += v;
                            (b + v, b)
                        }
                    } else {
                        (0.0f64.min(*v), 0.0f64.max(*v))
                    };
                    let x = if o.stacked {
                        x_center(i) - group_w / 2.0
                    } else {
                        x_center(i) - group_w / 2.0 + bar_w * si as f64
                    };
                    let (y1, y2) = (y_of(topv), y_of(base));
                    svg.push_str(&format!(r##"<rect x="{x:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="{c}" rx="2"><title>{}: {} = {}</title></rect>"##, y1.min(y2), bar_w.max(1.0), (y2 - y1).abs().max(0.5), esc(&data.labels[i]), esc(&s.name), fmt(*v)));
                    if o.show_values && n * sn <= 60 {
                        svg.push_str(&format!(r##"<text x="{:.1}" y="{:.1}" text-anchor="middle" font-size="10" fill="#374151">{}</text>"##, x + bar_w / 2.0, y1.min(y2) - 3.0, fmt(*v)));
                    }
                }
            },
            Kind::Line | Kind::Area => {
                let mut pts: Vec<(f64, f64, f64)> = Vec::new();
                for (i, v) in s.values.iter().enumerate().take(n) {
                    if let Some(v) = v {
                        let yv = if o.stacked {
                            stack_pos[i] += v;
                            stack_pos[i]
                        } else {
                            *v
                        };
                        pts.push((x_center(i), y_of(yv), *v));
                    }
                }
                if pts.is_empty() {
                    continue;
                }
                let path = if o.smooth && pts.len() > 2 {
                    smooth_path(&pts)
                } else {
                    pts.iter()
                        .enumerate()
                        .map(|(k, p)| {
                            format!("{}{:.1} {:.1}", if k == 0 { "M" } else { "L" }, p.0, p.1)
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                };
                if kind == Kind::Area {
                    svg.push_str(&format!(r##"<path d="{path} L{:.1} {zero_y:.1} L{:.1} {zero_y:.1} Z" fill="{c}" fill-opacity="0.25"/>"##, pts[pts.len() - 1].0, pts[0].0));
                }
                svg.push_str(&format!(r##"<path d="{path}" fill="none" stroke="{c}" stroke-width="2.2" stroke-linejoin="round" stroke-linecap="round"/>"##));
                if pts.len() <= 120 {
                    for (i, p) in pts.iter().enumerate() {
                        svg.push_str(&format!(r##"<circle cx="{:.1}" cy="{:.1}" r="3" fill="{c}"><title>{}: {} = {}</title></circle>"##, p.0, p.1, esc(data.labels.get(i).map(String::as_str).unwrap_or("")), esc(&s.name), fmt(p.2)));
                        if o.show_values && pts.len() <= 40 {
                            svg.push_str(&format!(r##"<text x="{:.1}" y="{:.1}" text-anchor="middle" font-size="10" fill="#374151">{}</text>"##, p.0, p.1 - 7.0, fmt(p.2)));
                        }
                    }
                }
            },
            Kind::Scatter => {
                // Labels are x values for scatter.
                let xs: Vec<Option<f64>> =
                    data.labels.iter().map(|l| l.parse::<f64>().ok()).collect();
                let xmin = xs.iter().flatten().cloned().fold(f64::MAX, f64::min);
                let xmax = xs.iter().flatten().cloned().fold(f64::MIN, f64::max);
                if xmin == f64::MAX {
                    return Err(EngineError::invalid("Scatter charts need numeric x values"));
                }
                let (xlo, xhi, xticks) = nice_ticks(xmin, xmax, 6);
                let x_of = |x: f64| left + (x - xlo) / (xhi - xlo).max(1e-9) * pw;
                if si == 0 {
                    for t in &xticks {
                        svg.push_str(&format!(r##"<text x="{:.1}" y="{:.1}" text-anchor="middle" fill="#6b7280">{}</text>"##, x_of(*t), top + ph + 30.0, fmt(*t)));
                    }
                }
                let mut sum_x = 0.0;
                let mut sum_y = 0.0;
                let mut sum_xy = 0.0;
                let mut sum_x2 = 0.0;
                let mut count = 0.0;
                for (i, v) in s.values.iter().enumerate().take(n) {
                    if let (Some(x), Some(y)) = (xs.get(i).copied().flatten(), v) {
                        svg.push_str(&format!(r##"<circle cx="{:.1}" cy="{:.1}" r="4" fill="{c}" fill-opacity="0.75"><title>{} = ({}, {})</title></circle>"##, x_of(x), y_of(*y), esc(&s.name), fmt(x), fmt(*y)));
                        sum_x += x;
                        sum_y += y;
                        sum_xy += x * y;
                        sum_x2 += x * x;
                        count += 1.0;
                    }
                }
                if count >= 2.0 {
                    let denom = count * sum_x2 - sum_x * sum_x;
                    if denom.abs() > 1e-12 {
                        let slope = (count * sum_xy - sum_x * sum_y) / denom;
                        let intercept = (sum_y - slope * sum_x) / count;
                        svg.push_str(&format!(r##"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="{c}" stroke-dasharray="4 4" stroke-width="1.5"/>"##, x_of(xlo), y_of(slope * xlo + intercept).max(top).min(top + ph.max(0.0)), x_of(xhi), y_of(slope * xhi + intercept).max(top).min(top + ph.max(0.0))));
                    }
                }
            },
        }
    }
    if o.show_legend && sn > 1 {
        let mut x = left;
        let y = h - 8.0;
        for (si, s) in data.series.iter().enumerate() {
            svg.push_str(&format!(r##"<rect x="{x:.1}" y="{:.1}" width="12" height="12" fill="{}" rx="2"/><text x="{:.1}" y="{y:.1}" fill="#374151">{}</text>"##, y - 10.0, color(o, si), x + 16.0, esc(&s.name)));
            x += 16.0 + 7.0 * s.name.chars().count() as f64 + 20.0;
        }
    }
    Ok(json!({
        "y_range": [lo, hi],
        "ticks": ticks,
        "plot_area": { "x": left, "y": top, "width": pw, "height": ph }
    }))
}

fn smooth_path(pts: &[(f64, f64, f64)]) -> String {
    let mut d = format!("M{:.1} {:.1}", pts[0].0, pts[0].1);
    for i in 0..pts.len() - 1 {
        let p0 = if i == 0 { pts[0] } else { pts[i - 1] };
        let p1 = pts[i];
        let p2 = pts[i + 1];
        let p3 = if i + 2 < pts.len() { pts[i + 2] } else { p2 };
        let c1 = (p1.0 + (p2.0 - p0.0) / 6.0, p1.1 + (p2.1 - p0.1) / 6.0);
        let c2 = (p2.0 - (p3.0 - p1.0) / 6.0, p2.1 - (p3.1 - p1.1) / 6.0);
        d.push_str(&format!(
            " C{:.1} {:.1}, {:.1} {:.1}, {:.1} {:.1}",
            c1.0, c1.1, c2.0, c2.1, p2.0, p2.1
        ));
    }
    d
}

fn hbar(
    data: &ChartData,
    o: &ChartOptions,
    w: f64,
    h: f64,
    svg: &mut String,
) -> EngineResult<Value> {
    let top = if o.title.is_empty() { 20.0 } else { 40.0 };
    let label_w = (data
        .labels
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(4)
        .min(24) as f64
        * 7.0
        + 16.0)
        .max(60.0);
    let left = label_w;
    let pw = (w - left - 30.0).max(10.0);
    let ph = (h - top - 30.0).max(10.0);
    let n = data.labels.len();
    let s = &data.series[0];
    let vmax = s.values.iter().flatten().cloned().fold(0.0f64, f64::max);
    let vmin = s.values.iter().flatten().cloned().fold(0.0f64, f64::min);
    let (lo, hi, ticks) = nice_ticks(vmin, vmax, 5);
    let x_of = |v: f64| left + (v - lo) / (hi - lo).max(1e-9) * pw;
    for t in &ticks {
        let x = x_of(*t);
        if o.show_grid {
            svg.push_str(&format!(
                r##"<line x1="{x:.1}" y1="{top}" x2="{x:.1}" y2="{:.1}" stroke="#e5e7eb"/>"##,
                top + ph
            ));
        }
        svg.push_str(&format!(
            r##"<text x="{x:.1}" y="{:.1}" text-anchor="middle" fill="#6b7280">{}</text>"##,
            top + ph + 16.0,
            fmt(*t)
        ));
    }
    let slot = ph / n as f64;
    for (i, l) in data.labels.iter().enumerate() {
        let y = top + slot * i as f64;
        let label: String = l.chars().take(24).collect();
        svg.push_str(&format!(
            r##"<text x="{:.1}" y="{:.1}" text-anchor="end" fill="#374151">{}</text>"##,
            left - 8.0,
            y + slot / 2.0 + 4.0,
            esc(&label)
        ));
        if let Some(Some(v)) = s.values.get(i) {
            let (x1, x2) = (x_of(0.0f64.min(*v)), x_of(0.0f64.max(*v)));
            svg.push_str(&format!(r##"<rect x="{x1:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="{}" rx="2"><title>{}: {}</title></rect>"##, y + slot * 0.15, (x2 - x1).max(0.5), slot * 0.7, color(o, 0), esc(l), fmt(*v)));
            if o.show_values {
                svg.push_str(&format!(
                    r##"<text x="{:.1}" y="{:.1}" font-size="10" fill="#374151">{}</text>"##,
                    x2 + 4.0,
                    y + slot / 2.0 + 4.0,
                    fmt(*v)
                ));
            }
        }
    }
    Ok(json!({ "x_range": [lo, hi] }))
}

fn pie(
    data: &ChartData,
    o: &ChartOptions,
    w: f64,
    h: f64,
    donut: bool,
    svg: &mut String,
) -> EngineResult<Value> {
    let s = &data.series[0];
    let vals: Vec<(String, f64)> = data
        .labels
        .iter()
        .zip(&s.values)
        .filter_map(|(l, v)| v.filter(|x| *x > 0.0).map(|x| (l.clone(), x)))
        .collect();
    if vals.is_empty() {
        return Err(EngineError::invalid("Pie charts need positive values"));
    }
    let total: f64 = vals.iter().map(|v| v.1).sum();
    let top = if o.title.is_empty() { 10.0 } else { 36.0 };
    let legend_w = if o.show_legend { 180.0 } else { 0.0 };
    let r = ((w - legend_w - 40.0) / 2.0).min((h - top - 20.0) / 2.0).max(20.0);
    let (cx, cy) = ((w - legend_w) / 2.0, top + (h - top) / 2.0);
    let mut angle = -std::f64::consts::FRAC_PI_2;
    let mut slices = Vec::new();
    for (i, (label, v)) in vals.iter().enumerate() {
        let sweep = v / total * std::f64::consts::TAU;
        let (a0, a1) = (angle, angle + sweep);
        let (x0, y0) = (cx + r * a0.cos(), cy + r * a0.sin());
        let (x1, y1) = (cx + r * a1.cos(), cy + r * a1.sin());
        let large = if sweep > std::f64::consts::PI { 1 } else { 0 };
        let c = color(o, i);
        let path = if vals.len() == 1 {
            format!(
                "M{:.2} {:.2} A{r} {r} 0 1 1 {:.2} {:.2} A{r} {r} 0 1 1 {:.2} {:.2}",
                cx + r,
                cy,
                cx - r,
                cy,
                cx + r,
                cy
            )
        } else if donut {
            let ir = r * 0.55;
            let (ix0, iy0) = (cx + ir * a0.cos(), cy + ir * a0.sin());
            let (ix1, iy1) = (cx + ir * a1.cos(), cy + ir * a1.sin());
            format!(
                "M{x0:.2} {y0:.2} A{r} {r} 0 {large} 1 {x1:.2} {y1:.2} L{ix1:.2} {iy1:.2} A{ir} \
                 {ir} 0 {large} 0 {ix0:.2} {iy0:.2} Z"
            )
        } else {
            format!("M{cx} {cy} L{x0:.2} {y0:.2} A{r} {r} 0 {large} 1 {x1:.2} {y1:.2} Z")
        };
        let pct = v / total * 100.0;
        svg.push_str(&format!(r##"<path d="{path}" fill="{c}" stroke="#fff" stroke-width="1.5"><title>{}: {} ({:.1}%)</title></path>"##, esc(label), fmt(*v), pct));
        if pct >= 4.0 && (o.show_values || vals.len() <= 8) {
            let mid = (a0 + a1) / 2.0;
            let lr = if donut { r * 0.78 } else { r * 0.65 };
            svg.push_str(&format!(r##"<text x="{:.1}" y="{:.1}" text-anchor="middle" font-size="11" fill="#fff" font-weight="600">{:.0}%</text>"##, cx + lr * mid.cos(), cy + lr * mid.sin() + 4.0, pct));
        }
        slices.push(json!({
            "label": label,
            "value": v,
            "percent": (pct * 100.0).round() / 100.0,
            "color": c
        }));
        angle = a1;
    }
    if donut {
        svg.push_str(&format!(r##"<text x="{cx:.1}" y="{:.1}" text-anchor="middle" font-size="18" font-weight="600" fill="#111">{}</text>"##, cy + 6.0, fmt(total)));
    }
    if o.show_legend {
        let lx = w - legend_w + 10.0;
        for (i, sl) in slices.iter().take(20).enumerate() {
            let y = top + 10.0 + i as f64 * 20.0;
            let label: String = sl["label"].as_str().unwrap_or("").chars().take(18).collect();
            svg.push_str(&format!(r##"<rect x="{lx:.1}" y="{y:.1}" width="12" height="12" fill="{}" rx="2"/><text x="{:.1}" y="{:.1}" fill="#374151">{} ({}%)</text>"##, sl["color"].as_str().unwrap_or("#888"), lx + 18.0, y + 10.0, esc(&label), sl["percent"].as_f64().unwrap_or(0.0).round()));
        }
    }
    Ok(json!({ "total": total, "slices": slices }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> ChartData {
        ChartData {
            labels: vec!["Q1".into(), "Q2".into(), "Q3".into()],
            series: vec![
                Series {
                    name: "Sales".into(),
                    values: vec![Some(10.0), Some(25.5), Some(-4.0)],
                },
                Series {
                    name: "Cost".into(),
                    values: vec![Some(5.0), None, Some(8.0)],
                },
            ],
        }
    }

    #[test]
    fn renders_all_kinds() {
        for kind in ["bar", "hbar", "line", "area", "pie", "donut"] {
            let v = render(
                kind,
                &data(),
                &ChartOptions {
                    title: "T".into(),
                    show_values: true,
                    stacked: kind == "bar",
                    ..Default::default()
                },
            )
            .unwrap();
            let svg = v["svg"].as_str().unwrap();
            assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{kind}");
            assert!(svg.contains("Sales") || matches!(kind, "hbar" | "pie" | "donut"), "{kind}");
        }
        let scatter = ChartData {
            labels: vec!["1".into(), "2".into(), "3".into(), "4".into()],
            series: vec![Series {
                name: "y".into(),
                values: vec![Some(2.0), Some(4.1), Some(5.9), Some(8.2)],
            }],
        };
        let v = render("scatter", &scatter, &ChartOptions::default()).unwrap();
        assert!(v["svg"].as_str().unwrap().contains("stroke-dasharray"));
        assert!(render("sunburst", &data(), &ChartOptions::default()).is_err());
        assert!(render(
            "bar",
            &ChartData { labels: vec![], series: vec![] },
            &ChartOptions::default()
        )
        .is_err());
        let (lo, hi, ticks) = nice_ticks(0.0, 97.0, 5);
        assert_eq!((lo, hi), (0.0, 100.0));
        assert_eq!(ticks.len(), 6);
        assert_eq!(fmt(1234567.0), "1.2M");
        assert_eq!(fmt(0.25), "0.25");
    }
}
