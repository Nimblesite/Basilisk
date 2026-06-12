//! Implements [BSK-E0013] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-typesafety
//! BSK-E0013: Return type mismatch — inferred return type incompatible with annotation.
//!
//! When a function has a return type annotation, the inferred return type must be
//! assignable to the declared type. This extends the original `-> None` check to
//! handle all return type mismatches using the inference system.

use crate::inference::infer_rhs;
use crate::span_util::slice_span;
use crate::types::InferredType;
use basilisk_resolver::{FunctionInfo, ResolvedModule, ReturnStmtInfo};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0013",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0013",
};

/// Emits BSK-E0013 for return type mismatches using inference system.
pub(crate) struct ReturnTypeMismatch;

impl Rule for ReturnTypeMismatch {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        module
            .functions
            .iter()
            .filter(|func| func.return_annotation.is_present())
            .for_each(|func| check_function(func, module, diagnostics));
    }
}

/// Returns true when E0013 cannot reliably verify a return against `ty` — at the
/// top level or nested inside a union, container, optional, callable, or type-form.
///
/// Two kinds defeat E0013's value-less return inference:
/// - `Named`: protocols/classes/aliases (and quote-mangled forward references)
///   need class-hierarchy/structural analysis E0013 cannot perform.
/// - `Literal`: verifying a `Literal[v]` target requires the *value* of the
///   returned expression, but `infer_rhs` only knows the kind (`return True`
///   infers `Bool`, not `Literal[True]`). Any `Literal`-target check is therefore
///   unreliable, so it is skipped.
///
/// See `check_function` for the rationale.
fn is_unverifiable_return_type(ty: &InferredType) -> bool {
    match ty {
        InferredType::Named(_) | InferredType::Literal(_) => true,
        InferredType::Optional(inner)
        | InferredType::List(inner)
        | InferredType::Set(inner)
        | InferredType::TypeForm(inner) => is_unverifiable_return_type(inner),
        InferredType::Dict(key, value) => {
            is_unverifiable_return_type(key) || is_unverifiable_return_type(value)
        }
        InferredType::Union(types) => types.iter().any(is_unverifiable_return_type),
        // The variable-length form `tuple[X, ...]` parses the `...` terminator to
        // `Named("...")`; that is a structural marker handled by `is_assignable_to`,
        // not an unresolvable type, so it must not trigger the skip.
        InferredType::Tuple(types) => types.iter().any(|elem| {
            !matches!(elem, InferredType::Named(name) if name == "...")
                && is_unverifiable_return_type(elem)
        }),
        InferredType::Callable(info) => {
            is_unverifiable_return_type(&info.return_type)
                || info.param_types.iter().any(is_unverifiable_return_type)
        }
        _ => false,
    }
}

fn check_function(func: &FunctionInfo, module: &ResolvedModule, out: &mut Vec<Diagnostic>) {
    // Generator functions have their own return type validation (E0120).
    // Return values in generators go through Generator[Y, S, R]'s ReturnType,
    // not the top-level annotation.
    if func.is_generator {
        return;
    }

    let Some(ann_span) = func.return_annotation_span else {
        return;
    };
    let Some(ann_text) = slice_span(&module.source, ann_span) else {
        return;
    };

    // Parse annotation text to InferredType
    let declared_type = InferredType::from_annotation(ann_text);

    // Named types (e.g. `Sequence[float]`, `Iterable[int]`, forward references
    // like `"Class | Any"`) require structural subtyping or generic variance
    // analysis that E0013 cannot perform.  A concrete return like `list[int]`
    // IS assignable to `Sequence[float]` at runtime, but `InferredType` has no
    // knowledge of the class hierarchy.  Skip the check to avoid FPs.
    //
    // This applies recursively: a union or container that *contains* a Named or
    // Literal type is equally unverifiable. Quoted forward references such as
    // `"int | Meta2"` parse (after `|`-splitting) into a union of `Named`
    // fragments with no concrete member, so a valid `return 1` would otherwise
    // be flagged. Empty-tuple forms like `tuple[()]` parse the `()` to
    // `Named("()")`. `Literal[...]` targets need the returned value, which the
    // kind-only return inference does not have. All are skipped.
    if is_unverifiable_return_type(&declared_type) {
        return;
    }

    // Special case for -> None functions: any valued return should be flagged
    if declared_type == InferredType::None_ {
        func.return_stmts
            .iter()
            .filter(|stmt| stmt.has_value)
            // Skip call expressions: without full type inference we cannot prove the
            // callee returns non-None (e.g. `return f(self)` where f: Callable[..., None]
            // is valid in a -> None function).
            .filter(|stmt| !stmt.value_is_call)
            .for_each(|stmt| {
                out.push(make_none_diagnostic(stmt, &func.name, &module.path));
            });
        return;
    }

    func.return_stmts
        .iter()
        .filter(|stmt| stmt.has_value)
        // Skip call expressions: without full type inference we cannot prove the
        // callee returns non-None (e.g. `return f(self)` where f: Callable[..., None]
        // is valid in a -> None function).
        .filter(|stmt| !stmt.value_is_call)
        .for_each(|stmt| {
            // Use inference system to get RHS type
            let inferred_type = infer_rhs(&stmt.rhs_kind);

            // Skip Unknown types - we can't prove they're incompatible
            if matches!(inferred_type, InferredType::Unknown) {
                return;
            }

            // Check assignability using inference system
            if !inferred_type.is_assignable_to(&declared_type) {
                out.push(make_diagnostic(
                    stmt,
                    &func.name,
                    &inferred_type,
                    &declared_type,
                    &module.path,
                ));
            }
        });
}

fn make_none_diagnostic(stmt: &ReturnStmtInfo, func_name: &str, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Function `{func_name}` is annotated `-> None` but has a `return` statement with a value"
        ),
        stmt.span,
        path,
        Some(
            "Either remove the return value or change the return type annotation".to_owned(),
        ),
        Some(
            "A function annotated `-> None` must only use bare `return` or fall off the end"
                .to_owned(),
        ),
    )
}

fn make_diagnostic(
    stmt: &ReturnStmtInfo,
    func_name: &str,
    inferred_type: &InferredType,
    declared_type: &InferredType,
    path: &str,
) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Function `{func_name}` return type mismatch: {inferred_type} is not assignable to {declared_type}"
        ),
        stmt.span,
        path,
        Some(
            "Either change the return value or update the return type annotation".to_owned(),
        ),
        Some(
            "Basilisk requires the inferred return type to be assignable to the declared type".to_owned(),
        ),
    )
}
