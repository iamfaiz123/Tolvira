//! Digests, HMAC, checksums and password hashing (bcrypt / argon2 / PBKDF2).

use engine_shared::{b64, limits, EngineError, EngineResult};
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::Digest;

/// Supported digest algorithm names.
pub const ALGORITHMS: &[&str] = &[
    "md5",
    "sha1",
    "sha224",
    "sha256",
    "sha384",
    "sha512",
    "sha512-256",
    "sha3-224",
    "sha3-256",
    "sha3-384",
    "sha3-512",
    "blake2b-512",
    "blake2s-256",
    "blake3",
    "crc32",
    "adler32",
    "keccak256",
];

fn normalise(name: &str) -> String {
    name.trim()
        .to_ascii_lowercase()
        .replace(['_', ' '], "-")
        .replace("sha-", "sha")
        .replace("blake-", "blake")
}

/// Computes one digest, returned as raw bytes.
pub fn digest(algorithm: &str, data: &[u8]) -> EngineResult<Vec<u8>> {
    let a = normalise(algorithm);
    Ok(match a.as_str() {
        "md5" => md5::Md5::digest(data).to_vec(),
        "sha1" => sha1::Sha1::digest(data).to_vec(),
        "sha224" => sha2::Sha224::digest(data).to_vec(),
        "sha256" => sha2::Sha256::digest(data).to_vec(),
        "sha384" => sha2::Sha384::digest(data).to_vec(),
        "sha512" => sha2::Sha512::digest(data).to_vec(),
        "sha512-256" => sha2::Sha512_256::digest(data).to_vec(),
        "sha3-224" => sha3::Sha3_224::digest(data).to_vec(),
        "sha3-256" => sha3::Sha3_256::digest(data).to_vec(),
        "sha3-384" => sha3::Sha3_384::digest(data).to_vec(),
        "sha3-512" => sha3::Sha3_512::digest(data).to_vec(),
        "keccak256" => sha3::Keccak256::digest(data).to_vec(),
        "blake2b-512" | "blake2b" => blake2::Blake2b512::digest(data).to_vec(),
        "blake2s-256" | "blake2s" => blake2::Blake2s256::digest(data).to_vec(),
        "blake3" => blake3::hash(data).as_bytes().to_vec(),
        "crc32" => crc32fast::hash(data).to_be_bytes().to_vec(),
        "adler32" => adler32(data).to_be_bytes().to_vec(),
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown hash algorithm `{other}`. Supported: {}",
                ALGORITHMS.join(", ")
            )))
        },
    })
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for &byte in chunk {
            a += byte as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

/// Formats digest bytes as `hex`, `base64` or `binary`.
pub fn encode_output(bytes: &[u8], format: &str) -> EngineResult<String> {
    Ok(match format {
        "" | "hex" => hex::encode(bytes),
        "hex-upper" | "HEX" => hex::encode_upper(bytes),
        "base64" => b64::encode(bytes),
        "base64url" => {
            base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, bytes)
        },
        "binary" => bytes.iter().map(|b| format!("{b:08b}")).collect::<Vec<_>>().join(" "),
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown output format `{other}` (hex, hex-upper, base64, base64url, binary)"
            )))
        },
    })
}

/// Resolves the `text` / `file` input pair shared by hashing actions.
pub fn input_bytes(text: Option<&str>, file: Option<&[u8]>) -> EngineResult<Vec<u8>> {
    match (file, text) {
        (Some(f), _) if !f.is_empty() => {
            limits::check_size(f.len(), limits::MAX_FILE_BYTES, "file")?;
            Ok(f.to_vec())
        },
        (_, Some(t)) if !t.is_empty() => Ok(t.as_bytes().to_vec()),
        _ => Err(EngineError::empty("input (`text` or `file`)")),
    }
}

/// Computes several digests at once and optionally compares with `expected`.
pub fn hash(
    data: &[u8],
    algorithms: &[String],
    format: &str,
    expected: Option<&str>,
) -> EngineResult<Value> {
    let algos: Vec<String> = if algorithms.is_empty() {
        vec!["md5".into(), "sha1".into(), "sha256".into(), "sha512".into()]
    } else {
        algorithms.to_vec()
    };
    let mut out = serde_json::Map::new();
    let mut matched: Option<String> = None;
    let exp = expected.map(|e| e.trim().to_ascii_lowercase().replace([' ', ':', '-'], ""));
    for a in &algos {
        let d = digest(a, data)?;
        let encoded = encode_output(&d, format)?;
        if let Some(e) = &exp {
            let hex_lower = hex::encode(&d);
            let b64s = b64::encode(&d).to_ascii_lowercase();
            if hex_lower == *e || b64s == *e {
                matched = Some(normalise(a));
            }
        }
        out.insert(normalise(a), json!(encoded));
    }
    Ok(json!({
        "hashes": out,
        "bytes": data.len(),
        "format": if format.is_empty() { "hex" } else { format },
        "expected": expected,
        "matches": exp.as_ref().map(|_| matched.is_some()),
        "matched_algorithm": matched,
    }))
}

fn hmac_bytes(algorithm: &str, key: &[u8], message: &[u8]) -> EngineResult<Vec<u8>> {
    macro_rules! mac {
        ($t:ty) => {{
            let mut m =
                <Hmac<$t>>::new_from_slice(key).map_err(|e| EngineError::invalid(e.to_string()))?;
            m.update(message);
            m.finalize().into_bytes().to_vec()
        }};
    }
    Ok(match normalise(algorithm).as_str() {
        "" | "sha256" => mac!(sha2::Sha256),
        "sha1" => mac!(sha1::Sha1),
        "sha224" => mac!(sha2::Sha224),
        "sha384" => mac!(sha2::Sha384),
        "sha512" => mac!(sha2::Sha512),
        "md5" => mac!(md5::Md5),
        "sha3-256" => mac!(sha3::Sha3_256),
        "sha3-512" => mac!(sha3::Sha3_512),
        other => {
            return Err(EngineError::invalid(format!(
                "Unsupported HMAC hash `{other}` (md5, sha1, sha224, sha256, sha384, sha512, \
                 sha3-256, sha3-512)"
            )))
        },
    })
}

/// Decodes a key given as text, hex (`hex:`) or base64 (`base64:`).
pub fn key_bytes(key: &str, encoding: &str) -> EngineResult<Vec<u8>> {
    if key.is_empty() {
        return Err(EngineError::empty("key"));
    }
    Ok(match encoding {
        "" | "utf8" | "text" => key.as_bytes().to_vec(),
        "hex" => hex::decode(key.trim())
            .map_err(|e| EngineError::invalid(format!("Key is not valid hex: {e}")))?,
        "base64" => b64::decode(key)?,
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown key encoding `{other}` (utf8, hex, base64)"
            )))
        },
    })
}

/// Computes an HMAC with the given hash algorithm.
pub fn hmac(message: &[u8], key: &[u8], algorithm: &str, format: &str) -> EngineResult<Value> {
    let mac = hmac_bytes(algorithm, key, message)?;
    Ok(json!({
        "hmac": encode_output(&mac, format)?,
        "algorithm": format!("HMAC-{}", normalise(if algorithm.is_empty() { "sha256" } else { algorithm }).to_uppercase()),
        "bytes": mac.len()
    }))
}

/// Verifies an HMAC in constant time.
pub fn hmac_verify(
    message: &[u8],
    key: &[u8],
    algorithm: &str,
    signature: &str,
) -> EngineResult<Value> {
    let mac = hmac_bytes(algorithm, key, message)?;
    let sig = signature.trim();
    let provided = hex::decode(sig)
        .or_else(|_| b64::decode(sig))
        .map_err(|_| EngineError::invalid("Signature must be hex or base64"))?;
    use subtle::ConstantTimeEq;
    let valid = mac.len() == provided.len() && mac.ct_eq(&provided).into();
    Ok(json!({ "valid": valid, "expected_length": mac.len(), "provided_length": provided.len() }))
}

/// Hashes a password with bcrypt at the given cost.
pub fn bcrypt_hash(password: &str, cost: u32) -> EngineResult<Value> {
    if password.is_empty() {
        return Err(EngineError::empty("password"));
    }
    if !(4..=14).contains(&cost) {
        return Err(EngineError::invalid(
            "bcrypt `cost` must be between 4 and 14 (10-12 recommended)",
        ));
    }
    if password.len() > 72 {
        return Err(EngineError::invalid(
            "bcrypt only uses the first 72 bytes; use argon2 for longer passwords",
        ));
    }
    let mut salt = [0u8; 16];
    engine_shared::rng::fill_bytes(&mut salt);
    let h = bcrypt::hash_with_salt(password.as_bytes(), cost, salt)
        .map_err(|e| EngineError::internal(e.to_string()))?;
    Ok(json!({
        "hash": h.to_string(),
        "cost": cost,
        "secure": engine_shared::rng::has_caller_entropy()
    }))
}

/// Verifies a bcrypt hash.
pub fn bcrypt_verify(password: &str, hash: &str) -> EngineResult<Value> {
    let valid = bcrypt::verify(password.as_bytes(), hash.trim())
        .map_err(|e| EngineError::invalid(format!("Invalid bcrypt hash: {e}")))?;
    Ok(json!({ "valid": valid }))
}

/// Hashes a password with Argon2 (id/i/d) using the given memory/time/parallelism costs.
pub fn argon2_hash(
    password: &str,
    variant: &str,
    m_cost_kib: u32,
    t_cost: u32,
    p_cost: u32,
) -> EngineResult<Value> {
    use argon2::password_hash::{PasswordHasher, SaltString};
    if password.is_empty() {
        return Err(EngineError::empty("password"));
    }
    let algorithm = match variant.to_ascii_lowercase().as_str() {
        "" | "argon2id" | "id" => argon2::Algorithm::Argon2id,
        "argon2i" | "i" => argon2::Algorithm::Argon2i,
        "argon2d" | "d" => argon2::Algorithm::Argon2d,
        other => return Err(EngineError::invalid(format!("Unknown argon2 variant `{other}`"))),
    };
    if !(8..=1_048_576).contains(&m_cost_kib)
        || !(1..=64).contains(&t_cost)
        || !(1..=16).contains(&p_cost)
    {
        return Err(EngineError::invalid(
            "argon2 parameters out of range (m_cost 8..1048576 KiB, t_cost 1..64, p_cost 1..16)",
        ));
    }
    let params = argon2::Params::new(m_cost_kib, t_cost, p_cost, None)
        .map_err(|e| EngineError::invalid(e.to_string()))?;
    let hasher = argon2::Argon2::new(algorithm, argon2::Version::V0x13, params);
    let mut salt = [0u8; 16];
    engine_shared::rng::fill_bytes(&mut salt);
    let salt_str =
        SaltString::encode_b64(&salt).map_err(|e| EngineError::internal(e.to_string()))?;
    let hash = hasher
        .hash_password(password.as_bytes(), &salt_str)
        .map_err(|e| EngineError::internal(e.to_string()))?;
    Ok(json!({
        "hash": hash.to_string(),
        "variant": format!("{algorithm}"),
        "m_cost_kib": m_cost_kib,
        "t_cost": t_cost,
        "p_cost": p_cost,
        "secure": engine_shared::rng::has_caller_entropy()
    }))
}

/// Verifies an Argon2 PHC string.
pub fn argon2_verify(password: &str, hash: &str) -> EngineResult<Value> {
    use argon2::password_hash::{PasswordHash, PasswordVerifier};
    let parsed = PasswordHash::new(hash.trim())
        .map_err(|e| EngineError::invalid(format!("Invalid argon2 hash string: {e}")))?;
    let valid = argon2::Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok();
    Ok(json!({ "valid": valid }))
}

/// Derives a key with PBKDF2-HMAC.
pub fn pbkdf2(
    password: &str,
    salt: &[u8],
    iterations: u32,
    length: usize,
    algorithm: &str,
    format: &str,
) -> EngineResult<Value> {
    if password.is_empty() {
        return Err(EngineError::empty("password"));
    }
    if !(1..=10_000_000).contains(&iterations) {
        return Err(EngineError::invalid("`iterations` must be between 1 and 10,000,000"));
    }
    if !(1..=1024).contains(&length) {
        return Err(EngineError::invalid("`length` must be between 1 and 1024 bytes"));
    }
    let mut out = vec![0u8; length];
    match normalise(algorithm).as_str() {
        "" | "sha256" => {
            pbkdf2::pbkdf2::<Hmac<sha2::Sha256>>(password.as_bytes(), salt, iterations, &mut out)
        },
        "sha1" => {
            pbkdf2::pbkdf2::<Hmac<sha1::Sha1>>(password.as_bytes(), salt, iterations, &mut out)
        },
        "sha512" => {
            pbkdf2::pbkdf2::<Hmac<sha2::Sha512>>(password.as_bytes(), salt, iterations, &mut out)
        },
        other => {
            return Err(EngineError::invalid(format!(
                "Unsupported PBKDF2 hash `{other}` (sha1, sha256, sha512)"
            )))
        },
    }
    .map_err(|e| EngineError::internal(e.to_string()))?;
    Ok(json!({
        "key": encode_output(&out, format)?,
        "salt": hex::encode(salt),
        "iterations": iterations,
        "length": length
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digests_known_answers() {
        let d = hash(
            b"abc",
            &[
                "md5".into(),
                "sha1".into(),
                "sha256".into(),
                "sha3-256".into(),
                "blake3".into(),
                "crc32".into(),
                "adler32".into(),
            ],
            "hex",
            Some("BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"),
        )
        .unwrap();
        assert_eq!(d["hashes"]["md5"], "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(d["hashes"]["sha1"], "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            d["hashes"]["sha256"],
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            d["hashes"]["sha3-256"],
            "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532"
        );
        assert_eq!(d["hashes"]["crc32"], "352441c2");
        assert_eq!(d["hashes"]["adler32"], "024d0127");
        assert_eq!(d["matches"], true);
        assert_eq!(d["matched_algorithm"], "sha256");
        assert!(digest("whirlpool", b"x").is_err());
        assert!(input_bytes(None, None).is_err());
        assert!(input_bytes(Some(""), Some(b"")).is_err());
        assert_eq!(encode_output(&[255, 1], "base64").unwrap(), "/wE=");
    }

    #[test]
    fn hmac_known_answer() {
        let h =
            hmac(b"The quick brown fox jumps over the lazy dog", b"key", "sha256", "hex").unwrap();
        assert_eq!(h["hmac"], "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8");
        let v = hmac_verify(
            b"The quick brown fox jumps over the lazy dog",
            b"key",
            "sha256",
            "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8",
        )
        .unwrap();
        assert_eq!(v["valid"], true);
        assert_eq!(hmac_verify(b"x", b"key", "sha256", "00").unwrap()["valid"], false);
        assert!(hmac(b"x", b"k", "crc32", "hex").is_err());
        assert_eq!(key_bytes("ff00", "hex").unwrap(), vec![255, 0]);
        assert!(key_bytes("zz", "hex").is_err());
    }

    #[test]
    fn password_hashing() {
        let b = bcrypt_hash("secret", 4).unwrap();
        assert!(b["hash"].as_str().unwrap().starts_with("$2"));
        assert_eq!(bcrypt_verify("secret", b["hash"].as_str().unwrap()).unwrap()["valid"], true);
        assert_eq!(bcrypt_verify("wrong", b["hash"].as_str().unwrap()).unwrap()["valid"], false);
        assert!(bcrypt_verify("x", "nope").is_err());
        assert!(bcrypt_hash("x", 3).is_err());
        let a = argon2_hash("secret", "argon2id", 64, 1, 1).unwrap();
        assert!(a["hash"].as_str().unwrap().starts_with("$argon2id$"));
        assert_eq!(argon2_verify("secret", a["hash"].as_str().unwrap()).unwrap()["valid"], true);
        assert_eq!(argon2_verify("nope", a["hash"].as_str().unwrap()).unwrap()["valid"], false);
        assert!(argon2_hash("x", "argon2x", 64, 1, 1).is_err());
        let p = pbkdf2("password", b"salt", 1, 20, "sha1", "hex").unwrap();
        assert_eq!(p["key"], "0c60c80f961f0e71f3a9b524af6012062fe037a6");
        assert!(pbkdf2("", b"salt", 1, 20, "sha1", "hex").is_err());
    }
}
