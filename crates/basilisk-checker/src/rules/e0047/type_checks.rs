//! Structural annotation validity checks for BSK-E0047.
//!
//! Contains pure functions for detecting structurally invalid type expressions
//! by examining annotation text (as a string slice), plus non-type name detection
//! and `ParamSpec` invalid annotation detection.

use std::collections::HashSet;

use basilisk_resolver::{ImportKind, ResolvedModule};

// ---------------------------------------------------------------------------
// Structural checks on annotation text
// ---------------------------------------------------------------------------

/// Returns `true` when the annotation text is a structurally invalid type expression.
pub(super) fn is_invalid_type_annotation(ann: &str) -> bool {
    let ann = ann.trim();

    if ann.is_empty() {
        return false;
    }

    // Handle string literal annotations (forward references).
    let content_to_check = if (ann.starts_with('"') && ann.ends_with('"'))
        || (ann.starts_with('\'') && ann.ends_with('\''))
    {
        &ann[1..ann.len() - 1]
    } else {
        ann
    };

    // `Annotated[...]` is validated by E0045 — skip here to avoid false positives.
    if content_to_check.starts_with("Annotated[") {
        return false;
    }

    // `Generic` or `Generic[...]` is only valid in class base lists.
    if content_to_check == "Generic" || content_to_check.starts_with("Generic[") {
        return true;
    }

    // List literal or list comprehension.
    if content_to_check.starts_with('[') {
        return true;
    }

    // Dict literal.
    if content_to_check.starts_with('{') {
        return true;
    }

    // F-string.
    if content_to_check.starts_with("f\"") || content_to_check.starts_with("f'") {
        return true;
    }

    // Negative numeric literal.
    if content_to_check.starts_with('-')
        && content_to_check[1..]
            .trim()
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
    {
        return true;
    }

    // Positive numeric literal inside a string annotation.
    if !content_to_check.is_empty()
        && content_to_check
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.')
        && content_to_check
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
    {
        return true;
    }

    // Boolean literal used as an annotation.
    if content_to_check == "True" || content_to_check == "False" {
        return true;
    }

    // Conditional expression: ` if ` at depth 0.
    if has_top_level_token(content_to_check, " if ") {
        return true;
    }

    // Boolean binary operators: ` or ` / ` and ` at depth 0.
    if has_top_level_token(content_to_check, " or ")
        || has_top_level_token(content_to_check, " and ")
    {
        return true;
    }

    // Tuple literal: `(int, str)` — parens with comma at depth 0.
    if content_to_check.starts_with('(')
        && content_to_check.ends_with(')')
        && paren_contains_top_level_comma(content_to_check)
    {
        return true;
    }

    // Lambda expression.
    if content_to_check.contains("lambda") {
        return true;
    }

    // Explicit eval() call.
    if content_to_check.starts_with("eval(") {
        return true;
    }

    // String literal as an operand in a `|` union.
    if has_string_literal_in_pipe_union(content_to_check) {
        return true;
    }

    false
}

/// Returns `true` when the text contains a `|` union at depth 0 where one of the
/// pipe-separated parts is a quoted string literal (a misused forward reference).
pub(super) fn has_string_literal_in_pipe_union(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut string_char = b'"';
    let mut part_start = 0usize;

    let mut parts: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let Some(&ch) = bytes.get(i) else { break };
        if in_string {
            if ch == string_char && (i == 0 || bytes.get(i.wrapping_sub(1)).copied() != Some(b'\\'))
            {
                in_string = false;
            }
        } else {
            match ch {
                b'"' | b'\'' => {
                    in_string = true;
                    string_char = ch;
                }
                b'[' | b'(' | b'{' => depth += 1,
                b']' | b')' | b'}' => depth -= 1,
                b'|' if depth == 0 => {
                    if let Some(slice) = s.get(part_start..i) {
                        parts.push(slice.trim());
                    }
                    part_start = i + 1;
                }
                _ => {}
            }
        }
        i += 1;
    }
    if parts.is_empty() {
        return false;
    }
    if let Some(slice) = s.get(part_start..) {
        parts.push(slice.trim());
    }

    parts.iter().any(|part| {
        let p = part.trim();
        (p.starts_with('"') && p.ends_with('"') && p.len() >= 2)
            || (p.starts_with('\'') && p.ends_with('\'') && p.len() >= 2)
    })
}

/// Returns `true` when the text contains `token` at bracket depth 0.
pub(super) fn has_top_level_token(s: &str, token: &str) -> bool {
    let mut depth = 0i32;
    let bytes = s.as_bytes();
    let tok = token.as_bytes();
    let tok_len = tok.len();
    let mut i = 0;
    while i < bytes.len() {
        match bytes.get(i).copied() {
            Some(b'[' | b'(' | b'{') => depth += 1,
            Some(b']' | b')' | b'}') => depth -= 1,
            Some(_) if depth == 0 => {
                if bytes.get(i..i + tok_len) == Some(tok) {
                    return true;
                }
            }
            _ => {}
        }
        i += 1;
    }
    false
}

/// Returns `true` when `(...)` contains a comma at depth 0 inside the parens.
pub(super) fn paren_contains_top_level_comma(s: &str) -> bool {
    let inner = &s[1..s.len() - 1];
    let mut depth = 0i32;
    for ch in inner.chars() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth -= 1,
            ',' if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Non-type name detection
// ---------------------------------------------------------------------------

/// Returns `true` when the annotation text is exactly a name bound to a non-type in module scope.
pub(super) fn is_non_type_name(ann: &str, non_type_names: &HashSet<String>) -> bool {
    let ann = ann.trim();

    let content_to_check = if (ann.starts_with('"') && ann.ends_with('"'))
        || (ann.starts_with('\'') && ann.ends_with('\''))
    {
        &ann[1..ann.len() - 1]
    } else {
        ann
    };

    if content_to_check.contains('[')
        || content_to_check.contains('.')
        || content_to_check.contains('(')
        || content_to_check.contains(' ')
    {
        return false;
    }
    non_type_names.contains(content_to_check)
}

/// Build a set of names that are definitely not valid type expressions:
/// - Names bound to modules via plain `import X` statements.
/// - Names bound to unannotated simple literal values.
pub(super) fn collect_non_type_names(module: &ResolvedModule) -> HashSet<String> {
    let mut names = HashSet::new();

    for import in &module.imports {
        if import.kind == ImportKind::Plain {
            let local_name = import
                .module
                .split('.')
                .next_back()
                .unwrap_or(import.module.as_str());
            let _ = names.insert(local_name.to_owned());
        }
    }

    for var in &module.module_vars {
        if var.has_annotation {
            continue;
        }
        let is_simple_literal = matches!(
            var.rhs_kind,
            basilisk_resolver::RhsKind::IntLiteral
                | basilisk_resolver::RhsKind::FloatLiteral
                | basilisk_resolver::RhsKind::StrLiteral
                | basilisk_resolver::RhsKind::BoolLiteral
                | basilisk_resolver::RhsKind::BytesLiteral
                | basilisk_resolver::RhsKind::EmptyList
                | basilisk_resolver::RhsKind::EmptyDict
                | basilisk_resolver::RhsKind::NoneValue
        );
        if is_simple_literal {
            let _ = names.insert(var.name.clone());
        }
    }

    names
}

// ---------------------------------------------------------------------------
// ParamSpec invalid annotation detection
// ---------------------------------------------------------------------------

/// Returns `true` when the annotation uses a `ParamSpec` in an invalid position.
///
/// Valid positions for `P` (a `ParamSpec`):
/// - As the parameters argument of `Callable`: `Callable[P, ReturnType]`
/// - Inside `Concatenate` as the LAST argument: `Concatenate[T, P]` inside Callable
/// - As a type parameter in `Generic[P]`
///
/// Invalid positions (detected here):
/// - Bare `P` as a direct annotation
/// - `Concatenate[...]` used outside of `Callable`
/// - `P` inside a non-Callable subscript: `list[P]`, `dict[str, P]`
/// - `P` as the return type of `Callable`: `Callable[[int, str], P]`
pub(super) fn is_paramspec_invalid_annotation(ann: &str, paramspec_names: &HashSet<&str>) -> bool {
    let ann = ann.trim();
    if ann.is_empty() || paramspec_names.is_empty() {
        return false;
    }

    if paramspec_names.contains(ann) {
        return true;
    }

    if ann.starts_with("Concatenate[") {
        return true;
    }

    if !ann.contains('[') {
        return false;
    }

    if !ann.starts_with("Callable[") {
        for name in paramspec_names {
            if ann.contains(name) {
                let name_len = name.len();
                let ann_bytes = ann.as_bytes();
                for start in 0..ann.len().saturating_sub(name_len - 1) {
                    if ann.get(start..).is_some_and(|s| s.starts_with(name)) {
                        let end = start + name_len;
                        let before_ok = start == 0
                            || ann_bytes
                                .get(start - 1)
                                .is_none_or(|&b| !b.is_ascii_alphanumeric() && b != b'_');
                        let after_ok = end >= ann.len()
                            || ann_bytes
                                .get(end)
                                .is_none_or(|&b| !b.is_ascii_alphanumeric() && b != b'_');
                        if before_ok && after_ok {
                            return true;
                        }
                    }
                }
            }
        }
        return false;
    }

    // `Callable[[int, str], P]` — ParamSpec as the return type.
    let inner = ann.trim_start_matches("Callable[").trim_end_matches(']');
    let last_arg = last_top_level_arg(inner);
    if let Some(last) = last_arg {
        let last_trimmed = last.trim();
        if paramspec_names.contains(last_trimmed) {
            return true;
        }
    }

    false
}

/// Return the last top-level comma-separated argument from a subscript's inner text.
pub(super) fn last_top_level_arg(inner: &str) -> Option<&str> {
    let mut depth = 0i32;
    let mut last_comma = None;
    let bytes = inner.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'[' | b'(' | b'{' => depth += 1,
            b']' | b')' | b'}' => depth -= 1,
            b',' if depth == 0 => last_comma = Some(i),
            _ => {}
        }
    }
    last_comma.map(|pos| &inner[pos + 1..])
}
