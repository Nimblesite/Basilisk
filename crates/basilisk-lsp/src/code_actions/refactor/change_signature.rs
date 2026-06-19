//! Implements [LSPARCH-FEATURES-CODEACTIONS]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-CODEACTIONS
//!
//! Change function signature refactoring.
//!
//! Offers to add a parameter with a default value to a function, or to remove
//! an unused parameter from a function and all its call sites.

use std::collections::HashMap;

use tower_lsp::lsp_types::{CodeAction, CodeActionKind, Position, Range, TextEdit, Url};

/// Offer to remove the parameter under the cursor from a function and all
/// call sites in the same file.
///
/// Only offered when:
/// - The cursor is on a parameter name in a `def` line.
/// - The function has at least 2 parameters (removing the last one is silly).
#[must_use]
pub(in crate::code_actions) fn remove_parameter(
    uri: &Url,
    source: &str,
    _range: &Range,
    resolved: &basilisk_resolver::ResolvedModule,
    position_offset: usize,
) -> Option<CodeAction> {
    let (func, param_idx) = find_param_at_offset(resolved, position_offset)?;

    // Don't offer to remove `self` or `cls`.
    let param = func.parameters.get(param_idx)?;
    if param.name == "self" || param.name == "cls" {
        return None;
    }

    // Must have at least 2 parameters.
    if func.parameters.len() < 2 {
        return None;
    }

    let mut edits = Vec::new();

    // 1. Remove the parameter from the def line.
    let def_edit = build_param_removal_edit(source, func, param_idx)?;
    edits.push(def_edit);

    // 2. Remove the corresponding argument from each call site.
    let call_edits = build_call_site_removals(source, &func.name, param_idx);
    edits.extend(call_edits);

    if edits.is_empty() {
        return None;
    }

    let mut changes = HashMap::new();
    let _ = changes.insert(uri.clone(), edits);

    Some(super::super::code_action_with_changes(
        format!("Remove parameter '{}' (basilisk)", param.name),
        CodeActionKind::new("refactor.rewrite.signature"),
        changes,
        false,
    ))
}

/// Find which function and parameter index the cursor is on.
fn find_param_at_offset(
    resolved: &basilisk_resolver::ResolvedModule,
    offset: usize,
) -> Option<(&basilisk_resolver::FunctionInfo, usize)> {
    let offset_u32 = u32::try_from(offset).unwrap_or(u32::MAX);
    for func in &resolved.functions {
        for (idx, param) in func.parameters.iter().enumerate() {
            let span = param.name_span;
            if span.start <= offset_u32 && offset_u32 < span.end {
                return Some((func, idx));
            }
        }
    }
    None
}

/// Build a text edit that removes parameter at `param_idx` from the def line.
fn build_param_removal_edit(
    source: &str,
    func: &basilisk_resolver::FunctionInfo,
    param_idx: usize,
) -> Option<TextEdit> {
    let loc = locate_params(source, func)?;

    // Split parameters and remove the one at param_idx.
    let params: Vec<&str> = split_params(loc.params_slice);
    if param_idx >= params.len() {
        return None;
    }

    let mut new_params: Vec<&str> = Vec::new();
    for (idx, param) in params.iter().enumerate() {
        if idx != param_idx {
            new_params.push(param);
        }
    }

    let new_params_str = new_params.join(", ");
    Some(build_params_edit(
        loc.def_line_num,
        loc.paren_start,
        loc.paren_end,
        &new_params_str,
    ))
}

/// Build text edits that remove the argument at `param_idx` from call sites.
fn build_call_site_removals(source: &str, func_name: &str, param_idx: usize) -> Vec<TextEdit> {
    let mut edits = Vec::new();
    let call_pattern = format!("{func_name}(");

    for (line_idx, line) in source.lines().enumerate() {
        if !line.contains(&call_pattern) {
            continue;
        }
        if let Some(edit) = remove_arg_from_call(line, &call_pattern, param_idx, line_idx) {
            edits.push(edit);
        }
    }
    edits
}

/// Remove the argument at `param_idx` from a function call on this line.
fn remove_arg_from_call(
    line: &str,
    call_pattern: &str,
    param_idx: usize,
    line_idx: usize,
) -> Option<TextEdit> {
    let call_pos = line.find(call_pattern)?;
    let args_start = call_pos + call_pattern.len();
    let args_text = line.get(args_start..)?;
    let paren_end = find_close_paren(args_text)?;
    let args_slice = args_text.get(..paren_end)?;

    let args: Vec<&str> = split_params(args_slice);
    if param_idx >= args.len() {
        return None;
    }

    let mut new_args: Vec<&str> = Vec::new();
    for (idx, arg) in args.iter().enumerate() {
        if idx != param_idx {
            new_args.push(arg);
        }
    }

    let new_args_str = new_args.join(", ");
    let line_u32 = u32::try_from(line_idx).unwrap_or(u32::MAX);
    let start_char = u32::try_from(args_start).unwrap_or(0);
    let end_char = u32::try_from(args_start + paren_end).unwrap_or(0);

    Some(TextEdit {
        range: Range {
            start: Position {
                line: line_u32,
                character: start_char,
            },
            end: Position {
                line: line_u32,
                character: end_char,
            },
        },
        new_text: new_args_str,
    })
}

/// Find the closing paren that matches depth 0.
fn find_close_paren(text: &str) -> Option<usize> {
    let mut depth: u32 = 0;
    for (idx, ch) in text.char_indices() {
        match ch {
            '(' | '[' => depth += 1,
            ')' if depth == 0 => return Some(idx),
            ')' | ']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

/// Split parameter/argument list on top-level commas.
fn split_params(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth: u32 = 0;
    let mut start = 0;

    for (idx, ch) in text.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                if let Some(part) = text.get(start..idx) {
                    parts.push(part.trim());
                }
                start = idx + 1;
            }
            _ => {}
        }
    }
    if let Some(part) = text.get(start..) {
        let trimmed = part.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed);
        }
    }
    parts
}

/// Offer to add a new parameter with a default value to the function under
/// the cursor.
///
/// Only offered when the cursor is on a `def` line.  The new parameter is
/// appended at the end with `new_param=None`.
#[must_use]
pub(in crate::code_actions) fn add_parameter(
    uri: &Url,
    source: &str,
    _range: &Range,
    resolved: &basilisk_resolver::ResolvedModule,
    position_offset: usize,
) -> Option<CodeAction> {
    let func = find_func_at_offset(resolved, position_offset)?;
    let loc = locate_params(source, func)?;

    let new_param = "new_param=None";
    let new_params = if loc.params_slice.trim().is_empty() {
        new_param.to_owned()
    } else {
        format!("{}, {new_param}", loc.params_slice.trim())
    };

    let edit = build_params_edit(
        loc.def_line_num,
        loc.paren_start,
        loc.paren_end,
        &new_params,
    );

    let mut changes = HashMap::new();
    let _ = changes.insert(uri.clone(), vec![edit]);

    Some(super::super::code_action_with_changes(
        "Add parameter (basilisk)".to_owned(),
        CodeActionKind::new("refactor.rewrite.signature"),
        changes,
        false,
    ))
}

/// Offer to sort parameters alphabetically (excluding self/cls).
///
/// Only offered when the function has 3+ non-self parameters (reordering
/// 2 is trivial and not worth a code action).
#[must_use]
pub(in crate::code_actions) fn reorder_parameters(
    uri: &Url,
    source: &str,
    _range: &Range,
    resolved: &basilisk_resolver::ResolvedModule,
    position_offset: usize,
) -> Option<CodeAction> {
    let func = find_func_at_offset(resolved, position_offset)?;
    let loc = locate_params(source, func)?;

    let params = split_params(loc.params_slice);
    let non_self = non_self_params(&params);
    // Need at least 3 to make reordering worthwhile.
    if non_self.len() < 3 {
        return None;
    }

    let sorted = sort_params(&params);
    // Don't offer if already sorted.
    if sorted == params.join(", ") {
        return None;
    }

    let edit = build_params_edit(loc.def_line_num, loc.paren_start, loc.paren_end, &sorted);

    let mut changes = HashMap::new();
    let _ = changes.insert(uri.clone(), vec![edit]);

    Some(super::super::code_action_with_changes(
        "Sort parameters alphabetically (basilisk)".to_owned(),
        CodeActionKind::new("refactor.rewrite.signature"),
        changes,
        false,
    ))
}

/// The location of a function's parameter list on its `def` line.
struct ParamList<'a> {
    def_line_num: usize,
    paren_start: usize,
    paren_end: usize,
    params_slice: &'a str,
}

/// Locate the parameter-list slice between the parens on `func`'s `def` line.
fn locate_params<'a>(
    source: &'a str,
    func: &basilisk_resolver::FunctionInfo,
) -> Option<ParamList<'a>> {
    let def_line_num = line_of_offset(source, func.def_span.start_usize());
    let line = source.lines().nth(def_line_num)?;
    let paren_start = line.find('(')?;
    let params_text = line.get(paren_start + 1..)?;
    let paren_end = find_close_paren(params_text)?;
    let params_slice = params_text.get(..paren_end)?;
    Some(ParamList {
        def_line_num,
        paren_start,
        paren_end,
        params_slice,
    })
}

/// Find the function whose `def_span` contains the given offset.
fn find_func_at_offset(
    resolved: &basilisk_resolver::ResolvedModule,
    offset: usize,
) -> Option<&basilisk_resolver::FunctionInfo> {
    let offset_u32 = u32::try_from(offset).unwrap_or(u32::MAX);
    resolved
        .functions
        .iter()
        .find(|f| f.def_span.start <= offset_u32 && offset_u32 < f.def_span.end)
}

/// Build a `TextEdit` that replaces the params between parens.
fn build_params_edit(
    def_line_num: usize,
    paren_start: usize,
    paren_end: usize,
    new_params: &str,
) -> TextEdit {
    let line_u32 = u32::try_from(def_line_num).unwrap_or(u32::MAX);
    let start_char = u32::try_from(paren_start + 1).unwrap_or(0);
    let end_char = u32::try_from(paren_start + 1 + paren_end).unwrap_or(0);

    TextEdit {
        range: Range {
            start: Position {
                line: line_u32,
                character: start_char,
            },
            end: Position {
                line: line_u32,
                character: end_char,
            },
        },
        new_text: new_params.to_owned(),
    }
}

/// Filter out self/cls from parameter list.
fn non_self_params<'a>(params: &[&'a str]) -> Vec<&'a str> {
    params
        .iter()
        .filter(|p| {
            let name = p
                .split('=')
                .next()
                .unwrap_or(p)
                .split(':')
                .next()
                .unwrap_or(p)
                .trim();
            name != "self" && name != "cls"
        })
        .copied()
        .collect()
}

/// Sort parameters alphabetically, keeping self/cls first.
fn sort_params(params: &[&str]) -> String {
    let mut self_params = Vec::new();
    let mut other_params = Vec::new();

    for param in params {
        let name = param
            .split('=')
            .next()
            .unwrap_or(param)
            .split(':')
            .next()
            .unwrap_or(param)
            .trim();
        if name == "self" || name == "cls" {
            self_params.push(*param);
        } else {
            other_params.push(*param);
        }
    }
    other_params.sort_unstable();
    self_params.extend(other_params);
    self_params.join(", ")
}

/// Convert a byte offset to a 0-based line number.
fn line_of_offset(source: &str, offset: usize) -> usize {
    let clamped = offset.min(source.len());
    source
        .get(..clamped)
        .unwrap_or(source)
        .chars()
        .filter(|&c| c == '\n')
        .count()
}
