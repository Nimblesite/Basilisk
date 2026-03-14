//! Call-site, subscript, and class-def checkers for BSK-E0148.

use std::collections::HashMap;

use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::Span;

use crate::diagnostic::{Diagnostic, Severity};

use super::{
    helpers::{call_span, expr_name, infer_literal_type, types_compatible},
    ModuleContext, CODE,
};

// ---------------------------------------------------------------------------
// Statement walking
// ---------------------------------------------------------------------------

/// Walk all statements, dispatching to the appropriate check.
pub(super) fn check_stmts(
    stmts: &[Stmt],
    ctx: &ModuleContext,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::FunctionDef(func) => {
                check_func_body(func, ctx, path, diag);
            }
            Stmt::ClassDef(cls) => {
                check_class_def(cls, path, diag);
                check_stmts(&cls.body, ctx, path, diag);
            }
            Stmt::Expr(expr_stmt) => {
                check_expr(&expr_stmt.value, ctx, path, diag);
            }
            _ => {}
        }
    }
}

/// Check all statements inside a function body.
pub(super) fn check_func_body(
    func: &ast::StmtFunctionDef,
    ctx: &ModuleContext,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    use super::helpers::ann_str;

    // Build a local var-type map from this function's parameters.
    let mut local_types = ctx.var_types.clone();
    let mut local_mapping_vars = ctx.mapping_vars.clone();

    for param in func
        .parameters
        .args
        .iter()
        .chain(func.parameters.posonlyargs.iter())
    {
        if let Some(ann) = &param.parameter.annotation {
            let ann_text = ann_str(ann);
            let _ = local_types.insert(param.parameter.name.to_string(), ann_text.clone());
            if let Some((key_ty, val_ty)) = super::parse_mapping_annotation(&ann_text) {
                let _ =
                    local_mapping_vars.insert(param.parameter.name.to_string(), (key_ty, val_ty));
            }
        }
    }

    let local_ctx = ModuleContext {
        constrained_tvars: ctx.constrained_tvars.clone(),
        constrained_funcs: ctx.constrained_funcs.clone(),
        var_types: local_types,
        mapping_vars: local_mapping_vars,
    };

    for stmt in &func.body {
        check_stmt_in_func(stmt, &local_ctx, path, diag);
    }
}

fn check_stmt_in_func(stmt: &Stmt, ctx: &ModuleContext, path: &str, diag: &mut Vec<Diagnostic>) {
    match stmt {
        Stmt::Expr(expr_stmt) => {
            check_expr(&expr_stmt.value, ctx, path, diag);
        }
        Stmt::Assign(assign) => {
            check_expr(&assign.value, ctx, path, diag);
        }
        Stmt::Return(ret) => {
            if let Some(value) = &ret.value {
                check_expr(value, ctx, path, diag);
            }
        }
        _ => {}
    }
}

fn check_expr(expr: &Expr, ctx: &ModuleContext, path: &str, diag: &mut Vec<Diagnostic>) {
    match expr {
        Expr::Call(call) => {
            check_call(call, ctx, path, diag);
        }
        Expr::Subscript(sub) => {
            check_subscript(sub, ctx, path, diag);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Call-site checking (constrained TypeVar)
// ---------------------------------------------------------------------------

fn check_call(call: &ast::ExprCall, ctx: &ModuleContext, path: &str, diag: &mut Vec<Diagnostic>) {
    let Some(callee_name) = expr_name(&call.func) else {
        return;
    };

    let Some(cfunc) = ctx.constrained_funcs.iter().find(|f| f.name == callee_name) else {
        return;
    };

    // Resolve the constraint groups for each argument that has a constrained TypeVar.
    // Map: TypeVar name -> (first_group_index, first_arg_type_text)
    let mut tv_group: HashMap<&str, (usize, String)> = HashMap::new();

    for (arg_idx, arg) in call.arguments.args.iter().enumerate() {
        let Some(tv_name) = cfunc.param_tv.get(arg_idx).and_then(|o| o.as_deref()) else {
            continue;
        };
        let Some(constrained_tv) = ctx.constrained_tvars.get(tv_name) else {
            continue;
        };

        // Determine the type of this argument.
        let arg_type = infer_arg_type(arg, &ctx.var_types);
        let Some(arg_type_str) = arg_type else {
            // Cannot determine type — skip conservatively.
            continue;
        };

        // Skip `Any`-typed arguments.
        if arg_type_str == "Any" {
            continue;
        }

        // Find which constraint group this argument belongs to.
        let Some(group) = constrained_tv.group_of(&arg_type_str) else {
            // Try to resolve via known subtypes: if arg_type_str is a class
            // in this module that inherits from one of the constraints, map
            // to that constraint's group.  We use a conservative heuristic.
            continue;
        };

        match tv_group.get(tv_name) {
            None => {
                let _ = tv_group.insert(tv_name, (group, arg_type_str));
            }
            Some(&(existing_group, ref _existing_type)) => {
                if existing_group != group {
                    let span = call_span(call);
                    diag.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Error,
                        message: format!(
                            "Constraint mismatch for TypeVar `{tv_name}` in call to `{callee_name}`: \
                             argument types belong to different constraint groups"
                        ),
                        span,
                        path: path.to_owned(),
                        help: Some(format!(
                            "TypeVar `{tv_name}` is constrained to `{}`; all arguments bound to \
                             the same TypeVar must use the same constraint",
                            constrained_tv.constraints.join("` or `")
                        )),
                        note: Some(
                            "PEP 484: arguments for a constrained TypeVar must all match the \
                             same constraint alternative"
                                .to_owned(),
                        ),
                    });
                    return; // One diagnostic per call.
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Subscript checking (Mapping key type)
// ---------------------------------------------------------------------------

fn check_subscript(
    sub: &ast::ExprSubscript,
    ctx: &ModuleContext,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    // Only check simple-name subscript targets (e.g. `m1[0]`, `m2[0]`).
    let Some(obj_name) = expr_name(&sub.value) else {
        return;
    };

    let Some((key_ty, _val_ty)) = ctx.mapping_vars.get(obj_name) else {
        return;
    };

    // Infer the type of the subscript key.
    let Some(idx_ty) = infer_literal_type(&sub.slice) else {
        return;
    };

    if !types_compatible(idx_ty, key_ty) {
        let span = Span {
            start: sub.range().start().to_u32(),
            end: sub.range().end().to_u32(),
        };
        diag.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Invalid subscript key type `{idx_ty}` for `{obj_name}` which expects key type `{key_ty}`"
            ),
            span,
            path: path.to_owned(),
            help: Some(format!(
                "`{obj_name}` is parameterized with key type `{key_ty}`; \
                 use a `{key_ty}` value as the subscript key"
            )),
            note: Some(
                "PEP 484: subscript key must be compatible with the declared key type parameter"
                    .to_owned(),
            ),
        });
    }
}

// ---------------------------------------------------------------------------
// Class-def checking (generic metaclass)
// ---------------------------------------------------------------------------

/// Check a class definition for use of a parameterized generic as a metaclass.
pub(super) fn check_class_def(cls: &ast::StmtClassDef, path: &str, diag: &mut Vec<Diagnostic>) {
    let Some(args) = &cls.arguments else {
        return;
    };

    for kw in &args.keywords {
        // Look for `metaclass=SomeGeneric[T]`.
        let Some(kw_name) = &kw.arg else {
            continue;
        };
        if kw_name.as_str() != "metaclass" {
            continue;
        }

        // Check if the metaclass value is a subscript (i.e. `Generic[T]`).
        if matches!(&kw.value, Expr::Subscript(_)) {
            let span = Span {
                start: cls.range().start().to_u32(),
                end: cls.range().end().to_u32(),
            };
            diag.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Class `{}` uses a parameterized generic type as its metaclass",
                    cls.name
                ),
                span,
                path: path.to_owned(),
                help: Some(
                    "Generic metaclasses are not supported by the Python type system".to_owned(),
                ),
                note: Some("PEP 484: generic metaclass instances are not supported".to_owned()),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Type inference helpers
// ---------------------------------------------------------------------------

/// Infer the type text of an argument expression, using the variable type map.
fn infer_arg_type<'a>(arg: &'a Expr, var_types: &'a HashMap<String, String>) -> Option<String> {
    match arg {
        Expr::Name(n) => {
            let name = n.id.as_str();
            // Look up the variable's declared type.
            var_types.get(name).cloned()
        }
        _ => infer_literal_type(arg).map(str::to_owned),
    }
}
