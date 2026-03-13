//! BSK-E0100: Augmented assignment widens `Literal` type.
//!
//! When a function parameter is annotated with `Literal[...]`, augmented
//! assignment (`+=`, `-=`, etc.) effectively reassigns the variable to a
//! widened type (e.g. `int` instead of `Literal[3, 4, 5]`), violating the
//! declared `Literal` constraint.
//!
//! ```python
//! def func(a: Literal[3, 4, 5]):
//!     a += 3  # E0100 — augmented assign widens Literal type
//! ```

use std::collections::HashSet;

use basilisk_resolver::{ResolvedModule, Span};
use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged as _;

use super::Rule;
use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0100",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0100",
};

/// Emits BSK-E0100 for augmented assignment on `Literal`-typed parameters.
pub(crate) struct LiteralAugmentedAssign;

impl Rule for LiteralAugmentedAssign {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // First, emit any violations collected by the resolver.
        for violation in &module.literal_augmented_assign_violations {
            diagnostics.push(make_diagnostic(
                &violation.var_name,
                violation.span,
                &module.path,
            ));
        }

        // Also walk the AST to find violations the resolver didn't collect.
        let Ok(parsed) = basilisk_parser::parse_source(module.source.clone(), module.path.clone())
        else {
            return;
        };

        walk_stmts(&parsed.ast.body, &module.path, diagnostics);
    }
}

/// Walk statements looking for function definitions with `Literal`-annotated parameters.
fn walk_stmts(stmts: &[Stmt], path: &str, diagnostics: &mut Vec<Diagnostic>) {
    for stmt in stmts {
        match stmt {
            Stmt::FunctionDef(func_def) => {
                // Collect parameter names annotated with `Literal[...]`.
                let literal_params = collect_literal_params(func_def);
                if !literal_params.is_empty() {
                    check_body_for_aug_assign(&func_def.body, &literal_params, path, diagnostics);
                }
                // Recurse into nested functions.
                walk_stmts(&func_def.body, path, diagnostics);
            }
            Stmt::ClassDef(class_def) => {
                walk_stmts(&class_def.body, path, diagnostics);
            }
            Stmt::If(if_stmt) => {
                walk_stmts(&if_stmt.body, path, diagnostics);
                for clause in &if_stmt.elif_else_clauses {
                    walk_stmts(&clause.body, path, diagnostics);
                }
            }
            Stmt::For(for_stmt) => {
                walk_stmts(&for_stmt.body, path, diagnostics);
                walk_stmts(&for_stmt.orelse, path, diagnostics);
            }
            Stmt::While(while_stmt) => {
                walk_stmts(&while_stmt.body, path, diagnostics);
                walk_stmts(&while_stmt.orelse, path, diagnostics);
            }
            Stmt::Try(try_stmt) => {
                walk_stmts(&try_stmt.body, path, diagnostics);
                for handler in &try_stmt.handlers {
                    let ruff_python_ast::ExceptHandler::ExceptHandler(h) = handler;
                    walk_stmts(&h.body, path, diagnostics);
                }
                walk_stmts(&try_stmt.orelse, path, diagnostics);
                walk_stmts(&try_stmt.finalbody, path, diagnostics);
            }
            Stmt::With(with_stmt) => {
                walk_stmts(&with_stmt.body, path, diagnostics);
            }
            _ => {}
        }
    }
}

/// Collect parameter names that are annotated with `Literal[...]`.
fn collect_literal_params(func_def: &ruff_python_ast::StmtFunctionDef) -> HashSet<String> {
    let mut literal_params = HashSet::new();
    let params = &func_def.parameters;

    for param in params
        .args
        .iter()
        .chain(params.posonlyargs.iter())
        .chain(params.kwonlyargs.iter())
    {
        if let Some(ann) = &param.parameter.annotation {
            if is_literal_annotation(ann) {
                let _ = literal_params.insert(param.parameter.name.to_string());
            }
        }
    }

    if let Some(vararg) = &params.vararg {
        if let Some(ann) = &vararg.annotation {
            if is_literal_annotation(ann) {
                let _ = literal_params.insert(vararg.name.to_string());
            }
        }
    }

    if let Some(kwarg) = &params.kwarg {
        if let Some(ann) = &kwarg.annotation {
            if is_literal_annotation(ann) {
                let _ = literal_params.insert(kwarg.name.to_string());
            }
        }
    }

    literal_params
}

/// Check if an annotation expression is `Literal[...]`.
fn is_literal_annotation(expr: &Expr) -> bool {
    match expr {
        Expr::Subscript(sub) => {
            matches!(sub.value.as_ref(), Expr::Name(name) if name.id.as_str() == "Literal")
        }
        _ => false,
    }
}

/// Check function body statements for augmented assignments targeting `Literal`-annotated params.
fn check_body_for_aug_assign(
    stmts: &[Stmt],
    literal_params: &HashSet<String>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in stmts {
        if let Stmt::AugAssign(aug) = stmt {
            if let Expr::Name(name) = aug.target.as_ref() {
                if literal_params.contains(name.id.as_str()) {
                    let range = aug.range();
                    let span = Span {
                        start: range.start().to_u32(),
                        end: range.end().to_u32(),
                    };
                    diagnostics.push(make_diagnostic(name.id.as_str(), span, path));
                }
            }
        }
    }
}

fn make_diagnostic(var_name: &str, span: Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!("Augmented assignment to `{var_name}` widens its `Literal` type"),
        span,
        path: path.to_owned(),
        help: Some(format!(
            "Use a separate variable instead: `result = {var_name} + ...`"
        )),
        note: Some(
            "`a += x` is equivalent to `a = a + x`, which changes the type of `a` \
             from `Literal[...]` to the wider base type"
                .to_owned(),
        ),
    }
}
