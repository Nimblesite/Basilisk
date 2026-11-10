//! Implements [LSPARCH-FEATURES-CODEACTIONS]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-CODEACTIONS
//!
//! Type syntax conversion refactoring actions: Union/pipe and Optional/pipe-None.

use std::collections::HashMap;

use tower_lsp::lsp_types::{CodeAction, CodeActionKind, Position, Range, TextEdit, Url};

use super::helpers::{
    contains_bare_pipe, find_annotation_end, find_annotation_start, find_matching_bracket,
    split_on_bare_pipe, split_type_args,
};

// ── Convert Union syntax ────────────────────────────────────────────────────

/// Offer to convert between `Union[X, Y]` and `X | Y` syntax.
///
/// Returns zero, one, or two actions depending on what patterns appear on the
/// line containing the cursor.
// Implements [REFACTOR-CONVERT] — the "Union[X, Y] ↔ X | Y" (PEP 604) row.
#[must_use]
pub(in crate::code_actions) fn convert_union_syntax(
    uri: &Url,
    source: &str,
    range: &Range,
) -> Vec<CodeAction> {
    let mut actions = Vec::new();
    let line_idx = usize::try_from(range.start.line).unwrap_or(usize::MAX);
    let Some(line_text) = source.lines().nth(line_idx) else {
        return actions;
    };

    // Union[X, Y] -> X | Y
    if let Some(action) = union_to_pipe(uri, line_text, range.start.line) {
        actions.push(action);
    }

    // X | Y -> Union[X, Y]  (only in annotation context: after `:` or `->`)
    if let Some(action) = pipe_to_union(uri, line_text, range.start.line) {
        actions.push(action);
    }

    actions
}

/// Convert `Union[X, Y, ...]` to `X | Y | ...` on a single line.
fn union_to_pipe(uri: &Url, line: &str, line_num: u32) -> Option<CodeAction> {
    let start_byte = line.find("Union[")?;
    let after_bracket = start_byte + "Union[".len();
    let close_bracket = find_matching_bracket(line, after_bracket)?;

    let inner = line.get(after_bracket..close_bracket)?.trim();
    let parts: Vec<&str> = split_type_args(inner);
    if parts.len() < 2 {
        return None;
    }

    let pipe_text = parts
        .iter()
        .map(|p| p.trim())
        .collect::<Vec<_>>()
        .join(" | ");

    let start_char = u32::try_from(start_byte).unwrap_or(u32::MAX);
    // +1 for the closing bracket
    let end_char = u32::try_from(close_bracket + 1).unwrap_or(u32::MAX);

    let edit_range = Range {
        start: Position {
            line: line_num,
            character: start_char,
        },
        end: Position {
            line: line_num,
            character: end_char,
        },
    };

    let mut changes = HashMap::new();
    let _ = changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: edit_range,
            new_text: pipe_text,
        }],
    );

    Some(super::super::code_action_with_changes(
        "Convert Union[X, Y] to X | Y (basilisk)".to_owned(),
        CodeActionKind::REFACTOR_REWRITE,
        changes,
        false,
    ))
}

/// Convert `X | Y` in an annotation context to `Union[X, Y]`.
fn pipe_to_union(uri: &Url, line: &str, line_num: u32) -> Option<CodeAction> {
    // Only offer in annotation context: look for `: ` or `-> `.
    let annotation_start = find_annotation_start(line)?;
    let raw_annotation = line.get(annotation_start..)?;

    // Trim trailing assignment (`= ...`) and comments (`# ...`) to isolate the
    // type annotation. A bare `=` not inside brackets marks the end of the type.
    let annotation_len = find_annotation_end(raw_annotation);
    let annotation = raw_annotation.get(..annotation_len)?.trim_end();

    // Must contain a bare `|` that is not inside brackets.
    if !contains_bare_pipe(annotation) {
        return None;
    }

    let parts: Vec<&str> = split_on_bare_pipe(annotation);
    if parts.len() < 2 {
        return None;
    }

    let union_text = format!(
        "Union[{}]",
        parts
            .iter()
            .map(|p| p.trim())
            .collect::<Vec<_>>()
            .join(", ")
    );

    let start_char = u32::try_from(annotation_start).unwrap_or(u32::MAX);
    let actual_end = u32::try_from(annotation_start + annotation.len()).unwrap_or(u32::MAX);

    let edit_range = Range {
        start: Position {
            line: line_num,
            character: start_char,
        },
        end: Position {
            line: line_num,
            character: actual_end,
        },
    };

    let mut changes = HashMap::new();
    let _ = changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: edit_range,
            new_text: union_text,
        }],
    );

    Some(super::super::code_action_with_changes(
        "Convert X | Y to Union[X, Y] (basilisk)".to_owned(),
        CodeActionKind::REFACTOR_REWRITE,
        changes,
        false,
    ))
}

// ── Convert Optional syntax ─────────────────────────────────────────────────

/// Offer to convert between `Optional[X]` and `X | None`.
// Implements [REFACTOR-CONVERT] — the "Optional[X] ↔ X | None" (PEP 604) row.
#[must_use]
pub(in crate::code_actions) fn convert_optional_syntax(
    uri: &Url,
    source: &str,
    range: &Range,
) -> Vec<CodeAction> {
    let mut actions = Vec::new();
    let line_idx = usize::try_from(range.start.line).unwrap_or(usize::MAX);
    let Some(line_text) = source.lines().nth(line_idx) else {
        return actions;
    };

    if let Some(action) = optional_to_pipe_none(uri, line_text, range.start.line) {
        actions.push(action);
    }

    actions
}

/// Convert `Optional[X]` to `X | None`.
fn optional_to_pipe_none(uri: &Url, line: &str, line_num: u32) -> Option<CodeAction> {
    let start_byte = line.find("Optional[")?;
    let after_bracket = start_byte + "Optional[".len();
    let close_bracket = find_matching_bracket(line, after_bracket)?;

    let inner = line.get(after_bracket..close_bracket)?.trim();
    if inner.is_empty() {
        return None;
    }

    let replacement = format!("{inner} | None");

    let start_char = u32::try_from(start_byte).unwrap_or(u32::MAX);
    let end_char = u32::try_from(close_bracket + 1).unwrap_or(u32::MAX);

    let edit_range = Range {
        start: Position {
            line: line_num,
            character: start_char,
        },
        end: Position {
            line: line_num,
            character: end_char,
        },
    };

    let mut changes = HashMap::new();
    let _ = changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: edit_range,
            new_text: replacement,
        }],
    );

    Some(super::super::code_action_with_changes(
        "Convert Optional[X] to X | None (basilisk)".to_owned(),
        CodeActionKind::REFACTOR_REWRITE,
        changes,
        false,
    ))
}
