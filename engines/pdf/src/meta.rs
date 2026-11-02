//! Inspection and metadata: document info, page sizes, outlines/bookmarks,
//! form fields and digital signature inspection.

use crate::doc::{self, pdf_date_to_iso, text_string, Rect};
use crate::pages::PdfOutput;
use engine_shared::{EngineError, EngineResult};
use lopdf::{Bookmark, Dictionary, Document, Object, ObjectId, Outline};
use serde::Deserialize;
use serde_json::{json, Map, Value};

fn info_dict(d: &Document) -> Option<Dictionary> {
    d.trailer
        .get(b"Info")
        .ok()
        .and_then(|o| d.dereference(o).ok())
        .and_then(|(_, o)| o.as_dict().ok())
        .cloned()
}

fn value_to_json(d: &Document, obj: &Object, depth: usize) -> Value {
    if depth > 4 {
        return Value::String("...".into());
    }
    match d.dereference(obj) {
        Ok((_, o)) => match o {
            Object::Null => Value::Null,
            Object::Boolean(b) => Value::Bool(*b),
            Object::Integer(i) => json!(i),
            Object::Real(r) => json!(r),
            Object::Name(n) => Value::String(format!("/{}", String::from_utf8_lossy(n))),
            Object::String(_, _) => Value::String(text_string(o).unwrap_or_default()),
            Object::Array(a) => {
                Value::Array(a.iter().map(|x| value_to_json(d, x, depth + 1)).collect())
            },
            Object::Dictionary(dict) => Value::Object(
                dict.iter()
                    .map(|(k, v)| {
                        (String::from_utf8_lossy(k).into_owned(), value_to_json(d, v, depth + 1))
                    })
                    .collect(),
            ),
            Object::Stream(s) => json!({ "stream_bytes": s.content.len() }),
            Object::Reference(_) => Value::Null,
        },
        Err(_) => Value::Null,
    }
}

/// Full document metadata report.
pub fn metadata(bytes: &[u8], password: Option<&str>) -> EngineResult<Value> {
    let d = doc::load(bytes, password)?;
    let mut standard = Map::new();
    let mut custom = Map::new();
    if let Some(info) = info_dict(&d) {
        for (k, v) in info.iter() {
            let key = String::from_utf8_lossy(k).into_owned();
            let val = value_to_json(&d, v, 0);
            match key.as_str() {
                "Title" | "Author" | "Subject" | "Keywords" | "Creator" | "Producer"
                | "Trapped" => {
                    standard.insert(key.to_lowercase(), val);
                },
                "CreationDate" | "ModDate" => {
                    let raw = val.as_str().unwrap_or("").to_string();
                    let name = if key == "CreationDate" {
                        "creation_date"
                    } else {
                        "modification_date"
                    };
                    standard
                        .insert(name.into(), json!(pdf_date_to_iso(&raw).unwrap_or(raw.clone())));
                    standard.insert(format!("{name}_raw"), json!(raw));
                },
                _ => {
                    custom.insert(key, val);
                },
            }
        }
    }
    let pages = d.get_pages();
    let first = pages.values().next().copied();
    let first_box = first.map(|p| doc::media_box(&d, p));
    let has_xmp = d.catalog().ok().and_then(|c| c.get(b"Metadata").ok()).is_some();
    let has_forms = d.catalog().ok().and_then(|c| c.get(b"AcroForm").ok()).is_some();
    let has_outlines = d.catalog().ok().and_then(|c| c.get(b"Outlines").ok()).is_some();
    let page_layout = d
        .catalog()
        .ok()
        .and_then(|c| c.get(b"PageLayout").ok())
        .and_then(|o| o.as_name().ok())
        .map(|n| String::from_utf8_lossy(n).into_owned());
    let page_mode = d
        .catalog()
        .ok()
        .and_then(|c| c.get(b"PageMode").ok())
        .and_then(|o| o.as_name().ok())
        .map(|n| String::from_utf8_lossy(n).into_owned());
    let lang = d
        .catalog()
        .ok()
        .and_then(|c| c.get(b"Lang").ok())
        .and_then(text_string);
    let xmp_len = d
        .catalog()
        .ok()
        .and_then(|c| c.get(b"Metadata").ok())
        .and_then(|m| d.dereference(m).ok())
        .and_then(|(_, m)| match m {
            Object::Stream(s) => s.decompressed_content().ok().map(|c| c.len()),
            _ => None,
        });
    Ok(json!({
        "version": d.version,
        "pages": pages.len(),
        "file_bytes": bytes.len(),
        "encrypted": d.was_encrypted(),
        "xref_recovered": d.xref_recovery.is_some(),
        "objects": d.objects.len(),
        "info": standard,
        "custom_info": custom,
        "has_xmp_metadata": has_xmp,
        "xmp_bytes": xmp_len,
        "has_forms": has_forms,
        "has_outlines": has_outlines,
        "page_layout": page_layout,
        "page_mode": page_mode,
        "language": lang,
        "first_page": first_box.map(|b| json!({ "width_pt": b.width(), "height_pt": b.height(), "size": doc::match_page_size(b.width(), b.height()).map(|(n, o)| format!("{n} {o}")) })),
    }))
}

/// Metadata fields to change; `None` leaves a field untouched and `Some("")` removes it.
#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct MetadataEdit {
    /// `/Title`.
    pub title: Option<String>,
    /// `/Author`.
    pub author: Option<String>,
    /// `/Subject`.
    pub subject: Option<String>,
    /// `/Keywords`.
    pub keywords: Option<String>,
    /// `/Creator`.
    pub creator: Option<String>,
    /// `/Producer`.
    pub producer: Option<String>,
    /// ISO-8601 (`2024-01-31T10:00:00Z`) or PDF (`D:20240131100000Z`) date.
    pub creation_date: Option<String>,
    /// Modification date (same formats as `creation_date`).
    pub modification_date: Option<String>,
    /// Additional custom keys.
    pub custom: std::collections::BTreeMap<String, String>,
    /// Keys to remove (standard or custom names, e.g. `Producer`).
    pub remove: Vec<String>,
    /// Remove the whole Info dictionary and XMP metadata before applying edits.
    pub clear_all: bool,
}

fn to_pdf_date(s: &str) -> EngineResult<String> {
    let t = s.trim();
    if t.starts_with("D:") {
        return Ok(t.to_string());
    }
    // Accept YYYY-MM-DD[THH:MM[:SS]][Z|±HH:MM]
    let digits: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 8 {
        return Err(EngineError::invalid(format!(
            "Invalid date `{s}` (use ISO-8601 like 2024-01-31T10:00:00Z)"
        )));
    }
    let main: String = digits.chars().take(14).collect();
    let mut out = format!("D:{main:0<14}");
    if let Some(idx) = t.rfind(['+', '-']).filter(|&i| i > 10) {
        let tz = &t[idx..];
        let tzd: String = tz.chars().filter(|c| c.is_ascii_digit()).collect();
        if tzd.len() >= 4 {
            out.push_str(&format!("{}{}'{}'", &tz[..1], &tzd[..2], &tzd[2..4]));
        }
    } else if t.ends_with('Z') {
        out.push('Z');
    }
    Ok(out)
}

/// Edits the Info dictionary.
pub fn set_metadata(
    bytes: &[u8],
    password: Option<&str>,
    edit: &MetadataEdit,
) -> EngineResult<Value> {
    let mut d = doc::load(bytes, password)?;
    let mut info = if edit.clear_all {
        Dictionary::new()
    } else {
        info_dict(&d).unwrap_or_default()
    };
    if edit.clear_all {
        if let Ok(cat) = d.catalog_mut() {
            cat.remove(b"Metadata");
        }
    }
    for key in &edit.remove {
        info.remove(key.as_bytes());
    }
    let mut set = |k: &str, v: &Option<String>| {
        if let Some(v) = v {
            if v.is_empty() {
                info.remove(k.as_bytes());
            } else {
                info.set(k, doc::encode_text_string(v));
            }
        }
    };
    set("Title", &edit.title);
    set("Author", &edit.author);
    set("Subject", &edit.subject);
    set("Keywords", &edit.keywords);
    set("Creator", &edit.creator);
    set("Producer", &edit.producer);
    if let Some(c) = &edit.creation_date {
        info.set("CreationDate", Object::string_literal(to_pdf_date(c)?));
    }
    if let Some(m) = &edit.modification_date {
        info.set("ModDate", Object::string_literal(to_pdf_date(m)?));
    }
    for (k, v) in &edit.custom {
        if k.trim().is_empty()
            || k.contains(|c: char| c.is_whitespace() || "/()<>[]{}%".contains(c))
        {
            return Err(EngineError::invalid(format!("Invalid custom metadata key `{k}`")));
        }
        info.set(k.as_str(), doc::encode_text_string(v));
    }
    let id = d.add_object(Object::Dictionary(info));
    d.trailer.set("Info", Object::Reference(id));
    let total = d.get_pages().len();
    let out = doc::save(&mut d)?;
    let report = metadata(&out, None)?;
    Ok(json!({
        "output": PdfOutput::new(out, total),
        "info": report["info"],
        "custom_info": report["custom_info"]
    }))
}

/// Reports every page's boxes, size, orientation and rotation.
pub fn page_sizes(bytes: &[u8], password: Option<&str>) -> EngineResult<Value> {
    let d = doc::load(bytes, password)?;
    let mut pages = Vec::new();
    let mut sizes: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for (n, pid) in d.get_pages() {
        let mb = doc::media_box(&d, pid);
        let cb = doc::crop_box(&d, pid);
        let rot = doc::rotation(&d, pid);
        let (w, h) = if rot == 90 || rot == 270 {
            (cb.height(), cb.width())
        } else {
            (cb.width(), cb.height())
        };
        let named = doc::match_page_size(w, h);
        let label = named
            .as_ref()
            .map(|(n, o)| format!("{n} ({o})"))
            .unwrap_or_else(|| format!("{:.0} x {:.0} pt", w, h));
        *sizes.entry(label).or_insert(0) += 1;
        pages.push(json!({
            "page": n,
            "media_box": mb,
            "crop_box": cb,
            "rotation": rot,
            "width_pt": w,
            "height_pt": h,
            "width_mm": w / doc::PT_PER_MM,
            "height_mm": h / doc::PT_PER_MM,
            "width_in": w / doc::PT_PER_INCH,
            "height_in": h / doc::PT_PER_INCH,
            "orientation": if w > h { "landscape" } else if h > w { "portrait" } else { "square" },
            "named_size": named.map(|(n, _)| n),
        }));
    }
    let uniform = sizes.len() <= 1;
    Ok(json!({ "pages": pages, "count": pages.len(), "uniform": uniform, "size_summary": sizes }))
}

#[allow(clippy::only_used_in_recursion)]
fn outline_to_json(
    d: &Document,
    items: &[Outline],
    page_index: &std::collections::HashMap<ObjectId, u32>,
    depth: usize,
) -> Vec<Value> {
    let mut out = Vec::new();
    for item in items {
        match item {
            Outline::Destination(dest) => {
                let title = dest.title().ok().and_then(text_string).unwrap_or_default();
                let page = dest.page().ok().and_then(|p| match p {
                    Object::Reference(r) => page_index.get(r).copied(),
                    Object::Integer(i) => Some(*i as u32 + 1),
                    _ => None,
                });
                out.push(json!({ "title": title, "page": page, "children": [] }));
            },
            Outline::SubOutlines(children) => {
                if depth > 32 {
                    continue;
                }
                let kids = outline_to_json(d, children, page_index, depth + 1);
                if let Some(Value::Object(last)) = out.last_mut() {
                    last.insert("children".into(), Value::Array(kids));
                } else {
                    out.extend(kids);
                }
            },
        }
    }
    out
}

/// Extracts the outline (bookmarks) tree.
pub fn outlines(bytes: &[u8], password: Option<&str>) -> EngineResult<Value> {
    let d = doc::load(bytes, password)?;
    let page_index: std::collections::HashMap<ObjectId, u32> =
        d.get_pages().into_iter().map(|(n, id)| (id, n)).collect();
    let mut named = indexmap::IndexMap::new();
    let items = match d.get_outlines(None, None, &mut named) {
        Ok(Some(items)) => outline_to_json(&d, &items, &page_index, 0),
        Ok(None) => vec![],
        Err(lopdf::Error::NoOutline) => vec![],
        Err(lopdf::Error::DictKey(_)) => vec![],
        Err(e) => return Err(doc::map_pdf_error(e)),
    };
    fn count(v: &[Value]) -> usize {
        v.iter()
            .map(|i| 1 + count(i["children"].as_array().map(|a| a.as_slice()).unwrap_or(&[])))
            .sum()
    }
    let total = count(&items);
    Ok(json!({ "outlines": items, "count": total, "has_outlines": total > 0 }))
}

/// One outline entry (recursive).
#[derive(Deserialize, Debug, Clone)]
pub struct BookmarkSpec {
    /// Display text.
    pub title: String,
    /// 1-based target page.
    pub page: u32,
    /// Nested entries.
    #[serde(default)]
    pub children: Vec<BookmarkSpec>,
    /// Render bold.
    #[serde(default)]
    pub bold: bool,
    /// Render italic.
    #[serde(default)]
    pub italic: bool,
}

/// Replaces the document outline with the given bookmark tree.
pub fn set_bookmarks(
    bytes: &[u8],
    password: Option<&str>,
    bookmarks: &[BookmarkSpec],
) -> EngineResult<Value> {
    let mut d = doc::load(bytes, password)?;
    let pages = d.get_pages();
    let total = pages.len();
    let _ = d.delete_outlines();
    d.bookmark_table.clear();
    d.bookmarks.clear();

    fn add(
        d: &mut Document,
        pages: &std::collections::BTreeMap<u32, ObjectId>,
        items: &[BookmarkSpec],
        parent: Option<u32>,
        depth: usize,
        count: &mut usize,
    ) -> EngineResult<()> {
        if depth > 32 {
            return Err(EngineError::invalid("Bookmark tree is nested too deeply"));
        }
        for b in items {
            if b.title.trim().is_empty() {
                return Err(EngineError::invalid("Bookmark titles must not be empty"));
            }
            let pid = *pages.get(&b.page).ok_or_else(|| {
                EngineError::not_found(format!(
                    "Bookmark `{}`: page {} does not exist",
                    b.title, b.page
                ))
            })?;
            let format = (b.italic as u32) | ((b.bold as u32) << 1);
            let id = d
                .add_bookmark(Bookmark::new(b.title.clone(), [0.0, 0.0, 0.0], format, pid), parent);
            *count += 1;
            add(d, pages, &b.children, Some(id), depth + 1, count)?;
        }
        Ok(())
    }
    let mut count = 0;
    add(&mut d, &pages, bookmarks, None, 0, &mut count)?;
    if let Some(outline_id) = d.build_outline() {
        let catalog = d.catalog_mut().map_err(doc::map_pdf_error)?;
        catalog.set("Outlines", Object::Reference(outline_id));
        catalog.set("PageMode", Object::Name(b"UseOutlines".to_vec()));
    } else if let Ok(catalog) = d.catalog_mut() {
        catalog.remove(b"Outlines");
    }
    let out = doc::save(&mut d)?;
    Ok(json!({ "output": PdfOutput::new(out, total), "bookmarks": count }))
}

fn field_full_name(d: &Document, dict: &Dictionary) -> String {
    let mut parts = Vec::new();
    let mut current = Some(dict.clone());
    let mut guard = 0;
    while let Some(c) = current {
        guard += 1;
        if guard > 32 {
            break;
        }
        if let Some(t) = c
            .get(b"T")
            .ok()
            .and_then(|t| d.dereference(t).ok())
            .and_then(|(_, t)| text_string(t))
        {
            parts.push(t);
        }
        current = c
            .get(b"Parent")
            .ok()
            .and_then(|p| d.dereference(p).ok())
            .and_then(|(_, p)| p.as_dict().ok().cloned());
    }
    parts.reverse();
    parts.join(".")
}

fn inherited_field_attr<'a>(d: &'a Document, dict: &'a Dictionary, key: &[u8]) -> Option<Object> {
    let mut current = Some(dict.clone());
    let mut guard = 0;
    while let Some(c) = current {
        guard += 1;
        if guard > 32 {
            break;
        }
        if let Some(v) = c
            .get(key)
            .ok()
            .and_then(|v| d.dereference(v).ok())
            .map(|(_, v)| v.clone())
        {
            return Some(v);
        }
        current = c
            .get(b"Parent")
            .ok()
            .and_then(|p| d.dereference(p).ok())
            .and_then(|(_, p)| p.as_dict().ok().cloned());
    }
    None
}

/// Collects all terminal form fields (widgets) with their page numbers.
fn collect_fields(d: &Document) -> Vec<(ObjectId, Dictionary, Option<u32>)> {
    let page_index: std::collections::HashMap<ObjectId, u32> =
        d.get_pages().into_iter().map(|(n, id)| (id, n)).collect();
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Walk the AcroForm /Fields tree.
    fn walk(
        d: &Document,
        id: ObjectId,
        out: &mut Vec<(ObjectId, Dictionary, Option<u32>)>,
        seen: &mut std::collections::HashSet<ObjectId>,
        page_index: &std::collections::HashMap<ObjectId, u32>,
        depth: usize,
    ) {
        if depth > 32 || !seen.insert(id) {
            return;
        }
        let dict = match d.get_dictionary(id) {
            Ok(x) => x.clone(),
            Err(_) => return,
        };
        let kids: Vec<ObjectId> = dict
            .get(b"Kids")
            .ok()
            .and_then(|k| d.dereference(k).ok())
            .and_then(|(_, k)| k.as_array().ok())
            .map(|a| a.iter().filter_map(|o| o.as_reference().ok()).collect())
            .unwrap_or_default();
        let kids_are_fields = kids
            .iter()
            .any(|k| d.get_dictionary(*k).map(|kd| kd.has(b"T")).unwrap_or(false));
        if kids.is_empty() || !kids_are_fields {
            let page = dict
                .get(b"P")
                .ok()
                .and_then(|p| p.as_reference().ok())
                .and_then(|p| page_index.get(&p).copied())
                .or_else(|| {
                    kids.first()
                        .and_then(|k| d.get_dictionary(*k).ok())
                        .and_then(|kd| kd.get(b"P").ok())
                        .and_then(|p| p.as_reference().ok())
                        .and_then(|p| page_index.get(&p).copied())
                });
            out.push((id, dict, page));
        } else {
            for k in kids {
                walk(d, k, out, seen, page_index, depth + 1);
            }
        }
    }

    if let Some(fields) = d
        .catalog()
        .ok()
        .and_then(|c| c.get(b"AcroForm").ok())
        .and_then(|a| d.dereference(a).ok())
        .and_then(|(_, a)| a.as_dict().ok())
        .and_then(|a| a.get(b"Fields").ok())
        .and_then(|f| d.dereference(f).ok())
        .and_then(|(_, f)| f.as_array().ok())
        .cloned()
    {
        for f in fields.iter().filter_map(|o| o.as_reference().ok()) {
            walk(d, f, &mut out, &mut seen, &page_index, 0);
        }
    }
    // Also pick up widget annotations not linked from AcroForm.
    for (n, pid) in d.get_pages() {
        if let Ok(annots) = d.get_page_annotations(pid) {
            for a in annots {
                if a.get(b"Subtype").ok().and_then(|s| s.as_name().ok()) == Some(b"Widget")
                    && (a.has(b"T") || a.has(b"Parent"))
                {
                    // Find its object id through the page's /Annots array.
                    if let Some(arr) = d
                        .get_dictionary(pid)
                        .ok()
                        .and_then(|p| p.get(b"Annots").ok())
                        .and_then(|x| d.dereference(x).ok())
                        .and_then(|(_, x)| x.as_array().ok())
                    {
                        for r in arr.iter().filter_map(|o| o.as_reference().ok()) {
                            if d.get_dictionary(r).map(|x| x == a).unwrap_or(false)
                                && seen.insert(r)
                            {
                                out.push((r, a.clone(), Some(n)));
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

/// Lists form fields with type, value, options and flags.
pub fn form_fields(bytes: &[u8], password: Option<&str>) -> EngineResult<Value> {
    let d = doc::load(bytes, password)?;
    let fields = collect_fields(&d);
    let need_appearances = d
        .catalog()
        .ok()
        .and_then(|c| c.get(b"AcroForm").ok())
        .and_then(|a| d.dereference(a).ok())
        .and_then(|(_, a)| a.as_dict().ok())
        .and_then(|a| a.get(b"NeedAppearances").ok())
        .and_then(|n| n.as_bool().ok())
        .unwrap_or(false);
    let mut list = Vec::new();
    for (id, dict, page) in &fields {
        let ft = inherited_field_attr(&d, dict, b"FT")
            .and_then(|o| o.as_name().ok().map(|n| n.to_vec()));
        let flags = inherited_field_attr(&d, dict, b"Ff")
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0);
        let kind = match ft.as_deref() {
            Some(b"Tx") => "text",
            Some(b"Btn") => {
                if flags & (1 << 16) != 0 {
                    "button"
                } else if flags & (1 << 15) != 0 {
                    "radio"
                } else {
                    "checkbox"
                }
            },
            Some(b"Ch") => {
                if flags & (1 << 17) != 0 {
                    "combobox"
                } else {
                    "listbox"
                }
            },
            Some(b"Sig") => "signature",
            _ => "unknown",
        };
        let value = inherited_field_attr(&d, dict, b"V").map(|v| value_to_json(&d, &v, 0));
        let default_value = inherited_field_attr(&d, dict, b"DV").map(|v| value_to_json(&d, &v, 0));
        let options = inherited_field_attr(&d, dict, b"Opt").map(|v| value_to_json(&d, &v, 0));
        let rect = dict.get(b"Rect").ok().and_then(|r| Rect::from_object(&d, r));
        let max_len = inherited_field_attr(&d, dict, b"MaxLen").and_then(|o| o.as_i64().ok());
        let export_values: Vec<String> = dict
            .get(b"AP")
            .ok()
            .and_then(|ap| d.dereference(ap).ok())
            .and_then(|(_, ap)| ap.as_dict().ok())
            .and_then(|ap| ap.get(b"N").ok())
            .and_then(|n| d.dereference(n).ok())
            .and_then(|(_, n)| n.as_dict().ok())
            .map(|n| {
                n.iter()
                    .map(|(k, _)| String::from_utf8_lossy(k).into_owned())
                    .filter(|k| k != "Off")
                    .collect()
            })
            .unwrap_or_default();
        list.push(json!({
            "name": field_full_name(&d, dict),
            "type": kind,
            "page": page,
            "value": value,
            "default_value": default_value,
            "options": options,
            "rect": rect,
            "read_only": flags & 1 != 0,
            "required": flags & 2 != 0,
            "multiline": kind == "text" && flags & (1 << 12) != 0,
            "password": kind == "text" && flags & (1 << 13) != 0,
            "max_length": max_len,
            "export_values": export_values,
            "object": format!("{} {}", id.0, id.1),
        }));
    }
    Ok(json!({ "fields": list, "count": list.len(), "need_appearances": need_appearances }))
}

/// Fills form fields by (full) name. Checkboxes accept `true`/`false`/export
/// value; choices accept the option text. Sets `NeedAppearances` so viewers
/// regenerate appearances. With `flatten`, widgets that have appearance
/// streams are drawn into the page and removed from the form.
pub fn fill_form(
    bytes: &[u8],
    password: Option<&str>,
    values: &Map<String, Value>,
    flatten: bool,
) -> EngineResult<Value> {
    if values.is_empty() && !flatten {
        return Err(EngineError::empty("field values"));
    }
    let mut d = doc::load(bytes, password)?;
    let fields = collect_fields(&d);
    let mut filled = Vec::new();
    let mut missing: Vec<String> = values.keys().cloned().collect();

    for (id, dict, _) in &fields {
        let name = field_full_name(&d, dict);
        let short = dict
            .get(b"T")
            .ok()
            .and_then(|t| d.dereference(t).ok())
            .and_then(|(_, t)| text_string(t))
            .unwrap_or_default();
        let value = values.get(&name).or_else(|| values.get(&short));
        let Some(value) = value else { continue };
        missing.retain(|m| m != &name && m != &short);
        let ft = inherited_field_attr(&d, dict, b"FT")
            .and_then(|o| o.as_name().ok().map(|n| n.to_vec()));
        let flags = inherited_field_attr(&d, dict, b"Ff")
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0);
        if flags & 1 != 0 {
            return Err(EngineError::invalid(format!("Field `{name}` is read-only")));
        }
        let export_values: Vec<String> = dict
            .get(b"AP")
            .ok()
            .and_then(|ap| d.dereference(ap).ok())
            .and_then(|(_, ap)| ap.as_dict().ok())
            .and_then(|ap| ap.get(b"N").ok())
            .and_then(|n| d.dereference(n).ok())
            .and_then(|(_, n)| n.as_dict().ok())
            .map(|n| {
                n.iter()
                    .map(|(k, _)| String::from_utf8_lossy(k).into_owned())
                    .filter(|k| k != "Off")
                    .collect()
            })
            .unwrap_or_default();
        let (v_obj, as_obj): (Object, Option<Object>) = match ft.as_deref() {
            Some(b"Btn") if flags & (1 << 16) == 0 => {
                let on = export_values.first().cloned().unwrap_or_else(|| "Yes".into());
                let state = match value {
                    Value::Bool(b) => {
                        if *b {
                            on.clone()
                        } else {
                            "Off".into()
                        }
                    },
                    Value::String(s) => {
                        let s = s.trim();
                        if s.eq_ignore_ascii_case("off")
                            || s.eq_ignore_ascii_case("false")
                            || s.is_empty()
                        {
                            "Off".into()
                        } else if s.eq_ignore_ascii_case("true")
                            || s.eq_ignore_ascii_case("yes")
                            || s.eq_ignore_ascii_case("on")
                        {
                            on.clone()
                        } else {
                            s.to_string()
                        }
                    },
                    other => other.to_string(),
                };
                (Object::Name(state.clone().into_bytes()), Some(Object::Name(state.into_bytes())))
            },
            Some(b"Sig") => {
                return Err(EngineError::invalid(format!(
                    "Field `{name}` is a signature field and cannot be filled"
                )))
            },
            _ => {
                let s = match value {
                    Value::String(s) => s.clone(),
                    Value::Null => String::new(),
                    Value::Array(a) => {
                        a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join("\n")
                    },
                    other => other.to_string(),
                };
                let max_len =
                    inherited_field_attr(&d, dict, b"MaxLen").and_then(|o| o.as_i64().ok());
                if let Some(m) = max_len {
                    if s.chars().count() as i64 > m {
                        return Err(EngineError::invalid(format!(
                            "Value for `{name}` exceeds max length {m}"
                        )));
                    }
                }
                (doc::encode_text_string(&s), None)
            },
        };
        // Set on the terminal field; if the value lives on a parent (radio
        // groups), also set it there.
        let target = d.get_dictionary_mut(*id).map_err(doc::map_pdf_error)?;
        target.set("V", v_obj.clone());
        if let Some(a) = &as_obj {
            target.set("AS", a.clone());
        }
        if let Some(parent) = dict.get(b"Parent").ok().and_then(|p| p.as_reference().ok()) {
            if ft.as_deref() == Some(b"Btn") && flags & (1 << 15) != 0 {
                if let Ok(pd) = d.get_dictionary_mut(parent) {
                    pd.set("V", v_obj.clone());
                }
            }
        }
        // Drop stale appearance for text fields so viewers regenerate.
        if ft.as_deref() == Some(b"Tx") || ft.as_deref() == Some(b"Ch") {
            if let Ok(t) = d.get_dictionary_mut(*id) {
                t.remove(b"AP");
            }
        }
        filled.push(name);
    }
    if !missing.is_empty() {
        return Err(EngineError::not_found(format!(
            "Unknown form field(s): {}",
            missing.join(", ")
        )));
    }

    // NeedAppearances = true
    let acro_id = d
        .catalog()
        .ok()
        .and_then(|c| c.get(b"AcroForm").ok())
        .and_then(|a| a.as_reference().ok());
    match acro_id {
        Some(aid) => {
            if let Ok(a) = d.get_dictionary_mut(aid) {
                a.set("NeedAppearances", Object::Boolean(true));
            }
        },
        None => {
            if let Ok(cat) = d.catalog_mut() {
                if let Ok(Object::Dictionary(a)) = cat.get_mut(b"AcroForm") {
                    a.set("NeedAppearances", Object::Boolean(true));
                }
            }
        },
    }

    let mut flattened = 0usize;
    let mut not_flattened = 0usize;
    if flatten {
        let page_map = d.get_pages();
        for (_, pid) in page_map {
            let annots: Vec<ObjectId> = d
                .get_dictionary(pid)
                .ok()
                .and_then(|p| p.get(b"Annots").ok())
                .and_then(|a| d.dereference(a).ok())
                .and_then(|(_, a)| a.as_array().ok())
                .map(|a| a.iter().filter_map(|o| o.as_reference().ok()).collect())
                .unwrap_or_default();
            let mut keep = Vec::new();
            let mut content = String::new();
            for (i, aid) in annots.iter().enumerate() {
                let a = match d.get_dictionary(*aid) {
                    Ok(a) => a.clone(),
                    Err(_) => continue,
                };
                if a.get(b"Subtype").ok().and_then(|s| s.as_name().ok()) != Some(b"Widget") {
                    keep.push(Object::Reference(*aid));
                    continue;
                }
                let rect = a.get(b"Rect").ok().and_then(|r| Rect::from_object(&d, r));
                let ap_n = a
                    .get(b"AP")
                    .ok()
                    .and_then(|ap| d.dereference(ap).ok())
                    .and_then(|(_, ap)| ap.as_dict().ok().cloned())
                    .and_then(|ap| ap.get(b"N").ok().cloned());
                let stream_ref: Option<ObjectId> = match ap_n {
                    Some(Object::Reference(r)) => match d.get_object(r) {
                        Ok(Object::Stream(_)) => Some(r),
                        Ok(Object::Dictionary(states)) => {
                            let state = a
                                .get(b"AS")
                                .ok()
                                .and_then(|s| s.as_name().ok().map(|n| n.to_vec()));
                            state
                                .and_then(|s| states.get(&s).ok())
                                .and_then(|o| o.as_reference().ok())
                        },
                        _ => None,
                    },
                    Some(Object::Dictionary(states)) => {
                        let state =
                            a.get(b"AS").ok().and_then(|s| s.as_name().ok().map(|n| n.to_vec()));
                        state
                            .and_then(|s| states.get(&s).ok())
                            .and_then(|o| o.as_reference().ok())
                    },
                    _ => None,
                };
                match (rect, stream_ref) {
                    (Some(r), Some(sid)) => {
                        let bbox = d.get_object(sid).ok().and_then(|o| match o {
                            Object::Stream(s) => {
                                s.dict.get(b"BBox").ok().and_then(|b| Rect::from_object(&d, b))
                            },
                            _ => None,
                        });
                        let (sx, sy) = bbox
                            .map(|b| {
                                (
                                    (r.width() / b.width().max(0.001)),
                                    (r.height() / b.height().max(0.001)),
                                )
                            })
                            .unwrap_or((1.0, 1.0));
                        let (bx, by) = bbox.map(|b| (b.x0, b.y0)).unwrap_or((0.0, 0.0));
                        let name = format!("DTBXW{i}");
                        content.push_str(&format!(
                            "q {sx:.4} 0 0 {sy:.4} {:.3} {:.3} cm /{name} Do Q\n",
                            r.x0 - bx * sx,
                            r.y0 - by * sy
                        ));
                        crate::overlay::add_page_resource(
                            &mut d,
                            pid,
                            "XObject",
                            &name,
                            Object::Reference(sid),
                        )?;
                        flattened += 1;
                    },
                    _ => {
                        not_flattened += 1;
                    },
                }
            }
            if !content.is_empty() {
                crate::overlay::append_content(&mut d, pid, content)?;
            }
            if let Ok(p) = d.get_dictionary_mut(pid) {
                if keep.is_empty() {
                    p.remove(b"Annots");
                } else {
                    p.set("Annots", Object::Array(keep));
                }
            }
        }
        if let Ok(cat) = d.catalog_mut() {
            cat.remove(b"AcroForm");
        }
    }
    let total = d.get_pages().len();
    let out = doc::save(&mut d)?;
    let mut warnings = Vec::new();
    if not_flattened > 0 {
        warnings.push(format!(
            "{not_flattened} widget(s) had no appearance stream and were removed without drawing \
             (values you just set on text fields are not rendered when flattening in the same \
             call)"
        ));
    }
    Ok(json!({
        "output": PdfOutput::new(out, total),
        "filled": filled,
        "flattened": flattened,
        "warnings": warnings
    }))
}

/// Inspects digital signature fields (no cryptographic verification).
pub fn signatures(bytes: &[u8], password: Option<&str>) -> EngineResult<Value> {
    let d = doc::load(bytes, password)?;
    let fields = collect_fields(&d);
    let mut sigs = Vec::new();
    for (_, dict, page) in &fields {
        let ft = inherited_field_attr(&d, dict, b"FT")
            .and_then(|o| o.as_name().ok().map(|n| n.to_vec()));
        if ft.as_deref() != Some(b"Sig") {
            continue;
        }
        let name = field_full_name(&d, dict);
        let v = inherited_field_attr(&d, dict, b"V").and_then(|v| v.as_dict().ok().cloned());
        match v {
            None => sigs.push(json!({ "field": name, "page": page, "signed": false })),
            Some(sig) => {
                let get = |k: &[u8]| {
                    sig.get(k)
                        .ok()
                        .and_then(|o| d.dereference(o).ok())
                        .and_then(|(_, o)| text_string(o))
                };
                let byte_range: Vec<i64> = sig
                    .get(b"ByteRange")
                    .ok()
                    .and_then(|b| b.as_array().ok())
                    .map(|a| a.iter().filter_map(|o| o.as_i64().ok()).collect())
                    .unwrap_or_default();
                let contents_len = sig
                    .get(b"Contents")
                    .ok()
                    .and_then(|c| c.as_str().ok())
                    .map(|c| c.len())
                    .unwrap_or(0);
                let covers_whole = byte_range.len() == 4
                    && (byte_range[2] + byte_range[3]) as usize == bytes.len();
                let subfilter = sig
                    .get(b"SubFilter")
                    .ok()
                    .and_then(|s| s.as_name().ok())
                    .map(|n| String::from_utf8_lossy(n).into_owned());
                let m = get(b"M");
                sigs.push(json!({
                    "field": name,
                    "page": page,
                    "signed": true,
                    "filter": sig.get(b"Filter").ok().and_then(|s| s.as_name().ok()).map(|n| String::from_utf8_lossy(n).into_owned()),
                    "sub_filter": subfilter,
                    "signer_name": get(b"Name"),
                    "reason": get(b"Reason"),
                    "location": get(b"Location"),
                    "contact_info": get(b"ContactInfo"),
                    "signing_time": m.as_deref().and_then(pdf_date_to_iso),
                    "signing_time_raw": m,
                    "byte_range": byte_range,
                    "covers_whole_document": covers_whole,
                    "signature_bytes": contents_len,
                }));
            },
        }
    }
    let doc_mdp = d.catalog().ok().and_then(|c| c.get(b"Perms").ok()).is_some();
    Ok(json!({
        "signatures": sigs,
        "count": sigs.len(),
        "signed": sigs.iter().any(|s| s["signed"] == true),
        "has_docmdp_permissions": doc_mdp,
        "note": "Signature presence and metadata only; cryptographic validation must be done with \
                 a trusted validator.",
    }))
}

/// Builds a one-line summary used by the legacy `inspect` action.
pub fn quick_inspect(bytes: &[u8]) -> Value {
    let valid = doc::looks_like_pdf(bytes);
    let header = String::from_utf8_lossy(&bytes[..bytes.len().min(16)]).into_owned();
    let version = header
        .find("%PDF-")
        .map(|i| header[i + 5..].chars().take(3).collect::<String>())
        .unwrap_or_else(|| "unknown".into());
    let (pages, encrypted, error) = match Document::load_mem(bytes) {
        Ok(d) => (d.get_pages().len(), d.is_encrypted(), Value::Null),
        Err(e) => (0, bytes.windows(8).any(|w| w == b"/Encrypt"), json!(e.to_string())),
    };
    json!({
        "valid": valid && error.is_null(),
        "version": version,
        "estimated_pages": pages,
        "pages": pages,
        "encryption_detected": encrypted,
        "error": error
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{form_pdf, sample_pdf};
    use engine_shared::b64;

    fn out_bytes(v: &Value) -> Vec<u8> {
        b64::decode(v["output"]["file"].as_str().unwrap()).unwrap()
    }

    #[test]
    fn metadata_roundtrip() {
        let pdf = sample_pdf(&["x"]);
        let m = metadata(&pdf, None).unwrap();
        assert_eq!(m["pages"], 1);
        let edit = MetadataEdit {
            title: Some("My Title".into()),
            author: Some("Zoë".into()),
            creation_date: Some("2024-01-31T10:00:00Z".into()),
            custom: [("Project".to_string(), "X".to_string())].into(),
            ..Default::default()
        };
        let r = set_metadata(&pdf, None, &edit).unwrap();
        assert_eq!(r["info"]["title"], "My Title");
        assert_eq!(r["info"]["author"], "Zoë");
        assert_eq!(r["info"]["creation_date"], "2024-01-31T10:00:00Z");
        assert_eq!(r["custom_info"]["Project"], "X");
        let bytes = out_bytes(&r);
        let cleared =
            set_metadata(&bytes, None, &MetadataEdit { clear_all: true, ..Default::default() })
                .unwrap();
        assert!(cleared["info"].as_object().unwrap().is_empty());
        assert!(set_metadata(
            &pdf,
            None,
            &MetadataEdit {
                creation_date: Some("bad".into()),
                ..Default::default()
            }
        )
        .is_err());
        assert!(set_metadata(
            &pdf,
            None,
            &MetadataEdit {
                custom: [("bad key".to_string(), "v".to_string())].into(),
                ..Default::default()
            }
        )
        .is_err());
        assert_eq!(to_pdf_date("2024-01-31T10:00:00+05:30").unwrap(), "D:20240131100000+05'30'");
    }

    #[test]
    fn sizes_and_outlines() {
        let pdf = sample_pdf(&["a", "b"]);
        let s = page_sizes(&pdf, None).unwrap();
        assert_eq!(s["count"], 2);
        assert_eq!(s["pages"][0]["named_size"], "A4");
        assert_eq!(s["uniform"], true);

        let o = outlines(&pdf, None).unwrap();
        assert_eq!(o["count"], 0);
        let bm = vec![BookmarkSpec {
            title: "Chapter 1".into(),
            page: 1,
            bold: true,
            italic: false,
            children: vec![BookmarkSpec {
                title: "Section".into(),
                page: 2,
                bold: false,
                italic: false,
                children: vec![],
            }],
        }];
        let r = set_bookmarks(&pdf, None, &bm).unwrap();
        assert_eq!(r["bookmarks"], 2);
        let o = outlines(&out_bytes(&r), None).unwrap();
        assert_eq!(o["count"], 2);
        assert_eq!(o["outlines"][0]["title"], "Chapter 1");
        assert_eq!(o["outlines"][0]["page"], 1);
        assert_eq!(o["outlines"][0]["children"][0]["page"], 2);
        assert!(set_bookmarks(
            &pdf,
            None,
            &[BookmarkSpec {
                title: "x".into(),
                page: 9,
                bold: false,
                italic: false,
                children: vec![]
            }]
        )
        .is_err());
    }

    #[test]
    fn forms() {
        let pdf = form_pdf();
        let f = form_fields(&pdf, None).unwrap();
        assert_eq!(f["count"], 3);
        let names: Vec<&str> = f["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"name") && names.contains(&"agree") && names.contains(&"colour"));
        let mut values = Map::new();
        values.insert("name".into(), json!("Alice"));
        values.insert("agree".into(), json!(true));
        values.insert("colour".into(), json!("Red"));
        let r = fill_form(&pdf, None, &values, false).unwrap();
        let after = form_fields(&out_bytes(&r), None).unwrap();
        let by_name = |n: &str| {
            after["fields"]
                .as_array()
                .unwrap()
                .iter()
                .find(|x| x["name"] == n)
                .unwrap()
                .clone()
        };
        assert_eq!(by_name("name")["value"], "Alice");
        assert_eq!(by_name("agree")["value"], "/Yes");
        assert_eq!(after["need_appearances"], true);
        let mut bad = Map::new();
        bad.insert("nope".into(), json!("x"));
        assert!(fill_form(&pdf, None, &bad, false).is_err());
        assert!(fill_form(&pdf, None, &Map::new(), false).is_err());
        let flat = fill_form(&out_bytes(&r), None, &Map::new(), true).unwrap();
        assert!(flat["flattened"].as_u64().unwrap() >= 1);
        assert_eq!(form_fields(&out_bytes(&flat), None).unwrap()["count"], 0);

        let s = signatures(&pdf, None).unwrap();
        assert_eq!(s["count"], 0);
        assert_eq!(s["signed"], false);
    }

    #[test]
    fn quick() {
        let q = quick_inspect(&sample_pdf(&["a"]));
        assert_eq!(q["valid"], true);
        assert_eq!(q["pages"], 1);
        assert_eq!(quick_inspect(b"nope")["valid"], false);
    }
}
