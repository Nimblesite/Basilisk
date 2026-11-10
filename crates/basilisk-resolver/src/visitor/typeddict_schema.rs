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

use crate::scope::ClassInfo;

use super::core::source_slice_span;

/// Maximum inheritance depth walked before bailing out (cycle guard).
const MAX_DEPTH: u32 = 64;

/// One field of a `TypedDict`'s effective (post-inheritance) schema.
pub(super) struct EffectiveField<'a> {
    /// The field name.
    pub name: &'a str,
    /// Raw annotation text of the most-derived declaration, if available. The
    /// text retains any `Required`/`NotRequired` wrapper, so downstream
    /// required-ness checks read the qualifier straight from it.
    pub annotation: Option<&'a str>,
    /// `true` when the most-derived declaration wraps the field in `ReadOnly`.
    pub readonly: bool,
}

/// Compute the effective field set for `class`, merging fields declared on the
/// class itself with those inherited from every transitive base. The
/// most-derived declaration of a field wins (a subclass redeclaration shadows
/// the inherited one), which carries the redeclared `ReadOnly` / required-ness
/// through to the consuming rules.
pub(super) fn effective_fields<'a>(
    class: &'a ClassInfo,
    class_map: &HashMap<&'a str, &'a ClassInfo>,
    source: &'a str,
) -> Vec<EffectiveField<'a>> {
    let mut seen: HashMap<&'a str, EffectiveField<'a>> = HashMap::new();
    let mut order: Vec<&'a str> = Vec::new();
    collect_fields(class, class_map, source, &mut seen, &mut order, 0);
    order
        .into_iter()
        .filter_map(|name| seen.remove(name))
        .collect()
}

/// Walk `class` then its bases, inserting each annotated field the first time it
/// is seen. Because the leaf class is visited before its bases, the first
/// insertion is always the most-derived declaration.
fn collect_fields<'a>(
    class: &'a ClassInfo,
    class_map: &HashMap<&'a str, &'a ClassInfo>,
    source: &'a str,
    seen: &mut HashMap<&'a str, EffectiveField<'a>>,
    order: &mut Vec<&'a str>,
    depth: u32,
) {
    if depth >= MAX_DEPTH {
        return;
    }
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
    for base in &class.bases {
        if let Some(base_class) = class_map.get(base.as_str()) {
            collect_fields(base_class, class_map, source, seen, order, depth + 1);
        }
    }
}
