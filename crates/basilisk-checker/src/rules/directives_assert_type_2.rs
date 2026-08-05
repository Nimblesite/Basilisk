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
            // has already answered — except for a user-defined alias, where it
            // abstains and the tuple-union expansion below may still decide.
            if let Some(actual) = call.actual_type.as_deref() {
                if let Some((actual, expected)) =
                    alias_tuple_union_verdict(types, module, actual, call.span)
                {
                    diagnostics.push(mismatch_diagnostic(&actual, &expected, call.span, module));
                }
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

/// Equivalence verdict for a value declared as a module-level alias of a
/// UNION OF TUPLE TYPES ([LINESCANPLAN-AST-MIGRATION]).
///
/// The resolver abstains on user aliases because names and expansions differ
/// textually; here both sides expand through the module's alias table and
/// compare as canonical member sets. The judgment fires only when every
/// member on BOTH sides is a `tuple[...]` form — tuple structure is rigid
/// (fixed arity, positional element types, PEP 646 unpacks), so two differing
/// canonical sets are provably non-equivalent, which is exactly what
/// `assert_type` rejects. The declared side is the resolver's flow output: a
/// parameter narrowed by an implemented guard no longer reports the alias
/// name and never reaches this comparison.
fn alias_tuple_union_verdict(
    types: &super::shared::module_types::ModuleTypes<'_>,
    module: &ResolvedModule,
    actual: &str,
    span: basilisk_resolver::Span,
) -> Option<(String, String)> {
    let oracle = types.oracle()?;
    let aliases = module_alias_map(module, oracle);
    let alias_rhs = aliases.get(actual.trim())?;

    // An open type variable anywhere makes the comparison a specialization
    // question, not an equivalence one — abstain.
    let type_vars: std::collections::HashSet<&str> = module
        .typevar_calls
        .iter()
        .map(|tv| tv.name.as_str())
        .collect();

    let (_, expected_expr) = assert_type_arguments(oracle, span)?;
    let actual_members = tuple_union_members(&aliases, &type_vars, alias_rhs)?;
    let expected_members = tuple_union_members(&aliases, &type_vars, expected_expr)?;
    (actual_members != expected_members).then(|| {
        (
            actual_members
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(" | "),
            expected_members
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(" | "),
        )
    })
}

/// Module-level type aliases: explicit `TypeAlias` annotations and implicit
/// union-of-types assignments, mapped to their RHS nodes.
fn module_alias_map<'m>(
    module: &'m ResolvedModule,
    oracle: &ModuleOracle<'m>,
) -> std::collections::HashMap<&'m str, &'m Expr> {
    let mut map = std::collections::HashMap::new();
    for var in &module.module_vars {
        let Some(rhs) = var.rhs_span.and_then(|span| oracle.expr(span)) else {
            continue;
        };
        let _ = map.insert(var.name.as_str(), rhs);
    }
    map
}

/// The canonical member set of a type expression, when EVERY member is a
/// builtin `tuple[...]` form after alias expansion. `None` when any member is
/// anything else, or when any member's canonical form references an open type
/// variable — those are questions for specialization, not equivalence.
fn tuple_union_members(
    aliases: &std::collections::HashMap<&str, &Expr>,
    type_vars: &std::collections::HashSet<&str>,
    expr: &Expr,
) -> Option<std::collections::BTreeSet<String>> {
    let mut members = Vec::new();
    collect_union_members(aliases, expr, 0, &mut members)?;
    members
        .into_iter()
        .map(|member| {
            let is_tuple = matches!(
                member,
                Expr::Subscript(subscript)
                    if matches!(subscript.value.as_ref(), Expr::Name(name) if name.id.as_str() == "tuple")
            );
            if !is_tuple || references_open_type(type_vars, member) {
                return None;
            }
            canonical(member)
        })
        .collect()
}

/// Does this expression reference a declared type variable anywhere?
fn references_open_type(type_vars: &std::collections::HashSet<&str>, expr: &Expr) -> bool {
    match expr {
        Expr::Name(name) => type_vars.contains(name.id.as_str()),
        Expr::Subscript(subscript) => {
            references_open_type(type_vars, &subscript.value)
                || references_open_type(type_vars, &subscript.slice)
        }
        Expr::Tuple(tuple) => tuple
            .elts
            .iter()
            .any(|elt| references_open_type(type_vars, elt)),
        Expr::BinOp(binop) => {
            references_open_type(type_vars, &binop.left)
                || references_open_type(type_vars, &binop.right)
        }
        Expr::Starred(starred) => references_open_type(type_vars, &starred.value),
        _ => false,
    }
}

/// Flatten a (possibly alias-named) union expression into its member nodes.
/// `None` when an alias chain is too deep or a member cannot be resolved.
fn collect_union_members<'m>(
    aliases: &std::collections::HashMap<&str, &'m Expr>,
    expr: &'m Expr,
    depth: usize,
    out: &mut Vec<&'m Expr>,
) -> Option<()> {
    if depth > 8 {
        return None;
    }
    match expr {
        Expr::BinOp(binop) if binop.op == ruff_python_ast::Operator::BitOr => {
            collect_union_members(aliases, &binop.left, depth + 1, out)?;
            collect_union_members(aliases, &binop.right, depth + 1, out)
        }
        Expr::Name(name) => {
            if let Some(rhs) = aliases.get(name.id.as_str()) {
                collect_union_members(aliases, rhs, depth + 1, out)
            } else {
                out.push(expr);
                Some(())
            }
        }
        other => {
            out.push(other);
            Some(())
        }
    }
}

/// A canonical, formatting-independent rendering of a type expression, with
/// union operands sorted so `int | str` and `str | int` agree. `None` for
/// node kinds the canonicalizer does not model.
fn canonical(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.to_string()),
        Expr::NoneLiteral(_) => Some("None".to_owned()),
        Expr::EllipsisLiteral(_) => Some("...".to_owned()),
        Expr::StringLiteral(lit) => Some(format!("'{}'", lit.value.to_str())),
        Expr::Attribute(attr) => Some(format!("{}.{}", canonical(&attr.value)?, attr.attr)),
        Expr::Starred(starred) => Some(format!("*{}", canonical(&starred.value)?)),
        Expr::Subscript(subscript) => {
            let head = canonical(&subscript.value)?;
            let args: Option<Vec<String>> = match subscript.slice.as_ref() {
                Expr::Tuple(tuple) => tuple.elts.iter().map(canonical).collect(),
                single => Some(vec![canonical(single)?]),
            };
            Some(format!("{head}[{}]", args?.join(", ")))
        }
        Expr::BinOp(binop) if binop.op == ruff_python_ast::Operator::BitOr => {
            let mut operands = [canonical(&binop.left)?, canonical(&binop.right)?];
            operands.sort_unstable();
            Some(operands.join(" | "))
        }
        _ => None,
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
