//! Inline variable refactoring action.

use std::collections::HashMap;

use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, Position, Range, TextEdit, Url, WorkspaceEdit,
};

use super::helpers::leading_indent_of_line;

/// Offer to inline a simple variable assignment by replacing all subsequent
/// occurrences of the variable name with its assigned expression and deleting
/// the assignment line.
///
/// Only offered when the assignment is a simple `name = expr` (no tuple
/// unpacking, no augmented assignment) and the name appears at least once
/// after the assignment within the same indentation scope.
#[must_use]
pub(in crate::code_actions) fn inline_variable(
    uri: &Url,
    source: &str,
    range: &Range,
) -> Option<CodeAction> {
    let line_idx = usize::try_from(range.start.line).unwrap_or(usize::MAX);
    let all_lines: Vec<&str> = source.lines().collect();
    let line_text = all_lines.get(line_idx)?;

    let indent = leading_indent_of_line(source, range.start.line);
    let (name, expr) = parse_simple_assignment(line_text)?;

    let replacements = find_occurrences(&all_lines, line_idx, name, &indent);
    if replacements.is_empty() {
        return None;
    }

    let edits = build_edits(range.start.line, &replacements, name, expr);
    Some(build_inline_action(uri, edits))
}

/// Parse a simple `name = expr` from a line. Returns `None` for augmented
/// assignments, tuple unpacking, or empty expressions.
fn parse_simple_assignment(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim();
    let eq_pos = trimmed.find('=')?;

    // Reject augmented assignments (+=, -=, etc.).
    if eq_pos > 0 {
        let prev_byte = trimmed.as_bytes().get(eq_pos - 1)?;
        if matches!(
            prev_byte,
            b'+' | b'-' | b'*' | b'/' | b'%' | b'&' | b'|' | b'^' | b'!' | b'<' | b'>'
        ) {
            return None;
        }
    }

    // Reject == comparisons.
    if trimmed.as_bytes().get(eq_pos + 1) == Some(&b'=') {
        return None;
    }

    let name = trimmed.get(..eq_pos)?.trim();
    let expr = trimmed.get(eq_pos + 1..)?.trim();

    // Reject tuple unpacking (commas in the target).
    if name.contains(',') || name.contains('(') || name.contains('[') {
        return None;
    }

    // Name must be a valid identifier: alphanumeric + underscores, not starting
    // with a digit.
    if !is_valid_identifier(name) || expr.is_empty() {
        return None;
    }

    Some((name, expr))
}

/// Check that a string is a valid Python identifier.
fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_alphabetic() && first != '_' {
        return false;
    }
    chars.all(|ch| ch.is_alphanumeric() || ch == '_')
}

/// Find all lines after the assignment that contain the variable name as a
/// whole word and are within the same indentation scope.
fn find_occurrences(
    all_lines: &[&str],
    assignment_line: usize,
    name: &str,
    base_indent: &str,
) -> Vec<(usize, Vec<usize>)> {
    let mut results = Vec::new();

    for (relative_idx, line) in all_lines.iter().enumerate().skip(assignment_line + 1) {
        // Stop at lines with less indentation (scope boundary), unless blank.
        let trimmed = line.trim();
        if !trimmed.is_empty() && !line.starts_with(base_indent) {
            break;
        }

        let positions = find_whole_word_positions(line, name);
        if !positions.is_empty() {
            results.push((relative_idx, positions));
        }
    }

    results
}

/// Find all byte positions where `name` appears as a whole word in `text`.
fn find_whole_word_positions(text: &str, name: &str) -> Vec<usize> {
    let mut positions = Vec::new();
    let bytes = text.as_bytes();
    let name_len = name.len();
    let mut search_from = 0;

    while let Some(offset) = text.get(search_from..).and_then(|s| s.find(name)) {
        let abs_pos = search_from + offset;
        let before_ok = abs_pos == 0
            || bytes
                .get(abs_pos - 1)
                .is_some_and(|b| !b.is_ascii_alphanumeric() && *b != b'_');
        let after_ok = bytes
            .get(abs_pos + name_len)
            .is_none_or(|b| !b.is_ascii_alphanumeric() && *b != b'_');

        if before_ok && after_ok {
            positions.push(abs_pos);
        }

        search_from = abs_pos + name_len.max(1);
    }

    positions
}

/// Build the list of text edits: one to delete the assignment line, and one
/// per occurrence to replace the variable name with the expression.
fn build_edits(
    assignment_line: u32,
    replacements: &[(usize, Vec<usize>)],
    name: &str,
    expr: &str,
) -> Vec<TextEdit> {
    let name_len = u32::try_from(name.len()).unwrap_or(u32::MAX);

    // Delete the assignment line (including the trailing newline).
    let mut edits = vec![TextEdit {
        range: Range {
            start: Position {
                line: assignment_line,
                character: 0,
            },
            end: Position {
                line: assignment_line + 1,
                character: 0,
            },
        },
        new_text: String::new(),
    }];

    // Shift all subsequent line numbers down by 1 because the assignment
    // line is being deleted.
    for &(line_idx, ref positions) in replacements {
        let target_line = u32::try_from(line_idx).unwrap_or(u32::MAX);
        for &col in positions {
            let start_char = u32::try_from(col).unwrap_or(u32::MAX);
            edits.push(TextEdit {
                range: Range {
                    start: Position {
                        line: target_line,
                        character: start_char,
                    },
                    end: Position {
                        line: target_line,
                        character: start_char + name_len,
                    },
                },
                new_text: expr.to_owned(),
            });
        }
    }

    edits
}

/// Construct the final `CodeAction` from a list of text edits.
fn build_inline_action(uri: &Url, edits: Vec<TextEdit>) -> CodeAction {
    let mut changes = HashMap::new();
    let _ = changes.insert(uri.clone(), edits);

    CodeAction {
        title: "Inline variable (basilisk)".to_owned(),
        kind: Some(CodeActionKind::new("refactor.inline.variable")),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(false),
        ..Default::default()
    }
}
