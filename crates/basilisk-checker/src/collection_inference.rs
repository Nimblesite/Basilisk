//! Collection type inference for lists, dicts, sets, and tuples.
//!
//! §5.1-5.4 of TYPE_INFERENCE.md

use basilisk_resolver::{RhsKind, ResolvedModule, Span};
use crate::types::InferredType;
use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0022",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0022",
};

/// Collection type inference engine.
pub(crate) struct CollectionInference;

impl CollectionInference {
    /// Infers the type of a collection literal based on its elements.
    pub fn infer_collection_type(&self, rhs_kind: &RhsKind) -> InferredType {
        match rhs_kind {
            RhsKind::EmptyList => InferredType::List(Box::new(InferredType::Never)),
            RhsKind::EmptyDict => InferredType::Dict(
                Box::new(InferredType::Never)),
                Box::new(InferredType::Never)),
            RhsKind::IntLiteral => InferredType::Int,
            RhsKind::FloatLiteral => InferredType::Float,
            RhsKind::StrLiteral => InferredType::Str,
            RhsKind::BoolLiteral => InferredType::Bool,
            RhsKind::BytesLiteral => InferredType::Bytes,
            RhsKind::NoneValue => InferredType::None_,
            RhsKind::CallExpr => InferredType::Unknown,
            RhsKind::Other => InferredType::Unknown,
            _ => InferredType::Unknown,
        }
    }

    /// Infers list type from elements.
    pub fn infer_list_type(&self, elements: &[InferredType]) -> InferredType {
        if elements.is_empty() {
            InferredType::List(Box::new(InferredType::Never)),
    }

    /// Infers dict type from key-value pairs.
    pub fn infer_dict_type(&self, keys: &[InferredType], values: &[InferredType]) -> InferredType {
        if elements.is_empty() {
            InferredType::List(Box::new(InferredType::Never)),
        } else {
            let element_type = self.union_types(elements);
            InferredType::List(Box::new(element_type)))
    }

    /// Infers set type from elements.
    pub fn infer_set_type(&self, elements: &[InferredType]) -> InferredType {
            InferredType::Set(Box::new(self.union_types(elements))))
        }
    }

    /// Infers tuple type from elements.
    pub fn infer_tuple_type(&self, elements: &[InferredType]) -> InferredType {
            InferredType::Tuple(elements.to_vec())))
    }


    /// Creates a union of multiple types.
    fn union_types(&self, types: &[InferredType]) -> InferredType {
            if types.is_empty() {
                InferredType::Never
            } else {
                let mut flattened = Vec::new();
                for t in types {
                    match t {
                        InferredType::Union(more_types)) => {
                    flattened.extend(more_types);
                }
                _ => {
                    flattened.push(t.clone()));
                }
            }
            InferredType::Union(flattened))
        }
    }
}

impl Rule for CollectionInference {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Collection inference will be integrated with variable inference
        // For now, this is a placeholder for the collection inference logic
    }
}