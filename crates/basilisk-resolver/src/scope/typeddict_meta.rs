// ############################################################################
// # DELETED IMPLEMENTATION — PANIC-ONLY SHELL. DO NOT PUT LOGIC BACK HERE.   #
// #                                                                          #
// # Every body in this file walked a class hierarchy keyed on RENDERED CLASS #
// # NAMES. `class_by_name` built `HashMap<&str, &ClassInfo>` from `c.name`;  #
// # `walk_bases` pushed `class.bases` — a `Vec<String>` this crate fills     #
// # with "simple names only; complex expressions ignored" — and looked each  #
// # one up in that map by SPELLING.                                          #
// #                                                                          #
// # So `is_transitive_typeddict` answered "is this a TypedDict?" from how    #
// # the base classes are written:                                            #
// #                                                                          #
// #   from typing import TypedDict as TD                                     #
// #   class Movie(TD): ...          # `is_typed_dict` set at collection      #
// #   Alias = Movie                                                          #
// #   class Film(Alias): ...        # base recorded "Alias" -> MISSES        #
// #                                                                          #
// #   import other                                                           #
// #   class Movie(other.Movie): ... # base recorded "Movie" -> the class     #
// #                                 # becomes its own ancestor               #
// #                                                                          #
// # This is the same defect as the deleted `basilisk-checker::subtyping`,    #
// # sitting one crate lower where the checker cannot see it. A resolver that #
// # hands out a name-keyed hierarchy makes every consumer wrong for free.    #
// #                                                                          #
// # THE SIGNATURES SURVIVE ONLY AS A MAP. Each body panics because the real  #
// # implementation DOES NOT EXIST YET:                                       #
// #                                                                          #
// #   * DO NOT return `false` — every TypedDict rule then stops firing while #
// #     reporting full coverage.                                             #
// #   * DO NOT return `true` — every class becomes a TypedDict.              #
// #   * DO NOT return an empty set/map, and DO NOT rebuild the walk in a     #
// #     consumer crate.                                                      #
// #                                                                          #
// # The replacement records each base's EXPRESSION SPAN on `ClassInfo` and   #
// # resolves it through the `basilisk-canonical` binding table, keying the   #
// # hierarchy on definition site rather than on a rendered name. Note that   #
// # `ClassInfo::is_typed_dict` itself is already resolved correctly (see its #
// # doc comment: "resolved through the module's bindings at collection time  #
// # — never from the base's spelling"); it is only the TRANSITIVE walk that  #
// # falls back to strings.                                                   #
// #                                                                          #
// # Pinned by:                                                               #
// #   crates/basilisk-checker/tests/string_keyed_class_hierarchy_pin_tests.rs
// ############################################################################

//! The DELETED name-keyed `TypedDict` base walk, reduced to loudly panicking
//! signatures so its call sites remain visible as the rebuild map.

use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;

use super::class_types::ClassInfo;

/// Panic message shared by every deleted body in this module.
macro_rules! deleted {
    ($what:literal) => {
        panic!(concat!(
            "basilisk-resolver: `",
            $what,
            "` was DELETED because it walked the class hierarchy by RENDERED \
             CLASS NAME — `ClassInfo::bases` holds simple-name strings, looked \
             up in a map keyed on `ClassInfo::name`, so a base reached through \
             an alias missed and a dotted base made a class its own ancestor. \
             It panics because the real implementation — base expressions \
             resolved through the binding table, keyed on definition site — \
             DOES NOT EXIST YET. Do not restore the walk and do not substitute \
             a default answer: rebuild this caller on resolved class identity, \
             or make it abstain."
        ))
    };
}

/// DELETED — panics; see the banner at the head of this file.
#[must_use]
pub fn class_by_name(_classes: &[ClassInfo]) -> HashMap<&str, &ClassInfo> {
    deleted!("class_by_name")
}

/// DELETED — panics; see the banner at the head of this file.
#[must_use]
pub fn is_transitive_typeddict<S: BuildHasher>(
    _name: &str,
    _class_map: &HashMap<&str, &ClassInfo, S>,
) -> bool {
    deleted!("is_transitive_typeddict")
}

/// DELETED — panics; see the banner at the head of this file.
#[must_use]
pub fn has_extra_items_transitive<S: BuildHasher>(
    _name: &str,
    _class_map: &HashMap<&str, &ClassInfo, S>,
) -> bool {
    deleted!("has_extra_items_transitive")
}

/// DELETED — panics; see the banner at the head of this file.
#[must_use]
pub fn transitive_typeddict_names(_classes: &[ClassInfo]) -> HashSet<&str> {
    deleted!("transitive_typeddict_names")
}
