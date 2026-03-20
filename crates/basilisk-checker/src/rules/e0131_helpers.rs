//! Helper types and functions for BSK-E0131.
//!
//! Provides annotation parsing, yield expression scanning, type inference,
//! and compatibility checks for generator yield/send/return type analysis.

use crate::rules::shared::{is_type_compatible, split_top_level_commas};

// ---------------------------------------------------------------------------
// Generator annotation
// ---------------------------------------------------------------------------

/// Parsed generator return annotation.
#[expect(
    clippy::struct_field_names,
    reason = "field names intentionally mirror the type parameter names"
)]
pub(super) struct GeneratorAnnotation {
    /// The yield type (first type parameter).
    pub(super) yield_type: String,
    /// The send type (second type parameter), if present.
    pub(super) send_type: Option<String>,
    /// The return type (third type parameter), if present.
    pub(super) return_type: Option<String>,
}

/// Try to parse a return annotation as a generator-like type.
///
/// Recognizes: `Generator[Y, S, R]`, `Iterator[Y]`, `Iterable[Y]`.
pub(super) fn parse_generator_annotation(ann: &str) -> Option<GeneratorAnnotation> {
    let ann = ann.trim();

    // Check for Generator[Y, S, R]
    if let Some(inner) = strip_generic_prefix(ann, "Generator") {
        let args = split_top_level_commas(inner);
        if args.is_empty() {
            return None;
        }
        return Some(GeneratorAnnotation {
            yield_type: args.first()?.trim().to_owned(),
            send_type: args.get(1).map(|s| s.trim().to_owned()),
            return_type: args.get(2).map(|s| s.trim().to_owned()),
        });
    }

    // Check for Iterator[Y]
    if let Some(inner) = strip_generic_prefix(ann, "Iterator") {
        let args = split_top_level_commas(inner);
        if args.is_empty() {
            return None;
        }
        return Some(GeneratorAnnotation {
            yield_type: args.first()?.trim().to_owned(),
            send_type: None,
            return_type: None,
        });
    }

    // Check for Iterable[Y]
    if let Some(inner) = strip_generic_prefix(ann, "Iterable") {
        let args = split_top_level_commas(inner);
        if args.is_empty() {
            return None;
        }
        return Some(GeneratorAnnotation {
            yield_type: args.first()?.trim().to_owned(),
            send_type: None,
            return_type: None,
        });
    }

    None
}

/// Strip a generic prefix like `Generator[` and return the inner content (without trailing `]`).
pub(super) fn strip_generic_prefix<'a>(ann: &'a str, prefix: &str) -> Option<&'a str> {
    let with_bracket = format!("{prefix}[");
    if !ann.starts_with(&with_bracket) {
        return None;
    }
    let inner_start = with_bracket.len();
    let inner_end = ann.rfind(']')?;
    if inner_end <= inner_start {
        return None;
    }
    ann.get(inner_start..inner_end)
}


// ---------------------------------------------------------------------------
// Yield expression scanning
// ---------------------------------------------------------------------------

/// A yield expression found in a function body.
pub(super) struct YieldExpr {
    /// The byte offset of the `yield` keyword in the source.
    pub(super) offset: u32,
    /// The text of the yielded expression (after `yield`).
    pub(super) expr_text: String,
    /// Whether this is a `yield from` expression.
    pub(super) is_yield_from: bool,
}

/// Find all yield expressions in a function body substring.
pub(super) fn find_yield_expressions(body: &str, body_offset: usize) -> Vec<YieldExpr> {
    let mut results = Vec::new();
    let bytes = body.as_bytes();
    let mut pos = 0;

    while pos < bytes.len() {
        let Some(&current_byte) = bytes.get(pos) else {
            break;
        };

        // Skip string literals (single/double/triple quoted)
        if current_byte == b'\'' || current_byte == b'"' {
            pos = skip_string(body, pos);
            continue;
        }

        // Skip comments
        if current_byte == b'#' {
            while pos < bytes.len() && bytes.get(pos).copied() != Some(b'\n') {
                pos += 1;
            }
            continue;
        }

        // Look for `yield` keyword
        if pos + 5 <= bytes.len() && body.get(pos..pos + 5) == Some("yield") {
            // Make sure it's a standalone keyword (not part of a larger identifier)
            let before_ok = pos == 0
                || bytes
                    .get(pos.wrapping_sub(1))
                    .is_none_or(|&b| !is_identifier_char(b));
            let after_pos = pos + 5;

            if before_ok && after_pos <= bytes.len() {
                // Check for `yield from`
                let is_yield_from = after_pos + 5 <= bytes.len()
                    && body.get(after_pos..after_pos + 5) == Some(" from")
                    && bytes
                        .get(after_pos + 5)
                        .is_none_or(|&b| !is_identifier_char(b));

                if is_yield_from {
                    let expr_start = after_pos + 5;
                    let expr_text = extract_yield_expr(body, expr_start);
                    if let Ok(offset) = u32::try_from(body_offset + pos) {
                        results.push(YieldExpr {
                            offset,
                            expr_text,
                            is_yield_from: true,
                        });
                    }
                } else if bytes
                    .get(after_pos)
                    .is_some_and(|&b| (b == b' ' || b == b'\n') && !is_identifier_char(b))
                {
                    let expr_text = extract_yield_expr(body, after_pos);
                    if let Ok(offset) = u32::try_from(body_offset + pos) {
                        results.push(YieldExpr {
                            offset,
                            expr_text,
                            is_yield_from: false,
                        });
                    }
                }
            }
        }

        pos += 1;
    }

    results
}

/// Extract the expression text after a yield keyword.
pub(super) fn extract_yield_expr(body: &str, start: usize) -> String {
    let rest = body.get(start..).unwrap_or("").trim_start();
    // Find the end of the expression: newline, comment, or end of string
    let mut depth = 0i32;
    let mut end = rest.len();
    for (idx, ch) in rest.char_indices() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => {
                if depth > 0 {
                    depth -= 1;
                } else {
                    end = idx;
                    break;
                }
            }
            '#' | '\n' if depth == 0 => {
                end = idx;
                break;
            }
            _ => {}
        }
    }
    rest.get(..end).unwrap_or("").trim().to_owned()
}

pub(super) fn is_identifier_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Skip past a string literal starting at `start`.
pub(super) fn skip_string(body: &str, start: usize) -> usize {
    let bytes = body.as_bytes();
    let Some(&quote) = bytes.get(start) else {
        return start;
    };

    // Check for triple quotes
    if start + 2 < bytes.len()
        && bytes.get(start + 1).copied() == Some(quote)
        && bytes.get(start + 2).copied() == Some(quote)
    {
        let mut pos = start + 3;
        while pos + 2 < bytes.len() {
            if bytes.get(pos).copied() == Some(quote)
                && bytes.get(pos + 1).copied() == Some(quote)
                && bytes.get(pos + 2).copied() == Some(quote)
            {
                return pos + 3;
            }
            pos += 1;
        }
        return bytes.len();
    }

    // Single quoted string
    let mut pos = start + 1;
    while pos < bytes.len() {
        let Some(&byte) = bytes.get(pos) else {
            break;
        };
        if byte == b'\\' {
            pos += 2;
            continue;
        }
        if byte == quote {
            return pos + 1;
        }
        if byte == b'\n' {
            return pos;
        }
        pos += 1;
    }
    bytes.len()
}

// ---------------------------------------------------------------------------
// Type inference and compatibility
// ---------------------------------------------------------------------------

/// Infer a simple type name from an expression text.
///
/// Returns the inferred type name for simple expressions:
/// - Integer literal (`3`, `-1`) -> `"int"`
/// - Float literal (`3.14`) -> `"float"`
/// - String literal (`"hello"`) -> `"str"`
/// - Boolean literal (`True`/`False`) -> `"bool"`
/// - `None` -> `"None"`
///
/// Returns `None` if the type cannot be inferred.
pub(super) fn infer_expr_type(expr: &str) -> Option<&str> {
    let expr = expr.trim();

    if expr.is_empty() {
        return None;
    }

    if expr == "True" || expr == "False" {
        return Some("bool");
    }

    if expr == "None" {
        return Some("None");
    }

    // Integer literal
    if expr.chars().all(|c| c.is_ascii_digit())
        || (expr.starts_with('-')
            && expr.len() > 1
            && expr
                .get(1..)
                .unwrap_or("")
                .chars()
                .all(|c| c.is_ascii_digit()))
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
    if (expr.starts_with('"') && expr.ends_with('"'))
        || (expr.starts_with('\'') && expr.ends_with('\''))
    {
        return Some("str");
    }

    None
}

/// Get the constructor name from an expression like `ClassName(...)`.
pub(super) fn get_constructor_name(expr: &str) -> Option<&str> {
    let expr = expr.trim();
    let paren_pos = expr.find('(')?;
    let name = expr.get(..paren_pos)?.trim();

    if name
        .chars()
        .next()
        .is_some_and(|c| c.is_uppercase() || c == '_')
        && name.chars().all(|c| c.is_alphanumeric() || c == '_')
    {
        Some(name)
    } else {
        None
    }
}

/// Extract the function name from a call expression like `generator17()`.
pub(super) fn extract_call_name(expr: &str) -> Option<&str> {
    let expr = expr.trim();
    let paren_pos = expr.find('(')?;
    let name = expr.get(..paren_pos)?.trim();

    if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        Some(name)
    } else {
        None
    }
}

/// Infer the element type of a list literal like `[1]`, `[1, 2, 3]`.
pub(super) fn infer_list_element_type(expr: &str) -> Option<&str> {
    let expr = expr.trim();
    if !expr.starts_with('[') || !expr.ends_with(']') {
        return None;
    }
    let inner = expr.get(1..expr.len().saturating_sub(1))?.trim();
    if inner.is_empty() {
        return None;
    }

    let first_elem = split_top_level_commas(inner);
    infer_expr_type(first_elem.first()?.trim())
}

/// Check send type compatibility.
///
/// For `yield from`, the outer generator's send type flows to the inner.
/// The outer's send type must be assignable to the inner's send type.
pub(super) fn is_send_type_compatible(outer_send: &str, inner_send: &str) -> bool {
    if outer_send == inner_send {
        return true;
    }
    if inner_send == "None" || outer_send == "None" {
        return true;
    }
    // float accepts int
    if inner_send == "float" && (outer_send == "int" || outer_send == "float") {
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Body bounds detection
// ---------------------------------------------------------------------------

/// Find the end byte index of a function body given its start and the def indent level.
pub(super) fn find_body_end(source: &str, body_start: usize, def_indent: usize) -> usize {
    let mut pos = body_start;
    let mut first_line = true;

    for line in source.get(body_start..).unwrap_or("").lines() {
        if first_line {
            first_line = false;
            pos += line.len() + 1;
            continue;
        }

        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            pos += line.len() + 1;
            continue;
        }

        let line_indent = line.len() - trimmed.len();
        if line_indent <= def_indent
            && (trimmed.starts_with("def ")
                || trimmed.starts_with("class ")
                || trimmed.starts_with("async def ")
                || trimmed.starts_with('@'))
        {
            return pos;
        }
        pos += line.len() + 1;
    }

    source.len()
}
