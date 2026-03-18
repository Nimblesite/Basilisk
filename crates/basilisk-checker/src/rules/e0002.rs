//! BSK-E0002: Missing return type annotation.

use basilisk_resolver::{FunctionInfo, ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::{guards::is_stub_context, Rule};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0002",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0002",
};

/// Emits BSK-E0002 for every function without a return type annotation.
///
/// Skipped for `@overload`, `@abstractmethod`, and `Protocol` methods.
pub(crate) struct MissingReturnAnnotation;

impl Rule for MissingReturnAnnotation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        module
            .functions
            .iter()
            .filter(|func| {
                !func.return_annotation.is_present() && !is_stub_context(func, &module.classes)
            })
            .for_each(|func| diagnostics.push(make_diagnostic(func, &module.path)));
    }
}

fn make_diagnostic(func: &FunctionInfo, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Missing return type annotation for function `{}`",
            func.name
        ),
        span: Span {
            start: func.name_span.start,
            end: func.params_end,
        },
        path: path.to_owned(),
        help: Some(format!(
            "Add a return type: `def {}(...) -> <type>:`",
            func.name
        )),
        note: Some("In Basilisk, all functions require an explicit return type".to_owned()),
    }
}
