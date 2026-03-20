//! Extract variable and extract constant refactoring actions.

use std::collections::HashMap;

use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, Position, Range, TextEdit, Url, WorkspaceEdit,
};

use super::helpers::{last_import_line, leading_indent_of_line, selected_text};

/// Offer to extract the selected expression into a local variable.
///
/// Inserts `extracted_value = <selection>` on the line before the current
/// statement and replaces the selection with `extracted_value`.
#[must_use]
pub(in crate::code_actions) fn extract_variable(
    uri: &Url,
    source: &str,
    range: &Range,
) -> Option<CodeAction> {
    let selected = selected_text(source, range)?;
    if selected.is_empty() || selected.contains('\n') {
        return None;
    }

    let var_name = "extracted_value";

    let insert_line = range.start.line;
    let indent = leading_indent_of_line(source, insert_line);
    let insert_text = format!("{indent}{var_name} = {selected}\n");

    let insert_pos = Position {
        line: insert_line,
        character: 0,
    };

    let edits = vec![
        // 1. Insert the new assignment before the current line.
        TextEdit {
            range: Range {
                start: insert_pos,
                end: insert_pos,
            },
            new_text: insert_text,
        },
        // 2. Replace the selected expression with the variable name.
        // The selection shifts down by one line because of the insertion above.
        TextEdit {
            range: Range {
                start: Position {
                    line: range.start.line + 1,
                    character: range.start.character,
                },
                end: Position {
                    line: range.end.line + 1,
                    character: range.end.character,
                },
            },
            new_text: var_name.to_owned(),
        },
    ];

    let mut changes = HashMap::new();
    let _ = changes.insert(uri.clone(), edits);

    Some(CodeAction {
        title: "Extract variable (basilisk)".to_owned(),
        kind: Some(CodeActionKind::new("refactor.extract.variable")),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(false),
        ..Default::default()
    })
}

/// Offer to extract the selected expression into a module-level constant.
///
/// Inserts `EXTRACTED_VALUE = <selection>` after the last import line and
/// replaces the selection with `EXTRACTED_VALUE`.
#[must_use]
pub(in crate::code_actions) fn extract_constant(
    uri: &Url,
    source: &str,
    range: &Range,
) -> Option<CodeAction> {
    let selected = selected_text(source, range)?;
    if selected.is_empty() || selected.contains('\n') {
        return None;
    }

    let const_name = "EXTRACTED_VALUE";
    let insert_line = last_import_line(source);
    let insert_text = format!("{const_name} = {selected}\n");

    let insert_pos = Position {
        line: insert_line,
        character: 0,
    };

    // Determine how many lines the insertion adds so we can adjust the
    // replacement range when the selection is on or after the insertion point.
    let selection_shift = u32::from(range.start.line >= insert_line);

    let edits = vec![
        TextEdit {
            range: Range {
                start: insert_pos,
                end: insert_pos,
            },
            new_text: insert_text,
        },
        TextEdit {
            range: Range {
                start: Position {
                    line: range.start.line + selection_shift,
                    character: range.start.character,
                },
                end: Position {
                    line: range.end.line + selection_shift,
                    character: range.end.character,
                },
            },
            new_text: const_name.to_owned(),
        },
    ];

    let mut changes = HashMap::new();
    let _ = changes.insert(uri.clone(), edits);

    Some(CodeAction {
        title: "Extract constant (basilisk)".to_owned(),
        kind: Some(CodeActionKind::new("refactor.extract.constant")),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(false),
        ..Default::default()
    })
}
