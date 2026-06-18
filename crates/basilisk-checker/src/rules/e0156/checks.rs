//! Implements [BSK-E0156] from [CHKARCH-DIAG-TYPEDDICT-EXTRA-ITEMS].
//! See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPEDDICT-EXTRA-ITEMS
//!
//! Pure decision logic for the PEP 728 checks. Each function takes already
//! resolved [`TdModel`] data and returns a diagnostic message (or none), leaving
//! AST traversal and span extraction to the parent module.

use std::collections::HashMap;

use crate::rules::shared::is_type_compatible;

use super::model::{
    ancestor_closed_true, effective_extra, explicit_extra, transitive_fields, Qualifier, TdModel,
};

/// `a` and `b` are *consistent* (mutually assignable) — the invariance check a
/// non-read-only `extra_items` pseudo-item requires.
fn is_consistent(a: &str, b: &str) -> bool {
    is_type_compatible(a, b) && is_type_compatible(b, a)
}

// ---------------------------------------------------------------------------
// Class-definition legality
// ---------------------------------------------------------------------------

/// All PEP 728 class-definition violations for a single `TypedDict`.
pub(super) fn class_def_errors(model: &TdModel, map: &HashMap<&str, &TdModel>) -> Vec<String> {
    let mut errors = Vec::new();
    closed_literal_error(model, &mut errors);
    closed_inheritance_errors(model, map, &mut errors);
    extra_items_qualifier_error(model, &mut errors);
    change_extra_items_error(model, map, &mut errors);
    errors
}

/// `closed=` must be a literal `True`/`False`.
fn closed_literal_error(model: &TdModel, errors: &mut Vec<String>) {
    if model.closed.as_ref().is_some_and(|c| c.value.is_none()) {
        errors.push("Argument to `closed` must be a literal `True` or `False`".to_owned());
    }
}

/// `closed=False` is illegal under a closed/`extra_items` superclass;
/// `closed=True` is illegal under a non-read-only `extra_items` superclass.
fn closed_inheritance_errors(
    model: &TdModel,
    map: &HashMap<&str, &TdModel>,
    errors: &mut Vec<String>,
) {
    let Some(closed) = &model.closed else {
        return;
    };
    match closed.value {
        Some(false) => {
            if ancestor_closed_true_via_base(model, map) {
                errors.push(format!(
                    "Cannot set `closed=False` on `{}` when a superclass is `closed=True`",
                    model.name
                ));
            } else if explicit_extra(&model.name, map, false).is_some() {
                errors.push(format!(
                    "Cannot set `closed=False` on `{}` when a superclass sets `extra_items`",
                    model.name
                ));
            }
        }
        Some(true) if explicit_extra(&model.name, map, false).is_some_and(|e| !e.readonly) => {
            errors.push(format!(
                "Cannot set `closed=True` on `{}` when a superclass has non-read-only `extra_items`",
                model.name
            ));
        }
        Some(true) | None => {}
    }
}

fn ancestor_closed_true_via_base(model: &TdModel, map: &HashMap<&str, &TdModel>) -> bool {
    model
        .bases
        .iter()
        .any(|base| ancestor_closed_true(base, map))
}

/// `extra_items=` may not wrap `Required[...]` / `NotRequired[...]`.
fn extra_items_qualifier_error(model: &TdModel, errors: &mut Vec<String>) {
    if let Some(extra) = &model.extra_items {
        match extra.qualifier {
            Some(Qualifier::Required) => {
                errors.push("`extra_items` value cannot be `Required[...]`".to_owned());
            }
            Some(Qualifier::NotRequired) => {
                errors.push("`extra_items` value cannot be `NotRequired[...]`".to_owned());
            }
            None => {}
        }
    }
}

/// A subclass may redeclare `extra_items` only when the nearest superclass that
/// declares it does so as `ReadOnly[...]`.
fn change_extra_items_error(
    model: &TdModel,
    map: &HashMap<&str, &TdModel>,
    errors: &mut Vec<String>,
) {
    if model.extra_items.is_none() {
        return;
    }
    if explicit_extra(&model.name, map, false).is_some_and(|e| !e.readonly) {
        errors.push(format!(
            "Cannot change `extra_items` on `{}` unless it is `ReadOnly` in the superclass",
            model.name
        ));
    }
}

// ---------------------------------------------------------------------------
// Dict-literal construction
// ---------------------------------------------------------------------------

/// For a dict-literal item whose key is outside the target schema, the value
/// type must be assignable to the target's `extra_items` type. Returns `None`
/// when the target declares no explicit `extra_items` (gate) or the value is
/// compatible.
pub(super) fn dict_extra_value_error(
    target: &str,
    value_ty: &str,
    map: &HashMap<&str, &TdModel>,
) -> Option<String> {
    let extra = explicit_extra(target, map, true)?;
    if is_type_compatible(value_ty, &extra.ty) {
        return None;
    }
    Some(format!(
        "`{value_ty}` is not assignable to extra item type `{}` of `{target}`",
        extra.ty
    ))
}

// ---------------------------------------------------------------------------
// TypedDict-to-TypedDict assignability
// ---------------------------------------------------------------------------

/// Whether assigning `source` to `target` violates PEP 728 `extra_items`
/// assignability. Only evaluated when `target` declares an explicit
/// `extra_items` (so plain-`TypedDict` targets are never flagged).
pub(super) fn td_assign_error(
    source: &str,
    target: &str,
    map: &HashMap<&str, &TdModel>,
) -> Option<String> {
    let _gate = explicit_extra(target, map, true)?;
    let (te, ro_t) = effective_extra(target, map);
    let target_fields = transitive_fields(target, map);

    if let Some(msg) = source_field_error(source, target, &target_fields, &te, ro_t, map) {
        return Some(msg);
    }
    extra_pseudo_item_error(source, &te, ro_t, map)
}

/// Each source field outside the target schema must satisfy the target's
/// `extra_items` pseudo-item.
fn source_field_error(
    source: &str,
    target: &str,
    target_fields: &[super::model::TdField],
    te: &str,
    ro_t: bool,
    map: &HashMap<&str, &TdModel>,
) -> Option<String> {
    transitive_fields(source, map)
        .iter()
        .filter(|f| !target_fields.iter().any(|tf| tf.name == f.name))
        .find_map(|f| {
            if ro_t {
                (!is_type_compatible(&f.ty, te))
                    .then(|| format!("`{}` is not assignable to `{te}`", f.ty))
            } else if f.required {
                Some(format!(
                    "`{}` is required in `{source}` but is an extra item in `{target}`",
                    f.name
                ))
            } else {
                (!is_consistent(&f.ty, te))
                    .then(|| format!("`{}` is not consistent with `{te}`", f.ty))
            }
        })
}

/// The source's effective `extra_items` pseudo-item must satisfy the target's.
fn extra_pseudo_item_error(
    source: &str,
    te: &str,
    ro_t: bool,
    map: &HashMap<&str, &TdModel>,
) -> Option<String> {
    let (ts, _ro_s) = effective_extra(source, map);
    let ok = if ro_t {
        is_type_compatible(&ts, te)
    } else {
        is_consistent(&ts, te)
    };
    (!ok).then(|| format!("`{ts}` is not assignable to extra items type `{te}`"))
}

// ---------------------------------------------------------------------------
// Constructor calls
// ---------------------------------------------------------------------------

/// A keyword argument outside the declared schema is rejected unless the
/// `TypedDict` declares a non-read-only `extra_items` whose type the argument's
/// value matches. `value_ty` is `None` when the value type cannot be inferred.
pub(super) fn construction_extra_error(
    td: &str,
    key: &str,
    value_ty: Option<&str>,
    map: &HashMap<&str, &TdModel>,
) -> Option<String> {
    match explicit_extra(td, map, true) {
        None => Some(format!(
            "Unrecognized item `{key}` for `{td}`: extra items are not allowed"
        )),
        Some(extra) if extra.readonly => None,
        Some(extra) => match value_ty {
            Some(ty) if !is_type_compatible(ty, &extra.ty) => Some(format!(
                "Wrong type for extra item `{key}`: `{ty}` is not assignable to `{}`",
                extra.ty
            )),
            _ => None,
        },
    }
}
