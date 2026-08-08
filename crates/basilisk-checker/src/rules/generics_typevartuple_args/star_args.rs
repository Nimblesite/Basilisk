//! Implements [`generics_typevartuple_args`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Call-site validation for `*args` annotated with an unpacked tuple type
//! (PEP 646): `*args: *tuple[int, ...]`, `*args: *tuple[int, str]`, and
//! mixed forms like `*args: *tuple[int, *tuple[str, ...], str]`.
//!
//! The `tuple` constructor is recognised through the binding table
//! ([ASTREBUILD-LAW]) — `typing.Tuple`, an aliased import, or a shadowed
//! `tuple` all resolve by identity, never by spelling. Expected element types
//! are lowered to [`TypeNode`]s and related to argument literals with
//! [`assignable`]; a relation the layer cannot decide abstains and no
//! diagnostic is emitted. Source text appears only in diagnostic messages.

use std::collections::{HashMap, HashSet};

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged as _;

use basilisk_resolver::{assignable, BindingTable, Span, TypeNode, TypingForm};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic};
use crate::span_util::slice_span;

use super::CODE;

/// An expected element type: the resolved node for the verdict and the
/// annotation's source text for diagnostic messages only.
struct Slot {
    node: TypeNode,
    text: String,
}

/// The shape of an unpacked-tuple `*args` annotation.
enum StarShape {
    /// `*tuple[int, str]` — exactly these argument types.
    Fixed(Vec<Slot>),
    /// `*tuple[int, ...]` — zero or more arguments of one type.
    Homogeneous(Slot),
    /// `*tuple[P.., *V, S..]` — fixed prefix, variadic middle, fixed suffix.
    Mixed {
        prefix: Vec<Slot>,
        /// Element type of the variadic middle; `None` when unknown (`*Ts`).
        middle: Option<Slot>,
        suffix: Vec<Slot>,
    },
    /// `*args: tuple[*Ts]` — each variadic argument is `tuple[*Ts]`, all sharing
    /// the same `TypeVarTuple`. The solver joins element *types*, so only the
    /// arity must agree: every tuple-literal argument must have equal length.
    SharedTvt,
}

/// A function whose `*args` carries an unpacked tuple annotation.
struct StarArgsFunction {
    /// Number of leading positional parameters consumed before `*args`.
    leading: usize,
    shape: StarShape,
}

/// Entry point: validate calls to functions with unpacked-tuple `*args`.
pub(super) fn check_star_args_calls(
    stmts: &[Stmt],
    bindings: &BindingTable,
    source: &str,
    tvt_names: &HashSet<&str>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut functions: HashMap<&str, StarArgsFunction> = HashMap::new();
    for stmt in stmts {
        let Stmt::FunctionDef(func) = stmt else {
            continue;
        };
        let Some(vararg) = func.parameters.vararg.as_deref() else {
            continue;
        };
        let shape = match vararg.annotation.as_deref() {
            // `*args: *tuple[...]` — the annotation is a starred tuple.
            Some(Expr::Starred(starred)) => parse_star_shape(bindings, source, &starred.value),
            // `*args: tuple[*Ts]` — the annotation is a (non-starred) `tuple`
            // subscript whose sole element unpacks a `TypeVarTuple`.
            Some(other) => shared_tvt_name(bindings, other, tvt_names).map(|_| StarShape::SharedTvt),
            None => None,
        };
        let Some(shape) = shape else {
            continue;
        };
        let leading = func.parameters.posonlyargs.len() + func.parameters.args.len();
        let _ = functions.insert(func.name.as_str(), StarArgsFunction { leading, shape });
    }
    if functions.is_empty() {
        return;
    }
    scan_stmts(stmts, &functions, source, path, diagnostics);
}

/// Does the expression's subscript base resolve to the builtin `tuple`
/// constructor (or its `typing.Tuple` alias)?
fn is_tuple_form(bindings: &BindingTable, expr: &Expr) -> bool {
    matches!(
        bindings.form_of_with_builtins(expr),
        Some(TypingForm::TupleClass | TypingForm::TupleAlias)
    )
}

/// The `TypeVarTuple` name a `tuple[*Ts]` annotation binds, when the base
/// resolves to the tuple form and the slice is exactly one starred reference
/// to a declared `TypeVarTuple`. `None` for any other annotation.
pub(super) fn shared_tvt_name<'e>(
    bindings: &BindingTable,
    expr: &'e Expr,
    tvt_names: &HashSet<&str>,
) -> Option<&'e str> {
    let Expr::Subscript(sub) = expr else {
        return None;
    };
    if !is_tuple_form(bindings, &sub.value) {
        return None;
    }
    // The slice must be exactly a single `*Name` unpacking a TypeVarTuple. Ruff
    // may model the lone starred element either directly as `Starred` or wrapped
    // in a single-element `Tuple`, so accept both shapes.
    let starred = match sub.slice.as_ref() {
        Expr::Starred(starred) => starred,
        Expr::Tuple(tuple) => match tuple.elts.as_slice() {
            [Expr::Starred(starred)] => starred,
            _ => return None,
        },
        _ => return None,
    };
    match starred.value.as_ref() {
        Expr::Name(name) if tvt_names.contains(name.id.as_str()) => Some(name.id.as_str()),
        _ => None,
    }
}

/// An expected-type slot: the lowered node plus the annotation's source text
/// (messages only).
fn slot(bindings: &BindingTable, source: &str, expr: &Expr) -> Slot {
    Slot {
        node: TypeNode::lower(bindings, expr),
        text: slice_span(source, Span::from(expr.range()))
            .unwrap_or("<type>")
            .trim()
            .to_owned(),
    }
}

/// Parse `tuple[...]` into a [`StarShape`].
fn parse_star_shape(bindings: &BindingTable, source: &str, expr: &Expr) -> Option<StarShape> {
    let Expr::Subscript(sub) = expr else {
        return None;
    };
    if !is_tuple_form(bindings, &sub.value) {
        return None;
    }
    let elts = basilisk_parser::subscript_elements(sub);

    if let [elem, Expr::EllipsisLiteral(_)] = elts.as_slice() {
        return Some(StarShape::Homogeneous(slot(bindings, source, elem)));
    }
    if let Some(star_idx) = elts.iter().position(|e| matches!(e, Expr::Starred(_))) {
        let (front, rest) = elts.split_at(star_idx);
        let Some((Expr::Starred(inner), tail)) = rest.split_first() else {
            return None;
        };
        let prefix = front.iter().map(|e| slot(bindings, source, e)).collect();
        let suffix = tail.iter().map(|e| slot(bindings, source, e)).collect();
        let middle = match inner.value.as_ref() {
            Expr::Subscript(mid_sub) if is_tuple_form(bindings, &mid_sub.value) => {
                match mid_sub.slice.as_ref() {
                    Expr::Tuple(t) => match t.elts.as_slice() {
                        [elem, Expr::EllipsisLiteral(_)] => Some(slot(bindings, source, elem)),
                        _ => return None,
                    },
                    _ => return None,
                }
            }
            // `*Ts` — unknown variadic middle.
            Expr::Name(_) => None,
            _ => return None,
        };
        return Some(StarShape::Mixed {
            prefix,
            middle,
            suffix,
        });
    }
    Some(StarShape::Fixed(
        elts.iter().map(|e| slot(bindings, source, e)).collect(),
    ))
}

/// Walk statements scanning expressions for calls.
fn scan_stmts(
    stmts: &[Stmt],
    functions: &HashMap<&str, StarArgsFunction>,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    basilisk_resolver::walk_all_stmts(stmts, &mut |stmt| match stmt {
        Stmt::Expr(node) => scan_expr(&node.value, functions, source, path, diagnostics),
        Stmt::Assign(node) => scan_expr(&node.value, functions, source, path, diagnostics),
        Stmt::AnnAssign(node) => {
            if let Some(value) = node.value.as_deref() {
                scan_expr(value, functions, source, path, diagnostics);
            }
        }
        Stmt::Return(node) => {
            if let Some(value) = node.value.as_deref() {
                scan_expr(value, functions, source, path, diagnostics);
            }
        }
        _ => {}
    });
}

/// Recursively check call expressions (including nested call arguments).
fn scan_expr(
    expr: &Expr,
    functions: &HashMap<&str, StarArgsFunction>,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Expr::Call(call) = expr else { return };
    if let Expr::Name(name) = call.func.as_ref() {
        if let Some(star_func) = functions.get(name.id.as_str()) {
            validate_star_call(call, name.id.as_str(), star_func, source, path, diagnostics);
        }
    }
    for arg in &call.arguments.args {
        scan_expr(arg, functions, source, path, diagnostics);
    }
    for kw in &call.arguments.keywords {
        scan_expr(&kw.value, functions, source, path, diagnostics);
    }
}

/// Validate one call's variadic arguments against the function's [`StarShape`].
fn validate_star_call(
    call: &ruff_python_ast::ExprCall,
    name: &str,
    star_func: &StarArgsFunction,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(star_args) = call.arguments.args.get(star_func.leading..) else {
        return;
    };
    let problem = match &star_func.shape {
        StarShape::Fixed(slots) => validate_fixed(star_args, slots, source),
        StarShape::Homogeneous(elem) => validate_each(star_args, elem, source),
        StarShape::Mixed {
            prefix,
            middle,
            suffix,
        } => validate_mixed(star_args, prefix, middle.as_ref(), suffix, source),
        StarShape::SharedTvt => validate_shared_tvt(star_args),
    };
    let Some(problem) = problem else { return };
    let range = call.range();
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        format!("Invalid arguments for `*args` of `{name}`: {problem}"),
        Span {
            start: range.start().to_u32(),
            end: range.end().to_u32(),
        },
        path,
        Some("The unpacked tuple annotation fixes the accepted argument shape".to_owned()),
        None,
    ));
}

/// Exact arity and element types.
fn validate_fixed(args: &[Expr], slots: &[Slot], source: &str) -> Option<String> {
    if args.len() != slots.len() {
        return Some(format!(
            "expected exactly {} argument{}, got {}",
            slots.len(),
            if slots.len() == 1 { "" } else { "s" },
            args.len()
        ));
    }
    args.iter()
        .zip(slots.iter())
        .find_map(|(arg, expected)| incompatible(arg, expected, source))
}

/// Every argument must match the homogeneous element type.
fn validate_each(args: &[Expr], elem: &Slot, source: &str) -> Option<String> {
    args.iter().find_map(|arg| incompatible(arg, elem, source))
}

/// Prefix/middle/suffix validation for mixed shapes.
fn validate_mixed(
    args: &[Expr],
    prefix: &[Slot],
    middle: Option<&Slot>,
    suffix: &[Slot],
    source: &str,
) -> Option<String> {
    let min = prefix.len() + suffix.len();
    if args.len() < min {
        return Some(format!(
            "expected at least {min} argument{}, got {}",
            if min == 1 { "" } else { "s" },
            args.len()
        ));
    }
    let (front, rest) = args.split_at(prefix.len());
    let (mid, back) = rest.split_at(rest.len() - suffix.len());

    front
        .iter()
        .zip(prefix.iter())
        .find_map(|(arg, expected)| incompatible(arg, expected, source))
        .or_else(|| {
            middle.and_then(|elem| mid.iter().find_map(|arg| incompatible(arg, elem, source)))
        })
        .or_else(|| {
            back.iter()
                .zip(suffix.iter())
                .find_map(|(arg, expected)| incompatible(arg, expected, source))
        })
}

/// Every tuple-literal argument binds the shared `TypeVarTuple` to its arity, so
/// all such arguments must have the same length. Element types are joined by the
/// solver and never conflict, so only length is checked. Non-tuple-literal
/// arguments are not analyzable and are ignored.
fn validate_shared_tvt(args: &[Expr]) -> Option<String> {
    let lengths: Vec<usize> = args
        .iter()
        .filter_map(|arg| match arg {
            Expr::Tuple(tuple) => Some(tuple.elts.len()),
            _ => None,
        })
        .collect();
    let first = lengths.first().copied()?;
    lengths.iter().any(|&len| len != first).then(|| {
        format!("every argument must be a tuple of the same length, but found lengths {lengths:?}")
    })
}

/// A human-readable problem when `arg` is provably incompatible with
/// `expected`; `None` when compatible or not analyzable.
///
/// The verdict comes from [`assignable`] over the argument's literal type and
/// the lowered expected type; anything the relation cannot decide — a call
/// result, a variable, an unresolved annotation — abstains.
fn incompatible(arg: &Expr, expected: &Slot, source: &str) -> Option<String> {
    if assignable(&TypeNode::of_literal_expr(arg), &expected.node) == Some(false) {
        let arg_text = slice_span(source, Span::from(arg.range()))
            .unwrap_or("<argument>")
            .trim();
        return Some(format!(
            "`{arg_text}` is not assignable to `{}`",
            expected.text
        ));
    }
    None
}
