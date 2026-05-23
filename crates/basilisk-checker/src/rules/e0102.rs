//! BSK-E0102: Invalid `TypeVar` default referencing another `TypeVar`.
//!
//! PEP 696 specifies constraints on `TypeVar` defaults that reference other `TypeVars`:
//!
//! 1. **Ordering**: When `TypeVar` T2 has default=T1, T1 must appear before T2 in generic parameter list
//! 2. **Outer scope references**: `TypeVar` cannot use a `TypeVar` from outer scope as default
//! 3. **Bound compatibility**: When T2 has default=T1, T1's bound must be a subtype of T2's bound
//! 4. **Constraint superset**: When T2 has default=T1 and T2 has constraints, T1's constraints must be a subset of T2's constraints
//!
//! ```python
//! from typing import TypeVar
//!
//! # Ordering violation
//! T2 = TypeVar("T2", default=T1)  # E — T1 not defined yet
//! T1 = TypeVar("T1")
//!
//! # Outer scope violation  
//! class Outer:
//!     T1 = TypeVar("T1")
//!     class Inner:
//!         T2 = TypeVar("T2", default=T1)  # E — T1 from outer scope
//!
//! # Bound compatibility violation
//! X1 = TypeVar("X1", bound=int)
//! Invalid1 = TypeVar("Invalid1", default=X1, bound=str)  # E — int is not a subtype of str
//!
//! # Constraint superset violation
//! Y1 = TypeVar("Y1", int, str)
//! Invalid2 = TypeVar("Invalid2", bool, complex, default=Y1)  # E — {bool, complex} is not a superset of {int, str}
//! ```

use std::collections::{HashMap, HashSet};

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

use crate::rules::shared::is_numeric_subtype;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0102",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0102",
};

/// Check if type `t1` is a subtype of type `t2` for bound compatibility.
fn is_subtype_for_bound(t1: &str, t2: &str) -> bool {
    is_numeric_subtype(t1, t2)
}

/// Check if constraints `c1` are a subset of constraints `c2`.
fn is_constraint_subset(c1: &[String], c2: &[String]) -> bool {
    // All constraints in c1 must be in c2
    c1.iter().all(|constraint| c2.contains(constraint))
}

/// Emits BSK-E0102 for `TypeVar` default referential violations.
pub(crate) struct TypeVarDefaultReferential;

/// Format a list of constraint names as backtick-quoted, comma-separated string.
fn format_constraints(constraints: &[String]) -> String {
    constraints
        .iter()
        .map(|c| format!("`{c}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Check ordering: default `TypeVar` must appear before this `TypeVar`.
fn check_ordering(
    tv: &basilisk_resolver::TypeVarCallInfo,
    default_name: &str,
    order_index: &HashMap<&str, usize>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let (Some(&default_pos), Some(&tv_pos)) = (
        order_index.get(default_name),
        order_index.get(tv.name.as_str()),
    ) {
        if default_pos >= tv_pos {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "`TypeVar` `{}` has `default={default_name}` but `{default_name}` \
                     must appear before `{}` in the parameter list",
                    tv.name, tv.name
                ),
                tv.span,
                path,
                Some(format!(
                    "Reorder the type parameters so that `{default_name}` comes before `{}`",
                    tv.name
                )),
                Some(
                    "When a TypeVar's default references another TypeVar, \
                     the referenced TypeVar must appear earlier in the parameter list"
                        .to_owned(),
                ),
            ));
        }
    }
}

/// Check bound compatibility: default's bound must be a subtype of this `TypeVar`'s bound.
fn check_bound_compatibility(
    tv: &basilisk_resolver::TypeVarCallInfo,
    default_tv: &basilisk_resolver::TypeVarCallInfo,
    default_name: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !tv.has_bound || !default_tv.has_bound {
        return;
    }
    if let (Some(ref tv_bound), Some(ref default_bound)) =
        (&tv.bound_type_name, &default_tv.bound_type_name)
    {
        if !is_subtype_for_bound(default_bound, tv_bound) {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "`TypeVar` `{}` has `default={default_name}` but \
                     `{default_name}`'s bound `{default_bound}` is not a subtype \
                     of `{}`'s bound `{tv_bound}`",
                    tv.name, tv.name
                ),
                tv.span,
                path,
                Some(format!(
                    "The default TypeVar's bound must be a subtype of this TypeVar's bound; \
                     `{default_bound}` is not a subtype of `{tv_bound}`"
                )),
                Some(
                    "When T2 has default=T1, T1's bound must be a subtype of T2's bound".to_owned(),
                ),
            ));
        }
    }
}

/// Check constraint compatibility between `TypeVar`s with defaults.
fn check_constraint_compatibility(
    tv: &basilisk_resolver::TypeVarCallInfo,
    default_tv: &basilisk_resolver::TypeVarCallInfo,
    default_name: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Case 3a: Both have constraints - default's constraints must be a subset
    if !tv.constraint_type_names.is_empty()
        && !default_tv.constraint_type_names.is_empty()
        && !is_constraint_subset(&default_tv.constraint_type_names, &tv.constraint_type_names)
    {
        let default_constraints = format_constraints(&default_tv.constraint_type_names);
        let tv_constraints = format_constraints(&tv.constraint_type_names);
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "`TypeVar` `{}` has `default={default_name}` but \
                 `{default_name}`'s constraints {{{default_constraints}}} are not a \
                 subset of `{}`'s constraints {{{tv_constraints}}}",
                tv.name, tv.name
            ),
            tv.span,
            path,
            Some(
                "The default TypeVar's constraints must be a subset of this TypeVar's constraints"
                    .to_owned(),
            ),
            Some(
                "When T2 has default=T1 and T2 has constraints, \
                 T1's constraints must be a subset of T2's constraints"
                    .to_owned(),
            ),
        ));
    }

    // Case 3b: Default has bound, this TypeVar has constraints
    if !tv.constraint_type_names.is_empty() && default_tv.has_bound {
        check_default_bound_vs_constraints(tv, default_tv, default_name, path, diagnostics);
    }

    // Case 3c: Default has constraints, this TypeVar has bound
    if tv.has_bound && !default_tv.constraint_type_names.is_empty() {
        check_default_constraints_vs_bound(tv, default_tv, default_name, path, diagnostics);
    }
}

/// Case 3b: Default has bound, this `TypeVar` has constraints.
fn check_default_bound_vs_constraints(
    tv: &basilisk_resolver::TypeVarCallInfo,
    default_tv: &basilisk_resolver::TypeVarCallInfo,
    default_name: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(ref default_bound) = default_tv.bound_type_name else {
        return;
    };
    let is_compatible = tv
        .constraint_type_names
        .iter()
        .any(|constraint| is_subtype_for_bound(default_bound, constraint));

    if !is_compatible {
        let tv_constraints = format_constraints(&tv.constraint_type_names);
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "`TypeVar` `{}` has `default={default_name}` but \
                 `{default_name}`'s bound `{default_bound}` is incompatible with \
                 `{}`'s constraints {{{tv_constraints}}}",
                tv.name, tv.name
            ),
            tv.span,
            path,
            Some(
                "The default TypeVar's bound must be compatible with at least one of this TypeVar's constraints"
                    .to_owned(),
            ),
            Some(
                "When T2 has default=T1 and T2 has constraints, \
                 T1's bound must be compatible with at least one constraint of T2"
                    .to_owned(),
            ),
        ));
    }
}

/// Case 3c: Default has constraints, this ```TypeVar``` has bound.
fn check_default_constraints_vs_bound(
    tv: &basilisk_resolver::TypeVarCallInfo,
    default_tv: &basilisk_resolver::TypeVarCallInfo,
    default_name: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(ref tv_bound) = tv.bound_type_name else {
        return;
    };
    let all_compatible = default_tv
        .constraint_type_names
        .iter()
        .all(|constraint| is_subtype_for_bound(constraint, tv_bound));

    if !all_compatible {
        let default_constraints = format_constraints(&default_tv.constraint_type_names);
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "`TypeVar` `{}` has `default={default_name}` but \
                 `{default_name}`'s constraints {{{default_constraints}}} are not all \
                 subtypes of `{}`'s bound `{tv_bound}`",
                tv.name, tv.name
            ),
            tv.span,
            path,
            Some(
                "All of the default TypeVar's constraints must be subtypes of this TypeVar's bound"
                    .to_owned(),
            ),
            Some(
                "When T2 has default=T1 and T2 has a bound, \
                 all of T1's constraints must be subtypes of T2's bound"
                    .to_owned(),
            ),
        ));
    }
}

impl Rule for TypeVarDefaultReferential {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let typevar_by_name: HashMap<&str, &basilisk_resolver::TypeVarCallInfo> = module
            .typevar_calls
            .iter()
            .filter(|tv| !tv.is_typevartuple && !tv.is_paramspec)
            .map(|tv| (tv.name.as_str(), tv))
            .collect();

        let typevar_names: HashSet<&str> = typevar_by_name.keys().copied().collect();

        let order_index: HashMap<&str, usize> = module
            .typevar_calls
            .iter()
            .filter(|tv| !tv.is_typevartuple && !tv.is_paramspec)
            .enumerate()
            .map(|(i, tv)| (tv.name.as_str(), i))
            .collect();

        for tv in &module.typevar_calls {
            if tv.is_typevartuple || tv.is_paramspec || !tv.has_default {
                continue;
            }
            let Some(ref default_name) = tv.default_type_name else {
                continue;
            };
            if !typevar_names.contains(default_name.as_str()) {
                continue;
            }
            let Some(default_tv) = typevar_by_name.get(default_name.as_str()) else {
                continue;
            };

            check_ordering(tv, default_name, &order_index, &module.path, diagnostics);
            check_bound_compatibility(tv, default_tv, default_name, &module.path, diagnostics);
            check_constraint_compatibility(tv, default_tv, default_name, &module.path, diagnostics);
        }
    }
}
