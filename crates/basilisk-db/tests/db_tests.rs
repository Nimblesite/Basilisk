//! Tests for [CHKARCH-INCREMENTAL-SALSA]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for basilisk-db.

#[test]
fn incremental_db_avoids_recheck_when_source_unchanged() {
    // Phase 2: the incremental database must cache results so that an unchanged
    // file does not need rechecking.  Currently always rechecks (placeholder).
    let hash = basilisk_db::hash_source("x: int = 1\n");
    assert!(
        !basilisk_db::needs_recheck(hash),
        "unchanged file must not need rechecking — Phase 2 incremental DB not yet implemented"
    );
}

#[test]
fn different_sources_produce_different_hashes() {
    // Hash function must distinguish different source strings.
    let hash_a = basilisk_db::hash_source("x = 1\n");
    let hash_b = basilisk_db::hash_source("x = 2\n");
    assert_ne!(
        hash_a, hash_b,
        "different sources must produce different hashes"
    );
}
