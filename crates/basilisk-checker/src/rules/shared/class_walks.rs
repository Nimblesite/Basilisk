//! Implements helpers for [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Class lookup maps and stack-safe transitive base-class walks (GitHub #278).
//!
//! Base names resolve to same-module classes by SIMPLE name, so `class
//! Client(httpx.Client)` records the base as `Client` and the by-name lookup
//! makes the class its own ancestor. These walks are iterative (explicit
//! worklist, zero recursion) so no chain depth can overflow the stack, and
//! `visited` bounds work to one visit per base name so cycles terminate.
//! Every transitive base walk must use these helpers or carry the same two
//! guards.
//!
//! `resolve` and `matches` receive each base name EXACTLY as recorded
//! (subscripts included), so call sites keep their own normalisation and the
//! helpers change nothing but termination.

use std::collections::{HashMap, HashSet};

use basilisk_resolver::{ClassInfo, FunctionInfo};

/// Build a `&str -> &ClassInfo` lookup map for every class in the module.
///
/// The returned map borrows from the slice; both must outlive the map.
pub(crate) fn class_name_map(classes: &[ClassInfo]) -> HashMap<&str, &ClassInfo> {
    classes.iter().map(|c| (c.name.as_str(), c)).collect()
}

/// Returns `true` when `predicate` holds for `cls` or for any class in its
/// transitive same-module base chain (bases resolve through `resolve`).
pub(crate) fn class_or_base_matches<'a>(
    cls: &'a ClassInfo,
    resolve: &dyn Fn(&str) -> Option<&'a ClassInfo>,
    predicate: &dyn Fn(&'a ClassInfo) -> bool,
) -> bool {
    let mut visited: HashSet<&str> = HashSet::new();
    let _ = visited.insert(cls.name.as_str());
    let mut worklist: Vec<&'a ClassInfo> = vec![cls];
    while let Some(current) = worklist.pop() {
        if predicate(current) {
            return true;
        }
        for base in &current.bases {
            if visited.insert(base.as_str()) {
                worklist.extend(resolve(base));
            }
        }
    }
    false
}

/// Build a `(class_name, method_name) -> Vec<&FunctionInfo>` lookup for every
/// method in the module (functions carrying a `class_name`).
///
/// Multiple definitions sharing a key (e.g. `@overload` signatures plus the
/// implementation) are preserved in declaration order. The returned map borrows
/// from the slice; both must outlive the map.
pub(crate) fn method_name_map(
    functions: &[FunctionInfo],
) -> HashMap<(&str, &str), Vec<&FunctionInfo>> {
    let mut map: HashMap<(&str, &str), Vec<&FunctionInfo>> = HashMap::new();
    for func in functions {
        if let Some(ref class_name) = func.class_name {
            map.entry((class_name.as_str(), func.name.as_str()))
                .or_default()
                .push(func);
        }
    }
    map
}
