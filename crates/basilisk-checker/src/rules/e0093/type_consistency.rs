//! Implements [BSK-E0093] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! `TypedDict` type consistency checks (PEP 589 §5).
//!
//! Validates assignments where the RHS is a `TypedDict`-typed variable:
//!
//! - `TypedDict` → `dict`: always an error
//! - `TypedDict` → `Mapping[str, T]`: error unless T is `object` or `Any`
//! - `TypedDict` → `TypedDict`: structural compatibility check

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, ResolvedModule};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic};
use crate::span_util::slice_span;

use super::CODE;

/// Check `TypedDict` type consistency for module-level assignments.
pub(super) fn check_typeddict_assignability(
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = &module.source;

    let td_classes: HashMap<&str, &ClassInfo> = module
        .classes
        .iter()
        .filter(|c| c.is_typed_dict)
        .map(|c| (c.name.as_str(), c))
        .collect();

    if td_classes.is_empty() {
        return;
    }

    let var_td_types = build_var_typeddict_map(&module.module_vars, source, &td_classes);

    for var in &module.module_vars {
        let Some(rhs_span) = var.rhs_span else {
            continue;
        };
        let Some(rhs_text) = slice_span(source, rhs_span) else {
            continue;
        };
        let rhs_name = rhs_text.trim();

        // RHS must be a simple variable name referencing a TypedDict-typed var.
        if !rhs_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        let Some(&rhs_td_name) = var_td_types.get(rhs_name) else {
            continue;
        };

        // Get the LHS annotation — either from this statement or a prior declaration.
        let ann_text = if let Some(ann_span) = var.annotation_span {
            slice_span(source, ann_span).map(str::trim)
        } else {
            // Reassignment: look up original type of this variable.
            var_td_types.get(var.name.as_str()).copied()
        };

        let Some(ann_text) = ann_text else {
            continue;
        };

        check_td_to_target(
            ann_text,
            rhs_td_name,
            &td_classes,
            var.name_span,
            &module.path,
            source,
            diagnostics,
        );
    }
}

/// Check an assignment where RHS is a `TypedDict` variable.
fn check_td_to_target(
    ann_text: &str,
    rhs_td_name: &str,
    td_classes: &HashMap<&str, &ClassInfo>,
    span: basilisk_resolver::Span,
    path: &str,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // TypedDict → dict[...]: always an error.
    if ann_text.starts_with("dict[") || ann_text == "dict" {
        emit_td_error(
            diagnostics,
            span,
            path,
            &format!("TypedDict `{rhs_td_name}` is not assignable to `{ann_text}`"),
            "A TypedDict is not consistent with any dict[...] type",
        );
        return;
    }

    // TypedDict → Mapping[str, T]: error unless T is object or Any.
    if let Some(val_type) = parse_mapping_value_type(ann_text) {
        if val_type != "object" && val_type != "Any" {
            emit_td_error(
                diagnostics,
                span,
                path,
                &format!("TypedDict `{rhs_td_name}` is not assignable to `{ann_text}`"),
                &format!(
                    "TypedDict is only assignable to Mapping[str, object], \
                     not Mapping[str, {val_type}]"
                ),
            );
        }
        return;
    }

    // TypedDict → TypedDict: structural compatibility.
    let Some(lhs_cls) = td_classes.get(ann_text) else {
        return;
    };
    let Some(rhs_cls) = td_classes.get(rhs_td_name) else {
        return;
    };
    if ann_text == rhs_td_name {
        return;
    }

    if let Some(detail) = check_structural_compat_with_classes(lhs_cls, rhs_cls, source, td_classes)
    {
        emit_td_error(
            diagnostics,
            span,
            path,
            &format!("TypedDict `{rhs_td_name}` is not assignable to `{ann_text}`: {detail}"),
            "TypedDict types use structural compatibility with invariant value types",
        );
    }
}

/// Emit a `TypedDict` assignability error.
fn emit_td_error(
    diagnostics: &mut Vec<Diagnostic>,
    span: basilisk_resolver::Span,
    path: &str,
    message: &str,
    help: &str,
) {
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        message.to_owned(),
        span,
        path,
        Some(help.to_owned()),
        Some("PEP 589: TypedDict type consistency rules".to_owned()),
    ));
}

/// Build a map from variable name to its `TypedDict` type name.
fn build_var_typeddict_map<'a>(
    vars: &'a [basilisk_resolver::VariableInfo],
    source: &'a str,
    td_classes: &HashMap<&str, &ClassInfo>,
) -> HashMap<&'a str, &'a str> {
    let mut map = HashMap::new();
    for var in vars {
        if !var.has_annotation {
            continue;
        }
        let Some(ann_span) = var.annotation_span else {
            continue;
        };
        let Some(ann_text) = slice_span(source, ann_span) else {
            continue;
        };
        let ann_text = ann_text.trim();
        if td_classes.contains_key(ann_text) {
            let _ = map.insert(var.name.as_str(), ann_text);
        }
    }
    map
}

/// Extract the value type T from `Mapping[str, T]`.
fn parse_mapping_value_type(ann: &str) -> Option<&str> {
    let inner = ann.strip_prefix("Mapping[")?.strip_suffix(']')?;
    let mut depth = 0i32;
    for (idx, ch) in inner.char_indices() {
        match ch {
            '[' | '(' => depth += 1,
            ']' | ')' => depth -= 1,
            ',' if depth == 0 => {
                return Some(inner.get(idx + 1..)?.trim());
            }
            _ => {}
        }
    }
    None
}

/// Unwrap `Required[T]` or `NotRequired[T]` wrappers to get the inner type.
fn unwrap_required(ann: &str) -> &str {
    if let Some(inner) = ann.strip_prefix("Required[") {
        inner.strip_suffix(']').unwrap_or(ann)
    } else if let Some(inner) = ann.strip_prefix("NotRequired[") {
        inner.strip_suffix(']').unwrap_or(ann)
    } else {
        ann
    }
}

/// Check structural compatibility with access to all `TypedDict` classes
/// for recursive structural comparison.
fn check_structural_compat_with_classes(
    lhs: &ClassInfo,
    rhs: &ClassInfo,
    source: &str,
    td_classes: &HashMap<&str, &ClassInfo>,
) -> Option<String> {
    let lhs_fields = extract_fields(lhs, source);
    let rhs_fields = extract_fields(rhs, source);

    let rhs_map: HashMap<&str, (&str, bool)> = rhs_fields
        .iter()
        .map(|(name, ann, req)| (*name, (*ann, *req)))
        .collect();

    for (lhs_name, lhs_ann, lhs_req) in &lhs_fields {
        let Some(&(rhs_ann, rhs_req)) = rhs_map.get(lhs_name) else {
            return Some(format!("missing key `{lhs_name}`"));
        };

        let lhs_type = unwrap_required(lhs_ann);
        let rhs_type = unwrap_required(rhs_ann);
        if lhs_type != rhs_type && !types_structurally_equal(lhs_type, rhs_type, source, td_classes)
        {
            return Some(format!(
                "value type for key `{lhs_name}` is `{rhs_type}`, expected `{lhs_type}`"
            ));
        }

        if lhs_req != &rhs_req {
            let (ls, rs) = if *lhs_req {
                ("required", "non-required")
            } else {
                ("non-required", "required")
            };
            return Some(format!(
                "key `{lhs_name}` is {rs} in source but {ls} in target"
            ));
        }
    }

    None
}

/// Check if two type annotations are structurally equal, considering
/// `TypedDict` structural equivalence.
fn types_structurally_equal(
    lhs: &str,
    rhs: &str,
    source: &str,
    td_classes: &HashMap<&str, &ClassInfo>,
) -> bool {
    if lhs == rhs {
        return true;
    }

    // If both are TypedDict names, check structural compatibility.
    if let (Some(lhs_cls), Some(rhs_cls)) = (td_classes.get(lhs), td_classes.get(rhs)) {
        return check_structural_compat_with_classes(lhs_cls, rhs_cls, source, td_classes)
            .is_none();
    }

    // For union types like `Literal[""] | Inner3`, split and compare components.
    if lhs.contains(" | ") && rhs.contains(" | ") {
        let lhs_parts: Vec<&str> = lhs.split(" | ").map(str::trim).collect();
        let rhs_parts: Vec<&str> = rhs.split(" | ").map(str::trim).collect();
        if lhs_parts.len() == rhs_parts.len() {
            return lhs_parts
                .iter()
                .zip(rhs_parts.iter())
                .all(|(l, r)| types_structurally_equal(l, r, source, td_classes));
        }
    }

    false
}

/// Extract field info: (name, `annotation_text`, `is_required`).
fn extract_fields<'a>(cls: &'a ClassInfo, source: &'a str) -> Vec<(&'a str, &'a str, bool)> {
    cls.attributes
        .iter()
        .filter_map(|attr| {
            let ann_text = slice_span(source, attr.annotation_span?)?.trim();
            let is_required = if ann_text.starts_with("Required[") {
                true
            } else if ann_text.starts_with("NotRequired[") {
                false
            } else {
                cls.is_typeddict_total
            };
            Some((attr.name.as_str(), ann_text, is_required))
        })
        .collect()
}
