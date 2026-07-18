//! Tests for [ANALYSIS-INCR-IMPORTS]. See docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-INCR-IMPORTS
#![allow(
    clippy::allow_attributes,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    missing_docs
)]
//! `resolve_module_imports` application tests for `basilisk_checker::imports`
//! (relocated from `basilisk-lsp`; behaviour-identical — public API only).

use std::fs;
use std::sync::Arc;

use basilisk_checker::imports::{
    bundled_stdlib_recognized, classify_unresolved, resolve_module_imports, ImportSearchPaths,
};
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
        version: Some(ver.to_owned()),
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
        version: Some("0.1.0".to_owned()),
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

/// Regression for GitHub #312: a package stub's `__init__.pyi` may build its
/// public API entirely out of re-exports — `from .sub import *` and the
/// redundant-alias form `from .sub import Name as Name` — per the typing
/// spec's import conventions
/// (<https://typing.python.org/en/latest/spec/distributing.html#import-conventions>).
/// The captured member API must include those re-exported names, otherwise
/// `imports_module_attribute` reports false "Module `X` has no attribute"
/// errors on spec-valid access (e.g. `asyncio.sleep` with
/// micropython-stdlib-stubs).
#[test]
fn captures_reexports_through_package_init_stub() {
    let stub_dir = make_tmp_dir("bsk_ir_reexport_pkg");
    let pkg = stub_dir.join("aio");
    fs::create_dir_all(&pkg).unwrap();
    // Mirrors micropython-stdlib-stubs' asyncio/__init__.pyi: nothing defined
    // locally, everything re-exported from submodules.
    fs::write(
        pkg.join("__init__.pyi"),
        "from .tasks import *\nfrom .tasks import Task as Task\nfrom .runners import *\n",
    )
    .unwrap();
    // Like typeshed's real tasks.pyi: `Task` arrives via a redundant-alias
    // re-export and `__all__` is a tuple inside version-gated branches.
    fs::write(
        pkg.join("tasks.pyi"),
        "import sys\nfrom _asyncio import Task as Task\n\nif sys.version_info >= (3, 12):\n    __all__ = (\"Task\", \"sleep\")\nelse:\n    __all__ = (\"Task\", \"sleep\")\n\nasync def sleep(delay: float) -> None: ...\n",
    )
    .unwrap();
    fs::write(
        pkg.join("runners.pyi"),
        "__all__ = (\"run\",)\n\ndef run(main: object) -> None: ...\n",
    )
    .unwrap();

    let mut paths = make_search_paths(vec![]);
    paths.stub_paths = vec![stub_dir.clone()];

    let mut resolved = module_with_plain_import("aio");
    resolve_module_imports(&mut resolved, &paths);

    let api = resolved
        .imported_modules
        .get("aio")
        .expect("package __init__.pyi under stub-paths must be captured");
    for name in ["sleep", "Task", "run"] {
        assert!(
            api.member_names.contains(name),
            "`{name}` is re-exported through aio/__init__.pyi and must be in \
             the captured member API (GitHub #312); got {:?}",
            api.member_names
        );
    }

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

// --------------------------------------------------------------------------
// Custom-typeshed canonicality ([STUBRES-CUSTOM-TYPESHED]).
//
// A configured `typeshed-path` is *the canonical source for standard-library
// types* (typing-spec import-resolution step 3). The load-bearing consequence
// for a PARTIAL custom typeshed (e.g. micropython-stubs, which ships only a
// subset of the stdlib): a stdlib module ABSENT from it must NOT be silently
// rescued by the bundled `phf` name-set — it has to fall through to
// `imports_unresolved`, exactly as it would for any other missing module.
// These tests pin that behaviour end-to-end through the real apply pipeline.
// --------------------------------------------------------------------------

/// Build a throwaway custom-typeshed directory whose `stdlib/` subtree contains
/// the given `.pyi` files. Returns the typeshed root (pass it as `typeshed_path`).
fn make_custom_typeshed(stdlib_files: &[(&str, &str)]) -> std::path::PathBuf {
    let root = make_tmp_dir("bsk_custom_typeshed");
    let stdlib = root.join("stdlib");
    fs::create_dir_all(&stdlib).unwrap();
    for (name, body) in stdlib_files {
        fs::write(stdlib.join(name), body).unwrap();
    }
    root
}

/// `bundled_stdlib_recognized` is the single gate that enforces canonicality:
/// it must recognise stdlib names ONLY while no custom typeshed is configured.
#[test]
fn bundled_stdlib_recognized_is_suppressed_by_a_custom_typeshed() {
    // Sanity: both are genuine stdlib modules in the bundled name-set.
    assert!(basilisk_stubs::is_stdlib_module("os"));
    assert!(basilisk_stubs::is_stdlib_module("fractions"));

    // No custom typeshed → the bundled name-set rescues stdlib modules.
    assert!(bundled_stdlib_recognized("os", false));
    assert!(bundled_stdlib_recognized("fractions", false));
    // A non-stdlib name is never rescued, custom typeshed or not.
    assert!(!bundled_stdlib_recognized("requests", false));
    assert!(!bundled_stdlib_recognized("requests", true));
    // A custom typeshed is canonical for step 3 → the bundled name-set is
    // bypassed entirely, so even real stdlib names are no longer rescued here;
    // they must resolve from the typeshed's stdlib/ subtree or fall through.
    assert!(!bundled_stdlib_recognized("os", true));
    assert!(!bundled_stdlib_recognized("fractions", true));
}

/// Through the full `resolve_module_imports` pipeline: a custom typeshed makes a
/// present stdlib module resolve from its `stdlib/` subtree, and — the crux —
/// flips an ABSENT stdlib module from "silently rescued" to "unresolved".
#[test]
fn custom_typeshed_resolves_present_and_fails_absent_stdlib() {
    // `os` present in the custom stdlib, `fractions` deliberately absent.
    let typeshed = make_custom_typeshed(&[("os.pyi", "def uname() -> str: ...\n")]);
    let imp = |m: &basilisk_resolver::ResolvedModule, name: &str| {
        m.imports
            .iter()
            .find(|i| i.module == name)
            .cloned()
            .unwrap()
    };

    // (1) WITHOUT a custom typeshed: nothing resolves on disk, but the bundled
    // name-set rescues BOTH stdlib modules, so neither carries a reason.
    let mut without = module_with_plain_imports(&["os", "fractions"]);
    resolve_module_imports(&mut without, &make_search_paths(vec![]));
    assert_eq!(imp(&without, "os").resolution, ImportResolution::Unresolved);
    assert_eq!(imp(&without, "os").unresolved_reason, None);
    assert_eq!(
        imp(&without, "fractions").unresolved_reason,
        None,
        "without a custom typeshed the bundled name-set rescues stdlib modules"
    );

    // (2) WITH the custom typeshed configured:
    let mut with = module_with_plain_imports(&["os", "fractions"]);
    let mut paths = make_search_paths(vec![]);
    paths.typeshed_path = Some(typeshed.clone());
    resolve_module_imports(&mut with, &paths);

    // `os` resolves to the custom typeshed's own stub — the canonical source.
    let os_import = imp(&with, "os");
    assert_ne!(
        os_import.resolution,
        ImportResolution::Unresolved,
        "os is present in the custom typeshed stdlib/ and must resolve"
    );
    let os_path = os_import.resolved_path.as_ref().unwrap();
    assert!(
        os_path.starts_with(typeshed.join("stdlib")),
        "os must resolve under the custom typeshed's stdlib/, got {os_path:?}"
    );
    assert!(os_path.ends_with("os.pyi"));
    assert_eq!(os_import.unresolved_reason, None);

    // `fractions` is absent from the custom typeshed. Because the typeshed is
    // canonical for step 3, the bundled name-set no longer rescues it: it falls
    // through to unresolved WITH a reason. This is the whole point of the fix.
    let fractions = imp(&with, "fractions");
    assert_eq!(fractions.resolution, ImportResolution::Unresolved);
    assert!(
        fractions.unresolved_reason.is_some(),
        "a stdlib module absent from a custom typeshed must fall through to \
         unresolved, not be rescued by the bundled name-set [STUBRES-CUSTOM-TYPESHED]"
    );

    let _ = fs::remove_dir_all(&typeshed);
}

/// Exports pulled from a custom-typeshed stub carry `StubCustomTypeshed`
/// provenance (hover reads "(custom typeshed)"); the SAME stub read without a
/// custom typeshed configured is a plain Tier-1 stub. Provenance must track the
/// source, never masquerade a `MicroPython` signature as bundled `CPython`.
#[test]
fn custom_typeshed_stub_exports_carry_custom_provenance() {
    use basilisk_checker::exports::populate_imported_symbols;
    use basilisk_stubs::TypeProvenance;

    let typeshed = make_custom_typeshed(&[("os.pyi", "def uname() -> str: ...\n")]);

    // Resolve `import os` against the custom typeshed so the import's
    // resolved_path points at <typeshed>/stdlib/os.pyi.
    let mut resolved = module_with_plain_import("os");
    let mut paths = make_search_paths(vec![]);
    paths.typeshed_path = Some(typeshed.clone());
    resolve_module_imports(&mut resolved, &paths);
    assert!(resolved.imports[0]
        .resolved_path
        .as_ref()
        .unwrap()
        .starts_with(typeshed.join("stdlib")));

    // No workspace exports — force the on-demand `.pyi` path in populate.
    let no_workspace = |_: &std::path::Path| -> Option<
        &'static [(String, basilisk_resolver::scope::ExternalSymbol)],
    > { None };

    // WITH the custom typeshed: the stub's `uname` export is CustomTypeshed.
    let mut with_custom = resolved.clone();
    populate_imported_symbols(
        &mut with_custom,
        no_workspace,
        basilisk_checker::exports::load_external_module,
        Some(&typeshed),
    );
    let uname = with_custom
        .imported_symbols
        .get("uname")
        .expect("plain `import os` must expose the stub's `uname` export");
    assert_eq!(
        uname.provenance,
        Some(TypeProvenance::StubCustomTypeshed),
        "a stub under the custom typeshed stdlib/ must carry StubCustomTypeshed \
         provenance so hover reads \"(custom typeshed)\" [STUBRES-CUSTOM-TYPESHED]"
    );

    // WITHOUT the custom typeshed argument (same file on disk): a plain Tier-1
    // stub. Only the `custom_typeshed` argument decides provenance.
    let mut without_custom = resolved.clone();
    populate_imported_symbols(
        &mut without_custom,
        no_workspace,
        basilisk_checker::exports::load_external_module,
        None,
    );
    assert_eq!(
        without_custom
            .imported_symbols
            .get("uname")
            .unwrap()
            .provenance,
        Some(TypeProvenance::StubTier1),
        "the same stub read without a custom typeshed is a plain Tier-1 stub"
    );

    let _ = fs::remove_dir_all(&typeshed);
}

/// The `custom_typeshed` argument alone must NOT stamp `StubCustomTypeshed`
/// provenance: a stub resolved from OUTSIDE the typeshed's `stdlib/` subtree
/// (here a user stub from `stub-paths`) stays a plain Tier-1 stub even while a
/// custom typeshed is configured. Only files under `<typeshed>/stdlib/` are the
/// custom typeshed's own ([STUBRES-CUSTOM-TYPESHED]). This pins the path gate in
/// `stub_source_for` — a mutant that ignores the path and always returns
/// `CustomTypeshed` when a typeshed is configured is caught here.
#[test]
fn custom_typeshed_does_not_taint_user_stubs_outside_its_stdlib() {
    use basilisk_checker::exports::populate_imported_symbols;
    use basilisk_stubs::TypeProvenance;

    // A configured custom typeshed (its stdlib/ ships os.pyi)…
    let typeshed = make_custom_typeshed(&[("os.pyi", "def uname() -> str: ...\n")]);
    // …and a SEPARATE user-stub dir, entirely outside the typeshed tree.
    let stub_dir = make_tmp_dir("bsk_user_stub_outside_ts");
    fs::write(
        stub_dir.join("cowsay.pyi"),
        "def tux(text: str) -> None: ...\n",
    )
    .unwrap();

    let mut resolved = module_with_plain_import("cowsay");
    let mut paths = make_search_paths(vec![]);
    paths.stub_paths = vec![stub_dir.clone()];
    paths.typeshed_path = Some(typeshed.clone());
    resolve_module_imports(&mut resolved, &paths);

    // Precondition: `cowsay` resolved to the user stub, NOT under the typeshed.
    let cowsay_path = resolved.imports[0].resolved_path.as_ref().unwrap();
    assert!(cowsay_path.ends_with("cowsay.pyi"));
    assert!(
        !cowsay_path.starts_with(typeshed.join("stdlib")),
        "user stub must resolve outside the custom typeshed's stdlib/, got {cowsay_path:?}"
    );

    let no_workspace = |_: &std::path::Path| -> Option<
        &'static [(String, basilisk_resolver::scope::ExternalSymbol)],
    > { None };

    // Even WITH the custom typeshed configured, a stub outside its stdlib/ is a
    // plain Tier-1 stub: provenance tracks the on-disk source, not merely whether
    // a custom typeshed exists.
    populate_imported_symbols(
        &mut resolved,
        no_workspace,
        basilisk_checker::exports::load_external_module,
        Some(&typeshed),
    );
    assert_eq!(
        resolved.imported_symbols.get("tux").unwrap().provenance,
        Some(TypeProvenance::StubTier1),
        "a user stub outside <typeshed>/stdlib/ must stay StubTier1 even with a \
         custom typeshed configured [STUBRES-CUSTOM-TYPESHED]"
    );

    let _ = fs::remove_dir_all(&typeshed);
    let _ = fs::remove_dir_all(&stub_dir);
}

#[test]
fn user_and_generated_stub_exports_keep_honest_provenance_on_disk_and_source() {
    use basilisk_checker::exports::{extract_stub_exports, extract_stub_exports_from_source};
    use basilisk_stubs::{StubSource, TypeProvenance};

    let dir = make_tmp_dir("bsk_user_generated_provenance");
    let manual_source = "def manual() -> int: ...\n";
    let generated_source =
        "# Auto-generated stub for `demo` (AST analysis)\ndef generated() -> int: ...\n";

    for (name, source, expected) in [
        ("manual", manual_source, None),
        (
            "generated",
            generated_source,
            Some(TypeProvenance::StubTier3),
        ),
    ] {
        let path = dir.join(format!("{name}.pyi"));
        fs::write(&path, source).unwrap();
        for exports in [
            extract_stub_exports(&path, name, StubSource::UserStub),
            extract_stub_exports_from_source(source, &path, name, StubSource::UserStub, None),
        ] {
            let provenance = exports
                .first()
                .and_then(|(_, symbol)| symbol.provenance)
                .expect("stub export provenance");
            match expected {
                Some(expected) => assert_eq!(provenance, expected),
                None => assert_eq!(
                    provenance.hover_label(),
                    None,
                    "a manual/create-local user stub must not be branded typeshed"
                ),
            }
        }
    }

    let _ = fs::remove_dir_all(&dir);
}
