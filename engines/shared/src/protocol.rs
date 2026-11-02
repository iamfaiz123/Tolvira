//! Request / response envelope shared by every engine.
//!
//! Request (written by `engine.worker.ts` / `engine-loader.ts`):
//!
//! ```json
//! { "action": "resize", "payload": { "file": "<base64>", "width": 100, "entropy": "<base64>" } }
//! ```
//!
//! Response:
//!
//! ```json
//! { "success": true,  "data": { ... } }
//! { "success": false, "error": "human readable", "error_code": "E_INVALID_INPUT" }
//! ```
//!
//! `error_code` is an additive field; the existing worker only reads
//! `success`, `data` and `error`, so the envelope stays backward compatible.

use crate::error::{EngineError, EngineResult};
use crate::rng;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

/// Wire format of an engine response.
#[derive(Serialize, Debug)]
pub struct EngineResponse<T: Serialize> {
    /// `true` when `data` is populated, `false` when `error`/`error_code` are.
    pub success: bool,
    /// Action result (only on success).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    /// Human-readable error message (only on failure).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Stable error code such as `E_INVALID_INPUT` (only on failure).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<&'static str>,
}

impl<T: Serialize> EngineResponse<T> {
    /// Wraps a successful result.
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            error_code: None,
        }
    }

    /// Wraps an [`EngineError`].
    pub fn err(e: EngineError) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(e.message),
            error_code: Some(e.code.as_str()),
        }
    }

    /// Serialises the response; never panics (falls back to a hand written
    /// JSON literal if serialisation itself fails).
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_else(|e| {
            format!(
                r#"{{"success":false,"error":"Failed to serialise response: {}","error_code":"E_INTERNAL"}}"#,
                e.to_string().replace('"', "'")
            )
            .into_bytes()
        })
    }
}

/// Parses the request envelope, dispatches it to `handler` and serialises the
/// result. This is what every engine's exported `execute` calls.
///
/// * Empty input → `E_BAD_REQUEST`.
/// * Unknown action / invalid payload → `E_BAD_REQUEST` with serde's message.
/// * Before dispatch, the optional `payload.entropy` (base64) field is mixed
///   into the shared RNG (see [`crate::rng`]).
pub fn run_request<A, F>(input: &[u8], handler: F) -> Vec<u8>
where
    A: DeserializeOwned,
    F: FnOnce(A) -> EngineResult<Value>,
{
    let response = match dispatch(input, handler) {
        Ok(data) => EngineResponse::ok(data),
        Err(e) => EngineResponse::<Value>::err(e),
    };
    response.to_bytes()
}

fn dispatch<A, F>(input: &[u8], handler: F) -> EngineResult<Value>
where
    A: DeserializeOwned,
    F: FnOnce(A) -> EngineResult<Value>,
{
    if input.iter().all(|b| b.is_ascii_whitespace()) {
        return Err(EngineError::bad_request(
            "Request body is empty. Expected JSON: {\"action\": \"...\", \"payload\": {...}}",
        ));
    }

    // First pass: generic JSON so we can pull out `payload.entropy` and give
    // precise error messages for missing `action` / `payload` fields.
    let raw: Value = serde_json::from_slice(input)
        .map_err(|e| EngineError::bad_request(format!("Request is not valid JSON: {e}")))?;

    let obj = raw
        .as_object()
        .ok_or_else(|| EngineError::bad_request("Request must be a JSON object"))?;

    let action = obj
        .get("action")
        .and_then(Value::as_str)
        .ok_or_else(|| EngineError::bad_request("Request is missing the string field `action`"))?;

    if let Some(entropy) = obj
        .get("payload")
        .and_then(Value::as_object)
        .and_then(|p| p.get("entropy"))
        .and_then(Value::as_str)
    {
        rng::seed_from_base64(entropy)?;
    }
    // Even without caller entropy, stir the pool with the request bytes so
    // repeated calls never produce identical "random" output.
    rng::stir(input);

    let parsed: A = serde_json::from_value(raw.clone()).map_err(|e| {
        EngineError::bad_request(format!("Invalid request for action `{action}`: {e}"))
    })?;

    handler(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(tag = "action", content = "payload")]
    enum TestAction {
        #[serde(rename = "echo")]
        Echo { text: String },
        #[serde(rename = "fail")]
        Fail {},
    }

    fn handler(a: TestAction) -> EngineResult<Value> {
        match a {
            TestAction::Echo { text } => Ok(serde_json::json!({ "text": text })),
            TestAction::Fail {} => Err(EngineError::invalid("nope")),
        }
    }

    fn run(input: &str) -> Value {
        serde_json::from_slice(&run_request::<TestAction, _>(input.as_bytes(), handler)).unwrap()
    }

    #[test]
    fn success_path() {
        let r = run(r#"{"action":"echo","payload":{"text":"hi"}}"#);
        assert_eq!(r["success"], true);
        assert_eq!(r["data"]["text"], "hi");
        assert!(r.get("error").is_none());
    }

    #[test]
    fn handler_error_has_code() {
        let r = run(r#"{"action":"fail","payload":{}}"#);
        assert_eq!(r["success"], false);
        assert_eq!(r["error_code"], "E_INVALID_INPUT");
        assert_eq!(r["error"], "nope");
    }

    #[test]
    fn empty_and_invalid_requests() {
        assert_eq!(run("")["error_code"], "E_BAD_REQUEST");
        assert_eq!(run("   ")["error_code"], "E_BAD_REQUEST");
        assert_eq!(run("{not json")["error_code"], "E_BAD_REQUEST");
        assert_eq!(run("[1,2]")["error_code"], "E_BAD_REQUEST");
        assert_eq!(run(r#"{"payload":{}}"#)["error_code"], "E_BAD_REQUEST");
        let r = run(r#"{"action":"unknown","payload":{}}"#);
        assert_eq!(r["error_code"], "E_BAD_REQUEST");
        assert!(r["error"].as_str().unwrap().contains("unknown"));
    }

    #[test]
    fn entropy_is_consumed() {
        let r = run(
            r#"{"action":"echo","payload":{"text":"x","entropy":"AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8="}}"#,
        );
        assert_eq!(r["success"], true);
        assert!(rng::has_caller_entropy());
        let bad = run(r#"{"action":"echo","payload":{"text":"x","entropy":"@@@"}}"#);
        assert_eq!(bad["error_code"], "E_INVALID_INPUT");
    }
}
