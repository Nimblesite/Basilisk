//! Implements [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
//! Source-level helpers: line/col computation.
//!
//! `basilisk_binary()` used to live here, so a fixture could spawn the built
//! CLI and drive a language server over its stdio. The CLI is inert
//! ([WITHDRAWAL-INERT]) — it starts no server — so there is nothing to spawn
//! and the helper is gone with the suites that used it.

/// Convert a byte offset in `source` into a 1-based (line, col) pair.
#[must_use]
pub fn line_col(source: &str, offset: u32) -> (usize, usize) {
    basilisk_common::text::line_col(source, usize::try_from(offset).unwrap_or(usize::MAX))
}
