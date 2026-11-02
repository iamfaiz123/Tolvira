//! Decoding (symphonia: WAV/MP3/FLAC/OGG Vorbis/AAC/M4A/ALAC/MKV-audio) and
//! encoding (WAV via hound, FLAC via flacenc).

use crate::pcm::Clip;
use engine_shared::{EngineError, EngineResult};
use serde_json::{json, Map, Value};
use std::io::Cursor;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Result of decoding: PCM plus container/codec facts and tags.
pub struct Decoded {
    /// Decoded PCM.
    pub clip: Clip,
    /// Container, codec, duration, bitrate and tag information.
    pub info: Value,
}

fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() < 12 {
        return None;
    }
    if &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
        return Some("wav");
    }
    if &bytes[0..4] == b"fLaC" {
        return Some("flac");
    }
    if &bytes[0..4] == b"OggS" {
        return Some("ogg");
    }
    if &bytes[0..3] == b"ID3" || (bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0) {
        return Some("mp3");
    }
    if &bytes[4..8] == b"ftyp" {
        return Some("m4a");
    }
    if bytes[0..4] == [0x1A, 0x45, 0xDF, 0xA3] {
        return Some("mkv");
    }
    if &bytes[0..4] == b"FORM" && &bytes[8..12] == b"AIFF" {
        return Some("aiff");
    }
    None
}

fn hint_from(name: Option<&str>, bytes: &[u8]) -> (Hint, String) {
    let mut hint = Hint::new();
    let sniffed = sniff(bytes);
    let ext = name
        .and_then(|n| n.rsplit('.').next())
        .map(|e| e.to_ascii_lowercase());
    if let Some(s) = sniffed {
        hint.with_extension(s);
    } else if let Some(e) = &ext {
        hint.with_extension(e);
    }
    (
        hint,
        sniffed
            .map(str::to_string)
            .or(ext)
            .unwrap_or_else(|| "unknown".into()),
    )
}

/// Decodes WAV/FLAC/MP3/OGG/AAC/… (symphonia) into interleaved f32 PCM.
pub fn decode(bytes: &[u8], name: Option<&str>) -> EngineResult<Decoded> {
    engine_shared::error::require_bytes(bytes, "audio")?;
    engine_shared::limits::check_size(bytes.len(), engine_shared::limits::MAX_FILE_BYTES, "audio")?;
    let (hint, container) = hint_from(name, bytes);
    if container == "aiff" {
        return Err(EngineError::unsupported_format("AIFF is not supported; convert to WAV first"));
    }
    let mss = MediaSourceStream::new(Box::new(Cursor::new(bytes.to_vec())), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions {
                enable_gapless: true,
                ..Default::default()
            },
            &MetadataOptions::default(),
        )
        .map_err(|e| match e {
            symphonia::core::errors::Error::Unsupported(m) => {
                EngineError::unsupported_format(format!("Unsupported audio container: {m}"))
            },
            other => EngineError::malformed(format!("Cannot read audio file: {other}")),
        })?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .cloned()
        .ok_or_else(|| {
            EngineError::unsupported_format(
                "No decodable audio track found (video-only or unknown codec)",
            )
        })?;
    let params = track.codec_params.clone();
    let mut decoder = symphonia::default::get_codecs()
        .make(&params, &DecoderOptions::default())
        .map_err(|e| EngineError::unsupported_format(format!("Unsupported audio codec: {e}")))?;
    let mut samples: Vec<f32> = Vec::new();
    let mut sample_rate = params.sample_rate.unwrap_or(0);
    let mut channels = params.channels.map(|c| c.count()).unwrap_or(0) as u16;
    let mut decode_errors = 0usize;
    let mut buf: Option<SampleBuffer<f32>> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break
            },
            Err(symphonia::core::errors::Error::ResetRequired) => break,
            Err(e) => {
                if samples.is_empty() {
                    return Err(EngineError::malformed(format!("Corrupt audio stream: {e}")));
                }
                break;
            },
        };
        if packet.track_id() != track.id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(audio) => {
                let spec = *audio.spec();
                sample_rate = spec.rate;
                channels = spec.channels.count() as u16;
                let need = audio.capacity() as u64;
                let b = match buf.as_mut() {
                    Some(b) if b.capacity() >= need as usize * channels as usize => b,
                    _ => buf.insert(SampleBuffer::<f32>::new(need, spec)),
                };
                b.copy_interleaved_ref(audio);
                samples.extend_from_slice(b.samples());
                Clip::check_len(samples.len())?;
            },
            Err(symphonia::core::errors::Error::DecodeError(_)) => {
                decode_errors += 1;
                if decode_errors > 64 && samples.is_empty() {
                    return Err(EngineError::malformed(
                        "Audio stream is corrupt (too many undecodable packets)",
                    ));
                }
            },
            Err(e) => return Err(EngineError::malformed(format!("Decode failed: {e}"))),
        }
    }
    if samples.is_empty() || sample_rate == 0 || channels == 0 {
        return Err(EngineError::malformed("No audio samples could be decoded"));
    }
    // Some encoders pad the final block; trust the declared frame count when present.
    if let Some(n) = params.n_frames {
        let declared = n as usize * channels as usize;
        if declared > 0 && samples.len() > declared {
            samples.truncate(declared);
        }
    }
    // Tags.
    let mut tags = Map::new();
    let mut collect = |meta: &symphonia::core::meta::MetadataRevision| {
        for t in meta.tags() {
            let key = t
                .std_key
                .map(|k| format!("{k:?}").to_lowercase())
                .unwrap_or_else(|| t.key.to_lowercase());
            let val = t.value.to_string();
            if !val.is_empty() && val.len() < 4096 {
                tags.entry(key).or_insert(Value::String(val));
            }
        }
    };
    let mut probed_meta = probed.metadata;
    if let Some(m) = probed_meta.get().as_ref().and_then(|m| m.current()) {
        collect(m);
    }
    if let Some(m) = format.metadata().current() {
        collect(m);
    }
    let codec_name = symphonia::default::get_codecs()
        .get_codec(params.codec)
        .map(|d| d.short_name.to_string())
        .unwrap_or_else(|| format!("{:?}", params.codec));
    let duration = samples.len() as f64 / channels as f64 / sample_rate as f64;
    let info = json!({
        "container": container,
        "codec": codec_name,
        "sample_rate": sample_rate,
        "channels": channels,
        "bits_per_sample": params.bits_per_sample,
        "duration_seconds": (duration * 1000.0).round() / 1000.0,
        "frames": samples.len() / channels as usize,
        "bitrate_kbps": if duration > 0.0 { ((bytes.len() as f64 * 8.0 / duration / 1000.0) * 10.0).round() / 10.0 } else { 0.0 },
        "lossless": matches!(codec_name.as_str(), "flac" | "pcm_s16le" | "pcm_s24le" | "pcm_s32le" | "pcm_f32le" | "pcm_f64le" | "pcm_s16be" | "pcm_s24be" | "pcm_s32be" | "alac" | "pcm_u8" | "pcm_s8"),
        "tags": tags,
        "decode_errors": decode_errors,
        "file_bytes": bytes.len(),
    });
    Ok(Decoded {
        clip: Clip::new(sample_rate, channels, samples),
        info,
    })
}

/// Output encoding options.
#[derive(Debug, Clone)]
pub struct EncodeOptions {
    /// `wav` (default) or `flac`.
    pub format: String,
    /// 16 (default), 24 or 32 (32 = IEEE float for WAV).
    pub bit_depth: u16,
}

impl Default for EncodeOptions {
    fn default() -> Self {
        Self { format: "wav".into(), bit_depth: 16 }
    }
}

/// Encodes a clip as WAV or FLAC; lossy targets return `E_UNSUPPORTED` with guidance.
pub fn encode(
    clip: &Clip,
    opts: &EncodeOptions,
) -> EngineResult<(Vec<u8>, &'static str, &'static str)> {
    if clip.is_empty() {
        return Err(EngineError::empty("audio"));
    }
    match opts.format.trim().to_ascii_lowercase().as_str() {
        "" | "wav" | "wave" => Ok((encode_wav(clip, opts.bit_depth)?, "wav", "audio/wav")),
        "flac" => Ok((encode_flac(clip, opts.bit_depth)?, "flac", "audio/flac")),
        "mp3" | "aac" | "m4a" | "ogg" | "opus" | "wma" => Err(EngineError::unsupported(format!(
            "Encoding to {} is not available in the WASM engine (no pure-Rust encoder); export \
             WAV/FLAC here and transcode with the browser MediaRecorder API or the backend ffmpeg \
             service",
            opts.format
        ))),
        other => Err(EngineError::unsupported_format(format!(
            "Unknown output format `{other}` (wav, flac)"
        ))),
    }
}

/// Encodes as PCM WAV at 8/16/24/32 bits (32 = float).
pub fn encode_wav(clip: &Clip, bit_depth: u16) -> EngineResult<Vec<u8>> {
    let (bits, fmt) = match bit_depth {
        8 | 16 => (16, hound::SampleFormat::Int),
        24 => (24, hound::SampleFormat::Int),
        32 => (32, hound::SampleFormat::Float),
        other => {
            return Err(EngineError::invalid(format!(
                "bit_depth {other} not supported (16, 24, 32)"
            )))
        },
    };
    let spec = hound::WavSpec {
        channels: clip.channels,
        sample_rate: clip.sample_rate,
        bits_per_sample: bits,
        sample_format: fmt,
    };
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut w = hound::WavWriter::new(&mut cursor, spec)
            .map_err(|e| EngineError::internal(e.to_string()))?;
        match bits {
            16 => {
                let mut sw = w.get_i16_writer(clip.samples.len() as u32);
                for s in &clip.samples {
                    sw.write_sample((s.clamp(-1.0, 1.0) * 32767.0).round() as i16);
                }
                sw.flush().map_err(|e| EngineError::internal(e.to_string()))?;
            },
            24 => {
                for s in &clip.samples {
                    w.write_sample((s.clamp(-1.0, 1.0) * 8_388_607.0).round() as i32)
                        .map_err(|e| EngineError::internal(e.to_string()))?;
                }
            },
            _ => {
                for s in &clip.samples {
                    w.write_sample(*s).map_err(|e| EngineError::internal(e.to_string()))?;
                }
            },
        }
        w.finalize().map_err(|e| EngineError::internal(e.to_string()))?;
    }
    Ok(cursor.into_inner())
}

/// Encodes as FLAC at 16 or 24 bits.
pub fn encode_flac(clip: &Clip, bit_depth: u16) -> EngineResult<Vec<u8>> {
    use flacenc::component::BitRepr;
    use flacenc::error::Verify;
    let bits = match bit_depth {
        8 | 16 => 16usize,
        24 | 32 => 24,
        other => {
            return Err(EngineError::invalid(format!(
                "bit_depth {other} not supported for FLAC (16, 24)"
            )))
        },
    };
    let scale = ((1i64 << (bits - 1)) - 1) as f32;
    let samples: Vec<i32> = clip
        .samples
        .iter()
        .map(|s| (s.clamp(-1.0, 1.0) * scale).round() as i32)
        .collect();
    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|e| EngineError::internal(format!("FLAC config: {e:?}")))?;
    let source = flacenc::source::MemSource::from_samples(
        &samples,
        clip.channels as usize,
        bits,
        clip.sample_rate as usize,
    );
    let stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size)
        .map_err(|e| EngineError::internal(format!("FLAC encode: {e:?}")))?;
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream
        .write(&mut sink)
        .map_err(|e| EngineError::internal(format!("FLAC write: {e:?}")))?;
    Ok(sink.as_slice().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pcm::tone;

    #[test]
    fn wav_and_flac_roundtrip() {
        let clip = tone("sine", 440.0, 0.5, 22_050, -6.0, 2).unwrap();
        for (fmt, depth) in [("wav", 16), ("wav", 24), ("wav", 32), ("flac", 16), ("flac", 24)] {
            let (bytes, ext, _) =
                encode(&clip, &EncodeOptions { format: fmt.into(), bit_depth: depth }).unwrap();
            assert_eq!(ext, fmt);
            let d = decode(&bytes, Some(&format!("x.{fmt}"))).unwrap();
            assert_eq!(d.clip.sample_rate, 22_050);
            assert_eq!(d.clip.channels, 2);
            assert_eq!(d.clip.frames(), clip.frames(), "{fmt}/{depth}");
            let max_err = d
                .clip
                .samples
                .iter()
                .zip(&clip.samples)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0f32, f32::max);
            assert!(max_err < 0.001, "{fmt}/{depth}: {max_err}");
            assert_eq!(d.info["container"], fmt);
        }
        assert!(
            matches!(encode(&clip, &EncodeOptions { format: "mp3".into(), bit_depth: 16 }), Err(e) if e.code == engine_shared::error::ErrorCode::Unsupported)
        );
        assert!(decode(b"definitely not audio data at all, nope", None).is_err());
        assert!(decode(&[], None).is_err());
        // Truncated WAV header → malformed / no samples.
        let (bytes, _, _) = encode(&clip, &EncodeOptions::default()).unwrap();
        assert!(decode(&bytes[..40], Some("x.wav")).is_err());
    }
}
