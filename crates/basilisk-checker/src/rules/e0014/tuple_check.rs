//! Tuple assignment checking for BSK-E0014.
//!
//! Validates that re-assignments to tuple-annotated module variables are
//! compatible with the declared tuple type, checking element count and
//! element type for fixed-length tuples and homogeneous variable-length tuples.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, Severity};
use crate::span_util::slice_span;

use super::CODE;

/// Check re-assignments to tuple-annotated variables against the tuple literal RHS.
///
/// For example, `t1: tuple[int]` declared, then `t1 = (1, 2)` assigned — error because
/// `(1, 2)` has 2 elements but `tuple[int]` requires exactly 1.
pub(super) fn check_tuple_reassignments(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    let source = &module.source;
    let path = &module.path;

    // Build map: var name → annotation text, for vars annotated with tuple types.
    let mut tuple_annotations: std::collections::HashMap<&str, &str> =
        std::collections::HashMap::new();
    for var in &module.module_vars {
        if !var.has_annotation {
            continue;
        }
        let Some(ann_span) = var.annotation_span else {
            continue;
        };
        let Some(ann_text) = slice_span(source, ann_span) else {
            continue;
        };
        let ann_trimmed = ann_text.trim();
        if is_tuple_annotation(ann_trimmed) {
            let _ = tuple_annotations.insert(var.name.as_str(), ann_trimmed);
        }
    }

    if tuple_annotations.is_empty() {
        return;
    }

    // Check unannotated re-assignments to tuple-annotated variables.
    for var in &module.module_vars {
        if var.has_annotation {
            continue;
        }
        let Some(&ann_text) = tuple_annotations.get(var.name.as_str()) else {
            continue;
        };
        let Some(rhs_span) = var.rhs_span else {
            continue;
        };
        let Some(rhs_text) = slice_span(source, rhs_span) else {
            continue;
        };
        let rhs_trimmed = rhs_text.trim();

        if !is_tuple_literal(rhs_trimmed) {
            continue;
        }

        if let Some(msg) = check_tuple_literal_mismatch(rhs_trimmed, ann_text) {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Type mismatch: `{}` is annotated `{ann_text}` but assigned {msg}",
                    var.name
                ),
                span: var.name_span,
                path: path.to_owned(),
                help: Some("Ensure the tuple literal matches the annotated tuple type".to_owned()),
                note: Some(
                    "Basilisk checks that tuple literals are compatible with the declared tuple type"
                        .to_owned(),
                ),
            });
        }
    }
}

/// Returns `true` if the annotation is a simple tuple type (no starred components).
///
/// Skips complex types like `tuple[int, *tuple[str, ...]]` that require variadic analysis.
pub(super) fn is_tuple_annotation(ann: &str) -> bool {
    if !ann.starts_with("tuple[") || !ann.ends_with(']') {
        return false;
    }
    // Skip annotations with starred components (TypeVarTuple unpacks)
    match ann.get("tuple[".len()..ann.len().saturating_sub(1)) {
        Some(inner) => !inner.contains('*'),
        None => false,
    }
}

/// Returns `true` if the source text looks like a tuple literal `(...)`.
fn is_tuple_literal(text: &str) -> bool {
    text.starts_with('(') && text.ends_with(')')
}

/// Returns `Some(description)` when the tuple literal is incompatible with the annotation.
fn check_tuple_literal_mismatch(rhs: &str, ann: &str) -> Option<String> {
    let inner_ann = ann.strip_prefix("tuple[")?.strip_suffix(']')?;

    // Inner content of the tuple literal `(...)`.
    let rhs_inner = rhs.strip_prefix('(')?.strip_suffix(')')?;
    let rhs_elems = split_tuple_literal_elems(rhs_inner);

    // Homogeneous variable-length tuple: `tuple[T, ...]`
    if let Some(elem_type) = inner_ann.strip_suffix(", ...") {
        let elem_type = elem_type.trim();
        for elem in &rhs_elems {
            let elem = elem.trim();
            if !elem.is_empty() && !literal_elem_matches(elem, elem_type) {
                return Some(format!(
                    "a tuple containing `{elem}` (incompatible with `{elem_type}`)"
                ));
            }
        }
        return None;
    }

    // Empty tuple: `tuple[()]`
    if inner_ann.trim() == "()" {
        if !(rhs_elems.is_empty()
            || rhs_elems.len() == 1 && rhs_elems.first().is_some_and(|elem| elem.trim().is_empty()))
        {
            return Some(format!(
                "a tuple with {} element(s) (expected empty tuple)",
                rhs_elems.len()
            ));
        }
        return None;
    }

    // Fixed-length tuple: split annotation into element types.
    let ann_elems = split_type_list(inner_ann);

    // Count mismatch.
    if rhs_elems.len() != ann_elems.len() {
        return Some(format!(
            "a {}-element tuple (expected {} element(s))",
            rhs_elems.len(),
            ann_elems.len()
        ));
    }

    // Element type mismatches.
    for (idx, (rhs_elem, ann_elem)) in rhs_elems.iter().zip(ann_elems.iter()).enumerate() {
        let rhs_e = rhs_elem.trim();
        let ann_e = ann_elem.trim();
        if !rhs_e.is_empty() && !literal_elem_matches(rhs_e, ann_e) {
            return Some(format!(
                "a tuple with element {idx} `{rhs_e}` (expected type `{ann_e}`)"
            ));
        }
    }

    None
}

/// Split the inner content of a tuple literal by top-level commas.
/// Handles trailing commas: `1,` → `["1"]`, `1, 2` → `["1", "2"]`.
fn split_tuple_literal_elems(inner: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;
    for (idx, ch) in inner.char_indices() {
        match ch {
            '(' | '[' | '{' => depth = depth.saturating_add(1),
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                if let Some(part) = inner.get(start..idx) {
                    parts.push(part.trim());
                }
                start = idx + 1;
            }
            _ => {}
        }
    }
    let remainder = inner.get(start..).unwrap_or_default().trim();
    if !remainder.is_empty() {
        parts.push(remainder);
    }
    parts
}

/// Split a comma-separated type list respecting bracket nesting.
fn split_type_list(inner: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;
    for (idx, ch) in inner.char_indices() {
        match ch {
            '[' | '(' => depth = depth.saturating_add(1),
            ']' | ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                if let Some(part) = inner.get(start..idx) {
                    let part = part.trim();
                    if !part.is_empty() {
                        parts.push(part);
                    }
                }
                start = idx + 1;
            }
            _ => {}
        }
    }
    let remainder = inner.get(start..).unwrap_or_default().trim();
    if !remainder.is_empty() {
        parts.push(remainder);
    }
    parts
}

/// Returns `true` if a literal element (source text) is compatible with `expected_type`.
pub(super) fn literal_elem_matches(elem: &str, expected: &str) -> bool {
    let expected_lower = expected.to_ascii_lowercase();
    let expected_base = expected_lower
        .split('[')
        .next()
        .unwrap_or(expected_lower.as_str())
        .trim();

    if expected_base == "any" || expected_base == "object" {
        return true;
    }

    let is_int_lit = elem
        .chars()
        .all(|c| c.is_ascii_digit() || c == '_' || c == 'x' || c == 'o' || c == 'b')
        && elem.chars().next().is_some_and(|c| c.is_ascii_digit());
    let is_str_lit = (elem.starts_with('"') && elem.ends_with('"'))
        || (elem.starts_with('\'') && elem.ends_with('\''));
    let is_float_lit =
        elem.contains('.') && elem.chars().next().is_some_and(|c| c.is_ascii_digit());
    let is_bytes_lit = (elem.starts_with("b\"") || elem.starts_with("b'"))
        && (elem.ends_with('"') || elem.ends_with('\''));
    let is_bool_lit = elem == "True" || elem == "False";
    let is_none_lit = elem == "None";

    match expected_base {
        "int" => is_int_lit || is_bool_lit,
        "float" | "complex" => is_float_lit || is_int_lit || is_bool_lit,
        "str" => is_str_lit,
        "bytes" => is_bytes_lit,
        "bool" => is_bool_lit,
        "none" => is_none_lit,
        _ => true, // Unknown types: don't flag
    }
}
