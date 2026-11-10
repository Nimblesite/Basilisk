//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Tuple-index checking for `key=` lambda parameters.
//!
//! `sorted(items, key=lambda pair: pair[4])` binds `pair` to an element of
//! `items`. When `items` is provably a collection of fixed-length tuples —
//! from an annotation (`list[tuple[str, int, int]]`) or a literal of uniform
//! tuples — an out-of-range literal index on the lambda parameter is a static
//! error (GitHub #284 follow-up). Emitted as [`TupleIndexViolation`]s, which
//! the `tuples_index` rule reports.

use ruff_python_ast::visitor::{walk_expr, Visitor};
use ruff_python_ast::{Expr, ExprCall, ExprLambda, Number, Stmt, UnaryOp};
use ruff_text_size::Ranged;

use crate::scope::{FunctionInfo, RhsKind, Span, TupleIndexViolation, VariableInfo};

use super::core::text_range_to_span;
use super::typeddict::split_top_level_args;

/// Builtins whose `key=` callback receives one element of the first argument
/// (receiver for the `list.sort` method form).
const KEY_CALLEES: &[&str] = &["sorted", "min", "max", "nlargest", "nsmallest", "sort"];

/// Collect out-of-range tuple index violations on `key=` lambda parameters.
pub(super) fn collect_key_lambda_tuple_violations(
    stmts: &[Stmt],
    functions: &[FunctionInfo],
    module_vars: &[VariableInfo],
    source: &str,
) -> Vec<TupleIndexViolation> {
    let mut collector = KeyLambdaCollector {
        functions,
        module_vars,
        source,
        out: Vec::new(),
    };
    for stmt in stmts {
        collector.visit_stmt(stmt);
    }
    collector.out
}

struct KeyLambdaCollector<'a> {
    functions: &'a [FunctionInfo],
    module_vars: &'a [VariableInfo],
    source: &'a str,
    out: Vec<TupleIndexViolation>,
}

impl<'a> Visitor<'a> for KeyLambdaCollector<'a> {
    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Call(call) = expr {
            self.check_call(call);
        }
        walk_expr(self, expr);
    }
}

impl KeyLambdaCollector<'_> {
    fn check_call(&mut self, call: &ExprCall) {
        let Some((iterable_name, lambda)) = key_lambda_target(call) else {
            return;
        };
        let Some(param) = sole_lambda_param(lambda) else {
            return;
        };
        let call_offset = text_range_to_span(call.range()).start_usize();
        let Some(tuple_len) = self.iterable_tuple_len(&iterable_name, call_offset) else {
            return;
        };
        collect_out_of_range_subscripts(&lambda.body, param, tuple_len, &mut self.out);
    }

    /// The fixed tuple length of the elements of `name`, resolved in the
    /// scope surrounding `offset`: enclosing functions innermost-first
    /// (locals, then parameters), then module variables.
    fn iterable_tuple_len(&self, name: &str, offset: usize) -> Option<usize> {
        let mut enclosing: Vec<&FunctionInfo> = self
            .functions
            .iter()
            .filter(|f| f.def_span.start_usize() <= offset && offset < f.def_span.end_usize())
            .collect();
        enclosing.sort_by_key(|f| std::cmp::Reverse(f.def_span.start));

        for func in enclosing {
            for var in func.local_vars.iter().chain(&func.local_unannotated_vars) {
                if var.name == name {
                    return self.variable_tuple_len(var);
                }
            }
            for param in &func.parameters {
                if param.name == name {
                    return self.annotation_tuple_len(param.annotation_span);
                }
            }
        }
        self.module_vars
            .iter()
            .find(|v| v.name == name)
            .and_then(|var| self.variable_tuple_len(var))
    }

    /// Tuple length for a variable: its annotation when present, else the
    /// uniform tuple length of its literal right-hand side.
    fn variable_tuple_len(&self, var: &VariableInfo) -> Option<usize> {
        if var.annotation_span.is_some() {
            return self.annotation_tuple_len(var.annotation_span);
        }
        uniform_tuple_len_from_rhs(&var.rhs_kind)
    }

    /// Tuple length from a container annotation like `list[tuple[str, int]]`.
    fn annotation_tuple_len(&self, span: Option<Span>) -> Option<usize> {
        let ann = span?.slice_source(self.source)?;
        fixed_tuple_len_from_container_annotation(ann)
    }
}

/// Match a call carrying a `key=<lambda>` whose element source is a simple
/// name: `sorted(items, key=...)` (first positional argument) or
/// `items.sort(key=...)` (method receiver). Returns the iterable's name and
/// the lambda.
fn key_lambda_target(call: &ExprCall) -> Option<(String, &ExprLambda)> {
    let lambda = call.arguments.keywords.iter().find_map(|kw| {
        let is_key = kw.arg.as_ref().is_some_and(|a| a.as_str() == "key");
        match (&kw.value, is_key) {
            (Expr::Lambda(lambda), true) => Some(lambda),
            _ => None,
        }
    })?;

    let iterable_name = match call.func.as_ref() {
        Expr::Name(callee) if KEY_CALLEES.contains(&callee.id.as_str()) => {
            match call.arguments.args.first()? {
                Expr::Name(arg) => arg.id.to_string(),
                _ => return None,
            }
        }
        Expr::Attribute(attr) if KEY_CALLEES.contains(&attr.attr.as_str()) => {
            match attr.value.as_ref() {
                Expr::Name(receiver) => receiver.id.to_string(),
                _ => return None,
            }
        }
        _ => return None,
    };
    Some((iterable_name, lambda))
}

/// The lambda's single positional parameter name, or `None` for any other
/// signature shape (a `key=` callback receives exactly one element).
fn sole_lambda_param(lambda: &ExprLambda) -> Option<&str> {
    let params = lambda.parameters.as_deref()?;
    let all_positional = params.posonlyargs.iter().chain(&params.args);
    match (
        all_positional.count(),
        params.vararg.is_none() && params.kwarg.is_none() && params.kwonlyargs.is_empty(),
    ) {
        (1, true) => params
            .posonlyargs
            .iter()
            .chain(&params.args)
            .next()
            .map(|p| p.parameter.name.as_str()),
        _ => None,
    }
}

/// Walk `expr` recording every `param[N]` subscript whose literal index falls
/// outside `[-tuple_len, tuple_len)`.
fn collect_out_of_range_subscripts(
    expr: &Expr,
    param: &str,
    tuple_len: usize,
    out: &mut Vec<TupleIndexViolation>,
) {
    struct SubscriptCollector<'a> {
        param: &'a str,
        tuple_len: usize,
        out: &'a mut Vec<TupleIndexViolation>,
    }
    impl<'a> Visitor<'a> for SubscriptCollector<'_> {
        fn visit_expr(&mut self, expr: &'a Expr) {
            if let Expr::Subscript(sub) = expr {
                if let (Expr::Name(base), Some(index)) =
                    (sub.value.as_ref(), literal_int(&sub.slice))
                {
                    let len = i64::try_from(self.tuple_len).unwrap_or(i64::MAX);
                    if base.id.as_str() == self.param && (index >= len || index < -len) {
                        self.out.push(TupleIndexViolation {
                            span: text_range_to_span(sub.range()),
                            tuple_var_name: self.param.to_owned(),
                            index_value: index,
                            tuple_length: self.tuple_len,
                        });
                    }
                }
            }
            walk_expr(self, expr);
        }
    }
    let mut collector = SubscriptCollector {
        param,
        tuple_len,
        out,
    };
    collector.visit_expr(expr);
}

/// A literal integer index: `3` or `-3`.
pub(super) fn literal_int(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::NumberLiteral(num) => match &num.value {
            Number::Int(value) => value.as_i64(),
            _ => None,
        },
        Expr::UnaryOp(unary) if unary.op == UnaryOp::USub => match unary.operand.as_ref() {
            Expr::NumberLiteral(num) => match &num.value {
                Number::Int(value) => value.as_i64().map(std::ops::Neg::neg),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

/// The uniform fixed tuple length of a literal collection's elements —
/// `[("a", 1, 2), ("b", 3, 4)]` → `Some(3)`. Mixed lengths or non-tuple
/// elements yield `None`.
fn uniform_tuple_len_from_rhs(rhs: &RhsKind) -> Option<usize> {
    let (RhsKind::List(elements) | RhsKind::Set(elements) | RhsKind::Tuple(elements)) = rhs else {
        return None;
    };
    let mut lens = elements.iter().map(|elem| match elem {
        RhsKind::Tuple(items) => Some(items.len()),
        _ => None,
    });
    let first = lens.next()??;
    lens.all(|len| len == Some(first)).then_some(first)
}

/// The fixed tuple length inside a one-argument container annotation:
/// `list[tuple[str, int, int]]` → `Some(3)`. Variadic inner tuples
/// (`tuple[int, ...]`) and non-container bases yield `None`.
fn fixed_tuple_len_from_container_annotation(annotation: &str) -> Option<usize> {
    const ELEMENT_CONTAINERS: &[&str] = &[
        "list",
        "set",
        "frozenset",
        "sequence",
        "iterable",
        "collection",
        "deque",
    ];
    let annotation = annotation.trim();
    let bracket = annotation.find('[')?;
    let base = annotation
        .get(..bracket)?
        .rsplit('.')
        .next()?
        .trim()
        .to_ascii_lowercase();
    if !ELEMENT_CONTAINERS.contains(&base.as_str()) {
        return None;
    }
    let inner = annotation.get(bracket + 1..annotation.len().checked_sub(1)?)?;
    fixed_tuple_len(inner.trim())
}

/// The length of a fixed-size tuple annotation: `tuple[str, int]` → `Some(2)`.
///
/// Variadic (`tuple[int, ...]`) and PEP 646 unpacked (`tuple[int, *Ts]`,
/// `tuple[int, *tuple[str, ...]]`) forms have no fixed length and yield `None`.
pub(super) fn fixed_tuple_len(annotation: &str) -> Option<usize> {
    let inner = annotation
        .strip_prefix("tuple[")
        .or_else(|| annotation.strip_prefix("Tuple["))?
        .strip_suffix(']')?;
    let elements = split_top_level_args(inner);
    if elements
        .iter()
        .any(|e| e.trim() == "..." || e.trim().starts_with('*'))
    {
        return None;
    }
    match elements.as_slice() {
        [] => Some(0),
        [only] if only.trim().is_empty() => Some(0),
        _ => Some(elements.len()),
    }
}
