//! Focused precedence tests for bundled-standard-library import resolution.

#![expect(
    clippy::expect_used,
    reason = "test-only filesystem setup uses expect for clear failures"
)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use basilisk_stubs::typeshed::archive::{Archive, ArchiveEntry, ArchiveVfs};
use basilisk_stubs::typeshed::gittree::FileMode;
use basilisk_stubs::typeshed::snapshot::Snapshot;
use basilisk_stubs::typeshed::source::{LicenseStatus, SourceIdentity, SourceKind, TypeshedStatus};

use super::{resolve_module, resolve_module_with_importer, ActiveTypeshed, ImportSearchPaths};

fn search_paths(site_packages: &Path) -> ImportSearchPaths {
    ImportSearchPaths {
        roots: Vec::new(),
        extra_paths: Vec::new(),
        stub_paths: Vec::new(),
        workspace_members: Vec::new(),
        site_packages: Some(site_packages.to_path_buf()),
        registry: None,
        typeshed_snapshot: None,
    }
}

fn write_module(directory: &Path, name: &str, extension: &str) -> PathBuf {
    std::fs::create_dir_all(directory).expect("create module directory");
    let path = directory.join(format!("{name}.{extension}"));
    std::fs::write(&path, "value: int = 1\n").expect("write module");
    path
}

fn custom_snapshot(modules: &[(&str, &str)]) -> ActiveTypeshed {
    let identity = SourceIdentity::Custom {
        digest: "resolve-tests".to_owned(),
    };
    let versions = modules
        .iter()
        .fold(String::new(), |mut versions, (module, _)| {
            // Writing into a `String` cannot fail.
            let _ = writeln!(&mut versions, "{module}: 3.0-");
            versions
        });
    let mut entries = if versions.is_empty() {
        Vec::new()
    } else {
        vec![ArchiveEntry {
            path: "stdlib/VERSIONS".to_owned(),
            mode: FileMode::Regular,
            data: versions.into_bytes(),
        }]
    };
    entries.extend(modules.iter().map(|(module, body)| ArchiveEntry {
        path: format!("stdlib/{module}.pyi"),
        mode: FileMode::Regular,
        data: body.as_bytes().to_vec(),
    }));
    let status = TypeshedStatus {
        active_source: SourceKind::Custom,
        commit: None,
        tree: None,
        license_status: LicenseStatus::NotSupplied,
        license_reference: None,
        warnings: Vec::new(),
    };
    let snapshot = Snapshot::build(
        identity,
        status,
        ArchiveVfs::new("custom-resolve-tests", Archive::new(entries)),
        None,
    )
    .expect("valid custom snapshot");
    ActiveTypeshed::new(Arc::new(snapshot), None)
}

#[test]
fn active_stdlib_is_terminal_before_site_packages() {
    let temp = tempfile::tempdir().expect("tempdir");
    let site_packages = temp.path().join("site-packages");
    let _shadow = write_module(&site_packages, "typing", "py");

    assert!(
        resolve_module(
            "typing",
            &ImportSearchPaths {
                typeshed_snapshot: Some(custom_snapshot(&[("typing", "value: int\n")])),
                ..search_paths(&site_packages)
            }
        )
        .is_some_and(|resolved| resolved.path.to_string_lossy().starts_with("typeshed:")),
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
fn custom_typeshed_miss_proceeds_to_installed_source() {
    let temp = tempfile::tempdir().expect("tempdir");
    let site_packages = temp.path().join("site-packages");
    let installed = write_module(&site_packages, "typing", "py");
    let mut paths = search_paths(&site_packages);
    paths.typeshed_snapshot = Some(custom_snapshot(&[]));

    let resolved = resolve_module("typing", &paths)
        .expect("a step-3 custom-typeshed miss must continue through later steps");
    assert_eq!(resolved.path, installed);
}

#[test]
fn custom_typeshed_stdlib_stub_resolves_before_terminal_step() {
    let temp = tempfile::tempdir().expect("tempdir");
    let site_packages = temp.path().join("site-packages");
    let _shadow = write_module(&site_packages, "typing", "py");
    let mut paths = search_paths(&site_packages);
    paths.typeshed_snapshot = Some(custom_snapshot(&[("typing", "value: int\n")]));

    let resolved = resolve_module("typing", &paths).expect("typeshed stub must resolve");
    assert!(resolved.path.to_string_lossy().starts_with("typeshed:"));
}

#[test]
fn custom_versions_admits_a_non_cpython_stdlib_module() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut paths = search_paths(&temp.path().join("site-packages"));
    paths.typeshed_snapshot = Some(custom_snapshot(&[("uasyncio", "value: int\n")]));

    let resolved = resolve_module("uasyncio", &paths).expect("custom VERSIONS admits module");
    assert!(resolved
        .path
        .to_string_lossy()
        .ends_with("stdlib/uasyncio.pyi"));
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
    let site_packages = temp.path().join("site-packages");
    let stub_package = site_packages.join("typing-stubs");
    let inline_package = site_packages.join("typing");

    let step_1 = write_module(&manual, "typing", "pyi");
    let step_2 = write_module(&root, "typing", "py");
    let step_3 = PathBuf::from("typeshed:custom-resolve-tests/stdlib/typing.pyi");
    std::fs::create_dir_all(&stub_package).expect("create stub package");
    let step_4 = stub_package.join("__init__.pyi");
    std::fs::write(&step_4, "step: int\n").expect("write stub package");
    std::fs::create_dir_all(&inline_package).expect("create inline package");
    std::fs::write(inline_package.join("py.typed"), "").expect("write py.typed");
    let step_5 = inline_package.join("__init__.pyi");
    std::fs::write(&step_5, "step: int\n").expect("write inline package");

    let mut paths = ImportSearchPaths {
        roots: vec![root],
        extra_paths: Vec::new(),
        stub_paths: vec![manual],
        workspace_members: Vec::new(),
        site_packages: Some(site_packages),
        registry: None,
        typeshed_snapshot: Some(custom_snapshot(&[("typing", "step: int\n")])),
    };

    for expected in [&step_1, &step_2] {
        let actual = resolve_module("typing", &paths).expect("current step must resolve");
        assert_eq!(&actual.path, expected);
        std::fs::remove_file(expected).expect("remove current winner");
    }
    assert_eq!(
        resolve_module("typing", &paths).map(|resolved| resolved.path),
        Some(step_3)
    );
    paths.typeshed_snapshot = Some(custom_snapshot(&[]));
    assert_eq!(
        resolve_module("typing", &paths).map(|resolved| resolved.path),
        Some(step_4.clone())
    );
    std::fs::remove_file(&step_4).expect("remove stub-package winner");
    std::fs::remove_dir(step_4.parent().expect("stub parent"))
        .expect("remove empty stub distribution");
    assert_eq!(
        resolve_module("typing", &paths).map(|resolved| resolved.path),
        Some(step_5.clone())
    );
    std::fs::remove_file(step_5).expect("remove inline winner");
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
fn step_five_resolves_untyped_source_and_prefers_pyi() {
    let temp = tempfile::tempdir().expect("tempdir");
    let site_packages = temp.path();
    let package = site_packages.join("foopkg");
    let py = write_module(&package, "api", "py");
    let untyped = resolve_module("foopkg.api", &search_paths(site_packages))
        .expect("an installed untyped module still resolves");
    assert_eq!(untyped.path, py);

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

/// GitHub #336 (bug 3): a `stub-paths` stub that declares nothing must be
/// treated as absent. Resolution falls through to the installed `.py` source,
/// so BSK-0152 still reports the package as untyped — an empty stub must not
/// turn the gate green while providing no type information.
#[test]
fn stub_path_stub_declaring_nothing_is_treated_as_absent() {
    let temp = tempfile::tempdir().expect("tempdir");
    let stubs = temp.path().join("stubs");
    std::fs::create_dir_all(&stubs).expect("create stubs dir");
    std::fs::write(
        stubs.join("foopkg.pyi"),
        "# Auto-generated stub\n\nfrom typing import Any\n",
    )
    .expect("write empty stub");
    let site_packages = temp.path().join("site-packages");
    let expected = write_module(&site_packages, "foopkg", "py");
    let mut paths = search_paths(&site_packages);
    paths.stub_paths.push(stubs);

    let resolved = resolve_module("foopkg", &paths)
        .expect("resolution must fall through to the installed source");
    assert_eq!(
        resolved.path, expected,
        "a declaration-less stub must not shadow the installed source"
    );
    assert_eq!(
        resolved.resolution,
        basilisk_resolver::scope::ImportResolution::SourcePy,
        "the fall-through resolution must be SourcePy so BSK-0152 can fire"
    );
}

/// GitHub #336 follow-up: a `stub-paths` stub that fails to parse (e.g. the
/// generator's former `-> <class 'str'>` output) must likewise be treated as
/// absent instead of silently counting as type information.
#[test]
fn stub_path_stub_that_fails_to_parse_is_treated_as_absent() {
    let temp = tempfile::tempdir().expect("tempdir");
    let stubs = temp.path().join("stubs");
    std::fs::create_dir_all(&stubs).expect("create stubs dir");
    std::fs::write(
        stubs.join("foopkg.pyi"),
        "from typing import Any\n\ndef get_host(self) -> <class 'str'>: ...\n",
    )
    .expect("write broken stub");
    let site_packages = temp.path().join("site-packages");
    let expected = write_module(&site_packages, "foopkg", "py");
    let mut paths = search_paths(&site_packages);
    paths.stub_paths.push(stubs);

    let resolved = resolve_module("foopkg", &paths)
        .expect("resolution must fall through to the installed source");
    assert_eq!(
        resolved.path, expected,
        "an unparseable stub must not shadow the installed source"
    );
    assert_eq!(
        resolved.resolution,
        basilisk_resolver::scope::ImportResolution::SourcePy,
        "the fall-through resolution must be SourcePy so BSK-0152 can fire"
    );
}

/// Guard: a `stub-paths` stub with real declarations keeps winning step 1 —
/// treating empty stubs as absent must not disturb valid manual stubs.
#[test]
fn stub_path_stub_with_declarations_still_wins_step_one() {
    let temp = tempfile::tempdir().expect("tempdir");
    let stubs = temp.path().join("stubs");
    std::fs::create_dir_all(&stubs).expect("create stubs dir");
    let expected = stubs.join("foopkg.pyi");
    std::fs::write(&expected, "class Widget: ...\n").expect("write real stub");
    let site_packages = temp.path().join("site-packages");
    let _shadow = write_module(&site_packages, "foopkg", "py");
    let mut paths = search_paths(&site_packages);
    paths.stub_paths.push(stubs);

    let resolved = resolve_module("foopkg", &paths).expect("manual stub must resolve");
    assert_eq!(resolved.path, expected);
}
