//! Case conversion, slugs, reversing, truncation, whitespace cleanup,
//! wrapping, find & replace and line pipelines.

use engine_shared::{EngineError, EngineResult};
use regex::{Regex, RegexBuilder};
use serde::Deserialize;
use serde_json::{json, Value};
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

/// Splits an identifier / sentence into lowercase words (handles camelCase,
/// snake_case, kebab-case, ALLCAPS acronyms and punctuation).
pub fn split_words(s: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_alphanumeric() {
            let prev = if i > 0 { chars[i - 1] } else { ' ' };
            let next = chars.get(i + 1).copied().unwrap_or(' ');
            let boundary = !current.is_empty()
                && ((c.is_uppercase() && prev.is_lowercase())
                    || (c.is_uppercase() && prev.is_uppercase() && next.is_lowercase())
                    || (c.is_ascii_digit() != prev.is_ascii_digit() && prev.is_alphanumeric()));
            if boundary {
                words.push(std::mem::take(&mut current));
            }
            current.push(c);
        } else if !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words.into_iter().map(|w| w.to_lowercase()).collect()
}

fn capitalize(w: &str) -> String {
    let mut c = w.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// `Title Case` (first letter of every word upper-cased).
pub fn to_title_case(s: &str) -> String {
    s.split_whitespace().map(capitalize).collect::<Vec<_>>().join(" ")
}

/// `camelCase`.
pub fn to_camel_case(s: &str) -> String {
    let w = split_words(s);
    w.iter()
        .enumerate()
        .map(|(i, x)| if i == 0 { x.clone() } else { capitalize(x) })
        .collect()
}
/// `PascalCase`.
pub fn to_pascal_case(s: &str) -> String {
    split_words(s).iter().map(|w| capitalize(w)).collect()
}
/// `snake_case`.
pub fn to_snake_case(s: &str) -> String {
    split_words(s).join("_")
}
/// `kebab-case`.
pub fn to_kebab_case(s: &str) -> String {
    split_words(s).join("-")
}

/// Smart title case: small words stay lowercase unless first/last.
pub fn to_smart_title(s: &str) -> String {
    const SMALL: &[&str] = &[
        "a", "an", "and", "as", "at", "but", "by", "for", "in", "nor", "of", "on", "or", "the",
        "to", "up", "yet", "so", "via", "vs",
    ];
    let ws: Vec<&str> = s.split_whitespace().collect();
    let n = ws.len();
    ws.iter()
        .enumerate()
        .map(|(i, w)| {
            let lower = w.to_lowercase();
            if i != 0 && i != n - 1 && SMALL.contains(&lower.as_str()) {
                lower
            } else {
                capitalize(&lower)
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `Sentence case` (only the first letter of each sentence upper-cased).
pub fn to_sentence_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut start = true;
    for c in s.chars() {
        if start && c.is_alphabetic() {
            out.extend(c.to_uppercase());
            start = false;
        } else {
            out.extend(c.to_lowercase());
        }
        if matches!(c, '.' | '!' | '?' | '\n') {
            start = true;
        }
    }
    out
}

/// Converts to the requested case. Supported: upper, lower, title,
/// smart-title, sentence, camel, pascal, snake, kebab, constant, dot, path,
/// train, alternating, inverse, capitalize.
pub fn convert_case(text: &str, target: &str) -> EngineResult<String> {
    Ok(match target.trim().to_ascii_lowercase().replace('_', "-").as_str() {
        "upper" | "uppercase" => text.to_uppercase(),
        "lower" | "lowercase" => text.to_lowercase(),
        "title" | "capitalize-words" => to_title_case(text),
        "smart-title" | "headline" => to_smart_title(text),
        "sentence" => to_sentence_case(text),
        "camel" => to_camel_case(text),
        "pascal" => to_pascal_case(text),
        "snake" => to_snake_case(text),
        "kebab" => to_kebab_case(text),
        "constant" | "screaming-snake" => to_snake_case(text).to_uppercase(),
        "dot" => split_words(text).join("."),
        "path" => split_words(text).join("/"),
        "train" => split_words(text)
            .iter()
            .map(|w| capitalize(w))
            .collect::<Vec<_>>()
            .join("-"),
        "alternating" | "sponge" => text
            .chars()
            .filter(|_| true)
            .enumerate()
            .map(|(i, c)| {
                if i % 2 == 0 {
                    c.to_lowercase().to_string()
                } else {
                    c.to_uppercase().to_string()
                }
            })
            .collect(),
        "inverse" | "toggle" => text
            .chars()
            .map(|c| {
                if c.is_uppercase() {
                    c.to_lowercase().to_string()
                } else {
                    c.to_uppercase().to_string()
                }
            })
            .collect(),
        "capitalize" => capitalize(text),
        other => return Err(EngineError::invalid(format!("Unsupported case: {other}"))),
    })
}

/// Removes diacritics (é → e) via NFD decomposition.
pub fn strip_accents(s: &str) -> String {
    s.nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect()
}

/// URL-friendly slug.
pub fn slug(
    text: &str,
    separator: &str,
    lowercase: bool,
    max_length: usize,
) -> EngineResult<Value> {
    engine_shared::error::require_text(text, "text")?;
    let sep = if separator.is_empty() { "-" } else { separator };
    let base = strip_accents(text);
    let replaced = base
        .replace('&', " and ")
        .replace('@', " at ")
        .replace('ß', "ss")
        .replace('æ', "ae")
        .replace('ø', "o")
        .replace('œ', "oe")
        .replace('đ', "d")
        .replace('ł', "l");
    let mut out = String::new();
    let mut pending_sep = false;
    for c in replaced.chars() {
        if c.is_ascii_alphanumeric() {
            if pending_sep && !out.is_empty() {
                out.push_str(sep);
            }
            pending_sep = false;
            out.push(c);
        } else if c.is_alphanumeric() {
            // Non-Latin letters are kept (percent-encoded by browsers) – transliteration beyond Latin is out of scope.
            if pending_sep && !out.is_empty() {
                out.push_str(sep);
            }
            pending_sep = false;
            out.push(c);
        } else {
            pending_sep = true;
        }
    }
    if lowercase {
        out = out.to_lowercase();
    }
    if max_length > 0 && out.chars().count() > max_length {
        let cut: String = out.chars().take(max_length).collect();
        out = match cut.rfind(sep) {
            Some(i) if i > 0 => cut[..i].to_string(),
            _ => cut,
        };
    }
    Ok(json!({ "slug": out, "length": out.chars().count() }))
}

/// Reverses by `chars`, `words` or `lines`.
pub fn reverse(text: &str, mode: &str) -> EngineResult<String> {
    Ok(match mode {
        "" | "characters" | "chars" => text.graphemes(true).rev().collect(),
        "words" => text.split(' ').rev().collect::<Vec<_>>().join(" "),
        "lines" => text.lines().rev().collect::<Vec<_>>().join("\n"),
        "words-in-place" | "each-word" => text
            .split(' ')
            .map(|w| w.graphemes(true).rev().collect::<String>())
            .collect::<Vec<_>>()
            .join(" "),
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown reverse mode `{other}` (characters, words, lines, each-word)"
            )))
        },
    })
}

/// Truncates by characters and/or words, appending `suffix` when text was cut.
pub fn truncate(
    text: &str,
    max_chars: Option<usize>,
    max_words: Option<usize>,
    suffix: &str,
    preserve_words: bool,
) -> EngineResult<Value> {
    let original_chars = text.chars().count();
    let mut result = text.to_string();
    let mut truncated = false;
    if let Some(mw) = max_words {
        let ws: Vec<&str> = text.split_whitespace().collect();
        if ws.len() > mw {
            result = ws[..mw].join(" ");
            truncated = true;
        }
    }
    if let Some(mc) = max_chars {
        if result.chars().count() > mc {
            let limit = mc.saturating_sub(suffix.chars().count());
            let mut cut: String = result.chars().take(limit).collect();
            if preserve_words {
                if let Some(i) = cut.rfind(char::is_whitespace) {
                    if i > 0 {
                        cut.truncate(i);
                    }
                }
            }
            result = cut.trim_end().to_string();
            truncated = true;
        }
    }
    if max_chars.is_none() && max_words.is_none() {
        return Err(EngineError::invalid("Provide `max_chars` or `max_words`"));
    }
    if truncated {
        result.push_str(suffix);
    }
    Ok(json!({
        "result": result,
        "truncated": truncated,
        "original_characters": original_chars,
        "characters": result.chars().count()
    }))
}

/// Switches for the [`cleanup`] pipeline; every flag defaults to `false`.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct CleanupOptions {
    /// Collapse runs of spaces/tabs into one space.
    pub collapse_spaces: bool,
    /// Collapse three or more newlines into two.
    pub collapse_newlines: bool,
    /// Trim whitespace from every line.
    pub trim_lines: bool,
    /// Trim the whole text.
    pub trim: bool,
    /// Drop empty lines.
    pub remove_empty_lines: bool,
    /// Replace tabs with this many spaces.
    pub tabs_to_spaces: Option<u8>,
    /// Convert CRLF/CR line endings to LF.
    pub normalize_newlines: bool,
    /// Remove zero-width characters.
    pub remove_zero_width: bool,
    /// Remove ASCII control characters.
    pub remove_control: bool,
    /// Replace curly quotes with straight ones.
    pub straight_quotes: bool,
    /// Strip HTML tags.
    pub remove_html: bool,
    /// Remove URLs.
    pub remove_urls: bool,
    /// Remove e-mail addresses.
    pub remove_emails: bool,
    /// Remove digits.
    pub remove_numbers: bool,
    /// Remove punctuation.
    pub remove_punctuation: bool,
    /// Remove emoji.
    pub remove_emoji: bool,
    /// Replace non-breaking spaces with regular spaces.
    pub nbsp_to_space: bool,
    /// Unicode normalization form to apply (`NFC`, `NFD`, `NFKC`, `NFKD`).
    pub unicode_form: Option<String>,
    /// Strip combining accents.
    pub strip_accents: bool,
    /// Remove duplicate lines.
    pub dedupe_lines: bool,
    /// Sort lines.
    pub sort_lines: bool,
}

/// Runs the cleanup pipeline described by `o` and reports what changed.
pub fn cleanup(text: &str, o: &CleanupOptions) -> EngineResult<Value> {
    let mut s = text.to_string();
    let mut applied = Vec::new();
    let mut apply = |name: &str, s: &mut String, f: &dyn Fn(&str) -> String| {
        let n = f(s);
        if n != *s {
            applied.push(name.to_string());
        }
        *s = n;
    };
    if o.normalize_newlines || o.collapse_newlines || o.remove_empty_lines || o.trim_lines {
        apply("normalize_newlines", &mut s, &|t| t.replace("\r\n", "\n").replace('\r', "\n"));
    }
    if o.remove_html {
        let re = crate::re(r"(?s)<script.*?</script>|<style.*?</style>|<[^>]+>")?;
        apply("remove_html", &mut s, &|t| {
            crate::markdown::decode_entities(&re.replace_all(t, ""))
        });
    }
    if o.remove_urls {
        let re = crate::re(r"(?i)\b(?:https?://|www\.)\S+")?;
        apply("remove_urls", &mut s, &|t| re.replace_all(t, "").into_owned());
    }
    if o.remove_emails {
        let re = crate::re(r"[\w.+-]+@[\w-]+\.[\w.-]+")?;
        apply("remove_emails", &mut s, &|t| re.replace_all(t, "").into_owned());
    }
    if o.remove_zero_width {
        apply("remove_zero_width", &mut s, &|t| {
            t.chars()
                .filter(|c| {
                    !matches!(*c, '\u{200B}'..='\u{200F}' | '\u{FEFF}' | '\u{2060}' | '\u{00AD}')
                })
                .collect()
        });
    }
    if o.remove_control {
        apply("remove_control", &mut s, &|t| {
            t.chars()
                .filter(|c| !c.is_control() || matches!(c, '\n' | '\t' | '\r'))
                .collect()
        });
    }
    if o.nbsp_to_space {
        apply("nbsp_to_space", &mut s, &|t| {
            t.replace(['\u{00A0}', '\u{2007}', '\u{202F}'], " ")
        });
    }
    if o.straight_quotes {
        apply("straight_quotes", &mut s, &|t| {
            t.replace(['\u{2018}', '\u{2019}', '\u{201A}', '\u{2032}'], "'")
                .replace(['\u{201C}', '\u{201D}', '\u{201E}', '\u{2033}'], "\"")
                .replace('\u{2013}', "-")
                .replace('\u{2014}', "--")
                .replace('\u{2026}', "...")
        });
    }
    if let Some(form) = &o.unicode_form {
        let f = form.to_uppercase();
        apply("unicode_normalize", &mut s, &|t| match f.as_str() {
            "NFD" => t.nfd().collect(),
            "NFKC" => t.nfkc().collect(),
            "NFKD" => t.nfkd().collect(),
            _ => t.nfc().collect(),
        });
    }
    if o.strip_accents {
        apply("strip_accents", &mut s, &|t| strip_accents(t));
    }
    if o.remove_emoji {
        apply("remove_emoji", &mut s, &|t| t.chars().filter(|c| !is_emoji(*c)).collect());
    }
    if o.remove_numbers {
        apply("remove_numbers", &mut s, &|t| t.chars().filter(|c| !c.is_numeric()).collect());
    }
    if o.remove_punctuation {
        apply("remove_punctuation", &mut s, &|t| {
            t.chars()
                .filter(|c| {
                    !c.is_ascii_punctuation()
                        && !matches!(*c, '\u{2018}'..='\u{201F}' | '«' | '»' | '¿' | '¡')
                })
                .collect()
        });
    }
    if let Some(n) = o.tabs_to_spaces {
        apply("tabs_to_spaces", &mut s, &|t| t.replace('\t', &" ".repeat(n as usize)));
    }
    if o.collapse_spaces {
        let re = crate::re(r"[ \t\u{00A0}]{2,}")?;
        apply("collapse_spaces", &mut s, &|t| re.replace_all(t, " ").into_owned());
    }
    if o.trim_lines {
        apply("trim_lines", &mut s, &|t| {
            t.lines().map(str::trim).collect::<Vec<_>>().join("\n")
        });
    }
    if o.remove_empty_lines {
        apply("remove_empty_lines", &mut s, &|t| {
            t.lines()
                .filter(|l| !l.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        });
    }
    if o.collapse_newlines {
        let re = crate::re(r"\n{3,}")?;
        apply("collapse_newlines", &mut s, &|t| re.replace_all(t, "\n\n").into_owned());
    }
    if o.dedupe_lines {
        apply("dedupe_lines", &mut s, &|t| {
            let mut seen = std::collections::HashSet::new();
            t.lines()
                .filter(|l| seen.insert(l.to_string()))
                .collect::<Vec<_>>()
                .join("\n")
        });
    }
    if o.sort_lines {
        apply("sort_lines", &mut s, &|t| {
            let mut v: Vec<&str> = t.lines().collect();
            v.sort();
            v.join("\n")
        });
    }
    if o.trim {
        apply("trim", &mut s, &|t| t.trim().to_string());
    }
    Ok(json!({
        "result": s,
        "applied": applied,
        "characters_before": text.chars().count(),
        "characters_after": s.chars().count()
    }))
}

/// Whether `c` is (part of) an emoji.
pub fn is_emoji(c: char) -> bool {
    matches!(c as u32, 0x1F300..=0x1FAFF | 0x2600..=0x27BF | 0x1F1E6..=0x1F1FF | 0xFE0F | 0x200D | 0x2B00..=0x2BFF | 0x1F000..=0x1F2FF)
}

/// Word wrapping / unwrapping / alignment.
pub fn wrap(text: &str, width: usize, mode: &str, indent: &str) -> EngineResult<Value> {
    if !(1..=10_000).contains(&width) && mode != "unwrap" {
        return Err(EngineError::invalid("`width` must be between 1 and 10000"));
    }
    let result = match mode {
        "" | "wrap" => {
            let mut out = Vec::new();
            for para in text.split("\n\n") {
                let mut line = String::new();
                for word in para.split_whitespace() {
                    let needed = if line.is_empty() {
                        word.chars().count()
                    } else {
                        line.chars().count() + 1 + word.chars().count()
                    };
                    if needed > width.saturating_sub(indent.chars().count()) && !line.is_empty() {
                        out.push(format!("{indent}{line}"));
                        line = word.to_string();
                    } else {
                        if !line.is_empty() {
                            line.push(' ');
                        }
                        line.push_str(word);
                    }
                }
                if !line.is_empty() {
                    out.push(format!("{indent}{line}"));
                }
                out.push(String::new());
            }
            out.pop();
            out.join("\n")
        },
        "unwrap" => text
            .split("\n\n")
            .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
            .join("\n\n"),
        "center" => text
            .lines()
            .map(|l| {
                let t = l.trim();
                let len = t.chars().count();
                if len >= width {
                    t.to_string()
                } else {
                    format!("{}{}", " ".repeat((width - len) / 2), t)
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        "right" => text
            .lines()
            .map(|l| {
                let t = l.trim();
                let len = t.chars().count();
                if len >= width {
                    t.to_string()
                } else {
                    format!("{}{}", " ".repeat(width - len), t)
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        "justify" => text
            .lines()
            .map(|l| justify_line(l.trim(), width))
            .collect::<Vec<_>>()
            .join("\n"),
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown wrap mode `{other}` (wrap, unwrap, center, right, justify)"
            )))
        },
    };
    Ok(json!({ "result": result, "lines": result.lines().count() }))
}

fn justify_line(line: &str, width: usize) -> String {
    let ws: Vec<&str> = line.split_whitespace().collect();
    if ws.len() < 2 {
        return line.to_string();
    }
    let total: usize = ws.iter().map(|w| w.chars().count()).sum();
    if total >= width {
        return ws.join(" ");
    }
    let gaps = ws.len() - 1;
    let spaces = width - total;
    let mut out = String::new();
    for (i, w) in ws.iter().enumerate() {
        out.push_str(w);
        if i < gaps {
            let extra = spaces / gaps + if i < spaces % gaps { 1 } else { 0 };
            out.push_str(&" ".repeat(extra));
        }
    }
    out
}

/// Builds a regex for find & replace.
pub fn build_regex(
    pattern: &str,
    is_regex: bool,
    case_sensitive: bool,
    whole_word: bool,
    multiline: bool,
) -> EngineResult<Regex> {
    if pattern.is_empty() {
        return Err(EngineError::empty("search pattern"));
    }
    let mut pat = if is_regex {
        pattern.to_string()
    } else {
        regex::escape(pattern)
    };
    if whole_word {
        pat = format!(r"\b(?:{pat})\b");
    }
    RegexBuilder::new(&pat)
        .case_insensitive(!case_sensitive)
        .multi_line(multiline)
        .size_limit(10 * 1024 * 1024)
        .build()
        .map_err(|e| EngineError::invalid(format!("Invalid regular expression: {e}")))
}

/// Literal/regex find & replace with optional whole-word and case-insensitive matching.
pub fn find_replace(
    text: &str,
    find: &str,
    replace: &str,
    is_regex: bool,
    case_sensitive: bool,
    whole_word: bool,
    multiline: bool,
    max_replacements: usize,
) -> EngineResult<Value> {
    let re = build_regex(find, is_regex, case_sensitive, whole_word, multiline)?;
    let count = re.find_iter(text).count();
    let replacement = if is_regex {
        replace.to_string()
    } else {
        replace.replace('$', "$$")
    };
    let result = if max_replacements > 0 {
        re.replacen(text, max_replacements, replacement.as_str())
    } else {
        re.replace_all(text, replacement.as_str())
    };
    let matches: Vec<Value> = re
        .find_iter(text)
        .take(200)
        .map(|m| json!({ "start": m.start(), "end": m.end(), "text": m.as_str() }))
        .collect();
    Ok(json!({
        "result": result.into_owned(),
        "matches": count,
        "replaced": if max_replacements > 0 { count.min(max_replacements) } else { count },
        "positions": matches
    }))
}

/// One step of the `lines` action pipeline.
#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum LineOp {
    /// Sort lines.
    Sort {
        /// Descending order.
        #[serde(default)]
        descending: bool,
        /// Numeric comparison.
        #[serde(default)]
        numeric: bool,
        /// Ignore case.
        #[serde(default)]
        case_insensitive: bool,
        /// Natural ("file2" < "file10") ordering.
        #[serde(default)]
        natural: bool,
        /// Sort by line length.
        #[serde(default)]
        by_length: bool,
    },
    /// Remove duplicate lines, keeping the first occurrence.
    Dedupe {
        /// Ignore case when comparing.
        #[serde(default)]
        case_insensitive: bool,
        /// Trim before comparing.
        #[serde(default)]
        trim: bool,
    },
    /// Drop empty lines.
    RemoveEmpty,
    /// Trim every line.
    Trim,
    /// Shuffle lines (needs caller entropy).
    Shuffle,
    /// Reverse line order.
    Reverse,
    /// Prefix lines with numbers.
    Number {
        /// First number.
        #[serde(default = "one")]
        start: i64,
        /// Number format; `{n}` is replaced by the line number.
        #[serde(default = "default_num_fmt")]
        format: String,
    },
    /// Add a prefix to every line.
    Prefix {
        /// Prefix text.
        text: String,
    },
    /// Add a suffix to every line.
    Suffix {
        /// Suffix text.
        text: String,
    },
    /// Keep only lines matching `contains` or `regex`.
    Filter {
        /// Substring to match.
        #[serde(default)]
        contains: Option<String>,
        /// Regular expression to match.
        #[serde(default)]
        regex: Option<String>,
        /// Keep non-matching lines instead.
        #[serde(default)]
        invert: bool,
        /// Case-sensitive matching.
        #[serde(default)]
        case_sensitive: bool,
    },
    /// Keep only lines that occur exactly once.
    Unique,
    /// Keep only lines that occur more than once (one copy each).
    RemoveDuplicatesOnly,
    /// Cut every line to a maximum length.
    Truncate {
        /// Maximum characters per line.
        length: usize,
    },
    /// Indent every line.
    Indent {
        /// Number of spaces.
        #[serde(default = "two")]
        spaces: usize,
    },
    /// Lower-case every line.
    Lowercase,
    /// Upper-case every line.
    Uppercase,
}
fn one() -> i64 {
    1
}
fn two() -> usize {
    2
}
fn default_num_fmt() -> String {
    "{n}. ".into()
}

fn natural_key(s: &str) -> Vec<(u8, String, u128)> {
    let mut key = Vec::new();
    let mut num = String::new();
    let mut txt = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            if !txt.is_empty() {
                key.push((1, txt.to_lowercase(), 0));
                txt.clear();
            }
            num.push(c);
        } else {
            if !num.is_empty() {
                key.push((0, String::new(), num.parse().unwrap_or(u128::MAX)));
                num.clear();
            }
            txt.push(c);
        }
    }
    if !num.is_empty() {
        key.push((0, String::new(), num.parse().unwrap_or(u128::MAX)));
    }
    if !txt.is_empty() {
        key.push((1, txt.to_lowercase(), 0));
    }
    key
}

fn leading_number(s: &str) -> f64 {
    let t = s.trim();
    let end = t
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '+'))
        .map(|(i, _)| i)
        .unwrap_or(t.len());
    t[..end].parse().unwrap_or(f64::INFINITY)
}

/// Runs a pipeline of line operations.
pub fn lines(text: &str, ops: &[LineOp]) -> EngineResult<Value> {
    if ops.is_empty() {
        return Err(EngineError::empty("operations"));
    }
    let original = text.lines().count();
    let mut v: Vec<String> = text.lines().map(str::to_string).collect();
    for op in ops {
        match op {
            LineOp::Sort {
                descending,
                numeric,
                case_insensitive,
                natural,
                by_length,
            } => {
                if *numeric {
                    v.sort_by(|a, b| {
                        leading_number(a)
                            .partial_cmp(&leading_number(b))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                } else if *natural {
                    v.sort_by_key(|a| natural_key(a));
                } else if *by_length {
                    v.sort_by_key(|a| a.chars().count());
                } else if *case_insensitive {
                    v.sort_by_key(|a| a.to_lowercase());
                } else {
                    v.sort();
                }
                if *descending {
                    v.reverse();
                }
            },
            LineOp::Dedupe { case_insensitive, trim } => {
                let mut seen = std::collections::HashSet::new();
                v.retain(|l| {
                    let mut k = if *trim { l.trim().to_string() } else { l.clone() };
                    if *case_insensitive {
                        k = k.to_lowercase();
                    }
                    seen.insert(k)
                });
            },
            LineOp::Unique => {
                let mut counts = std::collections::HashMap::new();
                for l in &v {
                    *counts.entry(l.clone()).or_insert(0) += 1;
                }
                v.retain(|l| counts[l] == 1);
            },
            LineOp::RemoveDuplicatesOnly => {
                let mut seen = std::collections::HashSet::new();
                v.retain(|l| seen.insert(l.clone()));
            },
            LineOp::RemoveEmpty => v.retain(|l| !l.trim().is_empty()),
            LineOp::Trim => v.iter_mut().for_each(|l| *l = l.trim().to_string()),
            LineOp::Shuffle => engine_shared::rng::shuffle(&mut v),
            LineOp::Reverse => v.reverse(),
            LineOp::Number { start, format } => {
                let width = (start + v.len() as i64).to_string().len();
                for (i, l) in v.iter_mut().enumerate() {
                    let n = start + i as i64;
                    let label = format
                        .replace("{n}", &n.to_string())
                        .replace("{N}", &format!("{n:0width$}"));
                    *l = format!("{label}{l}");
                }
            },
            LineOp::Prefix { text } => v.iter_mut().for_each(|l| *l = format!("{text}{l}")),
            LineOp::Suffix { text } => v.iter_mut().for_each(|l| l.push_str(text)),
            LineOp::Filter { contains, regex, invert, case_sensitive } => {
                let re = match (regex, contains) {
                    (Some(r), _) => build_regex(r, true, *case_sensitive, false, false)?,
                    (None, Some(c)) => build_regex(c, false, *case_sensitive, false, false)?,
                    (None, None) => {
                        return Err(EngineError::invalid("filter needs `contains` or `regex`"))
                    },
                };
                v.retain(|l| re.is_match(l) != *invert);
            },
            LineOp::Truncate { length } => {
                v.iter_mut().for_each(|l| *l = l.chars().take(*length).collect())
            },
            LineOp::Indent { spaces } => {
                v.iter_mut().for_each(|l| *l = format!("{}{l}", " ".repeat(*spaces)))
            },
            LineOp::Lowercase => v.iter_mut().for_each(|l| *l = l.to_lowercase()),
            LineOp::Uppercase => v.iter_mut().for_each(|l| *l = l.to_uppercase()),
        }
    }
    Ok(json!({
        "result": v.join("\n"),
        "lines_before": original,
        "lines_after": v.len(),
        "removed": original.saturating_sub(v.len())
    }))
}

/// Joins lines with a separator (text-joiner).
pub fn join(text: &str, separator: &str, skip_empty: bool, trim: bool, wrap_each: &str) -> Value {
    let parts: Vec<String> = text
        .lines()
        .map(|l| if trim { l.trim() } else { l })
        .filter(|l| !skip_empty || !l.is_empty())
        .map(|l| {
            if wrap_each.is_empty() {
                l.to_string()
            } else {
                format!("{wrap_each}{l}{wrap_each}")
            }
        })
        .collect();
    json!({ "result": parts.join(separator), "items": parts.len() })
}

/// Splits text into lines by a delimiter / regex / fixed length (text-splitter).
pub fn split(
    text: &str,
    delimiter: Option<&str>,
    regex: Option<&str>,
    chunk_chars: Option<usize>,
    trim: bool,
    skip_empty: bool,
) -> EngineResult<Value> {
    let mut parts: Vec<String> = if let Some(n) = chunk_chars {
        if n == 0 {
            return Err(EngineError::invalid("`chunk_chars` must be > 0"));
        }
        text.chars()
            .collect::<Vec<_>>()
            .chunks(n)
            .map(|c| c.iter().collect())
            .collect()
    } else if let Some(r) = regex {
        let re = build_regex(r, true, true, false, false)?;
        re.split(text).map(str::to_string).collect()
    } else {
        let d = delimiter.unwrap_or(",");
        if d.is_empty() {
            text.chars().map(|c| c.to_string()).collect()
        } else {
            text.split(d).map(str::to_string).collect()
        }
    };
    if trim {
        parts.iter_mut().for_each(|p| *p = p.trim().to_string());
    }
    if skip_empty {
        parts.retain(|p| !p.is_empty());
    }
    Ok(json!({ "parts": parts, "count": parts.len(), "result": parts.join("\n") }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cases() {
        assert_eq!(to_camel_case("hello world"), "helloWorld");
        assert_eq!(to_snake_case("HelloWorldHTTPServer2Go"), "hello_world_http_server_2_go");
        assert_eq!(to_kebab_case("hello world"), "hello-world");
        assert_eq!(to_title_case("hello world"), "Hello World");
        assert_eq!(to_pascal_case("hello world"), "HelloWorld");
        assert_eq!(convert_case("hello world", "constant").unwrap(), "HELLO_WORLD");
        assert_eq!(
            convert_case("the lord of the rings", "smart-title").unwrap(),
            "The Lord of the Rings"
        );
        assert_eq!(convert_case("hello. world! ok", "sentence").unwrap(), "Hello. World! Ok");
        assert_eq!(convert_case("ab", "alternating").unwrap(), "aB");
        assert_eq!(convert_case("aB", "inverse").unwrap(), "Ab");
        assert!(convert_case("x", "bogus").is_err());
    }

    #[test]
    fn slugs_reverse_truncate() {
        assert_eq!(slug("Héllo Wörld & Co!", "-", true, 0).unwrap()["slug"], "hello-world-and-co");
        assert_eq!(slug("A very long title indeed", "-", true, 12).unwrap()["slug"], "a-very-long");
        assert!(slug("  ", "-", true, 0).is_err());
        assert_eq!(reverse("héllo", "").unwrap(), "olléh");
        assert_eq!(reverse("a b c", "words").unwrap(), "c b a");
        assert_eq!(reverse("ab cd", "each-word").unwrap(), "ba dc");
        assert!(reverse("x", "zig").is_err());
        let t = truncate("one two three four", Some(10), None, "…", true).unwrap();
        assert_eq!(t["result"], "one two…");
        let t = truncate("one two three", None, Some(2), "", false).unwrap();
        assert_eq!(t["result"], "one two");
        assert!(truncate("x", None, None, "", false).is_err());
    }

    #[test]
    fn cleanup_and_wrap() {
        let o = CleanupOptions {
            collapse_spaces: true,
            trim_lines: true,
            remove_empty_lines: true,
            straight_quotes: true,
            remove_html: true,
            ..Default::default()
        };
        let r = cleanup("  <b>Hello</b>   “world”  \r\n\r\n\n  bye  ", &o).unwrap();
        assert_eq!(r["result"], "Hello \"world\"\nbye");
        let w = wrap("the quick brown fox jumps over the lazy dog", 10, "wrap", "").unwrap();
        assert_eq!(w["result"], "the quick\nbrown fox\njumps over\nthe lazy\ndog");
        assert_eq!(wrap("a\nb", 5, "center", "").unwrap()["result"], "  a\n  b");
        assert_eq!(wrap("a b", 5, "justify", "").unwrap()["result"], "a   b");
        assert!(wrap("x", 0, "wrap", "").is_err());
    }

    #[test]
    fn replace_and_lines() {
        let r = find_replace("Hello hello HELLO", "hello", "bye", false, false, true, false, 0)
            .unwrap();
        assert_eq!(r["matches"], 3);
        assert_eq!(r["result"], "bye bye bye");
        let r = find_replace("a1 b22", r"(\w)(\d+)", "$2$1", true, true, false, false, 0).unwrap();
        assert_eq!(r["result"], "1a 22b");
        assert!(find_replace("x", "(", "", true, true, false, false, 0).is_err());
        assert!(find_replace("x", "", "", false, true, false, false, 0).is_err());
        let ops = vec![
            LineOp::Trim,
            LineOp::RemoveEmpty,
            LineOp::Dedupe { case_insensitive: true, trim: false },
            LineOp::Sort {
                descending: false,
                numeric: false,
                case_insensitive: true,
                natural: false,
                by_length: false,
            },
            LineOp::Number { start: 1, format: "{n}. ".into() },
        ];
        let r = lines(" b \n\na\nB\n", &ops).unwrap();
        assert_eq!(r["result"], "1. a\n2. b");
        let nat = lines(
            "file10\nfile2\nfile1",
            &[LineOp::Sort {
                descending: false,
                numeric: false,
                case_insensitive: false,
                natural: true,
                by_length: false,
            }],
        )
        .unwrap();
        assert_eq!(nat["result"], "file1\nfile2\nfile10");
        let f = lines(
            "apple\nbanana\ncherry",
            &[LineOp::Filter {
                contains: Some("an".into()),
                regex: None,
                invert: false,
                case_sensitive: false,
            }],
        )
        .unwrap();
        assert_eq!(f["result"], "banana");
        assert!(lines("x", &[]).is_err());
        assert_eq!(join("a\n\nb", ", ", true, true, "'")["result"], "'a', 'b'");
        assert_eq!(split("a, b,,c", Some(","), None, None, true, true).unwrap()["count"], 3);
        assert_eq!(split("abcdef", None, None, Some(4), false, false).unwrap()["parts"][1], "ef");
    }
}
