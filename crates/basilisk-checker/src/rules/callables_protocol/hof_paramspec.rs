//! Implements [`callables_protocol`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Higher-order `ParamSpec` argument validation (PEP 612).
//! Implements [TYPEINF-GENERICS-PARAMSPEC].
//!
//! A function parameter annotated `Callable[Concatenate[T1, ..., P], R]`
//! requires arguments (including decorator applications) whose leading
//! positional parameters accept `T1, ...`.  When several parameters share one
//! `ParamSpec` (`def f(x: Callable[P, int], y: Callable[P, int])`), the
//! argument callables must have identical signatures.

use std::collections::{HashMap, HashSet};

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged as _;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic};
use crate::rules::shared::{ann_str, is_type_compatible, StarParam};

use super::CODE;

/// A function parameter that binds a `ParamSpec`.
struct PBindParam {
    position: usize,
    /// `Concatenate` prefix types (empty for bare `Callable[P, R]`).
    prefix: Vec<String>,
    /// The bound `ParamSpec` name.
    paramspec: String,
}

/// The comparable surface of a local function's signature.
#[derive(PartialEq, Eq)]
struct FnSignature {
    posonly: Vec<(String, Option<String>)>,
    standard: Vec<(String, Option<String>)>,
    kwonly: Vec<(String, Option<String>)>,
    vararg: StarParam,
    kwarg: StarParam,
}

impl FnSignature {
    /// Leading positional parameter annotations (positional-only + standard).
    fn positional_annotations(&self) -> impl Iterator<Item = Option<&str>> {
        self.posonly
            .iter()
            .chain(self.standard.iter())
            .map(|(_, ann)| ann.as_deref())
    }

    fn positional_count(&self) -> usize {
        self.posonly.len() + self.standard.len()
    }
}

/// Entry point.
pub(super) fn check_hof_paramspec_args(
    module: &ResolvedModule,
    stmts: &[Stmt],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let paramspec_names: HashSet<&str> =
        basilisk_resolver::collect_name_set_where(&module.typevar_calls, |tv| tv.is_paramspec);
    if paramspec_names.is_empty() {
        return;
    }

    let mut hofs: HashMap<&str, Vec<PBindParam>> = HashMap::new();
    let mut returns: HashMap<&str, (Vec<String>, String)> = HashMap::new();
    let mut signatures: HashMap<&str, FnSignature> = HashMap::new();
    for stmt in stmts {
        let Stmt::FunctionDef(func) = stmt else {
            continue;
        };
        let _ = signatures.insert(func.name.as_str(), fn_signature(func));
        let binds = collect_pbind_params(func, &paramspec_names);
        if !binds.is_empty() {
            let _ = hofs.insert(func.name.as_str(), binds);
        }
        if let Some(ret_bind) = func
            .returns
            .as_deref()
            .and_then(|ret| parse_callable_pbind(ret, &paramspec_names))
        {
            let _ = returns.insert(func.name.as_str(), ret_bind);
        }
    }
    if hofs.is_empty() {
        return;
    }

    let derived = derive_hof_results(stmts, &hofs, &returns, &signatures);
    check_derived_calls(stmts, &derived, &module.path, diagnostics);

    // Direct calls: `hof(fn_name, ...)`.
    basilisk_resolver::walk_all_stmts(stmts, &mut |stmt| {
        scan_stmt_calls(stmt, &hofs, &signatures, &module.path, diagnostics);
    });

    // Decorator applications: `@hof` above a function definition.
    for stmt in stmts {
        let Stmt::FunctionDef(func) = stmt else {
            continue;
        };
        for dec in &func.decorator_list {
            let Expr::Name(dec_name) = &dec.expression else {
                continue;
            };
            let Some(binds) = hofs.get(dec_name.id.as_str()) else {
                continue;
            };
            let Some(bind) = binds.first() else {
                continue;
            };
            let Some(sig) = signatures.get(func.name.as_str()) else {
                continue;
            };
            if let Some(problem) = prefix_mismatch(sig, &bind.prefix) {
                let range = dec.range();
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "`{}` is not compatible with the `Callable[Concatenate[...], ...]` \
                         parameter of decorator `{}`: {problem}",
                        func.name, dec_name.id
                    ),
                    Span {
                        start: range.start().to_u32(),
                        end: range.end().to_u32(),
                    },
                    &module.path,
                    Some(
                        "The decorated function must accept the Concatenate prefix as its \
                         leading positional parameters"
                            .to_owned(),
                    ),
                    None,
                ));
            }
        }
    }
}

/// Parameters of `func` that bind a `ParamSpec` through a `Callable` annotation.
fn collect_pbind_params(
    func: &ruff_python_ast::StmtFunctionDef,
    paramspec_names: &HashSet<&str>,
) -> Vec<PBindParam> {
    let params = &func.parameters;
    params
        .posonlyargs
        .iter()
        .chain(params.args.iter())
        .enumerate()
        .filter_map(|(position, pwd)| {
            let ann = pwd.parameter.annotation.as_deref()?;
            let (prefix, paramspec) = parse_callable_pbind(ann, paramspec_names)?;
            Some(PBindParam {
                position,
                prefix,
                paramspec,
            })
        })
        .collect()
}

/// Parse `Callable[P, R]` or `Callable[Concatenate[T.., P], R]`.
pub(super) fn parse_callable_pbind(
    ann: &Expr,
    paramspec_names: &HashSet<&str>,
) -> Option<(Vec<String>, String)> {
    let Expr::Subscript(sub) = ann else {
        return None;
    };
    if ann_str(&sub.value) != "Callable" {
        return None;
    }
    let Expr::Tuple(tup) = sub.slice.as_ref() else {
        return None;
    };
    let [params_part, _ret] = tup.elts.as_slice() else {
        return None;
    };
    match params_part {
        Expr::Name(n) if paramspec_names.contains(n.id.as_str()) => {
            Some((Vec::new(), n.id.to_string()))
        }
        Expr::Subscript(concat) if ann_str(&concat.value) == "Concatenate" => {
            let Expr::Tuple(args) = concat.slice.as_ref() else {
                return None;
            };
            let (last, prefix_elts) = args.elts.split_last()?;
            let Expr::Name(ps) = last else {
                return None;
            };
            if !paramspec_names.contains(ps.id.as_str()) {
                return None;
            }
            let prefix = prefix_elts.iter().map(ann_str).collect();
            Some((prefix, ps.id.to_string()))
        }
        _ => None,
    }
}

/// Extract the comparable signature of a function definition.
fn fn_signature(func: &ruff_python_ast::StmtFunctionDef) -> FnSignature {
    let params = &func.parameters;
    let pair = |pwd: &ruff_python_ast::ParameterWithDefault| {
        (
            pwd.parameter.name.to_string(),
            pwd.parameter.annotation.as_deref().map(ann_str),
        )
    };
    FnSignature {
        posonly: params.posonlyargs.iter().map(pair).collect(),
        standard: params.args.iter().map(pair).collect(),
        kwonly: params.kwonlyargs.iter().map(pair).collect(),
        vararg: params.vararg.as_deref().map_or(StarParam::Absent, |v| {
            StarParam::from_annotation(v.annotation.as_deref().map(ann_str))
        }),
        kwarg: params.kwarg.as_deref().map_or(StarParam::Absent, |k| {
            StarParam::from_annotation(k.annotation.as_deref().map(ann_str))
        }),
    }
}

/// Check one statement's expressions for calls to `ParamSpec`-binding HOFs.
fn scan_stmt_calls(
    stmt: &Stmt,
    hofs: &HashMap<&str, Vec<PBindParam>>,
    signatures: &HashMap<&str, FnSignature>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let value = match stmt {
        Stmt::Expr(node) => Some(node.value.as_ref()),
        Stmt::Assign(node) => Some(node.value.as_ref()),
        Stmt::AnnAssign(node) => node.value.as_deref(),
        Stmt::Return(node) => node.value.as_deref(),
        _ => None,
    };
    if let Some(expr) = value {
        scan_expr_calls(expr, hofs, signatures, path, diagnostics);
    }
}

/// Recursively check call expressions.
fn scan_expr_calls(
    expr: &Expr,
    hofs: &HashMap<&str, Vec<PBindParam>>,
    signatures: &HashMap<&str, FnSignature>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Expr::Call(call) = expr else { return };
    for arg in &call.arguments.args {
        scan_expr_calls(arg, hofs, signatures, path, diagnostics);
    }
    let Expr::Name(callee) = call.func.as_ref() else {
        return;
    };
    let Some(binds) = hofs.get(callee.id.as_str()) else {
        return;
    };
    check_call(
        call,
        callee.id.as_str(),
        binds,
        signatures,
        path,
        diagnostics,
    );
}

/// Validate one call's function-name arguments against the HOF's bindings.
fn check_call(
    call: &ruff_python_ast::ExprCall,
    callee: &str,
    binds: &[PBindParam],
    signatures: &HashMap<&str, FnSignature>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let arg_sig = |position: usize| -> Option<(&str, &FnSignature)> {
        let arg = call.arguments.args.get(position)?;
        let Expr::Name(name) = arg else { return None };
        signatures
            .get(name.id.as_str())
            .map(|sig| (name.id.as_str(), sig))
    };

    let mut problem: Option<String> = None;
    for bind in binds {
        if let Some((arg_name, sig)) = arg_sig(bind.position) {
            if let Some(mismatch) = prefix_mismatch(sig, &bind.prefix) {
                problem = Some(format!("`{arg_name}`: {mismatch}"));
                break;
            }
        }
    }

    // Shared ParamSpec: all bound arguments must have identical signatures.
    if problem.is_none() {
        let mut by_paramspec: HashMap<&str, Vec<&FnSignature>> = HashMap::new();
        let mut names: HashMap<&str, Vec<&str>> = HashMap::new();
        for bind in binds {
            if let Some((arg_name, sig)) = arg_sig(bind.position) {
                by_paramspec
                    .entry(bind.paramspec.as_str())
                    .or_default()
                    .push(sig);
                names
                    .entry(bind.paramspec.as_str())
                    .or_default()
                    .push(arg_name);
            }
        }
        for (paramspec, sigs) in &by_paramspec {
            let mismatched = sigs.windows(2).any(|pair| match pair {
                [first, second] => first != second,
                _ => false,
            });
            if mismatched {
                problem = Some(format!(
                    "arguments {} must have identical signatures to bind `{paramspec}`",
                    names
                        .get(paramspec)
                        .map(|n| n.join(", "))
                        .unwrap_or_default()
                ));
                break;
            }
        }
    }

    let Some(problem) = problem else { return };
    let range = call.range();
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        format!("Incompatible ParamSpec argument(s) to `{callee}`: {problem}"),
        Span {
            start: range.start().to_u32(),
            end: range.end().to_u32(),
        },
        path,
        Some(
            "Arguments binding a ParamSpec must satisfy the Concatenate prefix and bind \
             the ParamSpec consistently"
                .to_owned(),
        ),
        None,
    ));
}

/// A problem description when `sig` cannot accept the `Concatenate` prefix as
/// leading positional arguments; `None` when compatible.
fn prefix_mismatch(sig: &FnSignature, prefix: &[String]) -> Option<String> {
    if prefix.is_empty() {
        return None;
    }
    let available = sig.positional_count() + usize::from(sig.vararg.is_present());
    if available < prefix.len() && !sig.vararg.is_present() {
        return Some(format!(
            "expected at least {} leading positional parameter{}, found {}",
            prefix.len(),
            if prefix.len() == 1 { "" } else { "s" },
            sig.positional_count()
        ));
    }
    for (expected, actual) in prefix.iter().zip(sig.positional_annotations()) {
        let Some(actual) = actual else { continue };
        if !is_type_compatible(expected, actual) {
            return Some(format!(
                "leading parameter is `{actual}`, but the Concatenate prefix supplies \
                 `{expected}`"
            ));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Derived signatures of HOF results (`f1 = changes_return_type(returns_int)`)
// ---------------------------------------------------------------------------

/// For module-level `v = hof(g)` assignments where the HOF takes
/// `Callable[Concatenate[pre.., P], _]` and returns
/// `Callable[Concatenate[ret_pre.., P], _]`, compute `v`'s signature:
/// the return prefix as positional-only parameters, followed by `g`'s
/// parameters minus the consumed argument prefix.
fn derive_hof_results(
    stmts: &[Stmt],
    hofs: &HashMap<&str, Vec<PBindParam>>,
    returns: &HashMap<&str, (Vec<String>, String)>,
    signatures: &HashMap<&str, FnSignature>,
) -> HashMap<String, FnSignature> {
    let mut derived = HashMap::new();
    for stmt in stmts {
        let Stmt::Assign(assign) = stmt else { continue };
        let [Expr::Name(target)] = assign.targets.as_slice() else {
            continue;
        };
        let Expr::Call(call) = assign.value.as_ref() else {
            continue;
        };
        let Expr::Name(hof_name) = call.func.as_ref() else {
            continue;
        };
        let (Some(binds), Some((ret_prefix, ret_ps))) = (
            hofs.get(hof_name.id.as_str()),
            returns.get(hof_name.id.as_str()),
        ) else {
            continue;
        };
        let Some(bind) = binds.iter().find(|b| &b.paramspec == ret_ps) else {
            continue;
        };
        let Some(Expr::Name(arg_fn)) = call.arguments.args.get(bind.position) else {
            continue;
        };
        let Some(arg_sig) = signatures.get(arg_fn.id.as_str()) else {
            continue;
        };
        if let Some(sig) = derive_signature(arg_sig, bind.prefix.len(), ret_prefix) {
            let _ = derived.insert(target.id.to_string(), sig);
        }
    }
    derived
}

/// Drop `consumed` leading positionals from `base`, then prepend the return
/// prefix as anonymous positional-only parameters.
fn derive_signature(
    base: &FnSignature,
    consumed: usize,
    ret_prefix: &[String],
) -> Option<FnSignature> {
    if base.positional_count() < consumed {
        return None;
    }
    let mut remaining: Vec<(bool, (String, Option<String>))> = base
        .posonly
        .iter()
        .map(|p| (true, p.clone()))
        .chain(base.standard.iter().map(|p| (false, p.clone())))
        .collect();
    let leftover = remaining.split_off(consumed);

    let mut posonly: Vec<(String, Option<String>)> = ret_prefix
        .iter()
        .map(|ty| (String::new(), Some(ty.clone())))
        .collect();
    let mut standard = Vec::new();
    for (was_posonly, param) in leftover {
        if was_posonly {
            posonly.push(param);
        } else {
            standard.push(param);
        }
    }
    Some(FnSignature {
        posonly,
        standard,
        kwonly: base.kwonly.clone(),
        vararg: base.vararg.clone(),
        kwarg: base.kwarg.clone(),
    })
}

/// Validate calls to variables holding derived HOF results.
fn check_derived_calls(
    stmts: &[Stmt],
    derived: &HashMap<String, FnSignature>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if derived.is_empty() {
        return;
    }
    basilisk_resolver::walk_all_stmts(stmts, &mut |stmt| {
        let value = match stmt {
            Stmt::Expr(node) => Some(node.value.as_ref()),
            Stmt::Assign(node) => Some(node.value.as_ref()),
            Stmt::AnnAssign(node) => node.value.as_deref(),
            Stmt::Return(node) => node.value.as_deref(),
            _ => None,
        };
        let Some(Expr::Call(call)) = value else {
            return;
        };
        let Expr::Name(callee) = call.func.as_ref() else {
            return;
        };
        let Some(sig) = derived.get(callee.id.as_str()) else {
            return;
        };
        if let Some(problem) = derived_call_problem(call, sig) {
            let range = call.range();
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!("Invalid call to `{}`: {problem}", callee.id),
                Span {
                    start: range.start().to_u32(),
                    end: range.end().to_u32(),
                },
                path,
                Some(
                    "The callable's signature is determined by the ParamSpec binding of \
                     the higher-order function result"
                        .to_owned(),
                ),
                None,
            ));
        }
    });
}

/// A problem description for a call against a derived signature, if any.
fn derived_call_problem(call: &ruff_python_ast::ExprCall, sig: &FnSignature) -> Option<String> {
    // Keyword arguments must not name positional-only parameters.
    for kw in &call.arguments.keywords {
        let Some(kw_name) = kw.arg.as_ref() else {
            continue;
        };
        if sig
            .posonly
            .iter()
            .any(|(name, _)| !name.is_empty() && name == kw_name.as_str())
        {
            return Some(format!(
                "parameter `{kw_name}` is positional-only and cannot be passed by keyword"
            ));
        }
    }
    // Positional literal arguments must match parameter annotations
    // (overflow positions check against `*args`).
    let annotations: Vec<Option<&str>> = sig.positional_annotations().collect();
    for (idx, arg) in call.arguments.args.iter().enumerate() {
        let Some(actual) = crate::rules::shared::infer_expr_literal_type(arg) else {
            continue;
        };
        let expected = annotations
            .get(idx)
            .copied()
            .flatten()
            .or_else(|| sig.vararg.ty());
        let Some(expected) = expected else { continue };
        if !is_type_compatible(actual, expected) {
            return Some(format!(
                "argument {} has type `{actual}`, expected `{expected}`",
                idx + 1
            ));
        }
    }
    None
}
