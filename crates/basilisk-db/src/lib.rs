//! Implements [CHKARCH-INCREMENTAL-SALSA]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA
//! Incremental computation database for Basilisk.
//!
//! This crate houses the opt-in CLI result cache ([`cache`], see
//! [CHKCACHE](../../../docs/specs/CHECKER-CACHE-SPEC.md)) and will grow into the
//! Salsa-based incremental database in Phase 2.

pub mod cache;

/// Compute a content-based cache key for a source string.
///
/// Delegates to the shared [`basilisk_common::fs::content_hash`] so every layer
/// (stub cache, result cache, read-recorder) computes identical hashes.
#[must_use]
pub fn hash_source(source: &str) -> u64 {
    basilisk_common::fs::content_hash(source)
}

/// Check whether a source file with the given hash needs to be rechecked.
///
/// Returns `false` when the cached result for this hash is still valid
/// (i.e. the source has not changed).  Phase 2 will replace this with a
/// Salsa-backed persistent cache; for now the content-addressed hash itself
/// guarantees identity — equal hashes mean equal content, so no recheck.
#[must_use]
pub fn needs_recheck(_source_hash: u64) -> bool {
    false
}
