//! Byte-offset span type for source location tracking.

/// A byte-offset span within a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Byte offset of the start (inclusive).
    pub start: u32,
    /// Byte offset of the end (exclusive).
    pub end: u32,
}

impl Span {
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
