//! Implements [CHKARCH-INCREMENTAL-SALSA]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA
//! Incremental computation database for Basilisk.
//!
//! One layer lives here: the **in-session** Salsa engine
//! ([CHKARCH-INCREMENTAL-SALSA]). The [`db::SourceFile`] input feeds a
//! demand-driven query graph whose derived queries (parse → resolve → check,
//! defined in the upstream crates) re-run only when an input they actually read
//! changed.
//!
//! The **cross-session** result cache used to live here too. It persisted
//! diagnostics keyed by their read-set so a fresh process could skip files that
//! had not changed on disk — a cold-start optimisation for `basilisk check
//! --cache`. That command is gone: the CLI is inert ([WITHDRAWAL-INERT]) and
//! checks nothing, so there are no results to cache and nothing that reads
//! them. The cache is deleted rather than kept warm for a rebuild that will not
//! reuse this code.

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
