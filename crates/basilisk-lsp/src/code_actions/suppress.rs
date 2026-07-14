//! Implements [LSPARCH-FEATURES-CODEACTIONS]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-CODEACTIONS
//!
//! Suppression and severity-override code actions.
//!
//! Provides ergonomic line-level and file-level comment-based suppression for
//! any Basilisk diagnostic, plus a fallback `# type: ignore` action.

use std::collections::HashMap;

use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, Diagnostic, Position, Range, TextEdit, Url,
};

/// Build a `QUICKFIX` `CodeAction` that performs a single text insertion at `insert_pos`.
fn single_edit_action(
    uri: &Url,
    diag: &Diagnostic,
    insert_pos: Position,
    new_text: String,
    title: String,
    is_preferred: bool,
) -> CodeAction {
    let mut changes = HashMap::new();
    let _ = changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: Range {
                start: insert_pos,
                end: insert_pos,
            },
            new_text,
        }],
    );
    super::quickfix_action(title, diag, changes, is_preferred)
}

/// Append `  # type: ignore[CODE]` at the end of the diagnostic's source line.
pub(super) fn suppress_with_code(
    uri: &Url,
    diag: &Diagnostic,
    source: &str,
    code: &str,
) -> CodeAction {
    single_edit_action(
        uri,
        diag,
        line_end_position(diag, source),
        format!("  # type: ignore[{code}]"),
        format!("Ignore `{code}` on this line"),
        true,
    )
}

/// Append `  # type: warning[CODE]` to demote the error to a warning.
// Related to [AUTOFIX-ADOPTION] (error→warning demotion) but a DIFFERENT
// mechanism: this is a per-LINE inline comment, not the exact-file active-config
// override used by the adoption flow.
pub(super) fn demote_to_warning(
    uri: &Url,
    diag: &Diagnostic,
    source: &str,
    code: &str,
) -> CodeAction {
    single_edit_action(
        uri,
        diag,
        line_end_position(diag, source),
        format!("  # type: warning[{code}]"),
        format!("Demote `{code}` to warning on this line"),
        false,
    )
}

/// Insert `# basilisk: file-disabled[CODE]` at line 0 to disable for the whole file.
pub(super) fn disable_for_file(
    uri: &Url,
    diag: &Diagnostic,
    _source: &str,
    code: &str,
) -> CodeAction {
    single_edit_action(
        uri,
        diag,
        Position {
            line: 0,
            character: 0,
        },
        format!("# basilisk: file-disabled[{code}]\n"),
        format!("Disable `{code}` for this file"),
        false,
    )
}

/// Append `  # type: ignore` at the end of the diagnostic's source line.
pub(super) fn suppress_with_type_ignore(uri: &Url, diag: &Diagnostic, source: &str) -> CodeAction {
    single_edit_action(
        uri,
        diag,
        line_end_position(diag, source),
        "  # type: ignore".to_owned(),
        "Suppress with `# type: ignore` (basilisk)".to_owned(),
        false,
    )
}

/// Offer to disable a rule in the active project configuration.
///
/// This generates a command-based code action. The LSP client executes the
/// `basilisk.disableRule` command, which writes through the configuration
/// editor service to the project's `pyproject.toml` `[tool.basilisk]`.
pub(super) fn disable_in_project_config(uri: &Url, diag: &Diagnostic, code: &str) -> CodeAction {
    CodeAction {
        title: format!("Disable `{code}` in active project configuration"),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        command: Some(tower_lsp::lsp_types::Command {
            title: format!("Disable {code}"),
            command: "basilisk.disableRule".to_owned(),
            arguments: Some(vec![serde_json::json!({
                "rule": code,
                "severity": "off",
                "uri": uri,
            })]),
        }),
        is_preferred: Some(false),
        ..Default::default()
    }
}

// ── Helper ────────────────────────────────────────────────────────────────────

/// Get the end-of-line position for a diagnostic's line.
pub(super) fn line_end_position(diag: &Diagnostic, source: &str) -> Position {
    let line_idx = usize::try_from(diag.range.start.line).unwrap_or(usize::MAX);
    let line_char_count = source
        .lines()
        .nth(line_idx)
        .map_or(0, |l| l.chars().count());
    let line_char_len = u32::try_from(line_char_count).unwrap_or(u32::MAX);
    Position {
        line: diag.range.start.line,
        character: line_char_len,
    }
}
