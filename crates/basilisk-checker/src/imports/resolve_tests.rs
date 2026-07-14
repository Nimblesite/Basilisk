//! Focused precedence tests for bundled-standard-library import resolution.

#![expect(
    clippy::expect_used,
    reason = "test-only filesystem setup uses expect for clear failures"
)]

use std::path::{Path, PathBuf};

use super::{resolve_module, resolve_module_with_importer, ImportSearchPaths};

fn search_paths(site_packages: &Path) -> ImportSearchPaths {
    ImportSearchPaths {
        roots: Vec::new(),
        extra_paths: Vec::new(),
        stub_paths: Vec::new(),
        workspace_members: Vec::new(),
        site_packages: Some(site_packages.to_path_buf()),
        registry: None,
        typeshed_path: None,
    }
}

fn write_module(directory: &Path, name: &str, extension: &str) -> PathBuf {
    std::fs::create_dir_all(directory).expect("create module directory");
    let path = directory.join(format!("{name}.{extension}"));
    std::fs::write(&path, "value: int = 1\n").expect("write module");
    path
}

#[test]
fn bundled_stdlib_is_terminal_before_site_packages() {
    let temp = tempfile::tempdir().expect("tempdir");
    let site_packages = temp.path().join("site-packages");
    let _shadow = write_module(&site_packages, "typing", "py");

    assert!(
        resolve_module("typing", &search_paths(&site_packages)).is_none(),
        "site-packages must not shadow the standard library"
    );
}

#[test]
fn explicit_sources_still_precede_bundled_stdlib() {
    let temp = tempfile::tempdir().expect("tempdir");
    let manual_stubs = temp.path().join("manual-stubs");
    let roots = temp.path().join("root");
    let workspace = temp.path().join("workspace");
    let extra = temp.path().join("extra");
    let site_packages = temp.path().join("site-packages");
    let expected = write_module(&manual_stubs, "typing", "pyi");
    let _root_shadow = write_module(&roots, "typing", "py");
    let _workspace_shadow = write_module(&workspace, "typing", "py");
    let _extra_shadow = write_module(&extra, "typing", "py");
    let _site_shadow = write_module(&site_packages, "typing", "py");
    let mut paths = search_paths(&site_packages);
    paths.stub_paths.push(manual_stubs);
    paths.roots.push(roots);
    paths.workspace_members.push(workspace);
    paths.extra_paths.push(extra);

    let resolved = resolve_module("typing", &paths).expect("manual stub must resolve");
    assert_eq!(resolved.path, expected);
}

#[test]
fn importer_directory_still_shadows_bundled_stdlib() {
    let temp = tempfile::tempdir().expect("tempdir");
    let importer_directory = temp.path().join("scripts");
    let expected = write_module(&importer_directory, "typing", "py");
    let importer = importer_directory.join("check.py");
    std::fs::write(&importer, "import typing\n").expect("write importer");
    let site_packages = temp.path().join("site-packages");
    let _site_shadow = write_module(&site_packages, "typing", "py");

    let resolved =
        resolve_module_with_importer("typing", &search_paths(&site_packages), Some(&importer))
            .expect("importer-local module must resolve");
    assert_eq!(resolved.path, expected);
}

#[test]
fn custom_typeshed_missing_stdlib_is_canonical() {
    let temp = tempfile::tempdir().expect("tempdir");
    let site_packages = temp.path().join("site-packages");
    let typeshed = temp.path().join("typeshed");
    let _shadow = write_module(&site_packages, "typing", "py");
    std::fs::create_dir_all(typeshed.join("stdlib")).expect("create typeshed");
    let mut paths = search_paths(&site_packages);
    paths.typeshed_path = Some(typeshed);

    assert!(
        resolve_module("typing", &paths).is_none(),
        "a missing custom-typeshed stdlib module must stay unresolved"
    );
}

#[test]
fn custom_typeshed_stdlib_stub_resolves_before_terminal_step() {
    let temp = tempfile::tempdir().expect("tempdir");
    let site_packages = temp.path().join("site-packages");
    let typeshed = temp.path().join("typeshed");
    let expected = write_module(&typeshed.join("stdlib"), "typing", "pyi");
    let _shadow = write_module(&site_packages, "typing", "py");
    let mut paths = search_paths(&site_packages);
    paths.typeshed_path = Some(typeshed);

    let resolved = resolve_module("typing", &paths).expect("typeshed stub must resolve");
    assert_eq!(resolved.path, expected);
}

#[test]
fn external_module_still_falls_through_to_site_packages() {
    let temp = tempfile::tempdir().expect("tempdir");
    let site_packages = temp.path().join("site-packages");
    let expected = write_module(&site_packages, "requests", "py");

    let resolved =
        resolve_module("requests", &search_paths(&site_packages)).expect("external module");
    assert_eq!(resolved.path, expected);
}
