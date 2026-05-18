//! BSK-E0145: Invalid `type[X]` usage violations.
//!
//! Detects several categories of invalid use of `type[X]` (or `Type[X]`):
//!
//! 1. **Callable passed as `type[T]` argument** — `Callable` and other special
//!    forms are not valid class objects and cannot be passed where `type[T]` is
//!    expected.
//!
//! 2. **Incompatible class passed to `type[A | B]`** — when a function expects
//!    `type[A | B]`, passing a class that is neither `A` nor `B` is an error.
//!
//! 3. **Unknown attribute access on `type[object]`** — unlike `type[Any]`,
//!    `type[object]` only exposes `object`'s own attributes; accessing any other
//!    member is an error.
//!
//! 4. **Unknown attribute access on a `TypeAlias` bound to `type` / `Type`** —
//!    a bare alias such as `TA1: TypeAlias = Type` resolves to `type[Any]`, but
//!    the alias *name itself* (used at module scope like `TA1.unknown`) does not
//!    expose arbitrary attributes.

mod helpers;

use std::collections::HashMap;

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, error_diagnostic_owned};

use super::Rule;

use helpers::{
    expr_simple_name, expr_to_str, is_any_type_annotation, is_concrete_type_annotation,
    is_known_type_attr, is_type_annotation, is_typevar_call, strip_type_bracket, SPECIAL_FORMS,
};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0145",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0145",
};

/// Emits BSK-E0145 for invalid `type[X]` usages.
pub(crate) struct TypeBracketViolation;

impl Rule for TypeBracketViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };

        let ctx = ModuleCtx::build(&parsed.ast.body);
        check_stmts(&parsed.ast.body, &ctx, &module.path, diagnostics);
    }
}

// ---------------------------------------------------------------------------
// Module-level context
// ---------------------------------------------------------------------------

/// Collected per-module context needed by the checker.
struct ModuleCtx {
    /// Class names defined at module scope.
    class_names: Vec<String>,
    /// `TypeVar` names (i.e., assigned via `TypeVar(...)`).
    typevar_names: Vec<String>,
    /// `TypeAlias` bindings: alias name → annotation text of the RHS.
    type_aliases: HashMap<String, String>,
    /// Module-level function signatures: name → list of (`param_name`, `annotation_text`).
    func_params: HashMap<String, Vec<(String, String)>>,
}

impl ModuleCtx {
    fn build(stmts: &[Stmt]) -> Self {
        let mut class_names = Vec::new();
        let mut typevar_names = Vec::new();
        let mut type_aliases: HashMap<String, String> = HashMap::new();
        let mut func_params: HashMap<String, Vec<(String, String)>> = HashMap::new();

        for stmt in stmts {
            match stmt {
                Stmt::ClassDef(cls) => {
                    class_names.push(cls.name.to_string());
                }
                Stmt::Assign(assign) if assign.targets.len() == 1 => {
                    if let Some(name) = assign.targets.first().and_then(expr_simple_name) {
                        if is_typevar_call(&assign.value) {
                            typevar_names.push(name.to_owned());
                        }
                    }
                }
                Stmt::AnnAssign(ann) => {
                    let is_alias = match ann.annotation.as_ref() {
                        Expr::Name(n) => n.id.as_str() == "TypeAlias",
                        _ => false,
                    };
                    if is_alias {
                        if let Some(name) = expr_simple_name(&ann.target) {
                            if let Some(value) = &ann.value {
                                let rhs = expr_to_str(value);
                                let _ = type_aliases.insert(name.to_owned(), rhs);
                            }
                        }
                    }
                }
                Stmt::FunctionDef(func) => {
                    let mut params: Vec<(String, String)> = Vec::new();
                    for pwd in func
                        .parameters
                        .args
                        .iter()
                        .chain(func.parameters.posonlyargs.iter())
                    {
                        let pname = pwd.parameter.name.to_string();
                        let ann = pwd
                            .parameter
                            .annotation
                            .as_ref()
                            .map(|a| expr_to_str(a))
                            .unwrap_or_default();
                        if !ann.is_empty() {
                            params.push((pname, ann));
                        }
                    }
                    if !params.is_empty() {
                        let _ = func_params.insert(func.name.to_string(), params);
                    }
                }
                _ => {}
            }
        }

        Self {
            class_names,
            typevar_names,
            type_aliases,
            func_params,
        }
    }

    /// Returns true if `name` is a known module-level class.
    fn is_class(&self, name: &str) -> bool {
        self.class_names.iter().any(|c| c == name)
    }

    /// Returns true if `name` is a `TypeVar`.
    fn is_typevar(&self, name: &str) -> bool {
        self.typevar_names.iter().any(|t| t == name)
    }

    /// Returns true if `name` is a `TypeAlias` binding that resolves to a `type` / `Type` variant.
    fn is_type_alias(&self, name: &str) -> bool {
        self.type_aliases
            .get(name)
            .is_some_and(|rhs| is_type_annotation(rhs))
    }

    /// Return the union members if the annotation is `type[A | B | ...]`.
    fn type_union_members(ann: &str) -> Option<Vec<&str>> {
        let inner = strip_type_bracket(ann)?;
        if inner.contains(" | ") {
            Some(inner.split(" | ").map(str::trim).collect())
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Statement / expression walkers
// ---------------------------------------------------------------------------

fn check_stmts(stmts: &[Stmt], ctx: &ModuleCtx, path: &str, diag: &mut Vec<Diagnostic>) {
    for stmt in stmts {
        check_stmt(stmt, ctx, path, diag);
    }
}

fn check_stmt(stmt: &Stmt, ctx: &ModuleCtx, path: &str, diag: &mut Vec<Diagnostic>) {
    match stmt {
        Stmt::FunctionDef(func) => {
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
            check_module_expr(&expr_stmt.value, ctx, path, diag);
        }
        _ => {}
    }
}

/// Check bare expression statements at module (or outer) scope.
fn check_module_expr(expr: &Expr, ctx: &ModuleCtx, path: &str, diag: &mut Vec<Diagnostic>) {
    match expr {
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
        Expr::Attribute(attr) => {
            if let Some(obj_name) = expr_simple_name(&attr.value) {
                if ctx.is_type_alias(obj_name) && !is_known_type_attr(attr.attr.as_str()) {
                    let span = Span::from(attr.range());
                    diag.push(error_diagnostic_owned(
                        CODE.clone(),
                        format!(
                            "Attribute `{}` is not defined on `{obj_name}` \
                             (a `TypeAlias` of `type`/`Type`)",
                            attr.attr
                        ),
                        span,
                        path,
                        Some(format!(
                            "`{obj_name}` is a `TypeAlias` for a `type` annotation; \
                             it does not expose `{}`",
                            attr.attr
                        )),
                        Some(
                            "A `TypeAlias` binding to `type` or `Type` \
                             does not expose arbitrary attributes."
                                .to_owned(),
                        ),
                    ));
                }
            }
        }
        _ => {}
    }
}

/// Check expressions inside a function body.
fn check_func_body(
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
        Stmt::Expr(expr_stmt) => check_func_expr(&expr_stmt.value, ctx, param_anns, path, diag),
        Stmt::Assign(assign) => check_func_expr(&assign.value, ctx, param_anns, path, diag),
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
        Expr::Attribute(attr) => {
            if let Some(obj_name) = expr_simple_name(&attr.value) {
                if let Some(ann) = param_anns.get(obj_name) {
                    if is_concrete_type_annotation(ann) && !is_known_type_attr(attr.attr.as_str()) {
                        let span = Span::from(attr.range());
                        diag.push(error_diagnostic_owned(
                            CODE.clone(),
                            format!(
                                "Attribute `{}` is not defined on `{ann}`; \
                                 `{ann}` only exposes attributes of its type argument",
                                attr.attr
                            ),
                            span,
                            path,
                            Some(format!(
                                "Only attributes defined on `object` (e.g. `__name__`, `__mro__`) \
                                 are accessible on `{ann}`"
                            )),
                            Some(
                                "Per the typing spec, `type[X]` where X is a concrete type \
                                 only exposes attributes defined on X."
                                    .to_owned(),
                            ),
                        ));
                    }
                }
            }
        }
        Expr::Call(call) => {
            check_func_expr(&call.func, ctx, param_anns, path, diag);
            for arg in &call.arguments.args {
                check_func_expr(arg, ctx, param_anns, path, diag);
            }
        }
        _ => {}
    }
}

/// Validate a single argument passed where the parameter annotation is a `type[…]` annotation.
fn check_type_arg(
    arg_name: &str,
    param_ann: &str,
    arg_expr: &Expr,
    ctx: &ModuleCtx,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
    let span = Span::from(arg_expr.range());

    if SPECIAL_FORMS.contains(&arg_name) {
        let inner = strip_type_bracket(param_ann).unwrap_or("T");
        if is_any_type_annotation(param_ann) {
            diag.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Argument `{arg_name}` is a special typing form, not a class object; \
                     `type[{inner}]` requires a real class"
                ),
                span,
                path,
                Some(format!(
                    "`{arg_name}` is a special form and cannot be used as `type[{inner}]`"
                )),
                Some(
                    "Per the typing spec, only actual class objects satisfy `type[T]`; \
                     special forms like `Callable` are not class objects."
                        .to_owned(),
                ),
            ));
        }
        return;
    }

    let Some(members) = ModuleCtx::type_union_members(param_ann) else {
        return;
    };

    let is_member = members.contains(&arg_name);
    let is_tv = ctx.is_typevar(arg_name);

    if is_member || is_tv {
        return;
    }

    if ctx.is_class(arg_name) {
        diag.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Argument `{arg_name}` is not assignable to `{param_ann}`; \
                 `{arg_name}` is not one of `{}`",
                members.join(" | ")
            ),
            span,
            path,
            Some(format!(
                "Pass a class that is one of `{}`",
                members.join(" | ")
            )),
            Some(
                "Per the typing spec, `type[A | B]` only accepts classes that \
                 are subtypes of `A` or `B`."
                    .to_owned(),
            ),
        ));
    }
}
