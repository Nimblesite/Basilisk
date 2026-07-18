//! Tests for [ANALYSIS-CROSSLSP-IMPORT]. See docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-CROSSLSP-IMPORT
#![allow(
    clippy::allow_attributes,
    clippy::unwrap_used,
    clippy::expect_used,
    missing_docs
)]
//! Filesystem path-resolution tests for `basilisk_checker::imports` (relocated
//! from `basilisk-lsp`; behaviour-identical — they only touch the public API).

use std::fs;

use basilisk_checker::imports::{
    has_stub_package, is_inline_typed_package, resolve_module, resolve_module_with_importer,
    resolve_relative_import, ImportSearchPaths,
};
use basilisk_resolver::scope::ImportResolution;

mod import_support;
use import_support::{make_pkg, make_search_paths, make_tmp_dir, unique_tmp};

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
        typeshed_path: None,
        typeshed_snapshot: None,
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
    let requests = sp.join("requests");
    fs::create_dir_all(&requests).unwrap();
    fs::write(requests.join("py.typed"), "").unwrap();
    fs::write(requests.join("__init__.py"), "").unwrap();

    let paths = ImportSearchPaths {
        roots: vec![root.clone()],
        extra_paths: vec![],
        stub_paths: vec![],
        workspace_members: vec![],
        site_packages: Some(sp.clone()),
        registry: None,
        typeshed_path: None,
        typeshed_snapshot: None,
    };
    let result = resolve_module("requests", &paths).unwrap();
    assert!(result.path.ends_with("requests/__init__.py"));

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
        typeshed_path: None,
        typeshed_snapshot: None,
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
        typeshed_path: None,
        typeshed_snapshot: None,
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
        typeshed_path: None,
        typeshed_snapshot: None,
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
        typeshed_path: None,
        typeshed_snapshot: None,
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
        typeshed_path: None,
        typeshed_snapshot: None,
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
