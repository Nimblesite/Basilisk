//! Implements [BSK-E0043] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-immutability
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

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0043",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0043",
};

fn make_diagnostic(message: String, span: basilisk_resolver::Span, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        message,
        span,
        path,
        Some(
            "All arguments to `Generic[...]` must be TypeVar, TypeVarTuple, \
             or ParamSpec instances"
                .to_owned(),
        ),
        Some("PEP 484: `Generic[int]` is invalid; use a TypeVar instead".to_owned()),
    )
}

/// Emits BSK-E0043 when a non-TypeVar appears in `Generic[...]` or `Protocol[...]`.
pub(crate) struct NonTypeVarInGeneric;

impl Rule for NonTypeVarInGeneric {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Collect all module-level TypeVar names (traditional TypeVar calls).
        let typevar_names: HashSet<&str> =
            basilisk_resolver::collect_name_set(&module.typevar_calls);

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
            let pep695_params: HashSet<&str> = class
                .pep695_type_param_names
                .iter()
                .map(String::as_str)
                .collect();

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
                let is_likely_typevar =
                    name.chars().next().is_some_and(char::is_uppercase) || name.starts_with('_');
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

            // Check that all TypeVars used in non-Generic base subscripts are declared
            // in the explicit `Generic[...]` or `Protocol[...]` parameter list.
            // e.g. `class Bad(Iterable[T_co], Generic[S_co])` is invalid because
            // T_co is used in a base but not listed in Generic[S_co].
            if !class.generic_params.is_empty() {
                let declared_in_generic: HashSet<&str> =
                    basilisk_resolver::collect_name_set(&class.generic_params);

                // Collect TypeVar names used anywhere in base class expressions
                // but not declared in Generic[...]/Protocol[...].
                let mut undeclared: Vec<&str> = class
                    .base_expression_names
                    .iter()
                    .map(String::as_str)
                    .filter(|n| typevar_names.contains(n))
                    .filter(|n| !declared_in_generic.contains(n))
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect();
                undeclared.sort_unstable();

                if !undeclared.is_empty() {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "Type parameter`{}` used in a base class of `{}` \
                             but not listed in `Generic[...]` or `Protocol[...]`",
                            undeclared.join("`, `"),
                            class.name
                        ),
                        class.name_span,
                        &module.path,
                    ));
                }
            }
        }
    }
}
