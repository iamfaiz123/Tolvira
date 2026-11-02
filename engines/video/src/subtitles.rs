//! Subtitle parsing (SRT / WebVTT / ASS-SSA / LRC-ish), conversion, timing
//! shift & scale, validation and statistics.

use engine_shared::{EngineError, EngineResult};
use serde_json::{json, Value};

/// One subtitle cue.
#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    /// Start time in seconds.
    pub start: f64,
    /// End time in seconds.
    pub end: f64,
    /// Cue text (lines separated by `
    /// `).
    pub text: String,
    /// ASS style name, if any.
    pub style: Option<String>,
}

fn parse_time(s: &str) -> Option<f64> {
    let t = s.trim().replace(',', ".");
    let parts: Vec<&str> = t.split(':').collect();
    let secs = |x: &str| x.parse::<f64>().ok();
    match parts.len() {
        3 => Some(
            parts[0].parse::<f64>().ok()? * 3600.0
                + parts[1].parse::<f64>().ok()? * 60.0
                + secs(parts[2])?,
        ),
        2 => Some(parts[0].parse::<f64>().ok()? * 60.0 + secs(parts[1])?),
        1 => secs(parts[0]),
        _ => None,
    }
}

/// Guesses the subtitle format (`srt`, `vtt`, `ass`, `json`, `lrc`) from the content.
pub fn detect_format(text: &str) -> &'static str {
    let t = text.trim_start_matches('\u{feff}').trim_start();
    if t.starts_with("WEBVTT") {
        "vtt"
    } else if t.starts_with("[Script Info]")
        || t.contains("\n[Events]")
        || t.starts_with("[Events]")
    {
        "ass"
    } else if t.lines().take(5).any(|l| l.contains("-->")) {
        "srt"
    } else if t.starts_with('[')
        && t.lines()
            .next()
            .map(|l| l.contains(':') && l.contains(']'))
            .unwrap_or(false)
    {
        "lrc"
    } else if t.starts_with('{') || t.starts_with('[') {
        "json"
    } else {
        "unknown"
    }
}

/// Parses subtitles in the given (or detected) format into cues.
pub fn parse(text: &str, format_hint: &str) -> EngineResult<(Vec<Cue>, &'static str)> {
    engine_shared::error::require_text(text, "subtitles")?;
    engine_shared::limits::check_size(text.len(), 20 * 1024 * 1024, "subtitles")?;
    let text = text.trim_start_matches('\u{feff}');
    let fmt = if format_hint.is_empty() || format_hint == "auto" {
        detect_format(text)
    } else {
        match format_hint {
            "srt" => "srt",
            "vtt" | "webvtt" => "vtt",
            "ass" | "ssa" => "ass",
            "lrc" => "lrc",
            "json" => "json",
            other => {
                return Err(EngineError::invalid(format!(
                    "Unknown subtitle format `{other}` (srt, vtt, ass, lrc, json)"
                )))
            },
        }
    };
    let cues = match fmt {
        "srt" | "vtt" => parse_srt_vtt(text, fmt == "vtt")?,
        "ass" => parse_ass(text)?,
        "lrc" => parse_lrc(text)?,
        "json" => {
            let v: Value = serde_json::from_str(text)
                .map_err(|e| EngineError::malformed(format!("Invalid JSON: {e}")))?;
            let arr = v
                .as_array()
                .or_else(|| v.get("cues").and_then(Value::as_array))
                .ok_or_else(|| {
                    EngineError::invalid("JSON subtitles must be an array of {start, end, text}")
                })?;
            arr.iter()
                .map(|c| {
                    Ok(Cue {
                        start: c["start"]
                            .as_f64()
                            .ok_or_else(|| EngineError::invalid("cue.start must be seconds"))?,
                        end: c["end"]
                            .as_f64()
                            .ok_or_else(|| EngineError::invalid("cue.end must be seconds"))?,
                        text: c["text"].as_str().unwrap_or("").to_string(),
                        style: c["style"].as_str().map(str::to_string),
                    })
                })
                .collect::<EngineResult<Vec<_>>>()?
        },
        _ => {
            return Err(EngineError::unsupported_format(
                "Could not detect the subtitle format (expected SRT, WebVTT, ASS/SSA, LRC or JSON)",
            ))
        },
    };
    if cues.is_empty() {
        return Err(EngineError::malformed("No subtitle cues found"));
    }
    Ok((cues, fmt))
}

fn parse_srt_vtt(text: &str, vtt: bool) -> EngineResult<Vec<Cue>> {
    let mut cues = Vec::new();
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut blocks = normalized.split("\n\n").peekable();
    if vtt {
        // Skip header block(s) (WEBVTT, STYLE, NOTE, REGION).
        while let Some(b) = blocks.peek() {
            let t = b.trim_start();
            if t.starts_with("WEBVTT")
                || t.starts_with("STYLE")
                || t.starts_with("NOTE")
                || t.starts_with("REGION")
            {
                blocks.next();
            } else {
                break;
            }
        }
    }
    for block in blocks {
        let lines: Vec<&str> = block.lines().filter(|l| !l.trim().is_empty()).collect();
        if lines.is_empty() {
            continue;
        }
        if vtt
            && (lines[0].trim_start().starts_with("NOTE")
                || lines[0].trim_start().starts_with("STYLE"))
        {
            continue;
        }
        let timing_idx = lines.iter().position(|l| l.contains("-->"));
        let Some(ti) = timing_idx else { continue };
        let timing = lines[ti];
        let Some((a, b)) = timing.split_once("-->") else {
            continue;
        };
        let b_time = b.split_whitespace().next().unwrap_or("");
        let settings = b.split_whitespace().skip(1).collect::<Vec<_>>().join(" ");
        let (Some(start), Some(end)) = (parse_time(a), parse_time(b_time)) else {
            return Err(EngineError::malformed(format!("Bad timing line `{timing}`")));
        };
        let text = lines[ti + 1..].join("\n");
        cues.push(Cue {
            start,
            end,
            text,
            style: if settings.is_empty() { None } else { Some(settings) },
        });
    }
    Ok(cues)
}

fn parse_ass(text: &str) -> EngineResult<Vec<Cue>> {
    let mut cues = Vec::new();
    let mut format: Vec<String> = vec![
        "Layer", "Start", "End", "Style", "Name", "MarginL", "MarginR", "MarginV", "Effect", "Text",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let mut in_events = false;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_events = l.eq_ignore_ascii_case("[Events]");
            continue;
        }
        if !in_events {
            continue;
        }
        if let Some(f) = l.strip_prefix("Format:") {
            format = f.split(',').map(|s| s.trim().to_string()).collect();
            continue;
        }
        if let Some(d) = l.strip_prefix("Dialogue:") {
            let n = format.len();
            let parts: Vec<&str> = d.splitn(n, ',').map(str::trim).collect();
            if parts.len() < n {
                continue;
            }
            let get = |name: &str| {
                format
                    .iter()
                    .position(|f| f == name)
                    .and_then(|i| parts.get(i))
                    .copied()
                    .unwrap_or("")
            };
            let (Some(start), Some(end)) = (parse_time(get("Start")), parse_time(get("End")))
            else {
                continue;
            };
            let raw = get("Text");
            let cleaned = strip_ass_tags(raw).replace("\\N", "\n").replace("\\n", "\n");
            cues.push(Cue {
                start,
                end,
                text: cleaned,
                style: Some(get("Style").to_string()).filter(|s| !s.is_empty()),
            });
        }
    }
    Ok(cues)
}

fn strip_ass_tags(s: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in s.chars() {
        match c {
            '{' => depth += 1,
            '}' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {},
        }
    }
    out
}

fn parse_lrc(text: &str) -> EngineResult<Vec<Cue>> {
    let mut stamps: Vec<(f64, String)> = Vec::new();
    for line in text.lines() {
        let mut rest = line.trim();
        let mut times = Vec::new();
        while rest.starts_with('[') {
            let Some(close) = rest.find(']') else { break };
            let inner = &rest[1..close];
            if let Some(t) = parse_time(inner) {
                times.push(t);
            }
            rest = &rest[close + 1..];
        }
        for t in times {
            stamps.push((t, rest.trim().to_string()));
        }
    }
    stamps.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut cues = Vec::new();
    for i in 0..stamps.len() {
        let end = stamps.get(i + 1).map(|n| n.0).unwrap_or(stamps[i].0 + 3.0);
        if !stamps[i].1.is_empty() {
            cues.push(Cue {
                start: stamps[i].0,
                end,
                text: stamps[i].1.clone(),
                style: None,
            });
        }
    }
    Ok(cues)
}

fn fmt_time(t: f64, sep: char) -> String {
    let t = t.max(0.0);
    let h = (t / 3600.0).floor() as u64;
    let m = ((t % 3600.0) / 60.0).floor() as u64;
    let s = (t % 60.0).floor() as u64;
    let ms = ((t - t.floor()) * 1000.0).round() as u64;
    let (s, ms) = if ms >= 1000 { (s + 1, 0) } else { (s, ms) };
    format!("{h:02}:{m:02}:{s:02}{sep}{ms:03}")
}

/// Renders cues as SubRip.
pub fn to_srt(cues: &[Cue]) -> String {
    let mut out = String::new();
    for (i, c) in cues.iter().enumerate() {
        out.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            i + 1,
            fmt_time(c.start, ','),
            fmt_time(c.end, ','),
            c.text.trim()
        ));
    }
    out
}

/// Renders cues as WebVTT.
pub fn to_vtt(cues: &[Cue]) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for (i, c) in cues.iter().enumerate() {
        out.push_str(&format!(
            "{}\n{} --> {}{}\n{}\n\n",
            i + 1,
            fmt_time(c.start, '.'),
            fmt_time(c.end, '.'),
            c.style.as_ref().map(|s| format!(" {s}")).unwrap_or_default(),
            c.text.trim()
        ));
    }
    out
}

/// Renders cues as ASS/SSA.
pub fn to_ass(cues: &[Cue]) -> String {
    let mut out = String::from("[Script Info]\nScriptType: v4.00+\nPlayResX: 1920\nPlayResY: \
                                1080\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, \
                                PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, \
                                Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, \
                                BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, \
                                MarginV, Encoding\nStyle: \
                                Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H80000000,0,0,0,0,100,100,0,0,1,2,1,2,30,30,40,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n");
    let ass_time = |t: f64| {
        let t = t.max(0.0);
        format!(
            "{}:{:02}:{:02}.{:02}",
            (t / 3600.0).floor() as u64,
            ((t % 3600.0) / 60.0).floor() as u64,
            (t % 60.0).floor() as u64,
            ((t - t.floor()) * 100.0).round() as u64 % 100
        )
    };
    for c in cues {
        out.push_str(&format!(
            "Dialogue: 0,{},{},{},,0,0,0,,{}\n",
            ass_time(c.start),
            ass_time(c.end),
            c.style.clone().unwrap_or_else(|| "Default".into()),
            c.text.trim().replace('\n', "\\N")
        ));
    }
    out
}

/// Renders cues as a JSON array.
pub fn to_json(cues: &[Cue]) -> Value {
    Value::Array(
        cues.iter()
            .map(|c| {
                json!({
                    "start": (c.start * 1000.0).round() / 1000.0,
                    "end": (c.end * 1000.0).round() / 1000.0,
                    "text": c.text,
                    "style": c.style
                })
            })
            .collect(),
    )
}

/// Renders cues in the named format.
pub fn render(cues: &[Cue], format: &str) -> EngineResult<String> {
    Ok(match format {
        "srt" => to_srt(cues),
        "vtt" | "webvtt" => to_vtt(cues),
        "ass" | "ssa" => to_ass(cues),
        "json" => serde_json::to_string_pretty(&to_json(cues)).unwrap_or_default(),
        "txt" | "text" => cues
            .iter()
            .map(|c| c.text.replace('\n', " "))
            .collect::<Vec<_>>()
            .join("\n"),
        "csv" => {
            let mut s = String::from("start,end,text\n");
            for c in cues {
                s.push_str(&format!(
                    "{:.3},{:.3},\"{}\"\n",
                    c.start,
                    c.end,
                    c.text.replace('"', "\"\"").replace('\n', " ")
                ));
            }
            s
        },
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown output format `{other}` (srt, vtt, ass, json, txt, csv)"
            )))
        },
    })
}

/// Reports overlaps, ordering problems, reading speed and over-long cues.
pub fn validate(cues: &[Cue], max_cps: f64, max_duration: f64) -> Value {
    let mut issues = Vec::new();
    for (i, c) in cues.iter().enumerate() {
        let dur = c.end - c.start;
        if dur <= 0.0 {
            issues.push(json!({
                "cue": i + 1,
                "severity": "error",
                "message": "End time is not after start time"
            }));
        }
        if dur > max_duration {
            issues.push(json!({
                "cue": i + 1,
                "severity": "warning",
                "message": format!("Cue lasts {dur:.1}s (longer than {max_duration}s)")
            }));
        }
        if dur > 0.0 && dur < 0.5 {
            issues.push(json!({
                "cue": i + 1,
                "severity": "warning",
                "message": format!("Cue is very short ({dur:.2}s)")
            }));
        }
        let chars = c.text.chars().filter(|ch| !ch.is_whitespace()).count() as f64;
        if dur > 0.0 && chars / dur > max_cps {
            issues.push(json!({
                "cue": i + 1,
                "severity": "warning",
                "message": format!("Reading speed {:.0} chars/s exceeds {max_cps}", chars / dur)
            }));
        }
        if c.text.lines().count() > 2 {
            issues.push(
                json!({ "cue": i + 1, "severity": "info", "message": "More than two lines" }),
            );
        }
        if c.text.lines().any(|l| l.chars().count() > 42) {
            issues.push(json!({
                "cue": i + 1,
                "severity": "info",
                "message": "Line longer than 42 characters"
            }));
        }
        if c.text.trim().is_empty() {
            issues.push(json!({ "cue": i + 1, "severity": "warning", "message": "Empty cue" }));
        }
        if let Some(n) = cues.get(i + 1) {
            if n.start < c.end - 1e-6 {
                issues.push(json!({
                    "cue": i + 1,
                    "severity": "error",
                    "message": format!("Overlaps the next cue by {:.3}s", c.end - n.start)
                }));
            }
            if n.start < c.start {
                issues.push(json!({
                    "cue": i + 1,
                    "severity": "error",
                    "message": "Cues are not in chronological order"
                }));
            }
        }
    }
    let total_text: usize = cues.iter().map(|c| c.text.chars().count()).sum();
    let first = cues.first().map(|c| c.start).unwrap_or(0.0);
    let last = cues.last().map(|c| c.end).unwrap_or(0.0);
    json!({
        "valid": issues.iter().all(|i| i["severity"] != "error"),
        "issues": issues,
        "stats": {
            "cues": cues.len(),
            "characters": total_text,
            "first_start": first,
            "last_end": last,
            "span_seconds": last - first,
            "average_duration": if cues.is_empty() { 0.0 } else { cues.iter().map(|c| c.end - c.start).sum::<f64>() / cues.len() as f64 },
            "words": cues.iter().map(|c| c.text.split_whitespace().count()).sum::<usize>()
        },
    })
}

/// Shifts all cues by `seconds` and/or rescales time (`scale`, e.g. 25/23.976
/// for frame-rate conversion) around t = 0.
pub fn retime(cues: &[Cue], shift: f64, scale: f64) -> EngineResult<Vec<Cue>> {
    if scale <= 0.0 {
        return Err(EngineError::invalid("scale must be > 0"));
    }
    Ok(cues
        .iter()
        .map(|c| Cue {
            start: (c.start * scale + shift).max(0.0),
            end: (c.end * scale + shift).max(0.0),
            text: c.text.clone(),
            style: c.style.clone(),
        })
        .filter(|c| c.end > 0.0)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips() {
        let srt = "1\n00:00:01,000 --> 00:00:02,500\nHello\nworld\n\n2\n00:00:03,000 --> \
                   00:00:04,000\nBye\n";
        let (cues, f) = parse(srt, "").unwrap();
        assert_eq!(f, "srt");
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].end, 2.5);
        let vtt = to_vtt(&cues);
        assert!(vtt.starts_with("WEBVTT\n\n1\n00:00:01.000 --> 00:00:02.500\nHello\nworld"));
        let (back, f) = parse(&vtt, "").unwrap();
        assert_eq!(f, "vtt");
        assert_eq!(back, cues);
        let ass = to_ass(&cues);
        let (from_ass, f) = parse(&ass, "").unwrap();
        assert_eq!(f, "ass");
        assert_eq!(from_ass[0].text, "Hello\nworld");
        let lrc = "[00:12.00]First line\n[00:17.20]Second line\n";
        let (l, f) = parse(lrc, "").unwrap();
        assert_eq!(f, "lrc");
        assert_eq!(l[0].end, 17.2);
        let shifted = retime(&cues, 1.5, 1.0).unwrap();
        assert_eq!(shifted[0].start, 2.5);
        let scaled = retime(&cues, 0.0, 2.0).unwrap();
        assert_eq!(scaled[1].start, 6.0);
        let v = validate(
            &[
                Cue {
                    start: 1.0,
                    end: 0.5,
                    text: "x".into(),
                    style: None,
                },
                Cue {
                    start: 0.4,
                    end: 9.0,
                    text: "a".repeat(100),
                    style: None,
                },
            ],
            20.0,
            7.0,
        );
        assert_eq!(v["valid"], false);
        assert!(parse("", "").is_err());
        assert!(parse("garbage", "").is_err());
        assert!(parse("1\n00:00:01,000 --> bad\nx", "srt").is_err());
        assert!(render(&cues, "docx").is_err());
        assert!(render(&cues, "csv").unwrap().starts_with("start,end,text"));
    }
}
