//! Implements [`directives_assert_type_2`] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STRUCTURAL
//! `directives_assert_type_2`: `assert_type()` type mismatch.
//!
//! `assert_type(expr, Type)` is a static-analysis directive that verifies the
//! inferred type of `expr` equals `Type`. Two judgments feed it
//! ([NARROWPLAN-INTEGRATION] Step 5):
//!
//! - the resolver's flow-narrowed comparison of declared parameter types
//!   (`type_mismatch` on [`basilisk_resolver::AssertTypeCallInfo`]), and
//! - the module's span-indexed oracle — the SAME engine behind hover — for
//!   expressions the resolver cannot type (call results, attributes). The
//!   oracle verdict fires only when both sides are fully known and provably
//!   DISJOINT (neither assignable to the other), so spelling variance and
//!   literal widening can never manufacture a false positive
//!   ([CHKARCH-CONFORMANCE-MODE]).
//!
//! ```python
//! from typing import assert_type
//!
//! def f(a: int | str) -> None:
//!     assert_type(a, int)  # E — int | str is not int
//! ```

use basilisk_resolver::ResolvedModule;
use ruff_python_ast::Expr;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::oracle::ModuleOracle;
use crate::types::InferredType;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "directives_assert_type_2",
    docs_url: "https://www.basilisk-python.dev/errors/directives_assert_type_2",
};

/// Emits `directives_assert_type_2` when `assert_type(expr, T)` has a detectable type mismatch.
pub(crate) struct AssertTypeMismatch;

impl Rule for AssertTypeMismatch {
    fn check(
        &self,
        module: &ResolvedModule,
        ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        super::check_with_own_types(self, module, ctx, diagnostics);
    }

    fn check_with_types(
        &self,
        module: &ResolvedModule,
        types: &super::shared::module_types::ModuleTypes<'_>,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for call in module.assert_type_calls.iter().filter(|c| c.arg_count == 2) {
            if call.type_mismatch {
                let actual = call.actual_type.as_deref().unwrap_or("unknown");
                let expected = call.expected_type.as_deref().unwrap_or("unknown");
                diagnostics.push(mismatch_diagnostic(actual, expected, call.span, module));
                continue;
            }
            // The resolver compared declared types; when it typed the value it
            // has already answered.
            if call.actual_type.is_some() {
                continue;
            }
            if let Some((actual, expected)) = oracle_disjoint_verdict(types, module, call.span) {
                diagnostics.push(mismatch_diagnostic(
                    &actual.to_string(),
                    &expected.to_string(),
                    call.span,
                    module,
                ));
            }
        }
    }
}

/// The engine's verdict on one `assert_type(expr, T)` call: `Some((actual,
/// expected))` iff the value is a call to a module-level FUNCTION with a
/// declared return, and that return and the resolved `T` are both fully known
/// and PROVABLY DISJOINT. Anything less abstains — a class constructor's
/// result may be reshaped by `__new__`, a metaclass, or a descriptor, none of
/// which the engine's class/instance conflation models
/// ([CHKARCH-CONFORMANCE-MODE]).
fn oracle_disjoint_verdict(
    types: &super::shared::module_types::ModuleTypes<'_>,
    module: &ResolvedModule,
    span: basilisk_resolver::Span,
) -> Option<(InferredType, InferredType)> {
    let oracle = types.oracle()?;
    let resolver = types.annotations()?;
    let (value, expected_expr) = assert_type_arguments(oracle, span)?;
    let Expr::Call(value_call) = value else {
        return None;
    };
    let Expr::Name(callee) = value_call.func.as_ref() else {
        return None;
    };
    let is_module_function = module
        .functions
        .iter()
        .any(|function| function.class_name.is_none() && function.name == callee.id.as_str());
    if !is_module_function {
        return None;
    }
    let value_range = ruff_text_size::Ranged::range(value);
    let actual = oracle.synth_span(basilisk_resolver::Span::from(value_range))?;
    let expected = resolver.resolve(expected_expr);
    let both_known =
        crate::expr_type::is_fully_known(&actual) && crate::expr_type::is_fully_known(&expected);
    // BOTH sides must ground every nominal leaf: an unexpanded `TypeVar`
    // (`Named("T")`) is fully "known" structurally but is a question, not an
    // answer, and judging it would fire on every generic call.
    let both_grounded = grounded(resolver, &actual) && grounded(resolver, &expected);
    let disjoint = !actual.is_assignable_to(&expected) && !expected.is_assignable_to(&actual);
    (both_known && both_grounded && disjoint).then_some((actual, expected))
}

/// The two argument nodes of the `assert_type` call occupying `span`.
fn assert_type_arguments<'m>(
    oracle: &ModuleOracle<'m>,
    span: basilisk_resolver::Span,
) -> Option<(&'m Expr, &'m Expr)> {
    let Expr::Call(call) = oracle.expr(span)? else {
        return None;
    };
    let value = call.arguments.args.first()?;
    let expected = call.arguments.args.get(1)?;
    Some((value, expected))
}

/// Every nominal leaf of `expected` resolves to a class this module grounds —
/// an unresolved spelling is a question, not an answer.
fn grounded(resolver: &crate::annotation::AnnotationResolver<'_>, ty: &InferredType) -> bool {
    match ty {
        InferredType::Named(name) => resolver.is_grounded_name(name),
        InferredType::List(inner) | InferredType::Set(inner) | InferredType::Optional(inner) => {
            grounded(resolver, inner)
        }
        InferredType::Dict(key, value) => grounded(resolver, key) && grounded(resolver, value),
        InferredType::Tuple(items) | InferredType::Union(items) => {
            items.iter().all(|item| grounded(resolver, item))
        }
        _ => true,
    }
}

/// The one diagnostic shape both judgment paths share.
fn mismatch_diagnostic(
    actual: &str,
    expected: &str,
    span: basilisk_resolver::Span,
    module: &ResolvedModule,
) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Type mismatch in `assert_type()`: expression has type `{actual}` but expected `{expected}`"
        ),
        span,
        &module.path,
        Some("The type of the expression does not match the declared expected type".to_owned()),
        Some("assert_type(expr, T) requires the inferred type of expr to be exactly T".to_owned()),
    )
}
