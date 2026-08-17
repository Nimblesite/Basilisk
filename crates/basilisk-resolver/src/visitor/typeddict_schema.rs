//! Implements [CHKARCH-ARCH-PIPELINE] and the field-merge foundation of
//! [CHKARCH-DIAG-TYPEDDICT-READONLY-INHERITANCE]. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPEDDICT-READONLY-INHERITANCE
//! Effective `TypedDict` schema (post-inheritance field merge).
//!
//! Membership and `extra_items` predicates live in [`crate::scope`] (they only
//! need [`ClassInfo`] and are shared cross-crate). This module adds the field
//! merge: a `TypedDict`'s own fields joined with those inherited from its
//! transitive bases so that the most-derived declaration of each field —
//! carrying its `ReadOnly` qualifier and effective required-ness — wins.
//!
//! Required-ness is resolved here and nowhere downstream, because PEP 655
//! ties it to the DECLARING class: an explicit `Required[...]`/
//! `NotRequired[...]` qualifier decides outright, and an unqualified field
//! takes the `total=` of the class that declares it — so a `total=False`
//! base's fields stay optional inside a `total=True` subclass, and vice
//! versa. Both inputs ([`crate::scope::AttributeInfo::required`] and
//! [`ClassInfo::is_typeddict_total`]) were resolved through the module's
//! bindings at collection time; no text is consulted.

use std::collections::HashMap;

use crate::scope::{ClassGraph, ClassInfo, PrimitiveKind};

/// One field of a `TypedDict`'s effective (post-inheritance) schema.
pub(super) struct EffectiveField<'a> {
    /// The field name.
    pub name: &'a str,
    /// Whether the field must be present: the most-derived declaration's
    /// explicit `Required`/`NotRequired` qualifier, or its declaring class's
    /// `total=` when unqualified (PEP 655).
    pub required: bool,
    /// `true` when the most-derived declaration wraps the field in `ReadOnly`.
    ///
    /// Computed from `AttributeInfo::is_readonly` over the definition-site
    /// ancestry; consumed by `final_readonly::build_typeddict_readonly_map`,
    /// whose field sets are keyed by [`ClassInfo::name_span`].
    pub readonly: bool,
    /// The primitive classes the most-derived declaration accepts, when its
    /// whole annotation is judgeable ([`crate::scope::AttributeInfo::accepted_primitives`]).
    pub accepts: Option<&'a [PrimitiveKind]>,
}

/// Compute the effective field set for `class`, merging fields declared on the
/// class itself with those inherited from every transitive base. The
/// most-derived declaration of a field wins (a subclass redeclaration shadows
/// the inherited one), which carries the redeclared `ReadOnly` /
/// required-ness through to the consuming rules.
pub(super) fn effective_fields<'a>(
    class: &'a ClassInfo,
    graph: &ClassGraph<'a>,
) -> Vec<EffectiveField<'a>> {
    let mut seen: HashMap<&'a str, EffectiveField<'a>> = HashMap::new();
    let mut order: Vec<&'a str> = Vec::new();
    // `ancestors` yields the class before the classes it derives from, so the
    // first declaration of a field seen is always the most-derived one.
    for ancestor in graph.ancestors(class) {
        collect_fields(ancestor, &mut seen, &mut order);
    }
    order
        .into_iter()
        .filter_map(|name| seen.remove(name))
        .collect()
}

/// Insert each annotated field of `class` the first time it is seen.
fn collect_fields<'a>(
    class: &'a ClassInfo,
    seen: &mut HashMap<&'a str, EffectiveField<'a>>,
    order: &mut Vec<&'a str>,
) {
    for attr in &class.attributes {
        if !attr.has_annotation {
            continue;
        }
        let name = attr.name.as_str();
        if seen.contains_key(name) {
            continue;
        }
        let _ = seen.insert(
            name,
            EffectiveField {
                name,
                required: attr.required.unwrap_or(class.is_typeddict_total),
                readonly: attr.is_readonly,
                accepts: attr.accepted_primitives.as_deref(),
            },
        );
        order.push(name);
    }
}
