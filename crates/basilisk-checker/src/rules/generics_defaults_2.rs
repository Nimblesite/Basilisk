//! Implements [`generics_defaults_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `generics_defaults_2`: Incompatible `TypeVar` bound or constraint with its default.
//!
//! PEP 696 specifies two constraints on `TypeVar` defaults:
//!
//! 1. If both `bound` and `default` are specified, the default must be a subtype
//!    of the bound. The numeric subtype hierarchy is `bool <: int <: float <: complex`.
//!
//! 2. For constrained `TypeVar`s, the default must be one of the constraints exactly.
//!    (Even a subtype is disallowed — `float` is a subtype of `complex` but if the
//!    constraints are `[float, str]` and the default is `complex`, that is an error.)
//!
//! ```python
//! from typing import TypeVar
//!
//! Ok1 = TypeVar("Ok1", bound=float, default=int)     # OK — int <: float
//! Invalid1 = TypeVar("Invalid1", bound=str, default=int)  # E — int is not <: str
//!
//! Ok2 = TypeVar("Ok2", float, str, default=float)    # OK
//! Invalid2 = TypeVar("Invalid2", float, str, default=int)  # E — int not in {float, str}
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "generics_defaults_2",
    docs_url: "https://www.basilisk-python.dev/errors/generics_defaults_2",
};

/// Emits `generics_defaults_2` for `TypeVar` bound/constraint vs default incompatibilities.
pub(crate) struct TypeVarDefaultIncompatible;

impl Rule for TypeVarDefaultIncompatible {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Build a set of TypeVar names in scope — when the default is another TypeVar,
        // the check is referential and requires comparing bounds, which is out of scope
        // for this simple check. Skip those to avoid false positives.
        let typevar_names: std::collections::HashSet<&str> = module
            .typevar_calls
            .iter()
            .map(|tv| tv.name.as_str())
            .collect();
        // One subtyping implementation ([NARROWPLAN-SUBTYPING]): bound
        // verdicts route through the module-seeded context, so a default
        // that subclasses the bound is accepted, not just the numeric tower.
        let subtyping = crate::subtyping::module_context(module);

        for tv in &module.typevar_calls {
            // Only plain TypeVar can have bounds/constraints with defaults.
            // TypeVarTuple and ParamSpec have different semantics.
            if tv.is_typevartuple || tv.is_paramspec {
                continue;
            }
            if !tv.has_default {
                continue;
            }
            let Some(ref default_name) = tv.default_type_name else {
                continue;
            };
            // If the default is another TypeVar, referential checking (comparing bounds)
            // is needed — skip to avoid false positives.
            if typevar_names.contains(default_name.as_str()) {
                continue;
            }

            // Case 1: bound + default — default must be a subtype of bound.
            if tv.has_bound {
                if let Some(ref bound_name) = tv.bound_type_name {
                    if !subtyping.is_subtype(default_name, bound_name) {
                        diagnostics.push(error_diagnostic_owned(
                            CODE.clone(),
                            format!(
                                "`TypeVar` `{}` has `default={default_name}` which is not a \
                                 subtype of `bound={bound_name}`",
                                tv.name
                            ),
                            tv.span,
                            &module.path,
                            Some(format!(
                                "The default must be a subtype of the bound; \
                                 `{default_name}` is not a subtype of `{bound_name}`"
                            )),
                            None,
                        ));
                    }
                }
                continue; // A TypeVar has either bound OR constraints, not both.
            }

            // Case 2: constrained TypeVar — default must exactly match one constraint.
            if !tv.constraint_type_names.is_empty()
                && !tv.constraint_type_names.iter().any(|c| c == default_name)
            {
                let constraint_list = tv
                    .constraint_type_names
                    .iter()
                    .map(|c| format!("`{c}`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "`TypeVar` `{}` has `default={default_name}` which is not one of the \
                         constraints ({constraint_list})",
                        tv.name
                    ),
                    tv.span,
                    &module.path,
                    Some(format!(
                        "The default for a constrained `TypeVar` must be exactly one of its \
                         constraints; choose one of {constraint_list}"
                    )),
                    None,
                ));
            }
        }
    }
}
