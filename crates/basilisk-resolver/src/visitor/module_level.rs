//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Module Level visitor functions.

use ruff_python_ast::{Expr, Stmt, StmtFunctionDef};
use ruff_text_size::Ranged;

use crate::scope::{FloatParamIntAttrAccess, FunctionInfo};

use super::class_info_ext::expr_simple_name;
use super::core::{source_slice_range, source_slice_span, text_range_to_span};
use super::enum_checks::INT_ONLY_FLOAT_ATTRS;
use super::protocol_ext::{base_type_name, unqualified_base};

pub(crate) fn collect_float_param_int_attr_accesses(
    stmts: &[Stmt],
    source: &str,
) -> Vec<FloatParamIntAttrAccess> {
    let mut out = Vec::new();
    for stmt in stmts {
        if let Stmt::FunctionDef(func) = stmt {
            collect_float_accesses_in_function(func, source, &mut out);
            // Recurse into nested functions.
            out.extend(collect_float_param_int_attr_accesses(&func.body, source));
        }
    }
    out
}

pub(super) fn collect_float_accesses_in_function(
    func: &StmtFunctionDef,
    source: &str,
    out: &mut Vec<FloatParamIntAttrAccess>,
) {
    // Collect names of parameters annotated exactly as `float`.
    let float_params: Vec<&str> = super::walks::iter_all_params(&func.parameters)
        .filter(|p| {
            p.parameter.annotation.as_deref().is_some_and(|ann| {
                let range = ann.range();
                source_slice_range(source, range).map(str::trim) == Some("float")
            })
        })
        .map(|p| p.parameter.name.as_str())
        .collect();

    if float_params.is_empty() {
        return;
    }

    // Only walk the top-level statements of the function body (no recursion into
    // if/for/while/match/with/try blocks).
    for stmt in &func.body {
        let Stmt::Expr(expr_stmt) = stmt else {
            continue;
        };
        let Expr::Attribute(attr) = expr_stmt.value.as_ref() else {
            continue;
        };
        let Some(obj_name) = expr_simple_name(&attr.value) else {
            continue;
        };
        if !float_params.contains(&obj_name.as_str()) {
            continue;
        }
        let attr_name = attr.attr.as_str();
        if !INT_ONLY_FLOAT_ATTRS.contains(&attr_name) {
            continue;
        }
        out.push(FloatParamIntAttrAccess {
            param_name: obj_name,
            attr_name: attr_name.to_owned(),
            span: text_range_to_span(expr_stmt.range()),
        });
    }
}

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

pub(super) fn collect_generator_violations(
    functions: &[FunctionInfo],
    source: &str,
) -> Vec<crate::scope::GeneratorViolation> {
    let mut out = Vec::new();
    for func in functions {
        if !func.is_generator {
            continue;
        }
        let Some(ann_span) = func.return_annotation_span else {
            continue;
        };
        let Some(ann_text) = source_slice_span(source, ann_span) else {
            continue;
        };
        let base = unqualified_base(base_type_name(ann_text));

        // Skip user-defined types (may be Protocols with __next__/__anext__).
        // Only flag types that are clearly non-generator built-in types.
        let first_char = base.chars().next();
        if first_char.is_some_and(char::is_uppercase) {
            continue;
        }

        out.push(crate::scope::GeneratorViolation {
            span: ann_span,
            kind: crate::scope::GeneratorViolationKind::InvalidReturnType {
                func_name: func.name.clone(),
                return_type: ann_text.to_owned(),
            },
        });
    }
    out
}

// ---------------------------------------------------------------------------
// PEP 695 `type X = rhs` statement collection
// ---------------------------------------------------------------------------
