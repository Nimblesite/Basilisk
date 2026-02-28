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

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0043",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0043",
};

/// Emits BSK-E0043 when a non-TypeVar appears in `Generic[...]` or `Protocol[...]`.
pub(crate) struct NonTypeVarInGeneric;

impl Rule for NonTypeVarInGeneric {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for class in &module.classes {
            for span in &class.generic_non_typevar_args {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Non-TypeVar argument in `Generic[...]` or `Protocol[...]` for `{}`",
                        class.name
                    ),
                    span: *span,
                    path: module.path.clone(),
                    help: Some(
                        "All arguments to `Generic[...]` must be TypeVar, TypeVarTuple, \
                         or ParamSpec instances"
                            .to_owned(),
                    ),
                    note: Some(
                        "PEP 484: `Generic[int]` is invalid; use a TypeVar instead".to_owned(),
                    ),
                });
            }
        }
    }
}
