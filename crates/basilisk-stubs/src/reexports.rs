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

use crate::types::{StarReexport, StubModule, StubSource, StubTier};

/// Upper bound on transitive star-import hops. Real stub trees are shallow
/// (typeshed's asyncio is depth 1); the cap only stops pathological nesting.
const MAX_STAR_DEPTH: usize = 16;

/// All names `stub` re-exports beyond its own top-level definitions:
/// redundant-alias imports, `__all__` entries, and — recursively — the export
/// sets of star-imported stubs resolved relative to the stub's path.
#[must_use]
pub fn reexported_member_names(stub: &StubModule) -> HashSet<String> {
    let mut names: HashSet<String> = stub.reexported_names.iter().cloned().collect();
    names.extend(stub.dunder_all.iter().flatten().cloned());
    let mut visited = HashSet::from([stub.path.clone()]);
    collect_star_targets(
        &stub.path,
        &stub.star_reexports,
        &mut names,
        &mut visited,
        0,
    );
    names
}

/// Resolve each star-import target of `from` and fold its export set into
/// `names`. `visited` breaks import cycles; `depth` caps runaway nesting.
fn collect_star_targets(
    from: &Path,
    targets: &[StarReexport],
    names: &mut HashSet<String>,
    visited: &mut HashSet<PathBuf>,
    depth: usize,
) {
    if depth >= MAX_STAR_DEPTH {
        return;
    }
    for target in targets {
        let Some(path) = resolve_star_target(from, target) else {
            continue;
        };
        if visited.insert(path.clone()) {
            collect_star_exports(&path, names, visited, depth);
        }
    }
}

/// Fold one star-imported stub's export set into `names`: its `__all__` when
/// it defines one (authoritative, exactly like runtime `import *`), otherwise
/// its public top-level names plus its own re-exports, recursively.
fn collect_star_exports(
    path: &Path,
    names: &mut HashSet<String>,
    visited: &mut HashSet<PathBuf>,
    depth: usize,
) {
    // Source/tier don't affect name extraction — only names are read here.
    let Ok(stub) = crate::parse_pyi_file(
        path,
        &module_name_of(path),
        StubSource::UserStub,
        StubTier::Tier1,
    ) else {
        return;
    };
    if let Some(all) = &stub.dunder_all {
        names.extend(all.iter().cloned());
        return;
    }
    names.extend(public_definition_names(&stub));
    names.extend(
        stub.reexported_names
            .iter()
            .filter(|name| !name.starts_with('_'))
            .cloned(),
    );
    collect_star_targets(path, &stub.star_reexports, names, visited, depth + 1);
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

/// Resolve a `from <module> import *` target to a `.pyi` path.
///
/// Relative-import semantics: level 1 is the stub's own package — the
/// directory containing the file, for `__init__.pyi` and plain module stubs
/// alike — and each extra level climbs one parent. Absolute star imports
/// (level 0) are not followed: intra-package re-exports in stubs are
/// conventionally relative (typeshed style), and absolute targets would need
/// the full search-path context this crate deliberately doesn't hold.
fn resolve_star_target(from: &Path, target: &StarReexport) -> Option<PathBuf> {
    if target.level == 0 {
        return None;
    }
    let mut base = from.parent()?.to_path_buf();
    for _ in 1..target.level {
        base = base.parent()?.to_path_buf();
    }
    resolve_dotted_stub(&base, &target.module)
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

/// Module name for a stub path: the file stem, or the package directory name
/// for an `__init__.pyi`.
fn module_name_of(path: &Path) -> String {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("stub");
    if stem == "__init__" {
        return path
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .unwrap_or(stem)
            .to_owned();
    }
    stem.to_owned()
}
