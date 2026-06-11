//! Implements [BSK-E0092] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
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

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

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

    let all_typevar_names: HashSet<&str> =
        basilisk_resolver::collect_name_set(&module.typevar_calls);

    let paramspec_names: HashSet<&str> =
        basilisk_resolver::collect_name_set_where(&module.typevar_calls, |tv| tv.is_paramspec);

    let tvt_names = super::shared::typevar_tuple_names(&module.typevar_calls);

    let mut arities: HashMap<&'a str, TypeArity> = HashMap::new();

    // Class arities.
    for cls in &module.classes {
        if !cls.generic_params.is_empty() {
            if is_single_paramspec_generic(cls, &paramspec_names) {
                // A class generic over exactly one `ParamSpec` accepts any
                // number of type arguments: `ClassC[int, str]` means
                // `ClassC[[int, str]]` (PEP 612 unparenthesized shorthand).
                let _ = arities.insert(
                    cls.name.as_str(),
                    TypeArity {
                        min: None,
                        max: None,
                    },
                );
                continue;
            }
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
            let _ = arities.insert(
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
                    let _ = arities.insert(
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
                    let _ = arities.insert(
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
        let _ = arities.insert(
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

/// A type argument occupying a `ParamSpec` parameter slot must be a
/// parameters form: a list (`[int, str]`), `...`, another `ParamSpec`, or
/// `Concatenate[...]`.  A plain type (`ClassA[int, int]`) is an error.
///
/// Only multi-parameter generics are checked: a class generic over a single
/// `ParamSpec` treats all arguments as the implicit parameter list (PEP 612).
fn check_paramspec_slot_args(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    let paramspec_names: HashSet<&str> =
        basilisk_resolver::collect_name_set_where(&module.typevar_calls, |tv| tv.is_paramspec);
    if paramspec_names.is_empty() {
        return;
    }
    // Positional zip is only sound for fixed-arity generics: a TypeVarTuple
    // absorbs a variable number of arguments, so those classes are skipped.
    let class_params: HashMap<&str, Vec<&str>> = module
        .classes
        .iter()
        .filter(|cls| {
            cls.generic_params.len() > 1 && !cls.generic_params.iter().any(|p| p.is_typevartuple)
        })
        .map(|cls| {
            let names = cls.generic_params.iter().map(|p| p.name.as_str()).collect();
            (cls.name.as_str(), names)
        })
        .collect();

    for site in &module.generic_subscript_sites {
        let Some(params) = class_params.get(site.base_name.as_str()) else {
            continue;
        };
        let Some(text) = crate::span_util::slice_span(&module.source, site.span) else {
            continue;
        };
        let Some(args) = subscript_arg_texts(text) else {
            continue;
        };
        if args.len() != params.len() {
            continue;
        }
        for (param_name, arg) in params.iter().zip(args.iter()) {
            if !paramspec_names.contains(param_name) {
                continue;
            }
            if is_parameters_form(arg, &paramspec_names) {
                continue;
            }
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Type argument `{arg}` is not valid for `ParamSpec` parameter \
                     `{param_name}` of `{}`",
                    site.base_name
                ),
                site.span,
                &module.path,
                Some(
                    "A ParamSpec must be specialized with a parameter list (`[int, str]`), \
                     `...`, another ParamSpec, or `Concatenate[...]`"
                        .to_owned(),
                ),
                None,
            ));
        }
    }
}

/// Split `Base[a, b, ...]` into its top-level argument texts.
fn subscript_arg_texts(text: &str) -> Option<Vec<&str>> {
    let inner = text.trim().split_once('[')?.1.strip_suffix(']')?;
    Some(
        super::shared::split_top_level_commas(inner)
            .into_iter()
            .map(str::trim)
            .collect(),
    )
}

/// `true` when `arg` is a valid specialization for a `ParamSpec` slot.
fn is_parameters_form(arg: &str, paramspec_names: &HashSet<&str>) -> bool {
    arg.starts_with('[')
        || arg == "..."
        || paramspec_names.contains(arg)
        || arg.starts_with("Concatenate[")
}

impl Rule for TooFewTypeArguments {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let arities = compute_arities(module);
        check_paramspec_slot_args(module, diagnostics);

        for site in &module.generic_subscript_sites {
            // Check built-in types with fixed arity (e.g. `type[int, str]` is invalid).
            for &(builtin, exact) in BUILTIN_EXACT_ARITY {
                if site.base_name == builtin && site.arg_count > exact {
                    diagnostics.push(error_diagnostic_owned(
                        CODE.clone(),
                        format!(
                            "`{}` accepts exactly {exact} type argument, but {} {} provided",
                            site.base_name,
                            site.arg_count,
                            if site.arg_count == 1 { "was" } else { "were" }
                        ),
                        site.span,
                        &module.path,
                        Some("`type` takes exactly one type argument: `type[T]`".to_string()),
                        None,
                    ));
                }
            }

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
