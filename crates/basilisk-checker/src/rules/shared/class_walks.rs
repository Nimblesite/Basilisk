// ############################################################################
// # DELETED IMPLEMENTATION — PANIC-ONLY SHELL. DO NOT PUT LOGIC BACK HERE.   #
// #                                                                          #
// # This file WAS `crate::subtyping` under another name: a class hierarchy   #
// # keyed on RENDERED CLASS NAMES. `class_name_map` built                    #
// # `HashMap<&str, &ClassInfo>` from `c.name`, and `class_or_base_matches`   #
// # walked `ClassInfo::bases` — a `Vec<String>` the resolver fills with      #
// # "simple names only; complex expressions ignored" — looking each base up  #
// # in that map by SPELLING.                                                 #
// #                                                                          #
// # The old module comment stated the defect and shipped it anyway:          #
// #                                                                          #
// #   "Base names resolve to same-module classes by SIMPLE name, so `class   #
// #    Client(httpx.Client)` records the base as `Client` and the by-name    #
// #    lookup makes the class its own ancestor."                             #
// #                                                                          #
// # That is not a caveat on a working hierarchy. It IS the hierarchy: a      #
// # base reached through an alias misses, a dotted base collides with any    #
// # local class sharing its trailing word, and two distinct classes with the #
// # same rendered name are one entry. Roughly twenty rules asked this file   #
// # "does X inherit from Y?" and got an answer about spelling.               #
// #                                                                          #
// # `method_name_map` is the same defect for methods: it keys on             #
// # `(class_name, method_name)` STRINGS, so a method's owning class is       #
// # identified by how the class is written.                                  #
// #                                                                          #
// # THE SIGNATURES SURVIVE ONLY AS A MAP. Each body panics because the real  #
// # implementation DOES NOT EXIST YET:                                       #
// #                                                                          #
// #   * DO NOT return an empty map — every base lookup then misses and       #
// #     every inheritance rule silently stops firing.                        #
// #   * DO NOT return `false` from the walk — that blesses every illegal     #
// #     inheritance; `true` invents an ancestor for every class.             #
// #   * DO NOT rebuild the map under a new name in a rule module.            #
// #                                                                          #
// # The replacement resolves each base EXPRESSION through the binding table  #
// # to the class it denotes, and keys the hierarchy on definition site       #
// # (module path + name span), never on a rendered name. That needs base     #
// # SPANS on `ClassInfo`, which the resolver does not record yet.            #
// #                                                                          #
// # Pinned by:                                                               #
// #   crates/basilisk-checker/tests/string_keyed_class_hierarchy_pin_tests.rs
// ############################################################################

//! The DELETED string-keyed class hierarchy, reduced to loudly panicking
//! signatures so its call sites remain visible as the rebuild map.

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, FunctionInfo};

/// Panic message shared by every deleted body in this module.
macro_rules! deleted {
    ($what:literal) => {
        panic!(concat!(
            "basilisk-checker: `",
            $what,
            "` was DELETED because it keyed the class hierarchy on RENDERED \
             CLASS NAMES — `ClassInfo::bases` is a `Vec<String>` of simple \
             names, looked up in a map keyed on `ClassInfo::name`. A base \
             reached through an alias missed, a dotted base collided with any \
             local class sharing its trailing word, and two classes with the \
             same rendered name were one entry. It panics because the real \
             implementation — bases resolved through the binding table and \
             keyed on definition site — DOES NOT EXIST YET. Do not restore the \
             map and do not substitute an empty one: rebuild this caller on \
             resolved class identity, or make it abstain."
        ))
    };
}

/// DELETED — panics; see the banner at the head of this file.
pub(crate) fn class_name_map(_classes: &[ClassInfo]) -> HashMap<&str, &ClassInfo> {
    deleted!("class_name_map")
}

/// DELETED — panics; see the banner at the head of this file.
pub(crate) fn class_or_base_matches<'a>(
    _cls: &'a ClassInfo,
    _resolve: &dyn Fn(&str) -> Option<&'a ClassInfo>,
    _predicate: &dyn Fn(&'a ClassInfo) -> bool,
) -> bool {
    deleted!("class_or_base_matches")
}

/// DELETED — panics; see the banner at the head of this file.
pub(crate) fn method_name_map(
    _functions: &[FunctionInfo],
) -> HashMap<(&str, &str), Vec<&FunctionInfo>> {
    deleted!("method_name_map")
}
