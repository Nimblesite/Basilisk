//! Extract variable and extract constant refactoring actions.

use std::collections::HashMap;

use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, Position, Range, TextEdit, Url,
};

use super::helpers::{last_import_line, leading_indent_of_line, selected_text};

/// Offer to extract the selected expression into a local variable.
///
/// Returns up to two actions:
/// 1. "Extract variable (basilisk)" — replaces only the selection.
/// 2. "Extract variable — replace all (basilisk)" — replaces every identical
///    occurrence in the file.
#[must_use]
pub(in crate::code_actions) fn extract_variable(
    uri: &Url,
    source: &str,
    range: &Range,
) -> Vec<CodeAction> {
    let Some(selected) = selected_text(source, range) else {
        return Vec::new();
    };
    if selected.is_empty() || selected.contains('\n') {
        return Vec::new();
    }

    let var_name = "extracted_value";
    let insert_line = range.start.line;
    let indent = leading_indent_of_line(source, insert_line);
    let insert_text = format!("{indent}{var_name} = {selected}\n");

    let single_action = build_single_action(uri, range, &insert_text, insert_line, var_name);

    let mut actions = vec![single_action];

    if let Some(replace_all) =
        build_replace_all_action(uri, source, &selected, &insert_text, insert_line, var_name)
    {
        actions.push(replace_all);
    }

    actions
}

/// Build the single-occurrence extract variable action.
fn build_single_action(
    uri: &Url,
    range: &Range,
    insert_text: &str,
    insert_line: u32,
    var_name: &str,
) -> CodeAction {
    let insert_pos = Position {
        line: insert_line,
        character: 0,
    };

    let edits = vec![
        TextEdit {
            range: Range {
                start: insert_pos,
                end: insert_pos,
            },
            new_text: insert_text.to_owned(),
        },
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

    super::super::code_action_with_changes(
        "Extract variable (basilisk)".to_owned(),
        CodeActionKind::new("refactor.extract.variable"),
        changes,
        false,
    )
}

/// Build the "replace all" extract variable action when multiple occurrences
/// of the selected expression exist in the source.
fn build_replace_all_action(
    uri: &Url,
    source: &str,
    selected: &str,
    insert_text: &str,
    insert_line: u32,
    var_name: &str,
) -> Option<CodeAction> {
    let occurrences = find_all_occurrences(source, selected);
    if occurrences.len() < 2 {
        return None;
    }

    let insert_pos = Position {
        line: insert_line,
        character: 0,
    };

    let mut edits = vec![TextEdit {
        range: Range {
            start: insert_pos,
            end: insert_pos,
        },
        new_text: insert_text.to_owned(),
    }];

    for &(start_byte, end_byte) in &occurrences {
        let start = byte_offset_to_lsp_position(source, start_byte);
        let end = byte_offset_to_lsp_position(source, end_byte);

        // All occurrences shift down by 1 line because of the assignment insertion.
        let shifted_start = Position {
            line: start.line + 1,
            character: start.character,
        };
        let shifted_end = Position {
            line: end.line + 1,
            character: end.character,
        };

        edits.push(TextEdit {
            range: Range {
                start: shifted_start,
                end: shifted_end,
            },
            new_text: var_name.to_owned(),
        });
    }

    let mut changes = HashMap::new();
    let _ = changes.insert(uri.clone(), edits);

    Some(super::super::code_action_with_changes(
        "Extract variable \u{2014} replace all (basilisk)".to_owned(),
        CodeActionKind::new("refactor.extract.variable"),
        changes,
        false,
    ))
}

/// Find all byte-offset ranges where `needle` appears in `source`.
fn find_all_occurrences(source: &str, needle: &str) -> Vec<(usize, usize)> {
    let mut results = Vec::new();
    let mut search_from = 0;
    while let Some(pos) = source.get(search_from..).and_then(|s| s.find(needle)) {
        let abs_start = search_from + pos;
        let abs_end = abs_start + needle.len();
        results.push((abs_start, abs_end));
        search_from = abs_end;
    }
    results
}

/// Convert a byte offset to an LSP `Position` (0-based line, 0-based character).
fn byte_offset_to_lsp_position(source: &str, offset: usize) -> Position {
    let prefix = source.get(..offset).unwrap_or(source);
    let line = prefix.chars().filter(|&c| c == '\n').count();
    let last_newline = prefix.rfind('\n').map_or(0, |p| p + 1);
    let character = offset.saturating_sub(last_newline);
    Position {
        line: u32::try_from(line).unwrap_or(u32::MAX),
        character: u32::try_from(character).unwrap_or(u32::MAX),
    }
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

    Some(super::super::code_action_with_changes(
        "Extract constant (basilisk)".to_owned(),
        CodeActionKind::new("refactor.extract.constant"),
        changes,
        false,
    ))
}
