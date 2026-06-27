//! Implements [`callables_protocol`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! `ParamSpec` component rules (PEP 612): `P.args` / `P.kwargs` placement,
//! scoping, and transmission through `*args` / `**kwargs` forwarding calls.

use std::collections::{HashMap, HashSet};

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged as _;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic};
use crate::rules::shared::{ann_str, infer_expr_literal_type, is_type_compatible};

use super::hof_paramspec::parse_callable_pbind;
use super::CODE;

struct Ctx<'a> {
    paramspecs: HashSet<&'a str>,
    path: &'a str,
}

/// Callees visible from a components-function body: the enclosing function's
/// `Callable`-typed parameters and its nested function definitions, each with
/// the positional prefix expected before forwarded `*args`.
#[derive(Default)]
struct CalleeScope {
    /// Callee name → names of positional parameters before the `ParamSpec`.
    prefixes: HashMap<String, Vec<String>>,
}

/// Entry point.
pub(super) fn check_paramspec_components(
    module: &ResolvedModule,
    stmts: &[Stmt],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let paramspecs: HashSet<&str> =
        basilisk_resolver::collect_name_set_where(&module.typevar_calls, |tv| tv.is_paramspec);
    if paramspecs.is_empty() {
        return;
    }
    let ctx = Ctx {
        paramspecs,
        path: &module.path,
    };
    check_direct_forwarding_calls(stmts, &ctx, diagnostics);
    walk(
        stmts,
        &HashSet::new(),
        &CalleeScope::default(),
        &ctx,
        diagnostics,
    );
}

/// Component annotation classification: `P.args` / `P.kwargs`.
fn component_of<'a>(ann: &'a Expr, ctx: &Ctx<'_>) -> Option<(&'a str, &'static str)> {
    let Expr::Attribute(attr) = ann else {
        return None;
    };
    let Expr::Name(base) = attr.value.as_ref() else {
        return None;
    };
    if !ctx.paramspecs.contains(base.id.as_str()) {
        return None;
    }
    match attr.attr.as_str() {
        "args" => Some((base.id.as_str(), "args")),
        "kwargs" => Some((base.id.as_str(), "kwargs")),
        _ => None,
    }
}

/// Recursive scope walk validating component usage.
fn walk(
    stmts: &[Stmt],
    bound: &HashSet<String>,
    enclosing: &CalleeScope,
    ctx: &Ctx<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::FunctionDef(func) => {
                let inner_bound = extend_bound(func, bound, ctx);
                validate_signature(func, &inner_bound, ctx, diagnostics);
                if let Some((args_name, kwargs_name)) = forwarding_names(func, ctx) {
                    validate_forwarding_calls(
                        &func.body,
                        &args_name,
                        &kwargs_name,
                        enclosing,
                        ctx,
                        diagnostics,
                    );
                }
                let inner_scope = callee_scope(func, ctx);
                walk(&func.body, &inner_bound, &inner_scope, ctx, diagnostics);
            }
            Stmt::ClassDef(cls) => {
                // A class generic over a ParamSpec (`class C(Protocol[P])`)
                // binds it for all of its methods.
                let mut class_bound = bound.clone();
                class_bound.extend(
                    crate::rules::shared::class_generic_param_names(cls)
                        .into_iter()
                        .filter(|name| ctx.paramspecs.contains(name.as_str())),
                );
                walk(&cls.body, &class_bound, enclosing, ctx, diagnostics);
            }
            Stmt::AnnAssign(ann) if component_of(&ann.annotation, ctx).is_some() => {
                push(
                    diagnostics,
                    ctx,
                    ann.range(),
                    "ParamSpec components are only valid as `*args: P.args` and \
                     `**kwargs: P.kwargs` annotations",
                );
            }
            _ => {}
        }
    }
}

/// `ParamSpec`s bound by this function's `Callable`-annotated parameters.
fn extend_bound(
    func: &ruff_python_ast::StmtFunctionDef,
    bound: &HashSet<String>,
    ctx: &Ctx<'_>,
) -> HashSet<String> {
    let mut inner = bound.clone();
    for pwd in all_named_params(func) {
        if let Some(ann) = pwd.parameter.annotation.as_deref() {
            if let Some((_, ps)) = parse_callable_pbind(ann, &ctx.paramspecs) {
                let _ = inner.insert(ps);
            }
        }
    }
    inner
}

/// The callee scope a child function body sees: this function's
/// `Callable`-typed parameters plus its directly nested functions.
fn callee_scope(func: &ruff_python_ast::StmtFunctionDef, ctx: &Ctx<'_>) -> CalleeScope {
    let mut scope = CalleeScope::default();
    for pwd in all_named_params(func) {
        if let Some(ann) = pwd.parameter.annotation.as_deref() {
            if let Some((prefix, _)) = parse_callable_pbind(ann, &ctx.paramspecs) {
                // `Callable` prefixes are anonymous positional-only slots.
                let names = prefix.iter().map(|_| String::new()).collect();
                let _ = scope.prefixes.insert(pwd.parameter.name.to_string(), names);
            }
        }
    }
    for stmt in &func.body {
        let Stmt::FunctionDef(nested) = stmt else {
            continue;
        };
        if forwarding_names(nested, ctx).is_none() {
            continue;
        }
        let prefix = all_named_params(nested)
            .map(|pwd| pwd.parameter.name.to_string())
            .collect();
        let _ = scope.prefixes.insert(nested.name.to_string(), prefix);
    }
    scope
}

/// Iterator over positional-only + standard parameters.
fn all_named_params(
    func: &ruff_python_ast::StmtFunctionDef,
) -> impl Iterator<Item = &ruff_python_ast::ParameterWithDefault> {
    func.parameters
        .posonlyargs
        .iter()
        .chain(func.parameters.args.iter())
}

/// The `(args_name, kwargs_name)` of a function forwarding a `ParamSpec`.
fn forwarding_names(
    func: &ruff_python_ast::StmtFunctionDef,
    ctx: &Ctx<'_>,
) -> Option<(String, String)> {
    let vararg = func.parameters.vararg.as_deref()?;
    let kwarg = func.parameters.kwarg.as_deref()?;
    let (_, va_kind) = component_of(vararg.annotation.as_deref()?, ctx)?;
    let (_, kw_kind) = component_of(kwarg.annotation.as_deref()?, ctx)?;
    (va_kind == "args" && kw_kind == "kwargs")
        .then(|| (vararg.name.to_string(), kwarg.name.to_string()))
}

/// Validate component placement within one function signature.
fn validate_signature(
    func: &ruff_python_ast::StmtFunctionDef,
    bound: &HashSet<String>,
    ctx: &Ctx<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let params = &func.parameters;

    for pwd in params
        .posonlyargs
        .iter()
        .chain(params.args.iter())
        .chain(params.kwonlyargs.iter())
    {
        if let Some(ann) = pwd.parameter.annotation.as_deref() {
            if component_of(ann, ctx).is_some() {
                push(
                    diagnostics,
                    ctx,
                    pwd.range(),
                    "ParamSpec components are only valid on `*args` and `**kwargs`",
                );
            }
        }
    }

    let vararg_component = params
        .vararg
        .as_deref()
        .and_then(|v| v.annotation.as_deref())
        .and_then(|ann| component_of(ann, ctx));
    let kwarg_component = params
        .kwarg
        .as_deref()
        .and_then(|k| k.annotation.as_deref())
        .and_then(|ann| component_of(ann, ctx));

    let misplaced_star = matches!(vararg_component, Some((_, kind)) if kind != "args")
        || matches!(kwarg_component, Some((_, kind)) if kind != "kwargs");
    if misplaced_star {
        push(
            diagnostics,
            ctx,
            params.range(),
            "`*args` must be annotated `P.args` and `**kwargs` must be annotated \
             `P.kwargs`",
        );
        return;
    }

    match (vararg_component, kwarg_component) {
        (Some((ps_args, _)), Some((ps_kwargs, _))) => {
            if ps_args != ps_kwargs {
                push(
                    diagnostics,
                    ctx,
                    params.range(),
                    "`P.args` and `P.kwargs` must use the same ParamSpec",
                );
            } else if !bound.contains(ps_args) {
                push(
                    diagnostics,
                    ctx,
                    params.range(),
                    "ParamSpec components used out of scope: no enclosing binding of the \
                     ParamSpec through a `Callable` parameter",
                );
            } else if !params.kwonlyargs.is_empty() {
                push(
                    diagnostics,
                    ctx,
                    params.range(),
                    "no keyword-only parameters may appear between `*args: P.args` and \
                     `**kwargs: P.kwargs`",
                );
            }
        }
        (Some(_), None) => push(
            diagnostics,
            ctx,
            params.range(),
            "`*args: P.args` requires a matching `**kwargs: P.kwargs`",
        ),
        (None, Some(_)) => push(
            diagnostics,
            ctx,
            params.range(),
            "`**kwargs: P.kwargs` requires a matching `*args: P.args`",
        ),
        (None, None) => {}
    }
}

/// Validate `callee(prefix.., *args, **kwargs)` forwarding calls inside a
/// components-function body.
fn validate_forwarding_calls(
    stmts: &[Stmt],
    args_name: &str,
    kwargs_name: &str,
    enclosing: &CalleeScope,
    ctx: &Ctx<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    basilisk_resolver::walk_function_stmts(stmts, &mut |stmt| {
        let value = match stmt {
            Stmt::Expr(node) => Some(node.value.as_ref()),
            Stmt::Assign(node) => Some(node.value.as_ref()),
            Stmt::Return(node) => node.value.as_deref(),
            _ => None,
        };
        if let Some(expr) = value {
            scan_forwarding_expr(expr, args_name, kwargs_name, enclosing, ctx, diagnostics);
        }
    });
}

/// Recursively find and validate forwarding calls in an expression.
fn scan_forwarding_expr(
    expr: &Expr,
    args_name: &str,
    kwargs_name: &str,
    enclosing: &CalleeScope,
    ctx: &Ctx<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Expr::Call(call) = expr else { return };
    for arg in &call.arguments.args {
        scan_forwarding_expr(arg, args_name, kwargs_name, enclosing, ctx, diagnostics);
    }
    let Expr::Name(callee) = call.func.as_ref() else {
        return;
    };
    let Some(prefix) = enclosing.prefixes.get(callee.id.as_str()) else {
        return;
    };
    let star_idx = call
        .arguments
        .args
        .iter()
        .position(|a| matches!(a, Expr::Starred(_)));
    let Some(star_idx) = star_idx else { return };

    if let Some(problem) = forwarding_problem(call, star_idx, prefix, args_name, kwargs_name) {
        push(diagnostics, ctx, call.range(), &problem);
    }
}

/// A problem description for one forwarding call, if any.
fn forwarding_problem(
    call: &ruff_python_ast::ExprCall,
    star_idx: usize,
    prefix: &[String],
    args_name: &str,
    kwargs_name: &str,
) -> Option<String> {
    // The starred argument must forward `*args`, and `**` must forward kwargs.
    if let Some(Expr::Starred(starred)) = call.arguments.args.get(star_idx) {
        if let Expr::Name(name) = starred.value.as_ref() {
            if name.id.as_str() == kwargs_name {
                return Some("`*` must forward the `P.args` parameter, not `P.kwargs`".to_owned());
            }
            if name.id.as_str() != args_name {
                return None;
            }
        }
    }
    for kw in &call.arguments.keywords {
        if kw.arg.is_none() {
            if let Expr::Name(name) = &kw.value {
                if name.id.as_str() == args_name {
                    return Some(
                        "`**` must forward the `P.kwargs` parameter, not `P.args`".to_owned(),
                    );
                }
            }
        }
        if let Some(kw_name) = kw.arg.as_ref() {
            if prefix.iter().any(|p| p == kw_name.as_str()) {
                return Some(format!(
                    "prefix parameter `{kw_name}` must be passed positionally before \
                     the forwarded `*args`"
                ));
            }
        }
    }
    if star_idx != prefix.len() {
        return Some(format!(
            "expected {} positional prefix argument{} before the forwarded `*args`, \
             found {star_idx}",
            prefix.len(),
            if prefix.len() == 1 { "" } else { "s" },
        ));
    }
    if call.arguments.args.len() > star_idx + 1 {
        return Some("no positional arguments may follow the forwarded `*args`".to_owned());
    }
    None
}

/// Format and push a diagnostic.
fn push(
    diagnostics: &mut Vec<Diagnostic>,
    ctx: &Ctx<'_>,
    range: ruff_text_size::TextRange,
    message: &str,
) {
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        format!("Invalid ParamSpec usage: {message}"),
        Span {
            start: range.start().to_u32(),
            end: range.end().to_u32(),
        },
        ctx.path,
        Some("See PEP 612: the components of a ParamSpec".to_owned()),
        None,
    ));
}

/// `twice(a_int_b_str, 1, "A")` — a direct call to a function that takes a
/// `Callable[P, X]` plus `*args: P.args, **kwargs: P.kwargs` binds `P` to the
/// argument function's parameters; the remaining arguments must match them.
fn check_direct_forwarding_calls(stmts: &[Stmt], ctx: &Ctx<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let mut module_fns: HashMap<&str, &ruff_python_ast::StmtFunctionDef> = HashMap::new();
    for stmt in stmts {
        if let Stmt::FunctionDef(func) = stmt {
            let _ = module_fns.insert(func.name.as_str(), func);
        }
    }
    let forwarders: HashSet<&str> = module_fns
        .iter()
        .filter(|(_, func)| {
            let first_is_pbind = all_named_params(func)
                .next()
                .and_then(|pwd| pwd.parameter.annotation.as_deref())
                .and_then(|ann| parse_callable_pbind(ann, &ctx.paramspecs))
                .is_some_and(|(prefix, _)| prefix.is_empty());
            first_is_pbind && forwarding_names(func, ctx).is_some()
        })
        .map(|(name, _)| *name)
        .collect();
    if forwarders.is_empty() {
        return;
    }

    basilisk_resolver::walk_all_stmts(stmts, &mut |stmt| {
        let value = match stmt {
            Stmt::Expr(node) => Some(node.value.as_ref()),
            Stmt::Assign(node) => Some(node.value.as_ref()),
            _ => None,
        };
        let Some(Expr::Call(call)) = value else {
            return;
        };
        let Expr::Name(callee) = call.func.as_ref() else {
            return;
        };
        if !forwarders.contains(callee.id.as_str()) {
            return;
        }
        let Some(Expr::Name(arg_fn)) = call.arguments.args.first() else {
            return;
        };
        let Some(target) = module_fns.get(arg_fn.id.as_str()) else {
            return;
        };
        if let Some(problem) = direct_call_problem(call, target, arg_fn.id.as_str()) {
            push(diagnostics, ctx, call.range(), &problem);
        }
    });
}

/// Validate forwarded literal arguments against the target's parameters.
fn direct_call_problem(
    call: &ruff_python_ast::ExprCall,
    target: &ruff_python_ast::StmtFunctionDef,
    target_name: &str,
) -> Option<String> {
    let positional: Vec<(String, Option<String>)> = all_named_params(target)
        .map(|pwd| {
            (
                pwd.parameter.name.to_string(),
                pwd.parameter.annotation.as_deref().map(ann_str),
            )
        })
        .collect();

    for (idx, arg) in call.arguments.args.iter().skip(1).enumerate() {
        let Some(actual) = infer_expr_literal_type(arg) else {
            continue;
        };
        let Some((_, Some(expected))) = positional.get(idx) else {
            continue;
        };
        if !is_type_compatible(actual, expected) {
            return Some(format!(
                "forwarded argument {} has type `{actual}`, but `{target_name}` \
                 expects `{expected}`",
                idx + 1
            ));
        }
    }
    for kw in &call.arguments.keywords {
        let Some(kw_name) = kw.arg.as_ref() else {
            continue;
        };
        let Some(actual) = infer_expr_literal_type(&kw.value) else {
            continue;
        };
        let expected = positional
            .iter()
            .find(|(name, _)| name == kw_name.as_str())
            .and_then(|(_, ann)| ann.as_deref());
        let Some(expected) = expected else { continue };
        if !is_type_compatible(actual, expected) {
            return Some(format!(
                "forwarded keyword `{kw_name}` has type `{actual}`, but \
                 `{target_name}` expects `{expected}`",
            ));
        }
    }
    None
}
