//! Encryption: protect, unlock and permission inspection.

use crate::doc;
use crate::pages::PdfOutput;
use engine_shared::{EngineError, EngineResult};
use lopdf::encryption::crypt_filters::{Aes128CryptFilter, Aes256CryptFilter, CryptFilter};
use lopdf::{Document, EncryptionState, EncryptionVersion, Object, Permissions};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Permissions granted to users opening with the user password.
#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct PermissionFlags {
    /// Allow printing.
    pub print: bool,
    /// Allow high-quality printing.
    pub print_high_quality: bool,
    /// Allow copying text and graphics.
    pub copy: bool,
    /// Allow modifying content.
    pub modify: bool,
    /// Allow adding/changing annotations.
    pub annotate: bool,
    /// Allow filling form fields.
    pub fill_forms: bool,
    /// Allow inserting, rotating or deleting pages.
    pub assemble: bool,
    /// Allow screen readers to extract content.
    pub accessibility: bool,
}

impl Default for PermissionFlags {
    fn default() -> Self {
        PermissionFlags {
            print: true,
            print_high_quality: true,
            copy: true,
            modify: true,
            annotate: true,
            fill_forms: true,
            assemble: true,
            accessibility: true,
        }
    }
}

impl PermissionFlags {
    fn to_permissions(&self) -> Permissions {
        let mut p = Permissions::empty();
        if self.print {
            p |= Permissions::PRINTABLE;
        }
        if self.print_high_quality {
            p |= Permissions::PRINTABLE_IN_HIGH_QUALITY;
        }
        if self.copy {
            p |= Permissions::COPYABLE;
        }
        if self.modify {
            p |= Permissions::MODIFIABLE;
        }
        if self.annotate {
            p |= Permissions::ANNOTABLE;
        }
        if self.fill_forms {
            p |= Permissions::FILLABLE;
        }
        if self.assemble {
            p |= Permissions::ASSEMBLABLE;
        }
        if self.accessibility {
            p |= Permissions::COPYABLE_FOR_ACCESSIBILITY;
        }
        p
    }

    fn from_permissions(p: Permissions) -> Value {
        json!({
            "print": p.contains(Permissions::PRINTABLE),
            "print_high_quality": p.contains(Permissions::PRINTABLE_IN_HIGH_QUALITY),
            "copy": p.contains(Permissions::COPYABLE),
            "modify": p.contains(Permissions::MODIFIABLE),
            "annotate": p.contains(Permissions::ANNOTABLE),
            "fill_forms": p.contains(Permissions::FILLABLE),
            "assemble": p.contains(Permissions::ASSEMBLABLE),
            "accessibility": p.contains(Permissions::COPYABLE_FOR_ACCESSIBILITY),
        })
    }
}

/// Encrypts a document. `algorithm`: `aes256` (default, PDF 2.0),
/// `aes128` (PDF 1.6) or `rc4` (128-bit, PDF 1.4 – legacy viewers only).
pub fn protect(
    bytes: &[u8],
    password: Option<&str>,
    user_password: &str,
    owner_password: &str,
    algorithm: &str,
    flags: &PermissionFlags,
) -> EngineResult<Value> {
    if user_password.is_empty() && owner_password.is_empty() {
        return Err(EngineError::invalid(
            "Provide at least a `user_password` or an `owner_password`",
        ));
    }
    if user_password.len() > 127 || owner_password.len() > 127 {
        return Err(EngineError::invalid("Passwords must be at most 127 bytes"));
    }
    let owner = if owner_password.is_empty() {
        user_password
    } else {
        owner_password
    };
    let mut d = doc::load(bytes, password)?;
    // Encryption key derivation (RC4 / AES-128) and viewers need a file /ID.
    if d.trailer.get(b"ID").is_err() {
        let mut id = [0u8; 16];
        engine_shared::rng::fill_bytes(&mut id);
        d.trailer.set(
            "ID",
            Object::Array(vec![
                Object::String(id.to_vec(), lopdf::StringFormat::Hexadecimal),
                Object::String(id.to_vec(), lopdf::StringFormat::Hexadecimal),
            ]),
        );
    }
    let permissions = flags.to_permissions();
    let algo = algorithm.trim().to_ascii_lowercase();
    let state = match algo.as_str() {
        "" | "aes256" | "aes-256" => {
            // AES-256 needs a random file key. The pool is stirred per request;
            // callers should pass `entropy` for cryptographic strength
            // (`secure_entropy` in the response tells whether they did).
            let mut key = [0u8; 32];
            engine_shared::rng::fill_bytes(&mut key);
            let filter: Arc<dyn CryptFilter> = Arc::new(Aes256CryptFilter);
            EncryptionState::try_from(EncryptionVersion::V5 {
                encrypt_metadata: true,
                crypt_filters: BTreeMap::from([(b"StdCF".to_vec(), filter)]),
                file_encryption_key: &key,
                stream_filter: b"StdCF".to_vec(),
                string_filter: b"StdCF".to_vec(),
                owner_password: owner,
                user_password,
                permissions,
            })
        },
        "aes128" | "aes-128" => {
            let filter: Arc<dyn CryptFilter> = Arc::new(Aes128CryptFilter);
            EncryptionState::try_from(EncryptionVersion::V4 {
                document: &d,
                encrypt_metadata: true,
                crypt_filters: BTreeMap::from([(b"StdCF".to_vec(), filter)]),
                stream_filter: b"StdCF".to_vec(),
                string_filter: b"StdCF".to_vec(),
                owner_password: owner,
                user_password,
                permissions,
            })
        },
        "rc4" | "rc4-128" => EncryptionState::try_from(EncryptionVersion::V2 {
            document: &d,
            owner_password: owner,
            user_password,
            key_length: 128,
            permissions,
        }),
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown algorithm `{other}` (use aes256, aes128 or rc4)"
            )))
        },
    }
    .map_err(doc::map_pdf_error)?;

    d.encrypt(&state).map_err(doc::map_pdf_error)?;
    let total = d.get_pages().len();
    let out = doc::save(&mut d)?;
    Ok(json!({
        "output": PdfOutput::new(out, total),
        "algorithm": if algo.is_empty() { "aes256".to_string() } else { algo },
        "user_password_set": !user_password.is_empty(),
        "owner_password_set": !owner_password.is_empty(),
        "permissions": PermissionFlags::from_permissions(permissions),
        "secure_entropy": engine_shared::rng::has_caller_entropy(),
    }))
}

/// Removes encryption (requires the user or owner password unless the file
/// opens with an empty password).
pub fn unlock(bytes: &[u8], password: &str) -> EngineResult<Value> {
    let mut d = doc::load(bytes, Some(password))?;
    if !d.was_encrypted() {
        return Err(EngineError::invalid("The PDF is not encrypted; nothing to unlock"));
    }
    // `doc::load` already decrypted; make sure nothing references /Encrypt.
    d.trailer.remove(b"Encrypt");
    d.encryption_state = None;
    let total = d.get_pages().len();
    let out = doc::save(&mut d)?;
    Ok(json!({ "output": PdfOutput::new(out, total), "was_encrypted": true }))
}

/// Reports the encryption state and permission flags without modifying the file.
pub fn permissions(bytes: &[u8], password: Option<&str>) -> EngineResult<Value> {
    if bytes.is_empty() {
        return Err(EngineError::empty("PDF file"));
    }
    if !doc::looks_like_pdf(bytes) {
        return Err(EngineError::unsupported_format("Not a PDF file"));
    }
    let d = Document::load_mem(bytes).map_err(doc::map_pdf_error)?;
    if !d.is_encrypted() {
        if let Some(state) = &d.encryption_state {
            // Encrypted with an empty user password: lopdf decrypted it on load.
            return Ok(json!({
                "encrypted": true,
                "method": match state.version() { 5 => "AES-256", 4 => "AES-128/RC4 (V4)", 2 => "RC4", 1 => "RC4-40", _ => "Unknown" },
                "version": state.version(),
                "revision": state.revision(),
                "key_length_bits": state.key_length().unwrap_or(if state.version() >= 5 { 256 } else { 40 }),
                "encrypt_metadata": state.encrypt_metadata(),
                "user_password_required": false,
                "permissions": PermissionFlags::from_permissions(state.permissions()),
                "password_check": password.filter(|p| !p.is_empty()).map(|p| json!({ "user_password_ok": d.authenticate_user_password(p).is_ok(), "owner_password_ok": d.authenticate_owner_password(p).is_ok(), })),
            }));
        }
        return Ok(json!({
            "encrypted": false,
            "permissions": PermissionFlags::from_permissions(Permissions::all()),
            "user_password_required": false,
        }));
    }
    let enc = d.get_encrypted().map_err(doc::map_pdf_error)?;
    let filter = enc
        .get(b"Filter")
        .ok()
        .and_then(|f| f.as_name().ok())
        .map(|n| String::from_utf8_lossy(n).into_owned());
    let v = enc.get(b"V").ok().and_then(|v| v.as_i64().ok()).unwrap_or(0);
    let r = enc.get(b"R").ok().and_then(|v| v.as_i64().ok()).unwrap_or(0);
    let length = enc.get(b"Length").ok().and_then(|v| v.as_i64().ok()).unwrap_or(40);
    let p = enc.get(b"P").ok().and_then(|v| v.as_i64().ok()).unwrap_or(-1);
    let perms = Permissions::from_bits_retain(p as u64 | (0xffff_ffffu64 << 32));
    let method = match (v, r) {
        (5, _) => "AES-256",
        (4, _) => {
            let cfm = enc
                .get(b"CF")
                .ok()
                .and_then(|cf| cf.as_dict().ok())
                .and_then(|cf| cf.get(b"StdCF").ok())
                .and_then(|s| s.as_dict().ok())
                .and_then(|s| s.get(b"CFM").ok())
                .and_then(|c| c.as_name().ok());
            match cfm {
                Some(b"AESV2") => "AES-128",
                Some(b"V2") => "RC4-128",
                _ => "AES-128/RC4 (V4)",
            }
        },
        (2, _) => "RC4",
        (1, _) => "RC4-40",
        _ => "Unknown",
    };
    let opens_without_password = d.authenticate_user_password("").is_ok();
    let password_check = password.filter(|p| !p.is_empty()).map(|p| {
        json!({
            "user_password_ok": d.authenticate_user_password(p).is_ok(),
            "owner_password_ok": d.authenticate_owner_password(p).is_ok(),
        })
    });
    Ok(json!({
        "encrypted": true,
        "filter": filter,
        "method": method,
        "version": v,
        "revision": r,
        "key_length_bits": if v >= 5 { 256 } else { length },
        "encrypt_metadata": enc.get(b"EncryptMetadata").ok().and_then(|m| m.as_bool().ok()).unwrap_or(true),
        "user_password_required": !opens_without_password,
        "permissions": PermissionFlags::from_permissions(perms),
        "permission_bits": p,
        "password_check": password_check,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::sample_pdf;
    use engine_shared::b64;

    fn out_bytes(v: &Value) -> Vec<u8> {
        b64::decode(v["output"]["file"].as_str().unwrap()).unwrap()
    }

    #[test]
    fn protect_unlock_cycle() {
        let pdf = sample_pdf(&["hidden text"]);
        for algo in ["aes256", "aes128", "rc4"] {
            let flags = PermissionFlags { copy: false, ..Default::default() };
            let p = protect(&pdf, None, "user", "owner", algo, &flags).unwrap();
            let enc = out_bytes(&p);
            assert!(permissions(&enc, None).unwrap()["encrypted"].as_bool().unwrap(), "{algo}");
            let perms = permissions(&enc, Some("owner")).unwrap();
            assert_eq!(perms["permissions"]["copy"], false, "{algo}");
            assert_eq!(perms["password_check"]["owner_password_ok"], true, "{algo}");
            assert_eq!(perms["user_password_required"], true, "{algo}");
            // Wrong / missing password.
            assert_eq!(
                doc::load(&enc, None).unwrap_err().code,
                engine_shared::error::ErrorCode::Encrypted
            );
            assert_eq!(
                doc::load(&enc, Some("bad")).unwrap_err().code,
                engine_shared::error::ErrorCode::WrongPassword
            );
            let u = unlock(&enc, "user").unwrap();
            let plain = out_bytes(&u);
            let d = doc::load(&plain, None).unwrap();
            assert!(!d.is_encrypted());
            assert!(d.extract_text(&[1]).unwrap().contains("hidden text"), "{algo}");
            // Owner password also unlocks.
            assert!(unlock(&enc, "owner").is_ok());
        }
        assert!(unlock(&pdf, "x").is_err());
        assert!(protect(&pdf, None, "", "", "aes256", &PermissionFlags::default()).is_err());
        assert!(protect(&pdf, None, "a", "b", "des", &PermissionFlags::default()).is_err());
        assert_eq!(permissions(&pdf, None).unwrap()["encrypted"], false);
        assert!(permissions(b"", None).is_err());
    }
}
