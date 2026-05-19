//! Generic AST walkers shared between the resolver and downstream crates.
//!
//! These walkers fold the boilerplate of pre-order statement traversal so
//! callers only have to write the per-statement visitor body.

use ruff_python_ast::{ExceptHandler, Expr, ExprCall, ParameterWithDefault, Parameters, Stmt};

/// Walk every `Call` expression reachable from `stmts`, including nested
/// argument calls, into nested control-flow bodies and into nested function
/// and class definitions.
///
/// For each call, `visit` is invoked with the [`ExprCall`] node. Callers do
/// not need to recurse manually — `visit_calls` traverses arguments first,
/// then yields the outer call.
pub fn visit_calls(stmts: &[Stmt], visit: &mut impl FnMut(&ExprCall)) {
    walk_all_stmts(stmts, &mut |stmt| match stmt {
        Stmt::Expr(node) => visit_calls_in_expr(&node.value, visit),
        Stmt::Assign(node) => visit_calls_in_expr(&node.value, visit),
        Stmt::AnnAssign(node) => {
            if let Some(val) = node.value.as_deref() {
                visit_calls_in_expr(val, visit);
            }
        }
        Stmt::Return(node) => {
            if let Some(val) = &node.value {
                visit_calls_in_expr(val, visit);
            }
        }
        _ => {}
    });
}

fn visit_calls_in_expr(expr: &Expr, visit: &mut impl FnMut(&ExprCall)) {
    if let Expr::Call(call) = expr {
        for arg in &call.arguments.args {
            visit_calls_in_expr(arg, visit);
        }
        visit(call);
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
