//! Implements [STUBRES-TYPESHED-ACQUIRE] cache. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-ACQUIRE
//!
//! The on-disk immutable-ZIP cache.
//!
//! A verified download is cached as the **immutable ZIP** it was accepted as,
//! beside a small metadata record. Reuse **re-hashes the cached ZIP** and
//! compares it to the recorded SHA-256, so on-disk mutation is detected without
//! extraction — the checker never trusts a mutable extracted tree
//! ([STUBRES-TYPESHED-ACQUIRE]). Downloaded bytes expire after 24 hours; an
//! exact commit pin does not, so expiry reacquires that same immutable commit.

use std::fs::{self, File};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use super::gate::manifest::sha256_hex;

static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Maximum age of downloaded cached ZIP bytes. Commit selection is independent:
/// an explicit pin remains the same identity when its bytes must be reacquired.
pub const CACHE_MAX_AGE_SECONDS: u64 = 24 * 60 * 60;

/// A cache key derived from a source identity's opaque URI component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheKey(String);

impl CacheKey {
    /// Build a key from a source identity's URI component (a SHA or path digest).
    #[must_use]
    pub fn from_identity(uri_component: &str) -> Self {
        // The component is already a SHA or digest; keep only path-safe chars.
        let safe: String = uri_component
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' {
                    ch
                } else {
                    '_'
                }
            })
            .collect();
        Self(safe)
    }

    /// The on-disk subdirectory name.
    #[must_use]
    pub fn dir_name(&self) -> &str {
        &self.0
    }
}

/// The metadata stored beside a cached ZIP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheRecord {
    /// The commit SHA, when known.
    pub commit: Option<String>,
    /// The verified root-tree SHA, when content was attested.
    pub tree: Option<String>,
    /// SHA-256 of the cached ZIP bytes — the mutation check on every reuse.
    pub zip_sha256: String,
    /// Whether content verification ran when the entry was stored.
    pub verified: bool,
    /// Actual archive byte origin (`codeload` or `mirror`). This is persisted
    /// so reuse cannot be relabelled by a later configuration change.
    #[serde(default)]
    pub transport: Option<String>,
    /// Unix timestamp when these downloaded bytes passed the activation gates.
    /// Legacy records default to zero and are therefore never reused.
    #[serde(default)]
    pub acquired_at_unix_seconds: u64,
    /// Trusted recursive Git file metadata retained so offline cache reuse can
    /// restore modes that codeload ZIPs do not preserve.
    #[serde(default)]
    pub tree_files: Vec<CachedTreeFile>,
}

/// One persisted trusted recursive-tree leaf.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedTreeFile {
    /// Repository-relative path.
    pub path: String,
    /// Full Git blob object ID.
    pub oid: String,
    /// Canonical Git leaf mode (`100644`, `100755`, `120000`, or `160000`).
    pub mode: String,
}

/// A cached ZIP plus its metadata.
#[derive(Debug, Clone)]
pub struct CachedArchive {
    /// The immutable ZIP bytes.
    pub zip: Vec<u8>,
    /// Its metadata record.
    pub record: CacheRecord,
}

/// A cache operation failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CacheError {
    /// A filesystem error.
    #[error("cache io error: {0}")]
    Io(String),
    /// The metadata record could not be (de)serialized.
    #[error("cache metadata error: {0}")]
    Metadata(String),
    /// The cached ZIP's hash did not match its record — on-disk mutation.
    #[error("cached archive mutated: recorded {expected}, found {actual}")]
    Mutation {
        /// The recorded SHA-256.
        expected: String,
        /// The freshly computed SHA-256.
        actual: String,
    },
}

/// The on-disk immutable-ZIP cache rooted at a directory.
#[derive(Debug, Clone)]
pub struct DiskCache {
    root: PathBuf,
}

impl DiskCache {
    /// Root the cache at `root`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn entry_dir(&self, key: &CacheKey) -> PathBuf {
        self.root.join(key.dir_name())
    }

    /// Store `zip` and `record` atomically under `key`. The record's
    /// `zip_sha256` MUST already match `zip`; callers set it from the same bytes.
    ///
    /// # Errors
    ///
    /// Returns [`CacheError`] on I/O or serialization failure.
    pub fn store(
        &self,
        key: &CacheKey,
        zip: &[u8],
        record: &CacheRecord,
    ) -> Result<(), CacheError> {
        let dir = self.entry_dir(key);
        let generations = dir.join("generations");
        fs::create_dir_all(&generations).map_err(|err| CacheError::Io(err.to_string()))?;
        let actual = sha256_hex(zip);
        if actual != record.zip_sha256 {
            return Err(CacheError::Mutation {
                expected: record.zip_sha256.clone(),
                actual,
            });
        }
        let meta = serde_json::to_vec_pretty(record)
            .map_err(|err| CacheError::Metadata(err.to_string()))?;
        let generation = generations.join(&record.zip_sha256);
        if generation.is_dir() {
            if stored_generation_matches(&generation, record) {
                return Ok(());
            }
            quarantine_generation(&generations, &generation)?;
        }
        let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let staging = generations.join(format!(".stage-{}-{sequence}", std::process::id()));
        fs::create_dir(&staging).map_err(|err| CacheError::Io(err.to_string()))?;
        let staged = stage_generation(&staging, zip, &meta);
        if let Err(error) = staged {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
        match fs::rename(&staging, &generation) {
            Ok(()) => sync_dir(&generations),
            Err(_error) if generation.is_dir() => {
                let _ = fs::remove_dir_all(&staging);
                Ok(())
            }
            Err(error) => {
                let _ = fs::remove_dir_all(&staging);
                Err(CacheError::Io(error.to_string()))
            }
        }
    }

    /// Load the freshest cached encoding for `key`, re-hashing every ZIP before
    /// reuse. Multiple encodings can exist for one immutable commit.
    ///
    /// Returns `Ok(None)` when the entry is absent.
    ///
    /// # Errors
    ///
    /// Returns [`CacheError::Mutation`] if the stored ZIP no longer matches its
    /// recorded hash, or an I/O/metadata error otherwise.
    pub fn load_fresh(
        &self,
        key: &CacheKey,
        now_unix_seconds: u64,
    ) -> Result<Option<CachedArchive>, CacheError> {
        let dir = self.entry_dir(key);
        let generations = dir.join("generations");
        if !generations.is_dir() {
            return Ok(None);
        }
        let mut promoted = fs::read_dir(&generations)
            .map_err(|err| CacheError::Io(err.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| CacheError::Io(err.to_string()))?;
        promoted.retain(|entry| {
            entry.file_type().is_ok_and(|kind| kind.is_dir())
                && !entry.file_name().to_string_lossy().starts_with('.')
        });
        let mut freshest: Option<CachedArchive> = None;
        for entry in promoted {
            let candidate = load_generation(&generations, &entry.path())?;
            if !record_is_fresh(&candidate.record, now_unix_seconds) {
                continue;
            }
            let replace = match &freshest {
                None => true,
                Some(current) => {
                    let candidate_key = (
                        candidate.record.acquired_at_unix_seconds,
                        candidate.record.zip_sha256.as_str(),
                    );
                    let current_key = (
                        current.record.acquired_at_unix_seconds,
                        current.record.zip_sha256.as_str(),
                    );
                    candidate_key > current_key
                }
            };
            if replace {
                freshest = Some(candidate);
            }
        }
        Ok(freshest)
    }
}

fn load_generation(generations: &Path, generation: &Path) -> Result<CachedArchive, CacheError> {
    let zip =
        fs::read(generation.join("archive.zip")).map_err(|err| CacheError::Io(err.to_string()))?;
    let meta =
        fs::read(generation.join("meta.json")).map_err(|err| CacheError::Io(err.to_string()))?;
    let record: CacheRecord =
        serde_json::from_slice(&meta).map_err(|err| CacheError::Metadata(err.to_string()))?;
    let actual = sha256_hex(&zip);
    if actual != record.zip_sha256 {
        quarantine_generation(generations, generation)?;
        return Err(CacheError::Mutation {
            expected: record.zip_sha256,
            actual,
        });
    }
    Ok(CachedArchive { zip, record })
}

fn record_is_fresh(record: &CacheRecord, now_unix_seconds: u64) -> bool {
    record.acquired_at_unix_seconds != 0
        && now_unix_seconds >= record.acquired_at_unix_seconds
        && now_unix_seconds - record.acquired_at_unix_seconds < CACHE_MAX_AGE_SECONDS
}

fn stored_generation_matches(generation: &Path, expected: &CacheRecord) -> bool {
    let Ok(zip) = fs::read(generation.join("archive.zip")) else {
        return false;
    };
    let Ok(meta) = fs::read(generation.join("meta.json")) else {
        return false;
    };
    let Ok(record) = serde_json::from_slice::<CacheRecord>(&meta) else {
        return false;
    };
    record == *expected && sha256_hex(&zip) == expected.zip_sha256
}

fn quarantine_generation(generations: &Path, generation: &Path) -> Result<(), CacheError> {
    let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let quarantine = generations.join(format!(".corrupt-{}-{sequence}", std::process::id()));
    fs::rename(generation, quarantine).map_err(|err| CacheError::Io(err.to_string()))?;
    sync_dir(generations)
}

fn stage_generation(staging: &Path, zip: &[u8], meta: &[u8]) -> Result<(), CacheError> {
    write_synced(&staging.join("archive.zip"), zip)?;
    write_synced(&staging.join("meta.json"), meta)?;
    sync_dir(staging)
}

fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), CacheError> {
    let mut file = File::create(path).map_err(|err| CacheError::Io(err.to_string()))?;
    file.write_all(bytes)
        .map_err(|err| CacheError::Io(err.to_string()))?;
    file.sync_all()
        .map_err(|err| CacheError::Io(err.to_string()))
}

#[cfg(unix)]
fn sync_dir(path: &Path) -> Result<(), CacheError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|err| CacheError::Io(err.to_string()))
}

#[cfg(not(unix))]
fn sync_dir(_path: &Path) -> Result<(), CacheError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_000_000;

    fn record(zip: &[u8]) -> CacheRecord {
        CacheRecord {
            commit: Some("83c2518a9e6abbda0c44592c3483de459198f887".to_owned()),
            tree: None,
            zip_sha256: sha256_hex(zip),
            verified: true,
            transport: Some("codeload".to_owned()),
            acquired_at_unix_seconds: NOW,
            tree_files: Vec::new(),
        }
    }

    #[test]
    fn store_then_load_round_trips() {
        let Ok(dir) = tempfile::tempdir() else {
            return;
        };
        let cache = DiskCache::new(dir.path());
        let key = CacheKey::from_identity("83c2518a9e6abbda0c44592c3483de459198f887");
        let zip = b"PK\x03\x04 fake zip bytes";
        assert!(cache.store(&key, zip, &record(zip)).is_ok());
        let loaded = cache.load_fresh(&key, NOW);
        assert!(matches!(&loaded, Ok(Some(entry)) if entry.zip == zip));
    }

    #[test]
    fn store_rejects_bytes_that_do_not_match_the_recorded_digest(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let cache = DiskCache::new(dir.path());
        let key = CacheKey::from_identity("digest-mismatch");
        let expected = record(b"expected bytes");

        assert!(matches!(
            cache.store(&key, b"different bytes", &expected),
            Err(CacheError::Mutation { .. })
        ));
        assert!(matches!(cache.load_fresh(&key, NOW), Ok(None)));
        Ok(())
    }

    #[test]
    fn conflicting_metadata_for_one_encoding_is_quarantined_and_replaced(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let cache = DiskCache::new(dir.path());
        let key = CacheKey::from_identity("metadata-conflict");
        let zip = b"one immutable encoding";
        let first = record(zip);
        assert!(cache.store(&key, zip, &first).is_ok());

        let mut replacement = first;
        replacement.acquired_at_unix_seconds = NOW + 1;
        assert!(cache.store(&key, zip, &replacement).is_ok());

        let loaded = cache.load_fresh(&key, NOW + 1);
        assert!(matches!(
            loaded,
            Ok(Some(entry)) if entry.record == replacement && entry.zip == zip
        ));
        let generations = dir.path().join("metadata-conflict").join("generations");
        let quarantined = fs::read_dir(generations)?
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().starts_with(".corrupt-"));
        assert!(quarantined);
        Ok(())
    }

    #[test]
    fn missing_entry_is_none() {
        let Ok(dir) = tempfile::tempdir() else {
            return;
        };
        let cache = DiskCache::new(dir.path());
        let key = CacheKey::from_identity("deadbeef");
        assert!(matches!(cache.load_fresh(&key, NOW), Ok(None)));
    }

    #[test]
    fn mutation_is_detected_on_reuse() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let cache = DiskCache::new(dir.path());
        let key = CacheKey::from_identity("abc123");
        let zip = b"original bytes";
        assert!(cache.store(&key, zip, &record(zip)).is_ok());
        // Tamper with the stored ZIP on disk.
        let tampered = dir
            .path()
            .join("abc123")
            .join("generations")
            .join(sha256_hex(zip))
            .join("archive.zip");
        fs::write(&tampered, b"tampered bytes")?;
        assert!(matches!(
            cache.load_fresh(&key, NOW),
            Err(CacheError::Mutation { .. })
        ));
        assert!(matches!(cache.load_fresh(&key, NOW), Ok(None)));
        Ok(())
    }

    #[test]
    fn interrupted_staging_directory_never_activates() {
        let Ok(dir) = tempfile::tempdir() else {
            return;
        };
        let cache = DiskCache::new(dir.path());
        let key = CacheKey::from_identity("interrupted");
        let staging = dir
            .path()
            .join("interrupted")
            .join("generations")
            .join(".stage-interrupted");
        assert!(fs::create_dir_all(&staging).is_ok());
        assert!(fs::write(staging.join("archive.zip"), b"partial").is_ok());
        assert!(matches!(cache.load_fresh(&key, NOW), Ok(None)));

        let zip = b"complete generation";
        assert!(cache.store(&key, zip, &record(zip)).is_ok());
        assert!(matches!(cache.load_fresh(&key, NOW), Ok(Some(entry)) if entry.zip == zip));
    }

    #[test]
    fn concurrent_promotions_expose_only_complete_generations() {
        let Ok(dir) = tempfile::tempdir() else {
            return;
        };
        let cache = DiskCache::new(dir.path());
        let key = CacheKey::from_identity("concurrent");
        std::thread::scope(|scope| {
            for zip in [b"encoding-a".as_slice(), b"encoding-b".as_slice()] {
                let cache = cache.clone();
                let key = key.clone();
                let _ = scope.spawn(move || cache.store(&key, zip, &record(zip)));
            }
        });
        assert!(matches!(cache.load_fresh(&key, NOW), Ok(Some(_))));
    }

    #[test]
    fn bytes_expire_at_twenty_four_hour_boundary() {
        let Ok(dir) = tempfile::tempdir() else {
            return;
        };
        let cache = DiskCache::new(dir.path());
        let key = CacheKey::from_identity("expiry");
        let zip = b"immutable bytes";
        assert!(cache.store(&key, zip, &record(zip)).is_ok());
        assert!(matches!(
            cache.load_fresh(&key, NOW + CACHE_MAX_AGE_SECONDS - 1),
            Ok(Some(_))
        ));
        assert!(matches!(
            cache.load_fresh(&key, NOW + CACHE_MAX_AGE_SECONDS),
            Ok(None)
        ));
    }

    #[test]
    fn future_or_legacy_timestamp_is_not_reused() {
        let Ok(dir) = tempfile::tempdir() else {
            return;
        };
        let cache = DiskCache::new(dir.path());
        let key = CacheKey::from_identity("clock");
        let zip = b"clock bytes";
        let mut future = record(zip);
        future.acquired_at_unix_seconds = NOW + 1;
        assert!(cache.store(&key, zip, &future).is_ok());
        assert!(matches!(cache.load_fresh(&key, NOW), Ok(None)));

        let legacy_key = CacheKey::from_identity("legacy");
        let mut legacy = record(zip);
        legacy.acquired_at_unix_seconds = 0;
        assert!(cache.store(&legacy_key, zip, &legacy).is_ok());
        assert!(matches!(cache.load_fresh(&legacy_key, NOW), Ok(None)));
    }

    #[test]
    fn newer_fresh_encoding_wins_over_an_expired_lower_digest() {
        let Ok(dir) = tempfile::tempdir() else {
            return;
        };
        let cache = DiskCache::new(dir.path());
        let key = CacheKey::from_identity("same-commit");
        let first = b"first archive encoding";
        let second = b"second archive encoding";
        let (expired_zip, fresh_zip) = if sha256_hex(first) < sha256_hex(second) {
            (first.as_slice(), second.as_slice())
        } else {
            (second.as_slice(), first.as_slice())
        };
        let mut expired = record(expired_zip);
        expired.acquired_at_unix_seconds = NOW - CACHE_MAX_AGE_SECONDS;
        assert!(cache.store(&key, expired_zip, &expired).is_ok());
        assert!(cache.store(&key, fresh_zip, &record(fresh_zip)).is_ok());

        let loaded = cache.load_fresh(&key, NOW);
        assert!(matches!(loaded, Ok(Some(entry)) if entry.zip == fresh_zip));
    }

    #[test]
    fn equal_age_encodings_resolve_deterministically_by_digest(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let cache = DiskCache::new(dir.path());
        let key = CacheKey::from_identity("equal-age");
        let first = b"first equally fresh encoding";
        let second = b"second equally fresh encoding";
        assert!(cache.store(&key, first, &record(first)).is_ok());
        assert!(cache.store(&key, second, &record(second)).is_ok());

        let expected = if sha256_hex(first) > sha256_hex(second) {
            first.as_slice()
        } else {
            second.as_slice()
        };
        assert!(matches!(
            cache.load_fresh(&key, NOW),
            Ok(Some(entry)) if entry.zip == expected
        ));
        Ok(())
    }

    #[test]
    fn fresher_encoding_wins_even_when_its_digest_sorts_lower(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let cache = DiskCache::new(dir.path());
        let key = CacheKey::from_identity("freshness-first");
        let first = b"first different encoding";
        let second = b"second different encoding";
        let (lower_digest, higher_digest) = if sha256_hex(first) < sha256_hex(second) {
            (first.as_slice(), second.as_slice())
        } else {
            (second.as_slice(), first.as_slice())
        };
        let mut older = record(higher_digest);
        older.acquired_at_unix_seconds = NOW - 1;
        let newer = record(lower_digest);
        assert!(cache.store(&key, higher_digest, &older).is_ok());
        assert!(cache.store(&key, lower_digest, &newer).is_ok());

        assert!(matches!(
            cache.load_fresh(&key, NOW),
            Ok(Some(entry)) if entry.zip == lower_digest
        ));
        Ok(())
    }

    #[test]
    fn malformed_promoted_metadata_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let cache = DiskCache::new(dir.path());
        let key = CacheKey::from_identity("bad-metadata");
        let generation = dir
            .path()
            .join("bad-metadata")
            .join("generations")
            .join("generation");
        fs::create_dir_all(&generation)?;
        fs::write(generation.join("archive.zip"), b"bytes")?;
        fs::write(generation.join("meta.json"), b"not json")?;

        assert!(matches!(
            cache.load_fresh(&key, NOW),
            Err(CacheError::Metadata(_))
        ));
        Ok(())
    }

    #[test]
    fn key_sanitizes_unsafe_characters() {
        let key = CacheKey::from_identity("custom-/etc/passwd");
        assert!(!key.dir_name().contains('/'));
    }
}
