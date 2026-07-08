//! Implements [`directives_deprecated`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! Statement visitor for `directives_deprecated`.
//!
//! Contains `visit_stmt_for_usage` and helpers for assignment-related
//! deprecated checks.

use std::collections::HashMap;

use ruff_python_ast::{Expr, Operator, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::Span;

use crate::diagnostic::Diagnostic;

use super::collect::{collect_param_annotation_types, DeprecatedInfo};
use super::decorators::text_range_to_span;
use super::types::{DeprecatedUsageContext, VarType};
use super::visit_expr::{
    check_dunder_deprecated_on_type, check_setter_deprecated_on_type, op_to_dunder,
    visit_expr_for_usage,
};

/// Visit a statement looking for deprecated name usages.
#[expect(
    clippy::too_many_lines,
    reason = "statement visitor covers all statement variants"
)]
pub(super) fn visit_stmt_for_usage(
    stmt: &Stmt,
    ctx: &DeprecatedUsageContext<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match stmt {
        Stmt::Expr(expr_stmt) => {
            visit_expr_for_usage(
                &expr_stmt.value,
                ctx.deprecated,
                ctx.module_aliases,
                ctx.deprecated_members,
                ctx.var_types,
                ctx.path,
                diagnostics,
            );
        }
        Stmt::Assign(assign) => {
            visit_expr_for_usage(
                &assign.value,
                ctx.deprecated,
                ctx.module_aliases,
                ctx.deprecated_members,
                ctx.var_types,
                ctx.path,
                diagnostics,
            );
            for target in &assign.targets {
                // Check for deprecated property setter via assignment target (e.g. `spam.shape = ...`).
                check_assignment_target_deprecated(
                    target,
                    ctx.deprecated,
                    ctx.deprecated_members,
                    ctx.var_types,
                    ctx.path,
                    diagnostics,
                );
            }
        }
        Stmt::AugAssign(aug) => {
            // `spam += 1` triggers __add__; `spam.shape += "cube"` triggers property setter.
            visit_expr_for_usage(
                &aug.value,
                ctx.deprecated,
                ctx.module_aliases,
                ctx.deprecated_members,
                ctx.var_types,
                ctx.path,
                diagnostics,
            );
            check_aug_assign_deprecated(
                &aug.target,
                aug.op,
                ctx.deprecated,
                ctx.deprecated_members,
                ctx.var_types,
                ctx.path,
                diagnostics,
                text_range_to_span(aug.range()),
            );
        }
        Stmt::AnnAssign(ann) => {
            if let Some(value) = &ann.value {
                visit_expr_for_usage(
                    value,
                    ctx.deprecated,
                    ctx.module_aliases,
                    ctx.deprecated_members,
                    ctx.var_types,
                    ctx.path,
                    diagnostics,
                );
            }
        }
        Stmt::Return(ret) => {
            if let Some(value) = &ret.value {
                visit_expr_for_usage(
                    value,
                    ctx.deprecated,
                    ctx.module_aliases,
                    ctx.deprecated_members,
                    ctx.var_types,
                    ctx.path,
                    diagnostics,
                );
            }
        }
        Stmt::FunctionDef(func) => {
            // Create a scoped var_types with parameter annotations so that
            // e.g. `def foo(f: Deprecated)` resolves `f` to the right class
            // without polluting the outer scope.
            let mut scoped_var_types = ctx.var_types.clone();
            collect_param_annotation_types(func, &mut scoped_var_types);
            let scoped_ctx = DeprecatedUsageContext {
                deprecated: ctx.deprecated,
                module_aliases: ctx.module_aliases,
                deprecated_members: ctx.deprecated_members,
                var_types: &scoped_var_types,
                path: ctx.path,
                def_spans: ctx.def_spans,
            };
            for body_stmt in &func.body {
                visit_stmt_for_usage(body_stmt, &scoped_ctx, diagnostics);
            }
        }
        Stmt::ClassDef(cls) => {
            for body_stmt in &cls.body {
                visit_stmt_for_usage(body_stmt, ctx, diagnostics);
            }
        }
        Stmt::If(if_stmt) => {
            visit_expr_for_usage(
                &if_stmt.test,
                ctx.deprecated,
                ctx.module_aliases,
                ctx.deprecated_members,
                ctx.var_types,
                ctx.path,
                diagnostics,
            );
            for body_stmt in &if_stmt.body {
                visit_stmt_for_usage(body_stmt, ctx, diagnostics);
            }
            for elif in &if_stmt.elif_else_clauses {
                for body_stmt in &elif.body {
                    visit_stmt_for_usage(body_stmt, ctx, diagnostics);
                }
            }
        }
        Stmt::For(for_stmt) => {
            visit_expr_for_usage(
                &for_stmt.iter,
                ctx.deprecated,
                ctx.module_aliases,
                ctx.deprecated_members,
                ctx.var_types,
                ctx.path,
                diagnostics,
            );
            for body_stmt in &for_stmt.body {
                visit_stmt_for_usage(body_stmt, ctx, diagnostics);
            }
        }
        Stmt::While(while_stmt) => {
            visit_expr_for_usage(
                &while_stmt.test,
                ctx.deprecated,
                ctx.module_aliases,
                ctx.deprecated_members,
                ctx.var_types,
                ctx.path,
                diagnostics,
            );
            for body_stmt in &while_stmt.body {
                visit_stmt_for_usage(body_stmt, ctx, diagnostics);
            }
        }
        _ => {}
    }
}

/// Check if an assignment target accesses a deprecated property setter.
///
/// Handles `spam.shape = "cube"` where `spam` has been inferred as type `Spam`.
fn check_assignment_target_deprecated(
    target: &Expr,
    deprecated: &HashMap<String, DeprecatedInfo>,
    deprecated_members: &HashMap<String, HashMap<String, DeprecatedInfo>>,
    var_types: &HashMap<String, VarType>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Expr::Attribute(attr) = target {
        let member_name = attr.attr.as_str();
        if let Expr::Name(obj_name) = attr.value.as_ref() {
            let var_name = obj_name.id.as_str();
            if let Some(var_type) = var_types.get(var_name) {
                let span = text_range_to_span(target.range());
                check_setter_deprecated_on_type(
                    var_type,
                    member_name,
                    deprecated,
                    deprecated_members,
                    path,
                    diagnostics,
                    span,
                );
            }
        }
    }
}

/// Check augmented assignment for deprecated usage.
///
/// - `spam += 1` triggers the deprecated `__add__` method on `spam`'s type.
/// - `spam.shape += "cube"` triggers the deprecated property setter.
#[expect(
    clippy::too_many_arguments,
    reason = "deprecated usage check requires full context"
)]
fn check_aug_assign_deprecated(
    target: &Expr,
    op: Operator,
    deprecated: &HashMap<String, DeprecatedInfo>,
    deprecated_members: &HashMap<String, HashMap<String, DeprecatedInfo>>,
    var_types: &HashMap<String, VarType>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
    span: Span,
) {
    match target {
        Expr::Name(name) => {
            let var_name = name.id.as_str();
            if let Some(var_type) = var_types.get(var_name) {
                let dunder = op_to_dunder(op);
                check_dunder_deprecated_on_type(
                    var_type,
                    dunder,
                    deprecated,
                    deprecated_members,
                    path,
                    diagnostics,
                    span,
                );
            }
        }
        Expr::Attribute(attr) => {
            let member_name = attr.attr.as_str();
            if let Expr::Name(obj_name) = attr.value.as_ref() {
                let var_name = obj_name.id.as_str();
                if let Some(var_type) = var_types.get(var_name) {
                    check_setter_deprecated_on_type(
                        var_type,
                        member_name,
                        deprecated,
                        deprecated_members,
                        path,
                        diagnostics,
                        span,
                    );
                }
            }
        }
        _ => {}
    }
}
