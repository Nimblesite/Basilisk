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

use basilisk_resolver::{
    FunctionInfo, ParameterInfo, ResolvedModule, ReturnAnnotationKind, RhsKind,
};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::{guards::is_stub_context, Rule};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0011",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0011",
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
            "`Any` disables type checking for this parameter; use only when unavoidable"
                .to_owned(),
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

    for return_stmt in &func.return_stmts {
        if !return_stmt.has_value {
            continue;
        }

        let Some(ann_span) = func.return_annotation_span else {
            continue;
        };
        let Some(ann_text) = module
            .source
            .get(ann_span.start as usize..ann_span.end as usize)
        else {
            continue;
        };

        if is_incompatible_rhs_kind(&return_stmt.rhs_kind, ann_text) {
            out.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "return type mismatch: {} is not assignable to {}",
                    rhs_kind_type_name(&return_stmt.rhs_kind),
                    ann_text
                ),
                span: func.name_span,
                path: module.path.clone(),
                help: Some(
                    "Check the return type annotation and return statements".to_owned(),
                ),
                note: None,
            });
        }
    }
}

fn is_incompatible_rhs_kind(rhs_kind: &RhsKind, annotation: &str) -> bool {
    let base = annotation
        .split('[')
        .next()
        .unwrap_or(annotation)
        .trim()
        .to_ascii_lowercase();

    match rhs_kind {
        RhsKind::IntLiteral => base != "int" && base != "any",
        RhsKind::StrLiteral => base != "str" && base != "any",
        RhsKind::FloatLiteral => base != "float" && base != "any",
        RhsKind::BoolLiteral => base != "bool" && base != "any",
        RhsKind::BytesLiteral => base != "bytes" && base != "any",
        RhsKind::NoneValue => base != "none" && base != "any",
        _ => false,
    }
}

fn rhs_kind_type_name(rhs_kind: &RhsKind) -> &'static str {
    match rhs_kind {
        RhsKind::IntLiteral => "int",
        RhsKind::StrLiteral => "str",
        RhsKind::FloatLiteral => "float",
        RhsKind::BoolLiteral => "bool",
        RhsKind::BytesLiteral => "bytes",
        RhsKind::NoneValue => "None",
        _ => "unknown",
    }
}
