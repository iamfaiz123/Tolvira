//! # Dev Toolbox Developer Engine (`developer-engine`)
//!
//! Parsers, formatters, validators and lookups for everyday development work.
//! Everything runs offline inside WASM; lookups use embedded tables.
//!
//! | Action | Tool(s) |
//! |---|---|
//! | `regex_test` | regex-tester |
//! | `regex_explain` | regex-explainer |
//! | `cron_parse` | cron-expression-parser |
//! | `cron_generate` | cron-expression-generator |
//! | `semver_check` / `semver_sort` / `semver_max_satisfying` | semver-checker |
//! | `color_convert` / `color_contrast` / `color_mix` | color-code-converter |
//! | `sql_format` / `sql_minify` / `sql_validate` / `sql_erd` | sql-formatter, sql-minifier, sql-validator, sql-to-entity-diagram |
//! | `graphql_format` / `graphql_validate` / `graphql_schema` | graphql-formatter, graphql-validator, graphql-schema-viewer |
//! | `html_format` / `html_minify` / `html_to_jsx` | html-formatter, html-minifier, html-to-jsx |
//! | `css_format` / `css_minify` | css-formatter, css-minifier |
//! | `js_format` / `js_minify` / `js_inspect` | javascript-formatter, javascript-minifier, typescript-formatter |
//! | `ts_to_js` | typescript-to-javascript → `E_UNSUPPORTED` (needs a full TS parser) |
//! | `lint_markdown` / `lint_dockerfile` / `lint_yaml` / `validate_compose` | markdown-linter, dockerfile-linter, yaml-linter, docker-compose-validator |
//! | `gitignore` / `gitignore_templates` | gitignore-generator |
//! | `env_parse` | env-file-parser |
//! | `diff` / `diff_words` / `patch_parse` / `patch_apply` | diff-viewer, patch-parser |
//! | `chmod` | chmod-calculator |
//! | `mime` | mime-type-lookup |
//! | `http_status` / `http_status_all` | http-status-lookup |
//! | `user_agent` | user-agent-parser |
//! | `url_parse` | url-parser |
//! | `query_parse` / `query_build` | query-string-parser |
//!
//! `json-to-code` is served by the JSON engine (`to_code`).

pub mod code;
pub use color_core as color;
pub mod cron_tools;
pub mod diff;
pub mod graphql;
pub mod js;
pub mod lint;
pub mod markup;
pub mod misc;
pub mod regex_tools;
pub mod semver_tools;
pub mod sql;

use engine_shared::{EngineError, EngineResult};
use serde::Deserialize;
use serde_json::Value;

/// Compiles a regular expression that is part of the engine source. A failure
/// here is a programming error, but it is reported as `E_INTERNAL` rather than
/// aborting the WASM instance.
pub(crate) fn re(pattern: &str) -> EngineResult<fancy_regex::Regex> {
    fancy_regex::Regex::new(pattern)
        .map_err(|e| EngineError::internal(format!("built-in pattern failed to compile: {e}")))
}

fn default_true() -> bool {
    true
}
fn default_indent() -> usize {
    2
}
fn default_context() -> usize {
    3
}

/// Every action understood by the developer engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum DeveloperAction {
    /// Runs a regex against text and lists matches, groups and replacements.
    RegexTest {
        /// Regular expression (fancy-regex syntax: look-around and back-references allowed).
        pattern: String,
        /// Flags (`i`, `m`, `s`, `x`, `u`).
        #[serde(default)]
        flags: String,
        /// Text to search.
        #[serde(default)]
        text: String,
        /// Replacement template (`$1`, `${name}`) to apply.
        #[serde(default)]
        replacement: Option<String>,
        /// Maximum matches returned.
        #[serde(default)]
        max_matches: usize,
    },
    /// Explains a regex as a tree of plain-English nodes.
    RegexExplain {
        /// Regular expression.
        pattern: String,
        /// Flags.
        #[serde(default)]
        flags: String,
    },
    /// Parses a cron expression: description, fields and next run times.
    CronParse {
        /// Cron expression (5 or 6 fields, aliases like `@daily`).
        expression: String,
        /// Reference time (ISO string or Unix seconds) for the next runs.
        #[serde(default)]
        from: Option<Value>,
        /// Number of next runs.
        #[serde(default)]
        count: usize,
    },
    /// Builds a cron expression from presets or fields.
    CronGenerate {
        /// Generator inputs.
        #[serde(flatten)]
        input: cron_tools::GenerateInput,
    },
    /// Parses a version, compares with another and/or tests a range.
    SemverCheck {
        /// Version string.
        version: String,
        /// Version to compare against.
        #[serde(default)]
        other: Option<String>,
        /// Range such as `^1.2.0` or `>=1 <2`.
        #[serde(default)]
        range: Option<String>,
    },
    /// Sorts versions.
    SemverSort {
        /// Version strings.
        versions: Vec<String>,
        /// Descending order.
        #[serde(default)]
        descending: bool,
    },
    /// Highest version satisfying a range.
    SemverMaxSatisfying {
        /// Candidate versions.
        versions: Vec<String>,
        /// Range expression.
        range: String,
        /// Allow pre-release versions.
        #[serde(default)]
        include_prerelease: bool,
    },
    /// Parses a colour and returns every representation.
    ColorConvert {
        /// Colour in any CSS syntax (`#fff`, `rgb()`, `hsl()`, names…).
        color: String,
    },
    /// WCAG contrast between two colours.
    ColorContrast {
        /// Foreground colour.
        foreground: String,
        /// Background colour.
        background: String,
    },
    /// Mixes two colours.
    ColorMix {
        /// First colour.
        a: String,
        /// Second colour.
        b: String,
        /// Weight of `b` (0–1, default 0.5).
        #[serde(default)]
        weight: Option<f64>,
    },
    /// Formats SQL.
    SqlFormat {
        /// SQL text.
        sql: String,
        /// Indent width.
        #[serde(default = "default_indent")]
        indent: usize,
        /// Upper-case keywords.
        #[serde(default = "default_true")]
        uppercase: bool,
        /// Blank lines between statements.
        #[serde(default)]
        lines_between_queries: u8,
    },
    /// Minifies SQL.
    SqlMinify {
        /// SQL text.
        sql: String,
    },
    /// Parses SQL and reports syntax errors and statement kinds.
    SqlValidate {
        /// SQL text.
        sql: String,
        /// `generic` (default), `postgres`, `mysql`, `sqlite`, `mssql`.
        #[serde(default)]
        dialect: String,
    },
    /// Builds an ER diagram (Mermaid + DBML) from `CREATE TABLE` statements.
    SqlErd {
        /// DDL text.
        sql: String,
        /// SQL dialect.
        #[serde(default)]
        dialect: String,
        /// Infer relations from `<table>_id` column names.
        #[serde(default = "default_true")]
        infer_from_names: bool,
    },
    /// Formats a GraphQL schema or query.
    GraphqlFormat {
        /// GraphQL text.
        text: String,
        /// `schema`, `query` or `auto` (default).
        #[serde(default)]
        kind: String,
    },
    /// Validates GraphQL text, optionally against a schema.
    GraphqlValidate {
        /// GraphQL text.
        text: String,
        /// `schema`, `query` or `auto`.
        #[serde(default)]
        kind: String,
        /// Schema SDL used to validate a query.
        #[serde(default)]
        schema: Option<String>,
    },
    /// Summarises a schema: types, fields, relationships, Mermaid class diagram.
    GraphqlSchema {
        /// Schema SDL.
        text: String,
    },
    /// Pretty-prints HTML.
    HtmlFormat {
        /// HTML text.
        html: String,
        /// Indent width.
        #[serde(default = "default_indent")]
        indent: usize,
        /// Inline elements shorter than this stay on one line.
        #[serde(default)]
        max_inline: usize,
    },
    /// Minifies HTML.
    HtmlMinify {
        /// HTML text.
        html: String,
        /// Remove comments.
        #[serde(default = "default_true")]
        remove_comments: bool,
        /// Collapse `disabled="disabled"` style attributes.
        #[serde(default = "default_true")]
        collapse_boolean_attributes: bool,
    },
    /// Converts HTML to a JSX/TSX component.
    HtmlToJsx {
        /// HTML text.
        html: String,
        /// Component name.
        #[serde(default)]
        component_name: String,
        /// Emit TypeScript.
        #[serde(default)]
        typescript: bool,
    },
    /// Formats CSS.
    CssFormat {
        /// CSS text.
        css: String,
        /// Indent width.
        #[serde(default = "default_indent")]
        indent: usize,
    },
    /// Minifies CSS.
    CssMinify {
        /// CSS text.
        css: String,
    },
    /// Formats JavaScript (token-level; see `engines/README.md` for limits).
    JsFormat {
        /// JavaScript source.
        source: String,
        /// Indent width.
        #[serde(default = "default_indent")]
        indent: usize,
        /// Insert missing semicolons.
        #[serde(default = "default_true")]
        semicolons: bool,
        /// Prefer single quotes.
        #[serde(default)]
        single_quotes: bool,
    },
    /// Minifies JavaScript (whitespace/comments; no renaming).
    JsMinify {
        /// JavaScript source.
        source: String,
        /// Keep `/*! … */` license comments.
        #[serde(default = "default_true")]
        keep_license: bool,
    },
    /// Reports tokens, balance, features and TypeScript hints for JavaScript.
    JsInspect {
        /// JavaScript source.
        source: String,
    },
    /// Always returns `E_UNSUPPORTED` (transpiling needs the TypeScript compiler in the browser).
    TsToJs {
        /// TypeScript source (ignored).
        #[serde(default)]
        source: String,
    },
    /// Lints Markdown (headings, lists, line length, links…).
    LintMarkdown {
        /// Markdown text.
        text: String,
        /// Maximum line length (0 = ignore).
        #[serde(default)]
        max_line_length: usize,
    },
    /// Lints a Dockerfile (Hadolint-style rules).
    LintDockerfile {
        /// Dockerfile text.
        text: String,
    },
    /// Lints YAML (indentation, duplicates, line length, truthy values).
    LintYaml {
        /// YAML text.
        text: String,
        /// Maximum line length (0 = ignore).
        #[serde(default)]
        max_line_length: usize,
    },
    /// Validates a Docker Compose file.
    ValidateCompose {
        /// Compose YAML.
        text: String,
    },
    /// Generates a `.gitignore` from stack templates.
    Gitignore {
        /// Template names (`node`, `python`, `rust`, `macos`, …).
        #[serde(default)]
        stacks: Vec<String>,
        /// Extra patterns appended verbatim.
        #[serde(default)]
        custom: Vec<String>,
    },
    /// Lists available `.gitignore` templates.
    GitignoreTemplates {},
    /// Parses a `.env` file with interpolation and exports to other formats.
    EnvParse {
        /// Dotenv text.
        text: String,
        /// Expand `${VAR}` references.
        #[serde(default = "default_true")]
        interpolate: bool,
        /// `json` (default), `shell`, `docker`, `compose`, `dotenv`, `typescript`, `example`,
        /// `github`.
        #[serde(default)]
        output: String,
    },
    /// Unified diff of two texts.
    Diff {
        /// Old text.
        old: String,
        /// New text.
        new: String,
        /// Old file name for the header.
        #[serde(default)]
        old_name: String,
        /// New file name for the header.
        #[serde(default)]
        new_name: String,
        /// Context lines.
        #[serde(default = "default_context")]
        context: usize,
        /// Ignore whitespace differences.
        #[serde(default)]
        ignore_whitespace: bool,
        /// Ignore case differences.
        #[serde(default)]
        ignore_case: bool,
        /// Also return an inline (side-by-side friendly) representation.
        #[serde(default = "default_true")]
        inline: bool,
    },
    /// Word/character-level diff.
    DiffWords {
        /// Old text.
        old: String,
        /// New text.
        new: String,
        /// `words` (default) or `chars`.
        #[serde(default)]
        mode: String,
    },
    /// Parses a unified/git patch.
    PatchParse {
        /// Patch text.
        patch: String,
    },
    /// Applies a unified patch to text.
    PatchApply {
        /// Original text.
        text: String,
        /// Patch text.
        patch: String,
        /// Apply in reverse (un-apply).
        #[serde(default)]
        reverse: bool,
        /// Lines of fuzz allowed when matching context.
        #[serde(default)]
        fuzz: usize,
    },
    /// Converts between symbolic/octal permission notations and explains them.
    Chmod {
        /// `755`, `rwxr-xr-x`, `u+x`, …
        mode: String,
    },
    /// Looks up MIME types by extension/name or sniffs a sample.
    Mime {
        /// Extension, file name or MIME type.
        #[serde(default)]
        query: String,
        /// File bytes (base64) to sniff.
        #[serde(default, with = "engine_shared::b64::option")]
        sample: Option<Vec<u8>>,
    },
    /// Looks up an HTTP status code or name.
    HttpStatus {
        /// Code (`404`) or text (`not found`).
        query: String,
    },
    /// Lists every HTTP status code.
    HttpStatusAll {},
    /// Parses a User-Agent string (browser, engine, OS, device, bot).
    UserAgent {
        /// User-Agent string.
        user_agent: String,
    },
    /// Parses a URL into components, query parameters and normalised forms.
    UrlParse {
        /// URL text.
        url: String,
    },
    /// Parses a query string into an object (repeated keys → arrays).
    QueryParse {
        /// Query string (with or without `?`).
        query: String,
    },
    /// Builds a query string from an object.
    QueryBuild {
        /// Key/value object.
        object: Value,
        /// `brackets` (`a[]=1`, default), `indices` (`a[0]=1`), `repeat` (`a=1&a=2`) or `comma`.
        #[serde(default)]
        array_format: String,
        /// Sort keys.
        #[serde(default)]
        sort: bool,
    },
    // code_analysis catalog engine
    /// Counts code/comment/blank lines per file and language.
    CodeLineCount {
        /// Source files.
        files: Vec<code::SourceFile>,
    },
    /// Cyclomatic-complexity heuristics per function.
    CodeComplexity {
        /// Source files.
        files: Vec<code::SourceFile>,
    },
    /// Finds duplicated blocks across files.
    CodeDuplicates {
        /// Source files.
        files: Vec<code::SourceFile>,
        /// Minimum block length in lines.
        #[serde(default)]
        min_lines: usize,
        /// Ignore string literal contents when comparing.
        #[serde(default = "default_true")]
        ignore_strings: bool,
    },
    /// Sorts and groups import statements.
    ImportSort {
        /// Source file.
        #[serde(flatten)]
        file: code::SourceFile,
    },
    /// Finds imported identifiers that are never used.
    UnusedImports {
        /// Source file.
        #[serde(flatten)]
        file: code::SourceFile,
    },
    /// Lists dependencies from manifests (package.json, Cargo.toml, requirements.txt, go.mod…) with
    /// optional licence info.
    DependencyScan {
        /// Manifest files.
        files: Vec<code::SourceFile>,
        /// Package → licence map supplied by the caller.
        #[serde(default)]
        licenses: serde_json::Map<String, Value>,
    },
    /// Static SQL analysis.
    SqlAnalyze {
        /// SQL text.
        sql: String,
        /// SQL dialect.
        #[serde(default)]
        dialect: String,
    },
    /// Formats code in a supported language.
    CodeFormat {
        /// `js`, `css`, `html`, `json`, `sql`, `graphql`.
        #[serde(default)]
        language: String,
        /// Source text.
        source: String,
        /// Indent width.
        #[serde(default = "default_indent")]
        indent: usize,
    },
    /// Minifies code in a supported language.
    CodeMinify {
        /// `js`, `css`, `html`, `json`, `sql`.
        #[serde(default)]
        language: String,
        /// Source text.
        source: String,
    },
    /// Always returns `E_UNSUPPORTED`: playgrounds/sandboxes run in the browser JavaScript runtime.
    Sandbox {
        /// Catalog tool id, echoed back in the guidance.
        #[serde(default)]
        tool: String,
    },
}

/// Dispatches a [`DeveloperAction`] to its implementation.
pub fn handle_action(action: DeveloperAction) -> EngineResult<Value> {
    match action {
        DeveloperAction::RegexTest {
            pattern,
            flags,
            text,
            replacement,
            max_matches,
        } => regex_tools::test(&pattern, &flags, &text, replacement.as_deref(), max_matches),
        DeveloperAction::RegexExplain { pattern, flags } => regex_tools::explain(&pattern, &flags),
        DeveloperAction::CronParse { expression, from, count } => {
            cron_tools::parse(&expression, from.as_ref(), if count == 0 { 5 } else { count })
        },
        DeveloperAction::CronGenerate { input } => cron_tools::generate(&input),
        DeveloperAction::SemverCheck { version, other, range } => {
            semver_tools::check(&version, other.as_deref(), range.as_deref())
        },
        DeveloperAction::SemverSort { versions, descending } => {
            semver_tools::sort(&versions, descending)
        },
        DeveloperAction::SemverMaxSatisfying { versions, range, include_prerelease } => {
            semver_tools::max_satisfying(&versions, &range, include_prerelease)
        },
        DeveloperAction::ColorConvert { color } => color::convert(&color),
        DeveloperAction::ColorContrast { foreground, background } => {
            color::contrast(&foreground, &background)
        },
        DeveloperAction::ColorMix { a, b, weight } => color::mix(&a, &b, weight.unwrap_or(0.5)),
        DeveloperAction::SqlFormat {
            sql,
            indent,
            uppercase,
            lines_between_queries,
        } => sql::format(&sql, indent, uppercase, lines_between_queries),
        DeveloperAction::SqlMinify { sql } => sql::minify(&sql),
        DeveloperAction::SqlValidate { sql, dialect } => sql::validate(&sql, &dialect),
        DeveloperAction::SqlErd { sql, dialect, infer_from_names } => {
            sql::erd(&sql, &dialect, infer_from_names)
        },
        DeveloperAction::GraphqlFormat { text, kind } => graphql::format(&text, &kind),
        DeveloperAction::GraphqlValidate { text, kind, schema } => {
            graphql::validate(&text, &kind, schema.as_deref())
        },
        DeveloperAction::GraphqlSchema { text } => graphql::schema(&text),
        DeveloperAction::HtmlFormat { html, indent, max_inline } => {
            markup::format_html(&html, indent, max_inline)
        },
        DeveloperAction::HtmlMinify {
            html,
            remove_comments,
            collapse_boolean_attributes,
        } => markup::minify_html(&html, remove_comments, collapse_boolean_attributes),
        DeveloperAction::HtmlToJsx { html, component_name, typescript } => {
            markup::html_to_jsx(&html, &component_name, typescript)
        },
        DeveloperAction::CssFormat { css, indent } => markup::format_css(&css, indent),
        DeveloperAction::CssMinify { css } => markup::minify_css(&css),
        DeveloperAction::JsFormat {
            source,
            indent,
            semicolons,
            single_quotes,
        } => js::format(&source, indent, semicolons, single_quotes),
        DeveloperAction::JsMinify { source, keep_license } => js::minify(&source, keep_license),
        DeveloperAction::JsInspect { source } => js::inspect(&source),
        DeveloperAction::TsToJs { .. } => Err(js::ts_to_js_unsupported()),
        DeveloperAction::LintMarkdown { text, max_line_length } => {
            lint::markdown(&text, max_line_length)
        },
        DeveloperAction::LintDockerfile { text } => lint::dockerfile(&text),
        DeveloperAction::LintYaml { text, max_line_length } => lint::yaml(&text, max_line_length),
        DeveloperAction::ValidateCompose { text } => lint::compose(&text),
        DeveloperAction::Gitignore { stacks, custom } => misc::gitignore(&stacks, &custom),
        DeveloperAction::GitignoreTemplates {} => Ok(misc::gitignore_list()),
        DeveloperAction::EnvParse { text, interpolate, output } => {
            misc::env_parse(&text, interpolate, &output)
        },
        DeveloperAction::Diff {
            old,
            new,
            old_name,
            new_name,
            context,
            ignore_whitespace,
            ignore_case,
            inline,
        } => diff::diff(
            &old,
            &new,
            &old_name,
            &new_name,
            context,
            ignore_whitespace,
            ignore_case,
            inline,
        ),
        DeveloperAction::DiffWords { old, new, mode } => diff::diff_words(&old, &new, &mode),
        DeveloperAction::PatchParse { patch } => diff::parse_patch(&patch),
        DeveloperAction::PatchApply { text, patch, reverse, fuzz } => {
            diff::apply_patch(&text, &patch, reverse, fuzz)
        },
        DeveloperAction::Chmod { mode } => misc::chmod(&mode),
        DeveloperAction::Mime { query, sample } => misc::mime(&query, sample.as_deref()),
        DeveloperAction::HttpStatus { query } => misc::http_status(&query),
        DeveloperAction::HttpStatusAll {} => Ok(misc::http_status_all()),
        DeveloperAction::UserAgent { user_agent } => misc::user_agent(&user_agent),
        DeveloperAction::UrlParse { url } => misc::url_parse(&url),
        DeveloperAction::QueryParse { query } => misc::query_parse(&query),
        DeveloperAction::QueryBuild { object, array_format, sort } => {
            misc::query_build(&object, &array_format, sort)
        },
        DeveloperAction::CodeLineCount { files } => code::count_lines(&files),
        DeveloperAction::CodeComplexity { files } => code::complexity(&files),
        DeveloperAction::CodeDuplicates { files, min_lines, ignore_strings } => {
            code::duplicates(&files, if min_lines == 0 { 5 } else { min_lines }, ignore_strings)
        },
        DeveloperAction::ImportSort { file } => code::sort_imports(&file),
        DeveloperAction::UnusedImports { file } => code::unused_imports(&file),
        DeveloperAction::DependencyScan { files, licenses } => {
            code::dependencies(&files, &licenses)
        },
        DeveloperAction::SqlAnalyze { sql, dialect } => code::sql_analyze(&sql, &dialect),
        DeveloperAction::CodeFormat { language, source, indent } => {
            code::format_code(&language, &source, indent)
        },
        DeveloperAction::CodeMinify { language, source } => code::minify_code(&language, &source),
        DeveloperAction::Sandbox { tool } => Err(EngineError::unsupported(format!(
            "{} executes untrusted code; run it in the browser JavaScript runtime (sandboxed \
             iframe / Web Worker) or a backend sandbox, not in this analysis engine",
            if tool.is_empty() {
                "Code execution"
            } else {
                tool.as_str()
            }
        ))),
    }
}

engine_shared::export_engine!(handle_action, DeveloperAction);
