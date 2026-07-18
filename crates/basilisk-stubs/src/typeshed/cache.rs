//! Implements [STUBRES-TYPESHED-ACQUIRE] cache. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-ACQUIRE
//!
//! The on-disk immutable-ZIP cache.
//!
//! A verified download is cached as the **immutable ZIP** it was accepted as,
//! beside a small metadata record. Reuse **re-hashes the cached ZIP** and
//! compares it to the recorded SHA-256, so on-disk mutation is detected without
//! extraction — the checker never trusts a mutable extracted tree
//! ([STUBRES-TYPESHED-ACQUIRE]). Accepted immutable bytes have no time-based
//! expiry; explicit eviction or cache-off forces reacquisition through every
//! activation gate.

use std::fs::{self, File};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use super::gate::manifest::sha256_hex;

static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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

    /// Load the cached entry for `key`, re-hashing the ZIP to detect mutation.
    ///
    /// Returns `Ok(None)` when the entry is absent.
    ///
    /// # Errors
    ///
    /// Returns [`CacheError::Mutation`] if the stored ZIP no longer matches its
    /// recorded hash, or an I/O/metadata error otherwise.
    pub fn load(&self, key: &CacheKey) -> Result<Option<CachedArchive>, CacheError> {
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
        promoted.sort_by_key(std::fs::DirEntry::file_name);
        let Some(generation) = promoted.first().map(std::fs::DirEntry::path) else {
            return Ok(None);
        };
        let zip_path = generation.join("archive.zip");
        let meta_path = generation.join("meta.json");
        let zip = fs::read(&zip_path).map_err(|err| CacheError::Io(err.to_string()))?;
        let meta = fs::read(&meta_path).map_err(|err| CacheError::Io(err.to_string()))?;
        let record: CacheRecord =
            serde_json::from_slice(&meta).map_err(|err| CacheError::Metadata(err.to_string()))?;
        let actual = sha256_hex(&zip);
        if actual != record.zip_sha256 {
            quarantine_generation(&generations, &generation)?;
            return Err(CacheError::Mutation {
                expected: record.zip_sha256,
                actual,
            });
        }
        Ok(Some(CachedArchive { zip, record }))
    }
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
    fn record(zip: &[u8]) -> CacheRecord {
        CacheRecord {
            commit: Some("83c2518a9e6abbda0c44592c3483de459198f887".to_owned()),
            tree: None,
            zip_sha256: sha256_hex(zip),
            verified: true,
            transport: Some("codeload".to_owned()),
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
        let loaded = cache.load(&key);
        assert!(matches!(&loaded, Ok(Some(entry)) if entry.zip == zip));
    }

    #[test]
    fn missing_entry_is_none() {
        let Ok(dir) = tempfile::tempdir() else {
            return;
        };
        let cache = DiskCache::new(dir.path());
        let key = CacheKey::from_identity("deadbeef");
        assert!(matches!(cache.load(&key), Ok(None)));
    }

    #[test]
    fn mutation_is_detected_on_reuse() {
        let Ok(dir) = tempfile::tempdir() else {
            return;
        };
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
        assert!(fs::write(&tampered, b"tampered bytes").is_ok());
        assert!(matches!(cache.load(&key), Err(CacheError::Mutation { .. })));
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
        assert!(matches!(cache.load(&key), Ok(None)));

        let zip = b"complete generation";
        assert!(cache.store(&key, zip, &record(zip)).is_ok());
        assert!(matches!(cache.load(&key), Ok(Some(entry)) if entry.zip == zip));
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
        assert!(matches!(cache.load(&key), Ok(Some(_))));
    }

    #[test]
    fn key_sanitizes_unsafe_characters() {
        let key = CacheKey::from_identity("custom-/etc/passwd");
        assert!(!key.dir_name().contains('/'));
    }
}
