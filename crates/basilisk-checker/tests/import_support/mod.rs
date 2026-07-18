//! Shared fixtures for the import-resolution integration tests.
//! Implements [ANALYSIS-CROSSLSP-IMPORT] / [ANALYSIS-INCR-IMPORTS] test support.
#![allow(
    dead_code,
    clippy::allow_attributes,
    clippy::unwrap_used,
    reason = "each import test binary uses a subset of these shared fixtures"
)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use basilisk_checker::imports::ImportSearchPaths;
use basilisk_resolver::scope::{ImportKind, ImportResolution};

static TEST_CTR: AtomicU64 = AtomicU64::new(0);

/// Generate a unique temp dir path to avoid races between parallel tests.
pub fn unique_tmp(prefix: &str) -> PathBuf {
    let n = TEST_CTR.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{prefix}_{n}_{}", std::process::id()))
}

/// Create a unique tmp dir named `<prefix>_<n>_<pid>` and return its path.
/// The dir is left in place; tests should clean up with `fs::remove_dir_all` at the end.
pub fn make_tmp_dir(prefix: &str) -> PathBuf {
    let dir = unique_tmp(prefix);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Create `<parent>/<pkg_name>/` and populate it with empty files named in
/// `files`. Returns the package directory path.
pub fn make_pkg(parent: &Path, pkg_name: &str, files: &[&str]) -> PathBuf {
    let pkg = parent.join(pkg_name);
    std::fs::create_dir_all(&pkg).unwrap();
    for f in files {
        std::fs::write(pkg.join(f), "").unwrap();
    }
    pkg
}

/// Search paths rooted at `roots`, with every other search location empty.
pub fn make_search_paths(roots: Vec<PathBuf>) -> ImportSearchPaths {
    ImportSearchPaths {
        roots,
        extra_paths: vec![],
        stub_paths: vec![],
        workspace_members: vec![],
        site_packages: None,
        registry: None,
        typeshed_path: None,
        typeshed_snapshot: None,
    }
}

/// Build a `ResolvedModule` with a single plain `import <module>` statement.
pub fn module_with_plain_import(module: &str) -> basilisk_resolver::ResolvedModule {
    module_with_plain_imports(&[module])
}

/// Build a `ResolvedModule` with one plain `import <m>` statement per `modules`.
pub fn module_with_plain_imports(modules: &[&str]) -> basilisk_resolver::ResolvedModule {
    basilisk_resolver::ResolvedModule {
        path: "test.py".to_owned(),
        imports: modules
            .iter()
            .map(|module| basilisk_resolver::ImportInfo {
                module: (*module).to_owned(),
                names: vec![],
                span: basilisk_resolver::Span::new(0, 0),
                name_spans: Vec::new(),
                kind: ImportKind::Plain,
                resolution: ImportResolution::Unresolved,
                resolved_path: None,
                package_dep_kind: None,
                package_version: None,
                package_name: None,
                stub_distribution: None,
                unresolved_reason: None,
            })
            .collect(),
        ..basilisk_resolver::ResolvedModule::default()
    }
}
