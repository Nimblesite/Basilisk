//! Implements [`literals_literalstring`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Helper functions for `literals_literalstring`: `LiteralString` and `Literal` assignment
//! incompatibilities.
//!
//! This module contains text-level parsing utilities used by the rule to
//! identify and validate local annotated assignments inside function bodies.

use std::collections::HashMap;

use basilisk_resolver::{FunctionInfo, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

/// `literals_literalstring` error code shared between this module and the rule.
pub(super) const CODE: ErrorCode = ErrorCode {
    code: "literals_literalstring",
    docs_url: "https://www.basilisk-python.dev/errors/literals_literalstring",
};

/// Build a map from parameter name → annotation text for a function.
pub(super) fn param_annotations<'a>(
    func: &'a FunctionInfo,
    source: &'a str,
) -> HashMap<&'a str, &'a str> {
    let mut map = HashMap::new();
    for param in func
        .parameters
        .iter()
        .chain(func.vararg.iter())
        .chain(func.kwarg.iter())
    {
        if let Some(ann_span) = param.annotation_span {
            if let Some(ann_text) = slice_span(source, ann_span) {
                let _ = map.insert(param.name.as_str(), ann_text.trim());
            }
        }
    }
    map
}

/// Byte positions of every `:` immediately followed by `\n` in `source`.
///
/// Precomputed once per file so [`function_body_range`] can binary-search for a
/// function's signature-terminating colon instead of scanning from the `def` to
/// the end of the file on every function (which made the pass O(functions · n)
/// ≈ O(n²)). Positions are naturally ascending.
pub(super) fn colon_newline_positions(source: &str) -> Vec<usize> {
    source
        .as_bytes()
        .windows(2)
        .enumerate()
        .filter_map(|(idx, pair)| matches!(pair, [b':', b'\n']).then_some(idx))
        .collect()
}

/// Locate the function body in the source by finding the `:` after the
/// signature and returning everything from the next line to the end of
/// the function (approximated by the next `def ` or `class ` at the same
/// indentation, or end of file).
///
/// `colon_newlines` is the file-wide index from [`colon_newline_positions`];
/// the first entry at or after the `def` is the signature's terminating colon,
/// giving byte-for-byte the same `body_start` as the previous `find(":\n")`.
pub(super) fn function_body_range(
    func: &FunctionInfo,
    source: &str,
    colon_newlines: &[usize],
) -> Option<(usize, usize)> {
    // Start from the def keyword span.
    let def_start = usize::try_from(func.def_span.start).ok()?;

    // The signature-terminating colon is the first ":\n" at or after the def,
    // found by binary search over the precomputed positions. If there is none
    // (e.g. a single-line `def f(): ...` body, which has no ":\n"), there is no
    // multi-line body to locate — the previous code scanned to end-of-file only
    // to reach the same `None`, which is the O(n²) trap this avoids.
    let &colon_abs = colon_newlines.get(colon_newlines.partition_point(|&pos| pos < def_start))?;
    let body_start = colon_abs + 1; // after ':'

    // Determine the indentation of the `def` line.
    let line_start = source.get(..def_start)?.rfind('\n').map_or(0, |p| p + 1);
    let def_indent = def_start - line_start;

    // Scan forward to find the end: a line at the same or lesser indentation
    // that starts with `def `, `class `, or `@` (decorator), or EOF.
    let mut body_end = source.len();
    let mut pos = body_start;
    let mut first_line = true;
    let body_source = source.get(body_start..)?;
    for line in body_source.lines() {
        if first_line {
            first_line = false;
            pos += line.len() + 1; // +1 for '\n'
            continue;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            pos += line.len() + 1;
            continue;
        }

        // Count leading spaces.
        let line_indent = line.len() - line.trim_start().len();
        if line_indent <= def_indent
            && (trimmed.starts_with("def ")
                || trimmed.starts_with("class ")
                || trimmed.starts_with('@'))
        {
            body_end = pos;
            break;
        }
        pos += line.len() + 1;
    }

    Some((body_start, body_end))
}

/// A parsed annotated local assignment: `name: annotation = rhs`.
pub(super) struct LocalAssign<'a> {
    /// The annotation text (e.g. `Literal[""]`, `LiteralString`).
    pub annotation: &'a str,
    /// The right-hand side expression text (e.g. `b`, `f"{a} {non_literal}"`).
    pub rhs: &'a str,
    /// Byte offset into the source where the variable name starts.
    pub name_offset: usize,
    /// Length of the variable name.
    pub name_len: usize,
}

/// Parse annotated assignments from function body source text.
///
/// Looks for lines matching the pattern `name: Type = expr`.
pub(super) fn parse_annotated_assigns(body: &str, body_offset: usize) -> Vec<LocalAssign<'_>> {
    let mut results = Vec::new();

    for line in body.lines() {
        let trimmed = line.trim();
        // Skip comments, empty lines, decorators, def/class, etc.
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with("def ")
            || trimmed.starts_with("class ")
            || trimmed.starts_with('@')
            || trimmed.starts_with("return ")
            || trimmed.starts_with("assert_type")
            || trimmed.starts_with("if ")
            || trimmed.starts_with("else")
            || trimmed.starts_with("for ")
            || trimmed.starts_with("while ")
        {
            continue;
        }

        // Look for `name: Type = expr` pattern.
        if let Some(assign) = parse_single_annotated_assign(trimmed, line, body, body_offset) {
            results.push(assign);
        }
    }

    results
}

/// Try to parse a single `name: Type = expr` from a trimmed line.
fn parse_single_annotated_assign<'a>(
    trimmed: &'a str,
    raw_line: &'a str,
    body: &'a str,
    body_offset: usize,
) -> Option<LocalAssign<'a>> {
    // Find `: ` — annotation separator.
    let colon_pos = trimmed.find(": ")?;
    let name = trimmed[..colon_pos].trim();

    // Name must be a simple identifier.
    if !is_simple_identifier(name) {
        return None;
    }

    let after_colon = &trimmed[colon_pos + 2..];

    // Find `=` at depth 0 (skip `=` inside brackets).
    let eq_pos = find_top_level_eq(after_colon)?;
    let annotation = after_colon[..eq_pos].trim();
    let rhs = after_colon[eq_pos + 1..].trim();

    // Strip trailing comments from RHS.
    let rhs = strip_trailing_comment(rhs);

    if annotation.is_empty() || rhs.is_empty() {
        return None;
    }

    // Calculate source offset for the name.
    // SAFETY: both pointers come from substrings of the same source,
    // so the difference is always non-negative and fits in usize.
    let line_offset_in_body = raw_line
        .as_ptr()
        .addr()
        .saturating_sub(body.as_ptr().addr());
    let name_start_in_line = raw_line.len() - raw_line.trim_start().len();
    let name_offset = body_offset + line_offset_in_body + name_start_in_line;

    Some(LocalAssign {
        annotation,
        rhs,
        name_offset,
        name_len: name.len(),
    })
}

/// Check if a string is a simple Python identifier.
pub(super) fn is_simple_identifier(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap_or('0');
    if !first.is_alphabetic() && first != '_' {
        return false;
    }
    chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// Find the position of `=` at bracket depth 0 in `s`.
/// Skips `==`, `!=`, `<=`, `>=`.
pub(super) fn find_top_level_eq(source: &str) -> Option<usize> {
    let mut depth = 0i32;
    let bytes = source.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        let Some(&byte) = bytes.get(idx) else { break };
        match byte {
            b'[' | b'(' | b'{' => depth += 1,
            b']' | b')' | b'}' => depth -= 1,
            b'=' if depth == 0 => {
                // Skip `==`
                if bytes.get(idx + 1) == Some(&b'=') {
                    idx += 2;
                    continue;
                }
                // Skip `!=`, `<=`, `>=` — the `=` is part of a comparison
                if idx > 0
                    && bytes
                        .get(idx - 1)
                        .is_some_and(|&b| matches!(b, b'!' | b'<' | b'>'))
                {
                    idx += 1;
                    continue;
                }
                return Some(idx);
            }
            _ => {}
        }
        idx += 1;
    }
    None
}

/// Strip trailing `# comment` from RHS text.
pub(super) fn strip_trailing_comment(rhs: &str) -> &str {
    // Simple heuristic: find `#` not inside quotes.
    let mut in_single = false;
    let mut in_double = false;
    for (idx, ch) in rhs.char_indices() {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '#' if !in_single && !in_double => return rhs[..idx].trim(),
            _ => {}
        }
    }
    rhs
}

/// Extract the string value from a `Literal["value"]` annotation.
/// Returns `None` if the annotation is not of this form.
pub(super) fn extract_literal_string_value(ann: &str) -> Option<&str> {
    let ann = ann.trim();
    let inner = ann.strip_prefix("Literal[")?;
    let inner = inner.strip_suffix(']')?;
    let inner = inner.trim();

    // Must be a quoted string: "..." or '...'
    if (inner.starts_with('"') && inner.ends_with('"'))
        || (inner.starts_with('\'') && inner.ends_with('\''))
    {
        Some(&inner[1..inner.len() - 1])
    } else {
        None
    }
}

/// Extract interpolated variable names from an f-string.
/// For `f"{a} {non_literal}"`, returns `["a", "non_literal"]`.
pub(super) fn extract_fstring_names(fstring: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut chars = fstring.chars();
    while let Some(ch) = chars.next() {
        if ch == '{' {
            // Collect until '}' or non-identifier char.
            let mut name = String::new();
            for inner in chars.by_ref() {
                if inner == '}' || inner == '!' || inner == ':' || inner == '.' {
                    break;
                }
                name.push(inner);
            }
            let name = name.trim().to_owned();
            if !name.is_empty() && is_simple_identifier(&name) {
                names.push(name);
            }
        }
    }
    names
}

/// Returns `true` when the annotation text is plain `str` (not `LiteralString`,
/// not `Literal[...]`, not a generic like `list[str]`).
pub(super) fn is_plain_str_type(ann: &str) -> bool {
    let ann = ann.trim();
    ann == "str"
}

/// Split a generic annotation like `list[str]` into `("list", "str")`.
/// Returns `None` for non-generic annotations.
pub(super) fn split_generic(ann: &str) -> Option<(&str, &str)> {
    let ann = ann.trim();
    let bracket = ann.find('[')?;
    let container = ann[..bracket].trim();
    let rest = &ann[bracket + 1..];
    let inner = rest.strip_suffix(']')?.trim();
    Some((container, inner))
}

/// Returns `true` for invariant container types (mutable generics).
pub(super) fn is_invariant_container(name: &str) -> bool {
    matches!(
        name,
        "list" | "List" | "dict" | "Dict" | "set" | "Set" | "deque" | "Deque"
    )
}

/// Parse a simple function call `Name(arg1, arg2)` into the callee name
/// and the list of argument names.
pub(super) fn parse_simple_call(expr: &str) -> Option<(&str, Vec<String>)> {
    let paren = expr.find('(')?;
    let callee = expr[..paren].trim();
    if !is_simple_identifier(callee) {
        return None;
    }
    let rest = expr[paren + 1..].strip_suffix(')')?.trim();
    let args: Vec<String> = rest
        .split(',')
        .map(|a| a.trim().to_owned())
        .filter(|a| !a.is_empty())
        .collect();
    Some((callee, args))
}

/// Emit a `literals_literalstring` diagnostic for a literal value mismatch.
pub(super) fn emit_literal_value_mismatch(
    assign: &LocalAssign<'_>,
    target_value: &str,
    source_value: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Cannot assign `Literal[\"{source_value}\"]` to `Literal[\"{target_value}\"]` \
             — literal values are incompatible"
        ),
        Span {
            start: u32::try_from(assign.name_offset).unwrap_or(u32::MAX),
            end: u32::try_from(assign.name_offset + assign.name_len).unwrap_or(u32::MAX),
        },
        path,
        Some(format!(
            "The variable expects exactly `Literal[\"{target_value}\"]`, \
             but the parameter has type `Literal[\"{source_value}\"]`"
        )),
        Some(
            "PEP 586: Literal types are only compatible when their values match exactly".to_owned(),
        ),
    ));
}

/// Emit a `literals_literalstring` diagnostic for an f-string `LiteralString` violation.
pub(super) fn emit_fstring_literal_string_error(
    assign: &LocalAssign<'_>,
    name: &str,
    param_ann: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Cannot assign f-string to `LiteralString` — interpolated variable \
             `{name}` has type `{param_ann}`, which is not `LiteralString`"
        ),
        Span {
            start: u32::try_from(assign.name_offset).unwrap_or(u32::MAX),
            end: u32::try_from(assign.name_offset + assign.name_len).unwrap_or(u32::MAX),
        },
        path,
        Some(format!(
            "Change `{name}` to `LiteralString` or use `str` as the target type"
        )),
        Some(
            "PEP 675: an f-string is `LiteralString` only if all interpolated \
             expressions are compatible with `LiteralString`"
                .to_owned(),
        ),
    ));
}

/// Emit a `literals_literalstring` diagnostic for an invariant container generic mismatch.
pub(super) fn emit_invariant_container_mismatch(
    assign: &LocalAssign<'_>,
    param_ann: &str,
    ann: &str,
    ann_container: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Cannot assign `{param_ann}` to `{ann}` — \
             `{ann_container}` is invariant in its type parameter"
        ),
        Span {
            start: u32::try_from(assign.name_offset).unwrap_or(u32::MAX),
            end: u32::try_from(assign.name_offset + assign.name_len).unwrap_or(u32::MAX),
        },
        path,
        Some(format!(
            "Use `Sequence[str]` (covariant) instead of `{ann}` if you \
             need to accept `{param_ann}`"
        )),
        Some(
            "PEP 484: mutable generic containers like `list` are invariant — \
             `list[LiteralString]` is not a subtype of `list[str]`"
                .to_owned(),
        ),
    ));
}

/// Emit a `literals_literalstring` diagnostic for a container constructor call with str argument.
pub(super) fn emit_container_call_str_error(
    assign: &LocalAssign<'_>,
    rhs: &str,
    ann: &str,
    arg: &str,
    param_ann: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Cannot assign `{rhs}` to `{ann}` — argument `{arg}` \
             has type `{param_ann}`, not `LiteralString`"
        ),
        Span {
            start: u32::try_from(assign.name_offset).unwrap_or(u32::MAX),
            end: u32::try_from(assign.name_offset + assign.name_len).unwrap_or(u32::MAX),
        },
        path,
        Some(format!(
            "Change `{arg}` to `LiteralString` or relax the target annotation"
        )),
        Some(
            "PEP 675: `str` is not assignable to `LiteralString` — \
             `LiteralString` is a strict subtype of `str`"
                .to_owned(),
        ),
    ));
}
