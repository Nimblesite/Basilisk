//! Implements [`callables_protocol_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! Protocol compatibility checking for `callables_protocol_2`.

use basilisk_resolver::Span;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::callable::types_compat;
use super::context::{FuncSig, ProtocolInfo};

/// Bundle of `(target, func, proto, path, code, diag, span)` shared by every
/// protocol-compatibility sub-check. The original signatures repeated all
/// seven parameters; threading the context through one struct collapses the
/// duplication and keeps call sites short.
struct ProtoCheckCtx<'a> {
    target: &'a FuncSig,
    func: &'a FuncSig,
    proto: &'a ProtocolInfo,
    path: &'a str,
    code: &'a ErrorCode,
    span: Span,
    diag: &'a mut Vec<Diagnostic>,
}

impl ProtoCheckCtx<'_> {
    /// Push a no-help/no-note error diagnostic using this context's `code`,
    /// `span` and `path`. All E0140 protocol diagnostics share that triple
    /// and pass `None, None` for help/note.
    fn push_err(&mut self, message: String) {
        self.diag.push(error_diagnostic_owned(
            self.code.clone(),
            message,
            self.span,
            self.path,
            None,
            None,
        ));
    }
}

/// Check whether `func` satisfies the `proto` Protocol contract. Emits
/// diagnostics for any incompatibility found.
pub(super) fn check_protocol_func_compat(
    proto: &ProtocolInfo,
    func: &FuncSig,
    path: &str,
    code: &ErrorCode,
    diag: &mut Vec<Diagnostic>,
    span: Span,
) {
    if proto.has_extra_attrs {
        diag.push(error_diagnostic_owned(
            code.clone(),
            format!(
                "Function `{}` cannot satisfy protocol `{}`: protocol has extra attributes",
                func.name, proto.name
            ),
            span,
            path,
            None,
            None,
        ));
        return;
    }
    let Some(target) = &proto.call_sig else {
        return;
    };

    // If protocol has overloads, check source against each overload signature
    if !proto.overload_sigs.is_empty() {
        check_overload_compat(&proto.overload_sigs, func, proto, path, code, diag, span);
        return;
    }

    let mut ctx = ProtoCheckCtx {
        target,
        func,
        proto,
        path,
        code,
        span,
        diag,
    };
    if check_protocol_varargs_kwargs(&mut ctx) {
        return;
    }
    if check_protocol_param_counts(&mut ctx) {
        return;
    }
    check_protocol_defaults_and_kw(&mut ctx);
    check_protocol_param_types(&mut ctx);
}

/// Check that a source function can handle all overloaded `__call__` signatures.
///
/// For each overload, every parameter type must be accepted by the source function.
fn check_overload_compat(
    overloads: &[FuncSig],
    func: &FuncSig,
    proto: &ProtocolInfo,
    path: &str,
    code: &ErrorCode,
    diag: &mut Vec<Diagnostic>,
    span: Span,
) {
    for overload in overloads {
        for (idx, op) in overload.positional_params.iter().enumerate() {
            if let Some(fp) = func.positional_params.get(idx) {
                if !op.type_annotation.is_empty()
                    && !fp.type_annotation.is_empty()
                    && !types_compat(&op.type_annotation, &fp.type_annotation)
                {
                    diag.push(error_diagnostic_owned(
                        code.clone(),
                        format!(
                            "Function `{}` incompatible with `{}`: overload param `{}` type `{}` not accepted by `{}`",
                            func.name, proto.name, op.name, op.type_annotation, fp.type_annotation
                        ),
                        span,
                        path,
                        None,
                        None,
                    ));
                    return;
                }
            }
        }
    }
}

/// Check `*args` and `**kwargs` compatibility.
///
/// Returns `true` if a fatal mismatch was found and further checks should stop.
fn check_protocol_varargs_kwargs(ctx: &mut ProtoCheckCtx<'_>) -> bool {
    let (target, func, proto) = (ctx.target, ctx.func, ctx.proto);
    if target.has_varargs && !func.has_varargs && target.positional_params.is_empty() {
        ctx.push_err(format!(
            "Function `{}` incompatible with `{}`: missing `*args`",
            func.name, proto.name
        ));
        return true;
    }
    if target.has_kwargs && !func.has_kwargs {
        ctx.push_err(format!(
            "Function `{}` incompatible with `{}`: missing `**kwargs`",
            func.name, proto.name
        ));
        return true;
    }
    // A protocol whose `__call__` declared `**kwargs: Unpack[TypedDict]` (now
    // expanded into kw-only params) still requires the source callable to supply
    // `**kwargs` — either a real `**kwargs` or its own `Unpack[TypedDict]`. A source
    // with only fixed parameters cannot guarantee extra keys are rejected, so the
    // assignment is disallowed (typing spec: destination `**kwargs: Unpack[TD]` with
    // a source lacking `**kwargs`). [callables_protocol_2]
    if target.had_unpack_kwargs && !func.has_kwargs && !func.had_unpack_kwargs {
        ctx.push_err(format!(
            "Function `{}` incompatible with `{}`: missing `**kwargs`",
            func.name, proto.name
        ));
        return true;
    }
    false
}

/// Check positional parameter count compatibility.
///
/// Returns `true` if a fatal mismatch was found.
fn check_protocol_param_counts(ctx: &mut ProtoCheckCtx<'_>) -> bool {
    let (target, func, proto) = (ctx.target, ctx.func, ctx.proto);
    let src_req = func
        .positional_params
        .iter()
        .filter(|p| !p.has_default)
        .count();
    // When the protocol has only keyword-only params (no positional), source positional
    // params that match by name are acceptable (they can be called as keywords).
    let src_excess_positional =
        if target.positional_params.is_empty() && !target.kw_only_params.is_empty() {
            func.positional_params
                .iter()
                .filter(|p| {
                    !p.has_default
                        && !p.is_positional_only
                        && !target.kw_only_params.iter().any(|tk| tk.name == p.name)
                })
                .count()
        } else {
            src_req.saturating_sub(target.positional_params.len())
        };
    if src_excess_positional > 0 && !target.has_varargs {
        ctx.push_err(format!(
            "Function `{}` incompatible with `{}`: too many required params",
            func.name, proto.name
        ));
        return true;
    }
    let tgt_req = target
        .positional_params
        .iter()
        .filter(|p| !p.has_default)
        .count();
    if tgt_req > func.positional_params.len() && !func.has_varargs {
        ctx.push_err(format!(
            "Function `{}` incompatible with `{}`: missing required params",
            func.name, proto.name
        ));
        return true;
    }
    false
}

/// Check default-argument requirements, keyword-only params, and positional-only
/// mismatches.
fn check_protocol_defaults_and_kw(ctx: &mut ProtoCheckCtx<'_>) {
    check_positional_defaults(ctx);
    check_kw_only_presence_and_defaults(ctx);
    check_source_required_kw(ctx);
    check_positional_only_mismatch(ctx);
}

/// Check positional parameter default requirements.
fn check_positional_defaults(ctx: &mut ProtoCheckCtx<'_>) {
    let (target, func, proto) = (ctx.target, ctx.func, ctx.proto);
    for (idx, tp) in target.positional_params.iter().enumerate() {
        if tp.has_default {
            if let Some(sp) = func.positional_params.get(idx) {
                if !sp.has_default && !func.has_varargs {
                    ctx.push_err(format!(
                        "Function `{}` incompatible with `{}`: param `{}` needs default",
                        func.name, proto.name, sp.name
                    ));
                }
            }
        }
    }
}

/// Check keyword-only param presence and defaults against the protocol.
fn check_kw_only_presence_and_defaults(ctx: &mut ProtoCheckCtx<'_>) {
    let (target, func, proto) = (ctx.target, ctx.func, ctx.proto);
    for tkw in &target.kw_only_params {
        let matching_kw = func.kw_only_params.iter().find(|sk| sk.name == tkw.name);
        let matching_reg = func
            .positional_params
            .iter()
            .find(|sp| sp.name == tkw.name && !sp.is_positional_only);
        if matching_kw.is_none() && matching_reg.is_none() && !func.has_kwargs {
            ctx.push_err(format!(
                "Function `{}` incompatible with `{}`: missing keyword param `{}`",
                func.name, proto.name, tkw.name
            ));
            continue;
        }
        if tkw.has_default {
            let source_has_default = matching_kw.is_some_and(|p| p.has_default)
                || matching_reg.is_some_and(|p| p.has_default);
            if !source_has_default && !func.has_kwargs {
                ctx.push_err(format!(
                    "Function `{}` incompatible with `{}`: keyword param `{}` needs default",
                    func.name, proto.name, tkw.name
                ));
            }
        }
    }
}

/// Check source required kw-only params not present in the target protocol.
fn check_source_required_kw(ctx: &mut ProtoCheckCtx<'_>) {
    let (target, func, proto) = (ctx.target, ctx.func, ctx.proto);
    for skw in &func.kw_only_params {
        if skw.has_default {
            continue;
        }
        let in_target_kw = target.kw_only_params.iter().any(|tk| tk.name == skw.name);
        let in_target_pos = target
            .positional_params
            .iter()
            .any(|tp| tp.name == skw.name);
        if !in_target_kw && !in_target_pos && !target.has_kwargs {
            ctx.push_err(format!(
                "Function `{}` incompatible with `{}`: requires keyword `{}` not in protocol",
                func.name, proto.name, skw.name
            ));
        }
    }
}

/// Check positional-only parameter mismatches.
fn check_positional_only_mismatch(ctx: &mut ProtoCheckCtx<'_>) {
    let (target, func, proto) = (ctx.target, ctx.func, ctx.proto);
    for (idx, tp) in target.positional_params.iter().enumerate() {
        if !tp.is_positional_only {
            if let Some(sp) = func.positional_params.get(idx) {
                if sp.is_positional_only {
                    ctx.push_err(format!(
                        "Function `{}` incompatible with `{}`: param `{}` is pos-only but must accept keyword",
                        func.name, proto.name, sp.name
                    ));
                }
            }
        }
    }
}

/// Check parameter type compatibility (contravariant), `*args` type, and
/// `**kwargs` type.
fn check_protocol_param_types(ctx: &mut ProtoCheckCtx<'_>) {
    let (target, func, proto) = (ctx.target, ctx.func, ctx.proto);
    // Param type compat (contravariant)
    for (idx, tp) in target.positional_params.iter().enumerate() {
        if let Some(sp) = func.positional_params.get(idx) {
            if !tp.type_annotation.is_empty()
                && !sp.type_annotation.is_empty()
                && !types_compat(&tp.type_annotation, &sp.type_annotation)
            {
                ctx.push_err(format!(
                    "Function `{}` incompatible with `{}`: param `{}` type `{}` vs `{}`",
                    func.name, proto.name, sp.name, sp.type_annotation, tp.type_annotation
                ));
            }
        }
    }
    // Keyword-only param type compat
    for tkw in &target.kw_only_params {
        let source_param = func
            .kw_only_params
            .iter()
            .find(|sk| sk.name == tkw.name)
            .or_else(|| {
                func.positional_params
                    .iter()
                    .find(|sp| sp.name == tkw.name && !sp.is_positional_only)
            });
        if let Some(sp) = source_param {
            if !tkw.type_annotation.is_empty()
                && !sp.type_annotation.is_empty()
                && !types_compat(&tkw.type_annotation, &sp.type_annotation)
            {
                ctx.push_err(format!(
                    "Function `{}` incompatible with `{}`: keyword param `{}` type `{}` vs `{}`",
                    func.name, proto.name, sp.name, sp.type_annotation, tkw.type_annotation
                ));
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
        ctx.push_err(format!(
            "Function `{}` incompatible with `{}`: *args type `{}` vs `{}`",
            func.name, proto.name, func.varargs_type, target.varargs_type
        ));
    }
    // **kwargs type compat
    if target.has_kwargs
        && func.has_kwargs
        && !target.kwargs_type.is_empty()
        && !func.kwargs_type.is_empty()
        && !types_compat(&target.kwargs_type, &func.kwargs_type)
    {
        ctx.push_err(format!(
            "Function `{}` incompatible with `{}`: **kwargs type `{}` vs `{}`",
            func.name, proto.name, func.kwargs_type, target.kwargs_type
        ));
    }
}
