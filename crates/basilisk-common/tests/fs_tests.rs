//! Tests for [CHKCACHE-READSET-FS] / [CHKCACHE-READSET-GUARD].
//! See docs/specs/CHECKER-CACHE-SPEC.md#CHKCACHE-READSET
#![allow(clippy::allow_attributes, clippy::unwrap_used, clippy::expect_used)]
//! Crate-boundary tests for the read-tracking filesystem layer.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use basilisk_common::fs::{canonical_key, content_hash, read_tracked, ReadRecorder};

fn temp_file(name: &str, contents: &str) -> PathBuf {
    static CTR: AtomicU64 = AtomicU64::new(0);
    let n = CTR.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("bsk_fs_{}_{n}_{name}", std::process::id()));
    std::fs::write(&path, contents).expect("write temp file");
    path
}

#[test]
fn content_hash_is_deterministic_and_distinguishes() {
    assert_eq!(
        content_hash("abc"),
        content_hash("abc"),
        "stable for equal input"
    );
    assert_ne!(
        content_hash("abc"),
        content_hash("abd"),
        "differs for different input"
    );
}

#[test]
fn read_without_recorder_returns_content_and_records_nothing() {
    let file = temp_file("plain.py", "x = 1\n");
    let content = read_tracked(&file).expect("read");
    assert_eq!(content, "x = 1\n");
    // A recorder started afterwards sees only its own subsequent reads.
    let recorder = ReadRecorder::start();
    assert!(
        recorder.finish().is_empty(),
        "no reads recorded after start"
    );
    let _ = std::fs::remove_file(&file);
}

// Exercises [CHKCACHE-READSET]
#[test]
fn recorder_captures_reads_with_canonical_key_and_hash() {
    let file = temp_file("tracked.py", "y = 2\n");
    let recorder = ReadRecorder::start();
    let _ = read_tracked(&file).expect("read");
    let set = recorder.finish();
    assert_eq!(set.len(), 1, "exactly one file recorded");
    let key = canonical_key(&file);
    assert_eq!(
        set.get(&key).copied(),
        Some(content_hash("y = 2\n")),
        "key and hash recorded"
    );
    let _ = std::fs::remove_file(&file);
}

#[test]
fn read_error_propagates() {
    let missing = Path::new("/no/such/basilisk/fs/file.py");
    assert!(read_tracked(missing).is_err(), "missing file is an error");
}

#[test]
fn canonical_key_falls_back_for_missing_path() {
    let missing = Path::new("/no/such/basilisk/fs/key.py");
    assert_eq!(
        canonical_key(missing),
        missing.to_string_lossy(),
        "non-canonicalisable path falls back to itself"
    );
}

#[test]
fn dropping_recorder_without_finish_clears_state() {
    let file = temp_file("dropped.py", "z = 3\n");
    {
        let _recorder = ReadRecorder::start();
        let _ = read_tracked(&file).expect("read");
        // dropped here without finish()
    }
    // A fresh recorder must not inherit the dropped one's read-set.
    let recorder = ReadRecorder::start();
    let set = recorder.finish();
    assert!(
        set.is_empty(),
        "dropped recorder's reads must not leak forward"
    );
    let _ = std::fs::remove_file(&file);
}
