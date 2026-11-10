//! Implements [`assignment_compatibility`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Member-based protocol satisfaction (PEP 544): a class satisfies a protocol
//! when it provides every protocol member with a structurally compatible type.

use std::collections::{HashMap, HashSet};

use super::callable_check::{sigs_compatible, specialize_class_sigs, CallIndex};
use super::sig_model::{ClassEntry, Sig};

/// Member names contributed by well-known external protocol bases.
///
/// `Sized`, `Hashable` and their kin all require an import, so mapping a base's
/// SOURCE SPELLING to its members is not import resolution — a local class
/// named `Sized` was treated as the stdlib protocol, and
/// `from collections.abc import Sized as S` was not. That table is deleted;
/// rebuild it on the annotation cascade ([TYPEINF-ANNOTATION-RESOLUTION]).
fn well_known_member_names(_base: &str) -> &'static [&'static str] {
    &[]
}

/// A class's full member surface: method overload sets and attribute names.
type MemberSurface<'a> = (HashMap<&'a str, &'a Vec<Sig>>, HashSet<&'a str>);

/// All members of a class, walking local base classes recursively.
///
/// Methods earlier in the MRO win.  With `strict`, an unresolvable base makes
/// the member set unknown (`None`); otherwise unknown bases contribute nothing.
// ##################################################################
// # DELETED BODY. DO NOT RESTORE IT AND DO NOT RETURN A DEFAULT.
// #
// # `index.classes.get(base)` walked a class's member surface through a map keyed on rendered base names.
// #
// # `ClassInfo::bases` holds RENDERED SIMPLE NAMES ("complex
// # expressions ignored") and the lookup map is keyed on
// # `ClassInfo::name`, so an aliased base MISSED, a dotted base
// # collided with any local class sharing its trailing word, and two
// # classes with one rendered name were a single entry.
// #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
// ##################################################################
fn collect_members<'a>(
    _entry: &'a ClassEntry,
    _index: &'a CallIndex,
    _strict: bool,
) -> Option<MemberSurface<'a>> {
    panic!(
        "basilisk-checker: `protocol_members::collect_members` was DELETED because it identified base classes by \
         their RENDERED NAMES. It panics because the real implementation — base \
         expressions resolved through the binding table — DOES NOT EXIST YET. Do not \
         restore the name lookup and do not substitute a default answer."
    )
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
