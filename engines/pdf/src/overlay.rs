//! Drawing on existing pages: page numbers, watermarks, headers/footers,
//! stamps, annotations, page overlays, redaction and grayscale conversion.

use crate::doc::{self, escape_pdf_string, PageSelection, Rect};
use crate::fonts::{encode_winansi, StdFont};
use crate::images;
use crate::pages::PdfOutput;
use engine_shared::{EngineError, EngineResult};
use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use serde::Deserialize;
use serde_json::{json, Value};

/// Visual geometry of a page as displayed (accounts for `/Rotate`).
pub struct Visual {
    /// Transform that maps visual coordinates (origin bottom-left of the
    /// displayed page) to page user space.
    pub cm: String,
    /// Displayed width in points.
    pub width: f64,
    /// Displayed height in points.
    pub height: f64,
}

/// Computes the displayed geometry of a page, accounting for `/Rotate` and the crop box.
pub fn visual_geometry(doc: &Document, page_id: ObjectId) -> Visual {
    let b = doc::crop_box(doc, page_id);
    let (w, h) = (b.width(), b.height());
    match doc::rotation(doc, page_id) {
        90 => Visual {
            cm: format!("0 -1 1 0 {:.3} {:.3} cm", b.x0, b.y1),
            width: h,
            height: w,
        },
        180 => Visual {
            cm: format!("-1 0 0 -1 {:.3} {:.3} cm", b.x1, b.y1),
            width: w,
            height: h,
        },
        270 => Visual {
            cm: format!("0 1 -1 0 {:.3} {:.3} cm", b.x1, b.y0),
            width: h,
            height: w,
        },
        _ => Visual {
            cm: format!("1 0 0 1 {:.3} {:.3} cm", b.x0, b.y0),
            width: w,
            height: h,
        },
    }
}

/// Ensures `/Resources` exists on the page (as a direct dictionary) and
/// registers `obj` under `/<category>/<name>`.
pub fn add_page_resource(
    doc: &mut Document,
    page_id: ObjectId,
    category: &str,
    name: &str,
    obj: Object,
) -> EngineResult<()> {
    // Materialise inherited / referenced resources into a page-owned dict so
    // we never mutate a dictionary shared with other pages.
    let resources: Dictionary = match doc::inherited_attr(doc, page_id, b"Resources") {
        Some(r) => match doc.dereference(r) {
            Ok((_, Object::Dictionary(d))) => d.clone(),
            _ => Dictionary::new(),
        },
        None => Dictionary::new(),
    };
    let mut resources = resources;
    let mut cat: Dictionary = match resources.get(category.as_bytes()) {
        Ok(c) => match doc.dereference(c) {
            Ok((_, Object::Dictionary(d))) => d.clone(),
            _ => Dictionary::new(),
        },
        Err(_) => Dictionary::new(),
    };
    cat.set(name, obj);
    resources.set(category, Object::Dictionary(cat));
    let page = doc.get_dictionary_mut(page_id).map_err(doc::map_pdf_error)?;
    page.set("Resources", Object::Dictionary(resources));
    Ok(())
}

/// Appends a content stream drawn *after* the existing content (on top).
pub fn append_content(doc: &mut Document, page_id: ObjectId, content: String) -> EngineResult<()> {
    let existing = doc.get_page_contents(page_id);
    // Guard the existing content with q/Q so its graphics state does not leak.
    let pre = doc.add_object(Stream::new(Dictionary::new(), b"q\n".to_vec()));
    let post =
        doc.add_object(Stream::new(Dictionary::new(), format!("\nQ\n{content}").into_bytes()));
    let mut arr = vec![Object::Reference(pre)];
    arr.extend(existing.into_iter().map(Object::Reference));
    arr.push(Object::Reference(post));
    let dict = doc.get_dictionary_mut(page_id).map_err(doc::map_pdf_error)?;
    dict.set("Contents", Object::Array(arr));
    Ok(())
}

/// Prepends a content stream drawn *before* the existing content (under).
pub fn prepend_content(doc: &mut Document, page_id: ObjectId, content: String) -> EngineResult<()> {
    let existing = doc.get_page_contents(page_id);
    let pre =
        doc.add_object(Stream::new(Dictionary::new(), format!("{content}\nq\n").into_bytes()));
    let post = doc.add_object(Stream::new(Dictionary::new(), b"\nQ\n".to_vec()));
    let mut arr = vec![Object::Reference(pre)];
    arr.extend(existing.into_iter().map(Object::Reference));
    arr.push(Object::Reference(post));
    let dict = doc.get_dictionary_mut(page_id).map_err(doc::map_pdf_error)?;
    dict.set("Contents", Object::Array(arr));
    Ok(())
}

/// Named placement on a page.
#[derive(Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Position {
    /// Top-left corner.
    TopLeft,
    /// Top edge, centred.
    TopCenter,
    /// Top-right corner.
    TopRight,
    /// Page centre.
    Center,
    /// Bottom-left corner.
    BottomLeft,
    /// Bottom edge, centred.
    #[default]
    BottomCenter,
    /// Bottom-right corner.
    BottomRight,
}

/// Parses `#rrggbb`, `rgb(r,g,b)` or named colours into 0..1 components.
pub fn parse_color(s: &str) -> EngineResult<(f64, f64, f64)> {
    let t = s.trim().to_ascii_lowercase();
    let named = match t.as_str() {
        "black" => Some((0.0, 0.0, 0.0)),
        "white" => Some((1.0, 1.0, 1.0)),
        "red" => Some((1.0, 0.0, 0.0)),
        "green" => Some((0.0, 0.5, 0.0)),
        "blue" => Some((0.0, 0.0, 1.0)),
        "gray" | "grey" => Some((0.5, 0.5, 0.5)),
        "yellow" => Some((1.0, 1.0, 0.0)),
        "orange" => Some((1.0, 0.65, 0.0)),
        _ => None,
    };
    if let Some(c) = named {
        return Ok(c);
    }
    if let Some(hex) = t.strip_prefix('#') {
        let hex = if hex.len() == 3 {
            hex.chars().flat_map(|c| [c, c]).collect::<String>()
        } else {
            hex.to_string()
        };
        if hex.len() == 6 {
            if let Ok(v) = u32::from_str_radix(&hex, 16) {
                return Ok((
                    ((v >> 16) & 255) as f64 / 255.0,
                    ((v >> 8) & 255) as f64 / 255.0,
                    (v & 255) as f64 / 255.0,
                ));
            }
        }
    }
    if let Some(inner) = t.strip_prefix("rgb(").and_then(|r| r.strip_suffix(')')) {
        let parts: Vec<f64> = inner.split(',').filter_map(|p| p.trim().parse().ok()).collect();
        if parts.len() == 3 && parts.iter().all(|v| (0.0..=255.0).contains(v)) {
            return Ok((parts[0] / 255.0, parts[1] / 255.0, parts[2] / 255.0));
        }
    }
    Err(EngineError::invalid(format!(
        "Unrecognised colour `{s}` (use #rrggbb, rgb(r,g,b) or a colour name)"
    )))
}

fn anchor(pos: Position, vis: &Visual, text_w: f64, font_size: f64, margin: f64) -> (f64, f64) {
    let x = match pos {
        Position::TopLeft | Position::BottomLeft => margin,
        Position::TopCenter | Position::BottomCenter | Position::Center => {
            (vis.width - text_w) / 2.0
        },
        Position::TopRight | Position::BottomRight => vis.width - margin - text_w,
    };
    let y = match pos {
        Position::TopLeft | Position::TopCenter | Position::TopRight => {
            vis.height - margin - font_size
        },
        Position::Center => (vis.height - font_size) / 2.0,
        _ => margin,
    };
    (x, y)
}

struct TextStyle {
    font: StdFont,
    size: f64,
    color: (f64, f64, f64),
}

/// Builds the content for one text label in visual coordinates.
fn text_ops(font_key: &str, style: &TextStyle, text: &str, x: f64, y: f64) -> String {
    let (bytes, _) = encode_winansi(text);
    format!(
        "BT /{} {:.2} Tf {:.3} {:.3} {:.3} rg {:.3} {:.3} Td ({}) Tj ET\n",
        font_key,
        style.size,
        style.color.0,
        style.color.1,
        style.color.2,
        x,
        y,
        String::from_utf8_lossy(&escape_pdf_string(&bytes))
    )
}

/// Substitutes `{n}`, `{page}`, `{total}`, `{pages}` placeholders.
fn expand_template(template: &str, n: usize, total: usize) -> String {
    template
        .replace("{n}", &n.to_string())
        .replace("{page}", &n.to_string())
        .replace("{total}", &total.to_string())
        .replace("{pages}", &total.to_string())
}

/// Appearance of page-number and header/footer labels.
#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct LabelOptions {
    /// Standard font name (`helvetica`, `times`, `courier`, optionally `-bold`).
    pub font: String,
    /// Font size in points.
    pub font_size: f64,
    /// Text colour (`#RRGGBB`).
    pub color: String,
    /// Distance from the page edge in points.
    pub margin: f64,
    /// Placement on the page.
    pub position: Position,
}

impl Default for LabelOptions {
    fn default() -> Self {
        LabelOptions {
            font: "helvetica".into(),
            font_size: 10.0,
            color: "#000000".into(),
            margin: 28.0,
            position: Position::BottomCenter,
        }
    }
}

fn style_from(opts: &LabelOptions) -> EngineResult<TextStyle> {
    engine_shared::error::require_range(opts.font_size, 4.0, 200.0, "font_size")?;
    engine_shared::error::require_range(opts.margin, 0.0, 500.0, "margin")?;
    Ok(TextStyle {
        font: StdFont::resolve(&opts.font, false, false),
        size: opts.font_size,
        color: parse_color(&opts.color)?,
    })
}

const FONT_RES: &str = "DTBXFont";
const GS_RES: &str = "DTBXGS";

/// Adds page numbers. `format` supports `{n}` / `{page}` and `{total}`.
pub fn add_page_numbers(
    bytes: &[u8],
    password: Option<&str>,
    format: &str,
    start: i64,
    opts: &LabelOptions,
    pages: &PageSelection,
) -> EngineResult<Value> {
    let mut d = doc::load(bytes, password)?;
    let style = style_from(opts)?;
    let total = d.get_pages().len();
    let selected = pages.resolve(total)?;
    let page_map = d.get_pages();
    let font_id = style.font.add_to_document(&mut d);
    let template = if format.trim().is_empty() { "{n}" } else { format };
    for &n in &selected {
        let pid = page_map[&n];
        let label = expand_template(
            template,
            (n as i64 + start - 1).max(0) as usize,
            (total as i64 + start - 1).max(0) as usize,
        );
        let vis = visual_geometry(&d, pid);
        let w = style.font.str_width(&label, style.size);
        let (x, y) = anchor(opts.position, &vis, w, style.size, opts.margin);
        add_page_resource(&mut d, pid, "Font", FONT_RES, Object::Reference(font_id))?;
        let content = format!("q {}\n{}Q\n", vis.cm, text_ops(FONT_RES, &style, &label, x, y));
        append_content(&mut d, pid, content)?;
    }
    let out = doc::save(&mut d)?;
    Ok(json!({ "output": PdfOutput::new(out, total), "numbered_pages": selected }))
}

/// Adds a header and/or footer line.
pub fn add_header_footer(
    bytes: &[u8],
    password: Option<&str>,
    header: &str,
    footer: &str,
    align: &str,
    opts: &LabelOptions,
    pages: &PageSelection,
) -> EngineResult<Value> {
    if header.trim().is_empty() && footer.trim().is_empty() {
        return Err(EngineError::empty("header/footer text"));
    }
    let mut d = doc::load(bytes, password)?;
    let style = style_from(opts)?;
    let total = d.get_pages().len();
    let selected = pages.resolve(total)?;
    let page_map = d.get_pages();
    let font_id = style.font.add_to_document(&mut d);
    let (hp, fp) = match align {
        "left" => (Position::TopLeft, Position::BottomLeft),
        "right" => (Position::TopRight, Position::BottomRight),
        _ => (Position::TopCenter, Position::BottomCenter),
    };
    for &n in &selected {
        let pid = page_map[&n];
        let vis = visual_geometry(&d, pid);
        let mut content = format!("q {}\n", vis.cm);
        if !header.trim().is_empty() {
            let label = expand_template(header, n as usize, total);
            let w = style.font.str_width(&label, style.size);
            let (x, y) = anchor(hp, &vis, w, style.size, opts.margin);
            content.push_str(&text_ops(FONT_RES, &style, &label, x, y));
        }
        if !footer.trim().is_empty() {
            let label = expand_template(footer, n as usize, total);
            let w = style.font.str_width(&label, style.size);
            let (x, y) = anchor(fp, &vis, w, style.size, opts.margin);
            content.push_str(&text_ops(FONT_RES, &style, &label, x, y));
        }
        content.push_str("Q\n");
        add_page_resource(&mut d, pid, "Font", FONT_RES, Object::Reference(font_id))?;
        append_content(&mut d, pid, content)?;
    }
    let out = doc::save(&mut d)?;
    Ok(json!({ "output": PdfOutput::new(out, total), "pages": selected }))
}

/// Appearance of a text watermark.
#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct WatermarkOptions {
    /// Standard font name.
    pub font: String,
    /// Font size in points.
    pub font_size: f64,
    /// Text colour (`#RRGGBB`).
    pub color: String,
    /// 0..1
    pub opacity: f64,
    /// Degrees, counter-clockwise.
    pub rotation: f64,
    /// Placement on the page.
    pub position: Position,
    /// Draw under the existing content instead of on top.
    pub behind: bool,
    /// Repeat the watermark in a grid across the page.
    pub tiled: bool,
}

impl Default for WatermarkOptions {
    fn default() -> Self {
        WatermarkOptions {
            font: "helvetica".into(),
            font_size: 48.0,
            color: "#ff0000".into(),
            opacity: 0.25,
            rotation: 45.0,
            position: Position::Center,
            behind: false,
            tiled: false,
        }
    }
}

fn ext_gstate(opacity: f64) -> Object {
    let mut gs = Dictionary::new();
    gs.set("Type", Object::Name(b"ExtGState".to_vec()));
    gs.set("CA", Object::Real(opacity as f32));
    gs.set("ca", Object::Real(opacity as f32));
    Object::Dictionary(gs)
}

/// Adds a text watermark to the selected pages.
pub fn add_watermark(
    bytes: &[u8],
    password: Option<&str>,
    text: &str,
    opts: &WatermarkOptions,
    pages: &PageSelection,
) -> EngineResult<Value> {
    engine_shared::error::require_text(text, "watermark text")?;
    engine_shared::error::require_range(opts.opacity, 0.0, 1.0, "opacity")?;
    engine_shared::error::require_range(opts.font_size, 4.0, 400.0, "font_size")?;
    engine_shared::error::require_range(opts.rotation, -360.0, 360.0, "rotation")?;
    let mut d = doc::load(bytes, password)?;
    let total = d.get_pages().len();
    let selected = pages.resolve(total)?;
    let page_map = d.get_pages();
    let font = StdFont::resolve(&opts.font, true, false);
    let font_id = font.add_to_document(&mut d);
    let gs_id = d.add_object(ext_gstate(opts.opacity));
    let color = parse_color(&opts.color)?;
    let style = TextStyle { font, size: opts.font_size, color };
    let text_w = font.str_width(text, opts.font_size);
    let (cos, sin) = (opts.rotation.to_radians().cos(), opts.rotation.to_radians().sin());

    for &n in &selected {
        let pid = page_map[&n];
        let vis = visual_geometry(&d, pid);
        let mut content = format!("q {}\n/{} gs\n", vis.cm, GS_RES);
        let mut anchors = Vec::new();
        if opts.tiled {
            let step_x = (text_w + 80.0).max(120.0);
            let step_y = (opts.font_size * 4.0).max(120.0);
            let mut y = 0.0;
            let mut row = 0;
            while y < vis.height + step_y {
                let mut x = if row % 2 == 0 { 0.0 } else { -step_x / 2.0 };
                while x < vis.width + step_x {
                    anchors.push((x, y));
                    x += step_x;
                }
                y += step_y;
                row += 1;
            }
        } else {
            let (ax, ay) = anchor(opts.position, &vis, text_w, opts.font_size, 36.0);
            // Rotate around the centre of the text box.
            anchors.push((ax + text_w / 2.0, ay + opts.font_size / 2.0));
        }
        for (cx, cy) in anchors {
            let (ox, oy) = if opts.tiled {
                (0.0, 0.0)
            } else {
                (-text_w / 2.0, -opts.font_size / 2.0)
            };
            content.push_str(&format!(
                "q {cos:.5} {sin:.5} {:.5} {cos:.5} {cx:.3} {cy:.3} cm\n",
                -sin
            ));
            content.push_str(&text_ops(FONT_RES, &style, text, ox, oy));
            content.push_str("Q\n");
        }
        content.push_str("Q\n");
        add_page_resource(&mut d, pid, "Font", FONT_RES, Object::Reference(font_id))?;
        add_page_resource(&mut d, pid, "ExtGState", GS_RES, Object::Reference(gs_id))?;
        if opts.behind {
            prepend_content(&mut d, pid, content)?;
        } else {
            append_content(&mut d, pid, content)?;
        }
    }
    let out = doc::save(&mut d)?;
    Ok(json!({ "output": PdfOutput::new(out, total), "pages": selected }))
}

/// Placement and appearance of an image stamp.
#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct StampOptions {
    /// Visual x/y of the stamp's bottom-left corner in points (origin bottom-left).
    pub x: Option<f64>,
    /// Visual y coordinate of the bottom-left corner in points.
    pub y: Option<f64>,
    /// Target width/height in points (height keeps aspect ratio when omitted).
    pub width: Option<f64>,
    /// Target height in points.
    pub height: Option<f64>,
    /// Placement used when `x`/`y` are not given.
    pub position: Position,
    /// Distance from the page edge in points.
    pub margin: f64,
    /// Opacity 0–1.
    pub opacity: f64,
    /// Draw under the existing content.
    pub behind: bool,
}

impl Default for StampOptions {
    fn default() -> Self {
        StampOptions {
            x: None,
            y: None,
            width: None,
            height: None,
            position: Position::BottomRight,
            margin: 24.0,
            opacity: 1.0,
            behind: false,
        }
    }
}

/// Stamps an image (PNG/JPEG/...) onto the selected pages.
pub fn stamp_image(
    bytes: &[u8],
    password: Option<&str>,
    image: &[u8],
    opts: &StampOptions,
    pages: &PageSelection,
) -> EngineResult<Value> {
    engine_shared::error::require_bytes(image, "stamp image")?;
    engine_shared::error::require_range(opts.opacity, 0.0, 1.0, "opacity")?;
    let (xobj, iw, ih) = images::image_xobject(image)?;
    let mut d = doc::load(bytes, password)?;
    let total = d.get_pages().len();
    let selected = pages.resolve(total)?;
    let page_map = d.get_pages();
    let xobj_id = d.add_object(xobj);
    let gs_id = d.add_object(ext_gstate(opts.opacity));
    let aspect = ih as f64 / iw as f64;
    let (w, h) = match (opts.width, opts.height) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => (w, w * aspect),
        (None, Some(h)) => (h / aspect, h),
        (None, None) => ((iw as f64).min(200.0), (iw as f64).min(200.0) * aspect),
    };
    if w <= 0.0 || h <= 0.0 || !w.is_finite() || !h.is_finite() {
        return Err(EngineError::invalid("Stamp width/height must be positive"));
    }
    for &n in &selected {
        let pid = page_map[&n];
        let vis = visual_geometry(&d, pid);
        let (x, y) = match (opts.x, opts.y) {
            (Some(x), Some(y)) => (x, y),
            _ => {
                let (ax, ay) = anchor(opts.position, &vis, w, h, opts.margin);
                (ax, ay)
            },
        };
        let content = format!(
            "q {}\n/{} gs {w:.3} 0 0 {h:.3} {x:.3} {y:.3} cm /DTBXImg Do Q\n",
            vis.cm, GS_RES
        );
        add_page_resource(&mut d, pid, "XObject", "DTBXImg", Object::Reference(xobj_id))?;
        add_page_resource(&mut d, pid, "ExtGState", GS_RES, Object::Reference(gs_id))?;
        if opts.behind {
            prepend_content(&mut d, pid, content)?;
        } else {
            append_content(&mut d, pid, content)?;
        }
    }
    let out = doc::save(&mut d)?;
    Ok(json!({
        "output": PdfOutput::new(out, total),
        "pages": selected,
        "stamp": { "width_pt": w, "height_pt": h }
    }))
}

/// Draws a page of another PDF over (or under) the selected pages, scaled to
/// fit the target page.
pub fn overlay(
    bytes: &[u8],
    password: Option<&str>,
    overlay_bytes: &[u8],
    overlay_password: Option<&str>,
    overlay_page: u32,
    behind: bool,
    opacity: f64,
    pages: &PageSelection,
) -> EngineResult<Value> {
    engine_shared::error::require_range(opacity, 0.0, 1.0, "opacity")?;
    let mut d = doc::load(bytes, password)?;
    let src = doc::load(overlay_bytes, overlay_password)
        .map_err(|e| EngineError::new(e.code, format!("Overlay file: {}", e.message)))?;
    let src_pages = src.get_pages();
    let src_pid = *src_pages.get(&overlay_page).ok_or_else(|| {
        EngineError::not_found(format!("Overlay page {overlay_page} does not exist"))
    })?;

    // Import the overlay page then turn it into a Form XObject.
    let imported = doc::import_page(&mut d, &src, src_pid)?;
    let content = d.get_page_content(imported);
    let bbox = doc::media_box(&d, imported);
    let resources = d
        .get_dictionary(imported)
        .ok()
        .and_then(|p| p.get(b"Resources").ok().cloned())
        .unwrap_or(Object::Dictionary(Dictionary::new()));
    let mut form_dict = Dictionary::new();
    form_dict.set("Type", Object::Name(b"XObject".to_vec()));
    form_dict.set("Subtype", Object::Name(b"Form".to_vec()));
    form_dict.set("BBox", bbox.to_object());
    form_dict.set("Resources", resources);
    let mut form = Stream::new(form_dict, content);
    let _ = form.compress();
    let form_id = d.add_object(Object::Stream(form));
    d.objects.remove(&imported);
    let gs_id = d.add_object(ext_gstate(opacity));

    let total = d.get_pages().len();
    let selected = pages.resolve(total)?;
    let page_map = d.get_pages();
    for &n in &selected {
        let pid = page_map[&n];
        let target = doc::media_box(&d, pid);
        let s = (target.width() / bbox.width()).min(target.height() / bbox.height());
        let tx = target.x0 + (target.width() - bbox.width() * s) / 2.0 - bbox.x0 * s;
        let ty = target.y0 + (target.height() - bbox.height() * s) / 2.0 - bbox.y0 * s;
        let content =
            format!("q /{} gs {s:.5} 0 0 {s:.5} {tx:.3} {ty:.3} cm /DTBXOverlay Do Q\n", GS_RES);
        add_page_resource(&mut d, pid, "XObject", "DTBXOverlay", Object::Reference(form_id))?;
        add_page_resource(&mut d, pid, "ExtGState", GS_RES, Object::Reference(gs_id))?;
        if behind {
            prepend_content(&mut d, pid, content)?;
        } else {
            append_content(&mut d, pid, content)?;
        }
    }
    let out = doc::save(&mut d)?;
    Ok(json!({ "output": PdfOutput::new(out, total), "pages": selected }))
}

/// One annotation to add.
#[derive(Deserialize, Debug, Clone)]
pub struct Annotation {
    /// 1-based page number.
    pub page: u32,
    /// `square`, `highlight`, `note`, `text` (free text) or `link`.
    pub kind: String,
    /// `[x0, y0, x1, y1]` in page points (origin bottom-left).
    pub rect: [f64; 4],
    /// Stroke/fill colour (`#RRGGBB`).
    #[serde(default)]
    pub color: Option<String>,
    /// Note text / free-text content.
    #[serde(default)]
    pub contents: Option<String>,
    /// Target URL for `link` annotations.
    #[serde(default)]
    pub url: Option<String>,
    /// Opacity 0–1.
    #[serde(default)]
    pub opacity: Option<f64>,
    /// Font size for free-text annotations.
    #[serde(default)]
    pub font_size: Option<f64>,
}

/// Adds annotations (highlights, boxes, notes, free text, links).
pub fn add_annotations(
    bytes: &[u8],
    password: Option<&str>,
    annotations: &[Annotation],
) -> EngineResult<Value> {
    if annotations.is_empty() {
        return Err(EngineError::empty("annotations"));
    }
    let mut d = doc::load(bytes, password)?;
    let total = d.get_pages().len();
    let page_map = d.get_pages();
    let mut added = 0;
    for (i, a) in annotations.iter().enumerate() {
        let pid = *page_map.get(&a.page).ok_or_else(|| {
            EngineError::not_found(format!("Annotation {}: page {} does not exist", i + 1, a.page))
        })?;
        let r = Rect {
            x0: a.rect[0].min(a.rect[2]),
            y0: a.rect[1].min(a.rect[3]),
            x1: a.rect[0].max(a.rect[2]),
            y1: a.rect[1].max(a.rect[3]),
        };
        if r.width() <= 0.0 || r.height() <= 0.0 {
            return Err(EngineError::invalid(format!("Annotation {}: rect is empty", i + 1)));
        }
        let (cr, cg, cb) = parse_color(a.color.as_deref().unwrap_or(match a.kind.as_str() {
            "highlight" => "#ffff00",
            "note" => "#ffcc00",
            _ => "#ff0000",
        }))?;
        let opacity = a.opacity.unwrap_or(if a.kind == "highlight" { 0.5 } else { 1.0 });
        engine_shared::error::require_range(opacity, 0.0, 1.0, "opacity")?;
        let mut ann = Dictionary::new();
        ann.set("Type", Object::Name(b"Annot".to_vec()));
        ann.set("Rect", r.to_object());
        ann.set(
            "C",
            Object::Array(vec![
                Object::Real(cr as f32),
                Object::Real(cg as f32),
                Object::Real(cb as f32),
            ]),
        );
        ann.set("CA", Object::Real(opacity as f32));
        ann.set("F", Object::Integer(4));
        if let Some(c) = &a.contents {
            ann.set("Contents", doc::encode_text_string(c));
        }
        let ap_content: Option<String> = match a.kind.as_str() {
            "square" | "box" | "rectangle" => {
                ann.set("Subtype", Object::Name(b"Square".to_vec()));
                let mut bs = Dictionary::new();
                bs.set("W", Object::Integer(2));
                ann.set("BS", Object::Dictionary(bs));
                Some(format!(
                    "{cr:.3} {cg:.3} {cb:.3} RG 2 w 1 1 {:.3} {:.3} re S\n",
                    r.width() - 2.0,
                    r.height() - 2.0
                ))
            },
            "highlight" => {
                ann.set("Subtype", Object::Name(b"Highlight".to_vec()));
                ann.set(
                    "QuadPoints",
                    Object::Array(
                        vec![r.x0, r.y1, r.x1, r.y1, r.x0, r.y0, r.x1, r.y0]
                            .into_iter()
                            .map(|v| Object::Real(v as f32))
                            .collect(),
                    ),
                );
                Some(format!(
                    "/GSa gs {cr:.3} {cg:.3} {cb:.3} rg 0 0 {:.3} {:.3} re f\n",
                    r.width(),
                    r.height()
                ))
            },
            "note" | "sticky" => {
                ann.set("Subtype", Object::Name(b"Text".to_vec()));
                ann.set("Name", Object::Name(b"Comment".to_vec()));
                ann.set("Open", Object::Boolean(false));
                None
            },
            "text" | "freetext" => {
                ann.set("Subtype", Object::Name(b"FreeText".to_vec()));
                let fs = a.font_size.unwrap_or(12.0);
                ann.set(
                    "DA",
                    Object::string_literal(format!("/Helv {fs} Tf {cr:.3} {cg:.3} {cb:.3} rg")),
                );
                let text = a.contents.clone().unwrap_or_default();
                let (enc, _) = encode_winansi(&text);
                let font = StdFont::resolve("helvetica", false, false);
                let font_id = font.add_to_document(&mut d);
                let mut fonts = Dictionary::new();
                fonts.set("Helv", Object::Reference(font_id));
                let mut res = Dictionary::new();
                res.set("Font", Object::Dictionary(fonts));
                let mut form_dict = Dictionary::new();
                form_dict.set("Type", Object::Name(b"XObject".to_vec()));
                form_dict.set("Subtype", Object::Name(b"Form".to_vec()));
                form_dict.set(
                    "BBox",
                    Rect {
                        x0: 0.0,
                        y0: 0.0,
                        x1: r.width(),
                        y1: r.height(),
                    }
                    .to_object(),
                );
                form_dict.set("Resources", Object::Dictionary(res));
                let content = format!(
                    "BT /Helv {fs:.2} Tf {cr:.3} {cg:.3} {cb:.3} rg 2 {:.3} Td ({}) Tj ET\n",
                    r.height() - fs - 2.0,
                    String::from_utf8_lossy(&escape_pdf_string(&enc))
                );
                let ap_id = d.add_object(Stream::new(form_dict, content.into_bytes()));
                let mut ap = Dictionary::new();
                ap.set("N", Object::Reference(ap_id));
                ann.set("AP", Object::Dictionary(ap));
                None
            },
            "link" => {
                ann.set("Subtype", Object::Name(b"Link".to_vec()));
                let url = a.url.clone().ok_or_else(|| {
                    EngineError::invalid(format!("Annotation {}: link needs `url`", i + 1))
                })?;
                let mut action = Dictionary::new();
                action.set("S", Object::Name(b"URI".to_vec()));
                action.set("URI", Object::string_literal(url));
                ann.set("A", Object::Dictionary(action));
                ann.set(
                    "Border",
                    Object::Array(vec![Object::Integer(0), Object::Integer(0), Object::Integer(0)]),
                );
                None
            },
            other => {
                return Err(EngineError::invalid(format!(
                    "Annotation {}: unknown kind `{other}`",
                    i + 1
                )))
            },
        };
        if let Some(content) = ap_content {
            let mut gs = Dictionary::new();
            gs.set("Type", Object::Name(b"ExtGState".to_vec()));
            gs.set("CA", Object::Real(opacity as f32));
            gs.set("ca", Object::Real(opacity as f32));
            gs.set("BM", Object::Name(b"Multiply".to_vec()));
            let mut gss = Dictionary::new();
            gss.set("GSa", Object::Dictionary(gs));
            let mut res = Dictionary::new();
            res.set("ExtGState", Object::Dictionary(gss));
            let mut form_dict = Dictionary::new();
            form_dict.set("Type", Object::Name(b"XObject".to_vec()));
            form_dict.set("Subtype", Object::Name(b"Form".to_vec()));
            form_dict.set(
                "BBox",
                Rect {
                    x0: 0.0,
                    y0: 0.0,
                    x1: r.width(),
                    y1: r.height(),
                }
                .to_object(),
            );
            form_dict.set("Resources", Object::Dictionary(res));
            let ap_id = d.add_object(Stream::new(form_dict, content.into_bytes()));
            let mut ap = Dictionary::new();
            ap.set("N", Object::Reference(ap_id));
            ann.set("AP", Object::Dictionary(ap));
        }
        let ann_id = d.add_object(Object::Dictionary(ann));
        let page = d.get_dictionary_mut(pid).map_err(doc::map_pdf_error)?;
        let existing = page.get(b"Annots").ok().cloned();
        let mut annots = match existing {
            Some(Object::Array(a)) => a,
            Some(Object::Reference(r)) => d
                .get_object(r)
                .ok()
                .and_then(|o| o.as_array().ok())
                .cloned()
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        annots.push(Object::Reference(ann_id));
        let page = d.get_dictionary_mut(pid).map_err(doc::map_pdf_error)?;
        page.set("Annots", Object::Array(annots));
        added += 1;
    }
    let out = doc::save(&mut d)?;
    Ok(json!({ "output": PdfOutput::new(out, total), "added": added }))
}

/// A rectangle to redact on one page.
#[derive(Deserialize, Debug, Clone)]
pub struct RedactArea {
    /// 1-based page number.
    pub page: u32,
    /// `[x0, y0, x1, y1]` in page points (origin bottom-left).
    pub rect: [f64; 4],
}

/// Redacts text: removes every occurrence of `terms` from the page content
/// streams (replacing glyphs with spaces) and paints opaque boxes over the
/// given areas. Text inside images, vector outlines or composite-font runs
/// that cannot be decoded is *not* removed – the response lists warnings.
pub fn redact(
    bytes: &[u8],
    password: Option<&str>,
    terms: &[String],
    areas: &[RedactArea],
    case_sensitive: bool,
    color: &str,
) -> EngineResult<Value> {
    let terms: Vec<String> = terms
        .iter()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    if terms.is_empty() && areas.is_empty() {
        return Err(EngineError::empty("redaction terms/areas"));
    }
    let (cr, cg, cb) = parse_color(color)?;
    let mut d = doc::load(bytes, password)?;
    let total = d.get_pages().len();
    let page_map = d.get_pages();
    let mut removed = 0usize;
    let mut warnings: Vec<String> = Vec::new();

    if !terms.is_empty() {
        for (&n, &pid) in &page_map {
            let fonts = d.get_page_fonts(pid).unwrap_or_default();
            // Pre-compute font encodings (owned) so we can mutate afterwards.
            let mut encodings: std::collections::HashMap<Vec<u8>, (bool, lopdf::Encoding<'_>)> =
                std::collections::HashMap::new();
            for (name, font) in &fonts {
                let composite =
                    font.get(b"Subtype").ok().and_then(|s| s.as_name().ok()) == Some(b"Type0");
                if let Ok(enc) = font.get_font_encoding(&d) {
                    encodings.insert(name.clone(), (composite, enc));
                }
            }
            let content_bytes = d.get_page_content(pid);
            let mut content = match Content::decode(&content_bytes) {
                Ok(c) => c,
                Err(_) => {
                    warnings.push(format!(
                        "Page {n}: content stream could not be parsed; text not redacted"
                    ));
                    continue;
                },
            };
            let mut current_font: Vec<u8> = Vec::new();
            let mut page_hits = 0usize;
            let mut changed = false;
            for op in content.operations.iter_mut() {
                match op.operator.as_str() {
                    "Tf" => {
                        if let Some(Object::Name(n)) = op.operands.first() {
                            current_font = n.clone();
                        }
                    },
                    "Tj" | "'" | "\"" | "TJ" => {
                        let enc = encodings.get(&current_font);
                        for operand in op.operands.iter_mut() {
                            let strings: Vec<&mut Object> = match operand {
                                Object::String(_, _) => vec![operand],
                                Object::Array(arr) => arr
                                    .iter_mut()
                                    .filter(|o| matches!(o, Object::String(_, _)))
                                    .collect(),
                                _ => vec![],
                            };
                            for s in strings {
                                if let Object::String(raw, _) = s {
                                    let (composite, decoded) = match enc {
                                        Some((c, e)) => {
                                            (*c, Document::decode_text(e, raw).unwrap_or_default())
                                        },
                                        None => (false, raw.iter().map(|&b| b as char).collect()),
                                    };
                                    let hay = if case_sensitive {
                                        decoded.clone()
                                    } else {
                                        decoded.to_lowercase()
                                    };
                                    let hit = terms.iter().any(|t| {
                                        hay.contains(&if case_sensitive {
                                            t.clone()
                                        } else {
                                            t.to_lowercase()
                                        })
                                    });
                                    if !hit {
                                        continue;
                                    }
                                    page_hits += 1;
                                    changed = true;
                                    if composite || raw.len() != decoded.chars().count() {
                                        // Cannot map characters to bytes 1:1 – blank the whole run.
                                        for b in raw.iter_mut() {
                                            *b = b' ';
                                        }
                                        continue;
                                    }
                                    let chars: Vec<char> = decoded.chars().collect();
                                    let hay_chars: Vec<char> = hay.chars().collect();
                                    for t in &terms {
                                        let tc: Vec<char> = if case_sensitive {
                                            t.chars().collect()
                                        } else {
                                            t.to_lowercase().chars().collect()
                                        };
                                        if tc.is_empty() || tc.len() > hay_chars.len() {
                                            continue;
                                        }
                                        let mut i = 0;
                                        while i + tc.len() <= hay_chars.len() {
                                            if hay_chars[i..i + tc.len()] == tc[..] {
                                                for j in i..i + tc.len() {
                                                    raw[j] = b' ';
                                                }
                                                i += tc.len();
                                            } else {
                                                i += 1;
                                            }
                                        }
                                    }
                                    let _ = chars;
                                }
                            }
                        }
                    },
                    _ => {},
                }
            }
            if changed {
                removed += page_hits;
                let encoded = content.encode().map_err(|e| {
                    EngineError::internal(format!("Failed to re-encode content: {e}"))
                })?;
                // Replace all content streams with a single new one.
                let new_id = d.add_object(Stream::new(Dictionary::new(), encoded));
                let page = d.get_dictionary_mut(pid).map_err(doc::map_pdf_error)?;
                page.set("Contents", Object::Reference(new_id));
            }
        }
    }

    for (i, a) in areas.iter().enumerate() {
        let pid = *page_map.get(&a.page).ok_or_else(|| {
            EngineError::not_found(format!("Area {}: page {} does not exist", i + 1, a.page))
        })?;
        let r = Rect {
            x0: a.rect[0].min(a.rect[2]),
            y0: a.rect[1].min(a.rect[3]),
            x1: a.rect[0].max(a.rect[2]),
            y1: a.rect[1].max(a.rect[3]),
        };
        let content = format!(
            "q {cr:.3} {cg:.3} {cb:.3} rg {:.3} {:.3} {:.3} {:.3} re f Q\n",
            r.x0,
            r.y0,
            r.width(),
            r.height()
        );
        append_content(&mut d, pid, content)?;
    }
    if !areas.is_empty() {
        warnings.push(
            "Area boxes hide content visually but do not remove underlying text/images; combine \
             with `terms` to strip text."
                .into(),
        );
    }
    // Drop document-level metadata that may echo redacted text.
    let out = doc::save(&mut d)?;
    Ok(json!({
        "output": PdfOutput::new(out, total),
        "text_runs_redacted": removed,
        "areas_painted": areas.len(),
        "warnings": warnings
    }))
}

/// Converts page content and embedded raster images to grayscale.
///
/// Vector colours set with `rg`/`RG`/`k`/`K`/`sc`/`scn` (Device colour
/// spaces) are replaced with luminance equivalents; DCT/Flate RGB images are
/// re-encoded. Colours inside nested Form XObjects, shadings, patterns,
/// ICC/indexed colour spaces and annotations are left unchanged and reported.
pub fn grayscale(bytes: &[u8], password: Option<&str>) -> EngineResult<Value> {
    let mut d = doc::load(bytes, password)?;
    let total = d.get_pages().len();
    let page_map = d.get_pages();
    let mut images_converted = 0usize;
    let mut images_skipped = 0usize;
    let mut ops_converted = 0usize;
    let lum = |r: f64, g: f64, b: f64| 0.299 * r + 0.587 * g + 0.114 * b;
    let cmyk_gray = |c: f64, m: f64, y: f64, k: f64| {
        1.0 - (0.299 * c + 0.587 * m + 0.114 * y).min(1.0) * (1.0 - k) - k
    };
    let num = |o: &Object| o.as_float().ok().map(|f| f as f64);

    for &pid in page_map.values() {
        let content_bytes = d.get_page_content(pid);
        if let Ok(mut content) = Content::decode(&content_bytes) {
            let mut changed = false;
            for op in content.operations.iter_mut() {
                let nums: Vec<f64> = op.operands.iter().filter_map(num).collect();
                let all_num = nums.len() == op.operands.len();
                let (new_op, g): (Option<&str>, Option<f64>) =
                    match (op.operator.as_str(), nums.len(), all_num) {
                        ("rg", 3, true) => (Some("g"), Some(lum(nums[0], nums[1], nums[2]))),
                        ("RG", 3, true) => (Some("G"), Some(lum(nums[0], nums[1], nums[2]))),
                        ("k", 4, true) => {
                            (Some("g"), Some(cmyk_gray(nums[0], nums[1], nums[2], nums[3])))
                        },
                        ("K", 4, true) => {
                            (Some("G"), Some(cmyk_gray(nums[0], nums[1], nums[2], nums[3])))
                        },
                        ("sc" | "scn", 3, true) => {
                            (Some("g"), Some(lum(nums[0], nums[1], nums[2])))
                        },
                        ("SC" | "SCN", 3, true) => {
                            (Some("G"), Some(lum(nums[0], nums[1], nums[2])))
                        },
                        ("sc" | "scn", 4, true) => {
                            (Some("g"), Some(cmyk_gray(nums[0], nums[1], nums[2], nums[3])))
                        },
                        ("SC" | "SCN", 4, true) => {
                            (Some("G"), Some(cmyk_gray(nums[0], nums[1], nums[2], nums[3])))
                        },
                        _ => (None, None),
                    };
                if let (Some(o), Some(g)) = (new_op, g) {
                    *op = Operation::new(o, vec![Object::Real(g.clamp(0.0, 1.0) as f32)]);
                    changed = true;
                    ops_converted += 1;
                } else if matches!(op.operator.as_str(), "cs" | "CS") {
                    if let Some(Object::Name(n)) = op.operands.first() {
                        if n == b"DeviceRGB" || n == b"DeviceCMYK" {
                            op.operands[0] = Object::Name(b"DeviceGray".to_vec());
                            changed = true;
                        }
                    }
                }
            }
            if changed {
                if let Ok(encoded) = content.encode() {
                    let new_id = d.add_object(Stream::new(Dictionary::new(), encoded));
                    if let Ok(page) = d.get_dictionary_mut(pid) {
                        page.set("Contents", Object::Reference(new_id));
                    }
                }
            }
        }

        // Images referenced by the page resources.
        let mut image_ids = Vec::new();
        if let Ok((res, res_ids)) = d.get_page_resources(pid) {
            let mut dicts: Vec<Dictionary> = res.into_iter().cloned().collect();
            for rid in res_ids {
                if let Ok(r) = d.get_dictionary(rid) {
                    dicts.push(r.clone());
                }
            }
            for res in dicts {
                if let Some(x) = res
                    .get(b"XObject")
                    .ok()
                    .and_then(|x| d.dereference(x).ok())
                    .and_then(|(_, x)| x.as_dict().ok())
                {
                    for (_, o) in x.iter() {
                        if let Ok(id) = o.as_reference() {
                            image_ids.push(id);
                        }
                    }
                }
            }
        }
        for id in image_ids {
            let stream = match d.get_object(id) {
                Ok(Object::Stream(s)) => s.clone(),
                _ => continue,
            };
            if stream.dict.get(b"Subtype").ok().and_then(|s| s.as_name().ok()) != Some(b"Image") {
                continue;
            }
            if stream
                .dict
                .get(b"ColorSpace")
                .ok()
                .and_then(|c| d.dereference(c).ok())
                .and_then(|(_, c)| c.as_name().ok().map(|n| n == b"DeviceGray"))
                .unwrap_or(false)
            {
                continue;
            }
            match images::decode_image_stream(&d, &stream)
                .and_then(|(img, kind)| images::gray_stream(&img, kind, &stream))
            {
                Some(new_stream) => {
                    d.objects.insert(id, Object::Stream(new_stream));
                    images_converted += 1;
                },
                None => images_skipped += 1,
            }
        }
    }
    let out = doc::save(&mut d)?;
    let mut warnings = Vec::new();
    if images_skipped > 0 {
        warnings.push(format!(
            "{images_skipped} image(s) use an encoding/colour space that could not be converted"
        ));
    }
    warnings.push(
        "Colours inside nested form XObjects, shadings, patterns and annotations are not converted."
            .to_string(),
    );
    Ok(json!({
        "output": PdfOutput::new(out, total),
        "colour_operators_converted": ops_converted,
        "images_converted": images_converted,
        "images_skipped": images_skipped,
        "warnings": warnings,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{png_bytes, sample_pdf};
    use engine_shared::b64;

    fn reload(v: &Value) -> Document {
        let bytes = b64::decode(v["output"]["file"].as_str().unwrap()).unwrap();
        doc::load(&bytes, None).unwrap()
    }

    #[test]
    fn colours() {
        assert_eq!(parse_color("#ff0000").unwrap(), (1.0, 0.0, 0.0));
        assert_eq!(parse_color("#fff").unwrap(), (1.0, 1.0, 1.0));
        assert_eq!(parse_color("rgb(0, 255, 0)").unwrap(), (0.0, 1.0, 0.0));
        assert_eq!(parse_color("black").unwrap(), (0.0, 0.0, 0.0));
        assert!(parse_color("nope").is_err());
    }

    #[test]
    fn page_numbers_header_footer_watermark() {
        let pdf = sample_pdf(&["one", "two"]);
        let r = add_page_numbers(
            &pdf,
            None,
            "Page {n} of {total}",
            1,
            &LabelOptions::default(),
            &PageSelection::All,
        )
        .unwrap();
        let d = reload(&r);
        assert!(d.extract_text(&[2]).unwrap().contains("Page 2 of 2"));
        assert!(d.extract_text(&[1]).unwrap().contains("one"));

        let hf = add_header_footer(
            &pdf,
            None,
            "Header",
            "Footer {page}",
            "center",
            &LabelOptions::default(),
            &PageSelection::All,
        )
        .unwrap();
        let d = reload(&hf);
        let t = d.extract_text(&[1]).unwrap();
        assert!(t.contains("Header") && t.contains("Footer 1"));
        assert!(add_header_footer(
            &pdf,
            None,
            "",
            " ",
            "center",
            &LabelOptions::default(),
            &PageSelection::All
        )
        .is_err());

        let wm = add_watermark(
            &pdf,
            None,
            "CONFIDENTIAL",
            &WatermarkOptions::default(),
            &PageSelection::All,
        )
        .unwrap();
        let d = reload(&wm);
        assert!(d.extract_text(&[1]).unwrap().contains("CONFIDENTIAL"));
        let tiled = WatermarkOptions {
            tiled: true,
            behind: true,
            ..Default::default()
        };
        assert!(add_watermark(&pdf, None, "DRAFT", &tiled, &PageSelection::All).is_ok());
        assert!(
            add_watermark(&pdf, None, "  ", &WatermarkOptions::default(), &PageSelection::All)
                .is_err()
        );
        let bad = WatermarkOptions { opacity: 2.0, ..Default::default() };
        assert!(add_watermark(&pdf, None, "x", &bad, &PageSelection::All).is_err());
    }

    #[test]
    fn stamps_overlay_annotations() {
        let pdf = sample_pdf(&["base"]);
        let png = png_bytes(10, 5);
        let st = stamp_image(
            &pdf,
            None,
            &png,
            &StampOptions { width: Some(100.0), ..Default::default() },
            &PageSelection::All,
        )
        .unwrap();
        assert_eq!(st["stamp"]["height_pt"], 50.0);
        let d = reload(&st);
        assert!(d.extract_text(&[1]).unwrap().contains("base"));
        assert!(
            stamp_image(&pdf, None, b"", &StampOptions::default(), &PageSelection::All).is_err()
        );

        let top = sample_pdf(&["OVER"]);
        let ov = overlay(&pdf, None, &top, None, 1, false, 0.5, &PageSelection::All).unwrap();
        let d = reload(&ov);
        let pid = d.get_pages()[&1];
        let (res, _) = d.get_page_resources(pid).unwrap();
        assert!(res.unwrap().get(b"XObject").is_ok());
        assert!(overlay(&pdf, None, &top, None, 5, false, 1.0, &PageSelection::All).is_err());

        let anns = vec![
            Annotation {
                page: 1,
                kind: "highlight".into(),
                rect: [10.0, 10.0, 100.0, 30.0],
                color: None,
                contents: None,
                url: None,
                opacity: None,
                font_size: None,
            },
            Annotation {
                page: 1,
                kind: "square".into(),
                rect: [10.0, 40.0, 100.0, 80.0],
                color: Some("blue".into()),
                contents: Some("box".into()),
                url: None,
                opacity: None,
                font_size: None,
            },
            Annotation {
                page: 1,
                kind: "note".into(),
                rect: [10.0, 100.0, 30.0, 120.0],
                color: None,
                contents: Some("hello".into()),
                url: None,
                opacity: None,
                font_size: None,
            },
            Annotation {
                page: 1,
                kind: "text".into(),
                rect: [10.0, 140.0, 200.0, 160.0],
                color: None,
                contents: Some("free text".into()),
                url: None,
                opacity: None,
                font_size: Some(10.0),
            },
            Annotation {
                page: 1,
                kind: "link".into(),
                rect: [10.0, 180.0, 200.0, 200.0],
                color: None,
                contents: None,
                url: Some("https://example.com".into()),
                opacity: None,
                font_size: None,
            },
        ];
        let a = add_annotations(&pdf, None, &anns).unwrap();
        assert_eq!(a["added"], 5);
        let d = reload(&a);
        assert_eq!(d.get_page_annotations(d.get_pages()[&1]).unwrap().len(), 5);
        let bad = vec![Annotation {
            page: 1,
            kind: "weird".into(),
            rect: [0.0, 0.0, 1.0, 1.0],
            color: None,
            contents: None,
            url: None,
            opacity: None,
            font_size: None,
        }];
        assert!(add_annotations(&pdf, None, &bad).is_err());
        assert!(add_annotations(&pdf, None, &[]).is_err());
    }

    #[test]
    fn redact_and_grayscale() {
        let pdf = sample_pdf(&["Secret code 1234 here"]);
        let r = redact(
            &pdf,
            None,
            &["1234".into()],
            &[RedactArea { page: 1, rect: [0.0, 0.0, 50.0, 50.0] }],
            true,
            "black",
        )
        .unwrap();
        assert_eq!(r["text_runs_redacted"], 1);
        let d = reload(&r);
        let t = d.extract_text(&[1]).unwrap();
        assert!(!t.contains("1234"));
        assert!(t.contains("Secret code"));
        assert!(redact(&pdf, None, &[], &[], true, "black").is_err());

        let g = grayscale(&pdf, None).unwrap();
        assert!(g["colour_operators_converted"].as_u64().unwrap() >= 1);
        let d = reload(&g);
        assert!(d.extract_text(&[1]).unwrap().contains("Secret"));
    }
}
