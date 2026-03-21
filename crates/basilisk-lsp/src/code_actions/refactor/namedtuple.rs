//! `NamedTuple` conversion refactoring actions: class syntax to functional form
//! and functional form to class syntax.

use std::collections::HashMap;
use std::fmt::Write as _;

use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, Position, Range, TextEdit, Url, WorkspaceEdit,
};

/// Offer to convert between `NamedTuple` class syntax and `namedtuple()`
/// functional syntax.
///
/// Returns zero, one, or two actions depending on which patterns appear near
/// the cursor.
#[must_use]
pub(in crate::code_actions) fn convert_namedtuple(
    uri: &Url,
    source: &str,
    range: &Range,
) -> Vec<CodeAction> {
    let mut actions = Vec::new();

    if let Some(action) = class_to_functional(uri, source, range) {
        actions.push(action);
    }

    if let Some(action) = functional_to_class(uri, source, range) {
        actions.push(action);
    }

    actions
}

/// Parse a `class Name(NamedTuple):` header from the cursor line.
/// Returns the class name if the line matches.
fn parse_namedtuple_class_header(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("class ")?;
    let paren_pos = rest.find('(')?;
    let name = rest.get(..paren_pos)?.trim();

    let after_paren = rest.get(paren_pos + 1..)?;
    let close_pos = after_paren.find(')')?;
    let bases = after_paren.get(..close_pos)?.trim();

    if !bases.split(',').any(|b| b.trim() == "NamedTuple") {
        return None;
    }

    let after_close = after_paren.get(close_pos + 1..)?.trim();
    if !after_close.starts_with(':') {
        return None;
    }

    Some(name)
}

/// Collect field names from annotated attributes in the class body (lines
/// following the header with deeper indentation).
fn collect_class_fields<'a>(
    lines: &[&'a str],
    start_line: usize,
    base_indent: &str,
) -> Vec<&'a str> {
    let mut fields = Vec::new();
    let body_indent_len = base_indent.len() + 4;

    for line in lines.iter().skip(start_line + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Stop when indentation returns to the class level or less.
        let leading_ws = line.len() - line.trim_start().len();
        if leading_ws < body_indent_len {
            break;
        }

        // Skip non-field lines (methods, comments, pass, docstrings).
        if trimmed.starts_with("def ")
            || trimmed.starts_with('#')
            || trimmed == "pass"
            || trimmed.starts_with('"')
            || trimmed.starts_with('\'')
        {
            continue;
        }

        // Extract field name from `name: type` or `name: type = default`.
        if let Some(colon_pos) = trimmed.find(':') {
            if let Some(field_name) = trimmed.get(..colon_pos).map(str::trim) {
                if !field_name.is_empty() {
                    fields.push(field_name);
                }
            }
        }
    }

    fields
}

/// Count lines belonging to the class body (for determining replacement range).
fn class_body_line_count(lines: &[&str], header_line: usize, base_indent: &str) -> u32 {
    let body_indent_len = base_indent.len() + 4;
    let mut count: u32 = 0;

    for line in lines.iter().skip(header_line + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            count += 1;
            continue;
        }
        let leading_ws = line.len() - line.trim_start().len();
        if leading_ws < body_indent_len {
            break;
        }
        count += 1;
    }

    count
}

/// Convert `class Point(NamedTuple): ...` to functional form.
fn class_to_functional(uri: &Url, source: &str, range: &Range) -> Option<CodeAction> {
    let line_idx = usize::try_from(range.start.line).unwrap_or(usize::MAX);
    let all_lines: Vec<&str> = source.lines().collect();
    let line_text = all_lines.get(line_idx)?;

    let class_name = parse_namedtuple_class_header(line_text)?;
    let indent = line_text.get(..line_text.len() - line_text.trim_start().len())?;

    let fields = collect_class_fields(&all_lines, line_idx, indent);
    let field_list = fields
        .iter()
        .map(|f| format!("\"{f}\""))
        .collect::<Vec<_>>()
        .join(", ");

    let replacement =
        format!("{indent}{class_name} = namedtuple(\"{class_name}\", [{field_list}])\n");

    let body_lines = class_body_line_count(&all_lines, line_idx, indent);
    let end_line = range.start.line + 1 + body_lines;

    let edit_range = Range {
        start: Position {
            line: range.start.line,
            character: 0,
        },
        end: Position {
            line: end_line,
            character: 0,
        },
    };

    Some(build_action(
        uri,
        edit_range,
        replacement,
        "Convert NamedTuple class to functional (basilisk)",
    ))
}

/// Parse a functional `Name = namedtuple("Name", [...])` or
/// `Name = namedtuple("Name", "a b")` from a line.
/// Returns `(name, fields)`.
fn parse_functional_namedtuple(line: &str) -> Option<(String, Vec<String>)> {
    let trimmed = line.trim();
    let eq_pos = trimmed.find('=')?;
    let name = trimmed.get(..eq_pos)?.trim();

    let rhs = trimmed.get(eq_pos + 1..)?.trim();
    let call_body = rhs.strip_prefix("namedtuple(")?;
    let call_body = call_body.strip_suffix(')')?;

    // Skip past the first argument (the quoted name).
    let comma_pos = call_body.find(',')?;
    let fields_arg = call_body.get(comma_pos + 1..)?.trim();

    let fields = parse_fields_argument(fields_arg)?;

    Some((name.to_owned(), fields))
}

/// Parse the fields argument from a `namedtuple` call.
/// Supports `["a", "b"]` list syntax and `"a b"` / `"a, b"` string syntax.
fn parse_fields_argument(arg: &str) -> Option<Vec<String>> {
    let trimmed = arg.trim();

    // List syntax: ["a", "b", "c"]
    if let Some(inner) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        let fields = inner
            .split(',')
            .filter_map(|s| {
                let s = s.trim();
                s.strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                    .or_else(|| s.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
                    .map(String::from)
            })
            .collect();
        return Some(fields);
    }

    // String syntax: "a b" or "a, b"
    let unquoted = trimmed
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| {
            trimmed
                .strip_prefix('\'')
                .and_then(|s| s.strip_suffix('\''))
        })?;

    let fields = if unquoted.contains(',') {
        unquoted.split(',').map(|s| s.trim().to_owned()).collect()
    } else {
        unquoted.split_whitespace().map(String::from).collect()
    };

    Some(fields)
}

/// Convert functional `namedtuple(...)` to class syntax.
fn functional_to_class(uri: &Url, source: &str, range: &Range) -> Option<CodeAction> {
    let line_idx = usize::try_from(range.start.line).unwrap_or(usize::MAX);
    let line_text = source.lines().nth(line_idx)?;

    let (name, fields) = parse_functional_namedtuple(line_text)?;
    let indent = line_text.get(..line_text.len() - line_text.trim_start().len())?;
    let body_indent = format!("{indent}    ");

    let mut replacement = format!("{indent}class {name}(NamedTuple):\n");
    if fields.is_empty() {
        let _ = writeln!(replacement, "{body_indent}pass");
    } else {
        for field in &fields {
            let _ = writeln!(replacement, "{body_indent}{field}: Any");
        }
    }

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
        "Convert namedtuple to class syntax (basilisk)",
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
