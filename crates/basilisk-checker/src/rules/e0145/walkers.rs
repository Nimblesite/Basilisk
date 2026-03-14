//! Statement and expression walkers for BSK-E0145.

use std::collections::HashMap;

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::Span;

use crate::diagnostic::{Diagnostic, Severity};

use super::{
    CODE, ModuleCtx,
    helpers::{
        expr_simple_name, expr_to_str, is_any_type_annotation, is_concrete_type_annotation,
        is_known_type_attr, strip_type_bracket,
    },
};

/// Walk all top-level statements dispatching to per-statement checks.
pub(super) fn check_stmts(stmts: &[Stmt], ctx: &ModuleCtx, path: &str, diag: &mut Vec<Diagnostic>) {
    for stmt in stmts {
        check_stmt(stmt, ctx, path, diag);
    }
}

fn check_stmt(stmt: &Stmt, ctx: &ModuleCtx, path: &str, diag: &mut Vec<Diagnostic>) {
    match stmt {
        Stmt::FunctionDef(func) => {
            // Build per-function param annotation map (param_name → ann_text).
            let param_anns: HashMap<String, String> = func
                .parameters
                .args
                .iter()
                .chain(func.parameters.posonlyargs.iter())
                .filter_map(|pwd| {
                    let ann = pwd.parameter.annotation.as_ref().map(|a| expr_to_str(a))?;
                    Some((pwd.parameter.name.to_string(), ann))
                })
                .collect();

            check_func_body(&func.body, ctx, &param_anns, path, diag);

            // Also recurse into nested functions / classes.
            for body_stmt in &func.body {
                if let Stmt::FunctionDef(_) | Stmt::ClassDef(_) = body_stmt {
                    check_stmt(body_stmt, ctx, path, diag);
                }
            }
        }
        Stmt::ClassDef(cls) => {
            for body_stmt in &cls.body {
                check_stmt(body_stmt, ctx, path, diag);
            }
        }
        Stmt::Expr(expr_stmt) => {
            // Module-level bare expression statements (e.g. `TA1.unknown`).
            check_module_expr(&expr_stmt.value, ctx, path, diag);
        }
        _ => {}
    }
}

/// Check bare expression statements at module (or outer) scope for:
/// - `Callable` / special form passed as argument where `type[T]` is expected.
/// - Attribute access on `TypeAlias` names bound to `type`.
fn check_module_expr(expr: &Expr, ctx: &ModuleCtx, path: &str, diag: &mut Vec<Diagnostic>) {
    match expr {
        // ----------------------------------------------------------------
        // func5(Callable) etc. — `type[T]` call with an invalid argument.
        // ----------------------------------------------------------------
        Expr::Call(call) => {
            let Some(callee) = expr_simple_name(&call.func) else {
                return;
            };

            let Some(params) = ctx.func_params.get(callee) else {
                return;
            };

            for (arg_idx, arg_expr) in call.arguments.args.iter().enumerate() {
                let Some((_param_name, ann)) = params.get(arg_idx) else {
                    continue;
                };

                let Some(arg_name) = expr_simple_name(arg_expr) else {
                    continue;
                };

                check_type_arg(arg_name, ann, arg_expr, ctx, path, diag);
            }
        }

        // ----------------------------------------------------------------
        // TA1.unknown etc. — attribute access on a TypeAlias name.
        // ----------------------------------------------------------------
        Expr::Attribute(attr) => {
            if let Some(obj_name) = expr_simple_name(&attr.value) {
                if ctx.is_type_alias(obj_name) && !is_known_type_attr(attr.attr.as_str()) {
                    let span = Span {
                        start: attr.range().start().to_u32(),
                        end: attr.range().end().to_u32(),
                    };
                    diag.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Error,
                        message: format!(
                            "Attribute `{}` is not defined on `{obj_name}` \
                             (a `TypeAlias` of `type`/`Type`)",
                            attr.attr
                        ),
                        span,
                        path: path.to_owned(),
                        help: Some(format!(
                            "`{obj_name}` is a `TypeAlias` for a `type` annotation; \
                             it does not expose `{}`",
                            attr.attr
                        )),
                        note: Some(
                            "A `TypeAlias` binding to `type` or `Type` \
                             does not expose arbitrary attributes."
                                .to_owned(),
                        ),
                    });
                }
            }
        }

        _ => {}
    }
}

/// Check expressions inside a function body.
pub(super) fn check_func_body(
    stmts: &[Stmt],
    ctx: &ModuleCtx,
    param_anns: &HashMap<String, String>,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    for stmt in stmts {
        check_func_stmt(stmt, ctx, param_anns, path, diag);
    }
}

fn check_func_stmt(
    stmt: &Stmt,
    ctx: &ModuleCtx,
    param_anns: &HashMap<String, String>,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    match stmt {
        Stmt::Expr(expr_stmt) => {
            check_func_expr(&expr_stmt.value, ctx, param_anns, path, diag);
        }
        Stmt::Assign(assign) => {
            check_func_expr(&assign.value, ctx, param_anns, path, diag);
        }
        Stmt::AnnAssign(ann) => {
            if let Some(value) = &ann.value {
                check_func_expr(value, ctx, param_anns, path, diag);
            }
        }
        Stmt::Return(ret) => {
            if let Some(value) = &ret.value {
                check_func_expr(value, ctx, param_anns, path, diag);
            }
        }
        Stmt::If(if_stmt) => {
            check_func_body(&if_stmt.body, ctx, param_anns, path, diag);
            check_func_body(
                &if_stmt
                    .elif_else_clauses
                    .iter()
                    .flat_map(|c| c.body.iter())
                    .cloned()
                    .collect::<Vec<_>>(),
                ctx,
                param_anns,
                path,
                diag,
            );
        }
        _ => {}
    }
}

#[expect(
    clippy::only_used_in_recursion,
    reason = "param_anns is passed through recursive calls"
)]
fn check_func_expr(
    expr: &Expr,
    ctx: &ModuleCtx,
    param_anns: &HashMap<String, String>,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    match expr {
        // ----------------------------------------------------------------
        // param.unknown — attribute access on a `type[object]` parameter.
        // ----------------------------------------------------------------
        Expr::Attribute(attr) => {
            if let Some(obj_name) = expr_simple_name(&attr.value) {
                if let Some(ann) = param_anns.get(obj_name) {
                    if is_concrete_type_annotation(ann) && !is_known_type_attr(attr.attr.as_str()) {
                        let span = Span {
                            start: attr.range().start().to_u32(),
                            end: attr.range().end().to_u32(),
                        };
                        diag.push(Diagnostic {
                            code: CODE.clone(),
                            severity: Severity::Error,
                            message: format!(
                                "Attribute `{}` is not defined on `{ann}`; \
                                 `{ann}` only exposes attributes of its type argument",
                                attr.attr
                            ),
                            span,
                            path: path.to_owned(),
                            help: Some(format!(
                                "Only attributes defined on `object` (e.g. `__name__`, `__mro__`) \
                                 are accessible on `{ann}`"
                            )),
                            note: Some(
                                "Per the typing spec, `type[X]` where X is a concrete type \
                                 only exposes attributes defined on X."
                                    .to_owned(),
                            ),
                        });
                    }
                }
            }
        }

        // Recurse into calls to handle nested expressions.
        Expr::Call(call) => {
            check_func_expr(&call.func, ctx, param_anns, path, diag);
            for arg in &call.arguments.args {
                check_func_expr(arg, ctx, param_anns, path, diag);
            }
        }

        _ => {}
    }
}

/// Validate a single argument passed where the parameter annotation is a
/// `type[…]` annotation.
pub(super) fn check_type_arg(
    arg_name: &str,
    param_ann: &str,
    arg_expr: &Expr,
    ctx: &ModuleCtx,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    use super::SPECIAL_FORMS;

    let span = Span {
        start: arg_expr.range().start().to_u32(),
        end: arg_expr.range().end().to_u32(),
    };

    // -----------------------------------------------------------------
    // Case 1: A special form (Callable, etc.) is passed where type[T] is
    // expected.  Special forms are never valid class objects.
    // -----------------------------------------------------------------
    if SPECIAL_FORMS.contains(&arg_name) {
        let inner = strip_type_bracket(param_ann).unwrap_or("T");
        // Only flag if the parameter annotation is actually a `type[…]`.
        if is_any_type_annotation(param_ann) {
            diag.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Argument `{arg_name}` is a special typing form, not a class object; \
                     `type[{inner}]` requires a real class"
                ),
                span,
                path: path.to_owned(),
                help: Some(format!(
                    "`{arg_name}` is a special form and cannot be used as `type[{inner}]`"
                )),
                note: Some(
                    "Per the typing spec, only actual class objects satisfy `type[T]`; \
                     special forms like `Callable` are not class objects."
                        .to_owned(),
                ),
            });
        }
        return;
    }

    // -----------------------------------------------------------------
    // Case 2: Union-parameterised `type[A | B]` — argument must be one
    // of the union members or a subclass thereof (we check names only).
    // -----------------------------------------------------------------
    let Some(members) = ModuleCtx::type_union_members(param_ann) else {
        return;
    };

    // The argument is valid if:
    //   a) it is one of the union member names, OR
    //   b) it is a TypeVar (not a concrete class and therefore unchecked), OR
    //   c) we cannot determine the class hierarchy (skip).
    let is_member = members.contains(&arg_name);
    let is_tv = ctx.is_typevar(arg_name);

    if is_member || is_tv {
        return;
    }

    // Only emit if the argument is a known class that is NOT a union member.
    if ctx.is_class(arg_name) {
        diag.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Argument `{arg_name}` is not assignable to `{param_ann}`; \
                 `{arg_name}` is not one of `{}`",
                members.join(" | ")
            ),
            span,
            path: path.to_owned(),
            help: Some(format!(
                "Pass a class that is one of `{}`",
                members.join(" | ")
            )),
            note: Some(
                "Per the typing spec, `type[A | B]` only accepts classes that \
                 are subtypes of `A` or `B`."
                    .to_owned(),
            ),
        });
    }
}
