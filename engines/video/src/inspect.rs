//! Container inspection for MP4/MOV/M4V (mp4 crate) and MKV/WebM (matroska).

use engine_shared::{EngineError, EngineResult};
use mp4::{MediaType, Metadata, Mp4Reader, TrackType};
use serde_json::{json, Map, Value};
use std::io::Cursor;

/// Container format detected from magic bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    /// ISO BMFF (`ftyp`): MP4, M4V, MOV.
    Mp4,
    /// Matroska (`.mkv`).
    Matroska,
    /// WebM (Matroska with the `webm` doctype).
    WebM,
    /// RIFF AVI.
    Avi,
    /// MPEG program/transport stream.
    Mpeg,
    /// Ogg.
    Ogg,
    /// Flash Video.
    Flv,
    /// Not recognised.
    Unknown,
}

impl Container {
    /// Human-readable container name.
    pub fn label(self) -> &'static str {
        match self {
            Container::Mp4 => "mp4",
            Container::Matroska => "mkv",
            Container::WebM => "webm",
            Container::Avi => "avi",
            Container::Mpeg => "mpeg",
            Container::Ogg => "ogg",
            Container::Flv => "flv",
            Container::Unknown => "unknown",
        }
    }
}

/// Detects the container from the first bytes of the file.
pub fn detect(bytes: &[u8]) -> Container {
    if bytes.len() < 12 {
        return Container::Unknown;
    }
    if &bytes[4..8] == b"ftyp"
        || &bytes[4..8] == b"moov"
        || &bytes[4..8] == b"mdat"
        || &bytes[4..8] == b"wide"
    {
        return Container::Mp4;
    }
    if bytes[0..4] == [0x1A, 0x45, 0xDF, 0xA3] {
        // DocType string appears in the EBML header.
        let head = &bytes[..bytes.len().min(64)];
        return if head.windows(4).any(|w| w == b"webm") {
            Container::WebM
        } else {
            Container::Matroska
        };
    }
    if &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"AVI " {
        return Container::Avi;
    }
    if bytes[0..3] == [0x00, 0x00, 0x01]
        || (bytes[0] == 0x47 && bytes.len() > 188 && bytes[188] == 0x47)
    {
        return Container::Mpeg;
    }
    if &bytes[0..4] == b"OggS" {
        return Container::Ogg;
    }
    if &bytes[0..3] == b"FLV" {
        return Container::Flv;
    }
    Container::Unknown
}

/// Reduced aspect ratio, nearest common ratio and orientation for a frame size.
pub fn aspect_info(w: u32, h: u32) -> Value {
    if w == 0 || h == 0 {
        return Value::Null;
    }
    let g = gcd(w, h);
    let ratio = w as f64 / h as f64;
    let common = [
        (16.0 / 9.0, "16:9"),
        (4.0 / 3.0, "4:3"),
        (21.0 / 9.0, "21:9 (ultrawide)"),
        (1.0, "1:1"),
        (9.0 / 16.0, "9:16 (vertical)"),
        (3.0 / 2.0, "3:2"),
        (2.35, "2.35:1 (cinemascope)"),
        (1.85, "1.85:1 (widescreen film)"),
        (4.0 / 5.0, "4:5 (portrait)"),
        (5.0 / 4.0, "5:4"),
    ];
    let nearest = common
        .iter()
        .min_by(|a, b| (a.0 - ratio).abs().total_cmp(&(b.0 - ratio).abs()))
        .map(|(r, n)| json!({ "name": n, "exact": (r - ratio).abs() < 0.01 }));
    json!({
        "ratio": format!("{}:{}", w / g, h / g),
        "decimal": (ratio * 1000.0).round() / 1000.0,
        "nearest_common": nearest,
        "orientation": if w > h { "landscape" } else if w < h { "portrait" } else { "square" }
    })
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// Marketing label for a frame size (`1080p`, `4K UHD`, …).
pub fn resolution_label(w: u32, h: u32) -> &'static str {
    let (long, short) = (w.max(h), w.min(h));
    match () {
        _ if long >= 7680 || short >= 4320 => "8K UHD",
        _ if long >= 3840 || short >= 2160 => "4K UHD",
        _ if long >= 2560 || short >= 1440 => "1440p (QHD)",
        _ if long >= 1920 || short >= 1080 => "1080p (Full HD)",
        _ if long >= 1280 || short >= 720 => "720p (HD)",
        _ if long >= 854 || short >= 480 => "480p (SD)",
        _ if long >= 640 || short >= 360 => "360p",
        _ if long > 0 => "low resolution",
        _ => "unknown",
    }
}

/// Nearest standard frame rate and frame duration for `fps`.
pub fn fps_info(fps: f64) -> Value {
    if fps <= 0.0 || !fps.is_finite() {
        return Value::Null;
    }
    let standards = [
        (23.976, "23.976 (film, NTSC pulldown)"),
        (24.0, "24 (cinema)"),
        (25.0, "25 (PAL)"),
        (29.97, "29.97 (NTSC)"),
        (30.0, "30"),
        (48.0, "48 (HFR cinema)"),
        (50.0, "50 (PAL double)"),
        (59.94, "59.94 (NTSC double)"),
        (60.0, "60"),
        (90.0, "90"),
        (120.0, "120"),
        (240.0, "240"),
    ];
    let nearest = standards
        .iter()
        .min_by(|a, b| (a.0 - fps).abs().total_cmp(&(b.0 - fps).abs()))
        .copied()
        .unwrap_or((30.0, "30"));
    json!({
        "fps": (fps * 1000.0).round() / 1000.0,
        "nearest_standard": nearest.1,
        "is_standard": (nearest.0 - fps).abs() < 0.02,
        "frame_duration_ms": (1000.0 / fps * 1000.0).round() / 1000.0,
        "likely_variable_frame_rate": (nearest.0 - fps).abs() > 0.5
    })
}

/// Owner of an opened MP4 reader.
pub struct Mp4Info {
    /// Parsed MP4 structure.
    pub reader: Mp4Reader<Cursor<Vec<u8>>>,
}

/// Parses an MP4 (`mp4` crate), reporting truncated/corrupt files as `E_MALFORMED`.
pub fn open_mp4(bytes: &[u8]) -> EngineResult<Mp4Reader<Cursor<Vec<u8>>>> {
    let size = bytes.len() as u64;
    Mp4Reader::read_header(Cursor::new(bytes.to_vec()), size).map_err(|e| {
        let msg = e.to_string();
        if msg.contains("moov") || msg.contains("Box not found") {
            EngineError::malformed(format!(
                "MP4 is missing required boxes ({msg}); the file may be truncated or still being \
                 written"
            ))
        } else if msg.contains("Unsupported") || msg.contains("unsupported") {
            EngineError::unsupported_format(format!("MP4 feature not supported: {msg}"))
        } else {
            EngineError::malformed(format!("Cannot parse MP4: {msg}"))
        }
    })
}

/// Container-agnostic inspection dispatching on the detected format.
pub fn inspect(bytes: &[u8], name: Option<&str>) -> EngineResult<Value> {
    engine_shared::error::require_bytes(bytes, "video")?;
    engine_shared::limits::check_size(bytes.len(), engine_shared::limits::MAX_FILE_BYTES, "video")?;
    let container = detect(bytes);
    let mut out = match container {
        Container::Mp4 => inspect_mp4(bytes)?,
        Container::Matroska | Container::WebM => inspect_matroska(bytes, container)?,
        Container::Avi | Container::Mpeg | Container::Ogg | Container::Flv => {
            return Err(EngineError::unsupported_format(format!(
                "{} containers are not supported for inspection in the WASM engine (MP4/MOV/M4V \
                 and MKV/WebM are); remux to MP4 first",
                container.label().to_uppercase()
            )));
        },
        Container::Unknown => {
            return Err(EngineError::unsupported_format(
                "Unrecognised video container (expected MP4/MOV/M4V/MKV/WebM)",
            ));
        },
    };
    out["filename"] = json!(name);
    out["file_bytes"] = json!(bytes.len());
    out["file_mb"] = json!((bytes.len() as f64 / 1_048_576.0 * 100.0).round() / 100.0);
    if let Some(d) = out["duration_seconds"].as_f64().filter(|d| *d > 0.0) {
        out["overall_bitrate_kbps"] = json!((bytes.len() as f64 * 8.0 / d / 1000.0).round());
    }
    Ok(out)
}

fn media_type_label(t: &mp4::Mp4Track) -> String {
    match t.media_type() {
        Ok(MediaType::H264) => "H.264 / AVC".into(),
        Ok(MediaType::H265) => "H.265 / HEVC".into(),
        Ok(MediaType::VP9) => "VP9".into(),
        Ok(MediaType::AAC) => "AAC".into(),
        Ok(MediaType::TTXT) => "Timed text (tx3g)".into(),
        Err(_) => t
            .box_type()
            .map(|b| format!("{b}"))
            .unwrap_or_else(|_| "unknown".into()),
    }
}

/// Full MP4 inspection: tracks, codecs, duration, bitrate, fragmentation, fast-start.
pub fn inspect_mp4(bytes: &[u8]) -> EngineResult<Value> {
    let reader = open_mp4(bytes)?;
    let mut tracks = Vec::new();
    let mut video: Option<Value> = None;
    let mut audio_tracks = 0;
    let mut subtitle_tracks = 0;
    let mut ids: Vec<&u32> = reader.tracks().keys().collect();
    ids.sort();
    for id in ids {
        let t = &reader.tracks()[id];
        let kind = t
            .track_type()
            .map(|k| format!("{k}").to_lowercase())
            .unwrap_or_else(|_| "unknown".into());
        let codec = media_type_label(t);
        let duration = t.duration().as_secs_f64();
        let mut tj = json!({
            "id": t.track_id(),
            "type": kind,
            "codec": codec,
            "duration_seconds": (duration * 1000.0).round() / 1000.0,
            "timescale": t.timescale(),
            "language": t.language(),
            "sample_count": t.sample_count(),
            "bitrate_kbps": (t.bitrate() as f64 / 1000.0).round(),
            "box": t.box_type().map(|b| b.to_string()).unwrap_or_default()
        });
        match t.track_type() {
            Ok(TrackType::Video) => {
                let (w, h) = (t.width() as u32, t.height() as u32);
                tj["width"] = json!(w);
                tj["height"] = json!(h);
                tj["frame_rate"] = json!((t.frame_rate() * 1000.0).round() / 1000.0);
                tj["profile"] = json!(t.video_profile().map(|p| format!("{p:?}")).ok());
                tj["has_sps_pps"] =
                    json!(t.sequence_parameter_set().is_ok() && t.picture_parameter_set().is_ok());
                if video.is_none() {
                    video = Some(json!({
                        "width": w,
                        "height": h,
                        "frame_rate": t.frame_rate(),
                        "codec": media_type_label(t),
                        "track_id": t.track_id()
                    }));
                }
            },
            Ok(TrackType::Audio) => {
                audio_tracks += 1;
                tj["sample_rate"] = json!(t.sample_freq_index().map(|f| f.freq()).ok());
                tj["channels"] = json!(t.channel_config().map(|c| c as u8).ok());
                tj["audio_profile"] = json!(t.audio_profile().map(|p| format!("{p:?}")).ok());
            },
            Ok(TrackType::Subtitle) => subtitle_tracks += 1,
            Err(_) => {},
        }
        tracks.push(tj);
    }
    let duration = reader.duration().as_secs_f64();
    let meta = reader.metadata();
    let mut metadata = Map::new();
    if let Some(t) = meta.title() {
        metadata.insert("title".into(), json!(t));
    }
    if let Some(y) = meta.year() {
        metadata.insert("year".into(), json!(y));
    }
    if let Some(s) = meta.summary() {
        metadata.insert("summary".into(), json!(s));
    }
    if let Some(p) = meta.poster() {
        metadata.insert("poster_bytes".into(), json!(p.len()));
    }
    let (w, h, fps) = video
        .as_ref()
        .map(|v| {
            (
                v["width"].as_u64().unwrap_or(0) as u32,
                v["height"].as_u64().unwrap_or(0) as u32,
                v["frame_rate"].as_f64().unwrap_or(0.0),
            )
        })
        .unwrap_or((0, 0, 0.0));
    let faststart = {
        // moov before mdat → progressive streaming friendly.
        let moov = find_box(bytes, b"moov");
        let mdat = find_box(bytes, b"mdat");
        match (moov, mdat) {
            (Some(m), Some(d)) => Some(m < d),
            _ => None,
        }
    };
    Ok(json!({
        "container": "mp4",
        "brand": reader.major_brand().to_string(),
        "compatible_brands": reader.compatible_brands().iter().map(|b| b.to_string()).collect::<Vec<_>>(),
        "duration_seconds": (duration * 1000.0).round() / 1000.0,
        "duration_display": hms(duration),
        "timescale": reader.timescale(),
        "fragmented": reader.is_fragmented(),
        "fast_start": faststart,
        "tracks": tracks,
        "track_counts": {
            "video": video.is_some() as u8,
            "audio": audio_tracks,
            "subtitle": subtitle_tracks
        },
        "video": video,
        "resolution": if w > 0 { json!({ "width": w, "height": h, "label": resolution_label(w, h), "megapixels": (w as f64 * h as f64 / 1e6 * 100.0).round() / 100.0 }) } else { Value::Null },
        "aspect_ratio": aspect_info(w, h),
        "frame_rate": fps_info(fps),
        "estimated_frames": if fps > 0.0 { json!((duration * fps).round()) } else { Value::Null },
        "metadata": metadata,
        "lossless_edit_supported": tracks.iter().all(|t| matches!(t["codec"].as_str(), Some("H.264 / AVC") | Some("AAC") | Some("Timed text (tx3g)"))),
    }))
}

fn find_box(bytes: &[u8], name: &[u8; 4]) -> Option<usize> {
    let mut pos = 0usize;
    while pos + 8 <= bytes.len() {
        let size =
            u32::from_be_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]) as u64;
        let typ = &bytes[pos + 4..pos + 8];
        if typ == name {
            return Some(pos);
        }
        let size = if size == 1 && pos + 16 <= bytes.len() {
            bytes
                .get(pos + 8..pos + 16)
                .and_then(|b| <[u8; 8]>::try_from(b).ok())
                .map(u64::from_be_bytes)
                .unwrap_or(0)
        } else if size == 0 {
            (bytes.len() - pos) as u64
        } else {
            size
        };
        if size < 8 {
            return None;
        }
        pos = pos.saturating_add(size as usize);
    }
    None
}

/// Formats seconds as `hh:mm:ss.mmm`.
pub fn hms(seconds: f64) -> String {
    let total = seconds.max(0.0);
    let h = (total / 3600.0).floor() as u64;
    let m = ((total % 3600.0) / 60.0).floor() as u64;
    let s = total % 60.0;
    if h > 0 {
        format!("{h}:{m:02}:{s:06.3}")
    } else {
        format!("{m}:{s:06.3}")
    }
}

fn inspect_matroska(bytes: &[u8], container: Container) -> EngineResult<Value> {
    let mkv = matroska::Matroska::open(Cursor::new(bytes))
        .map_err(|e| EngineError::malformed(format!("Cannot parse Matroska/WebM: {e}")))?;
    let mut tracks = Vec::new();
    let mut video: Option<Value> = None;
    let (mut audio_n, mut sub_n) = (0, 0);
    for t in &mkv.tracks {
        let kind = match t.tracktype {
            matroska::Tracktype::Video => "video",
            matroska::Tracktype::Audio => "audio",
            matroska::Tracktype::Subtitle => "subtitle",
            matroska::Tracktype::Complex => "complex",
            matroska::Tracktype::Logo => "logo",
            matroska::Tracktype::Buttons => "buttons",
            matroska::Tracktype::Control => "control",
            matroska::Tracktype::Unknown => "unknown",
        };
        let codec = codec_name(&t.codec_id);
        let mut tj = json!({
            "id": t.number,
            "type": kind,
            "codec": codec,
            "codec_id": t.codec_id,
            "name": t.name,
            "language": t.language.as_ref().map(|l| format!("{l:?}")),
            "default": t.default,
            "forced": t.forced,
            "enabled": t.enabled,
            "codec_name": t.codec_name
        });
        if let Some(d) = t.default_duration {
            let fps = 1.0 / d.as_secs_f64();
            tj["frame_rate"] = json!((fps * 1000.0).round() / 1000.0);
        }
        match &t.settings {
            matroska::Settings::Video(v) => {
                tj["width"] = json!(v.pixel_width);
                tj["height"] = json!(v.pixel_height);
                tj["display_width"] = json!(v.display_width);
                tj["display_height"] = json!(v.display_height);
                if video.is_none() {
                    video = Some(json!({
                        "width": v.pixel_width,
                        "height": v.pixel_height,
                        "frame_rate": tj["frame_rate"].as_f64().unwrap_or(0.0),
                        "codec": codec,
                        "track_id": t.number
                    }));
                }
            },
            matroska::Settings::Audio(a) => {
                audio_n += 1;
                tj["sample_rate"] = json!(a.sample_rate);
                tj["channels"] = json!(a.channels);
                tj["bit_depth"] = json!(a.bit_depth);
            },
            matroska::Settings::None => {
                if kind == "subtitle" {
                    sub_n += 1;
                }
            },
        }
        tracks.push(tj);
    }
    let duration = mkv.info.duration.map(|d| d.as_secs_f64()).unwrap_or(0.0);
    let (w, h, fps) = video
        .as_ref()
        .map(|v| {
            (
                v["width"].as_u64().unwrap_or(0) as u32,
                v["height"].as_u64().unwrap_or(0) as u32,
                v["frame_rate"].as_f64().unwrap_or(0.0),
            )
        })
        .unwrap_or((0, 0, 0.0));
    let mut metadata = Map::new();
    if let Some(t) = &mkv.info.title {
        metadata.insert("title".into(), json!(t));
    }
    if !mkv.info.muxing_app.is_empty() {
        metadata.insert("muxing_app".into(), json!(mkv.info.muxing_app));
    }
    if !mkv.info.writing_app.is_empty() {
        metadata.insert("writing_app".into(), json!(mkv.info.writing_app));
    }
    let chapters: Vec<Value> = mkv
        .chapters
        .iter()
        .flat_map(|e| e.chapters.iter())
        .map(|c| {
            json!({
                "start_seconds": c.time_start.as_secs_f64(),
                "end_seconds": c.time_end.map(|d| d.as_secs_f64()),
                "title": c.display.first().map(|d| d.string.clone())
            })
        })
        .collect();
    Ok(json!({
        "container": container.label(),
        "duration_seconds": (duration * 1000.0).round() / 1000.0,
        "duration_display": hms(duration),
        "tracks": tracks,
        "track_counts": { "video": video.is_some() as u8, "audio": audio_n, "subtitle": sub_n },
        "video": video,
        "resolution": if w > 0 { json!({ "width": w, "height": h, "label": resolution_label(w, h), "megapixels": (w as f64 * h as f64 / 1e6 * 100.0).round() / 100.0 }) } else { Value::Null },
        "aspect_ratio": aspect_info(w, h),
        "frame_rate": fps_info(fps),
        "estimated_frames": if fps > 0.0 { json!((duration * fps).round()) } else { Value::Null },
        "metadata": metadata,
        "chapters": chapters,
        "attachments": mkv.attachments.iter().map(|a| json!({ "name": a.name, "mime": a.mime_type, "bytes": a.data.len() })).collect::<Vec<_>>(),
        "tags": mkv.tags.iter().flat_map(|t| t.simple.iter()).map(|s| json!({ "name": s.name, "value": match &s.value { Some(matroska::TagValue::String(v)) => json!(v), Some(matroska::TagValue::Binary(b)) => json!(format!("<{} bytes>", b.len())), None => Value::Null } })).collect::<Vec<_>>(),
        "lossless_edit_supported": false,
        "note": "Matroska/WebM editing (trim, mute) is not available in the WASM engine; use the \
                 backend ffmpeg service",
    }))
}

fn codec_name(id: &str) -> &'static str {
    match id {
        "V_MPEG4/ISO/AVC" => "H.264 / AVC",
        "V_MPEGH/ISO/HEVC" => "H.265 / HEVC",
        "V_VP8" => "VP8",
        "V_VP9" => "VP9",
        "V_AV1" => "AV1",
        "V_MPEG2" => "MPEG-2",
        "V_THEORA" => "Theora",
        "A_AAC" | "A_AAC/MPEG4/LC" => "AAC",
        "A_OPUS" => "Opus",
        "A_VORBIS" => "Vorbis",
        "A_MPEG/L3" => "MP3",
        "A_AC3" => "AC-3",
        "A_EAC3" => "E-AC-3",
        "A_FLAC" => "FLAC",
        "A_PCM/INT/LIT" => "PCM",
        "A_DTS" => "DTS",
        "A_TRUEHD" => "TrueHD",
        "S_TEXT/UTF8" => "SubRip (SRT)",
        "S_TEXT/ASS" | "S_TEXT/SSA" => "ASS/SSA",
        "S_TEXT/WEBVTT" => "WebVTT",
        "S_VOBSUB" => "VobSub",
        "S_HDMV/PGS" => "PGS",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(aspect_info(1920, 1080)["ratio"], "16:9");
        assert_eq!(aspect_info(1080, 1920)["orientation"], "portrait");
        assert_eq!(resolution_label(3840, 2160), "4K UHD");
        assert_eq!(resolution_label(1280, 720), "720p (HD)");
        assert_eq!(fps_info(29.97)["nearest_standard"], "29.97 (NTSC)");
        assert_eq!(hms(3725.5), "1:02:05.500");
        assert_eq!(detect(b"\x00\x00\x00\x20ftypisom\x00\x00\x02\x00isomiso2"), Container::Mp4);
        assert_eq!(
            detect(&[0x1A, 0x45, 0xDF, 0xA3, 0, 0, 0, 0, b'w', b'e', b'b', b'm', 0]),
            Container::WebM
        );
        assert!(inspect(b"definitely not a video file, nope nope", None).is_err());
        assert!(
            matches!(inspect(b"\x00\x00\x00\x20ftypisom\x00\x00\x02\x00isomiso2", None), Err(e) if e.code == engine_shared::error::ErrorCode::Malformed)
        );
    }
}
