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

use crate::types::InferredType;

/// Member names (lowercase) for every enum class in a module, keyed by the
/// lowercase class name. Both sides of a comparison are folded on the way in
/// ([`enum_expansion_assignable`]), so the table reads the same whether the
/// `Named` spelling came from the [TYPEINF-ANNOTATION-RESOLUTION] cascade —
/// which preserves a class's real case — or from the legacy case-folding
/// annotation parser it replaces.
pub(super) type EnumMembers = HashMap<String, Vec<String>>;

/// Build the [`EnumMembers`] environment for a module.
pub(super) fn collect_enum_member_sets(module: &ResolvedModule) -> EnumMembers {
    module
        .classes
        .iter()
        .filter(|class| class.is_enum)
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
        && attr.rhs_descriptor.is_none()
        && !sunder_or_dunder
}

/// Returns `true` when `inferred` is an enum type and `declared` is a literal
/// union naming EVERY member of that enum.
#[expect(
    dead_code,
    reason = "orphaned by the deletion of the spelling-keyed `check_vars` pipeline; retained as the map for the identity-keyed rebuild ([ASTREBUILD-PHASE-TYPEEXPR])"
)]
pub(super) fn enum_expansion_assignable(
    inferred: &InferredType,
    declared: &InferredType,
    enums: &EnumMembers,
) -> bool {
    let InferredType::Named(spelling) = inferred else {
        return false;
    };
    let enum_name = spelling.to_ascii_lowercase();
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
    let covered: HashSet<String> = arms
        .iter()
        .filter_map(|arm| match arm {
            InferredType::Named(name) => name
                .to_ascii_lowercase()
                .strip_prefix(prefix.as_str())
                .map(str::to_owned),
            _ => None,
        })
        .collect();
    members
        .iter()
        .all(|member| covered.contains(member.as_str()))
}
