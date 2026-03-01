//! Collection type inference for lists, dicts, sets, and tuples.
//!
//! §5.1-5.4 of TYPE_INFERENCE.md

use basilisk_resolver::RhsKind;
use crate::types::InferredType;

/// Infers the type of a list literal from its element RhsKinds.
#[must_use]
pub fn infer_list_type(elements: &[RhsKind]) -> InferredType {
    if elements.is_empty() {
        return InferredType::List(Box::new(InferredType::Never));
    }
    let elem_type = elements.iter()
        .map(infer_rhs)
        .fold(InferredType::Never, InferredType::union);
    InferredType::List(Box::new(elem_type)))
}

/// Infers the type of a dict literal.
#[must_use]
pub fn infer_dict_type(pairs: &[(RhsKind, RhsKind)]) -> InferredType {
    if pairs.is_empty() {
        return InferredType::Dict(Box::new(InferredType::Never)), Box::new(InferredType::Never)));
    }
    let key_type = pairs.iter()
        .map(|(k, _)| infer_rhs(k))
        .fold(InferredType::Never, InferredType::union);
    let value_type = pairs.iter()
        .map(|(_, v)| infer_rhs(v))
        .fold(InferredType::Never, InferredType::union);
    InferredType::Dict(Box::new(key_type)), Box::new(value_type)))
}

/// Infers the type of a set literal.
#[must_use]
pub fn infer_set_type(elements: &[RhsKind]) -> InferredType {
    if elements.is_empty() {
        return InferredType::Set(Box::new(InferredType::Never));
    }
    let elem_type = elements.iter()
        .map(infer_rhs)
        .fold(InferredType::Never, InferredType::union);
    InferredType::Set(Box::new(elem_type)))
}

/// Infers the type of a tuple literal (fixed-length, each element typed independently).
#[must_use]
pub fn infer_tuple_type(elements: &[RhsKind]) -> InferredType {
    let elem_types: Vec<InferredType> = elements.iter()
        .map(infer_rhs)
        .collect();
    InferredType::Tuple(elem_types))
}