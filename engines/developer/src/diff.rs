//! Text diffing (unified / side-by-side / inline word diff) and unified-patch
//! parsing & application.

use engine_shared::{EngineError, EngineResult};
use serde_json::{json, Value};
use similar::{Algorithm, ChangeTag, TextDiff};

const MAX: usize = 20 * 1024 * 1024;

fn check(a: &str, b: &str) -> EngineResult<()> {
    engine_shared::limits::check_size(a.len() + b.len(), MAX, "text")?;
    if a.is_empty() && b.is_empty() {
        return Err(EngineError::empty("text"));
    }
    Ok(())
}

/// Produces a unified diff, structured hunks for side-by-side rendering and
/// per-line inline word changes.
#[allow(clippy::too_many_arguments)]
pub fn diff(
    old: &str,
    new: &str,
    old_name: &str,
    new_name: &str,
    context: usize,
    ignore_whitespace: bool,
    ignore_case: bool,
    inline: bool,
) -> EngineResult<Value> {
    check(old, new)?;
    let norm = |s: &str| -> String {
        let mut out = String::with_capacity(s.len());
        for line in s.split_inclusive('\n') {
            let body = line.strip_suffix('\n').unwrap_or(line);
            let body = body.strip_suffix('\r').unwrap_or(body);
            let mut l = if ignore_whitespace {
                body.split_whitespace().collect::<Vec<_>>().join(" ")
            } else {
                body.to_string()
            };
            if ignore_case {
                l = l.to_lowercase();
            }
            out.push_str(&l);
            if line.ends_with('\n') {
                out.push('\n');
            }
        }
        out
    };
    let (a, b) = if ignore_whitespace || ignore_case {
        (norm(old), norm(new))
    } else {
        (old.to_string(), new.to_string())
    };
    let text_diff = TextDiff::configure()
        .algorithm(Algorithm::Myers)
        .newline_terminated(true)
        .diff_lines(&a, &b);
    let old_name = if old_name.is_empty() { "a" } else { old_name };
    let new_name = if new_name.is_empty() { "b" } else { new_name };
    let unified = text_diff
        .unified_diff()
        .context_radius(context)
        .header(old_name, new_name)
        .to_string();
    let mut added = 0;
    let mut removed = 0;
    let mut hunks = Vec::new();
    let mut rows = Vec::new();
    for (hi, group) in text_diff.grouped_ops(context).iter().enumerate() {
        let mut hunk_rows = Vec::new();
        let (mut os, mut oe, mut ns, mut ne) = (usize::MAX, 0, usize::MAX, 0);
        for op in group {
            for change in text_diff.iter_inline_changes(op) {
                let line_text: String = change
                    .iter_strings_lossy()
                    .map(|(_, s)| s.to_string())
                    .collect::<String>();
                let line_text = line_text.trim_end_matches('\n').to_string();
                let tag = match change.tag() {
                    ChangeTag::Equal => "equal",
                    ChangeTag::Delete => {
                        removed += 1;
                        "removed"
                    },
                    ChangeTag::Insert => {
                        added += 1;
                        "added"
                    },
                };
                if let Some(i) = change.old_index() {
                    os = os.min(i + 1);
                    oe = oe.max(i + 1);
                }
                if let Some(i) = change.new_index() {
                    ns = ns.min(i + 1);
                    ne = ne.max(i + 1);
                }
                let mut row = json!({
                    "type": tag,
                    "old_line": change.old_index().map(|i| i + 1),
                    "new_line": change.new_index().map(|i| i + 1),
                    "text": line_text
                });
                if inline && change.tag() != ChangeTag::Equal {
                    row["segments"] = Value::Array(
                        change
                            .iter_strings_lossy()
                            .map(|(emph, s)| {
                                json!({
                                    "text": s.trim_end_matches('\n'),
                                    "emphasized": emph
                                })
                            })
                            .collect(),
                    );
                }
                hunk_rows.push(row);
            }
        }
        rows.extend(hunk_rows.iter().cloned());
        hunks.push(json!({
            "index": hi,
            "old_start": if os == usize::MAX { 0 } else { os },
            "old_end": oe,
            "new_start": if ns == usize::MAX { 0 } else { ns },
            "new_end": ne,
            "lines": hunk_rows
        }));
    }
    let ratio = text_diff.ratio();
    Ok(json!({
        "identical": added == 0 && removed == 0,
        "similarity": (ratio * 1000.0).round() / 10.0,
        "added": added,
        "removed": removed,
        "changed_lines": added.max(removed),
        "old_lines": a.lines().count(),
        "new_lines": b.lines().count(),
        "unified": unified,
        "hunks": hunks,
        "lines": rows,
        "options": {
            "context": context,
            "ignore_whitespace": ignore_whitespace,
            "ignore_case": ignore_case
        },
    }))
}

/// Character / word level diff for short strings.
pub fn diff_words(old: &str, new: &str, mode: &str) -> EngineResult<Value> {
    check(old, new)?;
    if old.len() + new.len() > 2_000_000 {
        return Err(EngineError::limit("Inline diff is limited to 2 MB of text"));
    }
    let d = match mode {
        "chars" => TextDiff::from_chars(old, new),
        _ => TextDiff::from_words(old, new),
    };
    let segments: Vec<Value> = d.iter_all_changes().map(|c| json!({
        "type": match c.tag() { ChangeTag::Equal => "equal", ChangeTag::Delete => "removed", ChangeTag::Insert => "added" },
        "text": c.value()
    })).collect();
    let mut merged: Vec<Value> = Vec::new();
    for s in segments {
        if let Some(last) = merged.last_mut() {
            if last["type"] == s["type"] {
                let joined = format!(
                    "{}{}",
                    last["text"].as_str().unwrap_or(""),
                    s["text"].as_str().unwrap_or("")
                );
                last["text"] = json!(joined);
                continue;
            }
        }
        merged.push(s);
    }
    Ok(json!({
        "similarity": (d.ratio() * 1000.0).round() / 10.0,
        "segments": merged,
        "mode": if mode == "chars" { "chars" } else { "words" }
    }))
}

// ------------------------------------------------------------------ patch --

/// One hunk of a unified diff.
#[derive(Debug, Clone)]
pub struct Hunk {
    /// First line in the old file.
    pub old_start: usize,
    /// Number of old lines.
    pub old_count: usize,
    /// First line in the new file.
    pub new_start: usize,
    /// Number of new lines.
    pub new_count: usize,
    /// Text after the `@@ … @@` range (function context).
    pub header: String,
    /// Hunk lines as `(prefix, text)` with prefix ` `, `+` or `-`.
    pub lines: Vec<(char, String)>,
}

/// One file section of a patch.
#[derive(Debug, Clone, Default)]
pub struct PatchFile {
    /// Path in the old tree (`/dev/null` for new files).
    pub old_path: String,
    /// Path in the new tree.
    pub new_path: String,
    /// Hunks in order.
    pub hunks: Vec<Hunk>,
    /// File is created by the patch.
    pub is_new: bool,
    /// File is deleted by the patch.
    pub is_deleted: bool,
    /// File is renamed by the patch (git extended headers).
    pub is_rename: bool,
    /// Binary patch (hunks are not available).
    pub is_binary: bool,
    /// `old mode → new mode` from git headers.
    pub mode_change: Option<String>,
    /// Raw header lines preceding the first hunk.
    pub header_lines: Vec<String>,
}

fn parse_range(s: &str) -> Option<(usize, usize)> {
    let s = s.trim_start_matches(['-', '+']);
    match s.split_once(',') {
        Some((a, b)) => Some((a.parse().ok()?, b.parse().ok()?)),
        None => Some((s.parse().ok()?, 1)),
    }
}

/// Parses a unified diff (plain or `git diff` style) into files and hunks.
pub fn parse_patch_text(text: &str) -> EngineResult<Vec<PatchFile>> {
    engine_shared::error::require_text(text, "patch")?;
    engine_shared::limits::check_size(text.len(), MAX, "patch")?;
    let mut files: Vec<PatchFile> = Vec::new();
    let mut current: Option<PatchFile> = None;
    let mut hunk: Option<Hunk> = None;
    let lines: Vec<&str> = text.lines().collect();
    let finish_hunk = |cur: &mut Option<PatchFile>, h: &mut Option<Hunk>| {
        if let (Some(f), Some(hk)) = (cur.as_mut(), h.take()) {
            f.hunks.push(hk);
        }
    };
    let strip_prefix = |p: &str| -> String {
        let p = p.split('\t').next().unwrap_or(p).trim();
        let p = p.strip_prefix("a/").or_else(|| p.strip_prefix("b/")).unwrap_or(p);
        p.to_string()
    };
    for line in lines {
        if let Some(git_rest) = line.strip_prefix("diff --git ") {
            finish_hunk(&mut current, &mut hunk);
            if let Some(f) = current.take() {
                files.push(f);
            }
            let mut f = PatchFile::default();
            let parts: Vec<&str> = git_rest.split_whitespace().collect();
            if parts.len() >= 2 {
                f.old_path = strip_prefix(parts[0]);
                f.new_path = strip_prefix(parts[1]);
            }
            f.header_lines.push(line.to_string());
            current = Some(f);
            continue;
        }
        if let Some(rest) = line.strip_prefix("--- ") {
            if hunk.is_none() {
                let f = current.get_or_insert_with(PatchFile::default);
                let p = strip_prefix(rest);
                if p != "/dev/null" {
                    f.old_path = p;
                } else {
                    f.is_new = true;
                }
                f.header_lines.push(line.to_string());
                continue;
            }
        }
        if let Some(rest) = line.strip_prefix("+++ ") {
            if hunk.is_none() {
                if let Some(f) = current.as_mut() {
                    let p = strip_prefix(rest);
                    if p != "/dev/null" {
                        f.new_path = p;
                    } else {
                        f.is_deleted = true;
                    }
                    f.header_lines.push(line.to_string());
                    continue;
                }
            }
        }
        if line.starts_with("@@") {
            finish_hunk(&mut current, &mut hunk);
            if current.is_none() {
                current = Some(PatchFile::default());
            }
            let body = line.trim_start_matches('@').trim();
            let mut it = body.splitn(3, ' ');
            let old = it
                .next()
                .and_then(parse_range)
                .ok_or_else(|| EngineError::malformed(format!("Bad hunk header `{line}`")))?;
            let new = it
                .next()
                .and_then(parse_range)
                .ok_or_else(|| EngineError::malformed(format!("Bad hunk header `{line}`")))?;
            let header = body.split_once("@@").map(|x| x.1.trim()).unwrap_or("").to_string();
            hunk = Some(Hunk {
                old_start: old.0,
                old_count: old.1,
                new_start: new.0,
                new_count: new.1,
                header,
                lines: vec![],
            });
            continue;
        }
        if let Some(h) = hunk.as_mut() {
            if let Some(first) = line.chars().next() {
                match first {
                    ' ' | '+' | '-' => h.lines.push((first, line[1..].to_string())),
                    '\\' => {}, // "\ No newline at end of file"
                    _ => {
                        // Context line with missing leading space (some tools strip it) or end of hunk.
                        if h.lines.len() < h.old_count.max(h.new_count) {
                            h.lines.push((' ', line.to_string()));
                        } else {
                            finish_hunk(&mut current, &mut hunk);
                        }
                    },
                }
            } else {
                h.lines.push((' ', String::new()));
            }
            continue;
        }
        if let Some(f) = current.as_mut() {
            if line.starts_with("new file mode") {
                f.is_new = true;
            } else if line.starts_with("deleted file mode") {
                f.is_deleted = true;
            } else if line.starts_with("rename from")
                || line.starts_with("rename to")
                || line.starts_with("similarity index")
            {
                f.is_rename = true;
            } else if line.starts_with("Binary files") || line.starts_with("GIT binary patch") {
                f.is_binary = true;
            } else if line.starts_with("old mode") || line.starts_with("new mode") {
                f.mode_change = Some(line.to_string());
            }
            if !line.is_empty() {
                f.header_lines.push(line.to_string());
            }
        }
    }
    finish_hunk(&mut current, &mut hunk);
    if let Some(f) = current.take() {
        files.push(f);
    }
    if files.is_empty() || files.iter().all(|f| f.hunks.is_empty() && !f.is_binary) {
        return Err(EngineError::malformed(
            "No hunks found; expected a unified diff with `@@ -a,b +c,d @@` headers",
        ));
    }
    Ok(files)
}

/// Parses a unified/git diff into files, hunks and statistics.
pub fn parse_patch(text: &str) -> EngineResult<Value> {
    let files = parse_patch_text(text)?;
    let mut total_add = 0;
    let mut total_del = 0;
    let files_json: Vec<Value> = files
        .iter()
        .map(|f| {
            let add: usize = f.hunks.iter().map(|h| h.lines.iter().filter(|(c, _)| *c == '+').count()).sum();
            let del: usize = f.hunks.iter().map(|h| h.lines.iter().filter(|(c, _)| *c == '-').count()).sum();
            total_add += add;
            total_del += del;
            let mut problems = Vec::new();
            for (i, h) in f.hunks.iter().enumerate() {
                let oc = h.lines.iter().filter(|(c, _)| *c != '+').count();
                let nc = h.lines.iter().filter(|(c, _)| *c != '-').count();
                if oc != h.old_count || nc != h.new_count {
                    problems.push(format!("Hunk {} header says -{},{} +{},{} but body has \
                                           {oc}/{nc} lines", i + 1, h.old_start, h.old_count, h.new_start, h.new_count));
                }
            }
            json!({
                "old_path": f.old_path,
                "new_path": f.new_path,
                "status": if f.is_binary { "binary" } else if f.is_new { "added" } else if f.is_deleted { "deleted" } else if f.is_rename { "renamed" } else { "modified" },
                "additions": add,
                "deletions": del,
                "hunk_count": f.hunks.len(),
                "mode_change": f.mode_change,
                "problems": problems,
                "hunks": f.hunks.iter().map(|h| json!({ "old_start": h.old_start, "old_count": h.old_count, "new_start": h.new_start, "new_count": h.new_count, "section": h.header, "lines": h.lines.iter().map(|(c, t)| json!({ "type": match c { '+' => "added", '-' => "removed", _ => "context" }, "text": t })).collect::<Vec<_>>() })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let valid = files_json
        .iter()
        .all(|f| f["problems"].as_array().map(|p| p.is_empty()).unwrap_or(true));
    Ok(json!({
        "files": files_json,
        "file_count": files.len(),
        "additions": total_add,
        "deletions": total_del,
        "valid": valid,
        "format": if files.iter().any(|f| f.header_lines.iter().any(|l| l.starts_with("diff --git"))) { "git" } else { "unified" }
    }))
}

/// Applies a single-file unified patch to `text` (with fuzz: searches nearby lines for context).
pub fn apply_patch(text: &str, patch: &str, reverse: bool, fuzz: usize) -> EngineResult<Value> {
    let files = parse_patch_text(patch)?;
    if files.len() != 1 {
        return Err(EngineError::invalid(format!(
            "Patch touches {} files; apply_patch works on one file at a time",
            files.len()
        )));
    }
    let file = &files[0];
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let trailing_newline = text.ends_with('\n') || text.is_empty();
    let mut offset: isize = 0;
    let mut applied = Vec::new();
    let mut rejected = Vec::new();
    for (hi, h) in file.hunks.iter().enumerate() {
        let (old_lines, new_lines): (Vec<&String>, Vec<&String>) = if reverse {
            (
                h.lines.iter().filter(|(c, _)| *c != '-').map(|(_, t)| t).collect(),
                h.lines.iter().filter(|(c, _)| *c != '+').map(|(_, t)| t).collect(),
            )
        } else {
            (
                h.lines.iter().filter(|(c, _)| *c != '+').map(|(_, t)| t).collect(),
                h.lines.iter().filter(|(c, _)| *c != '-').map(|(_, t)| t).collect(),
            )
        };
        let start = if reverse { h.new_start } else { h.old_start };
        let expected = (start as isize - 1 + offset).max(0) as usize;
        let matches_at = |pos: usize| -> bool {
            pos + old_lines.len() <= lines.len()
                && old_lines.iter().enumerate().all(|(i, l)| lines[pos + i] == **l)
        };
        let mut found = None;
        if old_lines.is_empty() {
            found = Some(expected.min(lines.len()));
        } else {
            let radius = fuzz.max(1) * 50;
            for d in 0..=radius {
                if expected >= d && matches_at(expected - d) {
                    found = Some(expected - d);
                    break;
                }
                if matches_at(expected + d) {
                    found = Some(expected + d);
                    break;
                }
            }
            if found.is_none() && fuzz > 0 {
                // Trim context lines on both ends (fuzz levels).
                let ctx_lead = old_lines.len().min(fuzz);
                for trim in 1..=ctx_lead {
                    let core = &old_lines[trim..old_lines.len().saturating_sub(trim)];
                    if core.is_empty() {
                        break;
                    }
                    let m = |pos: usize| {
                        pos + core.len() <= lines.len()
                            && core.iter().enumerate().all(|(i, l)| lines[pos + i] == **l)
                    };
                    if let Some(pos) = (expected.saturating_sub(radius)
                        ..=(expected + radius).min(lines.len()))
                        .find(|p| m(*p))
                    {
                        // Apply with trimmed context.
                        let new_core: Vec<String> = new_lines
                            [trim.min(new_lines.len())..new_lines.len().saturating_sub(trim)]
                            .iter()
                            .map(|s| s.to_string())
                            .collect();
                        lines.splice(pos..pos + core.len(), new_core.iter().cloned());
                        offset += new_core.len() as isize - core.len() as isize;
                        applied.push(json!({ "hunk": hi + 1, "at_line": pos + 1, "fuzz": trim }));
                        found = Some(usize::MAX);
                        break;
                    }
                }
            }
        }
        match found {
            Some(usize::MAX) => {},
            Some(pos) => {
                lines.splice(pos..pos + old_lines.len(), new_lines.iter().map(|s| s.to_string()));
                offset += new_lines.len() as isize - old_lines.len() as isize;
                applied.push(json!({
                    "hunk": hi + 1,
                    "at_line": pos + 1,
                    "offset": pos as isize - (start as isize - 1),
                    "fuzz": 0
                }));
            },
            None => rejected.push(json!({
                "hunk": hi + 1,
                "expected_line": start,
                "reason": "Context does not match the input text"
            })),
        }
    }
    let mut result = lines.join("\n");
    if trailing_newline && !result.is_empty() {
        result.push('\n');
    }
    Ok(json!({
        "result": result,
        "applied": applied,
        "rejected": rejected,
        "success": rejected.is_empty(),
        "hunks": file.hunks.len(),
        "file": if file.new_path.is_empty() { file.old_path.clone() } else { file.new_path.clone() }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_diff() {
        let d =
            diff("a\nb\nc\n", "a\nB\nc\nd\n", "old.txt", "new.txt", 3, false, false, true).unwrap();
        assert_eq!(d["added"], 2);
        assert_eq!(d["removed"], 1);
        assert!(d["unified"]
            .as_str()
            .unwrap()
            .starts_with("--- old.txt\n+++ new.txt\n@@ -1,3 +1,4 @@"));
        assert_eq!(diff("a\n", "A\n", "", "", 3, false, true, false).unwrap()["identical"], true);
        assert_eq!(
            diff("a  b\n", "a b\n", "", "", 3, true, false, false).unwrap()["identical"],
            true
        );
        assert!(diff("", "", "", "", 3, false, false, false).is_err());
        let w = diff_words("the quick fox", "the slow fox", "").unwrap();
        assert_eq!(w["segments"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn patch_roundtrip() {
        let old = "line1\nline2\nline3\nline4\nline5\n";
        let new = "line1\nline2 changed\nline3\nline4\nline5\nline6\n";
        let d = diff(old, new, "f.txt", "f.txt", 1, false, false, false).unwrap();
        let patch = d["unified"].as_str().unwrap();
        let parsed = parse_patch(patch).unwrap();
        assert_eq!(parsed["file_count"], 1);
        assert_eq!(parsed["additions"], 2);
        assert_eq!(parsed["valid"], true);
        let applied = apply_patch(old, patch, false, 2).unwrap();
        assert_eq!(applied["result"], new);
        let reversed = apply_patch(new, patch, true, 2).unwrap();
        assert_eq!(reversed["result"], old);
        // Shifted context still applies.
        let shifted = format!("extra\nextra\n{old}");
        let a = apply_patch(&shifted, patch, false, 2).unwrap();
        assert_eq!(a["success"], true);
        assert_eq!(a["applied"][0]["offset"], 2);
        let bad = apply_patch("totally different\n", patch, false, 0).unwrap();
        assert_eq!(bad["success"], false);
        assert!(parse_patch("not a patch").is_err());
        let git = parse_patch(
            "diff --git a/x.rs b/x.rs\nnew file mode 100644\n--- /dev/null\n+++ b/x.rs\n@@ -0,0 \
             +1,2 @@\n+fn main() {}\n+\n",
        )
        .unwrap();
        assert_eq!(git["files"][0]["status"], "added");
        assert_eq!(git["format"], "git");
    }
}
