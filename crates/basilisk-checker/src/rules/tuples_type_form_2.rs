//! Implements [`tuples_type_form_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `tuples_type_form_2`: Invalid tuple type syntax.
//!
//! Validates tuple type annotations according to the typing spec's tuple form
//! (<https://typing.python.org/en/latest/spec/tuples.html>):
//!
//! - `tuple[T, ...]` must have exactly one type before `...`
//! - `tuple[...]` is invalid (must specify a type)
//! - `tuple[T, ..., U]` is invalid (`...` can only appear at the end)
//! - `tuple[T, U, ...]` is invalid (can't have multiple fixed types before `...`)
//!
//! ```python
//! t1: tuple[int, ...]        # OK
//! t2: tuple[int, int, ...]   # E — multiple fixed types before ...
//! t3: tuple[...]             # E — missing type before ...
//! t4: tuple[..., int]         # E — ... must be at the end
//! t5: tuple[int, ..., int]    # E — ... must be at the end
//! ```
//!
//! The annotation is recognised by resolving its base through the module's
//! binding table ([ASTREBUILD-LAW]) — `tuple`, `typing.Tuple`, and any alias
//! of either behave identically — and the elements are the subscript slice's
//! AST nodes, never a comma-split of the source text.

use basilisk_resolver::{ResolvedModule, Span, TypingForm};
use ruff_python_ast::Expr;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::{parse_module, ExprIndex};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "tuples_type_form_2",
    docs_url: "https://www.basilisk-python.dev/errors/tuples_type_form_2",
};

/// Emits `tuples_type_form_2` for invalid tuple type syntax.
pub(crate) struct InvalidTupleTypeSyntax;

impl Rule for InvalidTupleTypeSyntax {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(parsed) = parse_module(module) else {
            return;
        };
        let index = ExprIndex::build(&parsed.ast);

        let annotation_spans = module
            .module_vars
            .iter()
            .filter(|var| var.has_annotation)
            .filter_map(|var| var.annotation_span)
            .chain(
                module
                    .functions
                    .iter()
                    .filter_map(|func| func.return_annotation_span),
            );

        for ann_span in annotation_spans {
            let Some(ann_expr) = index.expr(ann_span) else {
                continue;
            };
            if let Some(error_msg) = tuple_ellipsis_violation(module, ann_expr) {
                diagnostics.push(make_diagnostic(error_msg, ann_span, &module.path));
            }
        }
    }
}

/// Build the rule's diagnostic for one invalid tuple annotation.
fn make_diagnostic(error_msg: &str, span: Span, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!("Invalid tuple type syntax: {error_msg}"),
        span,
        path,
        Some("Use valid tuple type syntax according to the typing spec".to_owned()),
        Some(
            "Tuple types must follow the pattern `tuple[T, ...]` with exactly one type before the ellipsis"
                .to_owned(),
        ),
    )
}

/// Returns `Some(error_message)` when the annotation is a `tuple[...]` form
/// whose top-level ellipsis placement is invalid.
///
/// Only top-level ellipsis misuse is flagged; a `...` nested inside a starred
/// unpack like `*tuple[str, ...]` is valid and is a distinct expression node,
/// so it never appears among the outer slice's elements.
fn tuple_ellipsis_violation(module: &ResolvedModule, expr: &Expr) -> Option<&'static str> {
    let Expr::Subscript(sub) = expr else {
        return None;
    };
    let form = module.bindings.form_of_with_builtins(&sub.value)?;
    if !matches!(form, TypingForm::TupleClass | TypingForm::TupleAlias) {
        return None;
    }

    // The subscript's elements: a tuple slice's elements, or the single
    // expression itself. `tuple[()]` yields an empty element list — the
    // valid empty-tuple form.
    let elements: Vec<&Expr> = match sub.slice.as_ref() {
        Expr::Tuple(tuple) => tuple.elts.iter().collect(),
        single => vec![single],
    };

    let ellipsis_positions: Vec<usize> = elements
        .iter()
        .enumerate()
        .filter(|(_, element)| matches!(element, Expr::EllipsisLiteral(_)))
        .map(|(position, _)| position)
        .collect();

    if ellipsis_positions.is_empty() {
        // No top-level `...` — valid fixed tuple.
        return None;
    }

    // More than one top-level `...` is always invalid.
    if ellipsis_positions.len() > 1 {
        return Some("ellipsis (...) must appear at the end of the tuple type");
    }

    let &ellipsis_pos = ellipsis_positions.first()?;

    // `...` must be the very last element.
    if ellipsis_pos != elements.len() - 1 {
        return Some("ellipsis (...) must appear at the end of the tuple type");
    }

    // Count elements before `...`.
    match ellipsis_pos {
        0 => Some("tuple[...] is invalid — must specify a type before the ellipsis"),
        1 => None,
        _ => Some("tuple[T, U, ...] is invalid — can only have one type before the ellipsis"),
    }
}
