//! Secure generators: passwords, passphrases, tokens, UUID, ULID, nanoid,
//! and password strength estimation.

use engine_shared::{rng, EngineError, EngineResult};
use serde_json::{json, Value};

static WORDLIST: &str = include_str!("../assets/wordlist.txt");

fn words() -> Vec<&'static str> {
    WORDLIST.lines().filter(|l| !l.is_empty()).collect()
}

const AMBIGUOUS: &str = "0O1lI|`'\"";

/// Generates random passwords. Requires caller entropy for `secure: true`.
#[allow(clippy::too_many_arguments)]
pub fn password(
    length: usize,
    count: usize,
    uppercase: bool,
    lowercase: bool,
    digits: bool,
    symbols: bool,
    exclude_ambiguous: bool,
    custom_symbols: Option<&str>,
    require_each: bool,
) -> EngineResult<Value> {
    if !(4..=256).contains(&length) {
        return Err(EngineError::invalid("`length` must be between 4 and 256"));
    }
    if !(1..=100).contains(&count) {
        return Err(EngineError::invalid("`count` must be between 1 and 100"));
    }
    let mut groups: Vec<Vec<char>> = Vec::new();
    if lowercase {
        groups.push(('a'..='z').collect());
    }
    if uppercase {
        groups.push(('A'..='Z').collect());
    }
    if digits {
        groups.push(('0'..='9').collect());
    }
    if symbols {
        groups.push(
            custom_symbols
                .filter(|s| !s.is_empty())
                .unwrap_or("!@#$%^&*()-_=+[]{};:,.<>?/~")
                .chars()
                .collect(),
        );
    }
    if exclude_ambiguous {
        for g in groups.iter_mut() {
            g.retain(|c| !AMBIGUOUS.contains(*c));
        }
        groups.retain(|g| !g.is_empty());
    }
    if groups.is_empty() {
        return Err(EngineError::invalid("Enable at least one character class"));
    }
    let alphabet: Vec<char> = groups.iter().flatten().copied().collect();
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let mut chars: Vec<char> = Vec::with_capacity(length);
        if require_each && groups.len() <= length {
            for g in &groups {
                chars.push(*rng::pick(g, "character class")?);
            }
        }
        while chars.len() < length {
            chars.push(*rng::pick(&alphabet, "password alphabet")?);
        }
        rng::shuffle(&mut chars);
        out.push(chars.into_iter().collect::<String>());
    }
    let entropy = (alphabet.len() as f64).log2() * length as f64;
    Ok(json!({
        "passwords": out,
        "password": out[0],
        "entropy_bits": (entropy * 10.0).round() / 10.0,
        "alphabet_size": alphabet.len(),
        "secure": rng::has_caller_entropy(),
        "warning": if rng::has_caller_entropy() { Value::Null } else { json!("\
            No caller entropy supplied: pass `entropy` from crypto.getRandomValues for \
            cryptographically secure output") },
    }))
}

/// Diceware style passphrase from the built-in 2048-word list (11 bits/word).
pub fn passphrase(
    word_count: usize,
    separator: &str,
    capitalize: bool,
    add_number: bool,
    count: usize,
) -> EngineResult<Value> {
    if !(2..=20).contains(&word_count) {
        return Err(EngineError::invalid("`words` must be between 2 and 20"));
    }
    if !(1..=50).contains(&count) {
        return Err(EngineError::invalid("`count` must be between 1 and 50"));
    }
    let list = words();
    let mut out = Vec::new();
    for _ in 0..count {
        let mut parts: Vec<String> = (0..word_count)
            .map(|_| {
                let w = rng::choose(&list).copied().unwrap_or("word");
                if capitalize {
                    let mut c = w.chars();
                    c.next()
                        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                        .unwrap_or_default()
                } else {
                    w.to_string()
                }
            })
            .collect();
        if add_number {
            let idx = rng::below(parts.len() as u64) as usize;
            parts[idx].push_str(&rng::below(100).to_string());
        }
        out.push(parts.join(separator));
    }
    let entropy =
        (list.len() as f64).log2() * word_count as f64 + if add_number { 6.6 } else { 0.0 };
    Ok(json!({
        "passphrases": out,
        "passphrase": out[0],
        "entropy_bits": (entropy * 10.0).round() / 10.0,
        "wordlist_size": list.len(),
        "secure": rng::has_caller_entropy()
    }))
}

/// Random bytes in the requested format (hex, base64, base64url, binary).
pub fn random_bytes(length: usize, format: &str, count: usize) -> EngineResult<Value> {
    if !(1..=4096).contains(&length) {
        return Err(EngineError::invalid("`length` must be between 1 and 4096 bytes"));
    }
    if !(1..=100).contains(&count) {
        return Err(EngineError::invalid("`count` must be between 1 and 100"));
    }
    let mut items = Vec::new();
    for _ in 0..count {
        let mut buf = vec![0u8; length];
        rng::fill_bytes(&mut buf);
        items.push(crate::hashing::encode_output(&buf, format)?);
    }
    Ok(json!({
        "values": items,
        "value": items[0],
        "bytes": length,
        "bits": length * 8,
        "secure": rng::has_caller_entropy()
    }))
}

/// Random integers in `[min, max]`.
pub fn random_numbers(min: i64, max: i64, count: usize, unique: bool) -> EngineResult<Value> {
    if min > max {
        return Err(EngineError::invalid("`min` must be <= `max`"));
    }
    if !(1..=10_000).contains(&count) {
        return Err(EngineError::invalid("`count` must be between 1 and 10000"));
    }
    let span = (max as i128 - min as i128 + 1) as u128;
    if unique && (count as u128) > span {
        return Err(EngineError::invalid("Not enough distinct values in the range for `unique`"));
    }
    let mut out: Vec<i64> = Vec::with_capacity(count);
    let mut seen = std::collections::HashSet::new();
    while out.len() < count {
        let v = rng::range_inclusive(min, max);
        if unique && !seen.insert(v) {
            continue;
        }
        out.push(v);
    }
    Ok(json!({ "values": out, "value": out[0], "secure": rng::has_caller_entropy() }))
}

fn uuid_v4() -> uuid::Uuid {
    let mut bytes = [0u8; 16];
    rng::fill_bytes(&mut bytes);
    uuid::Builder::from_random_bytes(bytes).into_uuid()
}

fn uuid_v7(timestamp_ms: u64) -> uuid::Uuid {
    let mut rand = [0u8; 10];
    rng::fill_bytes(&mut rand);
    uuid::Builder::from_unix_timestamp_millis(timestamp_ms, &rand).into_uuid()
}

/// UUID generation (v4 random, v7 time-ordered, v3/v5 name based, nil, max).
pub fn uuid(
    version: &str,
    count: usize,
    timestamp_ms: Option<u64>,
    namespace: Option<&str>,
    name: Option<&str>,
    format: &str,
) -> EngineResult<Value> {
    if !(1..=1000).contains(&count) {
        return Err(EngineError::invalid("`count` must be between 1 and 1000"));
    }
    let ns = |s: Option<&str>| -> EngineResult<uuid::Uuid> {
        let raw = s.ok_or_else(|| {
            EngineError::invalid("`namespace` is required (dns, url, oid, x500 or a UUID)")
        })?;
        Ok(match raw.to_ascii_lowercase().as_str() {
            "dns" => uuid::Uuid::NAMESPACE_DNS,
            "url" => uuid::Uuid::NAMESPACE_URL,
            "oid" => uuid::Uuid::NAMESPACE_OID,
            "x500" => uuid::Uuid::NAMESPACE_X500,
            other => uuid::Uuid::parse_str(other)
                .map_err(|e| EngineError::invalid(format!("Invalid namespace UUID: {e}")))?,
        })
    };
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let u = match version.trim().to_ascii_lowercase().trim_start_matches('v') {
            "" | "4" => uuid_v4(),
            "7" => uuid_v7(
                timestamp_ms.ok_or_else(|| {
                    EngineError::invalid(
                        "`timestamp_ms` (Date.now()) is required for UUID v7; the WASM engine has \
                         no clock",
                    )
                })? + i as u64 / 1000,
            ),
            "5" => uuid::Uuid::new_v5(
                &ns(namespace)?,
                name.ok_or_else(|| EngineError::invalid("`name` is required for v5"))?
                    .as_bytes(),
            ),
            "3" => uuid::Uuid::new_v3(
                &ns(namespace)?,
                name.ok_or_else(|| EngineError::invalid("`name` is required for v3"))?
                    .as_bytes(),
            ),
            "nil" | "0" => uuid::Uuid::nil(),
            "max" => uuid::Uuid::max(),
            other => {
                return Err(EngineError::invalid(format!(
                    "Unsupported UUID version `{other}` (4, 7, 5, 3, nil, max)"
                )))
            },
        };
        out.push(match format {
            "" | "hyphenated" => u.hyphenated().to_string(),
            "simple" | "compact" => u.simple().to_string(),
            "urn" => u.urn().to_string(),
            "braced" => u.braced().to_string(),
            "uppercase" => u.hyphenated().to_string().to_uppercase(),
            other => return Err(EngineError::invalid(format!("Unknown UUID format `{other}`"))),
        });
    }
    Ok(json!({
        "uuids": out,
        "uuid": out[0],
        "version": if version.is_empty() { "4".to_string() } else { version.to_string() },
        "secure": rng::has_caller_entropy() || !matches!(version, "" | "4" | "7")
    }))
}

const CROCKFORD: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// ULID: 48-bit timestamp + 80-bit randomness, Crockford base32.
pub fn ulid(count: usize, timestamp_ms: u64, lowercase: bool) -> EngineResult<Value> {
    if !(1..=1000).contains(&count) {
        return Err(EngineError::invalid("`count` must be between 1 and 1000"));
    }
    if timestamp_ms >= (1u64 << 48) {
        return Err(EngineError::invalid("`timestamp_ms` exceeds 48 bits"));
    }
    let mut out = Vec::new();
    let mut last_rand: u128 = 0;
    for i in 0..count {
        let rand: u128 = if i == 0 {
            let mut b = [0u8; 16];
            rng::fill_bytes(&mut b);
            u128::from_be_bytes(b) & ((1u128 << 80) - 1)
        } else {
            last_rand.wrapping_add(1) & ((1u128 << 80) - 1) // monotonic within the same ms
        };
        last_rand = rand;
        let value: u128 = ((timestamp_ms as u128) << 80) | rand;
        let mut s = [0u8; 26];
        let mut v = value;
        for idx in (0..26).rev() {
            s[idx] = CROCKFORD[(v & 31) as usize];
            v >>= 5;
        }
        let text = String::from_utf8_lossy(&s).into_owned();
        out.push(if lowercase { text.to_lowercase() } else { text });
    }
    Ok(json!({
        "ulids": out,
        "ulid": out[0],
        "timestamp_ms": timestamp_ms,
        "secure": rng::has_caller_entropy()
    }))
}

/// Decodes a ULID into its timestamp and randomness.
pub fn ulid_decode(text: &str) -> EngineResult<Value> {
    let t = text.trim().to_uppercase();
    if t.len() != 26 {
        return Err(EngineError::invalid("A ULID is exactly 26 characters"));
    }
    let mut value: u128 = 0;
    for c in t.bytes() {
        let idx = CROCKFORD.iter().position(|&x| x == c).ok_or_else(|| {
            EngineError::invalid(format!("Invalid ULID character `{}`", c as char))
        })?;
        value = (value << 5) | idx as u128;
    }
    let ts = (value >> 80) as u64;
    Ok(json!({
        "timestamp_ms": ts,
        "randomness_hex": format!("{:020x}", value & ((1u128 << 80) - 1)),
        "valid": ts < (1 << 48)
    }))
}

/// nanoid style identifiers.
pub fn nanoid(size: usize, alphabet: Option<&str>, count: usize) -> EngineResult<Value> {
    if !(2..=256).contains(&size) {
        return Err(EngineError::invalid("`size` must be between 2 and 256"));
    }
    if !(1..=1000).contains(&count) {
        return Err(EngineError::invalid("`count` must be between 1 and 1000"));
    }
    let alpha: Vec<char> = alphabet
        .filter(|a| !a.is_empty())
        .unwrap_or("_-0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ")
        .chars()
        .collect();
    if alpha.len() < 2 || alpha.len() > 256 {
        return Err(EngineError::invalid("`alphabet` must contain 2 to 256 characters"));
    }
    let ids: Vec<String> = (0..count)
        .map(|_| (0..size).filter_map(|_| rng::choose(&alpha).copied()).collect())
        .collect();
    let entropy = (alpha.len() as f64).log2() * size as f64;
    Ok(json!({
        "ids": ids,
        "id": ids[0],
        "entropy_bits": (entropy * 10.0).round() / 10.0,
        "secure": rng::has_caller_entropy()
    }))
}

const COMMON_PASSWORDS: &[&str] = &[
    "123456",
    "password",
    "12345678",
    "qwerty",
    "123456789",
    "12345",
    "1234",
    "111111",
    "1234567",
    "dragon",
    "123123",
    "baseball",
    "abc123",
    "football",
    "monkey",
    "letmein",
    "696969",
    "shadow",
    "master",
    "666666",
    "qwertyuiop",
    "123321",
    "mustang",
    "1234567890",
    "michael",
    "654321",
    "pussy",
    "superman",
    "1qaz2wsx",
    "7777777",
    "fuckyou",
    "121212",
    "000000",
    "qazwsx",
    "123qwe",
    "killer",
    "trustno1",
    "jordan",
    "jennifer",
    "zxcvbnm",
    "asdfgh",
    "hunter",
    "buster",
    "soccer",
    "harley",
    "batman",
    "andrew",
    "tigger",
    "sunshine",
    "iloveyou",
    "fuckme",
    "2000",
    "charlie",
    "robert",
    "thomas",
    "hockey",
    "ranger",
    "daniel",
    "starwars",
    "klaster",
    "112233",
    "george",
    "asshole",
    "computer",
    "michelle",
    "jessica",
    "pepper",
    "1111",
    "zxcvbn",
    "555555",
    "11111111",
    "131313",
    "freedom",
    "777777",
    "pass",
    "fuck",
    "maggie",
    "159753",
    "aaaaaa",
    "ginger",
    "princess",
    "joshua",
    "cheese",
    "amanda",
    "summer",
    "love",
    "ashley",
    "6969",
    "nicole",
    "chelsea",
    "biteme",
    "matthew",
    "access",
    "yankees",
    "987654321",
    "dallas",
    "austin",
    "thunder",
    "taylor",
    "matrix",
    "admin",
    "welcome",
    "password1",
    "p@ssw0rd",
    "passw0rd",
    "letmein1",
    "secret",
    "root",
    "toor",
    "changeme",
    "test",
    "guest",
];

/// Heuristic password strength estimate (entropy, patterns, crack times).
pub fn password_strength(password: &str) -> EngineResult<Value> {
    if password.is_empty() {
        return Err(EngineError::empty("password"));
    }
    let len = password.chars().count();
    let has_lower = password.chars().any(|c| c.is_lowercase());
    let has_upper = password.chars().any(|c| c.is_uppercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_symbol = password.chars().any(|c| !c.is_alphanumeric());
    let has_unicode = !password.is_ascii();
    let mut pool = 0usize;
    if has_lower {
        pool += 26;
    }
    if has_upper {
        pool += 26;
    }
    if has_digit {
        pool += 10;
    }
    if has_symbol {
        pool += 33;
    }
    if has_unicode {
        pool += 100;
    }
    let mut entropy = (pool.max(1) as f64).log2() * len as f64;
    let lower = password.to_lowercase();
    let mut warnings: Vec<String> = Vec::new();
    let mut suggestions: Vec<String> = Vec::new();
    let de_leet: String = lower
        .chars()
        .map(|c| match c {
            '0' => 'o',
            '1' | '!' => 'i',
            '3' => 'e',
            '4' | '@' => 'a',
            '5' | '$' => 's',
            '7' => 't',
            _ => c,
        })
        .collect();
    if COMMON_PASSWORDS.iter().any(|p| *p == lower || *p == de_leet) {
        entropy = entropy.min(10.0);
        warnings.push("This is one of the most common passwords".into());
    } else if COMMON_PASSWORDS
        .iter()
        .any(|p| p.len() >= 5 && (lower.contains(p) || de_leet.contains(p)))
    {
        entropy *= 0.5;
        warnings.push("Contains a very common password".into());
    }
    // Repeated characters / sequences.
    let chars: Vec<char> = password.chars().collect();
    let repeats = chars.windows(3).filter(|w| w[0] == w[1] && w[1] == w[2]).count();
    if repeats > 0 {
        entropy -= 4.0 * repeats as f64;
        warnings.push("Contains repeated characters".into());
    }
    let seq = chars
        .windows(3)
        .filter(|w| {
            let (a, b, c) = (w[0] as i32, w[1] as i32, w[2] as i32);
            (b - a == 1 && c - b == 1) || (a - b == 1 && b - c == 1)
        })
        .count();
    if seq > 0 {
        entropy -= 4.0 * seq as f64;
        warnings.push("Contains sequences like abc or 123".into());
    }
    let keyboard = ["qwerty", "asdf", "zxcv", "qaz", "wsx", "1qaz", "poiuy"];
    if keyboard.iter().any(|k| lower.contains(k)) {
        entropy -= 8.0;
        warnings.push("Contains a keyboard pattern".into());
    }
    let dict_words = words();
    if len >= 4 && dict_words.iter().any(|w| w.len() >= 5 && lower == *w) {
        entropy = entropy.min(14.0);
        warnings.push("Is a single dictionary word".into());
    }
    if chars.iter().rev().take(4).all(|c| c.is_ascii_digit())
        && len > 4
        && chars[..len - 4].iter().all(|c| c.is_alphabetic())
    {
        warnings.push("Word followed by digits (e.g. a year) is predictable".into());
        entropy -= 6.0;
    }
    let entropy = entropy.max(0.0);
    if len < 8 {
        suggestions.push("Use at least 12 characters".into());
    }
    if !(has_upper && has_lower) {
        suggestions.push("Mix upper and lower case letters".into());
    }
    if !has_digit {
        suggestions.push("Add digits".into());
    }
    if !has_symbol {
        suggestions.push("Add symbols".into());
    }
    if len < 16 && warnings.is_empty() {
        suggestions.push(
            "Longer passphrases (4+ random words) are easier to remember and stronger".into(),
        );
    }
    let score = match entropy {
        e if e < 28.0 => 0,
        e if e < 36.0 => 1,
        e if e < 60.0 => 2,
        e if e < 80.0 => 3,
        _ => 4,
    };
    let label = ["very weak", "weak", "fair", "strong", "very strong"][score];
    let guesses = 2f64.powf(entropy) / 2.0;
    let fmt = |seconds: f64| -> String {
        if seconds < 1.0 {
            "instantly".into()
        } else if seconds < 60.0 {
            format!("{:.0} seconds", seconds)
        } else if seconds < 3600.0 {
            format!("{:.0} minutes", seconds / 60.0)
        } else if seconds < 86400.0 {
            format!("{:.0} hours", seconds / 3600.0)
        } else if seconds < 31_557_600.0 {
            format!("{:.0} days", seconds / 86400.0)
        } else if seconds < 31_557_600.0 * 1000.0 {
            format!("{:.0} years", seconds / 31_557_600.0)
        } else if seconds < 31_557_600.0 * 1e9 {
            format!("{:.0} thousand years", seconds / 31_557_600.0 / 1000.0)
        } else {
            "centuries".into()
        }
    };
    Ok(json!({
        "score": score,
        "label": label,
        "entropy_bits": (entropy * 10.0).round() / 10.0,
        "length": len,
        "character_classes": {
            "lowercase": has_lower,
            "uppercase": has_upper,
            "digits": has_digit,
            "symbols": has_symbol,
            "unicode": has_unicode
        },
        "pool_size": pool,
        "crack_time": {
            "online_throttled_100_per_hour": fmt(guesses / (100.0 / 3600.0)),
            "online_unthrottled_10_per_second": fmt(guesses / 10.0),
            "offline_slow_hash_10k_per_second": fmt(guesses / 1e4),
            "offline_fast_hash_10b_per_second": fmt(guesses / 1e10),
        },
        "warnings": warnings,
        "suggestions": suggestions,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generators() {
        let p = password(16, 3, true, true, true, true, true, None, true).unwrap();
        assert_eq!(p["passwords"].as_array().unwrap().len(), 3);
        let pw = p["password"].as_str().unwrap();
        assert_eq!(pw.chars().count(), 16);
        assert!(pw.chars().any(|c| c.is_ascii_digit()) && pw.chars().any(|c| c.is_uppercase()));
        assert!(!pw.contains('0') && !pw.contains('l'));
        assert!(password(16, 1, false, false, false, false, false, None, false).is_err());
        assert!(password(2, 1, true, true, true, true, false, None, false).is_err());
        let pp = passphrase(4, "-", true, true, 2).unwrap();
        assert_eq!(pp["passphrase"].as_str().unwrap().split('-').count(), 4);
        assert!(pp["entropy_bits"].as_f64().unwrap() > 40.0);
        let rb = random_bytes(16, "hex", 1).unwrap();
        assert_eq!(rb["value"].as_str().unwrap().len(), 32);
        let rn = random_numbers(1, 6, 6, true).unwrap();
        let mut v: Vec<i64> = rn["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_i64().unwrap())
            .collect();
        v.sort();
        assert_eq!(v, vec![1, 2, 3, 4, 5, 6]);
        assert!(random_numbers(1, 2, 5, true).is_err());
        assert!(random_numbers(5, 1, 1, false).is_err());
    }

    #[test]
    fn identifiers() {
        let u = uuid("4", 2, None, None, None, "").unwrap();
        let s = u["uuid"].as_str().unwrap();
        assert_eq!(s.len(), 36);
        assert_eq!(&s[14..15], "4");
        let u7 = uuid("7", 1, Some(1_700_000_000_000), None, None, "simple").unwrap();
        assert_eq!(u7["uuid"].as_str().unwrap().len(), 32);
        assert!(uuid("7", 1, None, None, None, "").is_err());
        let u5 = uuid("5", 1, None, Some("dns"), Some("example.com"), "").unwrap();
        assert_eq!(u5["uuid"], "cfbff0d1-9375-5685-968c-48ce8b15ae17");
        assert_eq!(
            uuid("nil", 1, None, None, None, "urn").unwrap()["uuid"],
            "urn:uuid:00000000-0000-0000-0000-000000000000"
        );
        assert!(uuid("9", 1, None, None, None, "").is_err());
        let ul = ulid(2, 1_700_000_000_000, false).unwrap();
        let first = ul["ulid"].as_str().unwrap();
        assert_eq!(first.len(), 26);
        assert_eq!(ulid_decode(first).unwrap()["timestamp_ms"], 1_700_000_000_000u64);
        assert!(ul["ulids"][1].as_str().unwrap() > first);
        assert!(ulid_decode("short").is_err());
        let n = nanoid(21, None, 1).unwrap();
        assert_eq!(n["id"].as_str().unwrap().len(), 21);
        assert!(nanoid(21, Some("a"), 1).is_err());
    }

    #[test]
    fn strength() {
        let weak = password_strength("password").unwrap();
        assert_eq!(weak["score"], 0);
        assert!(weak["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("common")));
        let strong = password_strength("Tr0ub4dor&3-correct-horse-battery").unwrap();
        assert!(strong["score"].as_u64().unwrap() >= 3);
        assert_eq!(password_strength("abc123").unwrap()["score"], 0);
        assert!(password_strength("").is_err());
        assert!(password_strength("aaaaaaaa").unwrap()["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("repeated")));
    }
}
