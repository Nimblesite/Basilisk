//! Implements [ANALYSIS-CROSSLSP-IMPORT]. See docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-CROSSLSP-IMPORT
//! Filesystem path resolution: `module_name` + [`ImportSearchPaths`] → a file.
//!
//! This is the whole of import resolution: a static filesystem search, never an
//! execution of the target program ([STUBRES-STATIC-MODEL]). A module the search
//! cannot find — including one only a runtime `sys.meta_path` hook or a computed
//! `importlib.import_module(name)` could produce — resolves to nothing here and
//! is surfaced downstream as `imports_unresolved`.

use std::path::Path;

use basilisk_resolver::scope::{ImportResolution, UnresolvedReason};

use super::fs_cache::FsCache;
use super::{ImportSearchPaths, ResolvedImport};

// Implements [LSPUV-DIAGNOSTICS-MODULE-NOT-FOUND] — the registry-driven
// classifier (NotInDeps / NeedsSync / NotInstalled) that makes "module not found"
// context-aware; the diagnostic message text is emitted in basilisk-checker.
/// Classify why an import is unresolved using the package registry.
#[must_use]
pub fn classify_unresolved(
    module_name: &str,
    search_paths: &ImportSearchPaths,
) -> UnresolvedReason {
    let Some(registry) = &search_paths.registry else {
        return UnresolvedReason::Unknown;
    };

    // The registry is keyed by import name (issue #25): look the module up
    // directly — full dotted name first (`google.protobuf`), then the root.
    let root_module = module_name.split('.').next().unwrap_or(module_name);
    if let Some(info) = registry
        .lookup(module_name)
        .or_else(|| registry.lookup(root_module))
    {
        if info.kind == basilisk_uv::DepKind::Transitive {
            return UnresolvedReason::NotInDeps;
        }
        // Package is known but not found on filesystem — needs sync
        return UnresolvedReason::NeedsSync;
    }

    UnresolvedReason::NotInstalled
}

/// Resolve an absolute import following the typing spec's import-resolution
/// ordering ([STUBRES-PEP561-NORMATIVE], [STUBRES-PEP561-MAPPING],
/// [STUBRES-RESOLUTION-FLOW], [STUBRES-PEP561]). Steps, in order:
///
/// 1. **Manual stubs** — `.pyi` files in `stub-paths` directories (head of path)
/// 2. **User source** — `.py`/`.pyi` files in workspace roots and `extraPaths`
/// 3. **Standard-library typeshed** — exactly one acquisition-promoted snapshot
///    (custom, downloaded, or bundled) ([STUBRES-CUSTOM-TYPESHED])
/// 4. **Stub-only packages** — installed `foopkg-stubs` in site-packages
/// 5. **Installed packages** — stub-only packages first, then source packages;
///    `py.typed` controls provenance/type completeness rather than existence
#[must_use]
pub fn resolve_module(
    module_name: &str,
    search_paths: &ImportSearchPaths,
) -> Option<ResolvedImport> {
    resolve_module_cached(module_name, search_paths, &FsCache::new())
}

/// [`resolve_module`] with a caller-supplied [`FsCache`], so a loop resolving
/// many imports shares one set of directory listings.
pub(crate) fn resolve_module_cached(
    module_name: &str,
    search_paths: &ImportSearchPaths,
    fs: &FsCache,
) -> Option<ResolvedImport> {
    resolve_module_cached_with_importer(module_name, search_paths, None, fs)
}

/// Execute the six-step resolver, optionally treating the importing file's
/// directory as step-2 user code (`sys.path[0]`).
fn resolve_module_cached_with_importer(
    module_name: &str,
    search_paths: &ImportSearchPaths,
    importer_dir: Option<&Path>,
    fs: &FsCache,
) -> Option<ResolvedImport> {
    // 1. Manually supplied path entries. The typing specification places both
    //    manual stubs and Python source at the beginning of the path. Basilisk
    //    models those as `stub_paths` (`.pyi` only) plus `extra_paths`
    //    (`.pyi` before `.py`).
    for stub_dir in &search_paths.stub_paths {
        if let Some(resolved) = try_resolve_stub_only(module_name, stub_dir, fs) {
            return Some(resolved);
        }
    }
    for dir in &search_paths.extra_paths {
        if let Some(resolved) = try_resolve_in_dir(module_name, dir, fs) {
            return Some(resolved);
        }
    }

    // 2. User source discovered from the project/workspace (`.pyi` before
    //    `.py`), followed by the importing script's directory.
    for dir in search_paths
        .roots
        .iter()
        .chain(search_paths.workspace_members.iter())
    {
        if let Some(resolved) = try_resolve_in_dir(module_name, dir, fs) {
            return Some(resolved);
        }
    }
    if let Some(dir) = importer_dir {
        if let Some(resolved) = try_resolve_in_dir(module_name, dir, fs) {
            return Some(resolved);
        }
    }

    // 3. Standard-library typeshed — typing-spec import-resolution step 3.
    //    The acquisition gate supplies exactly one canonical snapshot: custom,
    //    downloaded, or bundled. [STUBRES-CUSTOM-TYPESHED]
    if let Some(active) = &search_paths.typeshed_snapshot {
        if let Some((snapshot, target)) = active.for_importer(importer_dir) {
            let located = match target {
                Some(target) => snapshot.read_stub_for_target(module_name, target.python_version),
                None => snapshot.read_stub(module_name),
            };
            if let Some((logical_uri, _)) = located {
                return Some(ResolvedImport {
                    path: logical_uri.into(),
                    resolution: ImportResolution::StubPyi,
                });
            }
        }
    }

    // 4+5. Site-packages: stub-only packages (-stubs), then installed source.
    // A `py.typed` marker controls the downstream provenance/completeness
    // classification; its absence does not turn an installed module into an
    // unresolved import ([STUBRES-TYPING-STATUS]).
    if let Some(sp) = &search_paths.site_packages {
        // 4. Check for `<module>-stubs` package first. A miss in a complete
        // stub distribution is terminal; partial/namespace misses continue.
        if let Some(resolved) = try_resolve_stub_package(module_name, sp, fs) {
            return Some(resolved);
        }
        let root = module_name.split('.').next().unwrap_or(module_name);
        let stubs_dir = sp.join(format!("{root}-stubs"));
        if fs.is_dir(&stubs_dir) && !stub_package_miss_allows_fallback(module_name, &stubs_dir, fs)
        {
            return None;
        }
        // 5. Resolve installed source. Typed and untyped packages remain
        // distinct downstream states, but both are terminal resolutions.
        if let Some(resolved) = try_resolve_in_dir(module_name, sp, fs) {
            return Some(resolved);
        }
    }

    None
}

/// Resolve an absolute import, also searching the importing file's own directory.
///
/// Python adds the directory of the script being run to `sys.path[0]`, so a
/// bare `import foo` in `scripts/test.py` can resolve to `scripts/foo.py` even
/// when `scripts/` is not listed as a workspace root.  This function replicates
/// that behaviour by checking the importer's directory after the normal PEP 561
/// search order but before concluding that the import is unresolved.
///
/// Use this in place of [`resolve_module`] whenever the path of the importing
/// file is known (i.e. everywhere in the workspace resolver loop).
#[must_use]
pub fn resolve_module_with_importer(
    module_name: &str,
    search_paths: &ImportSearchPaths,
    importing_file: Option<&Path>,
) -> Option<ResolvedImport> {
    resolve_module_with_importer_cached(module_name, search_paths, importing_file, &FsCache::new())
}

/// [`resolve_module_with_importer`] with a caller-supplied [`FsCache`]; the
/// per-module import loop in [`super::resolve_module_imports`] uses this so
/// every import of a file shares one set of directory listings.
pub(crate) fn resolve_module_with_importer_cached(
    module_name: &str,
    search_paths: &ImportSearchPaths,
    importing_file: Option<&Path>,
    fs: &FsCache,
) -> Option<ResolvedImport> {
    resolve_module_cached_with_importer(
        module_name,
        search_paths,
        importing_file.and_then(Path::parent),
        fs,
    )
}

/// Try resolving a module in a stub-only directory (only `.pyi` files).
fn try_resolve_stub_only(
    module_name: &str,
    stub_dir: &Path,
    fs: &FsCache,
) -> Option<ResolvedImport> {
    if !module_name.contains('.') {
        return try_resolve_stub_name(stub_dir, module_name, fs);
    }
    let (leading, last) = module_name.rsplit_once('.')?;
    let mut current = stub_dir.to_path_buf();
    for part in leading.split('.') {
        current = current.join(part);
        if !fs.is_dir(&current) {
            return None;
        }
    }

    try_resolve_stub_name(&current, last, fs)
}

fn try_resolve_stub_name(dir: &Path, name: &str, fs: &FsCache) -> Option<ResolvedImport> {
    let pyi_name = format!("{name}.pyi");

    // Only look for .pyi files in stub directories
    if fs.contains_file(dir, pyi_name.as_ref()) {
        return Some(ResolvedImport {
            path: dir.join(pyi_name),
            resolution: ImportResolution::StubPyi,
        });
    }

    // Package stub `name/__init__.pyi`, gated on `name/` existing so missing
    // packages are answered from the parent's cached listing.
    if fs.contains_dir(dir, name.as_ref()) {
        let pkg_dir = dir.join(name);
        let pkg_pyi = pkg_dir.join("__init__.pyi");
        if fs.is_file(&pkg_pyi) {
            return Some(ResolvedImport {
                path: pkg_pyi,
                resolution: ImportResolution::StubPyi,
            });
        }
    }

    None
}

/// Try resolving a PEP 561 stub-only package (`foopkg-stubs/`) in site-packages.
fn try_resolve_stub_package(
    module_name: &str,
    site_packages: &Path,
    fs: &FsCache,
) -> Option<ResolvedImport> {
    let root = module_name.split('.').next()?;
    let stubs_dir = site_packages.join(format!("{root}-stubs"));
    if !fs.is_dir(&stubs_dir) {
        return None;
    }

    // Resolve within the stubs package directory
    let remainder = module_name.strip_prefix(root);
    match remainder {
        // Top-level: look for __init__.pyi in the stubs dir
        None | Some("") => {
            let init_pyi = stubs_dir.join("__init__.pyi");
            if fs.is_file(&init_pyi) {
                return Some(ResolvedImport {
                    path: init_pyi,
                    resolution: ImportResolution::StubPyi,
                });
            }
            None
        }
        // Sub-module: strip leading dot and resolve within stubs dir
        Some(sub) => {
            let sub_name = sub.strip_prefix('.').unwrap_or(sub);
            try_resolve_stub_only(sub_name, &stubs_dir, fs)
        }
    }
}

/// Whether a module miss in an existing step-4 stub distribution may proceed
/// to the inline package at step 5.
///
/// PEP 561 permits fallback only for an explicit `partial\n` marker or for a
/// stub-only namespace package (identified by the absence of `__init__.pyi`).
/// A regular package nested inside a namespace is complete again unless it has
/// its own partial marker.
fn stub_package_miss_allows_fallback(module_name: &str, stubs_dir: &Path, fs: &FsCache) -> bool {
    if has_partial_marker(stubs_dir) {
        return true;
    }
    if fs.is_file(&stubs_dir.join("__init__.pyi")) {
        return false;
    }

    let mut current = stubs_dir.to_path_buf();
    let mut components = module_name.split('.');
    let _root = components.next();
    let mut remainder = components.peekable();
    while let Some(component) = remainder.next() {
        // The final segment is the desired module, not an enclosing package.
        if remainder.peek().is_none() {
            break;
        }
        current.push(component);
        if has_partial_marker(&current) {
            return true;
        }
        if fs.is_file(&current.join("__init__.pyi")) {
            return false;
        }
    }
    true
}

/// PEP 561's exact partial-stub marker is the line `partial\n`.
fn has_partial_marker(directory: &Path) -> bool {
    std::fs::read_to_string(directory.join("py.typed"))
        .is_ok_and(|contents| contents.lines().any(|line| line.trim() == "partial"))
}

/// Resolve a relative import (`from . import X`, `from ..utils import Y`).
#[must_use]
pub fn resolve_relative_import(
    importing_file: &Path,
    level: u32,
    module_name: &str,
    _search_paths: &ImportSearchPaths,
) -> Option<ResolvedImport> {
    let fs = FsCache::new();
    let mut base = importing_file.parent()?.to_path_buf();
    for _ in 1..level {
        base = base.parent()?.to_path_buf();
    }
    if module_name.is_empty() {
        return try_resolve_init(&base, &fs);
    }
    try_resolve_in_dir(module_name, &base, &fs)
}

/// Try resolving a dotted module name within a single directory.
fn try_resolve_in_dir(module_name: &str, dir: &Path, fs: &FsCache) -> Option<ResolvedImport> {
    if !module_name.contains('.') {
        return try_resolve_name(dir, module_name, fs);
    }
    let (leading, last) = module_name.rsplit_once('.')?;
    let mut current = dir.to_path_buf();

    // Navigate through package directories for all but the last part.
    for part in leading.split('.') {
        current = current.join(part);
        if !fs.is_dir(&current) {
            return None;
        }
    }

    try_resolve_name(&current, last, fs)
}

/// Try resolving a single name (the last segment) within a directory.
fn try_resolve_name(dir: &Path, name: &str, fs: &FsCache) -> Option<ResolvedImport> {
    let mut file_name = String::with_capacity(name.len() + 4);
    file_name.push_str(name);
    file_name.push_str(".pyi");
    // 1. name.pyi (stub preferred)
    if fs.contains_file(dir, file_name.as_ref()) {
        return Some(ResolvedImport {
            path: dir.join(file_name),
            resolution: ImportResolution::StubPyi,
        });
    }
    // 2. name.py
    file_name.truncate(name.len());
    file_name.push_str(".py");
    if fs.contains_file(dir, file_name.as_ref()) {
        return Some(ResolvedImport {
            path: dir.join(file_name),
            resolution: ImportResolution::SourcePy,
        });
    }
    // 3+4. name/__init__.pyi (package stub), then name/__init__.py (package).
    // Gate both on `name/` being a directory first: that answer comes from the
    // parent's cached listing, so missing packages cost no filesystem probe.
    if !fs.contains_dir(dir, name.as_ref()) {
        return None;
    }
    let pkg_dir = dir.join(name);
    let pkg_pyi = pkg_dir.join("__init__.pyi");
    if fs.is_file(&pkg_pyi) {
        return Some(ResolvedImport {
            path: pkg_pyi,
            resolution: ImportResolution::StubPyi,
        });
    }
    let pkg_py = pkg_dir.join("__init__.py");
    if fs.is_file(&pkg_py) {
        return Some(ResolvedImport {
            path: pkg_py,
            resolution: ImportResolution::SourcePy,
        });
    }
    None
}

/// Try resolving a directory as a package (`__init__.py` or `__init__.pyi`).
fn try_resolve_init(dir: &Path, fs: &FsCache) -> Option<ResolvedImport> {
    let init_pyi = dir.join("__init__.pyi");
    if fs.is_file(&init_pyi) {
        return Some(ResolvedImport {
            path: init_pyi,
            resolution: ImportResolution::StubPyi,
        });
    }
    let init_py = dir.join("__init__.py");
    if fs.is_file(&init_py) {
        return Some(ResolvedImport {
            path: init_py,
            resolution: ImportResolution::SourcePy,
        });
    }
    None
}

/// Check whether an installed package has a `py.typed` marker (PEP 561).
#[must_use]
pub fn is_inline_typed_package(module_name: &str, site_packages: &Path) -> bool {
    let root = module_name.split('.').next().unwrap_or(module_name);
    let pkg_dir = site_packages.join(root);
    pkg_dir.join("py.typed").is_file()
}

/// Check whether a stub-only package exists for a module in site-packages.
#[must_use]
pub fn has_stub_package(module_name: &str, site_packages: &Path) -> bool {
    let root = module_name.split('.').next().unwrap_or(module_name);
    site_packages.join(format!("{root}-stubs")).is_dir()
}
