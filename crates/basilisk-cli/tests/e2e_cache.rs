//! Tests for [CHKCACHE] / [CHKCACHE-TEST]. See docs/specs/CHECKER-CACHE-SPEC.md
#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic
)]
//! Coarse end-to-end tests for the opt-in result cache.
//!
//! Every test spawns the compiled `basilisk` binary, so it exercises the real
//! flag parsing, fingerprinting, read-set capture, on-disk entry, and replay.
//! The cardinal property under test is the [CHKCACHE-CONTRACT] guarantee: a hit
//! is returned only when nothing that affects the diagnostics has changed, so
//! the cache can never report a stale (wrong) result.

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

/// A throwaway directory unique to this process and call, holding both the
/// checked sources and the cache so tests never collide or touch the repo.
fn unique_dir(prefix: &str) -> PathBuf {
    static CTR: AtomicU64 = AtomicU64::new(0);
    let n = CTR.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("bsk_cache_{prefix}_{}_{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Write a `pyproject.toml` into `dir` opting into the annotation house rules
/// (`BSK-0001`/`BSK-0050` …), which are off by default — the default config is
/// pure PEP conformance. Tests that assert those diagnostics call this so they
/// see exactly what a user who enabled them would. No modes; this is
/// configuration. See [CHKARCH-CONFIGURATION-ONLY]. Callers use their own
/// unique dir and must not also write a `pyproject.toml` there (the config test
/// below writes its own instead of calling this).
fn opt_in_house_rules(dir: &std::path::Path) {
    std::fs::write(
        dir.join("pyproject.toml"),
        "[tool.basilisk.rules]\n\"BSK-0001\" = \"error\"\n\"BSK-0002\" = \"error\"\n",
    )
    .expect("write pyproject.toml");
}

/// Run `basilisk <subcommand> <target> --cache --cache-dir <cache> --cache-stats`.
///
/// `check` and `analyze` share the cache flags and pipeline; entries are
/// scope-free so both commands share them ([CHKARCH-COMMANDS]).
fn run_cached(subcommand: &str, target: &PathBuf, cache: &PathBuf) -> Output {
    Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg(subcommand)
        .arg(target)
        .arg("--cache")
        .arg("--cache-dir")
        .arg(cache)
        .arg("--cache-stats")
        .output()
        .expect("spawn basilisk")
}

/// Run `basilisk check` with the cache enabled.
fn check_cached(target: &PathBuf, cache: &PathBuf) -> Output {
    run_cached("check", target, cache)
}

/// Run `basilisk analyze` with the cache enabled — the command that renders
/// the opt-in house-rule diagnostics these fixtures produce
/// ([CHKARCH-COMMANDS]).
fn analyze_cached(target: &PathBuf, cache: &PathBuf) -> Output {
    run_cached("analyze", target, cache)
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Assert the `--cache-stats` line reports the expected hit/miss counts.
fn assert_stats(output: &Output, hits: usize, misses: usize) {
    let line = format!("cache: {hits} hit / {misses} miss");
    assert!(
        stderr(output).contains(&line),
        "expected `{line}` in stderr, got:\n{}",
        stderr(output)
    );
}

// ── CHKCACHE-TEST-HIT / CHKCACHE-TEST-STATS ─────────────────────────────────

/// A second run is a hit and replays byte-identical diagnostics.
#[test]
fn second_run_hits_with_identical_output() {
    let dir = unique_dir("hit");
    opt_in_house_rules(&dir);
    let target = dir.join("t.py");
    let cache = dir.join("cache");
    std::fs::write(&target, "def f(x):\n    return x\n").unwrap();

    let first = analyze_cached(&target, &cache);
    assert_stats(&first, 0, 1);
    let second = analyze_cached(&target, &cache);
    assert_stats(&second, 1, 0);

    assert_eq!(
        stdout(&first),
        stdout(&second),
        "a cache hit must replay byte-identical diagnostics"
    );
    assert!(
        stdout(&first).contains("BSK-0001"),
        "the fixture must produce a diagnostic to make the parity check meaningful"
    );
    assert_eq!(
        first.status.code(),
        second.status.code(),
        "exit code parity"
    );
}

// ── CHKCACHE-TEST-TARGET ────────────────────────────────────────────────────

/// Editing the target between runs yields a fresh result, never the stale one.
#[test]
fn editing_target_invalidates() {
    let dir = unique_dir("target");
    opt_in_house_rules(&dir);
    let target = dir.join("t.py");
    let cache = dir.join("cache");

    std::fs::write(&target, "def f(x):\n    return x\n").unwrap();
    let first = analyze_cached(&target, &cache);
    assert!(
        stdout(&first).contains("BSK-0001"),
        "first run reports error"
    );

    // Fix the error; the cached entry must NOT be served.
    std::fs::write(&target, "def f(x: int) -> int:\n    return x\n").unwrap();
    let second = analyze_cached(&target, &cache);
    assert_stats(&second, 0, 1);
    assert!(
        !stdout(&second).contains("BSK-0001"),
        "stale cached diagnostics must not survive a target edit:\n{}",
        stdout(&second)
    );
    assert_eq!(second.status.code(), Some(0), "fixed file must exit clean");
}

// ── CHKCACHE-TEST-DEP ───────────────────────────────────────────────────────

/// Editing an imported dependency invalidates the importer's cached result.
#[test]
fn editing_dependency_invalidates() {
    let dir = unique_dir("dep");
    let importer = dir.join("a.py");
    let dependency = dir.join("b.py");
    let cache = dir.join("cache");
    std::fs::write(
        &importer,
        "from b import helper\n\ndef use() -> int:\n    return helper()\n",
    )
    .unwrap();
    std::fs::write(&dependency, "def helper() -> int:\n    return 1\n").unwrap();

    assert_stats(&check_cached(&importer, &cache), 0, 1);
    assert_stats(&check_cached(&importer, &cache), 1, 0);

    // Touch the dependency: the importer must be re-checked, not served stale.
    std::fs::write(&dependency, "def helper() -> str:\n    return \"x\"\n").unwrap();
    assert_stats(&check_cached(&importer, &cache), 0, 1);
}

// ── CHKCACHE-TEST-CONFIG ────────────────────────────────────────────────────

/// A config change forces a miss even when every source file is unchanged.
#[test]
fn changing_config_invalidates() {
    let dir = unique_dir("config");
    let target = dir.join("m.py");
    let cache = dir.join("cache");
    let pyproject = dir.join("pyproject.toml");
    std::fs::write(&target, "x: int = 42\n").unwrap();
    std::fs::write(
        &pyproject,
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk.rules]\n\"BSK-0050\" = \"warning\"\n",
    )
    .unwrap();

    assert_stats(&check_cached(&target, &cache), 0, 1);
    assert_stats(&check_cached(&target, &cache), 1, 0);

    // Same source, different config: the fingerprint must differ → miss.
    std::fs::write(
        &pyproject,
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk.rules]\n\"BSK-0050\" = \"error\"\n",
    )
    .unwrap();
    assert_stats(&check_cached(&target, &cache), 0, 1);
}

// ── CHKCACHE-TEST-DISABLED ──────────────────────────────────────────────────

/// Without `--cache`, no cache directory is created and output is unchanged.
// Exercises [CHKCACHE-CLI]
#[test]
fn disabled_creates_no_cache_dir() {
    let dir = unique_dir("disabled");
    opt_in_house_rules(&dir);
    let target = dir.join("t.py");
    std::fs::write(&target, "def f(x):\n    return x\n").unwrap();

    let plain = Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("analyze")
        .arg(&target)
        .output()
        .expect("spawn basilisk");

    assert!(
        !dir.join(".basilisk").exists(),
        "no cache directory may be created without --cache"
    );
    assert!(
        stdout(&plain).contains("BSK-0001"),
        "plain analyze output must be unchanged"
    );
    assert!(
        !stderr(&plain).contains("cache:"),
        "no cache stats without --cache-stats"
    );
}
