//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Flow-sensitive narrowing for `assert_type` checking (part of [BSK-E0053]).
//!
//! `assert_type(expr, T)` compares the *static* type of `expr` against `T`. The
//! static type is heuristic and string-based (see `calls_and_reveal.rs`), keyed
//! off declared parameter annotations. This module narrows those annotations as
//! control flow guards are applied, so post-guard assertions match:
//!
//! - `isinstance(x, T)` plus a diverging branch narrows `x` afterwards (§7.1).
//! - `x is Enum.MEMBER` narrows `x` to `Literal[Enum.MEMBER]`; an exhaustive
//!   `if/elif/.../else` chain over a non-`Flag` enum narrows the `else` arm.
//! - A call to a `TypeGuard[U]` / `TypeIs[U]` function narrows its first
//!   positional argument to `U` (with `TypeVar` substitution).
//!
//! Only declared **parameters** are narrowed; locals keep an unknown type so the
//! checker stays conservative (no new false positives on un-annotated values).

use std::collections::{HashMap, HashSet};

use ruff_python_ast::{CmpOp, ExceptHandler, Expr, Stmt, StmtClassDef, StmtFunctionDef};
use ruff_text_size::Ranged as _;

use crate::scope::AssertTypeCallInfo;

use super::calls_and_reveal::build_assert_type_call_info;
use super::class_info_ext::expr_simple_name;
use super::core::source_slice_range;
use super::function_info::build_param_scope_owned;
use super::typeddict::{resolve_actual_type, split_subscript, split_top_level_args};

/// Variable → current (possibly narrowed) type-annotation text.
type Env = HashMap<String, String>;

/// A user-defined type guard (`-> TypeGuard[U]` / `-> TypeIs[U]`).
struct GuardInfo {
    /// `true` for `TypeIs` (narrows the negative branch too), `false` for `TypeGuard`.
    is_typeis: bool,
    /// The narrowed-to type text `U`.
    target: String,
    /// Parameter annotations in declaration order (including `self`/`cls`).
    params: Vec<String>,
}

/// An enum class and whether it is a `Flag`/`IntFlag` (whose members combine).
struct EnumInfo {
    members: HashSet<String>,
    is_flag: bool,
}

/// Module-global metadata used while narrowing.
struct NarrowCtx<'a> {
    source: &'a str,
    enums: HashMap<String, EnumInfo>,
    guards: HashMap<String, GuardInfo>,
    /// Context-manager classes whose `__exit__` may suppress (`bool`/`Literal[True]`).
    suppress_cms: HashSet<String>,
    type_vars: HashSet<String>,
    /// Signatures / class names / module vars / `TypeVar` metadata used to infer
    /// the return type of a call inside `assert_type(...)`.
    call_return: super::call_return::CallReturnCtx,
}

/// Collect every `assert_type(...)` call in `stmts`, applying flow narrowing.
pub(super) fn collect(stmts: &[Stmt], source: &str) -> Vec<AssertTypeCallInfo> {
    let mut ctx = NarrowCtx {
        source,
        enums: HashMap::new(),
        guards: HashMap::new(),
        suppress_cms: HashSet::new(),
        type_vars: HashSet::new(),
        call_return: super::call_return::collect(stmts, source),
    };
    collect_metadata(stmts, &mut ctx);
    let mut out = Vec::new();
    let env = Env::new();
    walk_body(stmts, &env, &ctx, &mut out);
    out
}

// ---------------------------------------------------------------------------
// Metadata collection (recursive over the whole module)
// ---------------------------------------------------------------------------

fn collect_metadata(stmts: &[Stmt], ctx: &mut NarrowCtx<'_>) {
    for stmt in stmts {
        match stmt {
            Stmt::Assign(node) => {
                if let Some(name) = single_target_name(node) {
                    if call_to_any(&node.value, &["TypeVar", "TypeVarTuple", "ParamSpec"]) {
                        let _ = ctx.type_vars.insert(name);
                    }
                }
            }
            Stmt::ClassDef(cls) => collect_class_metadata(cls, ctx),
            Stmt::FunctionDef(func) => {
                register_guard(func, ctx);
                collect_metadata(&func.body, ctx);
            }
            _ => {}
        }
    }
}

fn collect_class_metadata(cls: &StmtClassDef, ctx: &mut NarrowCtx<'_>) {
    if let Some(info) = enum_info(cls) {
        let _ = ctx.enums.insert(cls.name.to_string(), info);
    }
    if class_suppresses(cls, ctx.source) {
        let _ = ctx.suppress_cms.insert(cls.name.to_string());
    }
    for stmt in &cls.body {
        if let Stmt::FunctionDef(method) = stmt {
            register_guard(method, ctx);
        }
    }
}

/// Build [`EnumInfo`] when `cls` derives from an enum base.
fn enum_info(cls: &StmtClassDef) -> Option<EnumInfo> {
    let bases = base_names(cls);
    let is_enum = bases.iter().any(|b| {
        matches!(
            b.as_str(),
            "Enum" | "IntEnum" | "StrEnum" | "Flag" | "IntFlag"
        )
    });
    if !is_enum {
        return None;
    }
    let is_flag = bases
        .iter()
        .any(|b| matches!(b.as_str(), "Flag" | "IntFlag"));
    let members = cls
        .body
        .iter()
        .filter_map(member_name)
        .collect::<HashSet<_>>();
    Some(EnumInfo { members, is_flag })
}

/// The simple member name declared by an enum-body statement (`NAME = ...`).
fn member_name(stmt: &Stmt) -> Option<String> {
    match stmt {
        Stmt::Assign(node) => single_target_name(node),
        Stmt::AnnAssign(node) => expr_simple_name(&node.target),
        _ => None,
    }
}

/// Register `func` as a guard when it returns `TypeGuard[U]` / `TypeIs[U]`.
fn register_guard(func: &StmtFunctionDef, ctx: &mut NarrowCtx<'_>) {
    let Some(returns) = func.returns.as_deref() else {
        return;
    };
    let Expr::Subscript(sub) = returns else {
        return;
    };
    let Some(head) = expr_simple_name(&sub.value).or_else(|| attr_name(&sub.value)) else {
        return;
    };
    let is_typeis = match head.as_str() {
        "TypeIs" => true,
        "TypeGuard" => false,
        _ => return,
    };
    let Some(target) = source_slice_range(ctx.source, sub.slice.range()) else {
        return;
    };
    let params = super::walks::iter_all_params(&func.parameters)
        .map(|p| {
            p.parameter
                .annotation
                .as_deref()
                .and_then(|a| source_slice_range(ctx.source, a.range()))
                .unwrap_or("")
                .to_owned()
        })
        .collect();
    let _ = ctx.guards.insert(
        func.name.to_string(),
        GuardInfo {
            is_typeis,
            target: target.trim().to_owned(),
            params,
        },
    );
}

/// `true` when `cls` is a context manager whose `__exit__` may suppress
/// exceptions — i.e. returns exactly `bool` or `Literal[True]`.
fn class_suppresses(cls: &StmtClassDef, source: &str) -> bool {
    cls.body.iter().any(|stmt| {
        let Stmt::FunctionDef(method) = stmt else {
            return false;
        };
        if method.name.as_str() != "__exit__" {
            return false;
        }
        method
            .returns
            .as_deref()
            .and_then(|r| source_slice_range(source, r.range()))
            .map(str::trim)
            .is_some_and(|t| t == "bool" || t == "Literal[True]")
    })
}

// ---------------------------------------------------------------------------
// Body walker (mirrors the generic traversal, adding narrowing at `if`)
// ---------------------------------------------------------------------------

/// Infer the static type of `assert_type`'s first argument: the existing
/// name/literal resolution, plus call/subscript inference (enum lookup, generic
/// function/method returns) that the string-based resolver cannot do alone.
fn infer_actual_type(expr: &Expr, env: &Env, ctx: &NarrowCtx<'_>) -> Option<String> {
    if let Some(simple) = resolve_actual_type(expr, env, ctx.source) {
        return Some(simple);
    }
    match expr {
        // `Enum["MEMBER"]` performs a member lookup → the enum type.
        Expr::Subscript(sub) => {
            let base = expr_simple_name(&sub.value)?;
            ctx.enums.contains_key(base.as_str()).then_some(base)
        }
        // `Enum(value)` performs a value-based member lookup → the enum type;
        // otherwise try generic function/method return-type inference.
        Expr::Call(call) => {
            if let Some(callee) = expr_simple_name(&call.func) {
                if ctx.enums.contains_key(callee.as_str()) {
                    return Some(callee);
                }
            }
            super::call_return::infer_call_return(call, env, &ctx.type_vars, &ctx.call_return)
        }
        _ => None,
    }
}

fn walk_body(stmts: &[Stmt], env: &Env, ctx: &NarrowCtx<'_>, out: &mut Vec<AssertTypeCallInfo>) {
    let mut env = env.clone();
    for stmt in stmts {
        match stmt {
            Stmt::Expr(node) => {
                if let Expr::Call(call) = node.value.as_ref() {
                    if expr_simple_name(&call.func).is_some_and(|n| n == "assert_type") {
                        let actual = call
                            .arguments
                            .args
                            .first()
                            .and_then(|first| infer_actual_type(first, &env, ctx));
                        out.push(build_assert_type_call_info(call, actual, ctx.source));
                    }
                }
            }
            Stmt::FunctionDef(func) => {
                let params: Env = build_param_scope_owned(&func.parameters, ctx.source)
                    .into_iter()
                    .collect();
                walk_body(&func.body, &params, ctx, out);
            }
            Stmt::ClassDef(cls) => walk_body(&cls.body, &Env::new(), ctx, out),
            Stmt::If(node) => env = walk_if(node, &env, ctx, out),
            Stmt::For(node) => {
                walk_body(&node.body, &env, ctx, out);
                walk_body(&node.orelse, &env, ctx, out);
            }
            Stmt::While(node) => {
                walk_body(&node.body, &env, ctx, out);
                walk_body(&node.orelse, &env, ctx, out);
            }
            Stmt::With(node) => walk_body(&node.body, &env, ctx, out),
            Stmt::Try(node) => {
                walk_body(&node.body, &env, ctx, out);
                for handler in &node.handlers {
                    let ExceptHandler::ExceptHandler(h) = handler;
                    walk_body(&h.body, &env, ctx, out);
                }
                walk_body(&node.orelse, &env, ctx, out);
                walk_body(&node.finalbody, &env, ctx, out);
            }
            Stmt::Match(node) => {
                for case in &node.cases {
                    walk_body(&case.body, &env, ctx, out);
                }
            }
            _ => {}
        }
    }
}

/// Walk an `if/elif/else` chain, narrowing each branch. Returns the environment
/// in effect for statements *after* the chain.
fn walk_if(
    node: &ruff_python_ast::StmtIf,
    env: &Env,
    ctx: &NarrowCtx<'_>,
    out: &mut Vec<AssertTypeCallInfo>,
) -> Env {
    // `if` branch: narrowed positively.
    let if_env = then_env(env, &node.test, ctx);
    walk_body(&node.body, &if_env, ctx, out);

    // Tests seen so far (their negation accumulates for later branches).
    let mut prior_tests: Vec<&Expr> = vec![&node.test];
    let mut all_diverge = body_diverges(&node.body, ctx);
    let mut has_else = false;

    for clause in &node.elif_else_clauses {
        let neg_env = else_env(env, &prior_tests, ctx);
        if let Some(test) = &clause.test {
            let branch_env = then_env(&neg_env, test, ctx);
            walk_body(&clause.body, &branch_env, ctx, out);
            all_diverge = all_diverge && body_diverges(&clause.body, ctx);
            prior_tests.push(test);
        } else {
            has_else = true;
            walk_body(&clause.body, &neg_env, ctx, out);
        }
    }

    // After the chain: when there is no `else` and every preceding branch
    // diverges, the negative narrowing of all tests holds for what follows.
    if !has_else && all_diverge {
        else_env(env, &prior_tests, ctx)
    } else {
        env.clone()
    }
}

// ---------------------------------------------------------------------------
// Narrowing transforms
// ---------------------------------------------------------------------------

/// Environment inside the positive branch of `test`.
fn then_env(env: &Env, test: &Expr, ctx: &NarrowCtx<'_>) -> Env {
    let mut next = env.clone();
    if let Some((var, ty)) = then_narrowing(test, env, ctx) {
        let _ = next.insert(var, ty);
    }
    next
}

/// Environment inside a branch where every test in `tests` is false.
fn else_env(env: &Env, tests: &[&Expr], ctx: &NarrowCtx<'_>) -> Env {
    let mut next = env.clone();
    // (var, enum) -> members excluded so far.
    let mut excluded: HashMap<(String, String), HashSet<String>> = HashMap::new();
    for test in tests {
        for fact in else_facts(test, env, ctx) {
            match fact {
                ElseFact::Subtract { var, ty } => {
                    if let Some(cur) = next.get(&var) {
                        let narrowed = subtract_arm(cur, &ty);
                        let _ = next.insert(var, narrowed);
                    }
                }
                ElseFact::EnumMember {
                    var,
                    enum_name,
                    member,
                } => {
                    let _ = excluded.entry((var, enum_name)).or_default().insert(member);
                }
            }
        }
    }
    // Drop an enum arm only when all members of a non-`Flag` enum are excluded.
    for ((var, enum_name), members) in excluded {
        let exhaustive = ctx
            .enums
            .get(&enum_name)
            .is_some_and(|info| !info.is_flag && info.members.iter().all(|m| members.contains(m)));
        if exhaustive {
            if let Some(cur) = next.get(&var) {
                let narrowed = subtract_arm(cur, &enum_name);
                let _ = next.insert(var, narrowed);
            }
        }
    }
    next
}

/// The positive-branch narrowing `(variable, narrowed_type)` for a single test.
fn then_narrowing(test: &Expr, env: &Env, ctx: &NarrowCtx<'_>) -> Option<(String, String)> {
    match test {
        Expr::Call(call) => {
            if let Some((var, ty)) = isinstance_fact(call, ctx) {
                return env.contains_key(&var).then_some((var, ty));
            }
            guard_then(call, env, ctx)
        }
        Expr::Compare(cmp) => {
            let (var, enum_name, member) = enum_is_fact(cmp, ctx)?;
            env.contains_key(&var)
                .then(|| (var, format!("Literal[{enum_name}.{member}]")))
        }
        _ => None,
    }
}

/// Negative-branch subtraction facts for a test (recurses through `or`).
fn else_facts(test: &Expr, env: &Env, ctx: &NarrowCtx<'_>) -> Vec<ElseFact> {
    match test {
        Expr::BoolOp(b) if matches!(b.op, ruff_python_ast::BoolOp::Or) => b
            .values
            .iter()
            .flat_map(|v| else_facts(v, env, ctx))
            .collect(),
        Expr::Call(call) => {
            if let Some((var, ty)) = isinstance_fact(call, ctx) {
                return when_param(env, var, |var| ElseFact::Subtract {
                    var,
                    ty: ty.clone(),
                });
            }
            // Only `TypeIs` narrows the negative branch.
            match guard_then(call, env, ctx) {
                Some((var, ty)) if guard_is_typeis(call, ctx) => {
                    vec![ElseFact::Subtract { var, ty }]
                }
                _ => Vec::new(),
            }
        }
        Expr::Compare(cmp) => match enum_is_fact(cmp, ctx) {
            Some((var, enum_name, member)) if env.contains_key(&var) => {
                vec![ElseFact::EnumMember {
                    var,
                    enum_name,
                    member,
                }]
            }
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

enum ElseFact {
    Subtract {
        var: String,
        ty: String,
    },
    EnumMember {
        var: String,
        enum_name: String,
        member: String,
    },
}

fn when_param(env: &Env, var: String, make: impl Fn(String) -> ElseFact) -> Vec<ElseFact> {
    if env.contains_key(&var) {
        vec![make(var)]
    } else {
        Vec::new()
    }
}

/// `isinstance(x, T)` → `(x, "T")`. Only single-class checks are narrowed.
fn isinstance_fact(
    call: &ruff_python_ast::ExprCall,
    ctx: &NarrowCtx<'_>,
) -> Option<(String, String)> {
    if expr_simple_name(&call.func)? != "isinstance" || call.arguments.args.len() != 2 {
        return None;
    }
    let var = expr_simple_name(call.arguments.args.first()?)?;
    let ty_expr = call.arguments.args.get(1)?;
    if matches!(ty_expr, Expr::Tuple(_)) {
        return None;
    }
    let ty = source_slice_range(ctx.source, ty_expr.range())?.trim();
    Some((var, ty.to_owned()))
}

/// `x is Enum.MEMBER` → `(x, "Enum", "MEMBER")` when `Enum` is a known enum.
fn enum_is_fact(
    cmp: &ruff_python_ast::ExprCompare,
    ctx: &NarrowCtx<'_>,
) -> Option<(String, String, String)> {
    if cmp.ops.as_ref() != [CmpOp::Is] || cmp.comparators.len() != 1 {
        return None;
    }
    let var = expr_simple_name(&cmp.left)?;
    let Expr::Attribute(attr) = cmp.comparators.first()? else {
        return None;
    };
    let enum_name = expr_simple_name(&attr.value)?;
    if !ctx.enums.contains_key(&enum_name) {
        return None;
    }
    Some((var, enum_name, attr.attr.to_string()))
}

/// Positive-branch narrowing for a guard call `f(x)` / `obj.m(x)`.
fn guard_then(
    call: &ruff_python_ast::ExprCall,
    env: &Env,
    ctx: &NarrowCtx<'_>,
) -> Option<(String, String)> {
    let name = call_name(&call.func)?;
    let guard = ctx.guards.get(&name)?;
    let var = expr_simple_name(call.arguments.args.first()?)?;
    if !env.contains_key(&var) {
        return None;
    }
    let bindings = bind_guard_type_vars(guard, &call.arguments.args, env, ctx);
    Some((var, substitute_type_vars(&guard.target, &bindings)))
}

fn guard_is_typeis(call: &ruff_python_ast::ExprCall, ctx: &NarrowCtx<'_>) -> bool {
    call_name(&call.func)
        .and_then(|n| ctx.guards.get(&n))
        .is_some_and(|g| g.is_typeis)
}

/// Bind the guard's `TypeVar`s from the call arguments, aligning the call's
/// positional arguments with the guard's trailing parameters (skipping
/// `self`/`cls`, which are never passed positionally at the call site).
fn bind_guard_type_vars(
    guard: &GuardInfo,
    args: &[Expr],
    env: &Env,
    ctx: &NarrowCtx<'_>,
) -> HashMap<String, String> {
    let mut bindings = HashMap::new();
    let skip = guard.params.len().saturating_sub(args.len());
    for (param, arg) in guard.params.iter().skip(skip).zip(args.iter()) {
        if let Some(actual) = arg_type(arg, env, ctx) {
            bind_type_vars(param, &actual, &ctx.type_vars, &mut bindings);
        }
    }
    bindings
}

/// Static type text of a call argument: a parameter's narrowed type, or
/// `type[Name]` for a bare class reference (so `type[T]` binds correctly).
fn arg_type(arg: &Expr, env: &Env, ctx: &NarrowCtx<'_>) -> Option<String> {
    let name = expr_simple_name(arg)?;
    Some(env.get(&name).cloned().unwrap_or_else(|| {
        let _ = ctx;
        format!("type[{name}]")
    }))
}

/// Structurally match `pattern` against `actual`, binding any `TypeVar` in
/// `tvars` to the corresponding `actual` sub-expression.
pub(super) fn bind_type_vars(
    pattern: &str,
    actual: &str,
    tvars: &HashSet<String>,
    out: &mut HashMap<String, String>,
) {
    let pattern = pattern.trim();
    let actual = actual.trim();
    if tvars.contains(pattern) {
        let _ = out
            .entry(pattern.to_owned())
            .or_insert_with(|| actual.to_owned());
        return;
    }
    if let (Some((ph, pi)), Some((ah, ai))) = (split_subscript(pattern), split_subscript(actual)) {
        if ph == ah {
            let pargs = split_top_level_args(pi);
            let aargs = split_top_level_args(ai);
            if pargs.len() == aargs.len() {
                for (pa, aa) in pargs.iter().zip(aargs.iter()) {
                    bind_type_vars(pa, aa, tvars, out);
                }
            }
        }
    }
}

/// Replace whole-identifier `TypeVar` tokens in `ty` with their bindings.
pub(super) fn substitute_type_vars(ty: &str, bindings: &HashMap<String, String>) -> String {
    if bindings.is_empty() {
        return ty.to_owned();
    }
    let mut result = String::with_capacity(ty.len());
    let mut ident = String::new();
    let flush = |ident: &mut String, result: &mut String| {
        if !ident.is_empty() {
            match bindings.get(ident.as_str()) {
                Some(sub) => result.push_str(sub),
                None => result.push_str(ident),
            }
            ident.clear();
        }
    };
    for ch in ty.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            ident.push(ch);
        } else {
            flush(&mut ident, &mut result);
            result.push(ch);
        }
    }
    flush(&mut ident, &mut result);
    result
}

// ---------------------------------------------------------------------------
// Divergence analysis (for post-`if` narrowing)
// ---------------------------------------------------------------------------

/// `true` when control cannot fall through `stmts` (it always raises/returns,
/// or enters a non-suppressing `with` block that diverges).
fn body_diverges(stmts: &[Stmt], ctx: &NarrowCtx<'_>) -> bool {
    stmts.iter().any(|stmt| stmt_diverges(stmt, ctx))
}

fn stmt_diverges(stmt: &Stmt, ctx: &NarrowCtx<'_>) -> bool {
    match stmt {
        Stmt::Raise(_) | Stmt::Return(_) => true,
        Stmt::With(node) => !with_suppresses(node, ctx) && body_diverges(&node.body, ctx),
        _ => false,
    }
}

/// `true` when any context manager in the `with` may suppress exceptions.
fn with_suppresses(node: &ruff_python_ast::StmtWith, ctx: &NarrowCtx<'_>) -> bool {
    node.items.iter().any(|item| {
        cm_class_name(&item.context_expr).is_some_and(|name| ctx.suppress_cms.contains(&name))
    })
}

/// The class name of a `with CM():` / `with CM:` context expression.
fn cm_class_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Call(call) => expr_simple_name(&call.func).or_else(|| attr_name(&call.func)),
        other => expr_simple_name(other),
    }
}

// ---------------------------------------------------------------------------
// Small AST/string helpers
// ---------------------------------------------------------------------------

/// Remove a top-level union arm equal to `remove`; collapse a single survivor.
fn subtract_arm(ty: &str, remove: &str) -> String {
    if !ty.contains('|') {
        return ty.to_owned();
    }
    let remove = remove.trim();
    let kept: Vec<&str> = split_top_level_pipe(ty)
        .into_iter()
        .filter(|arm| arm.trim() != remove)
        .collect();
    if kept.is_empty() {
        ty.to_owned()
    } else {
        kept.iter()
            .map(|s| s.trim())
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

/// Split `ty` on top-level `|`, respecting `[](){}` nesting.
fn split_top_level_pipe(ty: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth: usize = 0;
    let mut start = 0;
    for (idx, ch) in ty.char_indices() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth = depth.saturating_sub(1),
            '|' if depth == 0 => {
                parts.push(&ty[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    parts.push(&ty[start..]);
    parts
}

fn single_target_name(node: &ruff_python_ast::StmtAssign) -> Option<String> {
    match node.targets.as_slice() {
        [target] => expr_simple_name(target),
        _ => None,
    }
}

fn base_names(cls: &StmtClassDef) -> Vec<String> {
    cls.arguments
        .as_deref()
        .map(|args| {
            args.args
                .iter()
                .filter_map(|b| expr_simple_name(b).or_else(|| attr_name(b)))
                .collect()
        })
        .unwrap_or_default()
}

/// The attribute name of an `a.b.c` expression (`c`), else `None`.
fn attr_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Attribute(attr) => Some(attr.attr.to_string()),
        _ => None,
    }
}

/// The callee name of a call func expression (`f` or `obj.m`).
fn call_name(func: &Expr) -> Option<String> {
    expr_simple_name(func).or_else(|| attr_name(func))
}

/// `true` when `expr` is a call to any of `names`.
fn call_to_any(expr: &Expr, names: &[&str]) -> bool {
    let Expr::Call(call) = expr else {
        return false;
    };
    call_name(&call.func).is_some_and(|n| names.contains(&n.as_str()))
}
