//! # Dev Toolbox Image Engine (`image-engine`)
//!
//! Pure Rust raster/SVG processing for the browser. Files travel as base64
//! in the JSON envelope (`file`, `files`, `overlay`, ...). Most actions accept
//! the shared output options `format`, `quality`, `png_level`, `background`.
//!
//! | Action | Tool(s) |
//! |---|---|
//! | `info` | image-dimensions, image-metadata-viewer, exif viewer |
//! | `resize` / `crop` / `crop_ratio` / `rotate` / `flip` | geometry |
//! | `convert` / `compress` | format conversion & optimisation (PNG/JPEG/WebP/GIF/BMP/ICO/TIFF) |
//! | `adjust` | grayscale, invert, sepia, blur, sharpen, brightness, contrast, saturation, hue, exposure, temperature, gamma, pixelate, denoise, threshold, posterize, vignette, opacity |
//! | `add_border` / `trim_border` / `round_corners` / `fit_print_size` | framing |
//! | `watermark_text` / `watermark_image` / `render_text` | watermarks & captions |
//! | `strip_metadata` / `set_dpi` / `to_data_url` / `from_data_url` | metadata & encoding |
//! | `palette` / `dominant_color` / `color_at` / `to_ascii` / `pixel_art` / `remove_background` | analysis & effects |
//! | `svg_to_raster` / `svg_optimize` / `svg_info` | SVG |
//! | `gif_frames` / `gif_resize` | animated GIF |
//! | `sprite` / `contact_sheet` / `difference` / `favicon` / `og_image` | composition |
//! | `aspect_ratio` / `calculate_crop` / `preset_crop` / `grayscale_filter` | legacy (v1 UI) |

pub mod codec;
pub mod colors;
pub mod compose;
pub mod filters;
pub mod gif;
pub mod meta;
pub mod svg;
#[cfg(any(test, feature = "testutil"))]
pub mod testutil;
pub mod transform;

use codec::{decode, EncodeOptions, OutFormat};
use engine_shared::{b64, EngineError, EngineResult};
use serde::Deserialize;
use serde_json::{json, Value};
use transform::OutputOptions;

fn default_fit() -> String {
    "inside".into()
}
fn default_true() -> bool {
    true
}
fn default_tolerance() -> u8 {
    10
}
fn default_red() -> String {
    "#ff0000".into()
}
fn default_columns() -> u32 {
    80
}
fn default_sample() -> u32 {
    1
}
fn default_pixel_size() -> u32 {
    8
}
fn default_colors() -> usize {
    16
}
fn default_palette_count() -> usize {
    8
}
fn default_precision() -> u8 {
    3
}
fn default_speed() -> f64 {
    1.0
}
fn default_mode_cover() -> String {
    "cover".into()
}
fn default_unit_mm() -> String {
    "mm".into()
}
fn default_dpi() -> f64 {
    300.0
}

/// Every action understood by the image engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum ImageAction {
    // ----- legacy (v1) ----------------------------------------------------
    /// Legacy v1: reports the pixel count for a grayscale preview (kept for the cropper UI).
    GrayscaleFilter {
        /// Image width.
        width: u32,
        /// Image height.
        height: u32,
    },
    /// Legacy v1: reduced aspect ratio for a size.
    AspectRatio {
        /// Image width.
        width: u32,
        /// Image height.
        height: u32,
    },
    /// Legacy v1: clamps a crop rectangle to the image.
    CalculateCrop {
        /// Original width.
        orig_width: u32,
        /// Original height.
        orig_height: u32,
        /// Crop left.
        crop_x: u32,
        /// Crop top.
        crop_y: u32,
        /// Crop width.
        crop_width: u32,
        /// Crop height.
        crop_height: u32,
    },
    /// Legacy v1: largest centred crop for a named preset.
    PresetCrop {
        /// Original width.
        orig_width: u32,
        /// Original height.
        orig_height: u32,
        /// Preset name (`square`, `16:9`, `4:3`, `instagram`, …).
        preset: String,
    },

    // ----- inspection -----------------------------------------------------
    /// Dimensions, format, colour type, EXIF/ICC presence, estimated memory.
    Info {
        /// Image bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
    },

    // ----- geometry -------------------------------------------------------
    /// Resizes by explicit size, scale factor or fit mode.
    Resize {
        /// Image bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Target width (keeps aspect ratio when `height` is omitted).
        #[serde(default)]
        width: Option<u32>,
        /// Target height (keeps aspect ratio when `width` is omitted).
        #[serde(default)]
        height: Option<u32>,
        /// Scale factor (alternative to width/height).
        #[serde(default)]
        scale: Option<f64>,
        /// `inside`/`contain`/`fit` (default), `cover` or `fill`/`stretch`/`exact`.
        #[serde(default = "default_fit")]
        fit: String,
        /// Resampling filter (`lanczos3` default, `triangle`, `catmullrom`, `gaussian`, `nearest`).
        #[serde(default)]
        filter: Option<String>,
        /// Allow the output to be larger than the input.
        #[serde(default = "default_true")]
        allow_upscale: bool,
        /// Output format/quality.
        #[serde(flatten)]
        out: OutputOptions,
    },
    /// Crops a rectangle.
    Crop {
        /// Image bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Left edge.
        x: u32,
        /// Top edge.
        y: u32,
        /// Crop width.
        width: u32,
        /// Crop height.
        height: u32,
        /// Output format/quality.
        #[serde(flatten)]
        out: OutputOptions,
    },
    /// Crops to an aspect ratio with gravity.
    CropRatio {
        /// Image bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Ratio such as `16:9` or `1.5`.
        ratio: String,
        /// `center` (default), `top`, `bottom`, `left`, `right`, `top-left`…
        #[serde(default)]
        gravity: String,
        /// Output format/quality.
        #[serde(flatten)]
        out: OutputOptions,
    },
    /// Rotates by an arbitrary angle (90° multiples are lossless).
    Rotate {
        /// Image bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Angle in degrees (clockwise).
        angle: f64,
        /// Grow the canvas so nothing is cut off.
        #[serde(default = "default_true")]
        expand: bool,
        /// Output format/quality.
        #[serde(flatten)]
        out: OutputOptions,
    },
    /// Mirrors horizontally or vertically.
    Flip {
        /// Image bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// `horizontal` or `vertical`.
        direction: String,
        /// Output format/quality.
        #[serde(flatten)]
        out: OutputOptions,
    },

    // ----- encoding -------------------------------------------------------
    /// Re-encodes in another format.
    Convert {
        /// Image bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Target format name.
        format: String,
        /// JPEG quality 1–100.
        #[serde(default)]
        quality: Option<u8>,
        /// PNG compression 0–2.
        #[serde(default)]
        png_level: Option<u8>,
        /// Background used when alpha is dropped.
        #[serde(default)]
        background: Option<String>,
    },
    /// Reduces file size (quality, downscale, optional target size).
    Compress {
        /// Image bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// JPEG/WebP quality.
        #[serde(default)]
        quality: Option<u8>,
        /// Downscale so width ≤ this.
        #[serde(default)]
        max_width: Option<u32>,
        /// Downscale so height ≤ this.
        #[serde(default)]
        max_height: Option<u32>,
        /// `jpeg`, `webp` or `png` (default: the input format).
        #[serde(default)]
        format: Option<String>,
        /// Iteratively lowers quality until the output is below this many KiB.
        #[serde(default)]
        target_kb: Option<u32>,
    },

    // ----- filters --------------------------------------------------------
    /// Applies a chain of colour/filter adjustments.
    Adjust {
        /// Image bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Adjustment steps in order.
        adjustments: Vec<filters::Adjustment>,
        /// Output format/quality.
        #[serde(flatten)]
        out: OutputOptions,
    },
    /// Pixel-art effect with palette reduction.
    PixelArt {
        /// Image bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Size of each output pixel block.
        #[serde(default = "default_pixel_size")]
        pixel_size: u32,
        /// Palette size.
        #[serde(default = "default_colors")]
        colors: usize,
        /// Output format/quality.
        #[serde(flatten)]
        out: OutputOptions,
    },
    /// Makes a (near-)uniform background colour transparent.
    RemoveBackground {
        /// Image bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Background colour to remove (default: sampled from the corners).
        #[serde(default)]
        color: Option<String>,
        /// Colour distance tolerance 0–255.
        #[serde(default = "default_tolerance")]
        tolerance: u8,
        /// Soften the mask edge.
        #[serde(default = "default_true")]
        feather: bool,
        /// Output format/quality.
        #[serde(flatten)]
        out: OutputOptions,
    },

    // ----- framing --------------------------------------------------------
    /// Adds a solid border.
    AddBorder {
        /// Image bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Border size on every side.
        #[serde(default)]
        size: u32,
        /// Border colour.
        #[serde(default = "default_black")]
        color: String,
        /// Top override.
        #[serde(default)]
        top: Option<u32>,
        /// Right override.
        #[serde(default)]
        right: Option<u32>,
        /// Bottom override.
        #[serde(default)]
        bottom: Option<u32>,
        /// Left override.
        #[serde(default)]
        left: Option<u32>,
        /// Output format/quality.
        #[serde(flatten)]
        out: OutputOptions,
    },
    /// Auto-crops uniform borders.
    TrimBorder {
        /// Image bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Colour tolerance 0–255.
        #[serde(default = "default_tolerance")]
        tolerance: u8,
        /// Output format/quality.
        #[serde(flatten)]
        out: OutputOptions,
    },
    /// Rounds the corners (transparent outside).
    RoundCorners {
        /// Image bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Radius in pixels or percent (`20`, `50%`).
        radius: String,
        /// Output format/quality.
        #[serde(flatten)]
        out: OutputOptions,
    },
    /// Resizes/pads to a physical print size at a DPI.
    FitPrintSize {
        /// Image bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Print width.
        width: f64,
        /// Print height.
        height: f64,
        /// `mm`, `cm` or `in`.
        #[serde(default = "default_unit_mm")]
        unit: String,
        /// Dots per inch.
        #[serde(default = "default_dpi")]
        dpi: f64,
        /// `fit`, `fill` or `stretch`.
        #[serde(default = "default_mode_cover")]
        mode: String,
        /// Output format/quality.
        #[serde(flatten)]
        out: OutputOptions,
    },

    // ----- watermarks & text ---------------------------------------------
    /// Draws a text watermark.
    WatermarkText {
        /// Image bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Watermark text.
        text: String,
        /// Font size, colour, opacity, position.
        #[serde(flatten)]
        options: compose::TextWatermarkOptions,
        /// Output format/quality.
        #[serde(flatten)]
        out: OutputOptions,
    },
    /// Overlays an image watermark.
    WatermarkImage {
        /// Image bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Watermark image (base64).
        #[serde(with = "b64")]
        overlay: Vec<u8>,
        /// Opacity, position, scale.
        #[serde(flatten)]
        options: compose::ImageWatermarkOptions,
        /// Output format/quality.
        #[serde(flatten)]
        out: OutputOptions,
    },
    /// Renders text to a PNG.
    RenderText {
        /// Text (may contain newlines).
        text: String,
        /// Font size in pixels.
        #[serde(default = "default_text_size")]
        font_size: f32,
        /// Text colour.
        #[serde(default = "default_black")]
        color: String,
        /// Use the bold face.
        #[serde(default)]
        bold: bool,
        /// Background colour (transparent when omitted).
        #[serde(default)]
        background: Option<String>,
        /// Padding in pixels.
        #[serde(default = "default_padding")]
        padding: u32,
        /// Wrap text at this width.
        #[serde(default)]
        max_width: Option<u32>,
    },

    // ----- metadata -------------------------------------------------------
    /// Removes EXIF/XMP/ICC metadata losslessly (JPEG/PNG).
    StripMetadata {
        /// Image bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Keep the ICC colour profile.
        #[serde(default)]
        keep_icc: bool,
    },
    /// Sets the DPI metadata (JPEG JFIF / PNG pHYs).
    SetDpi {
        /// Image bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Dots per inch.
        dpi: f64,
    },
    /// Wraps the bytes in a `data:` URL.
    ToDataUrl {
        /// Image bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
    },
    /// Parses a `data:` URL back into bytes and metadata.
    FromDataUrl {
        /// Data URL.
        data: String,
    },

    // ----- analysis -------------------------------------------------------
    /// Extracts a dominant-colour palette (median cut).
    Palette {
        /// Image bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Number of colours.
        #[serde(default = "default_palette_count")]
        count: usize,
    },
    /// Single most dominant colour.
    DominantColor {
        /// Image bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
    },
    /// Colour at a pixel (optionally averaged over a square).
    ColorAt {
        /// Image bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Pixel x.
        x: u32,
        /// Pixel y.
        y: u32,
        /// Sample square size (1 = exact pixel).
        #[serde(default = "default_sample")]
        sample: u32,
    },
    /// ASCII-art rendering.
    ToAscii {
        /// Image bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Output width in characters.
        #[serde(default = "default_columns")]
        columns: u32,
        /// Character ramp (`standard`, `blocks`, or custom string).
        #[serde(default)]
        charset: String,
        /// Invert brightness mapping.
        #[serde(default)]
        invert: bool,
    },

    // ----- svg ------------------------------------------------------------
    /// Rasterises SVG (resvg).
    SvgToRaster {
        /// SVG source.
        svg: String,
        /// Output width.
        #[serde(default)]
        width: Option<u32>,
        /// Output height.
        #[serde(default)]
        height: Option<u32>,
        /// Scale factor.
        #[serde(default)]
        scale: Option<f64>,
        /// Background colour (transparent when omitted).
        #[serde(default)]
        background: Option<String>,
        /// `png` (default), `jpeg`, `webp`, `gif` or `bmp`.
        #[serde(default)]
        format: Option<String>,
        /// JPEG quality.
        #[serde(default)]
        quality: Option<u8>,
    },
    /// Minifies SVG (whitespace, comments, number precision).
    SvgOptimize {
        /// SVG source.
        svg: String,
        /// Decimal places kept for numbers.
        #[serde(default = "default_precision")]
        precision: u8,
    },
    /// Reports SVG size, viewBox and element counts.
    SvgInfo {
        /// SVG source.
        svg: String,
    },

    // ----- gif ------------------------------------------------------------
    /// Extracts GIF frames as PNGs.
    GifFrames {
        /// GIF bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Keep every n-th frame.
        #[serde(default)]
        step: usize,
        /// Maximum frames returned.
        #[serde(default)]
        limit: usize,
    },
    /// Resizes/retimes an animated GIF.
    GifResize {
        /// GIF bytes (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// New width.
        #[serde(default)]
        width: Option<u32>,
        /// New height.
        #[serde(default)]
        height: Option<u32>,
        /// Playback speed multiplier.
        #[serde(default = "default_speed")]
        speed: f64,
        /// Drop every n-th frame (0 = keep all).
        #[serde(default)]
        drop_every: usize,
    },

    // ----- composition ----------------------------------------------------
    /// Packs images into a sprite sheet with a CSS map.
    Sprite {
        /// Images (base64).
        #[serde(with = "b64::list")]
        files: Vec<Vec<u8>>,
        /// Columns in the sheet.
        #[serde(default = "default_sprite_columns")]
        columns: u32,
        /// Gap between sprites.
        #[serde(default)]
        padding: u32,
        /// Background colour (transparent when omitted).
        #[serde(default)]
        background: Option<String>,
        /// CSS class name per image.
        #[serde(default)]
        names: Vec<String>,
    },
    /// Grid of thumbnails with captions.
    ContactSheet {
        /// Images (base64).
        #[serde(with = "b64::list")]
        files: Vec<Vec<u8>>,
        /// Grid layout options.
        #[serde(flatten)]
        options: compose::GridOptions,
    },
    /// Pixel diff of two images with a highlighted mask.
    Difference {
        /// First image (base64).
        #[serde(with = "b64")]
        file_a: Vec<u8>,
        /// Second image (base64).
        #[serde(with = "b64")]
        file_b: Vec<u8>,
        /// Per-channel difference threshold.
        #[serde(default)]
        threshold: u8,
        /// Highlight colour for differing pixels.
        #[serde(default = "default_red")]
        highlight: String,
    },
    /// Generates favicon PNGs and a multi-size ICO.
    Favicon {
        /// Source image (base64).
        #[serde(with = "b64")]
        file: Vec<u8>,
        /// Icon sizes in pixels.
        #[serde(default)]
        sizes: Vec<u32>,
        /// Background colour used to flatten alpha for ICO.
        #[serde(default)]
        background: Option<String>,
    },
    /// Renders a social-preview card.
    OgImage {
        /// Card layout options.
        #[serde(flatten)]
        options: compose::OgImageOptions,
    },
}

fn default_black() -> String {
    "#000000".into()
}
fn default_text_size() -> f32 {
    32.0
}
fn default_padding() -> u32 {
    8
}
fn default_sprite_columns() -> u32 {
    4
}

/// Dispatches an [`ImageAction`] to its implementation.
pub fn handle_action(action: ImageAction) -> EngineResult<Value> {
    match action {
        ImageAction::GrayscaleFilter { width, height } => {
            Ok(json!({ "pixels": width as u64 * height as u64, "supported": true }))
        },
        ImageAction::AspectRatio { width, height } => colors::legacy::aspect_ratio(width, height),
        ImageAction::CalculateCrop {
            orig_width,
            orig_height,
            crop_x,
            crop_y,
            crop_width,
            crop_height,
        } => colors::legacy::calculate_crop(
            orig_width,
            orig_height,
            crop_x,
            crop_y,
            crop_width,
            crop_height,
        ),
        ImageAction::PresetCrop { orig_width, orig_height, preset } => {
            colors::legacy::preset_crop(orig_width, orig_height, &preset)
        },

        ImageAction::Info { file } => codec::inspect(&file),

        ImageAction::Resize {
            file,
            width,
            height,
            scale,
            fit,
            filter,
            allow_upscale,
            out,
        } => transform::resize(
            &decode(&file)?,
            width,
            height,
            scale,
            &fit,
            filter.as_deref(),
            allow_upscale,
            &out,
        ),
        ImageAction::Crop { file, x, y, width, height, out } => {
            transform::crop(&decode(&file)?, x, y, width, height, &out)
        },
        ImageAction::CropRatio { file, ratio, gravity, out } => {
            transform::crop_ratio(&decode(&file)?, &ratio, &gravity, &out)
        },
        ImageAction::Rotate { file, angle, expand, out } => {
            transform::rotate(&decode(&file)?, angle, expand, None, &out)
        },
        ImageAction::Flip { file, direction, out } => {
            transform::flip(&decode(&file)?, &direction, &out)
        },

        ImageAction::Convert {
            file,
            format,
            quality,
            png_level,
            background,
        } => {
            let d = decode(&file)?;
            let out = OutputOptions {
                format: Some(format),
                quality,
                png_level,
                background,
            };
            let opts = out.encode_options(d.format)?;
            let o = codec::output(&d.image, &opts)?;
            Ok(json!({
                "output": o,
                "source_format": codec::format_name(d.format),
                "bytes_before": file.len()
            }))
        },
        ImageAction::Compress {
            file,
            quality,
            max_width,
            max_height,
            format,
            target_kb,
        } => compress(&file, quality, max_width, max_height, format.as_deref(), target_kb),

        ImageAction::Adjust { file, adjustments, out } => {
            filters::adjust(&decode(&file)?, &adjustments, &out)
        },
        ImageAction::PixelArt { file, pixel_size, colors, out } => {
            filters::pixel_art(&decode(&file)?, pixel_size, colors, &out)
        },
        ImageAction::RemoveBackground { file, color, tolerance, feather, out } => {
            filters::remove_background(&decode(&file)?, color.as_deref(), tolerance, feather, &out)
        },

        ImageAction::AddBorder {
            file,
            size,
            color,
            top,
            right,
            bottom,
            left,
            out,
        } => transform::add_border(&decode(&file)?, size, &color, top, right, bottom, left, &out),
        ImageAction::TrimBorder { file, tolerance, out } => {
            transform::trim_border(&decode(&file)?, tolerance, &out)
        },
        ImageAction::RoundCorners { file, radius, out } => {
            transform::round_corners(&decode(&file)?, &radius, &out)
        },
        ImageAction::FitPrintSize {
            file,
            width,
            height,
            unit,
            dpi,
            mode,
            out,
        } => transform::fit_print_size(&decode(&file)?, width, height, &unit, dpi, &mode, &out),

        ImageAction::WatermarkText { file, text, options, out } => {
            compose::watermark_text(&decode(&file)?, &text, &options, &out)
        },
        ImageAction::WatermarkImage { file, overlay, options, out } => {
            compose::watermark_image(&decode(&file)?, &overlay, &options, &out)
        },
        ImageAction::RenderText {
            text,
            font_size,
            color,
            bold,
            background,
            padding,
            max_width,
        } => compose::render_text_image(
            &text,
            font_size,
            &color,
            bold,
            background.as_deref(),
            padding,
            max_width,
        ),

        ImageAction::StripMetadata { file, keep_icc } => meta::strip_metadata(&file, keep_icc),
        ImageAction::SetDpi { file, dpi } => meta::set_dpi(&file, dpi),
        ImageAction::ToDataUrl { file } => meta::to_data_url(&file),
        ImageAction::FromDataUrl { data } => meta::from_data_url(&data),

        ImageAction::Palette { file, count } => colors::palette(&decode(&file)?, count),
        ImageAction::DominantColor { file } => colors::dominant_and_average(&decode(&file)?),
        ImageAction::ColorAt { file, x, y, sample } => {
            colors::color_at(&decode(&file)?, x, y, sample)
        },
        ImageAction::ToAscii { file, columns, charset, invert } => {
            colors::to_ascii(&decode(&file)?, columns, &charset, invert)
        },

        ImageAction::SvgToRaster {
            svg,
            width,
            height,
            scale,
            background,
            format,
            quality,
        } => svg::to_raster(
            &svg,
            width,
            height,
            scale,
            background.as_deref(),
            format.as_deref(),
            quality,
        ),
        ImageAction::SvgOptimize { svg, precision } => svg::optimize(&svg, precision),
        ImageAction::SvgInfo { svg } => svg::info(&svg),

        ImageAction::GifFrames { file, step, limit } => gif::extract_frames(&file, step, limit),
        ImageAction::GifResize { file, width, height, speed, drop_every } => {
            gif::resize_gif(&file, width, height, speed, drop_every)
        },

        ImageAction::Sprite {
            files,
            columns,
            padding,
            background,
            names,
        } => compose::sprite(&files, columns, padding, background.as_deref(), &names),
        ImageAction::ContactSheet { files, options } => compose::contact_sheet(&files, &options),
        ImageAction::Difference { file_a, file_b, threshold, highlight } => {
            compose::difference(&file_a, &file_b, threshold, &highlight)
        },
        ImageAction::Favicon { file, sizes, background } => {
            compose::favicon(&decode(&file)?, &sizes, background.as_deref())
        },
        ImageAction::OgImage { options } => compose::og_image(&options),
    }
}

/// Size-aware compression: optional downscale, then encode. With
/// `target_kb` the JPEG/WebP quality is searched (binary search) so the file
/// fits under the target.
fn compress(
    file: &[u8],
    quality: Option<u8>,
    max_width: Option<u32>,
    max_height: Option<u32>,
    format: Option<&str>,
    target_kb: Option<u32>,
) -> EngineResult<Value> {
    let d = decode(file)?;
    let mut img = d.image.clone();
    if max_width.is_some() || max_height.is_some() {
        let mw = max_width.unwrap_or(u32::MAX);
        let mh = max_height.unwrap_or(u32::MAX);
        if mw == 0 || mh == 0 {
            return Err(EngineError::invalid("`max_width`/`max_height` must be > 0"));
        }
        if img.width() > mw || img.height() > mh {
            img = img.resize(
                mw.min(img.width()),
                mh.min(img.height()),
                image::imageops::FilterType::Lanczos3,
            );
        }
    }
    let fmt = codec::resolve_format(format, d.format)?;
    let lossy = matches!(fmt, OutFormat::Jpeg);
    let mut opts = EncodeOptions {
        format: fmt,
        quality: quality.unwrap_or(80),
        png_level: 2,
        ..Default::default()
    };
    let mut bytes = codec::encode(&img, &opts)?;
    let mut final_quality = opts.quality;
    if let Some(kb) = target_kb {
        if kb == 0 {
            return Err(EngineError::invalid("`target_kb` must be > 0"));
        }
        let target = kb as usize * 1024;
        if lossy && bytes.len() > target {
            let (mut lo, mut hi) = (5u8, opts.quality);
            let mut best: Option<(u8, Vec<u8>)> = None;
            while lo <= hi {
                let mid = (lo as u16 + hi as u16) as u8 / 2;
                opts.quality = mid;
                let candidate = codec::encode(&img, &opts)?;
                if candidate.len() <= target {
                    best = Some((mid, candidate));
                    lo = mid + 1;
                } else {
                    if mid == 0 {
                        break;
                    }
                    hi = mid - 1;
                }
            }
            if let Some((q, b)) = best {
                final_quality = q;
                bytes = b;
            } else {
                final_quality = 5;
                opts.quality = 5;
                bytes = codec::encode(&img, &opts)?;
            }
        }
    }
    let (w, h) = (img.width(), img.height());
    let after = bytes.len();
    Ok(json!({
        "output": codec::ImageOutput::new(bytes, fmt, w, h),
        "bytes_before": file.len(),
        "bytes_after": after,
        "saved_percent": if !file.is_empty() { ((file.len() as f64 - after as f64) / file.len() as f64 * 1000.0).round() / 10.0 } else { 0.0 },
        "quality": if lossy { json!(final_quality) } else { Value::Null },
        "resized": (w, h) != (d.image.width(), d.image.height()),
        "target_met": target_kb.map(|kb| after <= kb as usize * 1024),
    }))
}

engine_shared::export_engine!(handle_action, ImageAction);
