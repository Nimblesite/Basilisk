//! BSK-E0092: Wrong number of type arguments to a generic class or type alias.
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

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0092",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0092",
};

/// Emits BSK-E0092 when a generic subscript provides too few or too many type arguments.
pub(crate) struct TooFewTypeArguments;

/// Arity bounds for a generic type: optional minimum (for "too few") and
/// optional maximum (for "too many").
struct TypeArity {
    min: Option<usize>,
    max: Option<usize>,
}

/// Computes arity bounds for every class and `TypeAlias` in `module`.
fn compute_arities<'a>(module: &'a ResolvedModule) -> HashMap<&'a str, TypeArity> {
    let tv_defaults: HashMap<&str, bool> = module
        .typevar_calls
        .iter()
        .map(|tv| (tv.name.as_str(), tv.has_default))
        .collect();

    let all_typevar_names: HashSet<&str> = module
        .typevar_calls
        .iter()
        .map(|tv| tv.name.as_str())
        .collect();

    let tvt_names: HashSet<&str> = module
        .typevar_calls
        .iter()
        .filter(|tv| tv.is_typevartuple)
        .map(|tv| tv.name.as_str())
        .collect();

    let mut arities: HashMap<&'a str, TypeArity> = HashMap::new();

    // Class arities.
    for cls in &module.classes {
        if !cls.generic_params.is_empty() {
            let has_tvt = cls.generic_params.iter().any(|p| p.is_typevartuple);
            let required = cls
                .generic_params
                .iter()
                .filter(|p| {
                    !p.is_typevartuple
                        && !tv_defaults.get(p.name.as_str()).copied().unwrap_or(false)
                })
                .count();
            let total = cls.generic_params.len();
            arities.insert(
                cls.name.as_str(),
                TypeArity {
                    min: (required > 0).then_some(required),
                    max: (!has_tvt).then_some(total),
                },
            );
        } else if !cls.base_expression_names.is_empty() {
            let has_tvt = cls
                .base_expression_names
                .iter()
                .any(|n| tvt_names.contains(n.as_str()));
            if !has_tvt {
                let implicit_arity = cls
                    .base_expression_names
                    .iter()
                    .filter(|n| all_typevar_names.contains(n.as_str()))
                    .collect::<HashSet<_>>()
                    .len();
                if implicit_arity > 0 {
                    arities.insert(
                        cls.name.as_str(),
                        TypeArity {
                            min: None,
                            max: Some(implicit_arity),
                        },
                    );
                } else if cls.has_subscript_base && !cls.has_pep695_type_params {
                    // All TypeVars in this class's bases are fully specialised with
                    // concrete types. The class itself has no free TypeVar parameters
                    // and must not be further subscripted.
                    // Exclude PEP 695 classes (`class Foo[T]`) because their type
                    // params don't appear in `base_expression_names`.
                    arities.insert(
                        cls.name.as_str(),
                        TypeArity {
                            min: None,
                            max: Some(0),
                        },
                    );
                }
            }
        }
    }

    // TypeAlias arities.
    for alias in &module.type_alias_defs {
        let has_tvt = alias
            .rhs_names
            .iter()
            .any(|n| tvt_names.contains(n.as_str()));
        if has_tvt {
            continue;
        }
        let free_tvs: HashSet<&str> = alias
            .rhs_names
            .iter()
            .filter(|n| all_typevar_names.contains(n.as_str()))
            .map(String::as_str)
            .collect();
        let total = free_tvs.len();
        let required = free_tvs
            .iter()
            .filter(|&&n| !tv_defaults.get(n).copied().unwrap_or(false))
            .count();
        arities.insert(
            alias.name.as_str(),
            TypeArity {
                min: (required > 0 && required < total).then_some(required),
                max: Some(total),
            },
        );
    }

    arities
}

/// Built-in generic names with a fixed exact arity.
const BUILTIN_EXACT_ARITY: &[(&str, usize)] = &[("type", 1)];

impl Rule for TooFewTypeArguments {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let arities = compute_arities(module);

        for site in &module.generic_subscript_sites {
            // Check built-in types with fixed arity (e.g. `type[int, str]` is invalid).
            for &(builtin, exact) in BUILTIN_EXACT_ARITY {
                if site.base_name == builtin && site.arg_count > exact {
                    diagnostics.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Error,
                        message: format!(
                            "`{}` accepts exactly {exact} type argument, but {} {} provided",
                            site.base_name,
                            site.arg_count,
                            if site.arg_count == 1 { "was" } else { "were" }
                        ),
                        span: site.span,
                        path: module.path.clone(),
                        help: Some("`type` takes exactly one type argument: `type[T]`".to_string()),
                        note: None,
                    });
                }
            }

            let Some(arity) = arities.get(site.base_name.as_str()) else {
                continue;
            };

            if let Some(min_args) = arity.min {
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
                    diagnostics.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Error,
                        message,
                        span: site.span,
                        path: module.path.clone(),
                        help: Some(help),
                        note: None,
                    });
                }
            }
        }
    }
}
