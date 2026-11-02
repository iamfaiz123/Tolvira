//! PEM parsing and X.509 certificate decoding.

use engine_shared::{b64, EngineError, EngineResult};
use serde_json::{json, Value};
use sha2::Digest;
use x509_parser::prelude::*;
use x509_parser::public_key::PublicKey;

/// Splits a PEM bundle into labelled DER blocks.
pub fn parse_pem(text: &str) -> EngineResult<Value> {
    if text.trim().is_empty() {
        return Err(EngineError::empty("pem"));
    }
    let blocks = pem_blocks(text)?;
    let mut out = Vec::new();
    for (label, der) in &blocks {
        let mut entry = json!({
            "label": label,
            "der_bytes": der.len(),
            "kind": describe_label(label),
            "sha256": hex::encode(sha2::Sha256::digest(der))
        });
        if label.contains("CERTIFICATE") && !label.contains("REQUEST") {
            if let Ok(c) = decode_certificate_der(der, None) {
                entry["certificate"] = c;
            }
        } else if label.contains("PUBLIC KEY") || label.contains("PRIVATE KEY") {
            if let Ok(info) = crate::ciphers::rsa_inspect(&pem_of(label, der)) {
                entry["rsa"] = info;
            }
        }
        out.push(entry);
    }
    Ok(json!({ "blocks": out, "count": out.len() }))
}

fn pem_of(label: &str, der: &[u8]) -> String {
    let body = b64::encode(der);
    // Base64 is ASCII, so chunking by byte never splits a character.
    let wrapped: Vec<&str> = body
        .as_bytes()
        .chunks(64)
        .filter_map(|c| std::str::from_utf8(c).ok())
        .collect();
    format!("-----BEGIN {label}-----\n{}\n-----END {label}-----\n", wrapped.join("\n"))
}

fn describe_label(label: &str) -> &'static str {
    match label {
        "CERTIFICATE" | "X509 CERTIFICATE" | "TRUSTED CERTIFICATE" => "X.509 certificate",
        "CERTIFICATE REQUEST" | "NEW CERTIFICATE REQUEST" => "PKCS#10 certificate signing request",
        "PRIVATE KEY" => "PKCS#8 private key",
        "ENCRYPTED PRIVATE KEY" => "PKCS#8 encrypted private key",
        "RSA PRIVATE KEY" => "PKCS#1 RSA private key",
        "RSA PUBLIC KEY" => "PKCS#1 RSA public key",
        "EC PRIVATE KEY" => "SEC1 EC private key",
        "PUBLIC KEY" => "SubjectPublicKeyInfo public key",
        "OPENSSH PRIVATE KEY" => "OpenSSH private key",
        "X509 CRL" => "certificate revocation list",
        "PKCS7" | "CMS" => "PKCS#7 / CMS container",
        "DH PARAMETERS" => "Diffie-Hellman parameters",
        _ => "unknown PEM block",
    }
}

/// Splits PEM text into `(label, DER bytes)` blocks.
pub fn pem_blocks(text: &str) -> EngineResult<Vec<(String, Vec<u8>)>> {
    let mut out = Vec::new();
    let mut label: Option<String> = None;
    let mut body = String::new();
    for line in text.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("-----BEGIN ") {
            label = Some(rest.trim_end_matches('-').trim().to_string());
            body.clear();
        } else if l.starts_with("-----END ") {
            if let Some(lab) = label.take() {
                let der = b64::decode(&body).map_err(|_| {
                    EngineError::malformed(format!("PEM block `{lab}` contains invalid base64"))
                })?;
                out.push((lab, der));
            }
        } else if label.is_some() && !l.is_empty() && !l.contains(':') {
            body.push_str(l);
        }
    }
    if out.is_empty() {
        return Err(EngineError::unsupported_format(
            "No PEM blocks found (expected -----BEGIN ...----- / -----END ...-----)",
        ));
    }
    Ok(out)
}

fn unix_from_asn1(t: &ASN1Time) -> i64 {
    t.timestamp()
}

fn iso(t: i64) -> String {
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

fn name_json(n: &X509Name) -> Value {
    let mut m = serde_json::Map::new();
    for rdn in n.iter() {
        for attr in rdn.iter() {
            let oid_str = attr.attr_type().to_id_string();
            let key = match oid_str.as_str() {
                "2.5.4.3" => "CN",
                "2.5.4.6" => "C",
                "2.5.4.7" => "L",
                "2.5.4.8" => "ST",
                "2.5.4.10" => "O",
                "2.5.4.11" => "OU",
                "1.2.840.113549.1.9.1" => "emailAddress",
                "2.5.4.5" => "serialNumber",
                "2.5.4.9" => "street",
                "2.5.4.17" => "postalCode",
                other => other,
            };
            let val = attr
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|_| format!("{:?}", attr.attr_value()));
            match m.get_mut(key) {
                Some(Value::Array(a)) => a.push(json!(val)),
                Some(existing) => {
                    let prev = existing.take();
                    *existing = json!([prev, val]);
                },
                None => {
                    m.insert(key.to_string(), json!(val));
                },
            }
        }
    }
    let mut v = Value::Object(m);
    v["dn"] = json!(n.to_string());
    v
}

/// Decodes an X.509 certificate (PEM text or DER bytes).
pub fn decode_certificate(
    input: &str,
    der: Option<&[u8]>,
    now_unix: Option<i64>,
) -> EngineResult<Value> {
    let der_bytes: Vec<u8> = match der {
        Some(d) if !d.is_empty() => {
            if d.starts_with(b"-----") {
                let blocks = pem_blocks(
                    std::str::from_utf8(d)
                        .map_err(|_| EngineError::unsupported_format("Not PEM text"))?,
                )?;
                blocks
                    .into_iter()
                    .find(|(l, _)| l.contains("CERTIFICATE"))
                    .map(|(_, d)| d)
                    .ok_or_else(|| EngineError::not_found("No CERTIFICATE block in PEM"))?
            } else {
                d.to_vec()
            }
        },
        _ => {
            if input.trim().is_empty() {
                return Err(EngineError::empty("certificate"));
            }
            if input.contains("-----BEGIN") {
                let blocks = pem_blocks(input)?;
                blocks
                    .into_iter()
                    .find(|(l, _)| l.contains("CERTIFICATE") && !l.contains("REQUEST"))
                    .map(|(_, d)| d)
                    .ok_or_else(|| EngineError::not_found("No CERTIFICATE block in PEM"))?
            } else {
                b64::decode(input).map_err(|_| {
                    EngineError::unsupported_format("Certificate must be PEM text or base64 DER")
                })?
            }
        },
    };
    decode_certificate_der(&der_bytes, now_unix)
}

/// Decodes an X.509 certificate (subject, issuer, validity, SANs, key, fingerprints); `now_unix`
/// drives the expiry check.
pub fn decode_certificate_der(der: &[u8], now_unix: Option<i64>) -> EngineResult<Value> {
    let (_, cert) = X509Certificate::from_der(der)
        .map_err(|e| EngineError::malformed(format!("Invalid X.509 certificate: {e}")))?;
    let tbs = &cert.tbs_certificate;
    let not_before = unix_from_asn1(&tbs.validity.not_before);
    let not_after = unix_from_asn1(&tbs.validity.not_after);
    let mut sans: Vec<String> = Vec::new();
    let mut key_usage: Vec<String> = Vec::new();
    let mut ext_key_usage: Vec<String> = Vec::new();
    let mut is_ca = false;
    let mut path_len: Option<u32> = None;
    let mut extensions = Vec::new();
    for ext in cert.extensions() {
        let oid = ext.oid.to_id_string();
        let name = match ext.parsed_extension() {
            ParsedExtension::SubjectAlternativeName(san) => {
                for gn in &san.general_names {
                    sans.push(match gn {
                        GeneralName::DNSName(d) => format!("DNS:{d}"),
                        GeneralName::RFC822Name(e) => format!("email:{e}"),
                        GeneralName::IPAddress(ip) => format!(
                            "IP:{}",
                            ip.iter()
                                .map(|b| b.to_string())
                                .collect::<Vec<_>>()
                                .join(if ip.len() == 4 { "." } else { ":" })
                        ),
                        GeneralName::URI(u) => format!("URI:{u}"),
                        other => format!("{other:?}"),
                    });
                }
                "subjectAltName"
            },
            ParsedExtension::KeyUsage(ku) => {
                let flags = [
                    ("digitalSignature", ku.digital_signature()),
                    ("nonRepudiation", ku.non_repudiation()),
                    ("keyEncipherment", ku.key_encipherment()),
                    ("dataEncipherment", ku.data_encipherment()),
                    ("keyAgreement", ku.key_agreement()),
                    ("keyCertSign", ku.key_cert_sign()),
                    ("cRLSign", ku.crl_sign()),
                ];
                key_usage = flags
                    .iter()
                    .filter(|(_, on)| *on)
                    .map(|(n, _)| n.to_string())
                    .collect();
                "keyUsage"
            },
            ParsedExtension::ExtendedKeyUsage(eku) => {
                if eku.server_auth {
                    ext_key_usage.push("serverAuth".into());
                }
                if eku.client_auth {
                    ext_key_usage.push("clientAuth".into());
                }
                if eku.code_signing {
                    ext_key_usage.push("codeSigning".into());
                }
                if eku.email_protection {
                    ext_key_usage.push("emailProtection".into());
                }
                if eku.time_stamping {
                    ext_key_usage.push("timeStamping".into());
                }
                if eku.ocsp_signing {
                    ext_key_usage.push("OCSPSigning".into());
                }
                "extendedKeyUsage"
            },
            ParsedExtension::BasicConstraints(bc) => {
                is_ca = bc.ca;
                path_len = bc.path_len_constraint;
                "basicConstraints"
            },
            ParsedExtension::SubjectKeyIdentifier(_) => "subjectKeyIdentifier",
            ParsedExtension::AuthorityKeyIdentifier(_) => "authorityKeyIdentifier",
            ParsedExtension::CRLDistributionPoints(_) => "cRLDistributionPoints",
            ParsedExtension::AuthorityInfoAccess(_) => "authorityInfoAccess",
            ParsedExtension::CertificatePolicies(_) => "certificatePolicies",
            ParsedExtension::SCT(_) => "signedCertificateTimestampList",
            ParsedExtension::NameConstraints(_) => "nameConstraints",
            _ => "other",
        };
        extensions.push(json!({ "oid": oid, "name": name, "critical": ext.critical }));
    }
    let spki = &tbs.subject_pki;
    let (key_type, key_bits) = match spki.parsed() {
        Ok(PublicKey::RSA(r)) => ("RSA".to_string(), Some(r.key_size())),
        Ok(PublicKey::EC(e)) => ("EC".to_string(), Some(e.key_size())),
        Ok(PublicKey::DSA(_)) => ("DSA".to_string(), None),
        Ok(PublicKey::Unknown(_)) => (spki.algorithm.algorithm.to_id_string(), None),
        _ => (spki.algorithm.algorithm.to_id_string(), None),
    };
    let sig_alg = match cert.signature_algorithm.algorithm.to_id_string().as_str() {
        "1.2.840.113549.1.1.11" => "sha256WithRSAEncryption".to_string(),
        "1.2.840.113549.1.1.12" => "sha384WithRSAEncryption".to_string(),
        "1.2.840.113549.1.1.13" => "sha512WithRSAEncryption".to_string(),
        "1.2.840.113549.1.1.5" => "sha1WithRSAEncryption (weak)".to_string(),
        "1.2.840.10045.4.3.2" => "ecdsa-with-SHA256".to_string(),
        "1.2.840.10045.4.3.3" => "ecdsa-with-SHA384".to_string(),
        "1.3.101.112" => "Ed25519".to_string(),
        "1.2.840.113549.1.1.10" => "RSASSA-PSS".to_string(),
        other => other.to_string(),
    };
    let self_signed = tbs.subject == tbs.issuer;
    let status = now_unix.map(|now| {
        if now < not_before {
            "not yet valid"
        } else if now > not_after {
            "expired"
        } else {
            "valid"
        }
    });
    Ok(json!({
        "version": tbs.version.0 + 1,
        "serial_number": cert.raw_serial_as_string().replace(':', ""),
        "subject": name_json(&tbs.subject),
        "issuer": name_json(&tbs.issuer),
        "not_before": iso(not_before),
        "not_after": iso(not_after),
        "not_before_unix": not_before,
        "not_after_unix": not_after,
        "validity_days": (not_after - not_before) / 86400,
        "days_remaining": now_unix.map(|n| (not_after - n) / 86400),
        "status": status,
        "self_signed": self_signed,
        "is_ca": is_ca,
        "path_length_constraint": path_len,
        "subject_alt_names": sans,
        "key_usage": key_usage,
        "extended_key_usage": ext_key_usage,
        "public_key": { "type": key_type, "bits": key_bits },
        "signature_algorithm": sig_alg,
        "fingerprints": {
            "sha1": hex::encode(sha1::Sha1::digest(der)),
            "sha256": hex::encode(sha2::Sha256::digest(der)),
            "md5": hex::encode(md5::Md5::digest(der))
        },
        "extensions": extensions,
        "der_bytes": der.len(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Self-signed test certificate (RSA 2048, CN=example.test) generated with openssl.
    const CERT: &str = include_str!("../assets/test_cert.pem");

    #[test]
    fn decode_cert() {
        let c = decode_certificate(CERT, None, Some(1_700_000_000)).unwrap();
        assert_eq!(c["subject"]["CN"], "example.test");
        assert_eq!(c["self_signed"], true);
        assert_eq!(c["public_key"]["type"], "RSA");
        assert_eq!(c["public_key"]["bits"], 2048);
        assert!(c["subject_alt_names"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s.as_str().unwrap().contains("DNS:example.test")));
        assert!(c["fingerprints"]["sha256"].as_str().unwrap().len() == 64);
        assert!(c["status"].is_string());
        assert!(decode_certificate("", None, None).is_err());
        assert!(decode_certificate("not a cert", None, None).is_err());
        assert!(decode_certificate(
            "-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----",
            None,
            None
        )
        .is_err());
        let p = parse_pem(CERT).unwrap();
        assert_eq!(p["count"], 1);
        assert_eq!(p["blocks"][0]["kind"], "X.509 certificate");
        assert_eq!(p["blocks"][0]["certificate"]["subject"]["CN"], "example.test");
        assert!(parse_pem("no pem here").is_err());
    }
}
