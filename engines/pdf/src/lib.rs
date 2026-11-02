//! # Dev Toolbox PDF Engine (`pdf-engine`)
//!
//! One WebAssembly binary serving every PDF utility of the catalog. Requests
//! use the shared `{ "action": ..., "payload": ... }` envelope; files travel
//! as base64 strings (`file`, `files`, `image`, ...). Every action that
//! opens a PDF accepts an optional `password`.
//!
//! | Action | Tool(s) | Output |
//! |---|---|---|
//! | `merge` | pdf-merge | `output.file` |
//! | `split` | pdf-split | `files[]` |
//! | `compress` | pdf-compress | `output.file`, size report |
//! | `rotate` / `crop` / `resize_pages` | pdf-rotate, pdf-crop, change-pdf-page-size | `output.file` |
//! | `reorder` / `delete_pages` / `extract_pages` / `duplicate_pages` | page tools | `output.file` |
//! | `add_page_numbers` / `add_watermark` / `add_header_footer` / `stamp` | decoration | `output.file` |
//! | `protect` / `unlock` / `permissions` | security | `output.file` / report |
//! | `metadata` / `set_metadata` / `page_sizes` | inspection | report / `output.file` |
//! | `repair` / `linearize` | maintenance | `output.file` |
//! | `extract_text` / `to_html` / `to_markdown` / `to_docx` / `extract_images` | conversion out | text / files |
//! | `from_images` / `from_text` / `from_markdown` / `from_html` | conversion in | `output.file` |
//! | `compare` / `overlay` / `annotate` / `redact` / `grayscale` | editing | report / `output.file` |
//! | `form_fields` / `fill_form` / `outlines` / `set_bookmarks` / `signatures` | forms & structure | report / `output.file` |
//! | `inspect` / `estimate_pages` | legacy (v1 compatible) | report |
//!
//! Errors use the stable codes from `engine_shared::error` (`E_ENCRYPTED`,
//! `E_WRONG_PASSWORD`, `E_MALFORMED`, `E_UNSUPPORTED_FORMAT`, ...).

pub mod create;
pub mod doc;
pub mod extract;
pub mod font_metrics;
pub mod fonts;
pub mod images;
pub mod meta;
pub mod optimize;
pub mod overlay;
pub mod pages;
pub mod security;
#[cfg(any(test, feature = "testutil"))]
pub mod testutil;

use doc::PageSelection;
use engine_shared::{b64, EngineError, EngineResult};
use serde::Deserialize;
use serde_json::{json, Value};

fn default_copies() -> usize {
    1
}
fn default_one() -> u32 {
    1
}
fn default_fit() -> String {
    "contain".into()
}
fn default_scale() -> String {
    "scale".into()
}
fn default_black() -> String {
    "#000000".into()
}

/// All PDF engine actions. `payload` fields are documented per variant.
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum PdfAction {
    // ----- legacy (v1) ----------------------------------------------------
    /// Legacy: `{ bytes_len, header }` *or* new style `{ file }`.
    Inspect {
        /// Base64 PDF (new style).
        #[serde(default)]
        file: Option<String>,
        /// Legacy: byte length reported by the client.
        #[serde(default)]
        bytes_len: Option<usize>,
        /// Legacy: first bytes of the file as text.
        #[serde(default)]
        header: Option<String>,
    },
    /// Legacy: estimates pages from a hint (kept for the v1 UI).
    EstimatePages {
        /// Page count hint.
        page_count_hint: usize,
    },

    // ----- page operations -----------------------------------------------
    /// Concatenates several PDFs in order.
    Merge {
        /// Input PDFs (base64).
        #[serde(with = "b64::list")]
        files: Vec<Vec<u8>>,
        /// Optional password per input (same order).
        #[serde(default)]
        passwords: Vec<String>,
    },
    /// Splits into several PDFs (each page, ranges or every `n` pages).
    Split {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Split strategy: `each` page, explicit `ranges`, or `every_n` pages.
        #[serde(default)]
        mode: pages::SplitMode,
        /// Ranges used in `ranges` mode (e.g. `1-3`, `4-6`).
        #[serde(default)]
        ranges: Vec<String>,
        /// Chunk size used in `every_n` mode (e.g. `2`).
        #[serde(default)]
        n: usize,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
    },
    /// Rotates pages by a multiple of 90°.
    Rotate {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
        /// Rotation in degrees (multiple of 90, may be negative).
        angle: i64,
        /// Pages to rotate.
        #[serde(default)]
        pages: PageSelection,
    },
    /// Sets the crop/media box.
    Crop {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
        /// Crop box or margins to trim.
        #[serde(flatten)]
        spec: pages::CropSpec,
        /// Pages to crop.
        #[serde(default)]
        pages: PageSelection,
    },
    /// Re-orders pages.
    Reorder {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
        /// New 1-based page order.
        order: Vec<u32>,
        /// Drop pages not listed in `order` (otherwise they are appended).
        #[serde(default)]
        drop_missing: bool,
    },
    /// Deletes pages.
    DeletePages {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
        /// Pages to delete.
        pages: PageSelection,
    },
    /// Extracts pages into a new PDF.
    ExtractPages {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
        /// Pages to keep.
        pages: PageSelection,
    },
    /// Duplicates pages in place.
    DuplicatePages {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
        /// Pages to duplicate.
        pages: PageSelection,
        /// Number of extra copies.
        #[serde(default = "default_copies")]
        copies: usize,
    },
    /// Scales or pads pages to a new size.
    ResizePages {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
        /// Target size (`A4`, `Letter`, `210mm x 297mm`, …).
        page_size: String,
        /// `scale` (proportional fit, default), `center` (keep size, pad) or `stretch`.
        #[serde(default = "default_scale")]
        mode: String,
        /// Pages to resize.
        #[serde(default)]
        pages: PageSelection,
    },

    // ----- decoration -----------------------------------------------------
    /// Stamps page numbers.
    AddPageNumbers {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
        /// Label format, e.g. `"Page {n} of {total}"`.
        #[serde(default)]
        format: String,
        /// Number assigned to the first page.
        #[serde(default = "default_one")]
        start: u32,
        /// Font, size, colour, margin and position.
        #[serde(flatten)]
        options: overlay::LabelOptions,
        /// Pages to label.
        #[serde(default)]
        pages: PageSelection,
    },
    /// Adds header and/or footer text.
    AddHeaderFooter {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
        /// Header text (supports `{n}`, `{total}`, `{date}`).
        #[serde(default)]
        header: String,
        /// Footer text (supports `{n}`, `{total}`, `{date}`).
        #[serde(default)]
        footer: String,
        /// `left`, `center` or `right`.
        #[serde(default)]
        align: String,
        /// Font, size, colour and margin.
        #[serde(flatten)]
        options: overlay::LabelOptions,
        /// Pages to label.
        #[serde(default)]
        pages: PageSelection,
    },
    /// Draws a diagonal/centred text watermark.
    AddWatermark {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
        /// Watermark text.
        text: String,
        /// Font, opacity, rotation and placement.
        #[serde(flatten)]
        options: overlay::WatermarkOptions,
        /// Pages to watermark.
        #[serde(default)]
        pages: PageSelection,
    },
    /// Places an image (PNG/JPEG) on pages.
    Stamp {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
        /// Stamp image (base64 PNG/JPEG).
        #[serde(with = "b64")]
        image: Vec<u8>,
        /// Placement, size and opacity.
        #[serde(flatten)]
        options: overlay::StampOptions,
        /// Pages to stamp.
        #[serde(default)]
        pages: PageSelection,
    },
    /// Overlays a page of another PDF (letterhead, background…).
    Overlay {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
        /// PDF providing the overlay page.
        #[serde(with = "b64")]
        overlay_file: Vec<u8>,
        /// Password of the overlay PDF.
        #[serde(default)]
        overlay_password: Option<String>,
        /// 1-based page of `overlay_file` to use.
        #[serde(default = "default_one")]
        overlay_page: u32,
        /// Draw the overlay under the existing content.
        #[serde(default)]
        behind: bool,
        /// Overlay opacity (0–1).
        #[serde(default = "default_opacity")]
        opacity: f64,
        /// Pages to overlay.
        #[serde(default)]
        pages: PageSelection,
    },
    /// Adds annotations (square, highlight, note, free text, link).
    Annotate {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
        /// Annotations to add.
        annotations: Vec<overlay::Annotation>,
    },
    /// Redacts text matches and/or rectangles (content is removed, not just covered).
    Redact {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
        /// Search terms whose occurrences are redacted.
        #[serde(default)]
        terms: Vec<String>,
        /// Explicit rectangles to redact.
        #[serde(default)]
        areas: Vec<overlay::RedactArea>,
        /// Case-sensitive term matching.
        #[serde(default)]
        case_sensitive: bool,
        /// Fill colour for redaction boxes (`#RRGGBB`).
        #[serde(default = "default_black")]
        color: String,
    },
    /// Converts page content to grayscale.
    Grayscale {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
    },

    // ----- security -------------------------------------------------------
    /// Encrypts with user/owner passwords and permissions.
    Protect {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for (already) encrypted input.
        #[serde(default)]
        password: Option<String>,
        /// Password required to open the document.
        #[serde(default)]
        user_password: String,
        /// Password that unlocks all permissions.
        #[serde(default)]
        owner_password: String,
        /// `aes-256` (default), `aes-128` or `rc4-128`.
        #[serde(default)]
        algorithm: String,
        /// Permission flags granted to the user password.
        #[serde(default)]
        permissions: security::PermissionFlags,
    },
    /// Removes encryption.
    Unlock {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password that opens the document.
        #[serde(default)]
        password: String,
    },
    /// Reports encryption status and permission flags.
    Permissions {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
    },

    // ----- metadata & structure ------------------------------------------
    /// Reads Info dictionary and XMP metadata.
    Metadata {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
    },
    /// Updates Info dictionary entries.
    SetMetadata {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
        /// Fields to set (`null` keeps, empty string clears).
        #[serde(flatten)]
        edit: meta::MetadataEdit,
    },
    /// Lists page sizes and orientations.
    PageSizes {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
    },
    /// Reads the outline (bookmark) tree.
    Outlines {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
    },
    /// Replaces the outline tree.
    SetBookmarks {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
        /// New bookmark tree.
        bookmarks: Vec<meta::BookmarkSpec>,
    },
    /// Lists AcroForm fields with their current values.
    FormFields {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
    },
    /// Fills AcroForm fields.
    FillForm {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
        /// Field name → value map.
        #[serde(default)]
        values: serde_json::Map<String, Value>,
        /// Flatten the form after filling (fields become static content).
        #[serde(default)]
        flatten: bool,
    },
    /// Lists digital signature fields (presence, signer, reason, date; no cryptographic
    /// verification).
    Signatures {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
    },

    // ----- maintenance ----------------------------------------------------
    /// Re-saves with object streams, metadata stripping and image re-compression.
    Compress {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Compression level: `extreme` (high compression, lower quality), `moderate` (recommended, balanced quality & size), or `light` (high quality, mild compression).
        #[serde(default)]
        level: optimize::CompressionLevel,
        /// Remove Info/XMP metadata and page thumbnails.
        #[serde(default = "default_true")]
        strip_metadata: bool,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
        /// Custom JPEG quality (1-100) overriding the compression level.
        #[serde(default)]
        image_quality: Option<u8>,
    },
    /// Rebuilds the cross-reference table and drops broken objects.
    Repair {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
    },
    /// Re-saves with linearisation hints (fast web view) and cross-reference streams.
    Linearize {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
    },

    // ----- conversion out -------------------------------------------------
    /// Extracts text by page.
    ExtractText {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
        /// Pages to extract.
        #[serde(default)]
        pages: PageSelection,
    },
    /// Converts text content to structural HTML.
    ToHtml {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
        /// Pages to convert.
        #[serde(default)]
        pages: PageSelection,
        /// HTML document title.
        #[serde(default)]
        title: Option<String>,
    },
    /// Converts text content to Markdown.
    ToMarkdown {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
        /// Pages to convert.
        #[serde(default)]
        pages: PageSelection,
    },
    /// Converts text content to a DOCX document.
    ToDocx {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
        /// Pages to convert.
        #[serde(default)]
        pages: PageSelection,
    },
    /// Extracts embedded raster images.
    ExtractImages {
        /// Input PDF.
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Password for encrypted input.
        #[serde(default)]
        password: Option<String>,
        /// Pages to scan.
        #[serde(default)]
        pages: PageSelection,
        /// Skip images smaller than this many pixels on either side.
        #[serde(default = "default_min_size")]
        min_size: u32,
    },
    /// Compares the text of two PDFs.
    Compare {
        /// First PDF.
        #[serde(with = "b64")]
        file_a: Vec<u8>,
        /// Second PDF.
        #[serde(with = "b64")]
        file_b: Vec<u8>,
        /// Password for the first PDF.
        #[serde(default)]
        password_a: Option<String>,
        /// Password for the second PDF.
        #[serde(default)]
        password_b: Option<String>,
        /// Maximum number of reported changes.
        #[serde(default)]
        max_changes: usize,
    },
    /// Not supported in WASM (no rasteriser): returns `E_UNSUPPORTED` with guidance.
    ToImages {},
    /// Not supported in WASM (no vector renderer): returns `E_UNSUPPORTED`.
    ToSvg {},

    // ----- conversion in --------------------------------------------------
    /// Builds a PDF from images (one page per image).
    FromImages {
        /// Images (base64 PNG/JPEG/GIF/BMP/WebP).
        #[serde(with = "b64::list")]
        images: Vec<Vec<u8>>,
        /// Page size; defaults to each image's pixel size at 72 dpi.
        #[serde(default)]
        page_size: Option<String>,
        /// Margin in points.
        #[serde(default)]
        margin: f64,
        /// `fit` (default), `fill` or `stretch`.
        #[serde(default = "default_fit")]
        fit: String,
    },
    /// Lays out plain text.
    FromText {
        /// Text content.
        text: String,
        /// Page size, font, margins…
        #[serde(flatten)]
        options: create::LayoutOptions,
    },
    /// Renders Markdown with headings, lists, code and quotes.
    FromMarkdown {
        /// Markdown source.
        markdown: String,
        /// Page size, font, margins…
        #[serde(flatten)]
        options: create::LayoutOptions,
    },
    /// Renders simple HTML (structural subset).
    FromHtml {
        /// HTML source.
        html: String,
        /// Page size, font, margins…
        #[serde(flatten)]
        options: create::LayoutOptions,
    },
}

fn default_opacity() -> f64 {
    1.0
}
fn default_min_size() -> u32 {
    8
}

/// Dispatches an action. Pure function: no I/O, no global state besides the
/// RNG pool.
pub fn handle_action(action: PdfAction) -> EngineResult<Value> {
    fn pw(p: &Option<String>) -> Option<&str> {
        p.as_deref().filter(|s| !s.is_empty())
    }
    match action {
        PdfAction::Inspect { file, bytes_len, header } => {
            if let Some(f) = file {
                let bytes = b64::decode(&f)?;
                if bytes.is_empty() {
                    return Err(EngineError::empty("PDF file"));
                }
                Ok(meta::quick_inspect(&bytes))
            } else {
                // v1 behaviour
                let header = header.unwrap_or_default();
                let bytes_len = bytes_len.unwrap_or(0);
                let is_pdf = header.starts_with("%PDF-");
                let version = if is_pdf && header.len() >= 8 {
                    header[5..8].to_string()
                } else {
                    "unknown".to_string()
                };
                Ok(json!({
                    "valid": is_pdf,
                    "version": version,
                    "estimated_pages": 1.max(bytes_len / 45_000),
                    "encryption_detected": header.contains("/Encrypt"),
                }))
            }
        },
        PdfAction::EstimatePages { page_count_hint } => Ok(json!({ "pages": page_count_hint })),

        PdfAction::Merge { files, passwords } => pages::merge(&files, &passwords),
        PdfAction::Split { file, password, mode, ranges, n } => {
            pages::split(&file, pw(&password), mode, &ranges, n)
        },
        PdfAction::Rotate { file, password, angle, pages } => {
            pages::rotate(&file, pw(&password), angle, &pages)
        },
        PdfAction::Crop { file, password, spec, pages } => {
            pages::crop(&file, pw(&password), &spec, &pages)
        },
        PdfAction::Reorder { file, password, order, drop_missing } => {
            pages::reorder(&file, pw(&password), &order, drop_missing)
        },
        PdfAction::DeletePages { file, password, pages } => {
            pages::delete_pages(&file, pw(&password), &pages)
        },
        PdfAction::ExtractPages { file, password, pages } => {
            pages::extract_pages(&file, pw(&password), &pages)
        },
        PdfAction::DuplicatePages { file, password, pages, copies } => {
            pages::duplicate_pages(&file, pw(&password), &pages, copies)
        },
        PdfAction::ResizePages { file, password, page_size, mode, pages } => {
            pages::resize_pages(&file, pw(&password), &page_size, &mode, &pages)
        },

        PdfAction::AddPageNumbers {
            file,
            password,
            format,
            start,
            options,
            pages,
        } => {
            overlay::add_page_numbers(&file, pw(&password), &format, start as i64, &options, &pages)
        },
        PdfAction::AddHeaderFooter {
            file,
            password,
            header,
            footer,
            align,
            options,
            pages,
        } => overlay::add_header_footer(
            &file,
            pw(&password),
            &header,
            &footer,
            &align,
            &options,
            &pages,
        ),
        PdfAction::AddWatermark { file, password, text, options, pages } => {
            overlay::add_watermark(&file, pw(&password), &text, &options, &pages)
        },
        PdfAction::Stamp { file, password, image, options, pages } => {
            overlay::stamp_image(&file, pw(&password), &image, &options, &pages)
        },
        PdfAction::Overlay {
            file,
            password,
            overlay_file,
            overlay_password,
            overlay_page,
            behind,
            opacity,
            pages,
        } => overlay::overlay(
            &file,
            pw(&password),
            &overlay_file,
            pw(&overlay_password),
            overlay_page,
            behind,
            opacity,
            &pages,
        ),
        PdfAction::Annotate { file, password, annotations } => {
            overlay::add_annotations(&file, pw(&password), &annotations)
        },
        PdfAction::Redact {
            file,
            password,
            terms,
            areas,
            case_sensitive,
            color,
        } => overlay::redact(&file, pw(&password), &terms, &areas, case_sensitive, &color),
        PdfAction::Grayscale { file, password } => overlay::grayscale(&file, pw(&password)),

        PdfAction::Protect {
            file,
            password,
            user_password,
            owner_password,
            algorithm,
            permissions,
        } => security::protect(
            &file,
            pw(&password),
            &user_password,
            &owner_password,
            &algorithm,
            &permissions,
        ),
        PdfAction::Unlock { file, password } => security::unlock(&file, &password),
        PdfAction::Permissions { file, password } => security::permissions(&file, pw(&password)),

        PdfAction::Metadata { file, password } => meta::metadata(&file, pw(&password)),
        PdfAction::SetMetadata { file, password, edit } => {
            meta::set_metadata(&file, pw(&password), &edit)
        },
        PdfAction::PageSizes { file, password } => meta::page_sizes(&file, pw(&password)),
        PdfAction::Outlines { file, password } => meta::outlines(&file, pw(&password)),
        PdfAction::SetBookmarks { file, password, bookmarks } => {
            meta::set_bookmarks(&file, pw(&password), &bookmarks)
        },
        PdfAction::FormFields { file, password } => meta::form_fields(&file, pw(&password)),
        PdfAction::FillForm { file, password, values, flatten } => {
            meta::fill_form(&file, pw(&password), &values, flatten)
        },
        PdfAction::Signatures { file, password } => meta::signatures(&file, pw(&password)),

        PdfAction::Compress {
            file,
            level,
            password,
            strip_metadata,
            image_quality,
        } => optimize::compress(&file, pw(&password), level, strip_metadata, image_quality),
        PdfAction::Repair { file, password } => optimize::repair(&file, pw(&password)),
        PdfAction::Linearize { file, password } => optimize::linearize(&file, pw(&password)),

        PdfAction::ExtractText { file, password, pages } => {
            extract::extract_text(&file, pw(&password), &pages)
        },
        PdfAction::ToHtml { file, password, pages, title } => {
            extract::to_html(&file, pw(&password), &pages, title.as_deref())
        },
        PdfAction::ToMarkdown { file, password, pages } => {
            extract::to_markdown(&file, pw(&password), &pages)
        },
        PdfAction::ToDocx { file, password, pages } => {
            extract::to_docx(&file, pw(&password), &pages)
        },
        PdfAction::ExtractImages { file, password, pages, min_size } => {
            images::extract_images(&file, pw(&password), &pages, min_size)
        },
        PdfAction::Compare {
            file_a,
            file_b,
            password_a,
            password_b,
            max_changes,
        } => extract::compare(&file_a, &file_b, pw(&password_a), pw(&password_b), max_changes),
        PdfAction::ToImages {} | PdfAction::ToSvg {} => Err(EngineError::unsupported(
            "Rasterising PDF pages is not available in the WASM engine (no renderer). Render \
             pages with the browser's PDF.js canvas output, or use `extract_images` to pull \
             embedded images.",
        )),

        PdfAction::FromImages { images, page_size, margin, fit } => {
            create::from_images(&images, page_size.as_deref(), margin, &fit)
        },
        PdfAction::FromText { text, options } => {
            engine_shared::error::require_text(&text, "text")?;
            engine_shared::limits::check_size(
                text.len(),
                engine_shared::limits::MAX_TEXT_CHARS,
                "text",
            )?;
            create::render_blocks(&create::text_to_blocks(&text), &options)
        },
        PdfAction::FromMarkdown { markdown, options } => {
            engine_shared::error::require_text(&markdown, "markdown")?;
            engine_shared::limits::check_size(
                markdown.len(),
                engine_shared::limits::MAX_TEXT_CHARS,
                "markdown",
            )?;
            create::render_blocks(&create::markdown_to_blocks(&markdown), &options)
        },
        PdfAction::FromHtml { html, options } => {
            engine_shared::error::require_text(&html, "html")?;
            engine_shared::limits::check_size(
                html.len(),
                engine_shared::limits::MAX_TEXT_CHARS,
                "html",
            )?;
            let blocks = create::html_to_blocks(&html);
            if blocks.is_empty() {
                return Err(EngineError::invalid("The HTML contains no renderable text"));
            }
            create::render_blocks(&blocks, &options)
        },
    }
}

fn default_true() -> bool {
    true
}

engine_shared::export_engine!(handle_action, PdfAction);
