//! Implements [CHKARCH-DIAG-PHILOSOPHY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-PHILOSOPHY
//! Safe source-text slicing helpers for [`basilisk_resolver::Span`].

/// Extract the source text covered by a [`basilisk_resolver::Span`].
///
/// Returns `None` if the byte range falls outside `source`.
#[must_use]
pub fn slice_span(source: &str, span: basilisk_resolver::Span) -> Option<&str> {
    let start: usize = span.start.try_into().ok()?;
    let end: usize = span.end.try_into().ok()?;
    source.get(start..end)
}

/// Convert a `ruff_text_size::TextRange` to a [`basilisk_resolver::Span`].
#[must_use]
pub fn text_range_to_span(range: ruff_text_size::TextRange) -> basilisk_resolver::Span {
    basilisk_resolver::Span {
        start: range.start().to_u32(),
        end: range.end().to_u32(),
    }
}

/// The source text of an AST node, for diagnostic MESSAGES only — never for a
/// verdict ([ASTREBUILD-LAW]).
#[must_use]
pub fn node_message_text<'a>(source: &'a str, node: &impl ruff_text_size::Ranged) -> &'a str {
    slice_span(source, text_range_to_span(node.range()))
        .unwrap_or("<expression>")
        .trim()
}

/// The diagnostic span of an AST node.
#[must_use]
pub fn node_span(node: &impl ruff_text_size::Ranged) -> basilisk_resolver::Span {
    text_range_to_span(node.range())
}
