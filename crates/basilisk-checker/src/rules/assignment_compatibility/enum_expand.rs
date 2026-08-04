//! Implements the enum literal expansion equivalence of
//! [TYPEINF-SUBTYPING-UNION]. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-SUBTYPING-UNION
//!
//! An enum type is equivalent to the union of literals of all its members, so
//! `Answer` is assignable to `Literal[Answer.Yes, Answer.No]` exactly when
//! `Yes`/`No` are ALL of `Answer`'s members (GitHub #374). Partial member
//! unions stay errors.

use std::collections::{HashMap, HashSet};

use basilisk_resolver::{AttributeInfo, ResolvedModule};

use crate::rules::guards::is_enum_class;
use crate::types::InferredType;

/// Member names (lowercase) for every enum class in a module, keyed by the
/// lowercase class name — matching the case-folded `InferredType::Named`
/// spellings produced by annotation parsing.
pub(super) type EnumMembers = HashMap<String, Vec<String>>;

/// Build the [`EnumMembers`] environment for a module.
pub(super) fn collect_enum_member_sets(module: &ResolvedModule) -> EnumMembers {
    module
        .classes
        .iter()
        .filter(|class| is_enum_class(class))
        .map(|class| {
            let members = class
                .attributes
                .iter()
                .filter(|attr| is_enum_member(attr))
                .map(|attr| attr.name.to_ascii_lowercase())
                .collect();
            (class.name.to_ascii_lowercase(), members)
        })
        .collect()
}

/// A member is an unannotated class-body value assignment that is not a
/// sunder/dunder name and not a `nonmember`/descriptor/lambda value —
/// mirroring the `Enum` metaclass's own member rules.
fn is_enum_member(attr: &AttributeInfo) -> bool {
    let sunder_or_dunder = attr.name.starts_with('_') && attr.name.ends_with('_');
    attr.has_value
        && !attr.has_annotation
        && !attr.rhs_is_nonmember_call
        && !attr.rhs_is_lambda
        && !attr.rhs_is_descriptor_call
        && !sunder_or_dunder
}

/// Returns `true` when `inferred` is an enum type and `declared` is a literal
/// union naming EVERY member of that enum.
pub(super) fn enum_expansion_assignable(
    inferred: &InferredType,
    declared: &InferredType,
    enums: &EnumMembers,
) -> bool {
    let InferredType::Named(enum_name) = inferred else {
        return false;
    };
    let Some(members) = enums.get(enum_name.as_str()) else {
        return false;
    };
    if members.is_empty() {
        return false;
    }
    let arms = match declared {
        InferredType::Union(arms) => arms.as_slice(),
        single => std::slice::from_ref(single),
    };
    let prefix = format!("{enum_name}.");
    let covered: HashSet<&str> = arms
        .iter()
        .filter_map(|arm| match arm {
            InferredType::Named(name) => name.strip_prefix(prefix.as_str()),
            _ => None,
        })
        .collect();
    members
        .iter()
        .all(|member| covered.contains(member.as_str()))
}
