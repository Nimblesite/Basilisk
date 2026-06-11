//! Implements [BSK-E0014] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! Member-based protocol satisfaction (PEP 544): a class satisfies a protocol
//! when it provides every protocol member with a structurally compatible type.

use std::collections::{HashMap, HashSet};

use super::callable_check::{sigs_compatible, specialize_class_sigs, CallIndex};
use super::sig_model::{ClassEntry, Sig};

/// Member names contributed by well-known external protocol bases.
///
/// These are presence-checked only: their signatures are standardized and a
/// mismatch would be caught by other rules.
fn well_known_member_names(base: &str) -> &'static [&'static str] {
    match base {
        "Sized" => &["__len__"],
        "Hashable" => &["__hash__"],
        _ => &[],
    }
}

/// A class's full member surface: method overload sets and attribute names.
type MemberSurface<'a> = (HashMap<&'a str, &'a Vec<Sig>>, HashSet<&'a str>);

/// All members of a class, walking local base classes recursively.
///
/// Methods earlier in the MRO win.  With `strict`, an unresolvable base makes
/// the member set unknown (`None`); otherwise unknown bases contribute nothing.
fn collect_members<'a>(
    entry: &'a ClassEntry,
    index: &'a CallIndex,
    strict: bool,
) -> Option<MemberSurface<'a>> {
    let mut methods: HashMap<&str, &Vec<Sig>> = HashMap::new();
    let mut attrs: HashSet<&str> = HashSet::new();
    let mut stack = vec![entry];
    let mut visited = 0usize;

    while let Some(current) = stack.pop() {
        visited += 1;
        if visited > 32 {
            return None;
        }
        for (name, sigs) in &current.methods {
            let _ = methods.entry(name.as_str()).or_insert(sigs);
        }
        attrs.extend(current.attrs.iter().map(String::as_str));
        for base in &current.bases {
            if let Some(base_entry) = index.classes.get(base) {
                stack.push(base_entry);
            } else {
                let known = well_known_member_names(base);
                if known.is_empty() && strict {
                    return None;
                }
                attrs.extend(known);
            }
        }
    }
    Some((methods, attrs))
}

/// `true` when `source` (a class) structurally satisfies `target` (a protocol),
/// with optional generic specialization arguments on either side.
pub(super) fn protocol_satisfied(
    (target, target_args): (&ClassEntry, Option<&[String]>),
    (source, source_args): (&ClassEntry, Option<&[String]>),
    index: &CallIndex,
) -> bool {
    // The target's member set must be fully enumerable; the source's may be
    // partial (missing members simply fail the check).
    let Some((target_methods, target_attrs)) = collect_members(target, index, true) else {
        return false;
    };
    let Some((source_methods, source_attrs)) = collect_members(source, index, false) else {
        return false;
    };

    // Every protocol attribute must exist on the source.
    let attr_ok = target_attrs
        .iter()
        .all(|attr| source_attrs.contains(attr) || source_methods.contains_key(attr));
    if !attr_ok {
        return false;
    }

    // Every protocol method must exist with a compatible signature.
    for (name, tgt_sigs) in &target_methods {
        if let Some(src_sigs) = source_methods.get(name) {
            let specialized_target =
                specialize_class_sigs(tgt_sigs, &target.generic_params, target_args, index);
            let specialized_source =
                specialize_class_sigs(src_sigs, &source.generic_params, source_args, index);
            if !sigs_compatible(&specialized_source, &specialized_target) {
                return false;
            }
        } else if !source_attrs.contains(*name) {
            return false;
        }
    }
    true
}
