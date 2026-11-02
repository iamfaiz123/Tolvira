//! Cron expression parser (field breakdown, English description, validation,
//! next runs in UTC from a caller supplied instant) and generator.

use chrono::{DateTime, TimeZone, Utc};
use engine_shared::{EngineError, EngineResult};
use serde::Deserialize;
use serde_json::{json, Value};
use std::str::FromStr;

const DOW_NAMES: [&str; 7] =
    ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MON_NAMES: [&str; 13] = [
    "",
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const DOW_ALIASES: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];
const MON_ALIASES: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

struct FieldSpec {
    name: &'static str,
    min: u32,
    max: u32,
    names: Option<&'static [&'static str]>,
}

const FIELDS: [FieldSpec; 6] = [
    FieldSpec {
        name: "second",
        min: 0,
        max: 59,
        names: None,
    },
    FieldSpec {
        name: "minute",
        min: 0,
        max: 59,
        names: None,
    },
    FieldSpec {
        name: "hour",
        min: 0,
        max: 23,
        names: None,
    },
    FieldSpec {
        name: "day-of-month",
        min: 1,
        max: 31,
        names: None,
    },
    FieldSpec {
        name: "month",
        min: 1,
        max: 12,
        names: Some(&MON_ALIASES),
    },
    FieldSpec {
        name: "day-of-week",
        min: 0,
        max: 7,
        names: Some(&DOW_ALIASES),
    },
];

fn expand_macro(expr: &str) -> Option<&'static str> {
    Some(match expr.trim().to_ascii_lowercase().as_str() {
        "@yearly" | "@annually" => "0 0 1 1 *",
        "@monthly" => "0 0 1 * *",
        "@weekly" => "0 0 * * 0",
        "@daily" | "@midnight" => "0 0 * * *",
        "@hourly" => "0 * * * *",
        "@minutely" => "* * * * *",
        _ => return None,
    })
}

/// Resolves a single token (number or alias) to a number for the field.
fn token_value(tok: &str, spec: &FieldSpec) -> Result<u32, String> {
    if let Ok(n) = tok.parse::<u32>() {
        return Ok(n);
    }
    if let Some(names) = spec.names {
        let up = tok.to_ascii_uppercase();
        if let Some(i) = names.iter().position(|n| *n == up) {
            return Ok(i as u32 + if spec.name == "month" { 1 } else { 0 });
        }
    }
    Err(format!("`{tok}` is not a valid {} value", spec.name))
}

/// Validates one field and expands it into the set of values it selects.
fn expand_field(field: &str, spec: &FieldSpec) -> Result<Vec<u32>, String> {
    let mut values = Vec::new();
    if field.is_empty() {
        return Err(format!("Empty {} field", spec.name));
    }
    for part in field.split(',') {
        let (range, step) = match part.split_once('/') {
            Some((r, s)) => (
                r,
                Some(
                    s.parse::<u32>()
                        .map_err(|_| format!("Invalid step `{s}` in {} field", spec.name))?,
                ),
            ),
            None => (part, None),
        };
        if step == Some(0) {
            return Err(format!("Step must be > 0 in {} field", spec.name));
        }
        let (lo, hi) = if range == "*" || range == "?" {
            (spec.min, if spec.name == "day-of-week" { 6 } else { spec.max })
        } else if let Some((a, b)) = range.split_once('-') {
            (token_value(a, spec)?, token_value(b, spec)?)
        } else {
            let v = token_value(range, spec)?;
            (
                v,
                if step.is_some() {
                    if spec.name == "day-of-week" {
                        6
                    } else {
                        spec.max
                    }
                } else {
                    v
                },
            )
        };
        if lo < spec.min || hi > spec.max {
            return Err(format!(
                "{} value out of range ({}-{}): `{part}`",
                spec.name, spec.min, spec.max
            ));
        }
        if lo > hi {
            return Err(format!("Range start is after its end in {} field: `{part}`", spec.name));
        }
        let mut v = lo;
        while v <= hi {
            values.push(if spec.name == "day-of-week" && v == 7 { 0 } else { v });
            v += step.unwrap_or(1);
            if step.is_none() && v > hi {
                break;
            }
        }
    }
    values.sort_unstable();
    values.dedup();
    Ok(values)
}

fn describe_field(field: &str, spec: &FieldSpec) -> String {
    let unit = spec.name;
    let label = |tok: &str| -> String {
        match (spec.name, token_value(tok, spec)) {
            ("month", Ok(i)) => MON_NAMES[i as usize].to_string(),
            ("day-of-week", Ok(i)) => DOW_NAMES[(i % 7) as usize].to_string(),
            _ => tok.to_string(),
        }
    };
    if field == "*" || field == "?" {
        return format!("every {unit}");
    }
    field
        .split(',')
        .map(|part| {
            let (range, step) = part
                .split_once('/')
                .map(|(r, s)| (r, Some(s)))
                .unwrap_or((part, None));
            let base = if range == "*" {
                String::new()
            } else if let Some((a, b)) = range.split_once('-') {
                format!("{} through {}", label(a), label(b))
            } else {
                label(range)
            };
            match (step, base.is_empty()) {
                (Some(s), true) => format!("every {s} {unit}s"),
                (Some(s), false) => format!("every {s} {unit}s from {base}"),
                (None, _) => base,
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// English sentence for a 5 or 6 field expression.
pub fn describe(fields: &[&str]) -> String {
    let (sec, min, hour, dom, mon, dow) = match fields.len() {
        5 => (None, fields[0], fields[1], fields[2], fields[3], fields[4]),
        6 | 7 => (Some(fields[0]), fields[1], fields[2], fields[3], fields[4], fields[5]),
        _ => return String::new(),
    };
    let simple = |f: &str| !f.contains(['/', ',', '-', '*']);
    let mut s = String::new();
    if let Some(sec) = sec {
        if sec != "0" && sec != "*" {
            s.push_str(&format!("At second {sec}, "));
        } else if sec == "*" {
            s.push_str("Every second, ");
        }
    }
    if simple(min) && simple(hour) {
        s.push_str(&format!("at {:0>2}:{:0>2}", hour, min));
    } else if simple(min) && hour == "*" {
        s.push_str(&if min == "0" {
            "every hour".to_string()
        } else {
            format!("at minute {min} past every hour")
        });
    } else if min == "*" && hour == "*" {
        s.push_str("every minute");
    } else if let (Some(step), true) = (min.strip_prefix("*/"), hour == "*") {
        s.push_str(&format!("every {step} minutes"));
    } else if let (Some(step), true) = (hour.strip_prefix("*/"), simple(min)) {
        s.push_str(&format!("at minute {min} past every {step} hours"));
    } else {
        s.push_str(&format!(
            "at {} past {}",
            describe_field(min, &FIELDS[1]),
            if hour == "*" {
                "every hour".to_string()
            } else {
                format!("hour {}", describe_field(hour, &FIELDS[2]))
            }
        ));
    }
    let dom_every = dom == "*" || dom == "?";
    let dow_every = dow == "*" || dow == "?";
    match (dom_every, dow_every) {
        (false, false) => s.push_str(&format!(
            ", on day-of-month {} and on {}",
            describe_field(dom, &FIELDS[3]),
            describe_field(dow, &FIELDS[5])
        )),
        (false, true) => {
            s.push_str(&format!(", on day-of-month {}", describe_field(dom, &FIELDS[3])))
        },
        (true, false) => s.push_str(&format!(", only on {}", describe_field(dow, &FIELDS[5]))),
        _ => {},
    }
    if mon != "*" {
        s.push_str(&format!(", only in {}", describe_field(mon, &FIELDS[4])));
    }
    let mut chars = s.chars();
    match chars.next() {
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
        None => s,
    }
}

fn unix_dow_to_quartz(field: &str) -> String {
    field
        .split(',')
        .map(|part| {
            let (body, step) = part
                .split_once('/')
                .map(|(b, s)| (b, Some(s)))
                .unwrap_or((part, None));
            let conv = |tok: &str| -> String {
                match tok.parse::<u32>() {
                    Ok(n) => (n % 7 + 1).to_string(),
                    Err(_) => tok.to_string(),
                }
            };
            let body = if body == "*" || body == "?" {
                body.to_string()
            } else if let Some((a, b)) = body.split_once('-') {
                format!("{}-{}", conv(a), conv(b))
            } else {
                conv(body)
            };
            match step {
                Some(s) => format!("{body}/{s}"),
                None => body,
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Parses, validates and explains a cron expression. `from` is an RFC 3339
/// instant or unix seconds; when omitted only the static analysis is returned.
pub fn parse(expression: &str, from: Option<&Value>, count: usize) -> EngineResult<Value> {
    let raw = expression.trim();
    engine_shared::error::require_text(raw, "expression")?;
    let macro_expanded = expand_macro(raw);
    let expr = macro_expanded.unwrap_or(raw);
    let fields: Vec<&str> = expr.split_whitespace().collect();
    if !(5..=7).contains(&fields.len()) {
        return Err(EngineError::invalid(format!(
            "Expected 5 fields (minute hour day-of-month month day-of-week) or 6 with leading \
             seconds; found {}",
            fields.len()
        )));
    }
    let has_seconds = fields.len() >= 6;
    let specs: Vec<&FieldSpec> = if has_seconds {
        FIELDS.iter().collect()
    } else {
        FIELDS[1..].iter().collect()
    };
    let mut errors: Vec<String> = Vec::new();
    let mut breakdown = Vec::new();
    for (i, f) in fields.iter().take(6).enumerate() {
        let spec = specs[i];
        let expanded = expand_field(f, spec);
        match &expanded {
            Ok(vals) => breakdown.push(json!({
                "field": spec.name,
                "value": f,
                "description": describe_field(f, spec),
                "matches": if vals.len() > 64 { json!(format!("{} values", vals.len())) } else { json!(vals) },
                "all": *f == "*"
            })),
            Err(e) => {
                errors.push(e.clone());
                breakdown.push(json!({ "field": spec.name, "value": f, "error": e }));
            }
        }
    }
    if fields.len() == 7 {
        breakdown.push(json!({
            "field": "year",
            "value": fields[6],
            "description": "year (Quartz extension)"
        }));
    }
    let warnings = {
        let mut w = Vec::new();
        let dom = fields[if has_seconds { 3 } else { 2 }];
        let dow = fields[if has_seconds { 5 } else { 4 }];
        if dom != "*" && dom != "?" && dow != "*" && dow != "?" {
            w.push(
                "Both day-of-month and day-of-week are restricted; standard cron runs when EITHER \
                 matches, Quartz requires one to be `?`.",
            );
        }
        if fields[if has_seconds { 1 } else { 0 }] == "*" {
            w.push("Runs every minute; make sure the job is idempotent and cheap.");
        }
        if dom.contains("31") {
            w.push("Day 31 exists only in 7 months; the job will not run in the others.");
        }
        if dom == "29" && fields[if has_seconds { 4 } else { 3 }] == "2" {
            w.push("29 February only occurs in leap years.");
        }
        w
    };
    let description = if errors.is_empty() {
        describe(&fields)
    } else {
        String::new()
    };
    let mut out = json!({
        "expression": raw,
        "expanded_from_macro": macro_expanded.is_some(),
        "normalized": fields.join(" "),
        "has_seconds": has_seconds,
        "valid": errors.is_empty(),
        "errors": errors,
        "warnings": warnings,
        "description": description,
        "fields": breakdown,
    });
    if out["valid"] == true {
        if let Some(from) = from.filter(|v| !v.is_null()) {
            let start = parse_instant(from)?;
            let mut parts: Vec<String> = fields.iter().map(|s| s.to_string()).collect();
            let dow_idx = if has_seconds { 5 } else { 4 };
            parts[dow_idx] = unix_dow_to_quartz(&parts[dow_idx]);
            let quartz = if has_seconds {
                parts.join(" ")
            } else {
                format!("0 {}", parts.join(" "))
            };
            let schedule = cron::Schedule::from_str(&quartz)
                .map_err(|e| EngineError::invalid(format!("Invalid cron expression: {e}")))?;
            let runs: Vec<Value> = schedule.after(&start).take(count.clamp(1, 100)).map(|d| json!({
                "utc": d.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                "unix": d.timestamp(),
                "weekday": DOW_NAMES[d.format("%w").to_string().parse::<usize>().unwrap_or(0)],
                "in_seconds": (d - start).num_seconds()
            })).collect();
            let interval = runs
                .windows(2)
                .map(|w| w[1]["unix"].as_i64().unwrap_or(0) - w[0]["unix"].as_i64().unwrap_or(0))
                .collect::<Vec<_>>();
            out["next_runs"] = json!(runs);
            out["from"] = json!(start.to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
            out["regular_interval_seconds"] =
                if !interval.is_empty() && interval.iter().all(|i| *i == interval[0]) {
                    json!(interval[0])
                } else {
                    Value::Null
                };
        }
    }
    Ok(out)
}

fn parse_instant(v: &Value) -> EngineResult<DateTime<Utc>> {
    match v {
        Value::Number(n) => {
            let secs = n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)).unwrap_or(0);
            let secs = if secs > 1_000_000_000_000 { secs / 1000 } else { secs };
            Utc.timestamp_opt(secs, 0)
                .single()
                .ok_or_else(|| EngineError::invalid("Invalid unix timestamp"))
        },
        Value::String(s) => {
            if let Ok(n) = s.trim().parse::<i64>() {
                return parse_instant(&json!(n));
            }
            DateTime::parse_from_rfc3339(s.trim())
                .map(|d| d.with_timezone(&Utc))
                .or_else(|_| {
                    chrono::NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%dT%H:%M:%S")
                        .or_else(|_| {
                            chrono::NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%d %H:%M:%S")
                        })
                        .or_else(|_| {
                            chrono::NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%d %H:%M")
                        })
                        .map(|n| n.and_utc())
                })
                .map_err(|_| {
                    EngineError::invalid(format!(
                        "Cannot parse `{s}` as an RFC 3339 date-time or unix timestamp"
                    ))
                })
        },
        _ => Err(EngineError::invalid("`from` must be an RFC 3339 string or unix timestamp")),
    }
}

/// Inputs for the generator. Each field accepts a list of values, a step, or
/// `every` to mean `*`.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct GenerateInput {
    /// Preset: `every_minute`, `hourly`, `daily`, `weekly`, `monthly`, `yearly`, `weekdays`,
    /// `weekends`, `custom`.
    pub preset: String,
    /// Minute values (`custom` preset).
    pub minutes: Vec<u32>,
    /// Hour values (`custom` preset).
    pub hours: Vec<u32>,
    /// Day-of-month values (`custom` preset).
    pub days_of_month: Vec<u32>,
    /// Month values (`custom` preset).
    pub months: Vec<u32>,
    /// Day-of-week values, 0 = Sunday (`custom` preset).
    pub days_of_week: Vec<u32>,
    /// Run every N minutes.
    pub every_minutes: Option<u32>,
    /// Run every N hours.
    pub every_hours: Option<u32>,
    /// Run every N days.
    pub every_days: Option<u32>,
    /// Hour for daily/weekly/monthly presets.
    pub hour: Option<u32>,
    /// Minute for the presets.
    pub minute: Option<u32>,
    /// Emit a 6-field expression with a seconds column.
    pub include_seconds: bool,
}

fn list_or_star(values: &[u32], spec: &FieldSpec) -> EngineResult<String> {
    if values.is_empty() {
        return Ok("*".into());
    }
    let mut v: Vec<u32> = values.to_vec();
    v.sort_unstable();
    v.dedup();
    for x in &v {
        if *x < spec.min || *x > spec.max {
            return Err(EngineError::invalid(format!(
                "{} value {x} is out of range {}-{}",
                spec.name, spec.min, spec.max
            )));
        }
    }
    // Compress consecutive runs into ranges.
    let mut parts = Vec::new();
    let mut i = 0;
    while i < v.len() {
        let mut j = i;
        while j + 1 < v.len() && v[j + 1] == v[j] + 1 {
            j += 1;
        }
        if j - i >= 2 {
            parts.push(format!("{}-{}", v[i], v[j]));
        } else {
            for x in &v[i..=j] {
                parts.push(x.to_string());
            }
        }
        i = j + 1;
    }
    Ok(parts.join(","))
}

/// Builds a cron expression (plus description and next runs) from a preset or explicit fields.
pub fn generate(input: &GenerateInput) -> EngineResult<Value> {
    let minute = input.minute.unwrap_or(0);
    let hour = input.hour.unwrap_or(0);
    let expr = match input.preset.as_str() {
        "every_minute" | "minutely" => "* * * * *".to_string(),
        "hourly" => format!("{minute} * * * *"),
        "daily" | "midnight" => format!("{minute} {hour} * * *"),
        "weekly" => {
            format!("{minute} {hour} * * {}", input.days_of_week.first().copied().unwrap_or(0))
        },
        "monthly" => {
            format!("{minute} {hour} {} * *", input.days_of_month.first().copied().unwrap_or(1))
        },
        "yearly" | "annually" => format!(
            "{minute} {hour} {} {} *",
            input.days_of_month.first().copied().unwrap_or(1),
            input.months.first().copied().unwrap_or(1)
        ),
        "weekdays" => format!("{minute} {hour} * * 1-5"),
        "weekends" => format!("{minute} {hour} * * 0,6"),
        "" | "custom" => {
            let m = if let Some(step) = input.every_minutes {
                format!("*/{}", step.clamp(1, 59))
            } else if input.minutes.is_empty() && input.minute.is_some() {
                minute.to_string()
            } else {
                list_or_star(&input.minutes, &FIELDS[1])?
            };
            let h = if let Some(step) = input.every_hours {
                format!("*/{}", step.clamp(1, 23))
            } else if input.hours.is_empty() && input.hour.is_some() {
                hour.to_string()
            } else if input.every_minutes.is_some() && input.hours.is_empty() {
                "*".into()
            } else {
                list_or_star(&input.hours, &FIELDS[2])?
            };
            let h = if input.every_minutes.is_none()
                && input.every_hours.is_none()
                && input.hours.is_empty()
                && input.hour.is_none()
                && !input.minutes.is_empty()
            {
                "*".to_string()
            } else {
                h
            };
            let dom = if let Some(step) = input.every_days {
                format!("*/{}", step.clamp(1, 31))
            } else {
                list_or_star(&input.days_of_month, &FIELDS[3])?
            };
            let mon = list_or_star(&input.months, &FIELDS[4])?;
            let dow = list_or_star(&input.days_of_week, &FIELDS[5])?;
            let (m, h) = if m == "*"
                && h == "*"
                && input.minute.is_none()
                && input.hour.is_none()
                && input.every_minutes.is_none()
                && input.every_hours.is_none()
                && (dom != "*" || mon != "*" || dow != "*")
            {
                ("0".to_string(), "0".to_string())
            } else {
                (m, h)
            };
            format!("{m} {h} {dom} {mon} {dow}")
        },
        other => return Err(EngineError::invalid(format!("Unknown preset `{other}`"))),
    };
    let expr = if input.include_seconds {
        format!("0 {expr}")
    } else {
        expr
    };
    let mut v = parse(&expr, None, 0)?;
    v["generated"] = json!(true);
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_describe() {
        let p = parse("*/15 9-17 * * 1-5", Some(&json!("2024-01-01T00:00:00Z")), 3).unwrap();
        assert_eq!(p["valid"], true);
        assert_eq!(
            p["description"],
            "At every 15 minutes past hour 9 through 17, only on Monday through Friday"
        );
        assert_eq!(p["next_runs"][0]["utc"], "2024-01-01T09:00:00Z");
        assert_eq!(p["regular_interval_seconds"], 900);
        assert_eq!(parse("0 9 * * *", None, 0).unwrap()["description"], "At 09:00");
        assert_eq!(
            parse("30 2 1 * *", None, 0).unwrap()["description"],
            "At 02:30, on day-of-month 1"
        );
        assert_eq!(parse("0 0 * * 0", None, 0).unwrap()["description"], "At 00:00, only on Sunday");
        assert_eq!(parse("@daily", None, 0).unwrap()["normalized"], "0 0 * * *");
        let bad = parse("61 * * * *", None, 0).unwrap();
        assert_eq!(bad["valid"], false);
        assert!(parse("* *", None, 0).is_err());
        assert!(parse("", None, 0).is_err());
        let sec = parse("*/10 * * * * *", Some(&json!(1704067200)), 2).unwrap();
        assert_eq!(sec["has_seconds"], true);
        assert_eq!(sec["regular_interval_seconds"], 10);
    }

    #[test]
    fn generation() {
        let g = generate(&GenerateInput {
            preset: "weekdays".into(),
            hour: Some(9),
            minute: Some(30),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(g["normalized"], "30 9 * * 1-5");
        let g = generate(&GenerateInput {
            every_minutes: Some(5),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(g["normalized"], "*/5 * * * *");
        let g = generate(&GenerateInput {
            minutes: vec![0, 30],
            hours: vec![8, 9, 10, 12],
            days_of_week: vec![1, 3],
            ..Default::default()
        })
        .unwrap();
        assert_eq!(g["normalized"], "0,30 8-10,12 * * 1,3");
        assert!(generate(&GenerateInput { hours: vec![25], ..Default::default() }).is_err());
    }
}
