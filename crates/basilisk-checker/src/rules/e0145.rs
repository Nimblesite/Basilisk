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
//!
//! ```python
//! from typing import Callable, TypeVar
//!
//! T = TypeVar("T")
//!
//! def func5(x: type[T]) -> None: pass
//!
//! func5(Callable)   # E — Callable is not a class
//!
//! class A: ...
//! class B: ...
//! class C: ...
//!
//! def func4(x: type[A | B]) -> None: pass
//! func4(C)          # E — C is not A or B
//!
//! def func8(a: type[object]) -> None:
//!     a.unknown     # E — type[object] does not expose arbitrary attributes
//! ```

use std::collections::HashMap;

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0145",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0145",
};

/// Emits BSK-E0145 for invalid `type[X]` usages.
pub(crate) struct TypeBracketViolation;

impl Rule for TypeBracketViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let Ok(parsed) = basilisk_parser::parse_source(module.source.clone(), module.path.clone())
        else {
            return;
        };

        let ctx = ModuleCtx::build(&parsed.ast.body);
        check_stmts(&parsed.ast.body, &ctx, &module.path, diagnostics);
    }
}

// ---------------------------------------------------------------------------
// Module-level context: collect class names, TypeAlias bindings, function
// parameter annotations.
// ---------------------------------------------------------------------------

/// Special-form names that are not valid class objects for `type[T]`.
const SPECIAL_FORMS: &[&str] = &[
    "Callable",
    "Union",
    "Optional",
    "ClassVar",
    "Final",
    "Literal",
    "Annotated",
    "TypeGuard",
    "TypeIs",
    "Never",
    "NoReturn",
    "LiteralString",
    "Self",
    "Unpack",
    "TypeVarTuple",
    "ParamSpec",
    "Concatenate",
    "Required",
    "NotRequired",
    "ReadOnly",
    "TypeAlias",
];

/// Known attributes on `type` / `object` (the metaclass API) that are always
/// legal to access on `type[object]` or a plain `type` annotation.
const KNOWN_TYPE_ATTRS: &[&str] = &[
    "__name__",
    "__qualname__",
    "__module__",
    "__bases__",
    "__mro__",
    "__subclasses__",
    "__doc__",
    "__dict__",
    "__slots__",
    "__annotations__",
    "__class__",
    "__init__",
    "__new__",
    "__repr__",
    "__str__",
    "__hash__",
    "__eq__",
    "__ne__",
    "__lt__",
    "__le__",
    "__gt__",
    "__ge__",
    "__abstractmethods__",
    "mro",
    "__init_subclass__",
    "__subclasshook__",
];

/// Collected per-module context needed by the checker.
struct ModuleCtx {
    /// Class names defined at module scope.
    class_names: Vec<String>,
    /// `TypeVar` names (i.e., assigned via `TypeVar(...)`).
    typevar_names: Vec<String>,
    /// `TypeAlias` bindings: alias name → annotation text of the RHS.
    /// e.g. `TA1: TypeAlias = Type` → `("TA1", "Type")`.
    type_aliases: HashMap<String, String>,
    /// Module-level function signatures: name → list of (`param_name``annotation_text`xt).
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
                Stmt::Assign(assign) => {
                    // Detect `X = TypeVar("X")` or `X = TypeVar("X", bound=...)`.
                    if assign.targets.len() == 1 {
                        if let Some(name) = expr_simple_name(&assign.targets[0]) {
                            if is_typevar_call(&assign.value) {
                                typevar_names.push(name.to_owned());
                            }
                        }
                    }
                }
                Stmt::AnnAssign(ann) => {
                    // Detect `TA: TypeAlias = Type` / `TA: TypeAlias = type[Any]` etc.
                    let is_alias = match ann.annotation.as_ref() {
                        Expr::Name(n) => n.id.as_str() == "TypeAlias",
                        _ => false,
                    };
                    if is_alias {
                        if let Some(name) = expr_simple_name(&ann.target) {
                            if let Some(value) = &ann.value {
                                let rhs = expr_to_str(value);
                                type_aliases.insert(name.to_owned(), rhs);
                            }
                        }
                    }
                }
                Stmt::FunctionDef(func) => {
                    let mut params: Vec<(String, String)> = Vec::new();
                    for pwd in &func.parameters.args {
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
                    for pwd in &func.parameters.posonlyargs {
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
                        func_params.insert(func.name.to_string(), params);
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

    /// Returns true if `name` is a `TypeAlias` binding that resolves to a
    /// `type` / `Type` variant (with or without a parameter).
    fn is_type_alias(&self, name: &str) -> bool {
        self.type_aliases
            .get(name)
            .is_some_and(|rhs| is_type_annotation(rhs))
    }

    /// Return the union members if the annotation is `type[A | B | ...]`.
    /// Returns `None` if it is not a union-parameterised `type[…]`.
    fn type_union_members(ann: &str) -> Option<Vec<&str>> {
        // Accept both `type[...]` and `Type[...]`.
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

#[allow(clippy::only_used_in_recursion)]
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
fn check_type_arg(
    arg_name: &str,
    param_ann: &str,
    arg_expr: &Expr,
    ctx: &ModuleCtx,
    path: &str,
    diag: &mut Vec<Diagnostic>,
) {
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

// ---------------------------------------------------------------------------
// Predicate helpers
// ---------------------------------------------------------------------------

/// Returns `true` if `ann` is a `type[…]` or `Type[…]` annotation of any form
/// (including `type[Any]`, `type[T]`, `type[A | B]`).
fn is_any_type_annotation(ann: &str) -> bool {
    strip_type_bracket(ann).is_some()
}

/// Strip the `type[` / `Type[` prefix + `]` suffix and return the inner text,
/// or `None` if the annotation is not of this form.
fn strip_type_bracket(ann: &str) -> Option<&str> {
    let ann = ann.trim();
    let inner = ann
        .strip_prefix("type[")
        .or_else(|| ann.strip_prefix("Type["))?;
    inner.strip_suffix(']')
}

/// Returns `true` if `ann` is a `type[X]` where `X` is a **concrete** (non-Any,
/// non-TypeVar) type — e.g. `type[object]`, `Type[object]`, `type[int]`.
///
/// We intentionally exclude `type[Any]` and `type[T]` (`TypeVar` parameters)
/// because those expose all attributes.
fn is_concrete_type_annotation(ann: &str) -> bool {
    let Some(inner) = strip_type_bracket(ann) else {
        return false;
    };
    let inner = inner.trim();
    // Exclude `Any` (exposes everything) and obvious TypeVar identifiers
    // (single uppercase letter).
    if inner == "Any" || inner.len() == 1 && inner.chars().next().is_some_and(char::is_uppercase) {
        return false;
    }
    // Include only known concrete types for which we know `unknown` is invalid.
    matches!(inner, "object" | "int" | "str" | "float" | "bool" | "bytes")
}

/// Returns `true` if `rhs` (the right-hand side of a `TypeAlias`) is a
/// `type` or `Type` annotation (bare or parameterised).
fn is_type_annotation(rhs: &str) -> bool {
    let rhs = rhs.trim();
    matches!(rhs, "type" | "Type") || rhs.starts_with("type[") || rhs.starts_with("Type[")
}

/// Returns `true` if `attr` is a well-known attribute on the `type` metaclass
/// or `object` that is always valid to access on any `type[X]`.
fn is_known_type_attr(attr: &str) -> bool {
    KNOWN_TYPE_ATTRS.contains(&attr)
}

// ---------------------------------------------------------------------------
// AST utility helpers
// ---------------------------------------------------------------------------

fn expr_simple_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(n) => Some(n.id.as_str()),
        _ => None,
    }
}

fn is_typevar_call(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Call(call)
            if matches!(call.func.as_ref(), Expr::Name(n) if n.id.as_str() == "TypeVar")
    )
}

/// Convert an expression to a readable annotation string (best-effort).
fn expr_to_str(expr: &Expr) -> String {
    match expr {
        Expr::Name(n) => n.id.to_string(),
        Expr::Subscript(s) => format!("{}[{}]", expr_to_str(&s.value), expr_to_str(&s.slice)),
        Expr::Attribute(a) => format!("{}.{}", expr_to_str(&a.value), a.attr),
        Expr::Tuple(t) => t
            .elts
            .iter()
            .map(expr_to_str)
            .collect::<Vec<_>>()
            .join(", "),
        Expr::BinOp(b) => format!("{} | {}", expr_to_str(&b.left), expr_to_str(&b.right)),
        Expr::NoneLiteral(_) => "None".to_owned(),
        Expr::EllipsisLiteral(_) => "...".to_owned(),
        _ => String::new(),
    }
}
