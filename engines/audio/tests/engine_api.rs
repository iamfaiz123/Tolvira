//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use audio_engine::process_request;
use serde_json::{json, Value};

fn call(action: &str, payload: Value) -> Value {
    serde_json::from_slice(&process_request(
        json!({ "action": action, "payload": payload }).to_string().as_bytes(),
    ))
    .unwrap()
}

fn ok(action: &str, payload: Value) -> Value {
    let v = call(action, payload);
    assert_eq!(v["success"], true, "{action}: {v}");
    v["data"].clone()
}

fn tone_wav(seconds: f64) -> String {
    ok(
        "tone",
        json!({
            "wave": "sine",
            "frequency": 440,
            "seconds": seconds,
            "sample_rate": 16000,
            "channels": 2
        }),
    )["file"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn valid_inputs() {
    let wav = tone_wav(2.0);
    let info = ok("inspect", json!({ "file": wav, "name": "tone.wav" }));
    assert_eq!(info["container"], "wav");
    assert_eq!(info["sample_rate"], 16000);
    assert_eq!(info["channels"], 2);
    assert_eq!(info["duration_seconds"], 2.0);
    assert!(info["waveform"]["peaks"].is_array());
    let mp3 = engine_shared::b64::encode(include_bytes!("fixtures/sample.mp3"));
    let info = ok("inspect", json!({ "file": mp3, "name": "sample.mp3" }));
    assert_eq!(info["container"], "mp3");
    assert_eq!(info["codec"], "mp3");
    assert!(info["duration_seconds"].as_f64().unwrap() > 0.5);
    let conv = ok(
        "convert",
        json!({
            "file": mp3,
            "name": "sample.mp3",
            "output": { "format": "flac" },
            "sample_rate": 22050,
            "channels": 1
        }),
    );
    assert_eq!(conv["format"], "flac");
    assert_eq!(conv["sample_rate"], 22050);
    assert_eq!(conv["channels"], 1);
    assert_eq!(conv["filename"], "sample.flac");
    let flac_info = ok("inspect", json!({ "file": conv["file"], "name": "x.flac" }));
    assert_eq!(flac_info["codec"], "flac");
    assert_eq!(
        ok("trim", json!({ "file": wav, "start": 0.5, "end": 1.0 }))["duration_seconds"],
        0.5
    );
    assert_eq!(
        ok("cut", json!({ "file": wav, "start": 0.0, "end": 1.5 }))["duration_seconds"],
        0.5
    );
    assert_eq!(ok("split", json!({ "file": wav, "mode": "parts", "parts": 4 }))["count"], 4);
    assert_eq!(ok("split", json!({ "file": wav, "times": [0.5, 1.5] }))["count"], 3);
    assert_eq!(
        ok("join", json!({ "files": [{ "file": wav }, { "file": wav }], "gap_seconds": 1 }))
            ["duration_seconds"],
        5.0
    );
    assert_eq!(
        ok(
            "mix",
            json!({
                "files": [{ "file": wav }, { "file": wav }],
                "gains_db": [0, -6],
                "offsets_seconds": [0, 1]
            })
        )["duration_seconds"],
        3.0
    );
    assert_eq!(ok("loop", json!({ "file": wav, "times": 2 }))["duration_seconds"], 4.0);
    assert_eq!(ok("reverse", json!({ "file": wav }))["duration_seconds"], 2.0);
    let vol = ok("volume", json!({ "file": wav, "gain_db": -12 }));
    assert!(vol["peak_db"].as_f64().unwrap() < -17.0);
    let norm = ok("normalize", json!({ "file": wav, "target_db": -3 }));
    assert!((norm["after"]["peak_db"].as_f64().unwrap() + 3.0).abs() < 0.2);
    assert_eq!(
        ok(
            "fade",
            json!({ "file": wav, "fade_in": 0.5, "fade_out": 0.5, "curve": "scurve" })
        )["curve"],
        "scurve"
    );
    assert_eq!(
        ok("speed", json!({ "file": wav, "factor": 2.0, "preserve_pitch": false }))
            ["duration_seconds"],
        1.0
    );
    let st = ok("speed", json!({ "file": wav, "factor": 0.5 }));
    assert!((st["duration_seconds"].as_f64().unwrap() - 4.0).abs() < 0.1);
    let p = ok("pitch", json!({ "file": wav, "semitones": 5 }));
    assert!((p["duration_seconds"].as_f64().unwrap() - 2.0).abs() < 0.1);
    assert_eq!(
        ok(
            "equalizer",
            json!({
                "file": wav,
                "bands": [
                    { "kind": "peaking", "frequency": 1000, "gain_db": 3 },
                    { "kind": "highpass", "frequency": 80 }
                ]
            })
        )["bands"],
        2
    );
    assert!(ok("bass_boost", json!({ "file": wav }))["gain_db"] == 6.0);
    assert!(ok("treble_boost", json!({ "file": wav, "gain_db": 3 }))["gain_db"] == 3.0);
    assert_eq!(
        ok("noise_reduction", json!({ "file": wav, "strength": 0.5 }))["duration_seconds"],
        2.0
    );
    assert!(ok("silence_removal", json!({ "file": wav }))["removed_seconds"].is_number());
    let wf = ok("waveform", json!({ "file": wav, "buckets": 50, "svg": true }));
    assert_eq!(wf["peaks"].as_array().unwrap().len(), 50);
    assert!(wf["svg"].as_str().unwrap().starts_with("<svg"));
    let sp = ok("spectrum", json!({ "file": wav, "bands": 32, "spectrogram_frames": 4 }));
    assert!((sp["peak_frequency_hz"].as_f64().unwrap() - 440.0).abs() < 20.0);
    assert_eq!(sp["spectrogram"].as_array().unwrap().len(), 4);
    let tone = ok("tone", json!({ "wave": "square", "frequency": 261.63, "seconds": 0.2 }));
    assert_eq!(tone["note"]["name"], "C4");
    assert!(
        ok("dtmf", json!({ "digits": "911" }))["duration_seconds"]
            .as_f64()
            .unwrap()
            > 0.6
    );
    assert_eq!(
        ok("extract_from_video", json!({ "file": mp3, "name": "clip.mp3" }))["filename"],
        "clip_audio.wav"
    );
    assert!(
        ok(
            "calc_file_size",
            json!({ "duration_seconds": 60, "sample_rate": 44100, "bit_depth": 16, "channels": 2 })
        )["uncompressed_bytes"]
            == 10_584_000
    );
    assert_eq!(ok("bpm_to_ms", json!({ "bpm": 120 }))["quarter_note_ms"], 500.0);
}

#[test]
fn invalid_inputs() {
    let wav = tone_wav(1.0);
    assert_eq!(
        call(
            "inspect",
            json!({ "file": engine_shared::b64::encode(b"not audio at all, just text bytes") })
        )["success"],
        false
    );
    assert_eq!(
        call("trim", json!({ "file": wav, "start": 5.0 }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("cut", json!({ "file": wav, "start": 0.5, "end": 0.2 }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("convert", json!({ "file": wav, "output": { "format": "mp3" } }))["error_code"],
        "E_UNSUPPORTED"
    );
    assert_eq!(
        call("convert", json!({ "file": wav, "output": { "format": "xyz" } }))["error_code"],
        "E_UNSUPPORTED_FORMAT"
    );
    assert_eq!(
        call("convert", json!({ "file": wav, "sample_rate": 10 }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("volume", json!({ "file": wav, "gain_db": 99 }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("speed", json!({ "file": wav, "factor": 50 }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("pitch", json!({ "file": wav, "semitones": 40 }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call(
            "equalizer",
            json!({ "file": wav, "bands": [{ "kind": "weird", "frequency": 100 }] })
        )["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("split", json!({ "file": wav, "mode": "parts", "parts": 1 }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("loop", json!({ "file": wav, "times": 0 }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(call("tone", json!({ "frequency": 50000 }))["error_code"], "E_INVALID_INPUT");
    assert_eq!(call("dtmf", json!({ "digits": "9X" }))["error_code"], "E_INVALID_INPUT");
    assert_eq!(call("to_video", json!({ "file": wav }))["error_code"], "E_UNSUPPORTED");
    assert_eq!(
        call(
            "calc_file_size",
            json!({ "duration_seconds": 0, "sample_rate": 1, "bit_depth": 1, "channels": 1 })
        )["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(call("nope", json!({}))["error_code"], "E_BAD_REQUEST");
}

#[test]
fn empty_inputs() {
    assert_eq!(call("inspect", json!({ "file": "" }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("inspect", json!({}))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("join", json!({ "files": [] }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("mix", json!({ "files": [] }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(
        call("equalizer", json!({ "file": tone_wav(0.2), "bands": [] }))["error_code"],
        "E_EMPTY_INPUT"
    );
    assert_eq!(call("dtmf", json!({ "digits": "" }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("waveform", json!({ "file": "" }))["error_code"], "E_EMPTY_INPUT");
}
