//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Generic AST walkers shared between the resolver and downstream crates.
//!
//! These walkers fold the boilerplate of pre-order statement traversal so
//! callers only have to write the per-statement visitor body.

use ruff_python_ast::{ExceptHandler, Expr, ExprCall, ParameterWithDefault, Parameters, Stmt};

/// Walk every `Call` expression in **every expression position** reachable
/// from `stmts` — statement values, receivers (`C(1).method()`), argument
/// lists, container literals, conditional expressions, comprehensions,
/// f-strings, lambda bodies, decorators, and nested function and class
/// definitions.
///
/// A call is a call wherever it appears; visiting only statement-outermost
/// expressions silently skipped the same error the bare statement reports
/// ([#381](https://github.com/Nimblesite/Basilisk/issues/381)). Calls are
/// yielded in source order, outer call before its nested calls.
pub fn visit_calls(stmts: &[Stmt], visit: &mut impl FnMut(&ExprCall)) {
    let mut collector = CallWalker { visit };
    for stmt in stmts {
        ruff_python_ast::visitor::Visitor::visit_stmt(&mut collector, stmt);
    }
}

/// The [`ruff_python_ast::visitor::Visitor`] behind [`visit_calls`]: default
/// traversal everywhere, yielding each [`ExprCall`] on the way down.
struct CallWalker<'v, F> {
    visit: &'v mut F,
}

impl<'a, F: FnMut(&'a ExprCall)> ruff_python_ast::visitor::Visitor<'a> for CallWalker<'_, F> {
    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Call(call) = expr {
            (self.visit)(call);
        }
        ruff_python_ast::visitor::walk_expr(self, expr);
    }
}

/// Returns `true` when `expr` is either the bare name `target` (`X`) or an
/// attribute whose last segment is `target` (`<anything>.X`). Used to match
/// decorators and callees that may be referenced via a module prefix.
///
/// For example, `is_name_or_attr_named(&expr, "dataclass")` is `true` for
/// both `@dataclass` and `@dataclasses.dataclass`.
pub fn is_name_or_attr_named(expr: &Expr, target: &str) -> bool {
    match expr {
        Expr::Name(n) => n.id.as_str() == target,
        Expr::Attribute(a) => a.attr.as_str() == target,
        _ => false,
    }
}

/// Iterate every formal parameter of a function — positional-only, regular
/// positional, and keyword-only — in declaration order. Replaces the
/// `posonlyargs.iter().chain(args.iter()).chain(kwonlyargs.iter())` chain
/// repeated dozens of times across the resolver.
pub fn iter_all_params(params: &Parameters) -> impl Iterator<Item = &ParameterWithDefault> {
    params
        .posonlyargs
        .iter()
        .chain(params.args.iter())
        .chain(params.kwonlyargs.iter())
}

/// Push the child statement-bodies of `stmt` onto `recurse`. Used by the
/// generic walkers to factor out the common "what counts as a child block"
/// logic across `if`/`for`/`while`/`with`/`try`/`match`.
fn for_each_child_block<'a>(stmt: &'a Stmt, mut recurse: impl FnMut(&'a [Stmt])) {
    match stmt {
        Stmt::If(node) => {
            recurse(&node.body);
            for clause in &node.elif_else_clauses {
                recurse(&clause.body);
            }
        }
        Stmt::For(node) => {
            recurse(&node.body);
            recurse(&node.orelse);
        }
        Stmt::While(node) => {
            recurse(&node.body);
            recurse(&node.orelse);
        }
        Stmt::With(node) => {
            recurse(&node.body);
        }
        Stmt::Try(node) => {
            recurse(&node.body);
            for handler in &node.handlers {
                let ExceptHandler::ExceptHandler(h) = handler;
                recurse(&h.body);
            }
            recurse(&node.orelse);
            recurse(&node.finalbody);
        }
        Stmt::Match(node) => {
            for case in &node.cases {
                recurse(&case.body);
            }
        }
        _ => {}
    }
}

/// Walk `stmts` in pre-order, invoking `visit` on every statement.
///
/// Recurses into `if`/`for`/`while`/`with`/`try`/`match` bodies. Does NOT
/// recurse into nested `FunctionDef` or `ClassDef` — those introduce a new
/// scope and are handled separately by their owners.
pub fn walk_function_stmts(stmts: &[Stmt], visit: &mut impl FnMut(&Stmt)) {
    for stmt in stmts {
        visit(stmt);
        for_each_child_block(stmt, |children| walk_function_stmts(children, visit));
    }
}

/// Walk `stmts` in pre-order, invoking `visit` on every statement, including
/// statements inside nested `FunctionDef` and `ClassDef` bodies.
pub fn walk_all_stmts(stmts: &[Stmt], visit: &mut impl FnMut(&Stmt)) {
    for stmt in stmts {
        visit(stmt);
        for_each_child_block(stmt, |children| walk_all_stmts(children, visit));
        match stmt {
            Stmt::FunctionDef(node) => walk_all_stmts(&node.body, visit),
            Stmt::ClassDef(node) => walk_all_stmts(&node.body, visit),
            _ => {}
        }
    }
}
