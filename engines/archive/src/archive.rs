//! ZIP / TAR / GZIP listing, testing, extraction, creation and conversion.

use crate::gzip;
use crate::InputFile;
use engine_shared::{b64, limits, EngineError, EngineResult};
use serde::Serialize;
use serde_json::{json, Value};
use std::io::{Cursor, Read, Write};
use zip::result::ZipError;

/// Default total uncompressed budget (bytes).
pub const DEFAULT_MAX_TOTAL: u64 = 512 * 1024 * 1024;
/// Entries inflating more than this ratio are treated as bombs.
pub const MAX_RATIO: u64 = 1000;
/// Maximum entries returned.
pub const MAX_ENTRIES: usize = 50_000;

/// Archive container detected from magic bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// PKZIP.
    Zip,
    /// POSIX/GNU tar.
    Tar,
    /// Gzip-compressed tar.
    TarGz,
    /// Single-member gzip stream.
    Gzip,
    /// 7-Zip (detected only; not extractable in WASM).
    SevenZip,
    /// RAR (detected only; not extractable in WASM).
    Rar,
    /// Not recognised.
    Unknown,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Zip => "zip",
            Kind::Tar => "tar",
            Kind::TarGz => "tar.gz",
            Kind::Gzip => "gzip",
            Kind::SevenZip => "7z",
            Kind::Rar => "rar",
            Kind::Unknown => "unknown",
        }
    }
    fn mime(self) -> &'static str {
        match self {
            Kind::Zip => "application/zip",
            Kind::Tar => "application/x-tar",
            Kind::TarGz => "application/gzip",
            Kind::Gzip => "application/gzip",
            Kind::SevenZip => "application/x-7z-compressed",
            Kind::Rar => "application/vnd.rar",
            Kind::Unknown => "application/octet-stream",
        }
    }
}

fn looks_like_tar(bytes: &[u8]) -> bool {
    // ustar magic at offset 257, or a plausible header checksum.
    if bytes.len() >= 265 && (&bytes[257..262] == b"ustar" || &bytes[257..263] == b"ustar\0") {
        return true;
    }
    if bytes.len() >= 512 {
        let header = &bytes[..512];
        let chk_field = std::str::from_utf8(&header[148..156])
            .unwrap_or("")
            .trim_matches(|c: char| c == '\0' || c == ' ');
        if let Ok(expected) = u64::from_str_radix(chk_field, 8) {
            let sum: u64 = header
                .iter()
                .enumerate()
                .map(|(i, &b)| if (148..156).contains(&i) { 32 } else { b as u64 })
                .sum();
            return sum == expected && expected > 0;
        }
    }
    false
}

/// Detects the archive kind from magic bytes (tar by ustar header / checksum).
pub fn detect(bytes: &[u8]) -> Kind {
    if bytes.len() >= 4 && &bytes[..4] == b"PK\x03\x04"
        || bytes.len() >= 4 && &bytes[..4] == b"PK\x05\x06"
    {
        return Kind::Zip;
    }
    if bytes.len() >= 6 && &bytes[..6] == b"7z\xBC\xAF\x27\x1C" {
        return Kind::SevenZip;
    }
    if bytes.len() >= 7 && &bytes[..7] == b"Rar!\x1A\x07\x00"
        || bytes.len() >= 8 && &bytes[..8] == b"Rar!\x1A\x07\x01\x00"
    {
        return Kind::Rar;
    }
    if gzip::is_gzip(bytes) {
        // Peek at the decompressed head to tell tar.gz from plain gzip.
        let mut dec = flate2::read::GzDecoder::new(bytes);
        let mut head = vec![0u8; 1024];
        let n = dec.read(&mut head).unwrap_or(0);
        head.truncate(n);
        return if looks_like_tar(&head) { Kind::TarGz } else { Kind::Gzip };
    }
    if looks_like_tar(bytes) {
        return Kind::Tar;
    }
    Kind::Unknown
}

/// Error for archive kinds the engine cannot process (defensive; normally
/// caught earlier by [`require_archive`]).
fn unsupported_kind(kind: Kind) -> EngineError {
    EngineError::unsupported(format!(
        "{} archives are not supported by the browser engine",
        kind.name()
    ))
}

fn require_archive(bytes: &[u8]) -> EngineResult<Kind> {
    if bytes.is_empty() {
        return Err(EngineError::empty("archive file"));
    }
    limits::check_size(bytes.len(), limits::MAX_FILE_BYTES, "archive file")?;
    match detect(bytes) {
        Kind::Unknown => Err(EngineError::unsupported_format(
            "Unrecognised archive (expected ZIP, TAR, TAR.GZ or GZIP)",
        )),
        Kind::SevenZip => {
            Err(EngineError::unsupported("7z archives are not supported by the browser engine"))
        },
        Kind::Rar => {
            Err(EngineError::unsupported("RAR archives are not supported by the browser engine"))
        },
        k => Ok(k),
    }
}

/// Reports the detected kind, MIME type and whether the engine can process it.
pub fn detect_report(bytes: &[u8]) -> EngineResult<Value> {
    if bytes.is_empty() {
        return Err(EngineError::empty("file"));
    }
    let k = detect(bytes);
    Ok(json!({
        "kind": k.name(),
        "mime": k.mime(),
        "supported": matches!(k, Kind::Zip | Kind::Tar | Kind::TarGz | Kind::Gzip),
        "bytes": bytes.len()
    }))
}

/// Normalises an entry name: forward slashes, no drive letters, no `..`.
pub fn sanitize_name(raw: &str) -> (String, bool) {
    let mut unsafe_path = false;
    let normalised = raw.replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    for part in normalised.split('/') {
        match part {
            "" | "." => {},
            ".." => {
                unsafe_path = true;
                parts.pop();
            },
            p if p.contains(':') => {
                // Drop Windows drive letters / stream names.
                unsafe_path = true;
                let tail = p.rsplit(':').next().unwrap_or("");
                if !tail.is_empty() {
                    parts.push(tail);
                }
            },
            p => parts.push(p),
        }
    }
    if normalised.starts_with('/') {
        unsafe_path = true;
    }
    let mut name = parts.join("/");
    if normalised.ends_with('/') && !name.is_empty() {
        name.push('/');
    }
    (name, unsafe_path)
}

/// One archive entry as reported by `list`/`extract`.
#[derive(Serialize, Clone)]
pub struct Entry {
    /// Entry path inside the archive.
    pub name: String,
    /// `true` for directories.
    pub is_dir: bool,
    /// Uncompressed size in bytes.
    pub size: u64,
    /// Stored size in bytes (ZIP only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compressed_size: Option<u64>,
    /// Compression method name (ZIP only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// CRC-32 as hex (ZIP only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crc32: Option<String>,
    /// `true` when the entry is encrypted.
    pub encrypted: bool,
    /// Modification time as Unix seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mtime_unix: Option<i64>,
    /// Modification time as ISO-8601.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<String>,
    /// Unix permission bits.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<u32>,
    /// `true` when the path escapes the extraction root (`..`, absolute).
    pub unsafe_path: bool,
}

fn zip_datetime_to_unix(dt: zip::DateTime) -> (i64, String) {
    let (y, m, d, hh, mm, ss) = (
        dt.year() as i64,
        dt.month() as i64,
        dt.day() as i64,
        dt.hour() as i64,
        dt.minute() as i64,
        dt.second() as i64,
    );
    (
        civil_to_unix(y, m, d, hh, mm, ss),
        format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}"),
    )
}

/// Converts a civil date/time (UTC) to Unix seconds.
pub fn civil_to_unix(y: i64, m: i64, d: i64, hh: i64, mm: i64, ss: i64) -> i64 {
    // Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    days * 86400 + hh * 3600 + mm * 60 + ss
}

/// Converts Unix seconds to `(year, month, day, hour, minute, second)` UTC.
pub fn unix_to_civil(t: i64) -> (i64, i64, i64, i64, i64, i64) {
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
    (y, m, d, secs / 3600, (secs % 3600) / 60, secs % 60)
}

fn iso(t: i64) -> String {
    let (y, m, d, hh, mm, ss) = unix_to_civil(t);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

fn map_zip_err(e: ZipError, what: &str) -> EngineError {
    match e {
        ZipError::InvalidPassword => {
            EngineError::wrong_password("The archive password is incorrect")
        },
        ZipError::UnsupportedArchive(ZipError::PASSWORD_REQUIRED) => {
            EngineError::encrypted("The archive is password protected; provide `password`")
        },
        ZipError::UnsupportedArchive(s) => {
            EngineError::unsupported(format!("{what}: unsupported ZIP feature ({s})"))
        },
        ZipError::InvalidArchive(s) => {
            EngineError::malformed(format!("{what}: invalid ZIP archive ({s})"))
        },
        ZipError::FileNotFound => EngineError::not_found(format!("{what}: entry not found")),
        ZipError::Io(e) => EngineError::malformed(format!("{what}: read error ({e})")),
        other => EngineError::malformed(format!("{what}: {other}")),
    }
}

fn open_zip(bytes: &[u8]) -> EngineResult<zip::ZipArchive<Cursor<&[u8]>>> {
    zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| map_zip_err(e, "open"))
}

fn zip_entries(bytes: &[u8]) -> EngineResult<(Vec<Entry>, Option<String>)> {
    let mut archive = open_zip(bytes)?;
    let comment = String::from_utf8_lossy(archive.comment()).into_owned();
    let mut out = Vec::new();
    for i in 0..archive.len().min(MAX_ENTRIES) {
        let f = archive.by_index_raw(i).map_err(|e| map_zip_err(e, "entry"))?;
        let (name, unsafe_path) = sanitize_name(f.name());
        let (mtime, modified) = match f.last_modified() {
            Some(dt) => {
                let (u, s) = zip_datetime_to_unix(dt);
                (Some(u), Some(s))
            },
            None => (None, None),
        };
        out.push(Entry {
            name,
            is_dir: f.is_dir(),
            size: f.size(),
            compressed_size: Some(f.compressed_size()),
            method: Some(format!("{}", f.compression())),
            crc32: Some(format!("{:08x}", f.crc32())),
            encrypted: f.encrypted(),
            mtime_unix: mtime,
            modified,
            mode: f.unix_mode(),
            unsafe_path,
        });
    }
    Ok((out, if comment.is_empty() { None } else { Some(comment) }))
}

fn tar_bytes(bytes: &[u8], kind: Kind, max_total: u64) -> EngineResult<std::borrow::Cow<'_, [u8]>> {
    Ok(match kind {
        Kind::TarGz => std::borrow::Cow::Owned(gzip::decompress(bytes, max_total)?.0),
        _ => std::borrow::Cow::Borrowed(bytes),
    })
}

fn tar_entries(data: &[u8]) -> EngineResult<Vec<Entry>> {
    let mut ar = tar::Archive::new(Cursor::new(data));
    let mut out = Vec::new();
    for entry in ar
        .entries()
        .map_err(|e| EngineError::malformed(format!("Invalid TAR: {e}")))?
    {
        let entry = entry.map_err(|e| EngineError::malformed(format!("Invalid TAR entry: {e}")))?;
        let header = entry.header();
        let raw = entry
            .path()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let (name, unsafe_path) = sanitize_name(&raw);
        let is_dir = header.entry_type().is_dir() || raw.ends_with('/');
        let mtime = header.mtime().ok().map(|m| m as i64);
        out.push(Entry {
            name: if is_dir && !name.ends_with('/') && !name.is_empty() {
                format!("{name}/")
            } else {
                name
            },
            is_dir,
            size: header.size().unwrap_or(0),
            compressed_size: None,
            method: None,
            crc32: None,
            encrypted: false,
            mtime_unix: mtime,
            modified: mtime.map(iso),
            mode: header.mode().ok(),
            unsafe_path,
        });
        if out.len() >= MAX_ENTRIES {
            break;
        }
    }
    Ok(out)
}

fn summary(entries: &[Entry], kind: Kind, archive_bytes: usize) -> Value {
    let files = entries.iter().filter(|e| !e.is_dir).count();
    let dirs = entries.len() - files;
    let total: u64 = entries.iter().map(|e| e.size).sum();
    let compressed: u64 = entries.iter().filter_map(|e| e.compressed_size).sum();
    json!({
        "kind": kind.name(),
        "files": files,
        "directories": dirs,
        "total_uncompressed_bytes": total,
        "total_compressed_bytes": if kind == Kind::Zip { json!(compressed) } else { Value::Null },
        "archive_bytes": archive_bytes,
        "compression_ratio": if total > 0 { ((archive_bytes as f64 / total as f64) * 1000.0).round() / 1000.0 } else { 1.0 },
        "encrypted_entries": entries.iter().filter(|e| e.encrypted).count(),
        "unsafe_paths": entries.iter().filter(|e| e.unsafe_path).count(),
    })
}

/// Lists entries of any supported archive.
pub fn list(bytes: &[u8], _password: Option<&str>) -> EngineResult<Value> {
    let kind = require_archive(bytes)?;
    let (entries, comment) = match kind {
        Kind::Zip => zip_entries(bytes)?,
        Kind::Tar | Kind::TarGz => {
            (tar_entries(&tar_bytes(bytes, kind, DEFAULT_MAX_TOTAL)?)?, None)
        },
        Kind::Gzip => {
            let (data, name, mtime) = gzip::decompress(bytes, DEFAULT_MAX_TOTAL)?;
            (
                vec![Entry {
                    name: name.unwrap_or_else(|| "file".into()),
                    is_dir: false,
                    size: data.len() as u64,
                    compressed_size: Some(bytes.len() as u64),
                    method: Some("gzip".into()),
                    crc32: None,
                    encrypted: false,
                    mtime_unix: mtime.map(|m| m as i64),
                    modified: mtime.map(|m| iso(m as i64)),
                    mode: None,
                    unsafe_path: false,
                }],
                None,
            )
        },
        // `require_archive` already rejected these kinds.
        Kind::SevenZip | Kind::Rar | Kind::Unknown => return Err(unsupported_kind(kind)),
    };
    let mut v = summary(&entries, kind, bytes.len());
    v["entries"] = json!(entries);
    v["comment"] = json!(comment);
    Ok(v)
}

/// Verifies every entry can be fully decompressed (CRC is checked by the
/// ZIP reader). With a password, also verifies the password.
pub fn test(bytes: &[u8], password: Option<&str>) -> EngineResult<Value> {
    let kind = require_archive(bytes)?;
    let mut failures = Vec::new();
    let mut tested = 0usize;
    let mut total = 0u64;
    let mut password_ok: Option<bool> = None;
    match kind {
        Kind::Zip => {
            let mut archive = open_zip(bytes)?;
            for i in 0..archive.len() {
                let name = archive
                    .by_index_raw(i)
                    .map(|f| f.name().to_string())
                    .unwrap_or_default();
                let encrypted = archive.by_index_raw(i).map(|f| f.encrypted()).unwrap_or(false);
                let result = match (encrypted, password) {
                    (true, Some(pw)) => archive.by_index_decrypt(i, pw.as_bytes()),
                    (true, None) => {
                        failures.push(json!({ "name": name, "error": "password required" }));
                        continue;
                    },
                    _ => archive.by_index(i),
                };
                match result {
                    Ok(mut f) => {
                        tested += 1;
                        let mut sink = std::io::sink();
                        let mut limited = (&mut f).take(DEFAULT_MAX_TOTAL);
                        match std::io::copy(&mut limited, &mut sink) {
                            Ok(n) => {
                                total += n;
                                if encrypted {
                                    password_ok = Some(true);
                                }
                            },
                            Err(e) => {
                                if encrypted && e.to_string().to_lowercase().contains("password") {
                                    password_ok = Some(false);
                                }
                                failures.push(json!({ "name": name, "error": e.to_string() }));
                            },
                        }
                    },
                    Err(ZipError::InvalidPassword) => {
                        password_ok = Some(false);
                        failures.push(json!({ "name": name, "error": "invalid password" }));
                    },
                    Err(e) => failures.push(json!({ "name": name, "error": e.to_string() })),
                }
            }
        },
        Kind::Tar | Kind::TarGz => {
            let data = tar_bytes(bytes, kind, DEFAULT_MAX_TOTAL)?;
            let mut ar = tar::Archive::new(Cursor::new(data.as_ref()));
            for entry in ar
                .entries()
                .map_err(|e| EngineError::malformed(format!("Invalid TAR: {e}")))?
            {
                match entry {
                    Ok(mut e) => {
                        tested += 1;
                        let name =
                            e.path().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
                        match std::io::copy(&mut e, &mut std::io::sink()) {
                            Ok(n) => total += n,
                            Err(err) => {
                                failures.push(json!({ "name": name, "error": err.to_string() }))
                            },
                        }
                    },
                    Err(e) => failures.push(json!({ "name": "?", "error": e.to_string() })),
                }
            }
        },
        Kind::Gzip => {
            tested = 1;
            match gzip::decompress(bytes, DEFAULT_MAX_TOTAL) {
                Ok((d, _, _)) => total = d.len() as u64,
                Err(e) => failures.push(json!({ "name": "gzip stream", "error": e.message })),
            }
        },
        // `require_archive` already rejected these kinds.
        Kind::SevenZip | Kind::Rar | Kind::Unknown => return Err(unsupported_kind(kind)),
    }
    Ok(json!({
        "kind": kind.name(),
        "ok": failures.is_empty(),
        "entries_tested": tested,
        "bytes_verified": total,
        "failures": failures,
        "password_ok": password_ok,
        "password_protected": kind == Kind::Zip && zip_entries(bytes).map(|(e, _)| e.iter().any(|x| x.encrypted)).unwrap_or(false),
    }))
}

#[derive(Serialize)]
struct Extracted {
    #[serde(flatten)]
    entry: Entry,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<String>,
}

/// Extracts entries (all or a selection) with bomb protection.
pub fn extract(
    bytes: &[u8],
    password: Option<&str>,
    only: &[String],
    max_total_bytes: Option<u64>,
    metadata_only: bool,
) -> EngineResult<Value> {
    let kind = require_archive(bytes)?;
    let budget = max_total_bytes
        .unwrap_or(DEFAULT_MAX_TOTAL)
        .min(4 * 1024 * 1024 * 1024);
    let wanted: std::collections::HashSet<&str> = only.iter().map(String::as_str).collect();
    let mut out: Vec<Extracted> = Vec::new();
    let mut total = 0u64;
    let mut skipped = Vec::new();

    let mut push = |entry: Entry, data: Option<Vec<u8>>, total: &mut u64| -> EngineResult<()> {
        if let Some(d) = &data {
            *total += d.len() as u64;
            if *total > budget {
                return Err(EngineError::limit(format!(
                    "Extraction exceeds the {budget} byte budget (possible decompression bomb); \
                     raise `max_total_bytes` or extract fewer entries"
                )));
            }
        }
        out.push(Extracted {
            entry,
            data: data.map(|d| b64::encode(&d)),
        });
        Ok(())
    };

    match kind {
        Kind::Zip => {
            let (entries, _) = zip_entries(bytes)?;
            let mut archive = open_zip(bytes)?;
            for (i, entry) in entries.into_iter().enumerate() {
                if !wanted.is_empty()
                    && !wanted.contains(entry.name.as_str())
                    && !wanted.contains(entry.name.trim_end_matches('/'))
                {
                    continue;
                }
                if entry.is_dir || metadata_only {
                    push(entry, None, &mut total)?;
                    continue;
                }
                if entry.size > budget {
                    return Err(EngineError::limit(format!(
                        "Entry `{}` ({} bytes) exceeds the extraction budget",
                        entry.name, entry.size
                    )));
                }
                let compressed = entry.compressed_size.unwrap_or(1).max(1);
                if entry.size / compressed > MAX_RATIO && entry.size > 1024 * 1024 {
                    skipped.push(
                        json!({ "name": entry.name, "reason": "suspicious compression ratio" }),
                    );
                    continue;
                }
                let mut f = match (entry.encrypted, password) {
                    (true, Some(pw)) => archive.by_index_decrypt(i, pw.as_bytes()),
                    (true, None) => Err(ZipError::UnsupportedArchive(ZipError::PASSWORD_REQUIRED)),
                    _ => archive.by_index(i),
                }
                .map_err(|e| map_zip_err(e, &entry.name))?;
                let mut data = Vec::with_capacity(entry.size.min(64 * 1024 * 1024) as usize);
                (&mut f).take(budget + 1).read_to_end(&mut data).map_err(|e| {
                    if entry.encrypted {
                        EngineError::wrong_password(format!(
                            "`{}`: {e} (wrong password?)",
                            entry.name
                        ))
                    } else {
                        EngineError::malformed(format!("`{}`: {e}", entry.name))
                    }
                })?;
                push(entry, Some(data), &mut total)?;
            }
        },
        Kind::Tar | Kind::TarGz => {
            let data = tar_bytes(bytes, kind, budget)?;
            let mut ar = tar::Archive::new(Cursor::new(data.as_ref()));
            for e in ar
                .entries()
                .map_err(|e| EngineError::malformed(format!("Invalid TAR: {e}")))?
            {
                let mut e =
                    e.map_err(|e| EngineError::malformed(format!("Invalid TAR entry: {e}")))?;
                let raw = e.path().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
                let (name, unsafe_path) = sanitize_name(&raw);
                let is_dir = e.header().entry_type().is_dir() || raw.ends_with('/');
                let name = if is_dir && !name.ends_with('/') && !name.is_empty() {
                    format!("{name}/")
                } else {
                    name
                };
                if !wanted.is_empty()
                    && !wanted.contains(name.as_str())
                    && !wanted.contains(name.trim_end_matches('/'))
                {
                    continue;
                }
                let mtime = e.header().mtime().ok().map(|m| m as i64);
                let entry = Entry {
                    name,
                    is_dir,
                    size: e.header().size().unwrap_or(0),
                    compressed_size: None,
                    method: None,
                    crc32: None,
                    encrypted: false,
                    mtime_unix: mtime,
                    modified: mtime.map(iso),
                    mode: e.header().mode().ok(),
                    unsafe_path,
                };
                if is_dir || metadata_only || !e.header().entry_type().is_file() {
                    push(entry, None, &mut total)?;
                    continue;
                }
                let mut buf = Vec::new();
                (&mut e)
                    .take(budget + 1)
                    .read_to_end(&mut buf)
                    .map_err(|err| EngineError::malformed(format!("`{}`: {err}", entry.name)))?;
                push(entry, Some(buf), &mut total)?;
            }
        },
        Kind::Gzip => {
            let (data, name, mtime) = gzip::decompress(bytes, budget)?;
            let entry = Entry {
                name: name.unwrap_or_else(|| "file".into()),
                is_dir: false,
                size: data.len() as u64,
                compressed_size: Some(bytes.len() as u64),
                method: Some("gzip".into()),
                crc32: None,
                encrypted: false,
                mtime_unix: mtime.map(|m| m as i64),
                modified: mtime.map(|m| iso(m as i64)),
                mode: None,
                unsafe_path: false,
            };
            push(entry, if metadata_only { None } else { Some(data) }, &mut total)?;
        },
        // `require_archive` already rejected these kinds.
        Kind::SevenZip | Kind::Rar | Kind::Unknown => return Err(unsupported_kind(kind)),
    }
    if !wanted.is_empty() && out.is_empty() {
        return Err(EngineError::not_found("None of the requested entries exist in the archive"));
    }
    Ok(json!({
        "kind": kind.name(),
        "entries": out,
        "count": out.len(),
        "total_bytes": total,
        "skipped": skipped
    }))
}

fn zip_mtime(unix: Option<i64>) -> zip::DateTime {
    let t = unix.unwrap_or(315_532_800); // 1980-01-01
    let (y, m, d, hh, mm, ss) = unix_to_civil(t.clamp(315_532_800, 4_354_819_199));
    zip::DateTime::from_date_and_time(y as u16, m as u8, d as u8, hh as u8, mm as u8, ss as u8)
        .unwrap_or_default()
}

/// Builds an archive from in-memory files.
pub fn create(
    files: &[InputFile],
    format: &str,
    level: u8,
    password: Option<&str>,
    method: Option<&str>,
    comment: Option<&str>,
) -> EngineResult<Value> {
    limits::check_count(files.len(), 10_000, "files")?;
    if level > 9 {
        return Err(EngineError::invalid("`level` must be between 0 and 9"));
    }
    let mut seen = std::collections::HashSet::new();
    let mut names = Vec::new();
    for f in files {
        let (name, unsafe_path) = sanitize_name(&f.name);
        if name.is_empty() {
            return Err(EngineError::invalid(format!("Invalid file name `{}`", f.name)));
        }
        if unsafe_path {
            return Err(EngineError::invalid(format!(
                "Unsafe file name `{}` (absolute paths and `..` are not allowed)",
                f.name
            )));
        }
        if !seen.insert(name.clone()) {
            return Err(EngineError::invalid(format!("Duplicate file name `{name}`")));
        }
        names.push(name);
    }
    let total_in: usize = files.iter().map(|f| f.data.len()).sum();
    limits::check_size(total_in, limits::MAX_FILE_BYTES * 2, "total input")?;

    let fmt = format.trim().trim_start_matches('.').to_ascii_lowercase();
    let (bytes, kind): (Vec<u8>, Kind) = match fmt.as_str() {
        "" | "zip" => {
            let mut buf = Cursor::new(Vec::new());
            {
                let mut zw = zip::ZipWriter::new(&mut buf);
                if let Some(c) = comment {
                    zw.set_comment(c);
                }
                let compression = match method.unwrap_or("deflate") {
                    "store" | "stored" | "none" => zip::CompressionMethod::Stored,
                    "deflate" | "deflated" => zip::CompressionMethod::Deflated,
                    other => {
                        return Err(EngineError::invalid(format!(
                            "Unknown ZIP method `{other}` (deflate or store)"
                        )))
                    },
                };
                for (f, name) in files.iter().zip(&names) {
                    let mut opts = zip::write::SimpleFileOptions::default()
                        .compression_method(compression)
                        .compression_level(if compression == zip::CompressionMethod::Stored {
                            None
                        } else {
                            Some(level as i64)
                        })
                        .last_modified_time(zip_mtime(f.mtime_unix))
                        .unix_permissions(f.mode.unwrap_or(if name.ends_with('/') {
                            0o755
                        } else {
                            0o644
                        }))
                        .large_file(f.data.len() as u64 >= 0xFFFF_FFFF);
                    if let Some(pw) = password {
                        opts = opts.with_aes_encryption(zip::AesMode::Aes256, pw);
                    }
                    if name.ends_with('/') {
                        zw.add_directory(name.trim_end_matches('/'), opts)
                            .map_err(|e| EngineError::internal(format!("ZIP write failed: {e}")))?;
                    } else {
                        zw.start_file(name.as_str(), opts)
                            .map_err(|e| EngineError::internal(format!("ZIP write failed: {e}")))?;
                        zw.write_all(&f.data)
                            .map_err(|e| EngineError::internal(format!("ZIP write failed: {e}")))?;
                    }
                }
                zw.finish()
                    .map_err(|e| EngineError::internal(format!("ZIP write failed: {e}")))?;
            }
            (buf.into_inner(), Kind::Zip)
        },
        "tar" | "tar.gz" | "tgz" => {
            if password.is_some() {
                return Err(EngineError::invalid(
                    "TAR archives cannot be password protected; use ZIP",
                ));
            }
            let mut tb = tar::Builder::new(Vec::new());
            for (f, name) in files.iter().zip(&names) {
                let mut header = tar::Header::new_gnu();
                header.set_mtime(f.mtime_unix.unwrap_or(0).max(0) as u64);
                if name.ends_with('/') {
                    header.set_entry_type(tar::EntryType::Directory);
                    header.set_mode(f.mode.unwrap_or(0o755));
                    header.set_size(0);
                    tb.append_data(&mut header, name.trim_end_matches('/'), std::io::empty())
                        .map_err(|e| EngineError::internal(format!("TAR write failed: {e}")))?;
                } else {
                    header.set_entry_type(tar::EntryType::Regular);
                    header.set_mode(f.mode.unwrap_or(0o644));
                    header.set_size(f.data.len() as u64);
                    tb.append_data(&mut header, name.as_str(), Cursor::new(&f.data))
                        .map_err(|e| EngineError::internal(format!("TAR write failed: {e}")))?;
                }
            }
            let tar = tb
                .into_inner()
                .map_err(|e| EngineError::internal(format!("TAR write failed: {e}")))?;
            if fmt == "tar" {
                (tar, Kind::Tar)
            } else {
                (gzip::compress(&tar, level, None, None)?, Kind::TarGz)
            }
        },
        "gz" | "gzip" => {
            if files.len() != 1 {
                return Err(EngineError::invalid(
                    "gzip compresses exactly one file; use tar.gz or zip for several",
                ));
            }
            (
                gzip::compress(
                    &files[0].data,
                    level,
                    Some(names[0].as_str()),
                    files[0].mtime_unix.map(|m| m.max(0) as u32),
                )?,
                Kind::Gzip,
            )
        },
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown archive format `{other}` (zip, tar, tar.gz, gz)"
            )))
        },
    };
    let suggested = match kind {
        Kind::Zip => "archive.zip",
        Kind::Tar => "archive.tar",
        Kind::TarGz => "archive.tar.gz",
        Kind::Gzip => "file.gz",
        _ => "archive",
    };
    Ok(json!({
        "output": {
            "file": b64::encode(&bytes),
            "mime": kind.mime(),
            "name": if kind == Kind::Gzip { format!("{}.gz", names[0]) } else { suggested.to_string() },
            "bytes": bytes.len()
        },
        "kind": kind.name(),
        "files": names.len(),
        "bytes_in": total_in,
        "bytes_out": bytes.len(),
        "ratio": if total_in > 0 { ((bytes.len() as f64 / total_in as f64) * 1000.0).round() / 1000.0 } else { 1.0 },
        "encrypted": password.is_some() && kind == Kind::Zip,
    }))
}

/// Converts between archive formats (extract + create).
pub fn convert(
    bytes: &[u8],
    password: Option<&str>,
    to: &str,
    level: u8,
    new_password: Option<&str>,
) -> EngineResult<Value> {
    let extracted = extract(bytes, password, &[], None, false)?;
    let mut files = Vec::new();
    for e in extracted["entries"].as_array().unwrap_or(&vec![]) {
        let name = e["name"].as_str().unwrap_or("").to_string();
        if e["is_dir"].as_bool().unwrap_or(false) {
            files.push(InputFile {
                name,
                data: vec![],
                mtime_unix: e["mtime_unix"].as_i64(),
                mode: e["mode"].as_u64().map(|m| m as u32),
            });
            continue;
        }
        let data = match e["data"].as_str() {
            Some(d) => b64::decode(d)?,
            None => continue,
        };
        files.push(InputFile {
            name,
            data,
            mtime_unix: e["mtime_unix"].as_i64(),
            mode: e["mode"].as_u64().map(|m| m as u32),
        });
    }
    if files.is_empty() {
        return Err(EngineError::invalid("The source archive contains no files"));
    }
    let mut result = create(&files, to, level, new_password, None, None)?;
    result["source_kind"] = extracted["kind"].clone();
    Ok(result)
}

/// Rebuilds a ZIP whose central directory is missing/corrupt by scanning
/// local file headers sequentially.
pub fn zip_repair(bytes: &[u8]) -> EngineResult<Value> {
    if bytes.is_empty() {
        return Err(EngineError::empty("archive file"));
    }
    if bytes.len() < 4 || &bytes[..2] != b"PK" {
        return Err(EngineError::unsupported_format("Not a ZIP archive (no PK signature)"));
    }
    let healthy = open_zip(bytes).is_ok();
    let mut recovered: Vec<InputFile> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    let mut cursor = Cursor::new(bytes);
    loop {
        match zip::read::read_zipfile_from_stream(&mut cursor) {
            Ok(Some(mut f)) => {
                let (name, unsafe_path) = sanitize_name(f.name());
                if unsafe_path || name.is_empty() {
                    errors.push(format!("Skipped entry with unsafe name `{}`", f.name()));
                    continue;
                }
                if f.is_dir() {
                    recovered.push(InputFile {
                        name,
                        data: vec![],
                        mtime_unix: f.last_modified().map(|d| zip_datetime_to_unix(d).0),
                        mode: f.unix_mode(),
                    });
                    continue;
                }
                if f.encrypted() {
                    errors.push(format!(
                        "Skipped encrypted entry `{name}` (cannot be repaired without password)"
                    ));
                    continue;
                }
                let mut data = Vec::new();
                match (&mut f).take(DEFAULT_MAX_TOTAL).read_to_end(&mut data) {
                    Ok(_) => recovered.push(InputFile {
                        name,
                        data,
                        mtime_unix: f.last_modified().map(|d| zip_datetime_to_unix(d).0),
                        mode: f.unix_mode(),
                    }),
                    Err(e) => errors.push(format!("Entry `{name}` is damaged: {e}")),
                }
            },
            Ok(None) => break,
            Err(e) => {
                errors.push(format!("Stopped scanning: {e}"));
                break;
            },
        }
    }
    if recovered.is_empty() {
        return Err(EngineError::malformed("No entries could be recovered from the ZIP"));
    }
    let mut result = create(&recovered, "zip", 6, None, None, None)?;
    result["recovered_entries"] = json!(recovered.len());
    result["was_readable"] = json!(healthy);
    result["errors"] = json!(errors);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{sample_files, zip_of};

    #[test]
    fn names_and_detection() {
        assert_eq!(sanitize_name("a/b/../c.txt"), ("a/c.txt".into(), true));
        assert_eq!(sanitize_name("/etc/passwd"), ("etc/passwd".into(), true));
        assert_eq!(sanitize_name("C:\\x\\y.txt"), ("x/y.txt".into(), true));
        assert_eq!(sanitize_name("dir/"), ("dir/".into(), false));
        assert_eq!(sanitize_name("./ok/file.txt"), ("ok/file.txt".into(), false));
        assert_eq!(detect(b"PK\x03\x04rest"), Kind::Zip);
        assert_eq!(detect(b"7z\xBC\xAF\x27\x1C"), Kind::SevenZip);
        assert_eq!(detect(b"Rar!\x1A\x07\x00"), Kind::Rar);
        assert_eq!(detect(b"hello"), Kind::Unknown);
        assert_eq!(civil_to_unix(2024, 1, 31, 10, 0, 0), 1_706_695_200);
        assert_eq!(unix_to_civil(1_706_695_200), (2024, 1, 31, 10, 0, 0));
    }

    #[test]
    fn zip_cycle() {
        let files = sample_files();
        let c = create(&files, "zip", 6, None, None, Some("hello")).unwrap();
        assert_eq!(c["files"], 3);
        let z = b64::decode(c["output"]["file"].as_str().unwrap()).unwrap();
        let l = list(&z, None).unwrap();
        assert_eq!(l["files"], 2);
        assert_eq!(l["directories"], 1);
        assert_eq!(l["comment"], "hello");
        assert_eq!(l["entries"][0]["modified"], "2024-01-31T10:00:00");
        let t = test(&z, None).unwrap();
        assert_eq!(t["ok"], true);
        let e = extract(&z, None, &[], None, false).unwrap();
        assert_eq!(e["count"], 3);
        let data = b64::decode(e["entries"][0]["data"].as_str().unwrap()).unwrap();
        assert_eq!(data, b"hello world");
        let only = extract(&z, None, &["docs/readme.md".into()], None, false).unwrap();
        assert_eq!(only["count"], 1);
        assert!(extract(&z, None, &["nope".into()], None, false).is_err());
        assert_eq!(
            extract(&z, None, &[], Some(5), false).unwrap_err().code,
            engine_shared::error::ErrorCode::LimitExceeded
        );
        let meta = extract(&z, None, &[], None, true).unwrap();
        assert!(meta["entries"][0]["data"].is_null());
        // store method
        let s = create(&files, "zip", 0, None, Some("store"), None).unwrap();
        assert!(s["bytes_out"].as_u64().unwrap() > c["bytes_out"].as_u64().unwrap());
        assert!(create(&files, "zip", 6, None, Some("lzma"), None).is_err());
        assert!(create(&[], "zip", 6, None, None, None).is_err());
        let dup = vec![files[0].clone(), files[0].clone()];
        assert!(create(&dup, "zip", 6, None, None, None).is_err());
        let bad = vec![InputFile {
            name: "../evil".into(),
            data: vec![],
            mtime_unix: None,
            mode: None,
        }];
        assert!(create(&bad, "zip", 6, None, None, None).is_err());
    }

    #[test]
    fn zip_passwords() {
        let files = sample_files();
        let c = create(&files, "zip", 6, Some("s3cret"), None, None).unwrap();
        assert_eq!(c["encrypted"], true);
        let z = b64::decode(c["output"]["file"].as_str().unwrap()).unwrap();
        let l = list(&z, None).unwrap();
        assert_eq!(l["encrypted_entries"], 2);
        assert_eq!(
            extract(&z, None, &[], None, false).unwrap_err().code,
            engine_shared::error::ErrorCode::Encrypted
        );
        assert_eq!(
            extract(&z, Some("wrong"), &[], None, false).unwrap_err().code,
            engine_shared::error::ErrorCode::WrongPassword
        );
        let e = extract(&z, Some("s3cret"), &[], None, false).unwrap();
        assert_eq!(b64::decode(e["entries"][0]["data"].as_str().unwrap()).unwrap(), b"hello world");
        let t = test(&z, Some("wrong")).unwrap();
        assert_eq!(t["ok"], false);
        assert_eq!(t["password_ok"], false);
        let t = test(&z, Some("s3cret")).unwrap();
        assert_eq!(t["password_ok"], true);
        assert_eq!(t["password_protected"], true);
        let t = test(&z, None).unwrap();
        assert_eq!(t["ok"], false);
    }

    #[test]
    fn tar_and_gzip_cycle() {
        let files = sample_files();
        for fmt in ["tar", "tar.gz"] {
            let c = create(&files, fmt, 6, None, None, None).unwrap();
            let t = b64::decode(c["output"]["file"].as_str().unwrap()).unwrap();
            assert_eq!(detect(&t).name(), fmt);
            let l = list(&t, None).unwrap();
            assert_eq!(l["files"], 2, "{fmt}");
            assert_eq!(test(&t, None).unwrap()["ok"], true);
            let e = extract(&t, None, &[], None, false).unwrap();
            assert_eq!(
                b64::decode(e["entries"][0]["data"].as_str().unwrap()).unwrap(),
                b"hello world"
            );
            assert_eq!(e["entries"][0]["mtime_unix"], 1_706_695_200);
        }
        let g = create(&files[..1], "gz", 9, None, None, None).unwrap();
        assert_eq!(g["output"]["name"], "a.txt.gz");
        let gz = b64::decode(g["output"]["file"].as_str().unwrap()).unwrap();
        assert_eq!(list(&gz, None).unwrap()["entries"][0]["name"], "a.txt");
        let e = extract(&gz, None, &[], None, false).unwrap();
        assert_eq!(b64::decode(e["entries"][0]["data"].as_str().unwrap()).unwrap(), b"hello world");
        assert!(create(&files, "gz", 6, None, None, None).is_err());
        assert!(create(&files, "tar", 6, Some("pw"), None, None).is_err());
        assert!(create(&files, "7z", 6, None, None, None).is_err());
    }

    #[test]
    fn convert_and_repair() {
        let z = zip_of(&sample_files());
        let c = convert(&z, None, "tar.gz", 6, None).unwrap();
        assert_eq!(c["kind"], "tar.gz");
        assert_eq!(c["source_kind"], "zip");
        let tgz = b64::decode(c["output"]["file"].as_str().unwrap()).unwrap();
        let back = convert(&tgz, None, "zip", 6, Some("pw")).unwrap();
        assert_eq!(back["encrypted"], true);

        // Corrupt the central directory (truncate the tail).
        let truncated = &z[..z.len() - 30];
        assert!(open_zip(truncated).is_err());
        let r = zip_repair(truncated).unwrap();
        assert_eq!(r["recovered_entries"], 3);
        assert_eq!(r["was_readable"], false);
        let fixed = b64::decode(r["output"]["file"].as_str().unwrap()).unwrap();
        assert_eq!(test(&fixed, None).unwrap()["ok"], true);
        assert!(zip_repair(b"hello").is_err());
        assert!(zip_repair(b"").is_err());
    }

    #[test]
    fn invalid_inputs() {
        assert_eq!(list(b"", None).unwrap_err().code, engine_shared::error::ErrorCode::EmptyInput);
        assert_eq!(
            list(b"not an archive at all", None).unwrap_err().code,
            engine_shared::error::ErrorCode::UnsupportedFormat
        );
        assert_eq!(
            list(b"7z\xBC\xAF\x27\x1C\x00", None).unwrap_err().code,
            engine_shared::error::ErrorCode::Unsupported
        );
        assert_eq!(
            list(b"PK\x03\x04garbage", None).unwrap_err().code,
            engine_shared::error::ErrorCode::Malformed
        );
        assert_eq!(detect_report(b"PK\x03\x04").unwrap()["kind"], "zip");
    }
}
