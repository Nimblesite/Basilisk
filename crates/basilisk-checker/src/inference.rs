//! The [TYPEINF-REQUIRED] / [TYPEINF-EXCEEDS] determination predicate.
//!
//! NOT the inference engine — the engine is [`crate::bidir::BidirEngine`]
//! ([TYPEINF-ALGO], [TYPEINF-TARGET]). The one predicate here reads the
//! resolver's syntactic [`RhsKind`] classification because its question is
//! syntactic: "does this value's SHAPE alone pin its declared type?" — the
//! gate the missing-annotation rules (BSK-0001/BSK-0002) apply before
//! demanding an annotation. Everything type-valued moved to the engine per
//! [NARROWPLAN-INTEGRATION]; the last shape mapping (`infer_rhs`) is deleted.

use basilisk_resolver::RhsKind;

/// Returns `true` when the value's shape alone fully determines a usable
/// declared type — a type with no `Unknown`/`Never` component and no
/// widening guess.
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
