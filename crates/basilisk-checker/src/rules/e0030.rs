//! BSK-E0030: Non-default `TypeVar` follows a default `TypeVar` in `Generic[...]`.
//!
//! PEP 696 §Ordering defines two ordering rules for type parameters in `Generic[...]`:
//!
//! 1. Once a `TypeVar` with a `default=` argument appears, all subsequent type variables
//!    must also have defaults.
//!
//! 2. A `TypeVar` with a `default=` cannot immediately follow a `TypeVarTuple` in
//!    `Generic[...]` because it would be ambiguous whether a type argument should be
//!    bound to the `TypeVarTuple` or the defaulted `TypeVar`. (`ParamSpec` with a
//!    default is allowed after a `TypeVarTuple`.)

use std::collections::HashMap;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, error_diagnostic_owned};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0030",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0030",
};

/// Per-TypeVar metadata looked up from `typevar_calls`.
struct TvMeta {
    has_default: bool,
    is_paramspec: bool,
}

/// Emits BSK-E0030 when a non-default `TypeVar` follows a default `TypeVar` in `Generic[...]`,
/// or when a defaulted `TypeVar` (not `ParamSpec`) follows a `TypeVarTuple`.
pub(crate) struct NonDefaultAfterDefault;

impl Rule for NonDefaultAfterDefault {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let tv_meta: HashMap<&str, TvMeta> = module
            .typevar_calls
            .iter()
            .map(|tv| {
                (
                    tv.name.as_str(),
                    TvMeta {
                        has_default: tv.has_default,
                        is_paramspec: tv.is_paramspec,
                    },
                )
            })
            .collect();

        for class in &module.classes {
            if class.generic_params.is_empty() {
                continue;
            }
            let mut seen_default = false;
            let mut seen_typevartuple = false;
            for param in &class.generic_params {
                if param.is_typevartuple {
                    // This position is a TypeVarTuple unpack (`*Ts`).
                    seen_typevartuple = true;
                    continue;
                }
                let meta = tv_meta.get(param.name.as_str());
                let has_default = meta.is_some_and(|m| m.has_default);
                let is_paramspec = meta.is_some_and(|m| m.is_paramspec);

                // Rule 2: TypeVar with default cannot follow a TypeVarTuple.
                // ParamSpec with default after TypeVarTuple is explicitly allowed.
                if seen_typevartuple && has_default && !is_paramspec {
                    diagnostics.push(error_diagnostic_owned(
                        CODE.clone(),
                        format!(
                            "`TypeVar` `{}` with `default=` cannot follow a `TypeVarTuple` \
                             in `Generic[...]` for `{}`",
                            param.name, class.name
                        ),
                        param.span,
                        &module.path,
                        Some(
                            "A defaulted `TypeVar` after a `TypeVarTuple` is ambiguous; \
                             use a `ParamSpec` instead if a trailing defaulted parameter is needed"
                                .to_owned(),
                        ),
                        Some(
                            "PEP 696: `TypeVar` with `default=` cannot immediately follow \
                             a `TypeVarTuple`"
                                .to_owned(),
                        ),
                    ));
                    continue;
                }

                // Rule 1: non-default TypeVar cannot follow a default TypeVar.
                if has_default {
                    seen_default = true;
                } else if seen_default {
                    diagnostics.push(error_diagnostic_owned(
                        CODE.clone(),
                        format!(
                            "Non-default `TypeVar` `{}` follows a default `TypeVar` in \
                             `Generic[...]` for `{}`",
                            param.name, class.name
                        ),
                        param.span,
                        &module.path,
                        Some(
                            "Move all non-default `TypeVar`s before any `TypeVar` with a `default=`"
                                .to_owned(),
                        ),
                        Some(
                            "PEP 696: non-default TypeVars must not follow default TypeVars"
                                .to_owned(),
                        ),
                    ));
                }
            }
        }
    }
}
