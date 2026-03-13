//! BSK-E0104: Cyclical type alias reference.
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

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0104",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0104",
};

/// Emits BSK-E0104 when a `TypeAlias`-annotated assignment's RHS contains a
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

impl Rule for CyclicalTypeAliasReference {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Build a set of all TypeAlias names for quick membership tests.
        let alias_names: HashSet<&str> = module
            .type_alias_defs
            .iter()
            .map(|a| a.name.as_str())
            .collect();

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
            if has_cycle(alias.name.as_str(), &graph) {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!("Type alias `{}` creates a cyclical reference", alias.name),
                    span: alias.span,
                    path: module.path.clone(),
                    help: Some(
                        "Remove the self-reference or break the mutual reference cycle".to_owned(),
                    ),
                    note: Some(
                        "A TypeAlias whose RHS forward-references itself (directly or \
                         through another alias) produces an infinite type that cannot be resolved"
                            .to_owned(),
                    ),
                });
            }
        }
    }
}
