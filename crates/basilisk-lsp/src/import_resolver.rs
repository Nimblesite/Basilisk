//! Import resolution engine — resolves Python import statements to filesystem
//! paths within the workspace, extra paths, and venv site-packages.
//!
//! This module implements LSP Plan Phase 7.1: the workspace module resolver.

use std::path::{Path, PathBuf};

use basilisk_resolver::scope::ImportResolution;

/// Result of resolving a single import to a filesystem path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedImport {
    /// The filesystem path the import resolved to.
    pub path: PathBuf,
    /// Whether the resolved file is a `.py` source or `.pyi` stub.
    pub resolution: ImportResolution,
}

/// Search paths used for import resolution, derived from workspace config.
#[derive(Debug, Clone)]
pub struct ImportSearchPaths {
    /// Workspace root directories.
    pub roots: Vec<PathBuf>,
    /// Extra paths from config (`extraPaths`).
    pub extra_paths: Vec<PathBuf>,
    /// Site-packages directory from venv config, if any.
    pub site_packages: Option<PathBuf>,
}

impl ImportSearchPaths {
    /// Build search paths from workspace config.
    #[must_use]
    pub fn from_config(
        roots: &[PathBuf],
        config: &crate::config::WorkspaceConfig,
    ) -> Self {
        let site_packages = resolve_site_packages(
            config.venv_path.as_deref(),
            config.venv.as_deref(),
        );

        Self {
            roots: roots.to_vec(),
            extra_paths: config.extra_paths.clone(),
            site_packages,
        }
    }

    /// Return all search directories in priority order:
    /// workspace roots → extra paths → site-packages.
    fn search_dirs(&self) -> Vec<&Path> {
        let mut dirs: Vec<&Path> = Vec::new();
        for root in &self.roots {
            dirs.push(root.as_path());
        }
        for extra in &self.extra_paths {
            dirs.push(extra.as_path());
        }
        if let Some(sp) = &self.site_packages {
            dirs.push(sp.as_path());
        }
        dirs
    }
}

/// Resolve a dotted module name (e.g. `"os.path"`, `"mypackage.utils"`) to a
/// filesystem path.
///
/// Search order:
/// 1. Workspace roots
/// 2. Extra paths (`extraPaths` from config)
/// 3. Venv site-packages
///
/// For each search directory, checks (in order):
/// 1. `<dir>/<module_as_path>.pyi` (stub file, preferred)
/// 2. `<dir>/<module_as_path>.py` (source file)
/// 3. `<dir>/<module_as_path>/__init__.pyi` (package stub)
/// 4. `<dir>/<module_as_path>/__init__.py` (package source)
#[must_use]
pub fn resolve_module(
    module_name: &str,
    search_paths: &ImportSearchPaths,
) -> Option<ResolvedImport> {
    if module_name.is_empty() {
        return None;
    }

    let module_rel_path = module_name.replace('.', std::path::MAIN_SEPARATOR_STR);

    for base_dir in search_paths.search_dirs() {
        if let Some(resolved) = try_resolve_in_dir(base_dir, &module_rel_path) {
            return Some(resolved);
        }
    }

    None
}

/// Resolve a relative import (e.g. `from . import X`, `from ..utils import Y`)
/// relative to the importing file's location.
///
/// `level` is the number of leading dots (1 = current package, 2 = parent, etc.).
/// `module_name` is the module portion after the dots (may be empty for `from . import X`).
#[must_use]
pub fn resolve_relative_import(
    importing_file: &Path,
    level: u32,
    module_name: &str,
    search_paths: &ImportSearchPaths,
) -> Option<ResolvedImport> {
    // Start from the importing file's directory.
    let mut base = importing_file.parent()?.to_path_buf();

    // Go up `level - 1` directories (level 1 = current package dir).
    for _ in 1..level {
        base = base.parent()?.to_path_buf();
    }

    if module_name.is_empty() {
        // `from . import X` — resolve to the package __init__.py
        return try_resolve_in_dir(&base, "__init__");
    }

    let module_rel_path = module_name.replace('.', std::path::MAIN_SEPARATOR_STR);
    try_resolve_in_dir(&base, &module_rel_path)
        .or_else(|| {
            // Also check workspace roots with the full computed path
            let full_path = base.join(&module_rel_path);
            for root in &search_paths.roots {
                if let Ok(relative) = full_path.strip_prefix(root) {
                    let rel_str = relative.to_string_lossy();
                    if let Some(resolved) = try_resolve_in_dir(root, &rel_str) {
                        return Some(resolved);
                    }
                }
            }
            None
        })
}

/// Try to resolve a module path within a single directory.
///
/// Checks `.pyi` before `.py`, and file before package (`__init__`).
fn try_resolve_in_dir(base_dir: &Path, module_rel_path: &str) -> Option<ResolvedImport> {
    let base_path = base_dir.join(module_rel_path);

    // 1. <module>.pyi (stub file — highest priority)
    let pyi_path = base_path.with_extension("pyi");
    if pyi_path.is_file() {
        return Some(ResolvedImport {
            path: pyi_path,
            resolution: ImportResolution::StubPyi,
        });
    }

    // 2. <module>.py (source file)
    let py_path = base_path.with_extension("py");
    if py_path.is_file() {
        return Some(ResolvedImport {
            path: py_path,
            resolution: ImportResolution::SourcePy,
        });
    }

    // 3. <module>/__init__.pyi (package stub)
    let init_pyi = base_path.join("__init__.pyi");
    if init_pyi.is_file() {
        return Some(ResolvedImport {
            path: init_pyi,
            resolution: ImportResolution::StubPyi,
        });
    }

    // 4. <module>/__init__.py (package source)
    let init_py = base_path.join("__init__.py");
    if init_py.is_file() {
        return Some(ResolvedImport {
            path: init_py,
            resolution: ImportResolution::SourcePy,
        });
    }

    None
}

/// Compute the site-packages path from venv config.
///
/// Given `venv_path` (e.g. `/home/user/project`) and `venv` (e.g. `.venv`),
/// computes `<venv_path>/<venv>/lib/python3.X/site-packages`.
///
/// Scans for the actual `python3.X` directory since the exact version may vary.
fn resolve_site_packages(venv_path: Option<&Path>, venv: Option<&str>) -> Option<PathBuf> {
    let venv_path = venv_path?;
    let venv_name = venv?;
    let venv_root = venv_path.join(venv_name);
    let lib_dir = venv_root.join("lib");

    if !lib_dir.is_dir() {
        return None;
    }

    // Find the python3.X directory inside lib/
    let entries = std::fs::read_dir(&lib_dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with("python3") && entry.path().is_dir() {
            let site_packages = entry.path().join("site-packages");
            if site_packages.is_dir() {
                return Some(site_packages);
            }
        }
    }

    None
}

/// Resolve all imports in a workspace index after a scan.
///
/// Iterates every file in the index and resolves each `ImportInfo` against the
/// search paths. Updates `ImportInfo.resolution` and `ImportInfo.resolved_path`
/// in place (by cloning and replacing the `Arc<ResolvedModule>`).
pub fn resolve_workspace_imports(
    index: &crate::workspace::WorkspaceIndex,
    search_paths: &ImportSearchPaths,
) {
    use std::sync::Arc;

    for mut entry in index.files.iter_mut() {
        let Some(resolved_arc) = entry.resolved.take() else {
            continue;
        };

        let mut resolved = Arc::try_unwrap(resolved_arc)
            .unwrap_or_else(|arc| (*arc).clone());

        let file_path = std::path::Path::new(&resolved.path);

        for import in &mut resolved.imports {
            let result = if import.module.is_empty() {
                // Bare relative import — resolve from file location
                resolve_relative_import(file_path, 1, "", search_paths)
            } else {
                resolve_module(&import.module, search_paths)
                    .or_else(|| {
                        // Try as relative import from the file's directory
                        resolve_relative_import(file_path, 1, &import.module, search_paths)
                    })
            };

            if let Some(ri) = result {
                import.resolution = ri.resolution;
                import.resolved_path = Some(ri.path);
            }
        }

        entry.resolved = Some(Arc::new(resolved));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn make_search_paths(roots: Vec<PathBuf>) -> ImportSearchPaths {
        ImportSearchPaths {
            roots,
            extra_paths: Vec::new(),
            site_packages: None,
        }
    }

    // ── resolve_module ─────────────────────────────────────────────────────

    #[test]
    fn test_resolve_simple_module() {
        let dir = std::env::temp_dir().join("bsk_import_test_simple");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("utils.py"), "x: int = 1\n").unwrap();

        let paths = make_search_paths(vec![dir.clone()]);
        let result = resolve_module("utils", &paths);
        assert!(result.is_some());
        let resolved = result.unwrap();
        assert_eq!(resolved.path, dir.join("utils.py"));
        assert_eq!(resolved.resolution, ImportResolution::SourcePy);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_prefers_pyi_over_py() {
        let dir = std::env::temp_dir().join("bsk_import_test_pyi");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("utils.py"), "x: int = 1\n").unwrap();
        std::fs::write(dir.join("utils.pyi"), "x: int\n").unwrap();

        let paths = make_search_paths(vec![dir.clone()]);
        let result = resolve_module("utils", &paths).unwrap();
        assert_eq!(result.path, dir.join("utils.pyi"));
        assert_eq!(result.resolution, ImportResolution::StubPyi);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_dotted_module() {
        let dir = std::env::temp_dir().join("bsk_import_test_dotted");
        let pkg_dir = dir.join("mypackage");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        std::fs::write(pkg_dir.join("__init__.py"), "").unwrap();
        std::fs::write(pkg_dir.join("utils.py"), "x: int = 1\n").unwrap();

        let paths = make_search_paths(vec![dir.clone()]);
        let result = resolve_module("mypackage.utils", &paths).unwrap();
        assert_eq!(result.path, pkg_dir.join("utils.py"));
        assert_eq!(result.resolution, ImportResolution::SourcePy);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_package_init() {
        let dir = std::env::temp_dir().join("bsk_import_test_pkg");
        let pkg_dir = dir.join("mypackage");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        std::fs::write(pkg_dir.join("__init__.py"), "").unwrap();

        let paths = make_search_paths(vec![dir.clone()]);
        let result = resolve_module("mypackage", &paths).unwrap();
        assert_eq!(result.path, pkg_dir.join("__init__.py"));
        assert_eq!(result.resolution, ImportResolution::SourcePy);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_nonexistent_returns_none() {
        let dir = std::env::temp_dir().join("bsk_import_test_none");
        std::fs::create_dir_all(&dir).unwrap();

        let paths = make_search_paths(vec![dir.clone()]);
        assert!(resolve_module("nonexistent", &paths).is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_empty_module_returns_none() {
        let paths = make_search_paths(vec![]);
        assert!(resolve_module("", &paths).is_none());
    }

    #[test]
    fn test_resolve_extra_paths() {
        let root = std::env::temp_dir().join("bsk_import_test_extra_root");
        let extra = std::env::temp_dir().join("bsk_import_test_extra_vendor");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&extra).unwrap();
        std::fs::write(extra.join("vendor_lib.py"), "x: int = 1\n").unwrap();

        let paths = ImportSearchPaths {
            roots: vec![root.clone()],
            extra_paths: vec![extra.clone()],
            site_packages: None,
        };
        let result = resolve_module("vendor_lib", &paths).unwrap();
        assert_eq!(result.path, extra.join("vendor_lib.py"));

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&extra);
    }

    #[test]
    fn test_resolve_workspace_root_takes_priority_over_extra() {
        let root = std::env::temp_dir().join("bsk_import_test_priority_root");
        let extra = std::env::temp_dir().join("bsk_import_test_priority_extra");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&extra).unwrap();
        // Same module in both — root should win
        std::fs::write(root.join("shared.py"), "from_root = True\n").unwrap();
        std::fs::write(extra.join("shared.py"), "from_extra = True\n").unwrap();

        let paths = ImportSearchPaths {
            roots: vec![root.clone()],
            extra_paths: vec![extra.clone()],
            site_packages: None,
        };
        let result = resolve_module("shared", &paths).unwrap();
        assert_eq!(result.path, root.join("shared.py"));

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&extra);
    }

    // ── resolve_relative_import ────────────────────────────────────────────

    #[test]
    fn test_resolve_relative_same_package() {
        let dir = std::env::temp_dir().join("bsk_import_test_rel");
        let pkg = dir.join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();
        std::fs::write(pkg.join("__init__.py"), "").unwrap();
        std::fs::write(pkg.join("a.py"), "").unwrap();
        std::fs::write(pkg.join("b.py"), "").unwrap();

        let paths = make_search_paths(vec![dir.clone()]);
        // from .b import something (inside pkg/a.py)
        let result = resolve_relative_import(&pkg.join("a.py"), 1, "b", &paths).unwrap();
        assert_eq!(result.path, pkg.join("b.py"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_relative_parent_package() {
        let dir = std::env::temp_dir().join("bsk_import_test_rel_parent");
        let pkg = dir.join("pkg");
        let sub = pkg.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(pkg.join("__init__.py"), "").unwrap();
        std::fs::write(pkg.join("helper.py"), "").unwrap();
        std::fs::write(sub.join("__init__.py"), "").unwrap();
        std::fs::write(sub.join("mod.py"), "").unwrap();

        let paths = make_search_paths(vec![dir.clone()]);
        // from ..helper import something (inside pkg/sub/mod.py)
        let result =
            resolve_relative_import(&sub.join("mod.py"), 2, "helper", &paths).unwrap();
        assert_eq!(result.path, pkg.join("helper.py"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_relative_bare_dot_import() {
        let dir = std::env::temp_dir().join("bsk_import_test_rel_bare");
        let pkg = dir.join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();
        std::fs::write(pkg.join("__init__.py"), "").unwrap();
        std::fs::write(pkg.join("a.py"), "").unwrap();

        let paths = make_search_paths(vec![dir.clone()]);
        // from . import X (inside pkg/a.py) → resolves to pkg/__init__.py
        let result =
            resolve_relative_import(&pkg.join("a.py"), 1, "", &paths).unwrap();
        assert_eq!(result.path, pkg.join("__init__.py"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── try_resolve_in_dir ─────────────────────────────────────────────────

    #[test]
    fn test_try_resolve_in_dir_pyi_before_py() {
        let dir = std::env::temp_dir().join("bsk_import_test_dir_pyi");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("mod.py"), "").unwrap();
        std::fs::write(dir.join("mod.pyi"), "").unwrap();

        let result = try_resolve_in_dir(&dir, "mod").unwrap();
        assert_eq!(result.resolution, ImportResolution::StubPyi);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_try_resolve_in_dir_init_fallback() {
        let dir = std::env::temp_dir().join("bsk_import_test_dir_init");
        let pkg = dir.join("mypkg");
        std::fs::create_dir_all(&pkg).unwrap();
        std::fs::write(pkg.join("__init__.py"), "").unwrap();

        let result = try_resolve_in_dir(&dir, "mypkg").unwrap();
        assert_eq!(result.path, pkg.join("__init__.py"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_try_resolve_in_dir_missing() {
        let dir = std::env::temp_dir().join("bsk_import_test_dir_missing");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(try_resolve_in_dir(&dir, "nope").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── resolve_site_packages ──────────────────────────────────────────────

    #[test]
    fn test_resolve_site_packages_missing_venv() {
        assert!(resolve_site_packages(None, None).is_none());
        assert!(resolve_site_packages(Some(Path::new("/tmp")), None).is_none());
        assert!(resolve_site_packages(None, Some(".venv")).is_none());
    }

    #[test]
    fn test_resolve_site_packages_valid_venv() {
        let dir = std::env::temp_dir().join("bsk_import_test_venv");
        let sp = dir.join(".venv/lib/python3.12/site-packages");
        std::fs::create_dir_all(&sp).unwrap();

        let result = resolve_site_packages(Some(&dir), Some(".venv"));
        assert_eq!(result, Some(sp));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_site_packages_integration() {
        let dir = std::env::temp_dir().join("bsk_import_test_venv_int");
        let sp = dir.join(".venv/lib/python3.12/site-packages");
        std::fs::create_dir_all(&sp).unwrap();
        std::fs::write(sp.join("requests.py"), "").unwrap();

        let paths = ImportSearchPaths {
            roots: vec![dir.join("src")],
            extra_paths: Vec::new(),
            site_packages: Some(sp.clone()),
        };

        let result = resolve_module("requests", &paths).unwrap();
        assert_eq!(result.path, sp.join("requests.py"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
