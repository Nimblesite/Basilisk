//! Implements [TYPEINF-ANNOTATION-RESOLUTION] — the span → annotation-node
//! index. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-ANNOTATION-RESOLUTION
//!
//! Resolver-derived data (`FunctionInfo::return_annotation_span`,
//! `VariableInfo::annotation_span`, …) records an annotation by span. This
//! index maps that span straight back to the AST node it came from, so a rule
//! holding a span resolves a **type expression** instead of slicing the source
//! and re-reading it as text.

use std::collections::HashMap;

use ruff_python_ast::{ExceptHandler, Expr, ModModule, Parameters, Stmt};
use ruff_text_size::Ranged as _;

/// Index every annotation expression in the module by its span.
pub(super) fn annotation_nodes(module: &ModModule) -> HashMap<(u32, u32), &Expr> {
    let mut index = HashMap::new();
    collect(&module.body, &mut index);
    index
}

/// Record one annotation node, keyed by its exact span.
fn record<'m>(expr: &'m Expr, index: &mut HashMap<(u32, u32), &'m Expr>) {
    let range = expr.range();
    let _ = index.insert(
        (u32::from(range.start()), u32::from(range.end())),
        expr,
    );
}

/// Walk every statement body: annotations appear at any nesting depth.
fn collect<'m>(body: &'m [Stmt], index: &mut HashMap<(u32, u32), &'m Expr>) {
    for stmt in body {
        collect_one(stmt, index);
    }
}

fn collect_one<'m>(stmt: &'m Stmt, index: &mut HashMap<(u32, u32), &'m Expr>) {
    match stmt {
        Stmt::FunctionDef(func) => {
            if let Some(returns) = func.returns.as_deref() {
                record(returns, index);
            }
            parameters(&func.parameters, index);
            collect(&func.body, index);
        }
        Stmt::AnnAssign(assign) => record(&assign.annotation, index),
        Stmt::ClassDef(class) => collect(&class.body, index),
        Stmt::If(if_stmt) => {
            collect(&if_stmt.body, index);
            for clause in &if_stmt.elif_else_clauses {
                collect(&clause.body, index);
            }
        }
        Stmt::For(for_stmt) => {
            collect(&for_stmt.body, index);
            collect(&for_stmt.orelse, index);
        }
        Stmt::While(while_stmt) => {
            collect(&while_stmt.body, index);
            collect(&while_stmt.orelse, index);
        }
        Stmt::With(with_stmt) => collect(&with_stmt.body, index),
        Stmt::Try(try_stmt) => {
            collect(&try_stmt.body, index);
            for ExceptHandler::ExceptHandler(handler) in &try_stmt.handlers {
                collect(&handler.body, index);
            }
            collect(&try_stmt.orelse, index);
            collect(&try_stmt.finalbody, index);
        }
        Stmt::Match(match_stmt) => {
            for case in &match_stmt.cases {
                collect(&case.body, index);
            }
        }
        _ => {}
    }
}

/// Every annotated parameter of one signature, `*args` / `**kwargs` included.
fn parameters<'m>(params: &'m Parameters, index: &mut HashMap<(u32, u32), &'m Expr>) {
    let positional = params
        .posonlyargs
        .iter()
        .chain(params.args.iter())
        .chain(params.kwonlyargs.iter())
        .map(|param| &param.parameter);
    let starred = params
        .vararg
        .as_deref()
        .into_iter()
        .chain(params.kwarg.as_deref());
    for parameter in positional.chain(starred) {
        if let Some(annotation) = parameter.annotation.as_deref() {
            record(annotation, index);
        }
    }
}
