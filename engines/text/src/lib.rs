//! # Dev Toolbox Text Engine (`text-engine`)
//!
//! Unicode-aware text utilities. The five v1 actions (`convert_case`,
//! `analyze`, `reverse`, `clean`, `format_json`) are preserved unchanged for
//! the existing UI components.
//!
//! | Action | Tool(s) |
//! |---|---|
//! | `stats` | word/character/line/sentence/paragraph counters, reading & speaking time, readability |
//! | `keyword_density` / `hashtags` | keyword-density-checker, hashtag-generator |
//! | `case` | text-case-converter, uppercase/lowercase/title/sentence-case converters |
//! | `slug` / `reverse_text` / `truncate` / `wrap` | slug-generator, text-reverser, text-truncator, text-formatter |
//! | `cleanup` | remove-extra-spaces, text-cleaner, unicode normalisation |
//! | `find_replace` | find-and-replace (literal / regex / whole word) |
//! | `lines` / `join` / `split` | sort/dedupe/number/filter lines, text-joiner, text-splitter |
//! | `diff` | text-diff, text-compare |
//! | `encode_codes` / `decode_codes` | text-to-binary/hex/decimal/unicode and back |
//! | `rot` / `morse_encode` / `morse_decode` | rot13, caesar, morse-code-converter |
//! | `unicode_inspect` / `unicode_normalize` / `unicode_escape` / `unicode_unescape` | unicode-inspector, unicode-normalizer, unicode-encoder/decoder |
//! | `extract` | text-extractor (emails, urls, numbers, hashtags, custom regex) |
//! | `markdown_to_html` / `markdown_parse` / `html_to_markdown` / `html_to_text` | markdown tools |
//! | `lorem` / `random_text` / `tts_prep` | generators, text-to-speech-prep |

pub mod analysis;
pub mod encoding;
pub mod generate;
pub mod html;
pub mod markdown;
pub mod transform;

use engine_shared::{limits, EngineError, EngineResult};
use serde::Deserialize;
use serde_json::{json, Value};

/// Compiles a regular expression that is part of the engine source. A failure
/// here is a programming error, but it is reported as `E_INTERNAL` rather than
/// aborting the WASM instance.
pub(crate) fn re(pattern: &str) -> EngineResult<regex::Regex> {
    regex::Regex::new(pattern)
        .map_err(|e| EngineError::internal(format!("built-in pattern failed to compile: {e}")))
}

fn default_reading_wpm() -> f64 {
    225.0
}
fn default_speaking_wpm() -> f64 {
    150.0
}
fn default_top() -> usize {
    10
}
fn default_true() -> bool {
    true
}
fn default_sep() -> String {
    " ".into()
}
fn default_dash() -> String {
    "-".into()
}
fn default_dot() -> String {
    ".".into()
}
fn default_word_sep() -> String {
    " / ".into()
}
fn default_base() -> u32 {
    2
}
fn default_13() -> i32 {
    13
}
fn default_count() -> usize {
    3
}
fn default_min_len() -> usize {
    3
}
fn default_context() -> usize {
    3
}
fn default_ellipsis() -> String {
    "…".into()
}
fn default_width() -> usize {
    80
}
fn default_comma() -> String {
    ",".into()
}

/// Every action understood by the text engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum TextAction {
    // ----- legacy v1 ------------------------------------------------------
    /// v1: change letter case (`target_case`: upper/lower/title/camel/…).
    ConvertCase {
        /// Input text.
        text: String,
        /// `upper`, `lower`, `title`, `smart-title`, `sentence`, `camel`, `pascal`, `snake`,
        /// `kebab`,
        /// `constant`, `dot`, `path`, `train`, `alternating` or `inverse`.
        target_case: String,
    },
    /// v1: word/char/line/sentence counts.
    Analyze {
        /// Input text.
        text: String,
    },
    /// v1: reverse characters.
    Reverse {
        /// Input text.
        text: String,
    },
    /// v1: tidy whitespace and lines.
    Clean {
        /// Input text.
        text: String,
        /// Trim each line.
        #[serde(default)]
        trim_lines: bool,
        /// Drop empty lines.
        #[serde(default)]
        remove_empty: bool,
        /// Remove duplicate lines.
        #[serde(default)]
        dedup_lines: bool,
        /// Sort lines alphabetically.
        #[serde(default)]
        sort_lines: bool,
    },
    /// v1: pretty-print JSON text.
    FormatJson {
        /// JSON source.
        text: String,
        /// Indent width in spaces.
        #[serde(default)]
        indent: usize,
    },

    // ----- analysis -------------------------------------------------------
    /// Detailed statistics: counts, reading/speaking time, frequencies.
    Stats {
        /// Input text.
        text: String,
        /// Reading speed used for the time estimate.
        #[serde(default = "default_reading_wpm")]
        reading_wpm: f64,
        /// Speaking speed used for the time estimate.
        #[serde(default = "default_speaking_wpm")]
        speaking_wpm: f64,
        /// How many most-frequent words to return.
        #[serde(default = "default_top")]
        top_words: usize,
    },
    /// One/two/three-word keyword density table.
    KeywordDensity {
        /// Input text.
        text: String,
        /// Number of entries per table.
        #[serde(default = "default_top")]
        top: usize,
        /// Minimum word length to count.
        #[serde(default = "default_min_len")]
        min_length: usize,
        /// Extra stop words to ignore.
        #[serde(default)]
        stop_words: Vec<String>,
    },
    /// Hashtag suggestions from frequent words.
    Hashtags {
        /// Input text.
        text: String,
        /// Number of hashtags.
        #[serde(default = "default_top")]
        count: usize,
    },

    // ----- transforms -----------------------------------------------------
    /// Convert between cases (upper, lower, title, sentence, camel, pascal, snake, kebab, constant,
    /// alternating…).
    Case {
        /// Input text.
        text: String,
        /// Target case name.
        target: String,
    },
    /// URL slug generation.
    Slug {
        /// Input text.
        text: String,
        /// Word separator (default `-`).
        #[serde(default = "default_dash")]
        separator: String,
        /// Lower-case the result.
        #[serde(default = "default_true")]
        lowercase: bool,
        /// Maximum slug length (0 = unlimited).
        #[serde(default)]
        max_length: usize,
    },
    /// Reverse by characters, words or lines.
    ReverseText {
        /// Input text.
        text: String,
        /// `chars`, `words` or `lines`.
        #[serde(default)]
        mode: String,
    },
    /// Truncate with an ellipsis-style suffix.
    Truncate {
        /// Input text.
        text: String,
        /// Maximum number of characters.
        #[serde(default)]
        max_chars: Option<usize>,
        /// Maximum number of words.
        #[serde(default)]
        max_words: Option<usize>,
        /// Suffix appended when truncated.
        #[serde(default = "default_ellipsis")]
        suffix: String,
        /// Avoid cutting inside a word.
        #[serde(default = "default_true")]
        preserve_words: bool,
    },
    /// Configurable cleanup pipeline (see [`transform::CleanupOptions`]).
    Cleanup {
        /// Input text.
        text: String,
        /// Cleanup switches.
        #[serde(flatten)]
        options: transform::CleanupOptions,
    },
    /// Hard/soft wrap at a column.
    Wrap {
        /// Input text.
        text: String,
        /// Column width.
        #[serde(default = "default_width")]
        width: usize,
        /// `wrap`, `hard` or `unwrap`.
        #[serde(default)]
        mode: String,
        /// Prefix for continuation lines.
        #[serde(default)]
        indent: String,
    },
    /// Literal or regex find & replace.
    FindReplace {
        /// Input text.
        text: String,
        /// Search term or pattern.
        find: String,
        /// Replacement (supports `$1` groups in regex mode).
        #[serde(default)]
        replace: String,
        /// Treat `find` as a regular expression.
        #[serde(default)]
        regex: bool,
        /// Case-sensitive matching.
        #[serde(default)]
        case_sensitive: bool,
        /// Match whole words only.
        #[serde(default)]
        whole_word: bool,
        /// `^`/`$` match at line boundaries.
        #[serde(default = "default_true")]
        multiline: bool,
        /// Stop after this many replacements (0 = all).
        #[serde(default)]
        max_replacements: usize,
    },
    /// Line-oriented operations applied in order.
    Lines {
        /// Input text.
        text: String,
        /// Operations pipeline.
        operations: Vec<transform::LineOp>,
    },
    /// Join lines with a separator.
    Join {
        /// Input text.
        text: String,
        /// Separator inserted between lines.
        #[serde(default = "default_comma")]
        separator: String,
        /// Skip empty lines.
        #[serde(default = "default_true")]
        skip_empty: bool,
        /// Trim each line first.
        #[serde(default = "default_true")]
        trim: bool,
        /// Wrap each line in this string (e.g. `"`).
        #[serde(default)]
        wrap_each: String,
    },
    /// Split text by delimiter, regex or fixed chunk size.
    Split {
        /// Input text.
        text: String,
        /// Literal delimiter.
        #[serde(default)]
        delimiter: Option<String>,
        /// Regular expression delimiter.
        #[serde(default)]
        regex: Option<String>,
        /// Fixed chunk length in characters.
        #[serde(default)]
        chunk_chars: Option<usize>,
        /// Trim each piece.
        #[serde(default = "default_true")]
        trim: bool,
        /// Drop empty pieces.
        #[serde(default)]
        skip_empty: bool,
    },
    /// Line/word/character diff between two texts.
    Diff {
        /// Left-hand text.
        a: String,
        /// Right-hand text.
        b: String,
        /// `lines`, `words` or `chars`.
        #[serde(default)]
        mode: String,
        /// Context lines for unified output.
        #[serde(default = "default_context")]
        context: usize,
        /// Ignore whitespace differences.
        #[serde(default)]
        ignore_whitespace: bool,
        /// Ignore case differences.
        #[serde(default)]
        ignore_case: bool,
    },

    // ----- encodings ------------------------------------------------------
    /// Text → numeric code points (binary, octal, decimal, hex).
    EncodeCodes {
        /// Input text.
        text: String,
        /// Numeric base (2, 8, 10, 16).
        #[serde(default = "default_base")]
        base: u32,
        /// `utf8` bytes or `codepoint` values.
        #[serde(default)]
        unit: String,
        /// Separator between codes.
        #[serde(default = "default_sep")]
        separator: String,
        /// Zero-pad codes to a fixed width.
        #[serde(default = "default_true")]
        pad: bool,
        /// Prefix per code (e.g. `0x`).
        #[serde(default)]
        prefix: String,
    },
    /// Numeric codes → text.
    DecodeCodes {
        /// Code list.
        codes: String,
        /// Numeric base (2, 8, 10, 16).
        #[serde(default = "default_base")]
        base: u32,
        /// `utf8` bytes or `codepoint` values.
        #[serde(default)]
        unit: String,
    },
    /// Caesar/ROT-N and ROT47 rotation.
    Rot {
        /// Input text.
        text: String,
        /// Rotation amount (13 = ROT13).
        #[serde(default = "default_13")]
        shift: i32,
        /// Also rotate digits (ROT5).
        #[serde(default)]
        rotate_digits: bool,
        /// Use ROT47 over printable ASCII.
        #[serde(default)]
        rot47: bool,
    },
    /// Text → Morse.
    MorseEncode {
        /// Input text.
        text: String,
        /// Dot symbol.
        #[serde(default = "default_dot")]
        dot: String,
        /// Dash symbol.
        #[serde(default = "default_dash")]
        dash: String,
        /// Separator between letters.
        #[serde(default = "default_sep")]
        letter_separator: String,
        /// Separator between words.
        #[serde(default = "default_word_sep")]
        word_separator: String,
    },
    /// Morse → text.
    MorseDecode {
        /// Morse code input.
        code: String,
        /// Dot symbol.
        #[serde(default = "default_dot")]
        dot: String,
        /// Dash symbol.
        #[serde(default = "default_dash")]
        dash: String,
        /// Separator between letters.
        #[serde(default = "default_sep")]
        letter_separator: String,
        /// Separator between words.
        #[serde(default = "default_word_sep")]
        word_separator: String,
    },
    /// Per-character Unicode report (code point, name block, category, UTF-8/16 bytes).
    UnicodeInspect {
        /// Input text.
        text: String,
        /// Maximum characters reported.
        #[serde(default)]
        limit: usize,
    },
    /// Unicode normalization.
    UnicodeNormalize {
        /// Input text.
        text: String,
        /// `NFC`, `NFD`, `NFKC` or `NFKD`.
        #[serde(default)]
        form: String,
        /// Remove combining accents after decomposition.
        #[serde(default)]
        strip_accents: bool,
    },
    /// Escape non-ASCII as `\uXXXX` / `&#x…;` / etc.
    UnicodeEscape {
        /// Input text.
        text: String,
        /// Escape style (`js`, `json`, `css`, `html`, `python`, `rust`, `unicode`).
        #[serde(default)]
        style: String,
        /// Escape every non-ASCII character (otherwise only control/ambiguous ones).
        #[serde(default = "default_true")]
        ascii_only: bool,
    },
    /// Unescape `\uXXXX`, `\u{…}`, `\xHH`, `&#…;` and `U+XXXX` sequences.
    UnicodeUnescape {
        /// Input text.
        text: String,
    },
    /// Extract emails, URLs, numbers, hashtags, dates… or a custom pattern.
    Extract {
        /// Input text.
        text: String,
        /// Extraction kind (`emails`, `urls`, `numbers`, `hashtags`, `mentions`, `phones`, `ipv4`,
        /// `dates`, `words`, `capitalized`, `custom`).
        kind: String,
        /// Regex used when `kind` is `custom`.
        #[serde(default)]
        pattern: Option<String>,
        /// Return unique matches only.
        #[serde(default = "default_true")]
        unique: bool,
    },

    // ----- markdown / html ------------------------------------------------
    /// Markdown → HTML (pulldown-cmark).
    MarkdownToHtml {
        /// Markdown source.
        markdown: String,
        /// Strip unsafe HTML/scripts from the output.
        #[serde(default = "default_true")]
        sanitize: bool,
        /// Add `id` attributes to headings.
        #[serde(default = "default_true")]
        heading_ids: bool,
        /// Enable GFM tables.
        #[serde(default = "default_true")]
        tables: bool,
        /// Enable footnotes.
        #[serde(default = "default_true")]
        footnotes: bool,
        /// Enable `~~strikethrough~~`.
        #[serde(default = "default_true")]
        strikethrough: bool,
        /// Enable task list checkboxes.
        #[serde(default = "default_true")]
        tasklists: bool,
        /// Convert quotes/dashes to typographic forms.
        #[serde(default)]
        smart_punctuation: bool,
        /// Wrap the fragment in a full HTML document.
        #[serde(default)]
        wrap_document: bool,
    },
    /// Markdown → structural JSON (headings, links, images, code blocks…).
    MarkdownParse {
        /// Markdown source.
        markdown: String,
    },
    /// HTML → Markdown.
    HtmlToMarkdown {
        /// HTML source.
        html: String,
    },
    /// HTML → plain text.
    HtmlToText {
        /// HTML source.
        html: String,
    },

    // ----- generators -----------------------------------------------------
    /// Lorem-ipsum generator.
    Lorem {
        /// Number of units to generate.
        #[serde(default = "default_count")]
        count: usize,
        /// `words`, `sentences` or `paragraphs`.
        #[serde(default)]
        unit: String,
        /// Begin with the classic "Lorem ipsum dolor sit amet".
        #[serde(default = "default_true")]
        start_with_lorem: bool,
        /// `text`, `html` or `markdown` output.
        #[serde(default)]
        format: String,
        /// Shortest sentence length.
        #[serde(default = "default_min_sentence")]
        min_sentence_words: usize,
        /// Longest sentence length.
        #[serde(default = "default_max_sentence")]
        max_sentence_words: usize,
    },
    /// Random words/sentences/strings (needs caller entropy).
    RandomText {
        /// `words` (default), `sentences`, `paragraphs`, `letters`, `alphanumeric`, `digits`,
        /// `hex`,
        /// `lines` or `custom`.
        #[serde(default)]
        kind: String,
        /// Number of items.
        #[serde(default = "default_top")]
        count: usize,
        /// Length of each generated string.
        #[serde(default)]
        length: usize,
        /// Characters used for `custom` strings (e.g. `abc123`).
        #[serde(default)]
        charset: Option<String>,
        /// Capitalise the first letter.
        #[serde(default)]
        capitalize: bool,
    },
    /// Prepares text for text-to-speech (strips markup, expands abbreviations, chunks).
    TtsPrep {
        /// Input text.
        text: String,
        /// Maximum characters per chunk.
        #[serde(default)]
        max_chars: usize,
        /// Expand common abbreviations (e.g. `Dr.` → `Doctor`).
        #[serde(default = "default_true")]
        expand_abbreviations: bool,
    },
}

fn default_min_sentence() -> usize {
    6
}
fn default_max_sentence() -> usize {
    14
}

fn check_len(text: &str) -> EngineResult<()> {
    limits::check_size(text.len(), limits::MAX_TEXT_CHARS, "text")
}

/// Dispatches a [`TextAction`] to its implementation.
pub fn handle_action(action: TextAction) -> EngineResult<Value> {
    match action {
        // legacy
        TextAction::ConvertCase { text, target_case } => {
            Ok(json!({ "result": transform::convert_case(&text, &target_case)? }))
        },
        TextAction::Analyze { text } => Ok(serde_json::to_value(analysis::legacy_analyze(&text))?),
        TextAction::Reverse { text } => {
            Ok(json!({ "result": text.chars().rev().collect::<String>() }))
        },
        TextAction::Clean {
            text,
            trim_lines,
            remove_empty,
            dedup_lines,
            sort_lines,
        } => {
            let mut lines: Vec<String> = text
                .lines()
                .map(|l| {
                    if trim_lines {
                        l.trim().to_string()
                    } else {
                        l.to_string()
                    }
                })
                .filter(|l| !remove_empty || !l.is_empty())
                .collect();
            if dedup_lines {
                let mut seen = std::collections::HashSet::new();
                lines.retain(|l| seen.insert(l.clone()));
            }
            if sort_lines {
                lines.sort();
            }
            Ok(json!({ "result": lines.join("\n") }))
        },
        TextAction::FormatJson { text, indent } => {
            let val: Value = serde_json::from_str(&text)
                .map_err(|e| EngineError::invalid(format!("Invalid JSON: {e}")))?;
            let formatted = if indent == 0 {
                serde_json::to_string(&val)?
            } else {
                serde_json::to_string_pretty(&val)?
            };
            Ok(json!({ "result": formatted }))
        },

        TextAction::Stats {
            text,
            reading_wpm,
            speaking_wpm,
            top_words,
        } => {
            check_len(&text)?;
            analysis::stats(&text, reading_wpm, speaking_wpm, top_words)
        },
        TextAction::KeywordDensity { text, top, min_length, stop_words } => {
            check_len(&text)?;
            analysis::keyword_density(&text, top, min_length, &stop_words)
        },
        TextAction::Hashtags { text, count } => analysis::hashtags(&text, count),

        TextAction::Case { text, target } => {
            check_len(&text)?;
            Ok(json!({ "result": transform::convert_case(&text, &target)?, "target": target }))
        },
        TextAction::Slug { text, separator, lowercase, max_length } => {
            transform::slug(&text, &separator, lowercase, max_length)
        },
        TextAction::ReverseText { text, mode } => {
            if text.is_empty() {
                return Err(EngineError::empty("text"));
            }
            Ok(json!({ "result": transform::reverse(&text, &mode)? }))
        },
        TextAction::Truncate {
            text,
            max_chars,
            max_words,
            suffix,
            preserve_words,
        } => transform::truncate(&text, max_chars, max_words, &suffix, preserve_words),
        TextAction::Cleanup { text, options } => {
            check_len(&text)?;
            transform::cleanup(&text, &options)
        },
        TextAction::Wrap { text, width, mode, indent } => {
            transform::wrap(&text, width, &mode, &indent)
        },
        TextAction::FindReplace {
            text,
            find,
            replace,
            regex,
            case_sensitive,
            whole_word,
            multiline,
            max_replacements,
        } => {
            check_len(&text)?;
            transform::find_replace(
                &text,
                &find,
                &replace,
                regex,
                case_sensitive,
                whole_word,
                multiline,
                max_replacements,
            )
        },
        TextAction::Lines { text, operations } => {
            check_len(&text)?;
            transform::lines(&text, &operations)
        },
        TextAction::Join {
            text,
            separator,
            skip_empty,
            trim,
            wrap_each,
        } => Ok(transform::join(&text, &separator, skip_empty, trim, &wrap_each)),
        TextAction::Split {
            text,
            delimiter,
            regex,
            chunk_chars,
            trim,
            skip_empty,
        } => transform::split(
            &text,
            delimiter.as_deref(),
            regex.as_deref(),
            chunk_chars,
            trim,
            skip_empty,
        ),
        TextAction::Diff {
            a,
            b,
            mode,
            context,
            ignore_whitespace,
            ignore_case,
        } => {
            check_len(&a)?;
            check_len(&b)?;
            generate::diff(&a, &b, &mode, context, ignore_whitespace, ignore_case)
        },

        TextAction::EncodeCodes { text, base, unit, separator, pad, prefix } => {
            encoding::encode_codes(&text, base, &unit, &separator, pad, &prefix)
        },
        TextAction::DecodeCodes { codes, base, unit } => {
            encoding::decode_codes(&codes, base, &unit)
        },
        TextAction::Rot { text, shift, rotate_digits, rot47 } => {
            encoding::rot(&text, shift, rotate_digits, rot47)
        },
        TextAction::MorseEncode {
            text,
            dot,
            dash,
            letter_separator,
            word_separator,
        } => encoding::morse_encode(&text, &dot, &dash, &letter_separator, &word_separator),
        TextAction::MorseDecode {
            code,
            dot,
            dash,
            letter_separator,
            word_separator,
        } => encoding::morse_decode(&code, &dot, &dash, &letter_separator, &word_separator),
        TextAction::UnicodeInspect { text, limit } => encoding::unicode_inspect(&text, limit),
        TextAction::UnicodeNormalize { text, form, strip_accents } => {
            encoding::unicode_normalize(&text, &form, strip_accents)
        },
        TextAction::UnicodeEscape { text, style, ascii_only } => {
            encoding::unicode_escape(&text, &style, ascii_only)
        },
        TextAction::UnicodeUnescape { text } => encoding::unicode_unescape(&text),
        TextAction::Extract { text, kind, pattern, unique } => {
            check_len(&text)?;
            encoding::extract(&text, &kind, pattern.as_deref(), unique)
        },

        TextAction::MarkdownToHtml {
            markdown,
            sanitize,
            heading_ids,
            tables,
            footnotes,
            strikethrough,
            tasklists,
            smart_punctuation,
            wrap_document,
        } => {
            check_len(&markdown)?;
            markdown::to_html(
                &markdown,
                sanitize,
                heading_ids,
                tables,
                footnotes,
                strikethrough,
                tasklists,
                smart_punctuation,
                wrap_document,
            )
        },
        TextAction::MarkdownParse { markdown } => {
            check_len(&markdown)?;
            markdown::parse(&markdown)
        },
        TextAction::HtmlToMarkdown { html } => {
            check_len(&html)?;
            html::to_markdown(&html)
        },
        TextAction::HtmlToText { html } => {
            check_len(&html)?;
            html::to_text(&html)
        },

        TextAction::Lorem {
            count,
            unit,
            start_with_lorem,
            format,
            min_sentence_words,
            max_sentence_words,
        } => generate::lorem(
            count,
            &unit,
            start_with_lorem,
            &format,
            min_sentence_words,
            max_sentence_words,
        ),
        TextAction::RandomText { kind, count, length, charset, capitalize } => {
            generate::random_text(&kind, count, length, charset.as_deref(), capitalize)
        },
        TextAction::TtsPrep { text, max_chars, expand_abbreviations } => {
            generate::tts_prep(&text, max_chars, expand_abbreviations)
        },
    }
}

engine_shared::export_engine!(handle_action, TextAction);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_actions_unchanged() {
        let r = handle_action(TextAction::ConvertCase {
            text: "Universal Utility Platform".into(),
            target_case: "kebab".into(),
        })
        .unwrap();
        assert_eq!(r["result"], "universal-utility-platform");
        let a = handle_action(TextAction::Analyze {
            text: "Hello world. This is a fast Rust WebAssembly engine.".into(),
        })
        .unwrap();
        assert_eq!(a["words"], 9);
        let f = handle_action(TextAction::FormatJson {
            text: r#"{"b":2,"a":1}"#.into(),
            indent: 2,
        })
        .unwrap();
        assert!(f["result"].as_str().unwrap().contains('\n'));
        assert!(handle_action(TextAction::FormatJson { text: "{bad".into(), indent: 2 }).is_err());
        let c = handle_action(TextAction::Clean {
            text: " b \n\n a \n b ".into(),
            trim_lines: true,
            remove_empty: true,
            dedup_lines: true,
            sort_lines: true,
        })
        .unwrap();
        assert_eq!(c["result"], "a\nb");
        assert_eq!(
            handle_action(TextAction::Reverse { text: "abc".into() }).unwrap()["result"],
            "cba"
        );
    }
}
