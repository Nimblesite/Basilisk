//! Implements [`generics_defaults_specialization`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `generics_defaults_specialization`: Wrong number of type arguments to a generic class or type alias.
//!
//! When a user-defined generic class has both required (non-default) and optional
//! (defaulted) type parameters, the minimum number of type arguments that must be
//! supplied when subscripting the class is the count of required parameters.
//!
//! Also detects when too many type arguments are supplied to a user-defined generic
//! class (one that has no `TypeVarTuple` and therefore a fixed maximum arity),
//! or to a `TypeAlias` that has a fixed number of free type variables.
//!
//! Additionally detects when a class that has fully specialised its generic base
//! (e.g. `class Foo(Bar[int])`) is subscripted further, since it has no free
//! type variables.
//!
//! ```python
//! from typing import Generic, TypeVar, TypeAlias
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
//!
//! class LinkedList(Generic[T]): ...
//!
//! LinkedList[int, str]  # E — 2 args but at most 1 allowed
//!
//! MyAlias: TypeAlias = LinkedList[T2]
//! MyAlias[int, str]  # E — 2 args but at most 1 allowed for the alias
//!
//! class Foo(LinkedList[int]): ...
//! Foo[str]  # E — Foo has no free type variables
//! ```

use std::collections::{HashMap, HashSet};

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "generics_defaults_specialization",
    docs_url: "https://www.basilisk-python.dev/errors/generics_defaults_specialization",
};

/// Emits `generics_defaults_specialization` when a generic subscript provides too few or too many type arguments.
pub(crate) struct TooFewTypeArguments;

/// Arity bounds for a generic type: optional minimum (for "too few") and
/// optional maximum (for "too many").
struct TypeArity {
    min: Option<usize>,
    max: Option<usize>,
}

// ##########################################################################
// # DELETED BODY. DO NOT RESTORE IT AND DO NOT RETURN A DEFAULT.
// #
// # `ClassInfo::base_expression_names` is a `Vec<String>` of RENDERED simple
// # names harvested from base-class expressions. This code matched those
// # strings against a set of TypeVar names collected the same way, so:
// #
// #   T = TypeVar("T")
// #   Alias = T
// #   class Foo(Generic[Alias]): ...      # TypeVar NOT recognised
// #
// #   class T: ...                        # unrelated class
// #   class Foo(Base[T]): ...             # treated as a TypeVar use
// #
// # Whether a base-expression name denotes a TypeVar is a question about the
// # binding it resolves to, not about the characters written.
// #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
// ##########################################################################
fn compute_arities<'a>(_module: &'a ResolvedModule) -> HashMap<&'a str, TypeArity> {
    panic!(
        "basilisk-checker: `compute_arities` was DELETED because it matched TypeVar identity by \
         RENDERED NAME against `base_expression_names`, so an aliased TypeVar was \
         invisible and any unrelated symbol spelled like one matched. It panics \
         because the real implementation — base expressions resolved through the \
         binding table — DOES NOT EXIST YET. Do not restore the name matching and \
         do not substitute a default answer."
    )
}

#[expect(
    dead_code,
    reason = "caller deleted for spelling dependence; retained for the rebuild — see \
              tests/string_keyed_class_hierarchy_pin_tests.rs"
)]
/// `true` when the class's only generic parameter is a `ParamSpec`.
fn is_single_paramspec_generic(
    cls: &basilisk_resolver::ClassInfo,
    paramspec_names: &HashSet<&str>,
) -> bool {
    matches!(
        cls.generic_params.as_slice(),
        [only] if paramspec_names.contains(only.name.as_str())
    )
}

impl Rule for TooFewTypeArguments {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let arities = compute_arities(module);

        for site in &module.generic_subscript_sites {
            let Some(arity) = arities.get(site.base_name.as_str()) else {
                continue;
            };

            if let Some(min_args) = arity.min {
                if site.arg_count < min_args {
                    diagnostics.push(error_diagnostic_owned(
                        CODE.clone(),
                        format!(
                            "Too few type arguments for `{}`; expected at least {min_args}, \
                             got {}",
                            site.base_name, site.arg_count
                        ),
                        site.span,
                        &module.path,
                        Some(format!(
                            "Supply at least {min_args} type argument{} for `{}`",
                            if min_args == 1 { "" } else { "s" },
                            site.base_name
                        )),
                        None,
                    ));
                }
            }

            if let Some(max_args) = arity.max {
                if site.arg_count > max_args {
                    let message = if max_args == 0 {
                        format!(
                            "`{}` cannot be subscripted; it has no free type parameters",
                            site.base_name
                        )
                    } else {
                        format!(
                            "Too many type arguments for `{}`; expected at most {max_args}, \
                             got {}",
                            site.base_name, site.arg_count
                        )
                    };
                    let help = if max_args == 0 {
                        format!(
                            "`{}` has no free type parameters and cannot be further subscripted",
                            site.base_name
                        )
                    } else {
                        format!(
                            "Supply at most {max_args} type argument{} for `{}`",
                            if max_args == 1 { "" } else { "s" },
                            site.base_name
                        )
                    };
                    diagnostics.push(error_diagnostic_owned(
                        CODE.clone(),
                        message,
                        site.span,
                        &module.path,
                        Some(help),
                        None,
                    ));
                }
            }
        }
    }
}
