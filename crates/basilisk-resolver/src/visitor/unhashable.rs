//! Unhashable visitor functions.

use ruff_python_ast::{ExceptHandler, Expr, Stmt};
use ruff_text_size::Ranged;

use crate::scope::UnhashableKeyRef;

use super::calls_and_reveal::collect_unhashable_hash_calls_from_stmt;
use super::core::text_range_to_span;

pub(super) fn collect_unhashable_keys_from_stmts(stmts: &[Stmt]) -> Vec<UnhashableKeyRef> {
    let mut out = Vec::new();
    for stmt in stmts {
        collect_unhashable_keys_from_stmt(stmt, &mut out);
    }
    out
}

#[allow(clippy::too_many_lines)]
pub(super) fn collect_unhashable_keys_from_stmt(stmt: &Stmt, out: &mut Vec<UnhashableKeyRef>) {
    match stmt {
        Stmt::Assign(node) => collect_unhashable_keys_from_expr(&node.value, out),
        Stmt::AnnAssign(node) => {
            if let Some(val) = node.value.as_deref() {
                collect_unhashable_keys_from_expr(val, out);
            }
        }
        Stmt::Return(node) => {
            if let Some(val) = node.value.as_deref() {
                collect_unhashable_keys_from_expr(val, out);
            }
        }
        Stmt::Expr(node) => collect_unhashable_keys_from_expr(&node.value, out),
        Stmt::If(node) => {
            collect_unhashable_keys_from_expr(&node.test, out);
            for s in &node.body {
                collect_unhashable_keys_from_stmt(s, out);
            }
            for clause in &node.elif_else_clauses {
                if let Some(test) = &clause.test {
                    collect_unhashable_keys_from_expr(test, out);
                }
                for s in &clause.body {
                    collect_unhashable_keys_from_stmt(s, out);
                }
            }
        }
        Stmt::For(node) => {
            collect_unhashable_keys_from_expr(&node.iter, out);
            for s in &node.body {
                collect_unhashable_keys_from_stmt(s, out);
            }
        }
        Stmt::While(node) => {
            collect_unhashable_keys_from_expr(&node.test, out);
            for s in &node.body {
                collect_unhashable_keys_from_stmt(s, out);
            }
        }
        Stmt::With(node) => {
            for s in &node.body {
                collect_unhashable_keys_from_stmt(s, out);
            }
        }
        Stmt::Try(node) => {
            for s in &node.body {
                collect_unhashable_keys_from_stmt(s, out);
            }
            for handler in &node.handlers {
                let ExceptHandler::ExceptHandler(h) = handler;
                for s in &h.body {
                    collect_unhashable_keys_from_stmt(s, out);
                }
            }
        }
        // Do NOT recurse into nested FunctionDef.
        _ => {}
    }
}

pub(super) fn collect_unhashable_keys_from_expr(expr: &Expr, out: &mut Vec<UnhashableKeyRef>) {
    match expr {
        Expr::Dict(dict) => {
            for item in &dict.items {
                if let Some(key) = item.key.as_ref() {
                    let key_type_opt = match key {
                        Expr::List(_) => Some("list"),
                        Expr::Set(_) => Some("set"),
                        Expr::Dict(_) => Some("dict"),
                        _ => None,
                    };
                    if let Some(key_type) = key_type_opt {
                        out.push(UnhashableKeyRef {
                            span: text_range_to_span(key.range()),
                            key_type,
                        });
                    }
                    collect_unhashable_keys_from_expr(key, out);
                }
                collect_unhashable_keys_from_expr(&item.value, out);
            }
        }
        Expr::List(list) => {
            for elt in &list.elts {
                collect_unhashable_keys_from_expr(elt, out);
            }
        }
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                collect_unhashable_keys_from_expr(elt, out);
            }
        }
        Expr::Call(call) => {
            collect_unhashable_keys_from_expr(&call.func, out);
            for arg in &call.arguments.args {
                collect_unhashable_keys_from_expr(arg, out);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Module-level call site collection
// ---------------------------------------------------------------------------

/// Returns the TypeVar-like callee name (`"TypeVar"`, `"TypeVarTuple"`, or `"ParamSpec"`),
/// or `None` if the expression is not a TypeVar-like call.
pub(super) fn collect_unhashable_hash_calls(
    stmts: &[Stmt],
    classes: &[crate::scope::ClassInfo],
) -> Vec<crate::scope::UnhashableHashCallViolation> {
    // Build the set of non-hashable dataclass names.
    let non_hashable: std::collections::HashSet<&str> = classes
        .iter()
        .filter(|cls| {
            cls.is_dataclass
                && !cls.is_dataclass_eq_false
                && !cls.is_dataclass_frozen
                && !cls.is_dataclass_unsafe_hash
                && !cls.method_names.iter().any(|m| m == "__hash__")
        })
        .map(|cls| cls.name.as_str())
        .collect();

    if non_hashable.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    for stmt in stmts {
        collect_unhashable_hash_calls_from_stmt(stmt, &non_hashable, &mut out);
    }
    out
}

pub(super) fn collect_unhashable_hash_calls_from_expr(
    expr: &Expr,
    non_hashable: &std::collections::HashSet<&str>,
    out: &mut Vec<crate::scope::UnhashableHashCallViolation>,
) {
    let Expr::Call(outer_call) = expr else {
        return;
    };
    let Expr::Attribute(attr) = outer_call.func.as_ref() else {
        return;
    };
    if attr.attr.as_str() != "__hash__" {
        return;
    }
    // The value of the attribute must be a constructor call: `ClassName(args)`
    let Expr::Call(inner_call) = attr.value.as_ref() else {
        return;
    };
    let Expr::Name(name) = inner_call.func.as_ref() else {
        return;
    };
    if non_hashable.contains(name.id.as_str()) {
        out.push(crate::scope::UnhashableHashCallViolation {
            class_name: name.id.to_string(),
            span: text_range_to_span(expr.range()),
        });
    }
}

// ---------------------------------------------------------------------------
// Module-level ordering comparison collection
// ---------------------------------------------------------------------------
