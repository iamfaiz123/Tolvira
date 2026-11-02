//! Catalog fuzzy-search engine used by the frontend search box.
//!
//! This crate predates the shared `{action, payload}` protocol and keeps its
//! own tiny ABI: `alloc` → `search(ptr, len)` → `result_ptr` / `result_len`.

use serde::Deserialize;
use std::cell::RefCell;

#[derive(Deserialize)]
struct Tool {
    id: u32,
    name: String,
    category: String,
    #[allow(dead_code)]
    slug: String,
    #[allow(dead_code)]
    description: String,
}

#[derive(Deserialize)]
struct SearchRequest {
    query: String,
    category: Option<String>,
    tools: Vec<Tool>,
}

thread_local! {
    static RESULT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn set_result(value: Vec<u8>) {
    RESULT.with(|result| *result.borrow_mut() = value);
}

/// Allocates `length` bytes owned by the caller; release them via `search`.
#[no_mangle]
pub extern "C" fn alloc(length: usize) -> *mut u8 {
    let buffer = vec![0u8; length].into_boxed_slice();
    Box::into_raw(buffer) as *mut u8
}

/// # Safety
/// `input_ptr`/`input_len` must describe a buffer previously returned by `alloc`.
#[no_mangle]
pub unsafe extern "C" fn search(input_ptr: *mut u8, input_len: usize) {
    // SAFETY: the caller contract above guarantees the buffer came from `alloc`.
    let input = unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(input_ptr, input_len)) };
    let request: SearchRequest = match serde_json::from_slice(&input) {
        Ok(request) => request,
        Err(_) => {
            set_result(b"[]".to_vec());
            return;
        },
    };
    drop(input);

    let query = normalize(&request.query);
    let tokens: Vec<&str> = query.split_whitespace().collect();
    let mut matches: Vec<(u32, i32)> = request
        .tools
        .into_iter()
        .filter_map(|tool| {
            if request
                .category
                .as_ref()
                .is_some_and(|category| category != &tool.category)
            {
                return None;
            }
            if tokens.is_empty() {
                return Some((tool.id, 0));
            }
            score(&tool.name, &tool.category, &tokens).map(|score| (tool.id, score))
        })
        .collect();
    matches.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let ids: Vec<u32> = matches.into_iter().map(|(id, _)| id).collect();
    set_result(serde_json::to_vec(&ids).unwrap_or_else(|_| b"[]".to_vec()));
}

/// Pointer to the last search result (JSON bytes).
#[no_mangle]
pub extern "C" fn result_ptr() -> *const u8 {
    RESULT.with(|result| result.borrow().as_ptr())
}

/// Length of the last search result.
#[no_mangle]
pub extern "C" fn result_len() -> usize {
    RESULT.with(|result| result.borrow().len())
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .map(|ch| if ch.is_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
}

fn score(name: &str, category: &str, query_tokens: &[&str]) -> Option<i32> {
    let normalized_name = normalize(name);
    let normalized_category = normalize(category);
    let name_words: Vec<&str> = normalized_name.split_whitespace().collect();
    let category_words: Vec<&str> = normalized_category.split_whitespace().collect();
    let mut total = 0;

    for query in query_tokens {
        let name_score = word_score(query, &normalized_name, &name_words, 100, 82, 60);
        let category_score = word_score(query, &normalized_category, &category_words, 32, 24, 18);
        let best = name_score.max(category_score);
        if best == 0 {
            return None;
        }
        total += best;
    }

    Some(total)
}

fn word_score(
    query: &str,
    full: &str,
    words: &[&str],
    exact_score: i32,
    prefix_score: i32,
    contains_score: i32,
) -> i32 {
    if full == query {
        return exact_score;
    }
    let mut best = 0;
    for word in words {
        let score = if *word == query {
            exact_score
        } else if word.starts_with(query) {
            prefix_score
        } else if word.contains(query) || full.contains(query) {
            contains_score
        } else {
            fuzzy_score(query, word)
        };
        best = best.max(score);
    }
    best
}

fn fuzzy_score(query: &str, word: &str) -> i32 {
    let query_chars: Vec<char> = query.chars().collect();
    let word_chars: Vec<char> = word.chars().collect();
    if query_chars.len() < 4 || word_chars.len() < 4 {
        return 0;
    }
    let distance = levenshtein(&query_chars, &word_chars);
    match distance {
        1 => 28,
        2 if query_chars.len() >= 6 => 12,
        _ => 0,
    }
}

fn levenshtein(left: &[char], right: &[char]) -> usize {
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    for (i, left_char) in left.iter().enumerate() {
        let mut current = vec![i + 1; right.len() + 1];
        for (j, right_char) in right.iter().enumerate() {
            current[j + 1] = (current[j] + 1)
                .min(previous[j + 1] + 1)
                .min(previous[j] + usize::from(left_char != right_char));
        }
        previous = current;
    }
    previous[right.len()]
}
