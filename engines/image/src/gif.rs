//! Animated GIF support: frame extraction and resizing.

use crate::codec::{self, EncodeOptions};
use engine_shared::{limits, EngineError, EngineResult};
use image::codecs::gif::{GifDecoder, GifEncoder, Repeat};
use image::{AnimationDecoder, Delay, DynamicImage, Frame};
use serde_json::{json, Value};
use std::io::Cursor;

fn frames_of(bytes: &[u8]) -> EngineResult<Vec<Frame>> {
    if bytes.is_empty() {
        return Err(EngineError::empty("gif file"));
    }
    if codec::detect_format(bytes)? != image::ImageFormat::Gif {
        return Err(EngineError::unsupported_format("The file is not a GIF"));
    }
    let decoder = GifDecoder::new(Cursor::new(bytes))
        .map_err(|e| EngineError::malformed(format!("GIF could not be decoded: {e}")))?;
    let frames: Vec<Frame> = decoder
        .into_frames()
        .take(limits::MAX_BATCH_FILES * 4)
        .collect::<Result<_, _>>()
        .map_err(|e| EngineError::malformed(format!("GIF frame error: {e}")))?;
    if frames.is_empty() {
        return Err(EngineError::malformed("GIF contains no frames"));
    }
    Ok(frames)
}

/// Counts the frames of a GIF without decoding pixel data.
pub fn frame_count(bytes: &[u8]) -> EngineResult<usize> {
    Ok(frames_of(bytes)?.len())
}

/// Extracts frames as PNG (optionally every `step`-th frame, max `limit`).
pub fn extract_frames(bytes: &[u8], step: usize, limit: usize) -> EngineResult<Value> {
    let frames = frames_of(bytes)?;
    let step = step.max(1);
    let limit = if limit == 0 { 200 } else { limit.min(500) };
    let mut out = Vec::new();
    let mut total_ms = 0u64;
    for (i, f) in frames.iter().enumerate() {
        let (num, den) = f.delay().numer_denom_ms();
        let delay_ms = if den == 0 { 0 } else { num as u64 / den as u64 };
        total_ms += delay_ms;
        if i % step != 0 || out.len() >= limit {
            continue;
        }
        let img = DynamicImage::ImageRgba8(f.buffer().clone());
        out.push(json!({
            "index": i,
            "delay_ms": delay_ms,
            "output": codec::output(&img, &EncodeOptions::default())?
        }));
    }
    Ok(json!({
        "frames": out,
        "total_frames": frames.len(),
        "extracted": out.len(),
        "duration_ms": total_ms,
        "width": frames[0].buffer().width(),
        "height": frames[0].buffer().height()
    }))
}

/// Resizes (and optionally speeds up) an animated GIF, preserving timing.
pub fn resize_gif(
    bytes: &[u8],
    width: Option<u32>,
    height: Option<u32>,
    speed: f64,
    drop_every: usize,
) -> EngineResult<Value> {
    let frames = frames_of(bytes)?;
    engine_shared::error::require_range(speed, 0.1, 10.0, "speed")?;
    let (sw, sh) = (frames[0].buffer().width(), frames[0].buffer().height());
    let (tw, th) = match (width, height) {
        (None, None) => (sw, sh),
        (Some(w), None) => (w.max(1), ((sh as f64 * w as f64 / sw as f64).round() as u32).max(1)),
        (None, Some(h)) => (((sw as f64 * h as f64 / sh as f64).round() as u32).max(1), h.max(1)),
        (Some(w), Some(h)) => (w.max(1), h.max(1)),
    };
    limits::check_dimensions(tw, th)?;
    let mut out = Vec::new();
    {
        let mut enc = GifEncoder::new_with_speed(Cursor::new(&mut out), 10);
        enc.set_repeat(Repeat::Infinite)
            .map_err(|e| EngineError::internal(e.to_string()))?;
        let mut kept = 0usize;
        for (i, f) in frames.iter().enumerate() {
            if drop_every > 1 && i % drop_every == drop_every - 1 {
                continue;
            }
            let resized = DynamicImage::ImageRgba8(f.buffer().clone())
                .resize_exact(tw, th, image::imageops::FilterType::Triangle)
                .to_rgba8();
            let (num, den) = f.delay().numer_denom_ms();
            let ms = if den == 0 { 100.0 } else { num as f64 / den as f64 } / speed;
            let delay = Delay::from_numer_denom_ms(ms.round().max(10.0) as u32, 1);
            enc.encode_frame(Frame::from_parts(resized, 0, 0, delay))
                .map_err(|e| EngineError::internal(format!("GIF encoding failed: {e}")))?;
            kept += 1;
        }
        if kept == 0 {
            return Err(EngineError::invalid("All frames were dropped"));
        }
    }
    Ok(json!({
        "output": codec::ImageOutput::new(out.clone(), codec::OutFormat::Gif, tw, th),
        "frames": frames.len(),
        "bytes_before": bytes.len(),
        "bytes_after": out.len(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{gif_bytes, png_bytes};

    #[test]
    fn frames_and_resize() {
        let g = gif_bytes(3, 12, 8);
        assert_eq!(frame_count(&g).unwrap(), 3);
        let f = extract_frames(&g, 1, 0).unwrap();
        assert_eq!(f["extracted"], 3);
        assert_eq!(f["frames"][0]["output"]["mime"], "image/png");
        let every2 = extract_frames(&g, 2, 0).unwrap();
        assert_eq!(every2["extracted"], 2);
        let r = resize_gif(&g, Some(6), None, 2.0, 0).unwrap();
        assert_eq!(r["output"]["width"], 6);
        assert_eq!(
            frame_count(
                &engine_shared::b64::decode(r["output"]["file"].as_str().unwrap()).unwrap()
            )
            .unwrap(),
            3
        );
        assert!(resize_gif(&png_bytes(4, 4), None, None, 1.0, 0).is_err());
        assert!(extract_frames(b"", 1, 0).is_err());
    }
}
