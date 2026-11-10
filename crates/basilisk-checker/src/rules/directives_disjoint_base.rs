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
/// itself if it is a disjoint base, otherwise to its own inherited disjoint bases
/// (external bases contribute nothing).
fn collect_candidates<'a>(
    cls: &'a ClassInfo,
    map: &HashMap<&'a str, &'a ClassInfo>,
) -> HashSet<&'a str> {
    let mut result = HashSet::new();
    for base in &cls.bases {
        let mut visited = HashSet::new();
        resolve_base(base.as_str(), map, &mut visited, &mut result);
    }
    result
}

fn resolve_base<'a>(
    name: &'a str,
    map: &HashMap<&'a str, &'a ClassInfo>,
    visited: &mut HashSet<&'a str>,
    out: &mut HashSet<&'a str>,
) {
    if !visited.insert(name) {
        return; // cycle guard
    }
    let Some(cls) = map.get(name) else {
        return; // external base — contributes no disjoint base
    };
    if has_nonempty_slots(cls) {
        let _ = out.insert(name);
        return;
    }
    for base in &cls.bases {
        resolve_base(base.as_str(), map, visited, out);
    }
}

/// `true` when one candidate is a (transitive) subclass of every other candidate.
fn has_dominator(candidates: &[&str], map: &HashMap<&str, &ClassInfo>) -> bool {
    candidates
        .iter()
        .any(|x| candidates.iter().all(|y| is_subclass(x, y, map, 0)))
}

fn is_subclass(child: &str, ancestor: &str, map: &HashMap<&str, &ClassInfo>, depth: u32) -> bool {
    if child == ancestor {
        return true;
    }
    if depth > 64 {
        return false; // cycle / pathological-depth guard
    }
    map.get(child).is_some_and(|cls| {
        cls.bases
            .iter()
            .any(|base| is_subclass(base, ancestor, map, depth + 1))
    })
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
