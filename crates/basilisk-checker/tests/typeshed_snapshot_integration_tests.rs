//! Active Typeshed Snapshot/VFS integration acceptance tests.
#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    missing_docs
)]

use std::collections::HashMap;
use std::fs;
use std::sync::Arc;

use basilisk_checker::imports::{resolve_module, ActiveTypeshed, ImportSearchPaths};
use basilisk_checker::{
    cross_resolved_module, FileRegistry, ResolvedFile, SearchPathsInput, SourceFile, WorkspaceFiles,
};
use basilisk_config::{BasiliskConfig, RuleSeverity};
use basilisk_stubs::types::{StubTarget, StubTargetPlatform};
use basilisk_stubs::typeshed::archive::{Archive, ArchiveEntry, ArchiveVfs};
use basilisk_stubs::typeshed::gittree::FileMode;
use basilisk_stubs::typeshed::snapshot::Snapshot;
use basilisk_stubs::typeshed::source::{
    LicenseStatus, Provenance, SourceIdentity, SourceKind, Transport, TypeshedStatus,
};
use basilisk_test_utils::EventDb;

fn entry(path: &str, source: &str) -> ArchiveEntry {
    ArchiveEntry {
        path: path.to_owned(),
        mode: FileMode::Regular,
        data: source.as_bytes().to_vec(),
    }
}

fn micropython_snapshot() -> Arc<Snapshot> {
    let identity = SourceIdentity::Custom {
        digest: "micropython-e2e".to_owned(),
    };
    let archive = Archive::new(vec![
        entry(
            "stdlib/VERSIONS",
            "asyncio: 3.11-\nasyncio.tasks: 3.11-\nasyncio.runners: 3.11-\n",
        ),
        entry(
            "stdlib/asyncio/__init__.pyi",
            "from .tasks import *\nfrom .tasks import Task as Task\nfrom .runners import *\n",
        ),
        entry(
            "stdlib/asyncio/tasks.pyi",
            "import sys\nif sys.platform == \"micropython\":\n    __all__ = (\"Task\", \"sleep\")\n    class Task: ...\n    async def sleep(delay: int) -> None: ...\nelse:\n    __all__ = (\"CpythonOnly\",)\n    class CpythonOnly: ...\n",
        ),
        entry(
            "stdlib/asyncio/runners.pyi",
            "__all__ = (\"run\",)\ndef run(main: object) -> None: ...\n",
        ),
    ]);
    let status = TypeshedStatus {
        active_source: SourceKind::Custom,
        commit: None,
        tree: None,
        transport: Transport::CustomPath,
        license_status: LicenseStatus::NotSupplied,
        license_reference: None,
        provenance: Provenance::UserManaged,
        signed_release: false,
        warnings: Vec::new(),
    };
    Arc::new(
        Snapshot::build(
            identity,
            status,
            ArchiveVfs::new("custom-micropython-e2e", archive),
            Some("yaml\tcustom-types-PyYAML\n"),
        )
        .expect("valid MicroPython snapshot"),
    )
}

fn search_paths(snapshot: Arc<Snapshot>) -> ImportSearchPaths {
    ImportSearchPaths {
        roots: Vec::new(),
        extra_paths: Vec::new(),
        stub_paths: Vec::new(),
        workspace_members: Vec::new(),
        site_packages: None,
        registry: None,
        typeshed_snapshot: Some(ActiveTypeshed::new(
            snapshot,
            Some(StubTarget {
                python_version: (3, 12),
                platform: StubTargetPlatform::Concrete("micropython".to_owned()),
            }),
        )),
    }
}

#[test]
fn active_snapshot_supplies_resolution_target_body_indexes_and_reexports() {
    let snapshot = micropython_snapshot();
    let paths = search_paths(Arc::clone(&snapshot));

    let asyncio = resolve_module("asyncio", &paths).expect("asyncio resolves from active VFS");
    assert_eq!(
        asyncio.path.to_string_lossy(),
        "typeshed:custom-micropython-e2e/stdlib/asyncio/__init__.pyi"
    );
    assert!(snapshot.versions_index.admits("asyncio.tasks", (3, 12)));
    assert_eq!(
        snapshot.module_index.path("asyncio.runners"),
        Some("stdlib/asyncio/runners.pyi")
    );
    assert_eq!(
        snapshot
            .read_stub("asyncio.tasks")
            .map(|(_, body)| body.contains("sleep(delay: int)")),
        Some(true),
        "the body must come from the selected generation, not a name-only baseline"
    );

    let db = EventDb::default();
    let search_input = SearchPathsInput::new(&db, paths);
    let workspace = WorkspaceFiles::new(&db, FileRegistry::default());
    let file = SourceFile::new(
        &db,
        "main.py".to_owned(),
        "from asyncio import sleep, Task, run\n".to_owned(),
    );
    let resolved = cross_resolved_module(&db, file, search_input, workspace);
    let ResolvedFile::Resolved(resolved) = resolved else {
        panic!("fixture must parse and resolve");
    };
    for name in ["sleep", "Task", "run"] {
        assert!(
            resolved.imported_symbols.contains_key(name),
            "MicroPython asyncio re-export `{name}` must resolve through the active VFS"
        );
    }
    assert!(
        !resolved.imported_symbols.contains_key("CpythonOnly"),
        "a concrete MicroPython target must not union the CPython guard branch"
    );
}

#[test]
fn plain_asyncio_import_exposes_active_snapshot_reexports_as_module_attributes() {
    let db = EventDb::default();
    let search_input = SearchPathsInput::new(&db, search_paths(micropython_snapshot()));
    let workspace = WorkspaceFiles::new(&db, FileRegistry::default());
    let file = SourceFile::new(
        &db,
        "main.py".to_owned(),
        "import asyncio\nasleep = asyncio.sleep\natask = asyncio.Task\narun = asyncio.run\nbad = asyncio.missing\n"
            .to_owned(),
    );
    let ResolvedFile::Resolved(resolved) =
        cross_resolved_module(&db, file, search_input, workspace)
    else {
        panic!("plain asyncio fixture must parse and resolve");
    };

    let api = resolved
        .imported_modules
        .get("asyncio")
        .expect("plain import must bind the active package API");
    for member in ["sleep", "Task", "run"] {
        assert!(
            api.member_names.contains(member),
            "active asyncio package must expose re-export `{member}`"
        );
    }

    let config = BasiliskConfig::with_rule_entries(HashMap::from([(
        "imports_module_attribute".to_owned(),
        RuleSeverity::Error,
    )]));
    let diagnostics = basilisk_checker::check_with_config(resolved, &config)
        .into_iter()
        .filter(|diagnostic| diagnostic.code.code == "imports_module_attribute")
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 1, "only asyncio.missing is invalid");
    assert!(diagnostics
        .first()
        .is_some_and(|diagnostic| diagnostic.message.contains("missing")));
}

#[test]
fn active_snapshot_miss_continues_to_installed_stub_package() {
    let root = std::env::temp_dir().join(format!(
        "basilisk_typeshed_snapshot_fallback_{}_{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let site_packages = root.join("site-packages");
    let stub_package = site_packages.join("fallback-stubs");
    fs::create_dir_all(&stub_package).unwrap();
    fs::write(
        stub_package.join("__init__.pyi"),
        "def from_step_four() -> int: ...\n",
    )
    .unwrap();

    let mut paths = search_paths(micropython_snapshot());
    paths.site_packages = Some(site_packages);
    let resolved = resolve_module("fallback", &paths)
        .expect("a step-3 Snapshot miss must continue to step-4 stub packages");
    assert!(resolved.path.ends_with("fallback-stubs/__init__.pyi"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn distribution_suggestions_come_from_the_active_generation() {
    let paths = search_paths(micropython_snapshot());
    let parsed = basilisk_parser::parse_source("import yaml\n".to_owned(), "main.py".to_owned())
        .expect("fixture parses");
    let mut resolved = basilisk_resolver::resolve(&parsed).expect("fixture resolves");

    basilisk_checker::imports::resolve_module_imports(&mut resolved, &paths);

    assert_eq!(
        resolved
            .imports
            .first()
            .and_then(|import| import.stub_distribution.as_deref()),
        Some("custom-types-PyYAML"),
        "diagnostics and code actions must not consult the bundled distribution table"
    );
}

#[test]
fn active_snapshot_miss_does_not_mix_in_another_stdlib_source() {
    let paths = search_paths(micropython_snapshot());
    let parsed =
        basilisk_parser::parse_source("import fractions\n".to_owned(), "main.py".to_owned())
            .expect("fixture parses");
    let mut resolved = basilisk_resolver::resolve(&parsed).expect("fixture resolves");
    basilisk_checker::imports::resolve_module_imports(&mut resolved, &paths);

    let config = BasiliskConfig::with_rule_entries(HashMap::from([(
        "imports_unresolved".to_owned(),
        RuleSeverity::Error,
    )]));
    let diagnostics = basilisk_checker::check_with_config(&resolved, &config);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.code == "imports_unresolved"),
        "an active-generation miss must not mix in another step-3 source"
    );
}

#[test]
fn sole_root_snapshot_and_target_apply_to_an_unowned_importer() {
    let snapshot = micropython_snapshot();
    let root = std::env::temp_dir().join("basilisk_typeshed_single_root_target");
    let workspace_root = root.join("workspace");
    let external_file = root.join("external/main.py");
    let mut paths = search_paths(Arc::clone(&snapshot));
    paths.typeshed_snapshot = ActiveTypeshed::from_roots(vec![(
        workspace_root,
        snapshot,
        Some(StubTarget {
            python_version: (3, 12),
            platform: StubTargetPlatform::Concrete("micropython".to_owned()),
        }),
    )]);

    let db = EventDb::default();
    let search_input = SearchPathsInput::new(&db, paths);
    let workspace = WorkspaceFiles::new(&db, FileRegistry::default());
    let file = SourceFile::new(
        &db,
        external_file.to_string_lossy().into_owned(),
        "from asyncio.tasks import sleep, CpythonOnly\n".to_owned(),
    );
    let ResolvedFile::Resolved(resolved) =
        cross_resolved_module(&db, file, search_input, workspace)
    else {
        panic!("external single-root fixture must parse and resolve");
    };

    assert!(
        resolved.imported_symbols.contains_key("sleep"),
        "an unowned importer must use the sole root's active snapshot and MicroPython target"
    );
    assert!(
        !resolved.imported_symbols.contains_key("CpythonOnly"),
        "the sole-root fallback must preserve the root's target guards"
    );
}

#[test]
fn shared_snapshot_is_memoized_per_root_target_and_never_leaks_to_unmatched_roots() {
    let snapshot = micropython_snapshot();
    let root = std::env::temp_dir().join("basilisk_typeshed_root_targets");
    let micropython_root = root.join("micropython");
    let cpython_root = root.join("cpython");
    let mut paths = search_paths(Arc::clone(&snapshot));
    paths.typeshed_snapshot = ActiveTypeshed::from_roots(vec![
        (
            micropython_root.clone(),
            Arc::clone(&snapshot),
            Some(StubTarget {
                python_version: (3, 12),
                platform: StubTargetPlatform::Concrete("micropython".to_owned()),
            }),
        ),
        (
            cpython_root.clone(),
            Arc::clone(&snapshot),
            Some(StubTarget {
                python_version: (3, 12),
                platform: StubTargetPlatform::Concrete("linux".to_owned()),
            }),
        ),
    ]);

    assert!(
        basilisk_checker::imports::resolve_module_with_importer(
            "asyncio",
            &paths,
            Some(&root.join("unmatched/main.py")),
        )
        .is_none(),
        "a failed/unconfigured root must not inherit another root's active generation"
    );

    let db = EventDb::default();
    let search_input = SearchPathsInput::new(&db, paths);
    let workspace = WorkspaceFiles::new(&db, FileRegistry::default());
    let source = "from asyncio.tasks import sleep, CpythonOnly\n".to_owned();
    let micropython_file = SourceFile::new(
        &db,
        micropython_root
            .join("main.py")
            .to_string_lossy()
            .into_owned(),
        source.clone(),
    );
    let cpython_file = SourceFile::new(
        &db,
        cpython_root.join("main.py").to_string_lossy().into_owned(),
        source,
    );

    let ResolvedFile::Resolved(micropython) =
        cross_resolved_module(&db, micropython_file, search_input, workspace)
    else {
        panic!("MicroPython fixture must resolve");
    };
    assert!(micropython.imported_symbols.contains_key("sleep"));
    assert!(!micropython.imported_symbols.contains_key("CpythonOnly"));

    let ResolvedFile::Resolved(cpython) =
        cross_resolved_module(&db, cpython_file, search_input, workspace)
    else {
        panic!("CPython fixture must resolve");
    };
    assert!(!cpython.imported_symbols.contains_key("sleep"));
    assert!(cpython.imported_symbols.contains_key("CpythonOnly"));
}
