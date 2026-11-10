//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Protocol Ext visitor functions.

/// Extract the base type name from an annotation string.
pub(super) fn base_type_name(annotation: &str) -> &str {
    annotation
        .find('[')
        .map_or(annotation, |idx| &annotation[..idx])
        .trim()
}

/// Strip string-annotation quotes and dotted module prefixes (issue #36).
pub(super) fn unqualified_base(base: &str) -> &str {
    let trimmed = base
        .trim_matches(|quote: char| quote == '"' || quote == '\'')
        .trim();
    trimmed.rsplit('.').next().unwrap_or(trimmed)
}
