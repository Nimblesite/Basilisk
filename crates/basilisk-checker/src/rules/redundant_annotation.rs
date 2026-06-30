//! Implements [BSK-W0050] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-structural
//! BSK-W0050: Redundant type annotation warning.
//!
//! Emits a warning when a type annotation is redundant because the inferred type
//! exactly matches the declared type. This is Basilisk's headline differentiator
//! from other type checkers.
//!
//! ```python
//! x: int = 42        # BSK-W0050 — annotation is redundant
//! y: str = "hello"   # BSK-W0050 — annotation is redundant
//! z: float = 42      # NO warning — annotation adds information (widening)
//! ```

use crate::inference::infer_rhs;
use crate::types::InferredType;
use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{warning_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-W0050",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-W0050",
};

/// Emits BSK-W0050 for redundant type annotations.
// Implements [TYPEINF-REDUNDANT] — when the written annotation is identical to
// what inference would produce, the annotation is noise and BSK-W0050 fires.
pub(crate) struct RedundantAnnotationWarning;

impl Rule for RedundantAnnotationWarning {
    fn opt_in_spec(&self) -> Option<crate::rule_tags::OptInSpec> {
        Some(crate::rule_tags::OptInSpec {
            code: CODE.code,
            tags: &["redundancy", "style"],
        })
    }

    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Check module-level variables
        module
            .module_vars
            .iter()
            .filter(|var| var.has_annotation)
            .filter_map(|var| {
                let annotation_text = extract_annotation(&module.source, var.name_span)?;

                // Use inference system to get RHS type
                let inferred_type = infer_rhs(&var.rhs_kind);

                // Skip if inference failed
                if matches!(inferred_type, InferredType::Unknown) {
                    return None;
                }

                // Parse annotation text to InferredType using existing parser
                let declared_type = InferredType::from_annotation(annotation_text);

                // Check if annotation is redundant (base type match)
                if types_match_for_w0050(&inferred_type, &declared_type) {
                    Some((var.name_span, var.name.clone(), annotation_text.to_owned()))
                } else {
                    None
                }
            })
            .for_each(|(span, name, annotation)| {
                diagnostics.push(make_diagnostic_for_var(
                    &name,
                    &annotation,
                    span,
                    &module.path,
                ));
            });

        // Check class attributes (skip TypedDict/Protocol/NamedTuple classes)
        module
            .classes
            .iter()
            .filter(|class| {
                // Skip TypedDict, Protocol, and NamedTuple classes (including subclasses).
                !class.bases.iter().any(|base| {
                    base.contains("TypedDict")
                        || base.contains("Protocol")
                        || base.contains("NamedTuple")
                }) && !transitively_inherits(class, &module.classes, &is_namedtuple_base)
                    && !annotation_defines_field(class, &module.classes)
            })
            .flat_map(|class| &class.attributes)
            .filter(|attr| attr.has_annotation && attr.has_value)
            .filter_map(|attr| {
                let annotation_text = extract_annotation(&module.source, attr.name_span)?;

                // Use inference system to get RHS type
                let inferred_type = infer_rhs(&attr.rhs_kind);

                // For class attributes with literal values, we can infer the type from the source
                let inferred_type = if matches!(inferred_type, InferredType::Unknown) {
                    // Try to infer from the source text
                    infer_type_from_source(&module.source, attr.name_span)
                } else {
                    inferred_type
                };

                // Skip if inference still failed
                if matches!(inferred_type, InferredType::Unknown) {
                    return None;
                }

                // Parse annotation text to InferredType using existing parser
                let declared_type = InferredType::from_annotation(annotation_text);

                // Check if annotation is redundant (base type match)
                if types_match_for_w0050(&inferred_type, &declared_type) {
                    Some((
                        attr.name_span,
                        attr.name.clone(),
                        annotation_text.to_owned(),
                    ))
                } else {
                    None
                }
            })
            .for_each(|(span, name, annotation)| {
                diagnostics.push(make_diagnostic_for_var(
                    &name,
                    &annotation,
                    span,
                    &module.path,
                ));
            });
    }
}

/// Extract the annotation text from the source line containing `name_span`.
///
/// Looks for `: <annotation>` on the same source line as the variable name,
/// stopping at the `=` sign that introduces the RHS.  Returns `None` if no
/// such pattern is found.
fn extract_annotation(source: &str, name_span: basilisk_resolver::Span) -> Option<&str> {
    // Find the byte offset of the start of the line containing the name.
    let start = name_span.start_usize();
    let line_start = source[..start].rfind('\n').map_or(0, |pos| pos + 1);
    let line_end = source[start..]
        .find('\n')
        .map_or(source.len(), |pos| start + pos);

    let line = source.get(line_start..line_end)?;

    // Position of the name within the line.
    let name_offset = start.checked_sub(line_start)?;

    // Find `: ` after the name position on this line.
    let colon_pos = line[name_offset..].find(": ")? + name_offset;
    let after_colon = colon_pos + 2; // skip ': '

    // Find `=` that ends the annotation (must be after the colon).
    let annotation_end = line[after_colon..]
        .find('=')
        .map_or(line.len(), |p| after_colon + p);

    let annotation = line.get(after_colon..annotation_end)?.trim();

    if annotation.is_empty() {
        None
    } else {
        Some(annotation)
    }
}

/// Check if types match for BSK-W0050 purposes (base type comparison)
fn types_match_for_w0050(inferred: &InferredType, declared: &InferredType) -> bool {
    use InferredType::{Bool, Bytes, Float, Int, None_, Str};

    // Only fire BSK-W0050 for simple scalar types that are exactly equal
    // Collection types and other complex types should not trigger BSK-W0050
    // because annotations provide useful documentation even when "redundant"
    match (inferred, declared) {
        // Basic scalar types (exact match)
        (Int, Int)
        | (Str, Str)
        | (Float, Float)
        | (Bool, Bool)
        | (Bytes, Bytes)
        | (None_, None_) => true,
        // All other types (collections, unions, optionals, etc.) - no warning
        _ => false,
    }
}

/// Infer a type from the assignment's source text when resolver inference fails.
///
/// Annotated class attributes carry `RhsKind::Other` (the resolver does not
/// classify the RHS of an `AnnAssign`), so BSK-W0050 recovers the literal type from
/// the source line.  When the RHS is *not* a recognisable literal — a name
/// reference, call, or arbitrary expression — Basilisk genuinely cannot infer
/// its type, so we return `Unknown`.  Claiming the annotation is redundant in
/// that case would be a false positive (issue #83): the annotation supplies a
/// type the inference engine does not have.
fn infer_type_from_source(source: &str, name_span: basilisk_resolver::Span) -> InferredType {
    // Extract the line containing the assignment
    let start = name_span.start_usize();
    let line_start = source[..start].rfind('\n').map_or(0, |pos| pos + 1);
    let line_end = source[start..]
        .find('\n')
        .map_or(source.len(), |pos| start + pos);

    let Some(line) = source.get(line_start..line_end) else {
        return InferredType::Unknown;
    };

    // Find the value after the '=' sign
    let Some(equals_pos) = line.find('=') else {
        return InferredType::Unknown;
    };

    let value_text = line[equals_pos + 1..].trim();

    // Simple literal detection
    if value_text.parse::<i64>().is_ok() {
        InferredType::Int
    } else if value_text.parse::<f64>().is_ok() {
        InferredType::Float
    } else if (value_text.starts_with('"') && value_text.ends_with('"'))
        || (value_text.starts_with('\'') && value_text.ends_with('\''))
    {
        InferredType::Str
    } else if value_text == "True" || value_text == "False" {
        InferredType::Bool
    } else if value_text == "None" {
        InferredType::None_
    } else if value_text.starts_with("b\"") && value_text.ends_with('"') {
        InferredType::Bytes
    } else {
        // RHS is not a recognisable literal (name reference, call, expression):
        // Basilisk cannot infer its type, so the annotation is informative — not
        // redundant.  Returning Unknown suppresses a false-positive BSK-W0050 (#83).
        InferredType::Unknown
    }
}

/// Create diagnostic for redundant annotation warning
fn make_diagnostic_for_var(
    name: &str,
    annotation: &str,
    span: basilisk_resolver::Span,
    path: &str,
) -> Diagnostic {
    warning_diagnostic_owned(
        CODE.clone(),
        format!(
            "Redundant type annotation: `{name}` is annotated `{annotation}` but the inferred type is identical"
        ),
        span,
        path,
        Some("Remove the redundant annotation to reduce noise".to_owned()),
        Some(
            "Basilisk warns about redundant annotations to encourage cleaner code".to_owned(),
        ),
    )
}

/// Whether annotated assignments in this class declare *fields* rather than
/// plain attributes (issue #110).
///
/// For `pydantic.BaseModel` subclasses, `@dataclass` classes (including
/// `dataclass_transform` factories), and attrs-style classes, the annotation is
/// what makes the assignment a field — removing it silently deletes the field
/// from validation/serialization or the generated `__init__`. BSK-W0050 must never
/// call such an annotation redundant.
fn annotation_defines_field(
    class: &basilisk_resolver::ClassInfo,
    all_classes: &[basilisk_resolver::ClassInfo],
) -> bool {
    class.is_dataclass
        || has_attrs_class_decorator(class)
        || transitively_inherits(class, all_classes, &is_pydantic_model_base)
}

/// attrs-style class decorators (`@define`, `@frozen`, `@mutable`, `@attr.s`,
/// `@attr.attrs`, …). The resolver records only the final name segment, so
/// `@attr.s` arrives as `"s"` and `@attrs.define` as `"define"`. A stray match
/// merely suppresses a warning — safe — whereas a miss corrupts a model.
fn has_attrs_class_decorator(class: &basilisk_resolver::ClassInfo) -> bool {
    class.decorator_spans.iter().any(|(name, _)| {
        matches!(
            name.as_str(),
            "define" | "frozen" | "mutable" | "attrs" | "s"
        )
    })
}

fn is_namedtuple_base(base: &str) -> bool {
    base == "NamedTuple"
}

fn is_pydantic_model_base(base: &str) -> bool {
    base == "BaseModel" || base.ends_with(".BaseModel")
}

/// Check if a class transitively inherits from a base matching `is_marker_base`,
/// directly or via locally-defined intermediate classes.
///
/// This catches subclasses like `class PointWithName(Point)` where `Point` itself
/// inherits from the marker base.
fn transitively_inherits(
    class: &basilisk_resolver::ClassInfo,
    all_classes: &[basilisk_resolver::ClassInfo],
    is_marker_base: &dyn Fn(&str) -> bool,
) -> bool {
    for base in &class.bases {
        let stripped = base.split('[').next().unwrap_or(base.as_str());
        if is_marker_base(stripped) {
            return true;
        }
        for other in all_classes {
            if other.name == stripped && transitively_inherits(other, all_classes, is_marker_base) {
                return true;
            }
        }
    }
    false
}
