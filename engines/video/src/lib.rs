//! # Dev Toolbox Video Engine (`video-engine` v2)
//!
//! Pure-Rust WASM has no video codecs, so this engine focuses on what can be
//! done *without decoding pictures*:
//!
//! | Action | Tool(s) | Method |
//! |---|---|---|
//! | `inspect` | video-metadata, video-resolution-check, video-frame-rate-check, video-aspect-ratio | MP4/MOV/M4V (mp4 crate), MKV/WebM (matroska) |
//! | `trim` / `cut` / `split` | video-trim, video-cut, video-split | lossless MP4 sample copy, keyframe aligned |
//! | `mute` / `remove_audio` | video-mute, video-remove-audio | drop audio tracks (MP4) |
//! | `extract_audio_track` | video-extract-audio | copy AAC track → M4A (decode to WAV/FLAC via audio engine) |
//! | `loop` | video-loop | repeat samples (MP4) |
//! | `concat` | video-merge, video-concat | same-codec MP4 concatenation |
//! | `extract_subtitles` | video-subtitle-extract | tx3g tracks → SRT/VTT |
//! | `subtitle_convert` / `subtitle_shift` / `subtitle_validate` | video-subtitle-burn (prep), subtitle tools | SRT/VTT/ASS/LRC/JSON |
//! | `gif_from_frames` / `contact_sheet` / `frame_plan` | video-gif, video-to-webp (GIF fallback), video-extract-frames, video-thumbnail, video-contact-sheet | browser grabs frames, engine assembles |
//! | `calc_bitrate` / `calc_frames` / `aspect_ratio` / `crop_calculator` / `speed_calculator` / `resize_calculator` / `timecode` / `estimate_size` | calculators | maths |
//! | `capabilities` | all | reports which tools run in WASM, in the browser (WebCodecs/canvas) or on the backend (ffmpeg) |
//!
//! Transcoding, resizing, filters, watermarking, rotation, speed change,
//! reversing, subtitle burning and WebP animation require a codec and return
//! `E_UNSUPPORTED` with a pointer to the right runtime.

pub mod frames;
pub mod inspect;
pub mod mp4ops;
pub mod subtitles;

use engine_shared::{EngineError, EngineResult};
use serde::Deserialize;
use serde_json::{json, Value};

/// A base64 file plus optional original file name.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct FileInput {
    /// File bytes (base64 on the wire).
    #[serde(with = "engine_shared::b64")]
    pub file: Vec<u8>,
    /// Original file name (used to derive output names).
    pub name: Option<String>,
}

fn base_name(input: &FileInput, fallback: &str) -> String {
    input
        .name
        .as_deref()
        .map(|n| n.rsplit('/').next().unwrap_or(n))
        .and_then(|n| {
            n.rsplit_once('.')
                .map(|(b, _)| b.to_string())
                .or_else(|| Some(n.to_string()))
        })
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn require_file(input: &FileInput) -> EngineResult<()> {
    engine_shared::error::require_bytes(&input.file, "video")?;
    engine_shared::limits::check_size(
        input.file.len(),
        engine_shared::limits::MAX_FILE_BYTES,
        "video",
    )
}

fn require_mp4(input: &FileInput) -> EngineResult<()> {
    require_file(input)?;
    match inspect::detect(&input.file) {
        inspect::Container::Mp4 => Ok(()),
        inspect::Container::Unknown => Err(EngineError::unsupported_format(
            "Unrecognised container; lossless editing needs an MP4/MOV/M4V file",
        )),
        other => Err(EngineError::unsupported(format!(
            "Lossless editing is only available for MP4/MOV/M4V; this is {}. Use the backend \
             ffmpeg service.",
            other.label().to_uppercase()
        ))),
    }
}

fn unsupported(tool: &str, why: &str, route: &str) -> EngineError {
    EngineError::unsupported(format!(
        "{tool} is not available in the WASM video engine: {why}. Recommended runtime: {route}."
    ))
}

/// Every action understood by the video engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum VideoAction {
    /// Container/track/codec information.
    Inspect {
        /// Input video.
        #[serde(flatten)]
        input: FileInput,
    },
    /// Lossless trim (MP4: keyframe-aligned sample range).
    Trim {
        /// Input video.
        #[serde(flatten)]
        input: FileInput,
        /// Start time in seconds.
        #[serde(default)]
        start: f64,
        /// End time in seconds (default: end of file).
        #[serde(default)]
        end: Option<f64>,
    },
    /// Removes a section from the middle (lossless, keyframe-aligned).
    Cut {
        /// Input video.
        #[serde(flatten)]
        input: FileInput,
        /// Start of the removed section in seconds.
        start: f64,
        /// End of the removed section in seconds.
        end: f64,
    },
    /// Splits into several files at timestamps or into equal parts.
    Split {
        /// Input video.
        #[serde(flatten)]
        input: FileInput,
        /// Split points in seconds.
        #[serde(default)]
        times: Vec<f64>,
        /// Number of equal parts (used when `times` is empty).
        #[serde(default)]
        parts: usize,
    },
    /// Removes audio tracks (alias of `remove_audio`).
    Mute {
        /// Input video.
        #[serde(flatten)]
        input: FileInput,
    },
    /// Removes audio tracks.
    RemoveAudio {
        /// Input video.
        #[serde(flatten)]
        input: FileInput,
    },
    /// Extracts the audio track into an M4A/MP4 container without re-encoding.
    ExtractAudioTrack {
        /// Input video.
        #[serde(flatten)]
        input: FileInput,
    },
    /// Repeats the media `times` times (lossless sample duplication).
    Loop {
        /// Input video.
        #[serde(flatten)]
        input: FileInput,
        /// Number of repetitions.
        times: u32,
    },
    /// Concatenates MP4 files with compatible tracks (no re-encoding).
    Concat {
        /// Inputs in order.
        files: Vec<FileInput>,
        /// Output file name.
        #[serde(default)]
        name: String,
    },
    /// Extracts text subtitle tracks (MP4 `tx3g`, Matroska SRT/ASS).
    ExtractSubtitles {
        /// Input video.
        #[serde(flatten)]
        input: FileInput,
    },
    /// Converts between SRT, WebVTT, ASS and JSON subtitle formats.
    SubtitleConvert {
        /// Subtitle text.
        text: String,
        /// `auto` (default), `srt`, `vtt`, `ass`, `json` or `lrc`.
        #[serde(default)]
        from: String,
        /// `srt`, `vtt`, `ass` or `json`.
        to: String,
    },
    /// Shifts and/or rescales subtitle timing.
    SubtitleShift {
        /// Subtitle text.
        text: String,
        /// `auto` (default), `srt`, `vtt`, `ass`, `json` or `lrc`.
        #[serde(default)]
        from: String,
        /// Target format (default: same as source).
        #[serde(default)]
        to: String,
        /// Seconds added to every cue (negative to move earlier).
        #[serde(default)]
        shift_seconds: f64,
        /// Timing scale factor.
        #[serde(default)]
        scale: Option<f64>,
        /// Original frame rate (for frame-rate conversion).
        #[serde(default)]
        from_fps: Option<f64>,
        /// Target frame rate (for frame-rate conversion).
        #[serde(default)]
        to_fps: Option<f64>,
    },
    /// Checks subtitles for overlaps, ordering, reading speed and duration.
    SubtitleValidate {
        /// Subtitle text.
        text: String,
        /// `auto` (default), `srt`, `vtt`, `ass`, `json` or `lrc`.
        #[serde(default)]
        from: String,
        /// Maximum characters per second before a warning.
        #[serde(default)]
        max_chars_per_second: Option<f64>,
        /// Maximum cue duration in seconds before a warning.
        #[serde(default)]
        max_duration: Option<f64>,
    },
    /// Builds an animated GIF from still frames.
    GifFromFrames {
        /// Frames (base64 PNG/JPEG).
        #[serde(with = "engine_shared::b64::list")]
        frames: Vec<Vec<u8>>,
        /// Frames per second.
        #[serde(default)]
        fps: Option<f64>,
        /// Output width (height keeps the aspect ratio).
        #[serde(default)]
        width: Option<u32>,
        /// Loop forever (otherwise play once).
        #[serde(default = "default_true")]
        loop_forever: bool,
        /// Quantiser quality 1–30 (lower is better).
        #[serde(default)]
        quality: Option<i32>,
    },
    /// Thumbnail grid from still frames.
    ContactSheet {
        /// Frames (base64 PNG/JPEG).
        #[serde(with = "engine_shared::b64::list")]
        frames: Vec<Vec<u8>>,
        /// Columns in the grid.
        #[serde(default)]
        columns: Option<u32>,
        /// Thumbnail width.
        #[serde(default)]
        thumb_width: Option<u32>,
        /// Gap between thumbnails.
        #[serde(default)]
        padding: Option<u32>,
        /// Background colour.
        #[serde(default)]
        background: Option<String>,
    },
    /// Computes timestamps at which the browser should grab frames.
    FramePlan {
        /// Media duration in seconds.
        duration: f64,
        /// Frames per second to capture.
        #[serde(default)]
        fps: Option<f64>,
        /// Total number of frames wanted.
        #[serde(default)]
        count: Option<usize>,
        /// Interval between frames in seconds.
        #[serde(default)]
        interval: Option<f64>,
        /// Start time.
        #[serde(default)]
        start: f64,
        /// End time (default: duration).
        #[serde(default)]
        end: Option<f64>,
    },
    /// Video bitrate needed to hit a target file size.
    CalcBitrate {
        /// Duration in seconds.
        duration_seconds: f64,
        /// Target size in megabytes.
        target_size_mb: f64,
        /// Audio bitrate to reserve.
        #[serde(default)]
        audio_bitrate_kbps: f64,
    },
    /// Total frame count for a duration and frame rate.
    CalcFrames {
        /// Duration in seconds.
        duration_seconds: f64,
        /// Frames per second.
        fps: f64,
    },
    /// Aspect-ratio information and letterbox/pillarbox maths for a target ratio.
    AspectRatio {
        /// Frame width.
        width: u32,
        /// Frame height.
        height: u32,
        /// Target ratio such as `16:9`.
        #[serde(default)]
        target: Option<String>,
    },
    /// Crop rectangle for a target aspect ratio.
    CropCalculator {
        /// Frame width.
        width: u32,
        /// Frame height.
        height: u32,
        /// Target ratio such as `1:1`.
        target: String,
        /// `center`, `top`, `bottom`, `left` or `right`.
        #[serde(default)]
        anchor: String,
    },
    /// Resulting duration/frame count after a speed change.
    SpeedCalculator {
        /// Duration in seconds.
        duration_seconds: f64,
        /// Speed factor (2 = twice as fast).
        factor: f64,
        /// Frame rate (for frame counts).
        #[serde(default)]
        fps: Option<f64>,
    },
    /// Even-dimension resize maths for encoders.
    ResizeCalculator {
        /// Frame width.
        width: u32,
        /// Frame height.
        height: u32,
        /// Target width.
        #[serde(default)]
        target_width: Option<u32>,
        /// Target height.
        #[serde(default)]
        target_height: Option<u32>,
        /// Named target such as `1080p`.
        #[serde(default)]
        target_label: Option<String>,
        /// Round to even dimensions (required by most codecs).
        #[serde(default = "default_true")]
        even: bool,
    },
    /// Converts between seconds, frames and SMPTE timecodes.
    Timecode {
        /// Input as seconds.
        #[serde(default)]
        seconds: Option<f64>,
        /// Input as `hh:mm:ss:ff` / `hh:mm:ss;ff`.
        #[serde(default)]
        timecode: Option<String>,
        /// Input as a frame count.
        #[serde(default)]
        frames: Option<u64>,
        /// Frame rate.
        fps: f64,
        /// Use drop-frame timecode (29.97/59.94).
        #[serde(default)]
        drop_frame: bool,
    },
    /// Estimates output size from bitrates or resolution heuristics.
    EstimateSize {
        /// Duration in seconds.
        duration_seconds: f64,
        /// Video bitrate.
        #[serde(default)]
        video_bitrate_kbps: Option<f64>,
        /// Audio bitrate.
        #[serde(default)]
        audio_bitrate_kbps: Option<f64>,
        /// Frame width (for the heuristic).
        #[serde(default)]
        width: Option<u32>,
        /// Frame height (for the heuristic).
        #[serde(default)]
        height: Option<u32>,
        /// Frame rate (for the heuristic).
        #[serde(default)]
        fps: Option<f64>,
        /// Codec name (`h264`, `h265`, `vp9`, `av1`) for the heuristic.
        #[serde(default)]
        codec: Option<String>,
    },
    /// Lists what this WASM build can and cannot do.
    Capabilities {},
    // Tools that need a codec – explicit, documented refusals.
    /// Always returns `E_UNSUPPORTED`: transcoding needs a codec (browser WebCodecs / ffmpeg
    /// backend).
    Transcode {
        /// Catalog tool id, echoed back in the guidance.
        #[serde(default)]
        tool: String,
    },
}

fn default_true() -> bool {
    true
}

/// Dispatches a [`VideoAction`] to its implementation.
pub fn handle_action(action: VideoAction) -> EngineResult<Value> {
    match action {
        VideoAction::Inspect { input } => inspect::inspect(&input.file, input.name.as_deref()),
        VideoAction::Trim { input, start, end } => {
            require_mp4(&input)?;
            mp4ops::trim(
                &input.file,
                start,
                end,
                &[],
                &format!("{}_trimmed", base_name(&input, "video")),
            )
        },
        VideoAction::Cut { input, start, end } => {
            require_mp4(&input)?;
            if end <= start {
                return Err(EngineError::invalid("end must be after start"));
            }
            let dur = inspect::open_mp4(&input.file)?.duration().as_secs_f64();
            let name = base_name(&input, "video");
            if start <= 0.0 {
                return mp4ops::trim(&input.file, end, None, &[], &format!("{name}_cut"));
            }
            if end >= dur {
                return mp4ops::trim(&input.file, 0.0, Some(start), &[], &format!("{name}_cut"));
            }
            let head = mp4ops::trim(&input.file, 0.0, Some(start), &[], "head")?;
            let tail = mp4ops::trim(&input.file, end, None, &[], "tail")?;
            let hb = engine_shared::b64::decode(head["file"].as_str().unwrap_or(""))?;
            let tb = engine_shared::b64::decode(tail["file"].as_str().unwrap_or(""))?;
            let mut v = mp4ops::concat(&[hb, tb], &format!("{name}_cut"))?;
            v["removed"] = json!({
                "start": start,
                "end": end,
                "note": "Cut points snap to keyframes"
            });
            Ok(v)
        },
        VideoAction::Split { input, times, parts } => {
            require_mp4(&input)?;
            let points = if !times.is_empty() {
                times
            } else {
                if parts < 2 {
                    return Err(EngineError::invalid("Provide `times` (seconds) or `parts` >= 2"));
                }
                let dur = inspect::open_mp4(&input.file)?.duration().as_secs_f64();
                (1..parts).map(|i| dur * i as f64 / parts as f64).collect()
            };
            mp4ops::split(&input.file, points, &base_name(&input, "video"))
        },
        VideoAction::Mute { input } | VideoAction::RemoveAudio { input } => {
            require_mp4(&input)?;
            mp4ops::mute(&input.file, &base_name(&input, "video"))
        },
        VideoAction::ExtractAudioTrack { input } => {
            require_mp4(&input)?;
            mp4ops::extract_audio(&input.file, &base_name(&input, "video"))
        },
        VideoAction::Loop { input, times } => {
            require_mp4(&input)?;
            mp4ops::repeat(&input.file, times, &base_name(&input, "video"))
        },
        VideoAction::Concat { files, name } => {
            engine_shared::limits::check_count(files.len(), 50, "files")?;
            for f in &files {
                require_mp4(f)?;
            }
            mp4ops::concat(
                &files.iter().map(|f| f.file.clone()).collect::<Vec<_>>(),
                if name.is_empty() { "merged" } else { &name },
            )
        },
        VideoAction::ExtractSubtitles { input } => {
            require_file(&input)?;
            match inspect::detect(&input.file) {
                inspect::Container::Mp4 => mp4ops::extract_subtitles(&input.file),
                inspect::Container::Matroska | inspect::Container::WebM => Err(unsupported(
                    "Subtitle extraction from MKV/WebM",
                    "block-level demuxing is not implemented",
                    "backend ffmpeg (`-map 0:s:0`)",
                )),
                _ => Err(EngineError::unsupported_format("Unrecognised container")),
            }
        },
        VideoAction::SubtitleConvert { text, from, to } => {
            let (cues, detected) = subtitles::parse(&text, &from)?;
            let out = subtitles::render(&cues, &to)?;
            Ok(json!({
                "text": out,
                "from": detected,
                "to": to,
                "cues": cues.len(),
                "filename": format!("subtitles.{}", if to == "webvtt" { "vtt" } else { to.as_str() })
            }))
        },
        VideoAction::SubtitleShift {
            text,
            from,
            to,
            shift_seconds,
            scale,
            from_fps,
            to_fps,
        } => {
            let (cues, detected) = subtitles::parse(&text, &from)?;
            let scale = match (scale, from_fps, to_fps) {
                (Some(s), _, _) => s,
                (None, Some(a), Some(b)) if a > 0.0 && b > 0.0 => a / b,
                _ => 1.0,
            };
            let out = subtitles::retime(&cues, shift_seconds, scale)?;
            let fmt = if to.is_empty() { detected.to_string() } else { to };
            Ok(json!({
                "text": subtitles::render(&out, &fmt)?,
                "format": fmt,
                "cues": out.len(),
                "shift_seconds": shift_seconds,
                "scale": scale,
                "dropped_cues": cues.len() - out.len()
            }))
        },
        VideoAction::SubtitleValidate {
            text,
            from,
            max_chars_per_second,
            max_duration,
        } => {
            let (cues, detected) = subtitles::parse(&text, &from)?;
            let mut v = subtitles::validate(
                &cues,
                max_chars_per_second.unwrap_or(20.0),
                max_duration.unwrap_or(7.0),
            );
            v["format"] = json!(detected);
            v["cues_json"] = subtitles::to_json(&cues);
            Ok(v)
        },
        VideoAction::GifFromFrames {
            frames,
            fps,
            width,
            loop_forever,
            quality,
        } => frames::gif(&frames, fps.unwrap_or(10.0), width, loop_forever, quality.unwrap_or(10)),
        VideoAction::ContactSheet {
            frames,
            columns,
            thumb_width,
            padding,
            background,
        } => {
            let bg = background
                .map(|b| parse_hex(&b))
                .transpose()?
                .unwrap_or([17, 17, 17, 255]);
            frames::sheet(
                &frames,
                columns.unwrap_or(4),
                thumb_width.unwrap_or(240),
                padding.unwrap_or(8),
                bg,
            )
        },
        VideoAction::FramePlan {
            duration,
            fps,
            count,
            interval,
            start,
            end,
        } => frames::plan(duration, fps, count, interval, start, end),
        VideoAction::CalcBitrate {
            duration_seconds,
            target_size_mb,
            audio_bitrate_kbps,
        } => {
            if duration_seconds <= 0.0 || target_size_mb <= 0.0 {
                return Err(EngineError::invalid(
                    "Duration and target size must be greater than zero",
                ));
            }
            let total_kbps = target_size_mb * 8192.0 / duration_seconds;
            let audio = if audio_bitrate_kbps <= 0.0 {
                128.0
            } else {
                audio_bitrate_kbps
            };
            let video = (total_kbps - audio).max(50.0);
            Ok(json!({
                "total_bitrate_kbps": (total_kbps * 10.0).round() / 10.0,
                "video_bitrate_kbps": (video * 10.0).round() / 10.0,
                "audio_bitrate_kbps": audio,
                "estimated_size_mb": target_size_mb,
                "two_pass_ffmpeg": format!("ffmpeg -i in.mp4 -c:v libx264 -b:v {}k -pass 1 -an -f \
                                            null /dev/null && ffmpeg -i in.mp4 -c:v libx264 -b:v \
                                            {}k -pass 2 -c:a aac -b:a {}k out.mp4", video.round(), video.round(), audio.round()),
                "quality_hint": quality_hint(video),
                "warning": if total_kbps - audio < 50.0 { json!("Target size is too small for \
                                                                 this duration; video quality \
                                                                 will be very poor") } else { Value::Null },
            }))
        },
        VideoAction::CalcFrames { duration_seconds, fps } => {
            if duration_seconds < 0.0 || fps <= 0.0 {
                return Err(EngineError::invalid("fps must be > 0 and duration >= 0"));
            }
            let total = (duration_seconds * fps).round() as u64;
            Ok(json!({
                "duration": duration_seconds,
                "fps": fps,
                "total_frames": total,
                "frame_duration_ms": (1000.0 / fps * 1000.0).round() / 1000.0,
                "timecode": to_timecode(duration_seconds, fps, false),
                "frame_rate": inspect::fps_info(fps)
            }))
        },
        VideoAction::AspectRatio { width, height, target } => {
            if width == 0 || height == 0 {
                return Err(EngineError::invalid("width and height must be > 0"));
            }
            let mut v = inspect::aspect_info(width, height);
            v["width"] = json!(width);
            v["height"] = json!(height);
            v["resolution_label"] = json!(inspect::resolution_label(width, height));
            if let Some(t) = target {
                let (tw, th) = parse_ratio(&t)?;
                let target_ratio = tw / th;
                let current = width as f64 / height as f64;
                // Letterbox/pillarbox: fit inside target canvas with same width.
                let (fit_w, fit_h) = if current > target_ratio {
                    (width as f64, width as f64 / target_ratio)
                } else {
                    (height as f64 * target_ratio, height as f64)
                };
                let (crop_w, crop_h) = if current > target_ratio {
                    (height as f64 * target_ratio, height as f64)
                } else {
                    (width as f64, width as f64 / target_ratio)
                };
                v["target"] = json!({
                    "ratio": t,
                    "pad_to": {
                        "width": even(fit_w),
                        "height": even(fit_h),
                        "bars": if current > target_ratio { "letterbox (top/bottom)" } else { "pillarbox (left/right)" },
                        "bar_size_px": even(if current > target_ratio { (fit_h - height as f64) / 2.0 } else { (fit_w - width as f64) / 2.0 })
                    },
                    "crop_to": {
                        "width": even(crop_w),
                        "height": even(crop_h),
                        "x": even((width as f64 - crop_w) / 2.0),
                        "y": even((height as f64 - crop_h) / 2.0)
                    },
                });
            }
            Ok(v)
        },
        VideoAction::CropCalculator { width, height, target, anchor } => {
            if width == 0 || height == 0 {
                return Err(EngineError::invalid("width and height must be > 0"));
            }
            let (tw, th) = parse_ratio(&target)?;
            let ratio = tw / th;
            let (cw, ch) = if width as f64 / height as f64 > ratio {
                ((height as f64 * ratio).floor(), height as f64)
            } else {
                (width as f64, (width as f64 / ratio).floor())
            };
            let (cw, ch) = (even(cw) as f64, even(ch) as f64);
            let (x, y) = match anchor.as_str() {
                "top" | "top-center" => ((width as f64 - cw) / 2.0, 0.0),
                "bottom" | "bottom-center" => ((width as f64 - cw) / 2.0, height as f64 - ch),
                "left" | "center-left" => (0.0, (height as f64 - ch) / 2.0),
                "right" | "center-right" => (width as f64 - cw, (height as f64 - ch) / 2.0),
                "top-left" => (0.0, 0.0),
                "top-right" => (width as f64 - cw, 0.0),
                "bottom-left" => (0.0, height as f64 - ch),
                "bottom-right" => (width as f64 - cw, height as f64 - ch),
                _ => ((width as f64 - cw) / 2.0, (height as f64 - ch) / 2.0),
            };
            Ok(json!({
                "source": { "width": width, "height": height },
                "target_ratio": target,
                "crop": { "x": even(x), "y": even(y), "width": cw as u32, "height": ch as u32 },
                "removed_percent": ((1.0 - (cw * ch) / (width as f64 * height as f64)) * 1000.0).round() / 10.0,
                "ffmpeg_filter": format!("crop={}:{}:{}:{}", cw as u32, ch as u32, even(x), even(y)),
                "css_object_position": format!("{:.1}% {:.1}%", if width as f64 - cw > 0.0 { x / (width as f64 - cw) * 100.0 } else { 50.0 }, if height as f64 - ch > 0.0 { y / (height as f64 - ch) * 100.0 } else { 50.0 })
            }))
        },
        VideoAction::SpeedCalculator { duration_seconds, factor, fps } => {
            if duration_seconds <= 0.0 || factor <= 0.0 {
                return Err(EngineError::invalid("duration and factor must be > 0"));
            }
            let new_dur = duration_seconds / factor;
            Ok(json!({
                "original_duration": duration_seconds,
                "factor": factor,
                "new_duration": (new_dur * 1000.0).round() / 1000.0,
                "new_duration_display": inspect::hms(new_dur),
                "time_saved_seconds": (duration_seconds - new_dur) * 1000.0 / 1000.0,
                "pitch_shift_semitones_if_unpreserved": (12.0 * factor.log2() * 100.0).round() / 100.0,
                "frames": fps.map(|f| json!({ "original": (duration_seconds * f).round(), "\
                    drop_or_duplicate_per_second": ((f * factor - f).abs() * 100.0).round() / 100.0 })),
                "ffmpeg_filter": format!("-filter_complex \
                                          \"[0:v]setpts={:.4}*PTS[v];[0:a]atempo={}[a]\" -map \
                                          \"[v]\" -map \"[a]\"", 1.0 / factor, atempo_chain(factor)),
                "css": format!("video.playbackRate = {factor}")
            }))
        },
        VideoAction::ResizeCalculator {
            width,
            height,
            target_width,
            target_height,
            target_label,
            even: want_even,
        } => {
            if width == 0 || height == 0 {
                return Err(EngineError::invalid("width and height must be > 0"));
            }
            let ratio = width as f64 / height as f64;
            let (tw, th) = match (target_width, target_height, target_label.as_deref()) {
                (Some(w), Some(h), _) => (w as f64, h as f64),
                (Some(w), None, _) => (w as f64, w as f64 / ratio),
                (None, Some(h), _) => (h as f64 * ratio, h as f64),
                (None, None, Some(label)) => {
                    let h = match label.to_ascii_lowercase().trim_end_matches('p') {
                        "2160" | "4k" => 2160.0,
                        "1440" | "2k" => 1440.0,
                        "1080" => 1080.0,
                        "720" => 720.0,
                        "480" => 480.0,
                        "360" => 360.0,
                        "240" => 240.0,
                        other => {
                            return Err(EngineError::invalid(format!(
                                "Unknown target label `{other}` (2160p, 1440p, 1080p, 720p, 480p, \
                                 360p, 240p)"
                            )))
                        },
                    };
                    if ratio >= 1.0 {
                        (h * ratio, h)
                    } else {
                        (h, h / ratio)
                    }
                },
                _ => {
                    return Err(EngineError::invalid(
                        "Provide target_width, target_height or target_label",
                    ))
                },
            };
            let (ow, oh) = if want_even {
                (even(tw), even(th))
            } else {
                (tw.round() as u32, th.round() as u32)
            };
            let scale = ow as f64 / width as f64;
            Ok(json!({
                "source": { "width": width, "height": height },
                "output": { "width": ow, "height": oh, "label": inspect::resolution_label(ow, oh) },
                "scale_factor": (scale * 1000.0).round() / 1000.0,
                "direction": if scale > 1.0 { "upscale (no new detail)" } else if scale < 1.0 { "downscale" } else { "same size" },
                "aspect_preserved": ((ow as f64 / oh as f64) - ratio).abs() < 0.02,
                "pixel_reduction_percent": ((1.0 - (ow as f64 * oh as f64) / (width as f64 * height as f64)) * 1000.0).round() / 10.0,
                "ffmpeg_filter": format!("scale={ow}:{oh}:flags=lanczos"),
                "bitrate_suggestion_kbps": suggested_bitrate(ow, oh, 30.0)
            }))
        },
        VideoAction::Timecode {
            seconds,
            timecode,
            frames,
            fps,
            drop_frame,
        } => {
            if fps <= 0.0 {
                return Err(EngineError::invalid("fps must be > 0"));
            }
            let secs = if let Some(s) = seconds {
                s
            } else if let Some(f) = frames {
                f as f64 / fps
            } else if let Some(tc) = timecode.as_deref().filter(|t| !t.trim().is_empty()) {
                parse_timecode(tc, fps)?
            } else {
                return Err(EngineError::empty("seconds, frames or timecode"));
            };
            if secs < 0.0 {
                return Err(EngineError::invalid("time must be >= 0"));
            }
            Ok(json!({
                "seconds": (secs * 1000.0).round() / 1000.0,
                "frames": (secs * fps).round() as u64,
                "timecode": to_timecode(secs, fps, drop_frame),
                "timecode_ms": inspect::hms(secs),
                "srt": format!("{:02}:{:02}:{:02},{:03}", (secs / 3600.0).floor() as u64, ((secs % 3600.0) / 60.0).floor() as u64, (secs % 60.0).floor() as u64, ((secs - secs.floor()) * 1000.0).round() as u64 % 1000),
                "fps": fps,
                "drop_frame": drop_frame
            }))
        },
        VideoAction::EstimateSize {
            duration_seconds,
            video_bitrate_kbps,
            audio_bitrate_kbps,
            width,
            height,
            fps,
            codec,
        } => {
            if duration_seconds <= 0.0 {
                return Err(EngineError::invalid("duration must be > 0"));
            }
            let (w, h) = (width.unwrap_or(1920), height.unwrap_or(1080));
            let f = fps.unwrap_or(30.0);
            let base = video_bitrate_kbps.unwrap_or_else(|| suggested_bitrate(w, h, f));
            let codec_factor = match codec.as_deref().map(str::to_ascii_lowercase).as_deref() {
                Some("h265") | Some("hevc") => 0.6,
                Some("av1") => 0.5,
                Some("vp9") => 0.7,
                Some("prores") => 8.0,
                _ => 1.0,
            };
            let vb = base * codec_factor;
            let ab = audio_bitrate_kbps.unwrap_or(128.0);
            let total_bits = (vb + ab) * 1000.0 * duration_seconds;
            let mb = total_bits / 8.0 / 1_048_576.0;
            Ok(json!({
                "video_bitrate_kbps": vb.round(),
                "audio_bitrate_kbps": ab,
                "codec": codec.unwrap_or_else(|| "h264".into()),
                "estimated_size_mb": (mb * 100.0).round() / 100.0,
                "estimated_size_gb": (mb / 1024.0 * 1000.0).round() / 1000.0,
                "per_minute_mb": (mb / duration_seconds * 60.0 * 100.0).round() / 100.0,
                "resolution": { "width": w, "height": h, "label": inspect::resolution_label(w, h) },
                "fps": f,
                "quality_hint": quality_hint(vb)
            }))
        },
        VideoAction::Capabilities {} => Ok(capabilities()),
        VideoAction::Transcode { tool } => Err(unsupported(
            if tool.is_empty() { "Transcoding" } else { &tool },
            "decoding/encoding video frames requires codecs that are not available as pure \
             Rust/WASM",
            "browser WebCodecs + canvas for small clips, or the backend ffmpeg service",
        )),
    }
}

fn even(v: f64) -> u32 {
    let r = v.round().max(0.0) as u32;
    r - (r % 2)
}

fn parse_ratio(s: &str) -> EngineResult<(f64, f64)> {
    let t = s.trim().replace('x', ":");
    if let Some((a, b)) = t.split_once(':') {
        let (a, b): (f64, f64) = (
            a.trim().parse().map_err(|_| EngineError::invalid("Bad ratio"))?,
            b.trim().parse().map_err(|_| EngineError::invalid("Bad ratio"))?,
        );
        if a <= 0.0 || b <= 0.0 {
            return Err(EngineError::invalid("Ratio components must be > 0"));
        }
        return Ok((a, b));
    }
    let r: f64 = t
        .parse()
        .map_err(|_| EngineError::invalid(format!("`{s}` is not a ratio (use 16:9 or 1.777)")))?;
    if r <= 0.0 {
        return Err(EngineError::invalid("Ratio must be > 0"));
    }
    Ok((r, 1.0))
}

fn parse_hex(s: &str) -> EngineResult<[u8; 4]> {
    let h = s.trim().trim_start_matches('#');
    if !(h.len() == 6 || h.len() == 8) || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(EngineError::invalid("background must be #rrggbb or #rrggbbaa"));
    }
    let v: Vec<u8> = (0..h.len() / 2)
        .map(|i| u8::from_str_radix(&h[i * 2..i * 2 + 2], 16))
        .collect::<Result<_, _>>()
        .map_err(|_| EngineError::invalid(format!("Invalid hex colour `{h}`")))?;
    Ok([v[0], v[1], v[2], v.get(3).copied().unwrap_or(255)])
}

fn atempo_chain(mut factor: f64) -> String {
    // ffmpeg atempo accepts 0.5..100 per filter; chain for extreme values.
    let mut parts = Vec::new();
    while factor < 0.5 {
        parts.push("atempo=0.5".to_string());
        factor /= 0.5;
    }
    parts.push(format!("atempo={factor:.4}"));
    parts.join(",")
}

fn quality_hint(video_kbps: f64) -> &'static str {
    match video_kbps {
        k if k < 500.0 => "very low (mobile/preview quality)",
        k if k < 1500.0 => "low (480p)",
        k if k < 3500.0 => "medium (720p)",
        k if k < 8000.0 => "good (1080p)",
        k if k < 20000.0 => "high (1440p/4K streaming)",
        _ => "very high (4K mastering)",
    }
}

fn suggested_bitrate(w: u32, h: u32, fps: f64) -> f64 {
    // ~0.1 bits per pixel per frame for H.264 at good quality.
    let bpp = 0.1;
    let kbps = w as f64 * h as f64 * fps * bpp / 1000.0;
    (kbps / 50.0).round() * 50.0
}

fn to_timecode(secs: f64, fps: f64, drop_frame: bool) -> String {
    let total_frames = (secs * fps + 1e-4).floor() as u64;
    let nominal = fps.round() as u64;
    if drop_frame && (nominal == 30 || nominal == 60) {
        let drop = if nominal == 30 { 2 } else { 4 };
        let frames_per_10min = nominal * 600 - drop * 9;
        let d = total_frames / frames_per_10min;
        let m = total_frames % frames_per_10min;
        let extra = if m > drop {
            drop * 9 * d + drop * ((m - drop) / (nominal * 60 - drop))
        } else {
            drop * 9 * d
        };
        let fc = total_frames + extra;
        let fr = fc % nominal;
        let s = (fc / nominal) % 60;
        let mi = (fc / (nominal * 60)) % 60;
        let h = fc / (nominal * 3600);
        return format!("{h:02}:{mi:02}:{s:02};{fr:02}");
    }
    let fr = total_frames % nominal.max(1);
    let s = (total_frames / nominal.max(1)) % 60;
    let mi = (total_frames / (nominal.max(1) * 60)) % 60;
    let h = total_frames / (nominal.max(1) * 3600);
    format!("{h:02}:{mi:02}:{s:02}:{fr:02}")
}

fn parse_timecode(tc: &str, fps: f64) -> EngineResult<f64> {
    let t = tc.trim().replace(';', ":");
    let parts: Vec<&str> = t.split(':').collect();
    let nums: Vec<f64> = parts
        .iter()
        .map(|p| {
            p.parse::<f64>()
                .map_err(|_| EngineError::invalid(format!("Bad timecode `{tc}` (use HH:MM:SS:FF)")))
        })
        .collect::<EngineResult<_>>()?;
    Ok(match nums.len() {
        4 => nums[0] * 3600.0 + nums[1] * 60.0 + nums[2] + nums[3] / fps,
        3 => nums[0] * 3600.0 + nums[1] * 60.0 + nums[2],
        2 => nums[0] * 60.0 + nums[1],
        1 => nums[0],
        _ => return Err(EngineError::invalid(format!("Bad timecode `{tc}`"))),
    })
}

/// Per-tool runtime map used by the UI to route requests.
pub fn capabilities() -> Value {
    let wasm = "wasm";
    let browser = "browser (WebCodecs / <video> + canvas / MediaRecorder)";
    let backend = "backend (ffmpeg service)";
    let entries = [
        ("video-metadata", wasm, "inspect"),
        ("video-resolution-check", wasm, "inspect"),
        ("video-frame-rate-check", wasm, "inspect"),
        ("video-aspect-ratio", wasm, "inspect / aspect_ratio"),
        (
            "video-trim",
            wasm,
            "trim (MP4 lossless, keyframe aligned); other containers → backend",
        ),
        ("video-cut", wasm, "cut (MP4 lossless)"),
        ("video-split", wasm, "split (MP4 lossless)"),
        ("video-mute", wasm, "mute"),
        ("video-remove-audio", wasm, "remove_audio"),
        (
            "video-extract-audio",
            wasm,
            "extract_audio_track (AAC copy) or audio-engine extract_from_video (decode)",
        ),
        ("video-loop", wasm, "loop (MP4)"),
        ("video-merge", wasm, "concat (identical codec settings) else backend"),
        ("video-concat", wasm, "concat"),
        ("video-subtitle-extract", wasm, "extract_subtitles (MP4 tx3g); MKV → backend"),
        ("video-gif", browser, "frame_plan → canvas frames → gif_from_frames"),
        (
            "video-to-webp",
            browser,
            "animated WebP needs the browser encoder; GIF via gif_from_frames",
        ),
        ("video-extract-frames", browser, "frame_plan + canvas"),
        ("video-thumbnail", browser, "frame_plan + canvas (single frame)"),
        (
            "video-contact-sheet",
            browser,
            "frame_plan + canvas → contact_sheet (wasm assembly)",
        ),
        ("video-compress", backend, "re-encode; calc_bitrate helps choose a target"),
        ("video-convert", backend, "transcode"),
        ("video-resize", backend, "resize_calculator provides the exact parameters"),
        ("video-crop", backend, "crop_calculator provides the crop rectangle"),
        ("video-rotate", backend, "metadata rotation flag or re-encode"),
        ("video-flip", backend, "filter"),
        ("video-speed", backend, "speed_calculator provides setpts/atempo"),
        ("video-reverse", backend, "filter"),
        ("video-volume", backend, "audio filter (or audio-engine on the extracted track)"),
        ("video-watermark", backend, "overlay filter"),
        (
            "video-subtitle-burn",
            backend,
            "subtitle_convert prepares SRT/ASS; burning needs the encoder",
        ),
    ];
    json!({
        "engine": "video-engine 2.0",
        "wasm_capabilities": [
            "container inspection (MP4/MOV/M4V/MKV/WebM)",
            "lossless MP4 trim/cut/split/mute/loop/concat/audio-track copy",
            "tx3g subtitle extraction",
            "subtitle parse/convert/retime/validate",
            "GIF + contact-sheet assembly from frames",
            "bitrate/frame/aspect/crop/speed/resize/timecode calculators"
        ],
        "not_in_wasm": "No video decoder/encoder is available as pure Rust; anything that changes \
                        pixels is routed to the browser or backend.",
        "tools": entries.iter().map(|(t, r, how)| json!({ "tool": t, "runtime": r, "how": how })).collect::<Vec<_>>(),
    })
}

engine_shared::export_engine!(handle_action, VideoAction);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculators() {
        assert_eq!(to_timecode(3661.5, 25.0, false), "01:01:01:12");
        assert_eq!(to_timecode(600.0, 29.97, true), "00:10:00;00");
        assert_eq!(to_timecode(1800.0 / 29.97, 29.97, true), "00:01:00;02");
        assert_eq!(parse_timecode("00:01:00:12", 24.0).unwrap(), 60.5);
        assert_eq!(parse_ratio("16:9").unwrap(), (16.0, 9.0));
        assert_eq!(parse_ratio("1.5").unwrap(), (1.5, 1.0));
        assert!(parse_ratio("abc").is_err());
        assert_eq!(even(1079.6), 1080);
        assert_eq!(atempo_chain(0.2), "atempo=0.5,atempo=0.5,atempo=0.8000");
        assert_eq!(suggested_bitrate(1920, 1080, 30.0), 6200.0);
    }
}
