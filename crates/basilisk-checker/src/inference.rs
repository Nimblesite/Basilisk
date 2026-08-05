//! ⚠️ LEGACY — condemned under [TYPEINF-LEGACY]. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-LEGACY.
//!
//! Syntactic [`RhsKind`] classification. NOT the inference engine — the
//! engine is [`crate::bidir::BidirEngine`] ([TYPEINF-ALGO], [TYPEINF-TARGET]),
//! and no new code may call into this module. Existing consumers are deleted
//! rule-by-rule per the demolition order in [NARROWPLAN-INTEGRATION]
//! (docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-INTEGRATION);
//! this module dies with its last caller. Still referenced for
//! [TYPEINF-VARS-SIMPLE] literal behavior and the [TYPEINF-REQUIRED] /
//! [TYPEINF-EXCEEDS] predicates until then.

use crate::types::InferredType;
use basilisk_resolver::RhsKind;

/// Infers the type of a right-hand-side expression.
#[must_use]
pub fn infer_rhs(rhs: &RhsKind) -> InferredType {
    match rhs {
        RhsKind::IntLiteral => InferredType::Int,
        RhsKind::FloatLiteral => InferredType::Float,
        // PEP 675: a literal expression is provably a LiteralString. Plain
        // `str` remains reserved for dynamic string values, preserving that
        // distinction through container inference.
        RhsKind::StrLiteral => InferredType::LiteralString,
        RhsKind::BoolLiteral => InferredType::Bool,
        RhsKind::BytesLiteral => InferredType::Bytes,
        RhsKind::NoneValue => InferredType::None_,
        RhsKind::EmptyList => InferredType::List(Box::new(InferredType::Never)),
        RhsKind::EmptyDict => {
            InferredType::Dict(Box::new(InferredType::Never), Box::new(InferredType::Never))
        }
        RhsKind::List(elements) => crate::collection_inference::infer_list_type(elements),
        RhsKind::Set(elements) => crate::collection_inference::infer_set_type(elements),
        RhsKind::Dict(pairs) => crate::collection_inference::infer_dict_type(pairs),
        RhsKind::Tuple(elements) => crate::collection_inference::infer_tuple_type(elements),
        // `KnownCall` feeds inferred-type *display* (hover, inlay hints — #253);
        // checker semantics deliberately keep call results Unknown, like `CallExpr`.
        RhsKind::CallExpr | RhsKind::KnownCall(_) | RhsKind::TypeCall | RhsKind::Other => {
            InferredType::Unknown
        }
        RhsKind::Lambda => {
            // Lambda expressions have type Callable[..., Unknown] since we don't know
            // parameter types or return type without analyzing the lambda body
            InferredType::Callable(crate::types::CallableInfo {
                // The gradual tail: the lambda's parameters are not pinned here.
                param_types: crate::types::gradual_params(Vec::new()),
                return_type: Box::new(InferredType::Unknown),
            })
        }
    }
}

/// Returns `true` when the CURRENT engine fully determines a usable declared
/// type from this RHS alone — i.e. [`infer_rhs`] produces a type with no
/// `Unknown`/`Never` component and no widening guess.
///
/// Implements [TYPEINF-EXCEEDS-REQUIRED]: a missing-annotation rule
/// (BSK-0001/BSK-0002) must never fire where this returns `true`, and must
/// keep firing where it returns `false`. The predicate is deliberately exactly
/// as strong as today's inference and no stronger:
///
/// - scalar literals (`int`/`float`/`str`/`bool`/`bytes`) determine their type;
/// - non-empty containers of determining elements determine theirs;
/// - `None` does NOT determine a declared type (`T | None` needs `T`);
/// - empty containers do NOT (element types unknown);
/// - calls, lambdas, names, and arbitrary expressions do NOT
///   ([TYPEINF-EXCEEDS-NOUNKNOWN] keeps them `Unknown`).
#[must_use]
pub fn rhs_fully_determines_type(rhs: &RhsKind) -> bool {
    match rhs {
        RhsKind::IntLiteral
        | RhsKind::FloatLiteral
        | RhsKind::StrLiteral
        | RhsKind::BoolLiteral
        | RhsKind::BytesLiteral => true,
        RhsKind::List(elements) | RhsKind::Set(elements) | RhsKind::Tuple(elements) => {
            !elements.is_empty() && elements.iter().all(rhs_fully_determines_type)
        }
        RhsKind::Dict(pairs) => {
            !pairs.is_empty()
                && pairs
                    .iter()
                    .all(|(k, v)| rhs_fully_determines_type(k) && rhs_fully_determines_type(v))
        }
        RhsKind::NoneValue
        | RhsKind::EmptyList
        | RhsKind::EmptyDict
        | RhsKind::CallExpr
        | RhsKind::KnownCall(_)
        | RhsKind::TypeCall
        | RhsKind::Lambda
        | RhsKind::Other => false,
    }
}

#[cfg(test)]
mod tests {
    use super::rhs_fully_determines_type;
    use basilisk_resolver::RhsKind;

    /// [TYPEINF-EXCEEDS-REQUIRED]: scalar literals fully determine a declared
    /// type; the annotation rules must stay silent on them.
    #[test]
    fn scalar_literals_determine_the_type() {
        for kind in [
            RhsKind::IntLiteral,
            RhsKind::FloatLiteral,
            RhsKind::StrLiteral,
            RhsKind::BoolLiteral,
            RhsKind::BytesLiteral,
        ] {
            assert!(
                rhs_fully_determines_type(&kind),
                "{kind:?} must determine its type"
            );
        }
    }

    /// [TYPEINF-EXCEEDS-REQUIRED]: `None`, empty containers, calls, lambdas,
    /// and arbitrary expressions do NOT determine a usable declared type — the
    /// annotation rules must keep firing there.
    #[test]
    fn non_determining_kinds_keep_the_rules_firing() {
        for kind in [
            RhsKind::NoneValue,
            RhsKind::EmptyList,
            RhsKind::EmptyDict,
            RhsKind::CallExpr,
            RhsKind::KnownCall(Box::new(RhsKind::IntLiteral)),
            RhsKind::TypeCall,
            RhsKind::Lambda,
            RhsKind::Other,
        ] {
            assert!(
                !rhs_fully_determines_type(&kind),
                "{kind:?} must NOT determine a type"
            );
        }
    }

    /// Containers determine their type iff non-empty and every element (or
    /// key/value pair) determines its own.
    #[test]
    fn containers_recurse_and_reject_unknown_elements() {
        assert!(rhs_fully_determines_type(&RhsKind::List(vec![
            RhsKind::IntLiteral,
            RhsKind::IntLiteral,
        ])));
        assert!(rhs_fully_determines_type(&RhsKind::Tuple(vec![
            RhsKind::StrLiteral,
            RhsKind::BoolLiteral,
        ])));
        assert!(rhs_fully_determines_type(&RhsKind::Dict(vec![(
            RhsKind::StrLiteral,
            RhsKind::IntLiteral,
        )])));
        // An uninferable element poisons the whole container.
        assert!(!rhs_fully_determines_type(&RhsKind::List(vec![
            RhsKind::IntLiteral,
            RhsKind::CallExpr,
        ])));
        assert!(!rhs_fully_determines_type(&RhsKind::Dict(vec![(
            RhsKind::StrLiteral,
            RhsKind::Other,
        )])));
        // Empty collections carry no element information.
        assert!(!rhs_fully_determines_type(&RhsKind::List(vec![])));
        assert!(!rhs_fully_determines_type(&RhsKind::Dict(vec![])));
    }
}
