//! Implements [BSK-E0130] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! Utility functions for BSK-E0130.

use crate::rules::shared::contains_typevar_reference;

/// Check if `line` is a simple assignment (e.g. `X = list[T]`), excluding
/// comparisons (`==`, `!=`, `<=`, `>=`) and augmented assignments (`+=`, etc.).
/// Used to identify module-level implicit type aliases per PEP 484.
pub(super) fn is_simple_assignment(line: &str) -> bool {
    let bytes = line.as_bytes();
    for (idx, &byte) in bytes.iter().enumerate() {
        if byte == b'=' {
            // Skip `==`
            if bytes.get(idx + 1) == Some(&b'=') {
                return false;
            }
            // Skip `!=`, `<=`, `>=`
            if idx > 0
                && bytes
                    .get(idx - 1)
                    .is_some_and(|b| matches!(b, b'!' | b'<' | b'>'))
            {
                return false;
            }
            // Skip augmented assignments (`+=`, `-=`, `*=`, `/=`, etc.)
            if idx > 0
                && bytes.get(idx - 1).is_some_and(|b| {
                    matches!(b, b'+' | b'-' | b'*' | b'/' | b'%' | b'&' | b'|' | b'^')
                })
            {
                return false;
            }
            return true;
        }
    }
    false
}

/// Collect the full text of a function signature that may span multiple lines.
/// Starting from the `def` line at `start_idx`, concatenates lines until the
/// closing `)` and `:` are found (or the end of the slice is reached).
pub(super) fn collect_full_signature(lines: &[&str], start_idx: usize) -> String {
    let mut sig = String::new();
    let mut depth = 0i32;
    for line in lines.iter().skip(start_idx) {
        if !sig.is_empty() {
            sig.push(' ');
        }
        sig.push_str(line.trim());
        for ch in line.chars() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth <= 0 {
                        return sig;
                    }
                }
                _ => {}
            }
        }
    }
    sig
}

/// Extract `TypeVar` names from a `Generic[T, S, ...]`, `Protocol[T]`, or
/// similar parameterized base class. PEP 544 specifies that `Protocol[T]`
/// implicitly binds `T` as a class-level `TypeVar`.
pub(super) fn extract_typevars_from_generic_base(line: &str) -> std::collections::HashSet<String> {
    let mut result = std::collections::HashSet::new();
    // Both `Generic[...]` and `Protocol[...]` bind TypeVars in the class scope.
    for keyword in &["Generic[", "Protocol["] {
        if let Some(start) = line.find(keyword) {
            let after = &line[start + keyword.len()..];
            if let Some(end) = after.find(']') {
                let params = &after[..end];
                for param in params.split(',') {
                    // Strip a leading `*` from a `TypeVarTuple` parameter (PEP 646)
                    // so `Generic[*Ts]` binds `Ts` to match its use in annotations.
                    let trimmed = param.trim().strip_prefix('*').unwrap_or(param.trim());
                    if !trimmed.is_empty()
                        && trimmed
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '_')
                    {
                        let _ = result.insert(trimmed.to_owned());
                    }
                }
            }
        }
    }
    result
}

/// Extract `TypeVar` names referenced in function parameter annotations and return type.
pub(super) fn extract_typevars_from_function_sig(
    line: &str,
    all_typevars: &std::collections::HashSet<String>,
) -> std::collections::HashSet<String> {
    let mut result = std::collections::HashSet::new();
    for typevar_name in all_typevars {
        if contains_typevar_reference(line, typevar_name) {
            let _ = result.insert(typevar_name.clone());
        }
    }
    result
}

pub(super) use crate::rules::shared::{leading_indent, span_for_line};

/// Extract PEP 695 type parameters from a class definition like `class Foo[T, S](bases):`.
///
/// Returns an empty set if the class does not use PEP 695 syntax.
pub(super) fn extract_pep695_type_params(class_line: &str) -> std::collections::HashSet<String> {
    let trimmed = class_line.trim();
    if !trimmed.starts_with("class ") {
        return std::collections::HashSet::new();
    }
    let after_class = &trimmed[6..];

    let Some(bracket_pos) = after_class.find('[') else {
        return std::collections::HashSet::new();
    };

    // If `(` appears before `[`, this is traditional `Generic[T]` syntax, not PEP 695
    if let Some(paren_pos) = after_class.find('(') {
        if paren_pos < bracket_pos {
            return std::collections::HashSet::new();
        }
    }
    // If `:` appears before `[`, there are no type params
    if let Some(colon_pos) = after_class.find(':') {
        if colon_pos < bracket_pos {
            return std::collections::HashSet::new();
        }
    }

    let inner = &after_class[bracket_pos + 1..];
    let Some(close) = inner.find(']') else {
        return std::collections::HashSet::new();
    };

    inner[..close]
        .split(',')
        .map(|s| {
            // Strip a leading `*` from a `TypeVarTuple` parameter (PEP 646).
            let trimmed = s.trim();
            trimmed.strip_prefix('*').unwrap_or(trimmed).to_owned()
        })
        .filter(|s| !s.is_empty())
        .collect()
}

/// Extract PEP 695 type parameters in declaration order (preserving order).
///
/// Returns a `Vec` with the order matching the source declaration,
/// unlike [`extract_pep695_type_params`] which returns an unordered `HashSet`.
pub(super) fn extract_pep695_type_params_ordered(class_line: &str) -> Vec<String> {
    let trimmed = class_line.trim();
    if !trimmed.starts_with("class ") {
        return Vec::new();
    }
    let after_class = &trimmed[6..];

    let Some(bracket_pos) = after_class.find('[') else {
        return Vec::new();
    };
    if let Some(paren_pos) = after_class.find('(') {
        if paren_pos < bracket_pos {
            return Vec::new();
        }
    }
    if let Some(colon_pos) = after_class.find(':') {
        if colon_pos < bracket_pos {
            return Vec::new();
        }
    }

    let inner = &after_class[bracket_pos + 1..];
    let Some(close) = inner.find(']') else {
        return Vec::new();
    };

    inner[..close]
        .split(',')
        .map(|s| {
            // Strip a leading `*` from a `TypeVarTuple` parameter (PEP 646).
            let trimmed = s.trim();
            trimmed.strip_prefix('*').unwrap_or(trimmed).to_owned()
        })
        .filter(|s| !s.is_empty())
        .collect()
}

/// Extract the `TypeVar` names from a `Generic[T, S]` base expression.
pub(super) fn extract_typevar_params_from_generic(source_line: &str) -> Vec<String> {
    // Try `Generic[T, ...]` first, then `Protocol[T, ...]`.
    let (start, prefix_len) = if let Some(pos) = source_line.find("Generic[") {
        (pos, 8)
    } else if let Some(pos) = source_line.find("Protocol[") {
        (pos, 9)
    } else {
        return Vec::new();
    };
    let after = &source_line[start + prefix_len..];
    let Some(end) = after.find(']') else {
        return Vec::new();
    };
    after[..end]
        .split(',')
        .map(|s| {
            // Strip a leading `*` from a `TypeVarTuple` parameter (PEP 646).
            let trimmed = s.trim();
            trimmed.strip_prefix('*').unwrap_or(trimmed).to_owned()
        })
        .filter(|s| !s.is_empty())
        .collect()
}

/// Infer a simple Python type name from a literal expression.
pub(super) fn infer_literal_type(expr: &str) -> Option<&'static str> {
    let expr = expr.trim();
    if expr == "True" || expr == "False" {
        return Some("bool");
    }
    if expr == "None" {
        return Some("None");
    }
    // Integer literal (possibly negative)
    if expr.chars().all(|c| c.is_ascii_digit())
        || (expr.starts_with('-')
            && expr.len() > 1
            && expr[1..].chars().all(|c| c.is_ascii_digit()))
    {
        return Some("int");
    }
    // Float literal
    if expr.contains('.')
        && expr
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
    {
        return Some("float");
    }
    // String literal
    if (expr.starts_with('"') && expr.ends_with('"') && expr.len() >= 2)
        || (expr.starts_with('\'') && expr.ends_with('\'') && expr.len() >= 2)
    {
        return Some("str");
    }
    // Bytes literal
    if (expr.starts_with("b\"") && expr.ends_with('"'))
        || (expr.starts_with("b'") && expr.ends_with('\''))
    {
        return Some("bytes");
    }
    None
}

/// Compute a per-line bitmask indicating which lines are inside triple-quoted strings.
///
/// Returns a `Vec<bool>` of the same length as `lines` where `true` means the line
/// is entirely inside (or is) a triple-quoted string. This is a conservative
/// approximation that handles `"""` and `'''` delimiters.
pub(super) fn compute_triple_quote_mask(lines: &[&str]) -> Vec<bool> {
    let mut mask = vec![false; lines.len()];
    let mut in_triple = false;
    // Which delimiter we are inside: `true` = `"""`, `false` = `'''`.
    let mut is_double = true;

    for (idx, &line) in lines.iter().enumerate() {
        let Some(slot) = mask.get_mut(idx) else {
            continue;
        };
        if in_triple {
            *slot = true;
            let closing = if is_double { "\"\"\"" } else { "'''" };
            if line.contains(closing) {
                in_triple = false;
            }
        } else {
            // Check if a triple-quote opens on this line without closing on the same line.
            let (opens_double, opens_single) = triple_quote_opens(line);
            if opens_double {
                *slot = true;
                in_triple = true;
                is_double = true;
            } else if opens_single {
                *slot = true;
                in_triple = true;
                is_double = false;
            }
        }
    }
    mask
}

/// Check whether `line` opens a triple-quoted string that does NOT close on the same line.
///
/// Returns `(opens_double, opens_single)` booleans for `"""` and `'''` respectively.
fn triple_quote_opens(line: &str) -> (bool, bool) {
    for delim in ["\"\"\"", "'''"] {
        let Some(first) = line.find(delim) else {
            continue;
        };
        // Check if there is a matching close on this same line after the opening.
        let after_open = first + delim.len();
        if line
            .get(after_open..)
            .is_some_and(|rest| !rest.contains(delim))
        {
            return (delim == "\"\"\"", delim == "'''");
        }
    }
    (false, false)
}

/// Compute the 0-based line index where a multi-line function signature ends.
///
/// Starting from a `def`/`async def` line at `start_idx`, tracks parenthesis depth
/// and returns the index of the line containing the closing `)`. If the signature
/// is entirely on one line, returns `start_idx`.
pub(super) fn signature_end_line(lines: &[&str], start_idx: usize) -> usize {
    let mut depth = 0i32;
    for (offset, &line) in lines.iter().enumerate().skip(start_idx) {
        for ch in line.chars() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth <= 0 {
                        return offset;
                    }
                }
                _ => {}
            }
        }
    }
    start_idx
}

/// Find the position of the matching close paren/bracket in a string that starts
/// after an opening delimiter.
pub(super) fn find_matching_close(text: &str) -> usize {
    let mut depth = 0i32;
    for (idx, ch) in text.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                if depth == 0 {
                    return idx;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    text.len()
}
