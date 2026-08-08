//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Module Level visitor functions.

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

use super::class_info_ext::expr_simple_name;
use super::core::text_range_to_span;

pub(super) fn collect_module_attr_accesses(
    stmts: &[Stmt],
) -> Vec<crate::scope::ModuleAttrAccessInfo> {
    let mut out = Vec::new();
    walk_module_level_if(stmts, &mut |stmt| match stmt {
        Stmt::Expr(node) => collect_attr_accesses_from_expr(&node.value, &mut out),
        Stmt::If(node) => {
            collect_attr_accesses_from_expr(&node.test, &mut out);
            for clause in &node.elif_else_clauses {
                if let Some(test) = &clause.test {
                    collect_attr_accesses_from_expr(test, &mut out);
                }
            }
        }
        _ => {}
    });
    out
}

/// Walk module-level statements, descending only into `if` (including elif/else)
/// branches. `for`/`while`/`with`/`try`/function/class bodies are NOT recursed.
/// Used by module-level analyses that conceptually live at the top of the file
/// but want to look through `if TYPE_CHECKING:` style guards.
fn walk_module_level_if(stmts: &[Stmt], visit: &mut impl FnMut(&Stmt)) {
    for stmt in stmts {
        visit(stmt);
        if let Stmt::If(node) = stmt {
            walk_module_level_if(&node.body, visit);
            for clause in &node.elif_else_clauses {
                walk_module_level_if(&clause.body, visit);
            }
        }
    }
}

pub(super) fn collect_attr_accesses_from_expr(
    expr: &Expr,
    out: &mut Vec<crate::scope::ModuleAttrAccessInfo>,
) {
    if let Expr::Attribute(attr) = expr {
        if let Some(object_name) = expr_simple_name(&attr.value) {
            out.push(crate::scope::ModuleAttrAccessInfo {
                object_name,
                attr_name: attr.attr.to_string(),
                span: text_range_to_span(expr.range()),
            });
        }
    }
}

pub(super) fn collect_module_order_comparisons(
    stmts: &[Stmt],
) -> Vec<crate::scope::ModuleOrderComparisonInfo> {
    let mut out = Vec::new();
    walk_module_level_if(stmts, &mut |stmt| match stmt {
        Stmt::Expr(node) => collect_order_comparisons_from_expr(&node.value, &mut out),
        Stmt::If(node) => {
            collect_order_comparisons_from_expr(&node.test, &mut out);
            for clause in &node.elif_else_clauses {
                if let Some(test) = &clause.test {
                    collect_order_comparisons_from_expr(test, &mut out);
                }
            }
        }
        _ => {}
    });
    out
}

pub(super) fn collect_order_comparisons_from_expr(
    expr: &Expr,
    out: &mut Vec<crate::scope::ModuleOrderComparisonInfo>,
) {
    let Expr::Compare(cmp) = expr else { return };
    let Some(left_name) = expr_simple_name(&cmp.left) else {
        return;
    };
    for (op, comparator) in cmp.ops.iter().zip(cmp.comparators.iter()) {
        let scope_op = match op {
            ruff_python_ast::CmpOp::Lt => crate::scope::CompareOp::Lt,
            ruff_python_ast::CmpOp::LtE => crate::scope::CompareOp::LtE,
            ruff_python_ast::CmpOp::Gt => crate::scope::CompareOp::Gt,
            ruff_python_ast::CmpOp::GtE => crate::scope::CompareOp::GtE,
            _ => continue,
        };
        let Some(right_name) = expr_simple_name(comparator) else {
            continue;
        };
        out.push(crate::scope::ModuleOrderComparisonInfo {
            left_name: left_name.clone(),
            right_name,
            op: scope_op,
            span: text_range_to_span(expr.range()),
        });
    }
}

// ---------------------------------------------------------------------------
// PEP 695 `type X = rhs` statement collection
// ---------------------------------------------------------------------------
