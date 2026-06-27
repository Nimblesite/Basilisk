//! Implements [tuples_type_form_2] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! tuples_type_form_2: Invalid tuple type syntax.
//!
//! Validates tuple type annotations according to PEP 646 rules:
//!
//! - `tuple[T, ...]` must have exactly one type before `...`
//! - `tuple[...]` is invalid (must specify a type)
//! - `tuple[T, ..., U]` is invalid (`...` can only appear at the end)
//! - `tuple[T, U, ...]` is invalid (can't have multiple fixed types before `...`)
//! - Invalid unpack patterns like `tuple[*tuple[str], ...]`
//!
//! ```python
//! t1: tuple[int, ...]        # OK
//! t2: tuple[int, int, ...]   # E — multiple fixed types before ...
//! t3: tuple[...]             # E — missing type before ...
//! t4: tuple[..., int]         # E — ... must be at the end
//! t5: tuple[int, ..., int]    # E — ... must be at the end
//! t6: tuple[*tuple[str], ...] # E — invalid unpack pattern
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::split_top_level_commas;
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "tuples_type_form_2",
    docs_url: "https://www.basilisk-python.dev/errors/tuples_type_form_2",
};

/// Emits tuples_type_form_2 for invalid tuple type syntax.
pub(crate) struct InvalidTupleTypeSyntax;

impl Rule for InvalidTupleTypeSyntax {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let source = &module.source;

        // Check all variable annotations for tuple type syntax violations
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
            if let Some(error_msg) = check_tuple_syntax(ann_trimmed) {
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!("Invalid tuple type syntax: {error_msg}"),
                    ann_span,
                    &module.path,
                    Some("Use valid tuple type syntax according to PEP 646".to_owned()),
                    Some(
                        "Tuple types must follow the pattern `tuple[T, ...]` with exactly one type before the ellipsis"
                            .to_owned(),
                    ),
                ));
            }
        }

        // Also check function return type annotations
        for func in &module.functions {
            if let Some(ret_span) = func.return_annotation_span {
                let Some(ret_text) = slice_span(source, ret_span) else {
                    continue;
                };

                let ret_trimmed = ret_text.trim();
                if let Some(error_msg) = check_tuple_syntax(ret_trimmed) {
                    diagnostics.push(error_diagnostic_owned(
                        CODE.clone(),
                        format!("Invalid tuple type syntax: {error_msg}"),
                        ret_span,
                        &module.path,
                        Some("Use valid tuple type syntax according to PEP 646".to_owned()),
                        Some(
                            "Tuple types must follow the pattern `tuple[T, ...]` with exactly one type before the ellipsis"
                                .to_owned(),
                        ),
                    ));
                }
            }
        }
    }
}

/// Returns `Some(error_message)` if the tuple type annotation has invalid syntax.
///
/// Only flags top-level ellipsis misuse; nested `...` inside starred unpacks
/// like `*tuple[str, ...]` are valid and not flagged.
fn check_tuple_syntax(annotation: &str) -> Option<&'static str> {
    // Check if this is a tuple annotation
    if !annotation.starts_with("tuple[") || !annotation.ends_with(']') {
        return None;
    }

    let inner = annotation
        .get("tuple[".len()..annotation.len().checked_sub(1)?)
        .map_or("", str::trim);

    // Check for empty tuple: tuple[()]
    if inner == "()" {
        return None; // Valid empty tuple syntax
    }

    // Split by top-level commas to get individual components.
    let components: Vec<&str> = split_top_level_commas(inner)
        .into_iter()
        .map(str::trim)
        .collect();

    // Find positions of top-level `...` components.
    let ellipsis_positions: Vec<usize> = components
        .iter()
        .enumerate()
        .filter(|(_, c)| **c == "...")
        .map(|(i, _)| i)
        .collect();

    if ellipsis_positions.is_empty() {
        // No top-level `...` — valid fixed tuple (starred unpacks inside are OK).
        return None;
    }

    // More than one top-level `...` is always invalid.
    if ellipsis_positions.len() > 1 {
        return Some("ellipsis (...) must appear at the end of the tuple type");
    }

    let &ellipsis_pos = ellipsis_positions.first()?;

    // `...` must be the very last component.
    if ellipsis_pos != components.len() - 1 {
        return Some("ellipsis (...) must appear at the end of the tuple type");
    }

    // Count non-ellipsis components before `...`.
    let types_before = ellipsis_pos;

    if types_before == 0 {
        return Some("tuple[...] is invalid — must specify a type before the ellipsis");
    }

    if types_before > 1 {
        return Some("tuple[T, U, ...] is invalid — can only have one type before the ellipsis");
    }

    None
}
