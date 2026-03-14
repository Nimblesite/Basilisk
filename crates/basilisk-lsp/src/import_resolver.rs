//! Import resolution engine — resolves `import X` to filesystem paths.
//!
//! Search order: workspace roots → extraPaths → venv site-packages.
//! File priority: `.pyi` stub preferred over `.py` source.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use basilisk_resolver::scope::{ImportKind, ImportResolution};

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
    /// Virtual environment site-packages directory, if detected.
    pub site_packages: Option<PathBuf>,
}

impl ImportSearchPaths {
    /// Build search paths from workspace config.
    #[must_use]
    pub fn from_config(roots: &[PathBuf], config: &crate::config::WorkspaceConfig) -> Self {
        let site_packages = resolve_site_packages(roots, config);
        Self {
            roots: roots.to_vec(),
            extra_paths: config.extra_paths.clone(),
            site_packages,
        }
    }
}

/// Resolve an absolute import (`import os.path` or `from os import path`).
#[must_use]
pub fn resolve_module(
    module_name: &str,
    search_paths: &ImportSearchPaths,
) -> Option<ResolvedImport> {
    let all_dirs = search_paths
        .roots
        .iter()
        .chain(search_paths.extra_paths.iter())
        .chain(search_paths.site_packages.iter());

    for dir in all_dirs {
        if let Some(resolved) = try_resolve_in_dir(module_name, dir) {
            return Some(resolved);
        }
    }
    None
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
    for &part in &parts[..parts.len() - 1] {
        current = current.join(part);
        if !current.is_dir() {
            return None;
        }
    }

    let last = parts[parts.len() - 1];
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

/// Detect site-packages directory from venv config.
fn resolve_site_packages(
    roots: &[PathBuf],
    config: &crate::config::WorkspaceConfig,
) -> Option<PathBuf> {
    let venv_dir = find_venv_dir(roots, config)?;
    // Unix: lib/pythonX.Y/site-packages
    let lib = venv_dir.join("lib");
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
    let win_sp = venv_dir.join("Lib").join("site-packages");
    if win_sp.is_dir() {
        return Some(win_sp);
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
/// its `resolution` and `resolved_path` fields in place.
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
        }
        entry.value_mut().resolved = Some(Arc::new(resolved));
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "test-only code: unwrap acceptable in unit tests")]
mod tests {
    use super::*;
    use std::fs;

    fn make_search_paths(roots: Vec<PathBuf>) -> ImportSearchPaths {
        ImportSearchPaths {
            roots,
            extra_paths: vec![],
            site_packages: None,
        }
    }

    #[test]
    fn test_resolve_simple_module() {
        let dir = std::env::temp_dir().join("bsk_ir_simple");
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
        let dir = std::env::temp_dir().join("bsk_ir_pyi");
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
        let dir = std::env::temp_dir().join("bsk_ir_pkg");
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
        let dir = std::env::temp_dir().join("bsk_ir_dotted");
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
        let dir = std::env::temp_dir().join("bsk_ir_unresolved");
        fs::create_dir_all(&dir).unwrap();

        let paths = make_search_paths(vec![dir.clone()]);
        let result = resolve_module("nonexistent", &paths);
        assert!(result.is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_relative_import_same_dir() {
        let dir = std::env::temp_dir().join("bsk_ir_rel");
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
        let dir = std::env::temp_dir().join("bsk_ir_rel_parent");
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
        let dir = std::env::temp_dir().join("bsk_ir_rel_bare");
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
        let root = std::env::temp_dir().join("bsk_ir_extra_root");
        let extra = std::env::temp_dir().join("bsk_ir_extra_lib");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&extra).unwrap();
        fs::write(extra.join("libmod.py"), "x = 1\n").unwrap();

        let paths = ImportSearchPaths {
            roots: vec![root.clone()],
            extra_paths: vec![extra.clone()],
            site_packages: None,
        };
        let result = resolve_module("libmod", &paths).unwrap();
        assert!(result.path.ends_with("libmod.py"));

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&extra);
    }

    #[test]
    fn test_site_packages_searched() {
        let root = std::env::temp_dir().join("bsk_ir_sp_root");
        let sp = std::env::temp_dir().join("bsk_ir_sp_pkgs");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&sp).unwrap();
        fs::write(sp.join("requests.py"), "").unwrap();

        let paths = ImportSearchPaths {
            roots: vec![root.clone()],
            extra_paths: vec![],
            site_packages: Some(sp.clone()),
        };
        let result = resolve_module("requests", &paths).unwrap();
        assert!(result.path.ends_with("requests.py"));

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&sp);
    }

    #[test]
    fn test_workspace_root_takes_priority() {
        let root = std::env::temp_dir().join("bsk_ir_prio_root");
        let extra = std::env::temp_dir().join("bsk_ir_prio_extra");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&extra).unwrap();
        fs::write(root.join("dup.py"), "root\n").unwrap();
        fs::write(extra.join("dup.py"), "extra\n").unwrap();

        let paths = ImportSearchPaths {
            roots: vec![root.clone()],
            extra_paths: vec![extra.clone()],
            site_packages: None,
        };
        let result = resolve_module("dup", &paths).unwrap();
        assert!(result.path.starts_with(&root));

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&extra);
    }

    #[test]
    fn test_package_init_pyi_preferred() {
        let dir = std::env::temp_dir().join("bsk_ir_pkg_pyi");
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
        let dir = std::env::temp_dir().join("bsk_ir_dotted_miss");
        fs::create_dir_all(&dir).unwrap();
        // No pkg/ directory exists.

        let paths = make_search_paths(vec![dir.clone()]);
        let result = resolve_module("pkg.sub.mod", &paths);
        assert!(result.is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_relative_import_too_many_levels() {
        let dir = std::env::temp_dir().join("bsk_ir_rel_deep");
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
        let dir = std::env::temp_dir().join("bsk_ir_venv");
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
        let dir = std::env::temp_dir().join("bsk_ir_venv_cfg");
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
        let dir = std::env::temp_dir().join("bsk_ir_sp_unix");
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
}
