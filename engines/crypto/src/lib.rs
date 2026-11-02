//! # Dev Toolbox Cryptography Engine (`crypto-engine`)
//!
//! Security notes:
//! * Randomness comes from the shared ChaCha20 pool. Pass `entropy` (base64
//!   of ≥16 bytes from `crypto.getRandomValues`) for cryptographically secure
//!   output; responses report `secure: true|false` accordingly. Key
//!   generation (`rsa_generate`) refuses to run without caller entropy.
//! * The engine has no clock: pass `now_unix` / `timestamp_unix` for
//!   expiry checks, TOTP and time based identifiers.
//! * Client-side hashing is for verification/learning only; never treat a
//!   browser demo as a password store.
//!
//! | Action | Tool(s) |
//! |---|---|
//! | `hash` | md5/sha1/sha256/sha512/sha3/blake3 generators, hash-calculator, file-hash-checker, checksum verifier |
//! | `hmac` / `hmac_verify` | hmac-generator, hmac-verifier |
//! | `bcrypt_hash` / `bcrypt_verify` / `argon2_hash` / `argon2_verify` / `pbkdf2` | password hashing |
//! | `password` / `passphrase` / `password_strength` | password-generator, passphrase-generator, password-strength-checker |
//! | `random_bytes` / `random_numbers` / `uuid` / `ulid` / `ulid_decode` / `nanoid` | secure random, token, uuid-generator, ulid-generator, nanoid-generator |
//! | `encrypt` / `decrypt` / `aes_gcm` | text-encryptor, aes-encrypt-decrypt |
//! | `rsa_generate` / `rsa_sign` / `rsa_verify` / `rsa_encrypt` / `rsa_decrypt` / `rsa_inspect` | rsa tools |
//! | `jwt_decode` / `jwt_sign` / `jwt_verify` | jwt-decoder, jwt-generator, jwt-validator, jwt-inspector |
//! | `totp` / `hotp` / `totp_uri` | totp-authenticator, qr-based-2fa-generator |
//! | `pem_parse` / `certificate_decode` | pem-parser, certificate-decoder, ssl-certificate-decoder |

pub mod certs;
pub mod ciphers;
pub mod generators;
pub mod hashing;

use engine_shared::{b64, EngineResult};
use serde::Deserialize;
use serde_json::Value;

fn default_true() -> bool {
    true
}
fn default_len() -> usize {
    16
}
fn default_one() -> usize {
    1
}
fn default_cost() -> u32 {
    10
}
fn default_m_cost() -> u32 {
    19 * 1024
}
fn default_t_cost() -> u32 {
    2
}
fn default_p_cost() -> u32 {
    1
}
fn default_iterations() -> u32 {
    600_000
}
fn default_key_len() -> usize {
    32
}
fn default_words() -> usize {
    4
}
fn default_dash() -> String {
    "-".into()
}
fn default_bits() -> usize {
    2048
}
fn default_digits() -> u32 {
    6
}
fn default_period() -> u32 {
    30
}
fn default_window() -> u32 {
    1
}
fn default_leeway() -> i64 {
    0
}
fn default_nanoid_size() -> usize {
    21
}
fn default_bytes_len() -> usize {
    32
}

/// Every action understood by the crypto engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum CryptoAction {
    /// Hashes text or a file with one or more algorithms.
    Hash {
        /// Text input (UTF-8).
        #[serde(default)]
        text: Option<String>,
        /// File input (base64) – used instead of `text` when present.
        #[serde(default, with = "b64::option")]
        file: Option<Vec<u8>>,
        /// Algorithms: `sha256` (default), `md5`, `sha1`, `sha224`, `sha384`, `sha512`, `sha3-256`,
        /// `sha3-512`, `blake2b-512`, `blake3`, `crc32`, `adler32`, `keccak256`.
        #[serde(default)]
        algorithms: Vec<String>,
        /// Output format (`hex`, `base64`, `binary`).
        #[serde(default)]
        format: String,
        /// Expected digest to compare against.
        #[serde(default)]
        expected: Option<String>,
    },
    /// Keyed hash (HMAC).
    Hmac {
        /// Text input.
        #[serde(default)]
        text: Option<String>,
        /// File input (base64).
        #[serde(default, with = "b64::option")]
        file: Option<Vec<u8>>,
        /// Secret key.
        key: String,
        /// How `key` is encoded (`utf8`, `hex`, `base64`).
        #[serde(default)]
        key_encoding: String,
        /// `sha256` (default), `sha1`, `sha224`, `sha384`, `sha512`, `md5`, `sha3-256` or
        /// `sha3-512`.
        #[serde(default)]
        algorithm: String,
        /// `hex` (default), `base64` or `binary`.
        #[serde(default)]
        format: String,
    },
    /// Verifies an HMAC signature in constant time.
    HmacVerify {
        /// Text input.
        #[serde(default)]
        text: Option<String>,
        /// File input (base64).
        #[serde(default, with = "b64::option")]
        file: Option<Vec<u8>>,
        /// Secret key.
        key: String,
        /// How `key` is encoded.
        #[serde(default)]
        key_encoding: String,
        /// `sha256` (default), `sha1`, `sha224`, `sha384`, `sha512`, `md5`, `sha3-256` or
        /// `sha3-512`.
        #[serde(default)]
        algorithm: String,
        /// Expected signature (hex or base64).
        signature: String,
    },
    /// bcrypt password hashing.
    BcryptHash {
        /// Password.
        password: String,
        /// Cost factor 4–14.
        #[serde(default = "default_cost")]
        cost: u32,
    },
    /// bcrypt verification.
    BcryptVerify {
        /// Password.
        password: String,
        /// bcrypt hash string.
        hash: String,
    },
    /// Argon2 password hashing.
    Argon2Hash {
        /// Password.
        password: String,
        /// `argon2id` (default), `argon2i` or `argon2d`.
        #[serde(default)]
        variant: String,
        /// Memory cost in KiB.
        #[serde(default = "default_m_cost")]
        m_cost_kib: u32,
        /// Iterations.
        #[serde(default = "default_t_cost")]
        t_cost: u32,
        /// Parallelism lanes.
        #[serde(default = "default_p_cost")]
        p_cost: u32,
    },
    /// Argon2 verification.
    Argon2Verify {
        /// Password.
        password: String,
        /// PHC-format hash string.
        hash: String,
    },
    /// PBKDF2 key derivation.
    Pbkdf2 {
        /// Password.
        password: String,
        /// Salt (generated when omitted).
        #[serde(default)]
        salt: Option<String>,
        /// Iteration count.
        #[serde(default = "default_iterations")]
        iterations: u32,
        /// Derived key length in bytes.
        #[serde(default = "default_key_len")]
        length: usize,
        /// `sha256` (default), `sha1` or `sha512`.
        #[serde(default)]
        algorithm: String,
        /// `hex` (default), `base64` or `binary`.
        #[serde(default)]
        format: String,
    },

    /// Random passwords (needs caller entropy).
    Password {
        /// Password length.
        #[serde(default = "default_len")]
        length: usize,
        /// Number of passwords.
        #[serde(default = "default_one")]
        count: usize,
        /// Include upper-case letters.
        #[serde(default = "default_true")]
        uppercase: bool,
        /// Include lower-case letters.
        #[serde(default = "default_true")]
        lowercase: bool,
        /// Include digits.
        #[serde(default = "default_true")]
        digits: bool,
        /// Include symbols.
        #[serde(default = "default_true")]
        symbols: bool,
        /// Exclude look-alike characters (`0O1lI`).
        #[serde(default)]
        exclude_ambiguous: bool,
        /// Custom symbol set.
        #[serde(default)]
        custom_symbols: Option<String>,
        /// Guarantee at least one character from each enabled class.
        #[serde(default = "default_true")]
        require_each: bool,
    },
    /// Diceware-style passphrases.
    Passphrase {
        /// Number of words.
        #[serde(default = "default_words")]
        words: usize,
        /// Word separator.
        #[serde(default = "default_dash")]
        separator: String,
        /// Capitalise words.
        #[serde(default)]
        capitalize: bool,
        /// Append a random number.
        #[serde(default)]
        add_number: bool,
        /// Number of passphrases.
        #[serde(default = "default_one")]
        count: usize,
    },
    /// Entropy/crack-time estimate and feedback for a password.
    PasswordStrength {
        /// Password to analyse.
        password: String,
    },
    /// Random bytes.
    RandomBytes {
        /// Byte length.
        #[serde(default = "default_bytes_len")]
        length: usize,
        /// `hex` (default), `base64` or `binary`.
        #[serde(default)]
        format: String,
        /// Number of values.
        #[serde(default = "default_one")]
        count: usize,
    },
    /// Random integers in a range.
    RandomNumbers {
        /// Lower bound (inclusive).
        min: i64,
        /// Upper bound (inclusive).
        max: i64,
        /// Number of values.
        #[serde(default = "default_one")]
        count: usize,
        /// Disallow repeats.
        #[serde(default)]
        unique: bool,
    },
    /// UUID generation (v4 random, v7 time-ordered, v3/v5 name-based, nil, max).
    Uuid {
        /// Version (`4`, `7`, `3`, `5`, `nil`, `max`).
        #[serde(default)]
        version: String,
        /// Number of UUIDs.
        #[serde(default = "default_one")]
        count: usize,
        /// Timestamp for v7 (`Date.now()`; the engine has no clock).
        #[serde(default)]
        timestamp_ms: Option<u64>,
        /// Namespace UUID for v3/v5.
        #[serde(default)]
        namespace: Option<String>,
        /// Name for v3/v5.
        #[serde(default)]
        name: Option<String>,
        /// `hyphenated` (default), `simple`, `urn` or `braced`.
        #[serde(default)]
        format: String,
    },
    /// ULID generation.
    Ulid {
        /// Number of ULIDs.
        #[serde(default = "default_one")]
        count: usize,
        /// Timestamp in milliseconds (`Date.now()`).
        timestamp_ms: u64,
        /// Lower-case output.
        #[serde(default)]
        lowercase: bool,
    },
    /// Decodes a ULID into timestamp and randomness.
    UlidDecode {
        /// ULID string.
        ulid: String,
    },
    /// Nano ID generation.
    Nanoid {
        /// ID length.
        #[serde(default = "default_nanoid_size")]
        size: usize,
        /// Custom alphabet.
        #[serde(default)]
        alphabet: Option<String>,
        /// Number of IDs.
        #[serde(default = "default_one")]
        count: usize,
    },

    /// Password-based encryption (PBKDF2 + AES-256-GCM or ChaCha20-Poly1305).
    Encrypt {
        /// Text to encrypt.
        #[serde(default)]
        text: Option<String>,
        /// File to encrypt (base64).
        #[serde(default, with = "b64::option")]
        file: Option<Vec<u8>>,
        /// Password.
        password: String,
        /// `aes-256-gcm` (default) or `chacha20-poly1305`.
        #[serde(default)]
        cipher: String,
        /// PBKDF2 iterations (default 210 000).
        #[serde(default)]
        iterations: Option<u32>,
    },
    /// Decrypts an `encrypt` output.
    Decrypt {
        /// Ciphertext envelope (base64).
        ciphertext: String,
        /// Password.
        password: String,
    },
    /// Raw AES-256-GCM with an explicit key.
    AesGcm {
        /// Text input.
        #[serde(default)]
        text: Option<String>,
        /// Binary input (base64).
        #[serde(default, with = "b64::option")]
        data: Option<Vec<u8>>,
        /// Key (32 bytes).
        key: String,
        /// How `key` is encoded (`hex`, `base64`, `utf8`).
        #[serde(default)]
        key_encoding: String,
        /// Nonce as hex (random when omitted; required for decrypt unless prefixed).
        #[serde(default)]
        nonce_hex: Option<String>,
        /// Additional authenticated data.
        #[serde(default)]
        aad: String,
        /// Decrypt instead of encrypt.
        #[serde(default)]
        decrypt: bool,
    },

    /// RSA key-pair generation.
    RsaGenerate {
        /// Key size in bits (2048–4096).
        #[serde(default = "default_bits")]
        bits: usize,
    },
    /// RSA signature.
    RsaSign {
        /// Private key (PKCS#8 or PKCS#1 PEM).
        private_key_pem: String,
        /// Text to sign.
        #[serde(default)]
        text: Option<String>,
        /// File to sign (base64).
        #[serde(default, with = "b64::option")]
        file: Option<Vec<u8>>,
        /// `pkcs1v15` (default) or `pss`.
        #[serde(default)]
        scheme: String,
        /// `sha256` (default), `sha384` or `sha512`.
        #[serde(default)]
        hash: String,
    },
    /// RSA signature verification.
    RsaVerify {
        /// Public key (SPKI or PKCS#1 PEM).
        public_key_pem: String,
        /// Signed text.
        #[serde(default)]
        text: Option<String>,
        /// Signed file (base64).
        #[serde(default, with = "b64::option")]
        file: Option<Vec<u8>>,
        /// Signature (base64).
        signature: String,
        /// `pkcs1v15` or `pss`.
        #[serde(default)]
        scheme: String,
        /// `sha256` (default), `sha1`, `sha224`, `sha384`, `sha512`, `md5`, `sha3-256` or
        /// `sha3-512`.
        #[serde(default)]
        hash: String,
    },
    /// RSA-OAEP encryption.
    RsaEncrypt {
        /// Public key PEM.
        public_key_pem: String,
        /// Text to encrypt.
        #[serde(default)]
        text: Option<String>,
        /// File to encrypt (base64).
        #[serde(default, with = "b64::option")]
        file: Option<Vec<u8>>,
    },
    /// RSA-OAEP decryption.
    RsaDecrypt {
        /// Private key PEM.
        private_key_pem: String,
        /// Ciphertext (base64).
        ciphertext: String,
    },
    /// RSA key inspection.
    RsaInspect {
        /// Public or private key PEM.
        pem: String,
    },

    /// Decodes a JWT without verifying it.
    JwtDecode {
        /// Token.
        token: String,
        /// Current time for expiry checks (`Date.now()/1000`).
        #[serde(default)]
        now_unix: Option<i64>,
    },
    /// Signs a JWT (HS256/384/512, RS256/384/512).
    JwtSign {
        /// Claims object.
        payload: Value,
        /// HMAC secret or RSA private key PEM.
        secret: String,
        /// `HS256` (default), `HS384`, `HS512`, `RS256`, `RS384` or `RS512`.
        #[serde(default)]
        algorithm: String,
        /// Extra header fields.
        #[serde(default)]
        header: Option<Value>,
    },
    /// Verifies a JWT signature and time claims.
    JwtVerify {
        /// Token.
        token: String,
        /// HMAC secret or RSA public key PEM.
        secret: String,
        /// Current time for `exp`/`nbf` checks.
        #[serde(default)]
        now_unix: Option<i64>,
        /// Clock-skew tolerance in seconds.
        #[serde(default = "default_leeway")]
        leeway_seconds: i64,
    },

    /// Time-based one-time password.
    Totp {
        /// Base32 secret.
        secret: String,
        /// Current Unix time.
        timestamp_unix: i64,
        /// Code length (6–8).
        #[serde(default = "default_digits")]
        digits: u32,
        /// Time step in seconds.
        #[serde(default = "default_period")]
        period: u32,
        /// `sha1` (default), `sha256` or `sha512`.
        #[serde(default)]
        algorithm: String,
        /// Number of adjacent steps to include.
        #[serde(default = "default_window")]
        window: u32,
    },
    /// Counter-based one-time password.
    Hotp {
        /// Base32 secret.
        secret: String,
        /// Counter value.
        counter: u64,
        /// Code length.
        #[serde(default = "default_digits")]
        digits: u32,
        /// `sha256` (default), `sha1`, `sha224`, `sha384`, `sha512`, `md5`, `sha3-256` or
        /// `sha3-512`.
        #[serde(default)]
        algorithm: String,
    },
    /// Builds an `otpauth://` URI (and generates a secret when omitted).
    TotpUri {
        /// Issuer shown in the authenticator app.
        #[serde(default)]
        issuer: String,
        /// Account name.
        account: String,
        /// Base32 secret (generated when omitted).
        #[serde(default)]
        secret: Option<String>,
        /// Code length.
        #[serde(default = "default_digits")]
        digits: u32,
        /// Time step in seconds.
        #[serde(default = "default_period")]
        period: u32,
        /// `sha256` (default), `sha1`, `sha224`, `sha384`, `sha512`, `md5`, `sha3-256` or
        /// `sha3-512`.
        #[serde(default)]
        algorithm: String,
    },

    /// Parses PEM into labelled DER blocks with basic structure info.
    PemParse {
        /// PEM text.
        pem: String,
    },
    /// Decodes an X.509 certificate.
    CertificateDecode {
        /// PEM certificate text.
        #[serde(default)]
        certificate: String,
        /// DER certificate bytes (base64) – alternative to `certificate`.
        #[serde(default, with = "b64::option")]
        der: Option<Vec<u8>>,
        /// Current time for the validity check.
        #[serde(default)]
        now_unix: Option<i64>,
    },
}

/// Dispatches a [`CryptoAction`] to its implementation.
pub fn handle_action(action: CryptoAction) -> EngineResult<Value> {
    use hashing::{input_bytes, key_bytes};
    match action {
        CryptoAction::Hash { text, file, algorithms, format, expected } => hashing::hash(
            &input_bytes(text.as_deref(), file.as_deref())?,
            &algorithms,
            &format,
            expected.as_deref(),
        ),
        CryptoAction::Hmac {
            text,
            file,
            key,
            key_encoding,
            algorithm,
            format,
        } => hashing::hmac(
            &input_bytes(text.as_deref(), file.as_deref())?,
            &key_bytes(&key, &key_encoding)?,
            &algorithm,
            &format,
        ),
        CryptoAction::HmacVerify {
            text,
            file,
            key,
            key_encoding,
            algorithm,
            signature,
        } => hashing::hmac_verify(
            &input_bytes(text.as_deref(), file.as_deref())?,
            &key_bytes(&key, &key_encoding)?,
            &algorithm,
            &signature,
        ),
        CryptoAction::BcryptHash { password, cost } => hashing::bcrypt_hash(&password, cost),
        CryptoAction::BcryptVerify { password, hash } => hashing::bcrypt_verify(&password, &hash),
        CryptoAction::Argon2Hash {
            password,
            variant,
            m_cost_kib,
            t_cost,
            p_cost,
        } => hashing::argon2_hash(&password, &variant, m_cost_kib, t_cost, p_cost),
        CryptoAction::Argon2Verify { password, hash } => hashing::argon2_verify(&password, &hash),
        CryptoAction::Pbkdf2 {
            password,
            salt,
            iterations,
            length,
            algorithm,
            format,
        } => {
            let salt_bytes = match salt {
                Some(s) if !s.is_empty() => s.into_bytes(),
                _ => {
                    let mut b = vec![0u8; 16];
                    engine_shared::rng::fill_bytes(&mut b);
                    b
                },
            };
            hashing::pbkdf2(&password, &salt_bytes, iterations, length, &algorithm, &format)
        },

        CryptoAction::Password {
            length,
            count,
            uppercase,
            lowercase,
            digits,
            symbols,
            exclude_ambiguous,
            custom_symbols,
            require_each,
        } => generators::password(
            length,
            count,
            uppercase,
            lowercase,
            digits,
            symbols,
            exclude_ambiguous,
            custom_symbols.as_deref(),
            require_each,
        ),
        CryptoAction::Passphrase {
            words,
            separator,
            capitalize,
            add_number,
            count,
        } => generators::passphrase(words, &separator, capitalize, add_number, count),
        CryptoAction::PasswordStrength { password } => generators::password_strength(&password),
        CryptoAction::RandomBytes { length, format, count } => {
            generators::random_bytes(length, &format, count)
        },
        CryptoAction::RandomNumbers { min, max, count, unique } => {
            generators::random_numbers(min, max, count, unique)
        },
        CryptoAction::Uuid {
            version,
            count,
            timestamp_ms,
            namespace,
            name,
            format,
        } => generators::uuid(
            &version,
            count,
            timestamp_ms,
            namespace.as_deref(),
            name.as_deref(),
            &format,
        ),
        CryptoAction::Ulid { count, timestamp_ms, lowercase } => {
            generators::ulid(count, timestamp_ms, lowercase)
        },
        CryptoAction::UlidDecode { ulid } => generators::ulid_decode(&ulid),
        CryptoAction::Nanoid { size, alphabet, count } => {
            generators::nanoid(size, alphabet.as_deref(), count)
        },

        CryptoAction::Encrypt { text, file, password, cipher, iterations } => ciphers::encrypt(
            &input_bytes(text.as_deref(), file.as_deref())?,
            &password,
            &cipher,
            iterations,
        ),
        CryptoAction::Decrypt { ciphertext, password } => ciphers::decrypt(&ciphertext, &password),
        CryptoAction::AesGcm {
            text,
            data,
            key,
            key_encoding,
            nonce_hex,
            aad,
            decrypt,
        } => {
            let input = match (&data, &text) {
                (Some(d), _) if !d.is_empty() => d.clone(),
                (_, Some(t)) if !t.is_empty() => {
                    if decrypt {
                        b64::decode(t)?
                    } else {
                        t.as_bytes().to_vec()
                    }
                },
                _ => return Err(engine_shared::EngineError::empty("data")),
            };
            let key = key_bytes(&key, if key_encoding.is_empty() { "hex" } else { &key_encoding })?;
            let nonce = nonce_hex
                .map(|n| {
                    hex::decode(n.trim()).map_err(|_| {
                        engine_shared::EngineError::invalid("nonce_hex is not valid hex")
                    })
                })
                .transpose()?;
            ciphers::aes_gcm_raw(&input, &key, nonce.as_deref(), aad.as_bytes(), decrypt)
        },

        CryptoAction::RsaGenerate { bits } => ciphers::rsa_generate(bits),
        CryptoAction::RsaSign {
            private_key_pem,
            text,
            file,
            scheme,
            hash,
        } => ciphers::rsa_sign(
            &private_key_pem,
            &input_bytes(text.as_deref(), file.as_deref())?,
            &scheme,
            &hash,
        ),
        CryptoAction::RsaVerify {
            public_key_pem,
            text,
            file,
            signature,
            scheme,
            hash,
        } => ciphers::rsa_verify(
            &public_key_pem,
            &input_bytes(text.as_deref(), file.as_deref())?,
            &signature,
            &scheme,
            &hash,
        ),
        CryptoAction::RsaEncrypt { public_key_pem, text, file } => {
            ciphers::rsa_encrypt(&public_key_pem, &input_bytes(text.as_deref(), file.as_deref())?)
        },
        CryptoAction::RsaDecrypt { private_key_pem, ciphertext } => {
            ciphers::rsa_decrypt(&private_key_pem, &ciphertext)
        },
        CryptoAction::RsaInspect { pem } => ciphers::rsa_inspect(&pem),

        CryptoAction::JwtDecode { token, now_unix } => ciphers::jwt_decode(&token, now_unix),
        CryptoAction::JwtSign { payload, secret, algorithm, header } => {
            ciphers::jwt_sign(&payload, &secret, &algorithm, header.as_ref())
        },
        CryptoAction::JwtVerify { token, secret, now_unix, leeway_seconds } => {
            ciphers::jwt_verify(&token, &secret, now_unix, leeway_seconds)
        },

        CryptoAction::Totp {
            secret,
            timestamp_unix,
            digits,
            period,
            algorithm,
            window,
        } => ciphers::totp(&secret, timestamp_unix, digits, period, &algorithm, window),
        CryptoAction::Hotp { secret, counter, digits, algorithm } => {
            ciphers::hotp(&secret, counter, digits, &algorithm)
        },
        CryptoAction::TotpUri {
            issuer,
            account,
            secret,
            digits,
            period,
            algorithm,
        } => ciphers::totp_uri(&issuer, &account, secret.as_deref(), digits, period, &algorithm),

        CryptoAction::PemParse { pem } => certs::parse_pem(&pem),
        CryptoAction::CertificateDecode { certificate, der, now_unix } => {
            certs::decode_certificate(&certificate, der.as_deref(), now_unix)
        },
    }
}

engine_shared::export_engine!(handle_action, CryptoAction);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_algorithms_listed() {
        let r = handle_action(CryptoAction::Hash {
            text: Some("abc".into()),
            file: None,
            algorithms: vec![],
            format: String::new(),
            expected: None,
        })
        .unwrap();
        assert_eq!(
            r["hashes"]["sha256"],
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(hashing::ALGORITHMS.contains(&"sha256"));
    }
}
