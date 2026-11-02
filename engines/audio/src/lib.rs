//! # Dev Toolbox Audio Engine (`audio-engine` v2)
//!
//! Decodes WAV, MP3, FLAC, OGG Vorbis, AAC/M4A, ALAC and the audio track of
//! MP4/MKV/WebM containers (symphonia, pure Rust), processes PCM in memory and
//! encodes WAV (16/24-bit, 32-bit float) or FLAC. Lossy *encoding* (MP3, AAC,
//! OGG) has no pure-Rust encoder and returns `E_UNSUPPORTED` with guidance.
//!
//! | Action | Tool(s) |
//! |---|---|
//! | `inspect` | audio-metadata, audio-duration, audio-sample-rate-converter (info) |
//! | `convert` | audio-convert, audio-compress, audio-sample-rate-converter, audio-channel-converter, audio-mono-stereo |
//! | `trim` / `cut` / `split` | audio-trim, audio-cut, audio-split |
//! | `join` / `mix` | audio-merge, audio-concat, audio-join |
//! | `loop` / `reverse` | audio-loop, audio-reverse |
//! | `volume` / `normalize` / `fade` | audio-volume, audio-normalize, audio-fade-in, audio-fade-out |
//! | `speed` / `pitch` | audio-speed, audio-pitch |
//! | `equalizer` / `bass_boost` / `treble_boost` | audio-equalizer, audio-bass-boost, audio-treble-boost |
//! | `noise_reduction` / `silence_removal` | audio-noise-reduction, audio-silence-removal |
//! | `waveform` / `spectrum` | audio-waveform, audio-spectrum, audio-visualizer |
//! | `tone` / `dtmf` | audio-tone-generator |
//! | `extract_from_video` | audio-extract-from-video (MP4/MKV/WebM audio track → WAV/FLAC) |
//! | `to_video` | audio-to-video → `E_UNSUPPORTED` (needs a video encoder) |
//! | `calc_file_size` / `bpm_to_ms` | legacy calculators (kept for compatibility) |
//!
//! Every file action accepts `file` (base64), optional `name`, and `output`
//! `{ format: "wav"|"flac", bit_depth: 16|24|32 }`.

pub mod codec;
pub mod pcm;

use codec::EncodeOptions;
use engine_shared::{EngineError, EngineResult};
use pcm::{BiquadKind, Clip};
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

/// Output encoding settings.
#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Output {
    /// `wav` (default) or `flac`.
    pub format: String,
    /// Bits per sample (16 default; 8/24/32 for WAV, 16/24 for FLAC).
    pub bit_depth: u16,
}

impl Default for Output {
    fn default() -> Self {
        Self { format: "wav".into(), bit_depth: 16 }
    }
}

impl Output {
    fn opts(&self) -> EncodeOptions {
        EncodeOptions {
            format: self.format.clone(),
            bit_depth: self.bit_depth,
        }
    }
}

fn load(input: &FileInput) -> EngineResult<codec::Decoded> {
    codec::decode(&input.file, input.name.as_deref())
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

fn emit(clip: &Clip, output: &Output, name: &str, suffix: &str) -> EngineResult<Value> {
    let (bytes, ext, mime) = codec::encode(clip, &output.opts())?;
    Ok(json!({
        "file": engine_shared::b64::encode(&bytes),
        "filename": format!("{name}{suffix}.{ext}"),
        "mime": mime,
        "bytes": bytes.len(),
        "format": ext,
        "sample_rate": clip.sample_rate,
        "channels": clip.channels,
        "bit_depth": if ext == "flac" { output.bit_depth.min(24) } else { output.bit_depth },
        "duration_seconds": (clip.duration() * 1000.0).round() / 1000.0,
        "peak_db": db(clip.peak()),
    }))
}

fn db(v: f32) -> f64 {
    if v <= 0.0 {
        -120.0
    } else {
        ((20.0 * (v as f64).log10()) * 100.0).round() / 100.0
    }
}

// ---------------------------------------------------------------- actions --

/// Decodes the file and reports format, duration, channels, loudness and a waveform overview.
pub fn inspect(input: &FileInput, waveform_buckets: usize) -> EngineResult<Value> {
    let d = load(input)?;
    let mut v = d.info;
    v["filename"] = json!(input.name);
    v["peak_db"] = json!(db(d.clip.peak()));
    v["rms_db"] = json!(db(d.clip.rms()));
    v["loudness_lufs_approx"] = json!((d.clip.loudness_lufs() * 10.0).round() / 10.0);
    v["silence_regions"] = json!(d.clip.silences(-50.0, 0.5).iter().map(|(s, e)| json!({ "start": (s * 1000.0).round() / 1000.0, "end": (e * 1000.0).round() / 1000.0 })).collect::<Vec<_>>());
    v["waveform"] = d
        .clip
        .waveform(if waveform_buckets == 0 { 200 } else { waveform_buckets });
    v["uncompressed_size_bytes"] = json!(d.clip.frames() as u64 * d.clip.channels as u64 * 2);
    Ok(v)
}

/// One equaliser band (biquad filter).
#[derive(Deserialize, Debug, Clone)]
pub struct EqBand {
    /// Filter type (`peaking` default, `low_shelf`, `high_shelf`, `low_pass`, `high_pass`, `notch`,
    /// `band_pass`).
    #[serde(default = "default_peaking")]
    pub kind: String,
    /// Centre/corner frequency in Hz.
    pub frequency: f64,
    /// Gain in dB (peaking/shelf filters).
    #[serde(default)]
    pub gain_db: f64,
    /// Quality factor (0 = sensible default).
    #[serde(default)]
    pub q: f64,
}

fn default_peaking() -> String {
    "peaking".into()
}

/// How `split` divides a clip.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct SplitSpec {
    /// `times` (split at the given seconds), `parts` (equal parts), `duration` (chunks of N
    /// seconds), `silence`.
    pub mode: String,
    /// Split points in seconds (`times` mode).
    pub times: Vec<f64>,
    /// Number of equal parts (`parts` mode).
    pub parts: usize,
    /// Chunk length in seconds (`duration` mode).
    pub seconds: f64,
    /// Silence threshold in dBFS (`silence` mode).
    pub threshold_db: f64,
    /// Minimum silence length in seconds (`silence` mode).
    pub min_silence: f64,
}

/// Every action understood by the audio engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum AudioAction {
    /// Format, duration, channels, loudness, tags and a waveform overview.
    Inspect {
        /// Input audio.
        #[serde(flatten)]
        input: FileInput,
        /// Number of waveform buckets (0 = none).
        #[serde(default)]
        waveform_buckets: usize,
    },
    /// Decodes and re-encodes with optional resampling, channel change and gain.
    Convert {
        /// Input audio.
        #[serde(flatten)]
        input: FileInput,
        /// Output format.
        #[serde(default)]
        output: Output,
        /// Target sample rate in Hz.
        #[serde(default)]
        sample_rate: Option<u32>,
        /// Target channel count (1 or 2).
        #[serde(default)]
        channels: Option<u16>,
        /// Gain applied in dB.
        #[serde(default)]
        gain_db: f64,
    },
    /// Keeps the section between `start` and `end`.
    Trim {
        /// Input audio.
        #[serde(flatten)]
        input: FileInput,
        /// Start in seconds.
        #[serde(default)]
        start: f64,
        /// End in seconds (default: end of clip).
        #[serde(default)]
        end: Option<f64>,
        /// Output format.
        #[serde(default)]
        output: Output,
    },
    /// Removes the section between `start` and `end`.
    Cut {
        /// Input audio.
        #[serde(flatten)]
        input: FileInput,
        /// Start of the removed section.
        start: f64,
        /// End of the removed section.
        end: f64,
        /// Output format.
        #[serde(default)]
        output: Output,
    },
    /// Splits into several clips.
    Split {
        /// Input audio.
        #[serde(flatten)]
        input: FileInput,
        /// Split strategy.
        #[serde(flatten)]
        spec: SplitSpec,
        /// Output format.
        #[serde(default)]
        output: Output,
    },
    /// Concatenates clips with an optional gap or crossfade.
    Join {
        /// Inputs in order.
        files: Vec<FileInput>,
        /// Silence inserted between clips.
        #[serde(default)]
        gap_seconds: f64,
        /// Crossfade length in seconds.
        #[serde(default)]
        crossfade_seconds: f64,
        /// Output format.
        #[serde(default)]
        output: Output,
        /// Output file name.
        #[serde(default)]
        name: String,
    },
    /// Mixes clips together with per-clip gain and offset.
    Mix {
        /// Inputs to mix.
        files: Vec<FileInput>,
        /// Gain per input in dB.
        #[serde(default)]
        gains_db: Vec<f64>,
        /// Start offset per input in seconds.
        #[serde(default)]
        offsets_seconds: Vec<f64>,
        /// Output format.
        #[serde(default)]
        output: Output,
        /// Output file name.
        #[serde(default)]
        name: String,
    },
    /// Repeats the clip.
    Loop {
        /// Input audio.
        #[serde(flatten)]
        input: FileInput,
        /// Number of repetitions.
        times: usize,
        /// Silence between repetitions.
        #[serde(default)]
        gap_seconds: f64,
        /// Output format.
        #[serde(default)]
        output: Output,
    },
    /// Plays the clip backwards.
    Reverse {
        /// Input audio.
        #[serde(flatten)]
        input: FileInput,
        /// Output format.
        #[serde(default)]
        output: Output,
    },
    /// Applies a fixed gain.
    Volume {
        /// Input audio.
        #[serde(flatten)]
        input: FileInput,
        /// Gain in dB.
        gain_db: f64,
        /// Output format.
        #[serde(default)]
        output: Output,
    },
    /// Normalises to a peak or RMS target.
    Normalize {
        /// Input audio.
        #[serde(flatten)]
        input: FileInput,
        /// `peak` (default) or `rms`.
        #[serde(default)]
        mode: String,
        /// Target level in dBFS.
        #[serde(default)]
        target_db: Option<f64>,
        /// Output format.
        #[serde(default)]
        output: Output,
    },
    /// Applies fade-in/fade-out envelopes.
    Fade {
        /// Input audio.
        #[serde(flatten)]
        input: FileInput,
        /// Fade-in length in seconds.
        #[serde(default)]
        fade_in: f64,
        /// Fade-out length in seconds.
        #[serde(default)]
        fade_out: f64,
        /// `linear` (default), `exponential` or `scurve`.
        #[serde(default)]
        curve: String,
        /// Output format.
        #[serde(default)]
        output: Output,
    },
    /// Changes playback speed (resampling or time-stretch).
    Speed {
        /// Input audio.
        #[serde(flatten)]
        input: FileInput,
        /// Speed factor (2 = twice as fast).
        factor: f64,
        /// Keep pitch using a granular time-stretch.
        #[serde(default = "default_true")]
        preserve_pitch: bool,
        /// Output format.
        #[serde(default)]
        output: Output,
    },
    /// Shifts pitch without changing duration.
    Pitch {
        /// Input audio.
        #[serde(flatten)]
        input: FileInput,
        /// Shift in semitones.
        semitones: f64,
        /// Output format.
        #[serde(default)]
        output: Output,
    },
    /// Multi-band parametric equaliser.
    Equalizer {
        /// Input audio.
        #[serde(flatten)]
        input: FileInput,
        /// Filter bands applied in order.
        bands: Vec<EqBand>,
        /// Output format.
        #[serde(default)]
        output: Output,
    },
    /// Low-shelf boost.
    BassBoost {
        /// Input audio.
        #[serde(flatten)]
        input: FileInput,
        /// Gain in dB (default 6).
        #[serde(default)]
        gain_db: Option<f64>,
        /// Shelf frequency in Hz (default 150).
        #[serde(default)]
        frequency: Option<f64>,
        /// Output format.
        #[serde(default)]
        output: Output,
    },
    /// High-shelf boost.
    TrebleBoost {
        /// Input audio.
        #[serde(flatten)]
        input: FileInput,
        /// Gain in dB (default 6).
        #[serde(default)]
        gain_db: Option<f64>,
        /// Shelf frequency in Hz (default 4000).
        #[serde(default)]
        frequency: Option<f64>,
        /// Output format.
        #[serde(default)]
        output: Output,
    },
    /// Spectral-subtraction noise reduction.
    NoiseReduction {
        /// Input audio.
        #[serde(flatten)]
        input: FileInput,
        /// Reduction strength 0–1.
        #[serde(default)]
        strength: Option<f64>,
        /// Start of the noise-only region used as profile.
        #[serde(default)]
        profile_start: Option<f64>,
        /// End of the noise-only region used as profile.
        #[serde(default)]
        profile_end: Option<f64>,
        /// Output format.
        #[serde(default)]
        output: Output,
    },
    /// Removes silent stretches.
    SilenceRemoval {
        /// Input audio.
        #[serde(flatten)]
        input: FileInput,
        /// Silence threshold in dBFS.
        #[serde(default)]
        threshold_db: Option<f64>,
        /// Minimum silence length to remove (seconds).
        #[serde(default)]
        min_duration: Option<f64>,
        /// Silence kept at each cut (seconds).
        #[serde(default)]
        keep_seconds: Option<f64>,
        /// Output format.
        #[serde(default)]
        output: Output,
    },
    /// Min/max peaks per bucket, optionally rendered as SVG.
    Waveform {
        /// Input audio.
        #[serde(flatten)]
        input: FileInput,
        /// Number of buckets.
        #[serde(default)]
        buckets: usize,
        /// Also return an SVG rendering.
        #[serde(default)]
        svg: bool,
        /// SVG width.
        #[serde(default)]
        width: u32,
        /// SVG height.
        #[serde(default)]
        height: u32,
        /// SVG stroke colour.
        #[serde(default)]
        color: String,
    },
    /// FFT spectrum, frequency bands and an optional spectrogram.
    Spectrum {
        /// Input audio.
        #[serde(flatten)]
        input: FileInput,
        /// FFT window size (power of two).
        #[serde(default)]
        fft_size: usize,
        /// Number of frequency bands.
        #[serde(default)]
        bands: usize,
        /// Number of spectrogram time frames (0 = none).
        #[serde(default)]
        spectrogram_frames: usize,
    },
    /// Synthesises a test tone.
    Tone {
        /// `sine`, `square`, `triangle`, `sawtooth` or `noise`.
        #[serde(default)]
        wave: String,
        /// Frequency in Hz.
        frequency: f64,
        /// Duration in seconds.
        #[serde(default)]
        seconds: Option<f64>,
        /// Sample rate in Hz.
        #[serde(default)]
        sample_rate: Option<u32>,
        /// Amplitude in dBFS.
        #[serde(default)]
        amplitude_db: Option<f64>,
        /// Channel count.
        #[serde(default)]
        channels: Option<u16>,
        /// Fade in/out in milliseconds to avoid clicks.
        #[serde(default)]
        fade_ms: Option<f64>,
        /// Output format.
        #[serde(default)]
        output: Output,
    },
    /// Generates DTMF dial tones.
    Dtmf {
        /// Digits `0-9`, `*`, `#`, `A-D`.
        digits: String,
        /// Length of each tone.
        #[serde(default)]
        tone_seconds: Option<f64>,
        /// Gap between tones.
        #[serde(default)]
        gap_seconds: Option<f64>,
        /// Sample rate in Hz.
        #[serde(default)]
        sample_rate: Option<u32>,
        /// Output format.
        #[serde(default)]
        output: Output,
    },
    /// Extracts the audio track from an MP4/MKV (decodes AAC/MP3/… to PCM).
    ExtractFromVideo {
        /// Input video.
        #[serde(flatten)]
        input: FileInput,
        /// Output format.
        #[serde(default)]
        output: Output,
    },
    /// Always returns `E_UNSUPPORTED` (muxing video needs the video engine/backend).
    ToVideo {
        /// Ignored; present so the tool payload validates.
        #[serde(default)]
        file: String,
    },
    /// PCM file-size calculator.
    CalcFileSize {
        /// Duration in seconds.
        duration_seconds: f64,
        /// Sample rate in Hz.
        sample_rate: u32,
        /// Bits per sample.
        bit_depth: u32,
        /// Channel count.
        channels: u32,
    },
    /// Tempo → note durations in milliseconds.
    BpmToMs {
        /// Beats per minute.
        bpm: f64,
    },
}

fn default_true() -> bool {
    true
}

fn with_clip(
    input: &FileInput,
    output: &Output,
    suffix: &str,
    f: impl FnOnce(Clip) -> EngineResult<(Clip, Value)>,
) -> EngineResult<Value> {
    let d = load(input)?;
    let (clip, extra) = f(d.clip)?;
    let mut v = emit(&clip, output, &base_name(input, "audio"), suffix)?;
    if let Some(obj) = extra.as_object() {
        for (k, val) in obj {
            v[k] = val.clone();
        }
    }
    v["source"] = json!({
        "container": d.info["container"],
        "codec": d.info["codec"],
        "sample_rate": d.info["sample_rate"],
        "channels": d.info["channels"],
        "duration_seconds": d.info["duration_seconds"]
    });
    Ok(v)
}

/// Dispatches an [`AudioAction`] to its implementation.
pub fn handle_action(action: AudioAction) -> EngineResult<Value> {
    match action {
        AudioAction::Inspect { input, waveform_buckets } => inspect(&input, waveform_buckets),
        AudioAction::Convert {
            input,
            output,
            sample_rate,
            channels,
            gain_db,
        } => with_clip(&input, &output, "", |mut c| {
            if let Some(r) = sample_rate {
                c = c.resample(r)?;
            }
            if let Some(ch) = channels {
                c = c.with_channels(ch)?;
            }
            if gain_db != 0.0 {
                c = c.gain_db(gain_db);
            }
            Ok((c, json!({})))
        }),
        AudioAction::Trim { input, start, end, output } => {
            with_clip(&input, &output, "_trimmed", |c| {
                Ok((
                    c.slice(start, end)?,
                    json!({
                        "start": start,
                        "end": end
                    }),
                ))
            })
        },
        AudioAction::Cut { input, start, end, output } => with_clip(&input, &output, "_cut", |c| {
            Ok((
                c.cut(start, end)?,
                json!({
                    "removed": { "start": start, "end": end }
                }),
            ))
        }),
        AudioAction::Split { input, spec, output } => {
            let d = load(&input)?;
            let clip = d.clip;
            let dur = clip.duration();
            let mut points: Vec<f64> = match spec.mode.as_str() {
                "" | "times" => spec.times.clone(),
                "parts" => {
                    if spec.parts < 2 || spec.parts > 500 {
                        return Err(EngineError::invalid("parts must be 2-500"));
                    }
                    (1..spec.parts).map(|i| dur * i as f64 / spec.parts as f64).collect()
                },
                "duration" | "seconds" => {
                    if spec.seconds <= 0.0 {
                        return Err(EngineError::invalid("seconds must be > 0"));
                    }
                    if dur / spec.seconds > 500.0 {
                        return Err(EngineError::limit(
                            "Splitting would create more than 500 files",
                        ));
                    }
                    let mut t = spec.seconds;
                    let mut p = Vec::new();
                    while t < dur {
                        p.push(t);
                        t += spec.seconds;
                    }
                    p
                },
                "silence" => clip
                    .silences(
                        if spec.threshold_db == 0.0 {
                            -40.0
                        } else {
                            spec.threshold_db
                        },
                        if spec.min_silence <= 0.0 { 0.5 } else { spec.min_silence },
                    )
                    .iter()
                    .map(|(s, e)| (s + e) / 2.0)
                    .collect(),
                other => {
                    return Err(EngineError::invalid(format!(
                        "Unknown split mode `{other}` (times, parts, duration, silence)"
                    )))
                },
            };
            points.retain(|p| *p > 0.0 && *p < dur);
            points.sort_by(f64::total_cmp);
            points.dedup();
            if points.is_empty() {
                return Err(EngineError::invalid("No split points inside the clip"));
            }
            let name = base_name(&input, "audio");
            let mut files = Vec::new();
            let mut start = 0.0;
            for (i, p) in points.iter().chain(std::iter::once(&dur)).enumerate() {
                let part = clip.slice(start, Some(*p))?;
                let mut v = emit(&part, &output, &name, &format!("_part{:02}", i + 1))?;
                v["start"] = json!((start * 1000.0).round() / 1000.0);
                v["end"] = json!((p * 1000.0).round() / 1000.0);
                files.push(v);
                start = *p;
            }
            Ok(json!({
                "files": files,
                "count": files.len(),
                "split_points": points,
                "source_duration": dur
            }))
        },
        AudioAction::Join {
            files,
            gap_seconds,
            crossfade_seconds,
            output,
            name,
        } => {
            engine_shared::limits::check_count(files.len(), 200, "files")?;
            let clips: Vec<Clip> = files
                .iter()
                .map(|f| load(f).map(|d| d.clip))
                .collect::<EngineResult<_>>()?;
            let joined = Clip::concat(&clips, gap_seconds.max(0.0), crossfade_seconds.max(0.0))?;
            let mut v = emit(&joined, &output, if name.is_empty() { "joined" } else { &name }, "")?;
            v["inputs"] = json!(clips.iter().map(|c| json!({ "duration_seconds": c.duration(), "sample_rate": c.sample_rate, "channels": c.channels })).collect::<Vec<_>>());
            Ok(v)
        },
        AudioAction::Mix {
            files,
            gains_db,
            offsets_seconds,
            output,
            name,
        } => {
            engine_shared::limits::check_count(files.len(), 32, "files")?;
            let clips: Vec<(Clip, f64, f64)> = files
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    load(f).map(|d| {
                        (
                            d.clip,
                            gains_db.get(i).copied().unwrap_or(0.0),
                            offsets_seconds.get(i).copied().unwrap_or(0.0),
                        )
                    })
                })
                .collect::<EngineResult<_>>()?;
            let mixed = Clip::mix(&clips)?;
            emit(&mixed, &output, if name.is_empty() { "mix" } else { &name }, "")
        },
        AudioAction::Loop { input, times, gap_seconds, output } => {
            with_clip(&input, &output, "_loop", |c| {
                Ok((
                    c.repeat(times, gap_seconds.max(0.0))?,
                    json!({
                        "times": times
                    }),
                ))
            })
        },
        AudioAction::Reverse { input, output } => {
            with_clip(&input, &output, "_reversed", |c| Ok((c.reverse(), json!({}))))
        },
        AudioAction::Volume { input, gain_db, output } => {
            if !(-60.0..=40.0).contains(&gain_db) {
                return Err(EngineError::invalid("gain_db must be between -60 and +40"));
            }
            with_clip(&input, &output, "_gain", |c| {
                let before = c.peak();
                let out = c.gain_db(gain_db);
                Ok((
                    out.clone(),
                    json!({
                        "gain_db": gain_db,
                        "peak_before_db": db(before),
                        "clipped": before as f64 * 10f64.powf(gain_db / 20.0) > 1.0
                    }),
                ))
            })
        },
        AudioAction::Normalize { input, mode, target_db, output } => {
            with_clip(&input, &output, "_normalized", |c| {
                let before_peak = db(c.peak());
                let before_rms = db(c.rms());
                let out = match mode.as_str() {
                    "" | "peak" => c.normalize_peak(target_db.unwrap_or(-1.0)),
                    "rms" | "loudness" | "lufs" => c.normalize_rms(target_db.unwrap_or(-18.0)),
                    other => {
                        return Err(EngineError::invalid(format!(
                            "Unknown normalize mode `{other}` (peak, rms)"
                        )))
                    },
                };
                Ok((
                    out.clone(),
                    json!({
                        "mode": if mode.is_empty() { "peak" } else { &mode },
                        "target_db": target_db.unwrap_or(if mode == "rms" { -18.0 } else { -1.0 }),
                        "before": { "peak_db": before_peak, "rms_db": before_rms },
                        "after": { "peak_db": db(out.peak()), "rms_db": db(out.rms()) }
                    }),
                ))
            })
        },
        AudioAction::Fade { input, fade_in, fade_out, curve, output } => {
            if fade_in <= 0.0 && fade_out <= 0.0 {
                return Err(EngineError::invalid("Provide fade_in and/or fade_out seconds"));
            }
            with_clip(&input, &output, "_fade", |c| {
                Ok((
                    c.fade(fade_in, fade_out, &curve),
                    json!({
                        "fade_in": fade_in,
                        "fade_out": fade_out,
                        "curve": if curve.is_empty() { "linear" } else { &curve }
                    }),
                ))
            })
        },
        AudioAction::Speed { input, factor, preserve_pitch, output } => {
            with_clip(&input, &output, "_speed", |c| {
                let out = if preserve_pitch {
                    c.stretch(factor)?
                } else {
                    c.speed(factor)?
                };
                Ok((out, json!({ "factor": factor, "preserve_pitch": preserve_pitch })))
            })
        },
        AudioAction::Pitch { input, semitones, output } => {
            with_clip(&input, &output, "_pitch", |c| {
                Ok((
                    c.pitch_shift(semitones)?,
                    json!({
                        "semitones": semitones
                    }),
                ))
            })
        },
        AudioAction::Equalizer { input, bands, output } => {
            if bands.is_empty() {
                return Err(EngineError::empty("bands"));
            }
            if bands.len() > 32 {
                return Err(EngineError::limit("At most 32 EQ bands"));
            }
            with_clip(&input, &output, "_eq", |mut c| {
                for b in &bands {
                    if !(-24.0..=24.0).contains(&b.gain_db) {
                        return Err(EngineError::invalid(
                            "band gain_db must be between -24 and +24",
                        ));
                    }
                    c = c.biquad(
                        BiquadKind::parse(&b.kind)?,
                        b.frequency,
                        if b.q <= 0.0 { 1.0 } else { b.q },
                        b.gain_db,
                    );
                }
                Ok((c, json!({ "bands": bands.len() })))
            })
        },
        AudioAction::BassBoost { input, gain_db, frequency, output } => {
            let g = gain_db.unwrap_or(6.0);
            if !(0.0..=24.0).contains(&g) {
                return Err(EngineError::invalid("gain_db must be between 0 and 24"));
            }
            with_clip(&input, &output, "_bass", |c| {
                Ok((
                    c.biquad(BiquadKind::LowShelf, frequency.unwrap_or(150.0), 0.707, g),
                    json!({
                        "gain_db": g,
                        "frequency": frequency.unwrap_or(150.0)
                    }),
                ))
            })
        },
        AudioAction::TrebleBoost { input, gain_db, frequency, output } => {
            let g = gain_db.unwrap_or(6.0);
            if !(0.0..=24.0).contains(&g) {
                return Err(EngineError::invalid("gain_db must be between 0 and 24"));
            }
            with_clip(&input, &output, "_treble", |c| {
                Ok((
                    c.biquad(BiquadKind::HighShelf, frequency.unwrap_or(4000.0), 0.707, g),
                    json!({
                        "gain_db": g,
                        "frequency": frequency.unwrap_or(4000.0)
                    }),
                ))
            })
        },
        AudioAction::NoiseReduction {
            input,
            strength,
            profile_start,
            profile_end,
            output,
        } => with_clip(&input, &output, "_denoised", |c| {
            let profile = match (profile_start, profile_end) {
                (Some(s), Some(e)) if e > s => Some((s, e)),
                _ => None,
            };
            let out = c.denoise(strength.unwrap_or(0.6), profile)?;
            Ok((
                out,
                json!({
                    "strength": strength.unwrap_or(0.6),
                    "profile": profile.map(|(s, e)| json!({ "start": s, "end": e })).unwrap_or(json!("\
                        auto (quietest 10% of frames)"))
                }),
            ))
        }),
        AudioAction::SilenceRemoval {
            input,
            threshold_db,
            min_duration,
            keep_seconds,
            output,
        } => with_clip(&input, &output, "_nosilence", |c| {
            let before = c.duration();
            let (out, regions) = c.remove_silence(
                threshold_db.unwrap_or(-40.0),
                min_duration.unwrap_or(0.5),
                keep_seconds.unwrap_or(0.2),
            );
            Ok((
                out.clone(),
                json!({
                    "removed_seconds": ((before - out.duration()) * 1000.0).round() / 1000.0,
                    "silence_regions": regions.iter().map(|(s, e)| json!({ "start": s, "end": e })).collect::<Vec<_>>(),
                    "threshold_db": threshold_db.unwrap_or(-40.0)
                }),
            ))
        }),
        AudioAction::Waveform {
            input,
            buckets,
            svg,
            width,
            height,
            color,
        } => {
            let d = load(&input)?;
            let buckets = if buckets == 0 { 400 } else { buckets };
            let mut v = d.clip.waveform(buckets);
            v["duration_seconds"] = json!(d.clip.duration());
            if svg {
                let (w, h) = (
                    if width == 0 { 800 } else { width.min(8000) },
                    if height == 0 { 120 } else { height.min(2000) },
                );
                let peaks = v["peaks"].as_array().cloned().unwrap_or_default();
                let n = peaks.len().max(1) as f64;
                let mut path = String::new();
                for (i, p) in peaks.iter().enumerate() {
                    let (min, max) = (p[0].as_f64().unwrap_or(0.0), p[1].as_f64().unwrap_or(0.0));
                    let x = i as f64 / n * w as f64;
                    let y1 = (0.5 - max / 2.0) * h as f64;
                    let y2 = (0.5 - min / 2.0) * h as f64;
                    path.push_str(&format!("M{x:.1} {y1:.1}V{:.1}", y2.max(y1 + 0.5)));
                }
                v["svg"] = json!(format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}"><path d="{path}" stroke="{}" stroke-width="{:.2}" fill="none"/></svg>"#,
                    if color.is_empty() { "#3b82f6" } else { &color },
                    (w as f64 / n * 0.8).max(0.5)
                ));
            }
            Ok(v)
        },
        AudioAction::Spectrum {
            input,
            fft_size,
            bands,
            spectrogram_frames,
        } => {
            let d = load(&input)?;
            let mut v = d.clip.spectrum(
                if fft_size == 0 { 2048 } else { fft_size },
                if bands == 0 { 64 } else { bands },
                spectrogram_frames.min(2000),
            );
            v["duration_seconds"] = json!(d.clip.duration());
            v["sample_rate"] = json!(d.clip.sample_rate);
            Ok(v)
        },
        AudioAction::Tone {
            wave,
            frequency,
            seconds,
            sample_rate,
            amplitude_db,
            channels,
            fade_ms,
            output,
        } => {
            let rate = sample_rate.unwrap_or(44_100);
            if !(8000..=192_000).contains(&rate) {
                return Err(EngineError::invalid("sample_rate must be 8000-192000"));
            }
            let mut clip = pcm::tone(
                &wave,
                frequency,
                seconds.unwrap_or(1.0),
                rate,
                amplitude_db.unwrap_or(-6.0),
                channels.unwrap_or(1),
            )?;
            let fade = fade_ms.unwrap_or(5.0) / 1000.0;
            if fade > 0.0 {
                clip = clip.fade(fade, fade, "linear");
            }
            let mut v = emit(
                &clip,
                &output,
                &format!(
                    "tone_{}_{}hz",
                    if wave.is_empty() { "sine" } else { &wave },
                    frequency.round() as i64
                ),
                "",
            )?;
            v["waveform"] = clip.waveform(200);
            v["frequency"] = json!(frequency);
            v["note"] = json!(note_name(frequency));
            Ok(v)
        },
        AudioAction::Dtmf {
            digits,
            tone_seconds,
            gap_seconds,
            sample_rate,
            output,
        } => {
            let clip = pcm::dtmf(
                &digits,
                tone_seconds.unwrap_or(0.15),
                gap_seconds.unwrap_or(0.1),
                sample_rate.unwrap_or(8000),
            )?;
            emit(&clip, &output, "dtmf", "")
        },
        AudioAction::ExtractFromVideo { input, output } => {
            let d = load(&input)?;
            let mut v = emit(&d.clip, &output, &base_name(&input, "audio"), "_audio")?;
            v["source"] = d.info;
            Ok(v)
        },
        AudioAction::ToVideo { .. } => Err(EngineError::unsupported(
            "Rendering audio into a video file requires a video encoder, which is not available \
             in WASM; use the backend ffmpeg service (audio-to-video)",
        )),
        AudioAction::CalcFileSize {
            duration_seconds,
            sample_rate,
            bit_depth,
            channels,
        } => {
            if duration_seconds <= 0.0 || sample_rate == 0 || bit_depth == 0 || channels == 0 {
                return Err(EngineError::invalid("All audio parameters must be greater than zero"));
            }
            let bytes_per_sec =
                (sample_rate as f64) * ((bit_depth as f64) / 8.0) * (channels as f64);
            let total_bytes = (bytes_per_sec * duration_seconds).round() as u64;
            Ok(json!({
                "uncompressed_bytes": total_bytes,
                "uncompressed_mb": total_bytes as f64 / (1024.0 * 1024.0),
                "mp3_128k_mb": (duration_seconds * 128.0 * 1000.0 / 8.0) / (1024.0 * 1024.0),
                "mp3_320k_mb": (duration_seconds * 320.0 * 1000.0 / 8.0) / (1024.0 * 1024.0),
                "flac_estimate_mb": total_bytes as f64 * 0.6 / (1024.0 * 1024.0),
                "aac_128k_mb": (duration_seconds * 128.0 * 1000.0 / 8.0) / (1024.0 * 1024.0),
            }))
        },
        AudioAction::BpmToMs { bpm } => {
            if bpm <= 0.0 {
                return Err(EngineError::invalid("BPM must be greater than zero"));
            }
            let q = 60_000.0 / bpm;
            Ok(json!({
                "bpm": bpm,
                "whole_note_ms": q * 4.0,
                "half_note_ms": q * 2.0,
                "quarter_note_ms": q,
                "eighth_note_ms": q / 2.0,
                "sixteenth_note_ms": q / 4.0,
                "thirty_second_note_ms": q / 8.0,
                "dotted_quarter_ms": q * 1.5,
                "triplet_eighth_ms": q / 3.0,
                "beats_per_second": bpm / 60.0
            }))
        },
    }
}

fn note_name(freq: f64) -> Value {
    if freq <= 0.0 {
        return Value::Null;
    }
    let names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let midi = 69.0 + 12.0 * (freq / 440.0).log2();
    let rounded = midi.round();
    let cents = ((midi - rounded) * 100.0).round();
    let idx = (rounded as i64).rem_euclid(12) as usize;
    let octave = (rounded as i64) / 12 - 1;
    json!({
        "name": format!("{}{}", names[idx], octave),
        "midi": rounded as i64,
        "cents_off": cents
    })
}

engine_shared::export_engine!(handle_action, AudioAction);
