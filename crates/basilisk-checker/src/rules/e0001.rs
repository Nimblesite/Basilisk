//! BSK-E0001: Missing parameter type annotation.

use basilisk_resolver::{FunctionInfo, ParameterInfo, ResolvedModule};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0001",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0001",
};

/// Emits BSK-E0001 for every unannotated regular parameter (not `*args`/`**kwargs`).
///
/// `*args` and `**kwargs` are handled by [`super::e0004`].
pub(crate) struct MissingParameterAnnotation;

impl Rule for MissingParameterAnnotation {
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
        .filter(|p| !p.has_annotation)
        .for_each(|p| out.push(make_diagnostic(p, path)));
}

fn make_diagnostic(param: &ParameterInfo, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!("Missing parameter type annotation for `{}`", param.name),
        span: param.name_span,
        path: path.to_owned(),
        help: Some(format!("Add a type annotation: `{}: <type>`", param.name)),
        note: Some("In Basilisk, all function parameters require explicit types".to_owned()),
    }
}
