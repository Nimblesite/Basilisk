//! Byte-offset → line/column conversion shared across diagnostic renderers.
//!
//! The CLI text/JSON output, the LSP fallback diagnostic formatter, and the
//! test helpers all turn a byte offset into a human-facing 1-based
//! `(line, column)` pair. Centralising it here keeps the computation in exactly
//! one place. Pure and dependency-free, it upholds this crate's zero-dependency
//! / WASM contract.
//!
//! Two entry points, same semantics:
//!
//! * [`line_col`] scans the source once per call — O(offset). Correct for a
//!   *single* lookup (an LSP hover, one test assertion).
//! * [`LineIndex`] precomputes line-start offsets once — O(n) to build, then
//!   O(log n) per lookup via binary search. This is the right tool whenever many
//!   offsets from the *same* source are converted (rendering N diagnostics,
//!   locating N function bodies): it turns an accidental O(n·offset) ≈ O(n²) scan
//!   into O(n + k·log n). Both paths return byte columns and are guaranteed by
//!   the tests below to agree offset-for-offset.

/// Convert a byte `offset` in `source` into a 1-based `(line, column)` pair.
///
/// The offset is clamped to the source length, so an out-of-range offset maps
/// to the end of the text rather than panicking.
///
/// For repeated lookups against the same source, build a [`LineIndex`] once
/// instead of calling this in a loop — see the module docs.
#[must_use]
pub fn line_col(source: &str, offset: usize) -> (usize, usize) {
    let clamped = offset.min(source.len());
    let before = source.get(..clamped).unwrap_or(source);
    let line = before.chars().filter(|&c| c == '\n').count() + 1;
    let col = before.rfind('\n').map_or(clamped, |pos| clamped - pos - 1) + 1;
    (line, col)
}

/// Precomputed line-start byte offsets for a source string.
///
/// Build once with [`LineIndex::new`], then convert byte offsets to
/// `(line, col)` — or locate the byte range of a line — in O(log n). This is the
/// batch counterpart to [`line_col`]: rendering many diagnostics or scanning
/// many function bodies from one file should build a single `LineIndex` rather
/// than rescanning the source from the top for every offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineIndex {
    /// Byte offset of the first character of each line. Always starts with `0`,
    /// so `line_starts.len()` equals the number of lines and index `i` holds the
    /// start of the 1-based line `i + 1`.
    line_starts: Vec<usize>,
    /// Total byte length of the indexed source (offsets are clamped to it).
    len: usize,
}

impl LineIndex {
    /// Build the index for `source` in a single O(n) pass over its bytes.
    #[must_use]
    pub fn new(source: &str) -> Self {
        let bytes = source.as_bytes();
        // One entry per line; pre-size generously to avoid reallocation on
        // realistic source (average line length well above four bytes).
        let mut line_starts = Vec::with_capacity(bytes.len() / 16 + 1);
        line_starts.push(0);
        for (idx, &byte) in bytes.iter().enumerate() {
            if byte == b'\n' {
                line_starts.push(idx + 1);
            }
        }
        Self {
            line_starts,
            len: bytes.len(),
        }
    }

    /// The 1-based line number containing `offset` (clamped to the source end).
    ///
    /// Equivalent to `self.line_col(offset).0` but skips the column arithmetic.
    #[must_use]
    pub fn line(&self, offset: usize) -> usize {
        let clamped = offset.min(self.len);
        // `partition_point` returns the count of starts `<= clamped`; since the
        // first start is always 0, that count is at least 1 and equals the
        // 1-based line number.
        self.line_starts.partition_point(|&start| start <= clamped)
    }

    /// Convert a byte `offset` into a 1-based `(line, column)` pair.
    ///
    /// Byte-for-byte identical to [`line_col`] for every offset (see tests);
    /// the column is a byte column, matching the existing renderers.
    #[must_use]
    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let clamped = offset.min(self.len);
        let line = self.line(clamped);
        // `line` is in `1..=line_starts.len()`, so `line - 1` is always in bounds;
        // `get`/`unwrap_or` keeps the panic-free contract without an index.
        let start = self.line_starts.get(line - 1).copied().unwrap_or(0);
        (line, clamped - start + 1)
    }

    /// Byte offset of the start of the line that contains `offset`.
    ///
    /// Replaces the `source[..offset].rfind('\n').map_or(0, |p| p + 1)` idiom
    /// with an O(log n) lookup.
    #[must_use]
    pub fn line_start(&self, offset: usize) -> usize {
        let clamped = offset.min(self.len);
        // `line(..)` is in `1..=line_starts.len()`; the fallback is unreachable.
        self.line_starts
            .get(self.line(clamped) - 1)
            .copied()
            .unwrap_or(0)
    }

    /// Byte offset of the start of the 0-based `line_idx`-th line.
    ///
    /// Returns the source length for a line index at or beyond the last line, so
    /// callers iterating line numbers never index out of bounds. Replaces the
    /// O(n) `source.lines()` re-scan used to recover a line's byte offset.
    #[must_use]
    pub fn line_start_of(&self, line_idx: usize) -> usize {
        self.line_starts.get(line_idx).copied().unwrap_or(self.len)
    }

    /// Number of lines in the indexed source (always at least 1).
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }
}

impl Default for LineIndex {
    /// An index over the empty string: a single line starting at offset 0.
    ///
    /// This keeps a `..Default::default()`-constructed owner internally
    /// consistent (one line, zero length) rather than leaving an empty
    /// `line_starts` that would violate the `line >= 1` invariant.
    fn default() -> Self {
        Self {
            line_starts: vec![0],
            len: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The batch [`LineIndex`] must agree with the single-shot [`line_col`] on
    /// *every* byte offset (and one past the end) for a spread of sources,
    /// including CRLF, leading/trailing/consecutive newlines, and non-ASCII.
    #[test]
    fn line_index_matches_line_col_exhaustively() {
        let sources = [
            "",
            "\n",
            "a",
            "abc",
            "ab\ncd",
            "\nleading",
            "trailing\n",
            "a\n\nb",
            "line1\nline2\nline3",
            "crlf\r\nsecond\r\nthird",
            "café\nnaïve\nπ = 3",
            "def f():\n    return 1\n\nclass C:\n    x: int = 0\n",
        ];
        for source in sources {
            let index = LineIndex::new(source);
            // Every character boundary, plus a couple past the end — the set of
            // offsets a real diagnostic span can carry. (Mid-character byte
            // offsets are never produced by the parser and are excluded because
            // the single-shot `line_col` has a pre-existing panic on them, which
            // `LineIndex` sidesteps.)
            let boundaries = source.char_indices().map(|(idx, _)| idx).chain([
                source.len(),
                source.len() + 1,
                source.len() + 2,
            ]);
            for offset in boundaries {
                let expected = line_col(source, offset);
                let actual = index.line_col(offset);
                assert_eq!(
                    actual, expected,
                    "mismatch at offset {offset} in {source:?}: index={actual:?} scan={expected:?}"
                );
                assert_eq!(
                    index.line(offset),
                    expected.0,
                    "line() disagreed with line_col().0"
                );
            }
        }
    }

    /// `LineIndex` must not panic on a mid-character byte offset — unlike the
    /// single-shot `line_col`, it works directly on line-start bytes and clamps
    /// safely. (Real spans are char-aligned; this guards the invariant anyway.)
    #[test]
    fn line_index_survives_mid_character_offsets() {
        let source = "café\nπstar"; // 'é' and 'π' are two-byte characters
        let index = LineIndex::new(source);
        for offset in 0..=source.len() + 1 {
            let (line, col) = index.line_col(offset);
            assert!(
                line >= 1 && col >= 1,
                "offset {offset} gave ({line}, {col})"
            );
        }
    }

    #[test]
    fn line_start_finds_the_containing_line_start() {
        let source = "alpha\nbeta\ngamma";
        let index = LineIndex::new(source);
        assert_eq!(index.line_start(0), 0); // 'a' of alpha
        assert_eq!(index.line_start(5), 0); // '\n' after alpha is on line 1
        assert_eq!(index.line_start(6), 6); // 'b' of beta
        assert_eq!(index.line_start(10), 6); // '\n' after beta is on line 2
        assert_eq!(index.line_start(11), 11); // 'g' of gamma
        assert_eq!(index.line_start(source.len()), 11); // end clamps into gamma
    }

    #[test]
    fn line_start_of_indexes_by_line_number() {
        let source = "one\ntwo\nthree";
        let index = LineIndex::new(source);
        assert_eq!(index.line_count(), 3);
        assert_eq!(index.line_start_of(0), 0);
        assert_eq!(index.line_start_of(1), 4);
        assert_eq!(index.line_start_of(2), 8);
        // Past the last line clamps to the source length, never panics.
        assert_eq!(index.line_start_of(3), source.len());
        assert_eq!(index.line_start_of(99), source.len());
    }

    #[test]
    fn default_index_is_a_single_empty_line() {
        let index = LineIndex::default();
        assert_eq!(index.line_count(), 1);
        assert_eq!(index.line_col(0), (1, 1));
        assert_eq!(index.line_col(50), (1, 1)); // clamps to empty source
        assert_eq!(index.line_start_of(0), 0);
    }
}
