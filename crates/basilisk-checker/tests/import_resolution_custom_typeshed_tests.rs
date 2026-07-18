//! Tests for [STUBRES-CUSTOM-TYPESHED].
//! See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CUSTOM-TYPESHED
#![allow(
    clippy::allow_attributes,
    clippy::unwrap_used,
    clippy::expect_used,
    missing_docs
)]
//! Custom `typeshed-path` filesystem resolution tests split from
//! `import_resolution_tests.rs`; they only touch the public import API.

use std::fs;

use basilisk_checker::imports::{resolve_module, ImportSearchPaths};
use basilisk_resolver::scope::ImportResolution;

mod import_support;
use import_support::make_tmp_dir;

#[test]
fn test_typeshed_path_overrides_stdlib_module() {
    // A custom typeshed dir supplies `stdlib/os.pyi`; `import os` must resolve
    // to it as the canonical stdlib source (spec step 3), not the name-only
    // bundled recognition (which resolves stdlib to no file at all).
    let typeshed = make_tmp_dir("bsk_ir_typeshed");
    let stdlib = typeshed.join("stdlib");
    fs::create_dir_all(&stdlib).unwrap();
    fs::write(stdlib.join("os.pyi"), "def uname() -> str: ...\n").unwrap();

    let paths = ImportSearchPaths {
        roots: vec![],
        extra_paths: vec![],
        stub_paths: vec![],
        workspace_members: vec![],
        site_packages: None,
        registry: None,
        typeshed_path: Some(typeshed.clone()),
        typeshed_snapshot: None,
    };
    let result = resolve_module("os", &paths).expect("custom typeshed should resolve `os`");
    assert_eq!(result.resolution, ImportResolution::StubPyi);
    assert!(
        result.path.starts_with(&stdlib),
        "expected resolution under the custom typeshed stdlib dir, got {:?}",
        result.path
    );

    let _ = fs::remove_dir_all(&typeshed);
}

#[test]
fn test_typeshed_path_resolves_stdlib_package() {
    // Package-form stdlib module: `stdlib/os/__init__.pyi`.
    let typeshed = make_tmp_dir("bsk_ir_typeshed_pkg");
    let os_pkg = typeshed.join("stdlib").join("os");
    fs::create_dir_all(&os_pkg).unwrap();
    fs::write(os_pkg.join("__init__.pyi"), "def uname() -> str: ...\n").unwrap();

    let paths = ImportSearchPaths {
        roots: vec![],
        extra_paths: vec![],
        stub_paths: vec![],
        workspace_members: vec![],
        site_packages: None,
        registry: None,
        typeshed_path: Some(typeshed.clone()),
        typeshed_snapshot: None,
    };
    let result = resolve_module("os", &paths).expect("custom typeshed should resolve `os` package");
    assert_eq!(result.resolution, ImportResolution::StubPyi);
    assert!(result.path.ends_with("__init__.pyi"));

    let _ = fs::remove_dir_all(&typeshed);
}

#[test]
fn test_no_typeshed_path_leaves_stdlib_unresolved_to_file() {
    // Without a custom typeshed, stdlib modules resolve to no file — the bundled
    // recognition is name-only (`is_stdlib_module`), applied downstream.
    let paths = ImportSearchPaths {
        roots: vec![],
        extra_paths: vec![],
        stub_paths: vec![],
        workspace_members: vec![],
        site_packages: None,
        registry: None,
        typeshed_path: None,
        typeshed_snapshot: None,
    };
    assert!(resolve_module("os", &paths).is_none());
}

#[test]
fn test_typeshed_path_uses_custom_stdlib_verbatim() {
    // A custom tree defines its own standard-library universe. It must not be
    // filtered through Basilisk's compiled CPython baseline.
    let typeshed = make_tmp_dir("bsk_ir_typeshed_nonstd");
    let stdlib = typeshed.join("stdlib");
    fs::create_dir_all(&stdlib).unwrap();
    fs::write(stdlib.join("requests.pyi"), "def get() -> None: ...\n").unwrap();

    let paths = ImportSearchPaths {
        roots: vec![],
        extra_paths: vec![],
        stub_paths: vec![],
        workspace_members: vec![],
        site_packages: None,
        registry: None,
        typeshed_path: Some(typeshed.clone()),
        typeshed_snapshot: None,
    };
    let resolved = resolve_module("requests", &paths).expect("custom tree is canonical");
    assert_eq!(resolved.path, stdlib.join("requests.pyi"));

    let _ = fs::remove_dir_all(&typeshed);
}

#[test]
fn test_stub_paths_shadow_custom_typeshed() {
    // Spec step 1 (stub-paths) sits at the head of the path and must win over
    // step 3 (typeshed-path) for the same stdlib module.
    let typeshed = make_tmp_dir("bsk_ir_typeshed_shadow_ts");
    let stdlib = typeshed.join("stdlib");
    fs::create_dir_all(&stdlib).unwrap();
    fs::write(stdlib.join("os.pyi"), "def uname() -> str: ...\n").unwrap();
    let stubs = make_tmp_dir("bsk_ir_typeshed_shadow_stubs");
    fs::write(stubs.join("os.pyi"), "def getcwd() -> str: ...\n").unwrap();

    let paths = ImportSearchPaths {
        roots: vec![],
        extra_paths: vec![],
        stub_paths: vec![stubs.clone()],
        workspace_members: vec![],
        site_packages: None,
        registry: None,
        typeshed_path: Some(typeshed.clone()),
        typeshed_snapshot: None,
    };
    let result = resolve_module("os", &paths).unwrap();
    assert!(
        result.path.starts_with(&stubs),
        "stub-paths (step 1) must shadow typeshed-path (step 3)"
    );

    let _ = fs::remove_dir_all(&typeshed);
    let _ = fs::remove_dir_all(&stubs);
}
