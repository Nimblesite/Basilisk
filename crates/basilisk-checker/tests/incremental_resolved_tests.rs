//! Tests for [CHKARCH-INCREMENTAL-SALSA]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA
#![allow(
    clippy::allow_attributes,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    missing_docs
)]
//! Behavioural tests for the **import-resolved** salsa query
//! (`checked_file_resolved`): it must equal the CLI's import-resolving pipeline
//! byte-for-byte, actually apply import resolution (so `imports_unresolved`
//! reflects the search paths), and invalidate when the file, config, OR the
//! `SearchPathsInput` changes.

use std::collections::HashMap;
use std::fs;

use basilisk_checker::imports::ImportSearchPaths;
use basilisk_checker::{
    check_with_config, checked_file_resolved, file_diagnostics_resolved, resolved_module,
    ConfigInput, ConfigValue, Diagnostic, FileRegistry, ResolvedFile, SearchPathsInput, SourceFile,
    WorkspaceFiles,
};
use basilisk_config::BasiliskConfig;
use basilisk_resolver::scope::ImportResolution;
use basilisk_test_utils::EventDb;
use salsa::Setter;

mod import_support;
use import_support::{make_search_paths, make_tmp_dir};

/// A unique module name so an unresolved `import` never accidentally resolves
/// against the test process's working directory.
const PROBE: &str = "zzz_unique_import_probe";

fn e0001_config() -> BasiliskConfig {
    BasiliskConfig {
        rules: HashMap::from([("BSK-E0001".to_owned(), basilisk_config::RuleSeverity::Error)]),
        ..BasiliskConfig::default()
    }
}

fn default_config(db: &EventDb) -> ConfigInput {
    ConfigInput::new(db, ConfigValue(BasiliskConfig::default()))
}

/// An empty workspace file registry (no cross-file edges) for tests that do not
/// exercise cross-file invalidation.
fn empty_workspace(db: &EventDb) -> WorkspaceFiles {
    WorkspaceFiles::new(db, FileRegistry::default())
}

/// The exact import-resolving pipeline `basilisk-cli`'s `process_file` runs.
fn reference_resolved(
    path: &str,
    text: &str,
    search_paths: &ImportSearchPaths,
    config: &BasiliskConfig,
) -> Vec<Diagnostic> {
    let parsed = basilisk_parser::parse_source(text.to_owned(), path.to_owned()).expect("parse");
    let mut resolved = basilisk_resolver::resolve(&parsed).expect("resolve");
    basilisk_checker::imports::resolve_module_imports(&mut resolved, search_paths);
    check_with_config(&resolved, config)
}

fn assert_same(got: &[Diagnostic], want: &[Diagnostic], label: &str) {
    assert_eq!(got.len(), want.len(), "{label}: diagnostic count");
    for (g, w) in got.iter().zip(want) {
        assert_eq!(g.code.code, w.code.code, "{label}: code");
        assert_eq!(g.span, w.span, "{label}: span");
        assert_eq!(g.message, w.message, "{label}: message");
        assert_eq!(g.severity, w.severity, "{label}: severity");
        assert_eq!(g.help, w.help, "{label}: help");
        assert_eq!(g.provenance, w.provenance, "{label}: provenance");
    }
}

/// The memoized resolved query equals the direct import-resolving pipeline
/// byte-for-byte, including for import-bearing files.
#[test]
fn resolved_query_equivalent_to_direct_import_pipeline() {
    let db = EventDb::default();
    let workspace = empty_workspace(&db);
    let config = default_config(&db);
    let sp_value = make_search_paths(vec![]);
    let search_paths = SearchPathsInput::new(&db, sp_value.clone());

    let fixtures: &[(&str, &str)] = &[
        ("clean.py", "x: int = 1\n"),
        ("bad_assign.py", "x: int = \"nope\"\n"),
        (
            "imports.py",
            "import nonexistent_pkg\n\nx = nonexistent_pkg.frobnicate()\n",
        ),
    ];
    for (path, src) in fixtures {
        let file = SourceFile::new(&db, (*path).to_owned(), (*src).to_owned());
        let got = file_diagnostics_resolved(&db, file, config, search_paths, workspace);
        let want = reference_resolved(path, src, &sp_value, &BasiliskConfig::default());
        assert_same(&got, &want, path);
    }
}

/// The resolved query actually applies `resolve_module_imports`: the same file
/// flags `imports_unresolved` with empty search paths, but not once the module
/// is on the search path.
#[test]
fn resolved_query_applies_import_resolution() {
    let db = EventDb::default();
    let workspace = empty_workspace(&db);
    let config = default_config(&db);
    let dir = make_tmp_dir("bsk_resolved_applies");
    fs::write(dir.join(format!("{PROBE}.py")), "x = 1\n").unwrap();
    let file = SourceFile::new(&db, "main.py".to_owned(), format!("import {PROBE}\n"));

    let empty = SearchPathsInput::new(&db, make_search_paths(vec![]));
    let empty_diags = file_diagnostics_resolved(&db, file, config, empty, workspace);
    assert!(
        empty_diags
            .iter()
            .any(|d| d.code.code == "imports_unresolved"),
        "with no search paths, `import {PROBE}` is unresolved and must flag imports_unresolved"
    );

    let rooted = SearchPathsInput::new(&db, make_search_paths(vec![dir.clone()]));
    let rooted_diags = file_diagnostics_resolved(&db, file, config, rooted, workspace);
    assert!(
        !rooted_diags
            .iter()
            .any(|d| d.code.code == "imports_unresolved"),
        "with the module on the search path, the import resolves and imports_unresolved must NOT fire"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// Editing the `SearchPathsInput` invalidates and re-runs the query exactly
/// once, and the new diagnostics reflect the new resolution environment.
#[test]
fn editing_search_paths_input_invalidates_resolved_query() {
    let mut db = EventDb::default();
    let workspace = empty_workspace(&db);
    let config = default_config(&db);
    let dir = make_tmp_dir("bsk_resolved_invalidate");
    fs::write(dir.join(format!("{PROBE}.py")), "x = 1\n").unwrap();
    let file = SourceFile::new(&db, "main.py".to_owned(), format!("import {PROBE}\n"));
    let search_paths = SearchPathsInput::new(&db, make_search_paths(vec![]));

    let _first = checked_file_resolved(&db, file, config, search_paths, workspace);
    let _ = db.executions_of("checked_file_resolved"); // drain priming

    // Add the dir to the search path — the probe module now resolves.
    let _previous = search_paths
        .set_value(&mut db)
        .to(make_search_paths(vec![dir.clone()]));
    let after = file_diagnostics_resolved(&db, file, config, search_paths, workspace);
    assert_eq!(
        db.executions_of("checked_file_resolved"),
        1,
        "editing the search-paths input must re-execute the query exactly once"
    );
    assert!(
        !after.iter().any(|d| d.code.code == "imports_unresolved"),
        "after adding the dir to the search path, imports_unresolved must clear"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// Editing the `ConfigInput` invalidates the resolved query too (it reads config).
#[test]
fn editing_config_invalidates_resolved_query() {
    let mut db = EventDb::default();
    let workspace = empty_workspace(&db);
    let search_paths = SearchPathsInput::new(&db, make_search_paths(vec![]));
    let config = ConfigInput::new(&db, ConfigValue(BasiliskConfig::default()));
    let file = SourceFile::new(
        &db,
        "f.py".to_owned(),
        "def f(x):\n    return x\n".to_owned(),
    );

    let _first = checked_file_resolved(&db, file, config, search_paths, workspace);
    let _ = db.executions_of("checked_file_resolved"); // drain priming

    let _previous = config.set_value(&mut db).to(ConfigValue(e0001_config()));
    let after = file_diagnostics_resolved(&db, file, config, search_paths, workspace);
    assert_eq!(
        db.executions_of("checked_file_resolved"),
        1,
        "editing the config input must re-execute the resolved query exactly once"
    );
    assert!(
        after.iter().any(|d| d.code.code == "BSK-E0001"),
        "an explicit severity must surface BSK-E0001 in the resolved query"
    );
}

/// The resolved query memoizes an unchanged input and re-runs on a source edit.
#[test]
fn resolved_query_memoizes_then_invalidates_on_source_edit() {
    let mut db = EventDb::default();
    let workspace = empty_workspace(&db);
    let config = default_config(&db);
    let search_paths = SearchPathsInput::new(&db, make_search_paths(vec![]));
    let file = SourceFile::new(&db, "a.py".to_owned(), "x: int = 1\n".to_owned());

    let _first = checked_file_resolved(&db, file, config, search_paths, workspace);
    assert_eq!(
        db.executions_of("checked_file_resolved"),
        1,
        "first check executes once"
    );

    let _second = checked_file_resolved(&db, file, config, search_paths, workspace);
    assert_eq!(
        db.executions_of("checked_file_resolved"),
        0,
        "an unchanged (file, config, search_paths, workspace) triple is served from the memo"
    );

    let _previous = file.set_text(&mut db).to("x: int = \"oops\"\n".to_owned());
    let _third = checked_file_resolved(&db, file, config, search_paths, workspace);
    assert_eq!(
        db.executions_of("checked_file_resolved"),
        1,
        "editing the source re-executes the query exactly once"
    );
}

/// A file that fails to parse yields no diagnostics through the resolved query —
/// identical to the batch CLI, which skips such files.
#[test]
fn resolved_query_unparseable_file_yields_no_diagnostics() {
    let db = EventDb::default();
    let workspace = empty_workspace(&db);
    let config = default_config(&db);
    let search_paths = SearchPathsInput::new(&db, make_search_paths(vec![]));
    let file = SourceFile::new(&db, "broken.py".to_owned(), "def (= :\n".to_owned());

    assert!(
        file_diagnostics_resolved(&db, file, config, search_paths, workspace).is_empty(),
        "an unparseable file must yield no diagnostics"
    );
    assert!(
        checked_file_resolved(&db, file, config, search_paths, workspace).is_empty(),
        "the memoized projection is empty too"
    );
}

/// `resolved_module` returns the import-resolved module for navigation.
#[test]
fn resolved_module_resolves_imports() {
    let db = EventDb::default();
    let workspace = empty_workspace(&db);
    let dir = make_tmp_dir("bsk_rm_resolves");
    fs::write(dir.join(format!("{PROBE}.py")), "x = 1\n").unwrap();
    let file = SourceFile::new(&db, "main.py".to_owned(), format!("import {PROBE}\n"));
    let search_paths = SearchPathsInput::new(&db, make_search_paths(vec![dir.clone()]));

    let ResolvedFile::Resolved(module) = resolved_module(&db, file, search_paths, workspace) else {
        panic!("the file parses and resolves");
    };
    let import = module
        .imports
        .iter()
        .find(|i| i.module == PROBE)
        .expect("the import is present");
    assert_ne!(
        import.resolution,
        ImportResolution::Unresolved,
        "resolve_module_imports must have resolved the sibling import in the navigable module"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// `resolved_module` yields `None` for a file that fails to parse.
#[test]
fn resolved_module_none_on_parse_error() {
    let db = EventDb::default();
    let workspace = empty_workspace(&db);
    let search_paths = SearchPathsInput::new(&db, make_search_paths(vec![]));
    let file = SourceFile::new(&db, "broken.py".to_owned(), "def (= :\n".to_owned());
    assert!(
        matches!(
            resolved_module(&db, file, search_paths, workspace),
            ResolvedFile::ParseError(_)
        ),
        "an unparseable file yields a ParseError outcome"
    );
}

/// A config-only edit re-runs the cheap `check`, but NOT `resolved_module`
/// (config is not one of its keys), so parse + resolve + import-resolution are
/// reused — the granularity win of splitting the two queries.
#[test]
fn config_edit_does_not_reresolve_module() {
    let mut db = EventDb::default();
    let workspace = empty_workspace(&db);
    let search_paths = SearchPathsInput::new(&db, make_search_paths(vec![]));
    let config = ConfigInput::new(&db, ConfigValue(BasiliskConfig::default()));
    let file = SourceFile::new(
        &db,
        "f.py".to_owned(),
        "def f(x):\n    return x\n".to_owned(),
    );

    let _first = checked_file_resolved(&db, file, config, search_paths, workspace);
    let _ = db.executions_of("resolved_module"); // drain priming
    let _ = db.executions_of("checked_file_resolved");

    let _previous = config.set_value(&mut db).to(ConfigValue(e0001_config()));
    let _after = checked_file_resolved(&db, file, config, search_paths, workspace);
    assert_eq!(
        db.executions_of("checked_file_resolved"),
        1,
        "a config edit re-runs the check step"
    );
    assert_eq!(
        db.executions_of("resolved_module"),
        0,
        "config is not a dependency of resolved_module — the resolved module is reused"
    );
}

/// Cross-file invalidation is **exports-precise**, not text-coarse:
/// `resolved_module(A)`'s own output does not depend on a non-stub imported
/// file's content (only on its existence, via the search paths), so editing
/// the imported `.py` must NOT re-execute A's parse+resolve — that would make
/// every dependency keystroke re-parse all importers. Content-dependence flows
/// through the exports-level `module_exports` edge in `cross_resolved_module`
/// (proven output-changing in `incremental_cross_tests.rs`), and the one place
/// a plain import's OUTPUT does depend on content — a user-stub `.pyi` API —
/// keeps its text edge (`editing_a_user_stub_updates_the_importer_diagnostics`).
#[test]
fn editing_a_non_stub_imported_file_does_not_reparse_the_importer() {
    let mut db = EventDb::default();
    let dir = make_tmp_dir("bsk_crossfile");
    let b_path = dir.join(format!("{PROBE}.py"));
    let c_path = dir.join("zzz_unrelated_probe.py");
    fs::write(&b_path, "x = 1\n").unwrap();
    fs::write(&c_path, "y = 1\n").unwrap();

    // A imports B (not C). B and C are workspace files with their own inputs.
    let a = SourceFile::new(
        &db,
        dir.join("a.py").to_string_lossy().into_owned(),
        format!("import {PROBE}\n"),
    );
    let b = SourceFile::new(
        &db,
        b_path.to_string_lossy().into_owned(),
        "x = 1\n".to_owned(),
    );
    let c = SourceFile::new(
        &db,
        c_path.to_string_lossy().into_owned(),
        "y = 1\n".to_owned(),
    );
    let search_paths = SearchPathsInput::new(&db, make_search_paths(vec![dir.clone()]));
    let workspace = WorkspaceFiles::new(
        &db,
        FileRegistry(HashMap::from([(b_path.clone(), b), (c_path.clone(), c)])),
    );

    let _first = resolved_module(&db, a, search_paths, workspace);
    let _ = db.executions_of("resolved_module"); // drain priming

    // Editing an unrelated file (A does not import C) must NOT re-run A.
    let _c_prev = c.set_text(&mut db).to("y = 2\n".to_owned());
    let _a_again = resolved_module(&db, a, search_paths, workspace);
    assert_eq!(
        db.executions_of("resolved_module"),
        0,
        "editing a file the importer does not import must not re-run its query"
    );

    // Editing the imported non-stub B must NOT re-run A's parse+resolve either:
    // A's resolved module is identical for any content of B, and re-executing
    // here is what would make a workspace sweep re-parse every importer.
    let _b_prev = b.set_text(&mut db).to("x = 2\n".to_owned());
    let _a_after = resolved_module(&db, a, search_paths, workspace);
    assert_eq!(
        db.executions_of("resolved_module"),
        0,
        "editing a non-stub imported file must not re-execute the importer's \
         resolved_module — its content only matters at the exports level"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// Cross-file invalidation that changes **output**: editing a workspace-tracked
/// user-stub `.pyi`'s content updates the importer's `imports_module_attribute`
/// diagnostics — proving the cross-file edge drives the result, not just
/// re-execution. The stub's `SourceFile` is edited (disk is left stale), so this
/// passes only because the query re-derives the stub API from the tracked text.
#[test]
fn editing_a_user_stub_updates_the_importer_diagnostics() {
    let mut db = EventDb::default();
    let stub_dir = make_tmp_dir("bsk_crossfile_stub");
    let x_pyi = stub_dir.join("xmod.pyi");
    fs::write(&x_pyi, "def foo() -> None: ...\n").unwrap();

    // A imports the user stub `xmod` and accesses `xmod.bar` (undeclared so far).
    let a = SourceFile::new(
        &db,
        "a.py".to_owned(),
        "import xmod\ny = xmod.bar\n".to_owned(),
    );
    let x = SourceFile::new(
        &db,
        x_pyi.to_string_lossy().into_owned(),
        "def foo() -> None: ...\n".to_owned(),
    );

    // The stub dir is a `stub-paths` entry, so `xmod` resolves as a user stub.
    let mut sp = make_search_paths(vec![]);
    sp.stub_paths = vec![stub_dir.clone()];
    let search_paths = SearchPathsInput::new(&db, sp);
    let config = default_config(&db);
    let workspace = WorkspaceFiles::new(&db, FileRegistry(HashMap::from([(x_pyi.clone(), x)])));

    let before = file_diagnostics_resolved(&db, a, config, search_paths, workspace);
    assert!(
        before
            .iter()
            .any(|d| d.code.code == "imports_module_attribute"),
        "accessing xmod.bar (undeclared in the stub) must flag imports_module_attribute"
    );

    // Edit the stub's SourceFile to declare `bar` (disk stays stale).
    let _prev = x
        .set_text(&mut db)
        .to("def foo() -> None: ...\ndef bar() -> None: ...\n".to_owned());

    let after = file_diagnostics_resolved(&db, a, config, search_paths, workspace);
    assert!(
        !after
            .iter()
            .any(|d| d.code.code == "imports_module_attribute"),
        "after the stub declares `bar`, the importer's imports_module_attribute must clear"
    );

    let _ = fs::remove_dir_all(&stub_dir);
}
