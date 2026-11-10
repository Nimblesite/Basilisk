//! Implements [`specialtypes_type`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! AST utility helpers and predicate functions for `specialtypes_type`.

use ruff_python_ast::Expr;

/// Special-form names that are not valid class objects for `type[T]`.
pub(super) const SPECIAL_FORMS: &[&str] = &[
    "Callable",
    "Union",
    "Optional",
    "ClassVar",
    "Final",
    "Literal",
    "Annotated",
    "TypeGuard",
    "TypeIs",
    "Never",
    "NoReturn",
    "LiteralString",
    "Self",
    "Unpack",
    "TypeVarTuple",
    "ParamSpec",
    "Concatenate",
    "Required",
    "NotRequired",
    "ReadOnly",
    "TypeAlias",
];

/// Known attributes on `type` / `object` (the metaclass API) that are always
/// legal to access on `type[object]` or a plain `type` annotation.
pub(super) const KNOWN_TYPE_ATTRS: &[&str] = &[
    "__name__",
    "__qualname__",
    "__module__",
    "__bases__",
    "__mro__",
    "__subclasses__",
    "__doc__",
    "__dict__",
    "__slots__",
    "__annotations__",
    "__class__",
    "__init__",
    "__new__",
    "__repr__",
    "__str__",
    "__hash__",
    "__eq__",
    "__ne__",
    "__lt__",
    "__le__",
    "__gt__",
    "__ge__",
    "__abstractmethods__",
    "mro",
    "__init_subclass__",
    "__subclasshook__",
];

/// Returns `true` if `ann` is a `type[…]` or `Type[…]` annotation of any form.
pub(super) fn is_any_type_annotation(ann: &str) -> bool {
    strip_type_bracket(ann).is_some()
}

/// Strip the `type[` / `Type[` prefix + `]` suffix and return the inner text,
/// or `None` if the annotation is not of this form.
pub(super) fn strip_type_bracket(ann: &str) -> Option<&str> {
    let ann = ann.trim();
    let inner = ann
        .strip_prefix("type[")
        .or_else(|| ann.strip_prefix("Type["))?;
    inner.strip_suffix(']')
}

/// Returns `true` if `ann` is a `type[X]` where `X` is a **concrete** (non-Any,
/// non-TypeVar) type — e.g. `type[object]`, `Type[object]`, `type[int]`.
pub(super) fn is_concrete_type_annotation(ann: &str) -> bool {
    let Some(inner) = strip_type_bracket(ann) else {
        return false;
    };
    let inner = inner.trim();
    if inner == "Any" || inner.len() == 1 && inner.chars().next().is_some_and(char::is_uppercase) {
        return false;
    }
    matches!(inner, "object" | "int" | "str" | "float" | "bool" | "bytes")
}

/// Returns `true` if `rhs` (the right-hand side of a `TypeAlias`) is a
/// `type` or `Type` annotation (bare or parameterised).
pub(super) fn is_type_annotation(rhs: &str) -> bool {
    let rhs = rhs.trim();
    matches!(rhs, "type" | "Type") || rhs.starts_with("type[") || rhs.starts_with("Type[")
}

/// Returns `true` if `attr` is a well-known attribute on the `type` metaclass.
pub(super) fn is_known_type_attr(attr: &str) -> bool {
    KNOWN_TYPE_ATTRS.contains(&attr)
}

/// Extract the simple name string from a `Name` expression.
pub(super) fn expr_simple_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(n) => Some(n.id.as_str()),
        _ => None,
    }
}

/// Returns `true` if the expression is a `TypeVar(...)` call.
pub(super) fn is_typevar_call(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Call(call)
            if matches!(call.func.as_ref(), Expr::Name(n) if n.id.as_str() == "TypeVar")
    )
}

/// Convert an expression to a readable annotation string (best-effort).
pub(super) fn expr_to_str(expr: &Expr) -> String {
    match expr {
        Expr::Name(n) => n.id.to_string(),
        Expr::Subscript(s) => format!("{}[{}]", expr_to_str(&s.value), expr_to_str(&s.slice)),
        Expr::Attribute(a) => format!("{}.{}", expr_to_str(&a.value), a.attr),
        Expr::Tuple(t) => t
            .elts
            .iter()
            .map(expr_to_str)
            .collect::<Vec<_>>()
            .join(", "),
        Expr::BinOp(b) => format!("{} | {}", expr_to_str(&b.left), expr_to_str(&b.right)),
        Expr::NoneLiteral(_) => "None".to_owned(),
        Expr::EllipsisLiteral(_) => "...".to_owned(),
        _ => String::new(),
    }
}
