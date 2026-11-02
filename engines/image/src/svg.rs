//! SVG rasterisation and optimisation via resvg/usvg.
//!
//! Text elements are not rendered (no font database is bundled to keep the
//! binary small); the response reports `text_elements` so the UI can warn.

use crate::codec::{self, parse_color, EncodeOptions, OutFormat};
use engine_shared::{limits, EngineError, EngineResult};
use image::{DynamicImage, RgbaImage};
use resvg::usvg;
use serde_json::{json, Value};

fn parse_tree(svg: &str) -> EngineResult<usvg::Tree> {
    if svg.trim().is_empty() {
        return Err(EngineError::empty("svg"));
    }
    limits::check_size(svg.len(), 32 * 1024 * 1024, "svg")?;
    let opts = usvg::Options::default();
    usvg::Tree::from_str(svg, &opts)
        .map_err(|e| EngineError::malformed(format!("SVG could not be parsed: {e}")))
}

fn count_text(svg: &str) -> usize {
    svg.matches("<text").count()
}

/// Rasterises an SVG. Either `width`/`height` (aspect preserved when one is
/// omitted) or `scale` selects the output size; defaults to the intrinsic size.
pub fn to_raster(
    svg: &str,
    width: Option<u32>,
    height: Option<u32>,
    scale: Option<f64>,
    background: Option<&str>,
    format: Option<&str>,
    quality: Option<u8>,
) -> EngineResult<Value> {
    let tree = parse_tree(svg)?;
    let size = tree.size();
    let (iw, ih) = (size.width() as f64, size.height() as f64);
    if iw <= 0.0 || ih <= 0.0 {
        return Err(EngineError::invalid(
            "SVG has no intrinsic size; set width/height or a viewBox",
        ));
    }
    let (tw, th) = if let Some(s) = scale {
        engine_shared::error::require_range(s, 0.01, 100.0, "scale")?;
        ((iw * s).round() as u32, (ih * s).round() as u32)
    } else {
        match (width, height) {
            (None, None) => (iw.round() as u32, ih.round() as u32),
            (Some(w), None) => (w, (ih * w as f64 / iw).round() as u32),
            (None, Some(h)) => ((iw * h as f64 / ih).round() as u32, h),
            (Some(w), Some(h)) => (w, h),
        }
    };
    let (tw, th) = (tw.max(1), th.max(1));
    limits::check_dimensions(tw, th)?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(tw, th)
        .ok_or_else(|| EngineError::limit("Raster target is too large"))?;
    if let Some(bg) = background {
        let c = parse_color(bg)?;
        pixmap.fill(resvg::tiny_skia::Color::from_rgba8(c[0], c[1], c[2], c[3]));
    }
    let transform =
        resvg::tiny_skia::Transform::from_scale(tw as f32 / iw as f32, th as f32 / ih as f32);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    // tiny-skia stores premultiplied RGBA; un-premultiply.
    let mut rgba = RgbaImage::new(tw, th);
    for (i, px) in pixmap.pixels().iter().enumerate() {
        let c = px.demultiply();
        let x = (i as u32) % tw;
        let y = (i as u32) / tw;
        rgba.put_pixel(x, y, image::Rgba([c.red(), c.green(), c.blue(), c.alpha()]));
    }
    let fmt = match format {
        Some(f) => OutFormat::parse(f)?,
        None => OutFormat::Png,
    };
    let bg = background
        .map(parse_color)
        .transpose()?
        .unwrap_or([255, 255, 255, 255]);
    let enc = EncodeOptions {
        format: fmt,
        quality: quality.unwrap_or(90),
        background: [bg[0], bg[1], bg[2]],
        ..Default::default()
    };
    let text_elements = count_text(svg);
    Ok(json!({
        "output": codec::output(&DynamicImage::ImageRgba8(rgba), &enc)?,
        "intrinsic": { "width": iw, "height": ih },
        "text_elements": text_elements,
        "warnings": if text_elements > 0 { vec!["<text> elements are not rendered by the WASM \
                                                 engine (no font database); convert text to paths \
                                                 first."] } else { vec![] },
    }))
}

/// Optimises/normalises an SVG by re-serialising the usvg tree
/// (resolves CSS/uses, drops metadata/editor data, rounds coordinates).
pub fn optimize(svg: &str, precision: u8) -> EngineResult<Value> {
    let tree = parse_tree(svg)?;
    let opts = usvg::WriteOptions {
        coordinates_precision: precision.clamp(1, 8),
        transforms_precision: precision.clamp(1, 8),
        ..Default::default()
    };
    let out = tree.to_string(&opts);
    let text_elements = count_text(svg);
    Ok(json!({
        "svg": out,
        "bytes_before": svg.len(),
        "bytes_after": out.len(),
        "saved_percent": if !svg.is_empty() { ((svg.len() as f64 - out.len() as f64) / svg.len() as f64 * 1000.0).round() / 10.0 } else { 0.0 },
        "size": { "width": tree.size().width(), "height": tree.size().height() },
        "warnings": if text_elements > 0 { vec!["<text> elements were converted/dropped: text \
                                                 shaping is unavailable in the WASM engine."] } else { vec![] },
    }))
}

/// Basic SVG information (size, viewBox, element counts).
pub fn info(svg: &str) -> EngineResult<Value> {
    let tree = parse_tree(svg)?;
    let count = |tag: &str| svg.matches(&format!("<{tag}")).count();
    Ok(json!({
        "width": tree.size().width(),
        "height": tree.size().height(),
        "elements": {
            "path": count("path"),
            "rect": count("rect"),
            "circle": count("circle"),
            "ellipse": count("ellipse"),
            "line": count("line"),
            "polygon": count("polygon"),
            "polyline": count("polyline"),
            "text": count("text"),
            "image": count("image"),
            "g": count("g"),
            "use": count("use")
        },
        "bytes": svg.len(),
        "has_scripts": svg.contains("<script"),
        "has_external_refs": svg.contains("xlink:href=\"http") || svg.contains("href=\"http"),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50" viewBox="0 0 100 50"><!-- comment --><rect x="0" y="0" width="50" height="50" fill="#ff0000"/><circle cx="75" cy="25" r="20" fill="blue"/></svg>"##;

    #[test]
    fn rasterise() {
        let r = to_raster(SVG, Some(200), None, None, None, None, None).unwrap();
        assert_eq!(r["output"]["width"], 200);
        assert_eq!(r["output"]["height"], 100);
        let out = crate::testutil::decode_output(&r).image.to_rgba8();
        assert_eq!(out.get_pixel(10, 10).0, [255, 0, 0, 255]);
        assert_eq!(out.get_pixel(190, 5)[3], 0);
        let jpg =
            to_raster(SVG, None, None, Some(0.5), Some("white"), Some("jpeg"), Some(80)).unwrap();
        assert_eq!(jpg["output"]["mime"], "image/jpeg");
        assert_eq!(jpg["output"]["width"], 50);
        assert!(to_raster("", None, None, None, None, None, None).is_err());
        assert!(to_raster("<svg><rect", None, None, None, None, None, None).is_err());
        assert!(to_raster("not svg at all", None, None, None, None, None, None).is_err());
    }

    #[test]
    fn optimise_and_info() {
        let o = optimize(SVG, 2).unwrap();
        assert!(!o["svg"].as_str().unwrap().contains("comment"));
        assert!(o["bytes_after"].as_u64().unwrap() > 0);
        let i = info(SVG).unwrap();
        assert_eq!(i["elements"]["rect"], 1);
        assert_eq!(i["width"], 100.0);
    }
}
