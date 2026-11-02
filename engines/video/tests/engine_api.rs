//! Protocol-level tests; test code may unwrap freely.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{json, Value};
use video_engine::process_request;

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

fn mp4() -> String {
    engine_shared::b64::encode(include_bytes!("fixtures/sample.mp4"))
}

fn png(color: [u8; 4]) -> String {
    let img = image::RgbaImage::from_pixel(32, 24, image::Rgba(color));
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    engine_shared::b64::encode(&out.into_inner())
}

#[test]
fn valid_inputs() {
    let info = ok("inspect", json!({ "file": mp4(), "name": "sample.mp4" }));
    assert_eq!(info["container"], "mp4");
    assert!(info["duration_seconds"].as_f64().unwrap() > 1.0);
    assert_eq!(info["track_counts"]["video"], 1);
    assert!(info["resolution"]["width"].as_u64().unwrap() > 0);
    let mkv = ok(
        "inspect",
        json!({
            "file": engine_shared::b64::encode(include_bytes!("fixtures/sample.mkv")),
            "name": "bbb.mkv"
        }),
    );
    assert_eq!(mkv["container"], "mkv");
    assert!(!mkv["tracks"].as_array().unwrap().is_empty());
    let dur = info["duration_seconds"].as_f64().unwrap();
    let t = ok(
        "trim",
        json!({ "file": mp4(), "name": "sample.mp4", "start": 1.0, "end": dur / 2.0 }),
    );
    assert_eq!(t["filename"], "sample_trimmed.mp4");
    assert!(t["bytes"].as_u64().unwrap() > 0);
    assert!(
        ok("cut", json!({ "file": mp4(), "start": 2.0, "end": 4.0 }))["bytes"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(ok("split", json!({ "file": mp4(), "parts": 3 }))["count"], 3);
    let has_audio = info["track_counts"]["audio"].as_u64().unwrap() > 0;
    if has_audio {
        assert_eq!(ok("mute", json!({ "file": mp4() }))["audio_removed"], true);
        assert_eq!(
            ok("extract_audio_track", json!({ "file": mp4(), "name": "v.mp4" }))["filename"],
            "v_audio.m4a"
        );
    }
    assert_eq!(ok("loop", json!({ "file": mp4(), "times": 2 }))["times"], 2);
    assert_eq!(
        ok("concat", json!({ "files": [{ "file": mp4() }, { "file": mp4() }] }))["inputs"],
        2
    );
    let srt = "1\n00:00:01,000 --> 00:00:02,000\nHi\n";
    assert!(ok("subtitle_convert", json!({ "text": srt, "to": "vtt" }))["text"]
        .as_str()
        .unwrap()
        .starts_with("WEBVTT"));
    let shifted = ok("subtitle_shift", json!({ "text": srt, "shift_seconds": 2.5 }));
    assert!(shifted["text"]
        .as_str()
        .unwrap()
        .contains("00:00:03,500 --> 00:00:04,500"));
    assert_eq!(ok("subtitle_validate", json!({ "text": srt }))["valid"], true);
    let gif = ok(
        "gif_from_frames",
        json!({ "frames": [png([255, 0, 0, 255]), png([0, 255, 0, 255])], "fps": 5 }),
    );
    assert_eq!(gif["frames"], 2);
    let three = vec![png([1, 2, 3, 255]); 3];
    assert_eq!(ok("contact_sheet", json!({ "frames": three, "columns": 2 }))["rows"], 2);
    assert_eq!(ok("frame_plan", json!({ "duration": 20, "count": 4 }))["count"], 4);
    assert!(
        ok("calc_bitrate", json!({ "duration_seconds": 600, "target_size_mb": 100 }))
            ["video_bitrate_kbps"]
            .as_f64()
            .unwrap()
            > 1000.0
    );
    assert_eq!(
        ok("calc_frames", json!({ "duration_seconds": 10, "fps": 24 }))["total_frames"],
        240
    );
    assert_eq!(
        ok("aspect_ratio", json!({ "width": 1920, "height": 1080, "target": "9:16" }))["target"]
            ["crop_to"]["width"],
        608
    );
    assert_eq!(
        ok("crop_calculator", json!({ "width": 1920, "height": 1080, "target": "1:1" }))["crop"]
            ["x"],
        420
    );
    assert_eq!(
        ok("speed_calculator", json!({ "duration_seconds": 120, "factor": 2 }))["new_duration"],
        60.0
    );
    assert_eq!(
        ok(
            "resize_calculator",
            json!({ "width": 3840, "height": 2160, "target_label": "1080p" })
        )["output"]["width"],
        1920
    );
    assert_eq!(ok("timecode", json!({ "seconds": 61.5, "fps": 24 }))["timecode"], "00:01:01:12");
    assert!(
        ok("estimate_size", json!({ "duration_seconds": 60 }))["estimated_size_mb"]
            .as_f64()
            .unwrap()
            > 10.0
    );
    assert!(ok("capabilities", json!({}))["tools"].as_array().unwrap().len() > 20);
}

#[test]
fn invalid_inputs() {
    assert_eq!(
        call(
            "inspect",
            json!({ "file": engine_shared::b64::encode(b"not a video at all, definitely") })
        )["error_code"],
        "E_UNSUPPORTED_FORMAT"
    );
    assert_eq!(
        call(
            "inspect",
            json!({
                "file": engine_shared::b64::encode(b"\x00\x00\x00\x20ftypisom\x00\x00\x02\x00isomiso2")
            })
        )["error_code"],
        "E_MALFORMED"
    );
    assert_eq!(
        call("trim", json!({ "file": mp4(), "start": 9999 }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call(
            "trim",
            json!({
                "file": engine_shared::b64::encode(include_bytes!("fixtures/sample.mkv")),
                "start": 0
            })
        )["error_code"],
        "E_UNSUPPORTED"
    );
    assert_eq!(
        call("loop", json!({ "file": mp4(), "times": 1 }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("concat", json!({ "files": [{ "file": mp4() }] }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(call("extract_subtitles", json!({ "file": mp4() }))["error_code"], "E_NOT_FOUND");
    assert_eq!(
        call("subtitle_convert", json!({ "text": "garbage", "to": "vtt" }))["error_code"],
        "E_UNSUPPORTED_FORMAT"
    );
    assert_eq!(
        call(
            "subtitle_convert",
            json!({ "text": "1\n00:00:01,000 --> 00:00:02,000\nHi\n", "to": "docx" })
        )["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("gif_from_frames", json!({ "frames": [engine_shared::b64::encode(b"nope")] }))
            ["error_code"],
        "E_UNSUPPORTED_FORMAT"
    );
    assert_eq!(
        call("calc_bitrate", json!({ "duration_seconds": 0, "target_size_mb": 1 }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("aspect_ratio", json!({ "width": 0, "height": 1 }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("crop_calculator", json!({ "width": 10, "height": 10, "target": "bad" }))
            ["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("timecode", json!({ "timecode": "zz", "fps": 25 }))["error_code"],
        "E_INVALID_INPUT"
    );
    assert_eq!(
        call("transcode", json!({ "tool": "video-compress" }))["error_code"],
        "E_UNSUPPORTED"
    );
    assert_eq!(call("nope", json!({}))["error_code"], "E_BAD_REQUEST");
}

#[test]
fn empty_inputs() {
    assert_eq!(call("inspect", json!({ "file": "" }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("inspect", json!({}))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("trim", json!({ "file": "" }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("concat", json!({ "files": [] }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(
        call("subtitle_convert", json!({ "text": "", "to": "vtt" }))["error_code"],
        "E_EMPTY_INPUT"
    );
    assert_eq!(call("gif_from_frames", json!({ "frames": [] }))["error_code"], "E_EMPTY_INPUT");
    assert_eq!(call("timecode", json!({ "fps": 25 }))["error_code"], "E_EMPTY_INPUT");
}
