//! Implements [CHKCACHE-READSET-FS] / [CHKCACHE-READSET-GUARD].
//! See docs/specs/CHECKER-CACHE-SPEC.md#chkcache-readset
//!
//! File reads with optional read-set recording for the result cache.
//!
//! All checker file reads (`basilisk_parser::parse_file`,
//! `basilisk_stubs::parse_pyi_file`) route through [`read_tracked`]. When a
//! [`ReadRecorder`] guard is active on the current thread, every read records
//! `(canonical_path -> content_hash)`. With no recorder active the behaviour is
//! byte-for-byte identical to `std::fs::read_to_string`, so the LSP and
//! non-cached CLI runs are unaffected.
//!
//! This is std-only (no external dependency) so the crate still compiles to the
//! WASM target the Zed extension requires.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::Path;

/// A recorded read-set: canonical path -> content hash, ordered for
/// deterministic fingerprinting.
pub type ReadSet = BTreeMap<String, u64>;

thread_local! {
    static RECORDER: RefCell<Option<ReadSet>> = const { RefCell::new(None) };
}

/// Compute a content hash for change detection (non-cryptographic).
///
/// Equal hashes mean equal content for the purposes of cache invalidation; the
/// cache re-verifies every input individually, so this is only ever used to
/// detect *change*, never to prove identity across a trust boundary.
#[must_use]
pub fn content_hash(content: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

/// Read a file to a string, recording it into the active [`ReadRecorder`] (if
/// any) on the current thread.
///
/// # Errors
///
/// Propagates any [`std::io::Error`] from reading `path`.
pub fn read_tracked(path: &Path) -> io::Result<String> {
    let content = std::fs::read_to_string(path)?;
    RECORDER.with(|cell| {
        if let Some(set) = cell.borrow_mut().as_mut() {
            let _ = set.insert(canonical_key(path), content_hash(&content));
        }
    });
    Ok(content)
}

/// Canonicalise a path for use as a stable read-set key, falling back to the
/// original path when canonicalisation fails (e.g. the file was just removed).
#[must_use]
pub fn canonical_key(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

/// RAII guard recording the files read via [`read_tracked`] on the current
/// thread for its lifetime.
///
/// Starting a recorder replaces any in-progress one; dropping it without calling
/// [`ReadRecorder::finish`] clears the thread-local so a later check never
/// inherits a stale read-set.
#[derive(Debug)]
#[must_use = "a ReadRecorder records nothing useful unless its read-set is taken via finish()"]
pub struct ReadRecorder {
    _private: (),
}

impl ReadRecorder {
    /// Begin recording reads on the current thread.
    pub fn start() -> Self {
        RECORDER.with(|cell| *cell.borrow_mut() = Some(ReadSet::new()));
        Self { _private: () }
    }

    /// Stop recording and return the recorded read-set.
    #[must_use]
    pub fn finish(self) -> ReadSet {
        RECORDER
            .with(|cell| cell.borrow_mut().take())
            .unwrap_or_default()
    }
}

impl Drop for ReadRecorder {
    fn drop(&mut self) {
        RECORDER.with(|cell| {
            let _ = cell.borrow_mut().take();
        });
    }
}
