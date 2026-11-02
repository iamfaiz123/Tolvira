//! Composition: text rendering, watermarks, overlays, sprites, contact
//! sheets, image diffs, favicons and social (OG) images.

use crate::codec::{self, parse_color, Decoded, EncodeOptions, OutFormat};
use crate::transform::OutputOptions;
use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use engine_shared::{b64, limits, EngineError, EngineResult};
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use serde::Deserialize;
use serde_json::{json, Value};

static FONT_BOLD: &[u8] = include_bytes!("../assets/DejaVuSans-Bold-subset.ttf");
static FONT_REGULAR: &[u8] = include_bytes!("../assets/DejaVuSans-subset.ttf");

/// The embedded fonts are validated by the unit tests; if parsing ever fails
/// text is simply skipped rather than aborting the instance.
fn font(bold: bool) -> Option<FontRef<'static>> {
    FontRef::try_from_slice(if bold { FONT_BOLD } else { FONT_REGULAR }).ok()
}

/// Measures a single line of text.
pub fn measure(text: &str, size: f32, bold: bool) -> (u32, u32) {
    let Some(f) = font(bold) else {
        return (0, 0);
    };
    let scaled = f.as_scaled(PxScale::from(size));
    let mut w = 0.0f32;
    let mut prev: Option<ab_glyph::GlyphId> = None;
    for ch in text.chars() {
        let id = scaled.glyph_id(ch);
        if let Some(p) = prev {
            w += scaled.kern(p, id);
        }
        w += scaled.h_advance(id);
        prev = Some(id);
    }
    (w.ceil() as u32, (scaled.ascent() - scaled.descent()).ceil() as u32)
}

/// Draws a line of text onto `canvas` at (x, y) = top-left of the text box.
pub fn draw_text(
    canvas: &mut RgbaImage,
    text: &str,
    x: f32,
    y: f32,
    size: f32,
    color: [u8; 4],
    bold: bool,
) {
    let Some(f) = font(bold) else {
        return;
    };
    let scaled = f.as_scaled(PxScale::from(size));
    let mut cursor = x;
    let baseline = y + scaled.ascent();
    let mut prev: Option<ab_glyph::GlyphId> = None;
    let (cw, ch) = canvas.dimensions();
    for ch_ in text.chars() {
        let id = scaled.glyph_id(ch_);
        if let Some(p) = prev {
            cursor += scaled.kern(p, id);
        }
        let glyph =
            id.with_scale_and_position(PxScale::from(size), ab_glyph::point(cursor, baseline));
        if let Some(outlined) = f.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, coverage| {
                let px = bounds.min.x as i64 + gx as i64;
                let py = bounds.min.y as i64 + gy as i64;
                if px < 0 || py < 0 || px >= cw as i64 || py >= ch as i64 {
                    return;
                }
                let a = (coverage * color[3] as f32 / 255.0).clamp(0.0, 1.0);
                if a <= 0.0 {
                    return;
                }
                let p = canvas.get_pixel_mut(px as u32, py as u32);
                blend_pixel(p, color, a);
            });
        }
        cursor += scaled.h_advance(id);
        prev = Some(id);
    }
}

fn blend_pixel(dst: &mut Rgba<u8>, src: [u8; 4], alpha: f32) {
    let da = dst[3] as f32 / 255.0;
    let out_a = alpha + da * (1.0 - alpha);
    if out_a <= 0.0 {
        return;
    }
    for c in 0..3 {
        let v = (src[c] as f32 * alpha + dst[c] as f32 * da * (1.0 - alpha)) / out_a;
        dst[c] = v.round().clamp(0.0, 255.0) as u8;
    }
    dst[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
}

/// Alpha-composites `src` over `dst` at (x, y) with global `opacity`.
pub fn composite(dst: &mut RgbaImage, src: &RgbaImage, x: i64, y: i64, opacity: f32) {
    let (dw, dh) = dst.dimensions();
    for (sx, sy, p) in src.enumerate_pixels() {
        let tx = x + sx as i64;
        let ty = y + sy as i64;
        if tx < 0 || ty < 0 || tx >= dw as i64 || ty >= dh as i64 {
            continue;
        }
        let a = (p[3] as f32 / 255.0) * opacity;
        if a <= 0.0 {
            continue;
        }
        blend_pixel(dst.get_pixel_mut(tx as u32, ty as u32), p.0, a);
    }
}

/// Named anchor on an image.
#[derive(Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Position {
    /// Top-left corner.
    TopLeft,
    /// Top edge, centred.
    TopCenter,
    /// Top-right corner.
    TopRight,
    /// Left edge, centred.
    CenterLeft,
    /// Image centre.
    Center,
    /// Right edge, centred.
    CenterRight,
    /// Bottom-left corner.
    BottomLeft,
    /// Bottom edge, centred.
    BottomCenter,
    /// Bottom-right corner.
    #[default]
    BottomRight,
    /// Repeat across the whole image.
    Tile,
}

fn anchor(pos: Position, cw: u32, ch: u32, w: u32, h: u32, margin: i64) -> (i64, i64) {
    let x = match pos {
        Position::TopLeft | Position::CenterLeft | Position::BottomLeft => margin,
        Position::TopCenter | Position::Center | Position::BottomCenter | Position::Tile => {
            (cw as i64 - w as i64) / 2
        },
        Position::TopRight | Position::CenterRight | Position::BottomRight => {
            cw as i64 - w as i64 - margin
        },
    };
    let y = match pos {
        Position::TopLeft | Position::TopCenter | Position::TopRight => margin,
        Position::CenterLeft | Position::Center | Position::CenterRight | Position::Tile => {
            (ch as i64 - h as i64) / 2
        },
        Position::BottomLeft | Position::BottomCenter | Position::BottomRight => {
            ch as i64 - h as i64 - margin
        },
    };
    (x, y)
}

/// Appearance of a text watermark.
#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct TextWatermarkOptions {
    /// Font size in pixels.
    pub font_size: f32,
    /// Text colour (`#rrggbb[aa]`).
    pub color: String,
    /// Opacity 0–1.
    pub opacity: f32,
    /// Anchor position.
    pub position: Position,
    /// Distance from the edges in pixels.
    pub margin: i64,
    /// Rotation in degrees (counter-clockwise).
    pub rotation: f32,
    /// Use the bold face.
    pub bold: bool,
    /// Optional background box behind the text (`#rrggbbaa`).
    pub box_color: Option<String>,
}

impl Default for TextWatermarkOptions {
    fn default() -> Self {
        TextWatermarkOptions {
            font_size: 32.0,
            color: "#ffffff".into(),
            opacity: 0.6,
            position: Position::BottomRight,
            margin: 16,
            rotation: 0.0,
            bold: true,
            box_color: None,
        }
    }
}

/// Renders text into a transparent RGBA tile (rotated if requested).
fn render_text_tile(text: &str, opts: &TextWatermarkOptions) -> EngineResult<RgbaImage> {
    let color = parse_color(&opts.color)?;
    let (tw, th) = measure(text, opts.font_size, opts.bold);
    let pad = 8u32;
    let mut tile = RgbaImage::from_pixel(tw + pad * 2, th + pad * 2, Rgba([0, 0, 0, 0]));
    if let Some(bc) = &opts.box_color {
        let bcol = parse_color(bc)?;
        for p in tile.pixels_mut() {
            *p = Rgba(bcol);
        }
    }
    draw_text(&mut tile, text, pad as f32, pad as f32, opts.font_size, color, opts.bold);
    if opts.rotation.abs() > 0.01 {
        let rotated = crate::transform::rotate_rgba(&tile, opts.rotation as f64)?;
        return Ok(rotated);
    }
    Ok(tile)
}

/// Adds a text watermark.
pub fn watermark_text(
    d: &Decoded,
    text: &str,
    opts: &TextWatermarkOptions,
    out: &OutputOptions,
) -> EngineResult<Value> {
    engine_shared::error::require_text(text, "watermark text")?;
    engine_shared::error::require_range(opts.opacity as f64, 0.0, 1.0, "opacity")?;
    engine_shared::error::require_range(opts.font_size as f64, 4.0, 1024.0, "font_size")?;
    let tile = render_text_tile(text, opts)?;
    let mut canvas = d.image.to_rgba8();
    place_tile(&mut canvas, &tile, opts.position, opts.margin, opts.opacity);
    let o = out.encode_options(d.format)?;
    Ok(json!({
        "output": codec::output(&DynamicImage::ImageRgba8(canvas), &o)?,
        "text_size": { "width": tile.width(), "height": tile.height() }
    }))
}

fn place_tile(
    canvas: &mut RgbaImage,
    tile: &RgbaImage,
    position: Position,
    margin: i64,
    opacity: f32,
) {
    let (cw, ch) = canvas.dimensions();
    if position == Position::Tile {
        let step_x = (tile.width() as i64 + margin.max(0) * 4).max(1);
        let step_y = (tile.height() as i64 + margin.max(0) * 4).max(1);
        let mut y = -(tile.height() as i64) / 2;
        let mut row = 0;
        while y < ch as i64 {
            let mut x = if row % 2 == 0 { 0 } else { -step_x / 2 };
            while x < cw as i64 {
                composite(canvas, tile, x, y, opacity);
                x += step_x;
            }
            y += step_y;
            row += 1;
        }
    } else {
        let (x, y) = anchor(position, cw, ch, tile.width(), tile.height(), margin);
        composite(canvas, tile, x, y, opacity);
    }
}

/// Placement of an image watermark.
#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct ImageWatermarkOptions {
    /// Opacity 0–1.
    pub opacity: f32,
    /// Anchor position.
    pub position: Position,
    /// Distance from the edges in pixels.
    pub margin: i64,
    /// Watermark width as a fraction of the base width (0.01 – 1). 0 = keep original size.
    pub scale: f32,
    /// Explicit x/y override (top-left of the watermark).
    pub x: Option<i64>,
    /// Explicit y override (top-left of the watermark).
    pub y: Option<i64>,
}

impl Default for ImageWatermarkOptions {
    fn default() -> Self {
        ImageWatermarkOptions {
            opacity: 0.6,
            position: Position::BottomRight,
            margin: 16,
            scale: 0.25,
            x: None,
            y: None,
        }
    }
}

/// Adds an image watermark / logo (also used as generic image overlay).
pub fn watermark_image(
    d: &Decoded,
    overlay: &[u8],
    opts: &ImageWatermarkOptions,
    out: &OutputOptions,
) -> EngineResult<Value> {
    let ov = codec::decode(overlay)
        .map_err(|e| EngineError::new(e.code, format!("Overlay image: {}", e.message)))?;
    engine_shared::error::require_range(opts.opacity as f64, 0.0, 1.0, "opacity")?;
    engine_shared::error::require_range(opts.scale as f64, 0.0, 4.0, "scale")?;
    let mut canvas = d.image.to_rgba8();
    let tile = if opts.scale > 0.0 {
        let target_w = ((canvas.width() as f32 * opts.scale).round() as u32).max(1);
        let target_h = ((ov.image.height() as f32 * target_w as f32 / ov.image.width() as f32)
            .round() as u32)
            .max(1);
        ov.image
            .resize_exact(target_w, target_h, image::imageops::FilterType::Lanczos3)
            .to_rgba8()
    } else {
        ov.image.to_rgba8()
    };
    match (opts.x, opts.y) {
        (Some(x), Some(y)) => composite(&mut canvas, &tile, x, y, opts.opacity),
        _ => place_tile(&mut canvas, &tile, opts.position, opts.margin, opts.opacity),
    }
    let o = out.encode_options(d.format)?;
    Ok(json!({
        "output": codec::output(&DynamicImage::ImageRgba8(canvas), &o)?,
        "overlay_size": { "width": tile.width(), "height": tile.height() }
    }))
}

/// Builds a sprite sheet from several images and returns CSS coordinates.
pub fn sprite(
    images: &[Vec<u8>],
    columns: u32,
    padding: u32,
    background: Option<&str>,
    names: &[String],
) -> EngineResult<Value> {
    limits::check_count(images.len(), 256, "images")?;
    if columns == 0 {
        return Err(EngineError::invalid("`columns` must be > 0"));
    }
    let decoded: Vec<Decoded> = images
        .iter()
        .enumerate()
        .map(|(i, b)| {
            codec::decode(b)
                .map_err(|e| EngineError::new(e.code, format!("Image {}: {}", i + 1, e.message)))
        })
        .collect::<EngineResult<_>>()?;
    let cols = columns.min(decoded.len() as u32);
    let rows = (decoded.len() as u32).div_ceil(cols);
    let cell_w = decoded.iter().map(|d| d.image.width()).max().unwrap_or(1);
    let cell_h = decoded.iter().map(|d| d.image.height()).max().unwrap_or(1);
    let sheet_w = cols * cell_w + (cols + 1) * padding;
    let sheet_h = rows * cell_h + (rows + 1) * padding;
    limits::check_dimensions(sheet_w, sheet_h)?;
    let bg = background.map(parse_color).transpose()?.unwrap_or([0, 0, 0, 0]);
    let mut sheet = RgbaImage::from_pixel(sheet_w, sheet_h, Rgba(bg));
    let mut css = String::new();
    let mut entries = Vec::new();
    for (i, d) in decoded.iter().enumerate() {
        let col = i as u32 % cols;
        let row = i as u32 / cols;
        let x = padding + col * (cell_w + padding);
        let y = padding + row * (cell_h + padding);
        composite(&mut sheet, &d.image.to_rgba8(), x as i64, y as i64, 1.0);
        let name = names.get(i).cloned().unwrap_or_else(|| format!("sprite-{}", i + 1));
        css.push_str(&format!(
            ".{name} {{ background-position: -{x}px -{y}px; width: {}px; height: {}px; }}\n",
            d.image.width(),
            d.image.height()
        ));
        entries.push(json!({
            "name": name,
            "x": x,
            "y": y,
            "width": d.image.width(),
            "height": d.image.height()
        }));
    }
    let out = codec::output(&DynamicImage::ImageRgba8(sheet), &EncodeOptions::default())?;
    Ok(json!({ "output": out, "sprites": entries, "css": css, "columns": cols, "rows": rows }))
}

/// Layout of a contact sheet / grid.
#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct GridOptions {
    /// Number of columns.
    pub columns: u32,
    /// Cell width in pixels.
    pub cell_width: u32,
    /// Cell height in pixels.
    pub cell_height: u32,
    /// Gap between cells in pixels.
    pub padding: u32,
    /// Background colour.
    pub background: String,
    /// Optional caption per image.
    pub labels: Vec<String>,
    /// Caption font size.
    pub label_size: f32,
    /// Caption colour.
    pub label_color: String,
    /// `contain` (default) or `cover`.
    pub fit: String,
    /// `png` (default), `jpeg` or `webp`.
    pub format: Option<String>,
    /// JPEG quality.
    pub quality: Option<u8>,
}

impl Default for GridOptions {
    fn default() -> Self {
        GridOptions {
            columns: 3,
            cell_width: 300,
            cell_height: 300,
            padding: 10,
            background: "#ffffff".into(),
            labels: vec![],
            label_size: 14.0,
            label_color: "#333333".into(),
            fit: "contain".into(),
            format: None,
            quality: None,
        }
    }
}

/// Arranges images in a grid (contact sheet / collage / mosaic).
pub fn contact_sheet(images: &[Vec<u8>], opts: &GridOptions) -> EngineResult<Value> {
    limits::check_count(images.len(), 400, "images")?;
    if opts.columns == 0 || opts.cell_width == 0 || opts.cell_height == 0 {
        return Err(EngineError::invalid("`columns`, `cell_width` and `cell_height` must be > 0"));
    }
    let cols = opts.columns.min(images.len() as u32);
    let rows = (images.len() as u32).div_ceil(cols);
    let label_h = if opts.labels.is_empty() {
        0
    } else {
        (opts.label_size * 1.6) as u32
    };
    let w = cols * opts.cell_width + (cols + 1) * opts.padding;
    let h = rows * (opts.cell_height + label_h) + (rows + 1) * opts.padding;
    limits::check_dimensions(w, h)?;
    let bg = parse_color(&opts.background)?;
    let label_color = parse_color(&opts.label_color)?;
    let mut sheet = RgbaImage::from_pixel(w, h, Rgba(bg));
    for (i, bytes) in images.iter().enumerate() {
        let d = codec::decode(bytes)
            .map_err(|e| EngineError::new(e.code, format!("Image {}: {}", i + 1, e.message)))?;
        let cell = if opts.fit == "cover" {
            d.image.resize_to_fill(
                opts.cell_width,
                opts.cell_height,
                image::imageops::FilterType::Lanczos3,
            )
        } else {
            d.image
                .resize(opts.cell_width, opts.cell_height, image::imageops::FilterType::Lanczos3)
        };
        let col = i as u32 % cols;
        let row = i as u32 / cols;
        let cx = opts.padding + col * (opts.cell_width + opts.padding);
        let cy = opts.padding + row * (opts.cell_height + label_h + opts.padding);
        let ox = cx + (opts.cell_width - cell.width()) / 2;
        let oy = cy + (opts.cell_height - cell.height()) / 2;
        composite(&mut sheet, &cell.to_rgba8(), ox as i64, oy as i64, 1.0);
        if let Some(label) = opts.labels.get(i) {
            let (lw, _) = measure(label, opts.label_size, false);
            let lx = cx as f32 + (opts.cell_width as f32 - lw as f32) / 2.0;
            draw_text(
                &mut sheet,
                label,
                lx.max(cx as f32),
                (cy + opts.cell_height) as f32 + 2.0,
                opts.label_size,
                label_color,
                false,
            );
        }
    }
    let fmt = match &opts.format {
        Some(f) => OutFormat::parse(f)?,
        None => OutFormat::Png,
    };
    let enc = EncodeOptions {
        format: fmt,
        quality: opts.quality.unwrap_or(85),
        background: [bg[0], bg[1], bg[2]],
        ..Default::default()
    };
    Ok(json!({
        "output": codec::output(&DynamicImage::ImageRgba8(sheet), &enc)?,
        "columns": cols,
        "rows": rows,
        "images": images.len()
    }))
}

/// Compares two images pixel by pixel and produces a diff visualisation.
pub fn difference(a: &[u8], b: &[u8], threshold: u8, highlight: &str) -> EngineResult<Value> {
    let da = codec::decode(a)
        .map_err(|e| EngineError::new(e.code, format!("Image A: {}", e.message)))?;
    let db = codec::decode(b)
        .map_err(|e| EngineError::new(e.code, format!("Image B: {}", e.message)))?;
    let (w, h) = (da.image.width().max(db.image.width()), da.image.height().max(db.image.height()));
    let pa = da.image.to_rgba8();
    let pb = db.image.to_rgba8();
    let hl = parse_color(highlight)?;
    let mut diff = RgbaImage::new(w, h);
    let mut changed = 0u64;
    let mut total_delta = 0u64;
    let (mut minx, mut miny, mut maxx, mut maxy) = (u32::MAX, u32::MAX, 0u32, 0u32);
    for y in 0..h {
        for x in 0..w {
            let qa = if x < pa.width() && y < pa.height() {
                pa.get_pixel(x, y).0
            } else {
                [0, 0, 0, 0]
            };
            let qb = if x < pb.width() && y < pb.height() {
                pb.get_pixel(x, y).0
            } else {
                [0, 0, 0, 0]
            };
            let delta = (0..4)
                .map(|c| (qa[c] as i32 - qb[c] as i32).unsigned_abs())
                .max()
                .unwrap_or(0);
            if delta > threshold as u32 {
                changed += 1;
                total_delta += delta as u64;
                diff.put_pixel(x, y, Rgba(hl));
                minx = minx.min(x);
                miny = miny.min(y);
                maxx = maxx.max(x);
                maxy = maxy.max(y);
            } else {
                // Faded original for context.
                let g = (0.299 * qa[0] as f64 + 0.587 * qa[1] as f64 + 0.114 * qa[2] as f64) as u8;
                let v = 128 + g / 2;
                diff.put_pixel(x, y, Rgba([v, v, v, 255]));
            }
        }
    }
    let total = (w as u64) * (h as u64);
    Ok(json!({
        "output": codec::output(&DynamicImage::ImageRgba8(diff), &EncodeOptions::default())?,
        "identical": changed == 0 && da.image.dimensions() == db.image.dimensions(),
        "same_dimensions": da.image.dimensions() == db.image.dimensions(),
        "dimensions_a": { "width": da.image.width(), "height": da.image.height() },
        "dimensions_b": { "width": db.image.width(), "height": db.image.height() },
        "changed_pixels": changed,
        "changed_percent": ((changed as f64 / total as f64) * 10000.0).round() / 100.0,
        "mean_delta": if changed > 0 { total_delta as f64 / changed as f64 } else { 0.0 },
        "bounding_box": if changed > 0 { json!({ "x": minx, "y": miny, "width": maxx - minx + 1, "height": maxy - miny + 1 }) } else { Value::Null },
    }))
}

/// Generates favicon assets: a multi-size ICO plus individual PNGs.
pub fn favicon(d: &Decoded, sizes: &[u32], background: Option<&str>) -> EngineResult<Value> {
    let sizes: Vec<u32> = if sizes.is_empty() {
        vec![16, 32, 48]
    } else {
        sizes.to_vec()
    };
    for &s in &sizes {
        if !(8..=256).contains(&s) {
            return Err(EngineError::invalid("Favicon sizes must be between 8 and 256 pixels"));
        }
    }
    let square = {
        let (w, h) = d.image.dimensions();
        if w == h {
            d.image.clone()
        } else {
            let side = w.max(h);
            let bg = background.map(parse_color).transpose()?.unwrap_or([0, 0, 0, 0]);
            let mut canvas = RgbaImage::from_pixel(side, side, Rgba(bg));
            composite(
                &mut canvas,
                &d.image.to_rgba8(),
                ((side - w) / 2) as i64,
                ((side - h) / 2) as i64,
                1.0,
            );
            DynamicImage::ImageRgba8(canvas)
        }
    };
    let mut pngs = Vec::new();
    let mut ico_images: Vec<image::DynamicImage> = Vec::new();
    for &s in &sizes {
        let r = square.resize_exact(s, s, image::imageops::FilterType::Lanczos3);
        pngs.push(json!({
            "size": s,
            "name": format!("favicon-{s}x{s}.png"),
            "output": codec::output(&r, &EncodeOptions::default())?
        }));
        ico_images.push(r);
    }
    // Multi-image ICO.
    let mut ico = Vec::new();
    {
        let enc = image::codecs::ico::IcoEncoder::new(std::io::Cursor::new(&mut ico));
        let rgbas: Vec<RgbaImage> = ico_images.iter().map(|i| i.to_rgba8()).collect();
        let items: Vec<image::codecs::ico::IcoFrame> = rgbas
            .iter()
            .map(|r| {
                image::codecs::ico::IcoFrame::as_png(
                    r.as_raw(),
                    r.width(),
                    r.height(),
                    image::ExtendedColorType::Rgba8,
                )
            })
            .collect::<Result<_, _>>()
            .map_err(|e| EngineError::internal(format!("ICO frame failed: {e}")))?;
        enc.encode_images(&items)
            .map_err(|e| EngineError::internal(format!("ICO encoding failed: {e}")))?;
    }
    let apple = square.resize_exact(180, 180, image::imageops::FilterType::Lanczos3);
    let html =
        format!(
        "<link rel=\"icon\" href=\"/favicon.ico\" sizes=\"{}\">\n{}<link rel=\"apple-touch-icon\" \
         href=\"/apple-touch-icon.png\">",
        sizes.iter().map(|s| format!("{s}x{s}")).collect::<Vec<_>>().join(" "),
        sizes.iter().map(|s| format!("<link rel=\"icon\" type=\"image/png\" sizes=\"{s}x{s}\" \
                                      href=\"/favicon-{s}x{s}.png\">\n")).collect::<String>()
    );
    Ok(json!({
        "ico": {
            "file": b64::encode(&ico),
            "mime": "image/x-icon",
            "name": "favicon.ico",
            "bytes": ico.len(),
            "sizes": sizes
        },
        "pngs": pngs,
        "apple_touch_icon": codec::output(&apple, &EncodeOptions::default())?,
        "html": html,
    }))
}

/// Parameters for a social-preview (Open Graph) card.
#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct OgImageOptions {
    /// Card width (default 1200).
    pub width: u32,
    /// Card height (default 630).
    pub height: u32,
    /// Main title.
    pub title: String,
    /// Secondary line.
    pub subtitle: String,
    /// Background colour (or gradient start).
    pub background: String,
    /// Optional second colour for a diagonal gradient.
    pub background_end: Option<String>,
    /// Title colour.
    pub title_color: String,
    /// Subtitle colour.
    pub subtitle_color: String,
    /// Title font size.
    pub title_size: f32,
    /// Subtitle font size.
    pub subtitle_size: f32,
    /// Inner padding in pixels.
    pub padding: u32,
    /// Optional logo / image placed in the top-left (base64).
    pub logo: Option<String>,
    /// Logo height in pixels (width keeps the aspect ratio).
    pub logo_height: u32,
    /// `png` (default), `jpeg` or `webp`.
    pub format: Option<String>,
    /// JPEG quality.
    pub quality: Option<u8>,
}

impl Default for OgImageOptions {
    fn default() -> Self {
        OgImageOptions {
            width: 1200,
            height: 630,
            title: String::new(),
            subtitle: String::new(),
            background: "#0f172a".into(),
            background_end: None,
            title_color: "#ffffff".into(),
            subtitle_color: "#94a3b8".into(),
            title_size: 72.0,
            subtitle_size: 32.0,
            padding: 80,
            logo: None,
            logo_height: 80,
            format: None,
            quality: None,
        }
    }
}

fn wrap_text(text: &str, size: f32, bold: bool, max_w: u32) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.lines() {
        let mut current = String::new();
        for word in para.split_whitespace() {
            let candidate = if current.is_empty() {
                word.to_string()
            } else {
                format!("{current} {word}")
            };
            if measure(&candidate, size, bold).0 <= max_w || current.is_empty() {
                current = candidate;
            } else {
                lines.push(std::mem::take(&mut current));
                current = word.to_string();
            }
        }
        lines.push(current);
    }
    lines
}

/// Generates an Open Graph / social preview image.
pub fn og_image(opts: &OgImageOptions) -> EngineResult<Value> {
    limits::check_dimensions(opts.width, opts.height)?;
    if opts.title.trim().is_empty() && opts.logo.is_none() {
        return Err(EngineError::empty("title"));
    }
    let bg = parse_color(&opts.background)?;
    let mut canvas = RgbaImage::from_pixel(opts.width, opts.height, Rgba(bg));
    if let Some(end) = &opts.background_end {
        let end = parse_color(end)?;
        let (w, h) = (opts.width as f32, opts.height as f32);
        for (x, y, p) in canvas.enumerate_pixels_mut() {
            let t = ((x as f32 / w) + (y as f32 / h)) / 2.0;
            for c in 0..4 {
                p[c] = (bg[c] as f32 + (end[c] as f32 - bg[c] as f32) * t).round() as u8;
            }
        }
    }
    let pad = opts.padding.min(opts.width / 3);
    let mut y = pad as f32;
    if let Some(logo) = &opts.logo {
        let bytes = b64::decode(logo)?;
        let l = codec::decode(&bytes)
            .map_err(|e| EngineError::new(e.code, format!("Logo: {}", e.message)))?;
        // `clamp` panics when min > max, so keep the bounds ordered for tiny canvases.
        let lh = opts.logo_height.max(8).min((opts.height / 2).max(8));
        let lw =
            ((l.image.width() as f32 * lh as f32 / l.image.height() as f32).round() as u32).max(1);
        let resized = l
            .image
            .resize_exact(lw, lh, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        composite(&mut canvas, &resized, pad as i64, y as i64, 1.0);
        y += lh as f32 + pad as f32 * 0.4;
    }
    let max_w = opts.width.saturating_sub(pad * 2).max(10);
    let title_color = parse_color(&opts.title_color)?;
    let title_lines = wrap_text(opts.title.trim(), opts.title_size, true, max_w);
    let sub_color = parse_color(&opts.subtitle_color)?;
    let sub_lines = if opts.subtitle.trim().is_empty() {
        vec![]
    } else {
        wrap_text(opts.subtitle.trim(), opts.subtitle_size, false, max_w)
    };
    let block_h = title_lines.len() as f32 * opts.title_size * 1.2
        + sub_lines.len() as f32 * opts.subtitle_size * 1.4
        + if sub_lines.is_empty() {
            0.0
        } else {
            opts.title_size * 0.4
        };
    // Vertically centre the text block in the remaining space.
    let mut ty = (y + (opts.height as f32 - pad as f32 - y - block_h) / 2.0).max(y);
    for line in &title_lines {
        draw_text(&mut canvas, line, pad as f32, ty, opts.title_size, title_color, true);
        ty += opts.title_size * 1.2;
    }
    ty += opts.title_size * 0.4;
    for line in &sub_lines {
        draw_text(&mut canvas, line, pad as f32, ty, opts.subtitle_size, sub_color, false);
        ty += opts.subtitle_size * 1.4;
    }
    let fmt = match &opts.format {
        Some(f) => OutFormat::parse(f)?,
        None => OutFormat::Png,
    };
    let enc = EncodeOptions {
        format: fmt,
        quality: opts.quality.unwrap_or(90),
        background: [bg[0], bg[1], bg[2]],
        ..Default::default()
    };
    Ok(json!({
        "output": codec::output(&DynamicImage::ImageRgba8(canvas), &enc)?,
        "title_lines": title_lines.len(),
        "subtitle_lines": sub_lines.len()
    }))
}

/// Renders text into a new transparent image (for captions / memes).
pub fn render_text_image(
    text: &str,
    size: f32,
    color: &str,
    bold: bool,
    background: Option<&str>,
    padding: u32,
    max_width: Option<u32>,
) -> EngineResult<Value> {
    engine_shared::error::require_text(text, "text")?;
    engine_shared::error::require_range(size as f64, 4.0, 1024.0, "font_size")?;
    let col = parse_color(color)?;
    let bg = background.map(parse_color).transpose()?.unwrap_or([0, 0, 0, 0]);
    let lines = match max_width {
        Some(mw) if mw > 0 => wrap_text(text, size, bold, mw.saturating_sub(padding * 2).max(1)),
        _ => text.lines().map(str::to_string).collect(),
    };
    let line_h = size * 1.25;
    let w = lines.iter().map(|l| measure(l, size, bold).0).max().unwrap_or(1) + padding * 2;
    let h = (lines.len() as f32 * line_h).ceil() as u32 + padding * 2;
    limits::check_dimensions(w.max(1), h.max(1))?;
    let mut canvas = RgbaImage::from_pixel(w.max(1), h.max(1), Rgba(bg));
    for (i, l) in lines.iter().enumerate() {
        draw_text(
            &mut canvas,
            l,
            padding as f32,
            padding as f32 + i as f32 * line_h,
            size,
            col,
            bold,
        );
    }
    Ok(json!({
        "output": codec::output(&DynamicImage::ImageRgba8(canvas), &EncodeOptions::default())?,
        "lines": lines.len()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{decode_output, png_bytes};

    #[test]
    fn text_rendering() {
        let (w, h) = measure("Hello", 20.0, true);
        assert!(w > 20 && h > 10);
        let mut canvas = RgbaImage::from_pixel(100, 40, Rgba([0, 0, 0, 0]));
        draw_text(&mut canvas, "Hi", 2.0, 2.0, 24.0, [255, 0, 0, 255], true);
        assert!(canvas.pixels().any(|p| p[3] > 0 && p[0] == 255));
        let r = render_text_image(
            "one two three four five six",
            16.0,
            "#000",
            false,
            Some("white"),
            4,
            Some(80),
        )
        .unwrap();
        assert!(r["lines"].as_u64().unwrap() >= 2);
        assert!(render_text_image(" ", 16.0, "#000", false, None, 4, None).is_err());
    }

    #[test]
    fn watermarks_and_overlay() {
        let d = codec::decode(&png_bytes(120, 60)).unwrap();
        let r = watermark_text(
            &d,
            "© Dev",
            &TextWatermarkOptions { font_size: 14.0, ..Default::default() },
            &OutputOptions::default(),
        )
        .unwrap();
        assert_eq!(r["output"]["width"], 120);
        let tiled = TextWatermarkOptions {
            position: Position::Tile,
            rotation: 30.0,
            font_size: 12.0,
            ..Default::default()
        };
        assert!(watermark_text(&d, "DRAFT", &tiled, &OutputOptions::default()).is_ok());
        assert!(watermark_text(
            &d,
            "",
            &TextWatermarkOptions::default(),
            &OutputOptions::default()
        )
        .is_err());
        let logo = png_bytes(10, 10);
        let r = watermark_image(
            &d,
            &logo,
            &ImageWatermarkOptions { scale: 0.5, ..Default::default() },
            &OutputOptions::default(),
        )
        .unwrap();
        assert_eq!(r["overlay_size"]["width"], 60);
        let r = watermark_image(
            &d,
            &logo,
            &ImageWatermarkOptions {
                scale: 0.0,
                x: Some(5),
                y: Some(5),
                ..Default::default()
            },
            &OutputOptions::default(),
        )
        .unwrap();
        assert_eq!(r["overlay_size"]["width"], 10);
        assert!(watermark_image(
            &d,
            b"bad",
            &ImageWatermarkOptions::default(),
            &OutputOptions::default()
        )
        .is_err());
    }

    #[test]
    fn sheets_and_sprites() {
        let imgs = vec![png_bytes(10, 10), png_bytes(20, 10), png_bytes(10, 20)];
        let s = sprite(&imgs, 2, 2, None, &["a".into()]).unwrap();
        assert_eq!(s["rows"], 2);
        assert!(s["css"].as_str().unwrap().contains(".a {"));
        assert_eq!(s["sprites"][1]["x"], 24);
        assert!(sprite(&[], 2, 2, None, &[]).is_err());
        let c = contact_sheet(
            &imgs,
            &GridOptions {
                columns: 2,
                cell_width: 30,
                cell_height: 30,
                padding: 5,
                labels: vec!["one".into(), "two".into()],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(c["output"]["width"], 75);
        assert!(contact_sheet(&imgs, &GridOptions { columns: 0, ..Default::default() }).is_err());
    }

    #[test]
    fn diff_favicon_og() {
        let a = png_bytes(10, 10);
        let mut img = codec::decode(&a).unwrap().image.to_rgba8();
        img.put_pixel(5, 5, Rgba([0, 0, 0, 255]));
        let b = codec::encode(&DynamicImage::ImageRgba8(img), &EncodeOptions::default()).unwrap();
        let d = difference(&a, &b, 0, "red").unwrap();
        assert_eq!(d["changed_pixels"], 1);
        assert_eq!(d["bounding_box"], json!({ "x": 5, "y": 5, "width": 1, "height": 1 }));
        assert_eq!(difference(&a, &a, 0, "red").unwrap()["identical"], true);
        assert!(difference(&a, b"x", 0, "red").is_err());

        let f =
            favicon(&codec::decode(&png_bytes(20, 10)).unwrap(), &[16, 32], Some("white")).unwrap();
        assert_eq!(f["pngs"].as_array().unwrap().len(), 2);
        let ico = b64::decode(f["ico"]["file"].as_str().unwrap()).unwrap();
        assert_eq!(codec::detect_format(&ico).unwrap(), image::ImageFormat::Ico);
        assert!(favicon(&codec::decode(&a).unwrap(), &[4], None).is_err());

        let og = og_image(&OgImageOptions {
            width: 300,
            height: 160,
            title: "Dev Toolbox".into(),
            subtitle: "fast".into(),
            background_end: Some("#1e3a8a".into()),
            title_size: 28.0,
            subtitle_size: 14.0,
            padding: 20,
            logo: Some(b64::encode(&png_bytes(8, 8))),
            logo_height: 16,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(og["output"]["width"], 300);
        let out = decode_output(&og);
        assert!(out.image.to_rgba8().pixels().any(|p| p[0] > 200 && p[1] > 200));
        assert!(og_image(&OgImageOptions { title: "".into(), ..Default::default() }).is_err());
    }
}
