//! Implements [BSK-E0001] from [CHKARCH-DIAG-MISSING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-missing
//! BSK-E0001: Missing parameter type annotation.

use basilisk_resolver::{FunctionInfo, ParameterInfo, ResolvedModule};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::{guards::is_stub_context, Rule};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0001",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0001",
};

/// Emits BSK-E0001 for every unannotated regular parameter (not `*args`/`**kwargs`).
///
/// `*args` and `**kwargs` are handled by [`super::e0004`].
/// Skipped for `@overload`, `@abstractmethod`, and `Protocol` methods.
pub(crate) struct MissingParameterAnnotation;

impl Rule for MissingParameterAnnotation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        module
            .functions
            .iter()
            .filter(|func| !is_stub_context(func, &module.classes))
            .for_each(|func| check_function(func, &module.path, diagnostics));
    }
}

fn check_function(func: &FunctionInfo, path: &str, out: &mut Vec<Diagnostic>) {
    func.parameters
        .iter()
        .filter(|p| !p.has_annotation && p.name != "self" && p.name != "cls")
        .for_each(|p| out.push(make_diagnostic(p, path)));
}

fn make_diagnostic(param: &ParameterInfo, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!("Missing parameter type annotation for `{}`", param.name),
        param.name_span,
        path,
        Some(format!("Add a type annotation: `{}: <type>`", param.name)),
        Some("In Basilisk, all function parameters require explicit types".to_owned()),
    )
}
