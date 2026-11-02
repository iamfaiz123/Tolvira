//! # Dev Toolbox Encoding Engine (`encoding-engine`)
//!
//! | Action | Tool(s) |
//! |---|---|
//! | `base64_encode` / `base64_decode` | base64-encoder, base64-decoder, base64-encode-decode (text or file, standard/url-safe, MIME wrapping) |
//! | `base32_encode` / `base32_decode` / `hex_encode` / `hex_decode` | hex-encoder, hex-decoder, base32 |
//! | `url_encode` / `url_decode` | url-encoder, url-decoder, url-encode-decode (component / query / path modes) |
//! | `html_encode` / `html_decode` | html-encoder, html-decoder, html-entity-encode-decode |
//! | `punycode_encode` / `punycode_decode` | punycode-converter (IDNA domains) |
//! | `quoted_printable_encode` / `quoted_printable_decode` | quoted-printable-converter |
//! | `mime_word_encode` / `mime_word_decode` | mime-word-decoder (RFC 2047) |
//! | `msgpack_encode` / `msgpack_decode` / `cbor_encode` / `cbor_decode` | messagepack-converter, cbor-viewer |
//! | `protobuf_inspect` | protobuf-inspector (schema-less wire format decoding) |
//! | `detect` | guess the encoding of a string |

pub mod codecs;
pub mod serial;

use engine_shared::{b64, EngineResult};
use serde::Deserialize;
use serde_json::Value;

fn default_true() -> bool {
    true
}

/// Every action understood by the encoding engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum EncodingAction {
    /// Text/file → base64.
    Base64Encode {
        /// Text input.
        #[serde(default)]
        text: Option<String>,
        /// File input (base64) – used instead of `text` when present.
        #[serde(default, with = "b64::option")]
        file: Option<Vec<u8>>,
        /// Use the URL-safe alphabet (`-_`).
        #[serde(default)]
        url_safe: bool,
        /// Append `=` padding.
        #[serde(default = "default_true")]
        padding: bool,
        /// Wrap output lines at this length (0 = no wrapping).
        #[serde(default)]
        line_length: usize,
        /// Emit a `data:<mime>;base64,` URI with this MIME type.
        #[serde(default)]
        mime: Option<String>,
    },
    /// base64 → text/bytes.
    Base64Decode {
        /// Base64 input (data-URI prefix allowed).
        text: String,
        /// Force text (`true`) or bytes (`false`) output; auto-detected when omitted.
        #[serde(default)]
        as_text: Option<bool>,
    },
    /// Text/file → base32.
    Base32Encode {
        /// Text input.
        #[serde(default)]
        text: Option<String>,
        /// File input (base64).
        #[serde(default, with = "b64::option")]
        file: Option<Vec<u8>>,
        /// Append `=` padding.
        #[serde(default = "default_true")]
        padding: bool,
    },
    /// base32 → text/bytes.
    Base32Decode {
        /// Base32 input.
        text: String,
    },
    /// Text/file → hex.
    HexEncode {
        /// Text input.
        #[serde(default)]
        text: Option<String>,
        /// File input (base64).
        #[serde(default, with = "b64::option")]
        file: Option<Vec<u8>>,
        /// Upper-case digits.
        #[serde(default)]
        uppercase: bool,
        /// Separator between bytes.
        #[serde(default)]
        separator: String,
        /// Prefix before every byte (e.g. `0x`).
        #[serde(default)]
        prefix: String,
    },
    /// hex → text/bytes.
    HexDecode {
        /// Hex input.
        text: String,
    },
    /// Percent-encoding.
    UrlEncode {
        /// Text input.
        text: String,
        /// `component` (default), `uri`, `query`, `path` or `all`.
        #[serde(default)]
        mode: String,
        /// Encode spaces as `+` (form style).
        #[serde(default)]
        space_as_plus: bool,
    },
    /// Percent-decoding.
    UrlDecode {
        /// Encoded input.
        text: String,
        /// Treat `+` as a space.
        #[serde(default = "default_true")]
        plus_as_space: bool,
    },
    /// HTML entity encoding.
    HtmlEncode {
        /// Text input.
        text: String,
        /// `minimal` (default), `non-ascii` or `all`.
        #[serde(default)]
        mode: String,
        /// Prefer named entities over numeric ones.
        #[serde(default)]
        named: bool,
    },
    /// HTML entity decoding.
    HtmlDecode {
        /// Encoded input.
        text: String,
    },
    /// Domain → Punycode/IDNA.
    PunycodeEncode {
        /// Unicode domain.
        text: String,
    },
    /// Punycode/IDNA → domain.
    PunycodeDecode {
        /// ASCII domain.
        text: String,
    },
    /// Quoted-printable encoding.
    QuotedPrintableEncode {
        /// Text input.
        text: String,
    },
    /// Quoted-printable decoding.
    QuotedPrintableDecode {
        /// Encoded input.
        text: String,
    },
    /// RFC 2047 encoded-word encoding for mail headers.
    MimeWordEncode {
        /// Header text.
        text: String,
        /// `B` (base64, default) or `Q` (quoted-printable).
        #[serde(default)]
        encoding: String,
        /// Charset label (UTF-8 / US-ASCII).
        #[serde(default)]
        charset: String,
    },
    /// RFC 2047 encoded-word decoding.
    MimeWordDecode {
        /// Header text.
        text: String,
    },
    /// JSON → MessagePack.
    MsgpackEncode {
        /// JSON source.
        json: String,
    },
    /// MessagePack → JSON.
    MsgpackDecode {
        /// MessagePack bytes (base64).
        #[serde(with = "b64")]
        data: Vec<u8>,
    },
    /// JSON → CBOR.
    CborEncode {
        /// JSON source.
        json: String,
    },
    /// CBOR → JSON.
    CborDecode {
        /// CBOR bytes (base64).
        #[serde(with = "b64")]
        data: Vec<u8>,
    },
    /// Decodes protobuf wire format without a schema (field numbers, wire types, guessed values).
    ProtobufInspect {
        /// Protobuf bytes (base64).
        #[serde(with = "b64")]
        data: Vec<u8>,
    },
    /// Guesses which encoding a string uses (base64, hex, URL, JWT, …).
    Detect {
        /// Input text.
        text: String,
    },
}

/// Dispatches an [`EncodingAction`] to its implementation.
pub fn handle_action(action: EncodingAction) -> EngineResult<Value> {
    use codecs::input_bytes;
    match action {
        EncodingAction::Base64Encode {
            text,
            file,
            url_safe,
            padding,
            line_length,
            mime,
        } => codecs::base64_encode(
            &input_bytes(text.as_deref(), file.as_deref())?,
            url_safe,
            padding,
            line_length,
            mime.as_deref(),
        ),
        EncodingAction::Base64Decode { text, as_text } => codecs::base64_decode(&text, as_text),
        EncodingAction::Base32Encode { text, file, padding } => {
            codecs::base32_encode(&input_bytes(text.as_deref(), file.as_deref())?, padding)
        },
        EncodingAction::Base32Decode { text } => codecs::base32_decode(&text),
        EncodingAction::HexEncode { text, file, uppercase, separator, prefix } => {
            codecs::hex_encode(
                &input_bytes(text.as_deref(), file.as_deref())?,
                uppercase,
                &separator,
                &prefix,
            )
        },
        EncodingAction::HexDecode { text } => codecs::hex_decode(&text),
        EncodingAction::UrlEncode { text, mode, space_as_plus } => {
            codecs::url_encode(&text, &mode, space_as_plus)
        },
        EncodingAction::UrlDecode { text, plus_as_space } => {
            codecs::url_decode(&text, plus_as_space)
        },
        EncodingAction::HtmlEncode { text, mode, named } => {
            codecs::html_encode(&text, &mode, named)
        },
        EncodingAction::HtmlDecode { text } => codecs::html_decode(&text),
        EncodingAction::PunycodeEncode { text } => codecs::punycode_encode(&text),
        EncodingAction::PunycodeDecode { text } => codecs::punycode_decode(&text),
        EncodingAction::QuotedPrintableEncode { text } => codecs::qp_encode(&text),
        EncodingAction::QuotedPrintableDecode { text } => codecs::qp_decode(&text),
        EncodingAction::MimeWordEncode { text, encoding, charset } => {
            codecs::mime_word_encode(&text, &encoding, &charset)
        },
        EncodingAction::MimeWordDecode { text } => codecs::mime_word_decode(&text),
        EncodingAction::MsgpackEncode { json } => serial::msgpack_encode(&json),
        EncodingAction::MsgpackDecode { data } => serial::msgpack_decode(&data),
        EncodingAction::CborEncode { json } => serial::cbor_encode(&json),
        EncodingAction::CborDecode { data } => serial::cbor_decode(&data),
        EncodingAction::ProtobufInspect { data } => serial::protobuf_inspect(&data),
        EncodingAction::Detect { text } => codecs::detect(&text),
    }
}

engine_shared::export_engine!(handle_action, EncodingAction);
