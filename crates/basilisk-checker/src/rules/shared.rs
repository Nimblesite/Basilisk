//! Shared helper functions used across multiple type checking rules.
//!
//! Consolidated from duplicated implementations in individual rule modules
//! to eliminate code duplication and improve maintainability.

use ruff_python_ast::{self as ast, Expr};

// ---------------------------------------------------------------------------
// String splitting
// ---------------------------------------------------------------------------

/// Split `s` at every top-level comma, respecting bracket nesting.
///
/// Returns slices into the original string (no allocation for the parts
/// themselves). Callers that need trimmed/owned values can chain
/// `.iter().map(|p| p.trim().to_owned())`.
pub(crate) fn split_top_level_commas(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth: usize = 0;
    let mut start = 0;
    for (idx, ch) in s.char_indices() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&s[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

// ---------------------------------------------------------------------------
// Annotation parsing
// ---------------------------------------------------------------------------

/// Parse a subscript annotation like `Name[A, B]` into `(name, [A, B])`.
///
/// Returns a borrowed name and owned, trimmed type argument strings.
/// Returns `None` when the annotation is not a valid subscript form.
pub(crate) fn parse_subscript_annotation(text: &str) -> Option<(&str, Vec<String>)> {
    let bracket_pos = text.find('[')?;
    let name = text[..bracket_pos].trim();
    if name.is_empty() {
        return None;
    }
    let inner = text.get(bracket_pos + 1..text.rfind(']')?)?;
    let args: Vec<String> = split_top_level_commas(inner)
        .iter()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    if args.is_empty() {
        return None;
    }
    Some((name, args))
}

// ---------------------------------------------------------------------------
// Expression helpers
// ---------------------------------------------------------------------------

/// Extract the simple name from a `Name` expression.
pub(crate) fn expr_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(n) => Some(n.id.as_str()),
        _ => None,
    }
}

/// Convert an annotation expression to a readable string.
pub(crate) fn ann_str(expr: &Expr) -> String {
    match expr {
        Expr::Name(n) => n.id.to_string(),
        Expr::Subscript(s) => format!("{}[{}]", ann_str(&s.value), ann_str(&s.slice)),
        Expr::Attribute(a) => format!("{}.{}", ann_str(&a.value), a.attr),
        Expr::Tuple(t) => t.elts.iter().map(ann_str).collect::<Vec<_>>().join(", "),
        Expr::BinOp(b) => format!("{} | {}", ann_str(&b.left), ann_str(&b.right)),
        Expr::NoneLiteral(_) => "None".to_owned(),
        Expr::StringLiteral(s) => s.value.to_str().to_owned(),
        Expr::List(l) => format!(
            "[{}]",
            l.elts.iter().map(ann_str).collect::<Vec<_>>().join(", ")
        ),
        Expr::NumberLiteral(n) => format!("{:?}", n.value),
        _ => "...".to_owned(),
    }
}

/// Infer the concrete type of a literal expression.
pub(crate) fn infer_expr_literal_type(expr: &Expr) -> Option<&'static str> {
    match expr {
        Expr::NumberLiteral(n) => match &n.value {
            ast::Number::Int(_) => Some("int"),
            ast::Number::Float(_) => Some("float"),
            ast::Number::Complex { .. } => Some("complex"),
        },
        Expr::StringLiteral(_) => Some("str"),
        Expr::BytesLiteral(_) => Some("bytes"),
        Expr::BooleanLiteral(_) => Some("bool"),
        Expr::NoneLiteral(_) => Some("None"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Type compatibility
// ---------------------------------------------------------------------------

/// Check numeric subtype relationship: `bool → int → float → complex`.
pub(crate) fn is_numeric_subtype(child: &str, parent: &str) -> bool {
    if child == parent {
        return true;
    }
    matches!(
        (child, parent),
        ("bool", "int" | "float" | "complex")
            | ("int", "float" | "complex")
            | ("float", "complex")
    )
}

/// Check if `actual` is assignable to `expected`.
///
/// Handles `Any`, `object`, the numeric tower (`bool → int → float → complex`),
/// and union types (`X | Y`).
pub(crate) fn is_type_compatible(actual: &str, expected: &str) -> bool {
    if actual == expected {
        return true;
    }
    if expected == "Any" || actual == "Any" || expected == "object" {
        return true;
    }
    if is_numeric_subtype(actual, expected) {
        return true;
    }
    if expected.contains('|') {
        return expected
            .split('|')
            .any(|part| is_type_compatible(actual, part.trim()));
    }
    false
}

// ---------------------------------------------------------------------------
// Identifier / typevar matching
// ---------------------------------------------------------------------------

/// Check whether `text` contains `name` as a whole word (not as a substring
/// of a longer identifier).
pub(crate) fn contains_typevar_reference(text: &str, name: &str) -> bool {
    let name_bytes = name.as_bytes();
    let text_bytes = text.as_bytes();
    let name_len = name_bytes.len();

    if name_len > text_bytes.len() {
        return false;
    }

    for start in 0..=(text_bytes.len() - name_len) {
        let Some(slice) = text_bytes.get(start..start + name_len) else {
            continue;
        };
        if slice != name_bytes {
            continue;
        }
        if start > 0
            && text_bytes
                .get(start - 1)
                .is_some_and(|&b| b.is_ascii_alphanumeric() || b == b'_')
        {
            continue;
        }
        let end = start + name_len;
        if end < text_bytes.len()
            && text_bytes
                .get(end)
                .is_some_and(|&b| b.is_ascii_alphanumeric() || b == b'_')
        {
            continue;
        }
        return true;
    }
    false
}
