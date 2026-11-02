//! Linters: Markdown, Dockerfile, YAML and docker-compose.

use engine_shared::{EngineError, EngineResult};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

const MAX: usize = 5 * 1024 * 1024;

fn issue(line: usize, severity: &str, rule: &str, message: impl Into<String>) -> Value {
    json!({ "line": line, "severity": severity, "rule": rule, "message": message.into() })
}

fn summary(issues: Vec<Value>, extra: Map<String, Value>) -> Value {
    let errors = issues.iter().filter(|i| i["severity"] == "error").count();
    let warnings = issues.iter().filter(|i| i["severity"] == "warning").count();
    let infos = issues.len() - errors - warnings;
    let mut v = json!({
        "valid": errors == 0,
        "issue_count": issues.len(),
        "errors": errors,
        "warnings": warnings,
        "info": infos,
        "issues": issues
    });
    for (k, val) in extra {
        v[k] = val;
    }
    v
}

fn check(text: &str, what: &str) -> EngineResult<()> {
    engine_shared::error::require_text(text, what)?;
    engine_shared::limits::check_size(text.len(), MAX, what)
}

// --------------------------------------------------------------- markdown --

/// Lints Markdown: heading levels, trailing spaces, list markers, bare URLs, line length.
pub fn markdown(text: &str, max_line_length: usize) -> EngineResult<Value> {
    check(text, "markdown")?;
    let lines: Vec<&str> = text.lines().collect();
    let mut issues = Vec::new();
    let mut in_code = false;
    let mut fence: Option<String> = None;
    let mut prev_heading_level = 0usize;
    let mut blank_run = 0usize;
    let mut list_marker: Option<char> = None;
    let mut first_content_line = true;
    let mut heading_texts: HashMap<String, usize> = HashMap::new();
    let mut headings = 0usize;
    let mut words = 0usize;
    let mut links = 0usize;
    let mut images = 0usize;
    let mut code_blocks = 0usize;
    for (idx, raw) in lines.iter().enumerate() {
        let n = idx + 1;
        let line = raw.trim_end_matches('\r');
        let trimmed = line.trim_start();
        // Fenced code blocks.
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let marker: String = trimmed.chars().take_while(|c| *c == '`' || *c == '~').collect();
            match &fence {
                None => {
                    fence = Some(marker.clone());
                    in_code = true;
                    code_blocks += 1;
                    if trimmed[marker.len()..].trim().is_empty() {
                        issues.push(issue(
                            n,
                            "info",
                            "MD040",
                            "Fenced code block has no language specified",
                        ));
                    }
                },
                Some(open) if trimmed.starts_with(open.as_str()) => {
                    fence = None;
                    in_code = false;
                },
                _ => {},
            }
            blank_run = 0;
            first_content_line = false;
            continue;
        }
        if in_code {
            continue;
        }
        if line.contains('\t') {
            issues.push(issue(n, "warning", "MD010", "Hard tab character; use spaces"));
        }
        if line.ends_with(' ') && !line.ends_with("  ") {
            issues.push(issue(
                n,
                "warning",
                "MD009",
                "Trailing whitespace (use exactly two spaces for a hard line break)",
            ));
        }
        if line.len() > 2 && line.ends_with("   ") {
            issues.push(issue(n, "warning", "MD009", "More than two trailing spaces"));
        }
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run == 2 {
                issues.push(issue(n, "warning", "MD012", "Multiple consecutive blank lines"));
            }
            continue;
        }
        blank_run = 0;
        if max_line_length > 0
            && line.chars().count() > max_line_length
            && !line.contains("http")
            && !line.starts_with('|')
        {
            issues.push(issue(
                n,
                "info",
                "MD013",
                format!("Line length {} exceeds {max_line_length}", line.chars().count()),
            ));
        }
        words += line.split_whitespace().count();
        links += line.matches("](").count();
        images += line.matches("![").count();
        // Headings.
        if let Some(rest) = trimmed.strip_prefix('#') {
            let level = 1 + rest.chars().take_while(|c| *c == '#').count();
            let content = rest.trim_start_matches('#');
            if level <= 6 {
                headings += 1;
                if !content.starts_with(' ') && !content.is_empty() {
                    issues.push(issue(n, "error", "MD018", "No space after hash in heading"));
                }
                let title = content.trim().trim_end_matches('#').trim();
                if title.is_empty() {
                    issues.push(issue(n, "error", "MD001", "Empty heading"));
                }
                if first_content_line && level != 1 {
                    issues.push(issue(
                        n,
                        "warning",
                        "MD002",
                        "First heading should be a top-level (H1) heading",
                    ));
                }
                if prev_heading_level > 0 && level > prev_heading_level + 1 {
                    issues.push(issue(
                        n,
                        "warning",
                        "MD001",
                        format!("Heading level jumps from H{prev_heading_level} to H{level}"),
                    ));
                }
                if title.ends_with(['.', ',', ';', ':', '!']) {
                    issues.push(issue(n, "info", "MD026", "Trailing punctuation in heading"));
                }
                if idx > 0 && !lines[idx - 1].trim().is_empty() {
                    issues.push(issue(
                        n,
                        "warning",
                        "MD022",
                        "Headings should be surrounded by blank lines",
                    ));
                }
                if lines.get(idx + 1).map(|l| !l.trim().is_empty()).unwrap_or(false) {
                    issues.push(issue(
                        n,
                        "warning",
                        "MD022",
                        "Headings should be followed by a blank line",
                    ));
                }
                if let Some(first) = heading_texts.get(&title.to_lowercase()) {
                    issues.push(issue(
                        n,
                        "warning",
                        "MD024",
                        format!("Duplicate heading `{title}` (first at line {first})"),
                    ));
                } else {
                    heading_texts.insert(title.to_lowercase(), n);
                }
                prev_heading_level = level;
            }
        } else if first_content_line && !trimmed.starts_with("---") && !trimmed.starts_with('<') {
            issues.push(issue(n, "info", "MD041", "First line should be a top-level heading"));
        }
        // Lists.
        let list_char = trimmed
            .chars()
            .next()
            .filter(|c| matches!(c, '-' | '*' | '+'))
            .filter(|_| trimmed.chars().nth(1) == Some(' '));
        if let Some(c) = list_char {
            match list_marker {
                None => list_marker = Some(c),
                Some(m) if m != c => {
                    issues.push(issue(
                        n,
                        "warning",
                        "MD004",
                        format!("Inconsistent list marker `{c}` (document uses `{m}`)"),
                    ));
                },
                _ => {},
            }
        }
        // Images without alt text.
        let mut search = line;
        while let Some(p) = search.find("![](") {
            issues.push(issue(n, "warning", "MD045", "Image without alternative text"));
            search = &search[p + 4..];
        }
        // Bare URLs.
        for w in line.split_whitespace() {
            if (w.starts_with("http://") || w.starts_with("https://"))
                && !line.contains(&format!("<{w}"))
                && !line.contains(&format!("({w}"))
                && !line.contains(&format!("]({w}"))
                && !line.contains(&format!("[{w}"))
            {
                issues.push(issue(
                    n,
                    "info",
                    "MD034",
                    format!(
                        "Bare URL `{}`; wrap in <> or a link",
                        w.chars().take(60).collect::<String>()
                    ),
                ));
                break;
            }
        }
        // Emphasis spacing `** text **`.
        if line.contains("** ") && line.contains(" **") && line.matches("**").count() >= 2 {
            issues.push(issue(n, "info", "MD037", "Spaces inside emphasis markers"));
        }
        // Empty links.
        if line.contains("]()") {
            issues.push(issue(n, "error", "MD042", "Link with empty URL"));
        }
        first_content_line = false;
    }
    if fence.is_some() {
        issues.push(issue(lines.len(), "error", "MD040", "Unclosed fenced code block"));
    }
    if !text.ends_with('\n') {
        issues.push(issue(lines.len(), "info", "MD047", "File should end with a single newline"));
    }
    let mut extra = Map::new();
    extra.insert(
        "stats".into(),
        json!({
            "lines": lines.len(),
            "words": words,
            "headings": headings,
            "links": links,
            "images": images,
            "code_blocks": code_blocks
        }),
    );
    Ok(summary(issues, extra))
}

// ------------------------------------------------------------- dockerfile --

/// Lints a Dockerfile: base image tags, `apt-get` hygiene, `ADD` vs `COPY`, root user, layer count.
pub fn dockerfile(text: &str) -> EngineResult<Value> {
    check(text, "dockerfile")?;
    let known = [
        "FROM",
        "RUN",
        "CMD",
        "LABEL",
        "MAINTAINER",
        "EXPOSE",
        "ENV",
        "ADD",
        "COPY",
        "ENTRYPOINT",
        "VOLUME",
        "USER",
        "WORKDIR",
        "ARG",
        "ONBUILD",
        "STOPSIGNAL",
        "HEALTHCHECK",
        "SHELL",
    ];
    // Join continuation lines.
    let mut logical: Vec<(usize, String)> = Vec::new();
    let mut current = String::new();
    let mut start_line = 0;
    for (idx, raw) in text.lines().enumerate() {
        let line = raw.trim_end();
        if current.is_empty() {
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                if line.trim_start().starts_with("# syntax")
                    || line.trim_start().starts_with("#syntax")
                {
                    logical.push((idx + 1, line.trim().to_string()));
                }
                continue;
            }
            start_line = idx + 1;
        }
        if let Some(stripped) = line.strip_suffix('\\') {
            current.push_str(stripped);
            current.push(' ');
        } else {
            current.push_str(line);
            logical.push((start_line, std::mem::take(&mut current)));
        }
    }
    if !current.is_empty() {
        logical.push((start_line, current));
    }
    let mut issues = Vec::new();
    let mut stages: Vec<String> = Vec::new();
    let mut seen_from = false;
    let mut from_count = 0;
    let mut cmd_count = 0;
    let mut entrypoint_count = 0;
    let mut has_user = false;
    let mut has_healthcheck = false;
    let mut run_count = 0;
    let mut consecutive_runs = 0;
    let mut exposed: Vec<String> = Vec::new();
    let mut instructions = 0;
    for (n, line) in &logical {
        if line.starts_with('#') {
            continue;
        }
        instructions += 1;
        let (instr, args) = line
            .trim()
            .split_once(char::is_whitespace)
            .map(|(i, a)| (i.to_string(), a.trim().to_string()))
            .unwrap_or((line.trim().to_string(), String::new()));
        let upper = instr.to_ascii_uppercase();
        if !known.contains(&upper.as_str()) {
            issues.push(issue(*n, "error", "DL1000", format!("Unknown instruction `{instr}`")));
            continue;
        }
        if instr != upper {
            issues.push(issue(
                *n,
                "info",
                "DL3000-case",
                format!("Instruction `{instr}` should be uppercase"),
            ));
        }
        if !seen_from && upper != "FROM" && upper != "ARG" {
            issues.push(issue(
                *n,
                "error",
                "DL3061",
                "The first instruction must be FROM (or ARG before FROM)",
            ));
        }
        if upper != "RUN" {
            consecutive_runs = 0;
        }
        match upper.as_str() {
            "FROM" => {
                seen_from = true;
                from_count += 1;
                let image = args.split_whitespace().next().unwrap_or("");
                if let Some(pos) = args.to_ascii_uppercase().find(" AS ") {
                    stages.push(args[pos + 4..].trim().to_string());
                }
                let is_stage_ref = stages.iter().any(|s| s == image);
                if !is_stage_ref && image != "scratch" && !image.starts_with('$') {
                    if !image.contains(':') && !image.contains('@') {
                        issues.push(issue(
                            *n,
                            "warning",
                            "DL3006",
                            format!("Image `{image}` has no tag; builds are not reproducible"),
                        ));
                    } else if image.ends_with(":latest") {
                        issues.push(issue(
                            *n,
                            "warning",
                            "DL3007",
                            "Using `latest` tag is non-deterministic; pin a version",
                        ));
                    }
                }
                has_user = false;
            },
            "MAINTAINER" => issues.push(issue(
                *n,
                "warning",
                "DL4000",
                "MAINTAINER is deprecated; use LABEL maintainer=...",
            )),
            "RUN" => {
                run_count += 1;
                consecutive_runs += 1;
                if consecutive_runs == 3 {
                    issues.push(issue(
                        *n,
                        "info",
                        "DL3059",
                        "Multiple consecutive RUN instructions; consider chaining with && to \
                         reduce layers",
                    ));
                }
                let a = args.as_str();
                if a.contains("apt-get install") || a.contains("apt install") {
                    if !a.contains("-y") && !a.contains("--yes") && !a.contains("--assume-yes") {
                        issues.push(issue(
                            *n,
                            "error",
                            "DL3014",
                            "apt-get install without -y will hang waiting for input",
                        ));
                    }
                    if !a.contains("--no-install-recommends") {
                        issues.push(issue(
                            *n,
                            "info",
                            "DL3015",
                            "Use --no-install-recommends with apt-get install",
                        ));
                    }
                    if !a.contains("rm -rf /var/lib/apt/lists") {
                        issues.push(issue(
                            *n,
                            "warning",
                            "DL3009",
                            "Delete /var/lib/apt/lists after apt-get install to shrink the image",
                        ));
                    }
                    if a.contains("apt-get install") && !a.contains("apt-get update") {
                        issues.push(issue(
                            *n,
                            "warning",
                            "DL3008-update",
                            "Run apt-get update in the same RUN as apt-get install",
                        ));
                    }
                }
                if a.contains("apt-get upgrade") || a.contains("dist-upgrade") {
                    issues.push(issue(
                        *n,
                        "warning",
                        "DL3005",
                        "Avoid apt-get upgrade/dist-upgrade in Dockerfiles",
                    ));
                }
                if a.contains("apk add") && !a.contains("--no-cache") {
                    issues.push(issue(*n, "info", "DL3019", "Use apk add --no-cache"));
                }
                if a.contains("pip install") && !a.contains("--no-cache-dir") {
                    issues.push(issue(*n, "info", "DL3042", "Use pip install --no-cache-dir"));
                }
                if a.contains("npm install") && !a.contains("npm ci") && !a.contains("-g") {
                    issues.push(issue(
                        *n,
                        "info",
                        "DL3016-npm",
                        "Prefer `npm ci` for reproducible installs",
                    ));
                }
                if a.contains("sudo ") {
                    issues.push(issue(
                        *n,
                        "warning",
                        "DL3004",
                        "Do not use sudo; use USER to switch users",
                    ));
                }
                if a.split("&&").any(|part| part.trim().starts_with("cd ")) {
                    issues.push(issue(*n, "warning", "DL3003", "Use WORKDIR instead of cd"));
                }
                if a.contains("curl") && a.contains("| sh") || a.contains("| bash") {
                    issues.push(issue(
                        *n,
                        "warning",
                        "DL-pipe-shell",
                        "Piping a download into a shell is risky; verify checksums",
                    ));
                }
                if a.contains("chmod 777") {
                    issues.push(issue(
                        *n,
                        "warning",
                        "DL-chmod777",
                        "chmod 777 grants world write access",
                    ));
                }
            },
            "ADD" => {
                if !args.starts_with("http")
                    && !args.ends_with(".tar")
                    && !args.contains(".tar.")
                    && !args.ends_with(".tgz")
                {
                    issues.push(issue(
                        *n,
                        "warning",
                        "DL3020",
                        "Use COPY instead of ADD for files and folders",
                    ));
                }
            },
            "COPY" => {
                if args.contains("--from") {
                    let from = args
                        .split("--from=")
                        .nth(1)
                        .and_then(|s| s.split_whitespace().next())
                        .unwrap_or("");
                    if !from.is_empty()
                        && !stages.iter().any(|s| s == from)
                        && from.parse::<usize>().is_err()
                        && !from.contains(':')
                        && !from.contains('/')
                    {
                        issues.push(issue(
                            *n,
                            "error",
                            "DL3022",
                            format!("COPY --from references unknown stage `{from}`"),
                        ));
                    }
                }
                if args.trim() == ". ." || args.trim() == "./ ./" {
                    issues.push(issue(
                        *n,
                        "info",
                        "DL-copy-all",
                        "COPY . . copies everything; add a .dockerignore",
                    ));
                }
            },
            "CMD" => {
                cmd_count += 1;
                if !args.trim_start().starts_with('[') {
                    issues.push(issue(
                        *n,
                        "info",
                        "DL3025",
                        "Use JSON (exec) form for CMD to receive signals correctly",
                    ));
                }
            },
            "ENTRYPOINT" => {
                entrypoint_count += 1;
                if !args.trim_start().starts_with('[') {
                    issues.push(issue(*n, "info", "DL3025", "Use JSON (exec) form for ENTRYPOINT"));
                }
            },
            "WORKDIR" => {
                if !args.starts_with('/') && !args.starts_with('$') {
                    issues.push(issue(
                        *n,
                        "warning",
                        "DL3000",
                        "WORKDIR should be an absolute path",
                    ));
                }
            },
            "USER" => {
                has_user = true;
                if args.trim() == "root" || args.trim() == "0" {
                    issues.push(issue(
                        *n,
                        "info",
                        "DL3002-root",
                        "Switching to root; ensure a later USER drops privileges",
                    ));
                }
            },
            "EXPOSE" => {
                for p in args.split_whitespace() {
                    let port = p.split('/').next().unwrap_or(p);
                    match port.parse::<u32>() {
                        Ok(v) if (1..=65535).contains(&v) => exposed.push(p.to_string()),
                        _ if port.starts_with('$') => exposed.push(p.to_string()),
                        _ => {
                            issues.push(issue(*n, "error", "DL3011", format!("Invalid port `{p}`")))
                        },
                    }
                }
            },
            "HEALTHCHECK" => has_healthcheck = true,
            "ENV" => {
                if args.contains("PASSWORD=")
                    || args.contains("SECRET=")
                    || args.contains("TOKEN=")
                    || args.contains("API_KEY=")
                {
                    issues.push(issue(
                        *n,
                        "warning",
                        "DL-secret",
                        "Secrets in ENV are baked into the image; use build secrets or runtime env",
                    ));
                }
            },
            "ARG"
                if args.to_ascii_uppercase().contains("PASSWORD")
                    || args.to_ascii_uppercase().contains("SECRET") =>
            {
                issues.push(issue(
                    *n,
                    "warning",
                    "DL-secret-arg",
                    "Build args are visible in image history; avoid secrets",
                ));
            },
            _ => {},
        }
    }
    if !seen_from {
        issues.push(issue(1, "error", "DL3061", "Dockerfile has no FROM instruction"));
    }
    if cmd_count > 1 {
        issues.push(issue(
            0,
            "warning",
            "DL4003",
            format!("{cmd_count} CMD instructions; only the last one takes effect"),
        ));
    }
    if entrypoint_count > 1 {
        issues.push(issue(
            0,
            "warning",
            "DL4004",
            format!("{entrypoint_count} ENTRYPOINT instructions; only the last one takes effect"),
        ));
    }
    if seen_from && !has_user {
        issues.push(issue(
            0,
            "info",
            "DL3002",
            "Container runs as root; add a USER instruction for the final stage",
        ));
    }
    if seen_from && !has_healthcheck && (cmd_count > 0 || entrypoint_count > 0) {
        issues.push(issue(0, "info", "DL-healthcheck", "No HEALTHCHECK defined"));
    }
    let mut extra = Map::new();
    extra.insert(
        "stats".into(),
        json!({
            "instructions": instructions,
            "stages": stages,
            "run_instructions": run_count,
            "exposed_ports": exposed,
            "multi_stage": from_count > 1
        }),
    );
    Ok(summary(issues, extra))
}

// ------------------------------------------------------------------- yaml --

/// Lints YAML: tabs, indentation, duplicate keys, trailing spaces, truthy values, line length.
pub fn yaml(text: &str, max_line_length: usize) -> EngineResult<Value> {
    check(text, "yaml")?;
    let mut issues = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut indent_unit: Option<usize> = None;
    let mut prev_indent = 0usize;
    let mut doc_count = 0;
    let mut has_doc_start = false;
    let mut in_block_scalar = false;
    let mut block_indent = 0usize;
    // (indent, keys seen at that indent within the current mapping)
    let mut scopes: Vec<(usize, Vec<String>)> = Vec::new();
    for (idx, raw) in lines.iter().enumerate() {
        let n = idx + 1;
        let line = raw.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if in_block_scalar {
            if indent > block_indent {
                continue;
            }
            in_block_scalar = false;
        }
        if line.starts_with("---") {
            doc_count += 1;
            if idx == 0
                || lines[..idx]
                    .iter()
                    .all(|l| l.trim().is_empty() || l.trim_start().starts_with('#'))
            {
                has_doc_start = true;
            }
            continue;
        }
        let content = line.trim_start();
        if content.starts_with('#') {
            if !content.starts_with("# ") && content.len() > 1 && !content.starts_with("#!") {
                issues.push(issue(n, "info", "comments", "Comment should start with `# `"));
            }
            continue;
        }
        if raw.contains('\t') && raw[..indent].contains('\t') {
            issues.push(issue(
                n,
                "error",
                "indentation",
                "Tabs are not allowed for indentation in YAML",
            ));
        }
        if raw.ends_with(' ') {
            issues.push(issue(n, "warning", "trailing-spaces", "Trailing whitespace"));
        }
        if max_line_length > 0 && line.chars().count() > max_line_length {
            issues.push(issue(
                n,
                "info",
                "line-length",
                format!("Line length {} exceeds {max_line_length}", line.chars().count()),
            ));
        }
        if indent > prev_indent {
            let step = indent - prev_indent;
            match indent_unit {
                None => indent_unit = Some(step),
                Some(u) if step != u && !content.starts_with('-') => issues.push(issue(
                    n,
                    "warning",
                    "indentation",
                    format!("Inconsistent indentation: expected {u} spaces, found {step}"),
                )),
                _ => {},
            }
        }
        prev_indent = indent;
        // Key checks.
        if let Some((key, value)) = split_key_value(content) {
            let v = value.trim();
            // Duplicate keys within one mapping.
            let key_indent = if content.starts_with("- ") { indent + 2 } else { indent };
            if content.starts_with("- ") {
                scopes.retain(|(i, _)| *i < key_indent);
            } else {
                scopes.retain(|(i, _)| *i <= key_indent);
            }
            match scopes.last_mut() {
                Some((i, keys)) if *i == key_indent => {
                    let k = key.trim().to_string();
                    if keys.contains(&k) {
                        issues.push(issue(
                            n,
                            "error",
                            "key-duplicates",
                            format!("Duplicate key `{k}` in the same mapping"),
                        ));
                    } else {
                        keys.push(k);
                    }
                },
                _ => scopes.push((key_indent, vec![key.trim().to_string()])),
            }
            if v.ends_with('|')
                || v.ends_with('>')
                || v.ends_with("|-")
                || v.ends_with(">-")
                || v.ends_with("|+")
                || v.ends_with(">+")
            {
                in_block_scalar = true;
                block_indent = indent;
            }
            if !key.is_empty()
                && !value.is_empty()
                && !value.starts_with(' ')
                && !key.ends_with('"')
                && !key.ends_with('\'')
            {
                issues.push(issue(
                    n,
                    "error",
                    "colons",
                    format!("Missing space after colon in `{key}:`"),
                ));
            }
            let lower = v.to_ascii_lowercase();
            if matches!(lower.as_str(), "yes" | "no" | "on" | "off" | "y" | "n") {
                issues.push(issue(
                    n,
                    "warning",
                    "truthy",
                    format!(
                        "Value `{v}` is parsed as boolean by YAML 1.1 parsers; quote it or use \
                         true/false"
                    ),
                ));
            }
            if v.starts_with('0') && v.len() > 1 && v.chars().all(|c| c.is_ascii_digit()) {
                issues.push(issue(
                    n,
                    "warning",
                    "octal-values",
                    format!("`{v}` may be parsed as octal; quote it"),
                ));
            }
            if v.contains(": ") && !v.starts_with(['"', '\'', '{', '[']) && !v.starts_with('#') {
                issues.push(issue(
                    n,
                    "warning",
                    "key-value",
                    "Unquoted value contains `: ` which may be parsed as a mapping",
                ));
            }
            if v == "~" || lower == "null" {
                issues.push(issue(
                    n,
                    "info",
                    "empty-values",
                    format!("Explicit null value for `{key}`"),
                ));
            }
        }
        if content.contains(",  ") || content.contains(" ,") {
            issues.push(issue(n, "info", "commas", "Irregular spacing around comma"));
        }
        if content.starts_with("- ") && content[2..].starts_with(' ') {
            issues.push(issue(n, "info", "hyphens", "Too many spaces after hyphen"));
        }
    }
    if !has_doc_start && doc_count == 0 && lines.len() > 20 {
        issues.push(issue(
            1,
            "info",
            "document-start",
            "Consider starting the document with `---`",
        ));
    }
    // Semantic parse (serde_yaml catches duplicate keys, bad structure).
    let mut docs = 0usize;
    let mut parse_error = None;
    let mut top_keys: Vec<String> = Vec::new();
    for doc in serde_yaml::Deserializer::from_str(text) {
        match Value::deserialize(doc) {
            Ok(v) => {
                docs += 1;
                if let Some(m) = v.as_object() {
                    if docs == 1 {
                        top_keys = m.keys().cloned().collect();
                    }
                }
            },
            Err(e) => {
                let msg = e.to_string();
                let line = e.location().map(|l| l.line()).unwrap_or(0);
                parse_error = Some(msg.clone());
                issues.push(issue(line, "error", "syntax", msg));
                break;
            },
        }
    }
    if !text.ends_with('\n') {
        issues.push(issue(
            lines.len(),
            "info",
            "new-line-at-end-of-file",
            "No newline at end of file",
        ));
    }
    let mut extra = Map::new();
    extra.insert(
        "stats".into(),
        json!({
            "lines": lines.len(),
            "documents": docs.max(doc_count.max(1)),
            "indent_unit": indent_unit,
            "top_level_keys": top_keys,
            "parse_error": parse_error
        }),
    );
    Ok(summary(issues, extra))
}

use serde::Deserialize;

fn split_key_value(content: &str) -> Option<(&str, &str)> {
    let c = content.strip_prefix("- ").unwrap_or(content);
    if c.starts_with(['"', '\'', '[', '{', '|', '>']) {
        // Quoted keys: find closing quote then colon.
        if let Some(q) = c.chars().next().filter(|q| *q == '"' || *q == '\'') {
            let end = c[1..].find(q)? + 1;
            let rest = &c[end + 1..];
            return rest.strip_prefix(':').map(|v| (&c[..=end], v));
        }
        return None;
    }
    let pos = c.find(':')?;
    let key = &c[..pos];
    if key.contains(' ') && !key.contains('"') {
        // Could be a plain scalar containing a colon; still treat as key if it looks like one.
    }
    if c[pos..].starts_with("://") {
        return None;
    }
    Some((key, &c[pos + 1..]))
}

// ---------------------------------------------------------------- compose --

/// Validates a Docker Compose file: services, images/builds, ports, volumes, depends_on cycles.
pub fn compose(text: &str) -> EngineResult<Value> {
    check(text, "compose")?;
    let doc: Value = serde_yaml::from_str(text)
        .map_err(|e| EngineError::malformed(format!("Invalid YAML: {e}")))?;
    let mut issues = Vec::new();
    let Some(root) = doc.as_object() else {
        return Err(EngineError::invalid("docker-compose file must be a mapping at the top level"));
    };
    if let Some(v) = root.get("version") {
        issues.push(issue(
            0,
            "info",
            "version-obsolete",
            format!(
                "Top-level `version: {}` is obsolete in the Compose Specification and ignored",
                v.as_str().unwrap_or(&v.to_string())
            ),
        ));
    }
    let allowed_top = [
        "version", "services", "networks", "volumes", "configs", "secrets", "name", "include", "x-",
    ];
    for k in root.keys() {
        if !allowed_top
            .iter()
            .any(|a| k == a || (a.ends_with('-') && k.starts_with(a)))
        {
            issues.push(issue(
                0,
                "error",
                "unknown-top-level",
                format!("Unknown top-level key `{k}`"),
            ));
        }
    }
    let services = root.get("services").and_then(Value::as_object);
    let Some(services) = services else {
        issues.push(issue(0, "error", "no-services", "No `services` section"));
        return Ok(summary(issues, Map::new()));
    };
    if services.is_empty() {
        issues.push(issue(0, "error", "no-services", "`services` is empty"));
    }
    let named_volumes: Vec<String> = root
        .get("volumes")
        .and_then(Value::as_object)
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    let named_networks: Vec<String> = root
        .get("networks")
        .and_then(Value::as_object)
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    let named_secrets: Vec<String> = root
        .get("secrets")
        .and_then(Value::as_object)
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    let service_names: Vec<String> = services.keys().cloned().collect();
    let mut host_ports: HashMap<String, String> = HashMap::new();
    let mut container_names: HashMap<String, String> = HashMap::new();
    let mut used_volumes: Vec<String> = Vec::new();
    let mut used_networks: Vec<String> = Vec::new();
    let mut graph: Vec<Value> = Vec::new();
    let known_keys = [
        "image",
        "build",
        "command",
        "entrypoint",
        "container_name",
        "depends_on",
        "environment",
        "env_file",
        "ports",
        "expose",
        "volumes",
        "networks",
        "restart",
        "healthcheck",
        "deploy",
        "labels",
        "logging",
        "user",
        "working_dir",
        "hostname",
        "extra_hosts",
        "dns",
        "cap_add",
        "cap_drop",
        "privileged",
        "read_only",
        "tmpfs",
        "ulimits",
        "sysctls",
        "secrets",
        "configs",
        "profiles",
        "platform",
        "pull_policy",
        "stop_grace_period",
        "stop_signal",
        "tty",
        "stdin_open",
        "init",
        "network_mode",
        "pid",
        "ipc",
        "shm_init",
        "shm_size",
        "security_opt",
        "devices",
        "links",
        "external_links",
        "volumes_from",
        "mem_limit",
        "cpus",
        "cpu_shares",
        "mem_reservation",
        "oom_kill_disable",
        "scale",
        "develop",
        "annotations",
        "attach",
        "cgroup",
        "runtime",
        "isolation",
        "mac_address",
        "domainname",
        "userns_mode",
        "group_add",
        "storage_opt",
        "cpu_count",
        "cpu_percent",
        "cpuset",
        "device_cgroup_rules",
        "extends",
        "post_start",
        "pre_stop",
        "gpus",
        "label_file",
        "models",
        "provider",
        "uts",
        "credential_spec",
        "blkio_config",
        "cpu_period",
        "cpu_quota",
        "cpu_rt_period",
        "cpu_rt_runtime",
        "memswap_limit",
        "mem_swappiness",
        "pids_limit",
        "oom_score_adj",
    ];
    for (name, svc) in services {
        let Some(s) = svc.as_object() else {
            issues.push(issue(
                0,
                "error",
                "service-not-mapping",
                format!("Service `{name}` must be a mapping"),
            ));
            continue;
        };
        for k in s.keys() {
            if !known_keys.contains(&k.as_str()) && !k.starts_with("x-") {
                issues.push(issue(
                    0,
                    "warning",
                    "unknown-service-key",
                    format!("Service `{name}`: unknown key `{k}`"),
                ));
            }
        }
        if !s.contains_key("image") && !s.contains_key("build") && !s.contains_key("extends") {
            issues.push(issue(
                0,
                "error",
                "image-or-build",
                format!("Service `{name}` needs `image` or `build`"),
            ));
        }
        if let Some(img) = s.get("image").and_then(Value::as_str) {
            if !img.contains(':') && !img.contains('@') && !img.contains("${") {
                issues.push(issue(
                    0,
                    "warning",
                    "image-tag",
                    format!("Service `{name}`: image `{img}` has no tag"),
                ));
            } else if img.ends_with(":latest") {
                issues.push(issue(
                    0,
                    "warning",
                    "image-latest",
                    format!("Service `{name}`: avoid the `latest` tag"),
                ));
            }
        }
        if let Some(cn) = s.get("container_name").and_then(Value::as_str) {
            if let Some(other) = container_names.get(cn) {
                issues.push(issue(
                    0,
                    "error",
                    "duplicate-container-name",
                    format!("container_name `{cn}` used by `{other}` and `{name}`"),
                ));
            }
            container_names.insert(cn.to_string(), name.clone());
            if s.get("deploy")
                .and_then(|d| d.get("replicas"))
                .and_then(Value::as_u64)
                .unwrap_or(1)
                > 1
            {
                issues.push(issue(
                    0,
                    "error",
                    "container-name-replicas",
                    format!("Service `{name}` sets container_name with replicas > 1"),
                ));
            }
        }
        if let Some(r) = s.get("restart").and_then(Value::as_str) {
            if !matches!(r, "no" | "always" | "on-failure" | "unless-stopped")
                && !r.starts_with("on-failure:")
            {
                issues.push(issue(
                    0,
                    "error",
                    "restart-policy",
                    format!("Service `{name}`: invalid restart policy `{r}`"),
                ));
            }
        } else {
            issues.push(issue(
                0,
                "info",
                "restart-missing",
                format!("Service `{name}` has no restart policy"),
            ));
        }
        if let Some(ports) = s.get("ports").and_then(Value::as_array) {
            for p in ports {
                let ps = match p {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Object(o) => format!(
                        "{}:{}",
                        o.get("published")
                            .map(|v| v.to_string().trim_matches('"').to_string())
                            .unwrap_or_default(),
                        o.get("target").map(|v| v.to_string()).unwrap_or_default()
                    ),
                    _ => String::new(),
                };
                let parts: Vec<&str> = ps.split(':').collect();
                let (host, container) = match parts.len() {
                    1 => (None, parts[0]),
                    2 => (Some(parts[0]), parts[1]),
                    3 => (Some(parts[1]), parts[2]),
                    _ => (None, ""),
                };
                let cport = container.split('/').next().unwrap_or("");
                if cport
                    .parse::<u32>()
                    .map(|v| v == 0 || v > 65535)
                    .unwrap_or(!cport.contains('-') && !cport.contains("${") && !cport.is_empty())
                {
                    issues.push(issue(
                        0,
                        "error",
                        "port-format",
                        format!("Service `{name}`: invalid port mapping `{ps}`"),
                    ));
                }
                if let Some(h) = host.filter(|h| !h.is_empty() && !h.contains("${")) {
                    if let Some(other) = host_ports.get(h) {
                        if other != name {
                            issues.push(issue(
                                0,
                                "error",
                                "port-conflict",
                                format!(
                                    "Host port {h} is published by both `{other}` and `{name}`"
                                ),
                            ));
                        }
                    }
                    host_ports.insert(h.to_string(), name.clone());
                    if let Ok(hp) = h.parse::<u32>() {
                        if hp < 1024 {
                            issues.push(issue(
                                0,
                                "info",
                                "privileged-port",
                                format!("Service `{name}` publishes privileged host port {hp}"),
                            ));
                        }
                    }
                }
            }
        }
        if let Some(deps) = s.get("depends_on") {
            let names: Vec<String> = match deps {
                Value::Array(a) => {
                    a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()
                },
                Value::Object(o) => {
                    for (d, cfg) in o {
                        if let Some(c) = cfg.get("condition").and_then(Value::as_str) {
                            if !matches!(
                                c,
                                "service_started"
                                    | "service_healthy"
                                    | "service_completed_successfully"
                            ) {
                                issues.push(issue(0, "error", "depends-condition", format!("Service `{name}`: invalid depends_on condition `{c}` for `{d}`")));
                            }
                            if c == "service_healthy"
                                && !services
                                    .get(d)
                                    .map(|s| s.get("healthcheck").is_some())
                                    .unwrap_or(true)
                            {
                                issues.push(issue(
                                    0,
                                    "warning",
                                    "healthcheck-missing",
                                    format!(
                                        "`{name}` waits for `{d}` to be healthy but `{d}` has no \
                                         healthcheck"
                                    ),
                                ));
                            }
                        }
                    }
                    o.keys().cloned().collect()
                },
                _ => vec![],
            };
            for d in &names {
                if !service_names.contains(d) {
                    issues.push(issue(
                        0,
                        "error",
                        "depends-unknown",
                        format!("Service `{name}` depends on unknown service `{d}`"),
                    ));
                }
                if d == name {
                    issues.push(issue(
                        0,
                        "error",
                        "depends-self",
                        format!("Service `{name}` depends on itself"),
                    ));
                }
                graph.push(json!({ "from": name, "to": d }));
            }
        }
        if let Some(vols) = s.get("volumes").and_then(Value::as_array) {
            for v in vols {
                let spec = match v {
                    Value::String(s) => s.clone(),
                    Value::Object(o) => {
                        if o.get("type").and_then(Value::as_str) == Some("volume") {
                            o.get("source").and_then(Value::as_str).unwrap_or("").to_string()
                        } else {
                            continue;
                        }
                    },
                    _ => continue,
                };
                let source = spec.split(':').next().unwrap_or("");
                if !source.is_empty() && !source.starts_with(['.', '/', '~', '$']) {
                    used_volumes.push(source.to_string());
                    if !named_volumes.contains(&source.to_string()) {
                        issues.push(issue(
                            0,
                            "error",
                            "volume-undefined",
                            format!(
                                "Service `{name}` uses named volume `{source}` which is not \
                                 declared under top-level `volumes`"
                            ),
                        ));
                    }
                }
                if spec.contains("/var/run/docker.sock") {
                    issues.push(issue(
                        0,
                        "warning",
                        "docker-socket",
                        format!("Service `{name}` mounts the Docker socket (host root equivalent)"),
                    ));
                }
            }
        }
        if let Some(nets) = s.get("networks") {
            let names: Vec<String> = match nets {
                Value::Array(a) => {
                    a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()
                },
                Value::Object(o) => o.keys().cloned().collect(),
                _ => vec![],
            };
            for nn in names {
                used_networks.push(nn.clone());
                if nn != "default" && !named_networks.contains(&nn) {
                    issues.push(issue(
                        0,
                        "error",
                        "network-undefined",
                        format!(
                            "Service `{name}` uses network `{nn}` which is not declared under \
                             top-level `networks`"
                        ),
                    ));
                }
            }
        }
        if let Some(secs) = s.get("secrets").and_then(Value::as_array) {
            for sec in secs {
                let sn = sec
                    .as_str()
                    .map(str::to_string)
                    .or_else(|| sec.get("source").and_then(Value::as_str).map(str::to_string))
                    .unwrap_or_default();
                if !named_secrets.contains(&sn) {
                    issues.push(issue(
                        0,
                        "error",
                        "secret-undefined",
                        format!("Service `{name}` uses undeclared secret `{sn}`"),
                    ));
                }
            }
        }
        if s.get("privileged").and_then(Value::as_bool) == Some(true) {
            issues.push(issue(
                0,
                "warning",
                "privileged",
                format!("Service `{name}` runs privileged"),
            ));
        }
        if let Some(env) = s.get("environment") {
            let entries: Vec<(String, String)> = match env {
                Value::Array(a) => a
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| {
                        s.split_once('=')
                            .map(|(k, v)| (k.to_string(), v.to_string()))
                            .unwrap_or((s.to_string(), String::new()))
                    })
                    .collect(),
                Value::Object(o) => o
                    .iter()
                    .map(|(k, v)| {
                        (k.clone(), v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()))
                    })
                    .collect(),
                _ => vec![],
            };
            for (k, v) in entries {
                let ku = k.to_ascii_uppercase();
                if (ku.contains("PASSWORD")
                    || ku.contains("SECRET")
                    || ku.contains("TOKEN")
                    || ku.contains("API_KEY"))
                    && !v.is_empty()
                    && !v.starts_with("${")
                {
                    issues.push(issue(
                        0,
                        "warning",
                        "hardcoded-secret",
                        format!(
                            "Service `{name}`: `{k}` looks like a hard-coded secret; use \
                             env_file, secrets or ${{VAR}} interpolation"
                        ),
                    ));
                }
            }
        }
        if let Some(hc) = s.get("healthcheck").and_then(Value::as_object) {
            if !hc.contains_key("test") && hc.get("disable") != Some(&Value::Bool(true)) {
                issues.push(issue(
                    0,
                    "error",
                    "healthcheck-test",
                    format!("Service `{name}`: healthcheck needs a `test`"),
                ));
            }
        }
    }
    for v in &named_volumes {
        if !used_volumes.contains(v) {
            issues.push(issue(
                0,
                "info",
                "volume-unused",
                format!("Named volume `{v}` is declared but not used"),
            ));
        }
    }
    for n in &named_networks {
        if !used_networks.contains(n) {
            issues.push(issue(
                0,
                "info",
                "network-unused",
                format!("Network `{n}` is declared but not used"),
            ));
        }
    }
    // Cycle detection in depends_on.
    let edges: Vec<(String, String)> = graph
        .iter()
        .filter_map(|g| Some((g["from"].as_str()?.to_string(), g["to"].as_str()?.to_string())))
        .collect();
    for start in &service_names {
        let mut stack = vec![start.clone()];
        let mut seen: Vec<String> = Vec::new();
        while let Some(cur) = stack.pop() {
            for (f, t) in &edges {
                if f == &cur {
                    if t == start {
                        issues.push(issue(
                            0,
                            "error",
                            "depends-cycle",
                            format!("Circular depends_on involving `{start}`"),
                        ));
                        stack.clear();
                        break;
                    }
                    if !seen.contains(t) {
                        seen.push(t.clone());
                        stack.push(t.clone());
                    }
                }
            }
        }
    }
    issues.sort_by(|a, b| a["message"].as_str().cmp(&b["message"].as_str()));
    issues.dedup();
    let mut extra = Map::new();
    extra.insert("services".into(), json!(service_names));
    extra.insert("volumes".into(), json!(named_volumes));
    extra.insert("networks".into(), json!(named_networks));
    extra.insert("dependency_graph".into(), json!(graph));
    Ok(summary(issues, extra))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_rules() {
        let r = markdown("## Intro\nSome text\t.\n\n\n- a\n* b\n![](x.png) http://x.y\n#Bad\n", 80)
            .unwrap();
        let rules: Vec<&str> = r["issues"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["rule"].as_str().unwrap())
            .collect();
        for expected in ["MD002", "MD010", "MD012", "MD004", "MD045", "MD034", "MD018", "MD022"] {
            assert!(rules.contains(&expected), "missing {expected}: {rules:?}");
        }
        assert_eq!(markdown("# Title\n\nHello\n", 0).unwrap()["issue_count"], 0);
    }

    #[test]
    fn dockerfile_rules() {
        let r = dockerfile(
            "FROM node\nMAINTAINER me\nRUN apt-get install curl\nRUN cd /app\nADD . /app\nWORKDIR \
             app\nEXPOSE 80 99999\nCMD npm start\nCMD npm run\nBOGUS x\n",
        )
        .unwrap();
        let rules: Vec<&str> = r["issues"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["rule"].as_str().unwrap())
            .collect();
        for expected in [
            "DL3006", "DL4000", "DL3014", "DL3003", "DL3020", "DL3000", "DL3011", "DL3025",
            "DL4003", "DL1000", "DL3002",
        ] {
            assert!(rules.contains(&expected), "missing {expected}: {rules:?}");
        }
        assert_eq!(r["valid"], false);
        let ok = dockerfile(
            "FROM node:20-alpine AS build\nWORKDIR /app\nCOPY package.json .\nRUN npm ci\nFROM \
             nginx:1.25\nCOPY --from=build /app/dist /usr/share/nginx/html\nUSER \
             nginx\nHEALTHCHECK CMD curl -f http://localhost/\nCMD [\"nginx\", \"-g\", \"daemon \
             off;\"]\n",
        )
        .unwrap();
        assert_eq!(ok["valid"], true, "{ok}");
        assert_eq!(ok["stats"]["multi_stage"], true);
    }

    #[test]
    fn yaml_rules() {
        let r = yaml("key:value\nflag: yes\nnum: 010\n\tbad: 1\nok: 1 \n", 0).unwrap();
        let rules: Vec<&str> = r["issues"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["rule"].as_str().unwrap())
            .collect();
        for expected in ["colons", "truthy", "octal-values", "indentation", "trailing-spaces"] {
            assert!(rules.contains(&expected), "missing {expected}: {rules:?}");
        }
        let dup = yaml("a: 1\na: 2\n", 0).unwrap();
        assert_eq!(dup["valid"], false);
        assert_eq!(yaml("a: 1\nb:\n  c: 2\n", 0).unwrap()["valid"], true);
    }

    #[test]
    fn compose_rules() {
        let r = compose(
            "version: '3'\nservices:\n  web:\n    image: nginx\n    ports: \
             ['80:80']\n    depends_on: [db, cache]\n    volumes: \
             ['data:/data']\n    environment:\n      DB_PASSWORD: hunter2\n  db:\n    image: \
             postgres:16\n    ports: ['80:5432']\n    restart: sometimes\nvolumes:\n  other: {}\n",
        )
        .unwrap();
        let rules: Vec<&str> = r["issues"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["rule"].as_str().unwrap())
            .collect();
        for expected in [
            "version-obsolete",
            "image-tag",
            "depends-unknown",
            "volume-undefined",
            "hardcoded-secret",
            "port-conflict",
            "restart-policy",
            "volume-unused",
        ] {
            assert!(rules.contains(&expected), "missing {expected}: {rules:?}");
        }
        let ok = compose("services:\n  app:\n    image: app:1.0\n    restart: unless-stopped\n")
            .unwrap();
        assert_eq!(ok["valid"], true);
        assert!(compose("- a\n- b").is_err());
    }
}
