//! LSP workspace coverage for [CHKCACHE-CONFIG] on the editor surface.
//! See docs/specs/CHECKER-CACHE-SPEC.md#CHKCACHE-CONFIG (GitHub #367).

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use std::sync::atomic::{AtomicU64, Ordering};

use basilisk_config::BasiliskConfig;
use basilisk_lsp::config::AnalysisMode;
use basilisk_lsp::workspace::WorkspaceIndex;

static TEST_CTR: AtomicU64 = AtomicU64::new(0);

fn unique_tmp(prefix: &str) -> std::path::PathBuf {
    let sequence = TEST_CTR.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{prefix}_{sequence}_{}", std::process::id()))
}

/// GitHub #367: `[tool.basilisk] cache = true` states the project's standing
/// cache policy ([CHKCACHE-CONFIG]) — a property of the project, not of which
/// surface happened to run. The editor's cold workspace scan is exactly the
/// "repeated batch run over a mostly-unchanged tree" the cache exists for,
/// yet the language server never consulted it: `.basilisk/cache/check` was
/// never created however often the window reloaded, while `basilisk check`
/// populated it from the same tree.
#[test]
fn cold_scan_with_cache_enabled_populates_the_persistent_cache() {
    let root = unique_tmp("bsk_lsp_persistent_cache_scan");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("pyproject.toml"),
        "[tool.basilisk]\ninclude = [\"src\"]\ncache = true\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src").join("app.py"),
        "def double(value: int) -> int:\n    return value * 2\n",
    )
    .unwrap();

    let idx = WorkspaceIndex::new(
        vec![root.clone()],
        AnalysisMode::WholeModule,
        BasiliskConfig::default(),
    );
    let (_results, file_count, _errors) = idx.scan();
    assert_eq!(file_count, 1, "the fixture's single source file must scan");

    let cache_dir = root.join(".basilisk").join("cache").join("check");
    let entries = std::fs::read_dir(&cache_dir).map_or(0, Iterator::count);
    assert_eq!(
        entries,
        1,
        "a cold scan of a `cache = true` project must store one persistent \
         entry per scanned file (GitHub #367); found {entries} entries in {}",
        cache_dir.display()
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// The read half of [CHKCACHE-LSP]: a second cold start (a fresh index — the
/// moment a reloaded VS Code window recreates the server) must REPLAY the
/// stored entry rather than re-check. Proven by poisoning the stored payload
/// with a sentinel diagnostic no checker rule would produce: if the sentinel
/// is published, the diagnostics came from the disk entry, not a fresh check.
#[test]
fn second_cold_scan_replays_the_stored_entry_instead_of_rechecking() {
    let root = unique_tmp("bsk_lsp_persistent_cache_replay");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("pyproject.toml"),
        "[tool.basilisk]\ninclude = [\"src\"]\ncache = true\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src").join("app.py"),
        "def double(value: int) -> int:\n    return value * 2\n",
    )
    .unwrap();

    // First session: populate the cache.
    let first = WorkspaceIndex::new(
        vec![root.clone()],
        AnalysisMode::WholeModule,
        BasiliskConfig::default(),
    );
    let _ = first.scan();
    drop(first);

    // Poison the stored payload with a sentinel diagnostic.
    let cache_dir = root.join(".basilisk").join("cache").join("check");
    let entry_path = std::fs::read_dir(&cache_dir)
        .expect("cache dir must exist after the first scan")
        .flatten()
        .map(|entry| entry.path())
        .next()
        .expect("one stored entry");
    let sentinel = basilisk_checker::CachedDiagnostic::from(&basilisk_checker::Diagnostic {
        code: basilisk_checker::ErrorCode {
            code: "BSK-0001",
            docs_url: "https://www.basilisk-python.dev/errors/BSK-0001",
        },
        severity: basilisk_checker::Severity::Error,
        message: "sentinel: replayed from the persistent cache".to_owned(),
        span: basilisk_resolver::Span::new(0, 3),
        path: root
            .join("src")
            .join("app.py")
            .to_string_lossy()
            .into_owned(),
        help: None,
        note: None,
        provenance: None,
    });
    let mut entry: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&entry_path).unwrap()).unwrap();
    let payload = entry
        .get_mut("payload")
        .expect("stored entries always carry a payload field");
    *payload = serde_json::to_value(vec![sentinel]).unwrap();
    std::fs::write(&entry_path, serde_json::to_string(&entry).unwrap()).unwrap();

    // Second session, fresh index: same bytes, same config — must replay.
    let second = WorkspaceIndex::new(
        vec![root.clone()],
        AnalysisMode::WholeModule,
        BasiliskConfig::default(),
    );
    let (results, _files, _errors) = second.scan();
    let published: Vec<String> = results
        .iter()
        .flat_map(|(_, diags)| diags.iter().map(|diag| diag.message.clone()))
        .collect();
    assert!(
        published
            .iter()
            .any(|message| message.contains("sentinel: replayed from the persistent cache")),
        "an unchanged file's second cold scan must replay the stored entry \
         (GitHub #367); published diagnostics were {published:?}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// Parity with `CHKCACHE-TEST-DISABLED`: without the `cache` key the scan
/// must not create the cache directory — behaviour stays byte-for-byte as
/// before the cache existed.
#[test]
fn scan_without_cache_key_creates_no_cache_directory() {
    let root = unique_tmp("bsk_lsp_persistent_cache_disabled");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("pyproject.toml"),
        "[tool.basilisk]\ninclude = [\"src\"]\n",
    )
    .unwrap();
    std::fs::write(root.join("src").join("app.py"), "value: int = 1\n").unwrap();

    let idx = WorkspaceIndex::new(
        vec![root.clone()],
        AnalysisMode::WholeModule,
        BasiliskConfig::default(),
    );
    let (_results, file_count, _errors) = idx.scan();
    assert_eq!(file_count, 1);
    assert!(
        !root.join(".basilisk").exists(),
        "an unconfigured project must never grow a .basilisk directory"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// [CHKCACHE-LSP] The cache must stay out of `crossModule` mode: replayed
/// entries carry CLI-parity diagnostics (they would drop cross-only
/// findings), and the cross queries share memos across importers, so a
/// per-file read recorder cannot capture a sound read-set to store.
#[test]
fn cross_module_scan_neither_stores_nor_replays() {
    let root = unique_tmp("bsk_lsp_persistent_cache_cross");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("pyproject.toml"),
        "[tool.basilisk]\ninclude = [\"src\"]\ncache = true\n",
    )
    .unwrap();
    std::fs::write(root.join("src").join("app.py"), "value: int = 1\n").unwrap();

    let idx = WorkspaceIndex::new(
        vec![root.clone()],
        AnalysisMode::CrossModule,
        BasiliskConfig::default(),
    );
    let (_results, file_count, _errors) = idx.scan();
    assert_eq!(file_count, 1);
    assert!(
        !root.join(".basilisk").exists(),
        "crossModule diagnostics are not CLI-parity and their read-sets are \
         not per-file capturable — the scan must not touch the cache"
    );

    let _ = std::fs::remove_dir_all(&root);
}
