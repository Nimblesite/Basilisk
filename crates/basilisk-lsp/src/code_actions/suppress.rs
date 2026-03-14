//! Suppression and severity-override code actions.
//!
//! Provides ergonomic line-level and file-level comment-based suppression for
//! any Basilisk diagnostic, plus a fallback `# type: ignore` action.

use std::collections::HashMap;

use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, Diagnostic, Position, Range, TextEdit, Url, WorkspaceEdit,
};

/// Append `  # type: ignore[CODE]` at the end of the diagnostic's source line.
pub(super) fn suppress_with_code(
    uri: &Url,
    diag: &Diagnostic,
    source: &str,
    code: &str,
) -> CodeAction {
    let comment = format!("  # type: ignore[{code}]");
    let insert_pos = line_end_position(diag, source);
    let mut changes = HashMap::new();
    let _ = changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: Range {
                start: insert_pos,
                end: insert_pos,
            },
            new_text: comment,
        }],
    );
    CodeAction {
        title: format!("Ignore `{code}` on this line"),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(true),
        ..Default::default()
    }
}

/// Append `  # type: warning[CODE]` to demote the error to a warning.
pub(super) fn demote_to_warning(
    uri: &Url,
    diag: &Diagnostic,
    source: &str,
    code: &str,
) -> CodeAction {
    let comment = format!("  # type: warning[{code}]");
    let insert_pos = line_end_position(diag, source);
    let mut changes = HashMap::new();
    let _ = changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: Range {
                start: insert_pos,
                end: insert_pos,
            },
            new_text: comment,
        }],
    );
    CodeAction {
        title: format!("Demote `{code}` to warning on this line"),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(false),
        ..Default::default()
    }
}

/// Insert `# basilisk: file-disabled[CODE]` at line 0 to disable for the whole file.
pub(super) fn disable_for_file(
    uri: &Url,
    diag: &Diagnostic,
    _source: &str,
    code: &str,
) -> CodeAction {
    let comment = format!("# basilisk: file-disabled[{code}]\n");
    let insert_pos = Position {
        line: 0,
        character: 0,
    };
    let mut changes = HashMap::new();
    let _ = changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: Range {
                start: insert_pos,
                end: insert_pos,
            },
            new_text: comment,
        }],
    );
    CodeAction {
        title: format!("Disable `{code}` for this file"),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(false),
        ..Default::default()
    }
}

/// Append `  # type: ignore` at the end of the diagnostic's source line.
pub(super) fn suppress_with_type_ignore(uri: &Url, diag: &Diagnostic, source: &str) -> CodeAction {
    let line_idx = usize::try_from(diag.range.start.line).unwrap_or(usize::MAX);
    let line_char_count = source
        .lines()
        .nth(line_idx)
        .map_or(0, |l| l.chars().count());
    let line_char_len = u32::try_from(line_char_count).unwrap_or(u32::MAX);
    let insert_pos = Position {
        line: diag.range.start.line,
        character: line_char_len,
    };

    let mut changes = HashMap::new();
    let _ = changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: Range {
                start: insert_pos,
                end: insert_pos,
            },
            new_text: "  # type: ignore".to_owned(),
        }],
    );
    CodeAction {
        title: "Suppress with `# type: ignore` (basilisk)".to_owned(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
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
