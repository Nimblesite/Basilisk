//! BSK-E0018: Undefined variable used in a return statement.
//!
//! When a function contains a `return <name>` statement and the name is not
//! defined anywhere in the function scope (not a parameter, not assigned via
//! `=`, `for`, `with`, or a nested `def`), it is flagged as undefined.
//!
//! ```python
//! def compute() -> int:
//!     return undefined_name   # undefined_name is never assigned → E0018
//! ```

use basilisk_resolver::{FunctionInfo, ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0018",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0018",
};

/// Emits BSK-E0018 for return statements that reference undefined names.
pub(crate) struct UndefinedVariable;

impl Rule for UndefinedVariable {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        module.functions.iter().for_each(|func| {
            check_function(func, &module.path, diagnostics);
        });
    }
}

fn check_function(func: &FunctionInfo, path: &str, out: &mut Vec<Diagnostic>) {
    let param_names: Vec<&str> = func.parameters.iter().map(|p| p.name.as_str()).collect();

    for (name, span) in &func.return_name_refs {
        if !param_names.contains(&name.as_str())
            && !func.all_local_assigns.iter().any(|a| a == name)
        {
            out.push(make_diagnostic(func, name, *span, path));
        }
    }
}

fn make_diagnostic(func: &FunctionInfo, name: &str, span: Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Function `{}` returns `{name}` but `{name}` is not defined in this scope",
            func.name
        ),
        span,
        path: path.to_owned(),
        help: Some(format!(
            "Define `{name}` before returning it, or check for a typo"
        )),
        note: Some(
            "Basilisk detects names in return expressions that have no visible definition"
                .to_owned(),
        ),
    }
}
