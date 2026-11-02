//! Lossless MP4 editing (no decoding): keyframe-aligned trim/split, drop
//! tracks (mute / video-only), extract the AAC track to M4A, loop, and
//! concatenate files with identical codec configuration.

use crate::inspect::open_mp4;
use engine_shared::{EngineError, EngineResult};
use mp4::{
    AacConfig, AvcConfig, MediaConfig, MediaType, Mp4Config, Mp4Reader, Mp4Sample, Mp4Writer,
    TrackConfig, TrackType, TtxtConfig,
};
use serde_json::{json, Value};
use std::io::Cursor;

type Reader = Mp4Reader<Cursor<Vec<u8>>>;

fn track_config(r: &Reader, id: u32) -> EngineResult<TrackConfig> {
    let t = &r.tracks()[&id];
    let media_conf = match t
        .media_type()
        .map_err(|e| EngineError::unsupported_format(format!("Track {id}: {e}")))?
    {
        MediaType::H264 => MediaConfig::AvcConfig(AvcConfig {
            width: t.width(),
            height: t.height(),
            seq_param_set: t
                .sequence_parameter_set()
                .map_err(|e| EngineError::malformed(format!("Track {id} has no SPS: {e}")))?
                .to_vec(),
            pic_param_set: t
                .picture_parameter_set()
                .map_err(|e| EngineError::malformed(format!("Track {id} has no PPS: {e}")))?
                .to_vec(),
        }),
        MediaType::AAC => MediaConfig::AacConfig(AacConfig {
            bitrate: t.bitrate(),
            profile: t.audio_profile().map_err(|e| EngineError::malformed(e.to_string()))?,
            freq_index: t
                .sample_freq_index()
                .map_err(|e| EngineError::malformed(e.to_string()))?,
            chan_conf: t
                .channel_config()
                .map_err(|e| EngineError::malformed(e.to_string()))?,
        }),
        MediaType::TTXT => MediaConfig::TtxtConfig(TtxtConfig {}),
        other => {
            return Err(EngineError::unsupported(format!(
                "Lossless editing supports H.264 video and AAC audio; track {id} is {other} \
                 (HEVC/VP9 output would lose its decoder configuration). Use the backend ffmpeg \
                 service for this file."
            )))
        },
    };
    Ok(TrackConfig {
        track_type: t.track_type().map_err(|e| EngineError::malformed(e.to_string()))?,
        timescale: t.timescale(),
        language: t.language().to_string(),
        media_conf,
    })
}

/// Samples of a track within [start, end) seconds, snapped to the previous
/// sync sample for video. Returns (samples, actual_start, actual_end).
fn collect_samples(
    r: &mut Reader,
    id: u32,
    start: f64,
    end: f64,
    snap_to_keyframe: bool,
) -> EngineResult<(Vec<Mp4Sample>, f64, f64)> {
    let timescale = r.tracks()[&id].timescale() as f64;
    let count = r
        .sample_count(id)
        .map_err(|e| EngineError::malformed(e.to_string()))?;
    let mut out = Vec::new();
    let mut actual_start: Option<f64> = None;
    let mut last_sync_before: Option<u32> = None;
    // First pass: find sync sample at/before start.
    let mut first_id = 1u32;
    for sid in 1..=count {
        let s = match r.read_sample(id, sid).map_err(|e| {
            EngineError::malformed(format!("Cannot read sample {sid} of track {id}: {e}"))
        })? {
            Some(s) => s,
            None => break,
        };
        let t = s.start_time as f64 / timescale;
        if s.is_sync || !snap_to_keyframe {
            last_sync_before = Some(sid);
        }
        if t >= start {
            first_id = if snap_to_keyframe {
                last_sync_before.unwrap_or(sid)
            } else {
                sid
            };
            break;
        }
        if sid == count {
            first_id = count + 1;
        }
    }
    let mut base: Option<u64> = None;
    let mut end_time = start;
    for sid in first_id..=count {
        let s = match r
            .read_sample(id, sid)
            .map_err(|e| EngineError::malformed(e.to_string()))?
        {
            Some(s) => s,
            None => break,
        };
        let t = s.start_time as f64 / timescale;
        if t >= end {
            break;
        }
        let b = *base.get_or_insert(s.start_time);
        if actual_start.is_none() {
            actual_start = Some(t);
        }
        end_time = t + s.duration as f64 / timescale;
        out.push(Mp4Sample {
            start_time: s.start_time - b,
            duration: s.duration,
            rendering_offset: s.rendering_offset,
            is_sync: s.is_sync,
            bytes: s.bytes,
        });
    }
    Ok((out, actual_start.unwrap_or(start), end_time))
}

fn writer_config(r: &Reader) -> Mp4Config {
    Mp4Config {
        major_brand: *r.major_brand(),
        minor_version: r.minor_version(),
        compatible_brands: r.compatible_brands().to_vec(),
        timescale: r.timescale(),
    }
}

fn new_writer(r: &Reader) -> EngineResult<Mp4Writer<Cursor<Vec<u8>>>> {
    Mp4Writer::write_start(Cursor::new(Vec::new()), &writer_config(r))
        .map_err(|e| EngineError::internal(e.to_string()))
}

fn sorted_track_ids(r: &Reader) -> Vec<u32> {
    let mut ids: Vec<u32> = r.tracks().keys().copied().collect();
    ids.sort_unstable();
    ids
}

fn finish(mut w: Mp4Writer<Cursor<Vec<u8>>>) -> EngineResult<Vec<u8>> {
    w.write_end().map_err(|e| EngineError::internal(e.to_string()))?;
    Ok(w.into_writer().into_inner())
}

fn file_json(bytes: Vec<u8>, name: &str, ext: &str) -> Value {
    let mime = match ext {
        "m4a" => "audio/mp4",
        _ => "video/mp4",
    };
    json!({
        "file": engine_shared::b64::encode(&bytes),
        "filename": format!("{name}.{ext}"),
        "mime": mime,
        "bytes": bytes.len()
    })
}

fn check_fragmented(r: &Reader) -> EngineResult<()> {
    if r.is_fragmented() {
        return Err(EngineError::unsupported(
            "Fragmented MP4 (fMP4/DASH segments) cannot be edited losslessly here; remux to a \
             progressive MP4 first",
        ));
    }
    Ok(())
}

/// Copies the selected tracks for [start, end) into a new MP4.
pub fn trim(
    bytes: &[u8],
    start: f64,
    end: Option<f64>,
    keep: &[TrackType],
    name: &str,
) -> EngineResult<Value> {
    let mut r = open_mp4(bytes)?;
    check_fragmented(&r)?;
    let duration = r.duration().as_secs_f64();
    let end = end.unwrap_or(duration);
    if start < 0.0 || end <= start || start >= duration {
        return Err(EngineError::invalid(format!(
            "Invalid range {start}s..{end}s (video is {duration:.3}s)"
        )));
    }
    let ids = sorted_track_ids(&r);
    let mut w = new_writer(&r)?;
    let mut kept = Vec::new();
    let mut actual = (f64::MAX, 0.0f64);
    let mut new_id = 0u32;
    for id in ids {
        let kind = r.tracks()[&id]
            .track_type()
            .map_err(|e| EngineError::malformed(e.to_string()))?;
        if !keep.is_empty() && !keep.contains(&kind) {
            continue;
        }
        let cfg = track_config(&r, id)?;
        let (samples, s, e) = collect_samples(&mut r, id, start, end, kind == TrackType::Video)?;
        if samples.is_empty() {
            continue;
        }
        w.add_track(&cfg).map_err(|e| EngineError::internal(e.to_string()))?;
        new_id += 1;
        for smp in &samples {
            w.write_sample(new_id, smp)
                .map_err(|e| EngineError::internal(e.to_string()))?;
        }
        actual.0 = actual.0.min(s);
        actual.1 = actual.1.max(e);
        kept.push(json!({
            "source_track": id,
            "type": format!("{kind}").to_lowercase(),
            "samples": samples.len(),
            "start": (s * 1000.0).round() / 1000.0,
            "end": (e * 1000.0).round() / 1000.0
        }));
    }
    if kept.is_empty() {
        return Err(EngineError::invalid("No samples fall inside the requested range"));
    }
    let out = finish(w)?;
    let mut v = file_json(out, name, "mp4");
    v["requested"] = json!({ "start": start, "end": end });
    v["actual"] = json!({
        "start": (actual.0 * 1000.0).round() / 1000.0,
        "end": (actual.1 * 1000.0).round() / 1000.0,
        "note": "Video cuts snap to the previous keyframe (no re-encoding)"
    });
    v["tracks"] = json!(kept);
    v["source_duration"] = json!(duration);
    Ok(v)
}

/// Splits into consecutive segments at the given times (keyframe aligned).
pub fn split(bytes: &[u8], mut points: Vec<f64>, name: &str) -> EngineResult<Value> {
    let r = open_mp4(bytes)?;
    let duration = r.duration().as_secs_f64();
    drop(r);
    points.retain(|p| *p > 0.0 && *p < duration);
    points.sort_by(f64::total_cmp);
    points.dedup();
    if points.is_empty() {
        return Err(EngineError::invalid("No split points inside the video"));
    }
    if points.len() > 100 {
        return Err(EngineError::limit("At most 100 split points"));
    }
    let mut files = Vec::new();
    let mut start = 0.0;
    for (i, p) in points.iter().chain(std::iter::once(&duration)).enumerate() {
        let mut part = trim(bytes, start, Some(*p), &[], &format!("{name}_part{:02}", i + 1))?;
        part["index"] = json!(i + 1);
        files.push(part);
        start = *p;
    }
    Ok(json!({
        "files": files,
        "count": files.len(),
        "split_points": points,
        "source_duration": duration
    }))
}

/// Removes all audio tracks.
pub fn mute(bytes: &[u8], name: &str) -> EngineResult<Value> {
    let r = open_mp4(bytes)?;
    if !r
        .tracks()
        .values()
        .any(|t| matches!(t.track_type(), Ok(TrackType::Video)))
    {
        return Err(EngineError::invalid("File has no video track"));
    }
    if !r
        .tracks()
        .values()
        .any(|t| matches!(t.track_type(), Ok(TrackType::Audio)))
    {
        return Err(EngineError::invalid("File already has no audio track"));
    }
    drop(r);
    let mut v = trim(
        bytes,
        0.0,
        None,
        &[TrackType::Video, TrackType::Subtitle],
        &format!("{name}_muted"),
    )?;
    v["audio_removed"] = json!(true);
    Ok(v)
}

/// Copies the AAC track into an .m4a container.
pub fn extract_audio(bytes: &[u8], name: &str) -> EngineResult<Value> {
    let r = open_mp4(bytes)?;
    let audio = r
        .tracks()
        .values()
        .find(|t| matches!(t.track_type(), Ok(TrackType::Audio)));
    let Some(a) = audio else {
        return Err(EngineError::not_found("File has no audio track"));
    };
    if !matches!(a.media_type(), Ok(MediaType::AAC)) {
        return Err(EngineError::unsupported(format!(
            "Audio track is {} – only AAC can be copied without transcoding; use the audio engine \
             `extract_from_video` to decode to WAV/FLAC instead",
            a.media_type()
                .map(|m| m.to_string())
                .unwrap_or_else(|_| "unknown".into())
        )));
    }
    drop(r);
    let mut v = trim(bytes, 0.0, None, &[TrackType::Audio], &format!("{name}_audio"))?;
    v["filename"] = json!(format!("{name}_audio.m4a"));
    v["mime"] = json!("audio/mp4");
    Ok(v)
}

/// Repeats the whole file `times` times.
pub fn repeat(bytes: &[u8], times: u32, name: &str) -> EngineResult<Value> {
    if !(2..=50).contains(&times) {
        return Err(EngineError::invalid("times must be 2-50"));
    }
    let mut r = open_mp4(bytes)?;
    check_fragmented(&r)?;
    let duration = r.duration().as_secs_f64();
    engine_shared::limits::check_size(
        bytes.len().saturating_mul(times as usize),
        engine_shared::limits::MAX_FILE_BYTES,
        "output",
    )?;
    let ids = sorted_track_ids(&r);
    let mut w = new_writer(&r)?;
    let mut new_id = 0;
    for id in ids {
        let cfg = track_config(&r, id)?;
        let (samples, _, _) = collect_samples(&mut r, id, 0.0, f64::MAX, false)?;
        if samples.is_empty() {
            continue;
        }
        w.add_track(&cfg).map_err(|e| EngineError::internal(e.to_string()))?;
        new_id += 1;
        let total: u64 = samples.iter().map(|s| s.duration as u64).sum();
        for k in 0..times as u64 {
            for s in &samples {
                let shifted = Mp4Sample {
                    start_time: s.start_time + k * total,
                    duration: s.duration,
                    rendering_offset: s.rendering_offset,
                    is_sync: s.is_sync,
                    bytes: s.bytes.clone(),
                };
                w.write_sample(new_id, &shifted)
                    .map_err(|e| EngineError::internal(e.to_string()))?;
            }
        }
    }
    let out = finish(w)?;
    let mut v = file_json(out, &format!("{name}_loop{times}"), "mp4");
    v["times"] = json!(times);
    v["duration_seconds"] = json!((duration * times as f64 * 1000.0).round() / 1000.0);
    Ok(v)
}

/// Concatenates MP4 files whose tracks have identical codec configuration.
pub fn concat(files: &[Vec<u8>], name: &str) -> EngineResult<Value> {
    if files.len() < 2 {
        return Err(EngineError::invalid("Provide at least two files to concatenate"));
    }
    engine_shared::limits::check_count(files.len(), 50, "files")?;
    let mut readers: Vec<Reader> =
        files.iter().map(|f| open_mp4(f)).collect::<EngineResult<_>>()?;
    for r in &readers {
        check_fragmented(r)?;
    }
    let first_ids = sorted_track_ids(&readers[0]);
    let configs: Vec<TrackConfig> = first_ids
        .iter()
        .map(|id| track_config(&readers[0], *id))
        .collect::<EngineResult<_>>()?;
    for (i, r) in readers.iter().enumerate().skip(1) {
        let ids = sorted_track_ids(r);
        if ids.len() != first_ids.len() {
            return Err(EngineError::invalid(format!(
                "File {} has {} tracks but the first file has {}",
                i + 1,
                ids.len(),
                first_ids.len()
            )));
        }
        for (k, id) in ids.iter().enumerate() {
            let cfg = track_config(r, *id)?;
            if cfg != configs[k] {
                return Err(EngineError::invalid(format!(
                    "File {} track {} has a different codec configuration (resolution, SPS/PPS, \
                     sample rate or timescale) – lossless concat needs identical encodes; \
                     re-encode with the backend ffmpeg service",
                    i + 1,
                    id
                )));
            }
        }
    }
    let cfg0 = writer_config(&readers[0]);
    let mut w = Mp4Writer::write_start(Cursor::new(Vec::new()), &cfg0)
        .map_err(|e| EngineError::internal(e.to_string()))?;
    for cfg in &configs {
        w.add_track(cfg).map_err(|e| EngineError::internal(e.to_string()))?;
    }
    let mut offsets: Vec<u64> = vec![0; configs.len()];
    let mut total_duration = 0.0;
    for r in readers.iter_mut() {
        total_duration += r.duration().as_secs_f64();
        let ids = sorted_track_ids(r);
        for (k, id) in ids.iter().enumerate() {
            let (samples, _, _) = collect_samples(r, *id, 0.0, f64::MAX, false)?;
            let mut end = 0u64;
            for s in &samples {
                let shifted = Mp4Sample {
                    start_time: s.start_time + offsets[k],
                    duration: s.duration,
                    rendering_offset: s.rendering_offset,
                    is_sync: s.is_sync,
                    bytes: s.bytes.clone(),
                };
                end = end.max(s.start_time + s.duration as u64);
                w.write_sample(k as u32 + 1, &shifted)
                    .map_err(|e| EngineError::internal(e.to_string()))?;
            }
            offsets[k] += end;
        }
    }
    let out = finish(w)?;
    let mut v = file_json(out, name, "mp4");
    v["inputs"] = json!(files.len());
    v["duration_seconds"] = json!((total_duration * 1000.0).round() / 1000.0);
    Ok(v)
}

/// Extracts tx3g timed-text tracks as SRT.
pub fn extract_subtitles(bytes: &[u8]) -> EngineResult<Value> {
    let mut r = open_mp4(bytes)?;
    let ids: Vec<u32> = sorted_track_ids(&r)
        .into_iter()
        .filter(|id| matches!(r.tracks()[id].track_type(), Ok(TrackType::Subtitle)))
        .collect();
    if ids.is_empty() {
        return Err(EngineError::not_found(
            "No subtitle (tx3g) track in this MP4. Soft subtitles in MKV and burned-in captions \
             cannot be extracted here.",
        ));
    }
    let mut tracks = Vec::new();
    for id in ids {
        let timescale = r.tracks()[&id].timescale() as f64;
        let lang = r.tracks()[&id].language().to_string();
        let (samples, _, _) = collect_samples(&mut r, id, 0.0, f64::MAX, false)?;
        let mut cues = Vec::new();
        for s in samples {
            if s.bytes.len() < 2 {
                continue;
            }
            let len = u16::from_be_bytes([s.bytes[0], s.bytes[1]]) as usize;
            let text =
                String::from_utf8_lossy(&s.bytes[2..(2 + len).min(s.bytes.len())]).to_string();
            if text.trim().is_empty() {
                continue;
            }
            cues.push(crate::subtitles::Cue {
                start: s.start_time as f64 / timescale,
                end: (s.start_time + s.duration as u64) as f64 / timescale,
                text,
                style: None,
            });
        }
        let srt = crate::subtitles::to_srt(&cues);
        tracks.push(json!({
            "track_id": id,
            "language": lang,
            "cues": cues.len(),
            "srt": srt,
            "vtt": crate::subtitles::to_vtt(&cues)
        }));
    }
    Ok(json!({ "tracks": tracks, "count": tracks.len() }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = include_bytes!("../tests/fixtures/sample.mp4");

    #[test]
    fn lossless_ops() {
        let info = crate::inspect::inspect(SAMPLE, Some("sample.mp4")).unwrap();
        let dur = info["duration_seconds"].as_f64().unwrap();
        assert!(dur > 1.0, "{info}");
        let t = trim(SAMPLE, 0.5, Some((dur - 0.5).max(1.0)), &[], "x").unwrap();
        let out = engine_shared::b64::decode(t["file"].as_str().unwrap()).unwrap();
        let back = crate::inspect::inspect(&out, None).unwrap();
        assert!(back["duration_seconds"].as_f64().unwrap() < dur);
        assert!(back["duration_seconds"].as_f64().unwrap() > 0.5);
        assert_eq!(back["track_counts"]["video"], 1);
        let s = split(SAMPLE, vec![dur / 2.0], "x").unwrap();
        assert_eq!(s["count"], 2);
        let has_audio = info["track_counts"]["audio"].as_u64().unwrap() > 0;
        if has_audio {
            let m = mute(SAMPLE, "x").unwrap();
            let mb = engine_shared::b64::decode(m["file"].as_str().unwrap()).unwrap();
            assert_eq!(crate::inspect::inspect(&mb, None).unwrap()["track_counts"]["audio"], 0);
            let a = extract_audio(SAMPLE, "x").unwrap();
            assert_eq!(a["mime"], "audio/mp4");
        } else {
            assert!(mute(SAMPLE, "x").is_err());
            assert!(extract_audio(SAMPLE, "x").is_err());
        }
        let l = repeat(SAMPLE, 2, "x").unwrap();
        let lb = engine_shared::b64::decode(l["file"].as_str().unwrap()).unwrap();
        let ld = crate::inspect::inspect(&lb, None).unwrap()["duration_seconds"]
            .as_f64()
            .unwrap();
        assert!((ld - dur * 2.0).abs() < dur * 0.05, "{ld} vs {dur}");
        let c = concat(&[SAMPLE.to_vec(), SAMPLE.to_vec()], "x").unwrap();
        let cb = engine_shared::b64::decode(c["file"].as_str().unwrap()).unwrap();
        assert!(
            (crate::inspect::inspect(&cb, None).unwrap()["duration_seconds"]
                .as_f64()
                .unwrap()
                - dur * 2.0)
                .abs()
                < dur * 0.05
        );
        assert!(trim(SAMPLE, dur + 5.0, None, &[], "x").is_err());
        assert!(extract_subtitles(SAMPLE).is_err());
        assert!(concat(&[SAMPLE.to_vec()], "x").is_err());
    }
}
