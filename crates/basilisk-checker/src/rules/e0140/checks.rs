//! Compatibility check functions for BSK-E0140.

use basilisk_resolver::Span;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::callable::{check_callable_compat, parse_callable_type};
use super::context::{ann_str, expr_name, extract_base_name, ModuleContext};
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
    for stmt in stmts {
        match stmt {
            Stmt::AnnAssign(ann) => {
                if let Some(name) = expr_name(&ann.target) {
                    annotations.push((name.to_owned(), (*ann.annotation).clone()));
                }
                if let Some(value) = &ann.value {
                    let span = Span {
                        start: ann.range().start().to_u32(),
                        end: ann.range().end().to_u32(),
                    };
                    check_assignment(&ann.annotation, value, ctx, path, code, diag, span);
                }
            }
            Stmt::Assign(assign) => {
                if assign.targets.len() == 1 {
                    if let Some(target_name) = assign.targets.first().and_then(expr_name) {
                        if let Some((_, prev_ann)) =
                            annotations.iter().rev().find(|(n, _)| n == target_name)
                        {
                            let span = Span {
                                start: assign.range().start().to_u32(),
                                end: assign.range().end().to_u32(),
                            };
                            check_assignment(prev_ann, &assign.value, ctx, path, code, diag, span);
                        }
                    }
                }
            }
            Stmt::FunctionDef(func) => {
                check_stmts_in_func(&func.body, ctx, path, code, diag);
            }
            Stmt::ClassDef(cls) => check_stmts(&cls.body, ctx, path, code, diag),
            _ => {}
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
                    let span = Span {
                        start: ann.range().start().to_u32(),
                        end: ann.range().end().to_u32(),
                    };
                    check_assignment(&ann.annotation, value, ctx, path, code, diag, span);
                }
            }
            Stmt::Assign(assign) => {
                if assign.targets.len() == 1 {
                    if let Some(target_name) = assign.targets.first().and_then(expr_name) {
                        if let Some((_, prev_ann)) = local_annotations
                            .iter()
                            .rev()
                            .find(|(n, _)| n == target_name)
                        {
                            let span = Span {
                                start: assign.range().start().to_u32(),
                                end: assign.range().end().to_u32(),
                            };
                            check_assignment(prev_ann, &assign.value, ctx, path, code, diag, span);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

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
    let ann_s = ann_str(annotation);

    // Callable[...] annotation
    if ann_s.starts_with("Callable[") {
        if let Some(cinfo) = parse_callable_type(&ann_s) {
            if let Some(fname) = value_name {
                if let Some(fsig) = ctx.find_func(fname) {
                    check_callable_compat(&cinfo, fsig, &ann_s, path, diag, span, code);
                }
            }
        }
        return;
    }

    // Protocol type annotation
    let base = extract_base_name(&ann_s);
    if ctx.is_non_protocol_class(&base) {
        if let Some(fname) = value_name {
            if ctx.find_func(fname).is_some() {
                diag.push(Diagnostic {
                    code: code.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Cannot assign function `{fname}` to non-protocol type `{base}`"
                    ),
                    span,
                    path: path.to_owned(),
                    help: None,
                    note: None,
                });
            }
        }
        return;
    }
    if let Some(protocol) = ctx.find_protocol(&base) {
        if let Some(fname) = value_name {
            if let Some(fsig) = ctx.find_func(fname) {
                check_protocol_func_compat(protocol, fsig, path, code, diag, span);
            }
        }
    }
}
