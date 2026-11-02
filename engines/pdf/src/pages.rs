//! Page level operations: merge, split, rotate, crop, reorder, delete,
//! extract, duplicate and resize.

use crate::doc::{self, PageSelection, Rect};
use engine_shared::{b64, limits, EngineError, EngineResult};
use lopdf::{Document, Object, ObjectId};
use serde::Serialize;
use serde_json::{json, Value};

/// Standard response for actions that return a PDF.
#[derive(Serialize)]
pub struct PdfOutput {
    /// Base64-encoded PDF bytes.
    pub file: String,
    /// Always `application/pdf`.
    pub mime: &'static str,
    /// Page count of the output.
    pub pages: usize,
    /// Size in bytes.
    pub bytes: usize,
}

impl PdfOutput {
    /// Encodes `bytes` and records the page count.
    pub fn new(bytes: Vec<u8>, pages: usize) -> Self {
        PdfOutput {
            bytes: bytes.len(),
            file: b64::encode(&bytes),
            mime: "application/pdf",
            pages,
        }
    }
}

/// Merges several PDFs (in order) into one document.
pub fn merge(files: &[Vec<u8>], passwords: &[String]) -> EngineResult<Value> {
    limits::check_count(files.len(), limits::MAX_BATCH_FILES, "files")?;
    let (mut out, pages_id) = doc::new_document();
    let mut sources = Vec::new();
    let mut total_pages = 0usize;

    for (i, bytes) in files.iter().enumerate() {
        let pw = passwords.get(i).map(String::as_str).filter(|p| !p.is_empty());
        let src = doc::load(bytes, pw)
            .map_err(|e| EngineError::new(e.code, format!("File {}: {}", i + 1, e.message)))?;
        let src_pages = doc::ordered_pages(&src);
        let mut imported = Vec::new();
        for pid in src_pages {
            let new_id = doc::import_page(&mut out, &src, pid)?;
            imported.push(new_id);
        }
        total_pages += imported.len();
        sources.push(json!({ "index": i + 1, "pages": imported.len() }));
        append_pages(&mut out, pages_id, &imported);
    }
    if total_pages == 0 {
        return Err(EngineError::invalid("None of the input files contain pages"));
    }
    let bytes = doc::save(&mut out)?;
    Ok(json!({
        "output": PdfOutput::new(bytes, total_pages),
        "sources": sources,
    }))
}

fn append_pages(doc: &mut Document, pages_id: ObjectId, ids: &[ObjectId]) {
    for &pid in ids {
        if let Ok(dict) = doc.get_dictionary_mut(pid) {
            dict.set("Type", Object::Name(b"Page".to_vec()));
            dict.set("Parent", Object::Reference(pages_id));
        }
    }
    if let Ok(pages) = doc.get_dictionary_mut(pages_id) {
        let mut kids = pages
            .get(b"Kids")
            .ok()
            .and_then(|k| k.as_array().ok())
            .cloned()
            .unwrap_or_default();
        kids.extend(ids.iter().map(|&id| Object::Reference(id)));
        let count = kids.len() as i64;
        pages.set("Kids", Object::Array(kids));
        pages.set("Count", Object::Integer(count));
    }
}

/// Builds a new document containing only `page_numbers` (1-based, in the
/// given order; duplicates allowed) from `src`.
pub fn subset(src: &Document, page_numbers: &[u32]) -> EngineResult<Document> {
    let (mut out, pages_id) = doc::new_document();
    let src_pages = src.get_pages();
    let mut ids = Vec::new();
    for &n in page_numbers {
        let pid = *src_pages
            .get(&n)
            .ok_or_else(|| EngineError::not_found(format!("Page {n} does not exist")))?;
        ids.push(doc::import_page(&mut out, src, pid)?);
    }
    append_pages(&mut out, pages_id, &ids);
    copy_document_info(src, &mut out);
    Ok(out)
}

/// Copies the `/Info` dictionary (title, author, ...) between documents.
pub fn copy_document_info(src: &Document, dst: &mut Document) {
    if let Some(info) = src
        .trailer
        .get(b"Info")
        .ok()
        .and_then(|o| src.dereference(o).ok())
        .and_then(|(_, o)| o.as_dict().ok())
        .cloned()
    {
        let id = dst.add_object(Object::Dictionary(info));
        dst.trailer.set("Info", Object::Reference(id));
    }
}

/// How `split` divides a document.
#[derive(serde::Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SplitMode {
    /// One output per page.
    #[default]
    Each,
    /// Explicit ranges, one output per range.
    Ranges,
    /// Fixed size chunks of `n` pages.
    EveryN,
}

/// Splits a document.
pub fn split(
    bytes: &[u8],
    password: Option<&str>,
    mode: SplitMode,
    ranges: &[String],
    n: usize,
) -> EngineResult<Value> {
    let src = doc::load(bytes, password)?;
    let total = src.get_pages().len();
    if total == 0 {
        return Err(EngineError::invalid("The PDF has no pages"));
    }
    let groups: Vec<(String, Vec<u32>)> = match mode {
        SplitMode::Each => (1..=total as u32).map(|p| (format!("page-{p}"), vec![p])).collect(),
        SplitMode::Ranges => {
            if ranges.is_empty() {
                return Err(EngineError::empty("ranges"));
            }
            ranges
                .iter()
                .map(|r| {
                    PageSelection::Spec(r.clone())
                        .resolve(total)
                        .map(|pages| (format!("pages-{}", r.replace([' ', ','], "_")), pages))
                })
                .collect::<EngineResult<Vec<_>>>()?
        },
        SplitMode::EveryN => {
            if n == 0 {
                return Err(EngineError::invalid("`n` must be at least 1"));
            }
            (1..=total as u32)
                .collect::<Vec<_>>()
                .chunks(n)
                .map(|c| (format!("pages-{}-{}", c[0], c[c.len() - 1]), c.to_vec()))
                .collect()
        },
    };
    if groups.len() > limits::MAX_BATCH_FILES {
        return Err(EngineError::limit(format!(
            "Split would produce {} files (limit {})",
            groups.len(),
            limits::MAX_BATCH_FILES
        )));
    }
    let mut outputs = Vec::new();
    for (name, pages) in groups {
        let mut part = subset(&src, &pages)?;
        let out = doc::save(&mut part)?;
        outputs.push(json!({
            "name": format!("{name}.pdf"),
            "pages": pages,
            "output": PdfOutput::new(out, pages.len()),
        }));
    }
    Ok(json!({ "total_pages": total, "files": outputs }))
}

/// Rotates pages by a multiple of 90 degrees (added to the existing rotation).
pub fn rotate(
    bytes: &[u8],
    password: Option<&str>,
    angle: i64,
    pages: &PageSelection,
) -> EngineResult<Value> {
    if angle % 90 != 0 {
        return Err(EngineError::invalid("Rotation angle must be a multiple of 90 degrees"));
    }
    let mut d = doc::load(bytes, password)?;
    let total = d.get_pages().len();
    let selected = pages.resolve(total)?;
    let page_map = d.get_pages();
    for &n in &selected {
        let pid = page_map[&n];
        let current = doc::rotation(&d, pid);
        let new_rot = (((current + angle) % 360) + 360) % 360;
        if let Ok(dict) = d.get_dictionary_mut(pid) {
            dict.set("Rotate", Object::Integer(new_rot));
        }
    }
    let out = doc::save(&mut d)?;
    Ok(json!({
        "output": PdfOutput::new(out, total),
        "rotated_pages": selected,
        "angle": angle,
    }))
}

/// Crop specification: either an explicit box or margins to remove.
#[derive(serde::Deserialize, Debug, Clone, Default)]
pub struct CropSpec {
    /// Explicit box `[x0, y0, x1, y1]` in points (PDF coordinates, origin bottom-left).
    #[serde(default)]
    pub r#box: Option<[f64; 4]>,
    /// Margins to trim in points.
    #[serde(default)]
    pub top: f64,
    /// Right margin to trim.
    #[serde(default)]
    pub right: f64,
    /// Bottom margin to trim.
    #[serde(default)]
    pub bottom: f64,
    /// Left margin to trim.
    #[serde(default)]
    pub left: f64,
    /// `pt` (default), `mm`, `in` or `percent` for the margins.
    #[serde(default)]
    pub unit: Option<String>,
    /// Which box to set: `crop` (default, CropBox) or `media` (MediaBox).
    #[serde(default)]
    pub target: Option<String>,
}

/// Sets the crop (or media) box on the selected pages.
pub fn crop(
    bytes: &[u8],
    password: Option<&str>,
    spec: &CropSpec,
    pages: &PageSelection,
) -> EngineResult<Value> {
    let mut d = doc::load(bytes, password)?;
    let total = d.get_pages().len();
    let selected = pages.resolve(total)?;
    let page_map = d.get_pages();
    let key: &[u8] = match spec.target.as_deref().unwrap_or("crop") {
        "media" | "mediabox" => b"MediaBox",
        "crop" | "cropbox" => b"CropBox",
        other => return Err(EngineError::invalid(format!("Unknown crop target `{other}`"))),
    };
    let factor = match spec.unit.as_deref().unwrap_or("pt") {
        "pt" => 1.0,
        "mm" => doc::PT_PER_MM,
        "in" => doc::PT_PER_INCH,
        "percent" | "%" => -1.0,
        other => return Err(EngineError::invalid(format!("Unknown unit `{other}`"))),
    };
    let mut results = Vec::new();
    for &n in &selected {
        let pid = page_map[&n];
        let base = doc::media_box(&d, pid);
        let new_rect = if let Some(b) = spec.r#box {
            Rect {
                x0: b[0].min(b[2]),
                y0: b[1].min(b[3]),
                x1: b[0].max(b[2]),
                y1: b[1].max(b[3]),
            }
        } else {
            let (l, r, t, btm) = if factor < 0.0 {
                (
                    base.width() * spec.left / 100.0,
                    base.width() * spec.right / 100.0,
                    base.height() * spec.top / 100.0,
                    base.height() * spec.bottom / 100.0,
                )
            } else {
                (spec.left * factor, spec.right * factor, spec.top * factor, spec.bottom * factor)
            };
            Rect {
                x0: base.x0 + l,
                y0: base.y0 + btm,
                x1: base.x1 - r,
                y1: base.y1 - t,
            }
        };
        if new_rect.width() < 1.0
            || new_rect.height() < 1.0
            || new_rect.x1 <= new_rect.x0
            || new_rect.y1 <= new_rect.y0
        {
            return Err(EngineError::invalid(format!(
                "Crop box for page {n} is empty ({:.1} x {:.1} pt)",
                new_rect.width(),
                new_rect.height()
            )));
        }
        if let Ok(dict) = d.get_dictionary_mut(pid) {
            dict.set(key.to_vec(), new_rect.to_object());
            if key == b"MediaBox" {
                dict.remove(b"CropBox");
            }
        }
        results.push(json!({ "page": n, "box": new_rect }));
    }
    let out = doc::save(&mut d)?;
    Ok(json!({ "output": PdfOutput::new(out, total), "pages": results }))
}

/// Reorders pages. `order` lists 1-based page numbers; pages not listed are
/// appended in their original order unless `drop_missing` is true.
pub fn reorder(
    bytes: &[u8],
    password: Option<&str>,
    order: &[u32],
    drop_missing: bool,
) -> EngineResult<Value> {
    let src = doc::load(bytes, password)?;
    let total = src.get_pages().len();
    if order.is_empty() {
        return Err(EngineError::empty("order"));
    }
    let mut seen = std::collections::HashSet::new();
    for &p in order {
        if p == 0 || p as usize > total {
            return Err(EngineError::not_found(format!("Page {p} is out of range (1-{total})")));
        }
        if !seen.insert(p) {
            return Err(EngineError::invalid(format!(
                "Page {p} appears more than once in `order`"
            )));
        }
    }
    let mut final_order = order.to_vec();
    if !drop_missing {
        for p in 1..=total as u32 {
            if !seen.contains(&p) {
                final_order.push(p);
            }
        }
    }
    let mut out = subset(&src, &final_order)?;
    let bytes = doc::save(&mut out)?;
    Ok(json!({ "output": PdfOutput::new(bytes, final_order.len()), "order": final_order }))
}

/// Deletes the selected pages.
pub fn delete_pages(
    bytes: &[u8],
    password: Option<&str>,
    pages: &PageSelection,
) -> EngineResult<Value> {
    let src = doc::load(bytes, password)?;
    let total = src.get_pages().len();
    let remove: std::collections::HashSet<u32> = pages.resolve(total)?.into_iter().collect();
    let keep: Vec<u32> = (1..=total as u32).filter(|p| !remove.contains(p)).collect();
    if keep.is_empty() {
        return Err(EngineError::invalid("Cannot delete every page of the document"));
    }
    let mut out = subset(&src, &keep)?;
    let bytes = doc::save(&mut out)?;
    Ok(json!({
        "output": PdfOutput::new(bytes, keep.len()),
        "deleted": remove.into_iter().collect::<std::collections::BTreeSet<_>>(),
        "remaining": keep.len(),
    }))
}

/// Extracts the selected pages into a new document (keeps selection order).
pub fn extract_pages(
    bytes: &[u8],
    password: Option<&str>,
    pages: &PageSelection,
) -> EngineResult<Value> {
    let src = doc::load(bytes, password)?;
    let total = src.get_pages().len();
    let selected = pages.resolve(total)?;
    let mut out = subset(&src, &selected)?;
    let bytes = doc::save(&mut out)?;
    Ok(json!({ "output": PdfOutput::new(bytes, selected.len()), "pages": selected }))
}

/// Duplicates the selected pages `copies` times (copies follow the original).
pub fn duplicate_pages(
    bytes: &[u8],
    password: Option<&str>,
    pages: &PageSelection,
    copies: usize,
) -> EngineResult<Value> {
    if copies == 0 || copies > 100 {
        return Err(EngineError::invalid("`copies` must be between 1 and 100"));
    }
    let src = doc::load(bytes, password)?;
    let total = src.get_pages().len();
    let dup: std::collections::HashSet<u32> = pages.resolve(total)?.into_iter().collect();
    let mut order = Vec::new();
    for p in 1..=total as u32 {
        order.push(p);
        if dup.contains(&p) {
            for _ in 0..copies {
                order.push(p);
            }
        }
    }
    if order.len() > limits::MAX_PDF_PAGES {
        return Err(EngineError::limit("Resulting document would exceed the page limit"));
    }
    let mut out = subset(&src, &order)?;
    let bytes = doc::save(&mut out)?;
    Ok(json!({ "output": PdfOutput::new(bytes, order.len()), "order": order }))
}

/// Resizes pages to a named size (or WxH). `mode`:
/// * `scale`  – scale content proportionally to fit (default),
/// * `center` – keep content size and centre it on the new page,
/// * `stretch`– scale non-proportionally to fill.
pub fn resize_pages(
    bytes: &[u8],
    password: Option<&str>,
    size: &str,
    mode: &str,
    pages: &PageSelection,
) -> EngineResult<Value> {
    let (tw, th) = doc::page_size_from_name(size)?;
    let mut d = doc::load(bytes, password)?;
    let total = d.get_pages().len();
    let selected = pages.resolve(total)?;
    let page_map = d.get_pages();
    for &n in &selected {
        let pid = page_map[&n];
        let mb = doc::media_box(&d, pid);
        let (sw, sh) = (mb.width(), mb.height());
        let (sx, sy) = match mode {
            "scale" | "fit" => {
                let s = (tw / sw).min(th / sh);
                (s, s)
            },
            "center" => (1.0, 1.0),
            "stretch" => (tw / sw, th / sh),
            other => return Err(EngineError::invalid(format!("Unknown resize mode `{other}`"))),
        };
        let tx = (tw - sw * sx) / 2.0 - mb.x0 * sx;
        let ty = (th - sh * sy) / 2.0 - mb.y0 * sy;
        wrap_page_content(
            &mut d,
            pid,
            &format!("q {sx:.5} 0 0 {sy:.5} {tx:.3} {ty:.3} cm\n"),
            "\nQ\n",
        )?;
        if let Ok(dict) = d.get_dictionary_mut(pid) {
            dict.set("MediaBox", Rect { x0: 0.0, y0: 0.0, x1: tw, y1: th }.to_object());
            dict.remove(b"CropBox");
            dict.remove(b"BleedBox");
            dict.remove(b"TrimBox");
            dict.remove(b"ArtBox");
        }
    }
    let out = doc::save(&mut d)?;
    Ok(json!({
        "output": PdfOutput::new(out, total),
        "page_size": { "width_pt": tw, "height_pt": th },
        "resized_pages": selected,
    }))
}

/// Wraps the existing content stream(s) of a page with `prefix` / `suffix`.
/// The original streams are kept and referenced from a new content array so
/// the page's graphics state cannot leak into the wrapper.
pub fn wrap_page_content(
    doc: &mut Document,
    page_id: ObjectId,
    prefix: &str,
    suffix: &str,
) -> EngineResult<()> {
    let existing = doc.get_page_contents(page_id);
    let pre_id =
        doc.add_object(lopdf::Stream::new(lopdf::Dictionary::new(), prefix.as_bytes().to_vec()));
    let suf_id =
        doc.add_object(lopdf::Stream::new(lopdf::Dictionary::new(), suffix.as_bytes().to_vec()));
    let mut arr = vec![Object::Reference(pre_id)];
    arr.extend(existing.into_iter().map(Object::Reference));
    arr.push(Object::Reference(suf_id));
    let dict = doc.get_dictionary_mut(page_id).map_err(doc::map_pdf_error)?;
    dict.set("Contents", Object::Array(arr));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::sample_pdf;

    #[test]
    fn merge_split_roundtrip() {
        let a = sample_pdf(&["A1", "A2"]);
        let b = sample_pdf(&["B1"]);
        let merged = merge(&[a.clone(), b.clone()], &[]).unwrap();
        assert_eq!(merged["output"]["pages"], 3);
        let merged_bytes = b64::decode(merged["output"]["file"].as_str().unwrap()).unwrap();
        let d = doc::load(&merged_bytes, None).unwrap();
        assert_eq!(d.get_pages().len(), 3);
        assert!(d.extract_text(&[3]).unwrap().contains("B1"));

        let parts = split(&merged_bytes, None, SplitMode::Each, &[], 0).unwrap();
        assert_eq!(parts["files"].as_array().unwrap().len(), 3);
        let chunks = split(&merged_bytes, None, SplitMode::EveryN, &[], 2).unwrap();
        assert_eq!(chunks["files"].as_array().unwrap().len(), 2);
        let ranges =
            split(&merged_bytes, None, SplitMode::Ranges, &["1-2".into(), "3".into()], 0).unwrap();
        assert_eq!(ranges["files"][0]["pages"], json!([1, 2]));
        assert!(split(&merged_bytes, None, SplitMode::Ranges, &[], 0).is_err());
        assert!(split(&merged_bytes, None, SplitMode::EveryN, &[], 0).is_err());
        assert!(merge(&[], &[]).is_err());
        assert!(merge(&[b"nope".to_vec()], &[]).is_err());
    }

    #[test]
    fn rotate_crop_resize() {
        let a = sample_pdf(&["A1", "A2"]);
        let r = rotate(&a, None, 90, &PageSelection::List(vec![2])).unwrap();
        let bytes = b64::decode(r["output"]["file"].as_str().unwrap()).unwrap();
        let d = doc::load(&bytes, None).unwrap();
        let pages = d.get_pages();
        assert_eq!(doc::rotation(&d, pages[&1]), 0);
        assert_eq!(doc::rotation(&d, pages[&2]), 90);
        assert!(rotate(&a, None, 45, &PageSelection::All).is_err());

        let spec = CropSpec {
            top: 10.0,
            left: 10.0,
            ..Default::default()
        };
        let c = crop(&a, None, &spec, &PageSelection::All).unwrap();
        let bytes = b64::decode(c["output"]["file"].as_str().unwrap()).unwrap();
        let d = doc::load(&bytes, None).unwrap();
        let cb = doc::crop_box(&d, d.get_pages()[&1]);
        assert_eq!(cb.x0, 10.0);
        assert!((cb.y1 - (842.0 - 10.0)).abs() < 0.01);
        let bad = CropSpec { top: 5000.0, ..Default::default() };
        assert!(crop(&a, None, &bad, &PageSelection::All).is_err());

        let rs = resize_pages(&a, None, "Letter", "scale", &PageSelection::All).unwrap();
        let bytes = b64::decode(rs["output"]["file"].as_str().unwrap()).unwrap();
        let d = doc::load(&bytes, None).unwrap();
        let mb = doc::media_box(&d, d.get_pages()[&1]);
        assert_eq!((mb.width(), mb.height()), (612.0, 792.0));
        assert!(d.extract_text(&[1]).unwrap().contains("A1"));
        assert!(resize_pages(&a, None, "Letter", "bogus", &PageSelection::All).is_err());
    }

    #[test]
    fn reorder_delete_extract_duplicate() {
        let a = sample_pdf(&["P1", "P2", "P3"]);
        let r = reorder(&a, None, &[3, 1], false).unwrap();
        assert_eq!(r["order"], json!([3, 1, 2]));
        let bytes = b64::decode(r["output"]["file"].as_str().unwrap()).unwrap();
        let d = doc::load(&bytes, None).unwrap();
        assert!(d.extract_text(&[1]).unwrap().contains("P3"));
        assert!(reorder(&a, None, &[1, 1], false).is_err());
        assert!(reorder(&a, None, &[9], false).is_err());

        let del = delete_pages(&a, None, &PageSelection::Spec("2".into())).unwrap();
        assert_eq!(del["remaining"], 2);
        assert!(delete_pages(&a, None, &PageSelection::All).is_err());

        let ex = extract_pages(&a, None, &PageSelection::List(vec![3, 2])).unwrap();
        assert_eq!(ex["output"]["pages"], 2);

        let dup = duplicate_pages(&a, None, &PageSelection::List(vec![1]), 2).unwrap();
        assert_eq!(dup["order"], json!([1, 1, 1, 2, 3]));
        assert!(duplicate_pages(&a, None, &PageSelection::All, 0).is_err());
    }
}
