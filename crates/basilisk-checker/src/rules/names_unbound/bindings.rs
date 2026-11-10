//! Which names a statement or pattern BINDS, for the definite-assignment walk
//! ([CHKARCH-DIAG-TYPESAFETY], [NARROWPLAN-INTEGRATION] Step 8). See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//!
//! Pure functions over the Ruff AST: no walk state, no diagnostics. The walk
//! itself lives in [`super::scan`].

use std::collections::HashSet;

use ruff_python_ast::{Pattern, Stmt};

use crate::narrow::target_names;

use super::nested_bodies;

/// Merge live branch states into `bound`; `true` when no branch is live
/// (the whole construct diverges).
pub(super) fn merge_alive(bound: &mut HashSet<String>, alive: Vec<HashSet<String>>) -> bool {
    let merged = alive
        .into_iter()
        .reduce(|acc, set| acc.intersection(&set).cloned().collect());
    match merged {
        Some(names) => {
            *bound = names;
            false
        }
        None => true,
    }
}

/// Names a simple (non-branching) statement definitely binds.
pub(super) fn bind_statement_targets(stmt: &Stmt, bound: &mut HashSet<String>) {
    let mut names = Vec::new();
    match stmt {
        Stmt::Assign(node) => {
            for target in &node.targets {
                target_names(target, &mut names);
            }
        }
        Stmt::AnnAssign(node) => target_names(&node.target, &mut names),
        Stmt::AugAssign(node) => target_names(&node.target, &mut names),
        Stmt::FunctionDef(node) => names.push(node.name.to_string()),
        Stmt::ClassDef(node) => names.push(node.name.to_string()),
        Stmt::TypeAlias(node) => target_names(&node.name, &mut names),
        Stmt::Import(node) => names.extend(import_bound_names(node)),
        Stmt::ImportFrom(node) => names.extend(from_import_bound_names(node)),
        Stmt::Delete(node) => {
            let mut deleted = Vec::new();
            for target in &node.targets {
                target_names(target, &mut deleted);
            }
            for name in deleted {
                let _ = bound.remove(&name);
            }
        }
        _ => {}
    }
    bound.extend(names);
}

/// Names a plain `import` statement binds (`import a.b` binds `a`).
pub(super) fn import_bound_names(node: &ruff_python_ast::StmtImport) -> Vec<String> {
    node.names
        .iter()
        .map(|alias| {
            alias.asname.as_ref().map_or_else(
                || {
                    alias
                        .name
                        .split('.')
                        .next()
                        .unwrap_or(alias.name.as_str())
                        .to_string()
                },
                std::string::ToString::to_string,
            )
        })
        .collect()
}

/// Names a `from ... import ...` statement binds.
pub(super) fn from_import_bound_names(node: &ruff_python_ast::StmtImportFrom) -> Vec<String> {
    node.names
        .iter()
        .filter(|alias| alias.name.as_str() != "*")
        .map(|alias| {
            alias
                .asname
                .as_ref()
                .map_or_else(|| alias.name.to_string(), std::string::ToString::to_string)
        })
        .collect()
}

/// Capture names a `match` pattern binds when it matches.
pub(super) fn pattern_names(pattern: &Pattern, bound: &mut HashSet<String>) {
    match pattern {
        Pattern::MatchAs(node) => {
            if let Some(name) = &node.name {
                let _ = bound.insert(name.to_string());
            }
            if let Some(inner) = &node.pattern {
                pattern_names(inner, bound);
            }
        }
        Pattern::MatchStar(node) => {
            if let Some(name) = &node.name {
                let _ = bound.insert(name.to_string());
            }
        }
        Pattern::MatchMapping(node) => {
            if let Some(rest) = &node.rest {
                let _ = bound.insert(rest.to_string());
            }
            for inner in &node.patterns {
                pattern_names(inner, bound);
            }
        }
        Pattern::MatchOr(node) => {
            for inner in &node.patterns {
                pattern_names(inner, bound);
            }
        }
        Pattern::MatchSequence(node) => {
            for inner in &node.patterns {
                pattern_names(inner, bound);
            }
        }
        Pattern::MatchClass(node) => {
            for inner in &node.arguments.patterns {
                pattern_names(inner, bound);
            }
            for kw in &node.arguments.keywords {
                pattern_names(&kw.pattern, bound);
            }
        }
        Pattern::MatchValue(_) | Pattern::MatchSingleton(_) => {}
    }
}

/// `case _:` and bare `case name:` match anything.
pub(super) fn irrefutable(pattern: &Pattern) -> bool {
    matches!(pattern, Pattern::MatchAs(node) if node.pattern.is_none())
}

/// Collect `global`/`nonlocal` declarations (not entering nested scopes).
pub(super) fn collect_escaped(stmts: &[Stmt], out: &mut HashSet<String>) {
    for stmt in stmts {
        match stmt {
            Stmt::Global(node) => out.extend(node.names.iter().map(ToString::to_string)),
            Stmt::Nonlocal(node) => out.extend(node.names.iter().map(ToString::to_string)),
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
            _ => {
                for body in nested_bodies(stmt) {
                    collect_escaped(body, out);
                }
            }
        }
    }
}
