//! BSK-E0011: Explicit `Any` annotation / return type mismatch.
//!
//! Two categories of diagnostic share this code:
//!
//! 1. **Explicit `Any`** — emitted as a `Warning` when a function parameter or
//!    return annotation is written as `Any` (from `typing`).  `Any` silences all
//!    type-checking for the annotated value and should be used only when
//!    intentional, with a comment explaining why.
//!
//! 2. **Return type mismatch** — emitted as an `Error` when the literal value
//!    returned by a function is clearly incompatible with the declared return
//!    type annotation (e.g. returning an `int` literal from a `-> str` function).
//!
//! ```python
//! from typing import Any
//!
//! # BAD (explicit Any)
//! def greet(name: Any) -> str: ...  # W: parameter `name` is annotated Any
//!
//! # BAD (return type mismatch)
//! def count() -> str:
//!     return 42   # E: int literal is not assignable to str
//!
//! # GOOD
//! def greet(name: str) -> str: ...
//! def count() -> int:
//!     return 42
//! ```

use crate::inference::infer_rhs;
use crate::span_util::slice_span;
use crate::types::InferredType;
use basilisk_resolver::{FunctionInfo, ParameterInfo, ResolvedModule, ReturnAnnotationKind};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::{guards::is_stub_context, Rule};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0011",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0011",
};

/// Emits BSK-E0011 for explicit `Any` annotations and for detectable return
/// type mismatches.
pub(crate) struct ReturnTypeMismatch;

impl Rule for ReturnTypeMismatch {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for func in &module.functions {
            check_explicit_any(func, &module.path, diagnostics);

            if !is_stub_context(func, &module.classes) {
                check_return_type_mismatch(func, module, diagnostics);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Explicit Any
// ---------------------------------------------------------------------------

fn check_explicit_any(func: &FunctionInfo, path: &str, out: &mut Vec<Diagnostic>) {
    if func.return_annotation == ReturnAnnotationKind::Any {
        out.push(make_return_any_diagnostic(func, path));
    }

    for param in &func.parameters {
        if param.annotation_is_any {
            out.push(make_param_any_diagnostic(param, path));
        }
    }

    if let Some(vararg) = &func.vararg {
        if vararg.annotation_is_any {
            out.push(make_param_any_diagnostic(vararg, path));
        }
    }

    if let Some(kwarg) = &func.kwarg {
        if kwarg.annotation_is_any {
            out.push(make_param_any_diagnostic(kwarg, path));
        }
    }
}

fn make_return_any_diagnostic(func: &FunctionInfo, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Warning,
        message: format!(
            "Function `{}` has `Any` as its return annotation — prefer a concrete type",
            func.name
        ),
        span: func.name_span,
        path: path.to_owned(),
        help: Some("Replace `Any` with the actual return type of this function".to_owned()),
        note: Some(
            "`Any` disables type checking for this return value; use only when unavoidable"
                .to_owned(),
        ),
    }
}

fn make_param_any_diagnostic(param: &ParameterInfo, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Warning,
        message: format!(
            "Parameter `{}` is annotated `Any` — prefer a concrete type",
            param.name
        ),
        span: param.name_span,
        path: path.to_owned(),
        help: Some(format!(
            "Replace `Any` on `{}` with the actual expected type",
            param.name
        )),
        note: Some(
            "`Any` disables type checking for this parameter; use only when unavoidable".to_owned(),
        ),
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
            out.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "return type mismatch: {inferred_type} is not assignable to {declared_type}"
                ),
                span: func.name_span,
                path: module.path.clone(),
                help: Some("Check the return type annotation and return statements".to_owned()),
                note: None,
            });
        }
    }
}
