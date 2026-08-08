//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Tuple-index checking for `key=` lambda parameters.
//!
//! `sorted(items, key=lambda pair: pair[4])` binds `pair` to an element of
//! `items`. When `items` is provably a literal collection of uniform fixed-length
//! tuples, an out-of-range literal index on the lambda parameter is a static
//! error (GitHub #284 follow-up). Emitted as [`TupleIndexViolation`]s, which the
//! `tuples_index` rule reports.

use ruff_python_ast::visitor::{walk_expr, Visitor};
use ruff_python_ast::{Expr, ExprCall, ExprLambda, Number, Stmt, UnaryOp};
use ruff_text_size::Ranged;

use crate::scope::{FunctionInfo, RhsKind, TupleIndexViolation, VariableInfo};

use super::core::text_range_to_span;

/// Builtins whose `key=` callback receives one element of the first argument
/// (receiver for the `list.sort` method form).
const KEY_CALLEES: &[&str] = &["sorted", "min", "max", "sort"];

/// Collect out-of-range tuple index violations on `key=` lambda parameters.
pub(super) fn collect_key_lambda_tuple_violations(
    stmts: &[Stmt],
    functions: &[FunctionInfo],
    module_vars: &[VariableInfo],
) -> Vec<TupleIndexViolation> {
    let mut collector = KeyLambdaCollector {
        functions,
        module_vars,
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
                    return variable_tuple_len(var);
                }
            }
            for param in &func.parameters {
                if param.name == name {
                    return None;
                }
            }
        }
        self.module_vars
            .iter()
            .find(|v| v.name == name)
            .and_then(variable_tuple_len)
    }
}

/// Tuple length for a variable: its annotation when present, else the
/// uniform tuple length of its literal right-hand side.
fn variable_tuple_len(var: &VariableInfo) -> Option<usize> {
    if var.annotation_span.is_some() {
        return None;
    }
    uniform_tuple_len_from_rhs(&var.rhs_kind)
}

/// Match a call carrying a `key=<lambda>` whose element source is a simple
/// name: `sorted(items, key=...)` (first positional argument) or
/// `items.sort(key=...)` (method receiver). Returns the iterable's name and
/// the lambda.
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
