//! BSK-E0118: Calling `super().method()` on an abstract method with no default
//! implementation.
//!
//! When a Protocol (or ABC) declares a method as `@abstractmethod` with only an
//! ellipsis (`...`) or `pass` body, calling `super().method()` from a subclass
//! is invalid because there is no concrete implementation to delegate to.
//!
//! ```python
//! from typing import Protocol
//! from abc import abstractmethod
//!
//! class PColor(Protocol):
//!     @abstractmethod
//!     def draw(self) -> str:
//!         ...
//!
//! class BadColor(PColor):
//!     def draw(self) -> str:
//!         return super().draw()  # E — no default implementation
//! ```

use std::collections::{HashMap, HashSet};

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0118",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0118",
};

/// Emits BSK-E0118 when a subclass method calls `super().method()` on a method
/// that is abstract and has no default implementation (body is `...` or `pass`).
pub(crate) struct SuperAbstractCall;

impl Rule for SuperAbstractCall {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let Ok(parsed) = basilisk_parser::parse_source(module.source.clone(), module.path.clone())
        else {
            return;
        };

        // Build a map: class_name -> set of method names that are abstract stubs.
        let abstract_stubs = collect_abstract_stub_methods(&parsed.ast.body);

        if abstract_stubs.is_empty() {
            return;
        }

        // Build a map: class_name -> list of base class names.
        let class_bases = collect_class_bases(&parsed.ast.body);

        // Walk all classes looking for super().method() calls in method bodies.
        for stmt in &parsed.ast.body {
            let Stmt::ClassDef(cls) = stmt else { continue };

            // Collect abstract stub methods from all bases of this class.
            let mut parent_stubs: HashSet<String> = HashSet::new();
            let empty_args: Box<[Expr]> = Box::new([]);
            for base_name in cls
                .arguments
                .as_ref()
                .map_or(&empty_args, |args| &args.args)
            {
                let Some(base) = expr_simple_name(base_name) else {
                    continue;
                };
                if let Some(stubs) = abstract_stubs.get(&base) {
                    parent_stubs.extend(stubs.iter().cloned());
                }
                // Also check transitive bases.
                collect_transitive_stubs(&base, &class_bases, &abstract_stubs, &mut parent_stubs);
            }

            if parent_stubs.is_empty() {
                continue;
            }

            // Walk method bodies for super().method_name() calls.
            for body_stmt in &cls.body {
                let Stmt::FunctionDef(func) = body_stmt else {
                    continue;
                };
                find_super_calls(&func.body, &parent_stubs, &module.path, diagnostics);
            }
        }
    }
}

/// Collect abstract methods with stub bodies (`...` or `pass`) per class.
fn collect_abstract_stub_methods(stmts: &[Stmt]) -> HashMap<String, HashSet<String>> {
    let mut result: HashMap<String, HashSet<String>> = HashMap::new();
    for stmt in stmts {
        let Stmt::ClassDef(cls) = stmt else { continue };
        let mut stubs = HashSet::new();
        for body_stmt in &cls.body {
            let Stmt::FunctionDef(func) = body_stmt else {
                continue;
            };
            let is_abstract = func.decorator_list.iter().any(
                |d| matches!(&d.expression, Expr::Name(n) if n.id.as_str() == "abstractmethod"),
            );
            if is_abstract && is_stub_body(&func.body) {
                let _ = stubs.insert(func.name.id.to_string());
            }
        }
        if !stubs.is_empty() {
            let _ = result.insert(cls.name.id.to_string(), stubs);
        }
    }
    result
}

/// Collect base class names for each class.
fn collect_class_bases(stmts: &[Stmt]) -> HashMap<String, Vec<String>> {
    let mut result: HashMap<String, Vec<String>> = HashMap::new();
    for stmt in stmts {
        let Stmt::ClassDef(cls) = stmt else { continue };
        let bases: Vec<String> = cls.arguments.as_ref().map_or_else(Vec::new, |args| {
            args.args.iter().filter_map(expr_simple_name).collect()
        });
        let _ = result.insert(cls.name.id.to_string(), bases);
    }
    result
}

/// Recursively collect abstract stub methods from transitive base classes.
fn collect_transitive_stubs(
    base_name: &str,
    class_bases: &HashMap<String, Vec<String>>,
    abstract_stubs: &HashMap<String, HashSet<String>>,
    out: &mut HashSet<String>,
) {
    let Some(bases) = class_bases.get(base_name) else {
        return;
    };
    for parent in bases {
        if let Some(stubs) = abstract_stubs.get(parent) {
            out.extend(stubs.iter().cloned());
        }
        collect_transitive_stubs(parent, class_bases, abstract_stubs, out);
    }
}

/// Check if a function body is `...` or `pass` (a stub with no real implementation).
fn is_stub_body(body: &[Stmt]) -> bool {
    // Allow optional leading docstring.
    let effective = if body.len() >= 2 {
        if let Some(Stmt::Expr(expr_stmt)) = body.first() {
            if matches!(&*expr_stmt.value, Expr::StringLiteral(_)) {
                body.get(1..).unwrap_or_default()
            } else {
                body
            }
        } else {
            body
        }
    } else {
        body
    };

    if effective.len() != 1 {
        return false;
    }
    let Some(first_effective) = effective.first() else {
        return false;
    };
    match first_effective {
        Stmt::Pass(_) => true,
        Stmt::Expr(expr_stmt) => matches!(&*expr_stmt.value, Expr::EllipsisLiteral(_)),
        _ => false,
    }
}

/// Extract a simple name from an expression.
fn expr_simple_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.to_string()),
        _ => None,
    }
}

/// Recursively walk statements looking for `super().method_name()` calls
/// where `method_name` is in the set of abstract stub methods.
fn find_super_calls(
    stmts: &[Stmt],
    parent_stubs: &HashSet<String>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Return(ret) => {
                if let Some(value) = &ret.value {
                    check_expr_for_super_call(value, parent_stubs, path, diagnostics);
                }
            }
            Stmt::Expr(expr_stmt) => {
                check_expr_for_super_call(&expr_stmt.value, parent_stubs, path, diagnostics);
            }
            Stmt::Assign(assign) => {
                check_expr_for_super_call(&assign.value, parent_stubs, path, diagnostics);
            }
            Stmt::AnnAssign(ann) => {
                if let Some(value) = &ann.value {
                    check_expr_for_super_call(value, parent_stubs, path, diagnostics);
                }
            }
            Stmt::If(if_stmt) => {
                find_super_calls(&if_stmt.body, parent_stubs, path, diagnostics);
                for clause in &if_stmt.elif_else_clauses {
                    find_super_calls(&clause.body, parent_stubs, path, diagnostics);
                }
            }
            Stmt::For(for_stmt) => {
                find_super_calls(&for_stmt.body, parent_stubs, path, diagnostics);
            }
            Stmt::While(while_stmt) => {
                find_super_calls(&while_stmt.body, parent_stubs, path, diagnostics);
            }
            Stmt::Try(try_stmt) => {
                find_super_calls(&try_stmt.body, parent_stubs, path, diagnostics);
                find_super_calls(&try_stmt.orelse, parent_stubs, path, diagnostics);
                find_super_calls(&try_stmt.finalbody, parent_stubs, path, diagnostics);
            }
            Stmt::With(with_stmt) => {
                find_super_calls(&with_stmt.body, parent_stubs, path, diagnostics);
            }
            _ => {}
        }
    }
}

/// Check if an expression is `super().method_name(...)` where `method_name` is
/// an abstract stub method.
fn check_expr_for_super_call(
    expr: &Expr,
    parent_stubs: &HashSet<String>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expr {
        Expr::Call(call) => {
            // Check if this call is `super().method_name(...)`
            if let Expr::Attribute(attr) = call.func.as_ref() {
                if is_super_call(&attr.value) {
                    let method_name = attr.attr.as_str();
                    if parent_stubs.contains(method_name) {
                        let span = Span {
                            start: call.range().start().to_u32(),
                            end: call.range().end().to_u32(),
                        };
                        diagnostics.push(Diagnostic {
                            code: CODE.clone(),
                            severity: Severity::Error,
                            message: format!(
                                "Cannot call `super().{method_name}()`: the method is abstract \
                                 with no default implementation"
                            ),
                            span,
                            path: path.to_owned(),
                            help: Some(format!(
                                "Provide a concrete implementation of `{method_name}` \
                                 instead of delegating to `super()`"
                            )),
                            note: Some(
                                "Abstract methods with only `...` or `pass` as their body \
                                 have no implementation to call via super()"
                                    .to_owned(),
                            ),
                        });
                    }
                }
            }
            // Also recurse into call arguments.
            for arg in &call.arguments.args {
                check_expr_for_super_call(arg, parent_stubs, path, diagnostics);
            }
        }
        Expr::BoolOp(op) => {
            for value in &op.values {
                check_expr_for_super_call(value, parent_stubs, path, diagnostics);
            }
        }
        Expr::BinOp(op) => {
            check_expr_for_super_call(&op.left, parent_stubs, path, diagnostics);
            check_expr_for_super_call(&op.right, parent_stubs, path, diagnostics);
        }
        Expr::If(if_expr) => {
            check_expr_for_super_call(&if_expr.test, parent_stubs, path, diagnostics);
            check_expr_for_super_call(&if_expr.body, parent_stubs, path, diagnostics);
            check_expr_for_super_call(&if_expr.orelse, parent_stubs, path, diagnostics);
        }
        _ => {}
    }
}

/// Check if an expression is `super()` (a call to `super` with no arguments).
fn is_super_call(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Call(call) if matches!(call.func.as_ref(), Expr::Name(n) if n.id.as_str() == "super")
            && call.arguments.args.is_empty()
    )
}
