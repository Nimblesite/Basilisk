//! Implements [`returns_compatibility_2`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! `returns_compatibility_2`: Return type mismatch — inferred return type incompatible with annotation.
//!
//! When a function has a return type annotation, the inferred return type must be
//! assignable to the declared type. This extends the original `-> None` check to
//! handle all return type mismatches using the inference system.

use crate::annotation::AnnotationResolver;
use crate::rules::shared::judge::TypeJudge;
use crate::rules::shared::module_types::ModuleTypes;
use crate::rules::shared::returns_judge::{judge_return, none_return_fires, ReturnVerdict};
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
        ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        super::check_with_own_types(self, module, ctx, diagnostics);
    }

    fn check_with_types(
        &self,
        module: &ResolvedModule,
        types: &ModuleTypes<'_>,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // One cascade and one oracle per module
        // ([TYPEINF-ANNOTATION-RESOLUTION], [NARROWPLAN-INTEGRATION]), shared by
        // every function's return annotation and returned expression.
        let Some(resolver) = types.annotations() else {
            return;
        };
        let judge = TypeJudge::new(types.oracle(), resolver, types.subtyping());
        module
            .functions
            .iter()
            .filter(|func| func.return_annotation.is_present())
            .for_each(|func| check_function(func, module, resolver, &judge, diagnostics));
    }
}

fn check_function(
    func: &FunctionInfo,
    module: &ResolvedModule,
    resolver: &AnnotationResolver<'_>,
    judge: &TypeJudge<'_, '_>,
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
    // `Bool`, not `Literal[True]`), and a `Protocol` / `TypedDict` target is
    // satisfied structurally rather than by kind — both are skipped,
    // recursively, since a union or container containing one is equally
    // unjudgeable here. Names the cascade could not resolve are already the
    // gradual `Unknown` and suppress through ordinary assignability
    // ([TYPEINF-ANNOTATION-RESOLUTION]).
    if super::shared::is_value_dependent_target(&declared_type)
        || resolver.is_structural_target(&declared_type)
    {
        return;
    }

    // A `-> None` function must only use a bare `return`. The engine can now
    // disprove the shape-level firing for a value it types `None` — including
    // `return f(self)` where `f` declares `-> None`, which the pre-engine rule
    // could only skip wholesale ([NARROWPLAN-INTEGRATION] Step 2).
    if declared_type == InferredType::None_ {
        func.return_stmts
            .iter()
            .filter(|stmt| stmt.has_value && none_return_fires(judge, stmt))
            .for_each(|stmt| {
                out.push(make_none_diagnostic(stmt, &func.name, &module.path));
            });
        return;
    }

    func.return_stmts
        .iter()
        .filter(|stmt| stmt.has_value)
        .for_each(|stmt| {
            if let ReturnVerdict::Mismatch(inferred_type) = judge_return(judge, stmt, &declared_type)
            {
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
