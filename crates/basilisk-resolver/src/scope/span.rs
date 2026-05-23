//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Byte-offset span type for source location tracking.

use ruff_text_size::TextRange;

/// A byte-offset span within a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Byte offset of the start (inclusive).
    pub start: u32,
    /// Byte offset of the end (exclusive).
    pub end: u32,
}

impl From<TextRange> for Span {
    fn from(range: TextRange) -> Self {
        Self {
            start: range.start().to_u32(),
            end: range.end().to_u32(),
        }
    }
}

impl Span {
    /// Create a new span from start and end byte offsets.
    #[must_use]
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    /// Does this span fully contain `other`?
    #[must_use]
    pub const fn contains_span(&self, other: Self) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    /// Does this span contain the byte offset?
    #[must_use]
    pub const fn contains_offset(&self, offset: u32) -> bool {
        self.start <= offset && offset < self.end
    }

    /// Slice `source` using this span without `as` conversions.
    ///
    /// Returns `None` if the span is out of bounds.
    #[must_use]
    pub fn slice_source<'a>(&self, source: &'a str) -> Option<&'a str> {
        source.get(self.as_range())
    }

    /// Convert start offset to `usize`.
    ///
    /// Safe because u32 fits in usize on all supported (32-bit+) targets.
    #[must_use]
    #[expect(
        clippy::as_conversions,
        reason = "u32 to usize is safe on 32-bit+ targets"
    )]
    pub const fn start_usize(&self) -> usize {
        self.start as usize
    }

    /// Convert end offset to `usize`.
    ///
    /// Safe because u32 fits in usize on all supported (32-bit+) targets.
    #[must_use]
    #[expect(
        clippy::as_conversions,
        reason = "u32 to usize is safe on 32-bit+ targets"
    )]
    pub const fn end_usize(&self) -> usize {
        self.end as usize
    }

    /// Convert this span to a `Range<usize>` for slicing.
    ///
    /// Safe because u32 fits in usize on all supported (32-bit+) targets.
    #[must_use]
    #[expect(
        clippy::as_conversions,
        reason = "u32 to usize is safe on 32-bit+ targets"
    )]
    pub const fn as_range(&self) -> std::ops::Range<usize> {
        self.start as usize..self.end as usize
    }
}
