//! BSK-E0002: Missing return type annotation.

use basilisk_resolver::{FunctionInfo, ResolvedModule};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0002",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0002",
};

/// Emits BSK-E0002 for every function without a return type annotation.
pub(crate) struct MissingReturnAnnotation;

impl Rule for MissingReturnAnnotation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        module
            .functions
            .iter()
            .filter(|func| !func.return_annotation.is_present())
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
        span: func.name_span,
        path: path.to_owned(),
        help: Some(format!(
            "Add a return type: `def {}(...) -> <type>:`",
            func.name
        )),
        note: Some("In Basilisk, all functions require an explicit return type".to_owned()),
    }
}
