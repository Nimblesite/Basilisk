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
        typeshed_snapshot: None,
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
fn manually_configured_python_source_precedes_workspace_code() {
    let temp = tempfile::tempdir().expect("tempdir");
    let manual = temp.path().join("manual-python");
    let root = temp.path().join("root");
    let expected = write_module(&manual, "chosen", "py");
    let _workspace_shadow = write_module(&root, "chosen", "pyi");
    let mut paths = search_paths(&temp.path().join("site-packages"));
    paths.extra_paths.push(manual);
    paths.roots.push(root);

    let resolved = resolve_module("chosen", &paths).expect("manual source must resolve first");
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
fn custom_versions_admits_a_non_cpython_stdlib_module() {
    let temp = tempfile::tempdir().expect("tempdir");
    let typeshed = temp.path().join("micropython-typeshed");
    let stdlib = typeshed.join("stdlib");
    let expected = write_module(&stdlib, "uasyncio", "pyi");
    std::fs::write(stdlib.join("VERSIONS"), "uasyncio: 3.4-\n").expect("write VERSIONS");
    let mut paths = search_paths(&temp.path().join("site-packages"));
    paths.typeshed_path = Some(typeshed);

    let resolved = resolve_module("uasyncio", &paths).expect("custom VERSIONS admits module");
    assert_eq!(resolved.path, expected);
}

#[test]
fn external_module_still_falls_through_to_site_packages() {
    let temp = tempfile::tempdir().expect("tempdir");
    let site_packages = temp.path().join("site-packages");
    let package = site_packages.join("requests");
    let expected = write_module(&package, "__init__", "py");
    std::fs::write(package.join("py.typed"), "").expect("write py.typed marker");

    let resolved =
        resolve_module("requests", &search_paths(&site_packages)).expect("external module");
    assert_eq!(resolved.path, expected);
}

#[test]
/// [STUBRES-PEP561-NORMATIVE]/[STUBRES-PEP561-MAPPING]/
/// [STUBRES-RESOLUTION-FLOW]: pinned mapping and branch order.
fn six_step_order_removes_each_winner_in_turn() {
    let temp = tempfile::tempdir().expect("tempdir");
    let manual = temp.path().join("manual");
    let root = temp.path().join("root");
    let typeshed = temp.path().join("typeshed");
    let site_packages = temp.path().join("site-packages");
    let stub_package = site_packages.join("typing-stubs");
    let inline_package = site_packages.join("typing");

    let step_1 = write_module(&manual, "typing", "pyi");
    let step_2 = write_module(&root, "typing", "py");
    let step_3 = write_module(&typeshed.join("stdlib"), "typing", "pyi");
    std::fs::create_dir_all(&stub_package).expect("create stub package");
    let step_4 = stub_package.join("__init__.pyi");
    std::fs::write(&step_4, "step: int\n").expect("write stub package");
    std::fs::create_dir_all(&inline_package).expect("create inline package");
    std::fs::write(inline_package.join("py.typed"), "").expect("write py.typed");
    let step_5 = inline_package.join("__init__.pyi");
    std::fs::write(&step_5, "step: int\n").expect("write inline package");

    let paths = ImportSearchPaths {
        roots: vec![root],
        extra_paths: Vec::new(),
        stub_paths: vec![manual],
        workspace_members: Vec::new(),
        site_packages: Some(site_packages),
        registry: None,
        typeshed_path: Some(typeshed),
        typeshed_snapshot: None,
    };

    for expected in [&step_1, &step_2, &step_3, &step_4, &step_5] {
        let actual = resolve_module("typing", &paths).expect("current step must resolve");
        assert_eq!(&actual.path, expected);
        std::fs::remove_file(expected).expect("remove current winner");
        if expected == &step_4 {
            std::fs::remove_dir(step_4.parent().expect("stub parent"))
                .expect("remove empty stub distribution");
        }
    }
    assert!(
        resolve_module("typing", &paths).is_none(),
        "step 6 vendors none"
    );
}

#[test]
fn complete_stub_package_miss_stops_before_inline_package() {
    let temp = tempfile::tempdir().expect("tempdir");
    let site_packages = temp.path();
    let stubs = site_packages.join("foopkg-stubs");
    std::fs::create_dir_all(&stubs).expect("create stubs");
    std::fs::write(stubs.join("__init__.pyi"), "").expect("mark complete package");
    let inline = site_packages.join("foopkg");
    let expected_inline = write_module(&inline, "missing", "pyi");
    std::fs::write(inline.join("py.typed"), "").expect("write marker");

    assert!(
        resolve_module("foopkg.missing", &search_paths(site_packages)).is_none(),
        "a miss in a complete step-4 distribution is terminal, despite {}",
        expected_inline.display()
    );
}

#[test]
fn partial_and_namespace_stub_misses_continue_to_inline_package() {
    for (case, init, marker) in [
        ("partial", true, Some("partial\n")),
        ("namespace", false, None),
    ] {
        let temp = tempfile::tempdir().expect("tempdir");
        let site_packages = temp.path();
        let stubs = site_packages.join("foopkg-stubs");
        std::fs::create_dir_all(&stubs).expect("create stubs");
        if init {
            std::fs::write(stubs.join("__init__.pyi"), "").expect("write init");
        }
        if let Some(contents) = marker {
            std::fs::write(stubs.join("py.typed"), contents).expect("write partial marker");
        }
        let inline = site_packages.join("foopkg");
        let expected = write_module(&inline, "missing", "pyi");
        std::fs::write(inline.join("py.typed"), "").expect("write inline marker");

        let resolved = resolve_module("foopkg.missing", &search_paths(site_packages))
            .expect("partial/namespace miss must continue");
        assert_eq!(resolved.path, expected, "{case}");
    }
}

#[test]
fn namespace_subpackage_marker_enables_step_five() {
    let temp = tempfile::tempdir().expect("tempdir");
    let site_packages = temp.path();
    let stubs = site_packages.join("foopkg-stubs");
    std::fs::create_dir_all(&stubs).expect("create namespace stubs");

    let inline_subpackage = site_packages.join("foopkg").join("sub");
    let expected = write_module(&inline_subpackage, "api", "pyi");
    std::fs::write(inline_subpackage.join("py.typed"), "").expect("write nested namespace marker");

    let resolved = resolve_module("foopkg.sub.api", &search_paths(site_packages))
        .expect("nested marker opts the namespace subpackage into step 5");
    assert_eq!(resolved.path, expected);
}

#[test]
fn step_five_requires_py_typed_and_prefers_pyi() {
    let temp = tempfile::tempdir().expect("tempdir");
    let site_packages = temp.path();
    let package = site_packages.join("foopkg");
    let py = write_module(&package, "api", "py");
    assert!(resolve_module("foopkg.api", &search_paths(site_packages)).is_none());

    std::fs::write(package.join("py.typed"), "").expect("write marker");
    let pyi = write_module(&package, "api", "pyi");
    let resolved = resolve_module("foopkg.api", &search_paths(site_packages))
        .expect("marked inline package resolves");
    assert_eq!(resolved.path, pyi);
    assert_ne!(resolved.path, py);
}

#[test]
fn importer_directory_is_user_code_before_site_packages() {
    let temp = tempfile::tempdir().expect("tempdir");
    let importer_dir = temp.path().join("scripts");
    let expected = write_module(&importer_dir, "foopkg", "py");
    let importer = importer_dir.join("main.py");
    std::fs::write(&importer, "import foopkg\n").expect("write importer");
    let site_packages = temp.path().join("site-packages");
    let installed = site_packages.join("foopkg");
    let _shadow = write_module(&installed, "__init__", "pyi");
    std::fs::write(installed.join("py.typed"), "").expect("write marker");

    let resolved =
        resolve_module_with_importer("foopkg", &search_paths(&site_packages), Some(&importer))
            .expect("importer-local user code resolves");
    assert_eq!(resolved.path, expected);
}
