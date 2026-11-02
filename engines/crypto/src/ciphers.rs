//! Symmetric encryption (AES-256-GCM / ChaCha20-Poly1305 with PBKDF2 key
//! derivation), RSA key generation / signatures / encryption, JWT and TOTP.

use engine_shared::{b64, rng, EngineError, EngineResult};
use hmac::{Hmac, Mac};
use serde_json::{json, Map, Value};
use sha2::Sha256;

const MAGIC: &[u8; 4] = b"DTB1";
const PBKDF2_ITERATIONS: u32 = 210_000;

/// Copies a 12-byte slice into a fixed nonce array.
fn nonce12(n: &[u8]) -> EngineResult<[u8; 12]> {
    n.try_into()
        .map_err(|_| EngineError::invalid("Nonce must be 12 bytes"))
}

fn derive_key(password: &str, salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut key = [0u8; 32];
    let _ = pbkdf2::pbkdf2::<Hmac<Sha256>>(password.as_bytes(), salt, iterations, &mut key);
    key
}

/// Encrypts with a password. Output layout (base64):
/// `DTB1 | cipher id (1) | iterations (4 BE) | salt (16) | nonce (12) | ciphertext+tag`.
pub fn encrypt(
    plaintext: &[u8],
    password: &str,
    cipher: &str,
    iterations: Option<u32>,
) -> EngineResult<Value> {
    use aes_gcm::aead::{Aead, KeyInit, Payload};
    if plaintext.is_empty() {
        return Err(EngineError::empty("plaintext"));
    }
    if password.is_empty() {
        return Err(EngineError::empty("password"));
    }
    let iterations = iterations.unwrap_or(PBKDF2_ITERATIONS).clamp(1000, 5_000_000);
    let mut salt = [0u8; 16];
    rng::fill_bytes(&mut salt);
    let mut nonce = [0u8; 12];
    rng::fill_bytes(&mut nonce);
    let key = derive_key(password, &salt, iterations);
    let (cipher_id, name, ct) = match cipher.to_ascii_lowercase().as_str() {
        "" | "aes-256-gcm" | "aes-gcm" | "aes" => {
            let c = aes_gcm::Aes256Gcm::new(&key.into());
            (
                1u8,
                "AES-256-GCM",
                c.encrypt(&nonce.into(), Payload { msg: plaintext, aad: MAGIC })
                    .map_err(|_| EngineError::internal("encryption failed"))?,
            )
        },
        "chacha20-poly1305" | "chacha20" | "chacha" => {
            let c = chacha20poly1305::ChaCha20Poly1305::new(&key.into());
            (
                2u8,
                "ChaCha20-Poly1305",
                c.encrypt(&nonce.into(), Payload { msg: plaintext, aad: MAGIC })
                    .map_err(|_| EngineError::internal("encryption failed"))?,
            )
        },
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown cipher `{other}` (aes-256-gcm, chacha20-poly1305)"
            )))
        },
    };
    let mut out = Vec::with_capacity(4 + 1 + 4 + 16 + 12 + ct.len());
    out.extend_from_slice(MAGIC);
    out.push(cipher_id);
    out.extend_from_slice(&iterations.to_be_bytes());
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(json!({
        "ciphertext": b64::encode(&out),
        "cipher": name,
        "kdf": format!("PBKDF2-HMAC-SHA256 ({iterations} iterations)"),
        "bytes": out.len(),
        "secure": rng::has_caller_entropy(),
        "format": "DTB1 | cipher(1) | iterations(4) | salt(16) | nonce(12) | ciphertext+tag",
    }))
}

/// Decrypts data produced by [`encrypt`]; returns bytes and UTF-8 text when valid.
pub fn decrypt(ciphertext_b64: &str, password: &str) -> EngineResult<Value> {
    use aes_gcm::aead::{Aead, KeyInit, Payload};
    let data = b64::decode(ciphertext_b64)?;
    if data.len() < 4 + 1 + 4 + 16 + 12 + 16 {
        return Err(EngineError::malformed("Ciphertext is too short or not produced by this tool"));
    }
    if &data[..4] != MAGIC {
        return Err(EngineError::unsupported_format(
            "Unrecognised ciphertext container (expected the DTB1 format produced by `encrypt`)",
        ));
    }
    let cipher_id = data[4];
    let iterations = u32::from_be_bytes([data[5], data[6], data[7], data[8]]);
    let salt = &data[9..25];
    let nonce: [u8; 12] = nonce12(&data[25..37])?;
    let ct = &data[37..];
    let key = derive_key(password, salt, iterations);
    let plain = match cipher_id {
        1 => aes_gcm::Aes256Gcm::new(&key.into())
            .decrypt(&nonce.into(), Payload { msg: ct, aad: MAGIC }),
        2 => chacha20poly1305::ChaCha20Poly1305::new(&key.into())
            .decrypt(&nonce.into(), Payload { msg: ct, aad: MAGIC }),
        _ => return Err(EngineError::unsupported_format("Unknown cipher id in container")),
    }
    .map_err(|_| {
        EngineError::wrong_password("Decryption failed: wrong password or corrupted data")
    })?;
    let text = String::from_utf8(plain.clone()).ok();
    Ok(json!({
        "plaintext": b64::encode(&plain),
        "text": text,
        "bytes": plain.len(),
        "cipher": if cipher_id == 1 { "AES-256-GCM" } else { "ChaCha20-Poly1305" }
    }))
}

/// Raw AES-GCM with a caller supplied hex/base64 key (128/192/256-bit) and
/// optional nonce – for interoperability with other tools.
pub fn aes_gcm_raw(
    data: &[u8],
    key: &[u8],
    nonce: Option<&[u8]>,
    aad: &[u8],
    decrypt_mode: bool,
) -> EngineResult<Value> {
    use aes_gcm::aead::{Aead, KeyInit, Payload};
    if data.is_empty() {
        return Err(EngineError::empty("data"));
    }
    let mut nonce_bytes = [0u8; 12];
    let (payload, nonce_used): (&[u8], [u8; 12]) = if decrypt_mode {
        match nonce {
            Some(n) if n.len() == 12 => (data, nonce12(n)?),
            Some(_) => return Err(EngineError::invalid("Nonce must be 12 bytes")),
            None => {
                if data.len() < 12 + 16 {
                    return Err(EngineError::malformed(
                        "Data too short (expected nonce || ciphertext)",
                    ));
                }
                (&data[12..], nonce12(&data[..12])?)
            },
        }
    } else {
        match nonce {
            Some(n) if n.len() == 12 => (data, nonce12(n)?),
            Some(_) => return Err(EngineError::invalid("Nonce must be 12 bytes")),
            None => {
                rng::fill_bytes(&mut nonce_bytes);
                (data, nonce_bytes)
            },
        }
    };
    let result = match key.len() {
        16 => {
            let c = aes_gcm::Aes128Gcm::new(key.into());
            if decrypt_mode {
                c.decrypt(&nonce_used.into(), Payload { msg: payload, aad })
            } else {
                c.encrypt(&nonce_used.into(), Payload { msg: payload, aad })
            }
        },
        32 => {
            let c = aes_gcm::Aes256Gcm::new(key.into());
            if decrypt_mode {
                c.decrypt(&nonce_used.into(), Payload { msg: payload, aad })
            } else {
                c.encrypt(&nonce_used.into(), Payload { msg: payload, aad })
            }
        },
        n => return Err(EngineError::invalid(format!("Key must be 16 or 32 bytes (got {n})"))),
    }
    .map_err(|_| {
        if decrypt_mode {
            EngineError::wrong_password("Decryption failed (wrong key, nonce or tampered data)")
        } else {
            EngineError::internal("encryption failed")
        }
    })?;
    if decrypt_mode {
        Ok(json!({
            "plaintext": b64::encode(&result),
            "text": String::from_utf8(result.clone()).ok(),
            "bytes": result.len()
        }))
    } else {
        let mut combined = nonce_used.to_vec();
        combined.extend_from_slice(&result);
        Ok(json!({
            "ciphertext": b64::encode(&result),
            "nonce": hex::encode(nonce_used),
            "nonce_and_ciphertext": b64::encode(&combined),
            "tag_included": true,
            "secure": nonce.is_some() || rng::has_caller_entropy()
        }))
    }
}

// ----------------------------------------------------------------- RSA ----

use rsa::pkcs1v15::{
    Signature as Pkcs1Sig, SigningKey as Pkcs1Signing, VerifyingKey as Pkcs1Verifying,
};
use rsa::pkcs8::{
    DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey, LineEnding,
};
use rsa::pss::{Signature as PssSig, SigningKey as PssSigning, VerifyingKey as PssVerifying};
use rsa::signature::{RandomizedSigner, SignatureEncoding, Signer, Verifier};
use rsa::traits::PublicKeyParts;
use rsa::{RsaPrivateKey, RsaPublicKey};

struct PoolRng;
impl rsa::rand_core::RngCore for PoolRng {
    fn next_u32(&mut self) -> u32 {
        rng::next_u32()
    }
    fn next_u64(&mut self) -> u64 {
        rng::next_u64()
    }
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        rng::fill_bytes(dest)
    }
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rsa::rand_core::Error> {
        rng::fill_bytes(dest);
        Ok(())
    }
}
impl rsa::rand_core::CryptoRng for PoolRng {}

/// Generates an RSA key pair (2048–4096 bits) and returns PKCS#8/SPKI PEMs.
pub fn rsa_generate(bits: usize) -> EngineResult<Value> {
    if ![1024, 2048, 3072, 4096].contains(&bits) {
        return Err(EngineError::invalid("`bits` must be 1024, 2048, 3072 or 4096"));
    }
    rng::require_secure()?;
    let key = RsaPrivateKey::new(&mut PoolRng, bits)
        .map_err(|e| EngineError::internal(format!("RSA key generation failed: {e}")))?;
    let public = key.to_public_key();
    let private_pem = key
        .to_pkcs8_pem(LineEnding::LF)
        .map_err(|e| EngineError::internal(e.to_string()))?
        .to_string();
    let public_pem = public
        .to_public_key_pem(LineEnding::LF)
        .map_err(|e| EngineError::internal(e.to_string()))?;
    let private_pkcs1 = rsa::pkcs1::EncodeRsaPrivateKey::to_pkcs1_pem(&key, LineEnding::LF)
        .map_err(|e| EngineError::internal(e.to_string()))?
        .to_string();
    let public_pkcs1 = rsa::pkcs1::EncodeRsaPublicKey::to_pkcs1_pem(&public, LineEnding::LF)
        .map_err(|e| EngineError::internal(e.to_string()))?;
    Ok(json!({
        "private_key_pem": private_pem,
        "public_key_pem": public_pem,
        "private_key_pkcs1_pem": private_pkcs1,
        "public_key_pkcs1_pem": public_pkcs1,
        "bits": bits,
        "modulus_hex": hex::encode(public.n().to_bytes_be()),
        "exponent": public.e().to_string(),
        "warning": if bits < 2048 { json!("1024-bit RSA is insecure; use 2048 bits or more") } else { Value::Null },
    }))
}

fn parse_private(pem: &str) -> EngineResult<RsaPrivateKey> {
    let t = pem.trim();
    RsaPrivateKey::from_pkcs8_pem(t)
        .or_else(|_| rsa::pkcs1::DecodeRsaPrivateKey::from_pkcs1_pem(t))
        .map_err(|e| EngineError::invalid(format!("Invalid RSA private key PEM: {e}")))
}

fn parse_public(pem: &str) -> EngineResult<RsaPublicKey> {
    let t = pem.trim();
    if let Ok(k) = RsaPublicKey::from_public_key_pem(t) {
        return Ok(k);
    }
    if let Ok(k) = rsa::pkcs1::DecodeRsaPublicKey::from_pkcs1_pem(t) {
        return Ok(k);
    }
    // Allow passing a private key to derive the public key.
    parse_private(t).map(|k| k.to_public_key()).map_err(|_| {
        EngineError::invalid(
            "Invalid RSA public key PEM (expected SPKI `PUBLIC KEY` or PKCS#1 `RSA PUBLIC KEY`)",
        )
    })
}

/// Signs data with an RSA private key (`pkcs1v15` or `pss`, SHA-256/384/512).
pub fn rsa_sign(
    private_pem: &str,
    message: &[u8],
    scheme: &str,
    hash: &str,
) -> EngineResult<Value> {
    if message.is_empty() {
        return Err(EngineError::empty("message"));
    }
    let key = parse_private(private_pem)?;
    let sig = match (scheme.to_ascii_lowercase().as_str(), hash.to_ascii_lowercase().as_str()) {
        ("" | "pkcs1v15" | "pkcs1", "" | "sha256") => {
            Pkcs1Signing::<Sha256>::new(key).sign(message).to_vec()
        },
        ("" | "pkcs1v15" | "pkcs1", "sha384") => {
            Pkcs1Signing::<sha2::Sha384>::new(key).sign(message).to_vec()
        },
        ("" | "pkcs1v15" | "pkcs1", "sha512") => {
            Pkcs1Signing::<sha2::Sha512>::new(key).sign(message).to_vec()
        },
        ("pss", "" | "sha256") => PssSigning::<Sha256>::new(key)
            .sign_with_rng(&mut PoolRng, message)
            .to_vec(),
        ("pss", "sha384") => PssSigning::<sha2::Sha384>::new(key)
            .sign_with_rng(&mut PoolRng, message)
            .to_vec(),
        ("pss", "sha512") => PssSigning::<sha2::Sha512>::new(key)
            .sign_with_rng(&mut PoolRng, message)
            .to_vec(),
        (s, h) => {
            return Err(EngineError::invalid(format!(
                "Unsupported scheme/hash `{s}`/`{h}` (pkcs1v15|pss with sha256|sha384|sha512)"
            )))
        },
    };
    Ok(json!({
        "signature": b64::encode(&sig),
        "signature_hex": hex::encode(&sig),
        "scheme": if scheme.is_empty() { "pkcs1v15" } else { scheme },
        "hash": if hash.is_empty() { "sha256" } else { hash }
    }))
}

/// Verifies an RSA signature.
pub fn rsa_verify(
    public_pem: &str,
    message: &[u8],
    signature: &str,
    scheme: &str,
    hash: &str,
) -> EngineResult<Value> {
    let key = parse_public(public_pem)?;
    let sig = b64::decode(signature).or_else(|_| {
        hex::decode(signature.trim())
            .map_err(|_| EngineError::invalid("Signature must be base64 or hex"))
    })?;
    let valid = match (scheme.to_ascii_lowercase().as_str(), hash.to_ascii_lowercase().as_str()) {
        ("" | "pkcs1v15" | "pkcs1", "" | "sha256") => Pkcs1Sig::try_from(sig.as_slice())
            .map(|s| Pkcs1Verifying::<Sha256>::new(key).verify(message, &s).is_ok())
            .unwrap_or(false),
        ("" | "pkcs1v15" | "pkcs1", "sha384") => Pkcs1Sig::try_from(sig.as_slice())
            .map(|s| Pkcs1Verifying::<sha2::Sha384>::new(key).verify(message, &s).is_ok())
            .unwrap_or(false),
        ("" | "pkcs1v15" | "pkcs1", "sha512") => Pkcs1Sig::try_from(sig.as_slice())
            .map(|s| Pkcs1Verifying::<sha2::Sha512>::new(key).verify(message, &s).is_ok())
            .unwrap_or(false),
        ("pss", "" | "sha256") => PssSig::try_from(sig.as_slice())
            .map(|s| PssVerifying::<Sha256>::new(key).verify(message, &s).is_ok())
            .unwrap_or(false),
        ("pss", "sha384") => PssSig::try_from(sig.as_slice())
            .map(|s| PssVerifying::<sha2::Sha384>::new(key).verify(message, &s).is_ok())
            .unwrap_or(false),
        ("pss", "sha512") => PssSig::try_from(sig.as_slice())
            .map(|s| PssVerifying::<sha2::Sha512>::new(key).verify(message, &s).is_ok())
            .unwrap_or(false),
        (s, h) => return Err(EngineError::invalid(format!("Unsupported scheme/hash `{s}`/`{h}`"))),
    };
    Ok(json!({ "valid": valid }))
}

/// Encrypts a short message with RSA-OAEP (SHA-256).
pub fn rsa_encrypt(public_pem: &str, message: &[u8]) -> EngineResult<Value> {
    if message.is_empty() {
        return Err(EngineError::empty("message"));
    }
    let key = parse_public(public_pem)?;
    let max = key.size() - 2 * 32 - 2;
    if message.len() > max {
        return Err(EngineError::invalid(format!(
            "RSA-OAEP can encrypt at most {max} bytes with this key; encrypt large data with AES \
             and wrap the key"
        )));
    }
    let ct = key
        .encrypt(&mut PoolRng, rsa::Oaep::new::<Sha256>(), message)
        .map_err(|e| EngineError::internal(e.to_string()))?;
    Ok(json!({ "ciphertext": b64::encode(&ct), "scheme": "RSA-OAEP-SHA256", "bytes": ct.len() }))
}

/// Decrypts an RSA-OAEP ciphertext.
pub fn rsa_decrypt(private_pem: &str, ciphertext: &str) -> EngineResult<Value> {
    let key = parse_private(private_pem)?;
    let ct = b64::decode(ciphertext)?;
    let plain = key.decrypt(rsa::Oaep::new::<Sha256>(), &ct).map_err(|_| {
        EngineError::wrong_password("RSA decryption failed (wrong key or corrupted ciphertext)")
    })?;
    Ok(json!({
        "plaintext": b64::encode(&plain),
        "text": String::from_utf8(plain.clone()).ok(),
        "bytes": plain.len()
    }))
}

/// Reports key size, exponent and fingerprint of a PEM key.
pub fn rsa_inspect(pem: &str) -> EngineResult<Value> {
    if let Ok(k) = parse_private(pem) {
        let public = k.to_public_key();
        return Ok(json!({
            "kind": "private",
            "bits": public.n().bits(),
            "exponent": public.e().to_string(),
            "public_key_pem": public.to_public_key_pem(LineEnding::LF).unwrap_or_default()
        }));
    }
    let k = parse_public(pem)?;
    Ok(json!({
        "kind": "public",
        "bits": k.n().bits(),
        "exponent": k.e().to_string(),
        "modulus_hex": hex::encode(k.n().to_bytes_be())
    }))
}

// ----------------------------------------------------------------- JWT ----

fn b64url(data: &[u8]) -> String {
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, data)
}

fn b64url_decode(s: &str) -> EngineResult<Vec<u8>> {
    b64::decode(s).map_err(|_| EngineError::malformed("JWT segment is not valid base64url"))
}

fn unix_to_iso(t: i64) -> String {
    let days = t.div_euclid(86400);
    let secs = t.rem_euclid(86400);
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

/// Decodes (without verifying) a JWT. `now_unix` enables expiry checks.
pub fn jwt_decode(token: &str, now_unix: Option<i64>) -> EngineResult<Value> {
    let t = token.trim().trim_start_matches("Bearer ").trim();
    if t.is_empty() {
        return Err(EngineError::empty("token"));
    }
    let parts: Vec<&str> = t.split('.').collect();
    if parts.len() != 3 && parts.len() != 5 {
        return Err(EngineError::malformed(format!(
            "A JWT has 3 dot-separated segments (JWE has 5); found {}",
            parts.len()
        )));
    }
    if parts.len() == 5 {
        let header: Value = serde_json::from_slice(&b64url_decode(parts[0])?)
            .map_err(|_| EngineError::malformed("JWE header is not valid JSON"))?;
        return Ok(json!({
            "type": "JWE",
            "header": header,
            "note": "Encrypted token; payload cannot be read without the decryption key"
        }));
    }
    let header: Value = serde_json::from_slice(&b64url_decode(parts[0])?)
        .map_err(|_| EngineError::malformed("JWT header is not valid JSON"))?;
    let payload: Value = serde_json::from_slice(&b64url_decode(parts[1])?)
        .map_err(|_| EngineError::malformed("JWT payload is not valid JSON"))?;
    let sig = b64url_decode(parts[2]).unwrap_or_default();
    let mut claims = Map::new();
    let mut status = Map::new();
    for (k, label) in [("exp", "expires_at"), ("iat", "issued_at"), ("nbf", "not_before")] {
        if let Some(ts) = payload.get(k).and_then(Value::as_i64) {
            claims.insert(label.into(), json!(unix_to_iso(ts)));
            if let Some(now) = now_unix {
                match k {
                    "exp" => {
                        status.insert("expired".into(), json!(now >= ts));
                        status.insert("expires_in_seconds".into(), json!(ts - now));
                    },
                    "nbf" => {
                        status.insert("not_yet_valid".into(), json!(now < ts));
                    },
                    _ => {},
                }
            }
        }
    }
    let alg = header.get("alg").and_then(Value::as_str).unwrap_or("").to_string();
    if alg.eq_ignore_ascii_case("none") {
        status.insert("warning".into(), json!("alg=none: token is unsigned"));
    }
    Ok(json!({
        "type": "JWS",
        "header": header,
        "payload": payload,
        "algorithm": alg,
        "signature_base64url": parts[2],
        "signature_bytes": sig.len(),
        "claims": claims,
        "status": status,
        "segments": {
            "header_bytes": parts[0].len(),
            "payload_bytes": parts[1].len(),
            "signature_bytes": parts[2].len()
        },
    }))
}

fn hs_sign(alg: &str, key: &[u8], input: &[u8]) -> EngineResult<Vec<u8>> {
    Ok(match alg {
        "HS256" => {
            let mut m = <Hmac<Sha256>>::new_from_slice(key)
                .map_err(|e| EngineError::invalid(e.to_string()))?;
            m.update(input);
            m.finalize().into_bytes().to_vec()
        },
        "HS384" => {
            let mut m = <Hmac<sha2::Sha384>>::new_from_slice(key)
                .map_err(|e| EngineError::invalid(e.to_string()))?;
            m.update(input);
            m.finalize().into_bytes().to_vec()
        },
        "HS512" => {
            let mut m = <Hmac<sha2::Sha512>>::new_from_slice(key)
                .map_err(|e| EngineError::invalid(e.to_string()))?;
            m.update(input);
            m.finalize().into_bytes().to_vec()
        },
        other => {
            return Err(EngineError::invalid(format!(
                "Unsupported algorithm `{other}` (HS256, HS384, HS512, RS256, RS384, RS512)"
            )))
        },
    })
}

/// Creates a signed JWT (HS256/384/512 with a secret, RS256/384/512 with a PEM key).
pub fn jwt_sign(
    payload: &Value,
    secret: &str,
    algorithm: &str,
    extra_header: Option<&Value>,
) -> EngineResult<Value> {
    let alg = if algorithm.is_empty() {
        "HS256".to_string()
    } else {
        algorithm.to_uppercase()
    };
    if secret.is_empty() {
        return Err(EngineError::empty("secret / private key"));
    }
    if !payload.is_object() {
        return Err(EngineError::invalid("JWT payload must be a JSON object"));
    }
    let mut header = Map::new();
    header.insert("alg".into(), json!(alg));
    header.insert("typ".into(), json!("JWT"));
    if let Some(Value::Object(extra)) = extra_header {
        for (k, v) in extra {
            header.insert(k.clone(), v.clone());
        }
    }
    let h = b64url(serde_json::to_string(&Value::Object(header.clone()))?.as_bytes());
    let p = b64url(serde_json::to_string(payload)?.as_bytes());
    let input = format!("{h}.{p}");
    let sig = if alg.starts_with("HS") {
        hs_sign(&alg, secret.as_bytes(), input.as_bytes())?
    } else if alg.starts_with("RS") {
        let hash = match alg.as_str() {
            "RS256" => "sha256",
            "RS384" => "sha384",
            "RS512" => "sha512",
            _ => return Err(EngineError::invalid("Unsupported RS algorithm")),
        };
        let r = rsa_sign(secret, input.as_bytes(), "pkcs1v15", hash)?;
        b64::decode(r["signature"].as_str().unwrap_or_default())?
    } else {
        return Err(EngineError::invalid(format!(
            "Unsupported algorithm `{alg}` (HS256/384/512, RS256/384/512)"
        )));
    };
    let token = format!("{input}.{}", b64url(&sig));
    Ok(json!({ "token": token, "header": header, "algorithm": alg, "length": token.len() }))
}

/// Verifies a JWT signature (HS* with secret, RS* with public key PEM) and
/// optionally its time claims.
pub fn jwt_verify(
    token: &str,
    secret: &str,
    now_unix: Option<i64>,
    leeway: i64,
) -> EngineResult<Value> {
    let decoded = jwt_decode(token, now_unix)?;
    if decoded["type"] == "JWE" {
        return Err(EngineError::unsupported("JWE tokens cannot be verified here"));
    }
    let t = token.trim().trim_start_matches("Bearer ").trim();
    let parts: Vec<&str> = t.split('.').collect();
    let alg = decoded["algorithm"].as_str().unwrap_or("").to_uppercase();
    let input = format!("{}.{}", parts[0], parts[1]);
    let provided = b64url_decode(parts[2])?;
    let signature_valid = if alg.starts_with("HS") {
        if secret.is_empty() {
            return Err(EngineError::empty("secret"));
        }
        let expected = hs_sign(&alg, secret.as_bytes(), input.as_bytes())?;
        use subtle::ConstantTimeEq;
        expected.len() == provided.len() && expected.ct_eq(&provided).into()
    } else if alg.starts_with("RS") {
        let hash = match alg.as_str() {
            "RS256" => "sha256",
            "RS384" => "sha384",
            "RS512" => "sha512",
            _ => return Err(EngineError::invalid("Unsupported RS algorithm")),
        };
        rsa_verify(secret, input.as_bytes(), &b64::encode(&provided), "pkcs1v15", hash)?["valid"]
            .as_bool()
            .unwrap_or(false)
    } else if alg == "NONE" {
        false
    } else {
        return Err(EngineError::unsupported(format!(
            "Algorithm `{alg}` verification is not supported (HS*, RS*)"
        )));
    };
    let payload = &decoded["payload"];
    let mut reasons: Vec<String> = Vec::new();
    if !signature_valid {
        reasons.push("signature mismatch".into());
    }
    if let Some(now) = now_unix {
        if let Some(exp) = payload.get("exp").and_then(Value::as_i64) {
            if now - leeway >= exp {
                reasons.push("token expired".into());
            }
        }
        if let Some(nbf) = payload.get("nbf").and_then(Value::as_i64) {
            if now + leeway < nbf {
                reasons.push("token not yet valid (nbf)".into());
            }
        }
    }
    Ok(json!({
        "valid": reasons.is_empty(),
        "signature_valid": signature_valid,
        "reasons": reasons,
        "algorithm": alg,
        "payload": payload,
        "header": decoded["header"]
    }))
}

// ---------------------------------------------------------------- TOTP ----

/// Decodes RFC 4648 base32 (case-insensitive, padding optional).
pub fn base32_decode(s: &str) -> EngineResult<Vec<u8>> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let clean: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-' && *c != '=')
        .collect::<String>()
        .to_uppercase();
    if clean.is_empty() {
        return Err(EngineError::empty("secret"));
    }
    let mut out = Vec::new();
    let mut buffer: u64 = 0;
    let mut bits = 0;
    for c in clean.bytes() {
        let v = ALPHABET.iter().position(|&a| a == c).ok_or_else(|| {
            EngineError::invalid(format!("Invalid base32 character `{}`", c as char))
        })?;
        buffer = (buffer << 5) | v as u64;
        bits += 5;
        if bits >= 8 {
            out.push((buffer >> (bits - 8)) as u8);
            bits -= 8;
        }
    }
    Ok(out)
}

/// Encodes bytes as RFC 4648 base32.
pub fn base32_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut out = String::new();
    let mut buffer: u64 = 0;
    let mut bits = 0;
    for &b in data {
        buffer = (buffer << 8) | b as u64;
        bits += 8;
        while bits >= 5 {
            out.push(ALPHABET[((buffer >> (bits - 5)) & 31) as usize] as char);
            bits -= 5;
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((buffer << (5 - bits)) & 31) as usize] as char);
    }
    out
}

fn hotp_code(secret: &[u8], counter: u64, digits: u32, algorithm: &str) -> EngineResult<String> {
    let mac = match algorithm.to_ascii_uppercase().as_str() {
        "" | "SHA1" => {
            let mut m = <Hmac<sha1::Sha1>>::new_from_slice(secret)
                .map_err(|e| EngineError::invalid(e.to_string()))?;
            m.update(&counter.to_be_bytes());
            m.finalize().into_bytes().to_vec()
        },
        "SHA256" => {
            let mut m = <Hmac<Sha256>>::new_from_slice(secret)
                .map_err(|e| EngineError::invalid(e.to_string()))?;
            m.update(&counter.to_be_bytes());
            m.finalize().into_bytes().to_vec()
        },
        "SHA512" => {
            let mut m = <Hmac<sha2::Sha512>>::new_from_slice(secret)
                .map_err(|e| EngineError::invalid(e.to_string()))?;
            m.update(&counter.to_be_bytes());
            m.finalize().into_bytes().to_vec()
        },
        other => {
            return Err(EngineError::invalid(format!(
                "Unsupported TOTP algorithm `{other}` (SHA1, SHA256, SHA512)"
            )))
        },
    };
    let offset = (mac[mac.len() - 1] & 0x0f) as usize;
    let code = ((mac[offset] as u32 & 0x7f) << 24)
        | ((mac[offset + 1] as u32) << 16)
        | ((mac[offset + 2] as u32) << 8)
        | mac[offset + 3] as u32;
    let modulo = 10u32.pow(digits);
    Ok(format!("{:0width$}", code % modulo, width = digits as usize))
}

/// TOTP code for `timestamp_unix` (seconds) with previous/next window codes.
pub fn totp(
    secret: &str,
    timestamp_unix: i64,
    digits: u32,
    period: u32,
    algorithm: &str,
    window: u32,
) -> EngineResult<Value> {
    if !(6..=8).contains(&digits) {
        return Err(EngineError::invalid("`digits` must be 6, 7 or 8"));
    }
    if !(15..=300).contains(&period) {
        return Err(EngineError::invalid("`period` must be between 15 and 300 seconds"));
    }
    let key = base32_decode(secret)?;
    let counter = (timestamp_unix / period as i64).max(0) as u64;
    let code = hotp_code(&key, counter, digits, algorithm)?;
    let remaining = period as i64 - timestamp_unix.rem_euclid(period as i64);
    let mut neighbours = Vec::new();
    for offset in 1..=window.min(5) as u64 {
        neighbours.push(json!({
            "offset": -(offset as i64),
            "code": hotp_code(&key, counter.saturating_sub(offset), digits, algorithm)?
        }));
        neighbours.push(json!({
            "offset": offset,
            "code": hotp_code(&key, counter + offset, digits, algorithm)?
        }));
    }
    Ok(json!({
        "code": code,
        "remaining_seconds": remaining,
        "counter": counter,
        "period": period,
        "digits": digits,
        "algorithm": if algorithm.is_empty() { "SHA1".to_string() } else { algorithm.to_uppercase() },
        "neighbours": neighbours
    }))
}

/// HMAC-based one-time password for a counter value.
pub fn hotp(secret: &str, counter: u64, digits: u32, algorithm: &str) -> EngineResult<Value> {
    if !(6..=8).contains(&digits) {
        return Err(EngineError::invalid("`digits` must be 6, 7 or 8"));
    }
    let key = base32_decode(secret)?;
    Ok(json!({ "code": hotp_code(&key, counter, digits, algorithm)?, "counter": counter }))
}

/// Builds an `otpauth://` URI (for QR enrolment) and optionally a fresh secret.
pub fn totp_uri(
    issuer: &str,
    account: &str,
    secret: Option<&str>,
    digits: u32,
    period: u32,
    algorithm: &str,
) -> EngineResult<Value> {
    if account.trim().is_empty() {
        return Err(EngineError::empty("account"));
    }
    let secret_b32 = match secret.filter(|s| !s.trim().is_empty()) {
        Some(s) => {
            base32_decode(s)?;
            s.trim().to_uppercase().replace([' ', '-'], "")
        },
        None => {
            let mut raw = [0u8; 20];
            rng::fill_bytes(&mut raw);
            base32_encode(&raw)
        },
    };
    let enc = |s: &str| {
        s.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~') {
                    c.to_string()
                } else {
                    c.to_string().bytes().map(|b| format!("%{b:02X}")).collect()
                }
            })
            .collect::<String>()
    };
    let label = if issuer.trim().is_empty() {
        enc(account.trim())
    } else {
        format!("{}:{}", enc(issuer.trim()), enc(account.trim()))
    };
    let mut uri = format!(
        "otpauth://totp/{label}?secret={secret_b32}&digits={digits}&period={period}&algorithm={}",
        if algorithm.is_empty() {
            "SHA1".to_string()
        } else {
            algorithm.to_uppercase()
        }
    );
    if !issuer.trim().is_empty() {
        uri.push_str(&format!("&issuer={}", enc(issuer.trim())));
    }
    Ok(json!({
        "uri": uri,
        "secret": secret_b32,
        "secure": secret.is_some() || rng::has_caller_entropy()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_encryption_roundtrip() {
        for cipher in ["aes-256-gcm", "chacha20-poly1305"] {
            let e = encrypt(b"hello world", "pw", cipher, Some(1000)).unwrap();
            let d = decrypt(e["ciphertext"].as_str().unwrap(), "pw").unwrap();
            assert_eq!(d["text"], "hello world");
            assert_eq!(
                decrypt(e["ciphertext"].as_str().unwrap(), "nope").unwrap_err().code,
                engine_shared::error::ErrorCode::WrongPassword
            );
        }
        assert!(encrypt(b"", "pw", "aes", None).is_err());
        assert!(encrypt(b"x", "pw", "des", None).is_err());
        assert!(decrypt("AAAA", "pw").is_err());
        let key = [7u8; 32];
        let raw = aes_gcm_raw(b"secret", &key, None, b"", false).unwrap();
        let combined = b64::decode(raw["nonce_and_ciphertext"].as_str().unwrap()).unwrap();
        let back = aes_gcm_raw(&combined, &key, None, b"", true).unwrap();
        assert_eq!(back["text"], "secret");
        assert!(aes_gcm_raw(b"x", &[1u8; 5], None, b"", false).is_err());
    }

    #[test]
    fn rsa_cycle() {
        engine_shared::rng::seed_from_base64(&b64::encode(&[3u8; 32])).unwrap();
        let k = rsa_generate(1024).unwrap();
        let private = k["private_key_pem"].as_str().unwrap();
        let public = k["public_key_pem"].as_str().unwrap();
        assert!(private.contains("BEGIN PRIVATE KEY"));
        let s = rsa_sign(private, b"msg", "pkcs1v15", "sha256").unwrap();
        assert_eq!(
            rsa_verify(public, b"msg", s["signature"].as_str().unwrap(), "pkcs1v15", "sha256")
                .unwrap()["valid"],
            true
        );
        assert_eq!(
            rsa_verify(public, b"other", s["signature"].as_str().unwrap(), "pkcs1v15", "sha256")
                .unwrap()["valid"],
            false
        );
        let ps = rsa_sign(private, b"msg", "pss", "sha256").unwrap();
        assert_eq!(
            rsa_verify(public, b"msg", ps["signature"].as_str().unwrap(), "pss", "sha256").unwrap()
                ["valid"],
            true
        );
        let e = rsa_encrypt(public, b"tiny").unwrap();
        assert_eq!(
            rsa_decrypt(private, e["ciphertext"].as_str().unwrap()).unwrap()["text"],
            "tiny"
        );
        assert!(rsa_encrypt(public, &[0u8; 500]).is_err());
        assert_eq!(rsa_inspect(public).unwrap()["kind"], "public");
        assert_eq!(rsa_inspect(private).unwrap()["bits"], 1024);
        assert!(rsa_inspect("junk").is_err());
        assert!(rsa_generate(512).is_err());
    }

    #[test]
    fn jwt_and_totp() {
        let payload = json!({ "sub": "123", "exp": 1_700_000_000, "iat": 1_699_990_000 });
        let t = jwt_sign(&payload, "secret", "HS256", None).unwrap();
        let token = t["token"].as_str().unwrap();
        let d = jwt_decode(token, Some(1_699_999_000)).unwrap();
        assert_eq!(d["payload"]["sub"], "123");
        assert_eq!(d["status"]["expired"], false);
        assert_eq!(d["claims"]["expires_at"], "2023-11-14T22:13:20Z");
        assert_eq!(jwt_verify(token, "secret", Some(1_699_999_000), 0).unwrap()["valid"], true);
        assert_eq!(
            jwt_verify(token, "wrong", Some(1_699_999_000), 0).unwrap()["signature_valid"],
            false
        );
        let v = jwt_verify(token, "secret", Some(1_800_000_000), 0).unwrap();
        assert_eq!(v["valid"], false);
        assert!(v["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r.as_str().unwrap().contains("expired")));
        assert!(jwt_decode("a.b", None).is_err());
        assert!(jwt_decode("", None).is_err());
        assert!(jwt_sign(&json!([1]), "s", "HS256", None).is_err());
        assert_eq!(jwt_sign(&payload, "s", "HS512", None).unwrap()["algorithm"], "HS512");
        // RFC 6238 test vector (SHA1, secret "12345678901234567890" = GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ) at t=59 → 94287082
        let code = totp("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 59, 8, 30, "SHA1", 1).unwrap();
        assert_eq!(code["code"], "94287082");
        assert_eq!(code["remaining_seconds"], 1);
        let c256 =
            totp("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZA", 59, 8, 30, "SHA256", 0)
                .unwrap();
        assert_eq!(c256["code"], "46119246");
        assert_eq!(
            hotp("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 0, 6, "SHA1").unwrap()["code"],
            "755224"
        );
        assert!(totp("!!!", 59, 6, 30, "SHA1", 0).is_err());
        let u = totp_uri("Dev Toolbox", "alice@example.com", None, 6, 30, "").unwrap();
        assert!(u["uri"]
            .as_str()
            .unwrap()
            .starts_with("otpauth://totp/Dev%20Toolbox:alice%40example.com?secret="));
        assert_eq!(base32_decode(&base32_encode(b"hello")).unwrap(), b"hello");
    }
}
