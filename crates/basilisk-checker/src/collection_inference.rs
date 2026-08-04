//! ⚠️ LEGACY — condemned under [TYPEINF-LEGACY]. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-LEGACY.
//!
//! [`RhsKind`]-shape collection inference. NOT the engine — container
//! synthesis lives in [`crate::bidir::BidirEngine`] with deferred
//! generalization ([TYPEINF-COLLECTIONS], [TYPEINF-TARGET-CONSTRAINTS]); no
//! new code may call into this module, and existing consumers are deleted per
//! [NARROWPLAN-INTEGRATION]. The behavior it still carries until then:
//! [TYPEINF-EXCEEDS-CONTAINERS] — union-of-element-types inference is
//! unconditional, no loose mode, no switch.

use crate::inference::infer_rhs;
use crate::types::InferredType;
use basilisk_resolver::RhsKind;

/// Infers the type of a list literal from its element [`RhsKind`]s.
// Implements [TYPEINF-COLLECTIONS-LISTS]
#[must_use]
pub fn infer_list_type(elements: &[RhsKind]) -> InferredType {
    if elements.is_empty() {
        return InferredType::List(Box::new(InferredType::Never));
    }
    let elem_type = elements
        .iter()
        .map(infer_rhs)
        .fold(InferredType::Never, InferredType::union);
    InferredType::List(Box::new(elem_type))
}

/// Infers the type of a dict literal from key-value [`RhsKind`] pairs.
// Implements [TYPEINF-COLLECTIONS-DICTS]
#[must_use]
pub fn infer_dict_type(pairs: &[(RhsKind, RhsKind)]) -> InferredType {
    if pairs.is_empty() {
        return InferredType::Dict(Box::new(InferredType::Never), Box::new(InferredType::Never));
    }
    let key_type = pairs
        .iter()
        .map(|(k, _)| infer_rhs(k))
        .fold(InferredType::Never, InferredType::union);
    let val_type = pairs
        .iter()
        .map(|(_, v)| infer_rhs(v))
        .fold(InferredType::Never, InferredType::union);
    InferredType::Dict(Box::new(key_type), Box::new(val_type))
}

/// Infers the type of a set literal from its element [`RhsKind`]s.
// Implements [TYPEINF-COLLECTIONS-SETS]
#[must_use]
pub fn infer_set_type(elements: &[RhsKind]) -> InferredType {
    if elements.is_empty() {
        return InferredType::Set(Box::new(InferredType::Never));
    }
    let elem_type = elements
        .iter()
        .map(infer_rhs)
        .fold(InferredType::Never, InferredType::union);
    InferredType::Set(Box::new(elem_type))
}

/// Infers the type of a collection (list or set) from its element [`RhsKind`]s.
#[must_use]
pub fn infer_collection_type(elements: &[RhsKind]) -> InferredType {
    infer_list_type(elements)
}

/// Infers the type of a tuple literal (each element typed independently).
#[must_use]
pub fn infer_tuple_type(elements: &[RhsKind]) -> InferredType {
    InferredType::Tuple(elements.iter().map(infer_rhs).collect())
}
