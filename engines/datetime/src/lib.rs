//! # Dev Toolbox Date & Time Engine (`datetime-engine`)
//!
//! The engine has **no clock**: every action that needs "now" takes it from
//! the payload (`now` as ISO-8601 or `now_unix`), so results are
//! deterministic and testable. Time zones use the IANA database bundled by
//! `chrono-tz`; DST transitions are handled by the library.
//!
//! | Action | Tool(s) |
//! |---|---|
//! | `parse` | date parsing / inspection (ISO, RFC 2822, RFC 3339, common formats) |
//! | `difference` | date-difference-calculator, days-between-dates, time-calculator |
//! | `add` | date-add-subtract |
//! | `age` | age-calculator, age-in-days-calculator |
//! | `convert_zone` / `zones` | time-zone-converter, world-clock |
//! | `timestamp` | unix-timestamp-converter, epoch-converter, timestamp-to-date, date-to-timestamp |
//! | `business_days` | business-days-calculator, working-days-calculator |
//! | `week_number` | week-number-calculator |
//! | `countdown` | countdown-calculator, birthday-countdown, event-countdown |
//! | `calendar` | calendar-generator |
//! | `meeting_planner` | meeting-time-planner, time-zone-meeting-planner |
//! | `cron_next` | cron-next-run |
//! | `pomodoro` | pomodoro-timer (schedule generation) |
//! | `format` | custom strftime formatting |

use chrono::{
    DateTime, Datelike, Duration, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, Offset,
    TimeZone, Timelike, Utc, Weekday,
};
use chrono_tz::Tz;
use engine_shared::{EngineError, EngineResult};
use serde::Deserialize;
use serde_json::{json, Value};
use std::str::FromStr;

/// Parses many date/time shapes into a UTC datetime plus the offset that
/// was present in the input (if any).
pub fn parse_datetime(
    input: &str,
    default_tz: Option<&Tz>,
) -> EngineResult<(DateTime<Utc>, Option<FixedOffset>, &'static str)> {
    let s = input.trim();
    if s.is_empty() {
        return Err(EngineError::empty("date"));
    }
    // Unix timestamps (seconds or milliseconds).
    if let Ok(n) = s.parse::<i64>() {
        let (secs, kind) = if n.abs() > 100_000_000_000 {
            (n / 1000, "unix_ms")
        } else {
            (n, "unix_seconds")
        };
        let dt = DateTime::<Utc>::from_timestamp(
            secs,
            if kind == "unix_ms" {
                ((n % 1000).unsigned_abs() * 1_000_000) as u32
            } else {
                0
            },
        )
        .ok_or_else(|| EngineError::invalid("Timestamp out of range"))?;
        return Ok((dt, None, kind));
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok((dt.with_timezone(&Utc), Some(*dt.offset()), "rfc3339"));
    }
    if let Ok(dt) = DateTime::parse_from_rfc2822(s) {
        return Ok((dt.with_timezone(&Utc), Some(*dt.offset()), "rfc2822"));
    }
    for fmt in [
        "%Y-%m-%dT%H:%M:%S%.f%z",
        "%Y-%m-%d %H:%M:%S%.f %z",
        "%Y-%m-%d %H:%M:%S %Z",
        "%Y-%m-%dT%H:%M:%S%z",
        "%Y-%m-%d %H:%M%z",
    ] {
        if let Ok(dt) = DateTime::parse_from_str(s, fmt) {
            return Ok((dt.with_timezone(&Utc), Some(*dt.offset()), "offset"));
        }
    }
    let naive_formats = [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M",
        "%Y/%m/%d %H:%M:%S",
        "%Y/%m/%d %H:%M",
        "%d/%m/%Y %H:%M:%S",
        "%d/%m/%Y %H:%M",
        "%m/%d/%Y %H:%M:%S",
        "%m/%d/%Y %H:%M",
        "%d-%m-%Y %H:%M",
        "%d %b %Y %H:%M",
        "%d %B %Y %H:%M",
        "%b %d, %Y %H:%M",
        "%B %d, %Y %H:%M",
    ];
    let date_formats = [
        "%Y-%m-%d",
        "%Y/%m/%d",
        "%d/%m/%Y",
        "%m/%d/%Y",
        "%d-%m-%Y",
        "%d.%m.%Y",
        "%Y%m%d",
        "%d %b %Y",
        "%d %B %Y",
        "%b %d, %Y",
        "%B %d, %Y",
        "%b %d %Y",
        "%B %d %Y",
        "%Y-%b-%d",
    ];
    let mut naive: Option<(NaiveDateTime, &'static str)> = None;
    for fmt in naive_formats {
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, fmt) {
            naive = Some((dt, "local_datetime"));
            break;
        }
    }
    if naive.is_none() {
        for fmt in date_formats {
            if let Ok(d) = NaiveDate::parse_from_str(s, fmt) {
                naive = Some((d.and_time(NaiveTime::MIN), "date"));
                break;
            }
        }
    }
    let (n, kind) = naive.ok_or_else(|| {
        EngineError::invalid(format!(
            "Unrecognised date `{s}`. Use ISO-8601 (2024-01-31T10:00:00Z), a date (2024-01-31, \
             31/01/2024, Jan 31, 2024) or a unix timestamp."
        ))
    })?;
    let utc = match default_tz {
        Some(tz) => local_to_utc(tz, n)?,
        None => Utc.from_utc_datetime(&n),
    };
    Ok((utc, None, kind))
}

/// Resolves a local wall-clock time in `tz`. Ambiguous times (DST fall-back)
/// take the earlier instant; non-existent times (DST gap) are moved forward
/// by the gap length, matching common calendar behaviour.
fn local_to_utc(tz: &Tz, n: NaiveDateTime) -> EngineResult<DateTime<Utc>> {
    match tz.from_local_datetime(&n) {
        chrono::LocalResult::Single(d) => Ok(d.with_timezone(&Utc)),
        chrono::LocalResult::Ambiguous(a, _) => Ok(a.with_timezone(&Utc)),
        chrono::LocalResult::None => {
            for minutes in [60, 30, 120] {
                if let chrono::LocalResult::Single(d) =
                    tz.from_local_datetime(&(n + Duration::minutes(minutes)))
                {
                    return Ok(d.with_timezone(&Utc));
                }
            }
            Err(EngineError::invalid("Local time does not exist in that time zone (DST gap)"))
        },
    }
}

fn parse_tz(name: Option<&str>) -> EngineResult<Option<Tz>> {
    match name.map(str::trim).filter(|n| !n.is_empty()) {
        None => Ok(None),
        Some(n) => {
            let canonical = match n.to_ascii_uppercase().as_str() {
                "UTC" | "GMT" | "Z" => "UTC".to_string(),
                "IST" => "Asia/Kolkata".to_string(),
                "EST" => "America/New_York".to_string(),
                "PST" => "America/Los_Angeles".to_string(),
                "CST" => "America/Chicago".to_string(),
                "MST" => "America/Denver".to_string(),
                "BST" => "Europe/London".to_string(),
                "CET" | "CEST" => "Europe/Paris".to_string(),
                "JST" => "Asia/Tokyo".to_string(),
                "AEST" => "Australia/Sydney".to_string(),
                "SGT" => "Asia/Singapore".to_string(),
                "HKT" => "Asia/Hong_Kong".to_string(),
                _ => n.to_string(),
            };
            Tz::from_str(&canonical).map(Some).map_err(|_| {
                EngineError::invalid(format!(
                    "Unknown time zone `{n}` (use IANA names like Europe/Berlin, Asia/Kolkata, \
                     America/New_York)"
                ))
            })
        },
    }
}

fn describe(dt: &DateTime<Utc>, tz: Option<&Tz>) -> Value {
    let local: Value = match tz {
        Some(z) => {
            let l = dt.with_timezone(z);
            json!({
                "iso": l.to_rfc3339(),
                "zone": z.name(),
                "abbreviation": l.format("%Z").to_string(),
                "offset": l.format("%:z").to_string(),
                "date": l.format("%Y-%m-%d").to_string(),
                "time": l.format("%H:%M:%S").to_string(),
                "weekday": l.format("%A").to_string()
            })
        },
        None => Value::Null,
    };
    json!({
        "iso_utc": dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "unix": dt.timestamp(),
        "unix_ms": dt.timestamp_millis(),
        "date": dt.format("%Y-%m-%d").to_string(),
        "time": dt.format("%H:%M:%S").to_string(),
        "weekday": dt.format("%A").to_string(),
        "day_of_year": dt.ordinal(),
        "iso_week": dt.iso_week().week(),
        "iso_week_year": dt.iso_week().year(),
        "quarter": (dt.month() - 1) / 3 + 1,
        "rfc2822": dt.to_rfc2822(),
        "leap_year": is_leap(dt.year()),
        "local": local,
    })
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Parses many date formats and reports the components in UTC/local.
pub fn parse(input: &str, tz: Option<&str>) -> EngineResult<Value> {
    let z = parse_tz(tz)?;
    let (dt, offset, kind) = parse_datetime(input, z.as_ref())?;
    let mut v = describe(&dt, z.as_ref());
    v["input_kind"] = json!(kind);
    v["input_offset"] = json!(offset.map(|o| o.to_string()));
    Ok(v)
}

fn ymd_diff(a: NaiveDate, b: NaiveDate) -> (i32, i32, i64) {
    // Calendar difference a → b (a <= b): whole months stepped from `a`
    // (end-of-month clamped), remainder in days.
    if b < a {
        return (0, 0, 0);
    }
    let mut months = 0i32;
    while add_months(a, months + 1) <= b {
        months += 1;
    }
    let anchor = add_months(a, months);
    let days = (b - anchor).num_days();
    (months / 12, months % 12, days)
}

/// Difference between two dates in every unit plus business days.
pub fn difference(
    from: &str,
    to: &str,
    tz: Option<&str>,
    include_end: bool,
) -> EngineResult<Value> {
    let z = parse_tz(tz)?;
    let (a, _, _) = parse_datetime(from, z.as_ref())?;
    let (b, _, _) = parse_datetime(to, z.as_ref())?;
    let (start, end, negative) = if a <= b { (a, b, false) } else { (b, a, true) };
    let dur = end - start
        + if include_end {
            Duration::days(1)
        } else {
            Duration::zero()
        };
    let secs = dur.num_seconds();
    let (y, m, d) = ymd_diff(
        start.date_naive(),
        (end + if include_end {
            Duration::days(1)
        } else {
            Duration::zero()
        })
        .date_naive(),
    );
    let total_days = dur.num_days();
    let weekdays = count_weekdays(start.date_naive(), end.date_naive(), include_end);
    let weeks = total_days / 7;
    let human = {
        let mut parts = Vec::new();
        if y != 0 {
            parts.push(format!("{y} year{}", if y == 1 { "" } else { "s" }));
        }
        if m != 0 {
            parts.push(format!("{m} month{}", if m == 1 { "" } else { "s" }));
        }
        if d != 0 || parts.is_empty() {
            parts.push(format!("{d} day{}", if d == 1 { "" } else { "s" }));
        }
        parts.join(", ")
    };
    Ok(json!({
        "from": describe(&a, z.as_ref()),
        "to": describe(&b, z.as_ref()),
        "negative": negative,
        "years": y,
        "months": m,
        "days": d,
        "human": human,
        "total": {
            "seconds": secs,
            "minutes": secs / 60,
            "hours": secs / 3600,
            "days": total_days,
            "weeks": weeks,
            "remaining_days_after_weeks": total_days % 7,
            "months_approx": (total_days as f64 / 30.436875 * 100.0).round() / 100.0,
            "years_approx": (total_days as f64 / 365.2425 * 1000.0).round() / 1000.0
        },
        "time_part": {
            "hours": (secs % 86400) / 3600,
            "minutes": (secs % 3600) / 60,
            "seconds": secs % 60
        },
        "weekdays": weekdays,
        "weekend_days": total_days + if include_end { 0 } else { 0 } - weekdays,
        "include_end": include_end,
    }))
}

fn count_weekdays(start: NaiveDate, end: NaiveDate, include_end: bool) -> i64 {
    let mut d = start;
    let mut n = 0;
    let last = if include_end {
        end
    } else {
        end.pred_opt().unwrap_or(end)
    };
    if end <= start && !include_end {
        return 0;
    }
    while d <= last {
        if !matches!(d.weekday(), Weekday::Sat | Weekday::Sun) {
            n += 1;
        }
        match d.succ_opt() {
            Some(next) => d = next,
            None => break,
        }
    }
    n
}

fn add_months(date: NaiveDate, months: i32) -> NaiveDate {
    let total = date.year() * 12 + date.month() as i32 - 1 + months;
    let (y, m) = (total.div_euclid(12), total.rem_euclid(12) as u32 + 1);
    let last_day = NaiveDate::from_ymd_opt(
        if m == 12 { y + 1 } else { y },
        if m == 12 { 1 } else { m + 1 },
        1,
    )
    .and_then(|d| d.pred_opt())
    .map(|d| d.day())
    .unwrap_or(28);
    // Day 1 of any month is always valid, so the fallback cannot be hit in practice.
    NaiveDate::from_ymd_opt(y, m, date.day().min(last_day))
        .or_else(|| NaiveDate::from_ymd_opt(y, m, 1))
        .unwrap_or(date)
}

/// Amount to add/subtract in `add`.
#[derive(Deserialize, Default, Debug, Clone)]
#[serde(default)]
pub struct Delta {
    /// Calendar years.
    pub years: i32,
    /// Calendar months.
    pub months: i32,
    /// Weeks.
    pub weeks: i64,
    /// Days.
    pub days: i64,
    /// Hours.
    pub hours: i64,
    /// Minutes.
    pub minutes: i64,
    /// Seconds.
    pub seconds: i64,
    /// Business days (Mon–Fri).
    pub business_days: i64,
}

/// Adds or subtracts a [`Delta`] to a date.
pub fn add(date: &str, delta: &Delta, subtract: bool, tz: Option<&str>) -> EngineResult<Value> {
    let z = parse_tz(tz)?;
    let (start, _, _) = parse_datetime(date, z.as_ref())?;
    let sign = if subtract { -1 } else { 1 };
    // Calendar arithmetic in the local zone to respect DST.
    let local_naive = match &z {
        Some(tz) => start.with_timezone(tz).naive_local(),
        None => start.naive_utc(),
    };
    let mut d = add_months(local_naive.date(), sign * (delta.years * 12 + delta.months));
    d += Duration::days(sign as i64 * (delta.weeks * 7 + delta.days));
    if delta.business_days != 0 {
        let mut remaining = delta.business_days.abs();
        while remaining > 0 {
            d += Duration::days(sign as i64);
            if !matches!(d.weekday(), Weekday::Sat | Weekday::Sun) {
                remaining -= 1;
            }
        }
    }
    let naive = d.and_time(local_naive.time())
        + Duration::seconds(
            sign as i64 * (delta.hours * 3600 + delta.minutes * 60 + delta.seconds),
        );
    let result = match &z {
        Some(tz) => local_to_utc(tz, naive)?,
        None => Utc.from_utc_datetime(&naive),
    };
    Ok(json!({
        "start": describe(&start, z.as_ref()),
        "result": describe(&result, z.as_ref()),
        "delta": {
            "years": delta.years,
            "months": delta.months,
            "weeks": delta.weeks,
            "days": delta.days,
            "hours": delta.hours,
            "minutes": delta.minutes,
            "seconds": delta.seconds,
            "business_days": delta.business_days,
            "direction": if subtract { "subtract" } else { "add" }
        },
        "difference_days": (result - start).num_days()
    }))
}

/// Age in years/months/days plus next birthday and zodiac.
pub fn age(birth: &str, now: &str, tz: Option<&str>) -> EngineResult<Value> {
    let z = parse_tz(tz)?;
    let (b, _, _) = parse_datetime(birth, z.as_ref())?;
    let (n, _, _) = parse_datetime(now, z.as_ref())?;
    if b > n {
        return Err(EngineError::invalid("Birth date is in the future"));
    }
    let bd = b.date_naive();
    let nd = n.date_naive();
    let (y, m, d) = ymd_diff(bd, nd);
    let total_days = (nd - bd).num_days();
    let mut next = NaiveDate::from_ymd_opt(nd.year(), bd.month(), bd.day())
        .or_else(|| NaiveDate::from_ymd_opt(nd.year(), 3, 1));
    if let Some(nb) = next {
        if nb < nd {
            next = NaiveDate::from_ymd_opt(nd.year() + 1, bd.month(), bd.day())
                .or_else(|| NaiveDate::from_ymd_opt(nd.year() + 1, 3, 1));
        }
    }
    // `March 1` always exists, so the fallback is only defensive.
    let next_birthday = next.unwrap_or(nd);
    let days_to_birthday = (next_birthday - nd).num_days();
    let zodiac = zodiac_sign(bd.month(), bd.day());
    const CHINESE: [&str; 12] = [
        "Monkey", "Rooster", "Dog", "Pig", "Rat", "Ox", "Tiger", "Rabbit", "Dragon", "Snake",
        "Horse", "Goat",
    ];
    let chinese = CHINESE[bd.year().rem_euclid(12) as usize];
    Ok(json!({
        "years": y,
        "months": m,
        "days": d,
        "human": format!("{y} years, {m} months, {d} days"),
        "total": {
            "days": total_days,
            "weeks": total_days / 7,
            "months": y as i64 * 12 + m as i64,
            "hours": (n - b).num_hours(),
            "minutes": (n - b).num_minutes()
        },
        "next_birthday": next_birthday.to_string(),
        "days_until_birthday": days_to_birthday,
        "turning": y + 1,
        "birthday_weekday": next_birthday.format("%A").to_string(),
        "born_weekday": bd.format("%A").to_string(),
        "zodiac": zodiac,
        "chinese_zodiac": chinese,
        "is_birthday_today": bd.month() == nd.month() && bd.day() == nd.day(),
        "decimal_age": (total_days as f64 / 365.2425 * 100.0).round() / 100.0,
    }))
}

fn zodiac_sign(m: u32, d: u32) -> &'static str {
    match (m, d) {
        (3, 21..=31) | (4, 1..=19) => "Aries",
        (4, 20..=30) | (5, 1..=20) => "Taurus",
        (5, 21..=31) | (6, 1..=20) => "Gemini",
        (6, 21..=30) | (7, 1..=22) => "Cancer",
        (7, 23..=31) | (8, 1..=22) => "Leo",
        (8, 23..=31) | (9, 1..=22) => "Virgo",
        (9, 23..=30) | (10, 1..=22) => "Libra",
        (10, 23..=31) | (11, 1..=21) => "Scorpio",
        (11, 22..=30) | (12, 1..=21) => "Sagittarius",
        (12, 22..=31) | (1, 1..=19) => "Capricorn",
        (1, 20..=31) | (2, 1..=18) => "Aquarius",
        _ => "Pisces",
    }
}

/// Converts a date-time into several time zones.
pub fn convert_zone(datetime: &str, from: Option<&str>, to: &[String]) -> EngineResult<Value> {
    let from_tz = parse_tz(from)?;
    let (dt, offset, _) = parse_datetime(datetime, from_tz.as_ref())?;
    let targets: Vec<String> = if to.is_empty() {
        vec![
            "UTC".into(),
            "America/New_York".into(),
            "Europe/London".into(),
            "Europe/Berlin".into(),
            "Asia/Kolkata".into(),
            "Asia/Tokyo".into(),
            "Australia/Sydney".into(),
        ]
    } else {
        to.to_vec()
    };
    let mut results = Vec::new();
    for name in &targets {
        let Some(tz) = parse_tz(Some(name))? else {
            continue;
        };
        let l = dt.with_timezone(&tz);
        let source_offset = from_tz
            .map(|z| dt.with_timezone(&z).offset().fix().local_minus_utc())
            .or_else(|| offset.map(|o| o.local_minus_utc()))
            .unwrap_or(0);
        let diff_h = (l.offset().fix().local_minus_utc() - source_offset) as f64 / 3600.0;
        results.push(json!({
            "zone": tz.name(),
            "iso": l.to_rfc3339(),
            "date": l.format("%Y-%m-%d").to_string(),
            "time": l.format("%H:%M").to_string(),
            "time_12h": l.format("%I:%M %p").to_string(),
            "weekday": l.format("%A").to_string(),
            "abbreviation": l.format("%Z").to_string(),
            "offset": l.format("%:z").to_string(),
            "offset_minutes": l.offset().fix().local_minus_utc() / 60,
            "difference_hours_from_source": diff_h,
            "day_shift": l.date_naive().signed_duration_since(dt.with_timezone(&from_tz.unwrap_or(Tz::UTC)).date_naive()).num_days(),
            "is_dst": is_dst(&tz, &dt),
        }));
    }
    Ok(json!({ "source": describe(&dt, from_tz.as_ref()), "results": results }))
}

fn is_dst(tz: &Tz, dt: &DateTime<Utc>) -> bool {
    // Compare the offset with the January and July offsets of the same year.
    let year = dt.year();
    let jan = tz
        .with_ymd_and_hms(year, 1, 15, 12, 0, 0)
        .single()
        .map(|d| d.offset().fix().local_minus_utc())
        .unwrap_or(0);
    let jul = tz
        .with_ymd_and_hms(year, 7, 15, 12, 0, 0)
        .single()
        .map(|d| d.offset().fix().local_minus_utc())
        .unwrap_or(0);
    let now = dt.with_timezone(tz).offset().fix().local_minus_utc();
    jan != jul && now == jan.max(jul)
}

/// Lists IANA time zones with their current offsets.
pub fn zones(filter: Option<&str>, now: Option<&str>) -> EngineResult<Value> {
    let ref_time = match now {
        Some(n) => parse_datetime(n, None)?.0,
        None => DateTime::<Utc>::from_timestamp(1_704_067_200, 0).unwrap_or_default(), // 2024-01-01
    };
    let f = filter.map(|f| f.to_lowercase()).unwrap_or_default();
    let list: Vec<Value> = chrono_tz::TZ_VARIANTS
        .iter()
        .filter(|tz| f.is_empty() || tz.name().to_lowercase().contains(&f))
        .take(600)
        .map(|tz| {
            let l = ref_time.with_timezone(tz);
            json!({
                "zone": tz.name(),
                "offset": l.format("%:z").to_string(),
                "abbreviation": l.format("%Z").to_string(),
                "offset_minutes": l.offset().fix().local_minus_utc() / 60
            })
        })
        .collect();
    Ok(json!({ "zones": list, "count": list.len(), "total_zones": chrono_tz::TZ_VARIANTS.len() }))
}

/// Unix timestamp ↔ date conversion.
pub fn timestamp(input: &str, tz: Option<&str>) -> EngineResult<Value> {
    let z = parse_tz(tz)?;
    let (dt, _, kind) = parse_datetime(input, z.as_ref())?;
    let mut v = describe(&dt, z.as_ref());
    v["input_kind"] = json!(kind);
    v["unix_micro"] = json!(dt.timestamp_micros());
    v["unix_nano"] = json!(dt.timestamp_nanos_opt());
    v["excel_serial"] = json!((dt.timestamp() as f64 / 86400.0) + 25569.0);
    v["julian_day"] = json!(dt.timestamp() as f64 / 86400.0 + 2440587.5);
    v["ticks_dotnet"] = json!(dt.timestamp() as i128 * 10_000_000 + 621_355_968_000_000_000);
    v["relative_to_epoch"] = json!(format!("{} days since 1970-01-01", dt.timestamp() / 86400));
    Ok(v)
}

/// Business-day arithmetic with custom weekends and holidays.
pub fn business_days(
    start: &str,
    end: Option<&str>,
    add_days: Option<i64>,
    holidays: &[String],
    weekend: &[String],
    tz: Option<&str>,
) -> EngineResult<Value> {
    let z = parse_tz(tz)?;
    let (s, _, _) = parse_datetime(start, z.as_ref())?;
    let sd = match &z {
        Some(tz) => s.with_timezone(tz).date_naive(),
        None => s.date_naive(),
    };
    let weekend_days: Vec<Weekday> = if weekend.is_empty() {
        vec![Weekday::Sat, Weekday::Sun]
    } else {
        weekend
            .iter()
            .map(|w| parse_weekday(w))
            .collect::<EngineResult<_>>()?
    };
    let holiday_set: std::collections::HashSet<NaiveDate> = holidays
        .iter()
        .map(|h| parse_datetime(h, None).map(|(d, _, _)| d.date_naive()))
        .collect::<EngineResult<_>>()?;
    let is_working =
        |d: NaiveDate| !weekend_days.contains(&d.weekday()) && !holiday_set.contains(&d);
    if let Some(n) = add_days {
        let mut d = sd;
        let mut remaining = n.abs();
        let step = if n >= 0 { 1 } else { -1 };
        while remaining > 0 {
            d += Duration::days(step);
            if is_working(d) {
                remaining -= 1;
            }
        }
        return Ok(json!({
            "start": sd.to_string(),
            "business_days_added": n,
            "result": d.to_string(),
            "result_weekday": d.format("%A").to_string(),
            "calendar_days": (d - sd).num_days()
        }));
    }
    let e = end.ok_or_else(|| EngineError::invalid("Provide `end` or `add_days`"))?;
    let (e, _, _) = parse_datetime(e, z.as_ref())?;
    let ed = match &z {
        Some(tz) => e.with_timezone(tz).date_naive(),
        None => e.date_naive(),
    };
    let (lo, hi) = if sd <= ed { (sd, ed) } else { (ed, sd) };
    let mut d = lo;
    let (mut working, mut weekends, mut hols) = (0, 0, 0);
    let mut holiday_hits = Vec::new();
    while d < hi {
        if weekend_days.contains(&d.weekday()) {
            weekends += 1;
        } else if holiday_set.contains(&d) {
            hols += 1;
            holiday_hits.push(d.to_string());
        } else {
            working += 1;
        }
        d += Duration::days(1);
    }
    let total = (hi - lo).num_days();
    Ok(json!({
        "start": lo.to_string(),
        "end": hi.to_string(),
        "business_days": working,
        "calendar_days": total,
        "weekend_days": weekends,
        "holidays": hols,
        "holidays_hit": holiday_hits,
        "working_hours_8h": working * 8,
        "note": "End date exclusive; set `add_days` to project forward instead"
    }))
}

fn parse_weekday(s: &str) -> EngineResult<Weekday> {
    Weekday::from_str(s.trim()).map_err(|_| EngineError::invalid(format!("Unknown weekday `{s}`")))
}

/// ISO and US week numbers.
pub fn week_number(date: &str, tz: Option<&str>) -> EngineResult<Value> {
    let z = parse_tz(tz)?;
    let (dt, _, _) = parse_datetime(date, z.as_ref())?;
    let d = match &z {
        Some(tz) => dt.with_timezone(tz).date_naive(),
        None => dt.date_naive(),
    };
    let iso = d.iso_week();
    let jan1 = NaiveDate::from_ymd_opt(d.year(), 1, 1).unwrap_or(d);
    let us_week = (d.ordinal() as i64 + jan1.weekday().num_days_from_sunday() as i64 - 1) / 7 + 1;
    let week_start = d - Duration::days(d.weekday().num_days_from_monday() as i64);
    let weeks_in_year = NaiveDate::from_ymd_opt(iso.year(), 12, 28)
        .map(|d| d.iso_week().week())
        .unwrap_or(52);
    Ok(json!({
        "date": d.to_string(),
        "iso_week": iso.week(),
        "iso_year": iso.year(),
        "iso_label": format!("{}-W{:02}", iso.year(), iso.week()),
        "us_week": us_week,
        "weeks_in_iso_year": weeks_in_year,
        "day_of_year": d.ordinal(),
        "days_in_year": if is_leap(d.year()) { 366 } else { 365 },
        "weekday": d.format("%A").to_string(),
        "week_start_monday": week_start.to_string(),
        "week_end_sunday": (week_start + Duration::days(6)).to_string(),
        "week_start_sunday": (d - Duration::days(d.weekday().num_days_from_sunday() as i64)).to_string(),
        "quarter": (d.month() - 1) / 3 + 1,
        "days_remaining_in_year": (if is_leap(d.year()) { 366 } else { 365 }) - d.ordinal() as i64,
    }))
}

/// Time remaining until a target (optionally recurring yearly).
pub fn countdown(target: &str, now: &str, tz: Option<&str>, yearly: bool) -> EngineResult<Value> {
    let z = parse_tz(tz)?;
    let (mut t, _, _) = parse_datetime(target, z.as_ref())?;
    let (n, _, _) = parse_datetime(now, z.as_ref())?;
    if yearly && t < n {
        // Advance to the next anniversary.
        let td = t.date_naive();
        let nd = n.date_naive();
        // Feb 29 targets roll over to Mar 1 in non-leap years.
        let mut candidate = NaiveDate::from_ymd_opt(nd.year(), td.month(), td.day())
            .or_else(|| NaiveDate::from_ymd_opt(nd.year(), 3, 1))
            .unwrap_or(nd);
        let time = t.time();
        if candidate.and_time(time) < n.naive_utc() {
            candidate = NaiveDate::from_ymd_opt(nd.year() + 1, td.month(), td.day())
                .or_else(|| NaiveDate::from_ymd_opt(nd.year() + 1, 3, 1))
                .unwrap_or(candidate);
        }
        t = Utc.from_utc_datetime(&candidate.and_time(time));
    }
    let dur = t - n;
    let past = dur < Duration::zero();
    let secs = dur.num_seconds().abs();
    let total_days = secs / 86400;
    let (y, m, d) = if past {
        ymd_diff(t.date_naive(), n.date_naive())
    } else {
        ymd_diff(n.date_naive(), t.date_naive())
    };
    Ok(json!({
        "target": describe(&t, z.as_ref()),
        "now": describe(&n, z.as_ref()),
        "status": if past { "past" } else if secs == 0 { "now" } else { "future" },
        "remaining": {
            "days": total_days,
            "hours": (secs % 86400) / 3600,
            "minutes": (secs % 3600) / 60,
            "seconds": secs % 60
        },
        "calendar": { "years": y, "months": m, "days": d },
        "total": {
            "seconds": secs,
            "minutes": secs / 60,
            "hours": secs / 3600,
            "days": total_days,
            "weeks": total_days / 7,
            "weekends": count_weekend_days(n.date_naive(), t.date_naive())
        },
        "percent_of_year_remaining": if !past { Some(((t - n).num_seconds() as f64 / 31_556_952.0 * 1000.0).round() / 10.0) } else { None },
        "human": format!("{}{} days, {} hours, {} minutes", if past { "-" } else { "" }, total_days, (secs % 86400) / 3600, (secs % 3600) / 60),
    }))
}

fn count_weekend_days(a: NaiveDate, b: NaiveDate) -> i64 {
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    let mut d = lo;
    let mut n = 0;
    while d < hi {
        if matches!(d.weekday(), Weekday::Sat | Weekday::Sun) {
            n += 1;
        }
        d += Duration::days(1);
    }
    n
}

/// Month grid(s) for a year with highlights.
pub fn calendar(
    year: i32,
    month: Option<u32>,
    week_start: &str,
    highlights: &[String],
) -> EngineResult<Value> {
    if !(1..=9999).contains(&year) {
        return Err(EngineError::invalid("Year must be between 1 and 9999"));
    }
    let start_monday = !matches!(week_start.to_lowercase().as_str(), "sunday" | "sun");
    let hl: std::collections::HashSet<NaiveDate> = highlights
        .iter()
        .filter_map(|h| parse_datetime(h, None).ok().map(|(d, _, _)| d.date_naive()))
        .collect();
    let months: Vec<u32> = match month {
        Some(m) if (1..=12).contains(&m) => vec![m],
        Some(_) => return Err(EngineError::invalid("Month must be 1-12")),
        None => (1..=12).collect(),
    };
    let mut out = Vec::new();
    for m in months {
        let Some(first) = NaiveDate::from_ymd_opt(year, m, 1) else {
            return Err(EngineError::invalid(format!("Year {year} is out of range")));
        };
        let days_in_month = add_months(first, 1).pred_opt().map(|d| d.day()).unwrap_or(28);
        let lead = if start_monday {
            first.weekday().num_days_from_monday()
        } else {
            first.weekday().num_days_from_sunday()
        } as usize;
        let mut weeks: Vec<Vec<Value>> = Vec::new();
        let mut week: Vec<Value> = vec![Value::Null; lead];
        for day in 1..=days_in_month {
            let Some(d) = NaiveDate::from_ymd_opt(year, m, day) else {
                continue;
            };
            if week.len() == 7 {
                weeks.push(std::mem::take(&mut week));
            }
            week.push(json!({
                "day": day,
                "date": d.to_string(),
                "weekday": d.weekday().to_string(),
                "weekend": matches!(d.weekday(), Weekday::Sat | Weekday::Sun),
                "highlight": hl.contains(&d),
                "iso_week": d.iso_week().week()
            }));
        }
        while week.len() < 7 {
            week.push(Value::Null);
        }
        weeks.push(week);
        let header: Vec<&str> = if start_monday {
            vec!["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
        } else {
            vec!["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
        };
        let text = {
            let mut s = format!(
                "{:^21}\n{}\n",
                format!("{} {}", first.format("%B"), year),
                header.iter().map(|h| &h[..2]).collect::<Vec<_>>().join(" ")
            );
            for w in &weeks {
                s.push_str(
                    &w.iter()
                        .map(|c| {
                            c.get("day")
                                .and_then(Value::as_u64)
                                .map(|d| format!("{d:>2}"))
                                .unwrap_or_else(|| "  ".into())
                        })
                        .collect::<Vec<_>>()
                        .join(" "),
                );
                s.push('\n');
            }
            s
        };
        out.push(json!({
            "year": year,
            "month": m,
            "name": first.format("%B").to_string(),
            "days": days_in_month,
            "first_weekday": first.weekday().to_string(),
            "header": header,
            "weeks": weeks,
            "text": text
        }));
    }
    Ok(json!({
        "year": year,
        "leap_year": is_leap(year),
        "week_start": if start_monday { "monday" } else { "sunday" },
        "months": out
    }))
}

/// Finds overlapping working hours across time zones.
pub fn meeting_planner(
    date: &str,
    zones: &[String],
    start_hour: u32,
    end_hour: u32,
    duration_minutes: u32,
) -> EngineResult<Value> {
    if zones.len() < 2 {
        return Err(EngineError::invalid("Provide at least two time zones"));
    }
    if start_hour >= end_hour || end_hour > 24 {
        return Err(EngineError::invalid("Working hours must satisfy 0 <= start < end <= 24"));
    }
    let tzs: Vec<Tz> = zones
        .iter()
        .map(|z| {
            parse_tz(Some(z))?
                .ok_or_else(|| EngineError::invalid(format!("Unknown time zone `{z}`")))
        })
        .collect::<EngineResult<_>>()?;
    let (day, _, _) = parse_datetime(date, Some(&tzs[0]))?;
    let base_day = day.with_timezone(&tzs[0]).date_naive();
    let mut slots = Vec::new();
    // Scan the day in the first zone, 30-minute steps.
    for half_hours in 0..48 {
        let local_naive = base_day.and_time(
            NaiveTime::from_hms_opt(half_hours / 2, (half_hours % 2) * 30, 0)
                .unwrap_or(NaiveTime::MIN),
        );
        let Some(utc) = tzs[0]
            .from_local_datetime(&local_naive)
            .single()
            .map(|d| d.with_timezone(&Utc))
        else {
            continue;
        };
        let end_utc = utc + Duration::minutes(duration_minutes as i64);
        let mut ok_count = 0;
        let locals: Vec<Value> = tzs
            .iter()
            .map(|tz| {
                let l = utc.with_timezone(tz);
                let le = end_utc.with_timezone(tz);
                let within = l.hour() >= start_hour
                    && (le.hour() < end_hour || (le.hour() == end_hour && le.minute() == 0))
                    && le.date_naive() == l.date_naive();
                if within {
                    ok_count += 1;
                }
                json!({
                    "zone": tz.name(),
                    "start": l.format("%H:%M").to_string(),
                    "end": le.format("%H:%M").to_string(),
                    "date": l.format("%Y-%m-%d").to_string(),
                    "within_hours": within
                })
            })
            .collect();
        if ok_count >= 2 {
            slots.push(json!({
                "utc": utc.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                "participants_available": ok_count,
                "all_available": ok_count == tzs.len(),
                "local": locals
            }));
        }
    }
    slots.sort_by(|a, b| {
        b["participants_available"]
            .as_u64()
            .cmp(&a["participants_available"].as_u64())
    });
    let best: Vec<Value> = slots.iter().filter(|s| s["all_available"] == true).cloned().collect();
    Ok(json!({
        "date": base_day.to_string(),
        "zones": zones,
        "working_hours": format!("{start_hour:02}:00-{end_hour:02}:00"),
        "duration_minutes": duration_minutes,
        "best_slots": best,
        "partial_slots": slots.iter().filter(|s| s["all_available"] != true).take(20).cloned().collect::<Vec<_>>(),
        "has_common_slot": !best.is_empty()
    }))
}

/// Next run times and a description for a cron expression.
pub fn cron_next(
    expression: &str,
    now: &str,
    tz: Option<&str>,
    count: usize,
) -> EngineResult<Value> {
    let z = parse_tz(tz)?.unwrap_or(Tz::UTC);
    let (n, _, _) = parse_datetime(now, Some(&z))?;
    let expr = expression.trim();
    if expr.is_empty() {
        return Err(EngineError::empty("cron expression"));
    }
    let fields = expr.split_whitespace().count();
    // The cron crate wants 6 or 7 fields (seconds first); accept classic 5-field syntax.
    let normalized = match fields {
        5 => {
            let mut parts: Vec<String> = expr.split_whitespace().map(str::to_string).collect();
            parts[4] = unix_dow_to_cron(&parts[4]);
            format!("0 {}", parts.join(" "))
        },
        6 | 7 => {
            let mut parts: Vec<String> = expr.split_whitespace().map(str::to_string).collect();
            parts[5] = unix_dow_to_cron(&parts[5]);
            parts.join(" ")
        },
        _ => {
            return Err(EngineError::invalid(format!(
                "Cron expression must have 5 fields (minute hour day month weekday) or 6/7 with \
                 seconds; found {fields}"
            )))
        },
    };
    let schedule = cron::Schedule::from_str(&normalized)
        .map_err(|e| EngineError::invalid(format!("Invalid cron expression: {e}")))?;
    let local_now = n.with_timezone(&z);
    let runs: Vec<Value> = schedule
        .after(&local_now)
        .take(count.clamp(1, 100))
        .map(|d| {
            json!({
                "local": d.to_rfc3339(),
                "utc": d.with_timezone(&Utc).to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                "unix": d.timestamp(),
                "weekday": d.format("%A").to_string(),
                "in_seconds": (d.with_timezone(&Utc) - n).num_seconds()
            })
        })
        .collect();
    let description = describe_cron(expr);
    Ok(json!({
        "expression": expr,
        "normalized": normalized,
        "timezone": z.name(),
        "next_runs": runs,
        "description": description,
        "interval_seconds": runs.windows(2).next().map(|w| w[1]["unix"].as_i64().unwrap_or(0) - w[0]["unix"].as_i64().unwrap_or(0))
    }))
}

/// Converts Unix day-of-week numbering (0/7 = Sunday … 6 = Saturday) to the
/// `cron` crate's Quartz style numbering (1 = Sunday … 7 = Saturday).
fn unix_dow_to_cron(field: &str) -> String {
    field
        .split(',')
        .map(|part| {
            let (body, step) = match part.split_once('/') {
                Some((b, s)) => (b, Some(s)),
                None => (part, None),
            };
            let conv = |tok: &str| -> String {
                match tok.parse::<u32>() {
                    Ok(n) => ((n % 7) + 1).to_string(),
                    Err(_) => tok.to_string(),
                }
            };
            let converted = match body.split_once('-') {
                Some((a, b)) => format!("{}-{}", conv(a), conv(b)),
                None => conv(body),
            };
            match step {
                Some(s) => format!("{converted}/{s}"),
                None => converted,
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn describe_cron(expr: &str) -> String {
    let f: Vec<&str> = expr.split_whitespace().collect();
    let (min, hour, dom, mon, dow) = match f.len() {
        5 => (f[0], f[1], f[2], f[3], f[4]),
        6 | 7 => (f[1], f[2], f[3], f[4], f[5]),
        _ => return String::new(),
    };
    let names_dow = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
    let names_mon = [
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
    let part = |v: &str, unit: &str, names: Option<&[&str]>| -> String {
        if v == "*" {
            return format!("every {unit}");
        }
        if let Some(step) = v.strip_prefix("*/") {
            return format!("every {step} {unit}s");
        }
        let label = |x: &str| -> String {
            match (names, x.parse::<usize>()) {
                (Some(n), Ok(i)) if i < n.len() => n[i].to_string(),
                _ => x.to_string(),
            }
        };
        if let Some((a, b)) = v.split_once('-') {
            return format!("{unit} {} to {}", label(a), label(b));
        }
        format!("{unit} {}", v.split(',').map(label).collect::<Vec<_>>().join(", "))
    };
    let time = if min != "*"
        && hour != "*"
        && !min.contains(['/', ',', '-'])
        && !hour.contains(['/', ',', '-'])
    {
        format!("at {:0>2}:{:0>2}", hour, min)
    } else {
        format!("{}, {}", part(min, "minute", None), part(hour, "hour", None))
    };
    let mut s = time;
    if dom != "*" {
        s.push_str(&format!(", on day-of-month {dom}"));
    }
    if mon != "*" {
        s.push_str(&format!(
            ", in {}",
            part(mon, "month", Some(&names_mon)).trim_start_matches("month ")
        ));
    }
    if dow != "*" {
        s.push_str(&format!(
            ", on {}",
            part(dow, "weekday", Some(&names_dow)).trim_start_matches("weekday ")
        ));
    }
    s
}

/// Pomodoro session schedule.
pub fn pomodoro(
    start: &str,
    work_minutes: u32,
    short_break: u32,
    long_break: u32,
    sessions: u32,
    sessions_before_long: u32,
    tz: Option<&str>,
) -> EngineResult<Value> {
    if work_minutes == 0 || sessions == 0 || sessions > 48 || sessions_before_long == 0 {
        return Err(EngineError::invalid(
            "work_minutes > 0, 1 <= sessions <= 48, sessions_before_long > 0",
        ));
    }
    let z = parse_tz(tz)?;
    let (mut t, _, _) = parse_datetime(start, z.as_ref())?;
    let fmt = |d: &DateTime<Utc>| match &z {
        Some(tz) => d.with_timezone(tz).format("%H:%M").to_string(),
        None => d.format("%H:%M").to_string(),
    };
    let mut blocks = Vec::new();
    for i in 1..=sessions {
        let end = t + Duration::minutes(work_minutes as i64);
        blocks.push(json!({
            "type": "work",
            "session": i,
            "start": fmt(&t),
            "end": fmt(&end),
            "minutes": work_minutes
        }));
        t = end;
        if i < sessions {
            let (kind, len) = if i % sessions_before_long == 0 {
                ("long_break", long_break)
            } else {
                ("short_break", short_break)
            };
            let end = t + Duration::minutes(len as i64);
            blocks
                .push(json!({ "type": kind, "start": fmt(&t), "end": fmt(&end), "minutes": len }));
            t = end;
        }
    }
    let total: i64 = blocks.iter().map(|b| b["minutes"].as_i64().unwrap_or(0)).sum();
    Ok(json!({
        "blocks": blocks,
        "total_minutes": total,
        "focus_minutes": work_minutes * sessions,
        "ends_at": fmt(&t)
    }))
}

/// Formats a date with a strftime pattern.
pub fn format(input: &str, pattern: &str, tz: Option<&str>) -> EngineResult<Value> {
    let z = parse_tz(tz)?;
    let (dt, _, _) = parse_datetime(input, z.as_ref())?;
    let p = if pattern.trim().is_empty() {
        "%Y-%m-%d %H:%M:%S"
    } else {
        pattern
    };
    let formatted = match &z {
        Some(tz) => dt.with_timezone(tz).format(p).to_string(),
        None => dt.format(p).to_string(),
    };
    let presets = json!({
        "iso": dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "rfc2822": dt.to_rfc2822(),
        "date": dt.format("%Y-%m-%d").to_string(),
        "us": dt.format("%m/%d/%Y").to_string(),
        "eu": dt.format("%d/%m/%Y").to_string(),
        "long": dt.format("%A, %B %-d, %Y").to_string(),
        "time_12h": dt.format("%-I:%M %p").to_string(),
        "compact": dt.format("%Y%m%dT%H%M%SZ").to_string(),
        "http": dt.format("%a, %d %b %Y %H:%M:%S GMT").to_string(),
        "sql": dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        "log": dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
    });
    Ok(json!({ "formatted": formatted, "pattern": p, "presets": presets }))
}

fn default_count() -> usize {
    5
}
fn default_9() -> u32 {
    9
}
fn default_17() -> u32 {
    17
}
fn default_60() -> u32 {
    60
}
fn default_25() -> u32 {
    25
}
fn default_5() -> u32 {
    5
}
fn default_15() -> u32 {
    15
}
fn default_4() -> u32 {
    4
}
fn default_monday() -> String {
    "monday".into()
}

/// Every action understood by the date/time engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum DateTimeAction {
    /// Parses a date string.
    Parse {
        /// Date text.
        date: String,
        /// IANA zone used for zone-less input.
        #[serde(default)]
        timezone: Option<String>,
    },
    /// Difference between two dates.
    Difference {
        /// Start date.
        from: String,
        /// End date.
        to: String,
        /// IANA zone used for zone-less input.
        #[serde(default)]
        timezone: Option<String>,
        /// Count the end date as a full day.
        #[serde(default)]
        include_end: bool,
    },
    /// Adds/subtracts a delta.
    Add {
        /// Base date.
        date: String,
        /// Amount to add.
        #[serde(flatten)]
        delta: Delta,
        /// Subtract instead of add.
        #[serde(default)]
        subtract: bool,
        /// IANA zone used for zone-less input.
        #[serde(default)]
        timezone: Option<String>,
    },
    /// Age calculator.
    Age {
        /// Birth date.
        birth_date: String,
        /// Reference "now" (`Date.now()` formatted by the client).
        now: String,
        /// IANA zone used for zone-less input.
        #[serde(default)]
        timezone: Option<String>,
    },
    /// Time-zone conversion.
    ConvertZone {
        /// Source date-time.
        datetime: String,
        /// Source zone (default UTC / embedded offset).
        #[serde(default)]
        from: Option<String>,
        /// Target zones (a default set when empty).
        #[serde(default)]
        to: Vec<String>,
    },
    /// Time-zone list.
    Zones {
        /// Substring filter on zone names.
        #[serde(default)]
        filter: Option<String>,
        /// Reference time for offsets.
        #[serde(default)]
        now: Option<String>,
    },
    /// Unix timestamp conversion.
    Timestamp {
        /// Timestamp (seconds or milliseconds) or date text.
        input: String,
        /// Zone for the local representation.
        #[serde(default)]
        timezone: Option<String>,
    },
    /// Business-day calculator.
    BusinessDays {
        /// Start date.
        start: String,
        /// End date (count mode).
        #[serde(default)]
        end: Option<String>,
        /// Business days to add (add mode).
        #[serde(default)]
        add_days: Option<i64>,
        /// Holiday dates to skip.
        #[serde(default)]
        holidays: Vec<String>,
        /// Weekend day names (default Saturday, Sunday).
        #[serde(default)]
        weekend: Vec<String>,
        /// IANA zone used for zone-less input.
        #[serde(default)]
        timezone: Option<String>,
    },
    /// Week number lookup.
    WeekNumber {
        /// Date text.
        date: String,
        /// IANA zone used for zone-less input.
        #[serde(default)]
        timezone: Option<String>,
    },
    /// Countdown to a target.
    Countdown {
        /// Target date-time.
        target: String,
        /// Reference "now".
        now: String,
        /// IANA zone used for zone-less input.
        #[serde(default)]
        timezone: Option<String>,
        /// Recur every year (birthdays, anniversaries).
        #[serde(default)]
        yearly: bool,
    },
    /// Calendar grid.
    Calendar {
        /// Year.
        year: i32,
        /// Month (all twelve when omitted).
        #[serde(default)]
        month: Option<u32>,
        /// `monday` or `sunday`.
        #[serde(default = "default_monday")]
        week_start: String,
        /// Dates to flag as highlighted.
        #[serde(default)]
        highlights: Vec<String>,
    },
    /// Meeting planner across zones.
    MeetingPlanner {
        /// Date to plan.
        date: String,
        /// IANA zones of the participants.
        zones: Vec<String>,
        /// Working day start hour (0–23).
        #[serde(default = "default_9")]
        start_hour: u32,
        /// Working day end hour (1–24).
        #[serde(default = "default_17")]
        end_hour: u32,
        /// Meeting length.
        #[serde(default = "default_60")]
        duration_minutes: u32,
    },
    /// Cron schedule preview.
    CronNext {
        /// Cron expression (5 or 6 fields).
        expression: String,
        /// Reference "now".
        now: String,
        /// Zone the schedule runs in.
        #[serde(default)]
        timezone: Option<String>,
        /// Number of upcoming runs.
        #[serde(default = "default_count")]
        count: usize,
    },
    /// Pomodoro planner.
    Pomodoro {
        /// Start time.
        start: String,
        /// Work interval in minutes.
        #[serde(default = "default_25")]
        work_minutes: u32,
        /// Short break in minutes.
        #[serde(default = "default_5")]
        short_break: u32,
        /// Long break in minutes.
        #[serde(default = "default_15")]
        long_break: u32,
        /// Total work sessions.
        #[serde(default = "default_4")]
        sessions: u32,
        /// Sessions before a long break.
        #[serde(default = "default_4")]
        sessions_before_long: u32,
        /// IANA zone used for zone-less input.
        #[serde(default)]
        timezone: Option<String>,
    },
    /// strftime formatting.
    Format {
        /// Date text.
        date: String,
        /// strftime pattern (e.g. `%Y-%m-%d`).
        #[serde(default)]
        pattern: String,
        /// IANA zone used for zone-less input.
        #[serde(default)]
        timezone: Option<String>,
    },
}

/// Dispatches a [`DateTimeAction`] to its implementation.
pub fn handle_action(action: DateTimeAction) -> EngineResult<Value> {
    match action {
        DateTimeAction::Parse { date, timezone } => parse(&date, timezone.as_deref()),
        DateTimeAction::Difference { from, to, timezone, include_end } => {
            difference(&from, &to, timezone.as_deref(), include_end)
        },
        DateTimeAction::Add { date, delta, subtract, timezone } => {
            add(&date, &delta, subtract, timezone.as_deref())
        },
        DateTimeAction::Age { birth_date, now, timezone } => {
            age(&birth_date, &now, timezone.as_deref())
        },
        DateTimeAction::ConvertZone { datetime, from, to } => {
            convert_zone(&datetime, from.as_deref(), &to)
        },
        DateTimeAction::Zones { filter, now } => zones(filter.as_deref(), now.as_deref()),
        DateTimeAction::Timestamp { input, timezone } => timestamp(&input, timezone.as_deref()),
        DateTimeAction::BusinessDays {
            start,
            end,
            add_days,
            holidays,
            weekend,
            timezone,
        } => business_days(
            &start,
            end.as_deref(),
            add_days,
            &holidays,
            &weekend,
            timezone.as_deref(),
        ),
        DateTimeAction::WeekNumber { date, timezone } => week_number(&date, timezone.as_deref()),
        DateTimeAction::Countdown { target, now, timezone, yearly } => {
            countdown(&target, &now, timezone.as_deref(), yearly)
        },
        DateTimeAction::Calendar { year, month, week_start, highlights } => {
            calendar(year, month, &week_start, &highlights)
        },
        DateTimeAction::MeetingPlanner {
            date,
            zones,
            start_hour,
            end_hour,
            duration_minutes,
        } => meeting_planner(&date, &zones, start_hour, end_hour, duration_minutes),
        DateTimeAction::CronNext { expression, now, timezone, count } => {
            cron_next(&expression, &now, timezone.as_deref(), count)
        },
        DateTimeAction::Pomodoro {
            start,
            work_minutes,
            short_break,
            long_break,
            sessions,
            sessions_before_long,
            timezone,
        } => pomodoro(
            &start,
            work_minutes,
            short_break,
            long_break,
            sessions,
            sessions_before_long,
            timezone.as_deref(),
        ),
        DateTimeAction::Format { date, pattern, timezone } => {
            format(&date, &pattern, timezone.as_deref())
        },
    }
}

engine_shared::export_engine!(handle_action, DateTimeAction);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing() {
        assert_eq!(parse("2024-01-31T10:00:00+05:30", None).unwrap()["unix"], 1_706_675_400);
        assert_eq!(parse("1706675400", None).unwrap()["iso_utc"], "2024-01-31T04:30:00Z");
        assert_eq!(parse("1706675400000", None).unwrap()["input_kind"], "unix_ms");
        assert_eq!(parse("31/01/2024", None).unwrap()["date"], "2024-01-31");
        assert_eq!(parse("Jan 31, 2024", None).unwrap()["date"], "2024-01-31");
        assert_eq!(parse("2024-01-31 10:00", Some("Asia/Kolkata")).unwrap()["unix"], 1_706_675_400);
        assert_eq!(parse("2024-01-31", Some("IST")).unwrap()["local"]["zone"], "Asia/Kolkata");
        assert!(parse("not a date", None).is_err());
        assert!(parse("", None).is_err());
        assert!(parse("2024-01-31", Some("Mars/Olympus")).is_err());
    }

    #[test]
    fn difference_add_age() {
        let d = difference("2024-01-31", "2025-03-01", None, false).unwrap();
        assert_eq!(d["years"], 1);
        assert_eq!(d["months"], 1);
        assert_eq!(d["days"], 1);
        assert_eq!(d["total"]["days"], 395);
        assert_eq!(difference("2024-01-01", "2024-01-08", None, false).unwrap()["weekdays"], 5);
        assert_eq!(difference("2024-03-10", "2024-03-01", None, false).unwrap()["negative"], true);
        let a = add("2024-01-31", &Delta { months: 1, ..Default::default() }, false, None).unwrap();
        assert_eq!(a["result"]["date"], "2024-02-29");
        let b = add(
            "2024-03-01T10:00:00",
            &Delta { business_days: 3, ..Default::default() },
            false,
            None,
        )
        .unwrap();
        assert_eq!(b["result"]["date"], "2024-03-06");
        // DST: adding a day across the spring-forward keeps the local time.
        let dst = add(
            "2024-03-30 12:00",
            &Delta { days: 1, ..Default::default() },
            false,
            Some("Europe/Berlin"),
        )
        .unwrap();
        assert_eq!(dst["result"]["local"]["time"], "12:00:00");
        assert_eq!(dst["result"]["local"]["offset"], "+02:00");
        let ag = age("1990-05-15", "2024-01-31", None).unwrap();
        assert_eq!(ag["years"], 33);
        assert_eq!(ag["months"], 8);
        assert_eq!(ag["days"], 16);
        assert_eq!(ag["next_birthday"], "2024-05-15");
        assert_eq!(ag["zodiac"], "Taurus");
        assert!(age("2030-01-01", "2024-01-01", None).is_err());
    }

    #[test]
    fn zones_and_timestamps() {
        let c = convert_zone(
            "2024-07-01 12:00",
            Some("Europe/London"),
            &["Asia/Kolkata".into(), "America/New_York".into()],
        )
        .unwrap();
        assert_eq!(c["results"][0]["time"], "16:30");
        assert_eq!(c["results"][0]["difference_hours_from_source"], 4.5);
        assert_eq!(c["results"][1]["time"], "07:00");
        assert_eq!(c["results"][1]["is_dst"], true);
        assert_eq!(c["results"][0]["is_dst"], false);
        assert!(convert_zone("2024-07-01", None, &["Nowhere/Land".into()]).is_err());
        let t = timestamp("2024-01-01T00:00:00Z", None).unwrap();
        assert_eq!(t["unix"], 1_704_067_200);
        assert_eq!(t["excel_serial"], 45292.0);
        assert!(zones(Some("kolkata"), None).unwrap()["count"].as_u64().unwrap() >= 1);
    }

    #[test]
    fn business_week_countdown() {
        let b = business_days(
            "2024-12-23",
            Some("2025-01-02"),
            None,
            &["2024-12-25".into(), "2025-01-01".into()],
            &[],
            None,
        )
        .unwrap();
        assert_eq!(b["business_days"], 6);
        assert_eq!(b["holidays"], 2);
        let f = business_days("2024-03-01", None, Some(5), &[], &[], None).unwrap();
        assert_eq!(f["result"], "2024-03-08");
        assert!(business_days("2024-03-01", None, None, &[], &[], None).is_err());
        let w = week_number("2024-12-30", None).unwrap();
        assert_eq!(w["iso_week"], 1);
        assert_eq!(w["iso_year"], 2025);
        let cd = countdown("2024-12-25", "2024-12-20T12:00:00Z", None, false).unwrap();
        assert_eq!(cd["remaining"]["days"], 4);
        assert_eq!(cd["remaining"]["hours"], 12);
        let yearly = countdown("1990-05-15", "2024-06-01", None, true).unwrap();
        assert_eq!(yearly["target"]["date"], "2025-05-15");
        assert_eq!(countdown("2020-01-01", "2024-01-01", None, false).unwrap()["status"], "past");
    }

    #[test]
    fn calendar_meeting_cron_pomodoro() {
        let c = calendar(2024, Some(2), "monday", &["2024-02-14".into()]).unwrap();
        assert_eq!(c["months"][0]["days"], 29);
        assert_eq!(c["months"][0]["first_weekday"], "Thu");
        assert!(c["months"][0]["text"].as_str().unwrap().contains("February 2024"));
        assert!(calendar(2024, Some(13), "monday", &[]).is_err());
        assert_eq!(
            calendar(2024, None, "sunday", &[]).unwrap()["months"]
                .as_array()
                .unwrap()
                .len(),
            12
        );
        let m = meeting_planner(
            "2024-06-10",
            &["Europe/London".into(), "Asia/Kolkata".into()],
            9,
            17,
            60,
        )
        .unwrap();
        assert_eq!(m["has_common_slot"], true);
        assert_eq!(m["best_slots"][0]["local"][0]["start"], "09:00");
        assert!(meeting_planner("2024-06-10", &["UTC".into()], 9, 17, 60).is_err());
        let cr =
            cron_next("0 9 * * 1-5", "2024-06-07T12:00:00Z", Some("Europe/Berlin"), 3).unwrap();
        assert_eq!(cr["next_runs"][0]["local"], "2024-06-10T09:00:00+02:00");
        assert!(cr["description"].as_str().unwrap().contains("at 09:00"));
        assert_eq!(
            cron_next("*/15 * * * *", "2024-06-07T12:00:00Z", None, 2).unwrap()["interval_seconds"],
            900
        );
        assert!(cron_next("bad", "2024-06-07T12:00:00Z", None, 1).is_err());
        assert!(cron_next("70 * * * *", "2024-06-07T12:00:00Z", None, 1).is_err());
        let p = pomodoro("2024-01-01T09:00:00", 25, 5, 15, 4, 4, None).unwrap();
        assert_eq!(p["focus_minutes"], 100);
        assert_eq!(p["ends_at"], "10:55");
        assert_eq!(
            format("2024-01-31T10:00:00Z", "%d %B %Y", None).unwrap()["formatted"],
            "31 January 2024"
        );
    }
}
