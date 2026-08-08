//! Implements helpers for [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Shared helper functions used across multiple type checking rules.
//!
//! Consolidated from duplicated implementations in individual rule modules
//! to eliminate code duplication and improve maintainability.

mod class_walks;
pub(crate) mod judge;
pub(crate) mod module_types;
pub(crate) mod oracle;
pub(crate) mod returns_judge;
mod runtime_names;
mod type_expr;
#[expect(
    dead_code,
    reason = "AST scaffolding preserved for its rebuilt consumer ([ASTREBUILD-PHASE-RESOLVER]); the text-matched caller was deleted under [ASTREBUILD-LAW]"
)]
pub(crate) mod typing_form;

pub(crate) use class_walks::{class_name_map, class_or_base_matches, method_name_map};
pub(crate) use runtime_names::{runtime_value_names, type_constructor_names};
pub(crate) use type_expr::{
    annotation_is_type_alias, is_type_expression, ExprIndex, StringPolicy, TypeExprJudge,
};

use std::collections::HashSet;

use crate::types::InferredType;
use basilisk_parser::ParsedModule;
use basilisk_resolver::{ResolvedModule, TypeVarCallInfo};
use ruff_python_ast::{self as ast, Expr};

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Return the resolved module's AST, parsing it once and sharing the result.
///
/// Every `Rule::check` implementation needs the AST and silently bails on parse
/// errors (those are reported separately as `BSK-0000`). Backed by the module's
/// [`LazyAst`](basilisk_resolver::LazyAst) cache, so the first rule to ask parses
/// the source and every later rule reuses it — a file is parsed once, not once
/// per parsing rule.
pub(crate) fn parse_module(module: &ResolvedModule) -> Option<&ParsedModule> {
    module.lazy_ast.get_or_parse(&module.source, &module.path)
}

// ---------------------------------------------------------------------------
// TypeVar helpers
// ---------------------------------------------------------------------------

/// Collect the names of every `TypeVarTuple` declared in the module.
///
/// The returned set borrows from the slice; both must outlive the set.
pub(crate) fn typevar_tuple_names(typevar_calls: &[TypeVarCallInfo]) -> HashSet<&str> {
    typevar_calls
        .iter()
        .filter(|tv| tv.is_typevartuple)
        .map(|tv| tv.name.as_str())
        .collect()
}

// ---------------------------------------------------------------------------
// Expression helpers
// ---------------------------------------------------------------------------

/// Extract the simple name from a `Name` expression.
pub(crate) fn expr_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(n) => Some(n.id.as_str()),
        _ => None,
    }
}

/// Infer the concrete type of a literal expression.
pub(crate) fn infer_expr_literal_type(expr: &Expr) -> Option<&'static str> {
    match expr {
        Expr::NumberLiteral(n) => match &n.value {
            ast::Number::Int(_) => Some("int"),
            ast::Number::Float(_) => Some("float"),
            ast::Number::Complex { .. } => Some("complex"),
        },
        Expr::StringLiteral(_) => Some("str"),
        Expr::BytesLiteral(_) => Some("bytes"),
        Expr::BooleanLiteral(_) => Some("bool"),
        Expr::NoneLiteral(_) => Some("None"),
        _ => None,
    }
}

/// Generic parameter names of a class definition: its PEP 695 type parameters.
pub(crate) fn class_generic_param_names(cls: &ruff_python_ast::StmtClassDef) -> Vec<String> {
    cls.type_params
        .as_ref()
        .map(|tp| {
            tp.type_params
                .iter()
                .map(|p| p.name().to_string())
                .collect()
        })
        .unwrap_or_default()
}

// `StarParam` — the shared star-parameter slot that carried its annotation
// as TEXT — was deleted under [ASTREBUILD-LAW]: every former consumer now
// models the slot locally with a lowered `TypeNode` payload.

// ---------------------------------------------------------------------------
// Return-type verifiability (shared by E0011 and E0013)
// ---------------------------------------------------------------------------

/// Returns true when a return target depends on the returned expression's
/// **value**, which the kind-only return inference does not have — at the top
/// level or nested inside a union, container, optional, callable, or type-form.
///
/// Verifying a `Literal[v]` target requires the value of the returned
/// expression, but `return True` infers `Bool`, not `Literal[True]`. Such a
/// check is unreliable, so it is skipped.
///
/// A *nominal* target is NOT in this category any more. It used to be: every
/// `InferredType::Named` was treated as unverifiable, which silenced the whole
/// return check for `-> MyClass` and `-> MyAlias`
/// ([#378](https://github.com/Nimblesite/Basilisk/issues/378)). Names now
/// arrive through the annotation cascade
/// ([TYPEINF-ANNOTATION-RESOLUTION](../../../../docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-ANNOTATION-RESOLUTION)),
/// which yields `Named` only for a resolved same-file nominal class and the
/// gradual `Unknown` for anything it cannot resolve — and `Unknown` suppresses
/// through ordinary assignability, with no rule-level skip needed.
///
/// Both E0011 and E0013 gate their assignability check on this so the two
/// sibling rules stay in lock-step.
pub(crate) fn is_value_dependent_target(ty: &InferredType) -> bool {
    match ty {
        InferredType::Literal(_) => true,
        InferredType::Optional(inner)
        | InferredType::List(inner)
        | InferredType::Set(inner)
        | InferredType::TypeForm(inner) => is_value_dependent_target(inner),
        InferredType::Dict(key, value) => {
            is_value_dependent_target(key) || is_value_dependent_target(value)
        }
        InferredType::Union(types) | InferredType::Tuple(types) => {
            types.iter().any(is_value_dependent_target)
        }
        InferredType::Callable(info) => {
            is_value_dependent_target(&info.return_type)
                || info.param_types.iter().any(is_value_dependent_target)
        }
        _ => false,
    }
}
