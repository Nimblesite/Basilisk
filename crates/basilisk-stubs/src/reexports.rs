//! Implements [STUBRES-PYI-REEXPORTS]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-PYI-REEXPORTS
//! Re-export following for stub modules (GitHub #312).
//!
//! A stub's public interface includes names it re-exports per the typing
//! spec's [import conventions](https://typing.python.org/en/latest/spec/distributing.html#import-conventions):
//! redundant-alias imports (`from y import x as x`, `import x as x`),
//! `__all__` entries, and the export sets of star-imported submodules
//! (`from .tasks import *`), followed recursively. Without this, a package
//! stub like typeshed's `asyncio/__init__.pyi` — whose whole API is
//! re-exported from submodules — appears to declare nothing, and
//! `imports_module_attribute` falsely flags `asyncio.sleep`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::types::{
    DunderAllItem, DunderAllMutation, StarReexport, StubModule, StubSource, StubTier,
};

/// All names `stub` re-exports beyond its own top-level definitions:
/// redundant-alias imports, `__all__` entries, and — recursively — the export
/// sets of star-imported stubs resolved relative to the stub's path.
#[must_use]
pub fn reexported_member_names(stub: &StubModule) -> HashSet<String> {
    let root = source_root(stub);
    let source = stub.source;
    let tier = stub.tier;
    let target = stub.target.clone();
    let mut loader = |module_name: &str| {
        let path = resolve_dotted_stub(&root, module_name)?;
        parse_filesystem_stub(&path, module_name, source, tier, target.as_ref())
    };
    reexported_member_names_with_loader(stub, &mut loader)
}

/// Compute re-exports through a caller-provided active-source loader.
///
/// The loader is keyed by canonical dotted module name, so archive VFS and
/// filesystem sources share target, `__all__`, star, and cycle semantics.
#[must_use]
pub fn reexported_member_names_with_loader(
    stub: &StubModule,
    loader: &mut impl FnMut(&str) -> Option<StubModule>,
) -> HashSet<String> {
    let mut names: HashSet<String> = stub.reexported_names.iter().cloned().collect();
    let mut stack = HashSet::from([stub.module_name.clone()]);
    if let Some(all) = effective_dunder_all(stub, loader, &mut stack) {
        names.extend(all);
    }
    names.extend(collect_star_targets(stub, loader, &mut stack));
    names
}

/// Resolve each star-import target and union its complete export set. The
/// recursion stack breaks cycles; valid finite chains have no arbitrary cap.
fn collect_star_targets(
    stub: &StubModule,
    loader: &mut impl FnMut(&str) -> Option<StubModule>,
    stack: &mut HashSet<String>,
) -> HashSet<String> {
    let mut names = HashSet::new();
    for target in &stub.star_reexports {
        let Some(module_name) = target_module_name(stub, target) else {
            continue;
        };
        let Some(target_stub) = loader(&module_name) else {
            continue;
        };
        names.extend(collect_star_exports(&target_stub, loader, stack));
    }
    names
}

/// Return one star-imported stub's export set: its `__all__` when
/// it defines one (authoritative, exactly like runtime `import *`), otherwise
/// its public top-level names plus its own re-exports, recursively.
fn collect_star_exports(
    stub: &StubModule,
    loader: &mut impl FnMut(&str) -> Option<StubModule>,
    stack: &mut HashSet<String>,
) -> HashSet<String> {
    if !stack.insert(stub.module_name.clone()) {
        return HashSet::new();
    }
    let default_exports: HashSet<String> = public_definition_names(stub)
        .chain(
            stub.reexported_names
                .iter()
                .filter(|name| !name.starts_with('_'))
                .cloned(),
        )
        .chain(collect_star_targets(stub, loader, stack))
        .collect();
    let exports = effective_dunder_all_with_default(stub, &default_exports, loader, stack)
        .unwrap_or(default_exports);
    let _ = stack.remove(&stub.module_name);
    exports
}

fn effective_dunder_all(
    stub: &StubModule,
    loader: &mut impl FnMut(&str) -> Option<StubModule>,
    stack: &mut HashSet<String>,
) -> Option<HashSet<String>> {
    effective_dunder_all_with_default(stub, &HashSet::new(), loader, stack)
}

fn effective_dunder_all_with_default(
    stub: &StubModule,
    default_exports: &HashSet<String>,
    loader: &mut impl FnMut(&str) -> Option<StubModule>,
    stack: &mut HashSet<String>,
) -> Option<HashSet<String>> {
    if stub.dunder_all_variants.iter().all(Vec::is_empty) {
        return stub
            .dunder_all
            .as_ref()
            .map(|entries| entries.iter().cloned().collect());
    }
    let alternatives = stub.dunder_all_variants.iter().map(|mutations| {
        if mutations.is_empty() {
            default_exports.clone()
        } else {
            evaluate_mutations(mutations, stub, loader, stack)
        }
    });
    Some(intersect_sets(alternatives))
}

fn evaluate_mutations(
    mutations: &[DunderAllMutation],
    stub: &StubModule,
    loader: &mut impl FnMut(&str) -> Option<StubModule>,
    stack: &mut HashSet<String>,
) -> HashSet<String> {
    let mut names = Vec::new();
    for mutation in mutations {
        match mutation {
            DunderAllMutation::Assign(items) => {
                names = resolve_items(items, stub, loader, stack);
            }
            DunderAllMutation::Extend(items) => {
                names.extend(resolve_items(items, stub, loader, stack));
            }
            DunderAllMutation::Append(name) => names.push(name.clone()),
            DunderAllMutation::Remove(name) => {
                if let Some(position) = names.iter().position(|entry| entry == name) {
                    let _ = names.remove(position);
                }
            }
        }
    }
    names.into_iter().collect()
}

fn resolve_items(
    items: &[DunderAllItem],
    stub: &StubModule,
    loader: &mut impl FnMut(&str) -> Option<StubModule>,
    stack: &mut HashSet<String>,
) -> Vec<String> {
    items
        .iter()
        .flat_map(|item| match item {
            DunderAllItem::Name(name) => vec![name.clone()],
            DunderAllItem::ModuleAll(target) => referenced_dunder_all(stub, target, loader, stack)
                .into_iter()
                .collect(),
        })
        .collect()
}

fn referenced_dunder_all(
    stub: &StubModule,
    target: &StarReexport,
    loader: &mut impl FnMut(&str) -> Option<StubModule>,
    stack: &mut HashSet<String>,
) -> HashSet<String> {
    let Some(module_name) = target_module_name(stub, target) else {
        return HashSet::new();
    };
    if !stack.insert(module_name.clone()) {
        return HashSet::new();
    }
    let names = loader(&module_name)
        .and_then(|target_stub| effective_dunder_all(&target_stub, loader, stack))
        .unwrap_or_default();
    let _ = stack.remove(&module_name);
    names
}

fn intersect_sets(mut alternatives: impl Iterator<Item = HashSet<String>>) -> HashSet<String> {
    let mut intersection = alternatives.next().unwrap_or_default();
    for alternative in alternatives {
        intersection.retain(|name| alternative.contains(name));
    }
    intersection
}

/// Top-level names a stub without `__all__` exposes to `import *`: definition
/// names that don't start with `_`. Qualified `Class.method` keys (which the
/// extractor also stores in `functions`/`overloads`) are not module members.
fn public_definition_names(stub: &StubModule) -> impl Iterator<Item = String> + '_ {
    stub.functions
        .keys()
        .chain(stub.overloads.keys())
        .chain(stub.classes.keys())
        .chain(stub.variables.keys())
        .filter(|name| !name.starts_with('_') && !name.contains('.'))
        .cloned()
}

fn target_module_name(stub: &StubModule, target: &StarReexport) -> Option<String> {
    if target.level == 0 {
        return (!target.module.is_empty()).then(|| target.module.clone());
    }
    let mut parts: Vec<&str> = stub.module_name.split('.').collect();
    if !is_package_stub(stub) {
        let _ = parts.pop();
    }
    for _ in 1..target.level {
        let _ = parts.pop()?;
    }
    parts.extend(target.module.split('.').filter(|part| !part.is_empty()));
    (!parts.is_empty()).then(|| parts.join("."))
}

fn is_package_stub(stub: &StubModule) -> bool {
    stub.path.file_name().and_then(|name| name.to_str()) == Some("__init__.pyi")
}

/// `<base>/a/b.pyi` or `<base>/a/b/__init__.pyi` for module path `a.b`;
/// `<base>/__init__.pyi` when the module path is empty (`from . import *`).
fn resolve_dotted_stub(base: &Path, module: &str) -> Option<PathBuf> {
    if module.is_empty() {
        return existing_file(base.join("__init__.pyi"));
    }
    let mut segments: Vec<&str> = module.split('.').collect();
    let last = segments.pop()?;
    let dir = segments
        .iter()
        .fold(base.to_path_buf(), |dir, segment| dir.join(segment));
    existing_file(dir.join(format!("{last}.pyi")))
        .or_else(|| existing_file(dir.join(last).join("__init__.pyi")))
}

/// `path` if it exists as a file, else `None`.
fn existing_file(path: PathBuf) -> Option<PathBuf> {
    path.is_file().then_some(path)
}

fn parse_filesystem_stub(
    path: &Path,
    module_name: &str,
    source: StubSource,
    tier: StubTier,
    target: Option<&crate::types::StubTarget>,
) -> Option<StubModule> {
    target.map_or_else(
        || crate::pyi_parser::parse_pyi_file(path, module_name, source, tier).ok(),
        |target| {
            crate::pyi_parser::parse_pyi_file_for_target(path, module_name, source, tier, target)
                .ok()
        },
    )
}

fn source_root(stub: &StubModule) -> PathBuf {
    let mut root = stub
        .path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .to_path_buf();
    let module_depth = stub
        .module_name
        .split('.')
        .filter(|part| !part.is_empty())
        .count();
    let package_depth = if is_package_stub(stub) {
        module_depth
    } else {
        module_depth.saturating_sub(1)
    };
    for _ in 0..package_depth {
        if let Some(parent) = root.parent() {
            root = parent.to_path_buf();
        }
    }
    root
}
