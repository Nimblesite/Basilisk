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

use std::fmt::Write as _;
use std::fs;
use std::sync::Arc;

use basilisk_checker::imports::{
    classify_unresolved, resolve_module_imports, ActiveTypeshed, ImportSearchPaths,
};
use basilisk_resolver::scope::{ImportResolution, PackageDepKind, UnresolvedReason};
use basilisk_stubs::typeshed::archive::{Archive, ArchiveEntry, ArchiveVfs};
use basilisk_stubs::typeshed::gittree::FileMode;
use basilisk_stubs::typeshed::snapshot::Snapshot;
use basilisk_stubs::typeshed::source::{LicenseStatus, SourceIdentity, SourceKind, TypeshedStatus};
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
        wheels: Vec::new(),
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
        wheels: Vec::new(),
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

/// Regression for GitHub #312 follow-up (comment 5053013115): a user stub may
/// re-export names from a STDLIB stub that resolves through the active step-3
/// Typeshed source — e.g. `MicroPython`'s `uio.pyi` is just `from io import *`,
/// with `io` living in the custom typeshed's `stdlib/` tree. The star target
/// is outside the user stub's own source root, so following the re-export
/// graph must fall back to the active snapshot; otherwise every re-exported
/// name is a false `imports_module_attribute` ("Module `uio` has no attribute
/// `StringIO`").
#[test]
fn user_stub_star_reexport_from_stdlib_stub_is_captured() {
    let stub_dir = make_tmp_dir("bsk_ir_reexport_stdlib");
    // Mirrors micropython-esp32-stubs' uio.pyi verbatim.
    fs::write(stub_dir.join("uio.pyi"), "from io import *\n").unwrap();

    // `io` exists ONLY in the active custom typeshed, not under stub_dir.
    let typeshed = make_custom_typeshed(&[("io.pyi", "class StringIO: ...\nclass BytesIO: ...\n")]);

    let mut paths = make_search_paths(vec![]);
    paths.stub_paths = vec![stub_dir.clone()];
    paths.typeshed_snapshot = Some(ActiveTypeshed::new(typeshed, None));

    let mut resolved = module_with_plain_import("uio");
    resolve_module_imports(&mut resolved, &paths);

    let api = resolved
        .imported_modules
        .get("uio")
        .expect("user stub under stub-paths must be captured");
    for name in ["StringIO", "BytesIO"] {
        assert!(
            api.member_names.contains(name),
            "`{name}` is star-re-exported from the stdlib `io` stub and must be \
             in the captured member API (GitHub #312 follow-up); got {:?}",
            api.member_names
        );
    }

    let _ = fs::remove_dir_all(&stub_dir);
}

/// The chained case from the same report: `uasyncio.pyi` is
/// `from asyncio import *`, and the stdlib `asyncio` stub is a PACKAGE whose
/// own API is built out of *relative* re-exports (`from .tasks import *`,
/// `from .tasks import Task as Task`). After crossing into the snapshot, the
/// re-export walk must keep resolving relative star targets *within* the
/// snapshot.
#[test]
fn user_stub_star_reexport_follows_stdlib_package_reexports() {
    let stub_dir = make_tmp_dir("bsk_ir_reexport_stdlib_pkg");
    // Mirrors micropython-esp32-stubs' uasyncio.pyi verbatim.
    fs::write(stub_dir.join("uasyncio.pyi"), "from asyncio import *\n").unwrap();

    let typeshed = make_custom_typeshed(&[
        (
            "asyncio/__init__.pyi",
            "from .tasks import *\nfrom .tasks import Task as Task\n",
        ),
        (
            "asyncio/tasks.pyi",
            "__all__ = (\"sleep\",)\n\nclass Task: ...\n\nasync def sleep(delay: float) -> None: ...\n",
        ),
    ]);

    let mut paths = make_search_paths(vec![]);
    paths.stub_paths = vec![stub_dir.clone()];
    paths.typeshed_snapshot = Some(ActiveTypeshed::new(typeshed, None));

    let mut resolved = module_with_plain_import("uasyncio");
    resolve_module_imports(&mut resolved, &paths);

    let api = resolved
        .imported_modules
        .get("uasyncio")
        .expect("user stub under stub-paths must be captured");
    for name in ["sleep", "Task"] {
        assert!(
            api.member_names.contains(name),
            "`{name}` reaches uasyncio via stdlib asyncio/__init__.pyi's own \
             re-exports and must be in the captured member API; got {:?}",
            api.member_names
        );
    }

    let _ = fs::remove_dir_all(&stub_dir);
}

/// Real stdlib stubs (`io`, `os`, …) define `__all__`; per runtime
/// `import *` semantics it is authoritative. A cross-boundary star re-export
/// must honour the stdlib stub's `__all__` — include exactly its entries, not
/// every public definition.
#[test]
fn user_stub_star_reexport_honours_stdlib_dunder_all() {
    let stub_dir = make_tmp_dir("bsk_ir_reexport_stdlib_all");
    fs::write(stub_dir.join("uio.pyi"), "from io import *\n").unwrap();

    let typeshed = make_custom_typeshed(&[(
        "io.pyi",
        "__all__ = (\"StringIO\",)\n\nclass StringIO: ...\nclass BytesIO: ...\n",
    )]);

    let mut paths = make_search_paths(vec![]);
    paths.stub_paths = vec![stub_dir.clone()];
    paths.typeshed_snapshot = Some(ActiveTypeshed::new(typeshed, None));

    let mut resolved = module_with_plain_import("uio");
    resolve_module_imports(&mut resolved, &paths);

    let api = resolved
        .imported_modules
        .get("uio")
        .expect("user stub under stub-paths must be captured");
    assert!(
        api.member_names.contains("StringIO"),
        "`StringIO` is in the stdlib stub's __all__ and must be captured; got {:?}",
        api.member_names
    );
    assert!(
        !api.member_names.contains("BytesIO"),
        "`BytesIO` is NOT in the stdlib stub's __all__, so `import *` must not \
         export it; got {:?}",
        api.member_names
    );

    let _ = fs::remove_dir_all(&stub_dir);
}

/// The redundant-alias convention crossing the same boundary:
/// `from io import StringIO as StringIO` in a user stub re-exports the name
/// regardless of where `io` resolves from.
#[test]
fn user_stub_alias_reexport_from_stdlib_stub_is_captured() {
    let stub_dir = make_tmp_dir("bsk_ir_reexport_stdlib_alias");
    fs::write(
        stub_dir.join("uio.pyi"),
        "from io import StringIO as StringIO\n",
    )
    .unwrap();

    let typeshed = make_custom_typeshed(&[("io.pyi", "class StringIO: ...\n")]);

    let mut paths = make_search_paths(vec![]);
    paths.stub_paths = vec![stub_dir.clone()];
    paths.typeshed_snapshot = Some(ActiveTypeshed::new(typeshed, None));

    let mut resolved = module_with_plain_import("uio");
    resolve_module_imports(&mut resolved, &paths);

    let api = resolved
        .imported_modules
        .get("uio")
        .expect("user stub under stub-paths must be captured");
    assert!(
        api.member_names.contains("StringIO"),
        "a redundant-alias re-export from a stdlib stub must be captured; got {:?}",
        api.member_names
    );

    let _ = fs::remove_dir_all(&stub_dir);
}

/// Guard: the snapshot is a FALLBACK, not an override. `umachine.pyi` star-
/// imports `machine`, which exists as a sibling user stub in the same
/// `stub-paths` dir — that local resolution must keep winning even when the
/// active typeshed also happens to carry a module of the same name.
#[test]
fn user_stub_star_reexport_prefers_sibling_stub_over_snapshot() {
    let stub_dir = make_tmp_dir("bsk_ir_reexport_sibling");
    fs::write(stub_dir.join("umachine.pyi"), "from machine import *\n").unwrap();
    fs::write(stub_dir.join("machine.pyi"), "def reset() -> None: ...\n").unwrap();

    let typeshed = make_custom_typeshed(&[("machine.pyi", "def snapshot_only() -> None: ...\n")]);

    let mut paths = make_search_paths(vec![]);
    paths.stub_paths = vec![stub_dir.clone()];
    paths.typeshed_snapshot = Some(ActiveTypeshed::new(typeshed, None));

    let mut resolved = module_with_plain_import("umachine");
    resolve_module_imports(&mut resolved, &paths);

    let api = resolved
        .imported_modules
        .get("umachine")
        .expect("user stub under stub-paths must be captured");
    assert!(
        api.member_names.contains("reset"),
        "the sibling `machine.pyi` in the same stub dir must resolve first; got {:?}",
        api.member_names
    );
    assert!(
        !api.member_names.contains("snapshot_only"),
        "the snapshot must not shadow a same-root sibling stub; got {:?}",
        api.member_names
    );

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
    // import is classified; the registry enriches only the packages it knows.
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

    // Without an active snapshot, `os` is unresolved too; no compiled table may
    // rescue it. The package registry still has no metadata for it.
    let os_import = import("os");
    assert_eq!(
        os_import.unresolved_reason,
        Some(UnresolvedReason::NotInstalled)
    );
    assert_eq!(os_import.package_dep_kind, None);
}

// --------------------------------------------------------------------------
// Custom-typeshed canonicality ([STUBRES-CUSTOM-TYPESHED]).
//
// A configured `typeshed-path` is *the canonical source for standard-library
// types* (typing-spec import-resolution step 3). The load-bearing consequence
// for a PARTIAL custom typeshed (e.g. micropython-stubs, which ships only a
// subset of the stdlib): a stdlib module ABSENT from it must NOT be silently
// rescued by a second source — it has to fall through to `imports_unresolved`,
// exactly as it would for any other missing module.
// These tests pin that behaviour end-to-end through the real apply pipeline.
// --------------------------------------------------------------------------

/// Build the immutable snapshot that acquisition promotes from a custom tree.
fn make_custom_typeshed(stdlib_files: &[(&str, &str)]) -> Arc<Snapshot> {
    let identity = SourceIdentity::Custom {
        digest: "import-apply-custom".to_owned(),
    };
    // One VERSIONS line per TOP-LEVEL module: `os.pyi` → `os`,
    // `asyncio/__init__.pyi` and `asyncio/tasks.pyi` both → `asyncio`.
    let top_level_modules: std::collections::BTreeSet<&str> = stdlib_files
        .iter()
        .map(|(name, _)| {
            name.split('/')
                .next()
                .unwrap_or(name)
                .trim_end_matches(".pyi")
        })
        .collect();
    let versions = top_level_modules
        .into_iter()
        .fold(String::new(), |mut versions, module| {
            // Writing into a `String` cannot fail.
            let _ = writeln!(&mut versions, "{module}: 3.0-");
            versions
        });
    let mut entries = vec![ArchiveEntry {
        path: "stdlib/VERSIONS".to_owned().into(),
        mode: FileMode::Regular,
        data: versions.into_bytes().into(),
    }];
    entries.extend(stdlib_files.iter().map(|(name, body)| ArchiveEntry {
        path: format!("stdlib/{name}").into(),
        mode: FileMode::Regular,
        data: body.as_bytes().to_vec().into(),
    }));
    let status = TypeshedStatus {
        active_source: SourceKind::Custom,
        commit: None,
        tree: None,
        license_status: LicenseStatus::NotSupplied,
        license_reference: None,
        warnings: Vec::new(),
    };
    let uri_identity = identity.uri_component();
    Arc::new(
        Snapshot::build(
            identity,
            status,
            ArchiveVfs::new(uri_identity, Archive::new(entries)),
            None,
        )
        .unwrap(),
    )
}

/// A checker without a promoted snapshot has no step-3 authority and may not
/// rescue imports from compiled Typeshed metadata.
#[test]
fn absent_snapshot_never_uses_compiled_stdlib_recognition() {
    let mut resolved = module_with_plain_imports(&["os", "fractions", "requests"]);
    resolve_module_imports(&mut resolved, &make_search_paths(vec![]));
    assert!(resolved
        .imports
        .iter()
        .all(|import| import.unresolved_reason.is_some()));
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

    // (1) WITHOUT an active snapshot: nothing resolves and no compiled table
    // may claim either stdlib module.
    let mut without = module_with_plain_imports(&["os", "fractions"]);
    resolve_module_imports(&mut without, &make_search_paths(vec![]));
    assert_eq!(imp(&without, "os").resolution, ImportResolution::Unresolved);
    assert!(imp(&without, "os").unresolved_reason.is_some());
    assert!(imp(&without, "fractions").unresolved_reason.is_some());

    // (2) WITH the custom typeshed configured:
    let mut with = module_with_plain_imports(&["os", "fractions"]);
    let mut paths = make_search_paths(vec![]);
    paths.typeshed_snapshot = Some(ActiveTypeshed::new(Arc::clone(&typeshed), None));
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
        os_path.to_string_lossy().starts_with("typeshed:custom-"),
        "os must resolve through the custom snapshot VFS, got {os_path:?}"
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

    // Resolve `import os` against the custom snapshot.
    let mut resolved = module_with_plain_import("os");
    let mut paths = make_search_paths(vec![]);
    paths.typeshed_snapshot = Some(ActiveTypeshed::new(Arc::clone(&typeshed), None));
    resolve_module_imports(&mut resolved, &paths);
    assert!(resolved.imports[0]
        .resolved_path
        .as_ref()
        .unwrap()
        .to_string_lossy()
        .starts_with("typeshed:custom-"));

    // No workspace exports — force the on-demand `.pyi` path in populate.
    let no_workspace = |_: &std::path::Path| -> Option<
        &'static [(String, basilisk_resolver::scope::ExternalSymbol)],
    > { None };

    // WITH the custom typeshed: the stub's `uname` export is CustomTypeshed.
    let mut with_custom = resolved.clone();
    populate_imported_symbols(
        &mut with_custom,
        no_workspace,
        |path, request| {
            basilisk_checker::exports::load_external_module_from_source(
                path,
                "def uname() -> str: ...\n",
                request,
                None,
            )
        },
        &[],
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

    // A normal filesystem `.pyi` remains a plain Tier-1 stub.
    let mut without_custom = resolved.clone();
    without_custom.imports[0].resolved_path = Some(std::path::PathBuf::from("os.pyi"));
    populate_imported_symbols(
        &mut without_custom,
        no_workspace,
        |path, request| {
            basilisk_checker::exports::load_external_module_from_source(
                path,
                "def uname() -> str: ...\n",
                request,
                None,
            )
        },
        &[],
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
}

/// A user stub resolved outside the active Typeshed VFS stays user-authored;
/// snapshot provenance never taints an unrelated step-1 file.
#[test]
fn custom_typeshed_does_not_taint_user_stubs_outside_its_stdlib() {
    use basilisk_checker::exports::populate_imported_symbols;
    use basilisk_stubs::TypeProvenance;

    // A separate user-stub directory is never relabelled as Typeshed.
    let stub_dir = make_tmp_dir("bsk_user_stub_outside_ts");
    fs::write(
        stub_dir.join("cowsay.pyi"),
        "def tux(text: str) -> None: ...\n",
    )
    .unwrap();

    let mut resolved = module_with_plain_import("cowsay");
    let mut paths = make_search_paths(vec![]);
    paths.stub_paths = vec![stub_dir.clone()];
    resolve_module_imports(&mut resolved, &paths);

    // Precondition: `cowsay` resolved to the user stub, NOT under the typeshed.
    let cowsay_path = resolved.imports[0].resolved_path.as_ref().unwrap();
    assert!(cowsay_path.ends_with("cowsay.pyi"));
    assert!(
        !cowsay_path.to_string_lossy().starts_with("typeshed:"),
        "user stub must resolve outside the active Typeshed VFS, got {cowsay_path:?}"
    );

    let no_workspace = |_: &std::path::Path| -> Option<
        &'static [(String, basilisk_resolver::scope::ExternalSymbol)],
    > { None };

    // Even WITH the custom typeshed configured, a stub outside its stdlib/ is a
    // user Tier-1 stub: provenance tracks the on-disk source, not merely whether
    // a custom typeshed exists.
    populate_imported_symbols(
        &mut resolved,
        no_workspace,
        basilisk_checker::exports::load_external_module,
        std::slice::from_ref(&stub_dir),
    );
    assert_eq!(
        resolved.imported_symbols.get("tux").unwrap().provenance,
        Some(TypeProvenance::StubUser),
        "a user stub outside <typeshed>/stdlib/ must stay StubUser even with a \
         custom typeshed configured [STUBRES-CUSTOM-TYPESHED]"
    );

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
