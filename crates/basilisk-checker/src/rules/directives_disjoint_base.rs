//! Implements [`directives_disjoint_base`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! `directives_disjoint_base`: PEP 800 disjoint bases.
//!
//! PEP 800 introduces disjoint bases. A class is a *disjoint base* when it
//! defines a non-empty `__slots__`. A class definition must have a single
//! *dominating* disjoint base among its bases:
//!
//! ```python
//! class Left:
//!     __slots__ = ("a",)
//!
//! class Right:
//!     __slots__ = ("b",)
//!
//! class Both(Left, Right): ...   # error — incompatible disjoint bases
//! ```
//!
//! # Coverage
//!
//! Disjoint-base-ness is recognised from ONE source: a class assigning a
//! non-empty `__slots__` in its own body. PEP 800's other sources are not
//! recognised, so a class made disjoint by any of them is invisible here and
//! a conflict involving it goes unreported. This is a partial implementation
//! and a silent one — there is no diagnostic saying "could not tell".

use std::collections::HashSet;

use basilisk_resolver::{ClassGraph, ClassInfo, ResolvedBase, ResolvedModule, RhsKind, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "directives_disjoint_base",
    docs_url: "https://www.basilisk-python.dev/errors/directives_disjoint_base",
};

/// Emits `directives_disjoint_base` for class definitions with incompatible
/// disjoint bases.
pub(crate) struct DisjointBaseViolation;

impl Rule for DisjointBaseViolation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let graph = ClassGraph::new(&module.classes);

        for cls in &module.classes {
            // A class must have a single dominating disjoint base among its bases.
            let candidates: Vec<&ClassInfo> = collect_candidates(&graph, cls).into_iter().collect();
            if candidates.len() > 1 && !has_dominator(&graph, &candidates) {
                let names: Vec<&str> = candidates.iter().map(|c| c.name.as_str()).collect();
                diagnostics.push(incompatible(cls.name_span, &names, &module.path));
            }
        }
    }
}

/// A class is a disjoint base when it assigns a non-empty `__slots__` (an empty
/// `__slots__ = ()` does not make the class a disjoint base).
fn has_nonempty_slots(cls: &ClassInfo) -> bool {
    cls.attributes
        .iter()
        .any(|attr| attr.name == "__slots__" && slots_value_nonempty(&attr.rhs_kind))
}

fn slots_value_nonempty(rhs: &RhsKind) -> bool {
    match rhs {
        RhsKind::Tuple(items) | RhsKind::List(items) | RhsKind::Set(items) => !items.is_empty(),
        RhsKind::Dict(items) => !items.is_empty(),
        // `__slots__ = "x"` declares a single slot.
        RhsKind::StrLiteral => true,
        _ => false,
    }
}

/// The disjoint bases reachable through a class's bases: each base resolves to
/// itself if it is a disjoint base, otherwise to its own inherited disjoint
/// bases. A base from another module contributes nothing — unknown, not absent.
///
/// REBUILT on definition-site identity. The deleted version walked
/// `ClassInfo::bases`, a `Vec<String>` the resolver fills with "simple names
/// only; complex expressions ignored", looking each base up in a map keyed on
/// `ClassInfo::name`. So a base reached through an alias MISSED, a dotted base
/// collided with any local class sharing its trailing word, and two classes
/// with one rendered name were a single entry. `ClassInfo::resolved_bases`
/// carries each base expression already resolved through the binding table.
fn collect_candidates<'a>(graph: &ClassGraph<'a>, cls: &'a ClassInfo) -> Vec<&'a ClassInfo> {
    // Keyed on definition site, so a module declaring two classes with one
    // name contributes two candidates rather than silently one.
    let mut found: Vec<&'a ClassInfo> = Vec::new();
    let mut collected: HashSet<Span> = HashSet::new();
    let mut seen: HashSet<Span> = HashSet::new();
    let mut worklist: Vec<&'a ClassInfo> = direct_local_bases(graph, cls);
    while let Some(base) = worklist.pop() {
        if !seen.insert(base.name_span) {
            continue;
        }
        if has_nonempty_slots(base) {
            // A disjoint base dominates everything above it, so the walk stops
            // here: its own bases cannot add a second independent candidate.
            if collected.insert(base.name_span) {
                found.push(base);
            }
        } else {
            worklist.extend(direct_local_bases(graph, base));
        }
    }
    found
}

/// The classes THIS MODULE defines that `cls` directly inherits from, in
/// declaration order.
fn direct_local_bases<'a>(graph: &ClassGraph<'a>, cls: &'a ClassInfo) -> Vec<&'a ClassInfo> {
    cls.resolved_bases
        .iter()
        .filter_map(|base| match base.resolved {
            ResolvedBase::LocalClass(site) => graph.at(site),
            ResolvedBase::Form(_) | ResolvedBase::Unknown => None,
        })
        .collect()
}

/// `true` when one candidate is a (transitive) subclass of every other candidate.
fn has_dominator(graph: &ClassGraph<'_>, candidates: &[&ClassInfo]) -> bool {
    candidates.iter().any(|dominator| {
        let chain: HashSet<Span> = graph
            .ancestors(dominator)
            .into_iter()
            .map(|ancestor| ancestor.name_span)
            .collect();
        candidates
            .iter()
            .all(|other| chain.contains(&other.name_span))
    })
}

// ##########################################################################
// # `directives_disjoint_base::is_subclass` IS GONE, NOT REBUILT IN PLACE.  #
// #                                                                        #
// # Its body was `map.get(child)` plus                                     #
// # `cls.bases.iter().any(|base| is_subclass(base, ...))` — a subclass     #
// # relation computed entirely between NAME STRINGS, with the depth        #
// # counter there to stop `class Client(httpx.Client)` recursing into      #
// # itself.                                                                #
// #                                                                        #
// # `ClassGraph::ancestors` is that relation on definition-site identity,  #
// # and its visited set is what makes a cyclic base list terminate, so the #
// # separate recursive walk had nothing left to do.                        #
// #                                                                        #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs             #
// ##########################################################################

fn incompatible(span: Span, candidates: &[&str], path: &str) -> Diagnostic {
    let mut names: Vec<&str> = candidates.to_vec();
    names.sort_unstable();
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Class has incompatible disjoint bases ({}): no single base is a subclass of all the \
             others",
            names.join(", ")
        ),
        span,
        path,
        Some("A class may have at most one dominating disjoint base".to_owned()),
        Some("PEP 800: two unrelated disjoint bases cannot share a common subclass".to_owned()),
    )
}
