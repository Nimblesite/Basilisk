//! Tests for Basilisk's type inference engine.

use basilisk_checker::inference::infer_rhs;
use basilisk_resolver::RhsKind;
use basilisk_checker::types::InferredType;

#[test]
fn test_basic_inference() {
    assert_eq!(infer_rhs(&RhsKind::IntLiteral), InferredType::Int);
    assert_eq!(infer_rhs(&RhsKind::StrLiteral), InferredType::Str);
    assert_eq!(infer_rhs(&RhsKind::FloatLiteral), InferredType::Float);
    assert_eq!(infer_rhs(&RhsKind::BoolLiteral), InferredType::Bool);
    assert_eq!(infer_rhs(&RhsKind::BytesLiteral), InferredType::Bytes);
    assert_eq!(infer_rhs(&RhsKind::NoneValue), InferredType::None_);
}

#[test]
fn test_empty_collections() {
    assert_eq!(
        infer_rhs(&RhsKind::EmptyList),
        InferredType::List(Box::new(InferredType::Never))
    );
    assert_eq!(
        infer_rhs(&RhsKind::EmptyDict),
        InferredType::Dict(
            Box::new(InferredType::Never),
            Box::new(InferredType::Never)
        )
    );
}

#[test]
fn test_unknown_types() {
    assert_eq!(infer_rhs(&RhsKind::CallExpr), InferredType::Unknown);
    assert_eq!(infer_rhs(&RhsKind::TypeCall), InferredType::Unknown);
    assert_eq!(infer_rhs(&RhsKind::Other), InferredType::Unknown);
}