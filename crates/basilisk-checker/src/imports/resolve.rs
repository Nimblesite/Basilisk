//! Implements [ANALYSIS-CROSSLSP-IMPORT]. See docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-CROSSLSP-IMPORT
//! Filesystem path resolution: `module_name` + [`ImportSearchPaths`] → a file.

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

/// Whether the **bundled** (name-only) stdlib recognition should rescue an
/// otherwise-unresolved import — typing-spec import-resolution step 3.
///
/// When a custom typeshed is configured (`typeshed-path`) that
/// directory is *the canonical source for standard-library types*
/// ([STUBRES-CUSTOM-TYPESHED]): a stdlib module absent from its `stdlib/` subtree
/// must fall through to `imports_unresolved` rather than be silently rescued by
/// the vendored `phf` name-set. Callers therefore gate every bundled-stdlib
/// suppression on this helper instead of calling `is_stdlib_module` directly, so
/// canonicality is enforced in exactly one place.
///
/// With no custom typeshed this is identical to
/// [`basilisk_stubs::is_stdlib_module`], so the conformance path (which never
/// sets `typeshed-path`) is unaffected — the branch is purely additive.
#[must_use]
pub fn bundled_stdlib_recognized(module_name: &str, custom_typeshed_configured: bool) -> bool {
    !custom_typeshed_configured && basilisk_stubs::is_stdlib_module(module_name)
}

/// Resolve an absolute import following the typing spec's import-resolution
/// ordering ([STUBRES-PEP561]). Steps, in order:
///
/// 1. **Manual stubs** — `.pyi` files in `stub-paths` directories (head of path)
/// 2. **User source** — `.py`/`.pyi` files in workspace roots and `extraPaths`
/// 3. **Standard-library typeshed** — a custom `typeshed-path/stdlib/` tree when
///    configured ([STUBRES-CUSTOM-TYPESHED]); otherwise stdlib names are
///    recognised downstream via `basilisk_stubs::is_stdlib_module()`
/// 4. **Stub-only packages** — installed `foopkg-stubs` in site-packages
/// 5. **Inline-typed packages** — installed packages with a `py.typed` marker
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
    // 1. User stubs (stub-paths: only .pyi files)
    for stub_dir in &search_paths.stub_paths {
        if let Some(resolved) = try_resolve_stub_only(module_name, stub_dir, fs) {
            return Some(resolved);
        }
    }

    // 2. User source (workspace roots + workspace members + extraPaths: .pyi preferred over .py)
    for dir in search_paths
        .roots
        .iter()
        .chain(search_paths.workspace_members.iter())
        .chain(search_paths.extra_paths.iter())
    {
        if let Some(resolved) = try_resolve_in_dir(module_name, dir, fs) {
            return Some(resolved);
        }
    }

    // 3. Standard-library typeshed — typing-spec import-resolution step 3.
    //    A configured custom typeshed directory is the canonical source for
    //    standard-library types: resolve stdlib modules against its `stdlib/`
    //    subtree in preference to the bundled (name-only) recognition applied
    //    downstream. [STUBRES-CUSTOM-TYPESHED]
    if let Some(typeshed) = &search_paths.typeshed_path {
        if basilisk_stubs::is_stdlib_module(module_name) {
            let stdlib_dir = typeshed.join("stdlib");
            if let Some(resolved) = try_resolve_stub_only(module_name, &stdlib_dir, fs) {
                return Some(resolved);
            }
        }
    }

    // 4+5. Site-packages: stub-only packages (-stubs), then inline-typed (py.typed), then plain
    if let Some(sp) = &search_paths.site_packages {
        // 4. Check for `<module>-stubs` package first
        if let Some(resolved) = try_resolve_stub_package(module_name, sp, fs) {
            return Some(resolved);
        }
        // 5. Check for inline-typed packages (py.typed marker) and plain packages
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
    // Standard PEP 561 search first.
    if let Some(resolved) = resolve_module_cached(module_name, search_paths, fs) {
        return Some(resolved);
    }
    // Fall back to the importer's own directory — mirrors Python's sys.path[0].
    let importer_dir = importing_file?.parent()?;
    try_resolve_in_dir(module_name, importer_dir, fs)
}

/// Try resolving a module in a stub-only directory (only `.pyi` files).
fn try_resolve_stub_only(
    module_name: &str,
    stub_dir: &Path,
    fs: &FsCache,
) -> Option<ResolvedImport> {
    let parts: Vec<&str> = module_name.split('.').collect();
    let mut current = stub_dir.to_path_buf();

    let (leading, trailing) = parts.split_at(parts.len().saturating_sub(1));
    for &part in leading {
        current = current.join(part);
        if !fs.is_dir(&current) {
            return None;
        }
    }

    let last = trailing.first()?;

    // Only look for .pyi files in stub directories
    let pyi = current.join(format!("{last}.pyi"));
    if fs.is_file(&pyi) {
        return Some(ResolvedImport {
            path: pyi,
            resolution: ImportResolution::StubPyi,
        });
    }

    // Package stub `name/__init__.pyi`, gated on `name/` existing so missing
    // packages are answered from the parent's cached listing.
    let pkg_dir = current.join(last);
    if fs.is_dir(&pkg_dir) {
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
    let parts: Vec<&str> = module_name.split('.').collect();
    let mut current = dir.to_path_buf();

    // Navigate through package directories for all but the last part.
    let (leading, trailing) = parts.split_at(parts.len().saturating_sub(1));
    for &part in leading {
        current = current.join(part);
        if !fs.is_dir(&current) {
            return None;
        }
    }

    let last = trailing.first()?;
    try_resolve_name(&current, last, fs)
}

/// Try resolving a single name (the last segment) within a directory.
fn try_resolve_name(dir: &Path, name: &str, fs: &FsCache) -> Option<ResolvedImport> {
    // 1. name.pyi (stub preferred)
    let pyi = dir.join(format!("{name}.pyi"));
    if fs.is_file(&pyi) {
        return Some(ResolvedImport {
            path: pyi,
            resolution: ImportResolution::StubPyi,
        });
    }
    // 2. name.py
    let py = dir.join(format!("{name}.py"));
    if fs.is_file(&py) {
        return Some(ResolvedImport {
            path: py,
            resolution: ImportResolution::SourcePy,
        });
    }
    // 3+4. name/__init__.pyi (package stub), then name/__init__.py (package).
    // Gate both on `name/` being a directory first: that answer comes from the
    // parent's cached listing, so missing packages cost no filesystem probe.
    let pkg_dir = dir.join(name);
    if !fs.is_dir(&pkg_dir) {
        return None;
    }
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
