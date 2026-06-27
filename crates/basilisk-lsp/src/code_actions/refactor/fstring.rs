//! Implements [LSPARCH-FEATURES-CODEACTIONS]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-CODEACTIONS
//!
//! F-string conversion refactoring actions: f-string to `.format()` and vice versa.

use tower_lsp::lsp_types::{CodeAction, Range, Url};

use super::helpers::{build_single_line_action, split_top_level_commas};

/// Offer to convert between f-string and `.format()` syntax.
///
/// Returns zero, one, or two actions depending on what patterns appear on the
/// line containing the cursor.
// Implements [REFACTOR-CONVERT] — the "f-string ↔ .format()" conversion row.
#[must_use]
pub(in crate::code_actions) fn convert_fstring(
    uri: &Url,
    source: &str,
    range: &Range,
) -> Vec<CodeAction> {
    let mut actions = Vec::new();
    let line_idx = usize::try_from(range.start.line).unwrap_or(usize::MAX);
    let Some(line_text) = source.lines().nth(line_idx) else {
        return actions;
    };

    if let Some(action) = fstring_to_format(uri, line_text, range.start.line) {
        actions.push(action);
    }

    if let Some(action) = format_to_fstring(uri, line_text, range.start.line) {
        actions.push(action);
    }

    actions
}

/// Find the byte offset and quote character of an f-string on the line.
///
/// Returns `(start_of_f, quote_char)` where `start_of_f` is the byte offset
/// of the `f` prefix.
fn find_fstring(line: &str) -> Option<(usize, char)> {
    for prefix in ["f\"", "f'"] {
        if let Some(pos) = line.find(prefix) {
            let quote = prefix.as_bytes().get(1).copied().unwrap_or(b'"');
            return Some((pos, char::from(quote)));
        }
    }
    None
}

/// Extract interpolation expressions from an f-string body.
///
/// Returns the list of expressions and the body with `{expr}` replaced by `{}`.
fn extract_interpolations(body: &str) -> (String, Vec<String>) {
    let mut result = String::with_capacity(body.len());
    let mut expressions = Vec::new();
    let mut chars = body.char_indices().peekable();

    while let Some((_, ch)) = chars.next() {
        if ch == '{' {
            let mut depth: u32 = 1;
            let mut expr = String::new();
            for (_, inner_ch) in chars.by_ref() {
                match inner_ch {
                    '{' => {
                        depth += 1;
                        expr.push(inner_ch);
                    }
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                        expr.push(inner_ch);
                    }
                    _ => expr.push(inner_ch),
                }
            }
            expressions.push(expr);
            result.push_str("{}");
        } else {
            result.push(ch);
        }
    }

    (result, expressions)
}

/// Convert `f"hello {name}"` to `"hello {}".format(name)`.
fn fstring_to_format(uri: &Url, line: &str, line_num: u32) -> Option<CodeAction> {
    let (fstring_start, quote) = find_fstring(line)?;

    // Skip past `f"` to find the closing quote.
    let body_start = fstring_start + 2;
    let body_end = line.get(body_start..)?.find(quote)? + body_start;
    let body = line.get(body_start..body_end)?;

    let (replaced_body, expressions) = extract_interpolations(body);
    if expressions.is_empty() {
        return None;
    }

    let args = expressions.join(", ");
    let new_text = format!("{quote}{replaced_body}{quote}.format({args})");

    let start_char = u32::try_from(fstring_start).unwrap_or(u32::MAX);
    // +1 for the closing quote
    let end_char = u32::try_from(body_end + 1).unwrap_or(u32::MAX);

    Some(build_single_line_action(
        uri,
        line_num,
        start_char,
        end_char,
        new_text,
        "Convert f-string to .format() (basilisk)",
    ))
}

// ── .format() to f-string ───────────────────────────────────────────────────

/// Find the string and `.format(...)` call on a line.
///
/// Returns `(start_byte, quote_char, string_body, format_args, total_end_byte)`.
fn find_format_call(line: &str) -> Option<(usize, char, &str, &str, usize)> {
    let format_pos = line.find(".format(")?;

    // Walk backwards from `.format(` to find the opening quote.
    let before_dot = line.get(..format_pos)?;
    let (string_start, quote) = find_string_start(before_dot)?;

    let body_start = string_start + 1;
    let body = before_dot.get(body_start..)?;

    // Find the matching `)` for `.format(`.
    let args_start = format_pos + ".format(".len();
    let args_end = find_matching_paren(line, args_start)?;
    let args = line.get(args_start..args_end)?;

    Some((string_start, quote, body, args, args_end + 1))
}

/// Find the start of a string literal by scanning backwards for a quote.
fn find_string_start(before_dot: &str) -> Option<(usize, char)> {
    for quote in ['"', '\''] {
        if before_dot.ends_with(quote) {
            // Find the opening quote (skip the closing one).
            let inner = before_dot.get(..before_dot.len() - 1)?;
            let open_pos = inner.rfind(quote)?;
            return Some((open_pos, quote));
        }
    }
    None
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

/// Replace `{}` placeholders with f-string interpolations from the arguments.
fn replace_placeholders(body: &str, args: &[&str]) -> String {
    let mut result = String::with_capacity(body.len());
    let mut arg_idx: usize = 0;
    let mut chars = body.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' && chars.peek() == Some(&'}') {
            let _ = chars.next(); // consume '}'
            let arg = args.get(arg_idx).unwrap_or(&"");
            result.push('{');
            result.push_str(arg.trim());
            result.push('}');
            arg_idx += 1;
        } else {
            result.push(ch);
        }
    }
    result
}

/// Convert `"hello {}".format(name)` to `f"hello {name}"`.
fn format_to_fstring(uri: &Url, line: &str, line_num: u32) -> Option<CodeAction> {
    let (string_start, quote, body, args_str, total_end) = find_format_call(line)?;

    let args: Vec<&str> = split_top_level_commas(args_str, true);
    if args.is_empty() {
        return None;
    }

    let new_body = replace_placeholders(body, &args);
    let new_text = format!("f{quote}{new_body}{quote}");

    let start_char = u32::try_from(string_start).unwrap_or(u32::MAX);
    let end_char = u32::try_from(total_end).unwrap_or(u32::MAX);

    Some(build_single_line_action(
        uri,
        line_num,
        start_char,
        end_char,
        new_text,
        "Convert .format() to f-string (basilisk)",
    ))
}

// ── Shared helpers ──────────────────────────────────────────────────────────
// `build_single_line_action` and `split_top_level_commas` live in `helpers`.
