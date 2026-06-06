//! Implements [CHKARCH-ARCH-PIPELINE] and the transitive-recognition foundation
//! of [CHKARCH-DIAG-TYPEDDICT-READONLY-INHERITANCE]. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPEDDICT-READONLY-INHERITANCE
//! `TypedDict` membership primitives shared across the resolver and checker.
//!
//! [`ClassInfo::is_typed_dict`] is only `true` when a class names `TypedDict`
//! *directly* among its bases. A subclass of another `TypedDict` is still a
//! `TypedDict` but carries `is_typed_dict == false`. Rules that decide
//! membership by that flag therefore overlook transitive subclasses. These
//! helpers walk the full base chain so every consumer agrees on what is a
//! `TypedDict`.

use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;

use super::class_types::ClassInfo;

/// Maximum inheritance depth walked before bailing out. Guards against cyclic
/// `bases` (illegal Python, but must not hang the resolver).
const MAX_DEPTH: u32 = 64;

/// Build a `class name -> &ClassInfo` lookup over a module's classes.
#[must_use]
pub fn class_by_name(classes: &[ClassInfo]) -> HashMap<&str, &ClassInfo> {
    classes.iter().map(|c| (c.name.as_str(), c)).collect()
}

/// Returns `true` when `name` resolves to a `TypedDict` directly or through any
/// transitive base class in `class_map`.
#[must_use]
pub fn is_transitive_typeddict<S: BuildHasher>(
    name: &str,
    class_map: &HashMap<&str, &ClassInfo, S>,
) -> bool {
    walk_bases(name, class_map, 0, &|class| class.is_typed_dict)
}

/// Returns `true` when this class — or any transitive `TypedDict` base — was
/// declared with the `extra_items=` keyword (PEP 728). Inherited so a subclass
/// of an `extra_items` `TypedDict` keeps accepting unknown keys.
#[must_use]
pub fn has_extra_items_transitive<S: BuildHasher>(
    name: &str,
    class_map: &HashMap<&str, &ClassInfo, S>,
) -> bool {
    walk_bases(name, class_map, 0, &|class| {
        class.class_keywords.iter().any(|kw| kw == "extra_items")
    })
}

/// The set of class names in `classes` that are `TypedDict`s directly or
/// transitively. Convenience wrapper that builds the lookup once; used by
/// callers that only have the class slice (e.g. cross-crate consumers).
#[must_use]
pub fn transitive_typeddict_names(classes: &[ClassInfo]) -> HashSet<&str> {
    let class_map = class_by_name(classes);
    classes
        .iter()
        .filter(|c| is_transitive_typeddict(c.name.as_str(), &class_map))
        .map(|c| c.name.as_str())
        .collect()
}

/// Strip `Required[...]`, `NotRequired[...]`, `ReadOnly[...]`, and
/// `Annotated[..., meta]` wrappers from a `TypedDict` field annotation, leaving
/// the underlying type text. Shared by the resolver's value-type compatibility
/// checks and the checker's redeclaration-legality rule (E0038) so both agree
/// on what a field's "core" type is.
#[must_use]
pub fn strip_typeddict_qualifiers(annotation: &str) -> &str {
    let mut result = annotation.trim();
    loop {
        let lower = result.to_ascii_lowercase();
        if let Some(inner) = try_strip_wrapper(&lower, result, "required[")
            .or_else(|| try_strip_wrapper(&lower, result, "notrequired["))
            .or_else(|| try_strip_wrapper(&lower, result, "readonly["))
        {
            result = inner.trim();
            continue;
        }
        // Annotated[T, ...] — keep only the first type arg.
        if let Some(inner) = try_strip_wrapper(&lower, result, "annotated[") {
            result = inner
                .find(',')
                .map_or(inner, |comma| &inner[..comma])
                .trim();
            continue;
        }
        break;
    }
    result
}

/// Try to strip a wrapper prefix (case-insensitive) and its matching `]`.
fn try_strip_wrapper<'a>(lower: &str, original: &'a str, prefix: &str) -> Option<&'a str> {
    if !lower.starts_with(prefix) || !original.ends_with(']') {
        return None;
    }
    Some(&original[prefix.len()..original.len() - 1])
}

/// Walk `name` and its transitive bases, returning `true` as soon as `predicate`
/// holds for any class in the chain.
fn walk_bases<S: BuildHasher>(
    name: &str,
    class_map: &HashMap<&str, &ClassInfo, S>,
    depth: u32,
    predicate: &dyn Fn(&ClassInfo) -> bool,
) -> bool {
    if depth >= MAX_DEPTH {
        return false;
    }
    let Some(class) = class_map.get(name) else {
        return false;
    };
    predicate(class)
        || class
            .bases
            .iter()
            .any(|base| walk_bases(base, class_map, depth + 1, predicate))
}
