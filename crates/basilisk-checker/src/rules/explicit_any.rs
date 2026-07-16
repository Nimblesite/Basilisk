//! Implements [BSK-0014] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-typesafety
//! BSK-0014: Explicit `Any` annotation.
//!
//! Emitted as a `Warning` when a function parameter or return annotation is
//! written as `Any` (from `typing`). `Any` silences all type checking for the
//! annotated value and should be used only when intentional.
//!
//! This is an opinionated *strictness nudge*, not a type-system requirement:
//! the typing spec treats `Any` as a fully valid type. It is therefore a
//! distinct (user-suppressible) code from the genuine return-type-mismatch
//! error ([`returns_compatibility`]); the two used to share a code, so a user could not
//! silence the style nudge while keeping the real type check. BSK-0014 itself is
//! never disabled for PEP conformance — like every rule it runs fully enabled
//! during scoring; there is no "spec-conformance mode" that turns it off. See
//! [CHKARCH-CONFORMANCE-MODE].
//!
//! ```python
//! from typing import Any
//!
//! def greet(name: Any) -> str: ...  # BSK-0014 — parameter `name` is annotated Any
//! def parse(text: str) -> Any: ...  # BSK-0014 — return annotation is Any
//!
//! def greet(name: str) -> str: ...  # NO warning — concrete types
//! ```

use basilisk_resolver::{FunctionInfo, ParameterInfo, ResolvedModule, ReturnAnnotationKind};

use crate::diagnostic::{warning_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-0014",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-0014",
};

/// Emits BSK-0014 for explicit `Any` annotations on parameters and returns.
pub(crate) struct ExplicitAny;

impl Rule for ExplicitAny {
    fn opt_in_spec(&self) -> Option<crate::rule_tags::OptInSpec> {
        Some(crate::rule_tags::OptInSpec {
            code: CODE.code,
            tags: &["style", "strictness"],
        })
    }

    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for func in &module.functions {
            check_explicit_any(func, &module.path, diagnostics);
        }
    }
}

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
    warning_diagnostic_owned(
        CODE.clone(),
        format!(
            "Function `{}` has `Any` as its return annotation — prefer a concrete type",
            func.name
        ),
        func.name_span,
        path,
        Some("Replace `Any` with the actual return type of this function".to_owned()),
        Some(
            "`Any` disables type checking for this return value; use only when unavoidable"
                .to_owned(),
        ),
    )
}

fn make_param_any_diagnostic(param: &ParameterInfo, path: &str) -> Diagnostic {
    warning_diagnostic_owned(
        CODE.clone(),
        format!(
            "Parameter `{}` is annotated `Any` — prefer a concrete type",
            param.name
        ),
        param.name_span,
        path,
        Some(format!(
            "Replace `Any` on `{}` with the actual expected type",
            param.name
        )),
        Some(
            "`Any` disables type checking for this parameter; use only when unavoidable".to_owned(),
        ),
    )
}
