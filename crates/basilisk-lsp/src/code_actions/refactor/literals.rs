//! Implements [LSPARCH-FEATURES-CODEACTIONS]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-CODEACTIONS
//!
//! Literal conversion refactoring actions: `dict()` to `{}` and `list()` to `[]`.

use tower_lsp::lsp_types::{CodeAction, Range, Url};

use super::helpers::{build_single_line_action, split_top_level_commas};

/// Offer to convert `dict()` to `{}` and/or `list()` to `[]` literal syntax.
///
/// Returns zero, one, or two actions depending on what patterns appear on the
/// line containing the cursor.
#[must_use]
pub(in crate::code_actions) fn convert_literals(
    uri: &Url,
    source: &str,
    range: &Range,
) -> Vec<CodeAction> {
    let mut actions = Vec::new();
    let line_idx = usize::try_from(range.start.line).unwrap_or(usize::MAX);
    let Some(line_text) = source.lines().nth(line_idx) else {
        return actions;
    };

    if let Some(action) = dict_call_to_literal(uri, line_text, range.start.line) {
        actions.push(action);
    }

    if let Some(action) = list_call_to_literal(uri, line_text, range.start.line) {
        actions.push(action);
    }

    actions
}

// ── dict() to {} ────────────────────────────────────────────────────────────

/// Find `dict(` on the line, ensuring it is a standalone call (not `OrderedDict(` etc.).
fn find_dict_call(line: &str) -> Option<usize> {
    let pos = line.find("dict(")?;

    // Ensure `dict` is not part of a longer identifier.
    if pos > 0 {
        let prev = line.as_bytes().get(pos - 1).copied().unwrap_or(b' ');
        if prev.is_ascii_alphanumeric() || prev == b'_' {
            return None;
        }
    }

    Some(pos)
}

/// Find the closing `)` that matches the paren at `start`, respecting nesting.
fn find_matching_paren(text: &str, start: usize) -> Option<usize> {
    let mut depth: u32 = 1;
    for (offset, ch) in text.get(start..)?.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(start + offset);
                }
            }
            _ => {}
        }
    }
    None
}

/// Parse keyword arguments from `dict(key=val, key2=val2)` into dict literal pairs.
fn parse_keyword_args(args: &str) -> Option<Vec<(String, String)>> {
    let parts = split_top_level_commas(args, false);
    let mut pairs = Vec::new();

    for part in parts {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        let eq_pos = trimmed.find('=')?;
        let key = trimmed.get(..eq_pos)?.trim();
        let value = trimmed.get(eq_pos + 1..)?.trim();

        // Keys must be valid identifiers (no spaces, no special chars).
        if key.is_empty()
            || value.is_empty()
            || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return None;
        }

        pairs.push((key.to_owned(), value.to_owned()));
    }

    Some(pairs)
}

/// Convert `dict(key=val, ...)` to `{"key": val, ...}`.
fn dict_call_to_literal(uri: &Url, line: &str, line_num: u32) -> Option<CodeAction> {
    let dict_start = find_dict_call(line)?;
    let args_start = dict_start + "dict(".len();
    let args_end = find_matching_paren(line, args_start)?;
    let args = line.get(args_start..args_end)?;

    let new_text = if args.trim().is_empty() {
        "{}".to_owned()
    } else {
        let pairs = parse_keyword_args(args)?;
        format_dict_literal(&pairs)
    };

    let start_char = u32::try_from(dict_start).unwrap_or(u32::MAX);
    let end_char = u32::try_from(args_end + 1).unwrap_or(u32::MAX);

    Some(build_single_line_action(
        uri,
        line_num,
        start_char,
        end_char,
        new_text,
        "Convert dict() to {} literal (basilisk)",
    ))
}

/// Format parsed keyword pairs as a dict literal string.
fn format_dict_literal(pairs: &[(String, String)]) -> String {
    let entries: Vec<String> = pairs
        .iter()
        .map(|(key, val)| format!("\"{key}\": {val}"))
        .collect();
    format!("{{{}}}", entries.join(", "))
}

// ── list() to [] ────────────────────────────────────────────────────────────

/// Find `list(` on the line, ensuring it is a standalone call.
fn find_list_call(line: &str) -> Option<usize> {
    let pos = line.find("list(")?;

    // Ensure `list` is not part of a longer identifier.
    if pos > 0 {
        let prev = line.as_bytes().get(pos - 1).copied().unwrap_or(b' ');
        if prev.is_ascii_alphanumeric() || prev == b'_' {
            return None;
        }
    }

    Some(pos)
}

/// Convert `list()` to `[]` or `list(iterable)` to `[*iterable]`.
fn list_call_to_literal(uri: &Url, line: &str, line_num: u32) -> Option<CodeAction> {
    let list_start = find_list_call(line)?;
    let args_start = list_start + "list(".len();
    let args_end = find_matching_paren(line, args_start)?;
    let args = line.get(args_start..args_end)?.trim();

    let new_text = if args.is_empty() {
        "[]".to_owned()
    } else {
        format!("[*{args}]")
    };

    let start_char = u32::try_from(list_start).unwrap_or(u32::MAX);
    let end_char = u32::try_from(args_end + 1).unwrap_or(u32::MAX);

    Some(build_single_line_action(
        uri,
        line_num,
        start_char,
        end_char,
        new_text,
        "Convert list() to [] literal (basilisk)",
    ))
}

// ── Shared helpers ──────────────────────────────────────────────────────────
// `build_single_line_action` and `split_top_level_commas` live in `helpers`.
