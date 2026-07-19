//! Implements [CHKCACHE-ENTRY] / [CHKCACHE-FINGERPRINT].
//! See docs/specs/CHECKER-CACHE-SPEC.md#chkcache-entry
//!
//! A generic, content-addressed, on-disk result cache.
//!
//! [`CheckCache`] is agnostic to *what* it caches: the consumer (the CLI)
//! supplies a serialisable payload (the diagnostics) and the fingerprint of the
//! non-file inputs (version, config, environment). The cache stores the exact
//! read-set captured during the check and, on lookup, re-verifies every recorded
//! file against its stored hash. A hit is returned only when the fingerprint and
//! every file match — the [`CHKCACHE-CONTRACT`] guarantee.

use std::path::{Path, PathBuf};

use basilisk_common::fs::{canonical_key, content_hash, ReadSet};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// The non-file inputs that affect a check result.
///
/// These are fingerprinted as a unit; any change forces a miss.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    /// Checker version (`CARGO_PKG_VERSION`).
    pub version: String,
    /// Hash of the effective configuration.
    pub config_hash: u64,
    /// Hash of the resolution environment (search paths, `uv.lock`).
    pub env_hash: u64,
    /// Identity of the active standard-library typeshed snapshot
    /// ([STUBRES-TYPESHED], [CHKCACHE-FINGERPRINT]): the resolved commit/tree
    /// SHA, a custom-tree digest, or the bundled-snapshot SHA.
    ///
    /// This is a *runtime* identity that [`Self::config_hash`] cannot
    /// represent: unpinned "Latest" can resolve to different commits between
    /// runs, and a bundled fallback substitutes different `.pyi` bodies, all
    /// under byte-identical configuration. A change forces a miss so cached
    /// diagnostics never outlive the stubs that produced them.
    pub typeshed_id: String,
}

/// A persistent result cache rooted at a directory.
#[derive(Debug, Clone)]
pub struct CheckCache {
    dir: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct Dep {
    path: String,
    hash: u64,
}

#[derive(Serialize, Deserialize)]
struct Entry<T> {
    version: String,
    config_hash: u64,
    env_hash: u64,
    typeshed_id: String,
    deps: Vec<Dep>,
    payload: T,
}

impl CheckCache {
    /// Create a cache rooted at `dir`. The directory is created lazily on the
    /// first [`CheckCache::store`].
    #[must_use]
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Return the cached payload for `target` iff the fingerprint matches and
    /// every recorded dependency is byte-identical to when it was stored.
    ///
    /// Returns `None` on any mismatch, missing/unreadable dependency, or
    /// unparseable entry — never a stale result.
    // Implements [CHKCACHE-CONTRACT]
    #[must_use]
    pub fn lookup<T: DeserializeOwned>(
        &self,
        target: &Path,
        fingerprint: &Fingerprint,
    ) -> Option<T> {
        let raw = std::fs::read_to_string(self.entry_path(target)).ok()?;
        let entry: Entry<T> = serde_json::from_str(&raw).ok()?;
        if entry.version != fingerprint.version
            || entry.config_hash != fingerprint.config_hash
            || entry.env_hash != fingerprint.env_hash
            || entry.typeshed_id != fingerprint.typeshed_id
        {
            return None;
        }
        entry
            .deps
            .iter()
            .all(Self::dep_unchanged)
            .then_some(entry.payload)
    }

    /// Store `payload` for `target`, recording the exact `read_set` so a future
    /// lookup can re-verify it.
    ///
    /// # Errors
    ///
    /// Returns an [`std::io::Error`] if the cache directory or entry file cannot
    /// be written, or if the payload cannot be serialised.
    pub fn store<T: Serialize>(
        &self,
        target: &Path,
        fingerprint: &Fingerprint,
        read_set: ReadSet,
        payload: &T,
    ) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let deps = read_set
            .into_iter()
            .map(|(path, hash)| Dep { path, hash })
            .collect();
        let entry = Entry {
            version: fingerprint.version.clone(),
            config_hash: fingerprint.config_hash,
            env_hash: fingerprint.env_hash,
            typeshed_id: fingerprint.typeshed_id.clone(),
            deps,
            payload,
        };
        let raw = serde_json::to_string(&entry).map_err(std::io::Error::other)?;
        std::fs::write(self.entry_path(target), raw)
    }

    /// Re-read a recorded dependency and compare against its stored hash.
    fn dep_unchanged(dep: &Dep) -> bool {
        std::fs::read_to_string(&dep.path).is_ok_and(|current| content_hash(&current) == dep.hash)
    }

    /// On-disk path for a target's entry: `<dir>/<hash-of-canonical-path>.json`.
    fn entry_path(&self, target: &Path) -> PathBuf {
        let key = canonical_key(target);
        self.dir.join(format!("{:016x}.json", content_hash(&key)))
    }
}
