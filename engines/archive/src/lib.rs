//! # Dev Toolbox Archive Engine (`archive-engine`)
//!
//! ZIP (Deflate/Store, ZipCrypto + AES-256 passwords), TAR, TAR.GZ and GZIP
//! handled fully in WebAssembly. Every extraction is guarded against
//! decompression bombs (`max_total_bytes`, per-entry ratio) and path
//! traversal (entries are returned with sanitised relative names).
//!
//! | Action | Tool(s) |
//! |---|---|
//! | `detect` | archive type sniffing |
//! | `list` | zip-view, archive-list-files (auto-detect zip / tar / tar.gz / gz) |
//! | `test` | archive-test-integrity, zip-password-check |
//! | `extract` | zip-extract, tar-extract, gzip-decompress (auto-detect) |
//! | `create` | zip-create, tar-create, gzip-compress (`format`: zip, tar, tar.gz, gz) |
//! | `convert` | file-archive-converter |
//! | `zip_repair` | zip-repair (rebuilds from local headers when the central directory is damaged) |
//! | `seven_zip_extract` / `rar_extract` | `E_UNSUPPORTED` with guidance |
//!
//! Modification times are taken from the caller (`mtime_unix`) because WASM
//! has no clock; they default to 1980-01-01 (ZIP) / 0 (TAR).

pub mod archive;
pub mod gzip;
#[cfg(any(test, feature = "testutil"))]
pub mod testutil;

use engine_shared::{b64, EngineError, EngineResult};
use serde::Deserialize;
use serde_json::Value;

/// A file to add to an archive.
#[derive(Deserialize, Debug, Clone)]
pub struct InputFile {
    /// Entry path inside the archive (`/` separators).
    pub name: String,
    /// File contents (base64 on the wire).
    #[serde(with = "b64")]
    pub data: Vec<u8>,
    /// Modification time as Unix seconds (default: now / epoch).
    #[serde(default)]
    pub mtime_unix: Option<i64>,
    /// Unix permission bits (e.g. 0o644). Defaults to 0o644 (0o755 for dirs).
    #[serde(default)]
    pub mode: Option<u32>,
}

fn default_level() -> u8 {
    6
}

/// Every action understood by the archive engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum ArchiveAction {
    /// Detects the archive kind.
    Detect {
        /// Archive bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
    },
    /// Lists entries without extracting.
    List {
        /// Archive bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted ZIPs.
        #[serde(default)]
        password: Option<String>,
    },
    /// Verifies CRCs / decompresses every entry and reports failures.
    Test {
        /// Archive bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted ZIPs.
        #[serde(default)]
        password: Option<String>,
    },
    /// Extracts entries (all or a selection) within a size budget.
    Extract {
        /// Archive bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted ZIPs.
        #[serde(default)]
        password: Option<String>,
        /// Only extract these entry names (all when empty).
        #[serde(default)]
        entries: Vec<String>,
        /// Total uncompressed budget in bytes (default 512 MiB).
        #[serde(default)]
        max_total_bytes: Option<u64>,
        /// Skip returning file data (metadata only) – useful for previews.
        #[serde(default)]
        metadata_only: bool,
    },
    /// Creates a ZIP / TAR / TAR.GZ / GZ archive from files.
    Create {
        /// Files to add.
        files: Vec<InputFile>,
        /// `zip` (default), `tar`, `tar.gz` / `tgz`, `gz` (single file).
        #[serde(default)]
        format: String,
        /// Compression level 0–9.
        #[serde(default = "default_level")]
        level: u8,
        /// ZIP only: AES-256 password protection.
        #[serde(default)]
        password: Option<String>,
        /// ZIP only: `deflate` (default) or `store`.
        #[serde(default)]
        method: Option<String>,
        /// ZIP archive comment.
        #[serde(default)]
        comment: Option<String>,
    },
    /// Re-packs an archive in another format.
    Convert {
        /// Archive bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password of the source ZIP.
        #[serde(default)]
        password: Option<String>,
        /// Target format (`zip`, `tar`, `tar.gz`, `gz`).
        to: String,
        /// Compression level 0–9.
        #[serde(default = "default_level")]
        level: u8,
        /// Password for the new ZIP (AES-256).
        #[serde(default)]
        new_password: Option<String>,
    },
    /// Salvages readable entries from a damaged ZIP by scanning local headers.
    ZipRepair {
        /// Archive bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
    },
    /// Always returns `E_UNSUPPORTED` (7z needs LZMA2/BCJ filters; use the backend).
    SevenZipExtract {},
    /// Always returns `E_UNSUPPORTED` (RAR is a proprietary format; use the backend).
    RarExtract {},
}

/// Dispatches an [`ArchiveAction`] to its implementation.
pub fn handle_action(action: ArchiveAction) -> EngineResult<Value> {
    let pw = |p: &Option<String>| -> Option<String> { p.clone().filter(|s| !s.is_empty()) };
    match action {
        ArchiveAction::Detect { file } => archive::detect_report(&file),
        ArchiveAction::List { file, password } => archive::list(&file, pw(&password).as_deref()),
        ArchiveAction::Test { file, password } => archive::test(&file, pw(&password).as_deref()),
        ArchiveAction::Extract {
            file,
            password,
            entries,
            max_total_bytes,
            metadata_only,
        } => archive::extract(
            &file,
            pw(&password).as_deref(),
            &entries,
            max_total_bytes,
            metadata_only,
        ),
        ArchiveAction::Create {
            files,
            format,
            level,
            password,
            method,
            comment,
        } => archive::create(
            &files,
            &format,
            level,
            pw(&password).as_deref(),
            method.as_deref(),
            comment.as_deref(),
        ),
        ArchiveAction::Convert { file, password, to, level, new_password } => archive::convert(
            &file,
            pw(&password).as_deref(),
            &to,
            level,
            pw(&new_password).as_deref(),
        ),
        ArchiveAction::ZipRepair { file } => archive::zip_repair(&file),
        ArchiveAction::SevenZipExtract {} => Err(EngineError::unsupported(
            "7z extraction is not available: the pure-Rust 7z crates require JavaScript bindings \
             on wasm32 which this engine cannot import. Use ZIP/TAR, or a native desktop tool.",
        )),
        ArchiveAction::RarExtract {} => Err(EngineError::unsupported(
            "RAR is a proprietary format without a pure-Rust decoder; extraction is not supported \
             in the browser engine.",
        )),
    }
}

engine_shared::export_engine!(handle_action, ArchiveAction);
