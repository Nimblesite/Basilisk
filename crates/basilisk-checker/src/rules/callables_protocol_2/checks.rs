//! Implements [`callables_protocol_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Compatibility check functions for `callables_protocol_2`.

use std::collections::HashMap;

use basilisk_resolver::Span;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::context::{annotation_base_name, expr_name, ModuleContext};
use super::protocol::check_protocol_func_compat;

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

// ---------------------------------------------------------------------------
// Statement traversal
// ---------------------------------------------------------------------------

/// Walk top-level statements checking annotated assignments for callable/protocol compatibility.
pub(super) fn check_stmts(
    stmts: &[Stmt],
    ctx: &ModuleContext,
    path: &str,
    code: &ErrorCode,
    diag: &mut Vec<Diagnostic>,
) {
    let mut annotations: Vec<(String, Expr)> = Vec::new();
    let proto_typed = collect_proto_typed_names(stmts, ctx);
    for stmt in stmts {
        match stmt {
            Stmt::AnnAssign(ann) => {
                if let Some(name) = expr_name(&ann.target) {
                    annotations.push((name.to_owned(), (*ann.annotation).clone()));
                }
                if let Some(value) = &ann.value {
                    let span = mk_span(ann.range());
                    check_assignment(&ann.annotation, value, ctx, path, code, diag, span);
                }
            }
            Stmt::Assign(assign) => check_top_assign(assign, &annotations, ctx, path, code, diag),
            Stmt::FunctionDef(func) => {
                check_stmts_in_func(&func.body, ctx, path, code, diag);
            }
            Stmt::ClassDef(cls) => check_stmts(&cls.body, ctx, path, code, diag),
            Stmt::Expr(expr_stmt) => {
                check_attr_access_in_expr(&expr_stmt.value, &proto_typed, ctx, path, code, diag);
            }
            _ => {}
        }
    }
}

/// Handle a top-level `Assign` statement for callable/protocol checking.
fn check_top_assign(
    assign: &ruff_python_ast::StmtAssign,
    annotations: &[(String, Expr)],
    ctx: &ModuleContext,
    path: &str,
    code: &ErrorCode,
    diag: &mut Vec<Diagnostic>,
) {
    if assign.targets.len() != 1 {
        return;
    }
    if let Some(target_name) = assign.targets.first().and_then(expr_name) {
        if let Some((_, prev_ann)) = annotations.iter().rev().find(|(n, _)| n == target_name) {
            let span = mk_span(assign.range());
            check_assignment(prev_ann, &assign.value, ctx, path, code, diag, span);
        }
    }
}

/// Walk function-body statements checking for callable/protocol compatibility.
pub(super) fn check_stmts_in_func(
    stmts: &[Stmt],
    ctx: &ModuleContext,
    path: &str,
    code: &ErrorCode,
    diag: &mut Vec<Diagnostic>,
) {
    let mut local_annotations: Vec<(String, Expr)> = Vec::new();
    for stmt in stmts {
        match stmt {
            Stmt::AnnAssign(ann) => {
                if let Some(name) = expr_name(&ann.target) {
                    local_annotations.push((name.to_owned(), (*ann.annotation).clone()));
                }
                if let Some(value) = &ann.value {
                    let span = mk_span(ann.range());
                    check_assignment(&ann.annotation, value, ctx, path, code, diag, span);
                }
            }
            Stmt::Assign(assign) => {
                handle_func_body_assign(assign, &local_annotations, ctx, path, code, diag);
            }
            _ => {}
        }
    }
}

/// Handle an assignment inside a function body.
fn handle_func_body_assign(
    assign: &ruff_python_ast::StmtAssign,
    local_annotations: &[(String, Expr)],
    ctx: &ModuleContext,
    path: &str,
    code: &ErrorCode,
    diag: &mut Vec<Diagnostic>,
) {
    if assign.targets.len() != 1 {
        return;
    }
    let Some(target) = assign.targets.first() else {
        return;
    };
    let Some(name) = expr_name(target) else {
        return;
    };
    if let Some((_, prev_ann)) = local_annotations.iter().rev().find(|(n, _)| n == name) {
        let span = mk_span(assign.range());
        check_assignment(prev_ann, &assign.value, ctx, path, code, diag, span);
    }
}

// ---------------------------------------------------------------------------
// Protocol attribute checking
// ---------------------------------------------------------------------------

/// Collect names of decorated functions/variables whose type is a protocol.
fn collect_proto_typed_names(stmts: &[Stmt], ctx: &ModuleContext) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for stmt in stmts {
        if let Stmt::FunctionDef(func) = stmt {
            if let Some(proto_name) = infer_decorated_proto_type(func, ctx) {
                let _ = result.insert(func.name.to_string(), proto_name);
            }
        }
    }
    result
}

/// If the function is decorated and the decorator returns a protocol type, return
/// the protocol base name.
fn infer_decorated_proto_type(
    func: &ruff_python_ast::StmtFunctionDef,
    ctx: &ModuleContext,
) -> Option<String> {
    for dec in &func.decorator_list {
        let dec_name = expr_name(&dec.expression)?;
        let dec_func = ctx.find_func(dec_name)?;
        // The return annotation's base class is captured structurally from
        // the AST ([ASTREBUILD-LAW]), never from rendered annotation text.
        let Some(base) = dec_func.return_base_name.as_deref() else {
            continue;
        };
        if ctx.find_protocol(base).is_some() {
            return Some(base.to_owned());
        }
    }
    None
}

/// Walk an expression tree and check attribute access on protocol-typed names.
fn check_attr_access_in_expr(
    expr: &Expr,
    proto_typed: &HashMap<String, String>,
    ctx: &ModuleContext,
    path: &str,
    code: &ErrorCode,
    diag: &mut Vec<Diagnostic>,
) {
    match expr {
        Expr::Attribute(attr) => {
            if let Some(var_name) = expr_name(&attr.value) {
                if let Some(proto_name) = proto_typed.get(var_name) {
                    if let Some(proto) = ctx.find_protocol(proto_name) {
                        let attr_name = attr.attr.as_str();
                        if !proto.attrs.iter().any(|a| a.name == attr_name) {
                            diag.push(error_diagnostic_owned(
                                code.clone(),
                                format!("Protocol `{proto_name}` has no attribute `{attr_name}`"),
                                mk_span(attr.range()),
                                path,
                                None,
                                None,
                            ));
                        }
                    }
                }
            }
        }
        Expr::Call(call) => {
            for arg in &call.arguments.args {
                check_attr_access_in_expr(arg, proto_typed, ctx, path, code, diag);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Assignment dispatch
// ---------------------------------------------------------------------------

/// Dispatch a single annotated assignment to the appropriate compatibility check.
fn check_assignment(
    annotation: &Expr,
    value: &Expr,
    ctx: &ModuleContext,
    path: &str,
    code: &ErrorCode,
    diag: &mut Vec<Diagnostic>,
    span: Span,
) {
    let value_name = expr_name(value);

    // The annotation's base class is read structurally from the AST
    // ([ASTREBUILD-LAW]): a bare name or the base of a subscript. Anything
    // else names no class this context models, so the check abstains.
    let Some(base) = annotation_base_name(annotation) else {
        return;
    };
    if ctx.is_non_protocol_class(base) {
        if let Some(fname) = value_name {
            if ctx.find_func(fname).is_some() {
                diag.push(error_diagnostic_owned(
                    code.clone(),
                    format!("Cannot assign function `{fname}` to non-protocol type `{base}`"),
                    span,
                    path,
                    None,
                    None,
                ));
            }
        }
        return;
    }
    if let Some(protocol) = ctx.find_protocol(base) {
        if let Some(fname) = value_name {
            if let Some(fsig) = ctx.find_func(fname) {
                check_protocol_func_compat(protocol, fsig, path, code, diag, span);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

/// Create a [`Span`] from a ruff range.
fn mk_span(range: ruff_text_size::TextRange) -> Span {
    Span {
        start: range.start().to_u32(),
        end: range.end().to_u32(),
    }
}
