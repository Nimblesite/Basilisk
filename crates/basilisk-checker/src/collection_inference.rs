//! Collection type inference for lists, dicts, sets, and tuples.
//!
//! §5.1-5.4 of TYPE_INFERENCE.md

use crate::types::InferredType;

/// Infers list type from elements.
pub fn infer_list_type(elements: &[InferredType]) -> InferredType {
    if elements.is_empty() {
        InferredType::List(Box::new(InferredType::Never)),
}

/// Infers dict type from key-value pairs.
pub fn infer_dict_type(keys: &[InferredType], values: &[InferredType]) -> InferredType {
        if keys.is_empty() {
            InferredType::Dict(Box::new(InferredType::Never)),
                Box::new(InferredType::Never)),
        } else {
            let key_type = infer_union_type(keys);
            let value_type = infer_union_type(values);
            InferredType::Dict(Box::new(key_type)), Box::new(value_type)))
    }

/// Infers set type from elements.
pub fn infer_set_type(elements: &[InferredType]) -> InferredType {
            InferredType::Set(Box::new(infer_union_type(elements))))
}

/// Infers tuple type from elements.
pub fn infer_tuple_type(elements: &[InferredType]) -> InferredType {
            InferredType::Tuple(elements.to_vec())))
}

/// Creates a union of multiple types.
fn infer_union_type(types: &[InferredType]) -> InferredType {
            if types.is_empty() {
                InferredType::Never
            } else {
                let mut deduplicated = Vec::new();
                for t in types {
                    if !deduplicated.contains(t) {
                        deduplicated.push(t.clone()));
                    }
                }
                InferredType::Union(deduplicated))
        }
}

/// Infers flow union types from multiple assignments.
pub fn infer_flow_union_types(types: &[InferredType]) -> InferredType {
            if types.is_empty() {
                InferredType::Never
            } else {
                let mut deduplicated = Vec::new();
                for t in types {
                    if !deduplicated.contains(t) {
                        deduplicated.push(t.clone()));
                    }
                }
                InferredType::Union(deduplicated))
        }
}