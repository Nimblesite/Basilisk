//! Incremental computation database for Basilisk.
//!
//! This crate will house the Salsa-based incremental database in Phase 2.
//! Currently a placeholder for workspace coherence.

/// Compute a cache key for a source string.
///
/// Returns a content-based hash suitable for detecting changes.
/// Phase 2: uses the standard library's `DefaultHasher`.  The real
/// implementation will use a collision-resistant hash (e.g. xxHash).
#[must_use]
pub fn hash_source(source: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
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
