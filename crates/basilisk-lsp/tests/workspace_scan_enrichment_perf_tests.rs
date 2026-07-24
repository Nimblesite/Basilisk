//! Regression coverage for GitHub #304 ([ANALYSIS-INCR-IMPORTS]).
//!
//! The workspace scan re-parsed external `py.typed` package modules **once per
//! workspace file per imported name**: `populate_imported_symbols`
//! (`crates/basilisk-checker/src/exports.rs`) keeps its external-module cache
//! local to each call, and `chase_py_typed_reexport` restarts with a fresh
//! `visited` set per imported name while re-parsing every module it walks.
//! On a workspace importing large PEP 561 packages (sqlalchemy, pydantic —
//! the dify/api reproduction in the issue) the scan burns
//! `files × names × package-closure` parses and pins a CPU core for hours,
//! while `basilisk check` finishes the same tree in seconds.
//!
//! The test measures the same scan over the same external package with 2 and
//! then 20 workspace files. External-package work must be shared across
//! workspace files, so the 10× file count must NOT cost ~10× the time.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use basilisk_lsp::config::AnalysisMode;
use basilisk_lsp::workspace::WorkspaceIndex;

static TEST_CTR: AtomicU64 = AtomicU64::new(0);

/// Modules in the package's star-re-export chain (`__init__ -> mod_000 ->
/// … -> mod_039`, targets defined only in the last module).
const CHAIN_LEN: usize = 40;
/// Names each workspace file imports from the package.
const IMPORTED_NAMES: usize = 10;
/// Filler functions per chain module, so each parse has realistic cost.
const FILLER_FUNCS: usize = 150;

fn unique_tmp(prefix: &str) -> PathBuf {
    let sequence = TEST_CTR.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{prefix}_{sequence}_{}", std::process::id()))
}

/// Write a PEP 561 (`py.typed`) package whose public names are only reachable
/// by chasing a chain of `from .mod_NNN import *` re-exports — the pydantic /
/// sqlalchemy shape that triggers the GitHub #304 blow-up.
fn write_py_typed_package(site_packages: &Path) {
    let pkg = site_packages.join("bigpkg");
    std::fs::create_dir_all(&pkg).unwrap();
    std::fs::write(pkg.join("py.typed"), "").unwrap();
    std::fs::write(pkg.join("__init__.py"), "from .mod_000 import *\n").unwrap();

    for module_idx in 0..CHAIN_LEN {
        let mut text = String::new();
        if module_idx + 1 < CHAIN_LEN {
            let _ = writeln!(text, "from .mod_{:03} import *", module_idx + 1);
        } else {
            // Only the last chain module defines the imported names.
            for name_idx in 0..IMPORTED_NAMES {
                let _ = writeln!(
                    text,
                    "def target_{name_idx}(value: int) -> int:\n    return value\n"
                );
            }
        }
        for filler_idx in 0..FILLER_FUNCS {
            let _ = writeln!(
                text,
                "def filler_{module_idx}_{filler_idx}(left: int, right: str) -> int:\n    \
                 total = left + len(right)\n    return total\n"
            );
        }
        std::fs::write(pkg.join(format!("mod_{module_idx:03}.py")), text).unwrap();
    }
}

/// Write `file_count` workspace files that each import the chased names.
fn write_workspace_files(root: &Path, file_count: usize) {
    let names: Vec<String> = (0..IMPORTED_NAMES)
        .map(|name_idx| format!("target_{name_idx}"))
        .collect();
    let import_line = format!("from bigpkg import {}\n", names.join(", "));
    let mut uses = String::new();
    for (name_idx, name) in names.iter().enumerate() {
        let _ = writeln!(uses, "value_{name_idx} = {name}(1)");
    }
    for file_idx in 0..file_count {
        std::fs::write(
            root.join(format!("app_{file_idx:03}.py")),
            format!("{import_line}{uses}"),
        )
        .unwrap();
    }
}

/// Build a workspace with `file_count` files importing from the shared
/// `py.typed` package fixture, run the startup scan, and return its duration.
fn timed_scan(prefix: &str, file_count: usize) -> (Duration, WorkspaceIndex, PathBuf) {
    let root = unique_tmp(prefix);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("pyproject.toml"), "[tool.basilisk]\n").unwrap();
    let site_packages = root.join("site-packages");
    write_py_typed_package(&site_packages);
    write_workspace_files(&root, file_count);

    let roots = vec![root.clone()];
    let config = basilisk_lsp::config::load_config(&root);
    let index = WorkspaceIndex::new(
        roots.clone(),
        AnalysisMode::WholeModule,
        basilisk_config::BasiliskConfig::default(),
    );
    let mut search_paths =
        basilisk_lsp::import_resolver::search_paths_from_config(&roots, &config, None);
    search_paths.site_packages = Some(site_packages);
    index.set_search_paths(search_paths);

    let started = Instant::now();
    let (_diagnostics, scanned_count, _errors) = index.scan();
    let elapsed = started.elapsed();
    assert_eq!(
        scanned_count, file_count,
        "scan must analyse exactly the workspace files"
    );
    (elapsed, index, root)
}

/// GitHub #304: the startup scan must share external `py.typed` package
/// parsing across workspace files instead of re-chasing the package's
/// re-export closure per file per imported name.
#[test]
fn scan_shares_py_typed_package_parses_across_workspace_files_github_304() {
    let (small_elapsed, small_index, small_root) = timed_scan("bsk_scan_304_small", 2);
    let (big_elapsed, big_index, big_root) = timed_scan("bsk_scan_304_big", 20);

    // Guard: the enrichment path actually ran — the chased re-export resolved.
    for (index, root) in [(&small_index, &small_root), (&big_index, &big_root)] {
        let entry = index.files.get(&root.join("app_000.py")).unwrap();
        let resolved = entry
            .resolved
            .as_ref()
            .expect("scan must store the resolved module");
        assert!(
            resolved.imported_symbols.contains_key("target_0"),
            "py.typed re-export chase must populate imported_symbols \
             (otherwise this test is not exercising the #304 code path)"
        );
    }

    // External-package work must be shared across workspace files: 10× the
    // workspace files must not cost ~10× the scan time. The bound is deliberately
    // loose (4× + a 2-second absolute floor) so machine speed and build profile
    // don't matter; the unfixed code multiplies external parsing per file and
    // lands near 10×.
    let allowed = std::cmp::max(small_elapsed * 4, Duration::from_secs(2));
    assert!(
        big_elapsed < allowed,
        "workspace scan re-does external py.typed package parsing per workspace \
         file (GitHub #304): 2 files took {small_elapsed:?}, 20 files took \
         {big_elapsed:?} (allowed: {allowed:?}). External module parses must be \
         shared across files."
    );

    let _ = std::fs::remove_dir_all(small_root);
    let _ = std::fs::remove_dir_all(big_root);
}
