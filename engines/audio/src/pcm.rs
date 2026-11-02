//! In-memory PCM clip and DSP operations (all f32, interleaved).

use engine_shared::{EngineError, EngineResult};
use rustfft::{num_complex::Complex, FftPlanner};
use serde_json::{json, Value};

/// Maximum decoded size we keep in memory (~ 1 GB of f32 → ~ 100 min stereo 44.1k).
pub const MAX_SAMPLES: usize = 256 * 1024 * 1024;

/// In-memory PCM audio.
#[derive(Debug, Clone)]
pub struct Clip {
    /// Samples per second.
    pub sample_rate: u32,
    /// Number of interleaved channels.
    pub channels: u16,
    /// Interleaved samples in [-1, 1].
    pub samples: Vec<f32>,
}

impl Clip {
    /// Wraps interleaved samples.
    pub fn new(sample_rate: u32, channels: u16, samples: Vec<f32>) -> Self {
        Clip { sample_rate, channels, samples }
    }

    /// Creates a silent clip.
    pub fn silence(sample_rate: u32, channels: u16, seconds: f64) -> Self {
        let frames = (seconds.max(0.0) * sample_rate as f64).round() as usize;
        Clip {
            sample_rate,
            channels,
            samples: vec![0.0; frames * channels as usize],
        }
    }

    /// Number of sample frames (samples per channel).
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }

    /// Duration in seconds.
    pub fn duration(&self) -> f64 {
        self.frames() as f64 / self.sample_rate as f64
    }

    /// `true` when there are no samples.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Enforces [`MAX_SAMPLES`].
    pub fn check_len(len: usize) -> EngineResult<()> {
        if len > MAX_SAMPLES {
            return Err(EngineError::limit(format!(
                "Result would have {len} samples; limit is {MAX_SAMPLES} (~100 minutes of stereo \
                 44.1 kHz)"
            )));
        }
        Ok(())
    }

    /// Extracts one channel.
    pub fn channel(&self, c: usize) -> Vec<f32> {
        let ch = self.channels as usize;
        self.samples.iter().skip(c).step_by(ch).copied().collect()
    }

    /// Interleaves per-channel buffers.
    pub fn from_channels(sample_rate: u32, chans: &[Vec<f32>]) -> Self {
        let frames = chans.iter().map(Vec::len).max().unwrap_or(0);
        let mut out = Vec::with_capacity(frames * chans.len());
        for f in 0..frames {
            for c in chans {
                out.push(c.get(f).copied().unwrap_or(0.0));
            }
        }
        Clip {
            sample_rate,
            channels: chans.len() as u16,
            samples: out,
        }
    }

    /// Down-mixes to mono.
    pub fn to_mono(&self) -> Clip {
        if self.channels == 1 {
            return self.clone();
        }
        let ch = self.channels as usize;
        let mono: Vec<f32> = self
            .samples
            .chunks(ch)
            .map(|f| f.iter().sum::<f32>() / ch as f32)
            .collect();
        Clip {
            sample_rate: self.sample_rate,
            channels: 1,
            samples: mono,
        }
    }

    /// Changes the channel count: mono↔stereo, or any count by averaging/duplicating.
    pub fn with_channels(&self, channels: u16) -> EngineResult<Clip> {
        if channels == 0 || channels > 8 {
            return Err(EngineError::invalid("channels must be 1-8"));
        }
        if channels == self.channels {
            return Ok(self.clone());
        }
        if channels == 1 {
            return Ok(self.to_mono());
        }
        let src_ch = self.channels as usize;
        let mut out = Vec::with_capacity(self.frames() * channels as usize);
        for frame in self.samples.chunks(src_ch) {
            for c in 0..channels as usize {
                out.push(frame[c.min(src_ch - 1)]);
            }
        }
        Ok(Clip {
            sample_rate: self.sample_rate,
            channels,
            samples: out,
        })
    }

    /// Resamples with 4-point cubic (Catmull-Rom) interpolation.
    pub fn resample(&self, target_rate: u32) -> EngineResult<Clip> {
        if !(1000..=384_000).contains(&target_rate) {
            return Err(EngineError::invalid("sample_rate must be between 1000 and 384000 Hz"));
        }
        if target_rate == self.sample_rate {
            return Ok(self.clone());
        }
        let ratio = self.sample_rate as f64 / target_rate as f64;
        let new_frames = (self.frames() as f64 / ratio).floor() as usize;
        Self::check_len(new_frames * self.channels as usize)?;
        let chans: Vec<Vec<f32>> = (0..self.channels as usize)
            .map(|c| resample_channel(&self.channel(c), ratio, new_frames))
            .collect();
        let mut out = Clip::from_channels(target_rate, &chans);
        // When downsampling, apply a gentle low-pass to limit aliasing.
        if target_rate < self.sample_rate {
            let cutoff = (target_rate as f64 * 0.45).min(20_000.0);
            out = out.biquad(BiquadKind::LowPass, cutoff, 0.707, 0.0);
        }
        Ok(out)
    }

    /// Applies a gain in dB.
    pub fn gain_db(&self, db: f64) -> Clip {
        let g = 10f64.powf(db / 20.0) as f32;
        Clip {
            sample_rate: self.sample_rate,
            channels: self.channels,
            samples: self.samples.iter().map(|s| (s * g).clamp(-1.0, 1.0)).collect(),
        }
    }

    /// Peak absolute sample value.
    pub fn peak(&self) -> f32 {
        self.samples.iter().fold(0.0f32, |m, s| m.max(s.abs()))
    }

    /// Root-mean-square level.
    pub fn rms(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        (self.samples.iter().map(|s| s * s).sum::<f32>() / self.samples.len() as f32).sqrt()
    }

    /// Approximate integrated loudness (K-weighting-free RMS in LUFS-ish units).
    pub fn loudness_lufs(&self) -> f64 {
        let rms = self.rms() as f64;
        if rms <= 0.0 {
            -100.0
        } else {
            -0.691 + 10.0 * (rms * rms).log10()
        }
    }

    /// Scales so the peak hits `target_db` dBFS.
    pub fn normalize_peak(&self, target_db: f64) -> Clip {
        let peak = self.peak();
        if peak <= 0.0 {
            return self.clone();
        }
        let target = 10f64.powf(target_db / 20.0) as f32;
        let g = target / peak;
        Clip {
            sample_rate: self.sample_rate,
            channels: self.channels,
            samples: self.samples.iter().map(|s| (s * g).clamp(-1.0, 1.0)).collect(),
        }
    }

    /// Scales so the RMS level hits `target_db` dBFS.
    pub fn normalize_rms(&self, target_db: f64) -> Clip {
        let rms = self.rms();
        if rms <= 0.0 {
            return self.clone();
        }
        let target = 10f64.powf(target_db / 20.0) as f32;
        let g = target / rms;
        Clip {
            sample_rate: self.sample_rate,
            channels: self.channels,
            samples: self.samples.iter().map(|s| (s * g).clamp(-1.0, 1.0)).collect(),
        }
    }

    /// Applies fade-in/fade-out envelopes (`linear`, `exponential`, `scurve`).
    pub fn fade(&self, fade_in: f64, fade_out: f64, curve: &str) -> Clip {
        let ch = self.channels as usize;
        let frames = self.frames();
        let fi = ((fade_in.max(0.0) * self.sample_rate as f64) as usize).min(frames);
        let fo = ((fade_out.max(0.0) * self.sample_rate as f64) as usize).min(frames);
        let shape = |t: f32| -> f32 {
            match curve {
                "exponential" | "exp" => t * t,
                "logarithmic" | "log" => (t * 9.0 + 1.0).log10(),
                "scurve" | "s" => 0.5 - 0.5 * (std::f32::consts::PI * t).cos(),
                _ => t,
            }
        };
        let mut out = self.samples.clone();
        for f in 0..frames {
            let mut g = 1.0f32;
            if fi > 0 && f < fi {
                g *= shape(f as f32 / fi as f32);
            }
            if fo > 0 && f >= frames - fo {
                g *= shape((frames - f) as f32 / fo as f32);
            }
            if g < 1.0 {
                for c in 0..ch {
                    out[f * ch + c] *= g;
                }
            }
        }
        Clip {
            sample_rate: self.sample_rate,
            channels: self.channels,
            samples: out,
        }
    }

    /// Reverses sample order.
    pub fn reverse(&self) -> Clip {
        let ch = self.channels as usize;
        let mut frames: Vec<&[f32]> = self.samples.chunks(ch).collect();
        frames.reverse();
        Clip {
            sample_rate: self.sample_rate,
            channels: self.channels,
            samples: frames.concat(),
        }
    }

    /// Extracts [start, end) seconds.
    pub fn slice(&self, start: f64, end: Option<f64>) -> EngineResult<Clip> {
        let dur = self.duration();
        let end = end.unwrap_or(dur);
        if start < 0.0 || end <= start {
            return Err(EngineError::invalid(format!(
                "Invalid range {start}s..{end}s (clip is {dur:.3}s)"
            )));
        }
        if start >= dur {
            return Err(EngineError::invalid(format!(
                "Start {start}s is beyond the clip duration ({dur:.3}s)"
            )));
        }
        let ch = self.channels as usize;
        let s = (start * self.sample_rate as f64).round() as usize * ch;
        let e = ((end.min(dur) * self.sample_rate as f64).round() as usize * ch)
            .min(self.samples.len());
        Ok(Clip {
            sample_rate: self.sample_rate,
            channels: self.channels,
            samples: self.samples[s..e].to_vec(),
        })
    }

    /// Removes [start, end) seconds.
    pub fn cut(&self, start: f64, end: f64) -> EngineResult<Clip> {
        let dur = self.duration();
        if start < 0.0 || end <= start || start >= dur {
            return Err(EngineError::invalid(format!(
                "Invalid cut range {start}s..{end}s (clip is {dur:.3}s)"
            )));
        }
        let ch = self.channels as usize;
        let s = (start * self.sample_rate as f64).round() as usize * ch;
        let e = ((end.min(dur) * self.sample_rate as f64).round() as usize * ch)
            .min(self.samples.len());
        let mut out = self.samples[..s].to_vec();
        out.extend_from_slice(&self.samples[e..]);
        Ok(Clip {
            sample_rate: self.sample_rate,
            channels: self.channels,
            samples: out,
        })
    }

    /// Concatenates clips (resampling to the first clip's rate) with an optional gap or crossfade.
    pub fn concat(clips: &[Clip], gap_seconds: f64, crossfade_seconds: f64) -> EngineResult<Clip> {
        let first = clips.first().ok_or_else(|| EngineError::empty("clips"))?;
        let rate = first.sample_rate;
        let channels = clips.iter().map(|c| c.channels).max().unwrap_or(1);
        let mut out = Clip {
            sample_rate: rate,
            channels,
            samples: Vec::new(),
        };
        for (i, c) in clips.iter().enumerate() {
            let c = c.resample(rate)?.with_channels(channels)?;
            if i > 0 && gap_seconds > 0.0 {
                out.samples.extend(Clip::silence(rate, channels, gap_seconds).samples);
            }
            if i > 0 && crossfade_seconds > 0.0 {
                let xf = ((crossfade_seconds * rate as f64) as usize * channels as usize)
                    .min(out.samples.len())
                    .min(c.samples.len());
                let start = out.samples.len() - xf;
                for k in 0..xf {
                    let t = k as f32 / xf as f32;
                    out.samples[start + k] = out.samples[start + k] * (1.0 - t) + c.samples[k] * t;
                }
                out.samples.extend_from_slice(&c.samples[xf..]);
            } else {
                out.samples.extend_from_slice(&c.samples);
            }
            Self::check_len(out.samples.len())?;
        }
        Ok(out)
    }

    /// Overlays clips (sum with optional per-clip gain and offset).
    pub fn mix(clips: &[(Clip, f64, f64)]) -> EngineResult<Clip> {
        let (first, _, _) = clips.first().ok_or_else(|| EngineError::empty("clips"))?;
        let rate = first.sample_rate;
        let channels = clips.iter().map(|(c, _, _)| c.channels).max().unwrap_or(1) as usize;
        let total_frames = clips
            .iter()
            .map(|(c, _, off)| {
                (off.max(0.0) * rate as f64) as usize
                    + (c.frames() as f64 * rate as f64 / c.sample_rate as f64) as usize
            })
            .max()
            .unwrap_or(0);
        Self::check_len(total_frames * channels)?;
        let mut out = vec![0.0f32; total_frames * channels];
        for (c, gain_db, offset) in clips {
            let c = c.resample(rate)?.with_channels(channels as u16)?;
            let g = 10f64.powf(gain_db / 20.0) as f32;
            let start = (offset.max(0.0) * rate as f64) as usize * channels;
            for (i, s) in c.samples.iter().enumerate() {
                if start + i < out.len() {
                    out[start + i] += s * g;
                }
            }
        }
        let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        if peak > 1.0 {
            for s in out.iter_mut() {
                *s /= peak;
            }
        }
        Ok(Clip {
            sample_rate: rate,
            channels: channels as u16,
            samples: out,
        })
    }

    /// Repeats the clip `times` times with a gap.
    pub fn repeat(&self, times: usize, gap_seconds: f64) -> EngineResult<Clip> {
        if times == 0 || times > 1000 {
            return Err(EngineError::invalid("times must be 1-1000"));
        }
        Self::check_len(self.samples.len() * times)?;
        let clips: Vec<Clip> = (0..times).map(|_| self.clone()).collect();
        Clip::concat(&clips, gap_seconds, 0.0)
    }

    /// Speed change by resampling (pitch follows, like a tape machine).
    pub fn speed(&self, factor: f64) -> EngineResult<Clip> {
        if !(0.1..=10.0).contains(&factor) {
            return Err(EngineError::invalid("speed factor must be between 0.1 and 10"));
        }
        let new_frames = (self.frames() as f64 / factor).floor() as usize;
        Self::check_len(new_frames * self.channels as usize)?;
        let chans: Vec<Vec<f32>> = (0..self.channels as usize)
            .map(|c| resample_channel(&self.channel(c), factor, new_frames))
            .collect();
        Ok(Clip::from_channels(self.sample_rate, &chans))
    }

    /// Time-stretch without pitch change (WSOLA). factor > 1 = faster/shorter.
    pub fn stretch(&self, factor: f64) -> EngineResult<Clip> {
        if !(0.25..=4.0).contains(&factor) {
            return Err(EngineError::invalid("stretch factor must be between 0.25 and 4"));
        }
        if (factor - 1.0).abs() < 1e-6 {
            return Ok(self.clone());
        }
        let out_frames = (self.frames() as f64 / factor) as usize;
        Self::check_len(out_frames * self.channels as usize)?;
        let chans: Vec<Vec<f32>> = (0..self.channels as usize)
            .map(|c| wsola(&self.channel(c), factor, self.sample_rate))
            .collect();
        Ok(Clip::from_channels(self.sample_rate, &chans))
    }

    /// Pitch shift in semitones, preserving duration.
    pub fn pitch_shift(&self, semitones: f64) -> EngineResult<Clip> {
        if !(-24.0..=24.0).contains(&semitones) {
            return Err(EngineError::invalid("semitones must be between -24 and 24"));
        }
        let ratio = 2f64.powf(semitones / 12.0);
        // Resample to change pitch (and duration), then stretch back to the original duration.
        let shifted = self.speed(ratio)?;
        shifted.stretch(1.0 / ratio)
    }

    /// Applies a biquad filter to every channel.
    pub fn biquad(&self, kind: BiquadKind, freq: f64, q: f64, gain_db: f64) -> Clip {
        let coeffs = Biquad::new(kind, freq, self.sample_rate as f64, q, gain_db);
        let ch = self.channels as usize;
        let mut states = vec![[0.0f64; 4]; ch];
        let mut out = Vec::with_capacity(self.samples.len());
        for (i, s) in self.samples.iter().enumerate() {
            let c = i % ch;
            out.push(coeffs.process(*s as f64, &mut states[c]).clamp(-1.0, 1.0) as f32);
        }
        Clip {
            sample_rate: self.sample_rate,
            channels: self.channels,
            samples: out,
        }
    }

    /// Detects silent regions (RMS below threshold for at least min_duration).
    pub fn silences(&self, threshold_db: f64, min_duration: f64) -> Vec<(f64, f64)> {
        let window = (self.sample_rate as usize / 100).max(1); // 10 ms
        let ch = self.channels as usize;
        let thr = 10f64.powf(threshold_db / 20.0) as f32;
        let frames = self.frames();
        let mut regions = Vec::new();
        let mut start: Option<usize> = None;
        let mut f = 0;
        while f < frames {
            let end = (f + window).min(frames);
            let slice = &self.samples[f * ch..end * ch];
            let rms = (slice.iter().map(|s| s * s).sum::<f32>() / slice.len().max(1) as f32).sqrt();
            if rms < thr {
                if start.is_none() {
                    start = Some(f);
                }
            } else if let Some(s) = start.take() {
                if (f - s) as f64 / self.sample_rate as f64 >= min_duration {
                    regions.push((
                        s as f64 / self.sample_rate as f64,
                        f as f64 / self.sample_rate as f64,
                    ));
                }
            }
            f = end;
        }
        if let Some(s) = start {
            if (frames - s) as f64 / self.sample_rate as f64 >= min_duration {
                regions.push((
                    s as f64 / self.sample_rate as f64,
                    frames as f64 / self.sample_rate as f64,
                ));
            }
        }
        regions
    }

    /// Removes silence: trims edges and shortens internal gaps to `keep` seconds.
    pub fn remove_silence(
        &self,
        threshold_db: f64,
        min_duration: f64,
        keep: f64,
    ) -> (Clip, Vec<(f64, f64)>) {
        let regions = self.silences(threshold_db, min_duration);
        if regions.is_empty() {
            return (self.clone(), regions);
        }
        let ch = self.channels as usize;
        let rate = self.sample_rate as f64;
        let dur = self.duration();
        let mut out = Vec::new();
        let mut cursor = 0.0;
        for (s, e) in &regions {
            let seg_end = *s;
            if seg_end > cursor {
                let a = (cursor * rate) as usize * ch;
                let b = ((seg_end * rate) as usize * ch).min(self.samples.len());
                out.extend_from_slice(&self.samples[a..b]);
            }
            let at_edge = *s <= 0.0 || *e >= dur - 1e-6;
            if !at_edge && keep > 0.0 {
                let k = ((keep.min(e - s) * rate) as usize) * ch;
                let a = (s * rate) as usize * ch;
                out.extend_from_slice(&self.samples[a..(a + k).min(self.samples.len())]);
            }
            cursor = *e;
        }
        if cursor < dur {
            let a = (cursor * rate) as usize * ch;
            out.extend_from_slice(&self.samples[a.min(self.samples.len())..]);
        }
        (
            Clip {
                sample_rate: self.sample_rate,
                channels: self.channels,
                samples: out,
            },
            regions,
        )
    }

    /// Spectral-gating noise reduction. The noise profile is taken from
    /// `profile` (start, end seconds) or the quietest 10% of frames.
    pub fn denoise(&self, strength: f64, profile: Option<(f64, f64)>) -> EngineResult<Clip> {
        let strength = strength.clamp(0.0, 1.0);
        let n = 2048usize;
        let hop = n / 4;
        let chans: Vec<Vec<f32>> = (0..self.channels as usize).map(|c| self.channel(c)).collect();
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(n);
        let ifft = planner.plan_fft_inverse(n);
        let window: Vec<f32> = (0..n)
            .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / n as f32).cos())
            .collect();
        let mut out_chans = Vec::new();
        for data in &chans {
            if data.len() < n {
                out_chans.push(data.clone());
                continue;
            }
            let frames_count = (data.len() - n) / hop + 1;
            // Magnitude spectra per frame.
            let mut spectra: Vec<Vec<Complex<f32>>> = Vec::with_capacity(frames_count);
            for f in 0..frames_count {
                let mut buf: Vec<Complex<f32>> = (0..n)
                    .map(|i| Complex::new(data[f * hop + i] * window[i], 0.0))
                    .collect();
                fft.process(&mut buf);
                spectra.push(buf);
            }
            // Noise profile.
            let noise_mag: Vec<f32> = match profile {
                Some((s, e)) => {
                    let fs = ((s * self.sample_rate as f64) as usize / hop)
                        .min(frames_count.saturating_sub(1));
                    let fe = ((e * self.sample_rate as f64) as usize / hop)
                        .max(fs + 1)
                        .min(frames_count.max(fs + 1));
                    (0..n)
                        .map(|k| {
                            spectra[fs..fe].iter().map(|sp| sp[k].norm()).sum::<f32>()
                                / (fe - fs) as f32
                        })
                        .collect()
                },
                None => {
                    let mut energies: Vec<(f32, usize)> = spectra
                        .iter()
                        .enumerate()
                        .map(|(i, sp)| (sp.iter().map(|c| c.norm_sqr()).sum::<f32>(), i))
                        .collect();
                    energies.sort_by(|a, b| a.0.total_cmp(&b.0));
                    let take = (energies.len() / 10).max(1);
                    let idx: Vec<usize> = energies.iter().take(take).map(|(_, i)| *i).collect();
                    (0..n)
                        .map(|k| {
                            idx.iter().map(|i| spectra[*i][k].norm()).sum::<f32>() / take as f32
                        })
                        .collect()
                },
            };
            // Gate & overlap-add.
            let mut out = vec![0.0f32; data.len()];
            let mut norm = vec![0.0f32; data.len()];
            let floor = 1.0 - strength;
            for (f, sp) in spectra.iter_mut().enumerate() {
                for k in 0..n {
                    let mag = sp[k].norm();
                    let thr = noise_mag[k] * (1.0 + 2.0 * strength as f32);
                    let g = if mag <= thr {
                        floor as f32 * 0.1
                    } else {
                        ((mag - thr) / mag).max(floor as f32 * 0.1)
                    };
                    sp[k] *= g;
                }
                ifft.process(sp);
                for i in 0..n {
                    out[f * hop + i] += sp[i].re / n as f32 * window[i];
                    norm[f * hop + i] += window[i] * window[i];
                }
            }
            for i in 0..out.len() {
                if norm[i] > 1e-6 {
                    out[i] /= norm[i];
                } else {
                    out[i] = data[i];
                }
            }
            out_chans.push(out);
        }
        Ok(Clip::from_channels(self.sample_rate, &out_chans))
    }

    /// Waveform peaks for visualisation (per bucket: min, max, rms) on a mono mix.
    pub fn waveform(&self, buckets: usize) -> Value {
        let mono = self.to_mono();
        let buckets = buckets.clamp(1, 20_000).min(mono.samples.len().max(1));
        let per = (mono.samples.len() as f64 / buckets as f64).max(1.0);
        let mut peaks = Vec::with_capacity(buckets);
        for b in 0..buckets {
            let s = (b as f64 * per) as usize;
            let e = (((b + 1) as f64 * per) as usize)
                .min(mono.samples.len())
                .max(s + 1)
                .min(mono.samples.len());
            let slice = &mono.samples[s..e];
            if slice.is_empty() {
                peaks.push(json!([0.0, 0.0, 0.0]));
                continue;
            }
            let min = slice.iter().cloned().fold(f32::INFINITY, f32::min);
            let max = slice.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let rms = (slice.iter().map(|s| s * s).sum::<f32>() / slice.len() as f32).sqrt();
            peaks.push(json!([r3(min), r3(max), r3(rms)]));
        }
        json!({
            "buckets": buckets,
            "seconds_per_bucket": per / self.sample_rate as f64,
            "peaks": peaks,
            "format": "[min, max, rms] per bucket, mono mix"
        })
    }

    /// Average magnitude spectrum in dB, with optional spectrogram frames.
    pub fn spectrum(&self, fft_size: usize, bands: usize, spectrogram_frames: usize) -> Value {
        let mono = self.to_mono();
        let n = fft_size.clamp(256, 16384).next_power_of_two();
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(n);
        let window: Vec<f32> = (0..n)
            .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / n as f32).cos())
            .collect();
        let frames = if mono.samples.len() >= n {
            (mono.samples.len() - n) / (n / 2) + 1
        } else {
            0
        };
        let mut avg = vec![0.0f32; n / 2];
        let mut spectro: Vec<Vec<f32>> = Vec::new();
        let stride = (frames / spectrogram_frames.max(1)).max(1);
        for f in 0..frames {
            let mut buf: Vec<Complex<f32>> = (0..n)
                .map(|i| Complex::new(mono.samples[f * (n / 2) + i] * window[i], 0.0))
                .collect();
            fft.process(&mut buf);
            let mags: Vec<f32> = buf[..n / 2].iter().map(|c| c.norm() / (n as f32 / 2.0)).collect();
            for (a, m) in avg.iter_mut().zip(&mags) {
                *a += m;
            }
            if spectrogram_frames > 0 && f % stride == 0 && spectro.len() < spectrogram_frames {
                spectro.push(bucketize(&mags, bands.clamp(8, 512), self.sample_rate, n));
            }
        }
        if frames > 0 {
            for a in avg.iter_mut() {
                *a /= frames as f32;
            }
        } else {
            // Short clip: single FFT zero-padded.
            let mut buf: Vec<Complex<f32>> = (0..n)
                .map(|i| Complex::new(mono.samples.get(i).copied().unwrap_or(0.0) * window[i], 0.0))
                .collect();
            fft.process(&mut buf);
            avg = buf[..n / 2].iter().map(|c| c.norm() / (n as f32 / 2.0)).collect();
        }
        let band_vals = bucketize(&avg, bands.clamp(8, 512), self.sample_rate, n);
        let bin_hz = self.sample_rate as f64 / n as f64;
        let (peak_bin, _) =
            avg.iter()
                .enumerate()
                .fold((0usize, 0.0f32), |m, (i, v)| if *v > m.1 { (i, *v) } else { m });
        let centroid = {
            let total: f32 = avg.iter().sum();
            if total > 0.0 {
                avg.iter()
                    .enumerate()
                    .map(|(i, v)| i as f32 * bin_hz as f32 * v)
                    .sum::<f32>()
                    / total
            } else {
                0.0
            }
        };
        json!({
            "fft_size": n,
            "bin_hz": bin_hz,
            "bands": band_vals.len(),
            "band_edges_hz": log_band_edges(bands.clamp(8, 512), self.sample_rate),
            "magnitude_db": band_vals,
            "peak_frequency_hz": (peak_bin as f64 * bin_hz * 10.0).round() / 10.0,
            "spectral_centroid_hz": (centroid * 10.0).round() / 10.0,
            "spectrogram": spectro,
        })
    }
}

fn r3(v: f32) -> f64 {
    ((v as f64) * 1000.0).round() / 1000.0
}

fn log_band_edges(bands: usize, rate: u32) -> Vec<f64> {
    let lo = 20.0f64;
    let hi = (rate as f64 / 2.0).min(20_000.0);
    (0..=bands)
        .map(|i| (lo * (hi / lo).powf(i as f64 / bands as f64) * 10.0).round() / 10.0)
        .collect()
}

/// Groups FFT magnitude bins into log-spaced bands, returned in dBFS.
fn bucketize(mags: &[f32], bands: usize, rate: u32, n: usize) -> Vec<f32> {
    let edges = log_band_edges(bands, rate);
    let bin_hz = rate as f64 / n as f64;
    edges
        .windows(2)
        .map(|w| {
            let lo = (w[0] / bin_hz) as usize;
            let hi = ((w[1] / bin_hz) as usize).max(lo + 1).min(mags.len());
            let lo = lo.min(mags.len().saturating_sub(1));
            let slice = &mags[lo..hi.max(lo + 1).min(mags.len())];
            let avg = if slice.is_empty() {
                0.0
            } else {
                slice.iter().sum::<f32>() / slice.len() as f32
            };
            let db = if avg > 1e-9 { 20.0 * avg.log10() } else { -120.0 };
            (db.max(-120.0) * 10.0).round() / 10.0
        })
        .collect()
}

fn resample_channel(data: &[f32], ratio: f64, new_len: usize) -> Vec<f32> {
    let n = data.len();
    if n == 0 {
        return vec![];
    }
    let get = |i: isize| -> f32 { data[i.clamp(0, n as isize - 1) as usize] };
    (0..new_len)
        .map(|i| {
            let pos = i as f64 * ratio;
            let idx = pos.floor() as isize;
            let t = (pos - idx as f64) as f32;
            let (p0, p1, p2, p3) = (get(idx - 1), get(idx), get(idx + 1), get(idx + 2));
            // Catmull-Rom
            let a = -0.5 * p0 + 1.5 * p1 - 1.5 * p2 + 0.5 * p3;
            let b = p0 - 2.5 * p1 + 2.0 * p2 - 0.5 * p3;
            let c = -0.5 * p0 + 0.5 * p2;
            (((a * t + b) * t + c) * t + p1).clamp(-1.0, 1.0)
        })
        .collect()
}

/// Waveform-similarity overlap-add time stretching for one channel.
fn wsola(data: &[f32], factor: f64, rate: u32) -> Vec<f32> {
    let win = (rate as usize / 25).max(256); // 40 ms
    let hop_out = win / 2;
    let hop_in = (hop_out as f64 * factor) as usize;
    let search = win / 4;
    if data.len() < win * 2 {
        return resample_channel(data, factor, (data.len() as f64 / factor) as usize);
    }
    let out_len = (data.len() as f64 / factor) as usize + win;
    let mut out = vec![0.0f32; out_len];
    let mut norm = vec![0.0f32; out_len];
    let window: Vec<f32> = (0..win)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / win as f32).cos())
        .collect();
    let mut in_pos: usize = 0;
    let mut out_pos: usize = 0;
    let mut prev_tail: Option<usize> = None; // position in input of the previous segment's continuation
    while out_pos + win < out_len && in_pos + win + search < data.len() {
        // Choose the best offset near in_pos that matches the natural continuation of the previous segment.
        let best = match prev_tail {
            None => in_pos,
            Some(tail) => {
                let lo = in_pos.saturating_sub(search);
                let hi = (in_pos + search).min(data.len() - win);
                let mut best = in_pos;
                let mut best_score = f32::NEG_INFINITY;
                let target = &data[tail..(tail + win).min(data.len())];
                let mut cand = lo;
                while cand <= hi {
                    let seg = &data[cand..cand + win];
                    let score: f32 =
                        seg.iter().zip(target.iter()).take(win / 2).map(|(a, b)| a * b).sum();
                    if score > best_score {
                        best_score = score;
                        best = cand;
                    }
                    cand += 4;
                }
                best
            },
        };
        for i in 0..win {
            if out_pos + i < out_len && best + i < data.len() {
                out[out_pos + i] += data[best + i] * window[i];
                norm[out_pos + i] += window[i];
            }
        }
        prev_tail = Some(best + hop_out);
        out_pos += hop_out;
        in_pos += hop_in;
    }
    let final_len = ((data.len() as f64 / factor) as usize).min(out_len);
    (0..final_len)
        .map(|i| {
            if norm[i] > 1e-4 {
                (out[i] / norm[i]).clamp(-1.0, 1.0)
            } else {
                0.0
            }
        })
        .collect()
}

// ----------------------------------------------------------------- biquad --

/// Biquad filter types (Audio EQ Cookbook).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BiquadKind {
    /// Low-pass.
    LowPass,
    /// High-pass.
    HighPass,
    /// Band-pass.
    BandPass,
    /// Notch.
    Notch,
    /// Low shelf.
    LowShelf,
    /// High shelf.
    HighShelf,
    /// Peaking EQ.
    Peaking,
}

impl BiquadKind {
    /// Parses a filter name (`lowpass`, `low_pass`, `peaking`, …).
    pub fn parse(s: &str) -> EngineResult<Self> {
        Ok(match s.trim().to_ascii_lowercase().replace(['-', '_', ' '], "").as_str() {
            "lowpass" | "lpf" | "low" => BiquadKind::LowPass,
            "highpass" | "hpf" | "high" => BiquadKind::HighPass,
            "bandpass" | "bpf" => BiquadKind::BandPass,
            "notch" | "bandstop" => BiquadKind::Notch,
            "lowshelf" => BiquadKind::LowShelf,
            "highshelf" => BiquadKind::HighShelf,
            "peaking" | "peak" | "bell" | "eq" => BiquadKind::Peaking,
            other => {
                return Err(EngineError::invalid(format!(
                    "Unknown filter type `{other}` (lowpass, highpass, bandpass, notch, lowshelf, \
                     highshelf, peaking)"
                )))
            },
        })
    }
}

/// RBJ audio EQ cookbook biquad.
pub struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

impl Biquad {
    /// Computes coefficients for the given type, frequency, sample rate, Q and gain.
    pub fn new(kind: BiquadKind, freq: f64, rate: f64, q: f64, gain_db: f64) -> Self {
        let freq = freq.max(10.0).min((rate / 2.0 - 1.0).max(10.0));
        let q = if q <= 0.0 { 0.707 } else { q };
        let a = 10f64.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f64::consts::PI * freq / rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q);
        let (b0, b1, b2, a0, a1, a2) = match kind {
            BiquadKind::LowPass => (
                (1.0 - cos) / 2.0,
                1.0 - cos,
                (1.0 - cos) / 2.0,
                1.0 + alpha,
                -2.0 * cos,
                1.0 - alpha,
            ),
            BiquadKind::HighPass => (
                (1.0 + cos) / 2.0,
                -(1.0 + cos),
                (1.0 + cos) / 2.0,
                1.0 + alpha,
                -2.0 * cos,
                1.0 - alpha,
            ),
            BiquadKind::BandPass => (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cos, 1.0 - alpha),
            BiquadKind::Notch => (1.0, -2.0 * cos, 1.0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha),
            BiquadKind::Peaking => (
                1.0 + alpha * a,
                -2.0 * cos,
                1.0 - alpha * a,
                1.0 + alpha / a,
                -2.0 * cos,
                1.0 - alpha / a,
            ),
            BiquadKind::LowShelf => {
                let sa = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) - (a - 1.0) * cos + sa),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                    a * ((a + 1.0) - (a - 1.0) * cos - sa),
                    (a + 1.0) + (a - 1.0) * cos + sa,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                    (a + 1.0) + (a - 1.0) * cos - sa,
                )
            },
            BiquadKind::HighShelf => {
                let sa = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) + (a - 1.0) * cos + sa),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                    a * ((a + 1.0) + (a - 1.0) * cos - sa),
                    (a + 1.0) - (a - 1.0) * cos + sa,
                    2.0 * ((a - 1.0) - (a + 1.0) * cos),
                    (a + 1.0) - (a - 1.0) * cos - sa,
                )
            },
        };
        Biquad {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    /// Direct form I; state = [x1, x2, y1, y2].
    pub fn process(&self, x: f64, s: &mut [f64; 4]) -> f64 {
        let y = self.b0 * x + self.b1 * s[0] + self.b2 * s[1] - self.a1 * s[2] - self.a2 * s[3];
        s[1] = s[0];
        s[0] = x;
        s[3] = s[2];
        s[2] = y;
        y
    }
}

// ------------------------------------------------------------------- tone --

/// Synthesises a waveform (`sine`, `square`, `triangle`, `sawtooth`, `noise`).
pub fn tone(
    wave: &str,
    frequency: f64,
    seconds: f64,
    sample_rate: u32,
    amplitude_db: f64,
    channels: u16,
) -> EngineResult<Clip> {
    if !(1.0..=(sample_rate as f64 / 2.0)).contains(&frequency) {
        return Err(EngineError::invalid(format!(
            "frequency must be between 1 Hz and {} Hz (Nyquist)",
            sample_rate / 2
        )));
    }
    if seconds <= 0.0 || seconds > 600.0 {
        return Err(EngineError::invalid("duration must be 0-600 seconds"));
    }
    let frames = (seconds * sample_rate as f64) as usize;
    Clip::check_len(frames * channels as usize)?;
    let amp = 10f64.powf(amplitude_db.min(0.0) / 20.0) as f32;
    let mut rng_state: u32 = 0x1234_5678;
    let mut mono = Vec::with_capacity(frames);
    let mut pink = [0.0f32; 7];
    for i in 0..frames {
        let t = i as f64 / sample_rate as f64;
        let phase = (t * frequency).fract();
        let v = match wave {
            "square" => {
                if phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            },
            "sawtooth" | "saw" => (2.0 * phase - 1.0) as f32,
            "triangle" => (4.0 * (phase - 0.5).abs() - 1.0) as f32,
            "white" | "white_noise" | "noise" => {
                rng_state ^= rng_state << 13;
                rng_state ^= rng_state >> 17;
                rng_state ^= rng_state << 5;
                (rng_state as f32 / u32::MAX as f32) * 2.0 - 1.0
            },
            "pink" | "pink_noise" => {
                rng_state ^= rng_state << 13;
                rng_state ^= rng_state >> 17;
                rng_state ^= rng_state << 5;
                let white = (rng_state as f32 / u32::MAX as f32) * 2.0 - 1.0;
                pink[0] = 0.99886 * pink[0] + white * 0.0555179;
                pink[1] = 0.99332 * pink[1] + white * 0.0750759;
                pink[2] = 0.96900 * pink[2] + white * 0.153_852;
                pink[3] = 0.86650 * pink[3] + white * 0.3104856;
                pink[4] = 0.55000 * pink[4] + white * 0.5329522;
                pink[5] = -0.7616 * pink[5] - white * 0.0168980;
                let out = pink.iter().take(6).sum::<f32>() + pink[6] + white * 0.5362;
                pink[6] = white * 0.115926;
                (out * 0.11).clamp(-1.0, 1.0)
            },
            "sine" | "sin" | "" => (2.0 * std::f64::consts::PI * frequency * t).sin() as f32,
            other => {
                return Err(EngineError::invalid(format!(
                    "Unknown waveform `{other}` (sine, square, sawtooth, triangle, white, pink)"
                )))
            },
        };
        mono.push(v * amp);
    }
    let clip = Clip { sample_rate, channels: 1, samples: mono };
    clip.with_channels(channels)
}

/// Dual-tone (DTMF) sequence.
pub fn dtmf(
    digits: &str,
    tone_seconds: f64,
    gap_seconds: f64,
    sample_rate: u32,
) -> EngineResult<Clip> {
    let mut clips = Vec::new();
    for d in digits.chars().filter(|c| !c.is_whitespace()) {
        let (lo, hi) = match d.to_ascii_uppercase() {
            '1' => (697.0, 1209.0),
            '2' => (697.0, 1336.0),
            '3' => (697.0, 1477.0),
            'A' => (697.0, 1633.0),
            '4' => (770.0, 1209.0),
            '5' => (770.0, 1336.0),
            '6' => (770.0, 1477.0),
            'B' => (770.0, 1633.0),
            '7' => (852.0, 1209.0),
            '8' => (852.0, 1336.0),
            '9' => (852.0, 1477.0),
            'C' => (852.0, 1633.0),
            '*' => (941.0, 1209.0),
            '0' => (941.0, 1336.0),
            '#' => (941.0, 1477.0),
            'D' => (941.0, 1633.0),
            other => {
                return Err(EngineError::invalid(format!(
                    "`{other}` is not a DTMF key (0-9, *, #, A-D)"
                )))
            },
        };
        let a = tone("sine", lo, tone_seconds, sample_rate, -6.0, 1)?;
        let b = tone("sine", hi, tone_seconds, sample_rate, -6.0, 1)?;
        clips.push(Clip::mix(&[(a, 0.0, 0.0), (b, 0.0, 0.0)])?);
    }
    if clips.is_empty() {
        return Err(EngineError::empty("digits"));
    }
    Clip::concat(&clips, gap_seconds, 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basics() {
        let c = tone("sine", 440.0, 1.0, 8000, -6.0, 2).unwrap();
        assert_eq!(c.channels, 2);
        assert_eq!(c.frames(), 8000);
        assert!((c.peak() - 0.501).abs() < 0.01);
        let mono = c.to_mono();
        assert_eq!(mono.frames(), 8000);
        let r = mono.resample(16000).unwrap();
        assert_eq!(r.frames(), 16000);
        let s = c.slice(0.25, Some(0.5)).unwrap();
        assert_eq!(s.frames(), 2000);
        assert_eq!(c.cut(0.0, 0.5).unwrap().frames(), 4000);
        assert!(c.slice(2.0, None).is_err());
        let n = c.normalize_peak(-1.0);
        assert!((n.peak() - 0.891).abs() < 0.01);
        let f = c.fade(0.5, 0.5, "linear");
        assert!(f.samples[0].abs() < 1e-6);
        let rev = mono.reverse();
        assert_eq!(rev.samples[0], mono.samples[mono.samples.len() - 1]);
        let cat = Clip::concat(&[mono.clone(), mono.clone()], 0.5, 0.0).unwrap();
        assert_eq!(cat.frames(), 8000 * 2 + 4000);
        assert_eq!(mono.repeat(3, 0.0).unwrap().frames(), 24000);
        assert_eq!(mono.speed(2.0).unwrap().frames(), 4000);
        let st = mono.stretch(2.0).unwrap();
        assert!((st.frames() as i64 - 4000).abs() < 100);
        let ps = mono.pitch_shift(12.0).unwrap();
        assert!((ps.frames() as i64 - 8000).abs() < 200);
    }

    #[test]
    fn filters_and_analysis() {
        let low = tone("sine", 100.0, 1.0, 8000, 0.0, 1).unwrap();
        let high = tone("sine", 3000.0, 1.0, 8000, 0.0, 1).unwrap();
        let mixed = Clip::mix(&[(low.clone(), 0.0, 0.0), (high.clone(), 0.0, 0.0)]).unwrap();
        let lp = mixed.biquad(BiquadKind::LowPass, 500.0, 0.707, 0.0);
        // After low-pass the 3 kHz component should be strongly attenuated.
        let spec = lp.spectrum(1024, 32, 0);
        let peak = spec["peak_frequency_hz"].as_f64().unwrap();
        assert!(peak < 300.0, "{peak}");
        let sil = {
            let mut c = tone("sine", 440.0, 1.0, 8000, -6.0, 1).unwrap();
            let gap = Clip::silence(8000, 1, 1.0);
            c.samples.extend(gap.samples);
            c.samples
                .extend(tone("sine", 440.0, 0.5, 8000, -6.0, 1).unwrap().samples);
            c
        };
        let regions = sil.silences(-50.0, 0.3);
        assert_eq!(regions.len(), 1);
        assert!((regions[0].0 - 1.0).abs() < 0.05);
        let (trimmed, _) = sil.remove_silence(-50.0, 0.3, 0.2);
        assert!((trimmed.duration() - 1.7).abs() < 0.1, "{}", trimmed.duration());
        let wf = sil.waveform(10);
        assert_eq!(wf["peaks"].as_array().unwrap().len(), 10);
        assert!(wf["peaks"][5][2].as_f64().unwrap() < 0.01);
        let noisy = Clip::mix(&[
            (tone("sine", 440.0, 2.0, 8000, -6.0, 1).unwrap(), 0.0, 0.0),
            (tone("white", 1.0, 2.0, 8000, -30.0, 1).unwrap(), 0.0, 0.0),
        ])
        .unwrap();
        let clean = noisy.denoise(0.8, None).unwrap();
        assert_eq!(clean.frames(), noisy.frames());
        assert!(dtmf("123", 0.1, 0.05, 8000).unwrap().duration() > 0.3);
        assert!(tone("sine", 5000.0, 1.0, 8000, 0.0, 1).is_err());
    }
}
