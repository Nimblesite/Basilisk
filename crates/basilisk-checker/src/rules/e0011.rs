//! Implements [BSK-E0011] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-typesafety
//! BSK-E0011: Return type mismatch.
//!
//! Emitted as an `Error` when the literal value returned by a function is
//! clearly incompatible with the declared return type annotation (e.g.
//! returning an `int` literal from a `-> str` function).
//!
//! The explicit-`Any` *warning* used to share this code; it now lives under its
//! own warning code ([BSK-W0014]) so the opinionated style nudge can be
//! silenced independently of this genuine type-safety error.
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

use crate::inference::infer_rhs;
use crate::span_util::slice_span;
use crate::types::InferredType;
use basilisk_resolver::{FunctionInfo, ResolvedModule};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::{guards::is_stub_context, Rule};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0011",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0011",
};

/// Emits BSK-E0011 for detectable return type mismatches.
pub(crate) struct ReturnTypeMismatch;

impl Rule for ReturnTypeMismatch {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for func in &module.functions {
            if !is_stub_context(func, &module.classes) {
                check_return_type_mismatch(func, module, diagnostics);
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

    for return_stmt in &func.return_stmts {
        if !return_stmt.has_value {
            continue;
        }

        // Skip call expressions: without full type inference we cannot prove the
        // callee returns an incompatible type
        if return_stmt.value_is_call {
            continue;
        }

        let Some(ann_span) = func.return_annotation_span else {
            continue;
        };
        let Some(ann_text) = slice_span(&module.source, ann_span) else {
            continue;
        };

        // Use inference system to get RHS type
        let inferred_type = infer_rhs(&return_stmt.rhs_kind);

        // Skip Unknown types - we can't prove they're incompatible
        if matches!(inferred_type, InferredType::Unknown) {
            continue;
        }

        // Parse annotation text to InferredType
        let declared_type = InferredType::from_annotation(ann_text);

        // Check assignability using inference system
        if !inferred_type.is_assignable_to(&declared_type) {
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
