//! Implements [LSPARCH-FEATURES-CODEACTIONS]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-CODEACTIONS
//!
//! Inline function refactoring action: replace a function call with the
//! function's single-expression body, substituting arguments for parameters.

use std::collections::HashMap;

use tower_lsp::lsp_types::{CodeAction, CodeActionKind, Position, Range, TextEdit, Url};

/// Offer to inline a function call when the called function is defined in the
/// same file and has a single `return expr` body.
///
/// Returns `None` if the function cannot be found, has multiple statements, or
/// the argument count does not match the parameter count.
// Implements [REFACTOR-INLINE-FUNC-ALGO] — resolve the call (step 1), require
// a single `return expr` body (step 2), substitute args for params (step 3),
// and replace the call expression with the substituted body (step 4). Per the
// spec, this initial implementation supports only single-expression bodies.
#[must_use]
pub(in crate::code_actions) fn inline_function_call(
    uri: &Url,
    source: &str,
    range: &Range,
) -> Option<CodeAction> {
    let line_idx = usize::try_from(range.start.line).unwrap_or(usize::MAX);
    let line_text = source.lines().nth(line_idx)?;

    let cursor_col = usize::try_from(range.start.character).unwrap_or(0);
    let (call_start, call_end, func_name, call_args) = parse_call_at_cursor(line_text, cursor_col)?;

    let (params, return_expr) = find_single_return_def(source, func_name)?;

    if params.len() != call_args.len() {
        return None;
    }

    let substituted = substitute_params(&return_expr, &params, &call_args);

    let edit_range = Range {
        start: Position {
            line: range.start.line,
            character: u32::try_from(call_start).unwrap_or(u32::MAX),
        },
        end: Position {
            line: range.start.line,
            character: u32::try_from(call_end).unwrap_or(u32::MAX),
        },
    };

    Some(build_inline_action(uri, edit_range, substituted))
}

/// Identify the function call at or near the cursor position.
/// Returns `(call_start, call_end, function_name, arguments)`.
fn parse_call_at_cursor(
    line: &str,
    cursor_col: usize,
) -> Option<(usize, usize, &str, Vec<String>)> {
    // Walk backwards from cursor to find the start of an identifier.
    let bytes = line.as_bytes();
    let ident_start = find_identifier_start(bytes, cursor_col)?;

    // Walk forward from ident_start to find the end of the identifier.
    let ident_end = find_identifier_end(bytes, ident_start);

    let func_name = line.get(ident_start..ident_end)?;
    if func_name.is_empty() {
        return None;
    }

    // Expect an opening paren immediately after the identifier.
    if bytes.get(ident_end) != Some(&b'(') {
        return None;
    }

    let close_paren = find_matching_paren(line, ident_end + 1)?;
    let args_str = line.get(ident_end + 1..close_paren)?;
    let call_args = split_args(args_str);

    Some((ident_start, close_paren + 1, func_name, call_args))
}

/// Walk backwards from `pos` to find the start of a Python identifier.
fn find_identifier_start(bytes: &[u8], pos: usize) -> Option<usize> {
    let mut start = pos.min(bytes.len().saturating_sub(1));

    // If cursor is on a non-identifier character, try one position back.
    if bytes
        .get(start)
        .is_some_and(|b| !b.is_ascii_alphanumeric() && *b != b'_')
    {
        start = start.checked_sub(1)?;
    }

    while start > 0
        && bytes
            .get(start - 1)
            .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
    {
        start -= 1;
    }

    // First character must not be a digit.
    if bytes.get(start).is_some_and(u8::is_ascii_digit) {
        return None;
    }

    Some(start)
}

/// Walk forward from `start` to find the end of a Python identifier.
fn find_identifier_end(bytes: &[u8], start: usize) -> usize {
    let mut end = start;
    while bytes
        .get(end)
        .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
    {
        end += 1;
    }
    end
}

/// Find the closing `)` that matches the paren at position `after_open`.
fn find_matching_paren(text: &str, after_open: usize) -> Option<usize> {
    let mut depth: u32 = 1;
    for (offset, ch) in text.get(after_open..)?.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(after_open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

/// Split function arguments at top-level commas.
fn split_args(text: &str) -> Vec<String> {
    if text.trim().is_empty() {
        return Vec::new();
    }

    let mut args = Vec::new();
    let mut depth: u32 = 0;
    let mut start = 0;

    for (idx, ch) in text.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                if let Some(part) = text.get(start..idx) {
                    args.push(part.trim().to_owned());
                }
                start = idx + 1;
            }
            _ => {}
        }
    }

    if let Some(part) = text.get(start..) {
        let trimmed = part.trim();
        if !trimmed.is_empty() {
            args.push(trimmed.to_owned());
        }
    }

    args
}

/// Search the source for a `def <name>(params):` with a single `return expr`
/// body. Returns `(parameter_names, return_expression)`.
fn find_single_return_def(source: &str, name: &str) -> Option<(Vec<String>, String)> {
    let def_prefix = format!("def {name}(");
    let all_lines: Vec<&str> = source.lines().collect();

    for (idx, line) in all_lines.iter().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with(&def_prefix) {
            continue;
        }

        let params = extract_def_params(trimmed, &def_prefix)?;
        let return_expr = extract_single_return(&all_lines, idx, line)?;

        return Some((params, return_expr));
    }

    None
}

/// Extract parameter names from a `def name(a, b, c):` line.
fn extract_def_params(trimmed: &str, def_prefix: &str) -> Option<Vec<String>> {
    let after_open = trimmed.get(def_prefix.len()..)?;
    let close_pos = after_open.find(')')?;
    let params_str = after_open.get(..close_pos)?;

    if params_str.trim().is_empty() {
        return Some(Vec::new());
    }

    let params = params_str
        .split(',')
        .map(|p| {
            let p = p.trim();
            // Strip type annotations: `x: int` -> `x`, `x: int = 0` -> `x`.
            p.split(':')
                .next()
                .unwrap_or(p)
                .split('=')
                .next()
                .unwrap_or(p)
                .trim()
                .to_owned()
        })
        .collect();

    Some(params)
}

/// Check that the function body (lines after the def) contains exactly one
/// `return expr` statement and return the expression.
// Implements [REFACTOR-INLINE-FUNC-ALGO] step 2 — validate the body is a
// single `return expr` (the only form supported by the initial implementation).
fn extract_single_return(all_lines: &[&str], def_idx: usize, def_line: &str) -> Option<String> {
    let def_indent_len = def_line.len() - def_line.trim_start().len();
    let body_indent_len = def_indent_len + 4;

    let mut body_statements = Vec::new();

    for line in all_lines.iter().skip(def_idx + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let leading_ws = line.len() - trimmed.len();
        if leading_ws < body_indent_len {
            break;
        }

        body_statements.push(trimmed);
    }

    // Must have exactly one statement, and it must be a return.
    if body_statements.len() != 1 {
        return None;
    }

    let stmt = body_statements.first()?;
    let expr = stmt.strip_prefix("return ")?;

    if expr.trim().is_empty() {
        return None;
    }

    Some(expr.trim().to_owned())
}

/// Substitute parameter names with call arguments in the return expression,
/// replacing only whole-word occurrences.
fn substitute_params(expr: &str, params: &[String], args: &[String]) -> String {
    let mut result = expr.to_owned();

    for (param, arg) in params.iter().zip(args.iter()) {
        result = replace_whole_word(&result, param, arg);
    }

    result
}

/// Replace all whole-word occurrences of `word` with `replacement` in `text`.
fn replace_whole_word(text: &str, word: &str, replacement: &str) -> String {
    let bytes = text.as_bytes();
    let word_len = word.len();
    let mut result = String::with_capacity(text.len());
    let mut search_from = 0;

    while let Some(offset) = text.get(search_from..).and_then(|s| s.find(word)) {
        let abs_pos = search_from + offset;
        let before_ok = abs_pos == 0
            || bytes
                .get(abs_pos - 1)
                .is_some_and(|b| !b.is_ascii_alphanumeric() && *b != b'_');
        let after_ok = bytes
            .get(abs_pos + word_len)
            .is_none_or(|b| !b.is_ascii_alphanumeric() && *b != b'_');

        if let Some(prefix) = text.get(search_from..abs_pos) {
            result.push_str(prefix);
        }

        if before_ok && after_ok {
            result.push_str(replacement);
        } else if let Some(original) = text.get(abs_pos..abs_pos + word_len) {
            result.push_str(original);
        }

        search_from = abs_pos + word_len;
    }

    if let Some(remainder) = text.get(search_from..) {
        result.push_str(remainder);
    }

    result
}

/// Build the final `CodeAction` for inlining a function call.
fn build_inline_action(uri: &Url, edit_range: Range, new_text: String) -> CodeAction {
    let mut changes = HashMap::new();
    let _ = changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: edit_range,
            new_text,
        }],
    );

    super::super::code_action_with_changes(
        "Inline function (basilisk)".to_owned(),
        CodeActionKind::new("refactor.inline.function"),
        changes,
        false,
    )
}
