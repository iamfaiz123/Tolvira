//! Structured engine errors with stable, machine readable codes.
//!
//! Every failure returned to the frontend carries a `code` from the table
//! below plus a human readable, actionable `message`. UI code should branch on
//! the code and display the message.

use std::fmt;

/// Stable error codes shared by all engines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// The request JSON could not be parsed or the action is unknown.
    BadRequest,
    /// A required input is empty (empty text, empty file, empty list).
    EmptyInput,
    /// An input value is present but invalid (range, format, enum...).
    InvalidInput,
    /// Input bytes are not the format the tool expects (not a PDF, not an image...).
    UnsupportedFormat,
    /// Input is the right format but is corrupt / truncated / malformed.
    Malformed,
    /// Input is encrypted and the password is missing or incorrect.
    Encrypted,
    /// A provided password / key is wrong.
    WrongPassword,
    /// Input exceeds a size or count limit.
    LimitExceeded,
    /// A referenced element (page, file, column, key) does not exist.
    NotFound,
    /// The requested operation is not supported by this engine / build.
    Unsupported,
    /// Secure randomness was required but no entropy was supplied.
    EntropyRequired,
    /// Unexpected internal failure (bug or library error).
    Internal,
}

impl ErrorCode {
    /// Stable string form used on the wire.
    pub const fn as_str(self) -> &'static str {
        match self {
            ErrorCode::BadRequest => "E_BAD_REQUEST",
            ErrorCode::EmptyInput => "E_EMPTY_INPUT",
            ErrorCode::InvalidInput => "E_INVALID_INPUT",
            ErrorCode::UnsupportedFormat => "E_UNSUPPORTED_FORMAT",
            ErrorCode::Malformed => "E_MALFORMED",
            ErrorCode::Encrypted => "E_ENCRYPTED",
            ErrorCode::WrongPassword => "E_WRONG_PASSWORD",
            ErrorCode::LimitExceeded => "E_LIMIT_EXCEEDED",
            ErrorCode::NotFound => "E_NOT_FOUND",
            ErrorCode::Unsupported => "E_UNSUPPORTED",
            ErrorCode::EntropyRequired => "E_ENTROPY_REQUIRED",
            ErrorCode::Internal => "E_INTERNAL",
        }
    }
}

/// Error type returned by every engine action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineError {
    /// Machine-readable code (serialised as `error_code`).
    pub code: ErrorCode,
    /// Human-readable explanation (serialised as `error`).
    pub message: String,
}

/// Convenience alias used by all engine handlers.
pub type EngineResult<T> = Result<T, EngineError>;

impl EngineError {
    /// Builds an error from a code and message.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
    /// Request envelope / payload could not be parsed (`E_BAD_REQUEST`).
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::BadRequest, msg)
    }
    /// A required input (`what`) was missing or empty (`E_EMPTY_INPUT`).
    pub fn empty(what: &str) -> Self {
        Self::new(ErrorCode::EmptyInput, format!("{what} must not be empty"))
    }
    /// Input is present but semantically invalid (`E_INVALID_INPUT`).
    pub fn invalid(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidInput, msg)
    }
    /// Bytes are not in a format this action handles (`E_UNSUPPORTED_FORMAT`).
    pub fn unsupported_format(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::UnsupportedFormat, msg)
    }
    /// Format recognised but the data is corrupt/truncated (`E_MALFORMED`).
    pub fn malformed(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::Malformed, msg)
    }
    /// Document is encrypted and no password was supplied (`E_ENCRYPTED`).
    pub fn encrypted(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::Encrypted, msg)
    }
    /// Supplied password does not open the document (`E_WRONG_PASSWORD`).
    pub fn wrong_password(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::WrongPassword, msg)
    }
    /// A size/count/time limit was exceeded (`E_LIMIT_EXCEEDED`).
    pub fn limit(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::LimitExceeded, msg)
    }
    /// A referenced item (page, key, entry…) does not exist (`E_NOT_FOUND`).
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, msg)
    }
    /// Feature is deliberately not implemented in WASM (`E_UNSUPPORTED`).
    pub fn unsupported(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unsupported, msg)
    }
    /// Action needs caller-supplied randomness (`E_ENTROPY_REQUIRED`).
    pub fn entropy_required() -> Self {
        Self::new(
            ErrorCode::EntropyRequired,
            "This action needs secure randomness. Pass `entropy` (base64 of >= 16 bytes from \
             crypto.getRandomValues) in the payload.",
        )
    }
    /// Unexpected failure that indicates a bug (`E_INTERNAL`).
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, msg)
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for EngineError {}

impl From<serde_json::Error> for EngineError {
    fn from(e: serde_json::Error) -> Self {
        EngineError::internal(format!("JSON serialisation failed: {e}"))
    }
}

impl From<std::io::Error> for EngineError {
    fn from(e: std::io::Error) -> Self {
        EngineError::internal(format!("I/O error: {e}"))
    }
}

/// Helper: validate that a string input is non-empty (after trimming) and
/// return a structured error otherwise.
pub fn require_text<'a>(value: &'a str, what: &str) -> EngineResult<&'a str> {
    if value.trim().is_empty() {
        Err(EngineError::empty(what))
    } else {
        Ok(value)
    }
}

/// Helper: validate that a byte input is non-empty.
pub fn require_bytes<'a>(value: &'a [u8], what: &str) -> EngineResult<&'a [u8]> {
    if value.is_empty() {
        Err(EngineError::empty(what))
    } else {
        Ok(value)
    }
}

/// Helper: validate that a float is finite and inside `[min, max]`.
pub fn require_range(value: f64, min: f64, max: f64, what: &str) -> EngineResult<f64> {
    if !value.is_finite() {
        return Err(EngineError::invalid(format!("{what} must be a finite number")));
    }
    if value < min || value > max {
        return Err(EngineError::invalid(format!(
            "{what} must be between {min} and {max} (got {value})"
        )));
    }
    Ok(value)
}

/// Helper: validate that a float is finite and strictly positive.
pub fn require_positive(value: f64, what: &str) -> EngineResult<f64> {
    if !value.is_finite() || value <= 0.0 {
        return Err(EngineError::invalid(format!(
            "{what} must be a positive number (got {value})"
        )));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_stable() {
        assert_eq!(ErrorCode::EmptyInput.as_str(), "E_EMPTY_INPUT");
        assert_eq!(
            EngineError::empty("text").to_string(),
            "[E_EMPTY_INPUT] text must not be empty"
        );
    }

    #[test]
    fn validators() {
        assert!(require_text("  ", "x").is_err());
        assert!(require_text("a", "x").is_ok());
        assert!(require_bytes(&[], "x").is_err());
        assert!(require_range(f64::NAN, 0.0, 1.0, "x").is_err());
        assert!(require_range(2.0, 0.0, 1.0, "x").is_err());
        assert_eq!(require_range(0.5, 0.0, 1.0, "x").unwrap(), 0.5);
        assert!(require_positive(0.0, "x").is_err());
        assert!(require_positive(-1.0, "x").is_err());
        assert!(require_positive(1.0, "x").is_ok());
    }
}
