//! Frame-sequence helpers: GIF assembly, sprite/contact sheet and frame
//! timing plans. Frame *extraction* from compressed video needs a decoder and
//! is performed by the browser (WebCodecs / <video> + canvas); the frontend
//! then hands PNG/JPEG frames to these actions.

use engine_shared::{EngineError, EngineResult};
use image::codecs::gif::{GifEncoder, Repeat};
use image::{imageops::FilterType, DynamicImage, Frame, GenericImageView, RgbaImage};
use serde_json::{json, Value};

const MAX_FRAMES: usize = 600;

fn load_frames(frames: &[Vec<u8>]) -> EngineResult<Vec<DynamicImage>> {
    engine_shared::limits::check_count(frames.len(), MAX_FRAMES, "frames")?;
    frames
        .iter()
        .enumerate()
        .map(|(i, f)| {
            if f.is_empty() {
                return Err(EngineError::empty(&format!("frames[{i}]")));
            }
            let img = image::load_from_memory(f).map_err(|e| {
                EngineError::unsupported_format(format!("frames[{i}]: cannot decode image ({e})"))
            })?;
            engine_shared::limits::check_dimensions(img.width(), img.height())?;
            Ok(img)
        })
        .collect()
}

/// Builds an animated GIF from frames (resized to the first frame / target width).
pub fn gif(
    frames: &[Vec<u8>],
    fps: f64,
    width: Option<u32>,
    loop_forever: bool,
    speed: i32,
) -> EngineResult<Value> {
    let imgs = load_frames(frames)?;
    if !(0.5..=50.0).contains(&fps) {
        return Err(EngineError::invalid(
            "fps must be between 0.5 and 50 (GIF timing is in 10 ms units)",
        ));
    }
    let (w0, h0) = imgs[0].dimensions();
    let target_w = width.unwrap_or(w0).clamp(8, 1920);
    let target_h = ((h0 as f64 * target_w as f64 / w0 as f64).round() as u32).max(1);
    let delay_ms = (1000.0 / fps).round() as u32;
    let mut out = Vec::new();
    {
        let mut enc = GifEncoder::new_with_speed(&mut out, speed.clamp(1, 30));
        enc.set_repeat(if loop_forever {
            Repeat::Infinite
        } else {
            Repeat::Finite(0)
        })
        .map_err(|e| EngineError::internal(e.to_string()))?;
        for img in &imgs {
            let resized: RgbaImage = if img.width() != target_w || img.height() != target_h {
                img.resize_exact(target_w, target_h, FilterType::Triangle).to_rgba8()
            } else {
                img.to_rgba8()
            };
            let frame =
                Frame::from_parts(resized, 0, 0, image::Delay::from_numer_denom_ms(delay_ms, 1));
            enc.encode_frame(frame)
                .map_err(|e| EngineError::internal(format!("GIF encode failed: {e}")))?;
        }
    }
    Ok(json!({
        "file": engine_shared::b64::encode(&out),
        "filename": "animation.gif",
        "mime": "image/gif",
        "bytes": out.len(),
        "frames": imgs.len(),
        "width": target_w,
        "height": target_h,
        "fps": fps,
        "duration_seconds": imgs.len() as f64 / fps,
        "looping": loop_forever
    }))
}

/// Arranges frames in a grid (contact sheet / sprite sheet) with optional timestamps.
pub fn sheet(
    frames: &[Vec<u8>],
    columns: u32,
    thumb_width: u32,
    padding: u32,
    background: [u8; 4],
) -> EngineResult<Value> {
    let imgs = load_frames(frames)?;
    let cols = columns.clamp(1, 20);
    let rows = (imgs.len() as u32).div_ceil(cols);
    let tw = thumb_width.clamp(16, 1024);
    let (w0, h0) = imgs[0].dimensions();
    let th = ((h0 as f64 * tw as f64 / w0 as f64).round() as u32).max(1);
    let width = cols * tw + (cols + 1) * padding;
    let height = rows * th + (rows + 1) * padding;
    engine_shared::limits::check_dimensions(width, height)?;
    let mut canvas = RgbaImage::from_pixel(width, height, image::Rgba(background));
    for (i, img) in imgs.iter().enumerate() {
        let thumb = img.resize_exact(tw, th, FilterType::Triangle).to_rgba8();
        let x = padding + (i as u32 % cols) * (tw + padding);
        let y = padding + (i as u32 / cols) * (th + padding);
        image::imageops::overlay(&mut canvas, &thumb, x as i64, y as i64);
    }
    let mut out = std::io::Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(canvas)
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| EngineError::internal(e.to_string()))?;
    let bytes = out.into_inner();
    Ok(json!({
        "file": engine_shared::b64::encode(&bytes),
        "filename": "contact_sheet.png",
        "mime": "image/png",
        "bytes": bytes.len(),
        "columns": cols,
        "rows": rows,
        "thumb": { "width": tw, "height": th },
        "width": width,
        "height": height,
        "cells": (0..imgs.len()).map(|i| json!({ "index": i, "x": padding + (i as u32 % cols) * (tw + padding), "y": padding + (i as u32 / cols) * (th + padding) })).collect::<Vec<_>>()
    }))
}

/// Computes which timestamps the browser should grab for thumbnails/frame export.
pub fn plan(
    duration: f64,
    fps: Option<f64>,
    count: Option<usize>,
    interval: Option<f64>,
    start: f64,
    end: Option<f64>,
) -> EngineResult<Value> {
    if duration <= 0.0 {
        return Err(EngineError::invalid("duration must be > 0"));
    }
    let end = end.unwrap_or(duration).min(duration);
    if start < 0.0 || end <= start {
        return Err(EngineError::invalid("Invalid start/end range"));
    }
    let span = end - start;
    let times: Vec<f64> = if let Some(i) = interval.filter(|i| *i > 0.0) {
        let n = (span / i).floor() as usize + 1;
        if n > 10_000 {
            return Err(EngineError::limit(
                "Plan would exceed 10,000 frames; increase the interval",
            ));
        }
        (0..n).map(|k| start + k as f64 * i).collect()
    } else if let Some(n) = count.filter(|n| *n > 0) {
        if n > 10_000 {
            return Err(EngineError::limit("count is limited to 10,000"));
        }
        (0..n).map(|k| start + span * (k as f64 + 0.5) / n as f64).collect()
    } else if let Some(f) = fps.filter(|f| *f > 0.0) {
        let n = (span * f).floor() as usize;
        if n > 10_000 {
            return Err(EngineError::limit("Plan would exceed 10,000 frames; lower the fps"));
        }
        (0..n).map(|k| start + k as f64 / f).collect()
    } else {
        (0..10).map(|k| start + span * (k as f64 + 0.5) / 10.0).collect()
    };
    Ok(json!({
        "timestamps": times.iter().map(|t| (t * 1000.0).round() / 1000.0).collect::<Vec<_>>(),
        "count": times.len(),
        "timecodes": times.iter().map(|t| crate::inspect::hms(*t)).collect::<Vec<_>>(),
        "start": start,
        "end": end,
        "how": "Seek a <video> element to each timestamp, draw to a canvas, export PNG/JPEG, then \
                call gif_from_frames / contact_sheet"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(color: [u8; 4], w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, image::Rgba(color));
        let mut out = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn gif_and_sheet() {
        let frames = vec![
            png([255, 0, 0, 255], 40, 30),
            png([0, 255, 0, 255], 40, 30),
            png([0, 0, 255, 255], 80, 60),
        ];
        let g = gif(&frames, 10.0, Some(32), true, 10).unwrap();
        assert_eq!(g["frames"], 3);
        assert_eq!(g["width"], 32);
        assert_eq!(g["height"], 24);
        let bytes = engine_shared::b64::decode(g["file"].as_str().unwrap()).unwrap();
        assert!(bytes.starts_with(b"GIF89a"));
        let s = sheet(&frames, 2, 40, 4, [0, 0, 0, 255]).unwrap();
        assert_eq!(s["rows"], 2);
        assert_eq!(s["width"], 2 * 40 + 3 * 4);
        assert!(gif(&[], 10.0, None, true, 10).is_err());
        assert!(gif(&[b"nope".to_vec()], 10.0, None, true, 10).is_err());
        assert!(gif(&frames, 100.0, None, true, 10).is_err());
        let p = plan(60.0, None, Some(6), None, 0.0, None).unwrap();
        assert_eq!(p["timestamps"][0], 5.0);
        assert_eq!(plan(10.0, None, None, Some(2.5), 0.0, None).unwrap()["count"], 5);
        assert!(plan(0.0, None, None, None, 0.0, None).is_err());
    }
}
