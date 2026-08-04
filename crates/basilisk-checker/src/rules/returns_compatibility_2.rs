//! Implements [`returns_compatibility_2`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! `returns_compatibility_2`: Return type mismatch — inferred return type incompatible with annotation.
//!
//! When a function has a return type annotation, the inferred return type must be
//! assignable to the declared type. This extends the original `-> None` check to
//! handle all return type mismatches using the inference system.

use crate::annotation::AnnotationResolver;
use crate::inference::{infer_rhs, literal_collection_assignable_to};
use crate::types::InferredType;
use basilisk_resolver::{FunctionInfo, ResolvedModule, ReturnStmtInfo};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "returns_compatibility_2",
    docs_url: "https://www.basilisk-python.dev/errors/returns_compatibility_2",
};

/// Emits `returns_compatibility_2` for return type mismatches using inference system.
pub(crate) struct ReturnTypeMismatch;

impl Rule for ReturnTypeMismatch {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // One cascade per module ([TYPEINF-ANNOTATION-RESOLUTION]), shared by
        // every function's return annotation.
        let Some(resolver) = AnnotationResolver::for_module(module) else {
            return;
        };
        module
            .functions
            .iter()
            .filter(|func| func.return_annotation.is_present())
            .for_each(|func| check_function(func, module, &resolver, diagnostics));
    }
}

fn check_function(
    func: &FunctionInfo,
    module: &ResolvedModule,
    resolver: &AnnotationResolver<'_>,
    out: &mut Vec<Diagnostic>,
) {
    // Generator functions have their own return type validation (E0120).
    // Return values in generators go through Generator[Y, S, R]'s ReturnType,
    // not the top-level annotation.
    if func.is_generator {
        return;
    }

    let Some(declared_type) = func
        .return_annotation_span
        .and_then(|span| resolver.resolve_span(span))
    else {
        return;
    };

    // A `Literal[...]` target needs the returned expression's *value*, which
    // the kind-only return inference does not have (`return True` infers
    // `Bool`, not `Literal[True]`), so it is skipped — recursively, since a
    // union or container containing one is equally value-dependent. Names the
    // cascade could not resolve are already the gradual `Unknown` and suppress
    // through ordinary assignability ([TYPEINF-ANNOTATION-RESOLUTION]).
    if super::shared::is_value_dependent_target(&declared_type) {
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

            // A returned collection literal is contextually typed against the
            // declared type ([TYPEINF-SPECIAL-LITERAL-CONTEXT]); a stored value
            // keeps invariant subtyping.
            let is_assignable = literal_collection_assignable_to(&stmt.rhs_kind, &declared_type)
                .unwrap_or_else(|| inferred_type.is_assignable_to(&declared_type));
            if !is_assignable {
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
