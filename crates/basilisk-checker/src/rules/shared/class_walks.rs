// ############################################################################
// # REBUILT ON DEFINITION-SITE IDENTITY. DO NOT PUT NAME KEYING BACK.        #
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
// # The prerequisite the deletion banner named — "base SPANS on `ClassInfo`, #
// # which the resolver does not record yet" — NOW EXISTS:                    #
// # `ClassInfo::resolved_bases` carries each base expression already         #
// # resolved through the binding table, and `basilisk_resolver::ClassGraph`  #
// # keys the hierarchy on definition site. Everything below delegates to it. #
// #                                                                          #
// # `class_name_map` is GONE rather than rebuilt: a `HashMap<&str, _>` over  #
// # class names has no lawful form. A caller that needs the class a NAME     #
// # denotes resolves that name's expression through the binding table.       #
// #                                                                          #
// # Pinned by:                                                               #
// #   crates/basilisk-checker/tests/string_keyed_class_hierarchy_pin_tests.rs
// ############################################################################

//! Shared walks over a module's class hierarchy, answered from resolved class
//! identity rather than from how a class happens to be written.

use std::collections::HashMap;

use basilisk_resolver::{ClassGraph, ClassInfo, FunctionInfo, Span};

/// Whether `cls` itself, or any class it inherits from IN THIS MODULE,
/// satisfies `predicate`.
///
/// Each edge comes from a base expression the resolver already resolved
/// through the binding table, so `Alias = Base; class Sub(Alias)` is one edge
/// to `Base`, and `class Client(httpx.Client)` is NO edge to a local class
/// spelled `Client` — the two failures the deleted by-name walk made in both
/// directions. The walk visits each class once, so a cyclic base list
/// terminates (GitHub #278, #398).
///
/// Answers `true` on a match and `false` otherwise. When the answer matters as
/// evidence of ABSENCE — "this class is definitely not a subclass of that
/// one" — use [`ClassGraph::ancestry`] instead and check its `complete` flag,
/// because a base from another module is an edge this module cannot follow.
pub(crate) fn class_or_base_matches(
    graph: &ClassGraph<'_>,
    cls: &ClassInfo,
    predicate: impl Fn(&ClassInfo) -> bool,
) -> bool {
    graph.ancestors(cls).into_iter().any(predicate)
}

/// Every method definition in the module, indexed by the DEFINITION SITE of
/// its owning class and its own name.
///
/// REBUILT from a `HashMap<(&str, &str), _>` keyed on the owning class's
/// rendered name, which merged two classes declared with the same name in one
/// module into a single entry — so a call to one class's method was checked
/// against the other class's signature. `FunctionInfo::class_site` is the
/// owning `class` statement's name span, the same key [`ClassGraph`] uses, so
/// an index built here lines up with the hierarchy exactly.
///
/// The method's own name is the name it is DEFINED under in the class body,
/// which is what an attribute access on that class looks up; it is not a
/// use-site spelling standing in for a resolution.
///
/// A `Vec` per key because a method may be defined more than once —
/// `@overload` stubs followed by an implementation, or a conditional
/// redefinition. Entries stay in source order, so the last is the one in force.
pub(crate) fn method_name_map(
    functions: &[FunctionInfo],
) -> HashMap<(Span, &str), Vec<&FunctionInfo>> {
    let mut map: HashMap<(Span, &str), Vec<&FunctionInfo>> = HashMap::new();
    for func in functions {
        let Some(site) = func.class_site else {
            continue;
        };
        map.entry((site, func.name.as_str()))
            .or_default()
            .push(func);
    }
    map
}
