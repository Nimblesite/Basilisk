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
