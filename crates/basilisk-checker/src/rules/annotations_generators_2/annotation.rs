//! Implements [`annotations_generators_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Annotation decomposition for `annotations_generators_2`.
//!
//! `Generator`, `Iterator`, and `Iterable` all require an import
//! (`typing` or `collections.abc`), so recognising them by matching an
//! annotation's source text against those spellings answers the wrong
//! question: it accepts a local class named `Iterator` and rejects
//! `from collections.abc import Iterator as Iter`. That recognition has been
//! deleted. It must be rebuilt on the annotation cascade
//! ([TYPEINF-ANNOTATION-RESOLUTION]), which compares against the module's
//! resolved imports rather than against the characters on the line.

/// Parsed generator return annotation.
#[expect(
    clippy::struct_field_names,
    reason = "field names intentionally mirror the type parameter names"
)]
pub(super) struct GeneratorAnnotation {
    /// The yield type (first type parameter).
    pub(super) yield_type: String,
    /// The send type (second type parameter), if present.
    pub(super) send_type: Option<String>,
    /// The return type (third type parameter), if present.
    pub(super) return_type: Option<String>,
}

/// Decompose a return annotation that denotes a generator-like type.
///
/// Always `None` until the generator forms are recognised through the
/// annotation cascade; the spelling-based recogniser this replaced is deleted.
pub(super) fn parse_generator_annotation(_ann: &str) -> Option<GeneratorAnnotation> {
    None
}
