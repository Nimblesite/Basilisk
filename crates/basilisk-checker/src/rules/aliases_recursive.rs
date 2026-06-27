//! Implements [aliases_recursive] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! aliases_recursive: Cyclical type alias reference.
//!
//! A `TypeAlias`-annotated assignment whose RHS contains a forward-reference
//! string that resolves back to the alias itself (directly or through a chain
//! of mutual references) creates an infinite type that cannot be resolved.
//!
//! ```python
//! from typing import TypeAlias, Union
//!
//! # Direct self-reference — the Union *only* wraps itself and a base type,
//! # producing an infinitely expanding alias:
//! RecursiveUnion: TypeAlias = Union["RecursiveUnion", int]  # E
//!
//! # Mutual reference — two aliases reference each other:
//! A: TypeAlias = Union["B", int]
//! B: TypeAlias = Union["A", str]  # E
//! ```

use std::collections::{HashMap, HashSet};

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "aliases_recursive",
    docs_url: "https://www.basilisk-python.dev/errors/aliases_recursive",
};

/// Emits aliases_recursive when a `TypeAlias`-annotated assignment's RHS contains a
/// cyclical forward reference (self-referential or mutually referential).
pub(crate) struct CyclicalTypeAliasReference;

/// Inner DFS helper — walks from `current` looking for a path back to
/// `target`.
fn has_cycle_inner(
    target: &str,
    current: &str,
    graph: &HashMap<&str, Vec<&str>>,
    visited: &mut HashSet<String>,
) -> bool {
    let Some(neighbours) = graph.get(current) else {
        return false;
    };
    for &neighbour in neighbours {
        if neighbour == target {
            return true;
        }
        if visited.insert(neighbour.to_owned())
            && has_cycle_inner(target, neighbour, graph, visited)
        {
            return true;
        }
    }
    false
}

/// Detect whether `start` can reach itself by following forward-reference
/// edges in the alias graph.  Returns `true` when a cycle is found.
fn has_cycle(start: &str, graph: &HashMap<&str, Vec<&str>>) -> bool {
    let Some(neighbours) = graph.get(start) else {
        return false;
    };
    let mut visited = HashSet::new();
    for &neighbour in neighbours {
        if neighbour == start {
            return true;
        }
        if visited.insert(neighbour.to_owned())
            && has_cycle_inner(start, neighbour, graph, &mut visited)
        {
            return true;
        }
    }
    false
}

/// Known container types that make recursive type aliases valid when they
/// wrap the self-reference (e.g. `list["Json"]` inside a Union).
const CONTAINER_TYPES: &[&str] = &[
    "list",
    "dict",
    "set",
    "frozenset",
    "tuple",
    "List",
    "Dict",
    "Set",
    "FrozenSet",
    "Tuple",
    "Mapping",
    "MutableMapping",
    "Sequence",
    "MutableSequence",
    "Iterable",
    "Iterator",
    "Deque",
    "DefaultDict",
    "OrderedDict",
    "ChainMap",
    "Counter",
];

/// Returns `true` if the alias's RHS references any known container type,
/// indicating the recursive reference is likely wrapped in a container
/// (e.g. `list["Json"]`) rather than appearing directly in a Union.
fn has_container_wrapper(alias: &basilisk_resolver::TypeAliasDefInfo) -> bool {
    alias
        .rhs_names
        .iter()
        .any(|name| CONTAINER_TYPES.contains(&name.as_str()))
}

impl Rule for CyclicalTypeAliasReference {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Build a set of all TypeAlias names for quick membership tests.
        let alias_names: HashSet<&str> =
            basilisk_resolver::collect_name_set(&module.type_alias_defs);

        // Build a directed graph: alias name -> list of other alias names it
        // forward-references via string literals.
        let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();
        for alias in &module.type_alias_defs {
            let mut edges: Vec<&str> = Vec::new();
            for string_ref in &alias.rhs_string_refs {
                if alias_names.contains(string_ref.as_str()) {
                    edges.push(string_ref.as_str());
                }
            }
            if !edges.is_empty() {
                let _ = graph.insert(alias.name.as_str(), edges);
            }
        }

        // For each alias that has outgoing edges, check if it participates in
        // a cycle.
        for alias in &module.type_alias_defs {
            if !graph.contains_key(alias.name.as_str()) {
                continue;
            }
            if !has_cycle(alias.name.as_str(), &graph) {
                continue;
            }
            // Valid recursive aliases wrap the self-reference in a container
            // type (e.g. `list["Json"]`, `dict[str, "Json"]`).  When the RHS
            // references a container type, the recursion terminates through
            // structural nesting and is not truly infinite.
            if has_container_wrapper(alias) {
                continue;
            }
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!("Type alias `{}` creates a cyclical reference", alias.name),
                alias.span,
                &module.path,
                Some("Remove the self-reference or break the mutual reference cycle".to_owned()),
                Some(
                    "A TypeAlias whose RHS forward-references itself (directly or \
                     through another alias) produces an infinite type that cannot be resolved"
                        .to_owned(),
                ),
            ));
        }
    }
}
