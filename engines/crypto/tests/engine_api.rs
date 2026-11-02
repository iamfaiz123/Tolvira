//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crypto_engine::process_request;
use engine_shared::b64;
use serde_json::{json, Value};

fn call(action: &str, payload: Value) -> Value {
    serde_json::from_slice(&process_request(
        json!({ "action": action, "payload": payload }).to_string().as_bytes(),
    ))
    .unwrap()
}
fn ok(action: &str, payload: Value) -> Value {
    let r = call(action, payload);
    assert_eq!(r["success"], true, "{action}: {r}");
    r["data"].clone()
}
fn err(action: &str, payload: Value) -> String {
    let r = call(action, payload);
    assert_eq!(r["success"], false, "{action}: {r}");
    r["error_code"].as_str().unwrap().to_string()
}

#[test]
fn hashing_and_keys() {
    let h = ok("hash", json!({ "text": "abc", "algorithms": ["sha256", "md5"] }));
    assert_eq!(h["hashes"]["md5"], "900150983cd24fb0d6963f7d28e17f72");
    let f = ok(
        "hash",
        json!({
            "file": b64::encode(b"abc"),
            "algorithms": ["sha1"],
            "expected": "a9993e364706816aba3e25717850c26c9cd0d89d"
        }),
    );
    assert_eq!(f["matches"], true);
    assert_eq!(err("hash", json!({ "text": "" })), "E_EMPTY_INPUT");
    assert_eq!(err("hash", json!({ "text": "x", "algorithms": ["rot13"] })), "E_INVALID_INPUT");
    assert_eq!(ok("hmac", json!({ "text": "msg", "key": "k" }))["algorithm"], "HMAC-SHA256");
    assert_eq!(err("hmac", json!({ "text": "msg", "key": "" })), "E_EMPTY_INPUT");
    let b = ok("bcrypt_hash", json!({ "password": "pw", "cost": 4 }));
    assert_eq!(
        ok("bcrypt_verify", json!({ "password": "pw", "hash": b["hash"] }))["valid"],
        true
    );
    let a = ok("argon2_hash", json!({ "password": "pw", "m_cost_kib": 64, "t_cost": 1 }));
    assert_eq!(
        ok("argon2_verify", json!({ "password": "pw", "hash": a["hash"] }))["valid"],
        true
    );
    assert_eq!(
        ok("pbkdf2", json!({ "password": "pw", "salt": "salt", "iterations": 1000 }))["length"],
        32
    );
}

#[test]
fn generators_and_ciphers() {
    let entropy = b64::encode(&[42u8; 32]);
    let p = ok("password", json!({ "length": 20, "entropy": entropy }));
    assert_eq!(p["secure"], true);
    assert_eq!(p["password"].as_str().unwrap().chars().count(), 20);
    assert_eq!(err("password", json!({ "length": 1 })), "E_INVALID_INPUT");
    assert_eq!(
        ok("passphrase", json!({ "words": 5 }))["passphrase"]
            .as_str()
            .unwrap()
            .split('-')
            .count(),
        5
    );
    assert_eq!(ok("password_strength", json!({ "password": "123456" }))["score"], 0);
    assert_eq!(err("password_strength", json!({ "password": "" })), "E_EMPTY_INPUT");
    assert_eq!(ok("random_bytes", json!({ "length": 8 }))["bits"], 64);
    assert_eq!(
        ok("uuid", json!({ "version": "4", "count": 2 }))["uuids"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(err("uuid", json!({ "version": "7" })), "E_INVALID_INPUT");
    assert_eq!(
        ok("ulid", json!({ "timestamp_ms": 1700000000000u64 }))["ulid"]
            .as_str()
            .unwrap()
            .len(),
        26
    );
    assert_eq!(ok("nanoid", json!({}))["id"].as_str().unwrap().len(), 21);
    let e = ok("encrypt", json!({ "text": "top secret", "password": "pw", "iterations": 1000 }));
    assert_eq!(
        ok("decrypt", json!({ "ciphertext": e["ciphertext"], "password": "pw" }))["text"],
        "top secret"
    );
    assert_eq!(
        err("decrypt", json!({ "ciphertext": e["ciphertext"], "password": "bad" })),
        "E_WRONG_PASSWORD"
    );
    assert_eq!(err("decrypt", json!({ "ciphertext": "AAAA", "password": "pw" })), "E_MALFORMED");
    let g = ok("aes_gcm", json!({ "text": "hi", "key": "00".repeat(32) }));
    assert_eq!(
        ok(
            "aes_gcm",
            json!({ "text": g["nonce_and_ciphertext"], "key": "00".repeat(32), "decrypt": true })
        )["text"],
        "hi"
    );
}

#[test]
fn rsa_generation_requires_entropy() {
    // Fresh thread => fresh RNG pool without caller entropy.
    assert_eq!(err("rsa_generate", json!({ "bits": 1024 })), "E_ENTROPY_REQUIRED");
    let k = ok("rsa_generate", json!({ "bits": 1024, "entropy": b64::encode(&[5u8; 32]) }));
    assert!(k["public_key_pem"].as_str().unwrap().contains("BEGIN PUBLIC KEY"));
}

#[test]
fn jwt_totp_certs() {
    let t = ok(
        "jwt_sign",
        json!({ "payload": { "sub": "u1", "exp": 2000000000 }, "secret": "s3cret" }),
    );
    let token = t["token"].as_str().unwrap();
    assert_eq!(
        ok("jwt_decode", json!({ "token": token, "now_unix": 1900000000 }))["status"]["expired"],
        false
    );
    assert_eq!(
        ok(
            "jwt_verify",
            json!({ "token": token, "secret": "s3cret", "now_unix": 1900000000 })
        )["valid"],
        true
    );
    assert_eq!(ok("jwt_verify", json!({ "token": token, "secret": "nope" }))["valid"], false);
    assert_eq!(err("jwt_decode", json!({ "token": "abc" })), "E_MALFORMED");
    assert_eq!(
        ok(
            "totp",
            json!({
                "secret": "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ",
                "timestamp_unix": 59,
                "digits": 8
            })
        )["code"],
        "94287082"
    );
    assert_eq!(err("totp", json!({ "secret": "", "timestamp_unix": 59 })), "E_EMPTY_INPUT");
    assert!(ok("totp_uri", json!({ "issuer": "X", "account": "a@b.c" }))["uri"]
        .as_str()
        .unwrap()
        .starts_with("otpauth://"));
    let cert = include_str!("../assets/test_cert.pem");
    assert_eq!(
        ok("certificate_decode", json!({ "certificate": cert }))["subject"]["CN"],
        "example.test"
    );
    assert_eq!(ok("pem_parse", json!({ "pem": cert }))["count"], 1);
    assert_eq!(err("certificate_decode", json!({ "certificate": "" })), "E_EMPTY_INPUT");
    assert_eq!(err("pem_parse", json!({ "pem": "nothing" })), "E_UNSUPPORTED_FORMAT");
}
