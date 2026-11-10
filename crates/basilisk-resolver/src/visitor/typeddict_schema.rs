//! Implements [CHKARCH-ARCH-PIPELINE] and the field-merge foundation of
//! [CHKARCH-DIAG-TYPEDDICT-READONLY-INHERITANCE]. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPEDDICT-READONLY-INHERITANCE
//! Effective `TypedDict` schema (post-inheritance field merge).
//!
//! Membership and `extra_items` predicates live in [`crate::scope`] (they only
//! need [`ClassInfo`] and are shared cross-crate). This module adds the one
//! operation that also needs the source text: merging a `TypedDict`'s own
//! fields with those inherited from its transitive bases so that the
//! most-derived declaration of each field — carrying its `ReadOnly` qualifier
//! and effective required-ness — wins.

use std::collections::HashMap;

use crate::scope::{ClassGraph, ClassInfo};

use super::core::source_slice_span;

/// One field of a `TypedDict`'s effective (post-inheritance) schema.
pub(super) struct EffectiveField<'a> {
    /// The field name.
    pub name: &'a str,
    /// Raw annotation text of the most-derived declaration, if available. The
    /// text retains any `Required`/`NotRequired` wrapper, so downstream
    /// required-ness checks read the qualifier straight from it.
    pub annotation: Option<&'a str>,
    /// `true` when the most-derived declaration wraps the field in `ReadOnly`.
    ///
    /// ORPHANED BY A DELETION, NOT UNUSED. Its only reader,
    /// `final_readonly::build_typeddict_readonly_map`, was deleted for keying
    /// the resulting field sets by CLASS NAME. The flag itself is computed
    /// lawfully from `AttributeInfo::is_readonly` over the definition-site
    /// ancestry, and it is the input the rebuild consumes.
    #[expect(
        dead_code,
        reason = "its only reader was deleted for keying read-only field sets by class name; \
                  this flag is the lawful input the identity-keyed rebuild consumes"
    )]
    pub readonly: bool,
}

/// Compute the effective field set for `class`, merging fields declared on the
/// class itself with those inherited from every transitive base. The
/// most-derived declaration of a field wins (a subclass redeclaration shadows
/// the inherited one), which carries the redeclared `ReadOnly` / required-ness
/// through to the consuming rules.
pub(super) fn effective_fields<'a>(
    class: &'a ClassInfo,
    graph: &ClassGraph<'a>,
    source: &'a str,
) -> Vec<EffectiveField<'a>> {
    let mut seen: HashMap<&'a str, EffectiveField<'a>> = HashMap::new();
    let mut order: Vec<&'a str> = Vec::new();
    // `ancestors` yields the class before the classes it derives from, so the
    // first declaration of a field seen is always the most-derived one.
    for ancestor in graph.ancestors(class) {
        collect_fields(ancestor, source, &mut seen, &mut order);
    }
    order
        .into_iter()
        .filter_map(|name| seen.remove(name))
        .collect()
}

/// Insert each annotated field of `class` the first time it is seen.
fn collect_fields<'a>(
    class: &'a ClassInfo,
    source: &'a str,
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
        let annotation = attr
            .annotation_span
            .and_then(|span| source_slice_span(source, span))
            .map(str::trim);
        let _ = seen.insert(
            name,
            EffectiveField {
                name,
                annotation,
                readonly: attr.is_readonly,
            },
        );
        order.push(name);
    }
}
