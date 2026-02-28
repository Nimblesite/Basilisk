//! BSK-E0041: Too few arguments in a function call.
//!
//! When a function is called with fewer positional arguments than it has
//! required parameters (parameters without default values), Basilisk reports
//! a missing-argument error.
//!
//! ```python
//! def func1(a: int, b: str) -> None: ...
//!
//! func1()  # E: missing required arguments
//! func1(1)  # E: missing required argument `b`
//! ```

use std::collections::HashMap;

use basilisk_resolver::{FunctionInfo, ResolvedModule};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0041",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0041",
};

/// Emits BSK-E0041 for call sites with too few positional arguments.
pub(crate) struct TooFewArguments;

impl Rule for TooFewArguments {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Build map from function name → &FunctionInfo (module-level functions only).
        let func_map: HashMap<&str, &FunctionInfo> = module
            .functions
            .iter()
            .filter(|f| f.class_name.is_none())
            .map(|f| (f.name.as_str(), f))
            .collect();

        for call in &module.calls {
            let Some(func) = func_map.get(call.callee.as_str()) else {
                continue;
            };

            // If the function has *args, any number of positional args is fine.
            if func.vararg.is_some() {
                continue;
            }

            // Count required parameters (those without defaults, excluding *args/**kwargs).
            let required_count = func.parameters.iter().filter(|p| !p.has_default).count();

            // Keyword arguments may satisfy positional requirements —
            // conservatively skip the check if any keywords are present.
            if !call.keywords.is_empty() {
                continue;
            }

            let provided_count = call.args.len();

            if provided_count < required_count {
                let missing = required_count - provided_count;
                let func_name = &func.name;
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Call to `{func_name}` is missing {missing} required argument{} \
                         (expected {required_count}, got {provided_count})",
                        if missing == 1 { "" } else { "s" },
                    ),
                    span: call.span,
                    path: module.path.clone(),
                    help: None,
                    note: None,
                });
            }
        }
    }
}
