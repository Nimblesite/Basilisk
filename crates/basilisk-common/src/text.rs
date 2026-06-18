//! Byte-offset → line/column conversion shared across diagnostic renderers.
//!
//! The CLI text/JSON output, the LSP fallback diagnostic formatter, and the
//! test helpers all turn a byte offset into a human-facing 1-based
//! `(line, column)` pair. Centralising it here keeps the computation in exactly
//! one place. Pure and dependency-free, it upholds this crate's zero-dependency
//! / WASM contract.

/// Convert a byte `offset` in `source` into a 1-based `(line, column)` pair.
///
/// The offset is clamped to the source length, so an out-of-range offset maps
/// to the end of the text rather than panicking.
#[must_use]
pub fn line_col(source: &str, offset: usize) -> (usize, usize) {
    let clamped = offset.min(source.len());
    let before = source.get(..clamped).unwrap_or(source);
    let line = before.chars().filter(|&c| c == '\n').count() + 1;
    let col = before.rfind('\n').map_or(clamped, |pos| clamped - pos - 1) + 1;
    (line, col)
}
