//! Implements [BSK-0050] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STRUCTURAL
//! BSK-0050: Redundant type annotation warning.
//!
//! Emits a warning when a type annotation is redundant because the inferred type
//! exactly matches the declared type. This is Basilisk's headline differentiator
//! from other type checkers.
//!
//! ```python
//! x: int = 42        # BSK-0050 — annotation is redundant
//! y: str = "hello"   # BSK-0050 — annotation is redundant
//! z: float = 42      # NO warning — annotation adds information (widening)
//! ```

use crate::types::InferredType;
use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{warning_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

/// The engine's type for the value at `span`, widened to annotation form —
/// `x: int = 5` reads as `int` against `int`, exactly what "the annotation
/// repeats what inference already knows" means ([TYPEINF-REDUNDANT]).
///
/// Only a value whose type is syntactically self-evident (a literal or a
/// display) can make an annotation REDUNDANT. A call's result type comes
/// from its callee, so annotating it adds information — and BSK-0003 demands
/// exactly that annotation, which BSK-0050 must never contradict.
fn oracle_widened(
    types: &super::shared::module_types::ModuleTypes<'_>,
    span: Option<basilisk_resolver::Span>,
) -> Option<InferredType> {
    use ruff_python_ast::Expr;
    let oracle = types.oracle()?;
    let span = span?;
    if !matches!(
        oracle.expr(span)?,
        Expr::NumberLiteral(_)
            | Expr::StringLiteral(_)
            | Expr::BytesLiteral(_)
            | Expr::BooleanLiteral(_)
            | Expr::NoneLiteral(_)
            | Expr::FString(_)
            | Expr::List(_)
            | Expr::Dict(_)
            | Expr::Set(_)
            | Expr::Tuple(_)
    ) {
        return None;
    }
    let ty = oracle.synth_span(span)?;
    crate::expr_type::is_fully_known(&ty).then(|| crate::expr_type::display_widened(&ty))
}

const CODE: ErrorCode = ErrorCode {
    code: "BSK-0050",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-0050",
};

/// Emits BSK-0050 for redundant type annotations.
// Implements [TYPEINF-REDUNDANT] — when the written annotation is identical to
// what inference would produce, the annotation is noise and BSK-0050 fires.
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
        ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        super::check_with_own_types(self, module, ctx, diagnostics);
    }

    fn check_with_types(
        &self,
        module: &ResolvedModule,
        types: &super::shared::module_types::ModuleTypes<'_>,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // The declared type comes from the shared cascade
        // ([TYPEINF-ANNOTATION-RESOLUTION]): an annotation that is redundant
        // *through an alias* (`type Age = int` then `x: Age = 1`) is redundant
        // all the same, and a name we cannot resolve is gradual, never a guess.
        let Some(resolver) = types.annotations() else {
            return;
        };
        // Check module-level variables
        module
            .module_vars
            .iter()
            .filter(|var| var.has_annotation)
            .filter_map(|var| {
                let annotation_text = extract_annotation(&module.source, var.name_span)?;

                // The value's type comes from the module's shared oracle.
                let inferred_type = oracle_widened(types, var.rhs_span)?;

                let declared_type = var
                    .annotation_span
                    .and_then(|span| resolver.resolve_span(span))
                    .or_else(|| resolver.resolve_text(annotation_text))?;

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

        // Check class attributes
        module
            .classes
            .iter()
            .filter(|class| !annotation_defines_field(class))
            .flat_map(|class| &class.attributes)
            .filter(|attr| attr.has_annotation && attr.has_value)
            .filter_map(|attr| {
                let annotation_text = extract_annotation(&module.source, attr.name_span)?;

                // The value's type comes from the module's shared oracle; a
                // class-body literal the oracle has no span for falls back to
                // the source-window inference until the resolver records
                // attribute value spans.
                let inferred_type = oracle_widened(types, attr.rhs_span)
                    .unwrap_or_else(|| infer_type_from_source(&module.source, attr.name_span));

                // Skip if inference still failed
                if matches!(inferred_type, InferredType::Unknown) {
                    return None;
                }

                let declared_type = attr
                    .annotation_span
                    .and_then(|span| resolver.resolve_span(span))
                    .or_else(|| resolver.resolve_text(annotation_text))?;

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

/// Check if types match for BSK-0050 purposes (base scalar comparison).
fn types_match_for_w0050(inferred: &InferredType, declared: &InferredType) -> bool {
    use InferredType::{Bool, Bytes, Float, Int, LiteralString, None_, Str};

    // Only fire BSK-0050 for simple scalar types that are exactly equal. A
    // string literal infers as `LiteralString`, which an explicit `str`
    // annotation genuinely restates. Collection and other complex types are
    // never flagged: the annotation documents element/key/value/tuple shape the
    // literal alone does not make obvious, so it stays useful even when the
    // engine could infer it (see test_w0050_{list,dict,set,tuple}_literal_no_warning).
    matches!(
        (inferred, declared),
        (Int, Int)
            | (Str | LiteralString, Str)
            | (Float, Float)
            | (Bool, Bool)
            | (Bytes, Bytes)
            | (None_, None_)
    )
}

/// Infer a type from the assignment's source text when resolver inference fails.
///
/// Annotated class attributes carry `RhsKind::Other` (the resolver does not
/// classify the RHS of an `AnnAssign`), so BSK-0050 recovers the literal type from
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
        // redundant.  Returning Unknown suppresses a false-positive BSK-0050 (#83).
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
/// In a `@dataclass` (including `dataclass_transform` factories) the annotation
/// is what makes the assignment a field — removing it silently deletes the
/// field from the generated `__init__`. BSK-0050 must never call such an
/// annotation redundant.
fn annotation_defines_field(class: &basilisk_resolver::ClassInfo) -> bool {
    class.is_dataclass
}
