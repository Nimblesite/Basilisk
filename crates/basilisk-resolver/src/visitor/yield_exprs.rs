//! Yield Exprs visitor functions.

use ruff_python_ast::{Expr, Stmt};

use crate::scope::RhsKind;

use super::calls_and_reveal::extract_call_name;
use super::core::{classify_rhs, text_range_to_span};

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
    for stmt in stmts {
        collect_yield_exprs_from_stmt(stmt, &mut out);
    }
    out
}

/// Recursively collect yield expressions from a single statement.
pub(super) fn collect_yield_exprs_from_stmt(
    stmt: &Stmt,
    out: &mut Vec<crate::scope::YieldExprInfo>,
) {
    match stmt {
        Stmt::Expr(node) => collect_yield_from_expr(&node.value, out),
        Stmt::Assign(node) => collect_yield_from_expr(&node.value, out),
        Stmt::AnnAssign(node) => {
            if let Some(value) = &node.value {
                collect_yield_from_expr(value, out);
            }
        }
        Stmt::If(node) => {
            for s in &node.body {
                collect_yield_exprs_from_stmt(s, out);
            }
            for clause in &node.elif_else_clauses {
                for s in &clause.body {
                    collect_yield_exprs_from_stmt(s, out);
                }
            }
        }
        Stmt::While(node) => {
            for s in &node.body {
                collect_yield_exprs_from_stmt(s, out);
            }
        }
        Stmt::For(node) => {
            for s in &node.body {
                collect_yield_exprs_from_stmt(s, out);
            }
        }
        Stmt::With(node) => {
            for s in &node.body {
                collect_yield_exprs_from_stmt(s, out);
            }
        }
        Stmt::Try(node) => {
            for s in &node.body {
                collect_yield_exprs_from_stmt(s, out);
            }
            for s in &node.finalbody {
                collect_yield_exprs_from_stmt(s, out);
            }
            for s in &node.orelse {
                collect_yield_exprs_from_stmt(s, out);
            }
        }
        // Do NOT recurse into nested function defs
        _ => {}
    }
}

/// Extract yield info from an expression node.
pub(super) fn collect_yield_from_expr(expr: &Expr, out: &mut Vec<crate::scope::YieldExprInfo>) {
    match expr {
        Expr::Yield(y) => {
            let (rhs_kind, call_name) = y.value.as_ref().map_or((RhsKind::NoneValue, None), |v| {
                (classify_rhs(v), extract_call_name(v))
            });
            out.push(crate::scope::YieldExprInfo {
                span: text_range_to_span(y.range),
                rhs_kind,
                is_yield_from: false,
                call_name,
            });
        }
        Expr::YieldFrom(yf) => {
            let rhs_kind = classify_rhs(&yf.value);
            let call_name = extract_call_name(&yf.value);
            out.push(crate::scope::YieldExprInfo {
                span: text_range_to_span(yf.range),
                rhs_kind,
                is_yield_from: true,
                call_name,
            });
        }
        _ => {}
    }
}
