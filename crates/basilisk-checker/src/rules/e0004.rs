//! BSK-E0004: Missing `*args` / `**kwargs` type annotation.
//!
//! Every variadic positional parameter (`*args`) and variadic keyword parameter
//! (`**kwargs`) must carry an explicit type annotation in strict Basilisk code.

use basilisk_resolver::{FunctionInfo, ParameterInfo, ResolvedModule};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

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
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Missing type annotation for `{}` (`*{}` parameter)",
            param.name, param.name
        ),
        span: param.name_span,
        path: path.to_owned(),
        help: Some(format!("Add a type annotation: `*{}: <type>`", param.name)),
        note: Some("In Basilisk, `*args` parameters require an explicit element type".to_owned()),
        provenance: None,
    }
}

fn make_kwarg_diagnostic(param: &ParameterInfo, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Missing type annotation for `{}` (`**{}` parameter)",
            param.name, param.name
        ),
        span: param.name_span,
        path: path.to_owned(),
        help: Some(format!("Add a type annotation: `**{}: <type>`", param.name)),
        note: Some("In Basilisk, `**kwargs` parameters require an explicit value type".to_owned()),
        provenance: None,
    }
}
