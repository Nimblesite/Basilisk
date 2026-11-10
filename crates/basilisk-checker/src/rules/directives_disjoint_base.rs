//! Implements [`directives_disjoint_base`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! `directives_disjoint_base`: PEP 800 disjoint bases.
//!
//! PEP 800 introduces `typing.disjoint_base`. A class is a *disjoint base* when
//! it is decorated `@disjoint_base` or defines a non-empty `__slots__`. A class
//! definition must have a single *dominating* disjoint base among its bases:
//!
//! ```python
//! @disjoint_base
//! class Left: ...
//! @disjoint_base
//! class Right: ...
//!
//! class Both(Left, Right): ...   # error — incompatible disjoint bases
//! ```
//!
//! The decorator may be used only on nominal classes (including `NamedTuple`); it
//! is an error to apply it to a function, a `TypedDict`, or a `Protocol`.

use std::collections::{HashMap, HashSet};

use basilisk_resolver::{ClassInfo, ResolvedModule, RhsKind, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::guards::is_protocol_class;
use super::shared::class_name_map;
use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "directives_disjoint_base",
    docs_url: "https://www.basilisk-python.dev/errors/directives_disjoint_base",
};

/// Emits `directives_disjoint_base` for `@disjoint_base` misuse and for class
/// definitions with incompatible disjoint bases.
pub(crate) struct DisjointBaseViolation;

impl Rule for DisjointBaseViolation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // `@disjoint_base` on a module-level function is forbidden.
        for func in &module.functions {
            if func.class_name.is_none() {
                if let Some(span) = disjoint_base_decorator_span(&func.decorator_spans) {
                    diagnostics.push(misuse(span, "a function", &module.path));
                }
            }
        }

        let map = class_name_map(&module.classes);

        for cls in &module.classes {
            // `@disjoint_base` on a TypedDict / Protocol is forbidden (NamedTuple
            // and plain nominal classes are allowed).
            if let Some(span) = disjoint_base_decorator_span(&cls.decorator_spans) {
                if cls.is_typed_dict {
                    diagnostics.push(misuse(span, "a TypedDict", &module.path));
                    continue;
                }
                if is_protocol_class(cls) {
                    diagnostics.push(misuse(span, "a Protocol", &module.path));
                    continue;
                }
            }

            // A class must have a single dominating disjoint base among its bases.
            let candidates: Vec<&str> = collect_candidates(cls, &map).into_iter().collect();
            if candidates.len() > 1 && !has_dominator(&candidates, &map) {
                diagnostics.push(incompatible(cls.name_span, &candidates, &module.path));
            }
        }
    }
}

/// The span of a `@disjoint_base` / `@*.disjoint_base` decorator, if present.
fn disjoint_base_decorator_span(decorators: &[(String, Span)]) -> Option<Span> {
    decorators
        .iter()
        .find(|(name, _)| name == "disjoint_base" || name.ends_with(".disjoint_base"))
        .map(|(_, span)| *span)
}

/// A class is a disjoint base when decorated `@disjoint_base` or when it defines
/// a non-empty `__slots__`.
fn is_disjoint_base(cls: &ClassInfo) -> bool {
    disjoint_base_decorator_span(&cls.decorator_spans).is_some() || has_nonempty_slots(cls)
}

/// `true` when the class assigns a non-empty `__slots__` (an empty `__slots__ = ()`
/// does not make the class a disjoint base).
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
    if is_disjoint_base(cls) {
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

fn misuse(span: Span, target: &str, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!("`@disjoint_base` cannot be applied to {target}"),
        span,
        path,
        Some("Remove the `@disjoint_base` decorator".to_owned()),
        Some(
            "PEP 800: `@disjoint_base` may be used only on nominal classes (including NamedTuple)"
                .to_owned(),
        ),
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
