//! Ternary expression / if-else block conversion refactoring actions.

use std::collections::HashMap;

use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, Position, Range, TextEdit, Url, WorkspaceEdit,
};

use super::helpers::leading_indent_of_line;

/// Offer to convert between ternary expressions and if/else blocks.
///
/// Returns up to two actions: one for ternary-to-if/else and one for
/// if/else-to-ternary, depending on what patterns appear near the cursor.
#[must_use]
pub(in crate::code_actions) fn convert_ternary(
    uri: &Url,
    source: &str,
    range: &Range,
) -> Vec<CodeAction> {
    let mut actions = Vec::new();

    if let Some(action) = ternary_to_if_else(uri, source, range) {
        actions.push(action);
    }

    if let Some(action) = if_else_to_ternary(uri, source, range) {
        actions.push(action);
    }

    actions
}

/// Find the position of `if` keyword at the top level (not inside strings,
/// brackets, or parens) within a ternary expression value portion.
fn find_bare_keyword(text: &str, keyword: &str) -> Option<usize> {
    let mut depth: u32 = 0;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let bytes = text.as_bytes();

    for (idx, &byte) in bytes.iter().enumerate() {
        if in_single_quote {
            if byte == b'\'' {
                in_single_quote = false;
            }
            continue;
        }
        if in_double_quote {
            if byte == b'"' {
                in_double_quote = false;
            }
            continue;
        }
        match byte {
            b'\'' => in_single_quote = true,
            b'"' => in_double_quote = true,
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => {
                if text.get(idx..).is_some_and(|s| s.starts_with(keyword)) {
                    let before_ok = idx == 0
                        || bytes
                            .get(idx - 1)
                            .is_some_and(|b| !b.is_ascii_alphanumeric() && *b != b'_');
                    let after_ok = bytes
                        .get(idx + keyword.len())
                        .is_some_and(|b| !b.is_ascii_alphanumeric() && *b != b'_');
                    if before_ok && after_ok {
                        return Some(idx);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Parse `target = true_val if condition else false_val` from a line.
fn parse_ternary_line(line: &str) -> Option<(&str, &str, &str, &str)> {
    let eq_pos = line.find('=')?;
    // Reject augmented assignments (+=, -=, etc.) and == comparisons.
    if eq_pos > 0
        && line
            .as_bytes()
            .get(eq_pos - 1)
            .is_some_and(|b| !b.is_ascii_whitespace())
    {
        let prev = line.as_bytes().get(eq_pos - 1)?;
        if matches!(
            prev,
            b'+' | b'-' | b'*' | b'/' | b'%' | b'&' | b'|' | b'^' | b'!' | b'<' | b'>'
        ) {
            return None;
        }
    }
    if line.as_bytes().get(eq_pos + 1) == Some(&b'=') {
        return None;
    }

    let target = line.get(..eq_pos)?.trim();
    let value_part = line.get(eq_pos + 1..)?.trim();

    let if_pos = find_bare_keyword(value_part, "if")?;
    let after_if = value_part.get(if_pos + 2..)?.trim_start();
    let else_pos = find_bare_keyword(after_if, "else")?;

    let true_val = value_part.get(..if_pos)?.trim();
    let condition = after_if.get(..else_pos)?.trim();
    let false_val = after_if.get(else_pos + 4..)?.trim();

    if target.is_empty() || true_val.is_empty() || condition.is_empty() || false_val.is_empty() {
        return None;
    }

    Some((target, true_val, condition, false_val))
}

/// Convert `x = val_a if cond else val_b` to an if/else block.
fn ternary_to_if_else(uri: &Url, source: &str, range: &Range) -> Option<CodeAction> {
    let line_idx = usize::try_from(range.start.line).unwrap_or(usize::MAX);
    let line_text = source.lines().nth(line_idx)?;
    let indent = leading_indent_of_line(source, range.start.line);

    let (target, true_val, condition, false_val) = parse_ternary_line(line_text)?;

    let body_indent = format!("{indent}    ");
    let replacement = format!(
        "{indent}if {condition}:\n{body_indent}{target} = {true_val}\n{indent}else:\n{body_indent}{target} = {false_val}\n"
    );

    let line_len = u32::try_from(line_text.len()).unwrap_or(u32::MAX);
    let edit_range = Range {
        start: Position {
            line: range.start.line,
            character: 0,
        },
        end: Position {
            line: range.start.line,
            character: line_len,
        },
    };

    Some(build_action(
        uri,
        edit_range,
        replacement,
        "Convert ternary to if/else (basilisk)",
    ))
}

/// Parse a simple if/else assignment pattern spanning exactly 4 lines.
fn parse_if_else_block<'a>(lines: &[&'a str]) -> Option<(&'a str, &'a str, &'a str, &'a str)> {
    if lines.len() < 4 {
        return None;
    }

    let if_line = lines.first()?.trim();
    let then_line = lines.get(1)?.trim();
    let else_line = lines.get(2)?.trim();
    let else_body = lines.get(3)?.trim();

    let condition = if_line.strip_prefix("if ")?.strip_suffix(':')?;

    if else_line != "else:" {
        return None;
    }

    let then_eq = then_line.find('=')?;
    let else_eq = else_body.find('=')?;

    let then_target = then_line.get(..then_eq)?.trim();
    let else_target = else_body.get(..else_eq)?.trim();

    if then_target != else_target || then_target.is_empty() {
        return None;
    }

    let true_val = then_line.get(then_eq + 1..)?.trim();
    let false_val = else_body.get(else_eq + 1..)?.trim();

    if true_val.is_empty() || false_val.is_empty() {
        return None;
    }

    Some((then_target, true_val, condition, false_val))
}

/// Convert a simple if/else block to a ternary expression.
fn if_else_to_ternary(uri: &Url, source: &str, range: &Range) -> Option<CodeAction> {
    let line_idx = usize::try_from(range.start.line).unwrap_or(usize::MAX);
    let all_lines: Vec<&str> = source.lines().collect();
    let block = all_lines.get(line_idx..line_idx + 4)?;

    let indent = leading_indent_of_line(source, range.start.line);
    let (target, true_val, condition, false_val) = parse_if_else_block(block)?;

    let replacement = format!("{indent}{target} = {true_val} if {condition} else {false_val}");

    let last_line = range.start.line + 3;
    let last_line_len = all_lines
        .get(usize::try_from(last_line).unwrap_or(usize::MAX))
        .map_or(0, |l| u32::try_from(l.len()).unwrap_or(u32::MAX));

    let edit_range = Range {
        start: Position {
            line: range.start.line,
            character: 0,
        },
        end: Position {
            line: last_line,
            character: last_line_len,
        },
    };

    Some(build_action(
        uri,
        edit_range,
        replacement,
        "Convert if/else to ternary (basilisk)",
    ))
}

/// Build a `CodeAction` with a single text-edit replacement.
fn build_action(uri: &Url, edit_range: Range, new_text: String, title: &str) -> CodeAction {
    let mut changes = HashMap::new();
    let _ = changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: edit_range,
            new_text,
        }],
    );

    CodeAction {
        title: title.to_owned(),
        kind: Some(CodeActionKind::REFACTOR_REWRITE),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(false),
        ..Default::default()
    }
}
