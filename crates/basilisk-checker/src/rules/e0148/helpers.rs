//! Utility helpers for BSK-E0148.

use ruff_python_ast::{self as ast, Expr};

use basilisk_resolver::Span;

/// Extract the simple name from an expression, if it is a bare `Name` node.
pub(super) fn expr_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(n) => Some(n.id.as_str()),
        _ => None,
    }
}

/// Convert an annotation expression to its text representation.
pub(super) fn ann_str(expr: &Expr) -> String {
    match expr {
        Expr::Name(n) => n.id.to_string(),
        Expr::Subscript(s) => format!("{}[{}]", ann_str(&s.value), ann_str(&s.slice)),
        Expr::Attribute(a) => format!("{}.{}", ann_str(&a.value), a.attr),
        Expr::Tuple(t) => t.elts.iter().map(ann_str).collect::<Vec<_>>().join(", "),
        Expr::BinOp(b) => format!("{} | {}", ann_str(&b.left), ann_str(&b.right)),
        Expr::NoneLiteral(_) => "None".to_owned(),
        Expr::StringLiteral(s) => s.value.to_str().to_owned(),
        _ => "...".to_owned(),
    }
}

/// Split a string by top-level commas (respecting bracket nesting).
pub(super) fn split_top_level(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (idx, ch) in s.char_indices() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth -= 1,
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

/// Build a span for a call expression.
pub(super) fn call_span(call: &ast::ExprCall) -> Span {
    use ruff_text_size::Ranged;
    Span {
        start: call.range().start().to_u32(),
        end: call.range().end().to_u32(),
    }
}

/// Infer the concrete type of a literal expression.
pub(super) fn infer_literal_type(expr: &Expr) -> Option<&'static str> {
    match expr {
        Expr::NumberLiteral(n) => match &n.value {
            ruff_python_ast::Number::Int(_) => Some("int"),
            ruff_python_ast::Number::Float(_) => Some("float"),
            ruff_python_ast::Number::Complex { .. } => Some("complex"),
        },
        Expr::StringLiteral(_) => Some("str"),
        Expr::BytesLiteral(_) => Some("bytes"),
        Expr::BooleanLiteral(_) => Some("bool"),
        Expr::NoneLiteral(_) => Some("None"),
        _ => None,
    }
}

/// Check if `actual` is compatible with `expected` for subscript key types.
pub(super) fn types_compatible(actual: &str, expected: &str) -> bool {
    if actual == expected {
        return true;
    }
    // Allow known widening: bool <: int, int <: float.
    matches!(
        (actual, expected),
        ("bool", "int" | "float") | ("int", "float")
    )
}
