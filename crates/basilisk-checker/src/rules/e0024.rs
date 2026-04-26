//! BSK-E0024: Invalid type form — numeric literal used as type annotation.
//!
//! Type annotations must be type expressions, not literal values.  Using a
//! number such as `42`, `3.14`, or `True` as a type annotation is always a
//! mistake (it is valid Python syntax but meaningless as a type).
//!
//! ```python
//! def f(x: 42) -> 0:   # both parameter and return annotation are literals
//!     ...
//! ```

use basilisk_resolver::{FunctionInfo, ParameterInfo, ResolvedModule, ReturnAnnotationKind};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0024",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0024",
};

const HELP: &str = "Use a type name like `int`, `str`, `float` instead of a literal value";

/// Emits BSK-E0024 for function parameters and return annotations that are
/// numeric or boolean literals.
pub(crate) struct InvalidTypeForm;

impl Rule for InvalidTypeForm {
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
        .filter(|p| p.annotation_is_numeric_literal)
        .for_each(|p| out.push(make_param_diagnostic(p, path)));

    if let Some(vararg) = &func.vararg {
        if vararg.annotation_is_numeric_literal {
            out.push(make_param_diagnostic(vararg, path));
        }
    }

    if let Some(kwarg) = &func.kwarg {
        if kwarg.annotation_is_numeric_literal {
            out.push(make_param_diagnostic(kwarg, path));
        }
    }

    if func.return_annotation == ReturnAnnotationKind::NumericLiteral {
        out.push(make_return_diagnostic(func, path));
    }
}

fn make_param_diagnostic(param: &ParameterInfo, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Invalid type annotation on `{}` — numeric literals are not valid types",
            param.name
        ),
        span: param.name_span,
        path: path.to_owned(),
        help: Some(HELP.to_owned()),
        note: Some(
            "A literal value used as a type annotation has no meaning to the type checker"
                .to_owned(),
        ),
        provenance: None,
    }
}

fn make_return_diagnostic(func: &FunctionInfo, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Invalid return type annotation on `{}` — numeric literals are not valid types",
            func.name
        ),
        span: func.name_span,
        path: path.to_owned(),
        help: Some(HELP.to_owned()),
        note: Some(
            "A literal value used as a type annotation has no meaning to the type checker"
                .to_owned(),
        ),
        provenance: None,
    }
}
