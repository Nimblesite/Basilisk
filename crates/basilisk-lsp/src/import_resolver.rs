//! Import resolution engine — resolves `import X` to filesystem paths.
//!
//! Search order: workspace roots → extraPaths → venv site-packages.
//! File priority: `.pyi` stub preferred over `.py` source.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use basilisk_resolver::scope::{ImportKind, ImportResolution, PackageDepKind};
use basilisk_uv::PackageRegistry;

use crate::workspace::WorkspaceIndex;

/// Result of resolving a single import to a filesystem path.
#[derive(Debug, Clone)]
pub struct ResolvedImport {
    /// Filesystem path of the resolved module.
    pub path: PathBuf,
    /// Whether this resolved to a `.pyi` stub or `.py` source.
    pub resolution: ImportResolution,
    /// Optional metadata from the uv package registry.
    pub package_info: Option<Arc<basilisk_uv::PackageInfo>>,
}

/// Why an import could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnresolvedReason {
    /// Package not in uv.lock at all.
    NotInstalled,
    /// In lock as transitive, not in pyproject.toml dependencies.
    NotInDeps,
    /// In pyproject.toml but lock file is stale or venv not synced.
    NeedsSync,
    /// Installed but no .pyi files.
    NoStubs,
    /// stdlib module not available in target Python version.
    WrongPythonVersion,
    /// Non-uv project or can't determine reason.
    Unknown,
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
    #[must_use]
    pub fn from_config(
        roots: &[PathBuf],
        config: &crate::config::WorkspaceConfig,
        registry: Option<Arc<PackageRegistry>>,
    ) -> Self {
        let site_packages = resolve_site_packages(roots, config);
        Self {
            roots: roots.to_vec(),
            extra_paths: config.extra_paths.clone(),
            stub_paths: config.stub_paths.clone(),
            workspace_members: Vec::new(),
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

    let root_module = module_name.split('.').next().unwrap_or(module_name);
    let import_name = basilisk_uv::import_map::package_to_import_name(root_module);

    if let Some(info) = registry.lookup(&import_name) {
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
            package_info: None,
        });
    }

    let pkg_pyi = current.join(last).join("__init__.pyi");
    if pkg_pyi.is_file() {
        return Some(ResolvedImport {
            path: pkg_pyi,
            resolution: ImportResolution::StubPyi,
            package_info: None,
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
                    package_info: None,
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
            package_info: None,
        });
    }
    // 2. name.py
    let py = dir.join(format!("{name}.py"));
    if py.is_file() {
        return Some(ResolvedImport {
            path: py,
            resolution: ImportResolution::SourcePy,
            package_info: None,
        });
    }
    // 3. name/__init__.pyi (package stub)
    let pkg_pyi = dir.join(name).join("__init__.pyi");
    if pkg_pyi.is_file() {
        return Some(ResolvedImport {
            path: pkg_pyi,
            resolution: ImportResolution::StubPyi,
            package_info: None,
        });
    }
    // 4. name/__init__.py (package)
    let pkg_py = dir.join(name).join("__init__.py");
    if pkg_py.is_file() {
        return Some(ResolvedImport {
            path: pkg_py,
            resolution: ImportResolution::SourcePy,
            package_info: None,
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
            package_info: None,
        });
    }
    let init_py = dir.join("__init__.py");
    if init_py.is_file() {
        return Some(ResolvedImport {
            path: init_py,
            resolution: ImportResolution::SourcePy,
            package_info: None,
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
fn resolve_site_packages(
    roots: &[PathBuf],
    config: &crate::config::WorkspaceConfig,
) -> Option<PathBuf> {
    // 1. Try venv-based discovery first.
    if let Some(sp) = resolve_venv_site_packages(roots, config) {
        return Some(sp);
    }
    // 2. Fall back to Python subprocess discovery.
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

        for import in &mut resolved.imports {
            let result = match import.kind {
                ImportKind::Plain | ImportKind::From | ImportKind::Star => {
                    resolve_module(&import.module, search_paths)
                }
            };
            if let Some(r) = result {
                import.resolution = r.resolution;
                import.resolved_path = Some(r.path);
            }

            // Annotate with dependency classification from the uv registry.
            import.package_dep_kind = classify_dep_kind(&import.module, search_paths);
        }
        entry.value_mut().resolved = Some(Arc::new(resolved));
    }
}

/// Determine the dependency kind for an import using the uv package registry.
///
/// Returns `None` for stdlib modules, non-uv projects, or imports not found
/// in the registry.
fn classify_dep_kind(
    module_name: &str,
    search_paths: &ImportSearchPaths,
) -> Option<PackageDepKind> {
    let registry = search_paths.registry.as_ref()?;

    // Skip stdlib modules — they have no package dep kind.
    if basilisk_stubs::is_stdlib_module(module_name) {
        return None;
    }

    let root_module = module_name.split('.').next().unwrap_or(module_name);
    let import_name = basilisk_uv::import_map::package_to_import_name(root_module);

    let info = registry.lookup(&import_name)?;
    Some(match info.kind {
        basilisk_uv::DepKind::Direct => PackageDepKind::Direct,
        basilisk_uv::DepKind::Dev => PackageDepKind::Dev,
        basilisk_uv::DepKind::Transitive => PackageDepKind::Transitive,
    })
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

    #[test]
    fn test_resolve_simple_module() {
        let dir = unique_tmp("bsk_ir_simple");
        fs::create_dir_all(&dir).unwrap();
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
        let dir = unique_tmp("bsk_ir_pyi");
        fs::create_dir_all(&dir).unwrap();
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
        let pkg = dir.join("mypkg");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(pkg.join("__init__.py"), "").unwrap();

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
        let dir = unique_tmp("bsk_ir_unresolved");
        fs::create_dir_all(&dir).unwrap();

        let paths = make_search_paths(vec![dir.clone()]);
        let result = resolve_module("nonexistent", &paths);
        assert!(result.is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_relative_import_same_dir() {
        let dir = unique_tmp("bsk_ir_rel");
        fs::create_dir_all(&dir).unwrap();
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
        let dir = unique_tmp("bsk_ir_rel_bare");
        fs::create_dir_all(&dir).unwrap();
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
        let root = unique_tmp("bsk_ir_extra_root");
        let extra = unique_tmp("bsk_ir_extra_lib");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&extra).unwrap();
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
        let root = unique_tmp("bsk_ir_sp_root");
        let sp = unique_tmp("bsk_ir_sp_pkgs");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&sp).unwrap();
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
        let root = unique_tmp("bsk_ir_prio_root");
        let extra = unique_tmp("bsk_ir_prio_extra");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&extra).unwrap();
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
        let pkg = dir.join("mypkg");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(pkg.join("__init__.py"), "").unwrap();
        fs::write(pkg.join("__init__.pyi"), "").unwrap();

        let paths = make_search_paths(vec![dir.clone()]);
        let result = resolve_module("mypkg", &paths).unwrap();
        assert_eq!(result.resolution, ImportResolution::StubPyi);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_dotted_module_intermediate_missing() {
        let dir = unique_tmp("bsk_ir_dotted_miss");
        fs::create_dir_all(&dir).unwrap();
        // No pkg/ directory exists.

        let paths = make_search_paths(vec![dir.clone()]);
        let result = resolve_module("pkg.sub.mod", &paths);
        assert!(result.is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_relative_import_too_many_levels() {
        let dir = unique_tmp("bsk_ir_rel_deep");
        fs::create_dir_all(&dir).unwrap();
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

    // ── Stub-path resolution (Phase 1.2) ────────────────────────────────

    #[test]
    fn test_stub_paths_searched_before_roots() {
        let root = unique_tmp("bsk_ir_stubpath_root");
        let stubs = unique_tmp("bsk_ir_stubpath_stubs");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&stubs).unwrap();
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
        let stubs = unique_tmp("bsk_ir_stubpath_pyi_only");
        fs::create_dir_all(&stubs).unwrap();
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
        let root = unique_tmp("bsk_ir_pep561_root");
        let sp = unique_tmp("bsk_ir_pep561_sp");
        fs::create_dir_all(&root).unwrap();
        let stubs_dir = sp.join("requests-stubs");
        fs::create_dir_all(&stubs_dir).unwrap();
        fs::write(stubs_dir.join("__init__.pyi"), "").unwrap();

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
        let stubs_dir = sp.join("requests-stubs");
        fs::create_dir_all(&stubs_dir).unwrap();
        fs::write(stubs_dir.join("__init__.pyi"), "").unwrap();
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
        let pkg = sp.join("rich");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(pkg.join("py.typed"), "").unwrap();
        fs::write(pkg.join("__init__.py"), "").unwrap();

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
