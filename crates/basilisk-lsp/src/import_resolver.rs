//! Implements [ANALYSIS-CROSSLSP-IMPORT]. See docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-CROSSLSP-IMPORT
//!
//! Config/environment adapter for import resolution.
//!
//! The **filesystem-pure** resolution engine (`resolve_module`,
//! `resolve_module_imports`, `ImportSearchPaths`, …) now lives in
//! `basilisk_checker::imports` so the memoized checker query can fold it in
//! ([CHKARCH-INCREMENTAL-SALSA]); it is re-exported below so
//! `basilisk_lsp::import_resolver::*` stays a stable path for the CLI and tests.
//! This module keeps only the parts that depend on the LSP's `WorkspaceConfig`
//! and `WorkspaceIndex`: building an [`ImportSearchPaths`] from config
//! (`search_paths_from_config`), venv / site-packages discovery, and the
//! whole-workspace scan (`resolve_workspace_imports`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use basilisk_uv::PackageRegistry;

use crate::workspace::WorkspaceIndex;

// The pure import engine, hoisted to basilisk-checker. Re-exported so existing
// callers (`basilisk_lsp::import_resolver::X`) keep resolving unchanged.
pub use basilisk_checker::imports::{
    classify_unresolved, has_stub_package, is_inline_typed_package, resolve_module,
    resolve_module_imports, resolve_module_with_importer, resolve_relative_import,
    ImportSearchPaths, ResolvedImport,
};

/// Build search paths from workspace config.
///
/// Automatically includes `.basilisk/stubs/` in each root as a stub
/// search path for auto-generated Tier 3 stubs.
#[must_use]
pub fn search_paths_from_config(
    roots: &[PathBuf],
    config: &crate::config::WorkspaceConfig,
    registry: Option<Arc<PackageRegistry>>,
) -> ImportSearchPaths {
    let site_packages = resolve_site_packages(roots, config);

    // Include user-configured stub paths + auto-generated stub cache dirs.
    let mut stub_paths = config.stub_paths.clone();
    for root in roots {
        let generated_stubs = root.join(basilisk_stubs::generate::cache::DEFAULT_CACHE_DIR);
        if generated_stubs.is_dir() {
            stub_paths.push(generated_stubs);
        }
    }

    ImportSearchPaths {
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

// Implements [LSPUV-DETECTION-FALLBACK] — the existing venv-discovery path used
// when uv detection fails or is ambiguous; uv integration is additive.
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
}
