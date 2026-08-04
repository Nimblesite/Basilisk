//! Implements [`returns_compatibility`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! `returns_compatibility`: Return type mismatch.
//!
//! Emitted as an `Error` when the literal value returned by a function is
//! clearly incompatible with the declared return type annotation (e.g.
//! returning an `int` literal from a `-> str` function).
//!
//! ```python
//! # BAD (return type mismatch)
//! def count() -> str:
//!     return 42   # E: int literal is not assignable to str
//!
//! # GOOD
//! def count() -> int:
//!     return 42
//! ```

use crate::annotation::AnnotationResolver;
use crate::inference::{infer_rhs, literal_collection_assignable_to};
use crate::types::InferredType;
use basilisk_resolver::{FunctionInfo, ResolvedModule};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::{
    guards::{is_no_type_check, is_stub_context},
    Rule,
};

const CODE: ErrorCode = ErrorCode {
    code: "returns_compatibility",
    docs_url: "https://www.basilisk-python.dev/errors/returns_compatibility",
};

/// Emits `returns_compatibility` for detectable return type mismatches.
// Implements [TYPEINF-FUNC-RETURN] — the returned value's inferred type must be
// assignable to the declared return type; a mismatch fires `returns_compatibility`.
pub(crate) struct ReturnTypeMismatch;

impl Rule for ReturnTypeMismatch {
    fn check(
        &self,
        module: &ResolvedModule,
        ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Standalone entry point (a single-rule test, or any caller outside the
        // driver): build the cascade the driver would otherwise share.
        let annotations = crate::annotation::AnnotationResolver::for_module(module);
        self.check_with_annotations(module, annotations.as_ref(), ctx, diagnostics);
    }

    fn check_with_annotations(
        &self,
        module: &ResolvedModule,
        annotations: Option<&crate::annotation::AnnotationResolver<'_>>,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // The declared type of every return annotation comes from the shared
        // cascade ([TYPEINF-ANNOTATION-RESOLUTION]); its tables are built once
        // per module, not once per function.
        let Some(resolver) = annotations else {
            return;
        };
        for func in &module.functions {
            // @no_type_check suppresses body checks (E0011); E0041 arity still applies.
            if !is_stub_context(func, &module.classes) && !is_no_type_check(func) {
                check_return_type_mismatch(func, module, resolver, diagnostics);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Return type mismatch
// ---------------------------------------------------------------------------

fn check_return_type_mismatch(
    func: &FunctionInfo,
    module: &ResolvedModule,
    resolver: &AnnotationResolver<'_>,
    out: &mut Vec<Diagnostic>,
) {
    if !func.return_annotation.is_present() {
        return;
    }

    // Generator functions have their own return type validation (E0120).
    // The return annotation (e.g. Generator[Y, S, R]) is not meant to be
    // checked directly against return statement values.
    if func.is_generator {
        return;
    }

    let Some(declared_type) = func
        .return_annotation_span
        .and_then(|span| resolver.resolve_span(span))
    else {
        return;
    };

    // Skip targets this rule cannot verify: a `Literal[...]` target needs the
    // returned expression's *value* (`return True` infers `Bool`, not
    // `Literal[True]`), and a `Protocol` / `TypedDict` target is satisfied
    // structurally, which a kind comparison cannot judge. Names the cascade
    // could not resolve are already the gradual `Unknown` and suppress through
    // ordinary assignability. Shared with E0013 so the two sibling
    // return-mismatch rules stay in lock-step.
    if super::shared::is_value_dependent_target(&declared_type)
        || resolver.is_structural_target(&declared_type)
    {
        return;
    }

    for return_stmt in &func.return_stmts {
        if !return_stmt.has_value {
            continue;
        }

        // Skip call expressions: without full type inference we cannot prove the
        // callee returns an incompatible type
        if return_stmt.value_is_call {
            continue;
        }

        // Use inference system to get RHS type
        let inferred_type = infer_rhs(&return_stmt.rhs_kind);

        // Skip Unknown types - we can't prove they're incompatible
        if matches!(inferred_type, InferredType::Unknown) {
            continue;
        }

        // A returned collection literal is contextually typed against the
        // declared type ([TYPEINF-SPECIAL-LITERAL-CONTEXT]); a stored value
        // keeps invariant subtyping.
        let is_assignable = literal_collection_assignable_to(&return_stmt.rhs_kind, &declared_type)
            .unwrap_or_else(|| inferred_type.is_assignable_to(&declared_type));
        if !is_assignable {
            out.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "return type mismatch: {inferred_type} is not assignable to {declared_type}"
                ),
                func.name_span,
                &module.path,
                Some("Check the return type annotation and return statements".to_owned()),
                None,
            ));
        }
    }
}
