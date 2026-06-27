//! Implements [ANALYSIS-CROSSLSP-IMPORT]. See docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-CROSSLSP-IMPORT
//!
//! Import resolution engine — resolves `import X` to filesystem paths.
//!
//! Search order: workspace roots → extraPaths → venv site-packages.
//! File priority: `.pyi` stub preferred over `.py` source.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use basilisk_resolver::scope::{
    ImportKind, ImportResolution, ImportedModuleApi, PackageDepKind, UnresolvedReason,
};
use basilisk_uv::PackageRegistry;

use crate::workspace::WorkspaceIndex;

/// Result of resolving a single import to a filesystem path.
#[derive(Debug, Clone)]
pub struct ResolvedImport {
    /// Filesystem path of the resolved module.
    pub path: PathBuf,
    /// Whether this resolved to a `.pyi` stub or `.py` source.
    pub resolution: ImportResolution,
}

/// Search paths used for import resolution.
#[derive(Debug, Clone)]
pub struct ImportSearchPaths {
    /// Workspace root directories.
    pub roots: Vec<PathBuf>,
    /// Extra module search paths from config (`extraPaths`).
    pub extra_paths: Vec<PathBuf>,
    /// User-provided stub directories from `stub-paths` config.
    pub stub_paths: Vec<PathBuf>,
    /// uv workspace member source roots (editable packages).
    ///
    /// Searched after workspace roots but before `extra_paths`.
    pub workspace_members: Vec<PathBuf>,
    /// Virtual environment site-packages directory, if detected.
    pub site_packages: Option<PathBuf>,
    /// Package registry from uv lock file, if available.
    pub registry: Option<Arc<PackageRegistry>>,
}

impl ImportSearchPaths {
    /// Build search paths from workspace config.
    ///
    /// Automatically includes `.basilisk/stubs/` in each root as a stub
    /// search path for auto-generated Tier 3 stubs.
    #[must_use]
    pub fn from_config(
        roots: &[PathBuf],
        config: &crate::config::WorkspaceConfig,
        registry: Option<Arc<PackageRegistry>>,
    ) -> Self {
        let site_packages = resolve_site_packages(roots, config);

        // Include user-configured stub paths + auto-generated stub cache dirs.
        let mut stub_paths = config.stub_paths.clone();
        for root in roots {
            let generated_stubs = root.join(basilisk_stubs::generate::cache::DEFAULT_CACHE_DIR);
            if generated_stubs.is_dir() {
                stub_paths.push(generated_stubs);
            }
        }

        Self {
            roots: roots.to_vec(),
            extra_paths: config.extra_paths.clone(),
            // Discover uv workspace members / `src/` layouts here so every
            // consumer (CLI and LSP) resolves first-party imports the same
            // way. Implements [LSPUV-WORKSPACE-IMPORT-RESOLUTION] (issue #24).
            workspace_members: basilisk_uv::discover_workspace_members(roots),
            stub_paths,
            site_packages,
            registry,
        }
    }
}

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

/// Resolve an absolute import following PEP 561 resolution order.
///
/// 1. **User stubs** — `.pyi` files in `stub-paths` directories
/// 2. **User source** — `.py`/`.pyi` files in workspace roots and `extraPaths`
/// 3. **Stub-only packages** — installed `foopkg-stubs` in site-packages
/// 4. **Inline-typed packages** — installed packages with `py.typed` marker
/// 5. **Bundled typeshed** — handled externally via `basilisk_stubs::is_stdlib_module()`
#[must_use]
pub fn resolve_module(
    module_name: &str,
    search_paths: &ImportSearchPaths,
) -> Option<ResolvedImport> {
    // 1. User stubs (stub-paths: only .pyi files)
    for stub_dir in &search_paths.stub_paths {
        if let Some(resolved) = try_resolve_stub_only(module_name, stub_dir) {
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
        if let Some(resolved) = try_resolve_in_dir(module_name, dir) {
            return Some(resolved);
        }
    }

    // 3+4. Site-packages: stub-only packages (-stubs), then inline-typed (py.typed), then plain
    if let Some(sp) = &search_paths.site_packages {
        // 3. Check for `<module>-stubs` package first
        if let Some(resolved) = try_resolve_stub_package(module_name, sp) {
            return Some(resolved);
        }
        // 4. Check for inline-typed packages (py.typed marker) and plain packages
        if let Some(resolved) = try_resolve_in_dir(module_name, sp) {
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
    // Standard PEP 561 search first.
    if let Some(resolved) = resolve_module(module_name, search_paths) {
        return Some(resolved);
    }
    // Fall back to the importer's own directory — mirrors Python's sys.path[0].
    let importer_dir = importing_file?.parent()?;
    try_resolve_in_dir(module_name, importer_dir)
}

/// Try resolving a module in a stub-only directory (only `.pyi` files).
fn try_resolve_stub_only(module_name: &str, stub_dir: &Path) -> Option<ResolvedImport> {
    let parts: Vec<&str> = module_name.split('.').collect();
    let mut current = stub_dir.to_path_buf();

    let (leading, trailing) = parts.split_at(parts.len().saturating_sub(1));
    for &part in leading {
        current = current.join(part);
        if !current.is_dir() {
            return None;
        }
    }

    let last = trailing.first()?;

    // Only look for .pyi files in stub directories
    let pyi = current.join(format!("{last}.pyi"));
    if pyi.is_file() {
        return Some(ResolvedImport {
            path: pyi,
            resolution: ImportResolution::StubPyi,
        });
    }

    let pkg_pyi = current.join(last).join("__init__.pyi");
    if pkg_pyi.is_file() {
        return Some(ResolvedImport {
            path: pkg_pyi,
            resolution: ImportResolution::StubPyi,
        });
    }

    None
}

/// Try resolving a PEP 561 stub-only package (`foopkg-stubs/`) in site-packages.
fn try_resolve_stub_package(module_name: &str, site_packages: &Path) -> Option<ResolvedImport> {
    let root = module_name.split('.').next()?;
    let stubs_dir = site_packages.join(format!("{root}-stubs"));
    if !stubs_dir.is_dir() {
        return None;
    }

    // Resolve within the stubs package directory
    let remainder = module_name.strip_prefix(root);
    match remainder {
        // Top-level: look for __init__.pyi in the stubs dir
        None | Some("") => {
            let init_pyi = stubs_dir.join("__init__.pyi");
            if init_pyi.is_file() {
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
            try_resolve_stub_only(sub_name, &stubs_dir)
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
    let mut base = importing_file.parent()?.to_path_buf();
    for _ in 1..level {
        base = base.parent()?.to_path_buf();
    }
    if module_name.is_empty() {
        return try_resolve_init(&base);
    }
    try_resolve_in_dir(module_name, &base)
}

/// Try resolving a dotted module name within a single directory.
fn try_resolve_in_dir(module_name: &str, dir: &Path) -> Option<ResolvedImport> {
    let parts: Vec<&str> = module_name.split('.').collect();
    let mut current = dir.to_path_buf();

    // Navigate through package directories for all but the last part.
    let (leading, trailing) = parts.split_at(parts.len().saturating_sub(1));
    for &part in leading {
        current = current.join(part);
        if !current.is_dir() {
            return None;
        }
    }

    let last = trailing.first()?;
    try_resolve_name(&current, last)
}

/// Try resolving a single name (the last segment) within a directory.
fn try_resolve_name(dir: &Path, name: &str) -> Option<ResolvedImport> {
    // 1. name.pyi (stub preferred)
    let pyi = dir.join(format!("{name}.pyi"));
    if pyi.is_file() {
        return Some(ResolvedImport {
            path: pyi,
            resolution: ImportResolution::StubPyi,
        });
    }
    // 2. name.py
    let py = dir.join(format!("{name}.py"));
    if py.is_file() {
        return Some(ResolvedImport {
            path: py,
            resolution: ImportResolution::SourcePy,
        });
    }
    // 3. name/__init__.pyi (package stub)
    let pkg_pyi = dir.join(name).join("__init__.pyi");
    if pkg_pyi.is_file() {
        return Some(ResolvedImport {
            path: pkg_pyi,
            resolution: ImportResolution::StubPyi,
        });
    }
    // 4. name/__init__.py (package)
    let pkg_py = dir.join(name).join("__init__.py");
    if pkg_py.is_file() {
        return Some(ResolvedImport {
            path: pkg_py,
            resolution: ImportResolution::SourcePy,
        });
    }
    None
}

/// Try resolving a directory as a package (`__init__.py` or `__init__.pyi`).
fn try_resolve_init(dir: &Path) -> Option<ResolvedImport> {
    let init_pyi = dir.join("__init__.pyi");
    if init_pyi.is_file() {
        return Some(ResolvedImport {
            path: init_pyi,
            resolution: ImportResolution::StubPyi,
        });
    }
    let init_py = dir.join("__init__.py");
    if init_py.is_file() {
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

/// Detect site-packages directory from venv config, then fall back to
/// `python3 -c "import sys; ..."` subprocess for system Python discovery.
///
/// Reads the `VIRTUAL_ENV` environment variable as the highest-priority
/// override — `source .venv/bin/activate` (and CI scripts that install
/// dependencies into a venv outside the workspace tree) set this to the
/// active venv root.
fn resolve_site_packages(
    roots: &[PathBuf],
    config: &crate::config::WorkspaceConfig,
) -> Option<PathBuf> {
    let virtual_env = std::env::var_os("VIRTUAL_ENV").map(PathBuf::from);
    resolve_site_packages_with_env(roots, config, virtual_env.as_deref())
}

/// `resolve_site_packages` with the `VIRTUAL_ENV` value injected.
///
/// Split out so tests can exercise the env-var path without mutating process
/// state (which is `unsafe` under the project's `unsafe_code = "deny"` lint).
fn resolve_site_packages_with_env(
    roots: &[PathBuf],
    config: &crate::config::WorkspaceConfig,
    virtual_env: Option<&Path>,
) -> Option<PathBuf> {
    // 1. Honour an active venv signalled by `VIRTUAL_ENV` — the standard
    //    Python convention. Issue #25.
    if let Some(venv) = virtual_env {
        if venv.is_dir() {
            if let Some(sp) = site_packages_in_dir(venv) {
                return Some(sp);
            }
        }
    }
    // 2. Try venv-based discovery from workspace roots / explicit config.
    if let Some(sp) = resolve_venv_site_packages(roots, config) {
        return Some(sp);
    }
    // 3. Fall back to Python subprocess discovery.
    detect_python_site_packages()
}

/// Find site-packages from a virtual environment directory.
fn resolve_venv_site_packages(
    roots: &[PathBuf],
    config: &crate::config::WorkspaceConfig,
) -> Option<PathBuf> {
    let venv_dir = find_venv_dir(roots, config)?;
    site_packages_in_dir(&venv_dir)
}

/// Search a Python installation directory for its site-packages path.
fn site_packages_in_dir(base: &Path) -> Option<PathBuf> {
    // Unix: lib/pythonX.Y/site-packages
    let lib = base.join("lib");
    if lib.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&lib) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("python") {
                    let sp = entry.path().join("site-packages");
                    if sp.is_dir() {
                        return Some(sp);
                    }
                }
            }
        }
    }
    // Windows: Lib/site-packages
    let win_sp = base.join("Lib").join("site-packages");
    if win_sp.is_dir() {
        return Some(win_sp);
    }
    None
}

/// Detect site-packages by running `python3 -c "import sys; ..."`.
///
/// Searches `sys.path` entries for directories ending in `site-packages`.
/// Returns the first valid site-packages directory found.
fn detect_python_site_packages() -> Option<PathBuf> {
    let output = std::process::Command::new("python3")
        .args(["-c", "import sys; print('\\n'.join(sys.path))"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let path = PathBuf::from(trimmed);
        if path.ends_with("site-packages") && path.is_dir() {
            return Some(path);
        }
    }
    None
}

/// Find the venv directory from config or by scanning workspace roots.
fn find_venv_dir(roots: &[PathBuf], config: &crate::config::WorkspaceConfig) -> Option<PathBuf> {
    // 1. Explicit venv path from config.
    if let Some(venv_path) = &config.venv_path {
        let venv_name = config.venv.as_deref().unwrap_or(".venv");
        let candidate = venv_path.join(venv_name);
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    // 2. Common venv directories in workspace roots.
    let venv_names = [".venv", "venv", ".env", "env"];
    for root in roots {
        for name in &venv_names {
            let candidate = root.join(name);
            if candidate.is_dir() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Resolve imports for all files in the workspace index.
///
/// Iterates every file in the index, resolves each `ImportInfo`, and updates
/// its `resolution`, `resolved_path`, and `package_dep_kind` fields in place.
pub fn resolve_workspace_imports(index: &WorkspaceIndex, search_paths: &ImportSearchPaths) {
    for mut entry in index.files.iter_mut() {
        let Some(resolved_arc) = entry.value_mut().resolved.take() else {
            continue;
        };
        let mut resolved = Arc::try_unwrap(resolved_arc).unwrap_or_else(|arc| (*arc).clone());
        resolve_module_imports(&mut resolved, search_paths);
        entry.value_mut().resolved = Some(Arc::new(resolved));
    }
}

/// Resolve every import in a single module against the search paths, in place.
///
/// Sets each `ImportInfo`'s `resolution`, `resolved_path`, `unresolved_reason`,
/// and uv package metadata. Shared by the whole-workspace scan
/// ([`resolve_workspace_imports`]) and the incremental single-file analysis
/// path so both agree on what resolves — preventing false `imports_unresolved` in the
/// editor for third-party imports that the CLI resolves.
/// Implements [ANALYSIS-INCR-IMPORTS].
pub fn resolve_module_imports(
    resolved: &mut basilisk_resolver::ResolvedModule,
    search_paths: &ImportSearchPaths,
) {
    // The file's own path, used to search its directory for sibling modules.
    let importing_file = PathBuf::from(&resolved.path);

    // Member APIs captured during the loop, inserted after it ends — we cannot
    // borrow `resolved.imported_modules` while iterating `resolved.imports`.
    let mut captured: Vec<(String, ImportedModuleApi)> = Vec::new();

    for import in &mut resolved.imports {
        let result = match import.kind {
            ImportKind::Plain | ImportKind::From | ImportKind::Star => {
                resolve_module_with_importer(&import.module, search_paths, Some(&importing_file))
            }
        };
        if let Some(r) = result {
            import.resolution = r.resolution;
            import.resolved_path = Some(r.path);
        } else if !basilisk_stubs::is_stdlib_module(&import.module) {
            // Classify why the import is unresolved for actionable diagnostics.
            import.unresolved_reason = Some(classify_unresolved(&import.module, search_paths));
        }

        // Capture the member API of plain `import X` statements backed by a
        // user/local stub, so `imports_module_attribute` can flag `X.undeclared_attr`. Only
        // single-segment plain imports resolved to a `.pyi` under a configured
        // `stub-paths` dir (Phase 1: the stubs the developer owns).
        if let Some((binding, api)) = capture_user_stub_api(import, search_paths) {
            captured.push((binding, api));
        }

        // Annotate with package metadata from the uv registry.
        enrich_package_metadata(import, search_paths);
    }

    for (binding, api) in captured {
        let _ = resolved.imported_modules.insert(binding, api);
    }
}

/// Build the [`ImportedModuleApi`] for a plain `import X` backed by a user stub,
/// or `None` if this import is out of scope (aliased/dotted/from-import, not a
/// user stub, or the stub fails to parse).
fn capture_user_stub_api(
    import: &basilisk_resolver::ImportInfo,
    search_paths: &ImportSearchPaths,
) -> Option<(String, ImportedModuleApi)> {
    if import.kind != ImportKind::Plain || import.module.contains('.') {
        return None;
    }
    let stub_path = import.resolved_path.as_ref()?;
    if stub_path.extension().is_none_or(|ext| ext != "pyi") {
        return None;
    }
    // A user stub is a `.pyi` under one of the configured stub-paths (which
    // includes the auto-added `.basilisk/stubs`). Other `.pyi` (typeshed,
    // `*-stubs`, py.typed packages) are deferred to Phase 2.
    if !search_paths
        .stub_paths
        .iter()
        .any(|dir| stub_path.starts_with(dir))
    {
        return None;
    }

    let stub = basilisk_stubs::parse_pyi_file(
        stub_path,
        &import.module,
        basilisk_stubs::StubSource::UserStub,
        basilisk_stubs::StubTier::Tier1,
    )
    .ok()?;

    let mut member_names = std::collections::HashSet::new();
    member_names.extend(stub.functions.keys().cloned());
    member_names.extend(stub.classes.keys().cloned());
    member_names.extend(stub.variables.keys().cloned());
    member_names.extend(stub.overloads.keys().cloned());

    let has_getattr = stub.functions.contains_key("__getattr__");

    Some((
        import.module.clone(),
        ImportedModuleApi {
            member_names,
            has_getattr,
            stub_path: stub_path.clone(),
        },
    ))
}

/// Enrich an import with package metadata from the uv registry.
///
/// Sets `package_dep_kind`, `package_version`, and `package_name` in a
/// single registry lookup. No-op for stdlib modules, non-uv projects, or
/// imports not found in the registry.
fn enrich_package_metadata(
    import: &mut basilisk_resolver::ImportInfo,
    search_paths: &ImportSearchPaths,
) {
    let Some(registry) = search_paths.registry.as_ref() else {
        return;
    };

    // Skip stdlib modules — they have no package metadata.
    if basilisk_stubs::is_stdlib_module(&import.module) {
        return;
    }

    // The registry is keyed by import name (issue #25): look the module up
    // directly — full dotted name first (`google.protobuf`), then the root.
    let root_module = import.module.split('.').next().unwrap_or(&import.module);
    let Some(info) = registry
        .lookup(&import.module)
        .or_else(|| registry.lookup(root_module))
    else {
        return;
    };

    import.package_dep_kind = Some(match info.kind {
        basilisk_uv::DepKind::Direct => PackageDepKind::Direct,
        basilisk_uv::DepKind::Dev => PackageDepKind::Dev,
        basilisk_uv::DepKind::Transitive => PackageDepKind::Transitive,
    });
    import.package_version = Some(info.version.clone());
    import.package_name = Some(info.name.clone());
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test-only code: unwrap acceptable in unit tests"
)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use std::fs;

    static TEST_CTR: AtomicU64 = AtomicU64::new(0);

    /// Generate a unique temp dir path to avoid races between parallel tests.
    fn unique_tmp(prefix: &str) -> PathBuf {
        let n = TEST_CTR.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("{prefix}_{n}_{}", std::process::id()))
    }

    /// Create a unique tmp dir named `<prefix>_<n>_<pid>` and return its path.
    /// The dir is left in place; tests should clean up with `fs::remove_dir_all` at the end.
    fn make_tmp_dir(prefix: &str) -> PathBuf {
        let dir = unique_tmp(prefix);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Create `<parent>/<pkg_name>/` and populate it with empty files named in
    /// `files`. Returns the package directory path. Collapses the
    /// `create_dir_all` + `fs::write(empty)` boilerplate that every package
    /// resolution test repeats.
    fn make_pkg(parent: &Path, pkg_name: &str, files: &[&str]) -> PathBuf {
        let pkg = parent.join(pkg_name);
        fs::create_dir_all(&pkg).unwrap();
        for f in files {
            fs::write(pkg.join(f), "").unwrap();
        }
        pkg
    }

    fn make_search_paths(roots: Vec<PathBuf>) -> ImportSearchPaths {
        ImportSearchPaths {
            roots,
            extra_paths: vec![],
            stub_paths: vec![],
            workspace_members: vec![],
            site_packages: None,
            registry: None,
        }
    }

    /// Build a `ResolvedModule` with a single plain `import <module>` statement.
    fn module_with_plain_import(module: &str) -> basilisk_resolver::ResolvedModule {
        basilisk_resolver::ResolvedModule {
            path: "test.py".to_owned(),
            imports: vec![basilisk_resolver::ImportInfo {
                module: module.to_owned(),
                names: vec![],
                span: basilisk_resolver::Span::new(0, 0),
                kind: ImportKind::Plain,
                resolution: ImportResolution::Unresolved,
                resolved_path: None,
                package_dep_kind: None,
                package_version: None,
                package_name: None,
                unresolved_reason: None,
            }],
            ..basilisk_resolver::ResolvedModule::default()
        }
    }

    #[test]
    fn captures_user_stub_member_api() {
        let stub_dir = make_tmp_dir("bsk_ir_userstub");
        fs::write(
            stub_dir.join("cowsay.pyi"),
            "from typing import Any\ndef tux(text: str) -> None: ...\ndef __getattr__(name: str) -> Any: ...\n",
        )
        .unwrap();

        let mut paths = make_search_paths(vec![]);
        paths.stub_paths = vec![stub_dir.clone()];

        let mut resolved = module_with_plain_import("cowsay");
        resolve_module_imports(&mut resolved, &paths);

        let api = resolved.imported_modules.get("cowsay").unwrap();
        assert!(api.member_names.contains("tux"));
        assert!(api.has_getattr, "module-level __getattr__ must be detected");
        assert!(api.stub_path.ends_with("cowsay.pyi"));

        let _ = fs::remove_dir_all(&stub_dir);
    }

    #[test]
    fn user_stub_without_getattr_is_strict() {
        let stub_dir = make_tmp_dir("bsk_ir_userstub_strict");
        fs::write(stub_dir.join("widget.pyi"), "def render() -> None: ...\n").unwrap();

        let mut paths = make_search_paths(vec![]);
        paths.stub_paths = vec![stub_dir.clone()];

        let mut resolved = module_with_plain_import("widget");
        resolve_module_imports(&mut resolved, &paths);

        let api = resolved.imported_modules.get("widget").unwrap();
        assert!(api.member_names.contains("render"));
        assert!(!api.has_getattr);

        let _ = fs::remove_dir_all(&stub_dir);
    }

    #[test]
    fn does_not_capture_non_stub_import() {
        // A plain `.py` source resolution (not a user stub) is not captured.
        let dir = make_tmp_dir("bsk_ir_nonstub");
        fs::write(dir.join("plainmod.py"), "x = 1\n").unwrap();

        let paths = make_search_paths(vec![dir.clone()]);
        let mut resolved = module_with_plain_import("plainmod");
        resolve_module_imports(&mut resolved, &paths);

        assert!(
            resolved.imported_modules.is_empty(),
            "non-user-stub imports must not populate imported_modules"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_simple_module() {
        let dir = make_tmp_dir("bsk_ir_simple");
        fs::write(dir.join("foo.py"), "x = 1\n").unwrap();

        let paths = make_search_paths(vec![dir.clone()]);
        let result = resolve_module("foo", &paths);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.resolution, ImportResolution::SourcePy);
        assert!(r.path.ends_with("foo.py"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_prefers_pyi() {
        let dir = make_tmp_dir("bsk_ir_pyi");
        fs::write(dir.join("bar.py"), "x = 1\n").unwrap();
        fs::write(dir.join("bar.pyi"), "x: int\n").unwrap();

        let paths = make_search_paths(vec![dir.clone()]);
        let result = resolve_module("bar", &paths).unwrap();
        assert_eq!(result.resolution, ImportResolution::StubPyi);
        assert!(result.path.ends_with("bar.pyi"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_package_init() {
        let dir = unique_tmp("bsk_ir_pkg");
        let _pkg = make_pkg(&dir, "mypkg", &["__init__.py"]);

        let paths = make_search_paths(vec![dir.clone()]);
        let result = resolve_module("mypkg", &paths).unwrap();
        assert_eq!(result.resolution, ImportResolution::SourcePy);
        assert!(result.path.ends_with("__init__.py"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_dotted_module() {
        let dir = unique_tmp("bsk_ir_dotted");
        let sub = dir.join("pkg").join("sub");
        fs::create_dir_all(&sub).unwrap();
        fs::write(dir.join("pkg").join("__init__.py"), "").unwrap();
        fs::write(sub.join("mod.py"), "x = 1\n").unwrap();

        let paths = make_search_paths(vec![dir.clone()]);
        let result = resolve_module("pkg.sub.mod", &paths).unwrap();
        assert!(result.path.ends_with("mod.py"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_unresolved() {
        let dir = make_tmp_dir("bsk_ir_unresolved");

        let paths = make_search_paths(vec![dir.clone()]);
        let result = resolve_module("nonexistent", &paths);
        assert!(result.is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_relative_import_same_dir() {
        let dir = make_tmp_dir("bsk_ir_rel");
        fs::write(dir.join("sibling.py"), "x = 1\n").unwrap();
        let importing = dir.join("main.py");

        let paths = make_search_paths(vec![dir.clone()]);
        let result = resolve_relative_import(&importing, 1, "sibling", &paths);
        assert!(result.is_some());
        assert!(result.unwrap().path.ends_with("sibling.py"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_relative_import_parent() {
        let dir = unique_tmp("bsk_ir_rel_parent");
        let sub = dir.join("pkg");
        fs::create_dir_all(&sub).unwrap();
        fs::write(dir.join("utils.py"), "x = 1\n").unwrap();
        let importing = sub.join("mod.py");

        let paths = make_search_paths(vec![dir.clone()]);
        let result = resolve_relative_import(&importing, 2, "utils", &paths);
        assert!(result.is_some());
        assert!(result.unwrap().path.ends_with("utils.py"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_relative_import_bare_dot() {
        let dir = make_tmp_dir("bsk_ir_rel_bare");
        fs::write(dir.join("__init__.py"), "").unwrap();
        let importing = dir.join("mod.py");

        let paths = make_search_paths(vec![dir.clone()]);
        let result = resolve_relative_import(&importing, 1, "", &paths);
        assert!(result.is_some());
        assert!(result.unwrap().path.ends_with("__init__.py"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_extra_paths_searched() {
        let root = make_tmp_dir("bsk_ir_extra_root");
        let extra = make_tmp_dir("bsk_ir_extra_lib");
        fs::write(extra.join("libmod.py"), "x = 1\n").unwrap();

        let paths = ImportSearchPaths {
            roots: vec![root.clone()],
            extra_paths: vec![extra.clone()],
            stub_paths: vec![],
            workspace_members: vec![],
            site_packages: None,
            registry: None,
        };
        let result = resolve_module("libmod", &paths).unwrap();
        assert!(result.path.ends_with("libmod.py"));

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&extra);
    }

    #[test]
    fn test_site_packages_searched() {
        let root = make_tmp_dir("bsk_ir_sp_root");
        let sp = make_tmp_dir("bsk_ir_sp_pkgs");
        fs::write(sp.join("requests.py"), "").unwrap();

        let paths = ImportSearchPaths {
            roots: vec![root.clone()],
            extra_paths: vec![],
            stub_paths: vec![],
            workspace_members: vec![],
            site_packages: Some(sp.clone()),
            registry: None,
        };
        let result = resolve_module("requests", &paths).unwrap();
        assert!(result.path.ends_with("requests.py"));

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&sp);
    }

    #[test]
    fn test_workspace_root_takes_priority() {
        let root = make_tmp_dir("bsk_ir_prio_root");
        let extra = make_tmp_dir("bsk_ir_prio_extra");
        fs::write(root.join("dup.py"), "root\n").unwrap();
        fs::write(extra.join("dup.py"), "extra\n").unwrap();

        let paths = ImportSearchPaths {
            roots: vec![root.clone()],
            extra_paths: vec![extra.clone()],
            stub_paths: vec![],
            workspace_members: vec![],
            site_packages: None,
            registry: None,
        };
        let result = resolve_module("dup", &paths).unwrap();
        assert!(result.path.starts_with(&root));

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&extra);
    }

    #[test]
    fn test_package_init_pyi_preferred() {
        let dir = unique_tmp("bsk_ir_pkg_pyi");
        let _pkg = make_pkg(&dir, "mypkg", &["__init__.py", "__init__.pyi"]);

        let paths = make_search_paths(vec![dir.clone()]);
        let result = resolve_module("mypkg", &paths).unwrap();
        assert_eq!(result.resolution, ImportResolution::StubPyi);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_dotted_module_intermediate_missing() {
        let dir = make_tmp_dir("bsk_ir_dotted_miss");
        // No pkg/ directory exists.

        let paths = make_search_paths(vec![dir.clone()]);
        let result = resolve_module("pkg.sub.mod", &paths);
        assert!(result.is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    // imports_unresolved false positive: sibling-module import — issue #22
    // `import configure_agent_backend` in scripts/configure_agent_backend_test.py
    // should resolve to the sibling configure_agent_backend.py even when the
    // scripts/ directory is not listed as a workspace root.
    #[test]
    fn test_sibling_module_resolved_when_importer_dir_not_in_roots() {
        let scripts_dir = make_tmp_dir("bsk_ir_sibling");
        let workspace_root = make_tmp_dir("bsk_ir_sibling_root");
        fs::write(scripts_dir.join("configure_agent_backend.py"), "x = 1\n").unwrap();
        let importing_file = scripts_dir.join("configure_agent_backend_test.py");

        // Workspace root does NOT include scripts_dir — only the project root is listed.
        let paths = make_search_paths(vec![workspace_root.clone()]);

        // A bare `import configure_agent_backend` from a file inside scripts_dir must
        // resolve to the sibling .py file.  The fix is resolve_module_with_importer().
        let result =
            resolve_module_with_importer("configure_agent_backend", &paths, Some(&importing_file));
        assert!(
            result.is_some(),
            "imports_unresolved false positive: sibling module in the same directory as the importing \
             file should resolve without the directory being listed as a workspace root"
        );
        let r = result.unwrap();
        assert_eq!(r.resolution, ImportResolution::SourcePy);
        assert!(r.path.ends_with("configure_agent_backend.py"));

        let _ = fs::remove_dir_all(&scripts_dir);
        let _ = fs::remove_dir_all(&workspace_root);
    }

    /// Regression for issue #24: a `tests/` directory that does NOT contain
    /// `__init__.py` (PEP 420 implicit namespace package) must still resolve
    /// `from tests.helpers import X` when the workspace root is on the
    /// search path. pytest enables this layout by adding the project root
    /// to `sys.path`; Basilisk needs to mirror that behaviour.
    #[test]
    fn test_resolve_tests_namespace_package_without_init() {
        let root = unique_tmp("bsk_ir_tests_ns");
        let tests = root.join("tests");
        fs::create_dir_all(&tests).unwrap();
        // No __init__.py — PEP 420 namespace package layout.
        fs::write(tests.join("helpers.py"), "TEST_BUNDLE = 1\n").unwrap();

        let paths = make_search_paths(vec![root.clone()]);
        let result = resolve_module("tests.helpers", &paths);

        assert!(
            result.is_some(),
            "imports_unresolved false positive: PEP 420 namespace package `tests/` (no __init__.py) \
             must resolve when the project root is on the search path"
        );
        let r = result.unwrap();
        assert_eq!(r.resolution, ImportResolution::SourcePy);
        assert!(r.path.ends_with("helpers.py"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_resolve_relative_import_too_many_levels() {
        let dir = make_tmp_dir("bsk_ir_rel_deep");
        let importing = dir.join("mod.py");

        let paths = make_search_paths(vec![dir.clone()]);
        // level=10 should fail — can't go above filesystem root.
        let result = resolve_relative_import(&importing, 10, "x", &paths);
        assert!(result.is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_find_venv_dir_common_names() {
        let dir = unique_tmp("bsk_ir_venv");
        let venv = dir.join(".venv");
        fs::create_dir_all(&venv).unwrap();

        let config = crate::config::WorkspaceConfig::default();
        let result = find_venv_dir(std::slice::from_ref(&dir), &config);
        assert!(result.is_some());
        assert!(result.unwrap().ends_with(".venv"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_find_venv_dir_explicit_config() {
        let dir = unique_tmp("bsk_ir_venv_cfg");
        let venv = dir.join("my_env");
        fs::create_dir_all(&venv).unwrap();

        let config = crate::config::WorkspaceConfig {
            venv_path: Some(dir.clone()),
            venv: Some("my_env".to_owned()),
            ..Default::default()
        };
        let result = find_venv_dir(&[], &config);
        assert!(result.is_some());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_site_packages_unix_layout() {
        let dir = unique_tmp("bsk_ir_sp_unix");
        let sp = dir
            .join(".venv")
            .join("lib")
            .join("python3.12")
            .join("site-packages");
        fs::create_dir_all(&sp).unwrap();

        let config = crate::config::WorkspaceConfig::default();
        let result = resolve_site_packages(std::slice::from_ref(&dir), &config);
        assert!(result.is_some());
        assert!(result.unwrap().ends_with("site-packages"));

        let _ = fs::remove_dir_all(&dir);
    }

    /// Regression for issue #25: when the user activates a venv outside the
    /// workspace (e.g. CI installs to `/tmp/nap-ci-prep-py312`), Basilisk
    /// could not locate site-packages and reported `imports_unresolved` `NeedsSync` for
    /// every installed dep. Honour the `VIRTUAL_ENV` environment variable —
    /// the standard Python convention set by `source .venv/bin/activate`.
    #[test]
    fn test_resolve_site_packages_uses_virtual_env_var() {
        let venv = unique_tmp("bsk_ir_external_venv");
        let sp = venv.join("lib").join("python3.12").join("site-packages");
        fs::create_dir_all(&sp).unwrap();

        // Workspace root is intentionally empty (no .venv inside it).
        let workspace = make_tmp_dir("bsk_ir_workspace_no_venv");

        let config = crate::config::WorkspaceConfig::default();
        let result =
            resolve_site_packages_with_env(std::slice::from_ref(&workspace), &config, Some(&venv));

        assert!(
            result.is_some(),
            "issue #25: VIRTUAL_ENV must be honoured when workspace has no venv"
        );
        let resolved = result.unwrap();
        assert!(
            resolved.ends_with("site-packages"),
            "expected site-packages dir, got {resolved:?}"
        );
        assert!(
            resolved.starts_with(&venv),
            "expected resolved path under {venv:?}, got {resolved:?}"
        );

        let _ = fs::remove_dir_all(&venv);
        let _ = fs::remove_dir_all(&workspace);
    }

    /// `VIRTUAL_ENV` pointing at a non-existent path must not crash and must
    /// fall through to the normal workspace-root scan.
    #[test]
    fn test_virtual_env_var_invalid_falls_through_to_workspace_scan() {
        let workspace = unique_tmp("bsk_ir_ws_with_venv");
        let sp = workspace
            .join(".venv")
            .join("lib")
            .join("python3.12")
            .join("site-packages");
        fs::create_dir_all(&sp).unwrap();

        let bogus = std::path::PathBuf::from("/definitely/does/not/exist");
        let config = crate::config::WorkspaceConfig::default();
        let result =
            resolve_site_packages_with_env(std::slice::from_ref(&workspace), &config, Some(&bogus));

        assert!(
            result.is_some(),
            "bogus VIRTUAL_ENV must not block fallback to workspace .venv scan"
        );
        let resolved = result.unwrap();
        assert!(
            resolved.starts_with(&workspace),
            "expected workspace .venv site-packages, got {resolved:?}"
        );

        let _ = fs::remove_dir_all(&workspace);
    }

    // ── Stub-path resolution (Phase 1.2) ────────────────────────────────

    #[test]
    fn test_stub_paths_searched_before_roots() {
        let root = make_tmp_dir("bsk_ir_stubpath_root");
        let stubs = make_tmp_dir("bsk_ir_stubpath_stubs");
        fs::write(root.join("mymod.py"), "x = 1\n").unwrap();
        fs::write(stubs.join("mymod.pyi"), "x: int\n").unwrap();

        let paths = ImportSearchPaths {
            roots: vec![root.clone()],
            extra_paths: vec![],
            stub_paths: vec![stubs.clone()],
            workspace_members: vec![],
            site_packages: None,
            registry: None,
        };
        let result = resolve_module("mymod", &paths).unwrap();
        // Stub-path .pyi should win over root .py
        assert_eq!(result.resolution, ImportResolution::StubPyi);
        assert!(result.path.starts_with(&stubs));

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&stubs);
    }

    #[test]
    fn test_stub_paths_only_pyi() {
        let stubs = make_tmp_dir("bsk_ir_stubpath_pyi_only");
        // Only .py in stub dir — should NOT be found (stubs are .pyi only)
        fs::write(stubs.join("mymod.py"), "x = 1\n").unwrap();

        let paths = ImportSearchPaths {
            roots: vec![],
            extra_paths: vec![],
            stub_paths: vec![stubs.clone()],
            workspace_members: vec![],
            site_packages: None,
            registry: None,
        };
        let result = resolve_module("mymod", &paths);
        assert!(
            result.is_none(),
            "stub-paths should only resolve .pyi files"
        );

        let _ = fs::remove_dir_all(&stubs);
    }

    // ── PEP 561 stub-only packages (Phase 1.3) ─────────────────────────

    #[test]
    fn test_stub_package_resolution() {
        let root = make_tmp_dir("bsk_ir_pep561_root");
        let sp = unique_tmp("bsk_ir_pep561_sp");
        let _stubs_dir = make_pkg(&sp, "requests-stubs", &["__init__.pyi"]);

        let paths = ImportSearchPaths {
            roots: vec![root.clone()],
            extra_paths: vec![],
            stub_paths: vec![],
            workspace_members: vec![],
            site_packages: Some(sp.clone()),
            registry: None,
        };
        let result = resolve_module("requests", &paths).unwrap();
        assert_eq!(result.resolution, ImportResolution::StubPyi);
        assert!(result.path.to_string_lossy().contains("requests-stubs"));

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&sp);
    }

    #[test]
    fn test_stub_package_submodule() {
        let sp = unique_tmp("bsk_ir_pep561_sub");
        let stubs_dir = make_pkg(&sp, "requests-stubs", &["__init__.pyi"]);
        fs::write(stubs_dir.join("api.pyi"), "def get() -> None: ...\n").unwrap();

        let paths = ImportSearchPaths {
            roots: vec![],
            extra_paths: vec![],
            stub_paths: vec![],
            workspace_members: vec![],
            site_packages: Some(sp.clone()),
            registry: None,
        };
        let result = resolve_module("requests.api", &paths).unwrap();
        assert_eq!(result.resolution, ImportResolution::StubPyi);
        assert!(result.path.ends_with("api.pyi"));

        let _ = fs::remove_dir_all(&sp);
    }

    #[test]
    fn test_py_typed_detection() {
        let sp = unique_tmp("bsk_ir_pytyped");
        let _pkg = make_pkg(&sp, "rich", &["py.typed", "__init__.py"]);

        assert!(is_inline_typed_package("rich", &sp));
        assert!(!is_inline_typed_package("flask", &sp));

        let _ = fs::remove_dir_all(&sp);
    }

    #[test]
    fn test_has_stub_package_detection() {
        let sp = unique_tmp("bsk_ir_has_stubs");
        let stubs = sp.join("requests-stubs");
        fs::create_dir_all(&stubs).unwrap();

        assert!(has_stub_package("requests", &sp));
        assert!(has_stub_package("requests.api", &sp));
        assert!(!has_stub_package("flask", &sp));

        let _ = fs::remove_dir_all(&sp);
    }
}
