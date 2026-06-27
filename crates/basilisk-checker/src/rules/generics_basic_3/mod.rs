//! generics_basic_3: Generic type argument violations.
//!
//! Detects several generic-type errors:
//!
//! 1. **Constrained `TypeVar` constraint mismatch**: When a function parameter is typed
//!    with a constrained `TypeVar` (e.g. `AnyStr = TypeVar("AnyStr", str, bytes)`),
//!    all arguments bound to the same type variable must belong to the same constraint.
//!    Passing `(str_val, bytes_val)` for `(x: AnyStr, y: AnyStr)` is an error.
//!
//! 2. **Mapping subscript key type mismatch**: When a `Mapping`-derived type has a
//!    known key type (e.g. `MyMap[str, int]`), indexing with a literal of the wrong
//!    type (e.g. `my_map[0]`) is an error.
//!
//! 3. **Generic metaclass usage**: Using a parameterized generic class as a metaclass
//!    (`metaclass=SomeGeneric[T]`) is not supported by the Python type system.

mod helpers;

use ruff_python_ast::{Expr, Stmt};

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

use helpers::{
    ann_str, check_call, check_class_def, check_subscript, resolve_mapping_annotation,
    ModuleContext,
};

/// Emits generics_basic_3 for generic type argument violations.
pub(crate) struct GenericTypeArgViolation;

impl Rule for GenericTypeArgViolation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };

        let ctx = ModuleContext::from_ast(&parsed.ast.body);
        check_stmts(&parsed.ast.body, &ctx, &module.path, diagnostics);
    }
}

// ---------------------------------------------------------------------------
// Statement walking
// ---------------------------------------------------------------------------

fn check_stmts(stmts: &[Stmt], ctx: &ModuleContext, path: &str, diag: &mut Vec<Diagnostic>) {
    for stmt in stmts {
        match stmt {
            Stmt::FunctionDef(func) => check_func_body(func, ctx, path, diag),
            Stmt::ClassDef(cls) => {
                check_class_def(cls, path, diag);
                check_stmts(&cls.body, ctx, path, diag);
            }
            Stmt::Expr(expr_stmt) => check_expr(&expr_stmt.value, ctx, path, diag),
            _ => {}
        }
    }
}

fn check_func_body(
    func: &ruff_python_ast::StmtFunctionDef,
    ctx: &ModuleContext,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
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
            if let Some(pair) = resolve_mapping_annotation(&ann_text, &ctx.class_bases) {
                let _ = local_mapping_vars.insert(param.parameter.name.to_string(), pair);
            }
        }
    }

    let local_ctx = ModuleContext {
        constrained_tvars: ctx.constrained_tvars.clone(),
        constrained_funcs: ctx.constrained_funcs.clone(),
        var_types: local_types,
        mapping_vars: local_mapping_vars,
        class_bases: ctx.class_bases.clone(),
    };

    for stmt in &func.body {
        check_stmt_in_func(stmt, &local_ctx, path, diag);
    }
}

fn check_stmt_in_func(stmt: &Stmt, ctx: &ModuleContext, path: &str, diag: &mut Vec<Diagnostic>) {
    match stmt {
        Stmt::Expr(expr_stmt) => check_expr(&expr_stmt.value, ctx, path, diag),
        Stmt::Assign(assign) => check_expr(&assign.value, ctx, path, diag),
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
        Expr::Call(call) => check_call(call, ctx, path, diag),
        Expr::Subscript(sub) => check_subscript(sub, ctx, path, diag),
        _ => {}
    }
}
