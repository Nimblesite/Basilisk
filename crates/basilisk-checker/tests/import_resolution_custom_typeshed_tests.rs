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

use basilisk_checker::imports::resolve_module;
use basilisk_resolver::scope::ImportResolution;

mod import_support;
use import_support::{custom_typeshed_snapshot, make_search_paths, make_tmp_dir};

#[test]
fn test_typeshed_path_overrides_stdlib_module() {
    // A custom typeshed dir supplies `stdlib/os.pyi`; `import os` must resolve
    // to it as the canonical stdlib source (spec step 3), not the name-only
    // bundled recognition (which resolves stdlib to no file at all).
    let mut paths = make_search_paths(vec![]);
    paths.typeshed_snapshot = Some(custom_typeshed_snapshot(&[(
        "os.pyi",
        "def uname() -> str: ...\n",
    )]));
    let result = resolve_module("os", &paths).expect("custom typeshed should resolve `os`");
    assert_eq!(result.resolution, ImportResolution::StubPyi);
    assert!(
        result
            .path
            .to_string_lossy()
            .starts_with("typeshed:custom-"),
        "expected resolution through the custom Typeshed VFS, got {:?}",
        result.path
    );
}

#[test]
fn test_typeshed_path_resolves_stdlib_package() {
    // Package-form stdlib module: `stdlib/os/__init__.pyi`.
    let mut paths = make_search_paths(vec![]);
    paths.typeshed_snapshot = Some(custom_typeshed_snapshot(&[(
        "os/__init__.pyi",
        "def uname() -> str: ...\n",
    )]));
    let result = resolve_module("os", &paths).expect("custom typeshed should resolve `os` package");
    assert_eq!(result.resolution, ImportResolution::StubPyi);
    assert!(result.path.ends_with("__init__.pyi"));
}

#[test]
fn test_no_active_snapshot_leaves_stdlib_unresolved_to_file() {
    // Without an active snapshot, configuration has supplied no step-3 source.
    let paths = make_search_paths(vec![]);
    assert!(resolve_module("os", &paths).is_none());
}

#[test]
fn test_typeshed_path_uses_custom_stdlib_verbatim() {
    // A custom tree defines its own standard-library universe. It must not be
    // filtered through Basilisk's compiled CPython baseline.
    let mut paths = make_search_paths(vec![]);
    paths.typeshed_snapshot = Some(custom_typeshed_snapshot(&[(
        "requests.pyi",
        "def get() -> None: ...\n",
    )]));
    let resolved = resolve_module("requests", &paths).expect("custom tree is canonical");
    assert!(resolved
        .path
        .to_string_lossy()
        .ends_with("stdlib/requests.pyi"));
}

#[test]
fn test_stub_paths_shadow_custom_typeshed() {
    // Spec step 1 (stub-paths) sits at the head of the path and must win over
    // step 3 (typeshed-path) for the same stdlib module.
    let stubs = make_tmp_dir("bsk_ir_typeshed_shadow_stubs");
    fs::write(stubs.join("os.pyi"), "def getcwd() -> str: ...\n").unwrap();

    let mut paths = make_search_paths(vec![]);
    paths.stub_paths = vec![stubs.clone()];
    paths.typeshed_snapshot = Some(custom_typeshed_snapshot(&[(
        "os.pyi",
        "def uname() -> str: ...\n",
    )]));
    let result = resolve_module("os", &paths).unwrap();
    assert!(
        result.path.starts_with(&stubs),
        "stub-paths (step 1) must shadow typeshed-path (step 3)"
    );

    let _ = fs::remove_dir_all(&stubs);
}
