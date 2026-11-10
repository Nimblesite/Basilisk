//! Implements [`directives_deprecated`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Span helpers for `directives_deprecated`.

use basilisk_resolver::Span;

/// Convert a `ruff_text_size::TextRange` to a [`Span`].
pub(super) fn text_range_to_span(range: ruff_text_size::TextRange) -> Span {
    Span {
        start: range.start().to_u32(),
        end: range.end().to_u32(),
    }
}
