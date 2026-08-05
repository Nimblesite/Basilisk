//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Yield Exprs visitor functions.

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

use crate::scope::RhsKind;

use super::core::{classify_rhs, text_range_to_span};

/// The callee name of a direct call expression (`f(...)`).
///
/// Attribute calls (`a.b(...)`) yield `None`: their bare method name is not a
/// module-scope callee, and downstream consumers resolve `call_name` against
/// module-level definitions — treating `NAME_SYNONYMS.get(...)` as a callee
/// named `get` produced name-as-type false positives (GitHub #281).
fn direct_call_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Call(call) => match call.func.as_ref() {
            Expr::Name(n) => Some(n.id.to_string()),
            _ => None,
        },
        _ => None,
    }
}

pub(super) fn stmt_contains_yield(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Expr(node) => matches!(node.value.as_ref(), Expr::Yield(_) | Expr::YieldFrom(_)),
        Stmt::Assign(node) => matches!(node.value.as_ref(), Expr::Yield(_) | Expr::YieldFrom(_)),
        Stmt::If(node) => {
            node.body.iter().any(stmt_contains_yield)
                || node
                    .elif_else_clauses
                    .iter()
                    .any(|c| c.body.iter().any(stmt_contains_yield))
        }
        Stmt::While(node) => node.body.iter().any(stmt_contains_yield),
        Stmt::For(node) => node.body.iter().any(stmt_contains_yield),
        Stmt::With(node) => node.body.iter().any(stmt_contains_yield),
        Stmt::Try(node) => {
            node.body.iter().any(stmt_contains_yield)
                || node.finalbody.iter().any(stmt_contains_yield)
                || node.orelse.iter().any(stmt_contains_yield)
        }
        _ => false,
    }
}

/// Collect all yield/yield-from expressions from a function body (recursively).
pub(super) fn collect_yield_exprs(stmts: &[Stmt]) -> Vec<crate::scope::YieldExprInfo> {
    let mut out = Vec::new();
    super::walks::walk_function_stmts(stmts, &mut |stmt| match stmt {
        Stmt::Expr(node) => collect_yield_from_expr(&node.value, &mut out),
        Stmt::Assign(node) => collect_yield_from_expr(&node.value, &mut out),
        Stmt::AnnAssign(node) => {
            if let Some(value) = &node.value {
                collect_yield_from_expr(value, &mut out);
            }
        }
        _ => {}
    });
    out
}

/// Extract yield info from an expression node.
pub(super) fn collect_yield_from_expr(expr: &Expr, out: &mut Vec<crate::scope::YieldExprInfo>) {
    match expr {
        Expr::Yield(y) => {
            let (rhs_kind, call_name) = y.value.as_ref().map_or((RhsKind::NoneValue, None), |v| {
                (classify_rhs(v), direct_call_name(v))
            });
            out.push(crate::scope::YieldExprInfo {
                span: text_range_to_span(y.range),
                rhs_kind,
                is_yield_from: false,
                call_name,
                value_span: y.value.as_deref().map(|value| text_range_to_span(value.range())),
            });
        }
        Expr::YieldFrom(yf) => {
            let rhs_kind = classify_rhs(&yf.value);
            let call_name = direct_call_name(&yf.value);
            out.push(crate::scope::YieldExprInfo {
                span: text_range_to_span(yf.range),
                rhs_kind,
                is_yield_from: true,
                call_name,
                value_span: Some(text_range_to_span(yf.value.range())),
            });
        }
        _ => {}
    }
}
