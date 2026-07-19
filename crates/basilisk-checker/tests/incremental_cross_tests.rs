//! Tests for [CHKARCH-INCREMENTAL-SALSA] cross-module queries. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA
#![allow(
    clippy::allow_attributes,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    missing_docs
)]
//! Behavioural tests for `module_exports` / `cross_resolved_module` /
//! `checked_file_cross`: cross-module `imported_symbols` population must follow
//! the import kind (`from` imports bind only named symbols; plain/star imports
//! publish the whole export set), prefer workspace-tracked content over disk,
//! respect PEP 561 (`py.typed` opt-in, stub provenance), and — the incremental
//! payoff — **backdate** body-only edits so importers are not re-checked, while
//! export-set edits propagate through the importer's diagnostics.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use basilisk_checker::{
    checked_file_cross, cross_resolved_module, file_diagnostics_cross, file_diagnostics_resolved,
    ConfigInput, ConfigValue, FileRegistry, ResolvedFile, SearchPathsInput, SourceFile,
    WorkspaceFiles,
};
use basilisk_config::BasiliskConfig;
use basilisk_resolver::scope::ExternalSymbolKind;
use basilisk_stubs::TypeProvenance;
use basilisk_test_utils::EventDb;
use salsa::Setter;

mod import_support;
use import_support::{custom_typeshed_snapshot, make_search_paths, make_tmp_dir};

fn default_config(db: &EventDb) -> ConfigInput {
    ConfigInput::new(db, ConfigValue(BasiliskConfig::default()))
}

/// A workspace of one importer plus tracked imported files: writes each file to
/// `dir` (so path-based import resolution succeeds), creates its `SourceFile`,
/// and returns the importer's file plus the registry-backed inputs.
fn tracked_workspace(
    db: &EventDb,
    dir: &Path,
    importer: (&str, &str),
    tracked: &[(&str, &str)],
) -> (
    SourceFile,
    SearchPathsInput,
    WorkspaceFiles,
    Vec<SourceFile>,
) {
    let (importer_name, importer_src) = importer;
    let importer_path = dir.join(importer_name);
    fs::write(&importer_path, importer_src).unwrap();
    let importer_file = SourceFile::new(
        db,
        importer_path.to_string_lossy().into_owned(),
        importer_src.to_owned(),
    );

    let mut registry: HashMap<PathBuf, SourceFile> = HashMap::new();
    let mut tracked_files = Vec::new();
    for (name, src) in tracked {
        let path = dir.join(name);
        fs::write(&path, src).unwrap();
        let file = SourceFile::new(db, path.to_string_lossy().into_owned(), (*src).to_owned());
        let _ = registry.insert(path, file);
        tracked_files.push(file);
    }

    let search_paths = SearchPathsInput::new(db, make_search_paths(vec![dir.to_path_buf()]));
    let workspace = WorkspaceFiles::new(db, FileRegistry(registry));
    (importer_file, search_paths, workspace, tracked_files)
}

/// The cross query's resolved module, or a panic when parse/resolve failed.
fn cross_module_of(
    db: &EventDb,
    file: SourceFile,
    search_paths: SearchPathsInput,
    workspace: WorkspaceFiles,
) -> std::sync::Arc<basilisk_resolver::ResolvedModule> {
    let ResolvedFile::Resolved(module) = cross_resolved_module(db, file, search_paths, workspace)
    else {
        panic!("the importer parses and resolves");
    };
    std::sync::Arc::clone(module)
}

/// `from lib import greet` binds ONLY the named symbols from the workspace-
/// tracked target — `other` is exported by lib but not imported.
#[test]
fn from_import_binds_only_named_symbols() {
    let db = EventDb::default();
    let dir = make_tmp_dir("bsk_cross_from");
    let (a, sp, ws, _) = tracked_workspace(
        &db,
        &dir,
        ("a.py", "from lib import greet\n"),
        &[(
            "lib.py",
            "def greet(name: str) -> str:\n    return name\n\ndef other() -> int:\n    return 1\n",
        )],
    );

    let module = cross_module_of(&db, a, sp, ws);
    let greet = module
        .imported_symbols
        .get("greet")
        .expect("`greet` must be populated from the workspace-tracked lib.py");
    assert_eq!(greet.kind, ExternalSymbolKind::Function);
    assert_eq!(greet.source_path, dir.join("lib.py"));
    assert_eq!(
        greet.provenance,
        Some(TypeProvenance::Source),
        "workspace-tracked exports carry Source provenance"
    );
    assert!(
        !module.imported_symbols.contains_key("other"),
        "a from-import must NOT bind symbols it does not name"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// A plain *aliased* import (`import lib as l`) publishes the whole export set
/// — the alias binds the module object, so the decision must key on `kind`,
/// never on `names` (regression guard ported from the LSP population pass).
#[test]
fn plain_aliased_import_publishes_all_exports() {
    let db = EventDb::default();
    let dir = make_tmp_dir("bsk_cross_alias");
    let (a, sp, ws, _) = tracked_workspace(
        &db,
        &dir,
        ("a.py", "import lib as l\n\nx: str = l.greet('world')\n"),
        &[(
            "lib.py",
            "def greet(name: str) -> str:\n    return name\n\nclass Dog:\n    name: str\n",
        )],
    );

    let module = cross_module_of(&db, a, sp, ws);
    assert!(
        module.imported_symbols.contains_key("greet"),
        "an aliased plain import must publish the module's exports, got keys: {:?}",
        module.imported_symbols.keys().collect::<Vec<_>>()
    );
    let dog = module
        .imported_symbols
        .get("Dog")
        .expect("classes are exported too");
    assert_eq!(dog.kind, ExternalSymbolKind::Class);

    let _ = fs::remove_dir_all(&dir);
}

/// An import resolving to an external `.pyi` stub (NOT workspace-tracked) is
/// parsed from disk and its symbols carry stub provenance and stub types.
#[test]
fn external_pyi_stub_symbols_carry_tier1_provenance() {
    let db = EventDb::default();
    let dir = make_tmp_dir("bsk_cross_stub");
    let stub_dir = dir.join("stubs");
    fs::create_dir_all(&stub_dir).unwrap();
    fs::write(
        stub_dir.join("acme.pyi"),
        "def fetch(url: str) -> bytes: ...\nVERSION: str\n",
    )
    .unwrap();

    let a = SourceFile::new(
        &db,
        dir.join("a.py").to_string_lossy().into_owned(),
        "from acme import fetch\n\nx = fetch('u')\n".to_owned(),
    );
    let mut sp_value = make_search_paths(vec![]);
    sp_value.stub_paths = vec![stub_dir];
    let sp = SearchPathsInput::new(&db, sp_value);
    let ws = WorkspaceFiles::new(&db, FileRegistry::default());

    let module = cross_module_of(&db, a, sp, ws);
    let fetch = module
        .imported_symbols
        .get("fetch")
        .expect("`fetch` from the external .pyi stub must be populated");
    assert_eq!(fetch.kind, ExternalSymbolKind::Function);
    assert_eq!(
        fetch.type_annotation.as_deref(),
        Some("bytes"),
        "stub return type must be picked up"
    );
    assert_eq!(
        fetch.provenance,
        Some(TypeProvenance::StubUser),
        "symbols from a configured user .pyi stub must carry StubUser provenance"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// [STUBRES-CUSTOM-TYPESHED] — the gated custom snapshot carried on the
/// **`SearchPathsInput`** must flow through `cross_resolved_module` into the
/// imported symbol's provenance, so hover reads `(custom typeshed)`.
///
/// This exercises the exact wire the LSP uses — `cross_resolved_module` reading
/// `search_paths.value(db).typeshed_snapshot` and resolving its logical VFS URI.
#[test]
fn custom_typeshed_provenance_threads_through_cross_query() {
    let db = EventDb::default();
    let dir = make_tmp_dir("bsk_cross_custom_ts");
    let a = SourceFile::new(
        &db,
        dir.join("a.py").to_string_lossy().into_owned(),
        "from os import uname\n\nname: str = uname()\n".to_owned(),
    );
    // The only step-3 authority is the promoted snapshot. No stub paths,
    // site-packages, or direct custom filesystem path participates.
    let mut sp_value = make_search_paths(vec![dir.clone()]);
    sp_value.typeshed_snapshot = Some(custom_typeshed_snapshot(&[(
        "os.pyi",
        "def uname() -> str: ...\n",
    )]));
    let sp = SearchPathsInput::new(&db, sp_value);
    let ws = WorkspaceFiles::new(&db, FileRegistry::default());

    let module = cross_module_of(&db, a, sp, ws);
    let uname = module
        .imported_symbols
        .get("uname")
        .expect("`from os import uname` must populate `uname` from the custom typeshed");

    // The import resolved to a stub UNDER the custom typeshed's stdlib/ — the
    // precondition for CustomTypeshed classification.
    assert!(
        uname
            .source_path
            .to_string_lossy()
            .starts_with("typeshed:custom-"),
        "the symbol must originate in the custom snapshot VFS, got {:?}",
        uname.source_path
    );
    assert_eq!(uname.kind, ExternalSymbolKind::Function);
    assert_eq!(
        uname.type_annotation.as_deref(),
        Some("str"),
        "the custom typeshed's `uname` return type must be read from the stub"
    );

    // The query resolved the logical URI from the active custom snapshot, so
    // provenance is CustomTypeshed — not bundled Tier1.
    assert_eq!(
        uname.provenance,
        Some(TypeProvenance::StubCustomTypeshed),
        "an active custom snapshot must thread through cross_resolved_module \
         into StubCustomTypeshed provenance [STUBRES-CUSTOM-TYPESHED]"
    );
    assert_ne!(
        uname.provenance,
        Some(TypeProvenance::StubTier1),
        "custom-typeshed provenance must not be downgraded to the bundled Tier1"
    );
    // …and that provenance renders the user-visible hover label distinctly.
    assert_eq!(
        uname.provenance.and_then(TypeProvenance::hover_label),
        Some("(custom typeshed)"),
        "hover for a custom-typeshed symbol must read '(custom typeshed)'"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// A third-party `.py` package with a PEP 561 `py.typed` marker has its inline
/// types consumed; a package WITHOUT the marker must be skipped (opt-in only).
#[test]
fn py_typed_marker_gates_inline_typed_packages() {
    let db = EventDb::default();
    let dir = make_tmp_dir("bsk_cross_pytyped");
    let sp_dir = dir.join("site-packages");
    let typed = sp_dir.join("widgets");
    let untyped = sp_dir.join("legacy");
    fs::create_dir_all(&typed).unwrap();
    fs::create_dir_all(&untyped).unwrap();
    fs::write(typed.join("py.typed"), "").unwrap();
    fs::write(
        typed.join("__init__.py"),
        "def render(node: int) -> str:\n    return str(node)\n",
    )
    .unwrap();
    fs::write(
        untyped.join("__init__.py"),
        "def helper(x: int) -> int:\n    return x\n",
    )
    .unwrap();

    let a = SourceFile::new(
        &db,
        dir.join("a.py").to_string_lossy().into_owned(),
        "from widgets import render\nfrom legacy import helper\n".to_owned(),
    );
    let mut sp_value = make_search_paths(vec![]);
    sp_value.site_packages = Some(sp_dir);
    let sp = SearchPathsInput::new(&db, sp_value);
    let ws = WorkspaceFiles::new(&db, FileRegistry::default());

    let module = cross_module_of(&db, a, sp, ws);
    let render = module
        .imported_symbols
        .get("render")
        .expect("`render` from the py.typed package must be populated");
    assert_eq!(
        render.type_annotation.as_deref(),
        Some("str"),
        "py.typed package's inline return type must be picked up"
    );
    assert!(
        !module.imported_symbols.contains_key("helper"),
        "symbols from a non-py.typed package must NOT be consumed (PEP 561)"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// The incremental payoff: a **body-only** edit to an imported file re-derives
/// its (unchanged) export set, salsa backdates it, and the importer's
/// cross-module diagnostics are NOT re-computed. An **export** edit does
/// propagate — and it propagates the *tracked* content, not the stale disk.
#[test]
fn body_edit_backdates_exports_and_export_edit_propagates() {
    let mut db = EventDb::default();
    let dir = make_tmp_dir("bsk_cross_backdate");
    let (a, sp, ws, tracked) = tracked_workspace(
        &db,
        &dir,
        (
            "a.py",
            "import lib\n\ndef use() -> int:\n    return thing\n",
        ),
        &[("lib.py", "def thing() -> int:\n    return 1\n")],
    );
    let lib = *tracked.first().expect("lib.py is tracked");
    let config = default_config(&db);

    let before = file_diagnostics_cross(&db, a, config, sp, ws);
    assert!(
        !before.iter().any(|d| d.code.code == "names_undefined"),
        "`thing` is exported by lib, so the importer must not flag it undefined"
    );
    let _ = db.executions_of("checked_file_cross"); // drain priming
    let _ = db.executions_of("cross_resolved_module");
    let _ = db.executions_of("module_exports");

    // Body-only edit: the export set is unchanged, so `module_exports` re-runs
    // but backdates, and the importer's cross queries are served from memo.
    let _prev = lib
        .set_text(&mut db)
        .to("def thing() -> int:\n    return 2\n".to_owned());
    let after_body = file_diagnostics_cross(&db, a, config, sp, ws);
    assert_eq!(
        db.executions_of("module_exports"),
        1,
        "the edited file's exports are re-derived once"
    );
    assert_eq!(
        db.executions_of("cross_resolved_module"),
        0,
        "a body-only edit must backdate: the importer's cross-module view is reused"
    );
    assert_eq!(
        db.executions_of("checked_file_cross"),
        0,
        "a body-only edit must not re-check the importer"
    );
    assert!(
        !after_body.iter().any(|d| d.code.code == "names_undefined"),
        "diagnostics are unchanged by a body-only edit"
    );

    // Export edit (rename `thing` away) IN MEMORY — disk still has `thing`.
    let _prev = lib
        .set_text(&mut db)
        .to("def renamed() -> int:\n    return 1\n".to_owned());
    let after_rename = file_diagnostics_cross(&db, a, config, sp, ws);
    assert!(
        after_rename
            .iter()
            .any(|d| d.code.code == "names_undefined" && d.message.contains("thing")),
        "renaming the export away must surface the importer's undefined reference \
         from the TRACKED text (disk is stale): {after_rename:?}"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// Sanity: for a file with no workspace imports, the cross query's diagnostics
/// equal the plain resolved query's — populating an empty `imported_symbols`
/// map must not perturb the result.
#[test]
fn cross_diagnostics_equal_resolved_without_workspace_imports() {
    let db = EventDb::default();
    let ws = WorkspaceFiles::new(&db, FileRegistry::default());
    let sp = SearchPathsInput::new(&db, make_search_paths(vec![]));
    let config = default_config(&db);
    let file = SourceFile::new(&db, "solo.py".to_owned(), "x: int = \"nope\"\n".to_owned());

    let cross = file_diagnostics_cross(&db, file, config, sp, ws);
    let plain = file_diagnostics_resolved(&db, file, config, sp, ws);
    assert_eq!(cross.len(), plain.len(), "diagnostic count must match");
    for (c, p) in cross.iter().zip(&plain) {
        assert_eq!(c.code.code, p.code.code);
        assert_eq!(c.span, p.span);
        assert_eq!(c.message, p.message);
    }
}

/// A parse error propagates through the cross query unchanged.
#[test]
fn cross_query_propagates_parse_error() {
    let db = EventDb::default();
    let ws = WorkspaceFiles::new(&db, FileRegistry::default());
    let sp = SearchPathsInput::new(&db, make_search_paths(vec![]));
    let file = SourceFile::new(&db, "broken.py".to_owned(), "def (= :\n".to_owned());
    assert!(
        matches!(
            cross_resolved_module(&db, file, sp, ws),
            ResolvedFile::ParseError(_)
        ),
        "an unparseable file yields a ParseError outcome through the cross query"
    );
    assert!(
        checked_file_cross(&db, file, default_config(&db), sp, ws).is_empty(),
        "and no diagnostics"
    );
}
