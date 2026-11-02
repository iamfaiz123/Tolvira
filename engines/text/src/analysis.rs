//! Counting, readability and keyword statistics.

use engine_shared::EngineResult;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use unicode_segmentation::UnicodeSegmentation;

/// Legacy v1 response shape (kept for the WordCounter UI).
#[derive(Serialize, serde::Deserialize)]
pub struct TextAnalysis {
    /// Whitespace-separated word count.
    pub words: usize,
    /// Number of Unicode scalar values.
    pub chars: usize,
    /// Character count excluding whitespace.
    pub chars_no_spaces: usize,
    /// Number of lines.
    pub lines: usize,
    /// Sentence count (terminal punctuation heuristic).
    pub sentences: usize,
    /// Estimated reading time at ~200 wpm.
    pub reading_time_seconds: usize,
}

/// Counts sentences by terminal punctuation (`.`, `!`, `?`).
pub fn count_sentences(text: &str) -> usize {
    if text.trim().is_empty() {
        return 0;
    }
    text.unicode_sentences()
        .filter(|s| s.chars().any(|c| c.is_alphanumeric()))
        .count()
}

/// Counts paragraphs separated by blank lines.
pub fn count_paragraphs(text: &str) -> usize {
    text.split('\n')
        .collect::<Vec<_>>()
        .join("\n")
        .split("\n\n")
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .count()
        .max(if text.trim().is_empty() { 0 } else { 1 })
}

/// Splits text into Unicode words (UAX #29).
pub fn words(text: &str) -> Vec<&str> {
    text.unicode_words().collect()
}

/// Original v1 `analyze` action (kept for the WordCounter UI).
pub fn legacy_analyze(text: &str) -> TextAnalysis {
    let words = text.split_whitespace().count();
    TextAnalysis {
        words,
        chars: text.chars().count(),
        chars_no_spaces: text.chars().filter(|c| !c.is_whitespace()).count(),
        lines: if text.is_empty() { 0 } else { text.lines().count() },
        sentences: text.split(['.', '!', '?']).filter(|s| !s.trim().is_empty()).count(),
        reading_time_seconds: if words == 0 { 0 } else { (words * 60 / 225).max(1) },
    }
}

/// Approximate English syllable count.
pub fn syllables(word: &str) -> usize {
    let w: String = word
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .collect();
    if w.is_empty() {
        return 0;
    }
    if w.len() <= 3 {
        return 1;
    }
    let vowels = |c: char| "aeiouy".contains(c);
    let mut count = 0;
    let mut prev_vowel = false;
    for c in w.chars() {
        let v = vowels(c);
        if v && !prev_vowel {
            count += 1;
        }
        prev_vowel = v;
    }
    if w.ends_with('e') && !w.ends_with("le") && count > 1 {
        count -= 1;
    }
    if (w.ends_with("es") || w.ends_with("ed")) && count > 1 {
        count -= 1;
    }
    count.max(1)
}

fn fmt_duration(seconds: f64) -> String {
    let s = seconds.round() as u64;
    if s < 60 {
        format!("{s} sec")
    } else if s < 3600 {
        format!("{} min {} sec", s / 60, s % 60)
    } else {
        format!("{} h {} min", s / 3600, (s % 3600) / 60)
    }
}

/// Full statistics report.
pub fn stats(
    text: &str,
    reading_wpm: f64,
    speaking_wpm: f64,
    top_words: usize,
) -> EngineResult<Value> {
    engine_shared::error::require_range(reading_wpm, 50.0, 2000.0, "reading_wpm")?;
    engine_shared::error::require_range(speaking_wpm, 50.0, 1000.0, "speaking_wpm")?;
    let word_list = words(text);
    let word_count = word_list.len();
    let chars = text.chars().count();
    let graphemes = text.graphemes(true).count();
    let chars_no_spaces = text.chars().filter(|c| !c.is_whitespace()).count();
    let letters = text.chars().filter(|c| c.is_alphabetic()).count();
    let digits = text.chars().filter(|c| c.is_ascii_digit()).count();
    let spaces = text.chars().filter(|c| c.is_whitespace()).count();
    let punctuation = text.chars().filter(|c| c.is_ascii_punctuation()).count();
    let lines = if text.is_empty() { 0 } else { text.lines().count() };
    let non_empty_lines = text.lines().filter(|l| !l.trim().is_empty()).count();
    let sentences = count_sentences(text);
    let paragraphs = count_paragraphs(text);
    let syllable_total: usize = word_list.iter().map(|w| syllables(w)).sum();
    let avg_word_len = if word_count > 0 {
        word_list.iter().map(|w| w.chars().count()).sum::<usize>() as f64 / word_count as f64
    } else {
        0.0
    };
    let longest = word_list
        .iter()
        .max_by_key(|w| w.chars().count())
        .map(|w| w.to_string())
        .unwrap_or_default();
    let mut freq: HashMap<String, usize> = HashMap::new();
    for w in &word_list {
        let lw = w.to_lowercase();
        if lw.chars().any(|c| c.is_alphabetic()) {
            *freq.entry(lw).or_insert(0) += 1;
        }
    }
    let unique = freq.len();
    let mut top: Vec<(String, usize)> = freq.into_iter().collect();
    top.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let top: Vec<Value> = top
        .into_iter()
        .take(top_words.clamp(1, 200))
        .map(|(w, n)| json!({
            "word": w,
            "count": n,
            "density": if word_count > 0 { ((n as f64 / word_count as f64) * 10000.0).round() / 100.0 } else { 0.0 }
        }))
        .collect();
    let (flesch, grade) = if word_count > 0 && sentences > 0 {
        let wps = word_count as f64 / sentences as f64;
        let spw = syllable_total as f64 / word_count as f64;
        (
            (206.835 - 1.015 * wps - 84.6 * spw).clamp(-100.0, 121.0),
            (0.39 * wps + 11.8 * spw - 15.59).max(0.0),
        )
    } else {
        (0.0, 0.0)
    };
    let reading_sec = word_count as f64 / reading_wpm * 60.0;
    let speaking_sec = word_count as f64 / speaking_wpm * 60.0;
    let level = match flesch as i64 {
        90..=200 => "very easy (5th grade)",
        80..=89 => "easy (6th grade)",
        70..=79 => "fairly easy (7th grade)",
        60..=69 => "standard (8th-9th grade)",
        50..=59 => "fairly difficult (10th-12th grade)",
        30..=49 => "difficult (college)",
        _ => "very difficult (college graduate)",
    };
    Ok(json!({
        "words": word_count,
        "unique_words": unique,
        "characters": chars,
        "graphemes": graphemes,
        "characters_no_spaces": chars_no_spaces,
        "bytes": text.len(),
        "letters": letters,
        "digits": digits,
        "spaces": spaces,
        "punctuation": punctuation,
        "lines": lines,
        "non_empty_lines": non_empty_lines,
        "sentences": sentences,
        "paragraphs": paragraphs,
        "syllables": syllable_total,
        "average_word_length": (avg_word_len * 100.0).round() / 100.0,
        "average_sentence_length": if sentences > 0 { ((word_count as f64 / sentences as f64) * 100.0).round() / 100.0 } else { 0.0 },
        "longest_word": longest,
        "reading_time_seconds": reading_sec.round(),
        "reading_time": fmt_duration(reading_sec),
        "speaking_time_seconds": speaking_sec.round(),
        "speaking_time": fmt_duration(speaking_sec),
        "flesch_reading_ease": (flesch * 10.0).round() / 10.0,
        "flesch_kincaid_grade": (grade * 10.0).round() / 10.0,
        "readability": level,
        "top_words": top,
        "lexical_density": if word_count > 0 { ((unique as f64 / word_count as f64) * 1000.0).round() / 10.0 } else { 0.0 },
    }))
}

/// Keyword density for single words and 2/3-word phrases.
pub fn keyword_density(
    text: &str,
    top: usize,
    min_length: usize,
    stop_words: &[String],
) -> EngineResult<Value> {
    engine_shared::error::require_text(text, "text")?;
    let default_stop: &[&str] = &[
        "the", "a", "an", "and", "or", "of", "to", "in", "on", "for", "with", "is", "are", "was",
        "were", "be", "it", "this", "that", "as", "at", "by", "from", "but", "not", "if", "we",
        "you", "they", "i", "he", "she", "his", "her", "its", "our", "your", "their", "has",
        "have", "had", "do", "does", "did", "will", "would", "can", "could", "so", "than", "then",
        "there", "here", "what", "which", "who", "how",
    ];
    let stops: std::collections::HashSet<String> = if stop_words.is_empty() {
        default_stop.iter().map(|s| s.to_string()).collect()
    } else {
        stop_words.iter().map(|s| s.to_lowercase()).collect()
    };
    let all: Vec<String> = words(text)
        .iter()
        .map(|w| w.to_lowercase())
        .filter(|w| w.chars().any(|c| c.is_alphanumeric()))
        .collect();
    let total = all.len();
    let count_phrases = |n: usize| -> Vec<Value> {
        let mut map: HashMap<String, usize> = HashMap::new();
        for win in all.windows(n) {
            if win
                .iter()
                .any(|w| stops.contains(w) || w.chars().count() < min_length)
            {
                continue;
            }
            *map.entry(win.join(" ")).or_insert(0) += 1;
        }
        let mut v: Vec<(String, usize)> =
            map.into_iter().filter(|(_, c)| *c > 1 || n == 1).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        v.into_iter().take(top.clamp(1, 100)).map(|(p, c)| json!({
            "phrase": p,
            "count": c,
            "density": if total > 0 { ((c as f64 / total as f64) * 10000.0).round() / 100.0 } else { 0.0 }
        })).collect()
    };
    Ok(json!({
        "total_words": total,
        "one_word": count_phrases(1),
        "two_word": count_phrases(2),
        "three_word": count_phrases(3)
    }))
}

/// Generates hashtags from the most frequent meaningful words.
pub fn hashtags(text: &str, count: usize) -> EngineResult<Value> {
    let kd = keyword_density(text, count.clamp(1, 50), 3, &[])?;
    let tags: Vec<String> = kd["one_word"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| e["phrase"].as_str())
        .map(|w| format!("#{}", w.chars().filter(|c| c.is_alphanumeric()).collect::<String>()))
        .filter(|t| t.len() > 1)
        .collect();
    let phrases: Vec<String> = kd["two_word"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| e["phrase"].as_str())
        .take(5)
        .map(|p| {
            format!(
                "#{}",
                p.split(' ')
                    .map(|w| {
                        let mut c = w.chars();
                        match c.next() {
                            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                            None => String::new(),
                        }
                    })
                    .collect::<String>()
            )
        })
        .collect();
    Ok(json!({ "hashtags": tags, "phrase_hashtags": phrases, "line": tags.join(" ") }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts() {
        let t = "Hello world. This is a test!\n\nSecond paragraph here.";
        let s = stats(t, 200.0, 150.0, 5).unwrap();
        assert_eq!(s["words"], 9);
        assert_eq!(s["sentences"], 3);
        assert_eq!(s["paragraphs"], 2);
        assert_eq!(s["lines"], 3);
        assert!(s["flesch_reading_ease"].as_f64().unwrap() > 50.0);
        let e = stats("", 200.0, 150.0, 5).unwrap();
        assert_eq!(e["words"], 0);
        assert_eq!(e["reading_time"], "0 sec");
        assert!(stats("x", 1.0, 150.0, 5).is_err());
        let l = legacy_analyze("Hello world. This is a fast Rust engine.");
        assert_eq!(l.words, 8);
        assert_eq!(l.sentences, 2);
    }

    #[test]
    fn syllables_and_keywords() {
        assert_eq!(syllables("hello"), 2);
        assert_eq!(syllables("the"), 1);
        assert_eq!(syllables("beautiful"), 3);
        let k = keyword_density("rust engine rust engine wasm rust the the", 5, 3, &[]).unwrap();
        assert_eq!(k["one_word"][0]["phrase"], "rust");
        assert_eq!(k["one_word"][0]["count"], 3);
        assert_eq!(k["two_word"][0]["phrase"], "rust engine");
        assert!(keyword_density("   ", 5, 3, &[]).is_err());
        let h = hashtags("rust rust wasm wasm engine", 3).unwrap();
        assert_eq!(h["hashtags"][0], "#rust");
    }
}
