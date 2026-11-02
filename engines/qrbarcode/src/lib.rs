//! # Dev Toolbox QR & Barcode Engine (`qrbarcode-engine`)
//!
//! | Action | Tool(s) |
//! |---|---|
//! | `qr_generate` | qr-code-generator, qr-code-to-svg, qr-code-to-png, qr-code-styler |
//! | `qr_wifi` / `qr_vcard` / `qr_url` / `qr_email` / `qr_sms` / `qr_geo` / `qr_event` | wifi-qr-generator, vcard-qr-generator, url-qr-generator |
//! | `qr_bulk` | bulk-qr-generator |
//! | `qr_decode` | qr-code-reader |
//! | `barcode_generate` | barcode-generator (EAN-13/8, UPC-A/E, Code 128, GS1-128, Code 39, ITF-14, I2of5, Codabar) |
//! | `barcode_validate` | barcode-generator / barcode-reader (GTIN, ISBN, ISSN check digits) |
//! | `barcode_read` | barcode-reader (EAN-13 / UPC-A / EAN-8 scanline decoder) |
//! | `payload_inspect` | qr-code-reader (classify decoded text) |
//!
//! Images travel as base64. QR decoding uses `rqrr`; it handles rotation and
//! perspective but not heavy blur. 1-D reading is limited to EAN/UPC.

pub mod barcode;
pub mod qr;

use engine_shared::EngineResult;
use serde::Deserialize;
use serde_json::{json, Value};

const MAX_BULK: usize = 500;

/// Every action understood by the QR/barcode engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum QrAction {
    /// QR code from arbitrary text/bytes.
    QrGenerate {
        /// Payload text.
        data: String,
        /// Colours, size, error correction, logo area, rounded modules.
        #[serde(default)]
        style: qr::Style,
        /// Output formats (`svg`, `png`, `ascii`).
        #[serde(default)]
        formats: Vec<String>,
        /// Include the raw module matrix.
        #[serde(default)]
        include_matrix: bool,
    },
    /// Wi-Fi join QR (`WIFI:` payload).
    QrWifi {
        /// Network name.
        ssid: String,
        /// Password.
        #[serde(default)]
        password: String,
        /// `WPA` (default), `WEP`, `nopass`, `WPA2-EAP`.
        #[serde(default)]
        security: String,
        /// Hidden network.
        #[serde(default)]
        hidden: bool,
        /// EAP method (enterprise networks).
        #[serde(default)]
        eap: Option<String>,
        /// EAP identity.
        #[serde(default)]
        identity: Option<String>,
        /// Rendering style.
        #[serde(default)]
        style: qr::Style,
        /// Output formats.
        #[serde(default)]
        formats: Vec<String>,
    },
    /// vCard 3.0 contact QR.
    QrVcard {
        /// Contact fields.
        #[serde(flatten)]
        card: qr::VCard,
        /// Rendering style.
        #[serde(default)]
        style: qr::Style,
        /// Output formats.
        #[serde(default)]
        formats: Vec<String>,
    },
    /// URL QR (validated and normalised).
    QrUrl {
        /// URL.
        url: String,
        /// Rendering style.
        #[serde(default)]
        style: qr::Style,
        /// Output formats.
        #[serde(default)]
        formats: Vec<String>,
    },
    /// `mailto:` QR.
    QrEmail {
        /// Recipient address.
        to: String,
        /// Subject line.
        #[serde(default)]
        subject: String,
        /// Message body.
        #[serde(default)]
        body: String,
        /// Rendering style.
        #[serde(default)]
        style: qr::Style,
        /// Output formats.
        #[serde(default)]
        formats: Vec<String>,
    },
    /// SMS QR.
    QrSms {
        /// Phone number.
        number: String,
        /// Pre-filled message.
        #[serde(default)]
        message: String,
        /// Rendering style.
        #[serde(default)]
        style: qr::Style,
        /// Output formats.
        #[serde(default)]
        formats: Vec<String>,
    },
    /// Geo-location QR (`geo:` URI).
    QrGeo {
        /// Latitude.
        latitude: f64,
        /// Longitude.
        longitude: f64,
        /// Optional place query.
        #[serde(default)]
        query: String,
        /// Rendering style.
        #[serde(default)]
        style: qr::Style,
        /// Output formats.
        #[serde(default)]
        formats: Vec<String>,
    },
    /// Calendar event QR (iCalendar `VEVENT`).
    QrEvent {
        /// Event title.
        title: String,
        /// Start date-time (ISO 8601).
        start: String,
        /// End date-time (ISO 8601).
        #[serde(default)]
        end: String,
        /// Location.
        #[serde(default)]
        location: String,
        /// Description.
        #[serde(default)]
        description: String,
        /// Rendering style.
        #[serde(default)]
        style: qr::Style,
        /// Output formats.
        #[serde(default)]
        formats: Vec<String>,
    },
    /// Generates many QR codes in one call.
    QrBulk {
        /// Items: strings or `{ "data", "name" }` objects.
        items: Vec<Value>,
        /// Rendering style.
        #[serde(default)]
        style: qr::Style,
        /// Output formats.
        #[serde(default)]
        formats: Vec<String>,
    },
    /// Decodes a QR code from an image.
    QrDecode {
        /// Image bytes (base64 PNG/JPEG/…).
        #[serde(with = "engine_shared::b64")]
        image: Vec<u8>,
    },
    /// Classifies a decoded payload (URL, Wi-Fi, vCard, …) and extracts its fields.
    PayloadInspect {
        /// Decoded text.
        text: String,
    },
    /// 1-D barcode generation.
    BarcodeGenerate {
        /// Symbology name (`ean13`, `upca`, `code128`, `code39`, `itf14`, `codabar`, …).
        symbology: String,
        /// Data to encode.
        data: String,
        /// Rendering style.
        #[serde(default)]
        style: barcode::BarStyle,
        /// Symbology-specific options (e.g. `{"check_digit": true}` for Code 39).
        #[serde(default)]
        options: Value,
        /// Output formats.
        #[serde(default)]
        formats: Vec<String>,
    },
    /// Validates EAN/UPC/GTIN/ISBN/ISSN check digits.
    BarcodeValidate {
        /// Code to validate.
        code: String,
    },
    /// Reads an EAN-13/EAN-8/UPC-A barcode from an image (scan-line decoder).
    BarcodeRead {
        /// Image bytes (base64).
        #[serde(with = "engine_shared::b64")]
        image: Vec<u8>,
    },
}

fn with_payload(payload: String, style: &qr::Style, formats: &[String]) -> EngineResult<Value> {
    let mut v = qr::generate(&payload, style, formats, false)?;
    v["payload"] = json!(payload);
    Ok(v)
}

/// Dispatches a [`QrAction`] to its implementation.
pub fn handle_action(action: QrAction) -> EngineResult<Value> {
    match action {
        QrAction::QrGenerate { data, style, formats, include_matrix } => {
            qr::generate(&data, &style, &formats, include_matrix)
        },
        QrAction::QrWifi {
            ssid,
            password,
            security,
            hidden,
            eap,
            identity,
            style,
            formats,
        } => with_payload(
            qr::wifi_payload(
                &ssid,
                &password,
                &security,
                hidden,
                eap.as_deref(),
                identity.as_deref(),
            )?,
            &style,
            &formats,
        ),
        QrAction::QrVcard { card, style, formats } => {
            with_payload(qr::vcard_payload(&card)?, &style, &formats)
        },
        QrAction::QrUrl { url, style, formats } => {
            with_payload(qr::url_payload(&url)?, &style, &formats)
        },
        QrAction::QrEmail { to, subject, body, style, formats } => {
            with_payload(qr::email_payload(&to, &subject, &body)?, &style, &formats)
        },
        QrAction::QrSms { number, message, style, formats } => {
            with_payload(qr::sms_payload(&number, &message)?, &style, &formats)
        },
        QrAction::QrGeo {
            latitude,
            longitude,
            query,
            style,
            formats,
        } => with_payload(qr::geo_payload(latitude, longitude, &query)?, &style, &formats),
        QrAction::QrEvent {
            title,
            start,
            end,
            location,
            description,
            style,
            formats,
        } => with_payload(
            qr::event_payload(&title, &start, &end, &location, &description)?,
            &style,
            &formats,
        ),
        QrAction::QrBulk { items, style, formats } => {
            engine_shared::limits::check_count(items.len(), MAX_BULK, "items")?;
            let formats = if formats.is_empty() {
                vec!["svg".to_string()]
            } else {
                formats
            };
            let mut results = Vec::new();
            let mut failures = 0;
            for (i, item) in items.iter().enumerate() {
                let (data, label) = match item {
                    Value::String(s) => (s.clone(), s.clone()),
                    Value::Object(o) => (
                        o.get("data").and_then(Value::as_str).unwrap_or("").to_string(),
                        o.get("label")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                            .unwrap_or_else(|| {
                                o.get("data").and_then(Value::as_str).unwrap_or("").to_string()
                            }),
                    ),
                    other => (other.to_string(), other.to_string()),
                };
                match qr::generate(&data, &style, &formats, false) {
                    Ok(mut v) => {
                        v["index"] = json!(i);
                        v["label"] = json!(label);
                        v["filename"] = json!(format!(
                            "qr_{:03}_{}.{}",
                            i + 1,
                            label
                                .chars()
                                .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                                .take(32)
                                .collect::<String>(),
                            if formats.iter().any(|f| f.eq_ignore_ascii_case("png")) {
                                "png"
                            } else {
                                "svg"
                            }
                        ));
                        results.push(v);
                    },
                    Err(e) => {
                        failures += 1;
                        results.push(json!({
                            "index": i,
                            "label": label,
                            "error": e.message,
                            "error_code": e.code.as_str()
                        }));
                    },
                }
            }
            Ok(json!({ "count": results.len(), "failures": failures, "items": results }))
        },
        QrAction::QrDecode { image } => qr::decode(&image),
        QrAction::PayloadInspect { text } => {
            engine_shared::error::require_text(&text, "text")?;
            let mut v = qr::classify_payload(&text);
            v["length"] = json!(text.len());
            v["fits_qr"] = json!(text.len() <= qr::MAX_PAYLOAD_BYTES);
            Ok(v)
        },
        QrAction::BarcodeGenerate { symbology, data, style, options, formats } => {
            barcode::generate(&symbology, &data, &style, &options, &formats)
        },
        QrAction::BarcodeValidate { code } => barcode::validate(&code),
        QrAction::BarcodeRead { image } => barcode::read(&image),
    }
}

engine_shared::export_engine!(handle_action, QrAction);
