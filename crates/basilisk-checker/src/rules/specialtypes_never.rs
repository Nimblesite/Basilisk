//! Implements [specialtypes_never] from [CHKARCH-DIAG-COERCION]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-coercion
//! specialtypes_never: `-> NoReturn` / `-> Never` function can fall through.
//!
//! A function declared with a return type of `NoReturn` or `Never` must
//! unconditionally raise an exception or call another `NoReturn` function on
//! every code path.  If the function can reach the end of its body without
//! raising (e.g. via an `if` without an `else`), the annotation is wrong.
//!
//! ```python
//! import sys
//! from typing import NoReturn
//!
//! def stop() -> NoReturn:         # OK — always raises
//!     raise RuntimeError("no way")
//!
//! def bad(x: int) -> NoReturn:    # E — can fall through when x == 0
//!     if x != 0:
//!         sys.exit(1)
//! ```
//!
//! ## Conservative scope
//!
//! The check is conservative: it only flags a function when **all** of the
//! following hold:
//!
//! 1. The declared return type is exactly `NoReturn` or `Never` (checked by
//!    extracting the annotation text from the span).
//! 2. The function body is not a stub (`...` or `pass`).
//! 3. The last top-level statement is **not** a `raise` statement and is
//!    **not** a standalone call expression (which may itself be `NoReturn`).
//!
//! This avoids false positives for valid patterns such as
//! `raise RuntimeError(...)` or `sys.exit(1)` as the terminating statement.

use basilisk_resolver::{FunctionInfo, ResolvedModule, ReturnAnnotationKind};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "specialtypes_never",
    docs_url: "https://www.basilisk-python.dev/errors/specialtypes_never",
};

/// Emits specialtypes_never when a `-> NoReturn` or `-> Never` function can fall through.
pub(crate) struct NoReturnFallThrough;

impl Rule for NoReturnFallThrough {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        module.functions.iter().for_each(|func| {
            check_function(func, &module.source, &module.path, diagnostics);
        });
    }
}

fn check_function(func: &FunctionInfo, source: &str, path: &str, out: &mut Vec<Diagnostic>) {
    // Only applies to functions with a present return annotation.
    if func.return_annotation == ReturnAnnotationKind::Missing {
        return;
    }
    // Stubs are exempt — `-> NoReturn: ...` is a valid overload signature.
    if func.is_stub_body {
        return;
    }
    // If the last statement terminates (raise or call), the function is fine.
    if func.body_last_stmt_terminates {
        return;
    }
    // Extract the return annotation text and check for NoReturn/Never.
    let Some(ann_span) = func.return_annotation_span else {
        return;
    };
    let Some(ann_text) = slice_span(source, ann_span) else {
        return;
    };
    if !is_noreturn_or_never(ann_text.trim()) {
        return;
    }
    out.push(make_diagnostic(func, path));
}

/// Returns `true` when the annotation text is `NoReturn` or `Never`
/// (possibly with a `typing.` / `typing_extensions.` prefix).
fn is_noreturn_or_never(ann: &str) -> bool {
    matches!(
        ann,
        "NoReturn"
            | "Never"
            | "typing.NoReturn"
            | "typing.Never"
            | "typing_extensions.NoReturn"
            | "typing_extensions.Never"
    )
}

fn make_diagnostic(func: &FunctionInfo, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Function `{}` is declared `-> NoReturn` / `-> Never` but may return implicitly",
            func.name
        ),
        func.name_span,
        path,
        Some("Ensure all code paths raise an exception or call a NoReturn function".to_owned()),
        Some(
            "A `NoReturn`/`Never` function must never return normally — add a `raise` \
             or unconditional call to a NoReturn function on every exit path"
                .to_owned(),
        ),
    )
}
