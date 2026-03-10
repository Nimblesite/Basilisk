//! BSK-E0019: Unbound variable on some code paths.
//!
//! When a function contains a `return <name>` statement and the name is
//! assigned in the function body, but only inside conditional branches
//! (e.g. `if`, `while`, `try`), it may be unbound when the `return` is
//! reached on other paths.
//!
//! ```python
//! def maybe_assign(flag: bool) -> int:
//!     if flag:
//!         result = 42
//!     return result   # result may be unbound if flag is False → E0019
//! ```

use basilisk_resolver::{FunctionInfo, ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0019",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0019",
};

/// Emits BSK-E0019 for return statements that reference conditionally-assigned names.
pub(crate) struct UnboundVariable;

impl Rule for UnboundVariable {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        module.functions.iter().for_each(|func| {
            check_function(func, &module.path, diagnostics);
        });
    }
}

fn check_function(func: &FunctionInfo, path: &str, out: &mut Vec<Diagnostic>) {
    let param_names: Vec<&str> = func.parameters.iter().map(|p| p.name.as_str()).collect();

    // Use top_level_return_name_refs to avoid false positives where a `return name`
    // is inside the same conditional branch that assigned `name`.
    for (name, span) in &func.top_level_return_name_refs {
        // Skip parameter names — always bound.
        if param_names.contains(&name.as_str()) {
            continue;
        }
        // Only flag names that ARE assigned somewhere (just not unconditionally).
        if !func.all_local_assigns.iter().any(|a| a == name) {
            continue;
        }
        // Flag if not assigned unconditionally at the top level.
        if !func.unconditional_assigns.iter().any(|a| a == name) {
            out.push(make_diagnostic(func, name, *span, path));
        }
    }
}

fn make_diagnostic(func: &FunctionInfo, name: &str, span: Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Function `{}` returns `{name}` but `{name}` may be unbound on some paths",
            func.name
        ),
        span,
        path: path.to_owned(),
        help: Some(format!(
            "Assign `{name}` unconditionally before the `return`, or add a default value"
        )),
        note: Some(
            "Basilisk detects variables that are assigned only inside conditional branches \
             (if/while/try) and may not be defined on every execution path"
                .to_owned(),
        ),
    }
}
