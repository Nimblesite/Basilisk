//! BSK-E0027: Duplicate `TypeVar` in a `Generic[...]` base.
//!
//! Each type parameter in `Generic[T1, T2, ...]` must be unique.
//! `Generic[T, T]` is an error per PEP 484.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, error_diagnostic_owned};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0027",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0027",
};

/// Emits BSK-E0027 when the same `TypeVar` appears more than once in `Generic[...]`.
pub(crate) struct DuplicateTypeVarInGeneric;

impl Rule for DuplicateTypeVarInGeneric {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for class in &module.classes {
            let params = &class.generic_params;
            let mut seen: Vec<&str> = Vec::new();
            for param in params {
                if seen.contains(&param.name.as_str()) {
                    diagnostics.push(error_diagnostic_owned(
                        CODE.clone(),
                        format!(
                            "TypeVar `{}` appears more than once in `Generic[...]` for `{}`",
                            param.name, class.name
                        ),
                        param.span,
                        &module.path,
                        Some(
                            "Each TypeVar must appear exactly once in the Generic base".to_owned(),
                        ),
                        Some(
                            "PEP 484: duplicate TypeVar parameters in Generic are invalid"
                                .to_owned(),
                        ),
                    ));
                } else {
                    seen.push(&param.name);
                }
            }
        }
    }
}
