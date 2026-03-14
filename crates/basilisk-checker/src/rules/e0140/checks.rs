//! Compatibility check functions for BSK-E0140.

use basilisk_resolver::Span;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::types::{
    ann_str, expr_name, extract_base_name, parse_callable_type, types_compat, CallableTypeInfo,
    FuncSig, ModuleContext, ProtocolInfo,
};

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
                    check_callable_compat(&cinfo, fsig, &ann_s, path, code, diag, span);
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

// ---------------------------------------------------------------------------
// Callable compatibility
// ---------------------------------------------------------------------------

/// Check that a function is compatible with a `Callable[...]` annotation.
fn check_callable_compat(
    ci: &CallableTypeInfo,
    func: &FuncSig,
    ann: &str,
    path: &str,
    code: &ErrorCode,
    diag: &mut Vec<Diagnostic>,
    span: Span,
) {
    if !ci.concatenate_prefix.is_empty() {
        let req = ci.concatenate_prefix.len();
        let fpos = func.positional_params.len();
        if fpos == 0 && !func.kw_only_params.is_empty() {
            diag.push(Diagnostic {
                code: code.clone(),
                severity: Severity::Error,
                message: format!(
                    "Function `{}` incompatible with `{ann}`: Concatenate requires positional params",
                    func.name
                ),
                span,
                path: path.to_owned(),
                help: None,
                note: None,
            });
            return;
        }
        if fpos < req {
            diag.push(Diagnostic {
                code: code.clone(),
                severity: Severity::Error,
                message: format!(
                    "Function `{}` incompatible with `{ann}`: needs at least {req} positional param(s) but has {fpos}",
                    func.name
                ),
                span,
                path: path.to_owned(),
                help: None,
                note: None,
            });
            return;
        }
        for (idx, exp) in ci.concatenate_prefix.iter().enumerate() {
            if let Some(param) = func.positional_params.get(idx) {
                let act = &param.type_annotation;
                if !act.is_empty() && !types_compat(exp, act) {
                    diag.push(Diagnostic {
                        code: code.clone(),
                        severity: Severity::Error,
                        message: format!(
                            "Function `{}` incompatible with `{ann}`: param {} type `{act}` vs required `{exp}`",
                            func.name,
                            idx + 1
                        ),
                        span,
                        path: path.to_owned(),
                        help: None,
                        note: None,
                    });
                }
            }
        }
        return;
    }
    if let Some(ptypes) = &ci.param_types {
        let exp = ptypes.len();
        let min = func
            .positional_params
            .iter()
            .filter(|p| !p.has_default)
            .count();
        let max = func.positional_params.len();
        if exp < min {
            diag.push(Diagnostic {
                code: code.clone(),
                severity: Severity::Error,
                message: format!(
                    "Function `{}` incompatible with `{ann}`: callable provides {exp} args but function requires {min}",
                    func.name
                ),
                span,
                path: path.to_owned(),
                help: None,
                note: None,
            });
        } else if exp > max && !func.has_varargs {
            diag.push(Diagnostic {
                code: code.clone(),
                severity: Severity::Error,
                message: format!(
                    "Function `{}` incompatible with `{ann}`: callable provides {exp} args but function accepts {max}",
                    func.name
                ),
                span,
                path: path.to_owned(),
                help: None,
                note: None,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Protocol compatibility
// ---------------------------------------------------------------------------

/// Check that a function is compatible with a Protocol's `__call__` signature.
pub(super) fn check_protocol_func_compat(
    proto: &ProtocolInfo,
    func: &FuncSig,
    path: &str,
    code: &ErrorCode,
    diag: &mut Vec<Diagnostic>,
    span: Span,
) {
    if proto.has_extra_attrs {
        diag.push(Diagnostic {
            code: code.clone(),
            severity: Severity::Error,
            message: format!(
                "Function `{}` cannot satisfy protocol `{}`: protocol has extra attributes",
                func.name, proto.name
            ),
            span,
            path: path.to_owned(),
            help: None,
            note: None,
        });
        return;
    }
    let Some(target) = &proto.call_sig else {
        return;
    };

    if check_protocol_varargs_kwargs(target, func, proto, path, code, diag, span) {
        return;
    }
    if check_protocol_param_counts(target, func, proto, path, code, diag, span) {
        return;
    }
    check_protocol_defaults_and_kw(target, func, proto, path, code, diag, span);
    check_protocol_param_types(target, func, proto, path, code, diag, span);
}

/// Check *args and **kwargs compatibility. Returns `true` if a fatal mismatch was found.
fn check_protocol_varargs_kwargs(
    target: &FuncSig,
    func: &FuncSig,
    proto: &ProtocolInfo,
    path: &str,
    code: &ErrorCode,
    diag: &mut Vec<Diagnostic>,
    span: Span,
) -> bool {
    if target.has_varargs && !func.has_varargs && target.positional_params.is_empty() {
        diag.push(Diagnostic {
            code: code.clone(),
            severity: Severity::Error,
            message: format!(
                "Function `{}` incompatible with `{}`: missing `*args`",
                func.name, proto.name
            ),
            span,
            path: path.to_owned(),
            help: None,
            note: None,
        });
        return true;
    }
    if target.has_kwargs && !func.has_kwargs {
        diag.push(Diagnostic {
            code: code.clone(),
            severity: Severity::Error,
            message: format!(
                "Function `{}` incompatible with `{}`: missing `**kwargs`",
                func.name, proto.name
            ),
            span,
            path: path.to_owned(),
            help: None,
            note: None,
        });
        return true;
    }
    false
}

/// Check positional parameter count compatibility. Returns `true` if a fatal mismatch was found.
fn check_protocol_param_counts(
    target: &FuncSig,
    func: &FuncSig,
    proto: &ProtocolInfo,
    path: &str,
    code: &ErrorCode,
    diag: &mut Vec<Diagnostic>,
    span: Span,
) -> bool {
    let src_req = func
        .positional_params
        .iter()
        .filter(|p| !p.has_default)
        .count();
    if src_req > target.positional_params.len() && !target.has_varargs {
        diag.push(Diagnostic {
            code: code.clone(),
            severity: Severity::Error,
            message: format!(
                "Function `{}` incompatible with `{}`: too many required params",
                func.name, proto.name
            ),
            span,
            path: path.to_owned(),
            help: None,
            note: None,
        });
        return true;
    }
    let tgt_req = target
        .positional_params
        .iter()
        .filter(|p| !p.has_default)
        .count();
    if tgt_req > func.positional_params.len() && !func.has_varargs {
        diag.push(Diagnostic {
            code: code.clone(),
            severity: Severity::Error,
            message: format!(
                "Function `{}` incompatible with `{}`: missing required params",
                func.name, proto.name
            ),
            span,
            path: path.to_owned(),
            help: None,
            note: None,
        });
        return true;
    }
    false
}

/// Check default-argument requirements, keyword-only params, and positional-only mismatches.
fn check_protocol_defaults_and_kw(
    target: &FuncSig,
    func: &FuncSig,
    proto: &ProtocolInfo,
    path: &str,
    code: &ErrorCode,
    diag: &mut Vec<Diagnostic>,
    span: Span,
) {
    // Default arg check
    for (idx, tp) in target.positional_params.iter().enumerate() {
        if tp.has_default {
            if let Some(sp) = func.positional_params.get(idx) {
                if !sp.has_default && !func.has_varargs {
                    diag.push(Diagnostic {
                        code: code.clone(),
                        severity: Severity::Error,
                        message: format!(
                            "Function `{}` incompatible with `{}`: param `{}` needs default",
                            func.name, proto.name, sp.name
                        ),
                        span,
                        path: path.to_owned(),
                        help: None,
                        note: None,
                    });
                }
            }
        }
    }
    // Keyword-only params
    for tkw in &target.kw_only_params {
        let has_kw = func.kw_only_params.iter().any(|sk| sk.name == tkw.name);
        let has_reg = func
            .positional_params
            .iter()
            .any(|sp| sp.name == tkw.name && !sp.is_positional_only);
        if !has_kw && !has_reg && !func.has_kwargs {
            diag.push(Diagnostic {
                code: code.clone(),
                severity: Severity::Error,
                message: format!(
                    "Function `{}` incompatible with `{}`: missing keyword param `{}`",
                    func.name, proto.name, tkw.name
                ),
                span,
                path: path.to_owned(),
                help: None,
                note: None,
            });
        }
    }
    // Positional-only mismatch
    for (idx, tp) in target.positional_params.iter().enumerate() {
        if !tp.is_positional_only {
            if let Some(sp) = func.positional_params.get(idx) {
                if sp.is_positional_only {
                    diag.push(Diagnostic {
                        code: code.clone(),
                        severity: Severity::Error,
                        message: format!(
                            "Function `{}` incompatible with `{}`: param `{}` is pos-only but must accept keyword",
                            func.name, proto.name, sp.name
                        ),
                        span,
                        path: path.to_owned(),
                        help: None,
                        note: None,
                    });
                }
            }
        }
    }
}

/// Check parameter type compatibility (contravariant), *args type, and **kwargs type.
fn check_protocol_param_types(
    target: &FuncSig,
    func: &FuncSig,
    proto: &ProtocolInfo,
    path: &str,
    code: &ErrorCode,
    diag: &mut Vec<Diagnostic>,
    span: Span,
) {
    // Param type compat (contravariant)
    for (idx, tp) in target.positional_params.iter().enumerate() {
        if let Some(sp) = func.positional_params.get(idx) {
            if !tp.type_annotation.is_empty()
                && !sp.type_annotation.is_empty()
                && !types_compat(&tp.type_annotation, &sp.type_annotation)
            {
                diag.push(Diagnostic {
                    code: code.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Function `{}` incompatible with `{}`: param `{}` type `{}` vs `{}`",
                        func.name, proto.name, sp.name, sp.type_annotation, tp.type_annotation
                    ),
                    span,
                    path: path.to_owned(),
                    help: None,
                    note: None,
                });
            }
        }
    }
    // *args type compat
    if target.has_varargs
        && func.has_varargs
        && !target.varargs_type.is_empty()
        && !func.varargs_type.is_empty()
        && !types_compat(&target.varargs_type, &func.varargs_type)
    {
        diag.push(Diagnostic {
            code: code.clone(),
            severity: Severity::Error,
            message: format!(
                "Function `{}` incompatible with `{}`: *args type `{}` vs `{}`",
                func.name, proto.name, func.varargs_type, target.varargs_type
            ),
            span,
            path: path.to_owned(),
            help: None,
            note: None,
        });
    }
    // **kwargs type compat
    if target.has_kwargs
        && func.has_kwargs
        && !target.kwargs_type.is_empty()
        && !func.kwargs_type.is_empty()
        && !types_compat(&target.kwargs_type, &func.kwargs_type)
    {
        diag.push(Diagnostic {
            code: code.clone(),
            severity: Severity::Error,
            message: format!(
                "Function `{}` incompatible with `{}`: **kwargs type `{}` vs `{}`",
                func.name, proto.name, func.kwargs_type, target.kwargs_type
            ),
            span,
            path: path.to_owned(),
            help: None,
            note: None,
        });
    }
}
