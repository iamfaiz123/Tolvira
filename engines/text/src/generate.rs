//! Generators (lorem ipsum, random text) and diffing / speech preparation.

use engine_shared::{rng, EngineError, EngineResult};
use serde_json::{json, Value};
use similar::{ChangeTag, TextDiff};

const LOREM_WORDS: &[&str] = &[
    "lorem",
    "ipsum",
    "dolor",
    "sit",
    "amet",
    "consectetur",
    "adipiscing",
    "elit",
    "sed",
    "do",
    "eiusmod",
    "tempor",
    "incididunt",
    "ut",
    "labore",
    "et",
    "dolore",
    "magna",
    "aliqua",
    "enim",
    "ad",
    "minim",
    "veniam",
    "quis",
    "nostrud",
    "exercitation",
    "ullamco",
    "laboris",
    "nisi",
    "aliquip",
    "ex",
    "ea",
    "commodo",
    "consequat",
    "duis",
    "aute",
    "irure",
    "in",
    "reprehenderit",
    "voluptate",
    "velit",
    "esse",
    "cillum",
    "fugiat",
    "nulla",
    "pariatur",
    "excepteur",
    "sint",
    "occaecat",
    "cupidatat",
    "non",
    "proident",
    "sunt",
    "culpa",
    "qui",
    "officia",
    "deserunt",
    "mollit",
    "anim",
    "id",
    "est",
    "laborum",
    "at",
    "vero",
    "eos",
    "accusamus",
    "iusto",
    "odio",
    "dignissimos",
    "ducimus",
    "blanditiis",
    "praesentium",
    "voluptatum",
    "deleniti",
    "atque",
    "corrupti",
    "quos",
    "dolores",
    "quas",
    "molestias",
    "recusandae",
    "itaque",
    "earum",
    "rerum",
    "hic",
    "tenetur",
    "sapiente",
    "delectus",
    "reiciendis",
    "voluptatibus",
    "maiores",
    "alias",
    "perferendis",
    "doloribus",
    "asperiores",
    "repellat",
];

const ENGLISH_WORDS: &[&str] = &[
    "the", "quick", "brown", "fox", "jumps", "over", "lazy", "dog", "river", "mountain", "light",
    "shadow", "window", "garden", "silver", "paper", "coffee", "morning", "evening", "music",
    "silent", "bridge", "forest", "ocean", "stone", "cloud", "engine", "rocket", "planet",
    "signal", "pattern", "memory", "future", "simple", "complex", "bright", "gentle", "rapid",
    "quiet", "golden", "market", "village", "journey", "letter", "number", "circle", "square",
    "winter", "summer", "autumn", "spring", "castle", "harbor", "meadow", "orchard", "thunder",
    "whisper", "compass", "lantern", "ribbon", "velvet", "marble", "copper", "cotton", "canvas",
    "pixel", "vector", "matrix", "cipher", "beacon", "anchor", "feather", "crystal", "ember",
    "glacier", "horizon", "island", "jungle", "kernel", "lattice", "nebula", "oracle", "prism",
    "quartz", "ridge", "summit", "timber", "umbrella", "valley", "willow", "zenith",
];

fn sentence(words: &[&str], n: usize, capital: bool) -> String {
    let mut s: Vec<String> = (0..n)
        .map(|_| rng::choose(words).map(|w| w.to_string()).unwrap_or_default())
        .collect();
    if capital {
        if let Some(first) = s.first_mut() {
            let mut c = first.chars();
            if let Some(f) = c.next() {
                *first = f.to_uppercase().collect::<String>() + c.as_str();
            }
        }
    }
    // Sprinkle a comma in longer sentences.
    if n > 8 {
        let at = n / 2;
        s[at].push(',');
    }
    format!("{}.", s.join(" "))
}

/// Lorem ipsum generator. `unit`: paragraphs (default), sentences, words.
pub fn lorem(
    count: usize,
    unit: &str,
    start_with_lorem: bool,
    format: &str,
    min_sentence: usize,
    max_sentence: usize,
) -> EngineResult<Value> {
    if count == 0 || count > 500 {
        return Err(EngineError::invalid("`count` must be between 1 and 500"));
    }
    let (lo, hi) = (
        min_sentence.clamp(3, 30),
        max_sentence.clamp(3, 40).max(min_sentence.clamp(3, 30)),
    );
    let classic = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor \
                   incididunt ut labore et dolore magna aliqua.";
    let mut paragraphs: Vec<String> = Vec::new();
    match unit {
        "words" | "word" => {
            let mut ws: Vec<String> = Vec::new();
            if start_with_lorem {
                ws.extend(
                    ["lorem", "ipsum", "dolor", "sit", "amet"]
                        .iter()
                        .take(count)
                        .map(|s| s.to_string()),
                );
            }
            while ws.len() < count {
                ws.push(rng::choose(LOREM_WORDS).copied().unwrap_or("lorem").to_string());
            }
            let mut s = ws.join(" ");
            if let Some(f) = s.get(..1) {
                s = f.to_uppercase() + &s[1..];
            }
            paragraphs.push(s);
        },
        "sentences" | "sentence" => {
            let mut ss = Vec::new();
            for i in 0..count {
                if i == 0 && start_with_lorem {
                    ss.push(classic.to_string());
                } else {
                    ss.push(sentence(
                        LOREM_WORDS,
                        rng::range_inclusive(lo as i64, hi as i64) as usize,
                        true,
                    ));
                }
            }
            paragraphs.push(ss.join(" "));
        },
        "" | "paragraphs" | "paragraph" => {
            for i in 0..count {
                let n = rng::range_inclusive(4, 8) as usize;
                let mut ss = Vec::new();
                for j in 0..n {
                    if i == 0 && j == 0 && start_with_lorem {
                        ss.push(classic.to_string());
                    } else {
                        ss.push(sentence(
                            LOREM_WORDS,
                            rng::range_inclusive(lo as i64, hi as i64) as usize,
                            true,
                        ));
                    }
                }
                paragraphs.push(ss.join(" "));
            }
        },
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown unit `{other}` (paragraphs, sentences, words)"
            )))
        },
    }
    let text = match format {
        "html" => paragraphs
            .iter()
            .map(|p| format!("<p>{p}</p>"))
            .collect::<Vec<_>>()
            .join("\n"),
        "markdown" => paragraphs.join("\n\n"),
        _ => paragraphs.join("\n\n"),
    };
    let words = text.split_whitespace().count();
    Ok(json!({
        "result": text,
        "paragraphs": paragraphs.len(),
        "words": words,
        "characters": text.chars().count()
    }))
}

/// Random text of a given kind: words, sentences, paragraphs, letters,
/// alphanumeric, digits, hex, or a custom character set.
pub fn random_text(
    kind: &str,
    count: usize,
    length: usize,
    charset: Option<&str>,
    capitalize: bool,
) -> EngineResult<Value> {
    if count == 0 || count > 10_000 {
        return Err(EngineError::invalid("`count` must be between 1 and 10000"));
    }
    let result = match kind {
        "" | "words" => {
            let mut ws: Vec<String> = (0..count)
                .map(|_| rng::choose(ENGLISH_WORDS).copied().unwrap_or("word").to_string())
                .collect();
            if capitalize {
                ws.iter_mut().for_each(|w| {
                    let mut c = w.chars();
                    if let Some(f) = c.next() {
                        *w = f.to_uppercase().collect::<String>() + c.as_str();
                    }
                });
            }
            ws.join(" ")
        },
        "sentences" => (0..count)
            .map(|_| sentence(ENGLISH_WORDS, rng::range_inclusive(6, 14) as usize, true))
            .collect::<Vec<_>>()
            .join(" "),
        "paragraphs" => (0..count)
            .map(|_| {
                (0..rng::range_inclusive(3, 6))
                    .map(|_| sentence(ENGLISH_WORDS, rng::range_inclusive(6, 14) as usize, true))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
            .join("\n\n"),
        "letters" | "alphanumeric" | "digits" | "hex" | "custom" | "lines" => {
            let set: Vec<char> = match kind {
                "letters" => ('a'..='z').chain('A'..='Z').collect(),
                "alphanumeric" => ('a'..='z').chain('A'..='Z').chain('0'..='9').collect(),
                "digits" => ('0'..='9').collect(),
                "hex" => "0123456789abcdef".chars().collect(),
                _ => charset
                    .filter(|c| !c.is_empty())
                    .ok_or_else(|| {
                        EngineError::invalid("`charset` is required for custom random text")
                    })?
                    .chars()
                    .collect(),
            };
            let len = if length == 0 { 16 } else { length.min(100_000) };
            let items: Vec<String> = (0..count)
                .map(|_| (0..len).filter_map(|_| rng::choose(&set).copied()).collect())
                .collect();
            items.join("\n")
        },
        other => return Err(EngineError::invalid(format!("Unknown kind `{other}`"))),
    };
    Ok(json!({ "result": result, "count": count, "secure": rng::has_caller_entropy() }))
}

/// Text diff (lines, words or characters) with unified output and stats.
pub fn diff(
    a: &str,
    b: &str,
    mode: &str,
    context: usize,
    ignore_whitespace: bool,
    ignore_case: bool,
) -> EngineResult<Value> {
    if a.is_empty() && b.is_empty() {
        return Err(EngineError::empty("both texts"));
    }
    let norm = |s: &str| -> String {
        let mut t = s.to_string();
        if ignore_case {
            t = t.to_lowercase();
        }
        if ignore_whitespace {
            t = t
                .lines()
                .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
                .collect::<Vec<_>>()
                .join("\n");
        }
        t
    };
    let (na, nb) = (norm(a), norm(b));
    let text_diff = match mode {
        "" | "lines" => TextDiff::from_lines(&na, &nb),
        "words" => TextDiff::from_words(&na, &nb),
        "chars" | "characters" => TextDiff::from_chars(&na, &nb),
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown diff mode `{other}` (lines, words, chars)"
            )))
        },
    };
    let unified = if matches!(mode, "" | "lines") {
        text_diff
            .unified_diff()
            .context_radius(context.min(50))
            .header("a", "b")
            .to_string()
    } else {
        String::new()
    };
    let mut ops = Vec::new();
    let (mut added, mut removed, mut equal) = (0usize, 0usize, 0usize);
    for change in text_diff.iter_all_changes() {
        let (tag, sym) = match change.tag() {
            ChangeTag::Equal => {
                equal += 1;
                ("equal", " ")
            },
            ChangeTag::Delete => {
                removed += 1;
                ("delete", "-")
            },
            ChangeTag::Insert => {
                added += 1;
                ("insert", "+")
            },
        };
        if ops.len() < 20_000 {
            ops.push(json!({
                "op": tag,
                "symbol": sym,
                "value": change.value(),
                "old_index": change.old_index(),
                "new_index": change.new_index()
            }));
        }
    }
    let html: String = text_diff
        .iter_all_changes()
        .take(20_000)
        .map(|c| {
            let v = crate::markdown::escape_html(c.value());
            match c.tag() {
                ChangeTag::Equal => v,
                ChangeTag::Delete => format!("<del>{v}</del>"),
                ChangeTag::Insert => format!("<ins>{v}</ins>"),
            }
        })
        .collect();
    Ok(json!({
        "identical": added == 0 && removed == 0,
        "similarity": (text_diff.ratio() as f64 * 10000.0).round() / 100.0,
        "added": added,
        "removed": removed,
        "unchanged": equal,
        "changes": ops,
        "unified": unified,
        "html": html,
        "mode": if mode.is_empty() { "lines" } else { mode },
    }))
}

/// Prepares text for a text-to-speech engine: strips markup, expands common
/// abbreviations and symbols, and splits into chunks of at most `max_chars`.
pub fn tts_prep(text: &str, max_chars: usize, expand_abbreviations: bool) -> EngineResult<Value> {
    engine_shared::error::require_text(text, "text")?;
    let max = if max_chars == 0 { 500 } else { max_chars.clamp(50, 5000) };
    let mut t = crate::html::to_text(text)
        .map(|v| v["text"].as_str().unwrap_or("").to_string())
        .unwrap_or_else(|_| text.to_string());
    // Markdown leftovers.
    let re_md =
        crate::re(r"(?m)^#{1,6}\s+|[*_`~]{1,3}|\[([^\]]+)\]\([^)]+\)|^\s*[-*+]\s+|^\s*>\s?")?;
    t = re_md.replace_all(&t, "$1").into_owned();
    let re_url = crate::re(r"(?i)\bhttps?://\S+")?;
    t = re_url.replace_all(&t, "link").into_owned();
    if expand_abbreviations {
        let table: &[(&str, &str)] = &[
            ("Dr.", "Doctor"),
            ("Mr.", "Mister"),
            ("Mrs.", "Missus"),
            ("Ms.", "Miz"),
            ("Prof.", "Professor"),
            ("St.", "Saint"),
            ("vs.", "versus"),
            ("e.g.", "for example"),
            ("i.e.", "that is"),
            ("etc.", "et cetera"),
            ("approx.", "approximately"),
            ("No.", "number"),
            ("Jan.", "January"),
            ("Feb.", "February"),
            ("Dec.", "December"),
            ("Inc.", "Incorporated"),
            ("Ltd.", "Limited"),
            ("&", "and"),
            ("%", " percent"),
            ("@", " at "),
            ("+", " plus "),
            ("=", " equals "),
            ("#", " number "),
            ("$", " dollars "),
            ("€", " euros "),
            ("£", " pounds "),
            ("°", " degrees"),
            ("…", "..."),
        ];
        for (k, v) in table {
            t = t.replace(k, v);
        }
    }
    let re_ws = crate::re(r"[ \t]+")?;
    t = re_ws.replace_all(&t, " ").into_owned();
    // Chunk by sentences.
    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();
    if t.trim().is_empty() {
        return Err(EngineError::invalid("Nothing is left after stripping markup"));
    }
    for sentence in unicode_segmentation::UnicodeSegmentation::unicode_sentences(t.as_str()) {
        let s = sentence.trim();
        if s.is_empty() {
            continue;
        }
        if current.chars().count() + s.chars().count() + 1 > max && !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
        }
        if s.chars().count() > max {
            // Hard split very long sentences at word boundaries.
            let mut piece = String::new();
            for w in s.split(' ') {
                if piece.chars().count() + w.chars().count() + 1 > max && !piece.is_empty() {
                    chunks.push(std::mem::take(&mut piece));
                }
                if !piece.is_empty() {
                    piece.push(' ');
                }
                piece.push_str(w);
            }
            if !piece.is_empty() {
                chunks.push(piece);
            }
            continue;
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(s);
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    let words: usize = chunks.iter().map(|c| c.split_whitespace().count()).sum();
    Ok(json!({
        "text": chunks.join("\n"),
        "chunks": chunks,
        "chunk_count": chunks.len(),
        "words": words,
        "estimated_seconds": (words as f64 / 150.0 * 60.0).round()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generators() {
        let l = lorem(3, "paragraphs", true, "text", 5, 10).unwrap();
        assert_eq!(l["paragraphs"], 3);
        assert!(l["result"].as_str().unwrap().starts_with("Lorem ipsum dolor"));
        let w = lorem(7, "words", false, "text", 5, 10).unwrap();
        assert_eq!(w["result"].as_str().unwrap().split(' ').count(), 7);
        assert!(lorem(3, "html", true, "html", 5, 10).is_err());
        assert!(lorem(0, "words", true, "text", 5, 10).is_err());
        let r = random_text("hex", 2, 8, None, false).unwrap();
        let lines: Vec<&str> = r["result"].as_str().unwrap().lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].len() == 8 && lines[0].chars().all(|c| c.is_ascii_hexdigit()));
        assert!(random_text("custom", 1, 4, None, false).is_err());
        assert_eq!(random_text("custom", 1, 4, Some("a"), false).unwrap()["result"], "aaaa");
        assert!(random_text("sentences", 2, 0, None, false).unwrap()["result"]
            .as_str()
            .unwrap()
            .contains('.'));
    }

    #[test]
    fn diffs() {
        let d = diff("a\nb\nc", "a\nx\nc", "lines", 1, false, false).unwrap();
        assert_eq!(d["added"], 1);
        assert_eq!(d["removed"], 1);
        assert!(d["unified"].as_str().unwrap().contains("-b"));
        assert_eq!(diff("Hello", "hello", "words", 0, false, true).unwrap()["identical"], true);
        assert_eq!(diff("a  b", "a b", "lines", 0, true, false).unwrap()["identical"], true);
        let c = diff("abc", "abd", "chars", 0, false, false).unwrap();
        assert!(c["html"].as_str().unwrap().contains("<del>c</del><ins>d</ins>"));
        assert!(diff("", "", "lines", 0, false, false).is_err());
        assert!(diff("a", "b", "bogus", 0, false, false).is_err());
    }

    #[test]
    fn tts() {
        let r = tts_prep(
            "# Title\n\nDr. Smith has 50% of **the** shares. See https://x.io now. Another \
             sentence here.",
            40,
            true,
        )
        .unwrap();
        let t = r["text"].as_str().unwrap();
        assert!(
            t.contains("Doctor Smith")
                && t.contains("50 percent")
                && !t.contains("**")
                && t.contains("link")
        );
        assert!(r["chunk_count"].as_u64().unwrap() >= 2);
        assert!(tts_prep(" ", 100, true).is_err());
    }
}
