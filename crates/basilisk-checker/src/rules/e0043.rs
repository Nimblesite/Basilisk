//! BSK-E0043: Non-TypeVar argument in `Generic[...]` or `Protocol[...]`.
//!
//! PEP 484 requires that all arguments to `Generic[...]` and `Protocol[...]`
//! be type variable names (`TypeVar`, `TypeVarTuple`, or `ParamSpec`).
//! Passing a concrete type (e.g. `Generic[int]`) is a type error.
//!
//! ```python
//! class Bad1(Generic[int]): ...      # E — `int` is not a TypeVar
//! class Bad2(Protocol[int]): ...     # E — `int` is not a TypeVar
//! ```

use std::collections::HashSet;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0043",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0043",
};

fn make_diagnostic(message: String, span: basilisk_resolver::Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message,
        span,
        path: path.to_owned(),
        help: Some(
            "All arguments to `Generic[...]` must be TypeVar, TypeVarTuple, \
             or ParamSpec instances"
                .to_owned(),
        ),
        note: Some(
            "PEP 484: `Generic[int]` is invalid; use a TypeVar instead".to_owned(),
        ),
    }
}

/// Emits BSK-E0043 when a non-TypeVar appears in `Generic[...]` or `Protocol[...]`.
pub(crate) struct NonTypeVarInGeneric;

impl Rule for NonTypeVarInGeneric {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Collect all module-level TypeVar names (traditional TypeVar calls).
        let typevar_names: HashSet<&str> = module
            .typevar_calls
            .iter()
            .map(|tv| tv.name.as_str())
            .collect();

        for class in &module.classes {
            // Flag non-simple-name args (subscripts etc.) in Generic/Protocol.
            for span in &class.generic_non_typevar_args {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Non-TypeVar argument in `Generic[...]` or `Protocol[...]` for `{}`",
                        class.name
                    ),
                    *span,
                    &module.path,
                ));
            }

            // Flag simple-name args that are NOT known TypeVars and NOT PEP 695 type params.
            // This catches `Generic[int]` where `int` is a builtin type, not a TypeVar.
            let pep695_params: HashSet<&str> =
                class.pep695_type_param_names.iter().map(String::as_str).collect();

            for param in &class.generic_params {
                let name = param.name.as_str();
                // Skip if it's a known TypeVar from this module.
                if typevar_names.contains(name) {
                    continue;
                }
                // Skip if it's a PEP 695 type parameter.
                if pep695_params.contains(name) {
                    continue;
                }
                // Skip well-known TypeVar-like names that might be imported.
                // These are common names that type checkers conventionally accept.
                // We only flag names that look like concrete types (lowercase or builtins).
                let is_likely_typevar = name.chars().next().is_some_and(|c| c.is_uppercase())
                    || name.starts_with('_');
                if is_likely_typevar {
                    continue;
                }
                diagnostics.push(make_diagnostic(
                    format!(
                        "`{}` is not a TypeVar but is used as a type parameter in `Generic[...]` or `Protocol[...]` for `{}`",
                        name, class.name
                    ),
                    param.span,
                    &module.path,
                ));
            }
        }
    }
}
