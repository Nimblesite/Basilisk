//! Utility functions for BSK-E0130.

use basilisk_resolver::Span;

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

/// Check if `name` appears as a whole identifier in `text` (not as part of a longer name).
pub(super) fn contains_typevar_reference(text: &str, typevar_name: &str) -> bool {
    let needle = typevar_name.as_bytes();
    let haystack = text.as_bytes();
    let needle_len = needle.len();

    if needle_len > haystack.len() {
        return false;
    }

    haystack
        .windows(needle_len)
        .enumerate()
        .any(|(idx, window)| {
            if window != needle {
                return false;
            }
            let before_ok = idx == 0
                || haystack
                    .get(idx - 1)
                    .is_some_and(|&b| !b.is_ascii_alphanumeric() && b != b'_');
            let after_ok = idx + needle_len >= haystack.len()
                || haystack
                    .get(idx + needle_len)
                    .is_some_and(|&b| !b.is_ascii_alphanumeric() && b != b'_');
            before_ok && after_ok
        })
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
                    let trimmed = param.trim();
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

/// Compute the leading whitespace count of a line.
pub(super) fn leading_indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Find the byte offset of a given 1-based line number in source text.
#[expect(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    reason = "byte offsets fit u32 for source files"
)]
pub(super) fn line_to_byte_offset(source: &str, target_line: usize) -> u32 {
    let mut current_line = 1usize;
    for (byte_idx, ch) in source.char_indices() {
        if current_line == target_line {
            return byte_idx as u32;
        }
        if ch == '\n' {
            current_line += 1;
        }
    }
    source.len() as u32
}

/// Build a span covering the trimmed content of the given 1-based line.
#[expect(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    reason = "u32<->usize safe on 32-bit+"
)]
pub(super) fn span_for_line(source: &str, line_number: usize) -> Span {
    let start = line_to_byte_offset(source, line_number) as usize;
    let line_text = source
        .get(start..)
        .and_then(|s| s.lines().next())
        .unwrap_or("");
    let trimmed_start = start + (line_text.len() - line_text.trim_start().len());
    let trimmed_end = start + line_text.trim_end().len();
    Span {
        start: trimmed_start as u32,
        end: trimmed_end as u32,
    }
}

/// Extract the `TypeVar` names from a `Generic[T, S]` base expression.
pub(super) fn extract_typevar_params_from_generic(source_line: &str) -> Vec<String> {
    let Some(start) = source_line.find("Generic[") else {
        return Vec::new();
    };
    let after = &source_line[start + 8..];
    let Some(end) = after.find(']') else {
        return Vec::new();
    };
    after[..end]
        .split(',')
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Parse a subscript annotation like `MyClass[int]` into `("MyClass", ["int"])`.
pub(super) fn parse_generic_annotation(ann: &str) -> Option<(String, Vec<String>)> {
    let bracket_pos = ann.find('[')?;
    let class_name = ann[..bracket_pos].trim().to_owned();
    if class_name.is_empty() {
        return None;
    }
    let inner = ann.get(bracket_pos + 1..ann.rfind(']')?)?;
    let type_args = split_top_level_type_args(inner);
    if type_args.is_empty() {
        return None;
    }
    Some((class_name, type_args))
}

/// Split comma-separated type args at the top level of brackets.
pub(super) fn split_top_level_type_args(inner: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (idx, ch) in inner.char_indices() {
        match ch {
            '[' | '(' => depth += 1,
            ']' | ')' => depth -= 1,
            ',' if depth == 0 => {
                args.push(inner[start..idx].trim().to_owned());
                start = idx + 1;
            }
            _ => {}
        }
    }
    let last = inner[start..].trim();
    if !last.is_empty() {
        args.push(last.to_owned());
    }
    args
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

/// Check if two type names are compatible.
pub(super) fn generic_types_compatible(actual: &str, expected: &str) -> bool {
    if expected == "Any" || actual == "Any" || expected == "object" {
        return true;
    }
    if actual == expected {
        return true;
    }
    // bool is subtype of int
    if expected == "int" && actual == "bool" {
        return true;
    }
    // int is subtype of float
    if expected == "float" && (actual == "int" || actual == "bool") {
        return true;
    }
    false
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
