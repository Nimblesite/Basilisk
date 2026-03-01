//! Type inference engine for Basilisk.

use basilisk_resolver::{RhsKind, VariableInfo};
use crate::types::InferredType;

/// Infers the type of a right-hand-side expression.
#[must_use]
pub fn infer_rhs(rhs: &RhsKind) -> InferredType {
    match rhs {
        RhsKind::IntLiteral => InferredType::Int,
        RhsKind::FloatLiteral => InferredType::Float,
        RhsKind::StrLiteral => InferredType::Str,
        RhsKind::BoolLiteral => InferredType::Bool,
        RhsKind::BytesLiteral => InferredType::Bytes,
        RhsKind::NoneValue => InferredType::None_,
        RhsKind::EmptyList => InferredType::List(Box::new(InferredType::Never)),
        RhsKind::EmptyDict => InferredType::Dict(
            Box::new(InferredType::Never), 
            Box::new(InferredType::Never)
        ),
        RhsKind::CallExpr | RhsKind::TypeCall | RhsKind::Other => InferredType::Unknown,
    }
}

/// Checks if a variable assignment is valid given its annotation and inferred RHS type.
///
/// # Errors
///
/// Returns an error if the RHS type cannot be inferred (i.e., it is `Unknown`).
pub fn check_annotated_variable(var_info: &VariableInfo) -> Result<(), String> {
    if var_info.has_annotation {
        let rhs_type = infer_rhs(&var_info.rhs_kind);
        
        // For now, we'll return an error if the RHS type is Unknown
        // In a real implementation, we would check assignability against the annotation
        if matches!(rhs_type, InferredType::Unknown) {
            return Err("RHS type cannot be inferred".to_string());
        }
    }
    
    Ok(())
}

/// Infers the type for a variable based on its RHS kind and annotation.
#[must_use]
pub fn infer_variable_type(var_info: &VariableInfo) -> InferredType {
    // If there's an annotation, we need to check assignability
    // For now, we just return the inferred type
    // In a full implementation, we would validate against the annotation
    infer_rhs(&var_info.rhs_kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use basilisk_resolver::Span;

    #[test]
    fn test_infer_rhs() {
        assert_eq!(infer_rhs(&RhsKind::IntLiteral), InferredType::Int);
        assert_eq!(infer_rhs(&RhsKind::FloatLiteral), InferredType::Float);
        assert_eq!(infer_rhs(&RhsKind::StrLiteral), InferredType::Str);
        assert_eq!(infer_rhs(&RhsKind::BoolLiteral), InferredType::Bool);
        assert_eq!(infer_rhs(&RhsKind::BytesLiteral), InferredType::Bytes);
        assert_eq!(infer_rhs(&RhsKind::NoneValue), InferredType::None_);
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
        assert_eq!(infer_rhs(&RhsKind::CallExpr), InferredType::Unknown);
        assert_eq!(infer_rhs(&RhsKind::TypeCall), InferredType::Unknown);
        assert_eq!(infer_rhs(&RhsKind::Other), InferredType::Unknown);
    }

    #[test]
    fn test_check_annotated_variable() {
        let var_info = VariableInfo {
            name: "x".to_string(),
            name_span: Span { start: 0, end: 1 },
            has_annotation: false,
            rhs_kind: RhsKind::IntLiteral,
            annotation_span: None,
            rhs_span: Some(Span { start: 4, end: 6 }),
        };
        
        assert!(check_annotated_variable(&var_info).is_ok());
    }

    #[test]
    fn test_infer_variable_type() {
        let var_info = VariableInfo {
            name: "x".to_string(),
            name_span: Span { start: 0, end: 1 },
            has_annotation: false,
            rhs_kind: RhsKind::IntLiteral,
            annotation_span: None,
            rhs_span: Some(Span { start: 4, end: 6 }),
        };
        
        assert_eq!(infer_variable_type(&var_info), InferredType::Int);
    }
}