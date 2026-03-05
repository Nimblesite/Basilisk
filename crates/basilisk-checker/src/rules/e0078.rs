//! BSK-E0078: `Self` type violations in generics.
//!
//! This rule detects two kinds of `Self` type violations:
//!
//! 1. **Return type mismatch**: A method (or classmethod) annotated `-> Self`
//!    returns a concrete class constructor call (e.g. `return Shape()`) instead
//!    of `self`, `cls()`, or another `Self`-compatible expression. In a
//!    subclass, `Self` resolves to the subclass type, so returning the parent
//!    class constructor is a type error.
//!
//! 2. **`Self` is not subscriptable**: `Self` cannot be parameterized (e.g.
//!    `Self[int]`). It already captures the full generic specialization of the
//!    enclosing class.
//!
//! ```python
//! from typing import Self
//!
//! class Shape:
//!     def method2(self) -> Self:
//!         return Shape()  # E — should return self, not Shape()
//!
//!     @classmethod
//!     def cls_method2(cls) -> Self:
//!         return Shape()  # E — should return cls(), not Shape()
//!
//! class Container(Generic[T]):
//!     def foo(self, other: Self[int]) -> None:  # E — Self is not subscriptable
//!         pass
//! ```

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0078",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0078",
};

/// Emits BSK-E0078 for `Self` return type mismatches and `Self` subscript usage.
pub(crate) struct SelfTypeViolation;

impl Rule for SelfTypeViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        let Ok(parsed) = basilisk_parser::parse_source(source.clone(), path.clone()) else {
            return;
        };

        for stmt in &parsed.ast.body {
            check_stmt(stmt, source, path, diagnostics);
        }
    }
}

/// Walk a statement looking for class definitions that contain Self-related violations.
fn check_stmt(
    stmt: &ruff_python_ast::Stmt,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::Stmt;

    let Stmt::ClassDef(cls) = stmt else {
        return;
    };

    let class_name = cls.name.as_str();

    for body_stmt in &cls.body {
        if let Stmt::FunctionDef(func_def) = body_stmt {
            check_method_self_return(func_def, class_name, source, path, diagnostics);
            check_self_subscript_in_params(func_def, path, diagnostics);
        }
    }
}

/// Check a method for returning a concrete class when `-> Self` is declared.
fn check_method_self_return(
    func_def: &ruff_python_ast::StmtFunctionDef,
    class_name: &str,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_text_size::Ranged as _;

    // Check if the return annotation is `Self`.
    let Some(return_ann) = &func_def.returns else {
        return;
    };

    let range = return_ann.range();
    let Some(ann_text) = source.get(range.start().to_usize()..range.end().to_usize()) else {
        return;
    };

    if ann_text.trim() != "Self" {
        return;
    }

    // Walk the function body for return statements returning a concrete class constructor.
    for body_stmt in &func_def.body {
        check_return_stmt_for_concrete_class(body_stmt, class_name, path, diagnostics);
    }
}

/// Recursively check statements for `return ClassName()` patterns.
fn check_return_stmt_for_concrete_class(
    stmt: &ruff_python_ast::Stmt,
    class_name: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::{Expr, Stmt};
    use ruff_text_size::Ranged as _;

    match stmt {
        Stmt::Return(ret) => {
            let Some(value) = &ret.value else {
                return;
            };

            // Check if the return value is a call to the class constructor.
            let Expr::Call(call) = value.as_ref() else {
                return;
            };

            let callee_name = match call.func.as_ref() {
                Expr::Name(name) => name.id.as_str(),
                _ => return,
            };

            // If the return is `ClassName()` where ClassName is the enclosing class,
            // that's an error because Self could be a subclass.
            if callee_name == class_name {
                let range = ret.range();
                let span = Span {
                    start: range.start().to_u32(),
                    end: range.end().to_u32(),
                };
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Return type `Self` requires returning the current class instance, \
                         but `{class_name}()` is not `Self` — it would be wrong in a subclass"
                    ),
                    span,
                    path: path.to_owned(),
                    help: Some(format!(
                        "Return `self` (or `cls()` in a classmethod) instead of \
                         `{class_name}()`"
                    )),
                    note: Some(
                        "`Self` resolves to the concrete subclass type, not the parent class"
                            .to_owned(),
                    ),
                });
            }
        }
        Stmt::If(if_stmt) => {
            for body_stmt in &if_stmt.body {
                check_return_stmt_for_concrete_class(body_stmt, class_name, path, diagnostics);
            }
            for clause in &if_stmt.elif_else_clauses {
                for body_stmt in &clause.body {
                    check_return_stmt_for_concrete_class(body_stmt, class_name, path, diagnostics);
                }
            }
        }
        Stmt::For(for_stmt) => {
            for body_stmt in &for_stmt.body {
                check_return_stmt_for_concrete_class(body_stmt, class_name, path, diagnostics);
            }
        }
        Stmt::While(while_stmt) => {
            for body_stmt in &while_stmt.body {
                check_return_stmt_for_concrete_class(body_stmt, class_name, path, diagnostics);
            }
        }
        _ => {}
    }
}

/// Check parameter annotations in a method for `Self[...]` subscript usage.
fn check_self_subscript_in_params(
    func_def: &ruff_python_ast::StmtFunctionDef,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Check all parameters for Self[...] annotation.
    for param_with_default in &func_def.parameters.args {
        let param = &param_with_default.parameter;
        if let Some(ann) = &param.annotation {
            check_expr_for_self_subscript(ann, path, diagnostics);
        }
    }

    // Check *args and **kwargs annotations too.
    if let Some(vararg) = &func_def.parameters.vararg {
        if let Some(ann) = &vararg.annotation {
            check_expr_for_self_subscript(ann, path, diagnostics);
        }
    }
    if let Some(kwarg) = &func_def.parameters.kwarg {
        if let Some(ann) = &kwarg.annotation {
            check_expr_for_self_subscript(ann, path, diagnostics);
        }
    }
}

/// Check if an expression is a `Self[...]` subscript.
fn check_expr_for_self_subscript(
    expr: &ruff_python_ast::Expr,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::Expr;
    use ruff_text_size::Ranged as _;

    let Expr::Subscript(sub) = expr else {
        return;
    };

    let Expr::Name(name) = sub.value.as_ref() else {
        return;
    };

    if name.id.as_str() != "Self" {
        return;
    }

    let range = sub.range();
    let span = Span {
        start: range.start().to_u32(),
        end: range.end().to_u32(),
    };
    diagnostics.push(Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: "`Self` is not subscriptable — it already captures the full type \
                  of the enclosing class"
            .to_owned(),
        span,
        path: path.to_owned(),
        help: Some("Use `Self` without type arguments".to_owned()),
        note: Some(
            "`Self` in a generic class automatically includes the class's type \
             parameters; subscripting it is invalid"
                .to_owned(),
        ),
    });
}
