//! Semantic versioning: parse, compare, range matching, bumping, sorting.

use engine_shared::{EngineError, EngineResult};
use semver::{Version, VersionReq};
use serde_json::{json, Value};

fn parse_version(s: &str) -> EngineResult<Version> {
    let t = s.trim().trim_start_matches(['v', 'V', '=']);
    engine_shared::error::require_text(t, "version")?;
    // Tolerate partial versions like `1.2` or `1`.
    let parts = t.split(['-', '+']).next().unwrap_or(t).split('.').count();
    let padded = match parts {
        1 if !t.contains(['-', '+']) => format!("{t}.0.0"),
        2 if !t.contains(['-', '+']) => format!("{t}.0"),
        _ => t.to_string(),
    };
    Version::parse(&padded)
        .map_err(|e| EngineError::invalid(format!("`{s}` is not a valid semantic version: {e}")))
}

fn version_json(v: &Version, original: &str) -> Value {
    json!({
        "input": original,
        "version": v.to_string(),
        "major": v.major,
        "minor": v.minor,
        "patch": v.patch,
        "prerelease": if v.pre.is_empty() { Value::Null } else { json!(v.pre.as_str()) },
        "build": if v.build.is_empty() { Value::Null } else { json!(v.build.as_str()) },
        "is_prerelease": !v.pre.is_empty(),
        "is_stable": v.major > 0 && v.pre.is_empty(),
        "is_initial_development": v.major == 0,
    })
}

/// Parses a version; optionally compares against another version and/or a range.
pub fn check(version: &str, other: Option<&str>, range: Option<&str>) -> EngineResult<Value> {
    let v = parse_version(version)?;
    let mut out = version_json(&v, version);
    out["bumps"] = json!({
        "major": Version::new(v.major + 1, 0, 0).to_string(),
        "minor": Version::new(v.major, v.minor + 1, 0).to_string(),
        "patch": Version::new(v.major, v.minor, v.patch + 1).to_string(),
        "release": Version::new(v.major, v.minor, v.patch).to_string(),
        "premajor": format!("{}.0.0-alpha.1", v.major + 1),
        "preminor": format!("{}.{}.0-alpha.1", v.major, v.minor + 1),
        "prepatch": format!("{}.{}.{}-alpha.1", v.major, v.minor, v.patch + 1),
        "prerelease": bump_prerelease(&v),
    });
    if let Some(o) = other.filter(|o| !o.trim().is_empty()) {
        let ov = parse_version(o)?;
        let ord = v.cmp_precedence(&ov);
        out["comparison"] = json!({
            "other": ov.to_string(),
            "result": match ord { std::cmp::Ordering::Less => "lower", std::cmp::Ordering::Equal => "equal", std::cmp::Ordering::Greater => "higher" },
            "symbol": match ord { std::cmp::Ordering::Less => "<", std::cmp::Ordering::Equal => "=", std::cmp::Ordering::Greater => ">" },
            "difference": diff_kind(&v, &ov),
            "breaking_change": if v.major != ov.major { true } else { v.major == 0 && v.minor != ov.minor },
        });
    }
    if let Some(r) = range.filter(|r| !r.trim().is_empty()) {
        let req = parse_range(r)?;
        out["range"] = json!({
            "input": r,
            "normalized": req.to_string(),
            "satisfied": req.matches(&v),
            "comparators": req.comparators.iter().map(|c| c.to_string()).collect::<Vec<_>>(),
            "explanation": explain_range(r)
        });
    }
    Ok(out)
}

fn bump_prerelease(v: &Version) -> String {
    if v.pre.is_empty() {
        return format!("{}.{}.{}-alpha.1", v.major, v.minor, v.patch + 1);
    }
    let pre = v.pre.as_str();
    let mut parts: Vec<&str> = pre.split('.').collect();
    let bumped;
    match parts.last().and_then(|l| l.parse::<u64>().ok()) {
        Some(n) => {
            bumped = (n + 1).to_string();
            parts.pop();
            parts.push(&bumped);
        },
        None => parts.push("1"),
    }
    format!("{}.{}.{}-{}", v.major, v.minor, v.patch, parts.join("."))
}

fn diff_kind(a: &Version, b: &Version) -> &'static str {
    if a.major != b.major {
        "major"
    } else if a.minor != b.minor {
        "minor"
    } else if a.patch != b.patch {
        "patch"
    } else if a.pre != b.pre {
        "prerelease"
    } else if a.build != b.build {
        "build"
    } else {
        "none"
    }
}

/// Accepts npm-style ranges (`^1.2.3`, `~1.2`, `>=1.0.0 <2.0.0`, `1.x`, `1.2.*`, `||` unions handled per alternative).
pub fn parse_range(r: &str) -> EngineResult<VersionReq> {
    let t = r.trim();
    engine_shared::error::require_text(t, "range")?;
    let normalized = t.replace(" - ", "..").replace(['x', 'X'], "*");
    // Hyphen ranges: `1.2.3 - 2.3.4` → `>=1.2.3, <=2.3.4`
    let normalized = if let Some((a, b)) = normalized.split_once("..") {
        format!(">={}, <={}", a.trim(), b.trim())
    } else {
        // Space separated comparators → comma separated (semver crate syntax).
        let toks: Vec<&str> = normalized.split_whitespace().collect();
        if toks.len() > 1 && !normalized.contains(',') {
            toks.join(", ")
        } else {
            normalized
        }
    };
    if normalized.contains("||") {
        return Err(EngineError::unsupported(
            "Union ranges (`||`) are not supported; check each alternative separately",
        ));
    }
    VersionReq::parse(&normalized)
        .map_err(|e| EngineError::invalid(format!("`{r}` is not a valid version range: {e}")))
}

fn explain_range(r: &str) -> String {
    let t = r.trim();
    if let Some(rest) = t.strip_prefix('^') {
        return format!(
            "Caret: compatible with {rest} – allows changes that do not modify the left-most \
             non-zero component"
        );
    }
    if let Some(rest) = t.strip_prefix('~') {
        return format!(
            "Tilde: approximately {rest} – allows patch-level changes (minor too if only major \
             given)"
        );
    }
    if t.contains(['x', 'X', '*']) {
        return "Wildcard: any value in the starred/x position".into();
    }
    if t.contains(" - ") {
        return "Hyphen range: inclusive between the two versions".into();
    }
    if t.starts_with(['>', '<', '=']) {
        return "Comparator set: every comparator must be satisfied".into();
    }
    "Exact version (equivalent to a caret range in npm/Cargo defaults)".into()
}

/// Sorts a list of versions; reports invalid entries and the latest stable.
pub fn sort(versions: &[String], descending: bool) -> EngineResult<Value> {
    if versions.is_empty() {
        return Err(EngineError::empty("versions"));
    }
    let mut parsed: Vec<(Version, String)> = Vec::new();
    let mut invalid = Vec::new();
    for v in versions {
        match parse_version(v) {
            Ok(p) => parsed.push((p, v.clone())),
            Err(_) => invalid.push(v.clone()),
        }
    }
    parsed.sort_by(|a, b| a.0.cmp_precedence(&b.0));
    if descending {
        parsed.reverse();
    }
    let latest_stable = parsed
        .iter()
        .filter(|(v, _)| v.pre.is_empty())
        .max_by(|a, b| a.0.cmp_precedence(&b.0))
        .map(|(v, _)| v.to_string());
    let latest = parsed
        .iter()
        .max_by(|a, b| a.0.cmp_precedence(&b.0))
        .map(|(v, _)| v.to_string());
    Ok(json!({
        "sorted": parsed.iter().map(|(v, o)| json!({ "input": o, "version": v.to_string() })).collect::<Vec<_>>(),
        "invalid": invalid,
        "latest": latest,
        "latest_stable": latest_stable,
        "count": parsed.len()
    }))
}

/// Picks the highest version in a list that satisfies a range.
pub fn max_satisfying(
    versions: &[String],
    range: &str,
    include_prerelease: bool,
) -> EngineResult<Value> {
    let req = parse_range(range)?;
    let mut best: Option<Version> = None;
    let mut matching = Vec::new();
    for v in versions {
        if let Ok(p) = parse_version(v) {
            if !include_prerelease && !p.pre.is_empty() {
                continue;
            }
            if req.matches(&p) {
                matching.push(p.to_string());
                if best
                    .as_ref()
                    .map(|b| p.cmp_precedence(b) == std::cmp::Ordering::Greater)
                    .unwrap_or(true)
                {
                    best = Some(p);
                }
            }
        }
    }
    Ok(json!({
        "range": req.to_string(),
        "max_satisfying": best.map(|b| b.to_string()),
        "matching": matching
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_checks() {
        let c = check("v1.2.3-beta.1+build5", Some("1.2.3"), Some("^1.0.0")).unwrap();
        assert_eq!(c["prerelease"], "beta.1");
        assert_eq!(c["comparison"]["result"], "lower");
        assert_eq!(c["bumps"]["prerelease"], "1.2.3-beta.2");
        assert_eq!(check("1.2", None, Some(">=1.0.0 <2.0.0")).unwrap()["range"]["satisfied"], true);
        assert_eq!(check("2.0.0", None, Some("1.x")).unwrap()["range"]["satisfied"], false);
        assert_eq!(
            check("1.5.0", None, Some("1.2.3 - 2.0.0")).unwrap()["range"]["satisfied"],
            true
        );
        assert!(check("abc", None, None).is_err());
        let s = sort(&["1.10.0".into(), "1.2.0".into(), "2.0.0-rc.1".into(), "junk".into()], false)
            .unwrap();
        assert_eq!(s["sorted"][2]["version"], "2.0.0-rc.1");
        assert_eq!(s["latest_stable"], "1.10.0");
        assert_eq!(s["invalid"][0], "junk");
        assert_eq!(
            max_satisfying(&["1.0.0".into(), "1.4.2".into(), "2.0.0".into()], "^1.0.0", false)
                .unwrap()["max_satisfying"],
            "1.4.2"
        );
    }
}
