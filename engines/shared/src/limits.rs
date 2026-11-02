//! Input size guards shared by the file based engines.
//!
//! The JSON envelope transports files as base64 (≈ 1.33× overhead) and the
//! whole request lives in WASM linear memory, so engines enforce explicit
//! limits and return `E_LIMIT_EXCEEDED` instead of exhausting memory.

use crate::error::{EngineError, EngineResult};

/// Default maximum size for a single uploaded file (bytes).
pub const MAX_FILE_BYTES: usize = 256 * 1024 * 1024;
/// Default maximum number of files in a batch operation.
pub const MAX_BATCH_FILES: usize = 500;
/// Default maximum text length (characters) for text processing tools.
pub const MAX_TEXT_CHARS: usize = 50 * 1024 * 1024;
/// Default maximum image dimension (pixels) on either axis.
pub const MAX_IMAGE_DIMENSION: u32 = 16_384;
/// Default maximum pixel count for decoded images (~268 MP).
pub const MAX_IMAGE_PIXELS: u64 = 268_435_456;
/// Default maximum page count for PDF operations.
pub const MAX_PDF_PAGES: usize = 10_000;

/// Checks that `len <= max`.
pub fn check_size(len: usize, max: usize, what: &str) -> EngineResult<()> {
    if len > max {
        return Err(EngineError::limit(format!(
            "{what} is too large ({} bytes, limit {} bytes)",
            len, max
        )));
    }
    Ok(())
}

/// Checks that a collection has between `1` and `max` items.
pub fn check_count(count: usize, max: usize, what: &str) -> EngineResult<()> {
    if count == 0 {
        return Err(EngineError::empty(what));
    }
    if count > max {
        return Err(EngineError::limit(format!(
            "{what} has too many items ({count}, limit {max})"
        )));
    }
    Ok(())
}

/// Checks image dimensions against the shared limits.
pub fn check_dimensions(width: u32, height: u32) -> EngineResult<()> {
    if width == 0 || height == 0 {
        return Err(EngineError::invalid("Image dimensions must be greater than zero"));
    }
    if width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION {
        return Err(EngineError::limit(format!(
            "Image dimension {width}x{height} exceeds the {MAX_IMAGE_DIMENSION}px limit"
        )));
    }
    if (width as u64) * (height as u64) > MAX_IMAGE_PIXELS {
        return Err(EngineError::limit(format!(
            "Image has too many pixels ({}x{}, limit {} pixels)",
            width, height, MAX_IMAGE_PIXELS
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits() {
        assert!(check_size(10, 5, "file").is_err());
        assert!(check_size(5, 5, "file").is_ok());
        assert!(check_count(0, 5, "files").is_err());
        assert!(check_count(6, 5, "files").is_err());
        assert!(check_count(3, 5, "files").is_ok());
        assert!(check_dimensions(0, 10).is_err());
        assert!(check_dimensions(20_000, 10).is_err());
        assert!(check_dimensions(16_384, 16_384).is_ok());
        assert!(check_dimensions(16_000, 16_384).is_ok());
    }
}
