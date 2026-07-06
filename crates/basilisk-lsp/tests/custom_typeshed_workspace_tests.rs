//! LSP workspace coverage for [STUBRES-CUSTOM-TYPESHED].
//! See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CUSTOM-TYPESHED

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use std::sync::atomic::{AtomicU64, Ordering};

use basilisk_lsp::config::AnalysisMode;
use basilisk_lsp::workspace::WorkspaceIndex;
use tower_lsp::lsp_types::{NumberOrString, Url};

static TEST_CTR: AtomicU64 = AtomicU64::new(0);

fn unique_tmp(prefix: &str) -> std::path::PathBuf {
    let sequence = TEST_CTR.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{prefix}_{sequence}_{}", std::process::id()))
}

fn has_imports_unresolved(diags: &[tower_lsp::lsp_types::Diagnostic]) -> bool {
    diags.iter().any(|diag| {
        matches!(
            &diag.code,
            Some(NumberOrString::String(code)) if code == "imports_unresolved"
        )
    })
}

#[test]
fn lsp_threads_custom_typeshed_into_imported_symbols() {
    let root = unique_tmp("bsk_lsp_custom_typeshed_symbols");
    let typeshed = root.join("typeshed-mp");
    let stdlib = typeshed.join("stdlib");
    std::fs::create_dir_all(&stdlib).unwrap();
    std::fs::write(
        stdlib.join("os.pyi"),
        "def uname() -> tuple[str, str, str, str, str]: ...\n",
    )
    .unwrap();
    std::fs::write(
        root.join("pyproject.toml"),
        "[tool.basilisk]\nanalysis-mode = \"crossModule\"\ntypeshed-path = \"typeshed-mp\"\n",
    )
    .unwrap();

    let main_path = root.join("main.py");
    let source = "from os import uname\nvalue = uname()\n";
    std::fs::write(&main_path, source).unwrap();

    let roots = vec![root.clone()];
    let config = basilisk_lsp::config::load_config(&root);
    let idx = WorkspaceIndex::new(
        roots.clone(),
        AnalysisMode::CrossModule,
        basilisk_config::BasiliskConfig::default(),
    );
    let search_paths =
        basilisk_lsp::import_resolver::search_paths_from_config(&roots, &config, None);
    assert_eq!(
        search_paths.typeshed_path,
        Some(typeshed.clone()),
        "LSP config must resolve relative typeshed-path against the workspace root"
    );
    idx.set_search_paths(search_paths);

    let uri = Url::from_file_path(&main_path).unwrap();
    let published = idx.set_open(&uri, source, 1);
    assert!(
        !has_imports_unresolved(&published),
        "custom typeshed stdlib import should resolve in published LSP diagnostics: {published:?}"
    );

    let entry = idx.files.get(&main_path).unwrap();
    let resolved = entry
        .resolved
        .as_ref()
        .expect("LSP must store the import-resolved module for navigation");
    let uname = resolved
        .imported_symbols
        .get("uname")
        .expect("custom typeshed export must populate imported_symbols");
    assert_eq!(
        uname.provenance,
        Some(basilisk_stubs::TypeProvenance::StubCustomTypeshed),
        "LSP cross-module symbol population must preserve custom-typeshed provenance"
    );
    assert!(
        uname.source_path.starts_with(stdlib),
        "imported symbol should come from the custom typeshed stdlib stub, got {:?}",
        uname.source_path
    );

    let _ = std::fs::remove_dir_all(&root);
}
