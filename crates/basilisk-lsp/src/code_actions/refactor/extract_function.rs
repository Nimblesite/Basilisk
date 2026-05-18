//! Extract function/method refactoring.
//!
//! Extracts a selected block of complete statements into a new function,
//! replacing the selection with a call to the new function.  Performs basic
//! data-flow analysis to determine parameters and return values.

use std::collections::HashMap;

use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, Position, Range, TextEdit, Url,
};

use super::helpers::{leading_indent_of_line, selected_text};

/// Offer to extract the selected statements into a new function.
///
/// Requirements:
/// - Selection must span one or more complete lines.
/// - Selection must not be empty.
/// - Selection must not contain `yield`, `break`, or `continue` (unextractable).
///
/// The extracted function is placed immediately before the current function
/// (if inside one) or at the current indentation level.  If the enclosing
/// function is `async`, the extracted function is also `async` and called
/// with `await`.  If the selection is inside a method (first param is
/// `self`/`cls`), the extraction produces a method with `self`/`cls`.
#[must_use]
pub(in crate::code_actions) fn extract_function(
    uri: &Url,
    source: &str,
    range: &Range,
) -> Option<CodeAction> {
    let selected = selected_text(source, range)?;
    if selected.trim().is_empty() {
        return None;
    }

    // Must span at least one complete line.
    if range.start.line == range.end.line && range.end.character == range.start.character {
        return None;
    }

    let lines: Vec<&str> = source.lines().collect();
    let start_line = usize::try_from(range.start.line).unwrap_or(usize::MAX);
    let end_line = usize::try_from(range.end.line).unwrap_or(usize::MAX);

    // Snap to complete lines.
    let end_line = if range.end.character == 0 && end_line > 0 {
        end_line.saturating_sub(1)
    } else {
        end_line
    };
    if start_line > end_line || end_line >= lines.len() {
        return None;
    }

    // Collect the selected lines.
    let selected_lines: Vec<&str> = lines
        .get(start_line..=end_line)
        .unwrap_or_default()
        .to_vec();
    if selected_lines.is_empty() {
        return None;
    }

    // Reject selections containing yield, break, or continue — these cannot
    // be extracted into a separate function without changing semantics.
    if contains_unextractable_keyword(&selected_lines) {
        return None;
    }

    // Detect context: is the enclosing function async?  Is it a method?
    let context = detect_enclosing_context(source, start_line);

    // Determine the body indentation.
    let body_indent = leading_indent_of_line(source, range.start.line);
    let func_indent = compute_enclosing_indent(source, start_line);

    // Perform data-flow analysis to find parameters and return values.
    let analysis = analyze_data_flow(&selected_lines, &lines, start_line, end_line);

    // Build parameter list, prepending self/cls for methods.
    let params_str = build_params_string(&analysis.params, &context);

    // Build the extracted function.
    let func_name = "extracted_function";
    let func_body = build_function_body(&selected_lines, &body_indent, &func_indent);
    let return_stmt = build_return_statement(&analysis.returns, &func_indent);

    let async_kw = if context.is_async { "async " } else { "" };
    let mut func_def =
        format!("{func_indent}{async_kw}def {func_name}({params_str}):\n{func_body}");
    if !return_stmt.is_empty() {
        func_def.push_str(&return_stmt);
    }
    // Formatting pass: ensure PEP 8 blank-line separation.
    // Top-level functions need 2 blank lines; nested need 1.
    let separator = if func_indent.is_empty() { "\n\n" } else { "\n" };
    func_def.push_str(separator);

    // Build the call expression (with await if async).
    let args_str = build_args_string(&analysis.params, &context);
    let call_expr = build_call_expression(
        func_name,
        &args_str,
        &analysis.returns,
        &body_indent,
        &context,
    );

    // Find insertion point: before the enclosing function/class, or at the
    // start of the selection if at module level.
    let insert_line = find_insertion_point(source, start_line);

    // Calculate line shift: the function definition adds N lines above the
    // selection, so the selection range shifts down.
    let func_line_count = u32::try_from(func_def.lines().count()).unwrap_or(0);
    let select_start = u32::try_from(start_line).unwrap_or(0);
    let select_end = u32::try_from(end_line).unwrap_or(0);
    let insert_u32 = u32::try_from(insert_line).unwrap_or(0);
    let shift = if insert_u32 <= select_start {
        func_line_count
    } else {
        0
    };

    let edits = build_extract_edits(
        insert_u32,
        func_def,
        select_start + shift,
        select_end + shift,
        call_expr,
    );

    let mut changes = HashMap::new();
    let _ = changes.insert(uri.clone(), edits);

    Some(super::super::code_action_with_changes(
        "Extract function (basilisk)".to_owned(),
        CodeActionKind::new("refactor.extract.function"),
        changes,
        false,
    ))
}

/// Build the text edits for an extract-function refactoring.
fn build_extract_edits(
    insert_line: u32,
    func_def: String,
    replace_start: u32,
    replace_end: u32,
    call_expr: String,
) -> Vec<TextEdit> {
    vec![
        // Insert the new function definition.
        TextEdit {
            range: Range {
                start: Position {
                    line: insert_line,
                    character: 0,
                },
                end: Position {
                    line: insert_line,
                    character: 0,
                },
            },
            new_text: func_def,
        },
        // Replace the selected lines with the call expression.
        TextEdit {
            range: Range {
                start: Position {
                    line: replace_start,
                    character: 0,
                },
                end: Position {
                    line: replace_end + 1,
                    character: 0,
                },
            },
            new_text: call_expr,
        },
    ]
}

// ── Data flow analysis ───────────────────────────────────────────────────────

/// Result of analyzing data flow through the selected statements.
struct DataFlowResult {
    /// Variables read inside the selection that are defined outside (→ params).
    params: Vec<String>,
    /// Variables written inside the selection that are read after it (→ returns).
    returns: Vec<String>,
}

/// Analyze which names flow in (parameters) and out (return values) of the
/// selected statements.
fn analyze_data_flow(
    selected: &[&str],
    all_lines: &[&str],
    start: usize,
    end: usize,
) -> DataFlowResult {
    let assigned_inside = collect_assigned_names(selected);
    let referenced_inside = collect_referenced_names(selected);

    // Names referenced inside but not assigned inside → parameters.
    let params: Vec<String> = referenced_inside
        .iter()
        .filter(|name| !assigned_inside.contains(name))
        .filter(|name| is_defined_before(all_lines, name, start))
        .cloned()
        .collect();

    // Names assigned inside and referenced after → return values.
    let returns: Vec<String> = assigned_inside
        .iter()
        .filter(|name| is_referenced_after(all_lines, name, end))
        .cloned()
        .collect();

    DataFlowResult { params, returns }
}

/// Collect names that appear on the left side of `=` in the given lines.
fn collect_assigned_names(lines: &[&str]) -> Vec<String> {
    let mut names = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        // Simple assignment: `name = ...` or `name: type = ...`
        if let Some(before_eq) = trimmed.split('=').next() {
            let target = before_eq.split(':').next().unwrap_or("").trim();
            if is_simple_name(target) && !target.is_empty() {
                // Skip augmented assignments.
                if !trimmed.contains("+=")
                    && !trimmed.contains("-=")
                    && !trimmed.contains("*=")
                    && !trimmed.contains("/=")
                    && !names.contains(&target.to_owned())
                {
                    names.push(target.to_owned());
                }
            }
        }
    }
    names
}

/// Collect names that are referenced (used) in the given lines.
fn collect_referenced_names(lines: &[&str]) -> Vec<String> {
    let mut names = Vec::new();
    for line in lines {
        for word in extract_identifiers(line) {
            if !names.contains(&word) && !is_keyword(&word) {
                names.push(word);
            }
        }
    }
    names
}

/// Check if a name is defined (assigned) before `start_line`.
fn is_defined_before(lines: &[&str], name: &str, start_line: usize) -> bool {
    for line in lines.get(..start_line).unwrap_or_default() {
        let trimmed = line.trim();
        if let Some(before_eq) = trimmed.split('=').next() {
            let target = before_eq.split(':').next().unwrap_or("").trim();
            if target == name {
                return true;
            }
        }
        // Also check function parameters.
        if trimmed.starts_with("def ")
            && (trimmed.contains(&format!("({name}")) || trimmed.contains(&format!(", {name}")))
        {
            return true;
        }
    }
    false
}

/// Check if a name is referenced after `end_line`.
fn is_referenced_after(lines: &[&str], name: &str, end_line: usize) -> bool {
    for line in lines.get(end_line + 1..).unwrap_or_default() {
        for word in extract_identifiers(line) {
            if word == name {
                return true;
            }
        }
    }
    false
}

/// Extract all identifiers from a line of Python code.
fn extract_identifiers(line: &str) -> Vec<String> {
    let mut identifiers = Vec::new();
    let bytes = line.as_bytes();
    let mut pos = 0;

    while pos < bytes.len() {
        if bytes
            .get(pos)
            .is_some_and(|&b| b.is_ascii_alphabetic() || b == b'_')
        {
            let start = pos;
            while pos < bytes.len()
                && bytes
                    .get(pos)
                    .is_some_and(|&b| b.is_ascii_alphanumeric() || b == b'_')
            {
                pos += 1;
            }
            if let Some(word) = line.get(start..pos) {
                identifiers.push(word.to_owned());
            }
        } else {
            pos += 1;
        }
    }
    identifiers
}

/// Check if a string is a simple Python name (no dots, brackets, etc.).
fn is_simple_name(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let bytes = s.as_bytes();
    bytes
        .first()
        .is_some_and(|&b| b.is_ascii_alphabetic() || b == b'_')
        && bytes
            .iter()
            .all(|&b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Check if a word is a Python keyword.
fn is_keyword(word: &str) -> bool {
    matches!(
        word,
        "False"
            | "None"
            | "True"
            | "and"
            | "as"
            | "assert"
            | "async"
            | "await"
            | "break"
            | "class"
            | "continue"
            | "def"
            | "del"
            | "elif"
            | "else"
            | "except"
            | "finally"
            | "for"
            | "from"
            | "global"
            | "if"
            | "import"
            | "in"
            | "is"
            | "lambda"
            | "nonlocal"
            | "not"
            | "or"
            | "pass"
            | "raise"
            | "return"
            | "try"
            | "while"
            | "with"
            | "yield"
    )
}

// ── Context detection ────────────────────────────────────────────────────────

/// Information about the enclosing function context.
struct EnclosingContext {
    /// Whether the enclosing function is `async def`.
    is_async: bool,
    /// The self/cls parameter name if inside a method, or `None`.
    self_param: Option<String>,
}

/// Detect whether the selection is inside an async function or a method.
fn detect_enclosing_context(source: &str, line: usize) -> EnclosingContext {
    let lines: Vec<&str> = source.lines().collect();
    for idx in (0..line.min(lines.len())).rev() {
        let trimmed = lines.get(idx).copied().unwrap_or("").trim_start();
        if let Some(rest) = trimmed.strip_prefix("def ").or_else(|| {
            trimmed
                .strip_prefix("async ")
                .and_then(|s| s.strip_prefix("def "))
        }) {
            let is_async = trimmed.starts_with("async ");
            let self_param = extract_self_param(rest);
            return EnclosingContext {
                is_async,
                self_param,
            };
        }
    }
    EnclosingContext {
        is_async: false,
        self_param: None,
    }
}

/// Extract the first parameter if it is `self` or `cls`.
fn extract_self_param(after_def_name: &str) -> Option<String> {
    let paren_content = after_def_name.split('(').nth(1)?;
    let first_param = paren_content.split(',').next()?.trim();
    let name = first_param.split(':').next()?.trim();
    if name == "self" || name == "cls" {
        return Some(name.to_owned());
    }
    None
}

/// Check if any selected line contains `yield`, `break`, or `continue`.
fn contains_unextractable_keyword(lines: &[&str]) -> bool {
    for line in lines {
        for word in extract_identifiers(line) {
            if matches!(word.as_str(), "yield" | "break" | "continue") {
                return true;
            }
        }
    }
    false
}

/// Build the parameter string, prepending self/cls for methods.
fn build_params_string(params: &[String], context: &EnclosingContext) -> String {
    match &context.self_param {
        Some(sp) => {
            if params.is_empty() {
                sp.clone()
            } else {
                format!("{sp}, {}", params.join(", "))
            }
        }
        None => params.join(", "),
    }
}

/// Build the arguments string (self/cls is passed implicitly, not as an arg).
fn build_args_string(params: &[String], _context: &EnclosingContext) -> String {
    params.join(", ")
}

// ── Code generation ──────────────────────────────────────────────────────────

/// Build the function body from the selected lines, re-indented to 4 spaces.
///
/// Blank lines are emitted without trailing whitespace (PEP 8 W291).
fn build_function_body(selected: &[&str], body_indent: &str, func_indent: &str) -> String {
    let mut body = String::new();
    for line in selected {
        let stripped = strip_prefix_indent(line, body_indent);
        if stripped.trim().is_empty() {
            // Blank line — no trailing whitespace.
            body.push('\n');
        } else {
            body.push_str(func_indent);
            body.push_str("    ");
            body.push_str(stripped);
            body.push('\n');
        }
    }
    body
}

/// Strip a known indent prefix from a line.
fn strip_prefix_indent<'a>(line: &'a str, prefix: &str) -> &'a str {
    line.strip_prefix(prefix).unwrap_or(line)
}

/// Build the return statement for the extracted function.
fn build_return_statement(returns: &[String], func_indent: &str) -> String {
    if returns.is_empty() {
        return String::new();
    }
    if returns.len() == 1 {
        let name = returns.first().map_or("result", String::as_str);
        return format!("{func_indent}    return {name}\n");
    }
    let names = returns.join(", ");
    format!("{func_indent}    return {names}\n")
}

/// Build the call expression that replaces the selection.
fn build_call_expression(
    func_name: &str,
    args: &str,
    returns: &[String],
    indent: &str,
    context: &EnclosingContext,
) -> String {
    let await_kw = if context.is_async { "await " } else { "" };
    let receiver = match &context.self_param {
        Some(sp) => format!("{sp}."),
        None => String::new(),
    };
    let call = format!("{await_kw}{receiver}{func_name}({args})");
    if returns.is_empty() {
        return format!("{indent}{call}\n");
    }
    if returns.len() == 1 {
        let name = returns.first().map_or("result", String::as_str);
        return format!("{indent}{name} = {call}\n");
    }
    let names = returns.join(", ");
    format!("{indent}{names} = {call}\n")
}

/// Compute the indentation of the enclosing function or class.
fn compute_enclosing_indent(source: &str, line: usize) -> String {
    let lines: Vec<&str> = source.lines().collect();
    // Walk backwards from the selection to find a `def ` or `class ` line.
    for idx in (0..line.min(lines.len())).rev() {
        let Some(src_line) = lines.get(idx) else {
            continue;
        };
        let trimmed = src_line.trim();
        if trimmed.starts_with("def ") || trimmed.starts_with("class ") {
            let indent_len = src_line.len() - trimmed.len();
            return src_line.get(..indent_len).unwrap_or("").to_owned();
        }
    }
    // Module level.
    String::new()
}

/// Find the line to insert the new function definition.
///
/// Walks backwards from `line` to find the start of the enclosing function
/// or class, then inserts immediately before it (with a blank line).
fn find_insertion_point(source: &str, line: usize) -> usize {
    let lines: Vec<&str> = source.lines().collect();
    for idx in (0..line.min(lines.len())).rev() {
        let Some(src_line) = lines.get(idx) else {
            continue;
        };
        let trimmed = src_line.trim();
        if trimmed.starts_with("def ") || trimmed.starts_with("class ") {
            // Insert before this line.  Include any decorators.
            let mut insert = idx;
            while insert > 0 {
                let prev = lines.get(insert - 1).copied().unwrap_or("");
                if prev.trim().starts_with('@') {
                    insert -= 1;
                } else {
                    break;
                }
            }
            return insert;
        }
    }
    // Module level — insert at the start of the selection.
    line
}
