//! Implements [`narrowing_typeis_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `narrowing_typeis_2`: `TypeIs` narrows to a type inconsistent with the input type.
//!
//! Per the typing spec: "It is an error to narrow to a type that is not
//! consistent with the input type." For `TypeIs`, the narrowed type must
//! be a subtype of the input type.

use basilisk_resolver::ResolvedModule;

use super::Rule;
use crate::diagnostic::{error_diag_help_note, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

const CODE: ErrorCode = ErrorCode {
    code: "narrowing_typeis_2",
    docs_url: "https://www.basilisk-python.dev/errors/narrowing_typeis_2",
};

/// Emits `narrowing_typeis_2` when a function returns `TypeIs[X]` but `X` is not
/// consistent with the first parameter type.
///
/// Implements [TYPEINF-NARROWING-TYPEIS] — the PEP 742 consistency precondition:
/// because `TypeIs` narrows bidirectionally, the narrowed type `X` must be a
/// subtype of (consistent with) the input parameter type.
pub(crate) struct TypeIsInconsistentNarrowing;

/// Extract the inner type from `TypeIs[X]` or `TypeGuard[X]`. Returns the inner type text.
fn extract_inner_type(ann_text: &str) -> Option<&str> {
    let prefix = "TypeIs[";
    let start = ann_text.find(prefix)?;
    let inner_start = start + prefix.len();
    let rest = ann_text.get(inner_start..)?;
    // Parsed annotations overwhelmingly end at this subscript. This covers
    // both simple and nested arguments (`TypeIs[list[int]]`) without a second
    // bracket walk; retain the general matcher for qualified/trailing forms.
    if let Some(inner) = rest.strip_suffix(']') {
        return Some(inner);
    }
    // Find matching closing bracket (handle nested brackets)
    let mut depth = 1u32;
    let mut end_pos = 0;
    for (idx, ch) in rest.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    end_pos = idx;
                    break;
                }
            }
            _ => {}
        }
    }
    if depth == 0 {
        rest.get(..end_pos)
    } else {
        None
    }
}

/// Returns `true` if the type text contains a `TypeVar` (single uppercase letter
/// or a known TypeVar-like name). When `TypeVars` are present, we can't statically
/// determine consistency without full type inference, so we assume consistent.
fn contains_typevar(type_text: &str) -> bool {
    if !type_text.as_bytes().iter().any(u8::is_ascii_uppercase) {
        return false;
    }
    // Check for single-letter uppercase names that are TypeVars
    // Also check common TypeVar patterns like T, T_A, T_co, etc.
    for segment in type_text.split(&['[', ']', ',', ' ']) {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        // Single uppercase letter (T, U, V, etc.)
        if segment.len() == 1
            && segment
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_uppercase())
        {
            return true;
        }
        // TypeVar patterns like T_A, T_co, T_contra
        if segment.starts_with("T_") || segment.starts_with("T1") || segment.starts_with("T2") {
            return true;
        }
    }
    false
}

/// Check if `narrowed` type is consistent with `input` type.
/// Returns `true` if they are consistent (no error).
///
/// For `TypeIs`, the narrowed type must be assignable to the input type.
/// This means narrowed must be a subtype of input.
fn is_consistent(narrowed: &str, input: &str) -> bool {
    let narrowed = narrowed.trim();
    let input = input.trim();

    // `object` accepts anything
    if input == "object" {
        return true;
    }

    // Same type is always consistent
    if narrowed == input {
        return true;
    }

    // `Any` is consistent with anything
    if input == "Any" || narrowed == "Any" {
        return true;
    }

    // Check numeric tower: int <: float <: complex
    if input == "float" && (narrowed == "int" || narrowed == "bool") {
        return true;
    }
    if input == "complex" && (narrowed == "int" || narrowed == "float" || narrowed == "bool") {
        return true;
    }
    if input == "int" && narrowed == "bool" {
        return true;
    }

    // If either type contains TypeVars, we can't determine consistency
    // without full type inference - assume consistent. Keep this after the
    // concrete scalar fast paths so ordinary lowercase builtins do no token
    // splitting.
    if contains_typevar(narrowed) || contains_typevar(input) {
        return true;
    }

    // For generic types like list[X] vs list[Y], check if it's the same base
    // Lists, sets, dicts are invariant, so list[int] is NOT a subtype of list[object]
    if let (Some(n_base), Some(i_base)) = (generic_base(narrowed), generic_base(input)) {
        // Same generic base - invariant containers are not subtypes
        if n_base == i_base {
            // For invariant types (list, dict, set), exact match is required
            // We already checked full string equality above, so if we're here
            // the type args differ → not consistent
            return false;
        }
    }

    // For simple types with no obvious subtype relationship, reject
    // This handles cases like str vs int
    false
}

/// Split a generic type `Base[Args]` into `(base, args)` text.
fn generic_base(type_text: &str) -> Option<&str> {
    let bracket = type_text.find('[')?;
    type_text.get(..bracket)
}

impl Rule for TypeIsInconsistentNarrowing {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let source = &module.source;

        for func in &module.functions {
            // Must have a return annotation span.
            let Some(ann_span) = func.return_annotation_span else {
                continue;
            };

            // Extract annotation text.
            let Some(ann_text) = slice_span(source, ann_span) else {
                continue;
            };

            // Only check TypeIs (not TypeGuard - TypeGuard has no consistency requirement)
            if !ann_text.contains("TypeIs[") {
                continue;
            }

            // Extract the inner narrowed type.
            let Some(narrowed_type) = extract_inner_type(ann_text) else {
                continue;
            };

            // Find the first non-self/cls parameter (the one being narrowed).
            let first_param = func
                .parameters
                .iter()
                .find(|param| param.name != "self" && param.name != "cls");

            let Some(param) = first_param else {
                continue;
            };

            // Get the parameter's annotation text.
            let Some(param_ann_span) = param.annotation_span else {
                continue;
            };

            let Some(param_type) = slice_span(source, param_ann_span) else {
                continue;
            };

            // Check consistency.
            if !is_consistent(narrowed_type, param_type) {
                diagnostics.push(error_diag_help_note(
                    CODE.clone(),
                    format!(
                        "`TypeIs[{narrowed_type}]` narrows to a type inconsistent with parameter type `{param_type}`"
                    ),
                    ann_span,
                    &module.path,
                    format!(
                        "The narrowed type `{narrowed_type}` must be consistent with the input type `{param_type}`"
                    ),
                    "Per the typing spec, TypeIs requires the narrowed type to be \
                     consistent with the input type",
                ));
            }
        }
    }
}
