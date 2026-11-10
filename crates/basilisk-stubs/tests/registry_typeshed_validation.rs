//! Implements [RESOLV-CANONICAL-REGISTRY] × [STUBRES-TYPESHED-BASELINE]:
//! every definition site the canonical registry declares must actually be
//! bound by the pinned typeshed snapshot.
//!
//! A registry row naming a symbol typeshed does not define (for example a
//! `collections.Deque` when the real class is lowercase `deque`) invents an
//! API: binding resolution can then "canonicalise" a use site to a definition
//! that does not exist, and no other test notices because the registry only
//! ever talks to itself.

use std::collections::{BTreeMap, BTreeSet};

use basilisk_canonical::all_definition_sites;
use basilisk_stubs::typeshed::bundle::bundled_snapshot;
use basilisk_stubs::typeshed::snapshot::Snapshot;
use ruff_python_ast::{Alias, Expr, Stmt};

/// How many `from M import *` hops to follow before declaring a name unbound.
const MAX_STAR_DEPTH: usize = 8;

/// The stub source for a dotted module, trying the flat and package layouts.
fn stub_source<'s>(snapshot: &'s Snapshot, module: &str) -> Option<&'s str> {
    let path = module.replace('.', "/");
    snapshot
        .vfs
        .read_str(&format!("stdlib/{path}.pyi"))
        .or_else(|| snapshot.vfs.read_str(&format!("stdlib/{path}/__init__.pyi")))
}

/// The name an import alias binds in the importing module.
fn alias_binds(alias: &Alias, name: &str) -> bool {
    match &alias.asname {
        Some(asname) => asname.as_str() == name,
        None => alias.name.as_str() == name,
    }
}

/// Whether one module-scope statement binds `name`, recursing into the
/// version/platform guards and `try` fallbacks stubs use at module scope.
fn stmt_binds(stmt: &Stmt, name: &str) -> bool {
    match stmt {
        Stmt::ClassDef(class) => class.name.as_str() == name,
        Stmt::FunctionDef(func) => func.name.as_str() == name,
        Stmt::Assign(assign) => assign.targets.iter().any(|target| expr_is_name(target, name)),
        Stmt::AnnAssign(assign) => expr_is_name(&assign.target, name),
        Stmt::TypeAlias(alias) => expr_is_name(&alias.name, name),
        Stmt::ImportFrom(import) => import.names.iter().any(|alias| alias_binds(alias, name)),
        Stmt::Import(import) => import.names.iter().any(|alias| alias_binds(alias, name)),
        Stmt::If(if_stmt) => {
            body_binds(&if_stmt.body, name)
                || if_stmt
                    .elif_else_clauses
                    .iter()
                    .any(|clause| body_binds(&clause.body, name))
        }
        Stmt::Try(try_stmt) => {
            body_binds(&try_stmt.body, name)
                || body_binds(&try_stmt.orelse, name)
                || body_binds(&try_stmt.finalbody, name)
        }
        _ => false,
    }
}

/// Whether `expr` is the bare name `name`.
fn expr_is_name(expr: &Expr, name: &str) -> bool {
    matches!(expr, Expr::Name(bound) if bound.id.as_str() == name)
}

/// Whether any statement in a body binds `name`.
fn body_binds(body: &[Stmt], name: &str) -> bool {
    body.iter().any(|stmt| stmt_binds(stmt, name))
}

/// Modules star-imported (`from M import *`) at module scope.
fn star_sources(body: &[Stmt], out: &mut Vec<String>) {
    for stmt in body {
        match stmt {
            Stmt::ImportFrom(import) => {
                let is_star = import.names.iter().any(|alias| alias.name.as_str() == "*");
                if is_star {
                    if let Some(module) = import.module.as_ref() {
                        out.push(module.to_string());
                    }
                }
            }
            Stmt::If(if_stmt) => {
                star_sources(&if_stmt.body, out);
                for clause in &if_stmt.elif_else_clauses {
                    star_sources(&clause.body, out);
                }
            }
            Stmt::Try(try_stmt) => {
                star_sources(&try_stmt.body, out);
                star_sources(&try_stmt.orelse, out);
                star_sources(&try_stmt.finalbody, out);
            }
            _ => {}
        }
    }
}

/// Whether `module`'s stub binds `name` at module scope, following
/// `from M import *` re-export chains up to [`MAX_STAR_DEPTH`] hops.
fn module_binds(
    snapshot: &Snapshot,
    module: &str,
    name: &str,
    depth: usize,
    seen: &mut BTreeSet<String>,
) -> bool {
    if depth > MAX_STAR_DEPTH || !seen.insert(module.to_owned()) {
        return false;
    }
    let Some(source) = stub_source(snapshot, module) else {
        return false;
    };
    let Ok(parsed) = basilisk_parser::parse_source(source.to_owned(), format!("{module}.pyi"))
    else {
        return false;
    };
    if body_binds(&parsed.ast.body, name) {
        return true;
    }
    let mut stars = Vec::new();
    star_sources(&parsed.ast.body, &mut stars);
    stars
        .iter()
        .any(|star| module_binds(snapshot, star, name, depth + 1, seen))
}

/// Every registry definition site resolves to a real binding in the pinned
/// typeshed snapshot — no invented modules, no invented names, no wrong case.
#[test]
fn every_registry_definition_site_exists_in_bundled_typeshed(
) -> Result<(), Box<dyn std::error::Error>> {
    let snapshot = bundled_snapshot()?;
    let mut by_module: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (module, name, _form) in all_definition_sites() {
        by_module.entry(module).or_default().push(name);
    }
    assert!(
        !by_module.is_empty(),
        "registry enumerated no definition sites — the registry failed to load"
    );

    let mut missing: Vec<String> = Vec::new();
    for (module, names) in &by_module {
        if stub_source(&snapshot, module).is_none() {
            missing.push(format!("{module} (module has no stub in typeshed)"));
            continue;
        }
        for name in names {
            let mut seen = BTreeSet::new();
            if !module_binds(&snapshot, module, name, 0, &mut seen) {
                missing.push(format!("{module}.{name}"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "registry definition sites that bundled typeshed does not define: {missing:#?}"
    );
    Ok(())
}
