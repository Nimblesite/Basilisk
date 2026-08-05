//! Implements [`specialtypes_never_2`] from [CHKARCH-DIAG-OPTIONAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OPTIONAL
//! `specialtypes_never_2`: `Never` type compatibility violations.
//!
//! Detects type compatibility errors involving the `Never` bottom type:
//!
//! 1. Assigning a parameter typed `Container[Never]` to a local annotated
//!    `Container[T]` where `T` is not `Never` or `Any` (invariant violation)
//! 2. Returning `ClassC[Never]()` from a function annotated `-> ClassC[U]`
//!    where the class's type parameter is invariant (not covariant)
//!
//! Every verdict is structural over the parsed `ruff` AST
//! ([LINESCANPLAN-AST-MIGRATION], issue #408): `Never` and `Any` resolve
//! through the import cascade under any spelling, assignments are
//! `AnnAssign` nodes, and return expressions are parsed calls — never
//! reconstructed source lines or `"]("` shape matching.
//!
//! ```python
//! from typing import Never, Any, Generic, TypeVar
//!
//! T = TypeVar("T")
//! U = TypeVar("U")
//!
//! def func(c: list[Never]):
//!     v: list[int] = c  # E0070 — list is invariant, list[Never] != list[int]
//!
//! class ClassC(Generic[T]):
//!     pass
//!
//! def func2(x: U) -> ClassC[U]:
//!     return ClassC[Never]()  # E0070 — ClassC is invariant
//! ```

use std::collections::HashMap;

use basilisk_resolver::{ResolvedModule, Span};
use ruff_python_ast::{Expr, Stmt, StmtFunctionDef};
use ruff_text_size::Ranged;

use crate::annotation::AnnotationResolver;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::ann_str;
use crate::rules::shared::typing_form::denotes;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "specialtypes_never_2",
    docs_url: "https://www.basilisk-python.dev/errors/specialtypes_never_2",
};

/// Emits `specialtypes_never_2` for Never type compatibility violations.
pub(crate) struct NeverTypeCompatibility;

impl Rule for NeverTypeCompatibility {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(parsed) = module.lazy_ast.get_or_parse(&module.source, &module.path) else {
            return;
        };
        let Some(resolver) = AnnotationResolver::for_module(module) else {
            return;
        };
        // Covariant TypeVar names exclude covariant contexts from the check.
        let covariant_tvars: Vec<&str> =
            basilisk_resolver::collect_names_where(&module.typevar_calls, |tv| tv.is_covariant);

        let ctx = NeverContext {
            resolver: &resolver,
            module,
            covariant_tvars,
        };
        walk_functions(&parsed.ast.body, &ctx, diagnostics);
    }
}

/// Everything a `Never`-compatibility verdict needs.
struct NeverContext<'m> {
    resolver: &'m AnnotationResolver<'m>,
    module: &'m ResolvedModule,
    covariant_tvars: Vec<&'m str>,
}

/// Recursively visit every function definition, however nested.
fn walk_functions(body: &[Stmt], ctx: &NeverContext<'_>, diagnostics: &mut Vec<Diagnostic>) {
    for stmt in body {
        match stmt {
            Stmt::FunctionDef(func_def) => {
                check_function(func_def, ctx, diagnostics);
                walk_functions(&func_def.body, ctx, diagnostics);
            }
            Stmt::ClassDef(class_def) => {
                walk_functions(&class_def.body, ctx, diagnostics);
            }
            _ => {}
        }
    }
}

/// A generic annotation split into its base name and single type argument.
struct GenericForm<'e> {
    base: &'e str,
    arg: &'e Expr,
}

/// Split `Base[Arg]` where the base is a simple name; `None` otherwise.
fn generic_form(expr: &Expr) -> Option<GenericForm<'_>> {
    let Expr::Subscript(subscript) = expr else {
        return None;
    };
    let Expr::Name(base) = subscript.value.as_ref() else {
        return None;
    };
    Some(GenericForm {
        base: base.id.as_str(),
        arg: &subscript.slice,
    })
}

/// Check one function for both violation shapes.
fn check_function(
    func_def: &StmtFunctionDef,
    ctx: &NeverContext<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_local_assignments(func_def, ctx, diagnostics);
    check_return_stmts(func_def, ctx, diagnostics);
}

/// Case 1: `v: Container[T] = param` where the parameter is
/// `Container[Never]` and `T` is neither `Never` nor `Any`.
fn check_local_assignments(
    func_def: &StmtFunctionDef,
    ctx: &NeverContext<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Parameters typed `Container[Never]`, mapped to their annotation node.
    let mut never_params: HashMap<&str, &Expr> = HashMap::new();
    for param in func_def.parameters.iter_non_variadic_params() {
        let Some(annotation) = param.annotation() else {
            continue;
        };
        let Some(form) = generic_form(annotation) else {
            continue;
        };
        if denotes(ctx.resolver, form.arg, "Never") {
            let _ = never_params.insert(param.name().as_str(), annotation);
        }
    }
    if never_params.is_empty() {
        return;
    }

    check_assign_body(&func_def.body, ctx, &never_params, diagnostics);
}

/// Walk one function body's statements (not nested defs) for the annotated
/// assignments of case 1.
fn check_assign_body(
    body: &[Stmt],
    ctx: &NeverContext<'_>,
    never_params: &HashMap<&str, &Expr>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in body {
        let Stmt::AnnAssign(assign) = stmt else {
            continue;
        };
        let Expr::Name(target) = assign.target.as_ref() else {
            continue;
        };
        let Some(Expr::Name(rhs)) = assign.value.as_deref() else {
            continue;
        };
        let Some(param_annotation) = never_params.get(rhs.id.as_str()) else {
            continue;
        };
        let Some(param_form) = generic_form(param_annotation) else {
            continue;
        };
        let Some(target_form) = generic_form(&assign.annotation) else {
            continue;
        };
        // Same invariant container, target argument neither Never nor Any.
        if target_form.base != param_form.base
            || denotes(ctx.resolver, target_form.arg, "Never")
            || denotes(ctx.resolver, target_form.arg, "Any")
        {
            continue;
        }

        let range = target.range();
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Cannot assign `{}` to `{}` annotated `{}`: \
                 `Never` is only compatible with `Never` and `Any` in invariant contexts",
                ann_str(param_annotation),
                target.id.as_str(),
                ann_str(&assign.annotation)
            ),
            Span {
                start: range.start().to_u32(),
                end: range.end().to_u32(),
            },
            &ctx.module.path,
            Some(
                "Change the annotation to `Never` or `Any`, or change the assigned value"
                    .to_owned(),
            ),
            Some(
                "PEP 484: `Never` is a bottom type and cannot be assigned to other types \
                 except in covariant contexts or when the target is `Any`"
                    .to_owned(),
            ),
        ));
    }
}

/// Case 2: `return ClassC[Never]()` from a function annotated `-> ClassC[U]`
/// with `U` invariant.
fn check_return_stmts(
    func_def: &StmtFunctionDef,
    ctx: &NeverContext<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(return_annotation) = func_def.returns.as_deref() else {
        return;
    };
    let Some(ann_form) = generic_form(return_annotation) else {
        return;
    };

    // The annotation's type argument must be invariant for the check to apply:
    // not Never, not Any, not a covariant TypeVar, and the class must not
    // declare a covariant parameter.
    if denotes(ctx.resolver, ann_form.arg, "Never") || denotes(ctx.resolver, ann_form.arg, "Any") {
        return;
    }
    if let Expr::Name(arg_name) = ann_form.arg {
        if ctx.covariant_tvars.contains(&arg_name.id.as_str()) {
            return;
        }
    }
    if is_class_param_covariant(ann_form.base, ctx) {
        return;
    }

    let walk = ReturnWalk {
        ctx,
        func_def,
        return_annotation,
        ann_form: &ann_form,
    };
    walk.check_body(&func_def.body, diagnostics);
}

/// The fixed context of one function's return-statement walk.
struct ReturnWalk<'m> {
    ctx: &'m NeverContext<'m>,
    func_def: &'m StmtFunctionDef,
    return_annotation: &'m Expr,
    ann_form: &'m GenericForm<'m>,
}

impl ReturnWalk<'_> {
    /// Walk one function body's statements (not nested defs) for the return
    /// statements of case 2. Return statements inside nested blocks still
    /// return from THIS function; nested defs do not.
    fn check_body(&self, body: &[Stmt], diagnostics: &mut Vec<Diagnostic>) {
        for stmt in body {
            match stmt {
                Stmt::Return(ret) => self.check_return(ret, diagnostics),
                Stmt::If(if_stmt) => {
                    self.check_body(&if_stmt.body, diagnostics);
                    for clause in &if_stmt.elif_else_clauses {
                        self.check_body(&clause.body, diagnostics);
                    }
                }
                Stmt::For(for_stmt) => {
                    self.check_body(&for_stmt.body, diagnostics);
                    self.check_body(&for_stmt.orelse, diagnostics);
                }
                Stmt::While(while_stmt) => {
                    self.check_body(&while_stmt.body, diagnostics);
                    self.check_body(&while_stmt.orelse, diagnostics);
                }
                Stmt::With(with_stmt) => {
                    self.check_body(&with_stmt.body, diagnostics);
                }
                Stmt::Try(try_stmt) => {
                    self.check_body(&try_stmt.body, diagnostics);
                    self.check_body(&try_stmt.orelse, diagnostics);
                    self.check_body(&try_stmt.finalbody, diagnostics);
                }
                _ => {}
            }
        }
    }

    /// One return statement: report `return ClassC[Never](...)` against the
    /// invariant `-> ClassC[U]` annotation.
    fn check_return(&self, ret: &ruff_python_ast::StmtReturn, diagnostics: &mut Vec<Diagnostic>) {
        let Some(value) = ret.value.as_deref() else {
            return;
        };
        // `ClassC[Never]()` returns an instance; a bare `ClassC[Never]`
        // subscript expression is the class object itself.
        let returned_type = match value {
            Expr::Call(call) => call.func.as_ref(),
            other => other,
        };
        let Some(ret_form) = generic_form(returned_type) else {
            return;
        };
        if ret_form.base != self.ann_form.base || !denotes(self.ctx.resolver, ret_form.arg, "Never")
        {
            return;
        }

        let range = ret.range();
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Cannot return `{}` from `{}` annotated \
                 `-> {}`: `Never` is only compatible with `Never` \
                 and `Any` in invariant contexts",
                ann_str(returned_type),
                self.func_def.name.as_str(),
                ann_str(self.return_annotation)
            ),
            Span {
                start: range.start().to_u32(),
                end: range.end().to_u32(),
            },
            &self.ctx.module.path,
            Some("Change the return type annotation or the returned value".to_owned()),
            Some(
                "PEP 484: `Never` is a bottom type and cannot substitute for invariant \
                 type parameters"
                    .to_owned(),
            ),
        ));
    }
}

/// Does the class's declared generic parameter list include a covariant
/// `TypeVar`?
fn is_class_param_covariant(class_name: &str, ctx: &NeverContext<'_>) -> bool {
    ctx.module
        .classes
        .iter()
        .filter(|cls| cls.name == class_name)
        .flat_map(|cls| &cls.generic_params)
        .any(|param| ctx.covariant_tvars.contains(&param.name.as_str()))
}
