//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Calls And Reveal visitor functions.

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

use crate::scope::{AssertTypeCallInfo, CallSite, RevealTypeCallInfo, RhsKind, Span, TypeArg};

use super::class_info_ext::expr_simple_name;
use super::core::{classify_rhs, source_slice_range, text_range_to_span, types_match};
use super::type_alias::is_user_defined_type_alias;
use super::typeddict::normalize_type_str;
use super::unhashable::collect_unhashable_hash_calls_from_expr;

pub(super) fn call_func_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(name) => Some(name.id.as_str()),
        Expr::Attribute(attr) => Some(attr.attr.as_str()),
        _ => None,
    }
}

pub(super) fn extract_call_name(expr: &Expr) -> Option<String> {
    if let Expr::Call(call) = expr {
        match call.func.as_ref() {
            Expr::Name(n) => Some(n.id.to_string()),
            Expr::Attribute(a) => Some(a.attr.to_string()),
            _ => None,
        }
    } else {
        None
    }
}

/// Returns `true` when a function body is a pure ellipsis stub (`...`).
///
/// Only `...` — optionally preceded by a docstring — is treated as a stub.
/// `pass` is valid in real function bodies and must not suppress diagnostics.
///
/// These stubs appear in `@overload` signatures, Protocol method declarations,
/// and abstract method placeholders where annotation enforcement should not apply.
pub(super) fn collect_reveal_type_calls(stmts: &[Stmt]) -> Vec<RevealTypeCallInfo> {
    let mut out = Vec::new();
    collect_reveal_type_calls_from_stmts(stmts, &mut out);
    out
}

/// Collect call sites from statements, including those inside function bodies.
pub(super) fn collect_calls_from_stmts(stmts: &[Stmt]) -> Vec<CallSite> {
    let mut out = Vec::new();
    collect_calls_from_stmts_internal(stmts, &mut out);
    out
}

pub(super) fn collect_calls_from_stmts_internal(stmts: &[Stmt], out: &mut Vec<CallSite>) {
    crate::walk_all_stmts(stmts, &mut |stmt| match stmt {
        Stmt::AnnAssign(node) => {
            if let Some(val) = node.value.as_deref() {
                if let Some(site) = call_site_from_expr(val) {
                    out.push(site);
                }
            }
        }
        Stmt::Assign(node) => {
            if let Some(site) = call_site_from_expr(&node.value) {
                out.push(site);
            }
        }
        Stmt::Expr(node) => {
            if let Some(site) = call_site_from_expr(&node.value) {
                out.push(site);
            }
        }
        Stmt::If(node) => {
            if let Some(site) = call_site_from_expr(&node.test) {
                out.push(site);
            }
            for clause in &node.elif_else_clauses {
                if let Some(ref test) = clause.test {
                    if let Some(site) = call_site_from_expr(test) {
                        out.push(site);
                    }
                }
            }
        }
        _ => {}
    });
}

pub(super) fn collect_reveal_type_calls_from_stmts(
    stmts: &[Stmt],
    out: &mut Vec<RevealTypeCallInfo>,
) {
    crate::walk_all_stmts(stmts, &mut |stmt| {
        let Stmt::Expr(node) = stmt else { return };
        let Expr::Call(call) = node.value.as_ref() else {
            return;
        };
        if expr_simple_name(&call.func).is_some_and(|n| n == "reveal_type") {
            out.push(RevealTypeCallInfo {
                arg_count: call.arguments.args.len(),
                span: text_range_to_span(call.range()),
            });
        }
    });
}

/// Extract `Generic[T, ...]` or `Protocol[T, ...]` type parameter names and
/// any non-TypeVar (non-simple-name) argument spans from a class definition.
///
/// Returns `(type_params, non_typevar_arg_spans)`.
pub(super) fn call_site_from_expr(expr: &Expr) -> Option<CallSite> {
    let Expr::Call(call) = expr else { return None };
    let callee = expr_simple_name(&call.func)?;
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

/// Extract [`BaseSubscriptEntry`] items from the base class expressions of a
/// class definition.
///
/// For each base class that is a subscript expression (e.g. `Base[T, int]`),
/// produces an entry with the base name, flat type argument names, rich
/// structured type args, and the source span.
/// Collect every `assert_type(...)` call in `stmts`, applying flow-sensitive
/// narrowing so post-guard assertions compare against the narrowed type.
///
/// Delegates to [`super::assert_narrow`]; see that module for the narrowing
/// rules ([BSK-E0053]).
pub(crate) fn collect_assert_type_calls_from_stmts(
    stmts: &[Stmt],
    source: &str,
) -> Vec<AssertTypeCallInfo> {
    super::assert_narrow::collect(stmts, source)
}

/// Build an [`AssertTypeCallInfo`] from a single `assert_type(...)` call,
/// resolving the first argument's type against the (possibly narrowed)
/// parameter environment `params`.
pub(super) fn build_assert_type_call_info(
    call: &ruff_python_ast::ExprCall,
    actual_type: Option<String>,
    source: &str,
) -> AssertTypeCallInfo {
    let arg_count = call.arguments.args.len();
    let span = text_range_to_span(call.range());

    if arg_count != 2 {
        // Arity error — type mismatch checking is not applicable.
        return AssertTypeCallInfo {
            arg_count,
            span,
            actual_type: None,
            expected_type: None,
            type_mismatch: false,
        };
    }

    let Some(first_arg) = call.arguments.args.first() else {
        return AssertTypeCallInfo {
            arg_count,
            span,
            actual_type: None,
            expected_type: None,
            type_mismatch: false,
        };
    };
    let Some(second_arg) = call.arguments.args.get(1) else {
        return AssertTypeCallInfo {
            arg_count,
            span,
            actual_type: None,
            expected_type: None,
            type_mismatch: false,
        };
    };
    let _ = first_arg;

    // Extract the expected type text from the second argument.
    let expected_type = extract_type_text(second_arg, source);

    // Compare normalized forms.
    // Skip when the actual type is a user-defined type alias that we cannot expand
    // without a full type engine — comparing alias names to their expansions produces
    // false positives (e.g. `GoodTypeAlias1` != `int | str` even though they're equal).
    let type_mismatch = match (&actual_type, &expected_type) {
        (Some(actual), Some(expected)) => {
            !types_match(actual, expected) && !is_user_defined_type_alias(actual)
        }
        _ => false,
    };

    AssertTypeCallInfo {
        arg_count,
        span,
        actual_type,
        expected_type,
        type_mismatch,
    }
}

/// Resolve the static type of `assert_type`'s first argument.
///
/// - If it is a name reference to a known parameter, returns its annotation text (normalized).
/// - If it is a literal, returns the corresponding primitive type name.
/// - Otherwise returns `None`.
pub(super) fn extract_type_text(expr: &Expr, source: &str) -> Option<String> {
    source_slice_range(source, expr.range()).map(normalize_type_str)
}

/// Normalize a type annotation string for comparison.
///
/// Strips outer `Annotated[T, ...]` wrappers, trims whitespace, and collapses
/// internal spacing around `|` union operators.
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
