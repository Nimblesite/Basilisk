//! Implements [LINESCANPLAN-AST-MIGRATION]. See
//! docs/plans/CHECKER-ELIMINATE-LINE-SCANNING-PLAN.md#LINESCANPLAN-AST-MIGRATION
//!
//! The ONE answer to "does this module-level name hold a runtime value rather
//! than a type?".
//!
//! Several rules need to reject a name used where a type expression belongs.
//! The tempting shortcut is a spelling heuristic — "lower case means runtime",
//! "capitalised means a type" — which is not in the typing spec and which only
//! ever appeared to work because the conformance fixtures happen to follow
//! PEP 8 ([CHKARCH-CONFORMANCE-MODE], issue #408). The real question is
//! structural: an unannotated module assignment binds a type when its RHS is a
//! type expression or a recognised type constructor, and binds a value
//! otherwise.

use std::collections::HashSet;

use basilisk_resolver::{ResolvedModule, RhsKind};
use ruff_python_ast::Expr;

use crate::annotation::AnnotationResolver;

use super::type_expr::{annotation_is_type_alias, is_type_expression, ExprIndex};
use super::{StringPolicy, TypeExprJudge};

/// Names this module binds to recognised type constructors — `TypeVar`,
/// `ParamSpec`, `TypeVarTuple`, `NewType`, `TypedDict`, `NamedTuple`, and
/// `TypeAliasType` calls all produce types even though their RHS is a call.
pub(crate) fn type_constructor_names(module: &ResolvedModule) -> HashSet<&str> {
    let mut names: HashSet<&str> = module
        .typevar_calls
        .iter()
        .map(|tv| tv.name.as_str())
        .collect();
    names.extend(module.newtype_calls.iter().map(|n| n.lhs_name.as_str()));
    names.extend(module.typeddict_calls.iter().map(|t| t.lhs_name.as_str()));
    names.extend(module.namedtuple_defs.iter().map(|n| n.lhs_name.as_str()));
    names.extend(
        module
            .type_alias_type_calls
            .iter()
            .map(|t| t.lhs_name.as_str()),
    );
    names
}

/// Module-level names bound to runtime values — the "not a type" set every
/// type-expression judge consults.
///
/// An unannotated assignment is either an implicit alias (its RHS is a type
/// expression), a recognised type-constructor binding, or an ordinary
/// variable. Only the last kind lands here. A second pass follows plain
/// name-to-name assignments (`B = A` where `A` is runtime) to a fixpoint.
pub(crate) fn runtime_value_names(
    module: &ResolvedModule,
    resolver: &AnnotationResolver<'_>,
    index: &ExprIndex<'_>,
) -> HashSet<String> {
    let constructors = type_constructor_names(module);
    let value_judge = TypeExprJudge {
        non_type: &|_| false,
        strings: StringPolicy::RejectValue,
    };
    let mut runtime: HashSet<String> = HashSet::new();
    for var in &module.module_vars {
        if annotation_is_type_alias(resolver, var.annotation_span) {
            continue; // Explicit aliases are judged separately.
        }
        if var.has_annotation
            || constructors.contains(var.name.as_str())
            || var.rhs_kind == RhsKind::TypeCall
        {
            continue;
        }
        let Some(rhs) = var.rhs_span.and_then(|span| index.expr(span)) else {
            continue;
        };
        if !is_type_expression(rhs, &value_judge) {
            let _ = runtime.insert(var.name.clone());
        }
    }
    propagate_runtime_refs(module, index, &mut runtime);
    runtime
}

/// Mark `B = A` runtime when `A` already is, until no assignment changes.
fn propagate_runtime_refs(
    module: &ResolvedModule,
    index: &ExprIndex<'_>,
    runtime: &mut HashSet<String>,
) {
    loop {
        let mut changed = false;
        for var in &module.module_vars {
            if var.has_annotation || runtime.contains(&var.name) {
                continue;
            }
            let Some(Expr::Name(rhs)) = var.rhs_span.and_then(|span| index.expr(span)) else {
                continue;
            };
            if runtime.contains(rhs.id.as_str()) {
                let _ = runtime.insert(var.name.clone());
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}
