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

use std::collections::{HashMap, HashSet};

use basilisk_resolver::{ClassInfo, ResolvedModule, RhsKind, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::shared::class_name_map;
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
        let map = class_name_map(&module.classes);

        for cls in &module.classes {
            // A class must have a single dominating disjoint base among its bases.
            let candidates: Vec<&str> = collect_candidates(cls, &map).into_iter().collect();
            if candidates.len() > 1 && !has_dominator(&candidates, &map) {
                diagnostics.push(incompatible(cls.name_span, &candidates, &module.path));
            }
        }
    }
}

#[expect(
    dead_code,
    reason = "caller deleted for spelling dependence; this AST-based helper reads \
              `__slots__` from resolved attributes and is retained for the rebuild — \
              see tests/string_keyed_class_hierarchy_pin_tests.rs"
)]
/// A class is a disjoint base when it assigns a non-empty `__slots__` (an empty
/// `__slots__ = ()` does not make the class a disjoint base).
fn has_nonempty_slots(cls: &ClassInfo) -> bool {
    cls.attributes
        .iter()
        .any(|attr| attr.name == "__slots__" && slots_value_nonempty(&attr.rhs_kind))
}

#[expect(
    dead_code,
    reason = "caller deleted for spelling dependence; this AST-based helper reads \
              `__slots__` from resolved attributes and is retained for the rebuild — \
              see tests/string_keyed_class_hierarchy_pin_tests.rs"
)]
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
/// itself if it is a disjoint base, otherwise to its own inherited disjoint bases
/// (external bases contribute nothing).
// ##########################################################################
// # DELETED BODY — `collect_candidates`. DO NOT RESTORE IT AND DO NOT RETURN A DEFAULT.
// #
// # `resolve_base(base.as_str(), map, ...)` walked PEP 800 disjoint bases through a name-keyed map.
// #
// # `ClassInfo::bases` is a `Vec<String>` the resolver fills with "simple
// # names only; complex expressions ignored", and the lookup map is keyed on
// # `ClassInfo::name`. So a base reached through an alias MISSED, a dotted
// # base collided with any local class sharing its trailing word, and two
// # classes with one rendered name were a single entry.
// #
// # The replacement resolves each base EXPRESSION through the binding table
// # and keys the hierarchy on definition site. That needs base SPANS on
// # `ClassInfo`, which the resolver does not record yet.
// #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
// ##########################################################################
fn collect_candidates<'a>(
    _cls: &'a ClassInfo,
    _map: &HashMap<&'a str, &'a ClassInfo>,
) -> HashSet<&'a str> {
    panic!(
        "basilisk-checker: `collect_candidates` was DELETED because it identified base classes by \
         their RENDERED NAMES, so an aliased base missed and a dotted base collided with \
         any local class sharing its trailing word. It panics because the real \
         implementation — base expressions resolved through the binding table — DOES NOT \
         EXIST YET. Do not restore the name lookup and do not substitute a default \
         answer in its place."
    )
}

// ##########################################################################
// # DELETED AND GONE — `resolve_base`. NO PANIC SHELL: its only caller
// # (`collect_candidates`) was deleted too, so there is no call site left to
// # keep visible. DO NOT RECREATE IT.
// #
// # It resolved a PEP 800 disjoint base by RENDERED NAME through a name-keyed
// # map and recursed on `cls.bases` strings.
// #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
// ##########################################################################

/// `true` when one candidate is a (transitive) subclass of every other candidate.
fn has_dominator(candidates: &[&str], map: &HashMap<&str, &ClassInfo>) -> bool {
    candidates
        .iter()
        .any(|x| candidates.iter().all(|y| is_subclass(x, y, map, 0)))
}

// ##########################################################################
// # DELETED BODY — `directives_disjoint_base::is_subclass`. DO NOT RESTORE IT AND DO NOT RETURN A DEFAULT.
// #
// # `map.get(child)` + `cls.bases.iter().any(|base| is_subclass(base, ...))` — a subclass relation computed entirely between NAME STRINGS.
// #
// # `ClassInfo::bases` is a `Vec<String>` the resolver fills with "simple
// # names only; complex expressions ignored", and the lookup map is keyed on
// # `ClassInfo::name`. So a base reached through an alias MISSED, a dotted
// # base collided with any local class sharing its trailing word, and two
// # classes with one rendered name were a single entry.
// #
// # The replacement resolves each base EXPRESSION through the binding table
// # and keys the hierarchy on definition site. That needs base SPANS on
// # `ClassInfo`, which the resolver does not record yet.
// #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
// ##########################################################################
fn is_subclass(
    _child: &str,
    _ancestor: &str,
    _map: &HashMap<&str, &ClassInfo>,
    _depth: u32,
) -> bool {
    panic!(
        "basilisk-checker: `directives_disjoint_base::is_subclass` was DELETED because it identified base classes by \
         their RENDERED NAMES, so an aliased base missed and a dotted base collided with \
         any local class sharing its trailing word. It panics because the real \
         implementation — base expressions resolved through the binding table — DOES NOT \
         EXIST YET. Do not restore the name lookup and do not substitute a default \
         answer in its place."
    )
}

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
