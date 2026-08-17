//! Implements [RESOLV-CANONICAL-BINDING] and the transitive-recognition
//! foundation of [CHKARCH-DIAG-TYPEDDICT-READONLY-INHERITANCE]. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#RESOLV-CANONICAL-BINDING
//! A module's class hierarchy, keyed on DEFINITION SITE.
//!
//! This is the rebuilt replacement for the deleted `class_by_name` /
//! `walk_bases` pair. The difference is the key. That walk built
//! `HashMap<&str, &ClassInfo>` from `ClassInfo::name` and looked up
//! `ClassInfo::bases`, a `Vec<String>` of simple names, so:
//!
//! * `Alias = Movie; class Film(Alias)` recorded the base as `"Alias"`, found
//!   no class of that name, and lost the inheritance edge entirely;
//! * `import other; class Film(other.Movie)` recorded the base as `"Movie"`
//!   and attached `Film` to whatever local class happened to be spelled that
//!   way — inventing a hierarchy the program does not have.
//!
//! Here the edges come from [`ClassInfo::resolved_bases`], which the visitor
//! fills by resolving each base expression through the module's binding table
//! down to the `class` statement it denotes. Two classes spelled the same are
//! two nodes; one class reached through several names is one node.
//!
//! [`ClassInfo::is_typed_dict`] is only set when a class names `TypedDict`
//! among its OWN bases. A subclass of another `TypedDict` is still a
//! `TypedDict`, so membership questions must walk the chain — that is what
//! [`ClassGraph::is_typed_dict`] is for.

use std::collections::{HashMap, HashSet};

use super::class_types::{ClassInfo, ResolvedBase};
use super::span::Span;

/// The result of walking a class's base chain: what was reached, and whether
/// anything was left unreachable.
///
/// The two fields answer different questions and callers need both. A rule
/// asking "does this chain contain a `TypedDict`?" is satisfied the moment it
/// finds one, and [`Self::classes`] alone will do. A rule asking "is this
/// class NOT a subclass of that one?" is making a claim about the WHOLE
/// hierarchy, and may only make it when [`Self::complete`] is `true` —
/// otherwise the honest answer is "unknown" ([CHKARCH-CONFORMANCE-MODE]).
#[derive(Debug)]
pub struct Ancestry<'a> {
    /// The class and every ancestor of it defined in this module, most-derived
    /// first, each appearing exactly once.
    pub classes: Vec<&'a ClassInfo>,
    /// Whether every base of every class reached was resolved.
    ///
    /// `false` when some base is a class from another module, a name this
    /// module never bound, or anything else the binding table could not follow
    /// — so [`Self::classes`] is a LOWER BOUND on the real ancestry rather
    /// than the whole of it.
    pub complete: bool,
}

/// A module's classes, indexed by the definition site each one is keyed on.
///
/// Borrows the class slice; building one is a single pass and costs no clones.
#[derive(Debug)]
pub struct ClassGraph<'a> {
    /// The classes this graph indexes, in declaration order.
    classes: &'a [ClassInfo],
    /// Definition site → index into [`Self::classes`].
    by_site: HashMap<Span, usize>,
}

impl<'a> ClassGraph<'a> {
    /// Index a module's classes by definition site.
    #[must_use]
    pub fn new(classes: &'a [ClassInfo]) -> Self {
        let by_site = classes
            .iter()
            .enumerate()
            .map(|(index, class)| (class.name_span, index))
            .collect();
        Self { classes, by_site }
    }

    /// The classes this graph indexes, in declaration order.
    #[must_use]
    pub fn classes(&self) -> &'a [ClassInfo] {
        self.classes
    }

    /// The class defined at `site`, if this module defines one there.
    #[must_use]
    pub fn at(&self, site: Span) -> Option<&'a ClassInfo> {
        self.classes.get(*self.by_site.get(&site)?)
    }

    /// `class` and every ancestor of it defined in this module, most-derived
    /// first, each visited exactly once — plus whether any edge was left
    /// unfollowed.
    ///
    /// Iterative by construction, so a chain of any depth costs heap rather
    /// than stack; the visited set bounds the walk to one visit per class, so
    /// a cyclic or self-referential base list — illegal Python, but reachable
    /// input (GitHub #398) — terminates in linear time instead of branching
    /// exponentially.
    ///
    /// A base that resolves outside this module contributes no edge, and that
    /// is exactly why the walk reports [`Ancestry::complete`]. Dropping such a
    /// base silently would hand callers a truncated chain indistinguishable
    /// from a fully known one, and the first thing a caller does with a chain
    /// is read a MISSING class out of it. `object` and the other typing forms
    /// are resolved, not unknown: they add no local node, but nothing about
    /// them is hidden.
    #[must_use]
    pub fn ancestry(&self, class: &'a ClassInfo) -> Ancestry<'a> {
        let mut visited: HashSet<Span> = HashSet::new();
        let mut order: Vec<&'a ClassInfo> = Vec::new();
        let mut worklist: Vec<&'a ClassInfo> = vec![class];
        let mut complete = true;
        while let Some(current) = worklist.pop() {
            if !visited.insert(current.name_span) {
                continue;
            }
            order.push(current);
            // Reversed so the worklist pops bases in declaration order, which
            // is the order a subclass's fields shadow inherited ones.
            for base in current.resolved_bases.iter().rev() {
                match base.resolved {
                    ResolvedBase::LocalClass(site) => match self.at(site) {
                        Some(base_class) => worklist.push(base_class),
                        // A site with no class behind it means the graph was
                        // built over a different class set than the one that
                        // resolved these bases. Nothing is known about that
                        // edge either.
                        None => complete = false,
                    },
                    ResolvedBase::Form(_) => {}
                    ResolvedBase::Unknown => complete = false,
                }
            }
        }
        Ancestry {
            classes: order,
            complete,
        }
    }

    /// [`Self::ancestry`] without the completeness flag, for callers whose
    /// question is answered by FINDING a class rather than by failing to.
    #[must_use]
    pub fn ancestors(&self, class: &'a ClassInfo) -> Vec<&'a ClassInfo> {
        self.ancestry(class).classes
    }

    /// Whether any class in `class`'s chain satisfies `predicate`.
    fn any_ancestor(&self, class: &'a ClassInfo, predicate: impl Fn(&ClassInfo) -> bool) -> bool {
        self.ancestors(class).into_iter().any(predicate)
    }

    /// Whether `class` is a `TypedDict` — declared directly, or through a base
    /// this module defines.
    #[must_use]
    pub fn is_typed_dict(&self, class: &'a ClassInfo) -> bool {
        self.any_ancestor(class, |ancestor| ancestor.is_typed_dict)
    }

    /// Whether `class` — or any `TypedDict` in its chain — was declared with
    /// PEP 728's `extra_items=` keyword, which makes unknown keys legal.
    ///
    /// Reading the keyword's own name is not a spelling test on a type: a
    /// class-definition keyword is fixed syntax at the definition site, needs
    /// no import, and cannot be aliased or rebound.
    #[must_use]
    pub fn has_extra_items(&self, class: &'a ClassInfo) -> bool {
        self.any_ancestor(class, |ancestor| {
            ancestor.class_keywords.iter().any(|kw| kw == "extra_items")
        })
    }

    /// Every class in this module that is a `TypedDict`, directly or through
    /// its bases, in declaration order.
    #[must_use]
    pub fn typed_dicts(&self) -> Vec<&'a ClassInfo> {
        self.classes
            .iter()
            .filter(|class| self.is_typed_dict(class))
            .collect()
    }
}

// `typed_dict_class_names` — a projection of `typed_dicts()` back onto
// `ClassInfo::name` — was REMOVED OUTRIGHT, not stubbed: there is no lawful
// caller for a set of class names. A caller that cannot hold a `Span` has an
// identity problem of its own, and handing it a spelling would hide that
// problem instead of forcing it. Callers that need membership use
// `ClassGraph::typed_dicts` and key on `ClassInfo::name_span`. Do not
// reintroduce the projection under any name.
