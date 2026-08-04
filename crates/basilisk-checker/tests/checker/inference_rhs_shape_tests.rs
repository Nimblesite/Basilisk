//! Tests for [TYPEINF-VARS-SIMPLE]. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-VARS-SIMPLE.
//!
//! `infer_rhs` maps a resolver-side [`RhsKind`] shape to an [`InferredType`].
//! It is condemned under [TYPEINF-LEGACY] and dies with its last caller; these
//! tests pin the behaviour every remaining caller still depends on so the
//! migration to [`basilisk_checker::bidir::BidirEngine`] is provably
//! behaviour-preserving rather than hopeful.

use basilisk_checker::inference::infer_rhs;
use basilisk_checker::types::InferredType;
use basilisk_resolver::RhsKind;

#[test]
fn infer_rhs_lambda() {
    let result = infer_rhs(&RhsKind::Lambda);
    assert!(matches!(result, InferredType::Callable(_)));
}

#[test]
fn infer_rhs_call_expr() {
    assert!(matches!(
        infer_rhs(&RhsKind::CallExpr),
        InferredType::Unknown
    ));
}

#[test]
fn infer_rhs_type_call() {
    assert!(matches!(
        infer_rhs(&RhsKind::TypeCall),
        InferredType::Unknown
    ));
}

#[test]
fn infer_rhs_other() {
    assert!(matches!(infer_rhs(&RhsKind::Other), InferredType::Unknown));
}

#[test]
fn infer_rhs_empty_list() {
    let result = infer_rhs(&RhsKind::EmptyList);
    assert!(matches!(result, InferredType::List(_)));
}

#[test]
fn infer_rhs_empty_dict() {
    let result = infer_rhs(&RhsKind::EmptyDict);
    assert!(matches!(result, InferredType::Dict(_, _)));
}

#[test]
fn infer_rhs_none() {
    assert!(matches!(
        infer_rhs(&RhsKind::NoneValue),
        InferredType::None_
    ));
}

#[test]
fn infer_rhs_bytes() {
    assert!(matches!(
        infer_rhs(&RhsKind::BytesLiteral),
        InferredType::Bytes
    ));
}

#[test]
fn infer_rhs_bool() {
    assert!(matches!(
        infer_rhs(&RhsKind::BoolLiteral),
        InferredType::Bool
    ));
}
