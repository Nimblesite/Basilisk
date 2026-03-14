//! Decorator detection helpers for BSK-E0115.

use ruff_python_ast::Expr;
use ruff_text_size::Ranged;

use basilisk_resolver::Span;

/// Convert a `ruff_text_size::TextRange` to a [`Span`].
pub(super) fn text_range_to_span(range: ruff_text_size::TextRange) -> Span {
    Span {
        start: range.start().to_u32(),
        end: range.end().to_u32(),
    }
}

/// Check if a decorator expression is `@deprecated(...)`.
///
/// Returns `None` if not a deprecated decorator, `Some(None)` if deprecated
/// without a message, and `Some(Some(msg))` if deprecated with a message.
#[expect(
    clippy::option_option,
    reason = "None=not deprecated, Some(None)=deprecated without message, Some(Some(msg))=deprecated with message"
)]
pub(super) fn is_deprecated_decorator(expr: &Expr) -> Option<Option<String>> {
    match expr {
        Expr::Call(call) => {
            let is_deprecated_name = match call.func.as_ref() {
                Expr::Name(name) => name.id.as_str() == "deprecated",
                Expr::Attribute(attr) => {
                    attr.attr.as_str() == "deprecated"
                        && matches!(attr.value.as_ref(), Expr::Name(n) if n.id.as_str() == "typing" || n.id.as_str() == "typing_extensions")
                }
                _ => false,
            };
            if !is_deprecated_name {
                return None;
            }
            // Extract the message from the first positional argument.
            let message = call.arguments.args.first().and_then(|arg| {
                if let Expr::StringLiteral(s) = arg {
                    Some(s.value.to_string())
                } else {
                    None
                }
            });
            Some(message)
        }
        Expr::Name(name) if name.id.as_str() == "deprecated" => Some(None),
        Expr::Attribute(attr)
            if attr.attr.as_str() == "deprecated"
                && matches!(attr.value.as_ref(), Expr::Name(n) if n.id.as_str() == "typing" || n.id.as_str() == "typing_extensions") =>
        {
            Some(None)
        }
        _ => None,
    }
}
