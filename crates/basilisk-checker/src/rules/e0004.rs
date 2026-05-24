//! Implements [BSK-E0004] from [CHKARCH-DIAG-MISSING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-missing
//! BSK-E0004: Missing `*args` / `**kwargs` type annotation.
//!
//! Every variadic positional parameter (`*args`) and variadic keyword parameter
//! (`**kwargs`) must carry an explicit type annotation in strict Basilisk code.

use basilisk_resolver::{FunctionInfo, ParameterInfo, ResolvedModule};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::{guards::is_stub_context, Rule};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0004",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0004",
};

/// Emits BSK-E0004 for unannotated `*args` and `**kwargs` parameters.
///
/// Skipped for `@overload`, `@abstractmethod`, and `Protocol` methods.
pub(crate) struct MissingVarArgAnnotation;

impl Rule for MissingVarArgAnnotation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        module
            .functions
            .iter()
            .filter(|func| !is_stub_context(func, &module.classes))
            .for_each(|func| check_function(func, &module.path, diagnostics));
    }
}

fn check_function(func: &FunctionInfo, path: &str, out: &mut Vec<Diagnostic>) {
    if let Some(vararg) = &func.vararg {
        if !vararg.has_annotation {
            out.push(make_vararg_diagnostic(vararg, path));
        }
    }

    if let Some(kwarg) = &func.kwarg {
        if !kwarg.has_annotation {
            out.push(make_kwarg_diagnostic(kwarg, path));
        }
    }
}

fn make_vararg_diagnostic(param: &ParameterInfo, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Missing type annotation for `{}` (`*{}` parameter)",
            param.name, param.name
        ),
        param.name_span,
        path,
        Some(format!("Add a type annotation: `*{}: <type>`", param.name)),
        Some("In Basilisk, `*args` parameters require an explicit element type".to_owned()),
    )
}

fn make_kwarg_diagnostic(param: &ParameterInfo, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Missing type annotation for `{}` (`**{}` parameter)",
            param.name, param.name
        ),
        param.name_span,
        path,
        Some(format!("Add a type annotation: `**{}: <type>`", param.name)),
        Some("In Basilisk, `**kwargs` parameters require an explicit value type".to_owned()),
    )
}
