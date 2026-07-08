//! Implements [LSPARCH-FEATURES-CODEACTIONS]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-CODEACTIONS
//!
//! Per-diagnostic quick-fix code actions.
//!
//! Each function maps a specific Basilisk diagnostic code to a targeted text
//! edit that resolves the problem.

// Implements [AUTOFIX-MASS] (Single-diagnostic scope) — these per-diagnostic
// edits are the unit that mass_fix collects and combines for File/Workspace
// scope. NOTE: the spec's `Fix` metadata struct ([AUTOFIX-METADATA]) with
// `safety`/`combinable`/`source` fields is NOT modelled here; each fn returns a
// bare `CodeAction`. Safety is tracked separately as static rule-code lists in
// mass_fix (SAFE_FIXABLE_RULES / ALL_FIXABLE_RULES). See report DEVIATION.

use std::collections::HashMap;

use tower_lsp::lsp_types::{CodeAction, Diagnostic, Position, Range, TextEdit, Url};

// ── Per-diagnostic quick fixes ───────────────────────────────────────────────

/// Insert `: Any` after the parameter name.
pub(super) fn fix_missing_param_annotation(uri: &Url, diag: &Diagnostic) -> CodeAction {
    single_insert(
        uri,
        diag,
        diag.range.end,
        ": Any",
        "Add `: Any` annotation (basilisk)",
    )
}

/// Insert ` -> None` after the closing `)` of the parameter list.
pub(super) fn fix_missing_return_annotation(uri: &Url, diag: &Diagnostic) -> CodeAction {
    single_insert(
        uri,
        diag,
        diag.range.end,
        " -> None",
        "Add `-> None` return type (basilisk)",
    )
}

/// Insert `: <inferred_type>` after the variable name.
///
/// The annotation is derived from the diagnostic message:
/// - "empty list"  → `: list[Any]`
/// - "empty dict"  → `: dict[str, Any]`
/// - anything else → `: Any` (catches the `None` RHS case)
pub(super) fn fix_missing_variable_annotation(uri: &Url, diag: &Diagnostic) -> CodeAction {
    let annotation = if diag.message.contains("empty list") {
        ": list[Any]"
    } else if diag.message.contains("empty dict") {
        ": dict[str, Any]"
    } else {
        ": Any"
    };
    single_insert(
        uri,
        diag,
        diag.range.end,
        annotation,
        &format!("Add `{annotation}` annotation (basilisk)"),
    )
}

/// Remove redundant type annotation (for BSK-W0050).
///
/// Example: `x: int = 42` → `x = 42`
/// Finds the colon on the diagnostic's source line and removes everything
/// from the colon to the equals sign (including the colon and space).
pub(super) fn fix_remove_redundant_annotation(
    uri: &Url,
    diag: &Diagnostic,
    source: &str,
) -> CodeAction {
    let line_idx = usize::try_from(diag.range.start.line).unwrap_or(usize::MAX);

    let line_text = source.lines().nth(line_idx).unwrap_or("");
    let colon_pos = line_text.find(':');
    let equals_pos = line_text.find('=');

    let (range_to_remove, new_text) = match (colon_pos, equals_pos) {
        (Some(colon), Some(eq)) if colon < eq => {
            let start = Position {
                line: diag.range.start.line,
                character: u32::try_from(colon).unwrap_or(u32::MAX),
            };
            let end = Position {
                line: diag.range.start.line,
                character: u32::try_from(eq).unwrap_or(u32::MAX),
            };
            (Range { start, end }, String::from(" "))
        }
        _ => (diag.range, String::new()),
    };

    let mut changes = HashMap::new();
    let _ = changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: range_to_remove,
            new_text,
        }],
    );
    super::quickfix_action(
        "Remove redundant type annotation (basilisk)".to_owned(),
        diag,
        changes,
        true,
    )
}

/// Insert `: Any` after the attribute name for missing class attribute annotations.
pub(super) fn fix_missing_attribute_annotation(uri: &Url, diag: &Diagnostic) -> CodeAction {
    single_insert(
        uri,
        diag,
        diag.range.end,
        ": Any",
        "Add `: Any` annotation to class attribute (basilisk)",
    )
}

// ── Shared helper ─────────────────────────────────────────────────────────────

/// Build a [`CodeAction`] that inserts `text` at `pos`.
pub(super) fn single_insert(
    uri: &Url,
    diag: &Diagnostic,
    pos: Position,
    text: &str,
    title: &str,
) -> CodeAction {
    let mut changes = HashMap::new();
    let _ = changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: Range {
                start: pos,
                end: pos,
            },
            new_text: text.to_owned(),
        }],
    );
    super::quickfix_action(title.to_owned(), diag, changes, true)
}
