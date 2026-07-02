//! Tests for [ANALYSIS-INCR-IMPORTS]. See docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-INCR-IMPORTS
#![allow(clippy::allow_attributes, clippy::unwrap_used, missing_docs)]
//! `resolve_module_imports` application tests for `basilisk_checker::imports`
//! (relocated from `basilisk-lsp`; behaviour-identical — public API only).

use std::fs;
use std::sync::Arc;

use basilisk_checker::imports::{classify_unresolved, resolve_module_imports, ImportSearchPaths};
use basilisk_resolver::scope::{ImportResolution, PackageDepKind, UnresolvedReason};
use basilisk_uv::PackageRegistry;

mod import_support;
use import_support::{
    make_search_paths, make_tmp_dir, module_with_plain_import, module_with_plain_imports,
};

/// A registry with a Direct dep (`requests`), its Transitive dep (`urllib3`),
/// and a Dev dep (`pytest`) — mirrors `basilisk-uv`'s own lock fixture so the
/// registry-classification branches (`classify_unresolved`,
/// `enrich_package_metadata`) are exercised inside `basilisk-checker`.
fn make_registry() -> Arc<PackageRegistry> {
    use std::collections::HashMap;

    use basilisk_uv::lockfile::{LockDependency, LockPackage, LockSource};
    use basilisk_uv::LockFile;

    let dep = |name: &str, ver: &str| LockDependency {
        name: name.to_owned(),
        version: Some(ver.to_owned()),
        marker: None,
        extra: HashMap::new(),
    };
    let registry_pkg = |name: &str, ver: &str, deps: Vec<LockDependency>| LockPackage {
        name: name.to_owned(),
        version: ver.to_owned(),
        source: Some(LockSource {
            registry: Some("https://pypi.org/simple".to_owned()),
            editable: None,
            virtual_field: None,
        }),
        dependencies: deps,
        dev_dependencies: HashMap::new(),
        extra: HashMap::new(),
    };
    let root = LockPackage {
        name: "my-project".to_owned(),
        version: "0.1.0".to_owned(),
        source: Some(LockSource {
            registry: None,
            editable: None,
            virtual_field: Some(".".to_owned()),
        }),
        dependencies: vec![dep("requests", "2.31.0")],
        dev_dependencies: HashMap::from([("dev".to_owned(), vec![dep("pytest", "8.0.0")])]),
        extra: HashMap::new(),
    };
    let lock = LockFile {
        version: 1,
        requires_python: Some(">=3.12".to_owned()),
        packages: vec![
            root,
            registry_pkg("requests", "2.31.0", vec![dep("urllib3", "2.1.0")]),
            registry_pkg("urllib3", "2.1.0", vec![]),
            registry_pkg("pytest", "8.0.0", vec![]),
        ],
        extra: HashMap::new(),
    };
    Arc::new(PackageRegistry::from_lock_file(
        &lock,
        &["requests".to_owned()],
    ))
}

fn search_paths_with_registry(registry: Arc<PackageRegistry>) -> ImportSearchPaths {
    let mut paths = make_search_paths(vec![]);
    paths.registry = Some(registry);
    paths
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
fn classify_unresolved_uses_the_registry() {
    let paths = search_paths_with_registry(make_registry());

    // Direct dep, known to the registry but not on disk → NeedsSync.
    assert_eq!(
        classify_unresolved("requests", &paths),
        UnresolvedReason::NeedsSync
    );
    // Dev dep (not transitive) → NeedsSync.
    assert_eq!(
        classify_unresolved("pytest", &paths),
        UnresolvedReason::NeedsSync
    );
    // Transitive-only dep → NotInDeps (it is not a declared dependency).
    assert_eq!(
        classify_unresolved("urllib3", &paths),
        UnresolvedReason::NotInDeps
    );
    // Absent from the registry entirely → NotInstalled.
    assert_eq!(
        classify_unresolved("flask", &paths),
        UnresolvedReason::NotInstalled
    );
    // No registry available → Unknown.
    assert_eq!(
        classify_unresolved("requests", &make_search_paths(vec![])),
        UnresolvedReason::Unknown
    );
}

#[test]
fn resolve_module_imports_classifies_and_enriches_unresolved() {
    let paths = search_paths_with_registry(make_registry());

    // None of these resolve on disk (no roots / site-packages), so each
    // non-stdlib import is classified and enriched from the registry; the
    // stdlib import (`os`) is skipped by both classification and enrichment.
    let mut resolved = module_with_plain_imports(&["requests", "urllib3", "pytest", "os"]);
    resolve_module_imports(&mut resolved, &paths);

    let import = |module: &str| {
        resolved
            .imports
            .iter()
            .find(|i| i.module == module)
            .cloned()
            .unwrap()
    };

    let requests = import("requests");
    assert_eq!(requests.resolution, ImportResolution::Unresolved);
    assert_eq!(
        requests.unresolved_reason,
        Some(UnresolvedReason::NeedsSync)
    );
    assert_eq!(requests.package_dep_kind, Some(PackageDepKind::Direct));
    assert_eq!(requests.package_version.as_deref(), Some("2.31.0"));
    assert_eq!(requests.package_name.as_deref(), Some("requests"));

    let urllib3 = import("urllib3");
    assert_eq!(urllib3.unresolved_reason, Some(UnresolvedReason::NotInDeps));
    assert_eq!(urllib3.package_dep_kind, Some(PackageDepKind::Transitive));

    let pytest = import("pytest");
    assert_eq!(pytest.package_dep_kind, Some(PackageDepKind::Dev));

    // stdlib import: no classification, no package metadata.
    let os_import = import("os");
    assert_eq!(os_import.unresolved_reason, None);
    assert_eq!(os_import.package_dep_kind, None);
}
