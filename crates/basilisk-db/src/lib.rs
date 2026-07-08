//! Implements [CHKARCH-INCREMENTAL-SALSA]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA
//! Incremental computation database for Basilisk.
//!
//! Two complementary layers live here:
//!
//! - [`db`] — the **in-session** Salsa engine ([CHKARCH-INCREMENTAL-SALSA]). The
//!   [`db::SourceFile`] input feeds a demand-driven query graph whose derived
//!   queries (parse → resolve → check, defined in the upstream crates) re-run
//!   only when an input they actually read changed. This is what makes an edit
//!   recompute one file instead of the whole workspace.
//! - [`cache`] — the **cross-session** content-addressed result cache
//!   ([CHKCACHE](../../../docs/specs/CHECKER-CACHE-SPEC.md),
//!   [CHKARCH-INCREMENTAL-CACHE]). It persists diagnostics keyed by their exact
//!   read-set so a fresh process skips re-checking files that did not change on
//!   disk, eliminating cold-start cost.

pub mod cache;
pub mod db;

pub use db::{BasiliskDatabase, Db, SourceFile};

/// Compute a content-based cache key for a source string.
///
/// Delegates to the shared [`basilisk_common::fs::content_hash`] so every layer
/// (Salsa input identity, result cache, read-recorder) computes identical
/// hashes.
#[must_use]
pub fn hash_source(source: &str) -> u64 {
    basilisk_common::fs::content_hash(source)
}
