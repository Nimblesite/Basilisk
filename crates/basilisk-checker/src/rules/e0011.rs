//! BSK-E0011: Explicit `Any` annotation without justification (Warning).
//!
//! Using `Any` defeats Basilisk's type safety guarantees.  Every use of `Any`
//! must be intentional and documented with a `# basilisk: allow[BSK-E0011]`
//! comment in the source.  This rule fires as a **Warning** so that it does
//! not block compilation but remains visible in reports.

use basilisk_resolver::{FunctionInfo, ParameterInfo, ResolvedModule, ReturnAnnotationKind};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0011",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0011",
};

const HELP: &str =
    "Replace `Any` with a specific type, or add `# basilisk: allow[BSK-E0011] -- reason`";

const NOTE: &str =
    "`Any` disables type checking for this value; use a union type or generic instead";

/// Emits BSK-E0011 (Warning) for every `Any`-annotated parameter and every
/// function with an `Any` return annotation.
pub(crate) struct ImplicitAny;

impl Rule for ImplicitAny {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        module
            .functions
            .iter()
            .for_each(|func| check_function(func, &module.path, diagnostics));
    }
}

fn check_function(func: &FunctionInfo, path: &str, out: &mut Vec<Diagnostic>) {
    func.parameters
        .iter()
        .filter(|p| p.annotation_is_any)
        .for_each(|p| out.push(make_param_diagnostic(p, path)));

    if let Some(vararg) = &func.vararg {
        if vararg.annotation_is_any {
            out.push(make_param_diagnostic(vararg, path));
        }
    }

    if let Some(kwarg) = &func.kwarg {
        if kwarg.annotation_is_any {
            out.push(make_param_diagnostic(kwarg, path));
        }
    }

    if func.return_annotation == ReturnAnnotationKind::Any {
        out.push(make_return_diagnostic(func, path));
    }
}

fn make_param_diagnostic(param: &ParameterInfo, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Warning,
        message: format!(
            "Explicit `Any` annotation on `{}` — add a justification comment",
            param.name
        ),
        span: param.name_span,
        path: path.to_owned(),
        help: Some(HELP.to_owned()),
        note: Some(NOTE.to_owned()),
    }
}

fn make_return_diagnostic(func: &FunctionInfo, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Warning,
        message: format!(
            "Explicit `Any` return annotation on `{}` — add a justification comment",
            func.name
        ),
        span: func.name_span,
        path: path.to_owned(),
        help: Some(HELP.to_owned()),
        note: Some(NOTE.to_owned()),
    }
}
