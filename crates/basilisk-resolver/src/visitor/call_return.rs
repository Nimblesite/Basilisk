//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Static return-type inference for *calls* used by `assert_type` checking
//! (part of `directives_assert_type_2`).
//!
//! The string-based resolver in `calls_and_reveal.rs` only types names and
//! literals. This module adds conservative inference for:
//!   * a generic **function** call — binding `TypeVar`s from the arguments and
//!     substituting them into the return annotation (`def f(x: T) -> T; f(1)` →
//!     `int`);
//!   * a generic **method** call on a variable of known type.
//!
//! It is deliberately false-positive averse: constructors,
//! `*args`/`**kwargs` callables, and unknown callees all resolve to `None`,
//! leaving the assertion unchecked rather than risking a wrong inference.

use std::collections::{HashMap, HashSet};

use ruff_python_ast::{Expr, ExprCall, Parameters, Stmt};
use ruff_text_size::Ranged as _;

use super::assert_narrow::{bind_type_vars, substitute_type_vars};
use super::class_info_ext::expr_simple_name;
use super::core::source_slice_range;
use super::typeddict::resolve_actual_type;

/// A callable signature: ordered parameters (name + optional annotation) and the
/// return annotation text. `has_varargs` marks `*args`/`**kwargs`, which defeat
/// positional binding.
struct SigInfo {
    params: Vec<(String, Option<String>)>,
    ret: Option<String>,
    has_varargs: bool,
}

/// Module-level facts needed to infer a call's return type.
pub(super) struct CallReturnCtx {
    signatures: HashMap<String, SigInfo>,
    class_names: HashSet<String>,
    module_vars: HashMap<String, String>,
    /// Callable keys defined more than once; their return type depends on
    /// overload resolution we do not perform, so they are never inferred.
    overloaded: HashSet<String>,
}

/// Collect signatures, class names and module variable annotations from the
/// module's top level (and one level of class bodies).
pub(super) fn collect(stmts: &[Stmt], source: &str) -> CallReturnCtx {
    let mut ctx = CallReturnCtx {
        signatures: HashMap::new(),
        class_names: HashSet::new(),
        module_vars: HashMap::new(),
        overloaded: HashSet::new(),
    };
    for stmt in stmts {
        match stmt {
            Stmt::FunctionDef(func) => {
                register_signature(&mut ctx, func.name.to_string(), func, source);
            }
            Stmt::ClassDef(cls) => {
                let _ = ctx.class_names.insert(cls.name.to_string());
                for member in &cls.body {
                    if let Stmt::FunctionDef(method) = member {
                        let key = format!("{}.{}", cls.name, method.name);
                        register_signature(&mut ctx, key, method, source);
                    }
                }
            }
            Stmt::AnnAssign(node) => {
                if let Expr::Name(target) = node.target.as_ref() {
                    if let Some(text) = source_slice_range(source, node.annotation.range()) {
                        let _ = ctx
                            .module_vars
                            .insert(target.id.to_string(), text.trim().to_owned());
                    }
                }
            }
            _ => {}
        }
    }
    ctx
}

/// Record a function/method signature, flagging the key as overloaded when it
/// is defined more than once.
fn register_signature(
    ctx: &mut CallReturnCtx,
    key: String,
    func: &ruff_python_ast::StmtFunctionDef,
    source: &str,
) {
    if ctx.signatures.contains_key(&key) {
        let _ = ctx.overloaded.insert(key.clone());
    }
    let _ = ctx.signatures.insert(
        key,
        sig_of(&func.parameters, func.returns.as_deref(), source),
    );
}

fn sig_of(parameters: &Parameters, returns: Option<&Expr>, source: &str) -> SigInfo {
    let params = parameters
        .posonlyargs
        .iter()
        .chain(parameters.args.iter())
        .map(|p| {
            let name = p.parameter.name.to_string();
            let ann = p
                .parameter
                .annotation
                .as_deref()
                .and_then(|a| source_slice_range(source, a.range()))
                .map(|t| t.trim().to_owned());
            (name, ann)
        })
        .collect();
    let ret =
        returns.and_then(|r| source_slice_range(source, r.range()).map(|t| t.trim().to_owned()));
    SigInfo {
        params,
        ret,
        has_varargs: parameters.vararg.is_some() || parameters.kwarg.is_some(),
    }
}

/// Infer the return type of `call`, or `None` when it cannot be decided safely.
pub(super) fn infer_call_return(
    call: &ExprCall,
    env: &HashMap<String, String>,
    type_vars: &HashSet<String>,
    ctx: &CallReturnCtx,
) -> Option<String> {
    let (key, is_method) = resolve_callee(call, env, ctx)?;
    if ctx.overloaded.contains(&key) {
        return None; // overload resolution is out of scope
    }
    let sig = ctx.signatures.get(&key)?;
    if sig.has_varargs {
        return None;
    }
    let ret = sig.ret.as_deref()?.trim().to_owned();
    let eff_params = effective_params(&sig.params, is_method);

    // Bind TypeVars from the positional arguments.
    let mut binds = HashMap::new();
    for (index, (_, ann)) in eff_params.iter().enumerate() {
        let Some(ann) = ann else { continue };
        let Some(arg) = call.arguments.args.get(index) else {
            break;
        };
        if let Some(actual) = resolve_actual_type(arg, env, "") {
            bind_type_vars(ann, &actual, type_vars, &mut binds);
        }
    }

    let candidate = if type_vars.contains(ret.as_str()) {
        binds.get(ret.as_str())?.clone()
    } else {
        substitute_type_vars(&ret, &binds)
    };
    // Only commit to a *fully concrete* inference: a leftover `TypeVar` means we
    // could not determine the type, so leave the assertion unchecked rather than
    // risk a false positive.
    is_fully_concrete(&candidate, type_vars).then_some(candidate)
}

/// `true` when `ty` contains no unresolved `TypeVar` — i.e. every identifier
/// token is a type we are confident about.
fn is_fully_concrete(ty: &str, type_vars: &HashSet<String>) -> bool {
    ty.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|token| !token.is_empty())
        .all(|token| !type_vars.contains(token))
}

/// `(signature key, is_method)` for a call we can analyse, else `None`.
fn resolve_callee(
    call: &ExprCall,
    env: &HashMap<String, String>,
    ctx: &CallReturnCtx,
) -> Option<(String, bool)> {
    match call.func.as_ref() {
        Expr::Name(name) => {
            let name = name.id.to_string();
            // Constructors are typed by the class, not its `__init__` return.
            (!ctx.class_names.contains(&name)).then_some((name, false))
        }
        Expr::Attribute(attr) => {
            let receiver = expr_simple_name(&attr.value)?;
            let receiver_ty = env
                .get(&receiver)
                .or_else(|| ctx.module_vars.get(&receiver))?;
            let base = receiver_ty.split('[').next().unwrap_or(receiver_ty).trim();
            Some((format!("{base}.{}", attr.attr), true))
        }
        _ => None,
    }
}

/// The positional parameters relevant to call binding — for a method, the
/// leading `self`/`cls` is dropped.
fn effective_params(
    params: &[(String, Option<String>)],
    is_method: bool,
) -> Vec<(String, Option<String>)> {
    if is_method {
        if let Some((first, _)) = params.first() {
            if first == "self" || first == "cls" {
                return params.iter().skip(1).cloned().collect();
            }
        }
    }
    params.to_vec()
}
