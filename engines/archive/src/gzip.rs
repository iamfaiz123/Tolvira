//! GZIP helpers (single member streams) with bomb protection.

use engine_shared::{EngineError, EngineResult};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::{Read, Write};

/// `true` when the bytes start with the gzip magic number.
pub fn is_gzip(bytes: &[u8]) -> bool {
    bytes.len() >= 3 && bytes[0] == 0x1F && bytes[1] == 0x8B && bytes[2] == 8
}

/// Compresses `data` with an optional original filename and mtime in the header.
pub fn compress(
    data: &[u8],
    level: u8,
    filename: Option<&str>,
    mtime: Option<u32>,
) -> EngineResult<Vec<u8>> {
    let level = level.min(9) as u32;
    let mut builder = flate2::GzBuilder::new();
    if let Some(name) = filename.filter(|n| !n.is_empty() && n.is_ascii()) {
        builder = builder.filename(name.as_bytes());
    }
    if let Some(t) = mtime {
        builder = builder.mtime(t);
    }
    let mut enc: GzEncoder<Vec<u8>> = builder.write(Vec::new(), Compression::new(level));
    enc.write_all(data)
        .map_err(|e| EngineError::internal(format!("gzip failed: {e}")))?;
    enc.finish()
        .map_err(|e| EngineError::internal(format!("gzip failed: {e}")))
}

/// Decompresses up to `max_bytes` of output; returns (data, original filename).
pub fn decompress(
    bytes: &[u8],
    max_bytes: u64,
) -> EngineResult<(Vec<u8>, Option<String>, Option<u32>)> {
    if !is_gzip(bytes) {
        return Err(EngineError::unsupported_format("Not a gzip stream (missing 1F 8B 08 header)"));
    }
    let mut dec = GzDecoder::new(bytes);
    let header = dec.header().cloned();
    let filename = header
        .as_ref()
        .and_then(|h| h.filename())
        .map(|f| String::from_utf8_lossy(f).into_owned());
    let mtime = header.as_ref().map(|h| h.mtime()).filter(|m| *m > 0);
    let mut out = Vec::new();
    let mut limited = (&mut dec).take(max_bytes + 1);
    limited
        .read_to_end(&mut out)
        .map_err(|e| EngineError::malformed(format!("gzip data is corrupt: {e}")))?;
    if out.len() as u64 > max_bytes {
        return Err(EngineError::limit(format!(
            "Decompressed data exceeds the {max_bytes} byte budget (possible decompression bomb)"
        )));
    }
    Ok((out, filename, mtime))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_limits() {
        let data = b"hello hello hello hello".repeat(100);
        let gz = compress(&data, 6, Some("hello.txt"), Some(1_700_000_000)).unwrap();
        assert!(is_gzip(&gz));
        let (back, name, mtime) = decompress(&gz, 1 << 20).unwrap();
        assert_eq!(back, data);
        assert_eq!(name.as_deref(), Some("hello.txt"));
        assert_eq!(mtime, Some(1_700_000_000));
        assert_eq!(
            decompress(&gz, 10).unwrap_err().code,
            engine_shared::error::ErrorCode::LimitExceeded
        );
        assert!(decompress(b"nope", 100).is_err());
        let mut corrupt = gz.clone();
        let n = corrupt.len();
        corrupt[n / 2] ^= 0xFF;
        assert!(decompress(&corrupt, 1 << 20).is_err());
    }
}
