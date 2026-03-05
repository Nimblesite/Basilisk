//! BSK-E0107: Variance incompatibility in base class parameterisation.
//!
//! When a class inherits from a generic base class (directly or through a type
//! alias), the `TypeVar` arguments must have compatible variance with the
//! corresponding type parameters declared by the base class.
//!
//! ```python
//! from typing import Generic, TypeVar
//!
//! T = TypeVar("T")            # invariant
//! T_co = TypeVar("T_co", covariant=True)
//!
//! class Base(Generic[T]): ...
//!
//! class Bad(Base[T_co]): ...  # E — invariant param gets covariant arg
//! ```

use std::collections::HashMap;

use basilisk_resolver::{ResolvedModule, TypeArg};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0107",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0107",
};

/// The variance of a `TypeVar`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Variance {
    Invariant,
    Covariant,
    Contravariant,
}

impl std::fmt::Display for Variance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Variance::Invariant => write!(f, "invariant"),
            Variance::Covariant => write!(f, "covariant"),
            Variance::Contravariant => write!(f, "contravariant"),
        }
    }
}

/// Compose two variances following the standard rules:
/// - co * co = co, co * contra = contra, contra * co = contra, contra * contra = co
/// - invariant * anything = invariant
fn compose_variance(outer: Variance, inner: Variance) -> Variance {
    match (outer, inner) {
        (Variance::Invariant, _) | (_, Variance::Invariant) => Variance::Invariant,
        (Variance::Covariant, v) | (v, Variance::Covariant) => v,
        (Variance::Contravariant, Variance::Contravariant) => Variance::Covariant,
    }
}

/// A single variance violation: the arg name, its variance, and the expected variance.
struct VarianceViolation {
    arg_name: String,
    arg_variance: Variance,
    expected_variance: Variance,
}

/// Emits BSK-E0107 for variance-incompatible `TypeVar` arguments in base classes.
pub(crate) struct VarianceIncompatibleBase;

impl Rule for VarianceIncompatibleBase {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Build a map of TypeVar name -> variance.
        let tv_variance: HashMap<&str, Variance> = module
            .typevar_calls
            .iter()
            .map(|tv| {
                let variance = if tv.is_covariant {
                    Variance::Covariant
                } else if tv.is_contravariant {
                    Variance::Contravariant
                } else {
                    Variance::Invariant
                };
                (tv.name.as_str(), variance)
            })
            .collect();

        // Build a map of class name -> ordered list of generic param names.
        let class_params: HashMap<&str, Vec<&str>> = module
            .classes
            .iter()
            .filter(|cls| !cls.generic_params.is_empty())
            .map(|cls| {
                let params: Vec<&str> =
                    cls.generic_params.iter().map(|p| p.name.as_str()).collect();
                (cls.name.as_str(), params)
            })
            .collect();

        // Build a map of alias name -> (base_name, ordered type arg names).
        let alias_info: HashMap<&str, (&str, &[String])> = module
            .type_alias_defs
            .iter()
            .filter_map(|alias| {
                let base = alias.rhs_base_name.as_deref()?;
                Some((
                    alias.name.as_str(),
                    (base, alias.rhs_type_arg_names.as_slice()),
                ))
            })
            .collect();

        for cls in &module.classes {
            for entry in &cls.base_subscripts {
                // Use rich type_args for nested variance checking when available,
                // fall back to flat type_arg_names otherwise.
                let violations = if entry.type_args.is_empty() {
                    resolve_and_check(
                        &entry.base_name,
                        &entry.type_arg_names,
                        &class_params,
                        &alias_info,
                        &tv_variance,
                    )
                } else {
                    resolve_and_check_rich(
                        &entry.base_name,
                        &entry.type_args,
                        &class_params,
                        &alias_info,
                        &tv_variance,
                    )
                };

                let Some(violations) = violations else {
                    continue;
                };

                if violations.is_empty() {
                    continue;
                }

                let details: Vec<String> = violations
                    .iter()
                    .map(|v| {
                        format!(
                            "`{}` is {} but the corresponding \
                             type parameter is {}",
                            v.arg_name, v.arg_variance, v.expected_variance
                        )
                    })
                    .collect();

                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Variance incompatibility in base class `{}`: {}",
                        entry.base_name,
                        details.join("; ")
                    ),
                    span: entry.span,
                    path: module.path.clone(),
                    help: Some(
                        "Each TypeVar argument must have the same variance \
                         as the corresponding type parameter in the base class."
                            .to_owned(),
                    ),
                    note: None,
                });
            }
        }
    }
}

/// Resolve through aliases and check variance compatibility using rich type args.
///
/// Returns `None` if the base cannot be resolved (not a known class or alias).
fn resolve_and_check_rich(
    base_name: &str,
    type_args: &[TypeArg],
    class_params: &HashMap<&str, Vec<&str>>,
    alias_info: &HashMap<&str, (&str, &[String])>,
    tv_variance: &HashMap<&str, Variance>,
) -> Option<Vec<VarianceViolation>> {
    // Direct class case: base_name is a known generic class.
    if let Some(params) = class_params.get(base_name) {
        let violations =
            check_rich_args_against_params(type_args, params, class_params, alias_info, tv_variance);
        return Some(violations);
    }

    // Alias case: expand the alias, substituting rich type args, then check recursively.
    let expanded = expand_alias_rich(base_name, type_args, alias_info, tv_variance, 0)?;
    resolve_and_check_rich(&expanded.0, &expanded.1, class_params, alias_info, tv_variance)
}

/// Expand one level of alias, substituting rich type args for the alias's free TypeVars.
/// Returns the resolved (base_name, type_args) after substitution, or None if not an alias.
fn expand_alias_rich(
    alias_name: &str,
    provided_args: &[TypeArg],
    alias_info: &HashMap<&str, (&str, &[String])>,
    tv_variance: &HashMap<&str, Variance>,
    depth: usize,
) -> Option<(String, Vec<TypeArg>)> {
    if depth > 10 {
        return None;
    }

    let &(target_base, alias_type_args) = alias_info.get(alias_name)?;

    // Find the alias's free TypeVars (those present in tv_variance).
    let free_tvs: Vec<&str> = alias_type_args
        .iter()
        .filter(|n| tv_variance.contains_key(n.as_str()))
        .map(String::as_str)
        .collect();

    // Build substitution: alias free TypeVar → provided rich type arg.
    let mut substitution: HashMap<&str, &TypeArg> = HashMap::new();
    for (free_tv, provided) in free_tvs.iter().zip(provided_args.iter()) {
        substitution.insert(*free_tv, provided);
    }

    // Apply substitution to alias_type_args to produce effective rich args.
    let effective_args: Vec<TypeArg> = alias_type_args
        .iter()
        .map(|arg| {
            if let Some(&replacement) = substitution.get(arg.as_str()) {
                replacement.clone()
            } else {
                TypeArg::Simple(arg.clone())
            }
        })
        .collect();

    // If target_base is itself an alias, expand recursively.
    if alias_info.contains_key(target_base) {
        return expand_alias_rich(target_base, &effective_args, alias_info, tv_variance, depth + 1);
    }

    Some((target_base.to_owned(), effective_args))
}

/// Check rich type args against the expected parameter variances.
///
/// For each type arg position, compute the effective variance of the argument
/// (which may be a nested generic) and compare it to the expected variance.
fn check_rich_args_against_params(
    type_args: &[TypeArg],
    param_names: &[&str],
    class_params: &HashMap<&str, Vec<&str>>,
    alias_info: &HashMap<&str, (&str, &[String])>,
    tv_variance: &HashMap<&str, Variance>,
) -> Vec<VarianceViolation> {
    let mut violations = Vec::new();
    for (type_arg, param_name) in type_args.iter().zip(param_names.iter()) {
        let Some(&expected_var) = tv_variance.get(*param_name) else {
            continue;
        };

        // Collect all leaf TypeVars and their composed variances.
        let mut leaves = Vec::new();
        collect_leaf_variances(type_arg, Variance::Covariant, class_params, alias_info, tv_variance, &mut leaves);

        for (leaf_name, effective_var) in leaves {
            // Bug 1 fix: invariant TypeVars are compatible with any position.
            if effective_var == Variance::Invariant {
                continue;
            }
            if effective_var != expected_var {
                violations.push(VarianceViolation {
                    arg_name: leaf_name,
                    arg_variance: effective_var,
                    expected_variance: expected_var,
                });
            }
        }
    }
    violations
}

/// Recursively collect leaf `TypeVar`s with their composed variance.
///
/// `accumulated` tracks the composed variance from the outer context down to
/// the current position. For a top-level arg, this starts as `Covariant`
/// (identity for composition).
fn collect_leaf_variances(
    type_arg: &TypeArg,
    accumulated: Variance,
    class_params: &HashMap<&str, Vec<&str>>,
    alias_info: &HashMap<&str, (&str, &[String])>,
    tv_variance: &HashMap<&str, Variance>,
    out: &mut Vec<(String, Variance)>,
) {
    match type_arg {
        TypeArg::Simple(name) => {
            if let Some(&var) = tv_variance.get(name.as_str()) {
                let effective = compose_variance(accumulated, var);
                out.push((name.clone(), effective));
            }
        }
        TypeArg::Subscript { base, args } => {
            // Look up the variance of each parameter in the nested generic class.
            if let Some(params) = class_params.get(base.as_str()) {
                for (inner_arg, param_name) in args.iter().zip(params.iter()) {
                    let param_var = tv_variance
                        .get(*param_name)
                        .copied()
                        .unwrap_or(Variance::Invariant);
                    let new_accumulated = compose_variance(accumulated, param_var);
                    collect_leaf_variances(inner_arg, new_accumulated, class_params, alias_info, tv_variance, out);
                }
            } else if let Some(&(target_base, alias_type_args)) = alias_info.get(base.as_str()) {
                // The base is an alias — expand it and recurse.
                let free_tvs: Vec<&str> = alias_type_args
                    .iter()
                    .filter(|n| tv_variance.contains_key(n.as_str()))
                    .map(String::as_str)
                    .collect();

                let mut substitution: HashMap<&str, &TypeArg> = HashMap::new();
                for (free_tv, provided) in free_tvs.iter().zip(args.iter()) {
                    substitution.insert(*free_tv, provided);
                }

                let effective_args: Vec<TypeArg> = alias_type_args
                    .iter()
                    .map(|arg| {
                        if let Some(&replacement) = substitution.get(arg.as_str()) {
                            replacement.clone()
                        } else {
                            TypeArg::Simple(arg.clone())
                        }
                    })
                    .collect();

                // Now check the expanded form against the target base.
                let expanded = TypeArg::Subscript {
                    base: target_base.to_owned(),
                    args: effective_args,
                };
                collect_leaf_variances(&expanded, accumulated, class_params, alias_info, tv_variance, out);
            }
        }
    }
}

/// Resolve through aliases and check variance compatibility.
///
/// Returns `None` if the base cannot be resolved (not a known class or alias).
/// Returns `Some(violations)` with details for each incompatible argument.
fn resolve_and_check(
    base_name: &str,
    type_arg_names: &[String],
    class_params: &HashMap<&str, Vec<&str>>,
    alias_info: &HashMap<&str, (&str, &[String])>,
    tv_variance: &HashMap<&str, Variance>,
) -> Option<Vec<VarianceViolation>> {
    // Direct class case: base_name is a known generic class.
    if let Some(params) = class_params.get(base_name) {
        let violations = check_args_against_params(type_arg_names, params, tv_variance);
        return Some(violations);
    }

    // Alias case: resolve through alias chain.
    resolve_alias_and_check(base_name, type_arg_names, class_params, alias_info, tv_variance, 0)
}

/// Compare type argument variances against expected parameter variances.
fn check_args_against_params(
    arg_names: &[String],
    param_names: &[&str],
    tv_variance: &HashMap<&str, Variance>,
) -> Vec<VarianceViolation> {
    let mut violations = Vec::new();
    for (arg_name, param_name) in arg_names.iter().zip(param_names.iter()) {
        let Some(&arg_var) = tv_variance.get(arg_name.as_str()) else {
            continue;
        };
        let Some(&expected_var) = tv_variance.get(*param_name) else {
            continue;
        };
        // Bug 1 fix: invariant TypeVars are compatible with any position.
        if arg_var == Variance::Invariant {
            continue;
        }
        if arg_var != expected_var {
            violations.push(VarianceViolation {
                arg_name: arg_name.clone(),
                arg_variance: arg_var,
                expected_variance: expected_var,
            });
        }
    }
    violations
}

/// Recursively resolve an alias and check the effective arguments against the
/// ultimate generic class's parameters.
///
/// `depth` guards against infinite alias chains.
fn resolve_alias_and_check(
    alias_name: &str,
    provided_args: &[String],
    class_params: &HashMap<&str, Vec<&str>>,
    alias_info: &HashMap<&str, (&str, &[String])>,
    tv_variance: &HashMap<&str, Variance>,
    depth: usize,
) -> Option<Vec<VarianceViolation>> {
    if depth > 10 {
        return None;
    }

    let &(target_base, alias_type_args) = alias_info.get(alias_name)?;

    // The alias's free TypeVars are the TypeVar names in alias_type_args that
    // are actual TypeVars (present in tv_variance). Build a substitution map
    // from the alias's free TypeVars to the provided arguments.
    let typevar_names: Vec<&str> = alias_type_args
        .iter()
        .filter(|n| tv_variance.contains_key(n.as_str()))
        .map(String::as_str)
        .collect();

    // Build substitution: alias free TypeVar -> provided arg.
    let mut substitution: HashMap<&str, &str> = HashMap::new();
    for (free_tv, provided) in typevar_names.iter().zip(provided_args.iter()) {
        substitution.insert(*free_tv, provided.as_str());
    }

    // Apply substitution to alias_type_args to get effective args for target_base.
    let effective_args: Vec<String> = alias_type_args
        .iter()
        .map(|arg| {
            substitution
                .get(arg.as_str())
                .map_or_else(|| arg.clone(), |s| (*s).to_owned())
        })
        .collect();

    // Now check against the target: is it a class or another alias?
    if let Some(params) = class_params.get(target_base) {
        let violations = check_args_against_params(&effective_args, params, tv_variance);
        return Some(violations);
    }

    // Target is another alias — resolve recursively.
    resolve_alias_and_check(
        target_base,
        &effective_args,
        class_params,
        alias_info,
        tv_variance,
        depth + 1,
    )
}
