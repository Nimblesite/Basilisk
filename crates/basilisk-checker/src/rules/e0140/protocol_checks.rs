//! Protocol compatibility check functions for BSK-E0140.

use basilisk_resolver::Span;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::callable::types_compat;
use super::context::{FuncSig, ProtocolInfo};

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

/// Returns `true` when `target` and `source` types are compatible.
fn types_compat(target: &str, source: &str) -> bool {
    use super::types::types_compat as tc;
    tc(target, source)
}
