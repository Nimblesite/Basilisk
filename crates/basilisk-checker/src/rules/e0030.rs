//! BSK-E0030: Non-default `TypeVar` follows a default `TypeVar` in `Generic[...]`.
//!
//! PEP 696 §Ordering requires that once a `TypeVar` with a `default=` argument
//! appears in `Generic[...]`, all subsequent type variables must also have
//! defaults.

use std::collections::HashMap;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0030",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0030",
};

/// Emits BSK-E0030 when a non-default `TypeVar` follows a default `TypeVar` in `Generic[...]`.
pub(crate) struct NonDefaultAfterDefault;

impl Rule for NonDefaultAfterDefault {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let typevar_defaults: HashMap<&str, bool> = module
            .typevar_calls
            .iter()
            .map(|tv| (tv.name.as_str(), tv.has_default))
            .collect();

        for class in &module.classes {
            if class.generic_params.is_empty() {
                continue;
            }
            let mut seen_default = false;
            for param in &class.generic_params {
                let has_default = typevar_defaults
                    .get(param.name.as_str())
                    .copied()
                    .unwrap_or(false);
                if has_default {
                    seen_default = true;
                } else if seen_default {
                    diagnostics.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Error,
                        message: format!(
                            "Non-default `TypeVar` `{}` follows a default `TypeVar` in `Generic[...]` for `{}`",
                            param.name, class.name
                        ),
                        span: param.span,
                        path: module.path.clone(),
                        help: Some(
                            "Move all non-default `TypeVar`s before any `TypeVar` with a `default=`"
                                .to_owned(),
                        ),
                        note: Some(
                            "PEP 696: non-default TypeVars must not follow default TypeVars"
                                .to_owned(),
                        ),
                    });
                }
            }
        }
    }
}
