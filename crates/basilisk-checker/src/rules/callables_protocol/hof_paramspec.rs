//! Implements [`callables_protocol`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Higher-order `ParamSpec` argument validation (PEP 612).
//! Implements [TYPEINF-GENERICS-PARAMSPEC].
//!
//! When several parameters of a function bind the same `ParamSpec`, the
//! argument callables must have identical signatures.
//!
//! Every verdict here is computed from resolved bindings and the
//! [`TypeNode`] relations ([ASTREBUILD-LAW]): the `Callable` and
//! `Concatenate` heads are recognised by what they resolve to, annotations
//! are related through [`assignable`]/[`equivalent`], and a diagnostic is
//! emitted only on a definite `Some(false)`. Source text appears in
//! diagnostic messages only.

use std::collections::{HashMap, HashSet};

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged as _;

use basilisk_resolver::{
    assignable, equivalent, BindingTable, ResolvedModule, Span, TypeNode, TypingForm,
};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic};
use crate::span_util::node_message_text;

use super::CODE;

/// A resolved annotation: the lowered node carries every verdict
/// ([ASTREBUILD-LAW]); the display text is for diagnostic messages only.
#[derive(Clone)]
pub(super) struct AnnInfo {
    /// The annotation lowered through the module's bindings.
    pub(super) node: TypeNode,
    /// Source rendering, used exclusively in diagnostic messages.
    pub(super) display: String,
}

impl AnnInfo {
    /// Lower `expr` through the module's bindings, keeping its source
    /// rendering for messages.
    pub(super) fn lower(bindings: &BindingTable, source: &str, expr: &Expr) -> Self {
        Self {
            node: TypeNode::lower(bindings, expr),
            display: node_message_text(source, expr).to_owned(),
        }
    }
}

/// A `*args`/`**kwargs` slot carrying the resolved annotation node — the
/// semantic replacement for the shared text-payload slot.
#[derive(Clone, Default)]
enum StarSlot {
    /// The signature has no such parameter.
    #[default]
    Absent,
    /// Present without an annotation (implicitly gradual).
    Untyped,
    /// Present with a resolved annotation.
    Typed(AnnInfo),
}

impl StarSlot {
    /// `true` when the parameter exists in the signature.
    fn is_present(&self) -> bool {
        !matches!(self, Self::Absent)
    }

    /// The resolved annotation; `None` for absent or untyped (gradual).
    fn ann(&self) -> Option<&AnnInfo> {
        match self {
            Self::Typed(ann) => Some(ann),
            Self::Absent | Self::Untyped => None,
        }
    }
}

/// A function parameter that binds a `ParamSpec`.
struct PBindParam {
    position: usize,
    /// Resolved prefix types required ahead of the bound `ParamSpec`
    /// (`Concatenate[prefix.., P]`).
    prefix: Vec<AnnInfo>,
    /// The bound `ParamSpec` name.
    paramspec: String,
}

/// The comparable surface of a local function's signature.
struct FnSignature {
    posonly: Vec<(String, Option<AnnInfo>)>,
    standard: Vec<(String, Option<AnnInfo>)>,
    kwonly: Vec<(String, Option<AnnInfo>)>,
    vararg: StarSlot,
    kwarg: StarSlot,
}

impl FnSignature {
    /// Leading positional parameter annotations (positional-only + standard).
    fn positional_annotations(&self) -> impl Iterator<Item = Option<&AnnInfo>> {
        self.posonly
            .iter()
            .chain(self.standard.iter())
            .map(|(_, ann)| ann.as_ref())
    }

    fn positional_count(&self) -> usize {
        self.posonly.len() + self.standard.len()
    }
}

/// `true` only when two signatures DEFINITELY differ: a structural
/// difference (arity, parameter names callable by keyword, star-parameter
/// presence) or an annotation pair the relation layer rejects
/// (`equivalent == Some(false)`). Unresolvable pairs abstain
/// ([RESOLV-CANONICAL-RELATION]) — a diagnostic may not come from a guess.
fn signatures_definitely_differ(a: &FnSignature, b: &FnSignature) -> bool {
    if a.posonly.len() != b.posonly.len()
        || a.standard.len() != b.standard.len()
        || a.kwonly.len() != b.kwonly.len()
        || a.vararg.is_present() != b.vararg.is_present()
        || a.kwarg.is_present() != b.kwarg.is_present()
    {
        return true;
    }
    let positional_pairs = a
        .posonly
        .iter()
        .chain(a.standard.iter())
        .zip(b.posonly.iter().chain(b.standard.iter()));
    for ((_, ann_a), (_, ann_b)) in positional_pairs {
        if anns_definitely_differ(ann_a.as_ref(), ann_b.as_ref()) {
            return true;
        }
    }
    // Standard and keyword-only parameters are callable by keyword, so their
    // names are part of the signature (PEP 612 binds them through `P`).
    let named = a
        .standard
        .iter()
        .chain(a.kwonly.iter())
        .zip(b.standard.iter().chain(b.kwonly.iter()));
    for ((name_a, _), (name_b, _)) in named {
        if name_a != name_b {
            return true;
        }
    }
    for ((_, ann_a), (_, ann_b)) in a.kwonly.iter().zip(b.kwonly.iter()) {
        if anns_definitely_differ(ann_a.as_ref(), ann_b.as_ref()) {
            return true;
        }
    }
    star_definitely_differs(&a.vararg, &b.vararg) || star_definitely_differs(&a.kwarg, &b.kwarg)
}

/// A definite mismatch between two optional annotations; missing annotations
/// are gradual and abstain.
fn anns_definitely_differ(a: Option<&AnnInfo>, b: Option<&AnnInfo>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => equivalent(&a.node, &b.node) == Some(false),
        _ => false,
    }
}

/// A definite mismatch between two star slots of equal presence.
fn star_definitely_differs(a: &StarSlot, b: &StarSlot) -> bool {
    anns_definitely_differ(a.ann(), b.ann())
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
    let bindings: &BindingTable = &module.bindings;
    let source = module.source.as_str();

    let mut hofs: HashMap<&str, Vec<PBindParam>> = HashMap::new();
    let mut returns: HashMap<&str, (Vec<AnnInfo>, String)> = HashMap::new();
    let mut signatures: HashMap<&str, FnSignature> = HashMap::new();
    for stmt in stmts {
        let Stmt::FunctionDef(func) = stmt else {
            continue;
        };
        let _ = signatures.insert(func.name.as_str(), fn_signature(bindings, source, func));
        let binds = collect_pbind_params(bindings, source, func, &paramspec_names);
        if !binds.is_empty() {
            let _ = hofs.insert(func.name.as_str(), binds);
        }
        if let Some(ret_bind) = func
            .returns
            .as_deref()
            .and_then(|ret| parse_callable_pbind(bindings, source, ret, &paramspec_names))
        {
            let _ = returns.insert(func.name.as_str(), ret_bind);
        }
    }
    if hofs.is_empty() {
        return;
    }

    let derived = derive_hof_results(stmts, &hofs, &returns, &signatures);
    check_derived_calls(module, stmts, &derived, diagnostics);

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

/// Parameters of `func` that bind a `ParamSpec` through their annotation.
fn collect_pbind_params(
    bindings: &BindingTable,
    source: &str,
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
            let (prefix, paramspec) = parse_callable_pbind(bindings, source, ann, paramspec_names)?;
            Some(PBindParam {
                position,
                prefix,
                paramspec,
            })
        })
        .collect()
}

/// Parse a `Callable[P, R]` / `Callable[Concatenate[T1, .., P], R]`
/// annotation binding one of `paramspec_names`, yielding the resolved
/// `Concatenate` prefix and the bound `ParamSpec` name.
///
/// The `Callable` and `Concatenate` heads are recognised by what they
/// RESOLVE to through the binding table ([ASTREBUILD-LAW]) — an aliased
/// import is the same form; a shadowed name is not.
pub(super) fn parse_callable_pbind(
    bindings: &BindingTable,
    source: &str,
    ann: &Expr,
    paramspec_names: &HashSet<&str>,
) -> Option<(Vec<AnnInfo>, String)> {
    let Expr::Subscript(sub) = ann else {
        return None;
    };
    if bindings.form_of_with_builtins(&sub.value) != Some(TypingForm::Callable) {
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
        Expr::Subscript(concat)
            if bindings.form_of_with_builtins(&concat.value) == Some(TypingForm::Concatenate) =>
        {
            let Expr::Tuple(parts) = concat.slice.as_ref() else {
                return None;
            };
            let (last, prefix) = parts.elts.split_last()?;
            let Expr::Name(ps) = last else {
                return None;
            };
            if !paramspec_names.contains(ps.id.as_str()) {
                return None;
            }
            let prefix = prefix
                .iter()
                .map(|e| AnnInfo::lower(bindings, source, e))
                .collect();
            Some((prefix, ps.id.to_string()))
        }
        _ => None,
    }
}

/// Extract the comparable signature of a function definition, lowering every
/// annotation through the module's bindings.
fn fn_signature(
    bindings: &BindingTable,
    source: &str,
    func: &ruff_python_ast::StmtFunctionDef,
) -> FnSignature {
    let params = &func.parameters;
    let pair = |pwd: &ruff_python_ast::ParameterWithDefault| {
        (
            pwd.parameter.name.to_string(),
            pwd.parameter
                .annotation
                .as_deref()
                .map(|ann| AnnInfo::lower(bindings, source, ann)),
        )
    };
    let star = |param: Option<&ruff_python_ast::Parameter>| match param {
        None => StarSlot::Absent,
        Some(p) => p
            .annotation
            .as_deref()
            .map_or(StarSlot::Untyped, |ann| {
                StarSlot::Typed(AnnInfo::lower(bindings, source, ann))
            }),
    };
    FnSignature {
        posonly: params.posonlyargs.iter().map(&pair).collect(),
        standard: params.args.iter().map(&pair).collect(),
        kwonly: params.kwonlyargs.iter().map(&pair).collect(),
        vararg: star(params.vararg.as_deref()),
        kwarg: star(params.kwarg.as_deref()),
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
    // Only a DEFINITE difference counts ([RESOLV-CANONICAL-RELATION]).
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
                [first, second] => signatures_definitely_differ(first, second),
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
/// leading positional arguments; `None` when compatible or unresolvable.
///
/// The prefix supplies a value of the prefix type into the leading parameter,
/// so the prefix type must be [`assignable`] to the parameter's annotation; a
/// mismatch is reported only on a definite `Some(false)`.
fn prefix_mismatch(sig: &FnSignature, prefix: &[AnnInfo]) -> Option<String> {
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
        if assignable(&expected.node, &actual.node) == Some(false) {
            return Some(format!(
                "leading parameter is `{}`, but the Concatenate prefix supplies `{}`",
                actual.display, expected.display
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
    returns: &HashMap<&str, (Vec<AnnInfo>, String)>,
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
    ret_prefix: &[AnnInfo],
) -> Option<FnSignature> {
    if base.positional_count() < consumed {
        return None;
    }
    let mut remaining: Vec<(bool, (String, Option<AnnInfo>))> = base
        .posonly
        .iter()
        .map(|p| (true, p.clone()))
        .chain(base.standard.iter().map(|p| (false, p.clone())))
        .collect();
    let leftover = remaining.split_off(consumed);

    let mut posonly: Vec<(String, Option<AnnInfo>)> = ret_prefix
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
    module: &ResolvedModule,
    stmts: &[Stmt],
    derived: &HashMap<String, FnSignature>,
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
        if let Some(problem) = derived_call_problem(call, sig, &module.source) {
            let range = call.range();
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!("Invalid call to `{}`: {problem}", callee.id),
                Span {
                    start: range.start().to_u32(),
                    end: range.end().to_u32(),
                },
                &module.path,
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
///
/// Argument types come from [`TypeNode::of_literal_expr`] and are related to
/// the resolved parameter annotation through [`assignable`]; only a definite
/// `Some(false)` reports. `source` feeds message text only.
fn derived_call_problem(
    call: &ruff_python_ast::ExprCall,
    sig: &FnSignature,
    source: &str,
) -> Option<String> {
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
    let annotations: Vec<Option<&AnnInfo>> = sig.positional_annotations().collect();
    for (idx, arg) in call.arguments.args.iter().enumerate() {
        let actual = TypeNode::of_literal_expr(arg);
        let expected = annotations
            .get(idx)
            .copied()
            .flatten()
            .or_else(|| sig.vararg.ann());
        let Some(expected) = expected else { continue };
        if assignable(&actual, &expected.node) == Some(false) {
            return Some(format!(
                "argument {} (`{}`) is not assignable to `{}`",
                idx + 1,
                node_message_text(source, arg),
                expected.display
            ));
        }
    }
    None
}
