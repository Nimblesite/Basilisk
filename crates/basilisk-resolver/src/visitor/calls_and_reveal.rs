//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Calls And Reveal visitor functions.

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

use crate::scope::{
    AssertTypeCallInfo, CallReceiver, CallSite, RevealTypeCallInfo, RhsKind, Span, TypeArg,
};

use super::class_info_ext::expr_simple_name;
use super::core::{classify_rhs, text_range_to_span};
use super::unhashable::collect_unhashable_hash_calls_from_expr;

pub(super) fn call_func_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(name) => Some(name.id.as_str()),
        Expr::Attribute(attr) => Some(attr.attr.as_str()),
        _ => None,
    }
}

pub(super) fn collect_reveal_type_calls(_stmts: &[Stmt]) -> Vec<RevealTypeCallInfo> {
    Vec::new()
}

/// Collect call sites from statements, including those inside function bodies.
/// Every supported call site in every expression position, in source order.
///
/// The walk is [`crate::visit_calls`], so `C(1).method()`, `f(C(1))`,
/// `[C(1)]`, and `C(1) if p else q` all record the `C(1)` site the bare
/// statement records ([#381](https://github.com/Nimblesite/Basilisk/issues/381));
/// [`call_site_from_call`] still decides which callee/receiver shapes are
/// representable.
pub(super) fn collect_calls_from_stmts(
    bindings: &basilisk_canonical::BindingTable,
    stmts: &[Stmt],
) -> Vec<CallSite> {
    let mut out = Vec::new();
    crate::visit_calls(stmts, &mut |call| {
        if let Some(site) = call_site_from_call(bindings, call) {
            out.push(site);
        }
    });
    out
}

/// Build a [`CallSite`] from a call node, when its callee shape is one the
/// site model represents (a bare name, or a method on a supported receiver).
pub(super) fn call_site_from_call(
    bindings: &basilisk_canonical::BindingTable,
    call: &ruff_python_ast::ExprCall,
) -> Option<CallSite> {
    let (callee, receiver, receiver_class_site) = match call.func.as_ref() {
        Expr::Name(name) => (name.id.to_string(), None, None),
        Expr::Attribute(attribute) => {
            // The receiver's CLASS is resolved from its expression, at the
            // call's own offset; the string in `CallReceiver` is kept for
            // message text only ([RESOLV-CANONICAL-BINDING]).
            let (receiver, site) = match attribute.value.as_ref() {
                Expr::StringLiteral(_) => (CallReceiver::StringLiteral, None),
                Expr::BytesLiteral(_) => (CallReceiver::BytesLiteral, None),
                Expr::Name(name) => (
                    CallReceiver::Name(name.id.to_string()),
                    bindings
                        .local_class_definition(attribute.value.as_ref())
                        .map(text_range_to_span),
                ),
                Expr::Call(constructor) => match constructor.func.as_ref() {
                    Expr::Name(name) => (
                        CallReceiver::Constructor(name.id.to_string()),
                        bindings
                            .local_class_definition(&constructor.func)
                            .map(text_range_to_span),
                    ),
                    _ => return None,
                },
                _ => return None,
            };
            (attribute.attr.to_string(), Some(receiver), site)
        }
        _ => return None,
    };
    let args: Vec<(RhsKind, Span)> = call
        .arguments
        .args
        .iter()
        .map(|arg| (classify_rhs(arg), text_range_to_span(arg.range())))
        .collect();
    let keywords: Vec<(String, RhsKind)> = call
        .arguments
        .keywords
        .iter()
        .filter_map(|kw| {
            kw.arg
                .as_ref()
                .map(|name| (name.to_string(), classify_rhs(&kw.value)))
        })
        .collect();
    // A keyword entry with no name (`kw.arg == None`) is a `**dict` unpack, which
    // hides an unknown number of named arguments from the static call view.
    let has_unpacked_kwargs = call.arguments.keywords.iter().any(|kw| kw.arg.is_none());
    Some(CallSite {
        callee,
        // Positional: which class the callee names is decided by the binding
        // in force AT THE CALL ([RESOLV-CANONICAL-BINDING]).
        callee_class_site: bindings
            .local_class_definition(&call.func)
            .map(text_range_to_span),
        receiver,
        receiver_class_site,
        args,
        keywords,
        has_unpacked_kwargs,
        span: text_range_to_span(call.range()),
    })
}

// ---------------------------------------------------------------------------
// Annotation analysis helpers
// ---------------------------------------------------------------------------

/// Maps a return annotation expression to its [`ReturnAnnotationKind`].
pub(super) fn expr_to_type_arg(expr: &Expr) -> TypeArg {
    match expr {
        Expr::Name(name) => TypeArg::Simple(name.id.to_string()),
        Expr::Subscript(sub) => {
            let base = expr_simple_name(&sub.value).unwrap_or_default();
            let args: Vec<TypeArg> = match sub.slice.as_ref() {
                Expr::Tuple(tup) => tup.elts.iter().map(expr_to_type_arg).collect(),
                other => vec![expr_to_type_arg(other)],
            };
            TypeArg::Subscript { base, args }
        }
        _ => TypeArg::Simple(expr_simple_name(expr).unwrap_or_default()),
    }
}

/// Delegates to [`super::assert_narrow::collect`].
pub(crate) fn collect_assert_type_calls_from_stmts(
    stmts: &[Stmt],
    source: &str,
) -> Vec<AssertTypeCallInfo> {
    super::assert_narrow::collect(stmts, source)
}

pub(super) fn collect_unhashable_hash_calls_from_stmt(
    stmt: &Stmt,
    non_hashable: &std::collections::HashSet<&str>,
    out: &mut Vec<crate::scope::UnhashableHashCallViolation>,
) {
    match stmt {
        Stmt::Expr(node) => {
            collect_unhashable_hash_calls_from_expr(&node.value, non_hashable, out);
        }
        Stmt::If(node) => {
            for s in &node.body {
                collect_unhashable_hash_calls_from_stmt(s, non_hashable, out);
            }
            for clause in &node.elif_else_clauses {
                for s in &clause.body {
                    collect_unhashable_hash_calls_from_stmt(s, non_hashable, out);
                }
            }
        }
        _ => {}
    }
}
