//! Tests for [LSPARCH-ARCH-STACK]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-STACK
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic
)]
//! A single generated file with a deeply chained expression must never take
//! the LSP server (or the CLI) down (GitHub #278).
//!
//! `ruff_python_parser` caps parenthesis and indentation nesting, but a long
//! binary-operator chain (`total = 1 + 1 + …`) parses fine and yields an
//! arbitrarily deep left-nested `BinOp` tree. The recursive resolver/checker
//! visitors then recurse once per level — on a default ~2 MB tokio worker
//! stack the workspace scan overflowed and aborted the whole server
//! (`thread 'tokio-rt-worker' has overflowed its stack`, exit `0xC00000FD`
//! on Windows), crash-looping VS Code's restart logic. The CLI had the same
//! exposure on the process main thread (~8 MiB on macOS/Linux, ~1 MiB on
//! Windows).
//!
//! These tests drive the REAL `basilisk` binary, because the fix lives in
//! the production entry points' thread/runtime construction — an in-process
//! fixture runs on the test's own runtime and cannot observe it.

mod lsp_stdio;

use std::process::Command;
use std::time::Duration;

use lsp_stdio::{unique_temp_dir, LspProcess};
use serde_json::json;

/// Terms in the generated chain. 10,000 is the confirmed real-binary repro
/// for issue #278: it overflows a default tokio worker stack during the
/// workspace scan, while remaining comfortably within the analysis stack
/// size mandated by [LSPARCH-ARCH-STACK].
const CHAIN_TERMS: usize = 10_000;

/// Terms that overflow the CLI's default main-thread stack (~8 MiB on
/// macOS/Linux): 20,000 aborts a debug build and 30,000 aborts release, so
/// 30,000 crashes every build flavour before the fix while staying well
/// inside the 64 MiB analysis stack.
const CLI_CHAIN_TERMS: usize = 30_000;

/// Write a workspace whose single file is an `n`-term `1 + 1 + …` chain and
/// return `(root, source)`.
fn chain_workspace(prefix: &str, n: usize) -> (std::path::PathBuf, String) {
    let root = unique_temp_dir(prefix);
    std::fs::create_dir_all(&root).expect("create workspace root");
    let chain = vec!["1"; n].join(" + ");
    let source = format!("total: int = {chain}\n");
    std::fs::write(root.join("generated.py"), &source).expect("write generated.py");
    (root, source)
}

#[test]
fn workspace_scan_survives_deeply_chained_binary_expression() {
    let (root, deep_source) = chain_workspace("bsk_deep_expr_ws", CHAIN_TERMS);

    let mut lsp = LspProcess::start_with(Some(&root), &json!(null));

    // The startup scan analyses generated.py. Before the fix the server
    // process died right here with a stack overflow, so stdout closed and no
    // scan-complete notification ever arrived.
    let scan_complete = lsp.wait_for_notification("basilisk/scanComplete", Duration::from_mins(2));
    assert_eq!(
        scan_complete["params"]["totalFiles"].as_u64(),
        Some(1),
        "scan must have analysed the generated file: {scan_complete}"
    );

    // The server must also survive the pathological file being OPENED — the
    // user's next click after the project loads — and stay responsive.
    let deep_uri = format!("file://{}/generated.py", root.to_string_lossy());
    lsp.did_open(&deep_uri, &deep_source);
    let hover = lsp.request(
        "textDocument/hover",
        &json!({
            "textDocument": { "uri": deep_uri },
            "position": { "line": 0, "character": 1 }
        }),
    );
    assert!(
        hover.get("contents").is_some(),
        "server must still answer hover on the deep file: {hover}"
    );

    drop(lsp);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cli_check_survives_deeply_chained_binary_expression() {
    let (root, _source) = chain_workspace("bsk_deep_expr_cli", CLI_CHAIN_TERMS);

    // `total: int = 1 + 1 + …` is well-typed, so a surviving checker exits 0
    // with no diagnostics. Before the fix the process aborted with a stack
    // overflow (SIGABRT — `status.code()` is None on Unix).
    let output = Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("check")
        .arg(&root)
        .output()
        .expect("run basilisk check");
    assert_eq!(
        output.status.code(),
        Some(0),
        "check must analyse the deep chain cleanly without crashing: status {:?}, stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// Terms beyond ANY stack: the measured 64 MiB crash floor is ~150,000 in
/// debug builds and ~210,000 in release, so 300,000 aborts every flavour
/// unless the parse-depth guard rejects the file first.
const UNCHECKABLE_CHAIN_TERMS: usize = 300_000;

// Tests [CHKARCH-ARCH-PARSEDEPTH] — see
// docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PARSEDEPTH.
#[test]
fn cli_check_rejects_pathological_expression_depth_instead_of_crashing() {
    let (root, _source) = chain_workspace("bsk_deep_expr_cap", UNCHECKABLE_CHAIN_TERMS);

    // Parity with the existing bracket/indent caps: the file is skipped with
    // a warning that explains why — matching CPython's own nesting rejections
    // — and the process must never abort.
    let output = Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("check")
        .arg(&root)
        .output()
        .expect("run basilisk check");
    assert_eq!(
        output.status.code(),
        Some(0),
        "the depth guard must skip the file like the bracket cap does, never crash: status {:?}",
        output.status
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("expression too deeply nested"),
        "the skip must be explained in the warning: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&root);
}
