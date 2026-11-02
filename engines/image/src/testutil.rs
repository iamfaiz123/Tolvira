//! Test fixtures (compiled for tests / the `testutil` feature).
//!
//! Only compiled for tests (`cfg(test)` / `testutil` feature), so test-style
//! `unwrap` is acceptable here.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::codec::{self, Decoded};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
use serde_json::Value;

/// RGB gradient PNG.
pub fn png_bytes(w: u32, h: u32) -> Vec<u8> {
    let img = RgbImage::from_fn(w, h, |x, y| {
        Rgb([(x * 255 / w.max(1)) as u8, (y * 255 / h.max(1)) as u8, 128])
    });
    let mut out = Vec::new();
    DynamicImage::ImageRgb8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), ImageFormat::Png)
        .unwrap();
    out
}

/// Baseline JPEG.
pub fn jpeg_bytes(w: u32, h: u32) -> Vec<u8> {
    let img = RgbImage::from_fn(w, h, |x, _| Rgb([(x * 255 / w.max(1)) as u8, 50, 200]));
    let mut out = Vec::new();
    DynamicImage::ImageRgb8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), ImageFormat::Jpeg)
        .unwrap();
    out
}

/// Animated GIF with `frames` frames.
pub fn gif_bytes(frames: u32, w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = image::codecs::gif::GifEncoder::new(std::io::Cursor::new(&mut out));
        enc.set_repeat(image::codecs::gif::Repeat::Infinite).unwrap();
        for i in 0..frames {
            let img = RgbaImage::from_fn(w, h, |x, _| Rgba([(x * 20 + i * 60) as u8, 0, 0, 255]));
            enc.encode_frame(image::Frame::from_parts(
                img,
                0,
                0,
                image::Delay::from_numer_denom_ms(100, 1),
            ))
            .unwrap();
        }
    }
    out
}

/// Decodes `output.file` from an action response.
pub fn decode_output(v: &Value) -> Decoded {
    let bytes =
        engine_shared::b64::decode(v["output"]["file"].as_str().expect("output.file")).unwrap();
    codec::decode(&bytes).unwrap()
}
