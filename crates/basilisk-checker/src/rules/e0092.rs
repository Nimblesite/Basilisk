//! BSK-E0092: Too few type arguments to a generic class.
//!
//! When a user-defined generic class has both required (non-default) and optional
//! (defaulted) type parameters, the minimum number of type arguments that must be
//! supplied when subscripting the class is the count of required parameters.
//!
//! ```python
//! from typing import Generic, TypeVar
//! from typing_extensions import TypeVar as TypeVarExt
//!
//! T1 = TypeVar("T1")
//! T2 = TypeVar("T2")
//! DefaultStrT = TypeVarExt("DefaultStrT", default=str)
//!
//! class AllTheDefaults(Generic[T1, T2, DefaultStrT]): ...
//!
//! AllTheDefaults[int]          # E — 1 arg but at least 2 required
//! AllTheDefaults[int, str]     # OK
//! AllTheDefaults[int, str, bytes]  # OK
//! ```

use std::collections::HashMap;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0092",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0092",
};

/// Emits BSK-E0092 when a generic subscript provides too few type arguments.
pub(crate) struct TooFewTypeArguments;

impl Rule for TooFewTypeArguments {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Build a map from TypeVar name → has_default.
        let tv_defaults: HashMap<&str, bool> = module
            .typevar_calls
            .iter()
            .map(|tv| (tv.name.as_str(), tv.has_default))
            .collect();

        // For each class, compute the minimum number of required type arguments
        // (number of non-default generic params).
        let class_min_args: HashMap<&str, usize> = module
            .classes
            .iter()
            .filter_map(|cls| {
                if cls.generic_params.is_empty() {
                    return None;
                }
                let required = cls
                    .generic_params
                    .iter()
                    .filter(|p| {
                        !p.is_typevartuple
                            && !tv_defaults
                                .get(p.name.as_str())
                                .copied()
                                .unwrap_or(false)
                    })
                    .count();
                // Only track classes that actually have required params with some optional ones too.
                if required == 0 || required == cls.generic_params.len() {
                    return None;
                }
                Some((cls.name.as_str(), required))
            })
            .collect();

        // Check each subscript site.
        for site in &module.generic_subscript_sites {
            let Some(&min_args) = class_min_args.get(site.base_name.as_str()) else {
                continue;
            };
            if site.arg_count < min_args {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Too few type arguments for `{}`; expected at least {min_args}, \
                         got {}",
                        site.base_name, site.arg_count
                    ),
                    span: site.span,
                    path: module.path.clone(),
                    help: Some(format!(
                        "Supply at least {min_args} type argument{} for `{}`",
                        if min_args == 1 { "" } else { "s" },
                        site.base_name
                    )),
                    note: None,
                });
            }
        }
    }
}
