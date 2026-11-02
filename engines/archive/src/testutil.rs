//! Test fixtures.
//!
//! Only compiled for tests (`cfg(test)` / `testutil` feature), so test-style
//! `unwrap` is acceptable here.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::archive;
use crate::InputFile;

/// A small set of files used by tests.
pub fn sample_files() -> Vec<InputFile> {
    vec![
        InputFile {
            name: "a.txt".into(),
            data: b"hello world".to_vec(),
            mtime_unix: Some(1_706_695_200),
            mode: None,
        },
        InputFile {
            name: "docs/".into(),
            data: vec![],
            mtime_unix: None,
            mode: None,
        },
        InputFile {
            name: "docs/readme.md".into(),
            data: b"# Readme\n".repeat(50),
            mtime_unix: Some(1_706_695_200),
            mode: Some(0o600),
        },
    ]
}

/// Zips the given files with default settings.
pub fn zip_of(files: &[InputFile]) -> Vec<u8> {
    let c = archive::create(files, "zip", 6, None, None, None).unwrap();
    engine_shared::b64::decode(c["output"]["file"].as_str().unwrap()).unwrap()
}
