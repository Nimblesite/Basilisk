//! Implements [`annotations_typeexpr`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! `annotations_typeexpr`: Invalid type form — numeric literal used as type annotation.
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

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "annotations_typeexpr",
    docs_url: "https://www.basilisk-python.dev/errors/annotations_typeexpr",
};

const HELP: &str = "Use a type name like `int`, `str`, `float` instead of a literal value";

/// Emits `annotations_typeexpr` for function parameters and return annotations that are
/// numeric or boolean literals.
pub(crate) struct InvalidTypeForm;

impl Rule for InvalidTypeForm {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
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
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Invalid type annotation on `{}` — numeric literals are not valid types",
            param.name
        ),
        param.name_span,
        path,
        Some(HELP.to_owned()),
        Some(
            "A literal value used as a type annotation has no meaning to the type checker"
                .to_owned(),
        ),
    )
}

fn make_return_diagnostic(func: &FunctionInfo, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Invalid return type annotation on `{}` — numeric literals are not valid types",
            func.name
        ),
        func.name_span,
        path,
        Some(HELP.to_owned()),
        Some(
            "A literal value used as a type annotation has no meaning to the type checker"
                .to_owned(),
        ),
    )
}
