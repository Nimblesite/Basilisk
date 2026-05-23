//! Bounded `TypeVar` attribute access violation collector.
//!
//! When a PEP 695 type parameter has a bound (e.g., `T: str`), attribute
//! accesses on parameters typed as `T` must be valid for the bound type.

use ruff_python_ast::{Expr, Stmt, TypeParam};
use ruff_text_size::Ranged;

use crate::scope::{BoundedTypeVarAttrViolation, Span};

/// Convert a `TextRange` into a resolver `Span`.
const fn text_range_to_span(range: ruff_text_size::TextRange) -> Span {
    Span {
        start: range.start().to_u32(),
        end: range.end().to_u32(),
    }
}

const STR_ATTRS: &[&str] = &[
    "capitalize",
    "casefold",
    "center",
    "count",
    "encode",
    "endswith",
    "expandtabs",
    "find",
    "format",
    "format_map",
    "index",
    "isalnum",
    "isalpha",
    "isascii",
    "isdecimal",
    "isdigit",
    "isidentifier",
    "islower",
    "isnumeric",
    "isprintable",
    "isspace",
    "istitle",
    "isupper",
    "join",
    "ljust",
    "lower",
    "lstrip",
    "maketrans",
    "partition",
    "removeprefix",
    "removesuffix",
    "replace",
    "rfind",
    "rindex",
    "rjust",
    "rpartition",
    "rsplit",
    "rstrip",
    "split",
    "splitlines",
    "startswith",
    "strip",
    "swapcase",
    "title",
    "translate",
    "upper",
    "zfill",
    "__add__",
    "__contains__",
    "__eq__",
    "__ge__",
    "__getitem__",
    "__gt__",
    "__hash__",
    "__iter__",
    "__le__",
    "__len__",
    "__lt__",
    "__mod__",
    "__mul__",
    "__ne__",
    "__repr__",
    "__rmul__",
    "__str__",
];

const INT_ATTRS: &[&str] = &[
    "bit_length",
    "bit_count",
    "to_bytes",
    "from_bytes",
    "as_integer_ratio",
    "is_integer",
    "conjugate",
    "denominator",
    "imag",
    "numerator",
    "real",
    "__abs__",
    "__add__",
    "__and__",
    "__bool__",
    "__ceil__",
    "__divmod__",
    "__eq__",
    "__float__",
    "__floor__",
    "__floordiv__",
    "__ge__",
    "__gt__",
    "__hash__",
    "__index__",
    "__int__",
    "__invert__",
    "__le__",
    "__lshift__",
    "__lt__",
    "__mod__",
    "__mul__",
    "__ne__",
    "__neg__",
    "__or__",
    "__pos__",
    "__pow__",
    "__radd__",
    "__rand__",
    "__rdivmod__",
    "__repr__",
    "__rfloordiv__",
    "__rlshift__",
    "__rmod__",
    "__rmul__",
    "__ror__",
    "__rpow__",
    "__rrshift__",
    "__rshift__",
    "__rsub__",
    "__rtruediv__",
    "__rxor__",
    "__str__",
    "__sub__",
    "__truediv__",
    "__trunc__",
    "__xor__",
];

const FLOAT_ATTRS: &[&str] = &[
    "as_integer_ratio",
    "conjugate",
    "fromhex",
    "hex",
    "imag",
    "is_integer",
    "real",
    "__abs__",
    "__add__",
    "__bool__",
    "__ceil__",
    "__divmod__",
    "__eq__",
    "__float__",
    "__floor__",
    "__floordiv__",
    "__ge__",
    "__gt__",
    "__hash__",
    "__int__",
    "__le__",
    "__lt__",
    "__mod__",
    "__mul__",
    "__ne__",
    "__neg__",
    "__pos__",
    "__pow__",
    "__radd__",
    "__rdivmod__",
    "__repr__",
    "__rfloordiv__",
    "__rmod__",
    "__rmul__",
    "__round__",
    "__rpow__",
    "__rsub__",
    "__rtruediv__",
    "__str__",
    "__sub__",
    "__truediv__",
    "__trunc__",
];

const BYTES_ATTRS: &[&str] = &[
    "capitalize",
    "center",
    "count",
    "decode",
    "endswith",
    "expandtabs",
    "find",
    "fromhex",
    "hex",
    "index",
    "isalnum",
    "isalpha",
    "isascii",
    "isdigit",
    "islower",
    "isspace",
    "istitle",
    "isupper",
    "join",
    "ljust",
    "lower",
    "lstrip",
    "maketrans",
    "partition",
    "removeprefix",
    "removesuffix",
    "replace",
    "rfind",
    "rindex",
    "rjust",
    "rpartition",
    "rsplit",
    "rstrip",
    "split",
    "splitlines",
    "startswith",
    "strip",
    "swapcase",
    "title",
    "translate",
    "upper",
    "zfill",
    "__add__",
    "__contains__",
    "__eq__",
    "__ge__",
    "__getitem__",
    "__gt__",
    "__hash__",
    "__iter__",
    "__le__",
    "__len__",
    "__lt__",
    "__mod__",
    "__mul__",
    "__ne__",
    "__repr__",
    "__rmul__",
    "__str__",
];

const LIST_ATTRS: &[&str] = &[
    "append",
    "clear",
    "copy",
    "count",
    "extend",
    "index",
    "insert",
    "pop",
    "remove",
    "reverse",
    "sort",
    "__add__",
    "__contains__",
    "__delitem__",
    "__eq__",
    "__ge__",
    "__getitem__",
    "__gt__",
    "__iadd__",
    "__imul__",
    "__iter__",
    "__le__",
    "__len__",
    "__lt__",
    "__mul__",
    "__ne__",
    "__repr__",
    "__reversed__",
    "__rmul__",
    "__setitem__",
    "__str__",
];

const DICT_ATTRS: &[&str] = &[
    "clear",
    "copy",
    "fromkeys",
    "get",
    "items",
    "keys",
    "pop",
    "popitem",
    "setdefault",
    "update",
    "values",
    "__contains__",
    "__delitem__",
    "__eq__",
    "__ge__",
    "__getitem__",
    "__gt__",
    "__iter__",
    "__le__",
    "__len__",
    "__lt__",
    "__ne__",
    "__or__",
    "__repr__",
    "__reversed__",
    "__ror__",
    "__setitem__",
    "__str__",
];

/// Returns the set of known attributes for a given built-in type name.
fn known_attrs_for_type(type_name: &str) -> Option<&'static [&'static str]> {
    match type_name {
        "str" => Some(STR_ATTRS),
        "int" => Some(INT_ATTRS),
        "float" => Some(FLOAT_ATTRS),
        "bytes" => Some(BYTES_ATTRS),
        "list" => Some(LIST_ATTRS),
        "dict" => Some(DICT_ATTRS),
        _ => None,
    }
}

/// Collect attribute accesses on bounded PEP 695 type variables where the
/// accessed attribute does not exist on the bound type.
pub(crate) fn collect(stmts: &[Stmt]) -> Vec<BoundedTypeVarAttrViolation> {
    let mut out = Vec::new();
    for stmt in stmts {
        let Stmt::ClassDef(cls) = stmt else { continue };
        let Some(type_params) = &cls.type_params else {
            continue;
        };
        let mut typevar_bounds: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for tp in &type_params.type_params {
            if let TypeParam::TypeVar(tv) = tp {
                if let Some(bound) = &tv.bound {
                    if let Expr::Name(name) = bound.as_ref() {
                        let _ = typevar_bounds.insert(tv.name.to_string(), name.id.to_string());
                    }
                }
            }
        }
        if typevar_bounds.is_empty() {
            continue;
        }
        for body_stmt in &cls.body {
            let Stmt::FunctionDef(func) = body_stmt else {
                continue;
            };
            let mut param_typevar: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            for param in func
                .parameters
                .as_ref()
                .args
                .iter()
                .chain(func.parameters.as_ref().kwonlyargs.iter())
            {
                let param_name = param.parameter.name.to_string();
                if let Some(ann) = &param.parameter.annotation {
                    if let Expr::Name(name) = ann.as_ref() {
                        let ann_name = name.id.to_string();
                        if typevar_bounds.contains_key(&ann_name) {
                            let _ = param_typevar.insert(param_name, ann_name);
                        }
                    }
                }
            }
            if param_typevar.is_empty() {
                continue;
            }
            walk_stmts(&func.body, &param_typevar, &typevar_bounds, &mut out);
        }
    }
    out
}

/// Recursively walk statements for attribute accesses on bounded typevar params.
fn walk_stmts(
    stmts: &[Stmt],
    param_typevar: &std::collections::HashMap<String, String>,
    typevar_bounds: &std::collections::HashMap<String, String>,
    out: &mut Vec<BoundedTypeVarAttrViolation>,
) {
    crate::walk_function_stmts(stmts, &mut |stmt| match stmt {
        Stmt::Expr(node) => walk_expr(&node.value, param_typevar, typevar_bounds, out),
        Stmt::Return(node) => {
            if let Some(val) = &node.value {
                walk_expr(val, param_typevar, typevar_bounds, out);
            }
        }
        Stmt::Assign(node) => walk_expr(&node.value, param_typevar, typevar_bounds, out),
        Stmt::AnnAssign(node) => {
            if let Some(val) = &node.value {
                walk_expr(val, param_typevar, typevar_bounds, out);
            }
        }
        _ => {}
    });
}

/// Check a single expression for attribute accesses on bounded typevar params.
fn walk_expr(
    expr: &Expr,
    param_typevar: &std::collections::HashMap<String, String>,
    typevar_bounds: &std::collections::HashMap<String, String>,
    out: &mut Vec<BoundedTypeVarAttrViolation>,
) {
    match expr {
        Expr::Attribute(attr) => {
            if let Expr::Name(name) = attr.value.as_ref() {
                let param_name = name.id.to_string();
                if let Some(typevar_name) = param_typevar.get(&param_name) {
                    if let Some(bound_type) = typevar_bounds.get(typevar_name) {
                        let attr_name = attr.attr.to_string();
                        if let Some(known) = known_attrs_for_type(bound_type) {
                            if !known.contains(&attr_name.as_str()) {
                                out.push(BoundedTypeVarAttrViolation {
                                    span: text_range_to_span(attr.range()),
                                    typevar_name: typevar_name.clone(),
                                    param_name,
                                    bound_type: bound_type.clone(),
                                    attr_name,
                                });
                            }
                        }
                    }
                }
            }
            walk_expr(&attr.value, param_typevar, typevar_bounds, out);
        }
        Expr::Call(call) => {
            walk_expr(&call.func, param_typevar, typevar_bounds, out);
            for arg in &call.arguments.args {
                walk_expr(arg, param_typevar, typevar_bounds, out);
            }
        }
        Expr::BinOp(node) => {
            walk_expr(&node.left, param_typevar, typevar_bounds, out);
            walk_expr(&node.right, param_typevar, typevar_bounds, out);
        }
        Expr::BoolOp(node) => {
            for val in &node.values {
                walk_expr(val, param_typevar, typevar_bounds, out);
            }
        }
        Expr::Compare(node) => {
            walk_expr(&node.left, param_typevar, typevar_bounds, out);
            for comp in &node.comparators {
                walk_expr(comp, param_typevar, typevar_bounds, out);
            }
        }
        Expr::UnaryOp(node) => {
            walk_expr(&node.operand, param_typevar, typevar_bounds, out);
        }
        _ => {}
    }
}
