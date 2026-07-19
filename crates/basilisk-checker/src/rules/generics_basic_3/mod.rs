//! `generics_basic_3`: Generic type argument violations.
//!
//! Implements the focused generic and constrained-`TypeVar` call-resolution
//! paths in [TYPEINF-GENERICS], [TYPEINF-GENERICS-TYPEVAR], and
//! [TYPEINF-GENERICS-CONSTRAINED].
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

use helpers::{check_call, check_class_def, check_subscript, ModuleContext, ScopeContext};

/// Emits `generics_basic_3` for generic type argument violations.
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
        let scope = ScopeContext::module_scope(&ctx);
        check_stmts(&parsed.ast.body, &scope, &module.path, diagnostics);
    }
}

// ---------------------------------------------------------------------------
// Statement walking
// ---------------------------------------------------------------------------

fn check_stmts(stmts: &[Stmt], scope: &ScopeContext<'_>, path: &str, diag: &mut Vec<Diagnostic>) {
    for stmt in stmts {
        match stmt {
            Stmt::FunctionDef(func) => check_func_body(func, scope.module(), path, diag),
            Stmt::ClassDef(cls) => {
                check_class_def(cls, path, diag);
                check_stmts(&cls.body, scope, path, diag);
            }
            Stmt::Expr(expr_stmt) => check_expr(&expr_stmt.value, scope, path, diag),
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
    // Overlay the function's parameter annotations on the module context —
    // no module-wide maps are copied ([CHKARCH-DIAG] traversal stays linear).
    let scope = ScopeContext::function_scope(ctx, func);

    for stmt in &func.body {
        check_stmt_in_func(stmt, &scope, path, diag);
    }
}

fn check_stmt_in_func(
    stmt: &Stmt,
    scope: &ScopeContext<'_>,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    match stmt {
        Stmt::Expr(expr_stmt) => check_expr(&expr_stmt.value, scope, path, diag),
        Stmt::Assign(assign) => check_expr(&assign.value, scope, path, diag),
        Stmt::Return(ret) => {
            if let Some(value) = &ret.value {
                check_expr(value, scope, path, diag);
            }
        }
        _ => {}
    }
}

fn check_expr(expr: &Expr, scope: &ScopeContext<'_>, path: &str, diag: &mut Vec<Diagnostic>) {
    match expr {
        Expr::Call(call) => check_call(call, scope, path, diag),
        Expr::Subscript(sub) => check_subscript(sub, scope, path, diag),
        _ => {}
    }
}
