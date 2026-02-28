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
/// Phase 2: always returns `true` (no incremental caching yet).
/// Once implemented, returns `false` when cached results are still valid.
#[must_use]
pub fn needs_recheck(_source_hash: u64) -> bool {
    true
}
