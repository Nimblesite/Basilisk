//! Mojo-inspired ownership and immutability analysis for Basilisk.
//!
//! Will house ownership tracking, immutability enforcement, and coercion
//! detection (BSK-E003x, BSK-E004x, BSK-E006x) in Phase 4.

/// Check a Python source string for Mojo-style ownership violations.
///
/// Phase 4: currently returns an empty list.  Once implemented, detects
/// mutation of `Borrowed` parameters, use-after-move, and implicit copies
/// (BSK-E003x / BSK-E004x / BSK-E006x).
#[must_use]
pub fn check_ownership(_source: &str) -> Vec<String> {
    Vec::new()
}
