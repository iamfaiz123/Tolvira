//! Document loading / saving helpers, error mapping, page selection parsing
//! and page geometry utilities shared by every PDF action.

use engine_shared::{limits, EngineError, EngineResult};
use lopdf::{Dictionary, Document, Error as PdfError, LoadOptions, Object, ObjectId, SaveOptions};
use serde::{Deserialize, Serialize};

/// Maximum bytes a single compressed stream may inflate to while loading.
/// Protects against decompression bombs inside untrusted PDFs.
pub const MAX_DECOMPRESSED_STREAM: usize = 512 * 1024 * 1024;

/// Points per inch / millimetre.
pub const PT_PER_INCH: f64 = 72.0;
/// PDF points per millimetre.
pub const PT_PER_MM: f64 = 72.0 / 25.4;

/// Loads a PDF from bytes with lenient parsing, decrypting it when a password
/// is supplied (or when the file is encrypted with an empty user password).
///
/// Error mapping:
/// * empty bytes → `E_EMPTY_INPUT`
/// * no `%PDF` header → `E_UNSUPPORTED_FORMAT`
/// * parse failure → `E_MALFORMED`
/// * encrypted without / with a wrong password → `E_ENCRYPTED` / `E_WRONG_PASSWORD`
pub fn load(bytes: &[u8], password: Option<&str>) -> EngineResult<Document> {
    if bytes.is_empty() {
        return Err(EngineError::empty("PDF file"));
    }
    limits::check_size(bytes.len(), limits::MAX_FILE_BYTES, "PDF file")?;
    if !looks_like_pdf(bytes) {
        return Err(EngineError::unsupported_format(
            "The file does not start with a %PDF header. Upload a PDF document.",
        ));
    }
    // lopdf decrypts while reading when the password is supplied through
    // LoadOptions (an empty user password is always tried first).
    let options = LoadOptions {
        password: password.map(str::to_string),
        max_decompressed_size: Some(MAX_DECOMPRESSED_STREAM),
        ..Default::default()
    };
    let doc = match Document::load_mem_with_options(bytes, options) {
        Ok(d) => d,
        Err(PdfError::InvalidPassword) => {
            return Err(EngineError::wrong_password("The supplied PDF password is incorrect."));
        },
        Err(PdfError::Decryption(e)) => {
            return Err(EngineError::encrypted(format!("The PDF could not be decrypted: {e}")));
        },
        Err(e) => return Err(map_pdf_error(e)),
    };

    if doc.is_encrypted() {
        // Still carries /Encrypt: no (or an empty) password was given and the
        // document does not open without one.
        return Err(match password {
            Some(p) if !p.is_empty() => {
                EngineError::wrong_password("The supplied PDF password is incorrect.")
            },
            _ => EngineError::encrypted(
                "The PDF is password protected. Provide `password` to open it.",
            ),
        });
    }

    if doc.get_pages().len() > limits::MAX_PDF_PAGES {
        return Err(EngineError::limit(format!(
            "PDF has more than {} pages",
            limits::MAX_PDF_PAGES
        )));
    }
    Ok(doc)
}

/// `%PDF` may be preceded by up to 1024 bytes of junk according to the spec.
pub fn looks_like_pdf(bytes: &[u8]) -> bool {
    let window = &bytes[..bytes.len().min(1024 + 4)];
    window.windows(4).any(|w| w == b"%PDF")
}

/// Serialises the document. Objects are renumbered and unreferenced objects
/// pruned so the output is compact and self-consistent.
pub fn save(doc: &mut Document) -> EngineResult<Vec<u8>> {
    save_with(doc, SaveOptions::default())
}

/// Serialises the document (optionally compressing object streams) and returns the bytes.
pub fn save_with(doc: &mut Document, options: SaveOptions) -> EngineResult<Vec<u8>> {
    // Classic cross-reference tables unless xref streams were explicitly
    // requested: they are universally supported and – unlike xref streams –
    // are read back correctly by lopdf for encrypted documents.
    if !options.use_xref_streams {
        doc.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
    }
    let mut out = Vec::new();
    doc.save_with_options(&mut out, options)
        .map_err(|e| EngineError::internal(format!("Failed to write PDF: {e}")))?;
    Ok(out)
}

/// Maps lopdf errors onto the shared error codes.
pub fn map_pdf_error(e: PdfError) -> EngineError {
    match e {
        PdfError::InvalidPassword => EngineError::wrong_password("Invalid PDF password"),
        PdfError::Decryption(d) => EngineError::encrypted(format!("PDF decryption failed: {d}")),
        PdfError::NotEncrypted => EngineError::invalid("The PDF is not encrypted"),
        PdfError::AlreadyEncrypted => EngineError::invalid("The PDF is already encrypted"),
        PdfError::PageNumberNotFound(n) => {
            EngineError::not_found(format!("Page {n} does not exist"))
        },
        PdfError::Decompress(d) => EngineError::malformed(format!(
            "A stream inside the PDF could not be decompressed: {d}"
        )),
        PdfError::Parse(p) => EngineError::malformed(format!("PDF parse error: {p}")),
        PdfError::IO(io) => EngineError::malformed(format!("PDF read error: {io}")),
        PdfError::Unimplemented(what) => {
            EngineError::unsupported(format!("PDF feature not supported: {what}"))
        },
        other => EngineError::malformed(format!("PDF structure error: {other}")),
    }
}

/// Parses a user page selection into 1-based page numbers.
///
/// Accepts either a JSON array of integers or a string like `"1-3, 5, 8-"`.
/// Validates every page against `total`.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(untagged)]
pub enum PageSelection {
    /// Every page.
    #[default]
    All,
    /// Explicit 1-based page numbers.
    List(Vec<u32>),
    /// Range specification such as `"1-3,5,8-"`.
    Spec(String),
}

impl PageSelection {
    /// Expands the selection into sorted, validated 1-based page numbers.
    pub fn resolve(&self, total: usize) -> EngineResult<Vec<u32>> {
        let total_u = total as u32;
        let out = match self {
            PageSelection::All => (1..=total_u).collect(),
            PageSelection::List(list) => list.clone(),
            PageSelection::Spec(spec) => parse_page_spec(spec, total_u)?,
        };
        if out.is_empty() {
            return Err(EngineError::empty("page selection"));
        }
        for &p in &out {
            if p == 0 || p > total_u {
                return Err(EngineError::not_found(format!(
                    "Page {p} is out of range (document has {total} pages)"
                )));
            }
        }
        Ok(out)
    }

    /// `true` when every page is selected.
    pub fn is_all(&self) -> bool {
        matches!(self, PageSelection::All)
    }
}

/// Parses `"1-3,5,8-"` style ranges. Open ended ranges use `total`.
pub fn parse_page_spec(spec: &str, total: u32) -> EngineResult<Vec<u32>> {
    let mut pages = Vec::new();
    for part in spec.split([',', ';', ' ']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((a, b)) = part.split_once('-') {
            let start: u32 = if a.trim().is_empty() {
                1
            } else {
                a.trim()
                    .parse()
                    .map_err(|_| EngineError::invalid(format!("Invalid page range `{part}`")))?
            };
            let end: u32 = if b.trim().is_empty() {
                total
            } else {
                b.trim()
                    .parse()
                    .map_err(|_| EngineError::invalid(format!("Invalid page range `{part}`")))?
            };
            if start == 0 || (end < start && !b.trim().is_empty()) {
                return Err(EngineError::invalid(format!("Invalid page range `{part}`")));
            }
            if end < start {
                return Err(EngineError::not_found(format!(
                    "Page {start} is out of range (document has {total} pages)"
                )));
            }
            pages.extend(start..=end);
        } else {
            let n: u32 = part
                .parse()
                .map_err(|_| EngineError::invalid(format!("Invalid page number `{part}`")))?;
            pages.push(n);
        }
    }
    Ok(pages)
}

/// A page rectangle in PDF points.
#[derive(Serialize, Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Left x coordinate.
    pub x0: f64,
    /// Bottom y coordinate.
    pub y0: f64,
    /// Right x coordinate.
    pub x1: f64,
    /// Top y coordinate.
    pub y1: f64,
}

impl Rect {
    /// Absolute width.
    pub fn width(&self) -> f64 {
        (self.x1 - self.x0).abs()
    }
    /// Absolute height.
    pub fn height(&self) -> f64 {
        (self.y1 - self.y0).abs()
    }
    /// Converts to a PDF array object.
    pub fn to_object(self) -> Object {
        Object::Array(vec![
            Object::Real(self.x0 as f32),
            Object::Real(self.y0 as f32),
            Object::Real(self.x1 as f32),
            Object::Real(self.y1 as f32),
        ])
    }
    /// Reads a rectangle from a (possibly indirect) PDF array.
    pub fn from_object(doc: &Document, obj: &Object) -> Option<Rect> {
        let obj = doc.dereference(obj).ok()?.1;
        let arr = obj.as_array().ok()?;
        if arr.len() != 4 {
            return None;
        }
        let mut v = [0f64; 4];
        for (i, o) in arr.iter().enumerate() {
            let o = doc.dereference(o).ok()?.1;
            v[i] = o.as_float().ok()? as f64;
        }
        Some(Rect {
            x0: v[0].min(v[2]),
            y0: v[1].min(v[3]),
            x1: v[0].max(v[2]),
            y1: v[1].max(v[3]),
        })
    }
}

/// Looks up an inheritable page attribute (`MediaBox`, `CropBox`, `Rotate`,
/// `Resources`) walking up the `Parent` chain.
pub fn inherited_attr<'a>(doc: &'a Document, page_id: ObjectId, key: &[u8]) -> Option<&'a Object> {
    let mut current = Some(page_id);
    let mut guard = 0;
    while let Some(id) = current {
        guard += 1;
        if guard > 64 {
            return None;
        }
        let dict = doc.get_dictionary(id).ok()?;
        if let Ok(v) = dict.get(key) {
            return Some(v);
        }
        current = dict.get(b"Parent").ok().and_then(|p| p.as_reference().ok());
    }
    None
}

/// Effective MediaBox of a page (falls back to US Letter when missing).
pub fn media_box(doc: &Document, page_id: ObjectId) -> Rect {
    inherited_attr(doc, page_id, b"MediaBox")
        .and_then(|o| Rect::from_object(doc, o))
        .unwrap_or(Rect { x0: 0.0, y0: 0.0, x1: 612.0, y1: 792.0 })
}

/// Effective CropBox (defaults to the MediaBox).
pub fn crop_box(doc: &Document, page_id: ObjectId) -> Rect {
    inherited_attr(doc, page_id, b"CropBox")
        .and_then(|o| Rect::from_object(doc, o))
        .unwrap_or_else(|| media_box(doc, page_id))
}

/// Effective `/Rotate` value normalised to 0/90/180/270.
pub fn rotation(doc: &Document, page_id: ObjectId) -> i64 {
    let r = inherited_attr(doc, page_id, b"Rotate")
        .and_then(|o| doc.dereference(o).ok())
        .and_then(|(_, o)| o.as_i64().ok())
        .unwrap_or(0);
    ((r % 360) + 360) % 360 / 90 * 90
}

/// Standard paper sizes in points (width, height, portrait).
pub const PAGE_SIZES: &[(&str, f64, f64)] = &[
    ("A0", 2383.94, 3370.39),
    ("A1", 1683.78, 2383.94),
    ("A2", 1190.55, 1683.78),
    ("A3", 841.89, 1190.55),
    ("A4", 595.28, 841.89),
    ("A5", 419.53, 595.28),
    ("A6", 297.64, 419.53),
    ("B4", 708.66, 1000.63),
    ("B5", 498.9, 708.66),
    ("Letter", 612.0, 792.0),
    ("Legal", 612.0, 1008.0),
    ("Tabloid", 792.0, 1224.0),
    ("Executive", 522.0, 756.0),
];

/// Resolves a named page size (case-insensitive, optional `-landscape`
/// suffix) or a `WxH` string in points / mm / in.
pub fn page_size_from_name(name: &str) -> EngineResult<(f64, f64)> {
    let lower = name.trim().to_ascii_lowercase();
    let (base, landscape) = match lower.strip_suffix("-landscape") {
        Some(b) => (b.to_string(), true),
        None => match lower.strip_suffix(" landscape") {
            Some(b) => (b.to_string(), true),
            None => (lower.clone(), false),
        },
    };
    for (n, w, h) in PAGE_SIZES {
        if n.to_ascii_lowercase() == base {
            return Ok(if landscape { (*h, *w) } else { (*w, *h) });
        }
    }
    // Custom "WxH" with optional unit suffix (pt default, mm, in).
    if let Some((w, h)) = base.split_once('x') {
        let parse = |s: &str| -> Option<f64> {
            let s = s.trim();
            let (num, factor) = if let Some(v) = s.strip_suffix("mm") {
                (v, PT_PER_MM)
            } else if let Some(v) = s.strip_suffix("in") {
                (v, PT_PER_INCH)
            } else if let Some(v) = s.strip_suffix("pt") {
                (v, 1.0)
            } else {
                (s, 1.0)
            };
            num.trim().parse::<f64>().ok().map(|n| n * factor)
        };
        if let (Some(w), Some(h)) = (parse(w), parse(h)) {
            if w > 0.0 && h > 0.0 && w <= 14400.0 && h <= 14400.0 {
                return Ok((w, h));
            }
        }
    }
    Err(EngineError::invalid(format!(
        "Unknown page size `{name}`. Use A4, Letter, Legal, A3, ... (optionally with -landscape) \
         or WxH (pt, mm or in)."
    )))
}

/// Finds the closest named paper size for a width/height (points), if any is
/// within 1% on both axes. Returns `(name, orientation)`.
pub fn match_page_size(w: f64, h: f64) -> Option<(String, &'static str)> {
    for (n, pw, ph) in PAGE_SIZES {
        let close = |a: f64, b: f64| ((a - b).abs() / b) < 0.01;
        if close(w, *pw) && close(h, *ph) {
            return Some((n.to_string(), "portrait"));
        }
        if close(w, *ph) && close(h, *pw) {
            return Some((n.to_string(), "landscape"));
        }
    }
    None
}

/// Returns the page object ids ordered by page number (1-based).
pub fn ordered_pages(doc: &Document) -> Vec<ObjectId> {
    doc.get_pages().into_values().collect()
}

/// Rebuilds a flat `/Pages` tree containing exactly `page_ids` (in order).
/// Any intermediate `/Pages` nodes are discarded; page objects get their
/// `/Parent` updated and inherited attributes are materialised on each page so
/// nothing is lost when the old tree goes away.
pub fn rebuild_page_tree(doc: &mut Document, page_ids: &[ObjectId]) -> EngineResult<()> {
    // Materialise inheritable attributes first (they live on the old tree).
    for &pid in page_ids {
        for key in [&b"MediaBox"[..], b"CropBox", b"Rotate", b"Resources"] {
            if let Some(v) = inherited_attr(doc, pid, key).cloned() {
                if let Ok(dict) = doc.get_dictionary_mut(pid) {
                    if !dict.has(key) {
                        dict.set(key.to_vec(), v);
                    }
                }
            }
        }
    }

    let catalog_id = doc
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .map_err(|_| EngineError::malformed("PDF trailer has no /Root catalog"))?;

    let pages_id = match doc
        .catalog()
        .ok()
        .and_then(|c| c.get(b"Pages").ok())
        .and_then(|p| p.as_reference().ok())
    {
        Some(id) => id,
        None => doc.new_object_id(),
    };

    let kids: Vec<Object> = page_ids.iter().map(|&id| Object::Reference(id)).collect();
    let mut pages_dict = Dictionary::new();
    pages_dict.set("Type", Object::Name(b"Pages".to_vec()));
    pages_dict.set("Count", Object::Integer(page_ids.len() as i64));
    pages_dict.set("Kids", Object::Array(kids));
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    for &pid in page_ids {
        if let Ok(dict) = doc.get_dictionary_mut(pid) {
            dict.set("Parent", Object::Reference(pages_id));
        }
    }
    if let Ok(catalog) = doc.get_dictionary_mut(catalog_id) {
        catalog.set("Pages", Object::Reference(pages_id));
    }
    Ok(())
}

/// Deep-copies a page (and everything it references) from `src` into `dst`,
/// returning the new page id. Object ids are offset so they never collide.
pub fn import_page(
    dst: &mut Document,
    src: &Document,
    page_id: ObjectId,
) -> EngineResult<ObjectId> {
    use std::collections::{HashMap, VecDeque};

    let mut map: HashMap<ObjectId, ObjectId> = HashMap::new();
    let mut queue: VecDeque<ObjectId> = VecDeque::new();
    queue.push_back(page_id);
    map.insert(page_id, dst.new_object_id());

    fn collect_refs(obj: &Object, out: &mut Vec<ObjectId>) {
        match obj {
            Object::Reference(r) => out.push(*r),
            Object::Array(a) => a.iter().for_each(|o| collect_refs(o, out)),
            Object::Dictionary(d) => d.iter().for_each(|(_, o)| collect_refs(o, out)),
            Object::Stream(s) => s.dict.iter().for_each(|(_, o)| collect_refs(o, out)),
            _ => {},
        }
    }

    fn remap(obj: &mut Object, map: &HashMap<ObjectId, ObjectId>) {
        match obj {
            Object::Reference(r) => {
                if let Some(n) = map.get(r) {
                    *r = *n;
                }
            },
            Object::Array(a) => a.iter_mut().for_each(|o| remap(o, map)),
            Object::Dictionary(d) => d.iter_mut().for_each(|(_, o)| remap(o, map)),
            Object::Stream(s) => s.dict.iter_mut().for_each(|(_, o)| remap(o, map)),
            _ => {},
        }
    }

    // Breadth-first walk of every reachable object, skipping the parent
    // chain (the destination tree provides the parent).
    let mut order = Vec::new();
    while let Some(id) = queue.pop_front() {
        let obj = match src.get_object(id) {
            Ok(o) => o,
            Err(_) => continue,
        };
        order.push(id);
        let mut refs = Vec::new();
        if id == page_id {
            // Do not drag the old /Pages tree along.
            if let Ok(dict) = obj.as_dict() {
                for (k, v) in dict.iter() {
                    if k != b"Parent" {
                        collect_refs(v, &mut refs);
                    }
                }
            }
        } else {
            collect_refs(obj, &mut refs);
        }
        for r in refs {
            if let std::collections::hash_map::Entry::Vacant(e) = map.entry(r) {
                // Skip references back into the page tree.
                if let Ok(d) = src.get_dictionary(r) {
                    if d.has_type(b"Pages") {
                        continue;
                    }
                }
                e.insert(dst.new_object_id());
                queue.push_back(r);
            }
        }
        if map.len() > 2_000_000 {
            return Err(EngineError::limit("PDF object graph is too large to copy"));
        }
    }

    for id in order {
        let mut obj = src.get_object(id).map_err(map_pdf_error)?.clone();
        if id == page_id {
            if let Ok(dict) = obj.as_dict_mut() {
                dict.remove(b"Parent");
                // Materialise inherited attributes on the copied page.
                for key in [&b"MediaBox"[..], b"CropBox", b"Rotate", b"Resources"] {
                    if !dict.has(key) {
                        if let Some(v) = inherited_attr(src, page_id, key).cloned() {
                            dict.set(key.to_vec(), v);
                        }
                    }
                }
            }
        }
        remap(&mut obj, &map);
        dst.objects.insert(map[&id], obj);
    }
    Ok(map[&page_id])
}

/// Creates an empty document with a catalog and an (empty) pages tree.
pub fn new_document() -> (Document, ObjectId) {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let mut pages = Dictionary::new();
    pages.set("Type", Object::Name(b"Pages".to_vec()));
    pages.set("Count", Object::Integer(0));
    pages.set("Kids", Object::Array(vec![]));
    doc.objects.insert(pages_id, Object::Dictionary(pages));
    let mut catalog = Dictionary::new();
    catalog.set("Type", Object::Name(b"Catalog".to_vec()));
    catalog.set("Pages", Object::Reference(pages_id));
    let catalog_id = doc.add_object(Object::Dictionary(catalog));
    doc.trailer.set("Root", Object::Reference(catalog_id));
    (doc, pages_id)
}

/// Appends a page object to the flat pages tree created by [`new_document`].
pub fn append_page(doc: &mut Document, pages_id: ObjectId, mut page: Dictionary) -> ObjectId {
    page.set("Type", Object::Name(b"Page".to_vec()));
    page.set("Parent", Object::Reference(pages_id));
    let page_id = doc.add_object(Object::Dictionary(page));
    if let Ok(pages) = doc.get_dictionary_mut(pages_id) {
        let mut kids = pages
            .get(b"Kids")
            .ok()
            .and_then(|k| k.as_array().ok())
            .cloned()
            .unwrap_or_default();
        kids.push(Object::Reference(page_id));
        let count = kids.len() as i64;
        pages.set("Kids", Object::Array(kids));
        pages.set("Count", Object::Integer(count));
    }
    page_id
}

/// Escapes a string for use inside a PDF literal string `( ... )`.
pub fn escape_pdf_string(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 8);
    for &b in bytes {
        match b {
            b'(' | b')' | b'\\' => {
                out.push(b'\\');
                out.push(b);
            },
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b'\t' => out.extend_from_slice(b"\\t"),
            _ => out.push(b),
        }
    }
    out
}

/// Converts a PDF text string object (PDFDocEncoding or UTF-16BE with BOM)
/// into a Rust string.
pub fn text_string(obj: &Object) -> Option<String> {
    match obj {
        Object::String(bytes, _) => Some(decode_text_string(bytes)),
        _ => None,
    }
}

/// Decodes a PDF text string (PDFDocEncoding or UTF-16BE with BOM) to UTF-8.
pub fn decode_text_string(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let units: Vec<u16> = bytes[2..]
            .chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        String::from_utf8_lossy(&bytes[3..]).into_owned()
    } else {
        // PDFDocEncoding is a superset of ASCII; treat as Latin-1.
        bytes.iter().map(|&b| b as char).collect()
    }
}

/// Encodes a Rust string as a PDF text string (UTF-16BE with BOM when it
/// contains non-ASCII characters).
pub fn encode_text_string(s: &str) -> Object {
    if s.is_ascii() {
        Object::string_literal(s.as_bytes().to_vec())
    } else {
        let mut out = vec![0xFE, 0xFF];
        for u in s.encode_utf16() {
            out.extend_from_slice(&u.to_be_bytes());
        }
        Object::String(out, lopdf::StringFormat::Hexadecimal)
    }
}

/// Formats a PDF date string (`D:YYYYMMDDHHmmSS...`) as ISO-8601 when possible.
pub fn pdf_date_to_iso(raw: &str) -> Option<String> {
    let s = raw.trim().trim_start_matches("D:");
    if s.len() < 4 || !s[..4].chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let digit = |from: usize, len: usize, default: &str| -> String {
        if s.len() >= from + len && s[from..from + len].chars().all(|c| c.is_ascii_digit()) {
            s[from..from + len].to_string()
        } else {
            default.to_string()
        }
    };
    let year = digit(0, 4, "0000");
    let month = digit(4, 2, "01");
    let day = digit(6, 2, "01");
    let hour = digit(8, 2, "00");
    let min = digit(10, 2, "00");
    let sec = digit(12, 2, "00");
    let mut iso = format!("{year}-{month}-{day}T{hour}:{min}:{sec}");
    if s.len() > 14 {
        let tz = &s[14..];
        match tz.chars().next() {
            Some('Z') => iso.push('Z'),
            Some('+') | Some('-') => {
                let sign = &tz[..1];
                let rest: String = tz[1..].chars().filter(|c| c.is_ascii_digit()).collect();
                if rest.len() >= 2 {
                    let hh = &rest[..2];
                    let mm = if rest.len() >= 4 { &rest[2..4] } else { "00" };
                    iso.push_str(&format!("{sign}{hh}:{mm}"));
                }
            },
            _ => {},
        }
    }
    Some(iso)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_spec_parsing() {
        assert_eq!(parse_page_spec("1-3,5,8-", 10).unwrap(), vec![1, 2, 3, 5, 8, 9, 10]);
        assert_eq!(parse_page_spec("-2", 5).unwrap(), vec![1, 2]);
        assert!(parse_page_spec("3-1", 5).is_err());
        assert!(parse_page_spec("x", 5).is_err());
        assert!(PageSelection::Spec("1,99".into()).resolve(5).is_err());
        assert!(PageSelection::List(vec![0]).resolve(5).is_err());
        assert!(PageSelection::List(vec![]).resolve(5).is_err());
        assert_eq!(PageSelection::All.resolve(3).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn page_sizes() {
        assert_eq!(page_size_from_name("a4").unwrap(), (595.28, 841.89));
        assert_eq!(page_size_from_name("A4-landscape").unwrap(), (841.89, 595.28));
        assert_eq!(page_size_from_name("Letter").unwrap(), (612.0, 792.0));
        let (w, h) = page_size_from_name("100mm x 200mm").unwrap();
        assert!((w - 283.46).abs() < 0.1 && (h - 566.93).abs() < 0.1);
        assert!(page_size_from_name("nope").is_err());
        assert_eq!(match_page_size(595.0, 842.0).unwrap().0, "A4");
        assert_eq!(match_page_size(792.0, 612.0).unwrap().1, "landscape");
        assert!(match_page_size(100.0, 100.0).is_none());
    }

    #[test]
    fn dates_and_strings() {
        assert_eq!(
            pdf_date_to_iso("D:20240102030405+05'30'").unwrap(),
            "2024-01-02T03:04:05+05:30"
        );
        assert_eq!(pdf_date_to_iso("D:20240102").unwrap(), "2024-01-02T00:00:00");
        assert_eq!(pdf_date_to_iso("D:20240102030405Z").unwrap(), "2024-01-02T03:04:05Z");
        assert!(pdf_date_to_iso("garbage").is_none());
        assert_eq!(decode_text_string(&[0xFE, 0xFF, 0x00, 0x41]), "A");
        assert_eq!(decode_text_string(b"Hi"), "Hi");
        assert_eq!(escape_pdf_string(b"a(b)\\"), b"a\\(b\\)\\\\");
        match encode_text_string("é") {
            Object::String(b, _) => assert_eq!(b, vec![0xFE, 0xFF, 0x00, 0xE9]),
            _ => panic!(),
        }
    }

    #[test]
    fn header_detection_and_load_errors() {
        assert!(looks_like_pdf(b"%PDF-1.4"));
        assert!(looks_like_pdf(b"junk\n%PDF-1.4"));
        assert!(!looks_like_pdf(b"PNG"));
        assert_eq!(load(b"", None).unwrap_err().code, engine_shared::error::ErrorCode::EmptyInput);
        assert_eq!(
            load(b"hello world", None).unwrap_err().code,
            engine_shared::error::ErrorCode::UnsupportedFormat
        );
        assert_eq!(
            load(b"%PDF-1.4 garbage", None).unwrap_err().code,
            engine_shared::error::ErrorCode::Malformed
        );
    }
}
