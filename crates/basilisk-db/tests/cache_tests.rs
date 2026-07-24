//! Tests for [CHKCACHE-ENTRY] / [CHKCACHE-FINGERPRINT] / [CHKCACHE-CONTRACT].
//! See docs/specs/CHECKER-CACHE-SPEC.md#CHKCACHE-ENTRY
#![allow(clippy::allow_attributes, clippy::unwrap_used, clippy::expect_used)]
//! Crate-boundary tests for the result cache, covering every soundness branch:
//! a hit is returned only when the fingerprint and every recorded file match.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use basilisk_common::fs::{canonical_key, content_hash};
use basilisk_db::cache::{CheckCache, Fingerprint};

/// A fresh, unique temp directory for one test.
fn temp_dir(name: &str) -> PathBuf {
    static CTR: AtomicU64 = AtomicU64::new(0);
    let n = CTR.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("bsk_db_{name}_{}_{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn fingerprint() -> Fingerprint {
    Fingerprint {
        version: "1.2.3".to_owned(),
        config_hash: 42,
        env_hash: 7,
        typeshed_id: "commit:6ef9f7719ecfff09dad8724ef42b621fd994fb5e".to_owned(),
    }
}

/// Write a source file and return its `(canonical key, content hash)`.
fn write_dep(dir: &Path, name: &str, contents: &str) -> (PathBuf, String, u64) {
    let path = dir.join(name);
    std::fs::write(&path, contents).expect("write dep");
    let key = canonical_key(&path);
    (path, key, content_hash(contents))
}

#[test]
fn roundtrip_hit_returns_payload() {
    let dir = temp_dir("roundtrip");
    let cache = CheckCache::new(dir.join("cache"));
    let (target, key, hash) = write_dep(&dir, "t.py", "x = 1\n");
    let read_set: BTreeMap<String, u64> = [(key, hash)].into_iter().collect();

    cache
        .store(&target, &fingerprint(), read_set, &vec!["diag".to_owned()])
        .expect("store");
    let hit: Option<Vec<String>> = cache.lookup(&target, &fingerprint());
    assert_eq!(hit, Some(vec!["diag".to_owned()]), "unchanged inputs → hit");
}

#[test]
fn missing_entry_is_a_miss() {
    let dir = temp_dir("noentry");
    let cache = CheckCache::new(dir.join("cache"));
    let (target, ..) = write_dep(&dir, "t.py", "x = 1\n");
    let miss: Option<Vec<String>> = cache.lookup(&target, &fingerprint());
    assert_eq!(miss, None, "no stored entry → miss");
}

#[test]
fn corrupt_entry_is_a_miss() {
    let dir = temp_dir("corrupt");
    let cache_dir = dir.join("cache");
    let cache = CheckCache::new(cache_dir.clone());
    let (target, key, hash) = write_dep(&dir, "t.py", "x = 1\n");
    let read_set: BTreeMap<String, u64> = [(key, hash)].into_iter().collect();
    cache
        .store(&target, &fingerprint(), read_set, &vec!["diag".to_owned()])
        .expect("store");

    // Overwrite the single entry file with garbage.
    for entry in std::fs::read_dir(&cache_dir).expect("read cache dir") {
        std::fs::write(entry.expect("dir entry").path(), "not json").expect("corrupt");
    }
    let miss: Option<Vec<String>> = cache.lookup(&target, &fingerprint());
    assert_eq!(miss, None, "unparseable entry → miss, never a panic");
}

#[test]
fn fingerprint_mismatches_are_misses() {
    let dir = temp_dir("fp");
    let cache = CheckCache::new(dir.join("cache"));
    let (target, key, hash) = write_dep(&dir, "t.py", "x = 1\n");
    let read_set: BTreeMap<String, u64> = [(key, hash)].into_iter().collect();
    cache
        .store(&target, &fingerprint(), read_set, &vec!["diag".to_owned()])
        .expect("store");

    let bumped_version = Fingerprint {
        version: "9.9.9".to_owned(),
        ..fingerprint()
    };
    let bumped_config = Fingerprint {
        config_hash: 999,
        ..fingerprint()
    };
    let bumped_env = Fingerprint {
        env_hash: 999,
        ..fingerprint()
    };
    // [STUBRES-TYPESHED]: the active typeshed snapshot is part of the identity.
    // A moved `main`, a bundled fallback, or a re-pinned commit resolves to a
    // different snapshot under otherwise-identical inputs and MUST miss so
    // cached diagnostics never outlive the stubs that produced them.
    let bumped_typeshed = Fingerprint {
        typeshed_id: "bundled:83c2518a9e6abbda0c44592c3483de459198f887".to_owned(),
        ..fingerprint()
    };
    for (label, fp) in [
        ("version", bumped_version),
        ("config", bumped_config),
        ("env", bumped_env),
        ("typeshed", bumped_typeshed),
    ] {
        let miss: Option<Vec<String>> = cache.lookup(&target, &fp);
        assert_eq!(miss, None, "{label} change must force a miss");
    }
}

#[test]
fn changed_dependency_is_a_miss() {
    let dir = temp_dir("changedep");
    let cache = CheckCache::new(dir.join("cache"));
    let (target, tkey, thash) = write_dep(&dir, "a.py", "import b\n");
    let (dep, dkey, dhash) = write_dep(&dir, "b.py", "old\n");
    let read_set: BTreeMap<String, u64> = [(tkey, thash), (dkey, dhash)].into_iter().collect();
    cache
        .store(&target, &fingerprint(), read_set, &vec!["diag".to_owned()])
        .expect("store");

    std::fs::write(&dep, "new contents\n").expect("edit dep");
    let miss: Option<Vec<String>> = cache.lookup(&target, &fingerprint());
    assert_eq!(miss, None, "a changed dependency must force a miss");
}

#[test]
fn missing_dependency_is_a_miss() {
    let dir = temp_dir("missdep");
    let cache = CheckCache::new(dir.join("cache"));
    let (target, tkey, thash) = write_dep(&dir, "a.py", "import b\n");
    let (dep, dkey, dhash) = write_dep(&dir, "b.py", "data\n");
    let read_set: BTreeMap<String, u64> = [(tkey, thash), (dkey, dhash)].into_iter().collect();
    cache
        .store(&target, &fingerprint(), read_set, &vec!["diag".to_owned()])
        .expect("store");

    std::fs::remove_file(&dep).expect("remove dep");
    let miss: Option<Vec<String>> = cache.lookup(&target, &fingerprint());
    assert_eq!(miss, None, "a deleted dependency must force a miss");
}

#[test]
fn unserialisable_payload_errors() {
    let dir = temp_dir("badpayload");
    let cache = CheckCache::new(dir.join("cache"));
    let (target, key, hash) = write_dep(&dir, "t.py", "x = 1\n");
    let read_set: BTreeMap<String, u64> = [(key, hash)].into_iter().collect();

    // A map with non-string keys cannot be serialised to JSON → store errors.
    let payload: HashMap<(i32, i32), i32> = [((1, 2), 3)].into_iter().collect();
    let result = cache.store(&target, &fingerprint(), read_set, &payload);
    assert!(
        result.is_err(),
        "an unserialisable payload must surface an error"
    );
}
