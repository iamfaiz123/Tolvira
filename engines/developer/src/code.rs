#![allow(clippy::type_complexity)]
//! Code analysis: line counting, complexity heuristics, duplicate detection,
//! import sorting / unused-import checks, dependency manifest scanning and a
//! SQL query analyser. Language detection is extension/heuristic based.

use engine_shared::{EngineError, EngineResult};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};

const MAX: usize = 20 * 1024 * 1024;

/// A source file passed to the code-analysis actions.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct SourceFile {
    /// File name (used for language detection).
    pub name: String,
    /// File content.
    pub content: String,
    /// Language override (`js`, `ts`, `python`, `rust`, `go`, `java`, …); detected from the name
    /// when empty.
    pub language: String,
}

#[derive(Clone, Copy)]
struct Lang {
    id: &'static str,
    exts: &'static [&'static str],
    line: &'static [&'static str],
    block: Option<(&'static str, &'static str)>,
    functions: &'static str,
}

const LANGS: &[Lang] = &[
    Lang {
        id: "javascript",
        exts: &["js", "mjs", "cjs", "jsx"],
        line: &["//"],
        block: Some(("/*", "*/")),
        functions: r"function\s+([A-Za-z_$][\w$]*)|([A-Za-z_$][\w$]*)\s*=\s*(?:async\s*)?(?:function|\([^)]*\)\s*=>|[A-Za-z_$][\w$]*\s*=>)|^\s*(?:async\s+)?([A-Za-z_$][\w$]*)\s*\([^)]*\)\s*\{",
    },
    Lang {
        id: "typescript",
        exts: &["ts", "tsx", "mts", "cts"],
        line: &["//"],
        block: Some(("/*", "*/")),
        functions: r"function\s+([A-Za-z_$][\w$]*)|([A-Za-z_$][\w$]*)\s*=\s*(?:async\s*)?(?:function|\([^)]*\)(?::\s*[^=]+)?\s*=>|[A-Za-z_$][\w$]*\s*=>)|^\s*(?:public|private|protected|static|async|\s)*([A-Za-z_$][\w$]*)\s*\([^)]*\)(?:\s*:\s*[^{]+)?\s*\{",
    },
    Lang {
        id: "python",
        exts: &["py", "pyw"],
        line: &["#"],
        block: Some(("\"\"\"", "\"\"\"")),
        functions: r"^\s*(?:async\s+)?def\s+([A-Za-z_]\w*)",
    },
    Lang {
        id: "rust",
        exts: &["rs"],
        line: &["//"],
        block: Some(("/*", "*/")),
        functions: r"\bfn\s+([A-Za-z_]\w*)",
    },
    Lang {
        id: "go",
        exts: &["go"],
        line: &["//"],
        block: Some(("/*", "*/")),
        functions: r"^\s*func\s+(?:\([^)]*\)\s*)?([A-Za-z_]\w*)",
    },
    Lang {
        id: "java",
        exts: &["java", "kt", "kts", "scala"],
        line: &["//"],
        block: Some(("/*", "*/")),
        functions: r"^\s*(?:public|private|protected|static|final|abstract|synchronized|override|fun|\s)*[\w<>\[\],\s?]*\s+([A-Za-z_]\w*)\s*\([^)]*\)\s*(?:throws\s+[\w,\s]+)?\s*\{",
    },
    Lang {
        id: "c",
        exts: &["c", "h", "cpp", "cc", "cxx", "hpp", "hh", "cs", "m", "mm", "swift"],
        line: &["//"],
        block: Some(("/*", "*/")),
        functions: r"^\s*[\w\*&:<>,\s]+?\s+\**([A-Za-z_]\w*)\s*\([^;{]*\)\s*(?:const\s*)?\{",
    },
    Lang {
        id: "php",
        exts: &["php"],
        line: &["//", "#"],
        block: Some(("/*", "*/")),
        functions: r"\bfunction\s+&?([A-Za-z_]\w*)",
    },
    Lang {
        id: "ruby",
        exts: &["rb", "rake"],
        line: &["#"],
        block: Some(("=begin", "=end")),
        functions: r"^\s*def\s+(?:self\.)?([A-Za-z_]\w*[?!=]?)",
    },
    Lang {
        id: "shell",
        exts: &["sh", "bash", "zsh"],
        line: &["#"],
        block: None,
        functions: r"^\s*(?:function\s+)?([A-Za-z_]\w*)\s*\(\)\s*\{",
    },
    Lang {
        id: "sql",
        exts: &["sql"],
        line: &["--"],
        block: Some(("/*", "*/")),
        functions: r"CREATE\s+(?:OR\s+REPLACE\s+)?(?:FUNCTION|PROCEDURE)\s+([\w.]+)",
    },
    Lang {
        id: "css",
        exts: &["css", "scss", "less"],
        line: &["//"],
        block: Some(("/*", "*/")),
        functions: r"^$",
    },
    Lang {
        id: "html",
        exts: &["html", "htm", "vue", "svelte", "xml"],
        line: &[],
        block: Some(("<!--", "-->")),
        functions: r"^$",
    },
    Lang {
        id: "yaml",
        exts: &["yml", "yaml", "toml", "ini", "cfg"],
        line: &["#"],
        block: None,
        functions: r"^$",
    },
    Lang {
        id: "json",
        exts: &["json"],
        line: &[],
        block: None,
        functions: r"^$",
    },
    Lang {
        id: "markdown",
        exts: &["md", "markdown", "txt"],
        line: &[],
        block: Some(("<!--", "-->")),
        functions: r"^$",
    },
    Lang {
        id: "lua",
        exts: &["lua"],
        line: &["--"],
        block: Some(("--[[", "]]")),
        functions: r"\bfunction\s+([\w.:]+)",
    },
    Lang {
        id: "dart",
        exts: &["dart"],
        line: &["//"],
        block: Some(("/*", "*/")),
        functions: r"^\s*[\w<>\[\]?]+\s+([A-Za-z_]\w*)\s*\([^)]*\)\s*(?:async\s*)?\{",
    },
];

fn detect_lang(file: &SourceFile) -> Lang {
    if !file.language.is_empty() {
        let l = file.language.to_ascii_lowercase();
        if let Some(found) = LANGS.iter().find(|x| x.id == l || x.exts.contains(&l.as_str())) {
            return *found;
        }
    }
    let ext = file.name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    if let Some(found) = LANGS.iter().find(|x| x.exts.contains(&ext.as_str())) {
        return *found;
    }
    // Content heuristics.
    let c = &file.content;
    if c.contains("def ") && c.contains(':') && !c.contains(';') {
        return LANGS[2];
    }
    if c.contains("fn ") && c.contains("let ") {
        return LANGS[3];
    }
    if c.contains("package main") || c.contains("func ") {
        return LANGS[4];
    }
    if c.contains("function") || c.contains("const ") || c.contains("=>") {
        return LANGS[0];
    }
    LANGS[0]
}

fn check_files(files: &[SourceFile]) -> EngineResult<()> {
    if files.is_empty() || files.iter().all(|f| f.content.trim().is_empty()) {
        return Err(EngineError::empty("files"));
    }
    engine_shared::limits::check_size(files.iter().map(|f| f.content.len()).sum(), MAX, "source")?;
    engine_shared::limits::check_count(files.len(), 2000, "files")
}

/// Classifies each line as code / comment / blank.
fn classify_lines(content: &str, lang: &Lang) -> (usize, usize, usize, Vec<bool>) {
    let (mut code, mut comment, mut blank) = (0, 0, 0);
    let mut in_block = false;
    let mut is_code = Vec::new();
    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() {
            blank += 1;
            is_code.push(false);
            continue;
        }
        if in_block {
            comment += 1;
            is_code.push(false);
            if let Some((_, end)) = lang.block {
                if line.contains(end) {
                    in_block = false;
                }
            }
            continue;
        }
        if let Some((start, end)) = lang.block {
            if let Some(rest) = line.strip_prefix(start) {
                comment += 1;
                is_code.push(false);
                if !rest.contains(end) {
                    in_block = true;
                }
                continue;
            }
        }
        if lang.line.iter().any(|p| line.starts_with(p)) {
            comment += 1;
            is_code.push(false);
            continue;
        }
        code += 1;
        is_code.push(true);
    }
    (code, comment, blank, is_code)
}

/// cloc-style counting per file and per language.
pub fn count_lines(files: &[SourceFile]) -> EngineResult<Value> {
    check_files(files)?;
    let mut per_file = Vec::new();
    let mut per_lang: HashMap<&str, [usize; 4]> = HashMap::new();
    let mut total = [0usize; 4];
    for f in files {
        let lang = detect_lang(f);
        let (code, comment, blank, _) = classify_lines(&f.content, &lang);
        let lines = f.content.lines().count();
        let e = per_lang.entry(lang.id).or_insert([0; 4]);
        for (i, v) in [lines, code, comment, blank].iter().enumerate() {
            e[i] += v;
            total[i] += v;
        }
        per_file.push(json!({
            "name": f.name,
            "language": lang.id,
            "lines": lines,
            "code": code,
            "comment": comment,
            "blank": blank,
            "bytes": f.content.len(),
            "longest_line": f.content.lines().map(|l| l.chars().count()).max().unwrap_or(0),
            "comment_ratio": if code + comment > 0 { ((comment as f64 / (code + comment) as f64) * 1000.0).round() / 10.0 } else { 0.0 }
        }));
    }
    let mut langs: Vec<Value> = per_lang
        .iter()
        .map(|(k, v)| {
            json!({
                "language": k,
                "files": files.iter().filter(|f| detect_lang(f).id == *k).count(),
                "lines": v[0],
                "code": v[1],
                "comment": v[2],
                "blank": v[3]
            })
        })
        .collect();
    langs.sort_by(|a, b| b["code"].as_u64().cmp(&a["code"].as_u64()));
    Ok(json!({
        "files": per_file,
        "languages": langs,
        "total": {
            "files": files.len(),
            "lines": total[0],
            "code": total[1],
            "comment": total[2],
            "blank": total[3],
            "comment_ratio": if total[1] + total[2] > 0 { ((total[2] as f64 / (total[1] + total[2]) as f64) * 1000.0).round() / 10.0 } else { 0.0 }
        }
    }))
}

/// Cyclomatic-complexity heuristics per function and per file.
pub fn complexity(files: &[SourceFile]) -> EngineResult<Value> {
    check_files(files)?;
    let decision = crate::re(
        r"\b(if|else\s+if|elif|for|foreach|while|case|when|catch|except|rescue|guard|unless|until)\b|&&|\|\||\?\?|\?\s*[^.:?]",
    )?;
    let mut out = Vec::new();
    let mut worst: Vec<Value> = Vec::new();
    for f in files {
        let lang = detect_lang(f);
        let func_re = crate::re(lang.functions)?;
        let lines: Vec<&str> = f.content.lines().collect();
        // Function start lines.
        let mut starts: Vec<(usize, String)> = Vec::new();
        for (i, l) in lines.iter().enumerate() {
            if let Ok(Some(c)) = func_re.captures(l) {
                let name = (1..c.len())
                    .filter_map(|k| c.get(k))
                    .map(|m| m.as_str().to_string())
                    .next()
                    .unwrap_or_else(|| "anonymous".into());
                if !matches!(
                    name.as_str(),
                    "if" | "for" | "while" | "switch" | "catch" | "return" | "function" | "else"
                ) {
                    starts.push((i, name));
                }
            }
        }
        let mut functions = Vec::new();
        let mut file_total = 1usize;
        let mut max_depth = 0usize;
        let mut depth: i64 = 0;
        for l in &lines {
            for ch in l.chars() {
                if ch == '{' {
                    depth += 1;
                    max_depth = max_depth.max(depth as usize);
                } else if ch == '}' {
                    depth -= 1;
                }
            }
            file_total += decision.find_iter(l).count();
        }
        if lang.id == "python" {
            max_depth = lines
                .iter()
                .map(|l| (l.len() - l.trim_start().len()) / 4)
                .max()
                .unwrap_or(0);
        }
        for (k, (start, name)) in starts.iter().enumerate() {
            let end = starts.get(k + 1).map(|s| s.0).unwrap_or(lines.len());
            let body = &lines[*start..end];
            let cc = 1 + body.iter().map(|l| decision.find_iter(l).count()).sum::<usize>();
            let len = body.iter().filter(|l| !l.trim().is_empty()).count();
            let params = lines[*start]
                .split('(')
                .nth(1)
                .and_then(|p| p.split(')').next())
                .map(|p| if p.trim().is_empty() { 0 } else { p.split(',').count() })
                .unwrap_or(0);
            let rating = match cc {
                1..=5 => "simple",
                6..=10 => "moderate",
                11..=20 => "complex",
                _ => "very complex",
            };
            let fj = json!({
                "name": name,
                "line": start + 1,
                "end_line": end,
                "length": len,
                "cyclomatic": cc,
                "parameters": params,
                "rating": rating
            });
            if cc > 10 || len > 80 || params > 5 {
                worst.push(json!({
                    "file": f.name,
                    "function": name,
                    "line": start + 1,
                    "cyclomatic": cc,
                    "length": len,
                    "parameters": params
                }));
            }
            functions.push(fj);
        }
        let (code, comment, _, _) = classify_lines(&f.content, &lang);
        // Maintainability index (SEI variant, without Halstead volume → use lines as proxy).
        let volume = ((code.max(1) as f64)
            * (f.content.split_whitespace().collect::<HashSet<_>>().len().max(2) as f64).log2())
        .max(1.0);
        let mi = (171.0
            - 5.2 * volume.ln()
            - 0.23 * file_total as f64
            - 16.2 * (code.max(1) as f64).ln()
            + 50.0 * ((2.4 * comment as f64 / code.max(1) as f64).min(1.0)).sqrt().sin())
        .clamp(0.0, 171.0)
            * 100.0
            / 171.0;
        out.push(json!({
            "name": f.name,
            "language": lang.id,
            "functions": functions,
            "function_count": functions.len(),
            "file_cyclomatic": file_total,
            "max_nesting_depth": max_depth,
            "code_lines": code,
            "average_function_length": if functions.is_empty() { 0.0 } else { (functions.iter().filter_map(|x| x["length"].as_f64()).sum::<f64>() / functions.len() as f64 * 10.0).round() / 10.0 },
            "maintainability_index": (mi * 10.0).round() / 10.0,
            "maintainability": if mi >= 80.0 { "high" } else if mi >= 50.0 { "moderate" } else { "low" }
        }));
    }
    worst.sort_by(|a, b| b["cyclomatic"].as_u64().cmp(&a["cyclomatic"].as_u64()));
    Ok(json!({
        "files": out,
        "hotspots": worst.into_iter().take(25).collect::<Vec<_>>(),
        "method": "Cyclomatic complexity = 1 + decision points \
                   (if/else-if/loops/case/catch/&&/||/?:); nesting from brace depth; \
                   maintainability index approximated without Halstead metrics"
    }))
}

fn normalize_line(l: &str) -> String {
    let t = l.trim();
    let mut out = String::new();
    let mut in_str: Option<char> = None;
    for c in t.chars() {
        match in_str {
            Some(q) => {
                if c == q {
                    in_str = None;
                    out.push_str("\"S\"");
                }
            },
            None => {
                if c == '"' || c == '\'' || c == '`' {
                    in_str = Some(c);
                } else if c.is_whitespace() {
                    if !out.ends_with(' ') {
                        out.push(' ');
                    }
                } else {
                    out.push(c);
                }
            },
        }
    }
    out
}

/// Finds duplicated blocks of at least `min_lines` normalised code lines.
pub fn duplicates(
    files: &[SourceFile],
    min_lines: usize,
    ignore_strings: bool,
) -> EngineResult<Value> {
    check_files(files)?;
    let min_lines = min_lines.clamp(3, 200);
    let mut windows: HashMap<u64, Vec<(usize, usize)>> = HashMap::new(); // hash → (file, line)
    let mut normalized: Vec<Vec<(usize, String)>> = Vec::new();
    for f in files {
        let lang = detect_lang(f);
        let (_, _, _, is_code) = classify_lines(&f.content, &lang);
        let lines: Vec<(usize, String)> = f
            .content
            .lines()
            .enumerate()
            .filter(|(i, l)| {
                is_code.get(*i).copied().unwrap_or(true)
                    && l.trim().len() > 2
                    && !matches!(l.trim(), "}" | "{" | "};" | ")" | "end" | "});")
            })
            .map(|(i, l)| {
                (
                    i + 1,
                    if ignore_strings {
                        normalize_line(l)
                    } else {
                        l.trim().to_string()
                    },
                )
            })
            .collect();
        normalized.push(lines);
    }
    for (fi, lines) in normalized.iter().enumerate() {
        if lines.len() < min_lines {
            continue;
        }
        for w in 0..=lines.len() - min_lines {
            let mut h: u64 = 0xcbf29ce484222325;
            for (_, l) in &lines[w..w + min_lines] {
                for b in l.bytes() {
                    h ^= b as u64;
                    h = h.wrapping_mul(0x100000001b3);
                }
                h ^= 0x1f;
                h = h.wrapping_mul(0x100000001b3);
            }
            windows.entry(h).or_default().push((fi, w));
        }
    }
    // Merge overlapping windows into blocks.
    let mut groups: Vec<Vec<(usize, usize)>> =
        windows.into_values().filter(|v| v.len() > 1).collect();
    groups.sort_by_key(|g| (g[0].0, g[0].1));
    let mut blocks: Vec<Value> = Vec::new();
    let mut covered: HashSet<(usize, usize)> = HashSet::new();
    let mut duplicated_lines = 0usize;
    for g in groups {
        if g.iter().any(|p| covered.contains(p)) {
            continue;
        }
        // Extend the block while all occurrences keep matching.
        let mut len = min_lines;
        loop {
            let ok = g.iter().all(|(fi, w)| {
                normalized[*fi].get(w + len).map(|x| &x.1)
                    == normalized[g[0].0].get(g[0].1 + len).map(|x| &x.1)
            }) && normalized[g[0].0].get(g[0].1 + len).is_some();
            if !ok {
                break;
            }
            len += 1;
        }
        for (fi, w) in &g {
            for k in 0..len {
                covered.insert((*fi, w + k));
            }
        }
        duplicated_lines += len * (g.len() - 1);
        let snippet: Vec<String> = normalized[g[0].0][g[0].1..g[0].1 + len.min(12)]
            .iter()
            .map(|x| x.1.clone())
            .collect();
        blocks.push(json!({
            "lines": len,
            "occurrences": g.iter().map(|(fi, w)| json!({ "file": files[*fi].name, "start_line": normalized[*fi][*w].0, "end_line": normalized[*fi][w + len - 1].0 })).collect::<Vec<_>>(),
            "snippet": snippet
        }));
    }
    let weight = |b: &Value| {
        b["lines"].as_u64().unwrap_or(0)
            * b["occurrences"].as_array().map(|o| o.len()).unwrap_or(0) as u64
    };
    blocks.sort_by_key(|b| std::cmp::Reverse(weight(b)));
    let total_code: usize = normalized.iter().map(Vec::len).sum();
    Ok(json!({
        "duplicate_blocks": blocks.len(),
        "duplicated_lines": duplicated_lines,
        "code_lines": total_code,
        "duplication_percent": if total_code > 0 { ((duplicated_lines as f64 / total_code as f64) * 1000.0).round() / 10.0 } else { 0.0 },
        "min_lines": min_lines,
        "blocks": blocks.into_iter().take(200).collect::<Vec<_>>()
    }))
}

// --------------------------------------------------------------- imports --

fn import_group(module: &str, lang: &str) -> u8 {
    let m = module.trim_matches(|c| c == '"' || c == '\'');
    match lang {
        "python" => {
            const STD: &[&str] = &[
                "os",
                "sys",
                "re",
                "json",
                "math",
                "time",
                "datetime",
                "collections",
                "itertools",
                "functools",
                "typing",
                "pathlib",
                "subprocess",
                "logging",
                "random",
                "string",
                "io",
                "csv",
                "abc",
                "enum",
                "dataclasses",
                "asyncio",
                "unittest",
                "argparse",
                "shutil",
                "tempfile",
                "copy",
                "hashlib",
                "base64",
                "urllib",
                "http",
                "socket",
                "threading",
                "multiprocessing",
                "decimal",
                "fractions",
                "statistics",
                "uuid",
                "inspect",
                "textwrap",
                "struct",
                "glob",
                "fnmatch",
                "pickle",
                "sqlite3",
                "xml",
                "html",
                "email",
                "zipfile",
                "tarfile",
                "gzip",
                "contextlib",
                "operator",
                "heapq",
                "bisect",
                "queue",
                "signal",
                "platform",
                "traceback",
                "warnings",
                "weakref",
                "types",
                "secrets",
                "hmac",
                "ipaddress",
                "pprint",
            ];
            if m.starts_with('.') {
                2
            } else if STD.contains(&m.split('.').next().unwrap_or("")) {
                0
            } else {
                1
            }
        },
        "go" => {
            if m.contains('.') && !m.starts_with("golang.org/x") {
                1
            } else {
                0
            }
        },
        "rust" => {
            if m.starts_with("std") || m.starts_with("core") || m.starts_with("alloc") {
                0
            } else if m.starts_with("crate") || m.starts_with("super") || m.starts_with("self") {
                2
            } else {
                1
            }
        },
        _ => {
            if m.starts_with('.')
                || m.starts_with('/')
                || m.starts_with("@/")
                || m.starts_with('~')
                || m.starts_with("src/")
            {
                2
            } else if m.starts_with("node:")
                || matches!(
                    m.split('/').next().unwrap_or(""),
                    "fs" | "path"
                        | "os"
                        | "http"
                        | "https"
                        | "crypto"
                        | "url"
                        | "util"
                        | "events"
                        | "stream"
                        | "child_process"
                        | "buffer"
                        | "assert"
                        | "net"
                        | "zlib"
                        | "readline"
                        | "process"
                )
            {
                0
            } else {
                1
            }
        },
    }
}

/// Sorts and groups import statements (stdlib / third-party / local).
pub fn sort_imports(file: &SourceFile) -> EngineResult<Value> {
    engine_shared::error::require_text(&file.content, "content")?;
    let lang = detect_lang(file);
    let lines: Vec<&str> = file.content.lines().collect();
    let (is_import, module_of): (Box<dyn Fn(&str) -> bool>, Box<dyn Fn(&str) -> String>) =
        match lang.id {
            "python" => (
                Box::new(|l: &str| {
                    l.trim_start().starts_with("import ") || l.trim_start().starts_with("from ")
                }),
                Box::new(|l: &str| {
                    let t = l.trim();
                    if let Some(r) = t.strip_prefix("from ") {
                        r.split_whitespace().next().unwrap_or("").to_string()
                    } else {
                        t.trim_start_matches("import ")
                            .split([',', ' '])
                            .next()
                            .unwrap_or("")
                            .to_string()
                    }
                }),
            ),
            "rust" => (
                Box::new(|l: &str| {
                    l.trim_start().starts_with("use ") || l.trim_start().starts_with("pub use ")
                }),
                Box::new(|l: &str| {
                    l.trim()
                        .trim_start_matches("pub ")
                        .trim_start_matches("use ")
                        .trim_end_matches(';')
                        .to_string()
                }),
            ),
            "go" => (
                Box::new(|l: &str| {
                    let t = l.trim();
                    t.starts_with("import ")
                        || (t.starts_with('"') && t.ends_with('"'))
                        || (t.contains(" \"") && t.ends_with('"') && !t.contains('('))
                }),
                Box::new(|l: &str| {
                    l.trim()
                        .trim_start_matches("import ")
                        .rsplit(' ')
                        .next()
                        .unwrap_or("")
                        .trim_matches('"')
                        .to_string()
                }),
            ),
            _ => (
                Box::new(|l: &str| {
                    let t = l.trim_start();
                    (t.starts_with("import ")
                        && (t.contains(" from ") || t.contains('\'') || t.contains('"')))
                        || t.starts_with("import(")
                        || (t.starts_with("const ") && t.contains("require("))
                }),
                Box::new(|l: &str| {
                    let t = l.trim();
                    if let Some(p) = t.find(" from ") {
                        t[p + 6..]
                            .trim()
                            .trim_end_matches(';')
                            .trim_matches(|c| c == '"' || c == '\'')
                            .to_string()
                    } else if let Some(p) = t.find("require(") {
                        t[p + 8..]
                            .split(')')
                            .next()
                            .unwrap_or("")
                            .trim_matches(|c| c == '"' || c == '\'')
                            .to_string()
                    } else {
                        t.trim_start_matches("import ")
                            .trim_end_matches(';')
                            .trim_matches(|c| c == '"' || c == '\'')
                            .to_string()
                    }
                }),
            ),
        };
    // Contiguous import block(s) at the top (skip comments/blank/shebang/"use strict"/go import parens).
    let mut imports: Vec<(usize, String)> = Vec::new();
    let mut first = usize::MAX;
    let mut last = 0;
    let mut in_go_block = false;
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim();
        if lang.id == "go" {
            if t.starts_with("import (") {
                in_go_block = true;
                first = first.min(i);
                last = i;
                continue;
            }
            if in_go_block {
                last = i;
                if t == ")" {
                    in_go_block = false;
                    continue;
                }
                if !t.is_empty() {
                    imports.push((i, t.to_string()));
                }
                continue;
            }
        }
        if is_import(l) {
            // Multi-line JS/TS imports: join until the line containing `from`.
            let mut stmt = l.to_string();
            let mut j = i;
            if lang.id != "python"
                && lang.id != "rust"
                && !stmt.contains(" from ")
                && stmt.trim_start().starts_with("import ")
                && stmt.contains('{')
                && !stmt.contains('}')
            {
                while j + 1 < lines.len() && !lines[j].contains(" from ") {
                    j += 1;
                    stmt.push(' ');
                    stmt.push_str(lines[j].trim());
                }
            }
            first = first.min(i);
            last = last.max(j);
            imports.push((i, stmt.split_whitespace().collect::<Vec<_>>().join(" ")));
            continue;
        }
        if first != usize::MAX
            && !t.is_empty()
            && !t.starts_with("//")
            && !t.starts_with('#')
            && !t.starts_with("/*")
            && !t.starts_with('*')
        {
            // First non-import statement ends the header block… unless we have not yet seen imports.
            if imports.is_empty() {
                continue;
            }
            break;
        }
    }
    if imports.is_empty() {
        return Err(EngineError::not_found("No import statements found at the top of the file"));
    }
    let mut groups: [Vec<String>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut seen = HashSet::new();
    let mut removed_duplicates = 0;
    for (_, stmt) in &imports {
        if !seen.insert(stmt.clone()) {
            removed_duplicates += 1;
            continue;
        }
        let g = import_group(&module_of(stmt), lang.id) as usize;
        groups[g].push(stmt.clone());
    }
    for g in groups.iter_mut() {
        g.sort_by_key(|s| module_of(s).to_lowercase());
    }
    let mut block: Vec<String> = Vec::new();
    if lang.id == "go" {
        block.push("import (".into());
        for (gi, g) in groups.iter().enumerate() {
            if g.is_empty() {
                continue;
            }
            if block.len() > 1 {
                block.push(String::new());
            }
            let _ = gi;
            for s in g {
                block.push(format!("\t{s}"));
            }
        }
        block.push(")".into());
    } else {
        for g in groups.iter() {
            if g.is_empty() {
                continue;
            }
            if !block.is_empty() {
                block.push(String::new());
            }
            block.extend(g.iter().cloned());
        }
    }
    let mut out: Vec<String> = lines[..first].iter().map(|s| s.to_string()).collect();
    out.extend(block.iter().cloned());
    out.extend(lines[last + 1..].iter().map(|s| s.to_string()));
    let result = out.join("\n") + if file.content.ends_with('\n') { "\n" } else { "" };
    Ok(json!({
        "sorted": result,
        "language": lang.id,
        "imports": imports.len(),
        "groups": {
            "standard_library": groups[0].len(),
            "third_party": groups[1].len(),
            "local": groups[2].len()
        },
        "duplicates_removed": removed_duplicates,
        "changed": result != file.content
    }))
}

/// Heuristic unused-import detection (JS/TS, Python, Rust, Go).
pub fn unused_imports(file: &SourceFile) -> EngineResult<Value> {
    engine_shared::error::require_text(&file.content, "content")?;
    let lang = detect_lang(file);
    let lines: Vec<&str> = file.content.lines().collect();
    let mut imported: Vec<(String, usize, String)> = Vec::new(); // (identifier, line, statement)
    let ident_re = crate::re(r"[A-Za-z_$][\w$]*")?;
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim();
        match lang.id {
            "python" => {
                if let Some(r) = t.strip_prefix("from ") {
                    if let Some(p) = r.find(" import ") {
                        for name in r[p + 8..].trim_matches(|c| c == '(' || c == ')').split(',') {
                            let n = name.trim();
                            if n.is_empty() || n == "*" {
                                continue;
                            }
                            let ident = n.split(" as ").last().unwrap_or(n).trim();
                            imported.push((ident.to_string(), i + 1, t.to_string()));
                        }
                    }
                } else if let Some(r) = t.strip_prefix("import ") {
                    for name in r.split(',') {
                        let n = name.trim();
                        let ident = n
                            .split(" as ")
                            .last()
                            .unwrap_or(n)
                            .split('.')
                            .next()
                            .unwrap_or(n)
                            .trim();
                        if !ident.is_empty() {
                            imported.push((ident.to_string(), i + 1, t.to_string()));
                        }
                    }
                }
            },
            "rust" => {
                if let Some(r) = t.strip_prefix("use ").or_else(|| t.strip_prefix("pub use ")) {
                    if t.starts_with("pub use") {
                        continue;
                    }
                    let r = r.trim_end_matches(';');
                    let names: Vec<String> = if let Some(start) = r.find('{') {
                        r[start + 1..r.rfind('}').unwrap_or(r.len())]
                            .split(',')
                            .map(|s| s.trim().split(" as ").last().unwrap_or("").trim().to_string())
                            .collect()
                    } else {
                        vec![r
                            .split(" as ")
                            .last()
                            .unwrap_or(r)
                            .rsplit("::")
                            .next()
                            .unwrap_or(r)
                            .to_string()]
                    };
                    for n in names {
                        if !n.is_empty() && n != "*" && n != "self" {
                            imported.push((n, i + 1, t.to_string()));
                        }
                    }
                }
            },
            "go" => {
                if (t.starts_with('"') && t.ends_with('"'))
                    || t.starts_with("import \"")
                    || (t.contains(" \"") && t.ends_with('"') && !t.starts_with("//"))
                {
                    let path = t
                        .trim_start_matches("import ")
                        .rsplit(' ')
                        .next()
                        .unwrap_or("")
                        .trim_matches('"');
                    let alias = t
                        .trim_start_matches("import ")
                        .split_whitespace()
                        .next()
                        .filter(|a| !a.starts_with('"'))
                        .map(str::to_string);
                    let ident = alias
                        .unwrap_or_else(|| path.rsplit('/').next().unwrap_or(path).to_string());
                    if ident != "_" && ident != "." && !path.is_empty() {
                        imported.push((ident, i + 1, t.to_string()));
                    }
                }
            },
            _ => {
                if let (true, Some(from_at)) = (t.starts_with("import "), t.find(" from ")) {
                    let spec = t[7..from_at].trim();
                    if spec.starts_with("type ") {
                        continue;
                    }
                    let mut names = Vec::new();
                    let (default_part, rest) = match spec.find('{') {
                        Some(p) => (
                            spec[..p].trim().trim_end_matches(','),
                            Some(&spec[p + 1..spec.rfind('}').unwrap_or(spec.len())]),
                        ),
                        None => (spec, None),
                    };
                    if !default_part.is_empty() {
                        if let Some(ns) = default_part.strip_prefix("* as ") {
                            names.push(ns.trim().to_string());
                        } else {
                            names.push(default_part.to_string());
                        }
                    }
                    if let Some(r) = rest {
                        for n in r.split(',') {
                            let n = n.trim().trim_start_matches("type ");
                            let ident = n.split(" as ").last().unwrap_or(n).trim();
                            if !ident.is_empty() {
                                names.push(ident.to_string());
                            }
                        }
                    }
                    for n in names {
                        imported.push((n, i + 1, t.to_string()));
                    }
                } else if t.starts_with("const ") && t.contains("require(") {
                    let spec = t[6..t.find('=').unwrap_or(6)].trim();
                    let names: Vec<String> = if spec.starts_with('{') {
                        spec.trim_matches(|c| c == '{' || c == '}')
                            .split(',')
                            .map(|s| {
                                s.trim().split(':').next_back().unwrap_or("").trim().to_string()
                            })
                            .collect()
                    } else {
                        vec![spec.to_string()]
                    };
                    for n in names {
                        if !n.is_empty() {
                            imported.push((n, i + 1, t.to_string()));
                        }
                    }
                }
            },
        }
    }
    if imported.is_empty() {
        return Err(EngineError::not_found("No import statements recognised"));
    }
    // Count identifier occurrences outside import lines.
    let import_lines: HashSet<usize> = imported.iter().map(|x| x.1).collect();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for (i, l) in lines.iter().enumerate() {
        if import_lines.contains(&(i + 1)) {
            continue;
        }
        let code = match l.find("//") {
            Some(p) if lang.id != "python" => &l[..p],
            _ => l,
        };
        for m in ident_re.find_iter(code).flatten() {
            *counts.entry(m.as_str().to_string()).or_insert(0) += 1;
        }
    }
    let unused: Vec<Value> = imported
        .iter()
        .filter(|(n, _, _)| counts.get(n).copied().unwrap_or(0) == 0)
        .map(|(n, line, stmt)| json!({ "name": n, "line": line, "statement": stmt }))
        .collect();
    let used: Vec<Value> = imported
        .iter()
        .filter(|(n, _, _)| counts.get(n).copied().unwrap_or(0) > 0)
        .map(|(n, line, _)| json!({ "name": n, "line": line, "references": counts[n] }))
        .collect();
    Ok(json!({
        "language": lang.id,
        "imported": imported.len(),
        "unused": unused,
        "used": used,
        "clean": imported.iter().all(|(n, _, _)| counts.get(n).copied().unwrap_or(0) > 0),
        "note": "Heuristic identifier matching; side-effect imports, re-exports and dynamic \
                 access (e.g. `module[name]`) are not understood"
    }))
}

// ---------------------------------------------------------- dependencies --

const LICENSES: &[(&str, &str, &str)] = &[
    ("MIT", "permissive", "Attribution required"),
    ("Apache-2.0", "permissive", "Attribution, NOTICE file, patent grant"),
    ("BSD-2-Clause", "permissive", "Attribution"),
    ("BSD-3-Clause", "permissive", "Attribution, no endorsement"),
    ("ISC", "permissive", "Attribution"),
    ("0BSD", "public-domain-like", "No conditions"),
    ("Unlicense", "public-domain-like", "No conditions"),
    ("CC0-1.0", "public-domain-like", "No conditions"),
    ("Zlib", "permissive", "Attribution in source"),
    ("MPL-2.0", "weak-copyleft", "File-level copyleft"),
    ("LGPL-2.1", "weak-copyleft", "Library copyleft; dynamic linking OK"),
    ("LGPL-3.0", "weak-copyleft", "Library copyleft"),
    ("EPL-2.0", "weak-copyleft", "Module-level copyleft"),
    ("GPL-2.0", "copyleft", "Derivative works must be GPL"),
    ("GPL-3.0", "copyleft", "Derivative works must be GPL"),
    ("AGPL-3.0", "network-copyleft", "Network use counts as distribution"),
    ("SSPL-1.0", "source-available", "Not OSI approved"),
    ("BUSL-1.1", "source-available", "Commercial restrictions until change date"),
    ("CC-BY-4.0", "permissive", "Attribution (content licence)"),
    ("CC-BY-SA-4.0", "copyleft", "Share-alike (content licence)"),
    ("Artistic-2.0", "permissive", "Perl licence"),
    ("BSL-1.0", "permissive", "Boost"),
    ("WTFPL", "public-domain-like", "No conditions"),
    ("UNLICENSED", "proprietary", "All rights reserved"),
];

fn classify_license(expr: &str) -> Value {
    let e = expr.trim();
    if e.is_empty() {
        return json!({ "expression": Value::Null, "category": "unknown", "risk": "unknown" });
    }
    let ids: Vec<&str> = e
        .split([' ', '(', ')'])
        .filter(|s| {
            !s.is_empty() && !matches!(s.to_ascii_uppercase().as_str(), "OR" | "AND" | "WITH")
        })
        .collect();
    let cats: Vec<&str> = ids
        .iter()
        .map(|id| {
            LICENSES
                .iter()
                .find(|(l, _, _)| {
                    l.eq_ignore_ascii_case(
                        id.trim_end_matches("-only")
                            .trim_end_matches("-or-later")
                            .trim_end_matches('+'),
                    )
                })
                .map(|(_, c, _)| *c)
                .unwrap_or("unknown")
        })
        .collect();
    let risk = if cats
        .iter()
        .any(|c| matches!(*c, "network-copyleft" | "proprietary" | "source-available"))
    {
        "high"
    } else if cats.contains(&"copyleft") {
        if e.to_uppercase().contains(" OR ") {
            "medium"
        } else {
            "high"
        }
    } else if cats.contains(&"weak-copyleft") {
        "medium"
    } else if cats.iter().all(|c| *c != "unknown") {
        "low"
    } else {
        "unknown"
    };
    json!({
        "expression": e,
        "category": if cats.len() == 1 { cats[0].to_string() } else { cats.join(" / ") },
        "risk": risk,
        "notes": ids.iter().filter_map(|id| LICENSES.iter().find(|(l, _, _)| l.eq_ignore_ascii_case(id)).map(|(_, _, n)| *n)).collect::<Vec<_>>()
    })
}

/// Parses dependency manifests and classifies licences (from the manifest,
/// lockfile `license` fields, or a caller-provided `licenses` map).
pub fn dependencies(files: &[SourceFile], licenses: &Map<String, Value>) -> EngineResult<Value> {
    check_files(files)?;
    let mut deps: Vec<Value> = Vec::new();
    let mut manifests = Vec::new();
    let mut known: HashMap<String, String> = licenses
        .iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
        .collect();
    // Harvest licence fields from lockfiles first.
    for f in files {
        let name = f.name.rsplit('/').next().unwrap_or(&f.name);
        if name == "package-lock.json" {
            if let Ok(v) = serde_json::from_str::<Value>(&f.content) {
                if let Some(pk) = v.get("packages").and_then(Value::as_object) {
                    for (path, info) in pk {
                        if let (Some(n), Some(l)) = (
                            path.rsplit("node_modules/").next(),
                            info.get("license").and_then(Value::as_str),
                        ) {
                            known.entry(n.to_string()).or_insert(l.to_string());
                        }
                    }
                }
            }
        }
    }
    for f in files {
        let name = f.name.rsplit('/').next().unwrap_or(&f.name).to_string();
        let mut add = |n: &str, ver: &str, kind: &str, eco: &str| {
            let lic = known.get(n).cloned().unwrap_or_default();
            deps.push(json!({
                "name": n,
                "version": ver,
                "kind": kind,
                "ecosystem": eco,
                "manifest": name,
                "license": classify_license(&lic)
            }));
        };
        match name.as_str() {
            "package.json" => {
                let v: Value = serde_json::from_str(&f.content)
                    .map_err(|e| EngineError::malformed(format!("package.json: {e}")))?;
                for (key, kind) in [
                    ("dependencies", "runtime"),
                    ("devDependencies", "dev"),
                    ("peerDependencies", "peer"),
                    ("optionalDependencies", "optional"),
                ] {
                    if let Some(m) = v.get(key).and_then(Value::as_object) {
                        for (n, ver) in m {
                            add(n, ver.as_str().unwrap_or(""), kind, "npm");
                        }
                    }
                }
                manifests.push(json!({
                    "file": name,
                    "ecosystem": "npm",
                    "project": v.get("name"),
                    "license": v.get("license").and_then(Value::as_str).map(classify_license)
                }));
            },
            "Cargo.toml" => {
                let mut section = String::new();
                let mut project_license = None;
                for l in f.content.lines() {
                    let t = l.trim();
                    if t.starts_with('[') {
                        section = t.trim_matches(['[', ']']).to_string();
                        continue;
                    }
                    if section == "package" && t.starts_with("license") {
                        project_license =
                            t.split('=').nth(1).map(|s| s.trim().trim_matches('"').to_string());
                    }
                    if section.ends_with("dependencies") || section.contains("dependencies.") {
                        if let Some((k, v)) = t.split_once('=') {
                            let n = k.trim();
                            if n.is_empty() || n.starts_with('#') {
                                continue;
                            }
                            let ver = if v.contains("version") {
                                v.split("version")
                                    .nth(1)
                                    .unwrap_or("")
                                    .split('"')
                                    .nth(1)
                                    .unwrap_or("")
                                    .to_string()
                            } else {
                                v.trim().trim_matches('"').to_string()
                            };
                            let kind = if section.starts_with("dev") {
                                "dev"
                            } else if section.starts_with("build") {
                                "build"
                            } else {
                                "runtime"
                            };
                            add(n, &ver, kind, "crates.io");
                        }
                    }
                }
                manifests.push(json!({
                    "file": name,
                    "ecosystem": "crates.io",
                    "license": project_license.map(|l| classify_license(&l))
                }));
            },
            "requirements.txt" | "requirements-dev.txt" | "constraints.txt" => {
                for l in f.content.lines() {
                    let t = l.trim();
                    if t.is_empty() || t.starts_with('#') || t.starts_with('-') {
                        continue;
                    }
                    let n = t
                        .split(['=', '>', '<', '~', '!', ';', '[', ' '])
                        .next()
                        .unwrap_or(t)
                        .trim();
                    add(
                        n,
                        t[n.len()..].trim(),
                        if name.contains("dev") { "dev" } else { "runtime" },
                        "pypi",
                    );
                }
                manifests.push(json!({ "file": name, "ecosystem": "pypi" }));
            },
            "pyproject.toml" => {
                let mut in_deps = false;
                for l in f.content.lines() {
                    let t = l.trim();
                    if t.starts_with("dependencies") && t.contains('[') {
                        in_deps = true;
                        continue;
                    }
                    if in_deps {
                        if t.starts_with(']') {
                            in_deps = false;
                            continue;
                        }
                        let spec = t.trim_matches(|c| c == '"' || c == ',' || c == '\'');
                        let n: String = spec
                            .chars()
                            .take_while(|c| {
                                c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.'
                            })
                            .collect();
                        if !n.is_empty() {
                            add(&n, spec[n.len()..].trim(), "runtime", "pypi");
                        }
                    }
                }
                manifests.push(json!({ "file": name, "ecosystem": "pypi" }));
            },
            "go.mod" => {
                let mut in_req = false;
                for l in f.content.lines() {
                    let t = l.trim();
                    if t.starts_with("require (") {
                        in_req = true;
                        continue;
                    }
                    if in_req && t == ")" {
                        in_req = false;
                        continue;
                    }
                    if in_req || t.starts_with("require ") {
                        let parts: Vec<&str> =
                            t.trim_start_matches("require ").split_whitespace().collect();
                        if parts.len() >= 2 {
                            add(
                                parts[0],
                                parts[1],
                                if t.contains("// indirect") {
                                    "indirect"
                                } else {
                                    "runtime"
                                },
                                "go",
                            );
                        }
                    }
                }
                manifests.push(json!({ "file": name, "ecosystem": "go" }));
            },
            "composer.json" => {
                let v: Value = serde_json::from_str(&f.content)
                    .map_err(|e| EngineError::malformed(format!("composer.json: {e}")))?;
                for (key, kind) in [("require", "runtime"), ("require-dev", "dev")] {
                    if let Some(m) = v.get(key).and_then(Value::as_object) {
                        for (n, ver) in m {
                            add(n, ver.as_str().unwrap_or(""), kind, "packagist");
                        }
                    }
                }
                manifests.push(json!({
                    "file": name,
                    "ecosystem": "packagist",
                    "license": v.get("license").map(|l| classify_license(l.to_string().trim_matches('"')))
                }));
            },
            "Gemfile" => {
                for l in f.content.lines() {
                    let t = l.trim();
                    if let Some(r) = t.strip_prefix("gem ") {
                        let parts: Vec<&str> = r
                            .split(',')
                            .map(|s| s.trim().trim_matches(|c| c == '"' || c == '\''))
                            .collect();
                        add(parts[0], parts.get(1).copied().unwrap_or(""), "runtime", "rubygems");
                    }
                }
                manifests.push(json!({ "file": name, "ecosystem": "rubygems" }));
            },
            "package-lock.json" | "pnpm-lock.yaml" | "yarn.lock" | "Cargo.lock" | "poetry.lock"
            | "go.sum" => manifests.push(json!({
                "file": name,
                "ecosystem": "lockfile",
                "note": "Used for licence fields only"
            })),
            _ => manifests.push(json!({
                "file": name,
                "ecosystem": "unknown",
                "note": "Not a recognised manifest"
            })),
        }
    }
    if deps.is_empty() {
        return Err(EngineError::not_found(
            "No dependencies found (supported: package.json, Cargo.toml, requirements.txt, \
             pyproject.toml, go.mod, composer.json, Gemfile)",
        ));
    }
    let mut by_risk: Map<String, Value> = Map::new();
    for d in &deps {
        let r = d["license"]["risk"].as_str().unwrap_or("unknown").to_string();
        by_risk.insert(r.clone(), json!(by_risk.get(&r).and_then(Value::as_u64).unwrap_or(0) + 1));
    }
    let unknown = deps.iter().filter(|d| d["license"]["category"] == "unknown").count();
    Ok(json!({
        "dependencies": deps,
        "count": deps.len(),
        "manifests": manifests,
        "by_risk": by_risk,
        "unknown_licenses": unknown,
        "note": if unknown > 0 { "Licence data comes from the manifests/lockfiles you upload \
                                  (package-lock.json carries `license` fields) or the `licenses` \
                                  map; the WASM engine has no network access to registries" } else { "" },
        "license_catalog": LICENSES.iter().map(|(id, cat, note)| json!({ "id": id, "category": cat, "note": note })).collect::<Vec<_>>()
    }))
}

// -------------------------------------------------------------- SQL query --

/// Static SQL analysis: statement kinds, tables, joins, missing WHERE on writes, `SELECT *`, risky
/// patterns.
pub fn sql_analyze(sql: &str, dialect: &str) -> EngineResult<Value> {
    let mut base = crate::sql::validate(sql, dialect)?;
    if base["valid"] != true {
        return Ok(base);
    }
    let upper = crate::sql::strip_comments(sql).to_ascii_uppercase();
    let count = |needle: &str| upper.matches(needle).count();
    let mut findings: Vec<Value> = Vec::new();
    let mut push = |sev: &str, code: &str, msg: &str| {
        findings.push(json!({ "severity": sev, "code": code, "message": msg }))
    };
    if upper.contains("SELECT *") {
        push(
            "warning",
            "select-star",
            "SELECT * fetches every column; list the needed columns to reduce I/O and avoid \
             breaking on schema changes",
        );
    }
    if upper.contains(" LIKE '%") || upper.contains(" LIKE \"%") {
        push(
            "warning",
            "leading-wildcard",
            "LIKE with a leading % cannot use a B-tree index (full scan)",
        );
    }
    for f in ["LOWER(", "UPPER(", "DATE(", "YEAR(", "CAST(", "COALESCE(", "SUBSTR", "TRIM("] {
        if let Some(p) = upper.find(" WHERE ") {
            if upper[p..].contains(f) {
                push(
                    "info",
                    "non-sargable",
                    &format!(
                        "Function {f}) applied to a column in WHERE prevents index use; consider \
                         computed columns or functional indexes"
                    ),
                );
                break;
            }
        }
    }
    if upper.contains("ORDER BY")
        && !upper.contains("LIMIT")
        && !upper.contains("FETCH FIRST")
        && !upper.contains(" TOP ")
    {
        push(
            "info",
            "order-without-limit",
            "ORDER BY without LIMIT sorts the whole result set",
        );
    }
    if upper.contains("SELECT DISTINCT") {
        push(
            "info",
            "distinct",
            "DISTINCT often hides a missing join condition or could be replaced by GROUP BY / \
             EXISTS",
        );
    }
    if upper.contains(" NOT IN (SELECT") {
        push(
            "warning",
            "not-in-subquery",
            "NOT IN (subquery) returns no rows if the subquery yields NULL; prefer NOT EXISTS",
        );
    }
    if (upper.contains("UPDATE ") || upper.contains("DELETE ")) && !upper.contains(" WHERE ") {
        push("error", "unfiltered-write", "UPDATE/DELETE without WHERE affects every row");
    }
    if upper.contains(" OR ") && upper.contains(" WHERE ") {
        push(
            "info",
            "or-conditions",
            "OR conditions on different columns may defeat index usage; consider UNION ALL",
        );
    }
    if count("JOIN") >= 5 {
        push(
            "info",
            "many-joins",
            "5+ joins – check join order, indexes on join keys and whether a materialised view \
             helps",
        );
    }
    if upper.contains("HAVING") && !upper.contains("GROUP BY") {
        push(
            "warning",
            "having-without-group",
            "HAVING without GROUP BY acts as a WHERE on aggregates",
        );
    }
    if upper.contains("OFFSET") {
        push(
            "info",
            "offset-pagination",
            "OFFSET pagination gets slower for deep pages; keyset pagination (WHERE id > ?) \
             scales better",
        );
    }
    let joins = count("JOIN");
    let subqueries = count("(SELECT");
    let ctes = count("WITH ");
    let aggregates = ["COUNT(", "SUM(", "AVG(", "MIN(", "MAX("]
        .iter()
        .map(|a| count(a))
        .sum::<usize>();
    let window = count(" OVER (") + count(" OVER(");
    let unions = count("UNION");
    let score = 1
        + joins * 2
        + subqueries * 3
        + ctes
        + aggregates
        + window * 2
        + unions * 2
        + count(" CASE ")
        + count(" GROUP BY ")
        + count(" HAVING ");
    base["analysis"] = json!({
        "joins": joins,
        "subqueries": subqueries,
        "ctes": count("WITH "),
        "aggregates": aggregates,
        "window_functions": window,
        "unions": unions,
        "case_expressions": count(" CASE "),
        "has_where": upper.contains(" WHERE "),
        "has_group_by": upper.contains(" GROUP BY "),
        "has_order_by": upper.contains(" ORDER BY "),
        "has_limit": upper.contains(" LIMIT ") || upper.contains("FETCH FIRST") || upper.contains(" TOP "),
        "complexity_score": score,
        "complexity": if score <= 3 { "simple" } else if score <= 8 { "moderate" } else if score <= 15 { "complex" } else { "very complex" },
        "findings": findings,
        "explain_hint": "Run EXPLAIN (ANALYZE) on the database to confirm index usage; these are \
                         static heuristics",
    });
    Ok(base)
}

/// Dispatchers for beautify/minify by language (reuse existing formatters).
pub fn format_code(language: &str, source: &str, indent: usize) -> EngineResult<Value> {
    engine_shared::error::require_text(source, "source")?;
    let lang = language.trim().to_ascii_lowercase();
    let l = if lang.is_empty() {
        detect_lang(&SourceFile {
            name: String::new(),
            content: source.to_string(),
            language: String::new(),
        })
        .id
        .to_string()
    } else {
        lang
    };
    let (out, key) = match l.as_str() {
        "javascript" | "js" | "typescript" | "ts" | "jsx" | "tsx" => {
            (crate::js::format(source, indent, true, false)?, "formatted")
        },
        "css" | "scss" | "less" => (crate::markup::format_css(source, indent)?, "formatted"),
        "html" | "htm" | "xml" | "svg" | "vue" => {
            (crate::markup::format_html(source, indent, 80)?, "formatted")
        },
        "sql" => (crate::sql::format(source, indent, true, 1)?, "formatted"),
        "json" => {
            let v: Value = serde_json::from_str(source)
                .map_err(|e| EngineError::malformed(format!("Invalid JSON: {e}")))?;
            (
                json!({ "formatted": serde_json::to_string_pretty(&v).unwrap_or_default() }),
                "formatted",
            )
        },
        "graphql" | "gql" => (crate::graphql::format(source, "")?, "formatted"),
        other => {
            return Err(EngineError::unsupported(format!(
                "No WASM formatter for `{other}` (javascript, typescript, css, html, sql, json, \
                 graphql); Python/Rust/Go formatters (black, rustfmt, gofmt) need their native \
                 toolchains"
            )))
        },
    };
    Ok(json!({ "language": l, "formatted": out[key], "details": out }))
}

/// Minifies source in a supported language (JS, CSS, HTML, JSON, SQL) by dispatching to the
/// dedicated minifiers.
pub fn minify_code(language: &str, source: &str) -> EngineResult<Value> {
    engine_shared::error::require_text(source, "source")?;
    let lang = language.trim().to_ascii_lowercase();
    let l = if lang.is_empty() {
        detect_lang(&SourceFile {
            name: String::new(),
            content: source.to_string(),
            language: String::new(),
        })
        .id
        .to_string()
    } else {
        lang
    };
    let out = match l.as_str() {
        "javascript" | "js" | "typescript" | "ts" => crate::js::minify(source, true)?,
        "css" | "scss" => crate::markup::minify_css(source)?,
        "html" | "htm" | "svg" | "xml" => crate::markup::minify_html(source, true, true)?,
        "sql" => crate::sql::minify(source)?,
        "json" => {
            let v: Value = serde_json::from_str(source)
                .map_err(|e| EngineError::malformed(format!("Invalid JSON: {e}")))?;
            let m = serde_json::to_string(&v).unwrap_or_default();
            json!({ "minified": m, "input_length": source.len(), "output_length": m.len() })
        },
        other => {
            return Err(EngineError::unsupported(format!(
                "No WASM minifier for `{other}` (javascript, typescript, css, html, sql, json)"
            )))
        },
    };
    Ok(json!({
        "language": l,
        "minified": out["minified"],
        "input_length": out["input_length"],
        "output_length": out["output_length"],
        "saved_percent": out["saved_percent"]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(name: &str, content: &str) -> SourceFile {
        SourceFile {
            name: name.into(),
            content: content.into(),
            language: String::new(),
        }
    }

    #[test]
    fn counting_and_complexity() {
        let js = "// header\n/* block\n comment */\nfunction a(x, y) {\n  if (x && y) { return 1; \
                  }\n  for (let i = 0; i < 3; i++) { if (i) {} }\n  return x ? 1 : 0;\n}\n\nconst \
                  b = (z) => z || 0;\n";
        let c = count_lines(&[f("a.js", js), f("b.py", "# c\nimport os\n\ndef go():\n    pass\n")])
            .unwrap();
        assert_eq!(c["total"]["files"], 2);
        assert_eq!(c["files"][0]["comment"], 3);
        assert_eq!(c["files"][1]["language"], "python");
        let cx = complexity(&[f("a.js", js)]).unwrap();
        let funcs = cx["files"][0]["functions"].as_array().unwrap();
        assert_eq!(funcs[0]["name"], "a");
        assert!(funcs[0]["cyclomatic"].as_u64().unwrap() >= 5, "{funcs:?}");
        assert_eq!(funcs[1]["name"], "b");
        assert!(count_lines(&[]).is_err());
    }

    #[test]
    fn duplicates_imports_deps() {
        let a = "function x() {\n  const a = 1;\n  const b = 2;\n  const c = a + b;\n  return c * \
                 2;\n}\n";
        let b = "function y() {\n  const a = 1;\n  const b = 2;\n  const c = a + b;\n  return c * \
                 2;\n}\n";
        let d = duplicates(&[f("a.js", a), f("b.js", b)], 3, true).unwrap();
        assert_eq!(d["duplicate_blocks"], 1);
        assert!(d["blocks"][0]["lines"].as_u64().unwrap() >= 4);
        let src = "import z from './z';\nimport fs from 'fs';\nimport { b, a } from \
                   'react';\nimport fs from 'fs';\n\nconsole.log(a, fs, z);\n";
        let s = sort_imports(&f("x.ts", src)).unwrap();
        let sorted = s["sorted"].as_str().unwrap();
        assert!(
            sorted.starts_with(
                "import fs from 'fs';\n\nimport { b, a } from 'react';\n\nimport z from './z';"
            ),
            "{sorted}"
        );
        assert_eq!(s["duplicates_removed"], 1);
        let u = unused_imports(&f("x.ts", src)).unwrap();
        assert_eq!(u["unused"][0]["name"], "b");
        let py = unused_imports(&f(
            "m.py",
            "import os, sys\nfrom typing import List, Dict\nprint(os.name, List)\n",
        ))
        .unwrap();
        let names: Vec<&str> = py["unused"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, vec!["sys", "Dict"]);
        assert!(sort_imports(&f("x.ts", "const a = 1;")).is_err());
        let deps = dependencies(&[f("package.json", r#"{"name":"app","license":"MIT","dependencies":{"react":"^18.0.0"},"devDependencies":{"vite":"^5"}}"#), f("Cargo.toml", "\
            [package]\nname=\"x\"\nlicense = \"Apache-2.0\"\n[dependencies]\nserde = { version = \
            \"1.0\", features = [\"derive\"] }\nanyhow = \"1\"\n[dev-dependencies]\ntempfile = \
            \"3\"\n"), f("requirements.txt", "\
            requests>=2.0\n# c\nnumpy==1.26.0\n")], &serde_json::from_str(r#"{"react":"MIT","vite":"MIT","serde":"MIT OR Apache-2.0","numpy":"BSD-3-Clause"}"#).unwrap()).unwrap();
        assert_eq!(deps["count"], 7);
        assert_eq!(deps["unknown_licenses"], 3);
        assert_eq!(deps["dependencies"][0]["license"]["risk"], "low");
        assert_eq!(classify_license("GPL-3.0-only")["risk"], "high");
        assert_eq!(classify_license("MIT OR GPL-2.0")["risk"], "medium");
        let q = sql_analyze(
            "SELECT * FROM users u JOIN orders o ON o.user_id = u.id WHERE LOWER(u.email) LIKE \
             '%x' ORDER BY u.id",
            "postgres",
        )
        .unwrap();
        let codes: Vec<&str> = q["analysis"]["findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["code"].as_str().unwrap())
            .collect();
        assert!(
            codes.contains(&"select-star")
                && codes.contains(&"leading-wildcard")
                && codes.contains(&"non-sargable")
                && codes.contains(&"order-without-limit"),
            "{codes:?}"
        );
        assert_eq!(format_code("json", "{\"a\":1}", 2).unwrap()["formatted"], "{\n  \"a\": 1\n}");
        assert!(format_code("python", "x=1", 2).is_err());
        assert_eq!(minify_code("css", "a { b: c; }").unwrap()["minified"], "a{b:c}");
    }
}
