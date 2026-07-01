//! Tests for [CHKARCH-INCREMENTAL-SALSA]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA
#![allow(
    clippy::allow_attributes,
    clippy::unwrap_used,
    clippy::expect_used,
    missing_docs
)]
//! Behavioural tests for the **import-resolved** salsa query
//! (`checked_file_resolved`): it must equal the CLI's import-resolving pipeline
//! byte-for-byte, actually apply import resolution (so `imports_unresolved`
//! reflects the search paths), and invalidate when the file, config, OR the
//! `SearchPathsInput` changes.

use std::fs;

use basilisk_checker::imports::ImportSearchPaths;
use basilisk_checker::{
    check_with_config, checked_file_resolved, file_diagnostics_resolved, ConfigInput, ConfigValue,
    Diagnostic, SearchPathsInput, SourceFile,
};
use basilisk_config::BasiliskConfig;
use basilisk_test_utils::EventDb;
use salsa::Setter;

mod import_support;
use import_support::{make_search_paths, make_tmp_dir};

/// A unique module name so an unresolved `import` never accidentally resolves
/// against the test process's working directory.
const PROBE: &str = "zzz_unique_import_probe";

fn default_config(db: &EventDb) -> ConfigInput {
    ConfigInput::new(db, ConfigValue(BasiliskConfig::default()))
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
        let got = file_diagnostics_resolved(&db, file, config, search_paths);
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
    let config = default_config(&db);
    let dir = make_tmp_dir("bsk_resolved_applies");
    fs::write(dir.join(format!("{PROBE}.py")), "x = 1\n").unwrap();
    let file = SourceFile::new(&db, "main.py".to_owned(), format!("import {PROBE}\n"));

    let empty = SearchPathsInput::new(&db, make_search_paths(vec![]));
    let empty_diags = file_diagnostics_resolved(&db, file, config, empty);
    assert!(
        empty_diags
            .iter()
            .any(|d| d.code.code == "imports_unresolved"),
        "with no search paths, `import {PROBE}` is unresolved and must flag imports_unresolved"
    );

    let rooted = SearchPathsInput::new(&db, make_search_paths(vec![dir.clone()]));
    let rooted_diags = file_diagnostics_resolved(&db, file, config, rooted);
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
    let config = default_config(&db);
    let dir = make_tmp_dir("bsk_resolved_invalidate");
    fs::write(dir.join(format!("{PROBE}.py")), "x = 1\n").unwrap();
    let file = SourceFile::new(&db, "main.py".to_owned(), format!("import {PROBE}\n"));
    let search_paths = SearchPathsInput::new(&db, make_search_paths(vec![]));

    let _first = checked_file_resolved(&db, file, config, search_paths);
    let _ = db.executions_of("checked_file_resolved"); // drain priming

    // Add the dir to the search path — the probe module now resolves.
    let _previous = search_paths
        .set_value(&mut db)
        .to(make_search_paths(vec![dir.clone()]));
    let after = file_diagnostics_resolved(&db, file, config, search_paths);
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
    let search_paths = SearchPathsInput::new(&db, make_search_paths(vec![]));
    let config = ConfigInput::new(&db, ConfigValue(BasiliskConfig::default()));
    let file = SourceFile::new(
        &db,
        "f.py".to_owned(),
        "def f(x):\n    return x\n".to_owned(),
    );

    let _first = checked_file_resolved(&db, file, config, search_paths);
    let _ = db.executions_of("checked_file_resolved"); // drain priming

    let _previous = config.set_value(&mut db).to(ConfigValue(BasiliskConfig {
        strict_annotations: true,
        ..BasiliskConfig::default()
    }));
    let after = file_diagnostics_resolved(&db, file, config, search_paths);
    assert_eq!(
        db.executions_of("checked_file_resolved"),
        1,
        "editing the config input must re-execute the resolved query exactly once"
    );
    assert!(
        after.iter().any(|d| d.code.code == "BSK-E0001"),
        "with strict_annotations the resolved query must surface BSK-E0001"
    );
}

/// The resolved query memoizes an unchanged input and re-runs on a source edit.
#[test]
fn resolved_query_memoizes_then_invalidates_on_source_edit() {
    let mut db = EventDb::default();
    let config = default_config(&db);
    let search_paths = SearchPathsInput::new(&db, make_search_paths(vec![]));
    let file = SourceFile::new(&db, "a.py".to_owned(), "x: int = 1\n".to_owned());

    let _first = checked_file_resolved(&db, file, config, search_paths);
    assert_eq!(
        db.executions_of("checked_file_resolved"),
        1,
        "first check executes once"
    );

    let _second = checked_file_resolved(&db, file, config, search_paths);
    assert_eq!(
        db.executions_of("checked_file_resolved"),
        0,
        "an unchanged (file, config, search_paths) triple is served from the memo"
    );

    let _previous = file.set_text(&mut db).to("x: int = \"oops\"\n".to_owned());
    let _third = checked_file_resolved(&db, file, config, search_paths);
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
    let config = default_config(&db);
    let search_paths = SearchPathsInput::new(&db, make_search_paths(vec![]));
    let file = SourceFile::new(&db, "broken.py".to_owned(), "def (= :\n".to_owned());

    assert!(
        file_diagnostics_resolved(&db, file, config, search_paths).is_empty(),
        "an unparseable file must yield no diagnostics"
    );
    assert!(
        checked_file_resolved(&db, file, config, search_paths).is_empty(),
        "the memoized projection is empty too"
    );
}
